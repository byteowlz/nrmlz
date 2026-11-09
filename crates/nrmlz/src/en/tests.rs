use super::*;
use proptest::prelude::*;
use std::collections::BTreeMap;

#[test]
fn handwritten_gold() {
    let mut count = 0;
    let mut classes = BTreeMap::new();
    let mut failures = Vec::new();
    for (line, s) in include_str!("../../../../tests/gold/en.tsv")
        .lines()
        .enumerate()
    {
        if s.starts_with('#') || s.trim().is_empty() {
            continue;
        }
        let cols: Vec<_> = s.split('\t').collect();
        assert_eq!(cols.len(), 4, "gold line {}", line + 1);
        assert!(
            !cols[3].trim().is_empty(),
            "missing rationale at {}",
            line + 1
        );
        let got = try_normalize(cols[1]).unwrap();
        if got != cols[2] {
            failures.push(format!(
                "{} [{}] {:?}\n expected {:?}\n      got {:?}",
                line + 1,
                cols[0],
                cols[1],
                cols[2],
                got
            ));
        }
        *classes.entry(cols[0]).or_insert(0) += 1;
        count += 1;
    }
    assert!(
        count >= 270,
        "English gold cases must not silently disappear"
    );
    for class in [
        "cardinal",
        "ordinal",
        "decimal",
        "year",
        "money",
        "percent",
        "clock",
        "date",
        "unit",
        "phone",
        "abbreviation",
        "address",
        "range",
        "symbol",
        "markup",
        "unicode",
        "fraction",
        "identifier",
        "version",
        "product",
        "context",
        "clinical",
        "record",
        "network",
        "postal",
    ] {
        assert!(classes.contains_key(class), "missing class {class}");
    }
    eprintln!(
        "English hand-written gold: {count} cases, {} classes",
        classes.len()
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn time_marker_keeps_the_sentence_boundary() {
    assert_eq!(
        normalize("Be there at 6:45 a.m. Please hurry."),
        "Be there at six forty-five A M. Please hurry."
    );
    assert_eq!(
        normalize("Call at 5 p.m. Thanks!"),
        "Call at five P M. Thanks!"
    );
    assert_eq!(
        normalize("It opens at 9 a.m. Monday."),
        "It opens at nine A M Monday."
    );
    assert_eq!(
        normalize("It opens at 9 a.m. on Monday."),
        "It opens at nine A M on Monday."
    );
}

#[test]
fn huge_numbers_and_unicode_boundaries() {
    let huge = "9".repeat(300);
    let expected = vec!["nine"; 300].join(" ");
    assert_eq!(try_normalize(&huge).unwrap(), expected);
    assert_eq!(
        try_normalize("é5 猫9 A12 5猫").unwrap(),
        "é5 猫9 A twelve 5猫"
    );
    assert_eq!(
        try_normalize("\u{e000}5\u{e001}").unwrap(),
        "\u{e000}five\u{e001}"
    );
    assert_eq!(
        clean_markup("&#xD800; &#999999999999; &unknown;").unwrap(),
        "&#xD800; &#999999999999; &unknown;"
    );
}

#[test]
fn preserved_content_and_public_stages() {
    assert_eq!(
        numbers("1.8 million and 3:30 PM").unwrap(),
        "one point eight million and three thirty P M"
    );
    assert_eq!(
        money("$1.5 and ¥50").unwrap(),
        "one dollar and fifty cents and fifty yen"
    );
    assert_eq!(units("2 kg/s").unwrap(), "two kilograms per second");
    assert_eq!(
        spell_letters("PDF NASA iPhone").unwrap(),
        "P D F NASA iPhone"
    );
    assert_eq!(
        abbreviations("Dr. Smith & Co.").unwrap(),
        "Doctor Smith and Company."
    );
    assert_eq!(
        clean_markup("**a** [b](https://example.org)").unwrap(),
        "a b"
    );
    warm_up();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn arbitrary_unicode_never_panics(chars in prop::collection::vec(any::<char>(), 0..200)) {
        let input: String = chars.into_iter().collect();
        let result = try_normalize(&input);
        prop_assert!(result.is_ok(), "normalization failed for {:?}: {:?}", input, result);
        prop_assert_eq!(normalize(&input), result.unwrap());
    }

    #[test]
    fn bare_cardinals_match_speller(n in 0u64..u64::MAX) {
        let expected = words::cardinal(n);
        prop_assert_eq!(try_normalize(&n.to_string()).unwrap(), expected.clone());
        prop_assert_eq!(try_normalize(&expected).unwrap(), expected);
    }

    #[test]
    fn fractional_zeros_and_signed_decimal_digits(n in 0u32..1_000_000, fraction in "[0-9]{1,12}") {
        let input = format!("-{n}.{fraction}");
        prop_assert_eq!(try_normalize(&input).unwrap(), format!("minus {} point {}", words::cardinal(n.into()), words::digits(&fraction)));
    }

    #[test]
    fn unicode_decimal_equivalence(n in 0u64..u64::MAX) {
        let ascii = n.to_string();
        let fullwidth: String = ascii.chars().map(|c| char::from_u32(0xff10 + c as u32 - '0' as u32).unwrap()).collect();
        prop_assert_eq!(try_normalize(&fullwidth).unwrap(), try_normalize(&ascii).unwrap());
    }

    #[test]
    fn every_clock_minute(h in 0u64..24, m in 0u64..60) {
        let got = try_normalize(&format!("{h:02}:{m:02}")).unwrap();
        let expected = match m {
            0 => format!("{} o'clock", words::cardinal(h)),
            1..=9 => format!("{} oh {}", words::cardinal(h), words::cardinal(m)),
            _ => format!("{} {}", words::cardinal(h), words::cardinal(m)),
        };
        prop_assert_eq!(got, expected);
    }

    #[test]
    fn money_tenths_equal_padded_hundredths(whole in 0u32..1_000_000, tenth in 0u8..10) {
        let a = format!("${whole}.{tenth}");
        let b = format!("${whole}.{tenth}0");
        prop_assert_eq!(try_normalize(&a).unwrap(), try_normalize(&b).unwrap());
    }

    #[test]
    fn unit_rate_denominators_are_singular(n in 2u64..100_000) {
        prop_assert_eq!(try_normalize(&format!("{n} kg/s")).unwrap(), format!("{} kilograms per second", words::cardinal(n)));
    }
}
