"""Freeze independently supplied evaluation inputs before annotation or system inference. No network."""
from collections import Counter
import json
from pathlib import Path
import unicodedata

from review_store import create_bundle, digest, load_bundle, read_json, require, sha, unique_object

SPLITS = {'development', 'selection', 'holdout'}


def copy_key(text):
    return ' '.join(unicodedata.normalize('NFKC', text).casefold().split())


def read_rows(path):
    path = Path(path)
    require(path.is_file() and not path.is_symlink(), 'expected regular JSONL input')
    return [json.loads(line, object_pairs_hook=unique_object) for line in path.read_text().splitlines() if line.strip()]


def validate_plan(plan):
    require(isinstance(plan, dict) and set(plan) == {'schema', 'purpose', 'families', 'policy'}, 'fresh-plan schema mismatch')
    require(plan['schema'] == 'nrmlz.fresh-plan.v1', 'unknown fresh-plan schema')
    families = plan['families']
    require(isinstance(families, dict) and families and all(isinstance(k, str) and k for k in families) and all(isinstance(v, str) for v in families.values()) and set(families.values()) == SPLITS, 'predeclare families in all three partitions')
    require(plan['purpose'] == 'text-normalization-evaluation', 'missing evaluation purpose')
    require(isinstance(plan['policy'], dict) and plan['policy'].get('primary') == 'verified-semantic-errors' and plan['policy'].get('promotion') == 'manual-after-human-and-audio-validation', 'declare semantic-first manual promotion policy')


def prepare_plan(plan_path, output):
    plan = read_json(plan_path)
    validate_plan(plan)
    hashes = create_bundle(output, {'plan.private.json': plan})
    return {'families': len(plan['families']), 'hashes': hashes, 'inputs': 0, 'validation_eligible': False}


def freeze(rows, plan, denied=()):
    validate_plan(plan)
    families = plan['families']
    require(isinstance(rows, list) and bool(rows), 'fresh inputs must not be empty')
    denied_keys = {copy_key(text) for text in denied}
    ids, copies, sources, used = set(), set(), {}, set()
    result = []
    for row in rows:
        require(isinstance(row, dict) and set(row) == {'id', 'source_id', 'family_id', 'lang', 'category', 'text', 'provenance'}, 'fresh row schema mismatch; no expected answers or system outputs allowed')
        require(all(isinstance(row[k], str) and row[k].strip() for k in ('id', 'source_id', 'family_id', 'category', 'text')), 'missing row identity/context')
        require(row['lang'] in {'en', 'de'}, 'unsupported fresh locale')
        require(row['family_id'] in families, 'undeclared family')
        origin = row['provenance']
        require(isinstance(origin, dict) and set(origin) == {'author_kind', 'license', 'origin'}, 'declare authorship, license and provenance')
        require(origin['author_kind'] in {'human', 'published', 'synthetic', 'mixed'}, 'invalid authorship kind')
        require(all(isinstance(origin[k], str) and origin[k].strip() for k in ('license', 'origin')), 'missing source licence/provenance')
        require(row['id'] not in ids, 'duplicate row id')
        ids.add(row['id'])
        key = copy_key(row['text'])
        require(key not in copies and key not in denied_keys, 'exact/normalized source-copy leakage')
        copies.add(key)
        split = families[row['family_id']]
        require(row['source_id'] not in sources or sources[row['source_id']] == split, 'one source crosses partitions')
        sources[row['source_id']] = split
        used.add(row['family_id'])
        result.append(dict(row, split=split, text_sha256=sha(row['text'].encode()), annotation_complete=False))
    require(used == set(families), 'every predeclared family must have retained inputs')
    return {'schema': 'nrmlz.fresh-inputs.v1', 'plan_sha256': digest(plan), 'rows': result,
            'limitations': ['partitioning follows declared source/family identities, not independently verified families',
                           'normalized exact-copy rejection is not near-duplicate detection',
                           'authorship/licence declarations are not independent annotation or permission certification',
                           'no expected answers, human review, system inference or validation eligibility yet']}


def prepare(inputs, plan_path, output, deny_paths):
    documents = load_bundle(plan_path)
    require(set(documents) == {'plan.private.json'}, 'expected a frozen plan bundle')
    plan = documents['plan.private.json']
    denied = []
    hashes = {}
    for path in deny_paths:
        values = read_rows(path)
        require(all(isinstance(row.get('text'), str) and row['text'] for row in values), 'deny corpus needs nonempty text fields')
        denied += [row['text'] for row in values]
        hashes[str(len(hashes))] = sha(Path(path).read_bytes())
    frozen = freeze(read_rows(inputs), plan, denied)
    frozen['source_artifacts'] = {'inputs_sha256': sha(Path(inputs).read_bytes()), 'plan_manifest_sha256': sha((Path(plan_path) / 'manifest.json').read_bytes()), 'deny_sha256': hashes}
    create_bundle(output, {'fresh.private.json': frozen, 'plan.private.json': plan})
    return {'partitions': dict(Counter(row['split'] for row in frozen['rows'])), 'rows': len(frozen['rows']),
            'annotation_complete': 0, 'human_certified': False, 'validation_eligible': False,
            'authorship': dict(Counter(row['provenance']['author_kind'] for row in frozen['rows']))}
