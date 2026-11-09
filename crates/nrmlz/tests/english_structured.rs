//! Independently authored context and identity properties, not benchmark-derived fixtures.
use nrmlz::{normalize, Lang, Lexicon, Profile};
use proptest::prelude::*;

mod common;

#[test]
fn structured_readings_survive_prose_context() {
    common::assert_readings_in_context(
        Lang::En,
        &[
            ("QX704C", "Q X seven hundred four C"),
            ("v8.02", "version eight point zero two"),
            ("⅝", "five eighths"),
            (
                "ISBN 9781402894626",
                "I S B N nine seven eight one four zero two eight nine four six two six",
            ),
        ],
        &[("", ""), ("Read ", "."), ("I noted (", ") yesterday.")],
    );
}

#[test]
fn english_spans_and_lexicons_keep_their_boundaries() {
    assert_eq!(
        normalize(Lang::De, "Hier: [en]PIN 0724 and v8.02[/en] fertig."),
        "Hier: [en]PIN zero seven two four and version eight point zero two[/en] fertig."
    );
    let mut lex = Lexicon::new();
    lex.insert("TOKEN", "PIN 0724, v8.02 and ½");
    assert_eq!(
        nrmlz::normalize_with(Lang::En, "TOKEN", Profile::NRMLZ, Some(&lex)),
        "PIN 0724, v8.02 and ½"
    );
    assert_eq!(
        nrmlz::normalize_with(Lang::De, "[en]TOKEN[/en]!", Profile::NRMLZ, Some(&lex)),
        "[en]PIN 0724, v8.02 and ½[/en]!"
    );
}

#[test]
fn ordinary_words_with_label_prefixes_are_not_ids() {
    for (input, expected) in [
        ("Pinterest2", "Pinterest two"),
        ("serializable4", "serializable four"),
        ("identity7", "identity seven"),
    ] {
        assert_eq!(normalize(Lang::En, input), expected);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn pin_preserves_every_digit(digits in "[0-9]{1,18}") {
        let expected = nrmlz::num::en::digits(&digits);
        for label in ["PIN ", "PIN:", "Your PIN is "] {
            let prefix = if label == "PIN:" { "PIN: " } else { label };
            prop_assert_eq!(normalize(Lang::En, &format!("{label}{digits}")), format!("{prefix}{expected}"));
        }
    }

    #[test]
    fn prefixed_version_never_drops_padding(a in "0[0-9]{1,4}", b in "0[0-9]{1,4}") {
        prop_assert_eq!(normalize(Lang::En, &format!("v{a}.{b}")), format!("version {} point {}", nrmlz::num::en::digits(&a), nrmlz::num::en::digits(&b)));
    }

    #[test]
    fn contextual_fraction_is_a_value_not_a_date(n in 1u64..30, d in 3u64..25) {
        let denominator = if d == 4 {
            if n == 1 { "quarter".to_owned() } else { "quarters".to_owned() }
        } else {
            format!("{}{}", nrmlz::num::en::ordinal(d), if n == 1 { "" } else { "s" })
        };
        prop_assert_eq!(normalize(Lang::En, &format!("The fraction is {n}/{d}.")), format!("The fraction is {} {denominator}.", nrmlz::num::en::cardinal(n)));
    }
}
