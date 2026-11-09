//! Typography folding for speech front-ends. The speech models we use tokenise at 4k pieces and
//! have no piece for `„` or `“`, so each reaches them as three `<0x..>` byte tokens and derails
//! the quoted span. Folding is opt-in and must never change normalisation output.

#[test]
fn folds_german_and_english_quotes() {
    assert_eq!(nrmlz::tts_safe("er sagte: „Los“"), "er sagte: \"Los\"");
    assert_eq!(nrmlz::tts_safe("sie sagte: “go”"), "sie sagte: \"go\"");
    assert_eq!(nrmlz::tts_safe("er sagte: ‘Los’"), "er sagte: 'Los'");
    assert_eq!(nrmlz::tts_safe("«Los» ‹go›"), "\"Los\" 'go'");
}

#[test]
fn folds_dashes_ellipsis_and_spaces() {
    assert_eq!(nrmlz::tts_safe("a – b — c"), "a - b - c");
    assert_eq!(nrmlz::tts_safe("warte…"), "warte...");
    assert_eq!(nrmlz::tts_safe("a\u{00a0}b"), "a b");
    assert_eq!(nrmlz::tts_safe("a\u{200b}b\u{feff}"), "ab");
}

#[test]
fn leaves_numbers_and_letters_alone() {
    for s in ["3,5 kg", "24,99 €", "Größe 42", "2025", "z. B. Straße"] {
        assert_eq!(nrmlz::tts_safe(s), s);
    }
}

#[test]
fn is_idempotent() {
    let once = nrmlz::tts_safe("„Los“ – ja…");
    assert_eq!(nrmlz::tts_safe(&once), once);
}

#[test]
fn output_contains_no_byte_fallback_prone_typography() {
    let folded = nrmlz::tts_safe("„Los“ ‚x‘ – — … NBSP\u{00a0}");
    for bad in [
        "\u{201e}", "\u{201c}", "\u{201d}", "\u{2018}", "\u{2019}", "\u{2013}", "\u{2014}",
        "\u{2026}", "\u{00a0}",
    ] {
        assert!(!folded.contains(bad), "left {bad:?} in {folded:?}");
    }
}
