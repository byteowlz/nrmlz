//! nrmlz's own German rules (the [`Fix`]es beyond the reference frontend). They run as extra stages around
//! the reference stages; with [`Profile::REFERENCE`] none of them runs.
//!
//! ```text
//! identifiers   (before phone_numbers)  IBAN, ISBN, PIN/Postfach/Notruf digit by digit, hyphen chains, ISO dates
//! pre_markup    (before clean_markup)   URLs with a scheme, LaTeX without $, hashtags and mentions
//! post_markup   (before expand_units)   dates, Unicode fractions, coordinates, Roman numerals, v1.2
//! pre_numbers   (before numbers)        durations, scores, decades, ordinal agreement, signs and comparisons
//! post_numbers  (after numbers)         "bei einer Million"
//! ```

use std::sync::LazyLock;

use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::{capitalize, small_int};

use super::abbreviations::letter_name;
use super::markup::{domain, tld_name};
use super::math::latex_to_text;
use super::numbers::{cardinal, cardinal_str, integer, ordinal_str};
use super::units::{article, ACCUSATIVE, DATIVE, TABLE as UNIT_TABLE};
use super::{Fix, Profile, MONTHS};

// ------------------------------------------------------------------------------------------- helpers

/// Digits one by one ("4711" -> "vier sieben eins eins"); other characters are dropped.
fn digit_words(s: &str) -> String {
    s.chars()
        .filter_map(crate::pystr::decimal_value)
        .map(|d| cardinal(d as u64))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Letters by their German names, digits one by one ("DE89" -> "deh eh acht neun").
fn spell_code(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for c in s.chars() {
        if let Some(d) = crate::pystr::decimal_value(c) {
            out.push(cardinal(d as u64));
        } else if let Some(name) = c.to_uppercase().next().and_then(letter_name) {
            out.push(name.to_string());
        }
    }
    out.join(" ")
}

fn month_name(n: u64) -> &'static str {
    MONTHS.split('|').nth(n as usize - 1).unwrap()
}

/// Month number of a German or English month abbreviation.
fn month_abbr(s: &str) -> Option<u64> {
    let s = s.trim_end_matches('.').to_lowercase();
    Some(match s.as_str() {
        "jan" | "jän" => 1,
        "feb" | "febr" => 2,
        "mär" | "mrz" | "mar" => 3,
        "apr" => 4,
        "mai" | "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" | "sept" => 9,
        "okt" | "oct" => 10,
        "nov" => 11,
        "dez" | "dec" => 12,
        _ => return None,
    })
}

const MONTH_ABBRS: &str =
    "Jan|Jän|Febr|Feb|Mär|Mrz|Mar|Apr|May|Jun|Jul|Aug|Sept|Sep|Okt|Oct|Nov|Dez|Dec";

// ------------------------------------------------------------------------------------------- identifiers

static IBAN: LazyRe = re!(r"\b([A-Z]{2}\d{2})((?:[ ]?[0-9A-Z]{4}){3,7})(?:[ ]?([0-9A-Z]{1,3}))?\b");
static ISBN: LazyRe = re!(
    r"\b(ISBN(?:-1[03])?(?:-Nummer)?:?[ \t]*)((?:97[89][- ]?)?\d{1,5}[- ]?\d{1,7}[- ]?\d{1,7}[- ]?[\dX])\b"
);
static KEYWORD_DIGITS: LazyRe = re!(concat!(
    r"\b((?:PIN|TAN|PUK|Postfach|Steuernummer|Steuer-ID|Kundennummer|Vertragsnummer|Bestellnummer|",
    r"Rechnungsnummer|Auftragsnummer|Artikelnummer|Seriennummer|Modellnummer|Ticketnummer|Aktenzeichen|",
    r"Notruf|Notrufnummer|Durchwahl|Kontonummer|BLZ|Bankleitzahl|Sendungsnummer|Trackingnummer)",
    r"(?:-Nummer|-Code)?(?:[ \t]*(?::|ist|lautet|Nr\.?|Nummer|lautet:))*[ \t]*)",
    r"(\d+(?:[ /-]\d+)*)(?![\d.,]\d)"
));
static ISO_DATE: LazyRe = re!(r"(?<![\w.-])(\d{4})-(\d{2})-(\d{2})(?![\w-])");
static HYPHEN_CHAIN: LazyRe = re!(r"(?<![\w.,/-])\d+(?:-\d+){2,}(?![\w/-]|[.,]\d)");

static HUGE_NUMBER: LazyRe = re!(r"\d{1,3}(?:\.\d{3}){150,}|\d{500,}");

