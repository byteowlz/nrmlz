//! German text frontend: turns written text (as a language model writes it) into words a TTS model can read.
//!
//! Port of `articulino_onnx/frontend/german.py` from Articulino-DE-EN-Realtime-Stream-TTS (eigenhand,
//! Apache-2.0, Hugging Face revision a760ce8d513534111c7679751869fa4c45ef0e54), translated to Rust for nrmlz.
//! The `reference` profile keeps the upstream rules and tables byte-identical. The default `nrmlz` profile
//! layers documented quality fixes over it (see docs/differences.md); those are not silently added to reference.
//!
//! [`normalize`] runs these stages, in this order:
//!
//! ```text
//! phone_numbers        +49 170 1234567 -> plus vier neun, eins sieben null, ...
//! clean_markup         Markdown, LaTeX, emoji, smileys, arrows, list numbers, e-mail and web addresses
//! expand_units         230 V -> 230 Volt, 1 h -> eine Stunde (units.json)
//! math_symbols         a², √2, 2^10, 5 + 3, 10 - 4
//! numbers              dates, times, money, fractions, ordinals, decimals, plain numbers
//! expand_abbreviations z. B. -> zum Beispiel (abbreviations.json)
//! spell_out_letters    PDF -> peh deh eff, single letters and vowel-less abbreviations
//! ```

mod abbreviations;
mod extra;
mod markup;
mod math;
mod numbers;
mod profile;
mod spans;
mod stream;
mod units;

pub(crate) mod tables {
    include!(concat!(env!("OUT_DIR"), "/de_tables.rs"));
}

pub(crate) use abbreviations::letter_name;
pub use abbreviations::{expand_abbreviations, spell_out_letters, HOLD_BACK};
pub use markup::clean_markup;
pub use math::{latex, math_symbols};
pub(crate) use numbers::cardinal;
pub use numbers::{numbers, phone_numbers};
pub use profile::{Fix, Profile};
pub use stream::StreamNormalizer;
pub use units::expand_units;

use crate::pyre::{re, LazyRe};
use crate::Error;

pub(crate) const MONTHS: &str =
    "Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember";

static SLASH_BETWEEN_WORDS: LazyRe = re!(r"(?<=[^\W\d_])[ \t]*/[ \t]*(?=[^\W\d_])"; needs &["/"]);
static MULTI_BLANK: LazyRe = re!(r"[ \t]{2,}");
static SPACE_BEFORE_PUNCT: LazyRe = re!(r"[ \t]+([,.;:!?])"; needs &[",", ".", ";", ":", "!", "?"]);

/// Prepare German text for a TTS model with the default profile (every fix): clean up, then write out units,
/// formulas, numbers and abbreviations, then spell out letters - in this order.
pub fn try_normalize(text: &str) -> Result<String, Error> {
    try_normalize_with(text, Profile::NRMLZ)
}

/// [`try_normalize`] with a chosen profile; [`Profile::REFERENCE`] is byte-identical to the Python frontend.
pub fn try_normalize_with(text: &str, p: Profile) -> Result<String, Error> {
    if p.has(Fix::CodeSwitching) && text.contains('[') {
        let (protected, spans) = spans::protect(text)?;
        return normalize_stages(&protected, p).map(|out| spans.restore(&out));
    }
    normalize_stages(text, p)
}

fn normalize_stages(text: &str, p: Profile) -> Result<String, Error> {
    // the extra stages are nrmlz's own rules (see extra.rs); with Profile::REFERENCE they pass text through
    let t = extra::identifiers(text, p)?;
    let t = phone_numbers(&t, p)?;
    let t = extra::pre_markup(&t, p)?;
    let t = clean_markup(&t, p)?;
    let t = extra::post_markup(&t, p)?;
    let t = expand_units(&t, p)?;
    let t = math_symbols(&t, p)?;
    let t = abbreviations::abbreviations_before_numbers(&t, p)?;
    let t = extra::pre_numbers(&t, p)?;
    let t = numbers(&t, p)?;
    let t = extra::post_numbers(&t, p)?;
    let t = expand_abbreviations(&t, p)?;
    let t = spell_out_letters(&t, p)?;
    let t = extra::leftover_symbols(&t, p);
    // a slash between words separates them ("Rolls-Royce/Snecma" was read as one word). Only here, after the
    // units: "km/h" is already "Kilometer pro Stunde".
    let t = SLASH_BETWEEN_WORDS.sub_lit(&t, " ")?;
    let t = MULTI_BLANK.sub_lit(&t, " ")?;
    let t = SPACE_BEFORE_PUNCT.sub(&t, r"\1")?; // no space before punctuation
    Ok(if p.has(Fix::TidySpaces) {
        crate::pystr::strip(&t).to_string()
    } else {
        t
    })
}

/// [`try_normalize`], returning the input unchanged where the Python reference would raise.
pub fn normalize(text: &str) -> String {
    normalize_with(text, Profile::NRMLZ)
}

/// [`normalize`] with a chosen profile.
pub fn normalize_with(text: &str, p: Profile) -> String {
    try_normalize_with(text, p).unwrap_or_else(|_| text.to_string())
}

/// Every pattern of the German frontend, compiled once (for warm-up and tests).
pub fn warm_up() {
    let _ = normalize_with(
        "Am 3. Mai um 15:30 Uhr kostet es 24,99 € (ca. 3 kg, z. B. PDF), $x^2$ – 10 - 4 = 6.",
        Profile::NRMLZ,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docstring_examples() {
        assert_eq!(
            normalize("Der Termin ist am 15.11. um 15:30 Uhr."),
            "Der Termin ist am fünfzehnten November um fünfzehn Uhr dreißig."
        );
    }
}
