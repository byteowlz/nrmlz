//! Markdown, LaTeX, emoji, smileys, arrows, list numbers, e-mail and web addresses. Port of the "markup"
//! section of german.py: language models answer in Markdown, which the model would read out or stumble over.

use std::sync::LazyLock;

use crate::pyre::{re, LazyRe, Match, Re, R};
use crate::pystr::{capitalize, is_alnum, is_upper, lstrip, rstrip};

use super::math::latex;
use super::numbers::ordinal_str;
use super::{Fix, Profile, MONTHS};

static CODE_FENCE: LazyRe = re!(r"```[\s\S]*?```"; needs &["```"]);
static RULE_LINE: LazyRe =
    re!(r"(?m)^[ \t]*([-*_])[ \t]*(?:\1[ \t]*){2,}$"; needs &["-", "*", "_"]);
static HEADING: LazyRe = re!(r"(?m)^[ \t]{0,3}#{1,6}[ \t]*"; needs &["#"]);
// Markdown bullets are only -, * and + (and the bullet sign); a dash at the start of a line is punctuation.
static BULLET: LazyRe = re!(r"(?m)^[ \t]{0,6}[-*+\u2022][ \t]+"; needs &["-", "*", "+", "•"]);
// "25. Juli 2000" at the start of a line is a date, not a list number.
static LIST_NUMBER: LazyRe = LazyLock::new(|| {
    Re::new(&format!(
        r"(?m)^[ \t]{{0,6}}\d{{1,2}}[.)][ \t]+(?![ \t])(?!(?:{MONTHS})\b)"
    ))
});
static QUOTE: LazyRe = re!(r"(?m)^[ \t]{0,3}>[ \t]?"; needs &[">"]);
static TABLE_ROW: LazyRe = re!(r"(?m)^[ \t]*\|(.*)\|[ \t]*$"; needs &["|"]);
static TABLE_SEPARATOR: LazyRe = re!(r"(?m)^[ \t]*\|[\s:|-]+\|[ \t]*$"; needs &["|"]);
static LINK: LazyRe = re!(r"\[([^\]]*)\]\([^)]*\)"; needs &["]("]);
static EMPHASIS: LazyRe = re!(r"(\*\*|__|\*|_)(?=\S)(.+?)(?<=\S)\1"; needs &["*", "_"]);
static NAME_UNDERSCORE: LazyRe = re!(r"(?<=\w)_+(?=\w)"; needs &["_"]);
static PY_SUFFIX: LazyRe = re!(r"\.py(?!\w)"; needs &[".py"]);
static BLANK_LINES: LazyRe = re!(r"\n{2,}"; needs &["\n\n"]);

static TLD_NAMES: &[(&str, &str)] = &[
    ("de", "deh eh"),
    ("at", "ah teh"),
    ("ch", "zeh hah"),
    ("eu", "eh uh"),
    ("io", "ih oh"),
    ("ai", "ah ih"),
    ("uk", "uh kah"),
    ("us", "uh ess"),
    ("fr", "eff err"),
    ("it", "ih teh"),
    ("nl", "enn ell"),
    ("tv", "teh fau"),
    ("me", "emm eh"),
    ("com", "komm"),
    ("net", "nett"),
    ("org", "org"),
    ("dev", "dev"),
    ("app", "äpp"),
    ("info", "info"),
    ("edu", "eh deh uh"),
    ("gov", "gaff"),
    ("online", "onlein"),
    ("shop", "schopp"),
];

pub(crate) fn tld_name(t: &str) -> Option<&'static str> {
    TLD_NAMES.iter().find(|(k, _)| *k == t).map(|(_, v)| *v)
}

static EMAIL: LazyRe =
    re!(r"(?<![\w.@])([\w.+-]+)@([\w-]+(?:\.[\w-]+)*\.([a-zA-Z]{2,6}))\b"; needs &["@"]);
