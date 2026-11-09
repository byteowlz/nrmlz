//! Python `re` semantics on top of fancy-regex.
//!
//! The German rules are written as Python regular expressions. [`Re::new`] takes a pattern in Python syntax
//! and rewrites the parts whose meaning differs in Rust:
//!
//! - `\w`, `\W`, `\s`, `\S`, `\d`, `\D` become explicit classes from the pinned Python's Unicode database
//!   (`\w` there is letters, numbers and `_`, without combining marks; `\s` includes U+001C..U+001F);
//! - `\b` and `\B` become lookarounds over that `\w`;
//! - `$` without MULTILINE becomes `(?=\n?\z)` (Python's `$` also matches before a final newline);
//! - escapes are emitted as `\x{..}` so that both parsers read them the same way.
//!
//! [`Re::sub`] reproduces `re.sub` from Python 3.7 on: an empty match is allowed right after a non-empty one,
//! and after an empty match the next one must not be empty at the same position.

use std::sync::LazyLock;

use fancy_regex::{Captures, Regex, RegexBuilder};

use crate::pystr::class_body;
use crate::pytables::{DECIMAL, SPACE, WORD};
use crate::Error;

static WORD_BODY: LazyLock<String> = LazyLock::new(|| class_body(WORD));
static SPACE_BODY: LazyLock<String> = LazyLock::new(|| class_body(SPACE));
static DIGIT_BODY: LazyLock<String> = LazyLock::new(|| class_body(DECIMAL));

/// Backtracking budget per search. Python has none; the patterns here are linear or quadratic, so a
/// generous limit only guards against pathological input.
const BACKTRACK_LIMIT: usize = 50_000_000;

/// Translate a Python pattern into fancy-regex syntax (see the module docs). With `native`, the classes and
/// word boundaries stay Rust's own; that variant is only used on text where both agree (see [`native_safe`]).
pub fn translate(py: &str, multiline: bool, native: bool) -> String {
    let (w, s, d): (&str, &str, &str) = if native {
        (r"\w", r"\s", r"\d")
    } else {
        (&WORD_BODY, &SPACE_BODY, &DIGIT_BODY)
    };
    let w = &w.to_string();
    let s = &s.to_string();
    let d = &d.to_string();
    let word = format!("[{w}]");
    let (boundary, non_boundary) = if native {
        (r"\b".to_string(), r"\B".to_string())
    } else {
        (
            format!("(?:(?<={word})(?!{word})|(?<!{word})(?={word}))"),
            format!("(?:(?<={word})(?={word})|(?<!{word})(?!{word}))"),
        )
    };

    let chars: Vec<char> = py.chars().collect();
    let mut out = String::with_capacity(py.len() * 2);
    let mut i = 0;
    let mut in_class = false;
    let mut class_fresh = false; // directly after "[" or "[^": a "]" there is literal
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let n = *chars.get(i + 1).expect("pattern ends with a backslash");
            i += 2;
            match n {
                'w' => out += &if in_class { w.clone() } else { word.clone() },
                'W' => out += &format!("[^{w}]"),
                's' => {
                    out += &if in_class {
                        s.clone()
                    } else {
                        format!("[{s}]")
                    }
                }
                'S' => out += &format!("[^{s}]"),
                'd' => {
                    out += &if in_class {
                        d.clone()
                    } else {
                        format!("[{d}]")
                    }
                }
                'D' => out += &format!("[^{d}]"),
                'b' if !in_class => out += &boundary,
                'B' if !in_class => out += &non_boundary,
                'A' if !in_class => out += r"\A",
                'Z' if !in_class => out += r"\z",
                'n' => out += r"\n",
                't' => out += r"\t",
                'r' => out += r"\r",
                'f' => out += r"\f",
                'v' => out += r"\v",
                'u' | 'U' | 'x' => {
                    let len = match n {
                        'u' => 4,
                        'U' => 8,
                        _ => 2,
                    };
                    let hex: String = chars[i..i + len].iter().collect();
                    i += len;
                    out += &format!("\\x{{{hex}}}");
                }
                '1'..='9' if !in_class => {
                    out.push('\\');
                    out.push(n);
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        out.push(chars[i]);
                        i += 1;
                    }
                }
                _ if n.is_ascii_alphanumeric() => panic!("unsupported escape \\{n} in {py:?}"),
                _ => out += &format!("\\x{{{:X}}}", n as u32),
            }
            class_fresh = false;
            continue;
        }
        if in_class {
            match c {
                ']' if !class_fresh => {
                    in_class = false;
                    out.push(']');
                }
                '^' if class_fresh && chars[i - 1] == '[' => out.push('^'),
                '[' | '&' | '~' | ']' => out += &format!("\\x{{{:X}}}", c as u32),
                _ => out.push(c),
            }
            class_fresh = c == '^' && chars[i - 1] == '[';
        } else {
            match c {
                '[' => {
                    in_class = true;
                    class_fresh = true;
                    out.push('[');
                }
                '$' if !multiline => out += r"(?=\n?\z)",
                _ => out.push(c),
            }
        }
        i += 1;
    }
    out
}

