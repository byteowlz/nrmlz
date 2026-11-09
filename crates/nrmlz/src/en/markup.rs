//! Light Markdown/HTML cleanup and explicit address pronunciation; not a document parser.
use super::{words, Text};
use crate::pyre::{re, LazyRe, R};

static LINK: LazyRe = re!(r#"!?\[([^\]\n]*)\]\([^\s()]+(?:[ \t]+"[^"\n]*")?\)"#);
static AUTOLINK: LazyRe = re!(r"<(https?://[^<>\s]+|[^<>\s]+@[^<>\s]+)>");
static BLOCK_TAG: LazyRe = re!(
    r"</?(?:p|div|br|li|h[1-6]|ul|ol|blockquote)(?:[ \t]+[^<>]*)?/?>",
    "i"
);
static TAG: LazyRe = re!(r"</?[A-Za-z][A-Za-z0-9]*(?:[ \t]+[^<>]*)?/?>");
static FENCE: LazyRe = re!(r"(?m)^[ \t]*```[^\n]*");
static HEADING: LazyRe = re!(r"(?m)^[ \t]{0,3}(?:#{1,6}[ \t]+|>[ \t]?)");
static BOLD: LazyRe = re!(r"\*\*([^*\n]+)\*\*|__([^_\n]+)__");
static EMPHASIS: LazyRe = re!(r"(?<!\w)\*([^*\n]+)\*(?!\w)|(?<!\w)_([^_\n]+)_(?!\w)");
static INLINE_CODE: LazyRe = re!(r"`([^`\n]+)`");
static ENTITY: LazyRe = re!(r"&(#x[0-9a-fA-F]+|#[0-9]+|amp|lt|gt|quot|apos|nbsp);");
static EMAIL: LazyRe = re!(r"(?<![\w.+%-])([\w.+%-]+@(?:[\w-]+\.)+[\w-]{2,63})(?![\w@-])");
static URL: LazyRe = re!(r#"(?<![\w@])(?:https?://|www\.)[^\s<>"'`()]+"#, "i");
static UNDECODED_ENTITY: LazyRe = re!(r"&(?:#[xX]?[0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);");
static DOMAIN: LazyRe = re!(
    r#"(?<![\w@.-])(?:[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?\.)+(?:com|org|net|edu|gov|io|dev|co|uk|de|ai)(?:/[^\s<>"'`()]+)?(?!\w)"#,
    "i"
);

/// Keep link labels and inline contents; decode common/numeric HTML entities. Unknown entities survive.
pub fn clean_markup(text: &str) -> R<String> {
    let t = LINK.sub(text, r"\1")?;
    let t = AUTOLINK.sub(&t, r"\1")?;
    let t = BLOCK_TAG.sub_lit(&t, " ")?;
    let t = TAG.sub_lit(&t, "")?;
    let t = FENCE.sub_lit(&t, "")?;
    let t = HEADING.sub_lit(&t, "")?;
    let t = BOLD.sub_fn(&t, |m| Ok(m.get(1).unwrap_or_else(|| m.g(2)).to_owned()))?;
    let t = EMPHASIS.sub_fn(&t, |m| Ok(m.get(1).unwrap_or_else(|| m.g(2)).to_owned()))?;
    let t = INLINE_CODE.sub(&t, r"\1")?;
    ENTITY.sub_fn(&t, |m| {
        let s = match m.g(1) {
            "amp" => "&".to_owned(),
            "lt" => "<".to_owned(),
            "gt" => ">".to_owned(),
            "quot" => "\"".to_owned(),
            "apos" => "'".to_owned(),
            "nbsp" => " ".to_owned(),
            value => {
                let code = if let Some(hex) = value.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    value.strip_prefix('#').and_then(|s| s.parse::<u32>().ok())
                };
                code.and_then(char::from_u32)
                    .filter(|c| !c.is_control())
                    .map_or_else(|| m.all().to_owned(), |c| c.to_string())
            }
        };
        Ok(s)
    })
}

fn address(s: &str) -> String {
    let scheme_end = s
        .find("://")
        .filter(|&i| s[..i].eq_ignore_ascii_case("http") || s[..i].eq_ignore_ascii_case("https"));
    let s = scheme_end.map_or(s, |i| &s[i + 3..]);
    let mut out = Vec::new();
    let mut run = String::new();
    for c in s.chars() {
        let name = match c {
            '@' => Some("at"),
            '.' => Some("dot"),
            '_' => Some("underscore"),
            '-' => Some("dash"),
            '+' => Some("plus"),
            '/' => Some("slash"),
            ':' => Some("colon"),
            '?' => Some("question mark"),
            '=' => Some("equals"),
            '&' => Some("and"),
            '#' => Some("hash"),
            '%' => Some("percent"),
            '~' => Some("tilde"),
            _ => None,
        };
        if c.is_ascii_digit() || name.is_some() {
            if !run.is_empty() {
                out.push(std::mem::take(&mut run));
            }
            out.push(name.map_or_else(|| words::digits(&c.to_string()), str::to_owned));
        } else {
            run.push(c);
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out.into_iter()
        .map(|s| {
            if s.eq_ignore_ascii_case("www") {
                "W W W".to_owned()
            } else {
                s
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn url(s: &str) -> String {
    let trimmed = s.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    format!("{}{}", address(trimmed), &s[trimmed.len()..])
}

pub(super) fn addresses(t: &mut Text) -> R<()> {
    t.apply(&UNDECODED_ENTITY, |m| m.all().to_owned())?;
    t.apply(&EMAIL, |m| address(m.all()))?;
    t.apply(&URL, |m| url(m.all()))?;
    t.apply(&DOMAIN, |m| url(m.all()))
}
