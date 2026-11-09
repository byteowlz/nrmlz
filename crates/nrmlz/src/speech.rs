//! Opt-in speech styles for TTS models that read letters and codes poorly.
//!
//! Neither pass changes default normalisation. Both were chosen by a listening/ASR tuner for one model
//! (Foxline's bilingual Pocket TTS); another model may prefer different forms.
//!
//! - `letter_names` (English): standalone capital letters become spoken letter names, "Q X" -> "cue ex",
//!   the clock markers "A M" -> "ae emm" and "P M" -> "pe em". "A" and "I" stay as written (article/pronoun). German
//!   already writes letter names ("kuh iks"), so the pass is a no-op there.
//! - `code_digits`: alphanumeric codes are read in small pieces instead of as one large number:
//!   English "QX-4471" -> "cue ex four four seven one", "flight UA 482" -> "flight you ay four eighty-two";
//!   German "QX-4471" -> "kuh iks, vierundvierzig einundsiebzig".

use crate::protected::ProtectedText;
use crate::pyre::{re, LazyRe, R};
use crate::Lang;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpeechStyle {
    pub letter_names: bool,
    pub code_digits: bool,
    /// Phone numbers as one sentence per digit group, for TTS front ends that synthesise each sentence as its
    /// own chunk (tuner run 2: EN 46% vs 8% as one chunk, DE 83% vs 75%).
    pub phone_chunks: bool,
}

impl SpeechStyle {
    pub fn is_plain(self) -> bool {
        self == SpeechStyle::default()
    }
}

/// English letter names; "A" and "I" keep their written form.
pub(crate) fn en_letter(c: char) -> Option<&'static str> {
    Some(match c {
        'A' => "A",
        'B' => "bee",
        'C' => "see",
        'D' => "dee",
        'E' => "ee",
        'F' => "ef",
        'G' => "gee",
        'H' => "aitch",
        'I' => "I",
        'J' => "jay",
        'K' => "kay",
        'L' => "el",
        'M' => "em",
        'N' => "en",
        'O' => "oh",
        'P' => "pee",
        'Q' => "cue",
        'R' => "ar",
        'S' => "es",
        'T' => "tee",
        'U' => "you",
        'V' => "vee",
        'W' => "double you",
        'X' => "ex",
        'Y' => "why",
        'Z' => "zee",
        _ => return None,
    })
}

