//! Conservative English normalization for speech. See `docs/en.md` for reading conventions.

mod abbreviations;
mod clinical;
mod dates;
mod fractions;
mod identifiers;
mod markup;
mod money;
mod network;
mod numbers;
mod postal;
mod records;
mod tables {
    include!(concat!(env!("OUT_DIR"), "/en_tables.rs"));
}
mod units;

pub use abbreviations::{abbreviations, spell_letters};
pub use markup::clean_markup;
pub use money::money;
pub use numbers::numbers;
pub use units::units;

use crate::num::en as words;
use crate::pyre::{re, LazyRe, Match, Re, R};

/// Recognized spans are final, including invalid structured tokens kept verbatim. This avoids both
/// cascading rewrites (decimal -> range) and private-use placeholders colliding with user text.
struct Span {
    text: String,
    final_: bool,
}

struct Text(Vec<Span>);

impl Text {
    fn new(s: &str) -> Self {
        Self(vec![Span {
            text: s.to_owned(),
            final_: false,
        }])
    }

    fn apply(&mut self, re: &Re, render: impl Fn(&Match<'_>) -> String) -> R<()> {
        self.recognize(re, |m| Some(render(m)))
    }

    /// A rejected candidate stays raw, including its context, for subsequent recognizers.
    fn recognize(&mut self, re: &Re, render: impl Fn(&Match<'_>) -> Option<String>) -> R<()> {
        let mut out = Vec::new();
        for span in &self.0 {
            if span.final_ {
                out.push(Span {
                    text: span.text.clone(),
                    final_: true,
                });
                continue;
            }
            let mut last = 0;
            for m in re.find_iter(&span.text)? {
                let Some(rendered) = render(&m) else { continue };
                if m.start() > last {
                    out.push(Span {
                        text: span.text[last..m.start()].to_owned(),
                        final_: false,
                    });
                }
                out.push(Span {
                    text: rendered,
                    final_: true,
                });
                last = m.end();
            }
            if last < span.text.len() {
                out.push(Span {
                    text: span.text[last..].to_owned(),
                    final_: false,
                });
            }
        }
        self.0 = out;
        Ok(())
    }

    fn finish(self) -> String {
        self.0.into_iter().map(|s| s.text).collect()
    }
}

// Shared decimal grammar, longest alternatives first; no numeric parsing or rounding via floats.
const UNSIGNED: &str = r"(?:[0-9]{1,3}(?:,[0-9]{3})+|[0-9]+)(?:\.[0-9]+)?|\.[0-9]+";

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if r"\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn ascii_decimals(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c != '\n' && c.is_whitespace() {
                ' '
            } else {
                crate::pystr::decimal_value(c).map_or(c, |d| char::from(b'0' + d))
            }
        })
        .collect()
}

fn integer(s: &str) -> String {
    words::cardinal_digits(&crate::pystr::digits_of(s))
}

/// Unlike quantities, sequence/version components must preserve meaningful leading zeros.
fn sequence_component(s: &str) -> String {
    if s.len() > 1 && s.starts_with('0') {
        words::digits(s)
    } else {
        integer(s)
    }
}

fn plain_number(s: &str) -> String {
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        ("minus ", s)
    } else if let Some(s) = s.strip_prefix('+') {
        ("plus ", s)
    } else {
        ("", s)
    };
    let s = s.replace(',', "");
    match s.split_once('.') {
        Some((a, b)) => format!("{sign}{} point {}", integer(a), words::digits(b)),
        None => format!("{sign}{}", integer(&s)),
    }
}

/// A consumed abbreviation dot also serves as sentence punctuation at the end of a sentence.
fn terminal_dot(m: &Match<'_>) -> &'static str {
    if m.all().ends_with('.') && dot_ends_sentence(m.after()) {
        "."
    } else {
        ""
    }
}

/// Whether an abbreviation dot followed by `after` also ends the sentence: end of text, or a word that
/// conventionally starts one. Deliberately conservative ("Dr. Smith" stays one sentence).
pub(crate) fn dot_ends_sentence(after: &str) -> bool {
    let after = after.trim_start();
    after.is_empty()
        || after.split_whitespace().next().is_some_and(|w| {
            matches!(
                w,
                "The"
                    | "This"
                    | "That"
                    | "It"
                    | "He"
                    | "She"
                    | "They"
                    | "We"
                    | "I"
                    | "You"
                    | "However"
                    | "Then"
                    | "Let's"
                    | "Let’s"
            )
        })
}

static SPACE_BEFORE_PUNCT: LazyRe = re!(r"[ \t]+([,.;:!?])");

fn cleanup(s: &str) -> R<String> {
    // Preserve paragraph boundaries; collapse Unicode horizontal whitespace without byte indexing.
    let mut out = String::new();
    let mut blank = false;
    for c in s.chars() {
        if c == '\n' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
            blank = false;
        } else if c.is_whitespace() {
            if !blank {
                out.push(' ');
            }
            blank = true;
        } else {
            out.push(c);
            blank = false;
        }
    }
    Ok(SPACE_BEFORE_PUNCT.sub(&out, r"\1")?.trim().to_owned())
}

/// Normalize English without guessing a pronunciation for ordinary words, names or unknown acronyms.
pub fn try_normalize(text: &str) -> Result<String, crate::Error> {
    let clean = clean_markup(text)?;
    let mut t = Text::new(&ascii_decimals(&clean));
    markup::addresses(&mut t)?;
    records::apply(&mut t)?;
    identifiers::explicit(&mut t)?;
    postal::apply(&mut t)?;
    network::apply(&mut t)?;
    identifiers::versions(&mut t)?;
    clinical::apply(&mut t)?;
    fractions::apply(&mut t)?;
    dates::apply(&mut t)?;
    numbers::phones(&mut t)?;
    money::apply(&mut t)?;
    units::apply(&mut t)?;
    numbers::clocks(&mut t)?;
    abbreviations::apply(&mut t)?;
    identifiers::products(&mut t)?;
    numbers::apply(&mut t)?;
    abbreviations::letters(&mut t)?;
    abbreviations::symbols(&mut t)?;
    cleanup(&t.finish())
}

pub fn normalize(text: &str) -> String {
    try_normalize(text).unwrap_or_else(|_| text.to_owned())
}

pub fn warm_up() {
    let _ = normalize("On March 14, 2026 at 3:30 p.m. it costs $24.99, approx. 3.5 kg.");
}

#[cfg(test)]
mod tests;
