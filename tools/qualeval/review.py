"""Private local human-review CLI. Never print real benchmark rows through a remote agent.

No model inference, downloads, server, or API grading. `next` displays private text for a local human;
use `status` / `next --metadata-only` in agent sessions. Pairwise preferences do not certify semantics.
"""
import argparse
import json
import os
from pathlib import Path
import secrets
import sys
import tomllib
import unicodedata

import qualeval as q
import fresh_eval
from review_pilot import RUBRIC, build, latest, SEVERITIES, PREFERENCES, status, summary, validate_vote
from review_store import answers, append_answer, create_bundle, digest, load_bundle, read_json, require, sha, unique_object


def fail(code, message):
    print(json.dumps({'ok': False, 'error': {'code': code, 'message': message}}))
    raise SystemExit(2)


class Parser(argparse.ArgumentParser):
    def error(self, message):
        fail('invalid_arguments', message)


def outputs(path):
    result = {}
    for line in path.read_text().splitlines():
        row = json.loads(line, object_pairs_hook=unique_object)
        require(set(row) == {'id', 'out'} and isinstance(row['out'], str) and row['out'], 'invalid output row')
        require(row['id'] not in result, 'duplicate system-output identity')
        result[row['id']] = row['out']
    return result


def prepare(args):
    cfg = tomllib.loads((q.ROOT / 'difftest.local.toml').read_text())['qualeval']
    items, source = q.load_items(cfg, args.lang)
    directory = q.OUT / args.lang
    scores = read_json(directory / 'scores.json')['metadata']
    require(source['source_sha256'] == scores['source_sha256'] and source['source_revision'] == scores['source_revision'], 'benchmark source differs from scored snapshot')
    systems = {name: outputs(directory / f'{name}.jsonl') for name in ('nrmlz', 'nemo', 'verbalize')}
    require(all(q.digest(out) == scores['system_outputs_sha256'][name] for name, out in systems.items()), 'candidate outputs differ from scored snapshot')
    # Reuse the existing cache-key contract, but never enable or configure its API path.
    label, version = q.run_judge({'judge_model': scores['judge_model'], 'judge_enabled': False}, args.lang, items, systems, directory)
    require(version == scores['judge_prompt_sha256'], 'cached prompt contract changed')
    labels = {i['id']: 'auto_exact' if q.canon(systems['nrmlz'][i['id']]) == q.canon(i['ref']) else label(i, systems['nrmlz'][i['id']]) for i in items}
    provenance = dict(scores, source=source, scores_sha256=sha((directory / 'scores.json').read_bytes()),
                      cache_sha256=sha((directory / 'judgments-v2.jsonl').read_bytes()),
                      preparation_revision=q.subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=q.ROOT, text=True).strip())
    bundle, key = build(items, systems, labels, secrets.token_hex(32), provenance,
                        args.expected_pending, args.controls, args.repeats)
    hashes = create_bundle(args.out, {'review.private.json': bundle, 'key.private.json': key})
    return {'cases': args.expected_pending + 3 * args.controls, 'pairs': len(bundle['tasks']),
            'hidden_repeats': args.repeats, 'human_labels': 0, 'hashes': hashes,
            'population_estimate': False, 'external_requests': 0}


def state_directory(args, bundle):
    default = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state'))) / 'nrmlz/reviews' / digest(bundle)
    path = (getattr(args, 'state', None) or default).expanduser()
    resolved, root = path.resolve(), q.ROOT.resolve()
    require(not resolved.is_relative_to(args.bundle.resolve()), 'mutable annotations must stay outside immutable bundles')
    require(not resolved.is_relative_to(root) or resolved.is_relative_to(root / 'target'), 'annotation state must stay outside tracked repository paths')
    return path


