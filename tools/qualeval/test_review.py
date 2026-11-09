"""Synthetic-only review/partition contracts. No real benchmark text, network or human labels."""
import copy
import json
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

import fresh_eval as fresh
import review
from review_pilot import build, digest, status, summary, validate_vote
from review_store import answers, append_answer, create_bundle, load_bundle, read_json


def fixture():
    items = [{'id': i, 'text': f'Synthetic quantity {i}.', 'ref': 'fixture only',
              'category': f'family-{i % 3}', 'split': 'dev' if i % 2 else 'test'} for i in range(8)]
    systems = {name: {i['id']: f'{word} fictional units.' for i in items}
               for name, word in [('nrmlz', 'five'), ('nemo', 'six'), ('verbalize', 'seven')]}
    labels = dict(enumerate(['unjudged', 'unjudged', 'auto_exact', 'auto_exact', 'unrecoverable', 'unrecoverable', 'acceptable', 'recoverable']))
    return items, systems, labels


def bundle_fixture():
    return build(*fixture(), 'test-seed', {'synthetic': True}, expected_pending=2, controls=1, repeats=2)


def event_for(task, mapping, kind='human', reviewer='fixture-reviewer'):
    verdicts = {name: ('correct' if name == 'nrmlz' else 'acceptable') for name in ('nrmlz', 'nemo', 'verbalize')}
    return validate_vote(reviewer, kind, task['id'], verdicts[mapping['a']], verdicts[mapping['b']],
                         'a' if mapping['a'] == 'nrmlz' else 'b', 'synthetic test only, not a real human judgment')


