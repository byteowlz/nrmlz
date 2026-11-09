"""Blinded paired calibration, not a representative benchmark or certified human annotation."""
from collections import Counter, defaultdict
import random

from review_store import digest, require

SEVERITIES = ('correct', 'acceptable', 'recoverable', 'unrecoverable', 'uncertain')
PREFERENCES = ('a', 'b', 'tie', 'both_bad', 'uncertain')
RUBRIC = {
    'correct': 'Natural, all information correct; valid alternate readings count as correct.',
    'acceptable': 'All information correct, but awkward wording.',
    'recoverable': 'An error a listener can recover, such as an ending or known abbreviation.',
    'unrecoverable': 'Wrong/lost value, identifier character, word or meaning; invented content.',
    'uncertain': 'Ambiguous source, insufficient expertise or context; never a passing label.',
    'preference': 'Meaning first, then naturalness; a/b/tie/both_bad/uncertain. No reference answer is shown.',
}


def balanced(pool, count, rng):
    buckets = defaultdict(list)
    for item in sorted(pool, key=lambda i: str(i['id'])):
        buckets[item['category']].append(item)
    categories = sorted(buckets)
    rng.shuffle(categories)
    for values in buckets.values():
        rng.shuffle(values)
    result = []
    while len(result) < count:
        available = [category for category in categories if buckets[category]]
        require(available, 'sampling stratum has insufficient cases')
        for category in available:
            result.append(buckets[category].pop())
            if len(result) == count:
                break
    return result


def build(items, systems, labels, seed, provenance, expected_pending=42, controls=6, repeats=6):
    require(set(systems) == {'nrmlz', 'nemo', 'verbalize'}, 'pilot requires the three pinned systems')
    ids = {i['id'] for i in items}
    require(len(ids) == len(items) and len({str(i) for i in ids}) == len(items) and bool(items), 'empty or duplicate source identities')
    require(all(set(out) == ids and all(isinstance(v, str) and v for v in out.values()) for out in systems.values()), 'missing, extra or empty candidate outputs')
    require(set(labels) == ids and set(labels.values()) <= {'auto_exact', 'correct', 'acceptable', 'recoverable', 'unrecoverable', 'unjudged'}, 'sampling labels do not cover sources')
    require(type(expected_pending) is int and expected_pending >= 0 and type(controls) is int and controls > 0 and type(repeats) is int and repeats >= 0, 'invalid protocol counts')
    rng = random.Random(seed)
    pending = [i for i in items if labels[i['id']] == 'unjudged']
    require(len(pending) == expected_pending, 'unjudged pool changed; explicitly revise the protocol')
    selected = [(i, 'pending') for i in pending]
    pools = {
        'auto_exact': [i for i in items if labels[i['id']] == 'auto_exact'],
        'cached_error': [i for i in items if labels[i['id']] == 'unrecoverable'],
        'cached_other': [i for i in items if labels[i['id']] in {'correct', 'acceptable', 'recoverable'}],
    }
    for name, pool in pools.items():
        selected += [(i, name) for i in balanced(pool, controls, rng)]
    require(len({i['id'] for i, _ in selected}) == len(selected), 'sampling strata overlap')
    protocol = {'schema': 'nrmlz.blind-pilot.v1', 'pending': expected_pending, 'controls_per_stratum': controls,
                'repeat_pairs': repeats, 'comparisons': ['nrmlz/nemo', 'nrmlz/verbalize'],
                'sampling': 'all pending plus category-balanced cached controls; biased calibration only',
                'rubric': RUBRIC, 'reviewer_kind': 'self-declared, not identity verification or linguistic certification'}
    tasks, key = [], {}

    def add(item, stratum, baseline, repeat_of=None):
        identity = digest([seed, item['id'], baseline, repeat_of])[:24]
        task_id = 'p-' + identity
        order = ['nrmlz', baseline]
        rng.shuffle(order)
        tasks.append({'id': task_id, 'input': item['text'], 'a': systems[order[0]][item['id']], 'b': systems[order[1]][item['id']]})
        key[task_id] = {'source': str(item['id']), 'category': item['category'], 'split': item['split'],
                        'stratum': stratum, 'a': order[0], 'b': order[1], 'repeat_of': repeat_of}
        return task_id

    originals = []
    for item, stratum in selected:
        for baseline in ('nemo', 'verbalize'):
            task_id = add(item, stratum, baseline)
            originals.append((task_id, item, stratum, baseline))
    require(repeats <= len(originals), 'too many repeats')
    for task_id, item, stratum, baseline in rng.sample(originals, repeats):
        add(item, stratum, baseline, task_id)
    rng.shuffle(tasks)
    bundle = {'schema': 'nrmlz.blind-pilot.v1', 'rubric': RUBRIC, 'tasks': tasks}
    private = {'schema': 'nrmlz.blind-pilot-key.v1', 'bundle_sha256': digest(bundle), 'protocol': protocol,
               'seed': seed, 'provenance': provenance, 'tasks': key}
    return bundle, private