def pilot(directory):
    documents = load_bundle(directory)
    require(set(documents) == {'review.private.json', 'key.private.json'}, 'not a pilot bundle')
    bundle, key = documents['review.private.json'], documents['key.private.json']
    require(bundle['schema'] == 'nrmlz.blind-pilot.v1' and key['schema'] == 'nrmlz.blind-pilot-key.v1', 'unknown pilot schema')
    require(digest(bundle) == key['bundle_sha256'], 'blind key does not match review bundle')
    require(len({t['id'] for t in bundle['tasks']}) == len(bundle['tasks']) and {t['id'] for t in bundle['tasks']} == set(key['tasks']), 'pilot task/key mismatch')
    return bundle, key


def next_task(args):
    bundle, key = pilot(args.bundle)
    state = answers(state_directory(args, bundle))
    progress = status(bundle, key, state, args.reviewer)
    records = latest(state, digest(bundle))
    task = next((t for t in bundle['tasks'] if (args.reviewer, t['id']) not in records), None)
    if args.metadata_only or task is None:
        return dict(progress, next_task=task['id'] if task else None)
    return dict(progress, task=task, rubric=bundle['rubric'])


def vote(args):
    bundle, key = pilot(args.bundle)
    task = next((t for t in bundle['tasks'] if t['id'] == args.task), None)
    require(task is not None, 'unknown task')
    event = validate_vote(args.reviewer, args.kind, args.task, args.a, args.b, args.preference, args.note)
    if task['a'] == task['b']:
        require(args.a == args.b and args.preference in {'tie', 'both_bad', 'uncertain'}, 'identical candidates require consistent verdicts')
    event['bundle_sha256'] = digest(bundle)
    revision = append_answer(state_directory(args, bundle), event, args.expected_revision, args.replace)
    return {'revision': revision, 'recorded': True, 'self_declared_kind': args.kind, 'human_certified': False}


def report(args):
    bundle, key = pilot(args.bundle)
    state = answers(state_directory(args, bundle))
    return summary(bundle, key, state, args.reviewer) if args.unblind else status(bundle, key, state, args.reviewer)


def safe_text(value):
    # Escape terminal control codes and newlines without changing stored/source text.
    encoded = json.dumps(value, ensure_ascii=False)
    return ''.join(f'\\u{ord(c):04x}' if unicodedata.category(c) in {'Cf', 'Cs'} else c for c in encoded)


def display_task(result):
    task = result['task']
    print(f"Task {task['id']} | revision {result['revision']} | remaining {result['remaining']}")
    for label, field in [('Input', 'input'), ('A', 'a'), ('B', 'b')]:
        print(label + ':', safe_text(task[field]))


def interactive(args):
    require(sys.stdin.isatty(), 'interactive review needs a local terminal; agents use status/metadata-only')
    require(args.max_pairs > 0, 'max-pairs must be positive')
    print(json.dumps(RUBRIC, ensure_ascii=False, indent=2))
    print('Enter A-severity B-severity preference, e.g. c a a. Severities: c/a/r/u/?. Preferences: a/b/t/x/?. q quits.')
    names = {'c': 'correct', 'a': 'acceptable', 'r': 'recoverable', 'u': 'unrecoverable', '?': 'uncertain'}
    prefs = {'a': 'a', 'b': 'b', 't': 'tie', 'x': 'both_bad', '?': 'uncertain'}
    completed = 0
    while completed < args.max_pairs:
        result = next_task(argparse.Namespace(bundle=args.bundle, state=args.state, reviewer=args.reviewer, metadata_only=False))
        if 'task' not in result:
            return result
        display_task(result)
        try:
            raw = input('A B preference > ').strip()
            if raw.lower() == 'q':
                break
            parts = raw.split()
            require(len(parts) == 3 and parts[0] in names and parts[1] in names and parts[2] in prefs, 'use three valid verdict codes; nothing saved')
            a, b = names[parts[0]], names[parts[1]]
            note = input('Reason (required for wrong meaning/uncertainty, otherwise optional) > ')
            vote(argparse.Namespace(bundle=args.bundle, state=args.state, reviewer=args.reviewer, kind=args.kind,
                 task=result['task']['id'], a=a, b=b, preference=prefs[parts[2]], note=note,
                 expected_revision=result['revision'], replace=False))
            completed += 1
        except (EOFError, KeyboardInterrupt):
            break
        except ValueError as exc:
            print(str(exc))
    return report(argparse.Namespace(bundle=args.bundle, state=args.state, reviewer=args.reviewer, unblind=False))


