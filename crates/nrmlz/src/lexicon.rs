//! User lexicon: a term list mapping a word to a replacement ("OK" -> "Okay", product names, respellings for
//! loanwords), applied in one pass before or after normalisation.
//!
//! Matching rules:
//! - Case-sensitive by default; per lexicon or per entry `case_sensitive = false`.
//! - Word-bounded: a term that starts (ends) with a letter, digit or underscore must not be preceded
//!   (followed) by one. A term edge that is punctuation ("C++", "z.B.") needs no boundary there.
//! - Longest term first; at each position the longest matching term wins. Replacements are not rescanned.
//! - A term ending in "." ("O.K.", "usw.") whose dot also ends the sentence keeps a "." after its
//!   replacement: at the end of the text in every language, and in English also before a word that
//!   conventionally starts a sentence (the abbreviation rule). German has no mid-text rule because it
//!   capitalises nouns ("z.B. Äpfel").
//!
//! Stage: `before` (default) matches raw terms before normalization. In the nrmlz profile the replacements
//! remain opaque until every normalizer rule finishes, so explicit user text always wins. The reference profile
//! retains the old rewriting order. `after` matches terms already in their normalized/spoken form.
//!
//! File formats (TOML or JSON with the same structure, or TSV `term<TAB>replacement`):
//!
//! ```toml
//! stage = "before"            # or "after"
//! case_sensitive = true       # default for all entries
//!
//! [terms]
//! OK = "Okay"
//! kubectl = "kjub kontroll"
//!
//! [[entry]]                   # entries with their own options
//! term = "nginx"
//! replacement = "Engine X"
//! case_sensitive = false
//! ```

use std::sync::OnceLock;

use crate::pyre::Re;
use crate::pystr::is_word;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    any(feature = "lexicon-toml", feature = "lexicon-json"),
    derive(serde::Deserialize)
)]
#[cfg_attr(
    any(feature = "lexicon-toml", feature = "lexicon-json"),
    serde(rename_all = "lowercase")
)]
pub enum LexiconStage {
    /// On the raw text, before normalisation.
    #[default]
    Before,
    /// On the normalised text.
    After,
}

