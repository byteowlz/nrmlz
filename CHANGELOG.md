# Changelog

## Unreleased - a.m. marker reads "ae emm"

- With `letter_names`, the English a.m. marker ("A M" from "a.m."/"AM") is spoken "ae emm" instead of "A em".
  Owner choice; respelling tuner run 7 (Parakeet Ultra, 24 renders each): "ae emm" 88%, "A em" 83%.
  A lone A keeps its rules (article, "ay" after another letter); default output unchanged.

## Unreleased - p.m. marker reads "pe em"

- With `letter_names`, the English p.m. marker ("P M" from "p.m."/"PM") is spoken "pe em" instead of "pee em".
  Owner choice by ear in the TTS playground; respelling tuner run 6 (Parakeet Ultra, 24 renders each) scores
  "pe em" 100%, the same as "pee em". A lone P elsewhere stays "pee"; default output unchanged.

## Unreleased - phone numbers as one sentence per digit group

- Speech style `phone_chunks` (`--phone-chunks`, config `phone_chunks`): English and German phone numbers are
  spoken as one sentence per digit group, "four one five. Five five five. Zero one nine nine, before Friday."
  Meant for TTS front ends that synthesise every sentence as its own chunk without regrouping; tuner run 2
  measured EN 46% vs 8% and DE 83% vs 75% against the number as one chunk. Years, prices and plain counts are
  not matched; German needs a leading 0 or +49 and 8-14 digits. Default output unchanged.

## Unreleased - letter runs: "ay" after another letter

- In English letter runs, "A" stays "A" only as the first letter; after another letter it is spoken "ay"
  ("UA" -> "you ay", "CIA" -> "see eye ay"). "I" stays only on its own. Measured with the respelling tuner:
  "flight you A four eighty-two" 17% vs "you ay four eighty-two" 71% (24 renders each).
- The letter-name pass now maps whole runs of standalone capitals, so both rules apply to normaliser-spelled
  initialisms too ("U S" -> "you es").

## Unreleased - opt-in speech styles and a.m./p.m. sentence boundary

- Add `SpeechStyle` with two opt-in passes and `normalize_styled`; `normalize_with` and all default output are
  unchanged (gold corpora identical):
  - `letter_names` (`--letter-names`): English standalone capitals become spoken letter names
    ("Q X" -> "cue ex", "A M" -> "A em", "Plan B" -> "Plan bee"); "A" and "I" stay. German already spells names.
  - `code_digits` (`--code-digits`): codes are read in pieces, English "QX-4471" -> "cue ex four four seven one",
    "flight UA 482" -> "flight you ay four eighty-two"; German "QX-4471" -> "kuh iks, vierundvierzig einundsiebzig".
  - Chosen with an automated TTS round-trip tuner for the Foxline bilingual Pocket model; see
    `lexicons/pocket-bilingual/README.md` for the measured rates.
- Fix: "6:45 a.m. Please hurry." merged two sentences ("A M Please"). After a.m./p.m., a capitalised word that is
  not a weekday or month now keeps the sentence boundary.
- Config keys `letter_names` and `code_digits`.

## Unreleased - Pocket bilingual TTS lexicons

- Add `crates/nrmlz/lexicons/pocket-bilingual/{de,en}.toml`: model-specific respellings for the Foxline
  bilingual Pocket TTS model, which reads "Okay"/"OK" as "okah". German: Okay/OK/O.K. -> "Okeh";
  English: -> "O K". Both chosen by listening, not by rule.
- `tests/pocket_lexicon.rs` pins the regression sentences, including sentence-final "O.K." and words that
  must stay untouched ("Token", "okayish").

## Unreleased - lexicon terms keep a sentence-final dot

- A lexicon term ending in "." ("O.K.", "usw.") used to swallow the dot even where it also ended the
  sentence: "That is O.K." became "That is O K" and "That's O.K. Then we go." merged into one sentence,
  which also merges speech chunks downstream.
- Now the replacement keeps a "." where the dot ended the sentence: at the end of the text in every
  language, and in English also before a conventional sentence starter, reusing the existing
  abbreviation rule (`dot_ends_sentence`, behaviour unchanged). German gets no mid-text rule because it
  capitalises nouns ("z.B. Äpfel" stays one sentence). Replacements that already end in sentence
  punctuation get no second dot.
- Add "Let's" to the English sentence starters, so "etc. Let's go" and "O.K. Let's go" keep their
  sentence boundary.
- Found while adding an "O.K." -> "O K" TTS lexicon entry; tests cover end of text, closing quotes,
  mid-sentence, following "?" and ",", English sentence starters and the German noun case.

