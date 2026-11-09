# nrmlz

Rust text normalisation for TTS: German and English, native and WebAssembly. Output is spoken words, not phonemes.

```text
Dichte in kg/m³. Der Preis pro m² steigt. Das wiegt 3,5 kg.
Dichte in Kilogramm pro Kubikmeter. Der Preis pro Quadratmeter steigt. Das wiegt drei Komma fünf Kilogramm.
```

## Profiles and lexicons

- **`nrmlz` (default):** improvements over Articulino, documented by deviation class.
- **`reference`:** the frozen Articulino German frontend, byte-identical to the pinned Python version.
- Decimal pause comma and built-in pronunciation respellings are **opt-in**, not defaults.
- Explicit user lexicon replacements are final in the default profile: `OK = "Okay"` stays `Okay`, even with
  built-in respellings enabled. Case-sensitive word boundaries, longest match, no recursive replacement.

```rust
use nrmlz::{normalize, normalize_with, Lang, Lexicon, Profile};
assert_eq!(normalize(Lang::De, "3,5 kg."), "drei Komma fünf Kilogramm.");
assert_eq!(normalize(Lang::En, "It costs $5.50."), "It costs five dollars and fifty cents.");
let mut lex = Lexicon::new();
lex.insert("OK", "Okay");
assert_eq!(normalize_with(Lang::De, "OK!", Profile::NRMLZ, Some(&lex)), "Okay!");
let compatibility = Profile::NRMLZ.with_decimal_pause(true).with_respellings(true);
```

Lexicon TOML (equivalent JSON and `term<TAB>replacement` TSV are supported):

```toml
stage = "before" # or "after"
[terms]
OK = "Okay"
```

English uses US number words, short scale, exact decimal money, 12/24-hour times, unambiguous named/ISO/US dates,
units, phones, abbreviations, e-mail/URLs, ranges and symbols. Ambiguous or invalid structured dates are preserved,
not guessed. Balanced `[en]...[/en]` spans inside German use English readings and keep their tags for TTS routing.
Spanish is a documented skeleton only; `es` returns an unsupported-language error.

## Command line

```sh
just install
nrmlz de "Preis pro m²: 24,99 €."
nrmlz en 'On March 14, 2026 at 3:30 p.m. it costs $24.99.'
nrmlz de --profile reference "3,5 kg."
nrmlz de --decimal-pause --respellings "Okay, 3,5 kg."
nrmlz de --lexicon terms.toml "OK!"
nrmlz de < sentences.txt                 # one text per line
nrmlz de --whole < answer.md             # one multiline input
nrmlz en --jsonl --field text < in.jsonl # adds "normalized"
nrmlz de --stream < tokens.txt           # German sentence streaming, protects English spans
```

Configuration priority: flags > `--config FILE` > `NRMLZ_*` environment > `./nrmlz.toml` >
`$XDG_CONFIG_HOME/nrmlz/config.toml` (or the usual `.config` fallback). The global file is created with defaults;
`crates/nrmlz-cli/config.schema.json` supplies validation/completions.

## WebAssembly

`just wasm` produces `crates/nrmlz-wasm/pkg` (Node/bun) and `pkg-web` (browser bindings).

```js
import init, { normalize, normalizeWith, Lexicon, Stream } from "./pkg-web/nrmlz_wasm.js";
await init();
normalize("en", "It weighs 3 kg.");
normalizeWith("de", "3,5 kg.", "reference");
const lex = Lexicon.fromJson('{"terms":{"OK":"Okay"}}');
lex.normalize("de", "OK!");
const s = new Stream(); s.push("Am 3. Mai. "); s.flush();
```

## Results (0.2.4, Apple Silicon)

| Check | Result |
|---|---|
| Frozen German reference | **15,694/15,694 identical**, including second pass and 7-character streaming; includes 180 new gold inputs |
| Independent gold | German **180/180**; English **270/270** |
| Browser-binding gold under bun and Node | **450/450 passing cases** |
| Default-profile deviations | All attributed to 28 named classes; no unexplained differences |
| Native throughput | German about **0.10 ms/text** on rule/stress inputs; English **0.24 ms/text** on gold inputs |
| WASM throughput | German **0.10–0.11 ms/text**, English **0.34–0.40 ms/text** on gold inputs; warm-up about 106 ms |
| WASM size, both languages | **1,343,386 B**, **526,877 B gzip**, **396,152 B brotli** |
| Native sizes | Rust library archive 4.62 MB (not including dependencies); smallest linked program 2.62 MB stripped; CLI 3.33 MB stripped |

### External quality evaluation

**Semantic errors first:** German has 44 model-estimated unrecoverable errors (plus 12 unjudged outputs) versus
reference 118. English has 57 labelled estimates **plus 42 unjudged outputs**. Both are incomplete, not comparable
error rates. No new grader API calls ran for 0.2.1–0.2.3. These estimates are not human-certified.

| PolyNorm-Bench WER, all 540 examples/locale | nrmlz | Articulino reference | NeMo 1.2.0 | verbalize 0.2.0 |
|---|---|---|---|---|
| German | **18.45%** | 30.24% | 52.08% | 27.83% |
| English 0.2.4 | **17.12%** | not applicable | 24.18% | 20.60% |

English now beats both on this WER measure, down from 26.04%. On the limited held-out half: **16.42%** versus
NeMo 25.26% and verbalize 20.64%. The fixes add structured fractions, digit-preserving IDs/ISBNs, padded versions
and mixed product/formula readings. Typed blood pressure, financial/medical IDs, IPv6/MAC/UUID and common US
street/unit components now have context-aware handling; these are format readings, not clinical/account validation. Verbalize still has more exact matches; **no all-metric SOTA claim** is made.
WER includes valid alternate readings and annotation errors; it is not paper-comparable. Earlier optional semantic
grading used a model API, not purely on-device inference. No benchmark-sourced inputs/targets, system candidates
or judgments are committed; gold inputs/expectations are independently authored.

Known gaps: German article rules cover an explicit common-noun/kitchen-unit table, not arbitrary nouns; English Roman
numerals, richer formulas, richer specialist notation/addresses and ambiguous formats. No acoustic listening/ASR validation is claimed. Detailed
counts, rationale and score provenance are in the working repository's `docs/differences.md`, `docs/quality.md` and
`docs/en.md`.

Human calibration is available through `just review --help` (local terminal/JSON, no model calls). The working
repository's `docs/human-review.md` defines the biased pilot, uncertainty handling and fresh-source freeze protocol.
No human labels or independent final-validation claim are generated automatically.

## Development

Rust 1.88+; verified with Rust 1.97. Use `just` for checks:

```sh
just check        # native + wasm32 check, clippy -D warnings, formatting
just test         # reference fixtures, gold, properties, CLI; no Python needed
just difftest     # live pinned Python reference; configure difftest.local.toml
just deviations  # per-class default/reference counts
just quality     # independent gold + scoped invariance/no-digits/lexicon tests
just qualeval de --reuse # local scores; API grading requires --judge --allow-judge-upload
just bench
just bench-en
just wasm         # wasm-bindgen + pinned binaryen optimisation, bun/Node/web checks
```

Apache-2.0. German reference rules/data: Copyright 2026 eigenhand, Articulino revision
`a760ce8d513534111c7679751869fa4c45ef0e54`. English is independently authored, with a pinned NeMo vocabulary
cross-check. See [NOTICE](NOTICE).
