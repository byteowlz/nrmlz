//! Property tests: no panics on arbitrary Unicode, bounded time, and stable results.

use std::time::{Duration, Instant};

use proptest::prelude::*;

/// Characters the rules care about, so random strings hit the interesting paths.
const ALPHABET: &str =
    "0123456789 .,:;!?-–−+*/=<>^()[]{}|#_`'\"$€£¥%°²³⁻₂√π×·@&§\n\tabcdefghxyzABCDEFKMPSUVXZ\
äöüßÄÖÜ😀→zBmkghsV";

fn rich_string() -> impl Strategy<Value = String> {
    let chars: Vec<char> = ALPHABET.chars().collect();
    proptest::collection::vec(proptest::sample::select(chars), 0..120)
        .prop_map(|v| v.into_iter().collect())
}

fn words() -> impl Strategy<Value = String> {
    let parts = vec![
        "am",
        "der",
        "3.",
        "Mai",
        "15:30",
        "Uhr",
        "24,99",
        "€",
        "z. B.",
        "PDF",
        "1",
        "h",
        "km/h",
        "10",
        "-",
        "4",
        "=",
        "6",
        "$x^2$",
        "**fett**",
        "\n",
        "1.",
        "+49",
        "170",
        "1234567",
        "info@x.de",
        "www.a.de",
        "Mo–Fr",
        "ca.",
        "%",
        "1/2",
        "2x",
        "täglich",
        "NICHT",
        "II",
        "😀",
        ":)",
        "→",
        "#",
        "|",
        "---",
        "1,80",
        "m",
        "in",
        "mit",
    ];
    proptest::collection::vec(proptest::sample::select(parts), 0..30).prop_map(|v| v.join(" "))
}

const LIMIT: Duration = Duration::from_millis(1500);

fn timed(text: &str) -> String {
    let t = Instant::now();
    let out = nrmlz::normalize_de(text);
    assert!(t.elapsed() < LIMIT, "took {:?} for {text:?}", t.elapsed());
    out
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, ..ProptestConfig::default() })]

    #[test]
    fn arbitrary_unicode_never_panics(s in any::<String>()) {
        let _ = timed(&s);
    }

    #[test]
    fn rule_characters_never_panic(s in rich_string()) {
        let out = timed(&s);
        // a second pass must not panic either
        let _ = timed(&out);
    }

    #[test]
    fn token_soup_is_deterministic(s in words()) {
        prop_assert_eq!(timed(&s), timed(&s));
    }

    #[test]
    fn streaming_never_panics(s in words(), size in 1usize..20) {
        let mut st = nrmlz::de::StreamNormalizer::new();
        let chars: Vec<char> = s.chars().collect();
        for c in chars.chunks(size) {
            let _ = st.push(&c.iter().collect::<String>());
        }
        let _ = st.flush("");
        prop_assert!(st.pending().is_empty());
    }
}

#[test]
fn long_inputs_stay_bounded() {
    // pathological shapes for backtracking regexes
    let cases = [
        "1.".repeat(2000),
        "a ".repeat(5000),
        "**".repeat(2000) + "x",
        "$".repeat(3000),
        "- ".repeat(3000) + "1",
        "12:".repeat(2000),
        "x - ".repeat(1500) + "1",
        "Am 3. Mai, ".repeat(400),
        "😀".repeat(3000),
        "\n".repeat(5000) + "a:",
    ];
    for c in &cases {
        let t = Instant::now();
        let _ = nrmlz::normalize_de(c);
        let took = t.elapsed();
        let head: String = c.chars().take(12).collect();
        eprintln!("{:>6} chars {head:?}...: {took:?}", c.chars().count());
        assert!(took < Duration::from_secs(10), "{head:?}... took {took:?}");
    }
}