/// IBAN, ISBN, numbers after PIN/Postfach/Notruf/..., ISO dates and chains like 978-3-16 (Fix::Identifiers).
/// Numbers too long for number words are read digit by digit (Fix::NoLeftovers; the reference fails there).
pub fn identifiers(text: &str, p: Profile) -> R<String> {
    if !super::numbers::has_digit(text) {
        return Ok(text.to_string());
    }
    let text = if p.has(Fix::NoLeftovers) {
        HUGE_NUMBER.sub_fn(text, |m| Ok(digit_words(m.all())))?
    } else {
        text.to_string()
    };
    if !p.has(Fix::Identifiers) {
        return Ok(text);
    }
    let text = IBAN.sub_fn(&text, |m| {
        let mut groups = vec![spell_code(m.g(1))];
        for g in m.g(2).split(' ').filter(|g| !g.is_empty()) {
            // groups of four without spaces: split them
            let chars: Vec<char> = g.chars().collect();
            for c in chars.chunks(4) {
                groups.push(spell_code(&c.iter().collect::<String>()));
            }
        }
        if let Some(rest) = m.get(3) {
            groups.push(spell_code(rest));
        }
        Ok(groups.join(", "))
    })?;
    let text = ISBN.sub_fn(&text, |m| {
        let groups: Vec<String> = m
            .g(2)
            .split(['-', ' '])
            .filter(|g| !g.is_empty())
            .map(spell_code)
            .collect();
        Ok(format!("{}{}", m.g(1), groups.join(", ")))
    })?;
    let text = KEYWORD_DIGITS.sub_fn(&text, |m| {
        let groups: Vec<String> = m.g(2).split([' ', '/', '-']).map(digit_words).collect();
        Ok(format!("{}{}", m.g(1), groups.join(", ")))
    })?;
    let text = ISO_DATE.sub_fn(&text, |m| {
        let (y, mo, d) = (m.g(1), small_int(m.g(2)), small_int(m.g(3)));
        if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
            return Ok(m.all().to_string());
        }
        Ok(format!("{d}. {} {y}", month_name(mo)))
    })?;
    // three or more numbers joined by hyphens are no range: codes (978-3-16-148410-0), records (7-2-1)
    HYPHEN_CHAIN.sub_fn(&text, |m| {
        let groups: Vec<String> = m
            .all()
            .split('-')
            .map(|g| {
                if g.chars().count() >= 3 {
                    Ok(digit_words(g))
                } else {
                    cardinal_str(g)
                }
            })
            .collect::<R<_>>()?;
        Ok(groups.join(", "))
    })
}

// ------------------------------------------------------------------------------------------- before markup

static SCHEME_URL: LazyRe =
    re!(r"\b(?:https?|ftp|sftp)://((?:[\w-]+\.)+([A-Za-z]{2,24}))\b((?:/[\w~%+.-]*)*)(?:\?\S*)?");
static BARE_LATEX: LazyRe = re!(concat!(
    r"(?<![$\\\w])\\(?:frac|dfrac|tfrac|sqrt|alpha|beta|gamma|delta|epsilon|theta|lambda|mu|pi|sigma|phi|omega|",
    r"cdot|times|div|pm|approx|neq|leq|geq|infty|sum|int|lim|partial|nabla|Delta|Omega)",
    r"(?:\[[^\]]*\])?(?:\{(?:[^{}]|\{[^{}]*\})*\})*(?:\s*\^\s*(?:\{[^{}]*\}|\w))?"
));
static HASHTAG: LazyRe = re!(r"(?<![\w#&])#(?=[^\W\d_])");
static MENTION: LazyRe = re!(r"(?<![\w.@])@(?=[A-Za-z_][\w.]*\w)");

fn url_with_scheme(m: &Match) -> R<String> {
    let host = m.g(1);
    let tld = m.g(2).to_lowercase();
    let spoken_host = if tld_name(&tld).is_some() {
        domain(host)
    } else {
        // unknown ending (youtu.be): letters spelled when short, else read as a word
        let head = &host[..host.len() - tld.len()];
        let tld_words = if tld.len() <= 3 {
            spell_code(&tld)
        } else {
            tld.clone()
        };
        let head_words = domain(head.trim_end_matches('.').to_string().as_str());
        format!("{head_words} Punkt {tld_words}")
    };
    let path: Vec<&str> = m.g(3).split('/').filter(|x| !x.is_empty()).collect();
    Ok(if path.is_empty() {
        spoken_host
    } else {
        format!("{spoken_host} Schrägstrich {}", path.join(" Schrägstrich "))
    })
}

/// URLs with a scheme and any ending, LaTeX commands outside $...$, hashtags and @-mentions.
pub fn pre_markup(text: &str, p: Profile) -> R<String> {
    let mut text = text.to_string();
    if p.has(Fix::Urls) && text.contains("://") {
        text = SCHEME_URL.sub_fn(&text, url_with_scheme)?;
    }
    if p.has(Fix::Hashtags) {
        if text.contains('#') {
            text = HASHTAG.sub_lit(&text, "Hashtag ")?;
        }
        if text.contains('@') {
            text = MENTION.sub_lit(&text, "at ")?;
        }
    }
    Ok(text)
}

// ------------------------------------------------------------------------------------------- after markup

static SLASH_DATE: LazyRe = re!(r"(?<![\w/.])(\d{1,2})/(\d{1,2})/(\d{4}|\d{2})(?![\w/])");
static DMY_ABBR: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.])(\d{{1,2}})(?:\.|st|nd|rd|th)?[ \t-]+({MONTH_ABBRS})\.?[ \t-]+(\d{{4}})(?![\w])"
    ))
});
static DAY_MONTH_ABBR: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.])(\d{{1,2}})\.[ \t]*({MONTH_ABBRS})\.(?=[ \t]|$|,)"
    ))
});
static FRACTION_CHAR: LazyRe =
    re!(r"(?<![\w.,])(\d+)?[ \t]?([½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅐⅛⅜⅝⅞⅑⅒])(?:[ \t]?([^\W\d_]{1,4}))?");
