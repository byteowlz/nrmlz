//! Profiles: the frozen Articulino behaviour, or nrmlz's own with named improvements.
//!
//! [`Profile::REFERENCE`] is the byte-identical port of the Articulino frontend; its differential test against
//! the Python version stays as a regression gate. [`Profile::NRMLZ`] (the default) enables every [`Fix`]. Each
//! fix is one documented deviation from the reference; the difftest attributes every difference on the
//! committed corpora to a fix and counts them per class (docs/differences.md).
//!
//! Two fixes are also user options, because some models need the reference behaviour:
//! [`Profile::with_decimal_pause`] (the comma after "Komma") and [`Profile::with_respellings`] (the
//! respelling entries of the abbreviation table, "Homebrew" -> "Hohmbruh").

/// One deviation from the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum Fix {
    /// Units with ² or ³ without a number before them: "pro m²" -> "pro Quadratmeter", "in kg/m³" ->
    /// "in Kilogramm pro Kubikmeter" (reference: "pro emm hoch zwei", "Kilogramm emm hoch drei").
    StandaloneUnits = 0,
    /// "3,5" is "drei Komma fünf", without the pause comma the reference adds ("drei Komma, fünf").
    PlainDecimal = 1,
    /// The respelling entries of the abbreviation table (Homebrew -> Hohmbruh, nginx -> Engine Eks, OK -> okeh,
    /// NASA -> Nasa) are not applied; acronyms spoken as words (NASA, PIN, OK) stay as written instead of being
    /// spelled out letter by letter.
    NoRespellings = 2,
    /// A list number before the same enumeration word is read once: "1. Erstens das" -> "Erstens das"
    /// (reference: "Erstens, Erstens das").
    SingleListWord = 3,
    /// Replacements of a user lexicon applied before normalisation are final: no later rule rewrites them
    /// (reference behaviour: "Okay" from the lexicon became "Okeh").
    LexiconFinal = 4,
    /// IBAN, ISBN and numbers after PIN, Postfach, Notruf, Kundennummer and similar are read digit by digit;
    /// three or more numbers joined by hyphens are a code, not a range (reference: "978-3-16" ->
    /// "neunhundertachtundsiebzig bis drei bis sechzehn"); ISO dates (2023-11-30).
    Identifiers = 5,
    /// Dates written 30/11/2023, 15-Aug-2023, "31. Dez." and "15. Mär." read as dates.
    Dates = 6,
    /// URLs with a scheme and any ending ("https://youtu.be/abc", "ftp://...").
    Urls = 7,
    /// LaTeX commands outside $...$ ("\frac{1}{2}" as language models often write them).
    BareLatex = 8,
    /// "#Klimawandel" -> "Hashtag Klimawandel", "@tagesschau" -> "at tagesschau".
    Hashtags = 9,
    /// Unicode fractions: "¾" -> "drei Viertel", "1½ kg" -> "eineinhalb Kilogramm", "eine ½ Note" -> "eine halbe".
    UnicodeFractions = 10,
    /// Coordinates: "52.52° N" -> "zweiundfünfzig Komma fünf zwei Grad Nord".
    Coordinates = 11,
    /// Roman numerals after names and keywords: "Ludwig XIV." -> "Ludwig der Vierzehnte", "Kapitel VII" ->
    /// "Kapitel sieben", "§ 823 II" -> "Paragraf ... Absatz zwei".
    RomanNumerals = 12,
    /// "v3.2.1" -> "Version drei Punkt zwei Punkt eins".
    VersionPrefix = 13,
    /// "in 1:45 Stunden" -> "in einer Stunde und fünfundvierzig Minuten" (reference: "ein Uhr fünfundvierzig").
    Durations = 14,
    /// Scores in a sports context and impossible times: "endete 24:18" -> "vierundzwanzig zu achtzehn".
    Scores = 15,
    /// Decades and number nouns: "in den 70ern" -> "in den Siebzigern", "ein 3er" -> "ein Dreier".
    Decades = 16,
    /// Ordinal endings after possessives and demonstratives ("mein 1. Besuch" -> "mein erster Besuch"),
    /// "ein 3. Platz" -> "ein dritter Platz", "der 3. Runde" -> "der dritten Runde", "bei einer Million".
    OrdinalAgreement = 17,
    /// No digits or unreadable symbols left: digits glued to any letter ("7µF"), "+49", "3<5", "$" after scale
    /// words.
    NoLeftovers = 18,
    /// More abbreviations (Abs., Art., St., Fr. before a name, Hbf., S. 12, ...), in data/de/abbreviations-nrmlz.json.
    MoreAbbreviations = 19,
    /// Mixed-case initialisms spelled like upper-case ones: "pH" -> "peh hah", "mRNA" -> "emm err enn ah".
    Initialisms = 20,
    /// No space at the start or end of the output (reference: " Pi ist ..." for "π ist ...").
    TidySpaces = 21,
    /// Balanced [en]...[/en] spans use the English normaliser and retain their tags for language routing.
    CodeSwitching = 22,
    /// Dotted clock times followed by "Uhr" are times, not decimal numbers: "13.15 Uhr" -> "dreizehn Uhr
    /// fünfzehn" (reference: "dreizehn Punkt fünfzehn Uhr").
    DottedClocks = 23,
    /// Letter codes before a number or a hyphenated number read by letter name and digit by digit: "Flug ER
    /// 7651" -> "Flug eh err sieben sechs fünf eins", "B-40884" -> "beh vier null acht acht vier".
    LetterCodes = 24,
    /// Kitchen units with an article after a bare 1: "1 Prise Salz" -> "eine Prise Salz", "1 EL Öl" ->
    /// "einen Esslöffel Öl" after a verb, "ein Teelöffel" otherwise.
    KitchenUnits = 25,
    /// A bare 1 before a known common noun takes its article: "Sie hat 1 Bruder" -> "einen Bruder",
    /// "1 Kind" -> "ein Kind", "mit 1 Schwester" -> "einer Schwester". Not a general noun parser.
    BareOneArticle = 26,
    /// A plain integer run longer than number words with a phone/identifier cue before it, or with an amount cue
    /// after it, takes that reading: "Telefonnummer 46255577782" digit by digit, "... 46255577782 Stück" as an
    /// amount. Without a cue the existing long-number fallback stays.
    NumberContext = 27,
}

