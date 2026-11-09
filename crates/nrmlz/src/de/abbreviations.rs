//! Abbreviations and symbols from data/de/abbreviations.json (z. B. -> zum Beispiel, % -> Prozent), and the
//! spelling of upper-case abbreviations and single letters. Port of the "abbreviations" and "spelling out"
//! sections of german.py.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::{char_len, is_upper_str, strip};

use super::tables::{
    ABBREVIATIONS, NRMLZ_ABBREVIATIONS, NRMLZ_BEFORE_NAME, NRMLZ_BEFORE_NUMBER, SYMBOLS,
};
use super::units::escape_py;
use super::{Fix, Profile};

/// Characters held back at the end of a stream buffer: the longest abbreviation plus a margin.
pub const HOLD_BACK: usize = 16;

// If an abbreviation ends with a period, that period was often also the end of the sentence ("im 3. Jh. v. Chr.
// Er zeigte"); written out, the pause would be lost. The period stays if a typical sentence start follows.
static SENTENCE_START: LazyRe = re!(concat!(
    r"\s+(?:Er|Sie|Es|Der|Die|Das|Den|Dem|Ein|Eine|Einen|Danach|Dann|",
    r"Später|Damals|Im|In|Am|Auch|Doch|Aber|Wir|Ich|Man|Dies|Diese|Dieser|",
    r"Dieses|So|Heute|Seit|Bis|Nach|Vor|Als|Wenn|Weil|Hier|Dort|Zudem|",
    r"Außerdem|Deshalb|Daher|Trotzdem|Jedoch|Allerdings)\b"
));

pub(crate) struct Rule {
    pub rx: Re,
    replacement: &'static str,
    keep_period: bool,
    /// A respelling (loanword or acronym pronounced as a word), not an abbreviation: off with
    /// Fix::NoRespellings.
    respelling: bool,
    /// Literal text every match starts with (the key up to its first space): a cheap pre-check.
    prefix: String,
}

impl Rule {
    pub fn may_match(&self, text: &str) -> bool {
        text.contains(self.prefix.as_str())
    }
}

/// Compiled rules: abbreviations first (longest first), then symbols.
pub(crate) static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let mut rules = Vec::new();
    // Longest first, otherwise "Mi." would match before "Mio." and leave the "o." behind. (stable sort)
    let mut abbr: Vec<(&str, &str)> = ABBREVIATIONS.to_vec();
    abbr.sort_by_key(|(k, _)| std::cmp::Reverse(char_len(k)));
    for (k, v) in abbr {
        let pat = escape_py(k).replace("\\ ", r"\s*");
        // no letter before and none after, otherwise "Str." would also match inside a word
        rules.push(Rule {
            rx: Re::new(&format!(
                r"(?<![\w\u00c0-\u024f]){pat}(?![\w\u00c0-\u024f])"
            )),
            replacement: v,
            keep_period: k.ends_with('.'),
            respelling: RESPELLINGS.contains(&k),
            prefix: k.split(' ').next().unwrap().to_string(),
        });
    }
    let mut symbols: Vec<(&str, &str)> = SYMBOLS.to_vec();
    symbols.sort_by_key(|(k, _)| std::cmp::Reverse(char_len(k)));
    for (k, v) in symbols {
        rules.push(Rule {
            rx: Re::new(&escape_py(k)),
            replacement: v,
            keep_period: false,
            respelling: false,
            prefix: k.to_string(),
        });
    }
    rules
});

/// nrmlz's additional abbreviations (Fix::MoreAbbreviations): plain ones and those before a name. They run
/// before the reference rules, so that "Fr. Müller" is "Frau Müller" while "Fr., 3. Mai" stays "Freitag".
static NRMLZ_RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let mut rules = Vec::new();
    let mut plain: Vec<(&str, &str)> = NRMLZ_ABBREVIATIONS.to_vec();
    plain.sort_by_key(|(k, _)| std::cmp::Reverse(char_len(k)));
    for (k, v) in plain {
        let pat = escape_py(k).replace("\\ ", r"\s*");
        rules.push(Rule {
            rx: Re::new(&format!(
                r"(?<![\w\u00c0-\u024f]){pat}(?![\w\u00c0-\u024f])"
            )),
            replacement: v,
            keep_period: k.ends_with('.'),
            respelling: false,
            prefix: k.split(' ').next().unwrap().to_string(),
        });
    }
    for (k, v) in NRMLZ_BEFORE_NAME {
        let pat = escape_py(k);
        rules.push(Rule {
            rx: Re::new(&format!(
                r"(?<![\w\u00c0-\u024f]){pat}(?=[ \t]*[A-ZÄÖÜ][a-zäöüß]+\b)(?![ \t]*(?:{}|{})\b)",
                super::MONTHS,
                "Montag|Dienstag|Mittwoch|Donnerstag|Freitag|Samstag|Sonntag|Morgen|Abend|Mittag|Nachmittag|Vormittag|Nacht"
            )),
            replacement: v,
            keep_period: false,
            respelling: false,
            prefix: k.to_string(),
        });
    }
    rules
});

