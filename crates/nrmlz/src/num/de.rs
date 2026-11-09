//! German number words, written to give the same strings as `num2words(n, lang="de")` 0.5.14 (cardinal,
//! ordinal, year), which the Articulino frontend uses. This is an independent implementation of the observed
//! output (checked exhaustively against the Python library, see tests and `just difftest`), not a translation
//! of num2words' code.
//!
//! Numbers are decimal digit strings, so any size works up to num2words' limit of 10^606.

use crate::Error;

const ONES: [&str; 20] = [
    "null",
    "eins",
    "zwei",
    "drei",
    "vier",
    "fünf",
    "sechs",
    "sieben",
    "acht",
    "neun",
    "zehn",
    "elf",
    "zwölf",
    "dreizehn",
    "vierzehn",
    "fünfzehn",
    "sechzehn",
    "siebzehn",
    "achtzehn",
    "neunzehn",
];
const TENS: [&str; 10] = [
    "", "", "zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig",
];

/// Largest group index: groups of three digits, 10^(3*k). Index 2 is Million, 3 Milliarde, ... up to 10^603.
const MAX_GROUP: usize = 201;

/// Scale word for 10^(3*k), k >= 2: Million, Milliarde, Billion, Billiarde, ..., then the Latin names in lower
/// case as num2words writes them (dezillion, ..., zentilliarde).
fn scale_name(k: usize) -> String {
    // Stems from 10^6 upwards, each giving "-illion" (10^(6j)) and "-illiarde" (10^(6j+3)).
    const LOWS: [&str; 9] = [
        "M", "B", "Tr", "Quadr", "Quint", "Sext", "Sept", "Okt", "Non",
    ];
    const UNITS: [&str; 10] = [
        "", "un", "duo", "tre", "quattuor", "quin", "sex", "sept", "okto", "novem",
    ];
    const TENS_L: [&str; 9] = [
        "dez",
        "vigint",
        "trigint",
        "quadragint",
        "quinquagint",
        "sexagint",
        "septuagint",
        "oktogint",
        "nonagint",
    ];
    let j = (k - 2) / 2; // stem index
    let stem = if j < 9 {
        LOWS[j].to_string()
    } else if j < 99 {
        let n = j - 9; // 0 -> dez, 1 -> undez, ...
        format!("{}{}", UNITS[n % 10], TENS_L[n / 10])
    } else {
        "zent".to_string()
    };
    if (k - 2).is_multiple_of(2) {
        stem + "illion"
    } else {
        stem + "illiarde"
    }
}

fn below_hundred(n: usize) -> String {
    if n < 20 {
        return ONES[n].to_string();
    }
    let (t, u) = (n / 10, n % 10);
    match u {
        0 => TENS[t].to_string(),
        1 => format!("einund{}", TENS[t]),
        _ => format!("{}und{}", ONES[u], TENS[t]),
    }
}

fn below_thousand(n: usize) -> String {
    let (h, r) = (n / 100, n % 100);
    let mut s = String::new();
    if h > 0 {
        s += if h == 1 { "ein" } else { ONES[h] };
        s += "hundert";
    }
    if r > 0 || h == 0 {
        s += &below_hundred(r);
    }
    s
}

/// 1 .. 999_999, written as one word.
fn below_million(n: usize) -> String {
    let (t, u) = (n / 1000, n % 1000);
    let mut s = String::new();
    if t > 0 {
        s += &if t == 1 {
            "ein".to_string()
        } else {
            below_thousand(t)
        };
        s += "tausend";
    }
    if u > 0 {
        s += &below_thousand(u);
    }
    s
}

/// Cardinal for a number given as decimal digits (most significant first, no sign).
pub fn cardinal_digits(digits: &[u8]) -> Result<String, Error> {
    let start = digits.iter().position(|&d| d != 0).unwrap_or(digits.len());
    let digits = &digits[start..];
    if digits.is_empty() {
        return Ok("null".to_string());
    }
    // groups of three from the right
    let mut groups: Vec<usize> = Vec::new();
    let mut end = digits.len();
    while end > 0 {
        let begin = end.saturating_sub(3);
        groups.push(
            digits[begin..end]
                .iter()
                .fold(0, |acc, &d| acc * 10 + d as usize),
        );
        end = begin;
    }
    if groups.len() > MAX_GROUP + 1 {
        return Err(Error::NumberTooLarge);
    }
    let mut words: Vec<String> = Vec::new();
    for k in (2..groups.len()).rev() {
        let c = groups[k];
        if c == 0 {
            continue;
        }
        let name = scale_name(k);
        if c == 1 {
            words.push(format!("eine {name}"));
        } else {
            let plural = if name.ends_with('e') {
                format!("{name}n")
            } else {
                format!("{name}en")
            };
            words.push(format!("{} {plural}", below_thousand(c)));
        }
    }
    let low = groups[0] + groups.get(1).copied().unwrap_or(0) * 1000;
    if low > 0 {
        words.push(below_million(low));
    }
    Ok(words.join(" "))
}