impl Fix {
    pub const ALL: &'static [Fix] = &[
        Fix::StandaloneUnits,
        Fix::PlainDecimal,
        Fix::NoRespellings,
        Fix::SingleListWord,
        Fix::LexiconFinal,
        Fix::Identifiers,
        Fix::Dates,
        Fix::Urls,
        Fix::BareLatex,
        Fix::Hashtags,
        Fix::UnicodeFractions,
        Fix::Coordinates,
        Fix::RomanNumerals,
        Fix::VersionPrefix,
        Fix::Durations,
        Fix::Scores,
        Fix::Decades,
        Fix::OrdinalAgreement,
        Fix::NoLeftovers,
        Fix::MoreAbbreviations,
        Fix::Initialisms,
        Fix::TidySpaces,
        Fix::CodeSwitching,
        Fix::DottedClocks,
        Fix::LetterCodes,
        Fix::KitchenUnits,
        Fix::BareOneArticle,
        Fix::NumberContext,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Fix::StandaloneUnits => "standalone-units",
            Fix::PlainDecimal => "plain-decimal",
            Fix::NoRespellings => "no-respellings",
            Fix::SingleListWord => "single-list-word",
            Fix::LexiconFinal => "lexicon-final",
            Fix::Identifiers => "identifiers",
            Fix::Dates => "dates",
            Fix::Urls => "urls",
            Fix::BareLatex => "bare-latex",
            Fix::Hashtags => "hashtags",
            Fix::UnicodeFractions => "unicode-fractions",
            Fix::Coordinates => "coordinates",
            Fix::RomanNumerals => "roman-numerals",
            Fix::VersionPrefix => "version-prefix",
            Fix::Durations => "durations",
            Fix::Scores => "scores",
            Fix::Decades => "decades",
            Fix::OrdinalAgreement => "ordinal-agreement",
            Fix::NoLeftovers => "no-leftovers",
            Fix::MoreAbbreviations => "more-abbreviations",
            Fix::Initialisms => "initialisms",
            Fix::TidySpaces => "tidy-spaces",
            Fix::CodeSwitching => "code-switching",
            Fix::DottedClocks => "dotted-clocks",
            Fix::LetterCodes => "letter-codes",
            Fix::KitchenUnits => "kitchen-units",
            Fix::BareOneArticle => "bare-one-article",
            Fix::NumberContext => "number-context",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Fix::StandaloneUnits => {
                "units with a power read as units without a number before them (pro m², kg/m³)"
            }
            Fix::PlainDecimal => "no pause comma after \"Komma\" (drei Komma fünf)",
            Fix::NoRespellings => {
                "no respelling of loanwords and acronyms; acronym words stay as written"
            }
            Fix::SingleListWord => "list number not doubled before the same enumeration word",
            Fix::LexiconFinal => "user lexicon replacements are not rewritten by later rules",
            Fix::Identifiers => {
                "IBAN, ISBN, PIN/Postfach/Notruf numbers digit by digit; hyphen chains are codes"
            }
            Fix::Dates => "slash, ISO, English-style and abbreviated-month dates",
            Fix::Urls => "URLs with a scheme and any ending",
            Fix::BareLatex => "LaTeX commands without $ delimiters",
            Fix::Hashtags => "hashtags and @-mentions",
            Fix::UnicodeFractions => "Unicode fraction characters",
            Fix::Coordinates => "geographic coordinates with N/S/O/W",
            Fix::RomanNumerals => "Roman numerals after names, keywords and paragraphs",
            Fix::VersionPrefix => "v1.2 as Version eins Punkt zwei",
            Fix::Durations => "h:mm Stunden / mm:ss Minuten as durations",
            Fix::Scores => "scores and impossible times as 'zu'",
            Fix::Decades => "70er, 70ern, 3er",
            Fix::OrdinalAgreement => {
                "ordinal endings after possessives, demonstratives, ein-words; einer Million"
            }
            Fix::NoLeftovers => "no digits or unreadable symbols left",
            Fix::MoreAbbreviations => "abbreviations missing from the reference table",
            Fix::Initialisms => "mixed-case initialisms (pH, mRNA) spelled",
            Fix::TidySpaces => "no space at the start or end of the output",
            Fix::CodeSwitching => "explicit English spans normalised in English, tags retained",
            Fix::DottedClocks => "13.15 Uhr as a clock time, not a decimal before Uhr",
            Fix::LetterCodes => "flight/record letter codes spelled, digits read one by one",
            Fix::KitchenUnits => "kitchen units (EL, TL, Prise) with the article of a bare 1",
            Fix::BareOneArticle => {
                "bare 1 before a known common noun takes ein/eine/einen/einem/einer"
            }
            Fix::NumberContext => {
                "phone/identifier cues read long integers digit by digit, amount cues as amounts"
            }
        }
    }

    fn bit(self) -> u32 {
        debug_assert!((self as u32) < 32);
        1 << self as u32
    }
}

