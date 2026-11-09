//! Spanish module boundary (planned; intentionally not implemented).
//!
//! The future `try_normalize` pipeline will share `num` and `pyre` machinery, with Spanish data under
//! `data/es`. It must handle gender/apocope (veintiuna personas, veintiún días), 1.º/1.ª/1.er, dates,
//! currency, units, telephone, addresses and code switching. Do not reuse English readings as a fallback.
//! `"es".parse::<crate::Lang>()` currently returns an error rather than pretending to support Spanish.
//!
//! Acceptance: a hand-reviewed Spanish gold set with per-line rationale, local PolyNorm es-MX evaluation,
//! and an unchanged frozen German reference gate. See docs/spanish-plan.md and the Spanish tracker issues.