static ARTICLE_BEFORE: LazyRe = re!(
    r"\b(ein|eine|einen|einem|einer|eines|der|die|das|den|dem|des)[ \t]+$",
    "i"
);
static COORDINATE: LazyRe =
    re!(r"(?<![\w.,])(\d{1,3})(?:[.,](\d+))?[ \t]?°[ \t]?([NSOEW])(?![\w])");
static VERSION_V: LazyRe = re!(r"(?<![\w.])v(?=\d+(?:\.\d+)+\b)");
static ROMAN_LEGAL: LazyRe = re!(r"(§+[ \t]*\d+[a-z]?)[ \t]+([IVX]{1,5})(?![\w])");
static ROMAN_KEYWORD: LazyRe = re!(concat!(
    r"\b(Kapitel|Band|Teil|Seite|Akt|Szene|Artikel|Abschnitt|Klasse|Stufe|Phase|Typ|Liga|Staffel|Folge|",
    r"Jahrgang|Mark|Bowl|Gruppe|Kategorie|Level|Runde|Anhang|Buch|Tafel|Tabelle|Teilband|Tome|Vol\.|Bd\.|Kap\.|Abs\.)",
    r"[ \t]+([IVXLCDM]{1,10})(?![\w])"
));
static ROMAN_REGNAL: LazyRe = re!(r"\b([A-ZÄÖÜ][a-zäöüß]{2,})[ \t]+([IVX]{1,6})(\.)?(?![\wÀ-ɏ])");
static ROMAN_VALID: LazyRe =
    re!(r"\AM{0,3}(?:CM|CD|D?C{0,3})(?:XC|XL|L?X{0,3})(?:IX|IV|V?I{0,3})\Z");
static WORD_BEFORE_NAME: LazyRe = re!(r"(\w+)[ \t]+$");
static SENTENCE_GOES_ON: LazyRe = re!(r"\A[ \t]+[a-zäöüß]");

const FEMALE_NAMES: &[&str] = &[
    "Elisabeth",
    "Elizabeth",
    "Maria",
    "Marie",
    "Katharina",
    "Viktoria",
    "Victoria",
    "Christina",
    "Margarete",
    "Margarethe",
    "Margrethe",
    "Anna",
    "Johanna",
    "Isabella",
    "Wilhelmina",
    "Juliana",
    "Beatrix",
    "Mathilde",
    "Luise",
    "Louise",
    "Charlotte",
    "Sophie",
    "Sofia",
    "Eleonore",
    "Theresia",
    "Kleopatra",
    "Jadwiga",
    "Silvia",
];
const NOT_NAMES: &[&str] = &[
    "Die",
    "Der",
    "Das",
    "Den",
    "Dem",
    "Des",
    "Ein",
    "Eine",
    "Und",
    "Oder",
    "Aber",
    "Wenn",
    "Weil",
    "Mit",
    "Von",
    "Bei",
    "Nach",
    "Für",
    "Auf",
    "Aus",
    "Seit",
    "Plan",
    "Vitamin",
    "Windows",
    "Abbildung",
    "Formel",
    "Kapitel",
];

fn roman_value(s: &str) -> Option<u64> {
    if s.is_empty() || !ROMAN_VALID.is_match(s).ok()? {
        return None;
    }
    let val = |c| match c {
        'I' => 1,
        'V' => 5,
        'X' => 10,
        'L' => 50,
        'C' => 100,
        'D' => 500,
        _ => 1000,
    };
    let chars: Vec<u64> = s.chars().map(val).collect();
    // subtractive notation: a smaller value before a larger one counts negative (IV, XC)
    let mut exact = 0i64;
    for (i, &v) in chars.iter().enumerate() {
        if chars.get(i + 1).is_some_and(|&n| n > v) {
            exact -= v as i64;
        } else {
            exact += v as i64;
        }
    }
    (exact > 0).then_some(exact as u64)
}

fn fraction_value(c: char) -> (u64, u64) {
    match c {
        '½' => (1, 2),
        '⅓' => (1, 3),
        '⅔' => (2, 3),
        '¼' => (1, 4),
        '¾' => (3, 4),
        '⅕' => (1, 5),
        '⅖' => (2, 5),
        '⅗' => (3, 5),
        '⅘' => (4, 5),
        '⅙' => (1, 6),
        '⅚' => (5, 6),
        '⅐' => (1, 7),
        '⅛' => (1, 8),
        '⅜' => (3, 8),
        '⅝' => (5, 8),
        '⅞' => (7, 8),
        '⅑' => (1, 9),
        _ => (1, 10),
    }
}

fn denominator_name(den: u64) -> String {
    capitalize(&super::numbers::ordinal_str(&den.to_string(), "").unwrap()) + "l"
}

