//! Exact decimal currency readings. Bare $ means USD and bare ¥ means JPY; qualified codes disambiguate.
use std::sync::LazyLock;

use super::{ascii_decimals, escape, integer, plain_number, words, Text, UNSIGNED};
use crate::pyre::{Match, Re, R};

struct Currency {
    aliases: &'static [&'static str],
    major: &'static str,
    plural: &'static str,
    // No commonly spoken minor unit for modern yen/won: read their decimal amounts as major units.
    minor: Option<(&'static str, &'static str)>,
}

// Independently authored currency names, not copied from NeMo or an evaluation corpus.
static CURRENCIES: &[Currency] = &[
    Currency {
        aliases: &["$", "US$", "USD"],
        major: "dollar",
        plural: "dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["€", "EUR"],
        major: "euro",
        plural: "euros",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["£", "GBP"],
        major: "pound",
        plural: "pounds",
        minor: Some(("penny", "pence")),
    },
    Currency {
        aliases: &["¥", "￥", "JP¥", "JPY"],
        major: "yen",
        plural: "yen",
        minor: None,
    },
    Currency {
        aliases: &["CN¥", "CNY", "RMB"],
        major: "yuan",
        plural: "yuan",
        minor: Some(("fen", "fen")),
    },
    Currency {
        aliases: &["₩", "KRW"],
        major: "won",
        plural: "won",
        minor: None,
    },
    Currency {
        aliases: &["C$", "CA$", "CAD"],
        major: "Canadian dollar",
        plural: "Canadian dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["A$", "AU$", "AUD"],
        major: "Australian dollar",
        plural: "Australian dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["NZ$", "NZD"],
        major: "New Zealand dollar",
        plural: "New Zealand dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["CHF"],
        major: "Swiss franc",
        plural: "Swiss francs",
        minor: Some(("centime", "centimes")),
    },
    Currency {
        aliases: &["₹", "INR"],
        major: "rupee",
        plural: "rupees",
        minor: Some(("paisa", "paise")),
    },
    Currency {
        aliases: &["₽", "RUB"],
        major: "ruble",
        plural: "rubles",
        minor: Some(("kopek", "kopeks")),
    },
    Currency {
        aliases: &["R$", "BRL"],
        major: "Brazilian real",
        plural: "Brazilian reals",
        minor: Some(("centavo", "centavos")),
    },
    Currency {
        aliases: &["MXN"],
        major: "Mexican peso",
        plural: "Mexican pesos",
        minor: Some(("centavo", "centavos")),
    },
    Currency {
        aliases: &["SEK"],
        major: "Swedish krona",
        plural: "Swedish kronor",
        minor: Some(("öre", "öre")),
    },
    Currency {
        aliases: &["NOK"],
        major: "Norwegian krone",
        plural: "Norwegian kroner",
        minor: Some(("øre", "øre")),
    },
    Currency {
        aliases: &["DKK"],
        major: "Danish krone",
        plural: "Danish kroner",
        minor: Some(("øre", "øre")),
    },
    Currency {
        aliases: &["PLN"],
        major: "Polish zloty",
        plural: "Polish zlotys",
        minor: Some(("grosz", "groszy")),
    },
    Currency {
        aliases: &["TRY"],
        major: "Turkish lira",
        plural: "Turkish lira",
        minor: Some(("kuruş", "kuruş")),
    },
    Currency {
        aliases: &["ZAR"],
        major: "rand",
        plural: "rand",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["S$", "SGD"],
        major: "Singapore dollar",
        plural: "Singapore dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["HK$", "HKD"],
        major: "Hong Kong dollar",
        plural: "Hong Kong dollars",
        minor: Some(("cent", "cents")),
    },
    Currency {
        aliases: &["AED"],
        major: "dirham",
        plural: "dirhams",
        minor: Some(("fils", "fils")),
    },
    Currency {
        aliases: &["SAR"],
        major: "riyal",
        plural: "riyals",
        minor: Some(("halala", "halalas")),
    },
];

fn alternatives() -> String {
    let mut keys: Vec<_> = CURRENCIES
        .iter()
        .flat_map(|c| c.aliases.iter().copied())
        .collect();
    keys.sort_by_key(|s| std::cmp::Reverse(s.len()));
    keys.into_iter().map(escape).collect::<Vec<_>>().join("|")
}