impl std::str::FromStr for LexiconStage {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "before" => Ok(LexiconStage::Before),
            "after" => Ok(LexiconStage::After),
            _ => Err(Error::Lexicon(format!(
                "unknown stage {s:?} (before or after)"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexiconEntry {
    pub term: String,
    pub replacement: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Default)]
pub struct Lexicon {
    entries: Vec<LexiconEntry>,
    stage: LexiconStage,
    explicit_stage: bool,
    compiled: OnceLock<Option<Re>>,
}

impl Clone for Lexicon {
    fn clone(&self) -> Self {
        Lexicon {
            entries: self.entries.clone(),
            stage: self.stage,
            explicit_stage: self.explicit_stage,
            compiled: OnceLock::new(),
        }
    }
}

fn escape(term: &str) -> String {
    let mut out = String::new();
    for c in term.chars() {
        if c.is_ascii_alphanumeric() || !c.is_ascii() {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

impl Lexicon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stage(mut self, stage: LexiconStage) -> Self {
        self.set_stage(stage);
        self
    }

    /// Whether the stage was set explicitly (in code or in the lexicon file) rather than left at the default.
    pub fn has_explicit_stage(&self) -> bool {
        self.explicit_stage
    }

    pub fn stage(&self) -> LexiconStage {
        self.stage
    }

    pub fn set_stage(&mut self, stage: LexiconStage) {
        self.stage = stage;
        self.explicit_stage = true;
    }

    pub fn entries(&self) -> &[LexiconEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add a case-sensitive term. A later entry for the same term replaces the earlier one.
    pub fn insert(&mut self, term: impl Into<String>, replacement: impl Into<String>) -> &mut Self {
        self.insert_entry(LexiconEntry {
            term: term.into(),
            replacement: replacement.into(),
            case_sensitive: true,
        })
    }

    pub fn insert_entry(&mut self, entry: LexiconEntry) -> &mut Self {
        if entry.term.is_empty() {
            return self;
        }
        self.entries
            .retain(|e| e.term != entry.term || e.case_sensitive != entry.case_sensitive);
        self.entries.push(entry);
        self.compiled = OnceLock::new();
        self
    }

    fn regex(&self) -> Option<&Re> {
        self.compiled
            .get_or_init(|| {
                if self.entries.is_empty() {
                    return None;
                }
                let mut order: Vec<usize> = (0..self.entries.len()).collect();
                order.sort_by_key(|&i| std::cmp::Reverse(self.entries[i].term.chars().count()));
                let alternatives: Vec<String> = order
                    .iter()
                    .map(|&i| {
                        let e = &self.entries[i];
                        let first = e.term.chars().next().unwrap();
                        let last = e.term.chars().last().unwrap();
                        let body = if e.case_sensitive {
                            escape(&e.term)
                        } else {
                            format!("(?i:{})", escape(&e.term))
                        };
                        let left = if is_word(first) { r"(?<!\w)" } else { "" };
                        let right = if is_word(last) { r"(?!\w)" } else { "" };
                        format!("({left}{body}{right})")
                    })
                    .collect();
                Some(Re::new(&alternatives.join("|")))
            })
            .as_ref()
    }

    /// `dot_ends_sentence(after)` decides whether a term's final "." ("O.K.", "usw.") also ended the
    /// sentence; if so and the replacement does not end one itself, a plain "." follows the replacement.
    fn rewrite(
        &self,
        text: &str,
        dot_ends_sentence: &dyn Fn(&str) -> bool,
        mut replacement: impl FnMut(&str) -> String,
    ) -> Result<String, Error> {
        let Some(re) = self.regex() else {
            return Ok(text.to_string());
        };
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.entries[i].term.chars().count()));
        re.sub_fn(text, |m| {
            let group = (1..=order.len()).find(|&g| m.get(g).is_some()).unwrap();
            let entry = &self.entries[order[group - 1]];
            let mut out = replacement(&entry.replacement);
            if entry.term.ends_with('.')
                && !entry.replacement.ends_with(['.', '!', '?', '…'])
                && dot_ends_sentence(m.after())
            {
                out.push('.');
            }
            Ok(out)
        })
    }

    /// Replace every term in `text`. A term's final dot is kept only at the end of the text, the one
    /// sentence boundary that needs no language knowledge.
    pub fn apply(&self, text: &str) -> String {
        self.apply_with(text, &text_ends)
    }

    pub(crate) fn apply_with(
        &self,
        text: &str,
        dot_ends_sentence: &dyn Fn(&str) -> bool,
    ) -> String {
        self.rewrite(text, dot_ends_sentence, str::to_string)
            .unwrap_or_else(|_| text.to_string())
    }

    /// Replace terms with collision-free opaque words. Restoration inserts the final user text only after
    /// all normalisation rules have run, even when the input contains Yi/CJK text or thousands of terms.
    pub(crate) fn protect(
        &self,
        text: &str,
        dot_ends_sentence: &dyn Fn(&str) -> bool,
    ) -> Option<(String, crate::protected::ProtectedText)> {
        let mut protected = crate::protected::ProtectedText::new(text);
        let out = self
            .rewrite(text, dot_ends_sentence, |value| {
                protected.insert(value.to_string())
            })
            .ok()?;
        Some((out, protected))
    }

    /// TSV: one `term<TAB>replacement` per line; empty lines and lines starting with `#` are skipped.
    pub fn from_tsv(text: &str) -> Result<Self, Error> {
        let mut lex = Lexicon::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let (term, replacement) = line
                .split_once('\t')
                .ok_or_else(|| Error::Lexicon(format!("line {}: no tab", n + 1)))?;
            lex.insert(term, replacement);
        }
        Ok(lex)
    }

    #[cfg(feature = "lexicon-toml")]
    pub fn from_toml(text: &str) -> Result<Self, Error> {
        let file: LexiconFile = toml::from_str(text).map_err(|e| Error::Lexicon(e.to_string()))?;
        Ok(file.into_lexicon())
    }

    #[cfg(feature = "lexicon-json")]
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let file: LexiconFile =
            serde_json::from_str(text).map_err(|e| Error::Lexicon(e.to_string()))?;
        Ok(file.into_lexicon())
    }

    /// Read a lexicon file; the format follows the extension (.toml, .json, .tsv).
    pub fn from_path(path: &std::path::Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Lexicon(format!("{}: {e}", path.display())))?;
        match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            #[cfg(feature = "lexicon-toml")]
            "toml" => Self::from_toml(&text),
            #[cfg(feature = "lexicon-json")]
            "json" => Self::from_json(&text),
            "tsv" | "txt" => Self::from_tsv(&text),
            other => Err(Error::Lexicon(format!(
                "unsupported lexicon format {other:?}"
            ))),
        }
    }
}