fn fraction(m: &Match) -> R<String> {
    let (num, den) = fraction_value(m.g(2).chars().next().unwrap());
    let whole = m.get(1);
    let unit = m.get(3).and_then(|u| UNIT_TABLE.get(u).map(|t| (u, t)));
    let mut words = match whole {
        // "1½" -> "eineinhalb", "2½" -> "zweieinhalb"; others "zwei und drei Viertel"
        Some(w) if den == 2 => {
            let head = if small_int(w) == 1 {
                "ein".to_string()
            } else {
                cardinal_str(w)?
            };
            format!("{head}einhalb")
        }
        Some(w) => {
            let count = if num == 1 {
                "ein".to_string()
            } else {
                cardinal(num)
            };
            format!("{} und {count} {}", cardinal_str(w)?, denominator_name(den))
        }
        None if den == 2 => {
            // "eine ½ Note" -> "eine halbe Note"
            match ARTICLE_BEFORE.search(m.before())? {
                Some(a) => match a.g(1).to_lowercase().as_str() {
                    "eine" | "die" | "das" => "halbe".to_string(),
                    "ein" | "der" => "halber".to_string(),
                    _ => "halben".to_string(),
                },
                None => "ein halb".to_string(),
            }
        }
        None => {
            let count = if num == 1 {
                "ein".to_string()
            } else {
                cardinal(num)
            };
            format!("{count} {}", denominator_name(den))
        }
    };
    let more_than_one = whole.is_some_and(|w| small_int(w) >= 1);
    match (m.get(3), unit) {
        (_, Some((_, t))) => {
            words += " ";
            words += if more_than_one { t.plural } else { t.singular };
        }
        (Some(other), None) => {
            words += " ";
            words += other;
        }
        _ => {}
    }
    Ok(words)
}

fn regnal(m: &Match) -> R<String> {
    let (name, numeral) = (m.g(1), m.g(2));
    let Some(n) = roman_value(numeral) else {
        return Ok(m.all().to_string());
    };
    let single = numeral.len() == 1;
    if NOT_NAMES.contains(&name) || n > 39 || (single && m.get(3).is_none()) {
        return Ok(m.all().to_string());
    }
    let female = FEMALE_NAMES.contains(&name);
    let dative = WORD_BEFORE_NAME.search(m.before())?.is_some_and(|w| {
        [
            "von", "mit", "unter", "bei", "nach", "zu", "seit", "vor", "aus",
        ]
        .contains(&w.g(1).to_lowercase().as_str())
    });
    let (art, ending) = match (female, dative) {
        (false, false) => ("der", ""),
        (true, false) => ("die", ""),
        (false, true) => ("dem", "n"),
        (true, true) => ("der", "n"),
    };
    let ordinal = capitalize(&ordinal_str(&n.to_string(), ending)?);
    // a period after the numeral marks the ordinal; it stays if it also ends the sentence
    let dot = if m.get(3).is_some() && !SENTENCE_GOES_ON.is_match(m.after())? {
        "."
    } else {
        ""
    };
    Ok(format!("{name} {art} {ordinal}{dot}"))
}

/// Transitive verbs whose object stands in the accusative, for the article of a bare 1.
const ACC_VERBS: &[&str] = &[
    "hat",
    "habe",
    "haben",
    "hast",
    "braucht",
    "brauche",
    "brauchen",
    "nimmt",
    "nehme",
    "nehmen",
    "gibt",
    "gebe",
    "geben",
    "gib",
    "mische",
    "mischen",
    "rühre",
    "rühren",
    "füge",
    "fügen",
    "erhitze",
    "erhitzen",
    "wiege",
    "wiegen",
    "schneide",
    "schneiden",
    "kaufe",
    "kaufen",
    "suche",
    "suchen",
    "finde",
    "finden",
    "sieht",
    "sehe",
    "sehen",
    "trifft",
    "treffe",
    "treffen",
];