class Pilot(unittest.TestCase):
    def test_sampling_blinding_repeats_and_determinism(self):
        bundle, key = bundle_fixture()
        self.assertEqual(len(bundle['tasks']), 12)
        self.assertEqual(sum(v['repeat_of'] is not None for v in key['tasks'].values()), 2)
        self.assertEqual(len({v['source'] for v in key['tasks'].values()}), 5)
        public = json.dumps(bundle)
        for forbidden in ['nrmlz', 'nemo', 'verbalize', 'unjudged', 'cached_error', 'fixture only']:
            if forbidden == 'nrmlz':
                continue  # schema product name is not a candidate mapping
            self.assertNotIn(forbidden, public)
        self.assertEqual(bundle_fixture(), (bundle, key))

    def test_prepare_reuses_cache_without_any_api_call(self):
        items, systems, labels = fixture()
        for item in items:
            if labels[item['id']] == 'auto_exact':
                item['ref'] = systems['nrmlz'][item['id']]
        version = review.q.digest(['fixture-model', review.q.JUDGE_PROMPT])
        cache = []
        for item in items:
            label = labels[item['id']]
            if label not in {'auto_exact', 'unjudged'}:
                key = review.q.digest([version, 'en', item['text'], item['ref'], systems['nrmlz'][item['id']]])
                cache.append({'key': key, 'verdict': {'label': label}})
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / 'difftest.local.toml').write_text('[qualeval]\n')
            directory = root / 'eval' / 'en'
            directory.mkdir(parents=True)
            for name, output in systems.items():
                (directory / f'{name}.jsonl').write_text(''.join(json.dumps({'id': k, 'out': v}) + '\n' for k, v in output.items()))
            meta = {'source_sha256': 'synthetic', 'source_revision': 'synthetic-revision',
                    'judge_model': 'fixture-model', 'judge_prompt_sha256': version,
                    'system_outputs_sha256': {n: review.q.digest(o) for n, o in systems.items()}}
            (directory / 'scores.json').write_text(json.dumps({'metadata': meta}))
            (directory / 'judgments-v2.jsonl').write_text(''.join(json.dumps(r) + '\n' for r in cache))
            with patch.object(review.q, 'ROOT', root), patch.object(review.q, 'OUT', root / 'eval'), patch.object(review.q, 'load_items', return_value=(items, {'source_sha256': 'synthetic', 'source_revision': 'synthetic-revision'})), patch.object(review.q.subprocess, 'check_output', return_value='synthetic-revision\n'), patch.object(review.q, 'judge_batch', side_effect=AssertionError('no network')) as network:
                result = review.prepare(SimpleNamespace(lang='en', out=root / 'pilot', expected_pending=2, controls=1, repeats=2))
                network.assert_not_called()
                self.assertEqual(result['pairs'], 12)
                self.assertEqual(result['external_requests'], 0)
                self.assertEqual(answers(root / 'pilot')['events'], [])

    def test_changed_pending_pool_and_missing_outputs_fail(self):
        args = fixture()
        with self.assertRaises(ValueError):
            build(*args, 'seed', {}, expected_pending=3, controls=1, repeats=0)
        del args[1]['nemo'][0]
        with self.assertRaises(ValueError):
            build(*args, 'seed', {}, expected_pending=2, controls=1, repeats=0)

    def test_complete_human_batch_reports_unique_cases_and_repeats(self):
        bundle, key = bundle_fixture()
        state = {'revision': 0, 'events': []}
        with tempfile.TemporaryDirectory() as root:
            for task in bundle['tasks']:
                event = event_for(task, key['tasks'][task['id']])
                event['bundle_sha256'] = digest(bundle)
                append_answer(root, event, len(state['events']))
                state = answers(root)
            result = summary(bundle, key, state, 'fixture-reviewer')
        self.assertEqual(result['systems']['nrmlz']['labels'], {'correct': 5})
        self.assertEqual(result['systems']['nemo']['cases'], 5)
        self.assertEqual(result['repeats'], {'answered': 2, 'consistent': 2})
        self.assertEqual(result['conflicts'], 0)
        self.assertFalse(result['human_certified'])

    def test_identical_outputs_cannot_receive_different_system_credit(self):
        items, systems, labels = fixture()
        systems['nemo'] = dict(systems['nrmlz'])
        bundle, key = build(items, systems, labels, 'seed', {}, expected_pending=2, controls=1, repeats=0)
        events = []
        for task in bundle['tasks']:
            event = event_for(task, key['tasks'][task['id']])
            event['bundle_sha256'] = digest(bundle)
            events.append(event)
        result = summary(bundle, key, {'revision': len(events), 'events': events}, 'fixture-reviewer')
        self.assertEqual(result['systems']['nrmlz']['labels'], {'uncertain': 5})
        self.assertEqual(result['systems']['nemo']['labels'], {'uncertain': 5})

    def test_partial_or_agent_labels_never_certify(self):
        bundle, key = bundle_fixture()
        with self.assertRaises(ValueError):
            summary(bundle, key, {'revision': 0, 'events': []}, 'fixture-reviewer')
        events = []
        for task in bundle['tasks']:
            event = event_for(task, key['tasks'][task['id']], kind='agent')
            event['bundle_sha256'] = digest(bundle)
            events.append(event)
        with self.assertRaises(ValueError):
            summary(bundle, key, {'revision': len(events), 'events': events}, 'fixture-reviewer')

    def test_ambiguous_annotations_and_contradictory_preferences(self):
        with self.assertRaises(ValueError):
            validate_vote('r', 'human', 't', 'unrecoverable', 'correct', 'a', 'test')
        with self.assertRaises(ValueError):
            validate_vote('r', 'human', 't', 'uncertain', 'correct', 'uncertain', '')
        self.assertEqual(validate_vote('r', 'human', 't', 'uncertain', 'correct', 'uncertain', 'source ambiguous')['a'], 'uncertain')

    def test_cross_pair_conflicts_are_unknown_not_majority_correct(self):
        bundle, key = bundle_fixture()
        events = []
        changed = None
        for task in bundle['tasks']:
            mapping = key['tasks'][task['id']]
            event = event_for(task, mapping)
            if changed is None:
                side = 'a' if mapping['a'] == 'nrmlz' else 'b'
                event[side] = 'unrecoverable'
                event['preference'] = 'uncertain'
                changed = mapping['source']
            event['bundle_sha256'] = digest(bundle)
            events.append(event)
        result = summary(bundle, key, {'revision': len(events), 'events': events}, 'fixture-reviewer')
        self.assertGreaterEqual(result['conflicts'], 1)
        self.assertEqual(result['systems']['nrmlz']['labels']['uncertain'], 1)
        self.assertEqual(result['systems']['nrmlz']['unrecoverable_bounds'], [0.0, 0.2])


