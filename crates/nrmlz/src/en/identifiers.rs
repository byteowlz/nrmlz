//! Structured identities before generic quantities: never drop a zero or read a serial as a range.
use super::{integer, sequence_component, tables, words, Text};
use crate::pyre::{re, LazyRe, R};

static ISBN: LazyRe = re!(
    r"\bISBN(?:-(10|13))?([ \t]*:?[ \t]*)((?:[0-9][ -]?){12}[0-9]|(?:[0-9][ -]?){9}[0-9Xx])(?![\w-]|[ \t]+[0-9])",
    "i"
);
static CODE: LazyRe = re!(
    r"\b(PIN|ZIP(?:[ \t]+code)?|postal[ \t]+code|serial(?:[ \t]+(?:number|no\.?))?|license[ \t]+plate|VIN|part[ \t]+(?:number|code)|product[ \t]+code|ID|identifier|SKU)(?=[ \t:#0-9])([ \t]*(?::|#)?[ \t]*(?:(?:is|was)[ \t]+)?)(?=[A-Z0-9-]*[0-9])([A-Z0-9]+(?:-[A-Z0-9]+)*)(?![\w-])",
    "i"
);
static VERSION: LazyRe = re!(
    r"\b(v|version)\.?[ \t]*([0-9]+(?:\.[0-9]+)+)(?:[+-]([A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*))?(?!\w|\.[0-9])",
    "i"
);
static PRODUCT: LazyRe = re!(
    r"(?<![\w./-])(?=[A-Za-z0-9-]*[0-9])[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*(?![\w/-]|\.[0-9])"
);

fn letters(s: &str) -> String {
    s.chars()
        .map(|c| c.to_ascii_uppercase().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Explicit IDs spell all characters; product names retain readable brand words and cardinal number groups.
pub(super) fn code_words(s: &str, digitwise: bool) -> String {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<_> = s.char_indices().collect();
    while start < chars.len() {
        let numeric = chars[start].1.is_ascii_digit();
        if !chars[start].1.is_ascii_alphanumeric() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < chars.len()
            && chars[end].1.is_ascii_alphanumeric()
            && chars[end].1.is_ascii_digit() == numeric
        {
            end += 1;
        }
        let a = chars[start].0;
        let b = chars.get(end).map_or(s.len(), |c| c.0);
        let run = &s[a..b];
        out.push(if numeric {
            if digitwise {
                words::digits(run)
            } else {
                sequence_component(run)
            }
        } else if !digitwise && tables::ACRONYM_WORDS.contains(&run) {
            run.to_owned()
        } else if digitwise || run.chars().all(|c| c.is_ascii_uppercase()) || run.len() <= 2 {
            letters(run)
        } else {
            run.to_owned()
        });
        start = end;
    }
    out.join(" ")
}

pub(super) fn explicit(t: &mut Text) -> R<()> {
    t.apply(&ISBN, |m| {
        let kind = m
            .get(1)
            .map(|n| format!(" {}", integer(n)))
            .unwrap_or_default();
        let colon = if m.g(2).contains(':') { ":" } else { "" };
        format!("I S B N{kind}{colon} {}", code_words(m.g(3), true))
    })?;
    t.apply(&CODE, |m| {
        let label = match m.g(1).to_ascii_uppercase().as_str() {
            "ID" => "I D",
            "VIN" => "V I N",
            "SKU" => "S K U",
            _ => m.g(1),
        };
        format!("{label}{} {}", m.g(2).trim_end(), code_words(m.g(3), true))
    })
}

pub(super) fn versions(t: &mut Text) -> R<()> {
    t.apply(&VERSION, |m| {
        let label = if m.g(1).eq_ignore_ascii_case("v") {
            "version"
        } else {
            m.g(1)
        };
        let value = m
            .g(2)
            .split('.')
            .map(sequence_component)
            .collect::<Vec<_>>()
            .join(" point ");
        let suffix = m
            .get(3)
            .map(|s| format!(" {}", code_words(s, false)))
            .unwrap_or_default();
        format!("{label} {value}{suffix}")
    })
}

pub(super) fn products(t: &mut Text) -> R<()> {
    t.apply(&PRODUCT, |m| code_words(m.all(), false))
}