/// Gender of common nouns that occur after a bare 1. A small, explicit list instead of a general parser.
const COMMON_NOUNS: &[(&str, char)] = &[
    ("Bruder", 'm'),
    ("Vater", 'm'),
    ("Sohn", 'm'),
    ("Freund", 'm'),
    ("Kollege", 'm'),
    ("Onkel", 'm'),
    ("Neffe", 'm'),
    ("Mann", 'm'),
    ("Junge", 'm'),
    ("Hund", 'm'),
    ("Tisch", 'm'),
    ("Stuhl", 'm'),
    ("Wagen", 'm'),
    ("Apfel", 'm'),
    ("Kaffee", 'm'),
    ("Tee", 'm'),
    ("Wein", 'm'),
    ("Zug", 'm'),
    ("Bus", 'm'),
    ("Termin", 'm'),
    ("Fehler", 'm'),
    ("Auftrag", 'm'),
    ("Brief", 'm'),
    ("Kuchen", 'm'),
    ("Ball", 'm'),
    ("Garten", 'm'),
    ("Baum", 'm'),
    ("Schlüssel", 'm'),
    ("Computer", 'm'),
    ("Monitor", 'm'),
    ("Drucker", 'm'),
    ("Vertrag", 'm'),
    ("Preis", 'm'),
    ("Rabatt", 'm'),
    ("Platz", 'm'),
    ("Weg", 'm'),
    ("Tag", 'm'),
    ("Monat", 'm'),
    ("Morgen", 'm'),
    ("Abend", 'm'),
    ("Kopf", 'm'),
    ("Arm", 'm'),
    ("Fuß", 'm'),
    ("Finger", 'm'),
    ("Schwester", 'f'),
    ("Mutter", 'f'),
    ("Tochter", 'f'),
    ("Freundin", 'f'),
    ("Kollegin", 'f'),
    ("Tante", 'f'),
    ("Nichte", 'f'),
    ("Frau", 'f'),
    ("Katze", 'f'),
    ("Blume", 'f'),
    ("Tasche", 'f'),
    ("Flasche", 'f'),
    ("Tasse", 'f'),
    ("Pizza", 'f'),
    ("Suppe", 'f'),
    ("Zeitung", 'f'),
    ("Frage", 'f'),
    ("Antwort", 'f'),
    ("Idee", 'f'),
    ("Rechnung", 'f'),
    ("Reise", 'f'),
    ("Woche", 'f'),
    ("Nacht", 'f'),
    ("Stadt", 'f'),
    ("Straße", 'f'),
    ("Wohnung", 'f'),
    ("Küche", 'f'),
    ("Schule", 'f'),
    ("Arbeit", 'f'),
    ("Pause", 'f'),
    ("Stunde", 'f'),
    ("Minute", 'f'),
    ("Karte", 'f'),
    ("Datei", 'f'),
    ("Seite", 'f'),
    ("Zeile", 'f'),
    ("Gruppe", 'f'),
    ("Familie", 'f'),
    ("Mannschaft", 'f'),
    ("Kind", 'n'),
    ("Auto", 'n'),
    ("Haus", 'n'),
    ("Buch", 'n'),
    ("Glas", 'n'),
    ("Bier", 'n'),
    ("Wasser", 'n'),
    ("Brot", 'n'),
    ("Ei", 'n'),
    ("Messer", 'n'),
    ("Telefon", 'n'),
    ("Handy", 'n'),
    ("Fahrrad", 'n'),
    ("Zimmer", 'n'),
    ("Fenster", 'n'),
    ("Problem", 'n'),
    ("Beispiel", 'n'),
    ("Projekt", 'n'),
    ("Team", 'n'),
    ("Paket", 'n'),
    ("Ticket", 'n'),
    ("Foto", 'n'),
    ("Video", 'n'),
    ("Spiel", 'n'),
    ("Jahr", 'n'),
    ("Wochenende", 'n'),
];

static BARE_ONE_NOUN: LazyLock<Re> = LazyLock::new(|| {
    let mut nouns = COMMON_NOUNS.to_vec();
    nouns.sort_by_key(|(n, _)| std::cmp::Reverse(n.chars().count()));
    let alt = nouns.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("|");
    Re::new(&format!(r"(?<![\w.,])1[ \t]+({alt})(?![\wÀ-ɏ])"))
});
static KITCHEN: LazyLock<Re> = LazyLock::new(|| {
    Re::new(r"(?<![\w.,])(\d+)[ \t]*(EL|TL|Esslöffel|Teelöffel|Prisen|Prise)(?![\wÀ-ɏ²³])")
});

/// Article for a quantity of one, including the verb object case that the reference unit table lacks.
fn quantity_article(gender: char, before: &str) -> R<&'static str> {
    let previous = match super::units::LAST_WORD.search(before)? {
        Some(m) => m.g(1).to_lowercase(),
        None => String::new(),
    };
    // An explicit preposition governs directly; a coordinated item inherits the verb before it.
    if DATIVE.contains(&previous.as_str()) || ACCUSATIVE.contains(&previous.as_str()) {
        return article(gender, before);
    }
    let clause = before
        .rsplit(['.', '!', '?', ';', '\n'])
        .next()
        .unwrap_or("");
    if clause
        .split_whitespace()
        .rev()
        .take(6)
        .any(|w| ACC_VERBS.contains(&w.to_lowercase().as_str()))
    {
        return Ok(match gender {
            'f' => "eine",
            'm' => "einen",
            _ => "ein",
        });
    }
    article(gender, before)
}

fn kitchen_unit(unit: &str) -> (&'static str, &'static str, char) {
    match unit {
        "EL" | "Esslöffel" => ("Esslöffel", "Esslöffel", 'm'),
        "TL" | "Teelöffel" => ("Teelöffel", "Teelöffel", 'm'),
        _ => ("Prise", "Prisen", 'f'),
    }
}

static DOTTED_CLOCK: LazyRe =
    re!(r"(?<![\w.,:])([01]?\d|2[0-4])\.([0-5]\d)[ \t]*Uhr\b"; needs &["Uhr"]);
static HYPHEN_LETTER_CODE: LazyRe =
    re!(r"(?<![\wÀ-ɏ'’.-])([A-ZÄÖÜ]{1,3})[ \t]*-[ \t]*(\d{2,})(?![\d.,]\d)");
static FLIGHT_CODE: LazyRe = re!(
    r"\b((?:Flug|Zug|Abflug|Ankunft|Kennzeichen|Code|Kürzel)[ \t]+)([A-ZÄÖÜ]{1,3})[ \t]+(\d{1,6})(?![\d.,]\d)"
);

