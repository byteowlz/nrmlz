//! Conservative US street/unit components, not general address or locale inference.
use super::{identifiers::code_words, terminal_dot, Text};
use crate::pyre::{re, LazyRe, R};

static STREET: LazyRe = re!(
    r"(?<!\w)([0-9]{1,6}[A-Z]?)[ \t]+(?:(NE|NW|SE|SW|N|S|E|W)\.?[ \t]+)?((?:[A-Z][A-Za-z'’-]*[ \t]+){1,5})(Street|St\.?|Road|Rd\.?|Avenue|Ave\.?|Boulevard|Blvd\.?|Lane|Ln\.?|Drive|Dr\.?|Court|Ct\.?|Way)(?!\w)"
);
static UNIT: LazyRe = re!(
    r"\b(?i:(apartment|apt\.?|suite|ste\.?|room|floor|building|bldg\.?|unit))([ \t]+#?[ \t]*|[ \t]*#[ \t]*)(?=[A-Z0-9-]*[0-9])([A-Z0-9]+(?:-[A-Z0-9]+)*)(?![\w-])"
);

fn direction(s: &str) -> &'static str {
    const WORDS: &[(&str, &str)] = &[
        ("N", "North"),
        ("S", "South"),
        ("E", "East"),
        ("W", "West"),
        ("NE", "Northeast"),
        ("NW", "Northwest"),
        ("SE", "Southeast"),
        ("SW", "Southwest"),
    ];
    WORDS
        .iter()
        .find_map(|(code, word)| (*code == s).then_some(*word))
        .unwrap_or("")
}

fn street(s: &str) -> &'static str {
    match s.trim_end_matches('.') {
        "Street" | "St" => "Street",
        "Road" | "Rd" => "Road",
        "Avenue" | "Ave" => "Avenue",
        "Boulevard" | "Blvd" => "Boulevard",
        "Lane" | "Ln" => "Lane",
        "Drive" | "Dr" => "Drive",
        "Court" | "Ct" => "Court",
        _ => "Way",
    }
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&STREET, |m| {
        let mut out = code_words(m.g(1), false);
        if let Some(d) = m.get(2) {
            out += &format!(" {}", direction(d));
        }
        format!(
            "{out} {} {}{}",
            m.g(3).trim_end(),
            street(m.g(4)),
            terminal_dot(m)
        )
    })?;
    t.apply(&UNIT, |m| {
        let name = match m.g(1).trim_end_matches('.').to_ascii_lowercase().as_str() {
            "apt" => "Apartment",
            "ste" => "Suite",
            "bldg" => "Building",
            _ => m.g(1),
        };
        format!("{name} {}", code_words(m.g(3), false))
    })
}
