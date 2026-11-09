"""Reference side of the differential test: runs the pinned Python Articulino frontend.

Reads JSON lines {"id": ..., "text": ...} on stdin and writes {"id", "out" | "error", "out2" | "error2", "ms"}:
out is normalize(text), out2 is normalize(out) (for the idempotence check). With --stream N the text is fed to
StreamNormalizer in chunks of N characters instead, and out is the concatenation of every released chunk.
The first output line describes the reference: {"reference": {...}} with the hash of german.py.

With --numbers it reads {"id", "n", "kind"} and writes num2words(int(n), lang="de", to=kind).

Run with the reference interpreter in isolated mode: python -I reference.py [--stream N] < in.jsonl > out.jsonl
"""
import hashlib
import json
import sys
import time

from importlib.metadata import version

from articulino_onnx.frontend import german


def run(fn, text):
    try:
        return {"out": fn(text)}
    except Exception as e:  # the Rust port must fail in the same places
        return {"error": type(e).__name__}


def streamed(text, size):
    s = german.StreamNormalizer()
    parts = []
    for i in range(0, len(text), size):
        parts.append(s.push(text[i:i + size]))
    parts.append(s.flush())
    return "\u241e".join(parts)  # record separator: the cut points must match as well


def numbers(out):
    from num2words import num2words
    for line in sys.stdin:
        item = json.loads(line)
        r = run(lambda n: num2words(int(n), lang="de", to=item["kind"]), item["n"])
        r["id"] = item["id"]
        out.write(json.dumps(r, ensure_ascii=False) + "\n")


def main():
    stream = int(sys.argv[sys.argv.index("--stream") + 1]) if "--stream" in sys.argv else 0
    src = open(german.__file__, "rb").read()
    meta = {"german_py_sha256": hashlib.sha256(src).hexdigest(), "python": sys.version.split()[0],
            "num2words": version("num2words")}
    out = sys.stdout
    out.write(json.dumps({"reference": meta}, ensure_ascii=False) + "\n")
    if "--numbers" in sys.argv:
        return numbers(out)
    for line in sys.stdin:
        item = json.loads(line)
        text = item["text"]
        t = time.perf_counter()
        if stream:
            r = run(lambda x: streamed(x, stream), text)
        else:
            r = run(german.normalize, text)
            if "out" in r:
                r2 = run(german.normalize, r["out"])
                r.update({k + "2": v for k, v in r2.items()})
        r["ms"] = round((time.perf_counter() - t) * 1000, 3)
        r["id"] = item["id"]
        out.write(json.dumps(r, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
