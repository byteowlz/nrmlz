//! Physical quantities and currencies right after a number: "230 V", "5 km/h", "1 h" -> "eine Stunde".
//! Port of the "units" section of german.py (table: data/de/units.json).

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::small_int;

use super::numbers::{cardinal, cardinal_str, has_digit};
use super::tables::UNITS;
use super::Profile;

pub(crate) struct Unit {
    pub singular: &'static str,
    pub plural: &'static str,
    pub gender: char,
    pub needs_space: bool,
}

pub(crate) static TABLE: LazyLock<HashMap<&'static str, Unit>> = LazyLock::new(|| {
    UNITS
        .iter()
        .map(|&(k, singular, plural, gender, needs_space)| {
            (
                k,
                Unit {
                    singular,
                    plural,
                    gender,
                    needs_space,
                },
            )
        })
        .collect()
});

fn escape(s: &str) -> String {
    // re.escape: only regex-special characters matter; the translator turns escapes into \x{..}.
    let mut out = String::new();
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub(crate) fn escape_py(s: &str) -> String {
    escape(s)
}

static UNIT_RX: LazyRe = LazyLock::new(|| {
    let mut keys: Vec<&str> = UNITS.iter().map(|u| u.0).collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.chars().count())); // stable, like Python's sorted
    let alt = keys.iter().map(|k| escape(k)).collect::<Vec<_>>().join("|");
    Re::new(&format!(
        r"(?<![\w.,])(-?\d+(?:[.,]\d+)*(?:\s?[-–]\s?\d+(?:[.,]\d+)*)?)(\s?)({alt})(?:/({alt}))?(?![\wÀ-ɏ²³])"
    ))
});
static CURRENCY_FIRST: LazyRe = re!(r"([$€£¥])\s?(\d+(?:[.,]\d+)*)");
pub(crate) static LAST_WORD: LazyRe = re!(r"(\w+)\s*$");
static METRES_CM: LazyRe = re!(r"\A\d+,\d\d\Z");

// "in einer Stunde", "um einen Grad", "ein Meter": the article depends on the case, which the preposition
// before usually gives away.
pub(crate) const DATIVE: &[&str] = &[
    "in",
    "mit",
    "nach",
    "vor",
    "seit",
    "bei",
    "von",
    "aus",
    "zu",
    "binnen",
    "innerhalb",
    "ab",
    "unter",
];
pub(crate) const ACCUSATIVE: &[&str] = &["für", "durch", "gegen", "ohne", "um", "über", "auf"];

pub(crate) fn article(gender: char, before: &str) -> R<&'static str> {
    let p = match LAST_WORD.search(before)? {
        Some(w) => w.g(1).to_lowercase(),
        None => String::new(),
    };
    let p = p.as_str();
    Ok(if DATIVE.contains(&p) {
        if gender == 'f' {
            "einer"
        } else {
            "einem"
        }
    } else if ACCUSATIVE.contains(&p) {
        match gender {
            'f' => "eine",
            'm' => "einen",
            _ => "ein",
        }
    } else if gender == 'f' {
        "eine"
    } else {
        "ein"
    })
}

/// 230 V -> 230 Volt, 1 h -> eine Stunde, 50 km/h -> 50 Kilometer pro Stunde.
pub fn expand_units(text: &str, _p: Profile) -> R<String> {
    if text.is_empty() || !has_digit(text) {
        return Ok(text.to_string());
    }
    let text = CURRENCY_FIRST.sub(text, r"\2 \1")?; // $5 -> 5 $
    UNIT_RX.sub_fn(&text, replace)
}

fn replace(m: &Match) -> R<String> {
    let (number, space, unit, denominator) = (m.g(1), m.g(2), m.g(3), m.get(4));
    let u = &TABLE[unit];
    // "16A", "4K-Monitor", "10 T-Shirts": no ampere, kelvin, tesla
    if u.needs_space && space.is_empty() {
        return Ok(m.all().to_string());
    }
    if unit.chars().count() == 1 && m.after().starts_with('-') {
        return Ok(m.all().to_string());
    }
    // body height and lengths with centimetres: "1,80 m" is "ein Meter achtzig"
    if unit == "m" && denominator.is_none() && METRES_CM.is_match(number)? {
        let (whole, cents) = number.split_once(',').unwrap();
        let front = if small_int(whole) == 1 {
            article('m', m.before())?.to_string()
        } else {
            cardinal_str(whole)?
        };
        let cents = small_int(cents);
        let tail = if cents > 0 {
            format!(" {}", cardinal(cents))
        } else {
            String::new()
        };
        return Ok(format!("{front} Meter{tail}"));
    }
    let one = number.trim_start_matches('-') == "1";
    let mut word = (if one { u.singular } else { u.plural }).to_string();
    if let Some(d) = denominator {
        word += " pro ";
        word += TABLE[d].singular;
    }
    if one {
        let sign = if number.starts_with('-') {
            "minus "
        } else {
            ""
        };
        return Ok(format!("{sign}{} {word}", article(u.gender, m.before())?));
    }
    Ok(format!("{number} {word}"))
}
