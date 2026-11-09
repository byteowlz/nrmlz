# nrmlz recipes. `just` lists them.

default:
    @just --list

# cargo check, clippy and formatting check
check:
    cargo check --workspace --all-targets
    cargo check -p nrmlz-wasm --target wasm32-unknown-unknown
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# unit, corpus and property tests (corpus expectations are committed; no Python needed)
test:
    cargo test --release -p nrmlz --features verify-native
    cargo test --release -p nrmlz-cli

# frozen-reference gate: byte identity with the pinned Python frontend (needs difftest.local.toml)
difftest *args:
    cargo run --release -p difftest -- {{args}}
    cargo run --release -p difftest -- --stream 7 {{args}}

# default-profile deviation classes (all configured corpora)
deviations *args:
    cargo run --release -p difftest -- --deviations {{args}}

# independent hand-written quality gold, scoped properties, and CLI tests
quality:
    cargo test --release -p nrmlz --test gold --test improvements
    cargo test --release -p nrmlz-cli

# Local PolyNorm scores only (non-commercial evaluation; data and judgments stay in target/).
# Set [qualeval] in difftest.local.toml. Remote-capable triage requires --judge --allow-judge-upload;
# it is not a human-certified metric. Without these flags, only local system inference/metrics run.
qualeval lang="de" *args:
    cargo build --release -p nrmlz-cli
    uv run --no-project python tools/qualeval/qualeval.py --lang {{lang}} {{args}}

# Private human review: next displays licensed text locally; agents use --metadata-only/status.
[positional-arguments]
review *args:
    uv run --no-project python tools/qualeval/review.py "$@"

review-test:
    uv run --no-project python -m unittest discover -s tools/qualeval -v

# exhaustive number-speller check against num2words (about 2 million cases, a few minutes)
difftest-numbers:
    cargo run --release -p difftest -- --numbers

# rule coverage of the committed corpora (patterns that never matched)
coverage:
    cargo run --release -p difftest --features coverage -- --only rules --only stress --out target/difftest-coverage

# regenerate the stress corpus and the expected outputs from the Python reference
expected:
    uv run --no-project python -I tools/scripts/gen_stress.py crates/nrmlz/data/de > tests/corpus/stress-de.jsonl
    cargo run --release -p difftest -- --write-expected

wasm_opt := "bunx --package binaryen@132.0.0 wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue --strip-debug --strip-producers"

# WebAssembly package (Node/bun in pkg/, browsers in pkg-web/), size-optimised, and its smoke test
wasm:
    cargo build --target wasm32-unknown-unknown --profile wasm-release -p nrmlz-wasm
    wasm-bindgen --target nodejs --out-dir crates/nrmlz-wasm/pkg target/wasm32-unknown-unknown/wasm-release/nrmlz_wasm.wasm
    wasm-bindgen --target web --out-dir crates/nrmlz-wasm/pkg-web target/wasm32-unknown-unknown/wasm-release/nrmlz_wasm.wasm
    {{wasm_opt}} crates/nrmlz-wasm/pkg/nrmlz_wasm_bg.wasm -o crates/nrmlz-wasm/pkg/nrmlz_wasm_bg.wasm
    {{wasm_opt}} crates/nrmlz-wasm/pkg-web/nrmlz_wasm_bg.wasm -o crates/nrmlz-wasm/pkg-web/nrmlz_wasm_bg.wasm
    @wc -c < crates/nrmlz-wasm/pkg/nrmlz_wasm_bg.wasm | awk '{printf "wasm: %d bytes\n", $1}'
    @gzip -9 -c crates/nrmlz-wasm/pkg/nrmlz_wasm_bg.wasm | wc -c | awk '{printf "gzip -9: %d bytes\n", $1}'
    bun crates/nrmlz-wasm/js/smoke.mjs
    node crates/nrmlz-wasm/js/smoke.mjs

# throughput, overall and per stage
bench *files:
    cargo run --release -p nrmlz --example bench -- {{files}}

# English throughput on the independent gold inputs
bench-en:
    cargo run --release -p nrmlz --example bench_en

# release build of the CLI
build:
    cargo build --release -p nrmlz-cli

# install the CLI into ~/.cargo/bin
install:
    cargo install --path crates/nrmlz-cli

fmt:
    cargo fmt --all
