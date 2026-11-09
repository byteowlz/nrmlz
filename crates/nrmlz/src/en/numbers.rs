//! Numeric tokens, clock readings and telephone digit groups.
use std::sync::LazyLock;

use super::{
    ascii_decimals, plain_number, sequence_component, terminal_dot, words, Text, UNSIGNED,
};
use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::small_int;
use crate::reading::{self, NumberReading};
use crate::Lang;

static CLOCK: LazyRe = re!(
    r"(?<![\w:])([0-9]{1,2}):([0-9]{2})(?::([0-9]{2}))?(?:[ \t]*([aApP]\.?[mM]\.?)\b\.?)?(?![\w:]|\.[0-9])"
);
static HOUR: LazyRe = re!(r"(?<![\w:])([0-9]{1,2})[ \t]*([aApP]\.?[mM]\.?)\b\.?(?!\w)");
static US_PHONE: LazyRe = re!(
    r"(?<![\w+])(?:([+]1|1)[ .-])?(?:\(([2-9][0-9]{2})\)[ .-]?|([2-9][0-9]{2})[ .-])([0-9]{3})[ .-]([0-9]{4})(?!\w|[.-][0-9])"
);
static INTL_PHONE: LazyRe =
    re!(r"(?<!\w)\+([1-9][0-9]{0,2}(?:[ -][0-9]{1,4}){1,5}|[1-9][0-9]{5,14})(?!\w|[ -][0-9])");
static ORDINAL: LazyRe = re!(r"(?<![\w.,])([0-9]+)(st|nd|rd|th)\b", "i");
static DOTTED: LazyRe = re!(r"(?<![\w.])[0-9]+(?:\.[0-9]+){2,}(?!\w|\.[0-9])");
static RANGE: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.,-])([+-]?(?:{UNSIGNED}))[ \t]*[-–—][ \t]*([+-]?(?:{UNSIGNED}))(?!\w|,[0-9]|\.[0-9]|[-–—][0-9])"
    ))
});
static NUMBER: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.,])([+-]?(?:{UNSIGNED}))(?!\w|,[0-9]|\.[0-9])"
    ))
});
static YEAR_CUE: LazyRe = re!(
    r"(?i)\b(in|since|from|until|by|before|after|during|year|circa|born|died|founded|published|released)[ \t]+$"
);

fn clock(m: &Match<'_>) -> String {
    let (h, min) = (small_int(m.g(1)), small_int(m.g(2)));
    let sec = m.get(3).map(small_int);
    let marker = m.get(4);
    if min > 59
        || sec.is_some_and(|s| s > 59)
        || (marker.is_some() && !(1..=12).contains(&h))
        || (marker.is_none() && h > 23)
    {
        return m.all().to_owned();
    }
    let hour = words::cardinal(h);
    let mut out = match min {
        0 => format!("{hour} o'clock"),
        1..=9 => format!("{hour} oh {}", words::cardinal(min)),
        _ => format!("{hour} {}", words::cardinal(min)),
    };
    if let Some(s) = sec {
        out += &format!(
            " and {} {}",
            words::cardinal(s),
            if s == 1 { "second" } else { "seconds" }
        );
    }
    if let Some(marker) = marker {
        out += if marker.starts_with(['a', 'A']) {
            " A M"
        } else {
            " P M"
        };
        out += marker_dot(m);
    }
    out
}