/// Article, kitchen-unit, dotted-clock and letter-code rules that must precede the number stage.
fn codes_and_articles(text: &str, p: Profile) -> R<String> {
    let mut text = text.to_string();
    if p.has(Fix::BareOneArticle) && text.contains('1') {
        text = BARE_ONE_NOUN.sub_fn(&text, |m| {
            let gender = COMMON_NOUNS
                .iter()
                .find_map(|(name, g)| (*name == m.g(1)).then_some(*g))
                .unwrap_or('n');
            Ok(format!(
                "{} {}",
                quantity_article(gender, m.before())?,
                m.g(1)
            ))
        })?;
    }
    if p.has(Fix::KitchenUnits) && super::numbers::has_digit(&text) {
        text = KITCHEN.sub_fn(&text, |m| {
            let (singular, plural, gender) = kitchen_unit(m.g(2));
            Ok(if m.g(1) == "1" {
                format!("{} {singular}", quantity_article(gender, m.before())?)
            } else {
                format!("{} {plural}", cardinal_str(m.g(1))?)
            })
        })?;
    }
    if p.has(Fix::DottedClocks) && text.contains("Uhr") {
        text = DOTTED_CLOCK.sub_fn(&text, |m| {
            let (h, min) = (small_int(m.g(1)), small_int(m.g(2)));
            Ok(if min == 0 {
                format!("{} Uhr", cardinal(h))
            } else {
                format!("{} Uhr {}", cardinal(h), cardinal(min))
            })
        })?;
    }
    if p.has(Fix::LetterCodes) && super::numbers::has_digit(&text) {
        text = FLIGHT_CODE.sub_fn(&text, |m| {
            Ok(format!(
                "{}{} {}",
                m.g(1),
                spell_code(m.g(2)),
                digit_words(m.g(3))
            ))
        })?;
        text = HYPHEN_LETTER_CODE.sub_fn(&text, |m| {
            Ok(format!("{} {}", spell_code(m.g(1)), digit_words(m.g(2))))
        })?;
    }
    Ok(text)
}

/// Slash, abbreviated-month and English-style dates, Unicode fractions, coordinates, Roman numerals, "v1.2".
/// Dotted clock times and letter/digit codes run before the number stage, which would read the period as
/// "Punkt" and a separated letter run as a German word.
pub fn post_markup(text: &str, p: Profile) -> R<String> {
    let mut text = text.to_string();
    // after clean_markup, which has read the delimited formulas ($...$, \(...\))
    if p.has(Fix::BareLatex) && text.contains('\\') {
        text = BARE_LATEX.sub_fn(&text, |m| Ok(format!(" {} ", latex_to_text(m.all(), p)?)))?;
    }
    text = codes_and_articles(&text, p)?;
    if p.has(Fix::Dates) && super::numbers::has_digit(&text) {
        text = SLASH_DATE.sub_fn(&text, |m| {
            let (d, mo) = (small_int(m.g(1)), small_int(m.g(2)));
            if !(1..=31).contains(&d) || !(1..=12).contains(&mo) {
                return Ok(m.all().to_string());
            }
            Ok(format!("{d}. {} {}", month_name(mo), m.g(3)))
        })?;
        text = DMY_ABBR.sub_fn(&text, |m| match month_abbr(m.g(2)) {
            Some(mo) => Ok(format!(
                "{}. {} {}",
                small_int(m.g(1)),
                month_name(mo),
                m.g(3)
            )),
            None => Ok(m.all().to_string()),
        })?;
        text = DAY_MONTH_ABBR.sub_fn(&text, |m| match month_abbr(m.g(2)) {
            Some(mo) => Ok(format!("{}. {}", m.g(1), month_name(mo))),
            None => Ok(m.all().to_string()),
        })?;
    }
    if p.has(Fix::UnicodeFractions)
        && text.contains([
            '½', '⅓', '⅔', '¼', '¾', '⅕', '⅖', '⅗', '⅘', '⅙', '⅚', '⅐', '⅛', '⅜', '⅝', '⅞', '⅑',
            '⅒',
        ])
    {
        text = FRACTION_CHAR.sub_fn(&text, fraction)?;
    }
    if p.has(Fix::Coordinates) && text.contains('°') {
        text = COORDINATE.sub_fn(&text, |m| {
            let direction = match m.g(3) {
                "N" => "Nord",
                "S" => "Süd",
                "O" | "E" => "Ost",
                _ => "West",
            };
            let number = match m.get(2) {
                Some(frac) => format!("{} Komma {}", cardinal_str(m.g(1))?, digit_words(frac)),
                None => cardinal_str(m.g(1))?,
            };
            Ok(format!("{number} Grad {direction}"))
        })?;
    }
    if p.has(Fix::RomanNumerals) && text.contains(['I', 'V', 'X', 'L', 'C', 'D', 'M']) {
        text = ROMAN_LEGAL.sub_fn(&text, |m| match roman_value(m.g(2)) {
            Some(n) => Ok(format!("{} Absatz {n}", m.g(1))),
            None => Ok(m.all().to_string()),
        })?;
        text = ROMAN_KEYWORD.sub_fn(&text, |m| match roman_value(m.g(2)) {
            Some(n) => Ok(format!("{} {n}", m.g(1))),
            None => Ok(m.all().to_string()),
        })?;
        text = ROMAN_REGNAL.sub_fn(&text, regnal)?;
    }
    if p.has(Fix::VersionPrefix) && text.contains('v') {
        text = VERSION_V.sub_lit(&text, "Version ")?;
    }
    Ok(text)
}

// ------------------------------------------------------------------------------------------- before numbers

static DURATION: LazyRe =
    re!(r"(?<![\w.,:])(\d{1,2}):([0-5]\d)[ \t]*(Stunden|Stunde|Std\.?|Minuten|Minute|min)\b");