def main():
    parser = Parser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    prep = subs.add_parser('prepare', help='freeze the private calibration pilot from existing local outputs/cache')
    prep.add_argument('--lang', choices=['en', 'de'], default='en')
    prep.add_argument('--out', type=Path, required=True)
    prep.add_argument('--expected-pending', type=int, default=42)
    prep.add_argument('--controls', type=int, default=6, help='per auto-exact/cached-error/cached-other stratum')
    prep.add_argument('--repeats', type=int, default=6)
    prep.set_defaults(run=prepare)
    for command in ('next', 'vote', 'status', 'report'):
        item = subs.add_parser(command)
        item.add_argument('--bundle', type=Path, required=True)
        item.add_argument('--state', type=Path, help='private mutable state; defaults to XDG_STATE_HOME/nrmlz/reviews/<bundle hash>')
        item.add_argument('--reviewer', required=True, help='pseudonymous local reviewer identifier')
        item.set_defaults(run={'next': next_task, 'vote': vote, 'status': report, 'report': report}[command], unblind=False)
        if command == 'next':
            item.add_argument('--metadata-only', action='store_true', help='safe for agent sessions: no benchmark text')
            item.add_argument('--text', action='store_true', help='human terminal display; do not send to remote agents')
        if command == 'report':
            item.add_argument('--unblind', action='store_true', help='counts-only system comparison after a complete human batch')
        if command == 'vote':
            item.add_argument('--task', required=True)
            item.add_argument('--kind', choices=['human', 'agent'], required=True)
            item.add_argument('--a', choices=SEVERITIES, required=True)
            item.add_argument('--b', choices=SEVERITIES, required=True)
            item.add_argument('--preference', choices=PREFERENCES, required=True)
            item.add_argument('--note', default='')
            item.add_argument('--expected-revision', type=int, required=True)
            item.add_argument('--replace', action='store_true')
    run = subs.add_parser('run', help='local terminal review with automatic task/revision handling')
    run.add_argument('--bundle', type=Path, required=True)
    run.add_argument('--state', type=Path)
    run.add_argument('--reviewer', required=True)
    run.add_argument('--kind', choices=['human', 'agent'], required=True)
    run.add_argument('--max-pairs', type=int, default=126)
    run.set_defaults(run=interactive)
    plan = subs.add_parser('fresh-plan', help='freeze a predeclared source-family split plan (no texts)')
    plan.add_argument('--input', type=Path, required=True)
    plan.add_argument('--out', type=Path, required=True)
    plan.set_defaults(run=lambda a: fresh_eval.prepare_plan(a.input, a.out))
    fresh = subs.add_parser('freeze-inputs', help='freeze supplied fresh inputs before annotations/system inference')
    fresh.add_argument('--input', type=Path, required=True)
    fresh.add_argument('--plan', type=Path, required=True, help='previously frozen fresh-plan bundle')
    fresh.add_argument('--out', type=Path, required=True)
    fresh.add_argument('--deny', type=Path, action='append', default=[], help='private JSONL with text fields: reject known exact copies')
    fresh.set_defaults(run=lambda a: fresh_eval.prepare(a.input, a.plan, a.out, a.deny))
    args = parser.parse_args()
    try:
        result = args.run(args)
    except (ValueError, KeyError, TypeError) as exc:
        # Contract failures contain no source snippets; never print raw records or tracebacks.
        fail('contract_rejected', str(exc) if isinstance(exc, ValueError) else 'malformed input contract')
    except OSError:
        fail('io_error', 'local filesystem/source access failed')
    if getattr(args, 'text', False) and 'task' in result:
        display_task(result)
        print(json.dumps(result['rubric'], ensure_ascii=False, indent=2))
    else:
        print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
