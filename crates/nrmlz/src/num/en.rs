//! English number words, US style: "one hundred twenty-three" (no "and"), short scale (million, billion),
//! hyphenated tens. Independently implemented; no reference corpus is embedded.

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 12] = [
    "",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion",
    "sextillion",
    "septillion",
    "octillion",
    "nonillion",
    "decillion",
];

fn below_hundred(n: usize) -> String {
    if n < 20 {
        return ONES[n].to_string();
    }
    let (t, u) = (n / 10, n % 10);
    if u == 0 {
        TENS[t].to_string()
    } else {
        format!("{}-{}", TENS[t], ONES[u])
    }
}

fn below_thousand(n: usize) -> String {
    let (h, r) = (n / 100, n % 100);
    match (h, r) {
        (0, r) => below_hundred(r),
        (h, 0) => format!("{} hundred", ONES[h]),
        (h, r) => format!("{} hundred {}", ONES[h], below_hundred(r)),
    }
}

/// Cardinal for decimal digits (most significant first). Numbers of 10^36 or more are read digit by digit.
pub fn cardinal_digits(digits: &[u8]) -> String {
    let start = digits.iter().position(|&d| d != 0).unwrap_or(digits.len());
    let digits = &digits[start..];
    if digits.is_empty() {
        return "zero".into();
    }
    if digits.len() > 3 * SCALES.len() {
        return digits
            .iter()
            .map(|&d| ONES[d as usize])
            .collect::<Vec<_>>()
            .join(" ");
    }
    let mut groups: Vec<usize> = Vec::new();
    let mut end = digits.len();
    while end > 0 {
        let begin = end.saturating_sub(3);
        groups.push(
            digits[begin..end]
                .iter()
                .fold(0, |a, &d| a * 10 + d as usize),
        );
        end = begin;
    }
    let mut words = Vec::new();
    for (k, &g) in groups.iter().enumerate().rev() {
        if g == 0 {
            continue;
        }
        words.push(below_thousand(g));
        if k > 0 {
            words.push(SCALES[k].to_string());
        }
    }
    words.join(" ")
}

pub fn cardinal(n: u64) -> String {
    let d: Vec<u8> = n.to_string().bytes().map(|b| b - b'0').collect();
    cardinal_digits(&d)
}

/// Ordinal of the last word: "twenty-one" -> "twenty-first".
pub fn ordinalize(cardinal: &str) -> String {
    let (head, last) = match cardinal.rfind([' ', '-']) {
        Some(i) => (&cardinal[..=i], &cardinal[i + 1..]),
        None => ("", cardinal),
    };
    let last = match last {
        "one" => "first".to_string(),
        "two" => "second".to_string(),
        "three" => "third".to_string(),
        "five" => "fifth".to_string(),
        "eight" => "eighth".to_string(),
        "nine" => "ninth".to_string(),
        "twelve" => "twelfth".to_string(),
        w if w.ends_with('y') => format!("{}ieth", &w[..w.len() - 1]),
        w => format!("{w}th"),
    };
    format!("{head}{last}")
}

pub fn ordinal_digits(digits: &[u8]) -> String {
    ordinalize(&cardinal_digits(digits))
}

pub fn ordinal(n: u64) -> String {
    ordinalize(&cardinal(n))
}

/// Years as spoken: 1990 "nineteen ninety", 1905 "nineteen oh five", 1900 "nineteen hundred", 2000 "two
/// thousand", 2005 "two thousand five", 2010 "twenty ten", 1066 "ten sixty-six".
pub fn year(n: u64) -> String {
    if !(1000..10000).contains(&n) {
        return cardinal(n);
    }
    let (hi, lo) = (n / 100, n % 100);
    if n % 1000 < 10 && hi.is_multiple_of(10) {
        // 2000-2009, 1000-1009, 3000...: "two thousand five"
        return cardinal(n);
    }
    match lo {
        0 => format!("{} hundred", cardinal(hi)),
        1..=9 => format!("{} oh {}", cardinal(hi), ONES[lo as usize]),
        _ => format!("{} {}", cardinal(hi), below_hundred(lo as usize)),
    }
}

/// Digits one by one ("zero" for 0).
pub fn digits(s: &str) -> String {
    s.chars()
        .filter_map(crate::pystr::decimal_value)
        .map(|d| ONES[d as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words() {
        assert_eq!(cardinal(0), "zero");
        assert_eq!(cardinal(21), "twenty-one");
        assert_eq!(cardinal(100), "one hundred");
        assert_eq!(cardinal(123), "one hundred twenty-three");
        assert_eq!(cardinal(1_000_001), "one million one");
        assert_eq!(cardinal(2_500_000_000), "two billion five hundred million");
        assert_eq!(ordinal(1), "first");
        assert_eq!(ordinal(22), "twenty-second");
        assert_eq!(ordinal(40), "fortieth");
        assert_eq!(ordinal(112), "one hundred twelfth");
        assert_eq!(ordinal(1000), "one thousandth");
        assert_eq!(year(1990), "nineteen ninety");
        assert_eq!(year(1905), "nineteen oh five");
        assert_eq!(year(1900), "nineteen hundred");
        assert_eq!(year(2000), "two thousand");
        assert_eq!(year(2005), "two thousand five");
        assert_eq!(year(2010), "twenty ten");
        assert_eq!(year(2026), "twenty twenty-six");
        assert_eq!(year(1066), "ten sixty-six");
        assert_eq!(year(1000), "one thousand");
    }
}
