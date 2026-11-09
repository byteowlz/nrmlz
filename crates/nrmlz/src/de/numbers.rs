//! Phone numbers, postcodes and numbers: dates, times, money, fractions, ordinals, decimals, plain numbers.
//! Port of the "numbers" section of german.py.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::num::de as words;
use crate::pyre::{re, LazyRe, Match, R};
use crate::pystr::{back_chars, char_len, digits_of, is_digit, is_digit_str, small_int};
use crate::reading::{self, NumberReading};
use crate::Lang;

use super::{Fix, Profile, MONTHS};

// ------------------------------------------------------------------------------------------- number words

/// `_cardinal(int(s))` for a string of decimal digits.
pub(crate) fn cardinal_str(s: &str) -> R<String> {
    words::cardinal_digits(&digits_of(s))
}

pub(crate) fn cardinal(n: u64) -> String {
    words::cardinal(n)
}

/// `num2words(n, to="ordinal") + ending`
pub(crate) fn ordinal_str(s: &str, ending: &str) -> R<String> {
    Ok(words::ordinal_digits(&digits_of(s))? + ending)
}

/// `" ".join(_cardinal(int(c)) for c in s if c.isdigit())`
pub(crate) fn digits(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for c in s.chars().filter(|&c| is_digit(c)) {
        let mut buf = [0u8; 4];
        out.push(cardinal_str(c.encode_utf8(&mut buf)).unwrap_or_default());
    }
    out.join(" ")
}

/// `_integer`: identifiers digit by digit, years 1100-1999 as years, the rest as cardinals.
pub(crate) fn integer(s: &str) -> R<String> {
    let len = char_len(s);
    if (len > 1 && s.starts_with('0')) || len > 9 {
        return Ok(s
            .chars()
            .map(|c| cardinal_str(&c.to_string()))
            .collect::<R<Vec<_>>>()?
            .join(" "));
    }
    let n = small_int(s);
    if len == 4 && (1100..=1999).contains(&n) {
        return Ok(words::year(n));
    }
    cardinal_str(s)
}

// ------------------------------------------------------------------------------------------- tables

pub(crate) static ORDINAL_TEN: &[&str] = &[
    "am", "im", "vom", "zum", "zur", "beim", "dem", "den", "des", "einem", "einen", "einer",
    "jedem", "jeden",
];
pub(crate) static ORDINAL_TE: &[&str] = &[
    "der", "die", "das", "ins", "ein", "eine", "jede", "jeder", "jedes",
];
static DATE_PREPOSITIONS: &[&str] = &["bis", "ab", "seit", "vor", "nach"];
const WEEKDAYS: &str = "Montag|Dienstag|Mittwoch|Donnerstag|Freitag|Samstag|Sonntag";

static MONTH_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| MONTHS.split('|').collect());
static MONTHS_LOWER: LazyLock<Vec<String>> =
    LazyLock::new(|| MONTHS.split('|').map(|m| m.to_lowercase()).collect());

/// Written-out ordinals 1-31 with their ending, for enumerations: "dritten" -> "n".
static ORDINAL_WORDS: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for n in 1..32 {
        for e in ["", "n", "r", "m"] {
            m.insert(words::ordinal(n) + e, e);
        }
    }
    m
});

fn ordinal_alternation() -> String {
    let mut all: Vec<&str> = ORDINAL_TEN.iter().chain(ORDINAL_TE).copied().collect();
    // Python sorts a set by length; equal lengths never compete for the same text, so their order is free.
    all.sort_by_key(|w| std::cmp::Reverse(w.len()));
    all.dedup();
    all.join("|")
}

// ------------------------------------------------------------------------------------------- patterns

