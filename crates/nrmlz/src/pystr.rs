//! Python `str` semantics the port relies on: character classes from the pinned Python's Unicode database,
//! `strip`, `isdigit`, `isupper`, code-point slicing and integer parsing of Unicode digits.

use crate::pytables::{DECIMAL, DIGIT, SPACE, UPPER, WORD};

fn in_table(table: &[(u32, u32)], c: char) -> bool {
    let cp = c as u32;
    table
        .binary_search_by(|&(a, b)| {
            if cp < a {
                std::cmp::Ordering::Greater
            } else if cp > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `re` `\w`: letters, numbers and underscore.
pub fn is_word(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '_';
    }
    in_table(WORD, c)
}

/// `str.isalnum()` for one character (`\w` without the underscore).
pub fn is_alnum(c: char) -> bool {
    c != '_' && is_word(c)
}

/// `str.isspace()` / `re` `\s`.
pub fn is_space(c: char) -> bool {
    in_table(SPACE, c)
}

/// `str.isdigit()` for one character (decimal digits plus superscripts, circled digits, ...).
pub fn is_digit(c: char) -> bool {
    in_table(DIGIT, c)
}

/// `str.isupper()` for one character.
pub fn is_upper(c: char) -> bool {
    in_table(UPPER, c)
}

/// Value of a Unicode decimal digit (`re` `\d`), as `int()` reads it.
pub fn decimal_value(c: char) -> Option<u8> {
    if c.is_ascii_digit() {
        return Some(c as u8 - b'0');
    }
    let cp = c as u32;
    DECIMAL
        .iter()
        .find(|&&(a, b)| a <= cp && cp <= b)
        .map(|&(a, _)| ((cp - a) % 10) as u8)
}

/// `str.isdecimal()`: non-empty and every character a decimal digit.
pub fn is_decimal_str(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| decimal_value(c).is_some())
}

/// `str.isdigit()` for a string.
pub fn is_digit_str(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_digit)
}

/// `str.isupper()` for a string: at least one cased character and no lower-case one. Only used on ASCII letters
/// and German umlauts, where "cased" means alphabetic.
pub fn is_upper_str(s: &str) -> bool {
    let mut cased = false;
    for c in s.chars() {
        if c.is_lowercase() {
            return false;
        }
        if is_upper(c) {
            cased = true;
        }
    }
    cased
}

pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

/// Digits of a string of decimal digits (`\d+`), leading zeros removed, as `int()` reads it.
pub fn digits_of(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.chars().filter_map(decimal_value).collect();
    let first = v
        .iter()
        .position(|&d| d != 0)
        .unwrap_or(v.len().saturating_sub(1));
    v.drain(..first);
    if v.is_empty() {
        v.push(0);
    }
    v
}

/// `int(s)` for a short digit string, saturating.
pub fn small_int(s: &str) -> u64 {
    s.chars().filter_map(decimal_value).fold(0u64, |acc, d| {
        acc.saturating_mul(10).saturating_add(d as u64)
    })
}

/// Byte offset of the code point `n` characters before `pos` (Python `s[pos - n:pos]`, clamped at 0).
pub fn back_chars(s: &str, pos: usize, n: usize) -> usize {
    let mut start = pos;
    for (i, _) in s[..pos].char_indices().rev().take(n) {
        start = i;
    }
    start
}

/// Byte offset of the code point `n` characters after `pos` (Python `s[pos:pos + n]`, clamped at the end).
pub fn fwd_chars(s: &str, pos: usize, n: usize) -> usize {
    match s[pos..].char_indices().nth(n) {
        Some((i, _)) => pos + i,
        None => s.len(),
    }
}

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// `str.capitalize()`: first character upper, the rest lower.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

/// Character-class body (without brackets) for a table, in regex syntax.
pub fn class_body(table: &[(u32, u32)]) -> String {
    let mut out = String::new();
    for &(a, b) in table {
        if a == b {
            out += &format!("\\x{{{a:X}}}");
        } else {
            out += &format!("\\x{{{a:X}}}-\\x{{{b:X}}}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert!(is_word('ä') && is_word('_') && is_word('²') && !is_word('\u{301}'));
        assert!(is_space('\u{1c}') && is_space('\u{a0}') && !is_space('x'));
        assert_eq!(decimal_value('٣'), Some(3));
        assert_eq!(decimal_value('\u{1D7D8}'), Some(0));
        assert!(is_digit('²') && decimal_value('²').is_none());
        assert_eq!(digits_of("007"), vec![7]);
        assert_eq!(digits_of("000"), vec![0]);
        assert_eq!(strip("\u{1c} a \u{3000}"), "a");
        assert_eq!(back_chars("abcä", 5, 2), 2);
        assert_eq!(fwd_chars("äbc", 0, 1), 2);
        assert!(is_upper_str("NICHT") && !is_upper_str("Nicht") && !is_upper_str("GROß"));
    }
}