/// Code points where Python's `\w`, `\d`, `\s` and Rust's Unicode classes agree (checked exhaustively in
/// the tests). Text made only of these runs on the faster native variant of each pattern. Left out: the
/// separators U+001C..U+001F (Python spaces), superscripts and fractions (Python word characters), combining
/// marks, variation selectors, joiners and connector punctuation (Rust word characters).
pub(crate) fn native_safe_char(c: char) -> bool {
    let cp = c as u32;
    match cp {
        0x00..=0x1B | 0x20..=0x7F => true,
        0x80..=0x24F => !matches!(cp, 0xB2 | 0xB3 | 0xB9 | 0xBC..=0xBE),
        0x2000..=0x206F => !matches!(cp, 0x200C | 0x200D | 0x203F | 0x2040 | 0x2054),
        0x20A0..=0x20CF | 0x2190..=0x23FF | 0x2500..=0x2775 | 0x2794..=0x2BFF => true,
        0x1F000..=0x1F0FF | 0x1F18A..=0x1FAFF => true,
        _ => false,
    }
}

/// Whether `text` can use the native variant.
pub(crate) fn native_safe(text: &str) -> bool {
    text.bytes()
        .all(|b| b < 0x80 && !(0x1C..=0x1F).contains(&b))
        || text.chars().all(native_safe_char)
}

/// A compiled Python pattern: the exact translation, and a native one for text where they agree.
#[derive(Debug)]
pub struct Re {
    exact: std::sync::OnceLock<Regex>,
    native: std::sync::OnceLock<Regex>,
    pattern: String,
    multiline: bool,
    flags: &'static str,
    /// Literal texts of which every match contains at least one: if none occurs, there is nothing to search.
    needles: &'static [&'static str],
    #[cfg(feature = "coverage")]
    src: String,
}

/// Rule coverage for the differential test: how often each pattern matched (feature `coverage`).
#[cfg(feature = "coverage")]
pub mod coverage {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    static HITS: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
    static TIME: Mutex<BTreeMap<String, std::time::Duration>> = Mutex::new(BTreeMap::new());

    pub(crate) fn time(p: &str, d: std::time::Duration) {
        *TIME.lock().unwrap().entry(p.to_string()).or_default() += d;
    }

    /// Time spent in each pattern's searches, longest first.
    pub fn timings() -> Vec<(String, std::time::Duration)> {
        let mut v: Vec<_> = TIME
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        v
    }

    pub(crate) fn register(p: &str) {
        HITS.lock().unwrap().entry(p.to_string()).or_insert(0);
    }

    pub(crate) fn hit(p: &str) {
        *HITS.lock().unwrap().entry(p.to_string()).or_insert(0) += 1;
    }

    /// Every pattern compiled so far with its match count.
    pub fn snapshot() -> Vec<(String, u64)> {
        HITS.lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}

/// One match, with the subject string it was found in (Python's `m.string`).
pub struct Match<'t> {
    caps: Captures<'t, str>,
    pub text: &'t str,
}

