"""Owner-only, no-clobber review bundles and revisioned local annotation state. No network."""
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import tempfile


def require(condition, message):
    if not condition:
        raise ValueError(message)


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def digest(value):
    return sha(encoded(value))


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def read_json(path):
    path = Path(path)
    require(not path.is_symlink() and path.is_file(), 'expected a regular JSON file')
    return json.loads(path.read_bytes(), object_pairs_hook=unique_object)


def private_file(path, value):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as handle:
        handle.write(encoded(value))
        handle.flush()
        os.fsync(handle.fileno())


def create_bundle(directory, documents):
    directory = Path(directory).expanduser()
    require(not directory.exists() and not directory.is_symlink(), 'output already exists; choose a new version')
    # Private benchmark bundles must not accidentally enter a tracked repository directory.
    root = Path(__file__).resolve().parents[2]
    resolved = directory.resolve()
    require(not resolved.is_relative_to(root) or resolved.is_relative_to(root / 'target'), 'use target/ or a private directory outside the repository')
    directory.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    directory.mkdir(mode=0o700)
    hashes = {}
    for name, document in documents.items():
        require('/' not in name and '\\' not in name and name not in {'.', '..', 'manifest.json'}, 'invalid bundle filename')
        private_file(directory / name, document)
        hashes[name] = sha((directory / name).read_bytes())
    # Written last: an interrupted preparation is never a complete bundle.
    private_file(directory / 'manifest.json', {'schema': 'nrmlz.private-bundle.v1', 'files': hashes, 'created_utc': datetime.now(timezone.utc).isoformat()})
    return hashes


def load_bundle(directory):
    directory = Path(directory).expanduser()
    require(not directory.is_symlink() and directory.is_dir(), 'expected a bundle directory')
    require(directory.stat().st_mode & 0o077 == 0, 'bundle directory must be owner-only (0700)')
    manifest = read_json(directory / 'manifest.json')
    require(manifest.get('schema') == 'nrmlz.private-bundle.v1', 'unknown bundle schema')
    result = {}
    for name, expected in manifest['files'].items():
        require('/' not in name and '\\' not in name and name not in {'.', '..'}, 'invalid manifest filename')
        path = directory / name
        require(not path.is_symlink() and path.is_file(), 'missing bundle artifact')
        require(path.stat().st_mode & 0o077 == 0, 'bundle files must be owner-only (0600)')
        require(sha(path.read_bytes()) == expected, 'bundle artifact hash mismatch')
        result[name] = read_json(path)
    return result


@contextmanager
def locked(directory):
    # Fail-fast exclusive lock, portable and never follows a preexisting symlink.
    path = Path(directory) / '.annotation.lock'
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        raise ValueError('annotation lock exists; retry or inspect a stale lock after a crash') from None
    try:
        with os.fdopen(fd, 'w') as handle:
            handle.write(str(os.getpid()))
        yield
    finally:
        path.unlink()


def answers(directory):
    directory = Path(directory)
    require(not directory.is_symlink(), 'annotation state must not be a symlink')
    if directory.exists():
        require(directory.is_dir() and directory.stat().st_mode & 0o077 == 0, 'annotation state must be owner-only (0700)')
    path = directory / 'answers.json'
    require(not path.is_symlink(), 'answers file must not be a symlink')
    if not path.exists():
        return {'revision': 0, 'events': []}
    require(path.stat().st_mode & 0o077 == 0, 'answers file must be owner-only (0600)')
    state = read_json(path)
    require(type(state.get('revision')) is int and state['revision'] == len(state['events']), 'invalid answer revision')
    previous = None
    for number, event in enumerate(state['events'], 1):
        content = {k: v for k, v in event.items() if k != 'sha256'}
        require(event.get('revision') == number and event.get('previous') == previous and digest(content) == event.get('sha256'), 'annotation history hash mismatch')
        previous = event['sha256']
    return state


def append_answer(directory, event, expected_revision, replace=False):
    directory = Path(directory)
    require(not directory.is_symlink(), 'annotation state must not be a symlink')
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    require(directory.stat().st_mode & 0o077 == 0, 'annotation state must be owner-only (0700)')
    with locked(directory):
        state = answers(directory)
        require(state['revision'] == expected_revision, 'stale answer revision; refresh before retrying')
        require(all(e['bundle_sha256'] == event['bundle_sha256'] for e in state['events']), 'annotation state belongs to another bundle')
        prior = [e for e in state['events'] if e['reviewer'] == event['reviewer']]
        require(all(e['kind'] == event['kind'] for e in prior), 'reviewer kind cannot change; use a separate reviewer identity')
        matching = [e for e in prior if e['task'] == event['task']]
        if matching:
            old = {k: matching[-1][k] for k in event}
            if old == event:
                return state['revision']
            require(replace, 'answer exists; correction requires --replace and the current revision')
        event = dict(event, at=datetime.now(timezone.utc).isoformat(), revision=state['revision'] + 1, previous=state['events'][-1]['sha256'] if state['events'] else None)
        event['sha256'] = digest(event)
        state['events'].append(event)
        state['revision'] += 1
        fd, temporary = tempfile.mkstemp(dir=directory, prefix='.answers-')
        try:
            with os.fdopen(fd, 'wb') as handle:
                handle.write(encoded(state))
                handle.flush()
                os.fsync(handle.fileno())
            destination = Path(directory) / 'answers.json'
            require(not destination.is_symlink(), 'answers file must not be a symlink')
            os.replace(temporary, destination)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
        return state['revision']