def latest(state, bundle_hash):
    result = {}
    for event in state['events']:
        require(event['bundle_sha256'] == bundle_hash, 'answer belongs to a different bundle')
        result[event['reviewer'], event['task']] = event
    return result


def validate_vote(reviewer, kind, task, a, b, preference, note):
    require(isinstance(reviewer, str) and 0 < len(reviewer) <= 80 and all(c.isascii() and (c.isalnum() or c in '._-') for c in reviewer), 'use a short pseudonymous reviewer identifier')
    require(kind in {'human', 'agent'}, 'reviewer kind must be explicit')
    require(a in SEVERITIES and b in SEVERITIES and preference in PREFERENCES, 'invalid verdict')
    require(isinstance(note, str) and len(note) <= 4000, 'note is too long')
    require(bool(note.strip()) or not ({a, b} & {'unrecoverable', 'uncertain'}), 'explain wrong meaning or uncertainty in --note')
    if a != 'uncertain' and b != 'uncertain':
        rank = {name: index for index, name in enumerate(SEVERITIES)}
        require(preference != 'a' or rank[a] <= rank[b], 'preference contradicts meaning-first severity')
        require(preference != 'b' or rank[b] <= rank[a], 'preference contradicts meaning-first severity')
        require(preference != 'tie' or rank[a] == rank[b], 'a tie requires equal severity')
    require(preference != 'both_bad' or {a, b} <= {'recoverable', 'unrecoverable'}, 'both_bad requires two erroneous readings')
    return {'reviewer': reviewer, 'kind': kind, 'task': task, 'a': a, 'b': b, 'preference': preference, 'note': note}


def status(bundle, key, state, reviewer):
    records = latest(state, digest(bundle))
    require(all(task in key['tasks'] for _, task in records), 'unknown task in annotation history')
    done = {task for who, task in records if who == reviewer}
    return {'revision': state['revision'], 'tasks': len(bundle['tasks']), 'answered': len(done),
            'remaining': len(bundle['tasks']) - len(done), 'reviewer': reviewer,
            'kind': next((e['kind'] for (who, _), e in records.items() if who == reviewer), None),
            'human_certified': False, 'purpose': 'biased calibration; no population estimate'}


def summary(bundle, key, state, reviewer):
    progress = status(bundle, key, state, reviewer)
    require(progress['remaining'] == 0, 'finish this reviewer batch before unblinding')
    require(progress['kind'] == 'human', 'agent answers cannot become human review evidence')
    records = latest(state, digest(bundle))
    records = {task: e for (who, task), e in records.items() if who == reviewer}
    observed = defaultdict(set)
    variants = {}
    tasks = {task['id']: task for task in bundle['tasks']}
    categories = {}
    preferences = Counter()
    repeat_total = repeat_same = 0
    for task_id, mapping in key['tasks'].items():
        event = records[task_id]
        source = mapping['source']
        categories[source] = mapping['category']
        for side in ('a', 'b'):
            identity = source, digest(tasks[task_id][side])
            observed[identity].add(event[side])
            variants[source, mapping[side]] = identity
        choice = event['preference']
        winner = mapping.get(choice, choice)
        if mapping['repeat_of']:
            original_id = mapping['repeat_of']
            old, old_map = records[original_id], key['tasks'][original_id]
            old_winner = old_map.get(old['preference'], old['preference'])
            old_labels = {old_map[side]: old[side] for side in ('a', 'b')}
            labels = {mapping[side]: event[side] for side in ('a', 'b')}
            repeat_total += 1
            repeat_same += labels == old_labels and winner == old_winner
        else:
            baseline = next(name for name in (mapping['a'], mapping['b']) if name != 'nrmlz')
            preferences[baseline, winner] += 1
    counts = {name: Counter() for name in ('nrmlz', 'nemo', 'verbalize')}
    conflicts = 0
    for (_, system), identity in variants.items():
        labels = observed[identity]
        verdict = next(iter(labels)) if len(labels) == 1 else 'uncertain'
        conflicts += len(labels) != 1
        counts[system][verdict] += 1
    size = len(categories)
    return dict(progress, systems={name: {'labels': dict(value), 'cases': size,
                'unrecoverable_bounds': [value['unrecoverable'] / size, (value['unrecoverable'] + value['uncertain']) / size]}
                for name, value in counts.items()},
                preferences={baseline: {winner: count for (other, winner), count in preferences.items() if other == baseline}
                             for baseline in ('nemo', 'verbalize')},
                repeats={'answered': repeat_total, 'consistent': repeat_same}, conflicts=conflicts,
                qualification='self-declared human labels only; biased pilot, not certified or representative')
