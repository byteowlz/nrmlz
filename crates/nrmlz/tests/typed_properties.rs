//! Independent identity/context checks for typed English, not benchmark-derived text.
use nrmlz::{normalize, Lang};
use proptest::prelude::*;

#[test]
fn technical_compression_zones_and_uuid_boundaries() {
    for (input, expected) in [
        ("Connect [2001:db8::42]:8042.", "Connect open bracket two zero zero one colon D B eight double colon four two close bracket port eight thousand forty-two."),
        ("IPv6 fe80::1%en0.", "I P version six F E eight zero double colon one percent E N zero."),
        ("MAC address 02:00:5E:10:00:07 and 3 devices.", "M A C address zero two colon zero zero colon five E colon one zero colon zero zero colon zero seven and three devices."),
        ("IPv6 2001:db8::/129.", "I P version six 2001:db8::/129."),
        ("IPv4 192.0.2.0/33.", "I P version four 192.0.2.0/33."),
        ("The MAC is fast.", "The MAC is fast."),
    ] {
        assert_eq!(normalize(Lang::En, input), expected, "{input}");
    }
    let value = "00000000-0000-0000-0000-000000000042";
    let expected = [
        ["zero"; 8].join(" "),
        ["zero"; 4].join(" "),
        ["zero"; 4].join(" "),
        ["zero"; 4].join(" "),
        format!("{} four two", ["zero"; 10].join(" ")),
    ]
    .join(" dash ");
    assert_eq!(
        normalize(Lang::En, &format!("UUID {value}.")),
        format!("U U I D {expected}.")
    );
}

#[test]
fn grouped_cards_are_complete_or_preserved_not_truncated() {
    for grouped in ["0000 0000 0000 0000 007", "0000 000000 00007"] {
        let count = grouped.bytes().filter(u8::is_ascii_digit).count();
        let expected = format!("Card number {} seven.", vec!["zero"; count - 1].join(" "));
        assert_eq!(
            normalize(Lang::En, &format!("Card number {grouped}.")),
            expected
        );
    }
    for literal in [
        "Card number 0000 0000 0000 0000 0007.",
        "Card number 0000 0000.",
    ] {
        assert_eq!(normalize(Lang::En, literal), literal);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn typed_records_keep_all_zeroes(digits in "0[0-9]{3,17}") {
        let spoken = nrmlz::num::en::digits(&digits);
        for (input, label) in [("Account number ", "Account number "), ("Routing number ", "Routing number "), ("MRN ", "M R N "), ("Member ID: ", "Member I D: ")] {
            prop_assert_eq!(normalize(Lang::En, &format!("{input}{digits}.")), format!("{label}{spoken}."));
        }
    }

    #[test]
    fn blood_pressure_keeps_two_values(a in 0u64..1000, b in 0u64..1000) {
        prop_assert_eq!(normalize(Lang::En, &format!("BP {a}/{b}.")), format!("B P {} over {}.", nrmlz::num::en::cardinal(a), nrmlz::num::en::cardinal(b)));
    }

    #[test]
    fn mac_hexadecimal_is_lossless(bytes in any::<[u8;6]>()) {
        let groups: Vec<_> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let expected = groups.iter().map(|s| s.chars().map(|c| if c.is_ascii_digit() { nrmlz::num::en::digits(&c.to_string()) } else { c.to_string() }).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" colon ");
        prop_assert_eq!(normalize(Lang::En, &format!("MAC {}.", groups.join(":"))), format!("M A C {expected}."));
    }
}