impl<'t> Match<'t> {
    pub fn start(&self) -> usize {
        self.caps.get(0).unwrap().start()
    }
    pub fn end(&self) -> usize {
        self.caps.get(0).unwrap().end()
    }
    /// `m.group(i)`, or `None` if the group did not take part.
    pub fn get(&self, i: usize) -> Option<&'t str> {
        self.caps.get(i).map(|m| m.as_str())
    }
    /// `m.group(i)`, with a group that did not take part read as "".
    pub fn g(&self, i: usize) -> &'t str {
        self.get(i).unwrap_or("")
    }
    pub fn all(&self) -> &'t str {
        self.g(0)
    }
    /// `m.string[:m.start()]`
    pub fn before(&self) -> &'t str {
        &self.text[..self.start()]
    }
    /// `m.string[m.end():]`
    pub fn after(&self) -> &'t str {
        &self.text[self.end()..]
    }
}

pub type R<T> = Result<T, Error>;

fn next_char(s: &str, pos: usize) -> usize {
    s[pos..]
        .chars()
        .next()
        .map_or(pos + 1, |c| pos + c.len_utf8())
}

impl Re {
    /// Compile a Python pattern. Flags as in Python: `s` (DOTALL), `i` (IGNORECASE). MULTILINE is read from a
    /// leading `(?m)`.
    pub fn with_flags(py: &str, flags: &'static str) -> Re {
        #[cfg(feature = "coverage")]
        coverage::register(py);
        Re {
            exact: std::sync::OnceLock::new(),
            native: std::sync::OnceLock::new(),
            pattern: py.to_string(),
            multiline: py.starts_with("(?m)"),
            flags,
            needles: &[],
            #[cfg(feature = "coverage")]
            src: py.to_string(),
        }
    }