/// Only whitespace and closing quotes/brackets remain: the term's dot ended the text.
pub(crate) fn text_ends(after: &str) -> bool {
    after
        .trim_start_matches(|c: char| c.is_whitespace() || "\"'”’»›)]".contains(c))
        .is_empty()
}

#[cfg(any(feature = "lexicon-toml", feature = "lexicon-json"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LexiconFile {
    #[serde(default)]
    stage: Option<LexiconStage>,
    #[serde(default = "yes")]
    case_sensitive: bool,
    #[serde(default)]
    terms: indexmap_lite::OrderedMap,
    #[serde(default)]
    entry: Vec<FileEntry>,
}

#[cfg(any(feature = "lexicon-toml", feature = "lexicon-json"))]
fn yes() -> bool {
    true
}

#[cfg(any(feature = "lexicon-toml", feature = "lexicon-json"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    term: String,
    replacement: String,
    case_sensitive: Option<bool>,
}

#[cfg(any(feature = "lexicon-toml", feature = "lexicon-json"))]
impl LexiconFile {
    fn into_lexicon(self) -> Lexicon {
        let mut lex = Lexicon::new();
        if let Some(stage) = self.stage {
            lex.set_stage(stage);
        }
        for (term, replacement) in self.terms.0 {
            lex.insert_entry(LexiconEntry {
                term,
                replacement,
                case_sensitive: self.case_sensitive,
            });
        }
        for e in self.entry {
            lex.insert_entry(LexiconEntry {
                term: e.term,
                replacement: e.replacement,
                case_sensitive: e.case_sensitive.unwrap_or(self.case_sensitive),
            });
        }
        lex
    }
}

/// A string map that keeps file order (later duplicates win), without pulling in indexmap.
#[cfg(any(feature = "lexicon-toml", feature = "lexicon-json"))]
mod indexmap_lite {
    use serde::de::{Deserializer, MapAccess, Visitor};

    #[derive(Default)]
    pub struct OrderedMap(pub Vec<(String, String)>);