static ORDINAL: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::with_flags(
        &format!(
            r"\b({})\s+(\d{{1,3}})\.(?=[\s,;:!?)\]]|$)",
            ordinal_alternation()
        ),
        "i",
    )
});
static DATE: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::new(&format!(
        r"\b(?:(\d{{1,2}})\.\s*(bis|und|oder|[–-])\s*)?(\d{{1,2}})\.\s*({MONTHS})\b"
    ))
});
static NUMERIC_DATE: LazyRe =
    re!(r"(?<![\d.,])(\d{1,2})\.(\d{1,2})\.(\d{4}|\d{2})?(?!\d)"; needs &["."]);
static DATE_CUE: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::with_flags(
        &format!(
            r"\b(?:am|vom|zum|bis|ab|seit|den|dem|der|des|vor|nach|zwischen|{WEEKDAYS}|Mo|Di|Mi|Do|Fr|Sa|So)\.?,?\s*$"
        ),
        "i",
    )
});
static DATE_LIST: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::new(&format!(
        r"(?:\b(?:{MONTHS})\b|\d{{1,2}}\.\d{{1,2}}\.)[^.!?\d]{{0,40}}(?:,|\bund|\boder|\bsowie|\bbis|[–-])\s*$"
    ))
});
static LAST_DATE: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::with_flags(
        &format!(r"\b(\d{{1,2}})\.\s*(?:{MONTHS})\b(?!.*\b\d{{1,2}}\.\s*(?:{MONTHS})\b)"),
        "s",
    )
});
// Fix::Dates: the weekday may still be abbreviated here ("am Fr., 3. Mai"), abbreviations come later
static APPOSITION_ABBR: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::with_flags(
        &format!(r"\b(?:am|vom|zum|seit|bis|ab)\s+(?:{WEEKDAYS}|Mo|Di|Mi|Do|Fr|Sa|So)\.?,\s*$"),
        "i",
    )
});
static APPOSITION: LazyRe = LazyLock::new(|| {
    crate::pyre::Re::with_flags(
        &format!(r"\b(?:am|vom|zum|seit|bis|ab)\s+(?:{WEEKDAYS}),\s*$"),
        "i",
    )
});
static WORDS: LazyRe = re!(r"[\wÀ-ɏ]+");
static CLOCK: LazyRe = re!(r"\b([01]?\d|2[0-4]):([0-5]\d)\b(?:\s*Uhr\b)?"; needs &[":"]);
static RATIO: LazyRe = re!(r"(?<![\w.,:])(\d+):(\d+)(?![\d:])"; needs &[":"]);
static MONEY: LazyRe =
    re!(r"\b(\d+),(\d{2})\s*(€|Euro|US-Dollar|Dollar|Pfund|Franken)\b"; needs &[","]);
