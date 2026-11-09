//! Properties of the nrmlz profile (separate from frozen-reference identity). Context invariance is scoped
//! to unambiguous spans: a date's case, St./Fr. and time/score readings intentionally depend on context.

use nrmlz::{de, normalize_with, Lang, Lexicon, Profile};
use proptest::prelude::*;

mod common;

#[test]
fn bare_one_articles_and_kitchen_units() {
    for (input, expected) in [
        ("Sie hat 1 Bruder.", "Sie hat einen Bruder."),
        ("Sie hat 1 Schwester.", "Sie hat eine Schwester."),
        ("Das ist 1 Kind.", "Das ist ein Kind."),
        ("Mit 1 Bruder.", "Mit einem Bruder."),
        ("Für 1 Schwester.", "Für eine Schwester."),
        ("Gib mir 1 Glas.", "Gib mir ein Glas."),
        ("1 Prise Salz", "eine Prise Salz."),
        ("2 EL Mehl", "zwei Esslöffel Mehl."),
        ("1 TL Zucker", "ein Teelöffel Zucker."),
        ("3 Prisen Salz", "drei Prisen Salz."),
        (
            "Das Rezept braucht 1 Prise Salz und 1 EL Öl.",
            "Das Rezept braucht eine Prise Salz und einen Esslöffel Öl.",
        ),
        (
            "1 Prise Salz und 1 EL Öl.",
            "eine Prise Salz und ein Esslöffel Öl.",
        ),
    ] {
        assert_eq!(de::normalize(input), expected, "{input}");
    }
    // The reference profile keeps its upstream readings.
    assert_eq!(
        normalize_with(Lang::De, "Sie hat 1 Bruder.", Profile::REFERENCE, None),
        "Sie hat eins Bruder."
    );
    assert_eq!(
        normalize_with(Lang::De, "1 Prise Salz.", Profile::REFERENCE, None),
        "eins Prise Salz."
    );
}

#[test]
fn dotted_clocks_and_letter_codes() {
    for (input, expected) in [
        ("13.15 Uhr", "dreizehn Uhr fünfzehn."),
        ("4.52 Uhr", "vier Uhr zweiundfünfzig."),
        ("13.00 Uhr", "dreizehn Uhr."),
        ("Flug ER 7651", "Flug eh err sieben sechs fünf eins."),
        ("Flug DA 1279", "Flug deh ah eins zwei sieben neun."),
        ("Flug ES 1205", "Flug eh ess eins zwei null fünf."),
        ("Gib B-40884 ein", "Gib beh vier null acht acht vier ein."),
        ("H-83379", "hah acht drei drei sieben neun."),
        ("XV-71066", "iks fau sieben eins null sechs sechs."),
    ] {
        assert_eq!(de::normalize(input), expected, "{input}");
    }
    // A period not followed by Uhr stays a decimal/version, and reference mode keeps the upstream reading.
    assert_eq!(de::normalize("13.15 Euro"), "dreizehn Punkt fünfzehn Euro.");
    assert_eq!(
        normalize_with(Lang::De, "13.15 Uhr", Profile::REFERENCE, None),
        "dreizehn Punkt fünfzehn Uhr."
    );
}

#[test]
fn owner_examples_and_opt_ins() {
    assert_eq!(
        de::normalize("Dichte in kg/m³."),
        "Dichte in Kilogramm pro Kubikmeter."
    );
    assert_eq!(de::normalize("Preis pro m²."), "Preis pro Quadratmeter.");
    assert_eq!(de::normalize("3,5 kg."), "drei Komma fünf Kilogramm.");
    assert_eq!(
        de::normalize_with("3,5 kg.", Profile::NRMLZ.with_decimal_pause(true)),
        "drei Komma, fünf Kilogramm."
    );
    assert_eq!(
        de::normalize("Homebrew, nginx und Okay."),
        "Homebrew, nginx und Okay."
    );
    assert_eq!(
        de::normalize_with("Homebrew und Okay.", Profile::NRMLZ.with_respellings(true)),
        "Hohmbruh und Okeh."
    );
    assert_eq!(
        de::normalize("1. Erstens das\n2. Zweitens jenes"),
        "Erstens das. Zweitens jenes."
    );
}