static SCORE: LazyRe = re!(r"(?<![\w.,:])(\d{1,3}):(\d{1,3})(?![\d:]|[ \t]*Uhr)");
static SCORE_CONTEXT: LazyRe = re!(concat!(
    r"\b(?:endete|endet|gewann|gewannen|gewinnt|verlor|verloren|verliert|siegte|siegten|Endstand|Endergebnis|",
    r"Ergebnis|Halbzeit|Halbzeitstand|Spielstand|Zwischenstand|Sieg|Niederlage|Remis|unentschieden|Tore|Toren|",
    r"Punkte|Satz|Sätze|Partie|Spiel|Match|Finale|führt|führte|Elfmeterschießen)\b"
));
static TIME_CUE: LazyRe = re!(
    r"\b(?:um|ab|bis|gegen|von|seit|zwischen|nach|vor|Uhrzeit|spätestens|frühestens)[ \t]*$",
    "i"
);
static DECADE: LazyRe = re!(r"(?<![\w.,])(\d+)er(n|s)?(?![\wÀ-ɏ])");
static DECADE_ATTRIBUTIVE: LazyRe = re!(r"\A[ \t]+[A-ZÄÖÜ]");
static SIGN_PLUS: LazyRe = re!(r"(?<![\w+])\+(?=\d)");
static TIGHT_LESS: LazyRe = re!(r"(?<=\d)<(?=\d)");
static TIGHT_GREATER: LazyRe = re!(r"(?<=\d)>(?=\d)");
static DETERMINER_ORDINAL: LazyRe = re!(
    concat!(
        r"\b((?:mein|dein|sein|ihr|Ihr|unser|euer|eu[e]?r|kein|ein|dies|jen|welch|manch|solch|der|des)",
        r"(?:e|en|em|er|es)?)[ \t]+(\d{1,3})\.(?=[ \t]+([^\W\d_][\wÀ-ɏ-]*))"
    ),
    "i"
);

const NEUTER_NOUNS: &[&str] = &[
    "Mal",
    "Jahr",
    "Jahrhundert",
    "Jahrtausend",
    "Jahrzehnt",
    "Kind",
    "Buch",
    "Kapitel",
    "Spiel",
    "Tor",
    "Album",
    "Konzert",
    "Semester",
    "Quartal",
    "Treffen",
    "Rennen",
    "Stück",
    "Haus",
    "Land",
    "Mitglied",
    "Jubiläum",
    "Werk",
    "Lied",
    "Gebot",
    "Gesetz",
    "Programm",
    "Projekt",
    "Level",
    "Ergebnis",
    "Bild",
    "Foto",
    "Video",
    "Team",
    "Auto",
    "Zimmer",
    "Stockwerk",
    "Geschoss",
    "Feld",
    "Element",
    "Exemplar",
    "Symphoniekonzert",
    "Festival",
    "Turnier",
    "Rad",
    "Glas",
    "Ei",
    "Mädchen",
    "Wort",
    "Ziel",
    "Angebot",
    "Paket",
    "Modell",
    "Kapitel",
    "Heft",
    "Thema",
];
const MASCULINE_E_NOUNS: &[&str] = &[
    "Kunde",
    "Junge",
    "Name",
    "Gedanke",
    "Buchstabe",
    "Friede",
    "Wille",
    "Glaube",
    "Kollege",
    "Bote",
    "Zeuge",
    "Erbe",
    "Experte",
    "Käse",
    "Affe",
    "Löwe",
    "Hase",
    "Neffe",
    "Riese",
    "Franzose",
    "Kunde",
    "Laie",
];

fn neuter(noun: &str) -> bool {
    let last = noun.rsplit('-').next().unwrap_or(noun);
    NEUTER_NOUNS
        .iter()
        .any(|n| last == *n || (last.len() > n.len() + 3 && last.ends_with(&n.to_lowercase())))
        || ["chen", "lein", "ment", "um", "tum", "nis"]
            .iter()
            .any(|s| last.ends_with(s))
}

fn feminine(noun: &str) -> bool {
    let last = noun.rsplit('-').next().unwrap_or(noun);
    if MASCULINE_E_NOUNS.contains(&last) {
        return false;
    }
    [
        "ung", "heit", "keit", "schaft", "ion", "tät", "ik", "ei", "ie", "enz", "anz", "ur", "e",
    ]
    .iter()
    .any(|s| last.ends_with(s))
}

/// Ending of an ordinal after a determiner, from the determiner's ending and the noun after it.
fn determiner_ending(det: &str, noun: &str) -> Option<&'static str> {
    let d = det.to_lowercase();
    let ein_word = [
        "mein", "dein", "sein", "ihr", "unser", "euer", "eur", "kein", "ein",
    ];
    if ein_word.contains(&d.as_str()) {
        return Some(if neuter(noun) { "s" } else { "r" }); // mein dritter Versuch, mein drittes Mal
    }
    if d == "der" {
        // only "der" + feminine noun differs from the reference: dative/genitive, "der dritten Runde"
        return feminine(noun).then_some("n");
    }
    if d == "des" {
        return Some("n");
    }
    for stem in [
        "mein", "dein", "sein", "ihr", "unser", "euer", "eur", "kein", "ein",
    ] {
        if let Some(e) = d.strip_prefix(stem) {
            return match e {
                "e" => Some(""),
                "en" | "em" | "er" | "es" => Some("n"),
                _ => None,
            };
        }
    }
    for stem in ["dies", "jen", "welch", "manch", "solch"] {
        if let Some(e) = d.strip_prefix(stem) {
            return match e {
                "e" | "es" => Some(""),
                "em" | "en" => Some("n"),
                "er" => Some(if feminine(noun) { "n" } else { "" }),
                _ => None,
            };
        }
    }
    None
}

