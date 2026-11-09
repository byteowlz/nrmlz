//! Gregorian validation and explicit US month/day date policy.
use std::sync::LazyLock;

use super::{words, Text};
use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::small_int;

const MONTHS: [&str; 12] = [
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
const MONTH_PATTERN: &str = r"January|February|March|April|May|June|July|August|September|October|November|December|Jan\.?|Feb\.?|Mar\.?|Apr\.?|Jun\.?|Jul\.?|Aug\.?|Sept?\.?|Oct\.?|Nov\.?|Dec\.?";
static ISO: LazyRe = re!(r"(?<![\w.-])([0-9]{4})-([0-9]{2})-([0-9]{2})(?![\w-])");
static SLASH: LazyRe = re!(r"(?<![\w/])([0-9]{1,2})/([0-9]{1,2})(?:/([0-9]{2,4}))?(?![\w/])");
static MDY: LazyLock<Re> = LazyLock::new(|| {
    Re::with_flags(
        &format!(
            r"\b({MONTH_PATTERN})[ \t]+([0-9]{{1,2}})(?:st|nd|rd|th)?(?:,?[ \t]+([0-9]{{4}}))?(?![\w])"
        ),
        "i",
    )
});
static DMY: LazyLock<Re> = LazyLock::new(|| {
    Re::with_flags(
        &format!(
            r"(?<!\w)([0-9]{{1,2}})(?:st|nd|rd|th)?[ \t]+(?:of[ \t]+)?({MONTH_PATTERN})(?:,?[ \t]+([0-9]{{4}}))?(?!\w)"
        ),
        "i",
    )
});

fn month(s: &str) -> u64 {
    let bare = s.trim_end_matches('.');
    MONTHS
        .iter()
        .position(|m| {
            m.eq_ignore_ascii_case(bare)
                || (bare.len() >= 3
                    && m.to_ascii_lowercase()
                        .starts_with(&bare.to_ascii_lowercase()))
        })
        .map_or(0, |i| i as u64 + 1)
}

fn valid(y: Option<u64>, mo: u64, d: u64) -> bool {
    let leap = match y {
        None => true,
        Some(y) => y % 4 == 0 && (y % 100 != 0 || y % 400 == 0),
    };
    let max = match mo {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    y != Some(0) && (1..=max).contains(&d)
}

fn render(m: &Match<'_>, mo: u64, day: &str, year: Option<&str>) -> String {
    let d = small_int(day);
    let y = year.map(small_int);
    if !valid(y, mo, d) {
        return m.all().to_owned();
    }
    let mut out = format!("{} {}", MONTHS[(mo - 1) as usize], words::ordinal(d));
    if let Some(y) = y {
        out += &format!(", {}", words::year(y));
    }
    out
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&ISO, |m| render(m, small_int(m.g(2)), m.g(3), m.get(1)))?;
    t.apply(&MDY, |m| render(m, month(m.g(1)), m.g(2), m.get(3)))?;
    t.apply(&DMY, |m| render(m, month(m.g(2)), m.g(1), m.get(3)))?;
    t.apply(&SLASH, |m| {
        // With a four-digit year this is explicitly US order. Without one only month/day where day>12
        // is unambiguous. Never guess the century of a two-digit year or reinterpret 3/4 as a date.
        if m.get(3).is_some_and(|y| y.len() != 4) || (m.get(3).is_none() && small_int(m.g(2)) <= 12)
        {
            return m.all().to_owned();
        }
        render(m, small_int(m.g(1)), m.g(2), m.get(3))
    })
}