## Unreleased - typography folding is the default

- Make TTS-safe typography folding the **default** for the nrmlz profile, in `normalize_de`, `normalize_en` and
  `normalize_with`: `„“”‘’«»‹›` -> straight quotes, `–—−` -> `-`, `…` -> `...`, non-breaking spaces -> space,
  zero-width/BOM dropped.
- Keep the `reference` profile byte-identical to the Articulino frontend: parity is now one explicit profile,
  not the default contract. Rationale: a speech front-end is the main consumer and byte parity there was
  costing correctness.
- Fold before *and* after normalisation, because lexicon entries can reintroduce typography.
- Measured on German sentences quoting English, three voices: 0.232-0.365 recognition error on typographic
  quotes against 0.026-0.043 folded, on the same clips. A 4k SentencePiece model has no piece for `„`, so it
  arrived as three `<0x..>` byte tokens and the quoted span was mispronounced or dropped.
- `--tts-safe` on the CLI remains as an explicit no-op switch; folding is already on.
## Unreleased - TTS-safe typography

- Add opt-in `tts_safe` folding plus `--tts-safe`: `„“”‘’«»` -> straight quotes, `–—−` -> `-`,
  `…` -> `...`, non-breaking and zero-width spaces folded or dropped. Default normalisation output is
  unchanged, so the frozen reference parity profile stays byte-identical.
- Motivation, measured: a 4k SentencePiece speech model has no piece for `„`/`“`, so each arrived as
  three `<0x..>` byte tokens. German sentences quoting English lost or mangled the quoted span
  (recognition error 0.23-0.37 typographic vs 0.026-0.043 folded on the same clips, three voices).
  Folding is a front-end concern, not number normalisation, so it stays separate and explicit.
- Tests cover quote/dash/space folding, idempotence, untouched numbers and letters, absence of
  fallback-prone characters, and that `normalize_de` output keeps typography.

## 0.2.4 - 2026-11-09

- Add one context decision for plain integer runs. Phone/identifier cues read digits (`My phone number is
  46255577782`, `Telefonnummer 462555`); amount cues read a cardinal (`46255577782 Stück`, `... units`).
  Without a cue, existing behaviour is unchanged: English long numbers stay amounts, German long numbers keep
  their digit fallback. New German `number-context` deviation class; reference profile unaffected.
- Group space-separated digit runs under `--phone-chunks` (both languages).
- English gold is unchanged; local WER moves to **17.12%** (57 labelled + 42 unjudged) and German to **18.45%**
  (44 + 12). No new API grading. German gold remains 180/180 and reference parity 15,694/15,694.

## 0.2.3 - 2026-11-08

- Add a local-only blind semantic calibration CLI: 42 unresolved cases plus 18 stratified controls, two baseline
  comparisons per input and six hidden repeats. Candidate identities/references/cache labels stay off review cards.
- Collect pairwise preference and separate severity/uncertainty with explicit reviewer kind. Agent labels cannot
  become human reports; human flags are self-declarations, not certification. Repeated/conflicting labels stay
  uncertain, and the targeted pilot does not estimate population rates.
- Freeze owner-only, hash-bound input/key bundles; keep mutable revisioned annotation state outside immutable
  dataset versions. Add stale-write/correction/lock/tamper and synthetic CLI tests. No external grading/server.
- Add a source/family-disjoint fresh-input freeze workflow and known-copy deny checks. No fresh independent final
  set or human answers are fabricated; annotation/audio validation and manual promotion are still required.
- Fix German `13.15 Uhr` being read as "Punkt fünfzehn": dotted clock times followed by `Uhr` now read as
  `dreizehn Uhr fünfzehn`. Spell letter codes consistently before number handling (`Flug ER 7651`,
  `B-40884`, `XV-71066`), with digits read individually. New deviation classes `dotted-clocks` and
  `letter-codes`; reference profile stays unchanged.
- Close the two declared German gold gaps: kitchen units (`EL`, `TL`, `Prise`) and a bare `1` before a known common
  noun now take the correct article (`eine Prise`, `einen Esslöffel` after a transitive verb, `einem Bruder` with a
  dative preposition). New `kitchen-units` and `bare-one-article` deviation classes; German gold is **180/180** and
  browser gold **450/450**. The reference profile is unchanged. German PolyNorm WER is **18.58%** (46 labelled
  unrecoverable estimates + 9 unjudged) versus reference 30.24%; English is unchanged at 17.35%. No new API grading.
- Normalizer version and recorded English WER/size results are unchanged; German deviations are re-attributed.