static TIMES: LazyRe = re!(r"(?<=\d)\s?[x×]\s?(?=\d)"; needs &["x", "×"]);
static THOUSANDS: LazyRe = re!(r"(?<!\d\.)\b\d{1,3}(?:\.\d{3})+\b(?!\.\d)"; needs &["."]);
static DECIMAL: LazyRe = re!(r"\b(\d+),(\d+)\b"; needs &[","]);
static RANGE: LazyRe = re!(r"\b(\d+)\s?[–-]\s?(\d+)\b"; needs &["-", "–"]);
static MINUS: LazyRe = re!(r"(?<![\w])-(?=\d)"; needs &["-"]);
static GLUED: LazyRe = re!(r"(?<=[A-Za-zÄÖÜäöüß])(?=\d)|(?<=\d)(?=[A-Za-zÄÖÜäöüß])");
static INTEGER: LazyRe = re!(r"\b\d+\b");
// Fix::NoLeftovers: digits glued to any letter ("7µF"), not only to Latin ones
static GLUED_ANY: LazyRe = re!(r"(?<=[^\W\d_])(?=\d)|(?<=\d)(?=[^\W\d_])");
static PHONE: LazyRe = re!(r"(?<![\w.,+])(\+\d{1,3}|0\d{2,5})((?:[ \t]?[/-][ \t]?\d+|[ \t]\d{2,}){1,4})(?![\d.,]\d)"; needs &["+", "0"]);
static PHONE_INTL: LazyRe = re!(r"(?<![\w.,])\+(\d{6,15})\b"; needs &["+"]);
static POSTCODE: LazyRe = re!(concat!(
    r"(?:(?<=\bPLZ )|(?<=\bPLZ: )|(?<=\bPLZ ist )|(?<=\bPostleitzahl )|(?<=\bPostleitzahl ist )",
    r"|(?<=\bin )(?=\d{5}[ \t]+[A-ZÄÖÜ][a-zäöüß]))(\d{5})\b"
); needs &["PLZ", "Postleitzahl", "in "]);
static THOUSANDS_DECIMAL: LazyRe = re!(r"(?<![\d.])(\d{1,3}(?:\.\d{3})+)(?=,\d)"; needs &["."]);
static NUMBER_SIGN: LazyRe = re!(r"#[ \t]?(\d+)\b"; needs &["#"]);
static FRACTION: LazyRe = re!(r"(?<![\w.,/])(\d{1,2})/(\d{1,2})(?![\w/])"; needs &["/"]);
static DIGIT_RUN: LazyRe = re!(r"\d+");
static ONE_UHR: LazyRe = re!(r"(?<![\w.,])1(?=\s+Uhr\b)"; needs &["Uhr"]);
static ONE_MILLION: LazyRe = re!(r"(?<![\w.,])1(?=\s+(?:Million|Milliarde|Billion)\b)"; needs &["Million", "Milliarde", "Billion"]);
static VERSION_POINT: LazyRe = re!(r"(?<=\d)\.(?=\d)"; needs &["."]);

pub(crate) fn has_digit(text: &str) -> bool {
    text.chars().any(is_digit)
}

// ------------------------------------------------------------------------------------------- phone numbers

/// Phone numbers and postcodes digit by digit. First in the chain: math_symbols would make "+49" "plus 49".
pub fn phone_numbers(text: &str, _p: Profile) -> R<String> {
    if text.is_empty() || !has_digit(text) {
        return Ok(text.to_string());
    }
    let text = PHONE.sub_fn(text, phone)?;
    let text = PHONE_INTL.sub_fn(&text, |m| Ok(format!("plus {}", digits(m.g(1)))))?;
    POSTCODE.sub_fn(&text, |m| Ok(digits(m.g(1))))
}

fn phone(m: &Match) -> R<String> {
    let front = m.g(1);
    let mut out = vec![match front.strip_prefix('+') {
        Some(rest) => format!("plus {}", digits(rest)),
        None => digits(front),
    }];
    for t in DIGIT_RUN.find_iter(m.g(2))? {
        out.push(digits(t.all()));
    }
    Ok(out.join(", "))
}

// ------------------------------------------------------------------------------------------- dates