class Storage(unittest.TestCase):
    def test_private_no_clobber_and_tamper_rejection(self):
        bundle, key = bundle_fixture()
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / 'pilot'
            create_bundle(path, {'review.private.json': bundle, 'key.private.json': key})
            self.assertEqual(path.stat().st_mode & 0o777, 0o700)
            self.assertEqual((path / 'review.private.json').stat().st_mode & 0o777, 0o600)
            self.assertEqual(load_bundle(path)['review.private.json'], bundle)
            shutil.copytree(path, Path(root) / 'relocated')
            self.assertEqual(load_bundle(Path(root) / 'relocated'), load_bundle(path))
            with self.assertRaises(ValueError):
                create_bundle(path, {})
            (path / 'review.private.json').write_text('{}')
            with self.assertRaises(ValueError):
                load_bundle(path)

    def test_machine_errors_are_structured_without_tracebacks(self):
        script = Path(__file__).with_name('review.py')
        result = subprocess.run([sys.executable, str(script), 'vote', '--not-a-real-flag'], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(json.loads(result.stdout)['error']['code'], 'invalid_arguments')
        self.assertNotIn('Traceback', result.stderr)

    def test_local_interactive_flow_and_immutable_state_separation(self):
        bundle, key = bundle_fixture()
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            create_bundle(root / 'pilot', {'review.private.json': bundle, 'key.private.json': key})
            args = SimpleNamespace(bundle=root / 'pilot', state=root / 'state', reviewer='fixture', kind='agent', max_pairs=1)
            with patch.object(review.sys.stdin, 'isatty', return_value=True), patch('builtins.input', side_effect=['c c t', 'synthetic only']), patch('builtins.print'):
                result = review.interactive(args)
            self.assertEqual(result['answered'], 1)
            self.assertEqual(result['kind'], 'agent')
            self.assertFalse((root / 'pilot' / 'answers.json').exists())
            args.state = root / 'pilot' / 'nested-state'
            with self.assertRaises(ValueError):
                review.state_directory(args, bundle)
            with patch.object(review.sys.stdin, 'isatty', return_value=False), self.assertRaises(ValueError):
                review.interactive(args)

    def test_revision_corrections_audit_and_kind_binding(self):
        with tempfile.TemporaryDirectory() as root:
            event = dict(validate_vote('r', 'agent', 't', 'correct', 'correct', 'tie', ''), bundle_sha256='synthetic')
            self.assertEqual(append_answer(root, event, 0), 1)
            self.assertEqual(append_answer(root, event, 1), 1)
            changed = dict(event, a='acceptable', b='acceptable')
            with self.assertRaises(ValueError):
                append_answer(root, changed, 0, True)
            with self.assertRaises(ValueError):
                append_answer(root, changed, 1)
            self.assertEqual(append_answer(root, changed, 1, True), 2)
            with self.assertRaises(ValueError):
                append_answer(root, dict(changed, kind='human'), 2, True)
            with self.assertRaises(ValueError):
                append_answer(root, dict(changed, bundle_sha256='another-bundle'), 2, True)
            self.assertEqual(answers(root)['revision'], 2)
            state = read_json(Path(root) / 'answers.json')
            state['events'][0]['a'] = 'unrecoverable'
            (Path(root) / 'answers.json').write_text(json.dumps(state))
            with self.assertRaises(ValueError):
                answers(root)

    def test_symlink_and_busy_lock_fail(self):
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / 'answers.json').symlink_to(Path(root) / 'missing')
            with self.assertRaises(ValueError):
                answers(root)
            (Path(root) / 'answers.json').unlink()
            (Path(root) / '.annotation.lock').write_text('synthetic')
            with self.assertRaises(ValueError):
                append_answer(root, {}, 0)

    def test_cli_next_is_blind_and_control_safe(self):
        bundle, key = bundle_fixture()
        bundle['tasks'][0]['input'] += '\x1b[31m'
        key['bundle_sha256'] = digest(bundle)
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / 'pilot'
            create_bundle(path, {'review.private.json': bundle, 'key.private.json': key})
            script = Path(__file__).with_name('review.py')
            cmd = [sys.executable, str(script), 'next', '--bundle', str(path), '--state', str(Path(root) / 'state'), '--reviewer', 'r']
            metadata = subprocess.run(cmd + ['--metadata-only'], capture_output=True, text=True, check=True)
            self.assertNotIn('Synthetic quantity', metadata.stdout)
            visible = subprocess.run(cmd + ['--text'], capture_output=True, text=True, check=True)
            self.assertNotIn('\x1b', visible.stdout)
            self.assertIn('\\u001b', visible.stdout)
            self.assertNotIn('fixture only', visible.stdout)
            vote = [sys.executable, str(script), 'vote', '--bundle', str(path), '--state', str(Path(root) / 'state'), '--reviewer', 'r', '--kind', 'agent', '--task', bundle['tasks'][0]['id'], '--a', 'correct', '--b', 'correct', '--preference', 'tie', '--expected-revision', '0']
            recorded = subprocess.run(vote, capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(recorded.stdout)['revision'], 1)
            stale = subprocess.run(vote, capture_output=True, text=True)
            self.assertEqual(stale.returncode, 2)
            self.assertEqual(json.loads(stale.stdout)['error']['code'], 'contract_rejected')
            self.assertEqual(answers(Path(root) / 'state')['revision'], 1)
            self.assertFalse((path / 'answers.json').exists())


