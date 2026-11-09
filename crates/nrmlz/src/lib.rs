//! nrmlz: text normalisation for speech synthesis.
//!
//! Turns written text (numbers, dates, times, money, units, abbreviations, e-mail and web addresses, formulas,
//! Markdown) into plain spoken words that any TTS model can read.
//!
//! German ([`normalize_de`]) improves on the Articulino frontend (Apache-2.0), preserving exact Python behavior
//! in [`Profile::REFERENCE`]. English ([`normalize_en`]) uses US conventions. A [`Lexicon`] supplies final user
//! replacements; explicit [en]...[/en] spans inside default German use English readings and retain their tags.
//!
//! ```
//! assert_eq!(
//!     nrmlz::normalize_de("Der Termin ist am 15.11. um 15:30 Uhr."),
//!     "Der Termin ist am fünfzehnten November um fünfzehn Uhr dreißig."
//! );
//! ```

pub mod de;
pub mod en;
pub mod es;
pub mod lexicon;
pub mod num;
mod protected;
mod pyre;
#[cfg(feature = "coverage")]
pub use pyre::coverage;
mod pystr;
mod pytables;
mod reading;
pub mod speech;

pub use de::{Fix, Profile};
pub use lexicon::{Lexicon, LexiconStage};
pub use speech::SpeechStyle;