    impl<'de> serde::Deserialize<'de> for OrderedMap {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = OrderedMap;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a table of term = replacement strings")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedMap, A::Error> {
                    let mut out = Vec::new();
                    while let Some((k, v)) = map.next_entry::<String, String>()? {
                        out.push((k, v));
                    }
                    Ok(OrderedMap(out))
                }
            }
            d.deserialize_map(V)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_and_case() {
        let mut lex = Lexicon::new();
        lex.insert("OK", "Okay")
            .insert("C++", "C plus plus")
            .insert("Kubernetes", "Kuberneetis");
        assert_eq!(
            lex.apply("OK, das ist OKAY und ok. OK!"),
            "Okay, das ist OKAY und ok. Okay!"
        );
        assert_eq!(
            lex.apply("C++-Code und xC++ (C++)"),
            "C plus plus-Code und xC++ (C plus plus)"
        );
        assert_eq!(
            lex.apply("Kubernetes-Cluster, Kubernetesse"),
            "Kuberneetis-Cluster, Kubernetesse"
        );
        lex.insert_entry(LexiconEntry {
            term: "nginx".into(),
            replacement: "Engine X".into(),
            case_sensitive: false,
        });
        assert_eq!(lex.apply("NGINX und Nginx"), "Engine X und Engine X");
    }

    #[test]
    fn longest_first_and_single_pass() {
        let mut lex = Lexicon::new();
        lex.insert("New York", "Nju Jork")
            .insert("New", "Nju")
            .insert("Nju", "XX");
        assert_eq!(
            lex.apply("New York und New Jersey"),
            "Nju Jork und Nju Jersey"
        );
    }

    #[cfg(feature = "lexicon-toml")]
    #[test]
    fn toml_file() {
        let lex = Lexicon::from_toml(
            "stage = \"after\"\n[terms]\nOK = \"Okay\"\n[[entry]]\nterm = \"ui\"\nreplacement = \"Ju Ai\"\ncase_sensitive = false\n",
        )
        .unwrap();
        assert_eq!(lex.stage(), LexiconStage::After);
        assert_eq!(lex.apply("OK UI"), "Okay Ju Ai");
        assert!(Lexicon::from_toml("bogus = 1").is_err());
    }

    #[cfg(feature = "lexicon-json")]
    #[test]
    fn json_file() {
        let lex =
            Lexicon::from_json(r#"{"terms": {"OK": "Okay"}, "case_sensitive": false}"#).unwrap();
        assert_eq!(lex.apply("ok"), "Okay");
    }

    #[test]
    fn tsv_file() {
        let lex = Lexicon::from_tsv("# comment\nSpO2\tSauerstoffsättigung\n").unwrap();
        assert_eq!(lex.apply("SpO2 92 %"), "Sauerstoffsättigung 92 %");
    }

    fn okay(lang_term: &str) -> Lexicon {
        let mut lex = Lexicon::new();
        for term in ["Okay", "OK", "O.K."] {
            lex.insert_entry(LexiconEntry {
                term: term.into(),
                replacement: lang_term.into(),
                case_sensitive: false,
            });
        }
        lex
    }

    #[test]
    fn dotted_term_keeps_the_dot_only_where_it_ends_the_sentence() {
        let lex = okay("O K");
        assert_eq!(lex.apply("That is O.K."), "That is O K.");
        assert_eq!(lex.apply("That is O.K.\""), "That is O K.\"");
        assert_eq!(lex.apply("It's o.k. with me."), "It's O K with me.");
        assert_eq!(lex.apply("O.K.?"), "O K?");
        assert_eq!(lex.apply("O.K., fine."), "O K, fine.");
        // Language-agnostic apply never guesses a mid-text sentence boundary.
        assert_eq!(
            lex.apply("That's O.K. Then we go."),
            "That's O K Then we go."
        );
        // A replacement that already ends a sentence gets no second dot.
        let mut abbr = Lexicon::new();
        abbr.insert("usw.", "und so weiter.");
        assert_eq!(abbr.apply("Obst usw."), "Obst und so weiter.");
    }

    #[test]
    fn normalisation_uses_each_language_sentence_rule() {
        use crate::{normalize_with_lexicon, Lang};
        let en = okay("O K");
        assert_eq!(
            normalize_with_lexicon(Lang::En, "That's O.K. Then we go.", &en),
            "That's O K. Then we go."
        );
        assert_eq!(
            normalize_with_lexicon(Lang::En, "That is O.K.", &en),
            "That is O K."
        );
        assert_eq!(
            normalize_with_lexicon(Lang::En, "It's O.K. with me.", &en),
            "It's O K with me."
        );
        let de = okay("Okeh");
        assert_eq!(
            normalize_with_lexicon(Lang::De, "Das ist O.K.", &de),
            "Das ist Okeh."
        );
        // German capitalises nouns: a capital after the dot is not a sentence start.
        let mut zb = Lexicon::new();
        zb.insert("z.B.", "zum Beispiel");
        assert_eq!(
            normalize_with_lexicon(Lang::De, "Obst, z.B. Äpfel.", &zb),
            "Obst, zum Beispiel Äpfel."
        );
    }
}