pub fn cardinal(n: u64) -> String {
    let digits: Vec<u8> = n.to_string().bytes().map(|b| b - b'0').collect();
    cardinal_digits(&digits).expect("u64 is far below the limit")
}

/// Ordinal ("dritte", "einundzwanzigste", "millionste") as num2words writes it.
pub fn ordinal_digits(digits: &[u8]) -> Result<String, Error> {
    let mut word = cardinal_digits(digits)?.to_lowercase();
    const ENDINGS: [(&str, &str); 11] = [
        ("eins", "ers"),
        ("drei", "drit"),
        ("acht", "ach"),
        ("sieben", "sieb"),
        ("ig", "igs"),
        ("ert", "erts"),
        ("end", "ends"),
        ("ion", "ions"),
        ("nen", "ns"),
        ("rde", "rds"),
        ("rden", "rds"),
    ];
    for (from, to) in ENDINGS {
        if let Some(stem) = word.strip_suffix(from) {
            word = format!("{stem}{to}");
            break;
        }
    }
    word += "te";
    if word == "eintausendste" || word == "einhundertste" {
        word = word.replacen("ein", "", 1);
    }
    // num2words then applies two rules to the last word if it is a scale ordinal ("millionste"): "eine " before
    // it goes ("eine millionste" -> "millionste"), then the space before it ("zwei millionste" -> "zweimillionste").
    let is_scale = |w: &str| {
        w.bytes().all(|b| b.is_ascii_lowercase())
            && ["illionste", "illiardste"]
                .iter()
                .any(|s| w.len() > s.len() && w.ends_with(s))
    };
    if let Some((head, last)) = word.rsplit_once(' ') {
        if is_scale(last) {
            word = match head.strip_suffix("eine") {
                Some(h) => format!("{}{last}", h.strip_suffix(' ').unwrap_or(h)),
                None => format!("{head}{last}"),
            };
        }
    }
    Ok(word)
}

pub fn ordinal(n: u64) -> String {
    let digits: Vec<u8> = n.to_string().bytes().map(|b| b - b'0').collect();
    ordinal_digits(&digits).expect("u64 is far below the limit")
}

/// `num2words(n, lang="de", to="year")` for the years the frontend reads that way (1100-1999):
/// "neunzehnhundertneunzig".
pub fn year(n: u64) -> String {
    if (n / 100).is_multiple_of(10) {
        return cardinal(n);
    }
    let (high, low) = (n / 100, n % 100);
    let mut s = cardinal(high);
    s += "hundert";
    if low > 0 {
        s += &cardinal(low);
    }
    s.replace(' ', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Vec<u8> {
        s.bytes().map(|b| b - b'0').collect()
    }

    #[test]
    fn known_values() {
        let cases = [
            (0, "null", "nullte"),
            (1, "eins", "erste"),
            (7, "sieben", "siebte"),
            (21, "einundzwanzig", "einundzwanzigste"),
            (100, "einhundert", "hundertste"),
            (101, "einhunderteins", "einhunderterste"),
            (1000, "eintausend", "tausendste"),
            (1100, "eintausendeinhundert", "eintausendeinhundertste"),
            (101000, "einhunderteinstausend", "einhunderteinstausendste"),
            (1000000, "eine Million", "millionste"),
            (1000001, "eine Million eins", "eine million erste"),
            (2000000, "zwei Millionen", "zweimillionste"),
            (
                101000000,
                "einhunderteins Millionen",
                "einhunderteinsmillionste",
            ),
            (
                1001000000,
                "eine Milliarde eine Million",
                "eine milliardemillionste",
            ),
            (
                2001000000,
                "zwei Milliarden eine Million",
                "zwei milliardenmillionste",
            ),
            (
                1000001000,
                "eine Milliarde eintausend",
                "eine milliarde eintausendste",
            ),
            (
                2000000000000003,
                "zwei Billiarden drei",
                "zwei billiarden dritte",
            ),
        ];
        for (n, c, o) in cases {
            assert_eq!(cardinal(n), c, "{n}");
            assert_eq!(ordinal(n), o, "{n}");
        }
        assert_eq!(year(1990), "neunzehnhundertneunzig");
        assert_eq!(year(1100), "elfhundert");
        assert_eq!(year(1901), "neunzehnhunderteins");
        let mut e60 = vec![1];
        e60.extend(std::iter::repeat_n(0, 60));
        assert_eq!(cardinal_digits(&e60).unwrap(), "eine dezillion");
        let mut e605 = vec![9];
        e605.extend(std::iter::repeat_n(0, 605));
        assert_eq!(cardinal_digits(&e605).unwrap(), "neunhundert zentilliarden");
        assert_eq!(ordinal_digits(&e605).unwrap(), "neunhundertzentilliardste");
        let mut e606 = vec![1];
        e606.extend(std::iter::repeat_n(0, 606));
        assert!(cardinal_digits(&e606).is_err());
        assert_eq!(cardinal_digits(&d("0007")).unwrap(), "sieben");
    }
}