/// Languages with a normaliser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl std::str::FromStr for Lang {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        match s.to_ascii_lowercase().as_str() {
            "de" | "de-de" | "de_de" | "german" | "deutsch" => Ok(Lang::De),
            "en" | "en-us" | "en_us" | "english" => Ok(Lang::En),
            _ => Err(Error::UnknownLanguage(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A number of 10^606 or more had to be spelled out (num2words raises OverflowError there too).
    NumberTooLarge,
    /// The regex engine gave up (backtracking limit); the Python reference has no limit.
    Regex(String),
    UnknownLanguage(String),
    /// A lexicon file could not be read.
    Lexicon(String),
    /// Unknown profile or fix name.
    UnknownProfile(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NumberTooLarge => write!(f, "number too large to spell out (10^606 or more)"),
            Error::Regex(e) => write!(f, "regex engine: {e}"),
            Error::UnknownLanguage(l) => write!(f, "unknown language {l:?} (supported: de, en)"),
            Error::Lexicon(e) => write!(f, "lexicon: {e}"),
            Error::UnknownProfile(e) => write!(f, "profile: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<fancy_regex::Error> for Error {
    fn from(e: fancy_regex::Error) -> Self {
        Error::Regex(e.to_string())
    }
}

/// Normalise German text with the improved nrmlz profile. On a runtime error, return the input unchanged;
/// [`de::try_normalize`] exposes errors. Oversized numbers use digit names in this profile; the frozen reference
/// profile instead reports the original num2words overflow.
pub fn normalize_de(text: &str) -> String {
    let folded = tts_safe(text);
    tts_safe(&de::try_normalize(&folded).unwrap_or_else(|_| folded.clone()))
}

/// Normalise English text using US conventions.
pub fn normalize_en(text: &str) -> String {
    tts_safe(&en::normalize(&tts_safe(text)))
}

/// Normalise `text` in `lang` (nrmlz profile).
pub fn normalize(lang: Lang, text: &str) -> String {
    normalize_with(lang, text, Profile::NRMLZ, None)
}

/// Normalise with a lexicon applied at its configured stage (nrmlz profile).
pub fn normalize_with_lexicon(lang: Lang, text: &str, lexicon: &Lexicon) -> String {
    normalize_with(lang, text, Profile::NRMLZ, Some(lexicon))
}

/// Normalise with a profile and an optional lexicon. With [`Fix::LexiconFinal`] (in the nrmlz profile), the
/// replacements of a `before` lexicon are final: no rule rewrites them.
pub fn normalize_with(
    lang: Lang,
    text: &str,
    profile: Profile,
    lexicon: Option<&Lexicon>,
) -> String {
    normalize_styled(lang, text, profile, lexicon, SpeechStyle::default())
}

/// [`normalize_with`] plus an opt-in [`SpeechStyle`] (spoken letter names, codes read in pieces). A plain
/// style gives exactly the [`normalize_with`] output.
pub fn normalize_styled(
    lang: Lang,
    text: &str,
    profile: Profile,
    lexicon: Option<&Lexicon>,
    style: SpeechStyle,
) -> String {
    // A speech front-end is the main consumer: fold typography the models cannot tokenise
    // unless the caller explicitly asked for the frozen reference profile, whose whole point is
    // byte identity with the Articulino frontend.
    let fold = profile != Profile::REFERENCE;
    let run = |t: &str| {
        let prepared = if fold { tts_safe(t) } else { t.to_string() };
        let lang_rules = |s: &str| match lang {
            Lang::De => de::normalize_with(s, profile),
            Lang::En => en::normalize(s),
        };
        let out = if style.is_plain() {
            lang_rules(&prepared)
        } else {
            styled(lang, &prepared, style, lang_rules)
        };
        // Rules can reintroduce typography from lexicon entries; fold again to be safe.
        if fold {
            tts_safe(&out)
        } else {
            out
        }
    };
    let Some(lexicon) = lexicon else {
        return run(text);
    };
    // Where a dotted term ("O.K.") ends the sentence, its replacement keeps the dot. English reuses its
    // abbreviation rule; German capitalises nouns, so only the end of the text is unambiguous there.
    let dot_ends_sentence: &dyn Fn(&str) -> bool = match lang {
        Lang::En => &en::dot_ends_sentence,
        Lang::De => &lexicon::text_ends,
    };
    match lexicon.stage() {
        LexiconStage::Before => {
            if profile.has(Fix::LexiconFinal) {
                if let Some((protected, replacements)) = lexicon.protect(text, dot_ends_sentence) {
                    return replacements.restore(&run(&protected));
                }
            }
            run(&lexicon.apply_with(text, dot_ends_sentence))
        }
        LexiconStage::After => lexicon.apply_with(&run(text), dot_ends_sentence),
    }
}

fn styled(lang: Lang, text: &str, style: SpeechStyle, rules: impl Fn(&str) -> String) -> String {
    let mut protected = protected::ProtectedText::new(text);
    let mut input = text.to_string();
    if style.phone_chunks {
        input = speech::protect_phones(lang, &input, &mut protected).unwrap_or(input);
    }
    if style.code_digits {
        input = speech::protect_codes(lang, &input, &mut protected).unwrap_or(input);
    }
    let mut out = rules(&input);
    if style.letter_names {
        out = speech::letter_names(lang, &out).unwrap_or(out);
    }
    protected.restore(&out)
}

/// Fold typography that a speech front-end cannot tokenise into the characters it can.
///
/// This is a separate, opt-in pass: it never changes [`normalize`] output, so the frozen
/// reference parity profile stays byte-identical. Call it immediately before synthesis, never
/// as part of normalisation, because it deliberately rewrites punctuation the normaliser treats
/// as meaning-bearing (a quotation mark is not a number format).
///
/// Without it, `„…“` reaches a 4k SentencePiece model as three `<0x..>` byte tokens each, which
/// derails generation: the quoted span is mispronounced or skipped entirely.
pub fn tts_safe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{201e}' | '\u{201c}' | '\u{201d}' | '\u{201f}' | '\u{00ab}' | '\u{00bb}'
            | '\u{2033}' => out.push('"'),
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' | '\u{2032}' | '\u{2039}'
            | '\u{203a}' => out.push('\''),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{00a0}' | '\u{2007}' | '\u{202f}' => out.push(' '),
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' => {}
            other => out.push(other),
        }
    }
    out
}
