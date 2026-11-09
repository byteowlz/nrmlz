//! Rational quantities, not dates: require explicit quantity context for ambiguous slash notation.
use std::sync::LazyLock;

use super::{integer, units, words, Text};
use crate::pyre::{re, LazyRe, Match, Re, R};

const GLYPHS: &str = "¼½¾⅐⅑⅒⅓⅔⅕⅖⅗⅘⅙⅚⅛⅜⅝⅞";
static SLASH: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w./])([+-]?)(?:([0-9]+)[ \t]+)?([0-9]+)[ \t]*/[ \t]*([0-9]+)(?:[ \t]*({})(?:/({}))?)?(?![\w/]|[ \t]*/)",
        units::alternatives(true),
        units::alternatives(false)
    ))
});
static GLYPH: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.])([+-]?)([0-9]+)?([{GLYPHS}])(?:[ \t]*({})(?:/({}))?)?(?!\w|/)",
        units::alternatives(true),
        units::alternatives(false)
    ))
});
static DATE_CUE: LazyRe = re!(
    r"\b(date|on|due|deadline|scheduled|meet|meeting|until|since|born)\b",
    "i"
);
static QUANTITY_CUE: LazyRe = re!(
    r"\b(fraction|ratio|mix|use|add|ate|eat|eaten|share|divide|divided|minus|plus|equals|equal|is|was|by|about|roughly|approximately|nearly|only)\b",
    "i"
);
static AFTER_CUE: LazyRe = re!(
    r"^[ \t]*(of|cup|cups|teaspoon|teaspoons|tablespoon|tablespoons|inch|inches|full|each|remaining|done|complete|finished)\b",
    "i"
);

fn quantity_context(m: &Match<'_>) -> bool {
    if m.get(2).is_some() || m.get(5).is_some() {
        return true;
    }
    let before = m
        .before()
        .rsplit(['.', '!', '?', ';', '\n'])
        .next()
        .unwrap_or("");
    if DATE_CUE.search(before).ok().flatten().is_some() {
        return false;
    }
    QUANTITY_CUE.search(before).ok().flatten().is_some()
        || AFTER_CUE.search(m.after()).ok().flatten().is_some()
}

fn fraction(n: &str, d: &str) -> String {
    let numerator = integer(n);
    let denominator = match d.parse::<u64>() {
        Ok(2) => if n.parse::<u64>() == Ok(1) {
            "half"
        } else {
            "halves"
        }
        .to_owned(),
        Ok(4) => if n.parse::<u64>() == Ok(1) {
            "quarter"
        } else {
            "quarters"
        }
        .to_owned(),
        Ok(1) | Err(_) => return format!("{numerator} over {}", integer(d)),
        Ok(d) => format!(
            "{}{}",
            words::ordinal(d),
            if n.parse::<u64>() == Ok(1) { "" } else { "s" }
        ),
    };
    format!("{numerator} {denominator}")
}

fn render(
    sign: &str,
    whole: Option<&str>,
    n: &str,
    d: &str,
    unit: Option<&str>,
    rate: Option<&str>,
) -> String {
    let sign = match sign {
        "-" => "minus ",
        "+" => "plus ",
        _ => "",
    };
    let mut out = match whole {
        Some(w) => format!("{sign}{} and {}", integer(w), fraction(n, d)),
        None => format!("{sign}{}", fraction(n, d)),
    };
    if let Some((singular, plural, _)) = unit.and_then(units::lookup) {
        let (a, b) = (n.parse::<u64>().ok(), d.parse::<u64>().ok());
        if whole.is_none() && a.zip(b).is_some_and(|(a, b)| a < b) {
            let article =
                if singular.starts_with(['a', 'e', 'i', 'o']) || singular.starts_with("hour") {
                    "an"
                } else {
                    "a"
                };
            out += &format!(" of {article} {singular}");
        } else {
            let one = whole.is_none() && a.is_some() && a == b;
            out += &format!(" {}", if one { singular } else { plural });
        }
        if let Some((denominator, _, _)) = rate.and_then(units::lookup) {
            out += &format!(" per {denominator}");
        }
    }
    out
}

fn glyph_fraction(s: &str) -> (&'static str, &'static str) {
    match s {
        "¼" => ("1", "4"),
        "½" => ("1", "2"),
        "¾" => ("3", "4"),
        "⅐" => ("1", "7"),
        "⅑" => ("1", "9"),
        "⅒" => ("1", "10"),
        "⅓" => ("1", "3"),
        "⅔" => ("2", "3"),
        "⅕" => ("1", "5"),
        "⅖" => ("2", "5"),
        "⅗" => ("3", "5"),
        "⅘" => ("4", "5"),
        "⅙" => ("1", "6"),
        "⅚" => ("5", "6"),
        "⅛" => ("1", "8"),
        "⅜" => ("3", "8"),
        "⅝" => ("5", "8"),
        "⅞" => ("7", "8"),
        _ => unreachable!("glyph recognizer and table agree"),
    }
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.recognize(&SLASH, |m| {
        if !quantity_context(m) {
            return None;
        }
        if m.g(4).bytes().all(|b| b == b'0') {
            return Some(m.all().to_owned());
        }
        Some(render(m.g(1), m.get(2), m.g(3), m.g(4), m.get(5), m.get(6)))
    })?;
    t.apply(&GLYPH, |m| {
        let (n, d) = glyph_fraction(m.g(3));
        render(m.g(1), m.get(2), n, d, m.get(4), m.get(5))
    })
}
