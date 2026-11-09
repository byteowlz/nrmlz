"""Local-only PolyNorm evaluation. Never commit its inputs, reference texts, outputs or judgments.

Usage: uv run --no-project python tools/qualeval/qualeval.py --lang de|en [--judge --allow-judge-upload] [--reuse]
Config: [qualeval] in difftest.local.toml (see example). Without --judge, deterministic WER/exact only.
Odd-index examples were used for development; even-index examples are a limited held-out evaluation.
Model judgments are a triage proxy, not human-certified unrecoverable-error rates. System names are hidden
from the judge; benchmark targets are explicitly fallible. Cache keys include model and prompt version.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target/qualeval"
LOCALE = {"de": "de-DE", "en": "en-US"}
LABELS = ["correct", "acceptable", "recoverable", "unrecoverable", "unjudged"]
NEMO_SCRIPT = '''
import json,sys
from nemo_text_processing.text_normalization.normalize import Normalizer
n=Normalizer(input_case="cased",lang=sys.argv[1],cache_dir=sys.argv[2])
for line in sys.stdin:
 d=json.loads(line)
 try: out=n.normalize(d["text"],verbose=False,punct_post_process=True)
 except Exception as e: out="<<error "+type(e).__name__+">>"
 print(json.dumps({"id":d["id"],"out":out},ensure_ascii=False),flush=True)
'''
JUDGE_PROMPT = '''Grade text-to-speech normalization in {language}. Read each INPUT and CANDIDATE as a listener.
REFERENCE is only one possible reading, and can contain errors: do not blindly prefer it. Ignore punctuation,
case, American/British wording, digit-group pauses and letters spelled by their names vs spaced letters.
Classify:
correct: natural and fully correct reading preserving all information.
acceptable: correct information but somewhat awkward.
recoverable: wrong grammatical ending, known abbreviation not expanded, awkward reading recoverable by listener.
unrecoverable: wrong/missing number, date, time, value, omitted words, hallucinated information, unreadable digits/symbols.
Do not count differences in valid reading as errors. Return JSON array only, one object per case:
[{"id":0,"label":"correct|acceptable|recoverable|unrecoverable","reason":"brief reason"}].
'''


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False).encode()).hexdigest()


def tokens(text):
    # WER uses casefolded words, Unicode letters/digits, splitting punctuation including hyphens.
    # No letter-name mappings, reference corrections or semantic rewrite is applied.
    return re.findall(r"[^\W_]+", text.casefold())


def distance(ref, hyp):
    row = list(range(len(hyp) + 1))
    for i, a in enumerate(ref, 1):
        nxt = [i]
        for j, b in enumerate(hyp, 1):
            nxt.append(min(row[j] + 1, nxt[-1] + 1, row[j - 1] + (a != b)))
        row = nxt
    return row[-1]


def canon(text):
    return "".join(tokens(text))


def run_process(command, payload):
    result = subprocess.run(command, input=payload, text=True, capture_output=True, cwd="/", check=True)
    return result.stdout


def load_items(cfg, lang):
    checkout = Path(cfg["polynorm_dir"]).expanduser()
    path = checkout / "polynorm_bench" / LOCALE[lang] / f"{LOCALE[lang]}_groundtruth.jsonl"
    items = []
    for line in path.read_text().splitlines():
        d = json.loads(line)
        items.append({"id": d["index"], "category": d["category"], "text": d["original_text"].strip(),
                      "ref": d["normalized_text"].strip(), "split": "dev" if int(d["index"]) % 2 else "test"})
    assert len(items) == 540 and len({i['id'] for i in items}) == 540
    return items, {"source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                   "source_revision": subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()}


def system_output(name, items, lang, cfg):
    payload = "".join(json.dumps({"id": i["id"], "text": i["text"]}) + "\n" for i in items)
    if name.startswith("nrmlz"):
        profile = "reference" if name == "nrmlz-reference" else "nrmlz"
        fixed_config = OUT / 'nrmlz-eval-config.toml'
        fixed_config.write_text('lexicon = ""\ndecimal_pause = false\nrespellings = false\nprofile = "nrmlz"\n')
        cmd = [str(ROOT / "target/release/nrmlz"), '--config', str(fixed_config), lang, "--jsonl", "--profile", profile, "--no-lexicon"]
        return {d["id"]: d["normalized"] for d in map(json.loads, run_process(cmd, payload).splitlines())}
    if name == "nemo":
        cache = OUT / "nemo-cache"
        cache.mkdir(parents=True, exist_ok=True)
        cmd = [str(Path(cfg["nemo_python"]).expanduser()), "-I", "-c", NEMO_SCRIPT, lang, str(cache)]
        return {d["id"]: d["out"] for d in map(json.loads, run_process(cmd, payload).splitlines())}
    if name == "verbalize":
        binary = str(Path(cfg["verbalize"]).expanduser())
        return {i["id"]: run_process([binary, "normalize", "--lang", lang], i["text"]).strip() for i in items}
    raise ValueError(f"unknown system {name}")


def judge_batch(cfg, lang, batch):
    provider = json.loads((Path.home() / ".pi/agent/models.json").read_text())["providers"][cfg["judge_provider"]]
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + provider["apiKey"]}
    headers.update(provider.get("headers") or {})
    cases = [{"id": n, "input": i["text"], "reference": i["ref"], "candidate": c} for n, (_, i, c) in enumerate(batch)]
    prompt = JUDGE_PROMPT.replace("{language}", {"de": "German", "en": "English"}[lang])
    body = {"model": cfg["judge_model"], "messages": [{"role": "user", "content": prompt + json.dumps(cases, ensure_ascii=False)}],
            "max_tokens": 2400, "temperature": 0, "thinking": {"type": "disabled"}, "reasoning_effort": "none"}
    for attempt in range(5):
        try:
            request = urllib.request.Request(provider["baseUrl"] + "/chat/completions", json.dumps(body).encode(), headers)
            message = json.load(urllib.request.urlopen(request, timeout=180))["choices"][0]["message"]
            raw = message.get("content") or ""
            arr = json.loads(raw[raw.index("["):raw.rindex("]") + 1])
            result = {d["id"]: d for d in arr}
            assert set(result) == set(range(len(batch)))
            assert all(d["label"] in LABELS[:-1] for d in arr)
            return [(key, result[n]) for n, (key, _, _) in enumerate(batch)]
        except Exception as exc:
            if attempt == 4:
                print(f"judge batch failed: {type(exc).__name__}", file=sys.stderr)
                return []
            time.sleep(5 * (attempt + 1))
    return []


def run_judge(cfg, lang, items, systems, directory):
    version = digest([cfg['judge_model'], JUDGE_PROMPT])
    path = directory / "judgments-v2.jsonl"
    cache = {}
    if path.exists():
        for line in path.read_text().splitlines():
            d = json.loads(line)
            cache[d['key']] = d['verdict']
    def key(i, c):
        return digest([version, lang, i['text'], i['ref'], c])
    todo = {}
    for outputs in systems.values():
        for i in items:
            c = outputs[i['id']]
            k = key(i, c)
            if canon(c) != canon(i['ref']) and k not in cache:
                todo[k] = (k, i, c)
    pending = list(todo.values())
    batches = [pending[n:n + 6] for n in range(0, len(pending), 6)]
    print(f"judge: {len(pending)} distinct non-exact outputs ({len(batches)} batches)", file=sys.stderr, flush=True)
    if cfg.get('judge_enabled') and batches:
        with concurrent.futures.ThreadPoolExecutor(2) as ex, path.open('a') as f:
            futures = [ex.submit(judge_batch, cfg, lang, batch) for batch in batches]
            for n, future in enumerate(concurrent.futures.as_completed(futures), 1):
                for k, verdict in future.result():
                    cache[k] = verdict
                    f.write(json.dumps({'key': k, 'verdict': verdict}, ensure_ascii=False) + '\n')
                f.flush()
                print(f"judge: {n}/{len(batches)} batches", file=sys.stderr, flush=True)
    return lambda i, c: ('correct' if canon(c) == canon(i['ref']) else cache.get(key(i, c), {}).get('label', 'unjudged')), version


def report(lang, items, systems, label, metadata):
    lines = [f"# PolyNorm-Bench {LOCALE[lang]} evaluation", "", "Scores only; CC BY-NC-ND 4.0 benchmark text stays local.", "",
             "Model-judged errors are **unvalidated triage estimates**, not human-certified error rates. Exact matches are",
             "auto-labelled correct; reference annotation errors and valid alternate readings can distort both metrics.",
             "WER is micro-averaged word edit distance, casefolded with punctuation/hyphens split; it is not paper-comparable.",
             ("German odd indices were inspected for development; even indices were not manually inspected (limited holdout)." if lang == 'de' else "English benchmark text was not inspected to author the rules or gold cases; the split is retained for comparison."), "",
             "| system | split | n | unrecoverable* | recoverable* | acceptable* | correct* | unjudged | WER | exact** |",
             "|---|---|---|---|---|---|---|---|---|---|"]
    summary = {}
    for split in ('test', 'dev', 'all'):
        subset = [i for i in items if split == 'all' or i['split'] == split]
        for name, outputs in systems.items():
            counts = dict.fromkeys(LABELS, 0)
            errors = reference_words = exact = 0
            for i in subset:
                c = outputs[i['id']]
                counts[label(i, c)] += 1
                r, h = tokens(i['ref']), tokens(c)
                errors += distance(r, h)
                reference_words += len(r)
                exact += canon(c) == canon(i['ref'])
            wer = errors / max(reference_words, 1)
            summary[f'{name}/{split}'] = dict(counts, n=len(subset), wer=wer, exact=exact)
            lines.append(f"| {name} | {split} | {len(subset)} | **{counts['unrecoverable']}** | {counts['recoverable']} | "
                         f"{counts['acceptable']} | {counts['correct']} | {counts['unjudged']} | {wer:.2%} | {exact} |")
    lines += ["", "\* Blind model judgment plus auto-exact labels; no claim of human QA. **Exact ignores punctuation/spacing/case.", "",
              "## Reproduction metadata", "", '```json', json.dumps(metadata, indent=2), '```', ""]
    return '\n'.join(lines), summary


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--lang', choices=LOCALE, default='de')
    ap.add_argument('--systems', default='nrmlz,nrmlz-reference,nemo,verbalize')
    ap.add_argument('--judge', action='store_true', help='optional semantic triage via configured model API (may be remote)')
    ap.add_argument('--allow-judge-upload', action='store_true', help='explicitly permit sending benchmark input/reference/candidate text to the configured judge API')
    ap.add_argument('--reuse', action='store_true', help='reuse pinned external-system outputs, rerun nrmlz')
    args = ap.parse_args()
    if args.judge and not args.allow_judge_upload:
        ap.error('--judge sends benchmark snippets to a potentially remote API; explicit --allow-judge-upload required')
    cfg = tomllib.loads((ROOT / 'difftest.local.toml').read_text())['qualeval']
    cfg['judge_enabled'] = args.judge
    items, meta = load_items(cfg, args.lang)
    directory = OUT / args.lang
    directory.mkdir(parents=True, exist_ok=True)
    systems = {}
    for name in args.systems.split(','):
        path = directory / f'{name}.jsonl'
        if args.reuse and path.exists() and not name.startswith('nrmlz'):
            output = {d['id']: d['out'] for d in map(json.loads, path.read_text().splitlines())}
        else:
            t = time.monotonic()
            output = system_output(name, items, args.lang, cfg)
            path.write_text(''.join(json.dumps({'id': k, 'out': v}, ensure_ascii=False) + '\n' for k, v in output.items()))
            print(f'{name}: {len(output)} texts, {time.monotonic()-t:.1f}s', file=sys.stderr, flush=True)
        assert set(output) == {i['id'] for i in items}, f'{name}: missing or extra results'
        systems[name] = output
    label, judge_version = run_judge(cfg, args.lang, items, systems, directory)
    meta.update({'nrmlz_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                 'nrmlz_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True)),
                 'binary_sha256': hashlib.sha256((ROOT / 'target/release/nrmlz').read_bytes()).hexdigest(),
                 'judge_model': cfg['judge_model'], 'judge_prompt_sha256': judge_version,
                 'system_outputs_sha256': {n: digest(o) for n, o in systems.items()}})
    text, summary = report(args.lang, items, systems, label, meta)
    (directory / 'report.md').write_text(text)
    (directory / 'scores.json').write_text(json.dumps({'metadata': meta, 'scores': summary}, indent=2))
    print(text)


if __name__ == '__main__':
    main()