impl std::str::FromStr for Fix {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, crate::Error> {
        Fix::ALL
            .iter()
            .copied()
            .find(|f| f.name() == s)
            .ok_or_else(|| crate::Error::UnknownProfile(format!("unknown fix {s:?}")))
    }
}

/// A set of enabled fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Profile {
    fixes: u32,
}

impl Profile {
    /// The Articulino frontend exactly (byte-identical to the pinned Python version).
    pub const REFERENCE: Profile = Profile { fixes: 0 };
    /// nrmlz's profile: every fix (the default).
    pub const NRMLZ: Profile = Profile { fixes: u32::MAX };

    pub fn has(self, fix: Fix) -> bool {
        self.fixes & fix.bit() != 0
    }

    pub fn with(self, fix: Fix) -> Profile {
        Profile {
            fixes: self.fixes | fix.bit(),
        }
    }

    pub fn without(self, fix: Fix) -> Profile {
        Profile {
            fixes: self.fixes & !fix.bit(),
        }
    }

    /// Read "3,5" as "drei Komma, fünf" (with a pause comma), for models trained on the reference frontend.
    pub fn with_decimal_pause(self, on: bool) -> Profile {
        if on {
            self.without(Fix::PlainDecimal)
        } else {
            self.with(Fix::PlainDecimal)
        }
    }

