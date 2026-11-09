//! Table-driven abbreviations, deliberate initialisms and symbol names. No word respelling heuristics.
use std::sync::LazyLock;

use super::{escape, tables, terminal_dot, Text};
use crate::pyre::{re, LazyRe, Re, R};

type Rule = (Re, &'static str);

fn rules(table: &'static [(&str, &str)]) -> Vec<Rule> {
    let mut pairs = table.to_vec();
    pairs.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    pairs
        .into_iter()
        .map(|(k, v)| (Re::new(&format!(r"(?<!\w){}(?!\w)", escape(k))), v))
        .collect()
}

static PLAIN: LazyLock<Vec<Rule>> = LazyLock::new(|| rules(tables::ABBREVIATIONS));
static NAMES: LazyLock<Vec<Rule>> = LazyLock::new(|| rules(tables::BEFORE_NAME));
static NUMBER: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let mut pairs = tables::BEFORE_NUMBER.to_vec();
    pairs.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    pairs
        .into_iter()
        .map(|(k, v)| (Re::new(&format!(r"(?<!\w){}(?=[ \t]*[0-9])", escape(k))), v))
        .collect()
});
static SYMBOLS: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let mut pairs = tables::SYMBOLS.to_vec();
    pairs.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    pairs
        .into_iter()
        .map(|(k, v)| (Re::new(&escape(k)), v))
        .collect()
});
static PLUS: LazyRe = re!(r"(?<![\w+])\+(?![\w+])");
static MINUS_SYMBOL: LazyRe = re!(r"−");
static COMPARISON: LazyRe = re!(r"[<>]");
static CAPITALS: LazyRe = re!(r"(?<!\w)([A-Z]{2,})(s?)(?!\w)");

pub(super) fn apply(t: &mut Text) -> R<()> {
    for (re, value) in NUMBER.iter() {
        t.apply(re, |_| format!("{value} "))?;
    }
    for (re, value) in NAMES.iter() {
        t.apply(re, |m| {
            let next = m.after().trim_start().chars().next();
            if next.is_some_and(char::is_uppercase) {
                return value.to_string();
            }
            let previous = m.before().split_whitespace().next_back().unwrap_or("");
            let after_name = tables::AFTER_NAME.iter().find(|(k, _)| *k == m.all());
            if previous.chars().next().is_some_and(char::is_uppercase) {
                if let Some((_, word)) = after_name {
                    return format!("{word}{}", terminal_dot(m));
                }
            }
            m.all().to_owned()
        })?;
    }
    for (re, value) in PLAIN.iter() {
        t.apply(re, |m| format!("{value}{}", terminal_dot(m)))?;
    }
    Ok(())
}

pub(super) fn letters(t: &mut Text) -> R<()> {
    t.apply(&CAPITALS, |m| {
        let w = m.g(1);
        if tables::ALWAYS_SPELLED.contains(&w) && !tables::ACRONYM_WORDS.contains(&w) {
            format!(
                "{}{}",
                w.chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
                m.g(2)
            )
        } else {
            m.all().to_owned()
        }
    })
}

pub(super) fn symbols(t: &mut Text) -> R<()> {
    for (re, value) in SYMBOLS.iter() {
        t.apply(re, |_| format!(" {value} "))?;
    }
    t.apply(&PLUS, |_| " plus ".to_owned())?;
    t.apply(&MINUS_SYMBOL, |_| " minus ".to_owned())?;
    t.apply(&COMPARISON, |m| {
        if m.all() == "<" {
            " is less than ".to_owned()
        } else {
            " is greater than ".to_owned()
        }
    })
}

pub fn abbreviations(text: &str) -> R<String> {
    let mut t = Text::new(text);
    apply(&mut t)?;
    symbols(&mut t)?;
    super::cleanup(&t.finish())
}

pub fn spell_letters(text: &str) -> R<String> {
    let mut t = Text::new(text);
    letters(&mut t)?;
    Ok(t.finish())
}