class Fresh(unittest.TestCase):
    def setUp(self):
        self.plan = {'schema': 'nrmlz.fresh-plan.v1', 'purpose': 'text-normalization-evaluation',
                     'families': {'a': 'development', 'b': 'selection', 'c': 'holdout'},
                     'policy': {'primary': 'verified-semantic-errors', 'promotion': 'manual-after-human-and-audio-validation'}}
        self.rows = [{'id': f'r{i}', 'source_id': f's{i}', 'family_id': family, 'lang': 'en',
                      'category': 'synthetic-fixture', 'text': f'Synthetic fresh quantity {i}.',
                      'provenance': {'author_kind': 'synthetic', 'license': 'test-only', 'origin': 'unit-test'}}
                     for i, family in enumerate('abc')]

    def test_freeze_is_unannotated_and_not_eligible(self):
        result = fresh.freeze(self.rows, self.plan)
        self.assertTrue(all(row['annotation_complete'] is False for row in result['rows']))
        self.assertEqual({row['split'] for row in result['rows']}, fresh.SPLITS)
        self.assertTrue(all('expected' not in row for row in result['rows']))

    def test_source_family_and_exact_copy_leakage_fail(self):
        bad = copy.deepcopy(self.rows)
        bad[1]['source_id'] = bad[0]['source_id']
        with self.assertRaises(ValueError):
            fresh.freeze(bad, self.plan)
        bad = copy.deepcopy(self.rows)
        bad[1]['text'] = '  ' + bad[0]['text'].upper() + '\n'
        with self.assertRaises(ValueError):
            fresh.freeze(bad, self.plan)
        with self.assertRaises(ValueError):
            fresh.freeze(self.rows, self.plan, [self.rows[0]['text']])

    def test_model_targets_unknown_sources_and_incomplete_partitions_fail(self):
        bad = copy.deepcopy(self.rows)
        bad[0]['expected'] = 'model output is not gold'
        with self.assertRaises(ValueError):
            fresh.freeze(bad, self.plan)
        with self.assertRaises(ValueError):
            fresh.freeze(self.rows[:-1], self.plan)
        bad = copy.deepcopy(self.rows)
        bad[0]['family_id'] = 'undeclared'
        with self.assertRaises(ValueError):
            fresh.freeze(bad, self.plan)

    def test_plan_must_be_frozen_and_hash_bound(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            plan_path, input_path = root / 'plan.json', root / 'inputs.jsonl'
            plan_path.write_text(json.dumps(self.plan))
            input_path.write_text(''.join(json.dumps(r) + '\n' for r in self.rows))
            fresh.prepare_plan(plan_path, root / 'frozen-plan')
            result = fresh.prepare(input_path, root / 'frozen-plan', root / 'frozen-inputs', [])
            self.assertEqual(result['annotation_complete'], 0)
            self.assertFalse(result['validation_eligible'])
            (root / 'frozen-plan' / 'plan.private.json').write_text('{}')
            with self.assertRaises(ValueError):
                fresh.prepare(input_path, root / 'frozen-plan', root / 'another', [])


if __name__ == '__main__':
    unittest.main()