## 0.2.2 - 2026-11-02

- Fix blood-pressure/fraction ambiguity: integer BP readings use `over`, with unit and mathematical-context
  contrasts. No clinical plausibility/diagnostic claim.
- Add zero-preserving financial/government/medical record fields, bounded grouped card/SSN formats, SWIFT/BIC
  and 13 common IBAN country lengths. Unsupported grouped IBAN/card forms remain literal; this is not account validation.
- Add locally validated IPv4/IPv6, compression/zones/CIDR/ports, literal MAC/UUID readings, conservative US
  street directions/suffixes and apartment/suite/room codes. Invalid labelled technical values stay literal.
- Add 54 independently authored realistic gold contrasts: English270/270, German178+2 known gaps,448 web gold
  cases under bun/Node. Preserve frozen German fidelity on15,694 inputs/second passes/streams. Native/wasm32,
  clippy-Dwarnings, formatting and all tests pass; incremental static quality has no major gates.
- English PolyNorm WER17.35% (limited holdout16.42%) vs NeMo24.18%/25.26% and verbalize20.60%/20.64%.
  No new API grading;42 outputs unjudged, so semantic superiority remains unproved. No benchmark-sourced
  expectations or data were imported. Input-hash audit flags one independently authored common-input overlap;
  the new gold is not claimed strictly input-disjoint.
- WASM 1,301,441 bytes / 510,961 gzip / 384,439 brotli; native English 0.264 ms/gold input, warm-up 13.8 ms. Working
  repository `docs/en.md`, `docs/quality.md` and score-only `docs/evaluation/0.2.2-en.json` record limits/provenance.

## 0.2.1 - 2026-11-02

Fix the English coverage gaps that made 0.2.0 trail NeMo.

- Recognize ISBN-10/13, PIN/ZIP/serial/VIN/SKU/part-code identity before cardinals, dates and ranges. Preserve
  every zero/check letter. Read prefixed versions and mixed product/formula tokens, preserving padded components,
  readable brands and known acronym words. No pronunciation respellings or benchmark-keyed exceptions.
- Add contextual rational/mixed quantities and Unicode fractions, irregular halves/quarters and unit/rate handling.
  Keep zero denominators and ambiguous calendar readings intact. Rejected recognizer candidates remain raw so
  another stage can interpret them; final lexicon output remains protected.
- Add 26 independently authored realistic English gold rows (216 total), plus identifier/version/fraction/context
  properties and span/lexicon integration tests. Deliberately change raw `A12` to `A twelve`. Share context assertions.
- Local PolyNorm English WER **17.63%**, down from 26.04%, versus NeMo 24.18% and verbalize 20.60%. Even-index limited
  holdout **16.50%** versus 25.26% and 20.64%. No benchmark text inspected to author fixes; no new API grading.
  **Semantic superiority is unverified:** 58 cached unrecoverable estimates plus 36 unjudged outputs are not a
  complete error rate. Verbalize still wins canonical exact count; no all-metric SOTA claim.
- Verified native/wasm32/clippy/formatting, all tests, 15,694 live German reference cases including second pass/stream,
  216 English gold and 394 web gold cases under bun/Node. Reference behavior unchanged.
- WASM 1,263,825 bytes / 498,507 gzip / 374,949 brotli; about 1.76% gzip growth for the new coverage. Native English
  0.267 ms/input, browser-binding English 0.409–0.418 ms/input. Detailed provenance and limitations: working
  repository `docs/quality.md` and `docs/evaluation/0.2.1-en.json`.

## 0.2.0 - 2026-11-02

Owner-approved quality track beyond the Articulino reference.

- Freeze the byte-identical German `reference` profile; make improved `nrmlz` the default. Decimal pause comma and
  built-in pronunciation respellings are opt-in. Explicit lexicon replacements are final, including when
  respellings are enabled; collision-free protection handles arbitrary user text.
- Fix standalone area/volume units (`pro m²`, `kg/m³`), duplicated list enumeration, identifiers, abbreviated dates,
  URLs, bare LaTeX, hashtags, Unicode fractions, coordinates, context-marked Roman numerals, versions, durations,
  scores, decades, ordinal agreement, leftover digits/symbols, missing abbreviations, mixed-case initials and
  whitespace. Counts and rationale per deviation class are in the working repository's `docs/differences.md`.
- Add English normalization: US cardinals/ordinals/decimals/years, exact decimal currencies, percent, times,
  unambiguous named/US/ISO dates, units, phone numbers, abbreviations, e-mail/URL, ranges and symbols. Tables are
  independently authored and cross-checked against pinned NeMo vocabulary; no pronunciation respellings.