    /// Declare literals of which every match contains one (a pre-check; must hold for the pattern).
    pub fn needs(mut self, needles: &'static [&'static str]) -> Re {
        self.needles = needles;
        self
    }

    fn may_match(&self, text: &str) -> bool {
        self.needles.is_empty() || self.needles.iter().any(|n| text.contains(n))
    }

    fn compile(&self, native: bool) -> Regex {
        let mut pat = translate(&self.pattern, self.multiline, native);
        if !self.flags.is_empty() {
            pat = format!("(?{}){pat}", self.flags);
        }
        RegexBuilder::new(&pat)
            .backtrack_limit(BACKTRACK_LIMIT)
            .build()
            .unwrap_or_else(|e| panic!("pattern {:?} does not compile: {e}", self.pattern))
    }

    /// The variant to use on `text`.
    fn re(&self, text: &str) -> &Regex {
        if native_safe(text) {
            self.native.get_or_init(|| self.compile(true))
        } else {
            self.exact.get_or_init(|| self.compile(false))
        }
    }

    pub fn new(py: &str) -> Re {
        Re::with_flags(py, "")
    }

    fn captures_from<'t>(&self, text: &'t str, pos: usize) -> R<Option<Match<'t>>> {
        if !self.may_match(text) {
            #[cfg(feature = "verify-native")]
            assert!(self
                .exact
                .get_or_init(|| self.compile(false))
                .captures_from_pos(text, pos)?
                .is_none());
            return Ok(None);
        }
        #[cfg(feature = "coverage")]
        let t = std::time::Instant::now();
        let m = self
            .re(text)
            .captures_from_pos(text, pos)?
            .map(|caps| Match { caps, text });
        #[cfg(feature = "verify-native")]
        if native_safe(text) {
            // the native variant must find exactly what the exact translation finds
            let exact = self
                .exact
                .get_or_init(|| self.compile(false))
                .captures_from_pos(text, pos)?;
            let spans = |c: &Captures<'t, str>| {
                (0..c.len())
                    .map(|i| c.get(i).map(|g| g.range()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                m.as_ref().map(|m| spans(&m.caps)),
                exact.as_ref().map(spans),
                "native and exact variants of {:?} differ on {text:?} at {pos}",
                self.pattern
            );
        }
        #[cfg(feature = "coverage")]
        coverage::time(&self.src, t.elapsed());
        #[cfg(feature = "coverage")]
        if m.is_some() {
            coverage::hit(&self.src);
        }
        Ok(m)
    }

    /// `pattern.search(text)`
    pub fn search<'t>(&self, text: &'t str) -> R<Option<Match<'t>>> {
        self.captures_from(text, 0)
    }

    /// `pattern.search(text, pos, endpos)`: the text past `endpos` does not exist for the pattern.
    pub fn search_range<'t>(
        &self,
        text: &'t str,
        pos: usize,
        endpos: usize,
    ) -> R<Option<Match<'t>>> {
        self.captures_from(&text[..endpos], pos)
    }

    /// `pattern.match(text, pos, endpos)`: anchored at `pos`.
    pub fn match_range<'t>(
        &self,
        text: &'t str,
        pos: usize,
        endpos: usize,
    ) -> R<Option<Match<'t>>> {
        Ok(self
            .captures_from(&text[..endpos], pos)?
            .filter(|m| m.start() == pos))
    }

    /// `pattern.match(text, pos)`
    pub fn match_at<'t>(&self, text: &'t str, pos: usize) -> R<Option<Match<'t>>> {
        self.match_range(text, pos, text.len())
    }

    pub fn is_match(&self, text: &str) -> R<bool> {
        if !self.may_match(text) {
            return Ok(false);
        }
        let hit = self.re(text).is_match(text)?;
        #[cfg(feature = "verify-native")]
        if native_safe(text) {
            assert_eq!(
                hit,
                self.exact
                    .get_or_init(|| self.compile(false))
                    .is_match(text)?,
                "{:?}",
                self.pattern
            );
        }
        #[cfg(feature = "coverage")]
        if hit {
            coverage::hit(&self.src);
        }
        Ok(hit)
    }

    /// `pattern.finditer(text)`
    pub fn find_iter<'t>(&self, text: &'t str) -> R<Vec<Match<'t>>> {
        let mut out = Vec::new();
        self.walk(text, |m| {
            out.push(m);
            Ok(())
        })?;
        Ok(out)
    }

    /// Visit the matches in the order `re.sub` / `re.finditer` find them.
    fn walk<'t>(&self, text: &'t str, mut f: impl FnMut(Match<'t>) -> R<()>) -> R<()> {
        let mut pos = 0;
        let mut must_advance = false;
        while pos <= text.len() {
            let Some(m) = self.captures_from(text, pos)? else {
                break;
            };
            if must_advance && m.start() == pos && m.end() == pos {
                // No empty match where the last empty one ended. (The patterns used here are either always
                // empty or never empty, so trying the next position is the same as Python's retry.)
                if pos >= text.len() {
                    break;
                }
                pos = next_char(text, pos);
                must_advance = false;
                continue;
            }
            let (start, end) = (m.start(), m.end());
            f(m)?;
            must_advance = start == end;
            pos = end;
        }
        Ok(())
    }

    /// `re.sub(pattern, function, text)`
    pub fn sub_fn(&self, text: &str, mut f: impl FnMut(&Match) -> R<String>) -> R<String> {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        self.walk(text, |m| {
            out.push_str(&text[last..m.start()]);
            out.push_str(&f(&m)?);
            last = m.end();
            Ok(())
        })?;
        out.push_str(&text[last..]);
        Ok(out)
    }

    /// `re.sub(pattern, template, text)` with a Python template (`\1`, `\g<1>`, `\\`).
    pub fn sub(&self, text: &str, template: &str) -> R<String> {
        let parts = parse_template(template);
        self.sub_fn(text, |m| {
            let mut s = String::new();
            for p in &parts {
                match p {
                    TplPart::Lit(l) => s.push_str(l),
                    TplPart::Group(g) => s.push_str(m.g(*g)),
                }
            }
            Ok(s)
        })
    }

    /// `re.sub(pattern, literal, text)`
    pub fn sub_lit(&self, text: &str, literal: &str) -> R<String> {
        self.sub_fn(text, |_| Ok(literal.to_string()))
    }
}

enum TplPart {
    Lit(String),
    Group(usize),
}

fn parse_template(t: &str) -> Vec<TplPart> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let n = chars[i + 1];
            if n.is_ascii_digit() {
                let mut j = i + 1;
                let mut num = 0;
                while j < chars.len() && chars[j].is_ascii_digit() && j < i + 3 {
                    num = num * 10 + chars[j].to_digit(10).unwrap() as usize;
                    j += 1;
                }
                parts.push(TplPart::Lit(std::mem::take(&mut lit)));
                parts.push(TplPart::Group(num));
                i = j;
                continue;
            }
            if n == 'g' && chars.get(i + 2) == Some(&'<') {
                let close = chars[i + 3..].iter().position(|&x| x == '>').unwrap() + i + 3;
                let num: usize = chars[i + 3..close]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap();
                parts.push(TplPart::Lit(std::mem::take(&mut lit)));
                parts.push(TplPart::Group(num));
                i = close + 1;
                continue;
            }
            lit.push(match n {
                'n' => '\n',
                't' => '\t',
                '\\' => '\\',
                _ => panic!("unsupported template escape \\{n}"),
            });
            i += 2;
            continue;
        }
        lit.push(c);
        i += 1;
    }
    parts.push(TplPart::Lit(lit));
    parts
}