/// "S. 12" -> "Seite 12", "Abs. 1", "Art. 3" (before numbers are written out; Fix::MoreAbbreviations).
static NRMLZ_NUMBER_RULES: LazyLock<Vec<(Re, &'static str, String)>> = LazyLock::new(|| {
    NRMLZ_BEFORE_NUMBER
        .iter()
        .map(|(k, v)| {
            let rx = Re::new(&format!(
                r"(?<![\w\u00c0-\u024f]){}(?=[ \t]*(?:\d|[IVX]+\b))",
                escape_py(k)
            ));
            (rx, *v, k.to_string())
        })
        .collect()
});

pub(crate) fn abbreviations_before_numbers(text: &str, p: Profile) -> R<String> {
    if !p.has(Fix::MoreAbbreviations) {
        return Ok(text.to_string());
    }
    let mut text = text.to_string();
    for (rx, v, key) in NRMLZ_NUMBER_RULES.iter() {
        if text.contains(key.as_str()) {
            text = rx.sub_lit(&text, v)?;
        }
    }
    Ok(text)
}

/// Entries of abbreviations.json that change how a word is written to steer its pronunciation (respellings)
/// rather than expand an abbreviation. Fix::NoRespellings leaves them as written. Kept: expansions ("z. B."),
/// initialisms read by letter names ("GmbH"), symbol readings ("C++"), "PLZ".
pub(crate) static RESPELLINGS: &[&str] = &[
    "ui", "nginx", "Nginx", "NGINX", "Homebrew", "homebrew", "TOML", "RAM", "NASA", "NATO",
    "UNESCO", "UNICEF", "UNO", "TÜV", "PIN", "TAN", "JSON", "WLAN", "iOS", "macOS", "ok", "OK",
    "Ok", "FIFA", "UEFA", "IKEA", "DAX", "ASCII", "GIF", "JPEG", "BIOS", "AIDS", "OPEC", "COVID",
    "okay", "Okay",
];

/// All-caps entries of the table (RAM -> Ramm), also for plurals ("RAMs").
static ACRONYM_WORDS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let caps = Re::new(r"\A[A-ZÄÖÜ]{2,6}\Z");
    ABBREVIATIONS
        .iter()
        .filter(|(k, _)| caps.is_match(k).unwrap())
        .copied()
        .collect()
});

// "Mo–Fr" is a range, not two abbreviations with a dash.
static WEEKDAY_RANGE: LazyRe = re!(concat!(
    r"\b(Mo|Di|Mi|Do|Fr|Sa|So)\.?[ \t]*[–-][ \t]*(Mo|Di|Mi|Do|Fr|Sa|So)\b",
    r"(?:\.(?=[ \t]*[\da-zäöüß]))?"
); needs &["-", "–"]);
static MULTI_BLANK: LazyRe = re!(r"[ \t]{2,}");

fn weekday(abbr: &str) -> &'static str {
    match abbr {
        "Mo" => "Montag",
        "Di" => "Dienstag",
        "Mi" => "Mittwoch",
        "Do" => "Donnerstag",
        "Fr" => "Freitag",
        "Sa" => "Samstag",
        _ => "Sonntag",
    }
}

fn keep_period(replacement: &str, m: &Match) -> R<String> {
    // also at the end of the text: the period was the end of the sentence as well
    let end = strip(m.after()).is_empty();
    let dot = end || SENTENCE_START.match_at(m.text, m.end())?.is_some();
    Ok(if dot {
        format!("{replacement}.")
    } else {
        replacement.to_string()
    })
}