fn is_month(w: &str) -> bool {
    MONTHS.split('|').any(|m| m == w)
}

/// Durations (1:45 Stunden), scores (24:18), decades (70er), ordinals after possessives and demonstratives,
/// "+49", "3<5".
pub fn pre_numbers(text: &str, p: Profile) -> R<String> {
    if !super::numbers::has_digit(text) {
        return Ok(text.to_string());
    }
    let mut text = text.to_string();
    if p.has(Fix::Durations) && text.contains(':') {
        text = DURATION.sub_fn(&text, |m| {
            let (a, b) = (m.g(1), m.g(2));
            let hours = m.g(3).starts_with('S');
            let (big, small) = if hours {
                ("Stunde", "Minute")
            } else {
                ("Minute", "Sekunde")
            };
            let first = if small_int(a) == 1 {
                format!("{} {big}", article('f', m.before())?)
            } else {
                format!("{} {big}n", cardinal_str(a)?)
            };
            if small_int(b) == 0 {
                return Ok(first);
            }
            let second = if small_int(b) == 1 {
                format!("eine {small}")
            } else {
                format!("{} {small}n", cardinal_str(b)?)
            };
            Ok(format!("{first} und {second}"))
        })?;
    }
    if p.has(Fix::Scores) && text.contains(':') {
        let scored = SCORE_CONTEXT.search(&text)?.is_some();
        text = SCORE.sub_fn(&text, |m| {
            let (h, mi) = (small_int(m.g(1)), small_int(m.g(2)));
            let impossible_time = h > 24 || mi > 59 || (h == 24 && mi != 0) || m.g(2).len() != 2;
            if impossible_time || (scored && !TIME_CUE.search(m.before())?.is_some()) {
                return Ok(format!("{} zu {}", m.g(1), m.g(2)));
            }
            Ok(m.all().to_string())
        })?;
    }
    if p.has(Fix::Decades) && text.contains("er") {
        text = DECADE.sub_fn(&text, |m| {
            let number = integer(m.g(1))?;
            let word = format!("{number}er{}", m.g(2));
            // "die 70er Jahre" is an adjective, "in den 70ern", "ein 3er" and "70er-Jahre" are nouns
            Ok(
                if m.g(2).is_empty() && DECADE_ATTRIBUTIVE.is_match(m.after())? {
                    word
                } else {
                    capitalize(&word)
                },
            )
        })?;
    }
    if p.has(Fix::OrdinalAgreement) && text.contains('.') {
        text = DETERMINER_ORDINAL.sub_fn(&text, |m| {
            let noun = m.g(3);
            if is_month(noun) {
                return Ok(m.all().to_string()); // dates have their own rule
            }
            match determiner_ending(m.g(1), noun) {
                Some(e) => Ok(format!("{} {}", m.g(1), ordinal_str(m.g(2), e)?)),
                None => Ok(m.all().to_string()),
            }
        })?;
    }
    if p.has(Fix::NoLeftovers) {
        if text.contains('+') {
            text = SIGN_PLUS.sub_lit(&text, "plus ")?;
        }
        if text.contains('<') {
            text = TIGHT_LESS.sub_lit(&text, " kleiner als ")?;
        }
        if text.contains('>') {
            text = TIGHT_GREATER.sub_lit(&text, " größer als ")?;
        }
    }
    Ok(text)
}

// ------------------------------------------------------------------------------------------- after numbers

static DATIVE_ONE_SCALE: LazyRe = re!(
    r"\b(bei|mit|von|aus|zu|nach|seit|vor|unter|über|knapp|rund)[ \t]+eine[ \t]+(Million|Milliarde|Billion|Billiarde)\b"
);

/// "bei eine Million" -> "bei einer Million".
pub fn post_numbers(text: &str, p: Profile) -> R<String> {
    if p.has(Fix::OrdinalAgreement) && text.contains("eine ") {
        return DATIVE_ONE_SCALE.sub_fn(text, |m| {
            let prep = m.g(1).to_lowercase();
            // "über eine Million" and "rund eine Million" are accusative/adverbial: unchanged
            if ["über", "rund", "knapp"].contains(&prep.as_str()) {
                return Ok(m.all().to_string());
            }
            Ok(format!("{} einer {}", m.g(1), m.g(2)))
        });
    }
    Ok(text.to_string())
}

/// Characters a TTS model cannot read that may survive every other rule ($, £, ¥ after scale words).
pub fn leftover_symbols(text: &str, p: Profile) -> String {
    if !p.has(Fix::NoLeftovers) || !text.contains(['$', '£', '¥']) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '$' => out.push_str(" Dollar"),
            '£' => out.push_str(" Pfund"),
            '¥' => out.push_str(" Yen"),
            c => out.push(c),
        }
    }
    out
}
