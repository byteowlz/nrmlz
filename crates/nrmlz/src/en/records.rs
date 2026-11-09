//! Financial, government and medical record values are identities, not quantities.
use std::sync::LazyLock;

use super::{identifiers::code_words, Text};
use crate::pyre::{re, LazyRe, Match, Re, R};

static FIELD: LazyRe = re!(
    r"\b(account[ \t]+(?:number|no\.?)|routing[ \t]+number|(?:credit[ \t]+|debit[ \t]+)?card[ \t]+number|CVV|CVC|SSN|social[ \t]+security[ \t]+number|tax[ \t]+(?:ID|number)|TIN|EIN|passport[ \t]+(?:number|no\.?)|medical[ \t]+record[ \t]+number|MRN|member[ \t]+(?:ID|number)|policy[ \t]+(?:number|ID)|insurance[ \t]+(?:ID|number)|patient[ \t]+ID|claim[ \t]+(?:number|ID)|IBAN)(?=[ \t:#0-9])([ \t]*(?::|#)?[ \t]*(?:(?:is|was)[ \t]+)?)(?=[A-Z0-9.-]*[0-9])([A-Z0-9]+(?:[-.][A-Z0-9]+)*)(?![\w/:-]|\.[A-Z0-9])",
    "i"
);
static CARD: LazyRe = re!(
    r"\b((?:credit[ \t]+|debit[ \t]+)?card[ \t]+number)([ \t]*:?[ \t]*)((?:[0-9]{4}(?:[ \t-]+[0-9]{4}){3}[ \t-]+[0-9]{1,3}|[0-9]{4}(?:[ \t-]+[0-9]{4}){2}[ \t-]+[0-9]{1,4}|[0-9]{4}[ \t-]+[0-9]{6}[ \t-]+[0-9]{5}))(?!\w|[ \t-]+[0-9])",
    "i"
);
static UNKNOWN_CARD: LazyRe = re!(
    r"\b(?:credit[ \t]+|debit[ \t]+)?card[ \t]+number[ \t]*:?[ \t]*[0-9]+(?:[ \t-]+[0-9]+)+(?![\w-])",
    "i"
);
static SPACED_SSN: LazyRe = re!(
    r"\b(SSN|social[ \t]+security[ \t]+number)([ \t]*:?[ \t]*)([0-9]{3}[ \t]+[0-9]{2}[ \t]+[0-9]{4})(?!\w)",
    "i"
);
static UNKNOWN_IBAN: LazyRe = re!(
    r"\bIBAN[ \t]*:?[ \t]*[A-Z]{2}[0-9]{2}(?:[ \t]+[A-Z0-9]{1,4}){1,8}(?!\w)",
    "i"
);
static BIC: LazyRe = re!(
    r"\b(SWIFT(?:[ \t]+BIC)?|BIC)([ \t]*:?[ \t]+|:)([A-Z]{4}[A-Z]{2}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)(?!\w)",
    "i"
);
// Public ISO 13616 country lengths; strict groups prevent consuming following prose.
// Reading a field is not an account/checksum-validity claim. Other compact forms use FIELD.
const IBAN_LENGTHS: &[(&str, usize)] = &[
    ("DE", 22),
    ("GB", 22),
    ("IE", 22),
    ("FR", 27),
    ("ES", 24),
    ("IT", 27),
    ("NL", 18),
    ("BE", 16),
    ("CH", 21),
    ("AT", 20),
    ("LU", 20),
    ("PT", 25),
    ("PL", 28),
];
static IBAN: LazyLock<Re> = LazyLock::new(|| {
    let variants: Vec<_> = IBAN_LENGTHS
        .iter()
        .flat_map(|(country, length)| {
            let (groups, tail) = ((length - 4) / 4, (length - 4) % 4);
            let head = format!(r"(?i:{country})[0-9]{{2}}");
            let compact = format!(r"{head}[A-Za-z0-9]{{{}}}", length - 4);
            let mut spaced = format!(r"{head}(?:[ \t]+[A-Za-z0-9]{{4}}){{{groups}}}");
            if tail > 0 {
                spaced += &format!(r"[ \t]+[A-Za-z0-9]{{{tail}}}");
            }
            [compact, spaced]
        })
        .collect();
    Re::new(&format!(
        r"\b(?i:IBAN)([ \t]*:?[ \t]*)((?:{}))(?!\w)",
        variants.join("|")
    ))
});

fn label(s: &str) -> String {
    s.split_whitespace()
        .map(|w| match w.to_ascii_uppercase().as_str() {
            "ID" | "IBAN" | "BIC" | "CVV" | "CVC" | "SSN" | "MRN" | "TIN" | "EIN" => {
                code_words(w, true)
            }
            _ => w.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn field(m: &Match<'_>) -> String {
    format!(
        "{}{} {}",
        label(m.g(1)),
        m.g(2).trim_end(),
        code_words(m.g(3), true)
    )
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.apply(&IBAN, |m| {
        format!("I B A N{} {}", m.g(1).trim_end(), code_words(m.g(2), true))
    })?;
    t.apply(&UNKNOWN_IBAN, |m| m.all().to_owned())?;
    t.apply(&CARD, field)?;
    t.apply(&UNKNOWN_CARD, |m| m.all().to_owned())?;
    t.apply(&SPACED_SSN, field)?;
    t.apply(&BIC, field)?;
    t.apply(&FIELD, field)
}
