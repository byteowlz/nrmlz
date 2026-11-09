//! Context decides whether an unformatted integer is an identifier or an amount. Independently authored cases.
use nrmlz::{normalize, normalize_styled, Lang, Profile, SpeechStyle};

fn digits_en(s: &str) -> String {
    nrmlz::num::en::digits(s)
}

#[test]
fn english_context_selects_digit_reading() {
    let long = "46255577782";
    let spoken = digits_en(long);
    for input in [
        format!("My phone number is {long}."),
        format!("Call {long}."),
        format!("The order number is {long}."),
        format!("Reach me at {long}."),
        format!("Fax: {long}."),
        format!("Customer number {long}."),
    ] {
        assert!(normalize(Lang::En, &input).contains(&spoken), "{input}");
    }
    assert_eq!(
        normalize(Lang::En, "The code is 4625."),
        "The code is four six two five."
    );
    assert_eq!(
        normalize(Lang::En, "My PIN is 0074."),
        "My PIN is zero zero seven four."
    );
}

#[test]
fn english_amount_context_selects_cardinal_reading() {
    assert_eq!(
        normalize(Lang::En, "We sold 46255577782 units."),
        "We sold forty-six billion two hundred fifty-five million five hundred seventy-seven thousand seven hundred eighty-two units."
    );
    assert_eq!(
        normalize(Lang::En, "The budget is 46255577782 dollars."),
        "The budget is forty-six billion two hundred fifty-five million five hundred seventy-seven thousand seven hundred eighty-two dollars."
    );
    // A bare long run without a cue stays an amount; an explicit year cue stays a year.
    assert_eq!(
        normalize(Lang::En, "Reference 2500000000"),
        "Reference two billion five hundred million"
    );
    assert_eq!(
        normalize(Lang::En, "In 1996 we sold 42 units."),
        "In nineteen ninety-six we sold forty-two units."
    );
}

#[test]
fn german_context_selects_amount_reading() {
    let amount = "sechsundvierzig Milliarden zweihundertfünfundfünfzig Millionen fünfhundertsiebenundsiebzigtausendsiebenhundertzweiundachtzig";
    for unit in ["Stück", "Euro", "Prozent", "Einwohner"] {
        assert_eq!(
            normalize(Lang::De, &format!("Wir haben 46255577782 {unit} verkauft.")),
            format!("Wir haben {amount} {unit} verkauft.")
        );
    }
    assert_eq!(
        normalize(Lang::De, "46.255.577.782 Stück"),
        format!("{amount} Stück.")
    );
    assert_eq!(
        normalize(Lang::De, "Wir haben 46255577782 kg verkauft."),
        format!("Wir haben {amount} Kilogramm verkauft.")
    );
}

#[test]
fn german_context_selects_digit_reading() {
    let spoken = "vier sechs zwei fünf fünf fünf sieben sieben sieben acht zwei";
    for input in [
        "Telefonnummer 46255577782",
        "Meine Nummer ist 46255577782",
        "Kundennummer 46255577782",
        "PIN 0074",
    ] {
        assert!(
            normalize(Lang::De, input).contains(spoken)
                || normalize(Lang::De, input).contains("null null sieben vier"),
            "{input}"
        );
    }
    assert_eq!(
        normalize(Lang::De, "Telefonnummer 462555"),
        "Telefonnummer vier sechs zwei fünf fünf fünf."
    );
    assert_eq!(
        normalize(Lang::De, "In 1996 haben wir 42 Stück verkauft."),
        "In neunzehnhundertsechsundneunzig haben wir zweiundvierzig Stück verkauft."
    );
}

#[test]
fn phone_chunks_group_spaced_digits() {
    let style = SpeechStyle {
        phone_chunks: true,
        ..SpeechStyle::default()
    };
    let input = "4 6 2 5 5 5 7 7 7 8 2";
    assert_eq!(
        normalize_styled(Lang::De, input, Profile::NRMLZ, None, style),
        "vier sechs zwei. Fünf fünf fünf. Sieben sieben sieben. Acht zwei."
    );
    assert_eq!(
        normalize_styled(Lang::En, input, Profile::NRMLZ, None, style),
        "four six two. Five five five. Seven seven seven. Eight two"
    );
    // Default normalization still reads the digits one by one.
    assert_eq!(
        normalize(Lang::En, input),
        "four six two five five five seven seven seven eight two"
    );
}