/// A lazily compiled Python pattern: `static RX: Lazy = re!(r"...")`.
pub type LazyRe = LazyLock<Re>;

macro_rules! re {
    ($p:expr) => {
        std::sync::LazyLock::new(|| $crate::pyre::Re::new($p))
    };
    ($p:expr, $flags:expr) => {
        std::sync::LazyLock::new(|| $crate::pyre::Re::with_flags($p, $flags))
    };
    ($p:expr, $flags:expr; needs $n:expr) => {
        std::sync::LazyLock::new(|| $crate::pyre::Re::with_flags($p, $flags).needs($n))
    };
    ($p:expr; needs $n:expr) => {
        std::sync::LazyLock::new(|| $crate::pyre::Re::new($p).needs($n))
    };
}
pub(crate) use re;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_sub_semantics() {
        // re.sub('x*', '-', 'abxd') == '-a-b--d-'
        assert_eq!(Re::new("x*").sub_lit("abxd", "-").unwrap(), "-a-b--d-");
        assert_eq!(
            Re::new(r"(?<=\d)(?=[a-z])").sub_lit("3a4b", " ").unwrap(),
            "3 a4 b"
        );
        assert_eq!(
            Re::new(r"(\w+) (\w+)").sub("ab cd", r"\2 \1").unwrap(),
            "cd ab"
        );
    }

    #[test]
    fn python_dollar_and_word() {
        // Python: re.search('a$', 'ba\n') matches, '\w' covers '²' but not a combining mark.
        assert!(Re::new("a$").is_match("ba\n").unwrap());
        assert!(!Re::new("a$").is_match("ba\n\n").unwrap());
        assert!(Re::new(r"^\w+$").is_match("m²").unwrap());
        assert!(!Re::new(r"^\w+$").is_match("a\u{308}").unwrap());
        assert!(Re::new(r"\bPLZ\b").is_match("die PLZ ist").unwrap());
        assert!(!Re::new(r"\bm\b").is_match("m²").unwrap());
        assert!(Re::new(r"(?m)x$").is_match("x\ny").unwrap());
        assert!(Re::new(r"[^\W\d_]").is_match("1_a").unwrap());
        assert!(!Re::new(r"[^\W\d_]").is_match("1_2").unwrap());
        assert!(Re::new(r"\s").is_match("\u{1c}").unwrap());
        assert!(Re::new(r"[)(\]\[DP]+").is_match("]").unwrap());
    }

    #[test]
    fn native_classes_agree_on_safe_text() {
        use crate::pystr::{is_space, is_word};
        let (w, d, sp) = (
            regex::Regex::new(r"^\w$").unwrap(),
            regex::Regex::new(r"^\d$").unwrap(),
            regex::Regex::new(r"^\s$").unwrap(),
        );
        let mut checked = 0;
        let mut bad = Vec::new();
        for cp in 0..=0x10FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if !native_safe_char(c) {
                continue;
            }
            let s = c.to_string();
            if is_word(c) != w.is_match(&s)
                || crate::pystr::decimal_value(c).is_some() != d.is_match(&s)
                || is_space(c) != sp.is_match(&s)
            {
                bad.push(format!("U+{cp:04X}"));
            }
            checked += 1;
        }
        assert!(bad.is_empty(), "classes differ at {}", bad.join(" "));
        assert!(checked > 5000);
    }

    #[test]
    fn ranges() {
        let re = Re::new(r"[ \t]*$");
        let m = re.search_range("ab  cd", 0, 4).unwrap().unwrap();
        assert_eq!((m.start(), m.end()), (2, 4));
        assert!(Re::new("b").match_at("ab", 1).unwrap().is_some());
        assert!(Re::new("b").match_at("abb", 0).unwrap().is_none());
    }
}