// Capture the complete amount before validating precision: never silently round $1.234 to $1.23.
static RANGE_PREFIX: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w$€£.,])([+-]?)({0})[ \t]*([+-]?(?:{UNSIGNED}))[ \t]*[-–—][ \t]*(?:({0})[ \t]*)?([+-]?(?:{UNSIGNED}))(?:[ \t]+(thousand|million|billion|trillion)\b)?(?!\w|,[0-9]|\.[0-9])",
        alternatives()
    ))
});
static RANGE_SUFFIX: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.,])([+-]?(?:{UNSIGNED}))[ \t]*[-–—][ \t]*([+-]?(?:{UNSIGNED}))(?:[ \t]+(thousand|million|billion|trillion))?[ \t]*({})(?!\w)",
        alternatives()
    ))
});
static PREFIX: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w$€£.,])([+-]?)({})[ \t]*([+-]?(?:{UNSIGNED}))(?:[ \t]+(thousand|million|billion|trillion)\b)?(?!\w|,[0-9]|\.[0-9])",
        alternatives()
    ))
});
static SUFFIX: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"(?<![\w.,])([+-]?(?:{UNSIGNED}))(?:[ \t]+(thousand|million|billion|trillion))?[ \t]*({})(?!\w)",
        alternatives()
    ))
});

fn amount(raw: &str, code: &str, scale: Option<&str>) -> Option<String> {
    let c = CURRENCIES.iter().find(|c| c.aliases.contains(&code))?;
    let (sign, raw) = if let Some(s) = raw.strip_prefix('-') {
        ("minus ", s)
    } else if let Some(s) = raw.strip_prefix('+') {
        ("plus ", s)
    } else {
        ("", raw)
    };
    if raw.starts_with(['-', '+']) {
        return None;
    }
    if let Some(scale) = scale {
        return Some(format!("{sign}{} {scale} {}", plain_number(raw), c.plural));
    }
    let raw = raw.replace(',', "");
    let (whole, frac) = raw.split_once('.').map_or((raw.as_str(), ""), |p| p);
    let one = whole.parse::<u64>().ok() == Some(1) && frac.chars().all(|c| c == '0');
    let major = if one { c.major } else { c.plural };
    let Some((minor, minor_plural)) = c.minor else {
        return Some(format!("{sign}{} {major}", plain_number(&raw)));
    };
    if frac.len() > 2 {
        return None;
    }
    let cents = match frac.len() {
        0 => 0,
        1 => frac.parse::<u64>().ok()? * 10,
        _ => frac.parse::<u64>().ok()?,
    };
    // Major-unit agreement depends on the integer part even when a nonzero minor part follows.
    let major = if whole.parse::<u64>().ok() == Some(1) {
        c.major
    } else {
        c.plural
    };
    let zero = whole.is_empty() || whole.chars().all(|c| c == '0');
    let mut out = if zero && cents > 0 {
        String::new()
    } else {
        format!("{} {major}", integer(whole))
    };
    if cents > 0 {
        if !out.is_empty() {
            out += " and ";
        }
        out += &format!(
            "{} {}",
            words::cardinal(cents),
            if cents == 1 { minor } else { minor_plural }
        );
    }
    Some(format!("{sign}{out}"))
}

fn prefix(m: &Match<'_>) -> String {
    let raw = format!("{}{}", m.g(1), m.g(3));
    amount(&raw, m.g(2), m.get(4)).unwrap_or_else(|| m.all().to_owned())
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&RANGE_PREFIX, |m| {
        let left = format!("{}{}", m.g(1), m.g(3));
        let a = amount(&left, m.g(2), m.get(6));
        let b = amount(m.g(5), m.get(4).unwrap_or_else(|| m.g(2)), m.get(6));
        a.zip(b)
            .map_or_else(|| m.all().to_owned(), |(a, b)| format!("{a} to {b}"))
    })?;
    t.apply(&RANGE_SUFFIX, |m| {
        let a = amount(m.g(1), m.g(4), m.get(3));
        let b = amount(m.g(2), m.g(4), m.get(3));
        a.zip(b)
            .map_or_else(|| m.all().to_owned(), |(a, b)| format!("{a} to {b}"))
    })?;
    t.apply(&PREFIX, prefix)?;
    t.apply(&SUFFIX, |m| {
        amount(m.g(1), m.g(3), m.get(2)).unwrap_or_else(|| m.all().to_owned())
    })
}

/// Expand supported monetary tokens without floating-point arithmetic.
pub fn money(text: &str) -> R<String> {
    let mut t = Text::new(&ascii_decimals(text));
    apply(&mut t)?;
    Ok(t.finish())
}