/// Write out abbreviations and symbols from abbreviations.json (z. B. -> zum Beispiel, % -> Prozent).
pub fn expand_abbreviations(text: &str, p: Profile) -> R<String> {
    if text.is_empty() {
        return Ok(text.to_string());
    }
    let mut text = WEEKDAY_RANGE.sub_fn(text, |m| {
        Ok(format!("{} bis {}", weekday(m.g(1)), weekday(m.g(2))))
    })?;
    let extra: &[Rule] = if p.has(Fix::MoreAbbreviations) {
        &NRMLZ_RULES
    } else {
        &[]
    };
    for rule in extra.iter().chain(RULES.iter()) {
        if !rule.may_match(&text) || (rule.respelling && p.has(Fix::NoRespellings)) {
            continue;
        }
        text = if rule.keep_period {
            rule.rx
                .sub_fn(&text, |m| keep_period(rule.replacement, m))?
        } else {
            rule.rx.sub_lit(&text, rule.replacement)?
        };
    }
    MULTI_BLANK.sub_lit(&text, " ")
}

// ------------------------------------------------------------------------------------------- spelling out
//
// The model reads letter by letter after lower-casing: "PDF" becomes the made-up word "pdf". So upper-case
// abbreviations are written out with German letter names. Abbreviations spoken as a word (NASA, TÜV, PIN) are
// in abbreviations.json and replaced before. Real words in capitals (NICHT, ACHTUNG) stay.

pub(crate) fn letter_name(c: char) -> Option<&'static str> {
    Some(match c {
        'A' => "ah",
        'B' => "beh",
        'C' => "zeh",
        'D' => "deh",
        'E' => "eh",
        'F' => "eff",
        'G' => "geh",
        'H' => "hah",
        'I' => "ih",
        'J' => "jott",
        'K' => "kah",
        'L' => "ell",
        'M' => "emm",
        'N' => "enn",
        'O' => "oh",
        'P' => "peh",
        'Q' => "kuh",
        'R' => "err",
        'S' => "ess",
        'T' => "teh",
        'U' => "uh",
        'V' => "fau",
        'W' => "weh",
        'X' => "iks",
        'Y' => "üpsilon",
        'Z' => "zett",
        'Ä' => "äh",
        'Ö' => "öh",
        'Ü' => "üh",
        _ => return None,
    })
}

/// Python `c.upper()` looked up in the letter table (ß upper-cases to "SS", which is not in it).
fn letter_of_upper(c: char) -> Option<&'static str> {
    let mut up = c.to_uppercase();
    let first = up.next()?;
    if up.next().is_some() {
        return None;
    }
    letter_name(first)
}

// short German words written in capitals ("DAS IST GUT"): not spelled out
static SHORT_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    "AB AN AM AUF AUS BEI BIS DA DAS DEM DEN DER DES DIE DU EIN ER ES FÜR GUT HER HIN ICH IHR IM IN IST JA \
     JE MAL MIT NEU NIE NUN NUR OB OFT SIE SO TAG UM UND UNS VOM VON VOR WAS WER WIE WIR WO ZU ZUM ZUR ALT NEIN DOCH NOCH \
     SEHR HIER JETZT BITTE DANKE ALLE ODER ABER AUCH"
        .split_whitespace()
        .collect()
});
// longer abbreviations with a vowel that are spelled out anyway (otherwise: 4+ letters with a vowel = a word)
static ALWAYS_SPELLED: &[&str] = &[
    "ADAC", "DSGVO", "ISBN", "SUV", "DFB", "HTTPS", "VPN", "UKW", "IHK", "AOK", "ABS", "GPS",
    "USB", "FAQ", "EDV", "OECD", "UNHCR", "WHO", "AGB", "BGB", "STVO", "IOS", "USA", "EU", "ARD",
    "CDU", "CSU", "SPD", "FDP", "AFD", "ICE", "IC", "DB", "ÖPNV", "HIV",
];
static INTERJECTIONS: &[&str] = &[
    "hm", "hmm", "pst", "psst", "brr", "grr", "pff", "pfff", "tss", "sch", "st", "ts", "pscht",
];