    /// Apply the respelling entries of the abbreviation table (Homebrew -> Hohmbruh, OK -> okeh).
    pub fn with_respellings(self, on: bool) -> Profile {
        if on {
            self.without(Fix::NoRespellings)
        } else {
            self.with(Fix::NoRespellings)
        }
    }

    /// The enabled fixes.
    pub fn fixes(self) -> Vec<Fix> {
        Fix::ALL.iter().copied().filter(|&f| self.has(f)).collect()
    }

    pub fn name(self) -> String {
        if self == Profile::NRMLZ || self.fixes() == Fix::ALL {
            return "nrmlz".into();
        }
        if self.fixes().is_empty() {
            return "reference".into();
        }
        let mut s = "reference".to_string();
        for f in self.fixes() {
            s += "+";
            s += f.name();
        }
        s
    }
}

impl Default for Profile {
    fn default() -> Self {
        Profile::NRMLZ
    }
}

impl std::str::FromStr for Profile {
    type Err = crate::Error;
    /// "nrmlz" (alias "default"), "reference" (alias "articulino"), each optionally followed by "+fix" or
    /// "-fix" terms: "reference+plain-decimal", "nrmlz-no-respellings".
    fn from_str(s: &str) -> Result<Self, crate::Error> {
        let bases = [
            ("nrmlz", Profile::NRMLZ),
            ("default", Profile::NRMLZ),
            ("reference", Profile::REFERENCE),
            ("articulino", Profile::REFERENCE),
        ];
        let (base, mut rest) = bases
            .iter()
            .find_map(|(n, p)| s.strip_prefix(n).map(|r| (*p, r)))
            .ok_or_else(|| crate::Error::UnknownProfile(format!("unknown profile {s:?}")))?;
        let mut p = base;
        // fix names contain '-', so read the longest fix name after each operator
        while !rest.is_empty() {
            let (op, body) = rest.split_at(1);
            if op != "+" && op != "-" {
                return Err(crate::Error::UnknownProfile(format!(
                    "unknown profile {s:?}"
                )));
            }
            let fix = Fix::ALL
                .iter()
                .copied()
                .filter(|f| body.starts_with(f.name()))
                .max_by_key(|f| f.name().len())
                .ok_or_else(|| crate::Error::UnknownProfile(format!("unknown fix in {s:?}")))?;
            p = if op == "+" {
                p.with(fix)
            } else {
                p.without(fix)
            };
            rest = &body[fix.name().len()..];
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!("nrmlz".parse::<Profile>().unwrap(), Profile::NRMLZ);
        assert_eq!("default".parse::<Profile>().unwrap(), Profile::NRMLZ);
        assert_eq!("reference".parse::<Profile>().unwrap(), Profile::REFERENCE);
        let p: Profile = "reference+standalone-units".parse().unwrap();
        assert!(p.has(Fix::StandaloneUnits) && !p.has(Fix::PlainDecimal));
        let p: Profile = "nrmlz-plain-decimal-no-respellings".parse().unwrap();
        assert!(
            !p.has(Fix::PlainDecimal) && !p.has(Fix::NoRespellings) && p.has(Fix::StandaloneUnits)
        );
        assert!("nope".parse::<Profile>().is_err());
        assert!("nrmlz+nope".parse::<Profile>().is_err());
        assert_eq!(
            Profile::NRMLZ.with_decimal_pause(true),
            Profile::NRMLZ.without(Fix::PlainDecimal)
        );
    }
}