/// Ending of the ordinal in a date, read from the word before: "am 25. Juli" -> n, "der 25. Juli" -> "",
/// without article ("Berlin, 25. Juli 2000") -> r.
fn date_ending(before: &str, p: Profile) -> R<&'static str> {
    let tail = &before[back_chars(before, before.len(), 80)..];
    let lowered = tail.to_lowercase();
    let words: Vec<String> = WORDS
        .find_iter(&lowered)?
        .iter()
        .map(|m| m.all().to_string())
        .collect();
    let w = words.last().map(String::as_str).unwrap_or("");
    // apposition: "am Freitag, 25. Juli" - the case comes from "am"
    if APPOSITION.search(before)?.is_some()
        || (p.has(Fix::Dates) && APPOSITION_ABBR.search(before)?.is_some())
    {
        return Ok("n");
    }
    // "am dritten und 4. Mai", "der dritte bis 5. Mai": like the number before
    if matches!(w, "und" | "oder" | "bis") && words.len() > 1 {
        let v = &words[words.len() - 2];
        for (suffix, e) in [("ten", "n"), ("ter", "r"), ("te", "")] {
            if v.ends_with(suffix) && !is_digit_str(v) {
                return Ok(e);
            }
        }
    }
    // "der 14. November, 28. November und 5. Dezember": like the date before
    let is_month = |x: &str| MONTHS_LOWER.iter().any(|m| m == x);
    let month_idx: isize = if is_month(w) {
        -1
    } else if matches!(w, "und" | "oder" | "sowie" | "bis")
        && words.len() > 1
        && is_month(&words[words.len() - 2])
    {
        -2
    } else {
        0
    };
    if month_idx != 0 {
        // already written out ("vierzehnte November, 28. November") or still digits
        if words.len() as isize > -month_idx {
            let idx = words.len() as isize + month_idx - 1;
            if let Some(e) = ORDINAL_WORDS.get(&words[idx as usize]) {
                return Ok(e);
            }
        }
        if let Some(previous) = LAST_DATE.search(before)? {
            return date_ending(&before[..previous.start()], p);
        }
    }
    if w == "zwischen" {
        return Ok("m"); // zwischen achtundzwanzigstem November
    }
    // "am ersten, 2. und 3. Mai": like the ordinal already written out before
    if let Some(e) = ORDINAL_WORDS.get(w) {
        return Ok(e);
    }
    if ORDINAL_TEN.contains(&w) || DATE_PREPOSITIONS.contains(&w) {
        return Ok("n");
    }
    if ORDINAL_TE.contains(&w) {
        return Ok("");
    }
    Ok("r")
}

/// "am 3." -> "am dritten". At the end of the text the period was the sentence end as well.
fn ordinal_after_article(m: &Match) -> R<String> {
    let lower = m.g(1).to_lowercase();
    let ending = if ORDINAL_TEN.contains(&lower.as_str()) {
        "n"
    } else {
        ""
    };
    let dot = if m.end() == m.text.len() { "." } else { "" };
    Ok(format!("{} {}{dot}", m.g(1), ordinal_str(m.g(2), ending)?))
}

fn numeric_date(m: &Match) -> R<String> {
    let (day, month, year) = (small_int(m.g(1)), small_int(m.g(2)), m.get(3));
    if !((1..=31).contains(&day) && (1..=12).contains(&month)) {
        return Ok(m.all().to_string());
    }
    // without a year only with a hint at a date: date word before, enumeration after a date, or the
    // two-digit form "28.11." ("Python 3.12." stays)
    let before = m.before();
    let two_digit = char_len(m.g(1)) == 2 && char_len(m.g(2)) == 2;
    if year.is_none()
        && !(two_digit || DATE_CUE.search(before)?.is_some() || DATE_LIST.search(before)?.is_some())
    {
        return Ok(m.all().to_string());
    }
    let mut s = format!("{day}. {}", MONTH_NAMES[(month - 1) as usize]);
    if let Some(y) = year {
        s += " ";
        s += y;
    }
    if year.is_none() && m.end() == m.text.len() {
        s += ".";
    }
    Ok(s)
}

fn date(m: &Match, p: Profile) -> R<String> {
    let e = date_ending(m.before(), p)?;
    let day = format!("{} {}", ordinal_str(m.g(3), e)?, m.g(4));
    if let Some(first) = m.get(1) {
        let between = if matches!(m.g(2), "–" | "-") {
            "bis"
        } else {
            m.g(2)
        };
        return Ok(format!("{} {between} {day}", ordinal_str(first, e)?));
    }
    Ok(day)
}

// ------------------------------------------------------------------------------------------- fractions

/// "3/4" -> "drei Viertel"; only if numerator < denominator <= 10 ("24/7" and "9/11" stay pairs of numbers).
fn fraction(m: &Match) -> R<String> {
    let (num, den) = (small_int(m.g(1)), small_int(m.g(2)));
    if !(0 < num && num < den && den <= 10) {
        return Ok(m.all().to_string());
    }
    let name = if den == 2 {
        "halb".to_string()
    } else {
        crate::pystr::capitalize(&words::ordinal(den)) + "l"
    };
    let count = if num == 1 {
        "ein".to_string()
    } else {
        cardinal(num)
    };
    Ok(format!("{count} {name}"))
}