// Single letters (formulas, "Plan B") are spoken by their names. Not: hyphenated words (U-Bahn, E-Mail, x-mal)
// and the English "I".
static SINGLE_LETTER: LazyRe = re!(r"(?<![\wÀ-ɏ'’.-])([a-zA-ZäöüÄÖÜ])(?![\wÀ-ɏ'’-]|\.[a-zA-Z])");
// lower-case runs without a vowel (paths, short forms: "srv", "mnt")
static NO_VOWEL: LazyRe = re!(r"(?<![\wÀ-ɏ'’-])([bcdfghjklmnpqrstvwxzß]{2,5})(?![\wÀ-ɏ'’-])");
static UPPERCASE_ABBR: LazyRe = re!(r"(?<![\wÀ-ɏ])([A-ZÄÖÜ]{2,6})(s?)(?![\wÀ-ɏ])");
static ROMAN: LazyRe = re!(r"\A[IVX]+\Z");
// Fix::Initialisms: a lower-case letter, then upper-case ones (pH, mRNA, eID)
static MIXED_INITIALISM: LazyRe = re!(r"(?<![\wÀ-ɏ'’-])[a-z][A-ZÄÖÜ]{1,4}(?![\wÀ-ɏ'’-])");
static WORD_BEFORE: LazyRe = re!(r"([A-Za-zÄÖÜäöüß]+)[ \t]+$");
static WORD_AFTER: LazyRe = re!(r"[ \t]+([A-Za-zÄÖÜäöüß]+)");

fn has_vowel(w: &str) -> bool {
    w.chars().any(|c| "AEIOUÄÖÜY".contains(c))
}

fn spelled(word: &str, plural: &str) -> String {
    let names: Vec<&str> = word.chars().filter_map(letter_of_upper).collect();
    names.join(" ") + plural
}

/// A neighbour word also in capitals ("BITTE SOFORT MELDEN") means these are words - but only if one of them
/// is a long word with a vowel. Only direct neighbours without punctuation in between.
fn shouting(m: &Match) -> R<bool> {
    let before = WORD_BEFORE.search(m.before())?;
    let after = WORD_AFTER.match_at(m.after(), 0)?;
    let neighbours: Vec<&str> = [before, after]
        .into_iter()
        .flatten()
        .map(|x| x.g(1))
        .filter(|x| is_upper_str(x) && char_len(x) >= 2)
        .collect();
    let long_word = neighbours
        .iter()
        .chain(std::iter::once(&m.g(1)))
        .any(|x| char_len(x) >= 4 && has_vowel(x));
    Ok(long_word && !neighbours.is_empty())
}

fn replace_upper(m: &Match, p: Profile) -> R<String> {
    let (w, plural) = (m.g(1), m.g(2));
    if SHORT_WORDS.contains(w) || ROMAN.is_match(w)? {
        return Ok(m.all().to_string()); // Roman numerals (Ludwig II) stay
    }
    if p.has(Fix::NoRespellings) && ACRONYM_WORDS.contains_key(w) && RESPELLINGS.contains(&w) {
        return Ok(m.all().to_string()); // spoken as a word (NASA, PIN, OK): left as written
    }
    if let Some(word) = ACRONYM_WORDS.get(w) {
        return Ok(format!("{word}{plural}")); // plural of a table word: RAMs -> Ramms
    }
    if ALWAYS_SPELLED.contains(&w) || !has_vowel(w) {
        return Ok(spelled(w, plural));
    }
    if shouting(m)? {
        return Ok(m.all().to_string());
    }
    if char_len(w) <= 3 {
        return Ok(spelled(w, plural));
    }
    Ok(m.all().to_string()) // 4+ letters with a vowel: a word (NICHT)
}

/// Spell out upper-case abbreviations, single letters and vowel-less runs with German letter names.
pub fn spell_out_letters(text: &str, p: Profile) -> R<String> {
    if text.is_empty() {
        return Ok(text.to_string());
    }
    let text = UPPERCASE_ABBR.sub_fn(text, |m| replace_upper(m, p))?;
    let text = if p.has(Fix::Initialisms) {
        MIXED_INITIALISM.sub_fn(&text, |m| Ok(spelled(m.all(), "")))?
    } else {
        text
    };
    let text = NO_VOWEL.sub_fn(&text, |m| {
        let run = m.g(1);
        if INTERJECTIONS.contains(&run) {
            return Ok(m.all().to_string());
        }
        Ok(run
            .chars()
            .filter_map(letter_of_upper)
            .collect::<Vec<_>>()
            .join(" "))
    })?;
    SINGLE_LETTER.sub_fn(&text, |m| {
        let c = m.g(1);
        if c == "I" {
            return Ok(m.all().to_string());
        }
        Ok(letter_of_upper(c.chars().next().unwrap())
            .unwrap()
            .to_string())
    })
}