- Normalize explicit `[en]...[/en]` spans inside German in English, retain routing tags and protect spans while
  streaming. Add English/profile APIs to CLI and WASM.
- Add independent gold with rationale per row: German 178 passing + 2 known gaps, English 190 passing. Add scoped
  context-invariance, no-digits, abbreviation-expansion and final-lexicon properties.
- Add local PolyNorm DE/EN WER plus optional API-based blind model triage, source/binary/prompt/output hashes,
  metric tests and separate quality/deviation recipes. No CC BY-NC-ND benchmark text, outputs or judgments in git.
  The initial semantic grading sent snippets to the configured model API, not purely on-device. Future API
  grading requires explicit `--judge --allow-judge-upload` acknowledgment; local WER needs neither.
- Reserve a Spanish module boundary and document future morphology/locale/quality requirements; no Spanish runtime.
- Declare Rust 1.88 minimum for current dependencies; verification ran on Rust 1.97.

### Verification and limits

- Frozen reference: 15,694/15,694 identical in text, second pass and streaming; 2,030,994/2,030,994 number-word
  cases identical to num2words. Default deviations: zero unexplained.
- PolyNorm model-judged unrecoverable estimates (all 540 items/locale): German 53 vs reference 118; English 111 vs
  NeMo 87 and verbalize 55. WER: German 19.00% vs reference 30.24%; English 26.04% vs NeMo 24.18% and verbalize 20.60%.
  These are not human-certified error rates or paper-comparable metrics. English still needs broader coverage.
- WASM with both languages: 1,242,254 bytes (489,873 gzip, 368,469 brotli), 368 passing gold cases under bun and Node
  browser bindings. About 0.11 ms/German gold input and 0.41–0.43 ms/English gold input; roughly 90 ms warm-up.
- Native: about 0.11 ms/German rule/stress input and 0.28 ms/English gold input; stripped CLI 3.10 MB.
- No acoustic/listening validation. German bare-one agreement and kitchen units remain two declared gold gaps;
  English specialist classes, Roman numerals and ambiguous/invalid formats remain limitations.

## 0.1.0 - 2026-11-02

First release: German.

### Added

- `nrmlz` library: `normalize_de` / `de::try_normalize`, a Rust port of the Articulino German frontend
  (articulino-onnx 1.0.0, Hugging Face revision a760ce8d). It has the same stages, word lists and data tables, and
  byte-identical output. The stages are public.
- `de::StreamNormalizer`: sentence-by-sentence normalisation of a token stream with the same cut points as the
  Python version.
- German number words compatible with num2words 0.5.14 (cardinal, ordinal, year, up to 10^606), implemented
  independently.
- Python-compatible regex layer: Python `re` patterns run on fancy-regex with Python's Unicode classes (Unicode
  15.0 tables), `\b`, `$` and `re.sub` empty-match semantics. A native fast path is used where both agree.
- Lexicon stage: TOML, JSON or TSV term lists, word-bounded and case-sensitive by default, applied before or after
  normalisation.
- `nrmlz` CLI: `nrmlz de` for arguments, stdin lines, `--whole`, `--jsonl` and `--stream`, plus `--lexicon`. Config
  lives in `$XDG_CONFIG_HOME/nrmlz/config.toml`, with a JSON schema.
- WebAssembly bindings (`nrmlz-wasm`, wasm-bindgen): `normalize(lang, text)`, `Lexicon`, `Stream`, with Node and
  web builds and a bun/Node smoke test.
- Differential test harness (`tools/difftest`, the acceptance gate) against the pinned Python frontend, covering:
  - text mode, second pass and streaming;
  - an exhaustive num2words comparison;
  - rule coverage.
- Corpora: rule examples and a generated stress set of 2,773 lines covering every rule, with reference outputs, so
  `cargo test` checks byte identity without Python. Property tests check no panics on arbitrary Unicode, bounded
  time, and idempotence where Python is idempotent.
- `docs/research-sota.md`: state of text normalisation for TTS (November 2026) and the case for this design.
- `docs/differences.md`: acceptance results and every deviation.

### Results

- 15,514 of 15,514 texts identical (rules, stress, 12,331 loanword sentences, 328 teacher texts, 6 number
  sentences). This holds on a second pass and when streamed in 7-character chunks.
- 2,030,994 of 2,030,994 number words identical to num2words.
- Native about 6x faster than the Python reference: 0.047 ms against 0.30 ms per text.
- WebAssembly 1.05 MB (418 KB gzip), about 0.2 ms per sentence under Node/bun.