#[test]
fn lexicon_wins_even_when_respellings_are_enabled() {
    let mut lex = Lexicon::new();
    lex.insert("OK", "Okay")
        .insert("Cluster", "K8s")
        .insert("foo", "PDF 3,5");
    let p = Profile::NRMLZ.with_respellings(true);
    assert_eq!(
        normalize_with(Lang::De, "OK und Cluster und foo.", p, Some(&lex)),
        "Okay und K8s und PDF 3,5."
    );
    assert_eq!(
        normalize_with(Lang::De, "ꀀ, OK!", p, Some(&lex)),
        "ꀀ, Okay!"
    );
    // Only the reference profile intentionally preserves old lexicon precedence.
    assert_eq!(
        normalize_with(Lang::De, "OK.", Profile::REFERENCE, Some(&lex)),
        "Okeh."
    );
}

#[test]
fn explicit_english_spans_and_streaming() {
    let text = "Ich nehme 3 Bücher; [en]I need 5 books and $2.50.[/en] Danach 4 Euro.";
    let expected = "Ich nehme drei Bücher; [en]I need five books and two dollars and fifty cents.[/en] Danach vier Euro.";
    assert_eq!(de::normalize(text), expected);
    let mut stream = de::StreamNormalizer::new();
    let mut out = String::new();
    for c in text.chars() {
        out.push_str(&stream.push(&c.to_string()));
    }
    out.push_str(&stream.flush(""));
    assert_eq!(out.trim(), expected);
    assert_eq!(
        de::normalize("Ein [EN]12 cats[/EN] sind da."),
        "Ein [en]twelve cats[/en] sind da."
    );
    let mut lex = Lexicon::new();
    lex.insert("OK", "Okay 7");
    let out = normalize_with(
        Lang::De,
        "Hier: [en]OK and 3 books[/en].",
        Profile::NRMLZ,
        Some(&lex),
    );
    assert_eq!(out, "Hier: [en]Okay 7 and three books[/en].");
}

#[test]
fn unambiguous_spans_are_invariant_in_prose() {
    common::assert_readings_in_context(
        nrmlz::Lang::De,
        &[
            ("3 kg", "drei Kilogramm"),
            ("85 m²", "fünfundachtzig Quadratmeter"),
            ("3,5", "drei Komma fünf"),
            ("z. B.", "zum Beispiel"),
            ("PDF", "peh deh eff"),
            ("kg/m³", "Kilogramm pro Kubikmeter"),
            ("50 %", "fünfzig Prozent"),
        ],
        &[
            ("Etwa ", " genügen."),
            ("Das sind ", ", nicht mehr."),
            ("Hinweis: ", "."),
            ("Er sagt: (", ")."),
        ],
    );
}

#[test]
fn all_committed_inputs_leave_no_ascii_digits() {
    for raw in [
        include_str!("../../../tests/corpus/rules-de.jsonl"),
        include_str!("../../../tests/corpus/stress-de.jsonl"),
    ] {
        for line in raw.lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            let input = row["text"].as_str().unwrap();
            let out = de::try_normalize(input).unwrap();
            assert!(
                !out.chars().any(|c| c.is_ascii_digit()),
                "{}: digits survived",
                row["id"]
            );
        }
    }
}

#[test]
fn expansion_table_reaches_spoken_words() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../data/de/abbreviations.json")).unwrap();
    for key in data["abbreviations"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| k.ends_with('.'))
    {
        let out = de::normalize(&format!("Hinweis: {key} und mehr."));
        assert!(
            !out.contains(&format!(" {key} ")),
            "unexpanded table entry {key:?}: {out}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]
    #[test]
    fn quantity_reading_is_context_invariant(n in 2u64..1000) {
        let spoken = nrmlz::num::de::cardinal(n);
        for prefix in ["Etwa", "Genau", "Mit", "Hinweis:"] {
            prop_assert_eq!(de::normalize(&format!("{prefix} {n} kg.")), format!("{prefix} {spoken} Kilogramm."));
        }
    }
    #[test]
    fn no_digits_left_in_generated_quantities(n in 0u64..1_000_000_000, d in 0u64..10_000) {
        let out = de::normalize(&format!("Etwa {n},{d:04} µg, Version {n}.2.3, #{}.", d + 1));
        prop_assert!(!out.chars().any(|c| c.is_ascii_digit()), "{}", out);
    }
    #[test]
    fn arbitrary_lexicon_replacement_is_final(replacement in "[A-Za-z0-9 ,._%]{0,80}") {
        let mut lex = Lexicon::new();
        lex.insert("TERM", replacement.clone());
        let out = normalize_with(Lang::De, "Hier ist TERM!", Profile::NRMLZ, Some(&lex));
        prop_assert_eq!(out, format!("Hier ist {replacement}!"));
    }
}
