//! Measurement units use case-sensitive symbols and singular rate denominators.
use std::sync::LazyLock;

use super::{ascii_decimals, escape, plain_number, tables, Text, UNSIGNED};
use crate::pyre::{Re, R};

pub(super) fn alternatives(require_space: bool) -> String {
    let mut entries = tables::UNITS.to_vec();
    entries.sort_by_key(|u| std::cmp::Reverse(u.0.len()));
    entries
        .into_iter()
        .map(|(key, _, _, space)| {
            let key = escape(key);
            if require_space && space {
                format!(r"(?<=[ \t]){key}")
            } else {
                key
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

static UNIT: LazyLock<Re> = LazyLock::new(|| {
    let alt = alternatives(true);
    let denom = alternatives(false);
    Re::new(&format!(
        r"(?<![\w.,])([+-]?(?:{UNSIGNED}))([ \t]*)({alt})(?:/({denom}))?(?![\w/])"
    ))
});
static UNIT_RANGE: LazyLock<Re> = LazyLock::new(|| {
    let alt = alternatives(true);
    let denom = alternatives(false);
    Re::new(&format!(
        r"(?<![\w.,-])([+-]?(?:{UNSIGNED}))[ \t]*[-–—][ \t]*([+-]?(?:{UNSIGNED}))([ \t]*)({alt})(?:/({denom}))?(?![\w/])"
    ))
});

pub(super) fn lookup(key: &str) -> Option<(&'static str, &'static str, bool)> {
    tables::UNITS
        .iter()
        .find(|u| u.0 == key)
        .map(|u| (u.1, u.2, u.3))
}

fn is_one(number: &str) -> bool {
    let n = number.trim_start_matches(['-', '+']).replace(',', "");
    let (whole, frac) = n.split_once('.').unwrap_or((&n, ""));
    whole.parse::<u64>().ok() == Some(1) && frac.chars().all(|c| c == '0')
}

fn reading(number: &str, space: &str, unit: &str, denominator: Option<&str>) -> Option<String> {
    let (singular, plural, needs_space) = lookup(unit)?;
    if needs_space && space.is_empty() {
        return None;
    }
    let mut out = (if is_one(number) { singular } else { plural }).to_owned();
    if let Some(d) = denominator {
        out += " per ";
        out += lookup(d)?.0;
    }
    Some(out)
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&UNIT_RANGE, |m| {
        reading(m.g(2), m.g(3), m.g(4), m.get(5))
            .map(|u| format!("{} to {} {u}", plain_number(m.g(1)), plain_number(m.g(2))))
            .unwrap_or_else(|| m.all().to_owned())
    })?;
    t.apply(&UNIT, |m| {
        reading(m.g(1), m.g(2), m.g(3), m.get(4))
            .map(|u| format!("{} {u}", plain_number(m.g(1))))
            .unwrap_or_else(|| m.all().to_owned())
    })
}

pub fn units(text: &str) -> R<String> {
    let mut t = Text::new(&ascii_decimals(text));
    apply(&mut t)?;
    Ok(t.finish())
}