// ------------------------------------------------------------------------------------------- numbers

/// Write out digits in German words. Order matters: first the forms with period, comma and colon, then the
/// bare integers.
pub fn numbers(text: &str, p: Profile) -> R<String> {
    if text.is_empty() || !has_digit(text) {
        return Ok(text.to_string());
    }
    let text = THOUSANDS_DECIMAL.sub_fn(text, |m| Ok(m.g(1).replace('.', "")))?;
    let text = NUMBER_SIGN.sub(&text, r"Nummer \1")?;
    let text = NUMERIC_DATE.sub_fn(&text, numeric_date)?;
    let text = ORDINAL.sub_fn(&text, ordinal_after_article)?;
    let text = DATE.sub_fn(&text, |m| date(m, p))?;
    let text = CLOCK.sub_fn(&text, |m| {
        let hours = if small_int(m.g(1)) == 1 {
            "ein".to_string()
        } else {
            cardinal_str(m.g(1))?
        };
        let minutes = if m.g(2) == "00" {
            String::new()
        } else {
            format!(" {}", cardinal_str(m.g(2))?)
        };
        Ok(format!("{hours} Uhr{minutes}"))
    })?;
    // what CLOCK did not take as a time is a ratio or a score
    let text = RATIO.sub(&text, r"\1 zu \2")?;
    let text = MONEY.sub_fn(&text, |m| {
        let amount = if m.g(1) == "1" {
            "ein".to_string()
        } else {
            cardinal_str(m.g(1))?
        };
        let currency = if m.g(3) == "€" { "Euro" } else { m.g(3) };
        let cents = if m.g(2) == "00" {
            String::new()
        } else {
            format!(" {}", cardinal_str(m.g(2))?)
        };
        Ok(format!("{amount} {currency}{cents}"))
    })?;
    let text = TIMES.sub_lit(&text, " mal ")?; // 2 x 3 m
    let text = FRACTION.sub_fn(&text, fraction)?; // 3/4 -> drei Viertel
    let text = ONE_UHR.sub_lit(&text, "ein")?; // ein Uhr
    let text = ONE_MILLION.sub_lit(&text, "eine")?;
    let text = THOUSANDS.sub_fn(&text, |m| cardinal_str(&m.all().replace('.', "")))?;
    // "Komma," with a short pause: without it the Articulino model swallowed it before "fünf"/"vier"
    // (reference; Fix::PlainDecimal drops the comma, Profile::with_decimal_pause brings it back)
    let text = DECIMAL.sub_fn(&text, |m| {
        let decimals: Vec<String> = m
            .g(2)
            .chars()
            .map(|c| cardinal_str(&c.to_string()))
            .collect::<R<_>>()?;
        let comma = if p.has(Fix::PlainDecimal) {
            "Komma"
        } else {
            "Komma,"
        };
        Ok(format!(
            "{} {comma} {}",
            integer(m.g(1))?,
            decimals.join(" ")
        ))
    })?;
    // a period between digits now is a version, IP or decimal: "zwei Punkt fünf"
    let text = VERSION_POINT.sub_lit(&text, " Punkt ")?;
    let text = RANGE.sub(&text, r"\1 bis \2")?;
    let text = MINUS.sub_lit(&text, "minus ")?;
    let text = if p.has(Fix::NoLeftovers) {
        GLUED_ANY.sub_lit(&text, " ")?
    } else {
        GLUED.sub_lit(&text, " ")?
    };
    INTEGER.sub_fn(&text, |m| {
        let raw = m.all();
        if !p.has(Fix::NumberContext) {
            return integer(raw);
        }
        match reading::decide(Lang::De, m.before(), m.after(), false) {
            NumberReading::Digits => Ok(digits(raw)),
            NumberReading::Amount => cardinal_str(raw),
            NumberReading::Default => integer(raw),
        }
    })
}