/// Spoken names for a run of letters. English: "A" stays only as the first letter of a run (after another
/// letter it reads as the article, tuner run 2), "I" stays only on its own (the pronoun).
fn letters(lang: Lang, s: &str) -> String {
    let run: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    run.iter()
        .enumerate()
        .filter_map(|(i, &c)| match (lang, c) {
            (Lang::En, 'A') if i > 0 => Some("ay"),
            (Lang::En, 'I') if run.len() > 1 => Some("eye"),
            (Lang::En, _) => en_letter(c),
            (Lang::De, _) => crate::de::letter_name(c),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn small(lang: Lang, n: u64) -> String {
    match lang {
        Lang::En => crate::num::en::cardinal(n),
        Lang::De => crate::de::cardinal(n),
    }
}

fn digit_words(lang: Lang, s: &str) -> String {
    s.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| match (lang, d) {
            (Lang::En, _) => crate::num::en::cardinal(d as u64),
            (Lang::De, 1) => "eins".to_string(),
            (Lang::De, _) => crate::de::cardinal(d as u64),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Digit pairs from the right end: "4471" -> "44 71", "482" -> "4 82"; a pair with a leading zero is read
/// digit by digit ("07" -> "zero seven" / "null sieben").
fn pairs(lang: Lang, s: &str) -> String {
    let (head, rest) = s.split_at(s.len() % 2);
    let mut out = Vec::new();
    if !head.is_empty() {
        out.push(digit_words(lang, head));
    }
    for pair in rest.as_bytes().chunks(2) {
        let p = std::str::from_utf8(pair).unwrap();
        if p.starts_with('0') {
            out.push(digit_words(lang, p));
        } else {
            out.push(small(lang, p.parse().unwrap()));
        }
    }
    out.join(" ")
}

static CODE: LazyRe = re!(r"(?<![\w-])([A-Z]{1,3})-?([0-9]{3,})(?![\w-])");
static FLIGHT: LazyRe = re!(r"\b([Ff]light)([ \t]+)([A-Z]{2})[ \t]?([0-9]{1,4})\b");

/// Replace codes with their final spoken form, protected from the normaliser.
pub(crate) fn protect_codes(lang: Lang, text: &str, protected: &mut ProtectedText) -> R<String> {
    let text = if lang == Lang::En {
        FLIGHT.sub_fn(text, |m| {
            let spoken = format!("{} {}", letters(lang, m.g(3)), pairs(lang, m.g(4)));
            Ok(format!("{}{}{}", m.g(1), m.g(2), protected.insert(spoken)))
        })?
    } else {
        text.to_string()
    };
    CODE.sub_fn(&text, |m| {
        let spoken = match lang {
            Lang::En => format!("{} {}", letters(lang, m.g(1)), digit_words(lang, m.g(2))),
            Lang::De => format!("{}, {}", letters(lang, m.g(1)), pairs(lang, m.g(2))),
        };
        Ok(protected.insert(spoken))
    })
}

static EN_PHONE: LazyRe =
    re!(r"(?<![\w+(-])(?:\+?1[ .-]?)?\(?([0-9]{3})\)?[ .-]?([0-9]{3})[ .-]([0-9]{4})(?![\w-])");
static DE_PHONE: LazyRe =
    re!(r"(?<![\w+-])((?:\+49[ ]?|0)[0-9]{2,4})((?:[ /-][0-9]{2,8}){1,3})(?![\w-])");
static SPACED_DIGITS: LazyRe = re!(r"(?<![\w+])([0-9](?:[ \t]+[0-9]){6,})(?![\w])");

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Digit groups as written; a group longer than five digits splits into three-digit pieces from the left.
fn phone_groups(groups: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for g in groups {
        let digits: String = g.chars().filter(char::is_ascii_digit).collect();
        if digits.len() > 5 {
            let mut rest = digits.as_str();
            while rest.len() > 3 {
                out.push(rest[..3].to_string());
                rest = &rest[3..];
            }
            out.push(rest.to_string());
        } else if !digits.is_empty() {
            out.push(digits);
        }
    }
    out
}

/// One sentence per group: the first group ends the running sentence, later groups start new ones, and the
/// last group takes a comma when the sentence goes on ("..., before Friday.").
fn phone_sentences(lang: Lang, groups: &[String], after: &str) -> String {
    let words: Vec<String> = groups.iter().map(|g| digit_words(lang, g)).collect();
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            out.push_str(w);
        } else {
            out.push_str(". ");
            out.push_str(&capitalise(w));
        }
    }
    if words.len() > 1
        && after
            .trim_start()
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric())
    {
        out.push(',');
    }
    out
}

pub(crate) fn protect_phones(lang: Lang, text: &str, protected: &mut ProtectedText) -> R<String> {
    let text = SPACED_DIGITS.sub_fn(text, |m| {
        let digits: String = m.all().chars().filter(char::is_ascii_digit).collect();
        let groups = phone_groups(&[&digits]);
        Ok(protected.insert(phone_sentences(lang, &groups, m.after())))
    })?;
    match lang {
        Lang::En => EN_PHONE.sub_fn(&text, |m| {
            let groups = phone_groups(&[m.g(1), m.g(2), m.g(3)]);
            Ok(protected.insert(phone_sentences(lang, &groups, m.after())))
        }),
        Lang::De => DE_PHONE.sub_fn(&text, |m| {
            let total: usize = m.all().chars().filter(char::is_ascii_digit).count();
            if !(8..=14).contains(&total) {
                return Ok(m.all().to_string());
            }
            let mut parts = vec![m.g(1).trim_start_matches("+49").trim()];
            parts.extend(m.g(2).split([' ', '/', '-']).filter(|s| !s.is_empty()));
            let groups = phone_groups(&parts);
            Ok(protected.insert(phone_sentences(lang, &groups, m.after())))
        }),
    }
}

static CAPITAL_RUN: LazyRe = re!(r"(?<![\w'’.-])([A-Z](?:[ \t][A-Z])*)(?![\w'’-])");

/// English spoken letter names for standalone capitals left by normalisation ("Q X", "A M", "Plan B").
/// The clock markers read "ae emm" (a.m.; owner choice, tuner run 7: 88% vs 83% for "A em") and "pe em"
/// (p.m.; owner choice, tuner run 6: 100% like "pee em"); lone letters elsewhere keep their names.
pub(crate) fn letter_names(lang: Lang, text: &str) -> R<String> {
    if lang != Lang::En {
        return Ok(text.to_string());
    }
    CAPITAL_RUN.sub_fn(text, |m| {
        let run: String = m.g(1).chars().filter(|c| !c.is_whitespace()).collect();
        Ok(match run.as_str() {
            "AM" => "ae emm".to_string(),
            "PM" => "pe em".to_string(),
            _ => letters(lang, m.g(1)),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{normalize_styled, Profile};

    fn en(text: &str, style: SpeechStyle) -> String {
        normalize_styled(Lang::En, text, Profile::NRMLZ, None, style)
    }

    fn de(text: &str, style: SpeechStyle) -> String {
        normalize_styled(Lang::De, text, Profile::NRMLZ, None, style)
    }

    const BOTH: SpeechStyle = SpeechStyle {
        letter_names: true,
        code_digits: true,
        phone_chunks: false,
    };

    #[test]
    fn plain_style_is_default_output() {
        for t in [
            "The code is QX-4471.",
            "Boards at 6:45 a.m. on Friday.",
            "Your flight UA 482 is late.",
        ] {
            assert_eq!(en(t, SpeechStyle::default()), crate::normalize(Lang::En, t));
        }
    }

    #[test]
    fn english_tuner_winners() {
        assert_eq!(
            en("The tracking code is QX-4471.", BOTH),
            "The tracking code is cue ex four four seven one."
        );
        // Tuner run 2: "you A four eighty-two" 17% vs "you ay four eighty-two" 71%: after another letter a
        // bare A reads as the article.
        assert_eq!(
            en("Your flight UA 482 boards now.", BOTH),
            "Your flight you ay four eighty-two boards now."
        );
        assert_eq!(
            en("The code is QA-1234.", BOTH),
            "The code is cue ay one two three four."
        );
        assert_eq!(
            en("The code is AB-1234.", BOTH),
            "The code is A bee one two three four."
        );
        // Owner choice for a.m.; tuner run 7 (Parakeet): "ae emm" 88% vs "A em" 83%.
        assert_eq!(
            en("Boards at 6:45 a.m. on Friday.", BOTH),
            "Boards at six forty-five ae emm on Friday."
        );
        assert_eq!(en("Wake me at 6 AM.", BOTH), "Wake me at six ae emm.");
        // Owner choice for p.m. after listening; tuner run 6 (Parakeet): "pe em" 100%, same as "pee em".
        assert_eq!(
            en("Call before 5:30 p.m. on Friday.", BOTH),
            "Call before five thirty pe em on Friday."
        );
        assert_eq!(
            en("Doors open at 7 PM tonight.", BOTH),
            "Doors open at seven pe em tonight."
        );
        // A lone P outside the clock marker keeps its letter name.
        assert_eq!(
            en("The code is P-1234.", BOTH),
            "The code is pee one two three four."
        );
    }

    #[test]
    fn english_letters_spare_words_and_pronouns() {
        assert_eq!(
            en("I think Plan B is fine.", BOTH),
            "I think Plan bee is fine."
        );
        assert_eq!(en("A cat sat.", BOTH), "A cat sat.");
        // Initialisms that normalisation already spells become letter names too.
        assert_eq!(en("It's U.S. policy.", BOTH), "It's you es policy.");
    }

    #[test]
    fn german_codes_in_pairs() {
        assert_eq!(
            de("Die Sendungsnummer ist QX-4471.", BOTH),
            "Die Sendungsnummer ist kuh iks, vierundvierzig einundsiebzig."
        );
        assert_eq!(
            de("Code AB-0712 bitte.", BOTH),
            "Code ah beh, null sieben zwölf bitte."
        );
        assert_eq!(
            de("Gate B12.", BOTH),
            de("Gate B12.", SpeechStyle::default())
        );
    }

    const PHONE: SpeechStyle = SpeechStyle {
        letter_names: false,
        code_digits: false,
        phone_chunks: true,
    };

    #[test]
    fn phone_numbers_become_one_sentence_per_digit_group() {
        // Tuner run 2: separate synthesis chunks per group, EN 46% vs 8%, DE 83% vs 75%.
        assert_eq!(
            en("Please call me back at 415-555-0199 before Friday.", PHONE),
            "Please call me back at four one five. Five five five. Zero one nine nine, before Friday."
        );
        assert_eq!(
            en("Call (415) 555-0199.", PHONE),
            "Call four one five. Five five five. Zero one nine nine."
        );
        assert_eq!(
            de("Ruf mich bitte unter 0151 2345 6789 zurück.", PHONE),
            "Ruf mich bitte unter null eins fünf eins. Zwei drei vier fünf. Sechs sieben acht neun, zurück."
        );
        assert_eq!(
            de("Die Nummer ist 0711-123456.", PHONE),
            "Die Nummer ist null sieben eins eins. Eins zwei drei. Vier fünf sechs."
        );
        // Not phone numbers: years, prices, plain counts stay with the normal rules.
        for t in ["In 2019 we sold 4471 units.", "It costs 1,234 dollars."] {
            assert_eq!(en(t, PHONE), crate::normalize(Lang::En, t));
        }
        let t = "Im Jahr 2019 kamen 4471 Gäste.";
        assert_eq!(de(t, PHONE), crate::normalize(Lang::De, t));
    }

    #[test]
    fn pairs_from_the_right() {
        assert_eq!(pairs(Lang::En, "482"), "four eighty-two");
        assert_eq!(pairs(Lang::En, "1207"), "twelve zero seven");
        assert_eq!(pairs(Lang::De, "4471"), "vierundvierzig einundsiebzig");
        assert_eq!(pairs(Lang::De, "101"), "eins null eins");
    }
}