static URL: LazyRe = LazyLock::new(|| {
    let mut tlds: Vec<&str> = TLD_NAMES.iter().map(|t| t.0).collect();
    tlds.sort_by_key(|t| std::cmp::Reverse(t.len()));
    Re::new(&format!(
        r"(?<![\w@/.])(?:https?://)?((?:www\.)?(?:[\w-]+\.)+({}))\b((?:/[\w~%+-]+(?:\.[\w~%+-]+)*)*/?)(?:\?\S*)?",
        tlds.join("|")
    ))
});
static LOCAL_SEPARATOR: LazyRe = re!(r"[._]");
static SYMBOL_LINE: LazyRe = re!(r"(?m)^[ \t]*[^\wÀ-ɏ\n]+[ \t]*$");
// Emoji and smileys are removed (as a space, so "super😂toll" does not become "supertoll").
static EMOJI: LazyRe = re!(
    r"[\U0001F000-\U0001FAFF\u2600-\u27BF\u2B00-\u2BFF\uFE0E\uFE0F\u200D\u20E3\U000E0020-\U000E007F\u3030\u303D]+"
);
// text smileys only when free-standing: ":)" yes, "12:30" and "1:3" no
static SMILEY: LazyRe = re!(concat!(
    r"(?<![\w])(?:[:;=][-'o^]?[)(\]\[DPpOo3|/\\]+|[xX]D+|\^\^|<3",
    r"|¯\\_\(ツ\)_/¯)(?=\s|$|[.,!?])"
); needs &[":", ";", "=", "x", "X", "^", "<3", "¯"]);
// arrows ("Berlin → Hauptstadt", "A -> B") are a pause, not a word
static ARROW: LazyRe = re!(r"[ \t]*(?:[\u2190-\u21FF\u27F5-\u27FF]|->|=>)[ \t]*");
static TIMES_SPACED: LazyRe = re!(r"(?<=[\w)])\s+\*\s+(?=[\w(])"; needs &["*"]);
static TIMES_TIGHT: LazyRe = re!(r"(?<=\d)\*(?=\d)"; needs &["*"]);
static FIRST_NUMBER: LazyRe = re!(r"\d+");
static COLON_END: LazyRe = re!(r"(?m):[ \t]*$"; needs &[":"]);
static WORD_END: LazyRe = re!(r#"(?m)([\wÀ-ɏ)"“”'])[ \t]*$"#);
static NEWLINE: LazyRe = re!(r"[ \t]*\n[ \t]*"; needs &["\n"]);

pub(crate) fn domain(d: &str) -> String {
    let parts: Vec<&str> = d.split('.').collect();
    let (last, head) = parts.split_last().unwrap();
    let mut words: Vec<String> = head
        .iter()
        .map(|t| {
            if t.to_lowercase() == "www" {
                "weh weh weh".to_string()
            } else {
                t.to_string()
            }
        })
        .collect();
    words.push(tld_name(&last.to_lowercase()).map_or_else(|| last.to_string(), str::to_string));
    words
        .iter()
        .map(|w| w.replace('-', " Strich "))
        .collect::<Vec<_>>()
        .join(" Punkt ")
}

fn email(m: &Match) -> R<String> {
    let who = LOCAL_SEPARATOR
        .sub_lit(m.g(1), " Punkt ")?
        .replace('-', " Strich ")
        .replace('+', " plus ");
    Ok(format!("{who} at {}", domain(m.g(2))))
}

fn url(m: &Match) -> R<String> {
    // without scheme and without www/known ending it is no address: "z.B." and "Version 3.2" stay
    let path: Vec<&str> = m.g(3).split('/').filter(|x| !x.is_empty()).collect();
    let path = path.join(" Schrägstrich ");
    let mut out = domain(m.g(1));
    if !path.is_empty() {
        out += " Schrägstrich ";
        out += &path;
    }
    Ok(out)
}

/// Remove an emoji/smiley. If it stood in place of punctuation ("regnen :) Nimm ..."), a comma keeps the pause.
fn drop(m: &Match) -> R<String> {
    let before = rstrip(m.before());
    let after = lstrip(m.after());
    let alnum_before = before.chars().last().is_some_and(is_alnum);
    let upper_after = after.chars().next().is_some_and(is_upper);
    Ok(if alnum_before && upper_after {
        ", "
    } else {
        " "
    }
    .to_string())
}

/// List number as an enumeration word: 1. -> Erstens, 3) -> Drittens (ordinal + "ns").
fn list_word(m: &Match, p: Profile) -> R<String> {
    let n = FIRST_NUMBER.search(m.all())?.unwrap();
    let ordinal = ordinal_str(n.all(), "")?;
    if ordinal == "nullte" {
        return Ok(String::new());
    }
    let word = capitalize(&ordinal) + "ns";
    if p.has(Fix::SingleListWord) {
        // "1. Erstens das": the item already says it
        let after = m.after();
        if let Some(rest) = after
            .get(..word.len())
            .filter(|h| h.to_lowercase() == word.to_lowercase())
        {
            let next = after[rest.len()..].chars().next();
            if !next.is_some_and(crate::pystr::is_word) {
                return Ok(String::new());
            }
        }
    }
    Ok(word + ", ")
}

/// Turn Markdown (and LaTeX, emoji, addresses) into something that can be read aloud.
pub fn clean_markup(text: &str, p: Profile) -> R<String> {
    if text.is_empty() {
        return Ok(text.to_string());
    }
    let mut text = CODE_FENCE.sub_lit(text, " ")?;
    text = latex(&text, p)?; // before emphasis: _ and * belong to the formula there
    text = TIMES_SPACED.sub_lit(&text, " mal ")?; // 5 * 3, before the asterisks go
    text = TIMES_TIGHT.sub_lit(&text, " mal ")?; // 5*3
    text = text.replace('`', "");
    text = EMOJI.sub_fn(&text, drop)?;
    text = SMILEY.sub_fn(&text, drop)?;
    text = ARROW.sub_lit(&text, ", ")?;
    text = TABLE_SEPARATOR.sub_lit(&text, "")?; // |---|---| goes entirely
    text = TABLE_ROW.sub_fn(
        &text,
        |m| Ok(crate::pystr::strip(m.g(1)).replace('|', ", ")),
    )?;
    text = RULE_LINE.sub_lit(&text, "")?;
    text = HEADING.sub_lit(&text, "")?;
    text = QUOTE.sub_lit(&text, "")?;
    text = LINK.sub(&text, r"\1")?;
    text = NAME_UNDERSCORE.sub_lit(&text, " ")?;
    text = PY_SUFFIX.sub_lit(&text, " Punkt Python")?;
    text = EMAIL.sub_fn(&text, email)?;
    text = URL.sub_fn(&text, url)?;
    // Emphasis before list numbers: "**3. Wirtschaft:**" starts with asterisks, the number is at the line
    // start only afterwards.
    text = EMPHASIS.sub(&text, r"\2")?;
    // Asterisks and underscores left over belong to nothing (e.g. bold cut at a chunk border).
    text = text.replace('*', "").replace('_', " ");
    text = BULLET.sub_lit(&text, "")?;
    text = LIST_NUMBER.sub_fn(&text, |m| list_word(m, p))?; // "2. " -> "Zweitens, "
    text = SYMBOL_LINE.sub_lit(&text, "")?; // lines without any letter
    text = BLANK_LINES.sub_lit(&text, "\n")?; // blank lines -> one line break
                                              // Lines are pauses. A heading ("Wirtschaft:") or list item gets a sentence end, otherwise the model reads
                                              // it into the next sentence. Line breaks never occurred in training, so they become spaces.
    text = COLON_END.sub_lit(&text, ".")?;
    text = WORD_END.sub(&text, r"\1.")?;
    NEWLINE.sub_lit(&text, " ")
}
