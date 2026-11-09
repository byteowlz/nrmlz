//! Regression sentences for the Foxline bilingual Pocket TTS lexicons (lexicons/pocket-bilingual).

use nrmlz::lexicon::Lexicon;
use nrmlz::{normalize_styled, normalize_with_lexicon, Lang, Profile, SpeechStyle};

fn lexicon(lang: &str) -> Lexicon {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("lexicons/pocket-bilingual")
        .join(format!("{lang}.toml"));
    Lexicon::from_path(&path).unwrap()
}

#[test]
fn german_okay_is_respelled_okeh() {
    let lex = lexicon("de");
    for (input, expected) in [
        ("Okay?", "Okeh?"),
        ("OK, das ist okay.", "Okeh, das ist Okeh."),
        (
            "Okay, dann machen wir das so.",
            "Okeh, dann machen wir das so.",
        ),
        ("Das ist O.K.", "Das ist Okeh."),
        ("Das Token ist ok.", "Das Token ist Okeh."),
    ] {
        assert_eq!(
            normalize_with_lexicon(Lang::De, input, &lex),
            expected,
            "{input}"
        );
    }
}

#[test]
fn english_okay_is_spelled_o_k() {
    let lex = lexicon("en");
    for (input, expected) in [
        ("Okay?", "O K?"),
        ("OK, let's do it that way.", "O K, let's do it that way."),
        ("That is O.K.", "That is O K."),
        ("That's O.K. Let's go.", "That's O K. Let's go."),
        ("It's o.k. with me.", "It's O K with me."),
        ("The token is okayish.", "The token is okayish."),
    ] {
        assert_eq!(
            normalize_with_lexicon(Lang::En, input, &lex),
            expected,
            "{input}"
        );
    }
}

const POCKET: SpeechStyle = SpeechStyle {
    phone_chunks: false,
    letter_names: true,
    code_digits: true,
};

#[test]
fn full_pocket_pipeline_matches_tuner_winners() {
    let en = lexicon("en");
    assert_eq!(
        normalize_styled(Lang::En, "Your flight UA 482 boards at gate B12 at 6:45 a.m. Please be there by 6:15.", Profile::NRMLZ, Some(&en), POCKET),
        "Your flight you ay four eighty-two boards at gate bee twelve at six forty-five ae emm. Please be there by six fifteen."
    );
    assert_eq!(
        normalize_styled(
            Lang::En,
            "The tracking code is QX-4471. OK?",
            Profile::NRMLZ,
            Some(&en),
            POCKET
        ),
        "The tracking code is cue ex four four seven one. O K?"
    );
    let de = lexicon("de");
    assert_eq!(
        normalize_styled(
            Lang::De,
            "Die Sendungsnummer ist QX-4471. Okay?",
            Profile::NRMLZ,
            Some(&de),
            POCKET
        ),
        "Die Sendungsnummer ist kuh iks, vierundvierzig einundsiebzig. Okeh?"
    );
}