/// After "a.m."/"p.m." a capitalised word nearly always starts a new sentence ("6 a.m. Please hurry"); only
/// weekday and month names commonly follow without one ("9 a.m. Monday").
fn marker_dot(m: &Match<'_>) -> &'static str {
    let first = m.after().split_whitespace().next().unwrap_or("");
    let word = first.trim_end_matches(|c: char| !c.is_alphanumeric());
    let calendar = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    if m.all().ends_with('.')
        && word.chars().next().is_some_and(char::is_uppercase)
        && !calendar.contains(&word)
    {
        "."
    } else {
        terminal_dot(m)
    }
}

pub(super) fn clocks(t: &mut Text) -> R<()> {
    t.apply(&CLOCK, clock)?;
    t.apply(&HOUR, |m| {
        let h = small_int(m.g(1));
        if !(1..=12).contains(&h) {
            return m.all().to_owned();
        }
        format!(
            "{} {}{}",
            words::cardinal(h),
            if m.g(2).starts_with(['a', 'A']) {
                "A M"
            } else {
                "P M"
            },
            marker_dot(m)
        )
    })
}

pub(super) fn phones(t: &mut Text) -> R<()> {
    t.apply(&US_PHONE, |m| {
        let mut groups = Vec::new();
        if let Some(country) = m.get(1) {
            groups.push(words::digits(country));
        }
        groups.push(words::digits(m.get(2).unwrap_or_else(|| m.g(3))));
        groups.push(words::digits(m.g(4)));
        groups.push(words::digits(m.g(5)));
        groups.join(", ")
    })?;
    t.apply(&INTL_PHONE, |m| {
        let n = m.g(1).bytes().filter(u8::is_ascii_digit).count();
        if !(6..=15).contains(&n) {
            return m.all().to_owned();
        }
        format!(
            "plus {}",
            m.g(1)
                .split([' ', '-'])
                .map(words::digits)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&ORDINAL, |m| {
        // The recognizer is ASCII-only after Unicode decimal canonicalization.
        let digits = m.g(1);
        let n = small_int(&digits[digits.len().saturating_sub(2)..]);
        let suffix = if (11..=13).contains(&(n % 100)) {
            "th"
        } else {
            match n % 10 {
                1 => "st",
                2 => "nd",
                3 => "rd",
                _ => "th",
            }
        };
        // Reject an invalid suffix rather than confidently saying an incorrectly written ordinal.
        if !m.g(2).eq_ignore_ascii_case(suffix) {
            return m.all().to_owned();
        }
        words::ordinal_digits(&crate::pystr::digits_of(m.g(1)))
    })?;
    t.apply(&DOTTED, |m| {
        m.all()
            .split('.')
            .map(sequence_component)
            .collect::<Vec<_>>()
            .join(" dot ")
    })?;
    t.apply(&RANGE, |m| {
        format!("{} to {}", plain_number(m.g(1)), plain_number(m.g(2)))
    })?;
    t.apply(&NUMBER, number)
}

/// A plain integer is a year, an identifier read digit by digit, or a cardinal; formatting/context decides.
fn number(m: &Match<'_>) -> String {
    let raw = m.g(1);
    let unsigned = raw.trim_start_matches(['+', '-']);
    if !unsigned.contains('.') {
        let n = small_int(raw);
        if raw.len() == 4
            && (1000..2100).contains(&n)
            && YEAR_CUE.search(m.before()).ok().flatten().is_some()
        {
            return words::year(n);
        }
        let digits: String = unsigned.chars().filter(char::is_ascii_digit).collect();
        if reading::decide(Lang::En, m.before(), m.after(), unsigned.contains(','))
            == NumberReading::Digits
        {
            let sign = if raw.starts_with('-') {
                "minus "
            } else if raw.starts_with('+') {
                "plus "
            } else {
                ""
            };
            return format!("{sign}{}", words::digits(&digits));
        }
    }
    plain_number(raw)
}

/// Expand structured numbers without interpreting markup or addresses.
pub fn numbers(text: &str) -> R<String> {
    let mut t = Text::new(&ascii_decimals(text));
    super::records::apply(&mut t)?;
    super::identifiers::explicit(&mut t)?;
    super::network::apply(&mut t)?;
    super::identifiers::versions(&mut t)?;
    super::clinical::apply(&mut t)?;
    super::fractions::apply(&mut t)?;
    super::dates::apply(&mut t)?;
    phones(&mut t)?;
    super::money::apply(&mut t)?;
    clocks(&mut t)?;
    apply(&mut t)?;
    Ok(t.finish())
}
