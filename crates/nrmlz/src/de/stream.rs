//! Normalising a token stream sentence by sentence, without cutting through abbreviations, list numbers or
//! ordinals at a chunk border. Port of `StreamNormalizer` from german.py.

use crate::pyre::{re, LazyRe};
use crate::pystr::back_chars;
use crate::Error;

use super::abbreviations::{HOLD_BACK, RULES};

// periods after numbers where the stream must not be cut
static LIST_PERIOD: LazyRe = re!(r"(?m)^[ \t>]*(?:\*\*|__|\*|_)?\d{1,3}[.)]");
static NUMBER_PERIOD: LazyRe = re!(r"\d\.(?=[ \t]+\S)");
static SENTENCE_END: LazyRe = re!(r"[.!?\u2026][\s]|\n");

type NormalizeFn = Box<dyn Fn(&str) -> Result<String, Error> + Send + Sync>;

/// Releases whole sentences of a growing text, normalised; a short tail is held back until more text arrives
/// or [`StreamNormalizer::flush`] is called.
pub struct StreamNormalizer {
    rest: String,
    normalize: NormalizeFn,
    english_spans: bool,
    preserve_separator: bool,
}

impl Default for StreamNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamNormalizer {
    /// With the nrmlz profile.
    pub fn new() -> Self {
        Self::with_profile(super::Profile::NRMLZ)
    }

    /// Normalise released chunks with `profile` ([`super::Profile::REFERENCE`]: as the Python version).
    pub fn with_profile(profile: super::Profile) -> Self {
        StreamNormalizer {
            rest: String::new(),
            normalize: Box::new(move |t| super::try_normalize_with(t, profile)),
            english_spans: profile.has(super::Fix::CodeSwitching),
            preserve_separator: profile.has(super::Fix::TidySpaces),
        }
    }

    /// Use `f` on every released chunk instead of the German normaliser (same cut points).
    pub fn with_normalizer(f: impl Fn(&str) -> String + Send + Sync + 'static) -> Self {
        StreamNormalizer {
            rest: String::new(),
            normalize: Box::new(move |t| Ok(f(t))),
            english_spans: true,
            preserve_separator: true,
        }
    }

    /// Add text; return the normalised complete sentences, or "" if none is complete yet. A released chunk
    /// the normaliser cannot handle (a number of 10^606 or more) is returned unchanged.
    pub fn push(&mut self, text: &str) -> String {
        self.release(text, false).unwrap_or_else(|(raw, _)| raw)
    }

    /// Add the last text and return everything left, normalised.
    pub fn flush(&mut self, text: &str) -> String {
        self.release(text, true).unwrap_or_else(|(raw, _)| raw)
    }

    /// [`StreamNormalizer::push`] with the error of the Python reference (the chunk is consumed either way).
    pub fn try_push(&mut self, text: &str) -> Result<String, Error> {
        self.release(text, false).map_err(|(_, e)| e)
    }

    /// [`StreamNormalizer::flush`] with the error of the Python reference.
    pub fn try_flush(&mut self, text: &str) -> Result<String, Error> {
        self.release(text, true).map_err(|(_, e)| e)
    }

    fn release(&mut self, text: &str, all: bool) -> Result<String, (String, Error)> {
        self.rest.push_str(text);
        let cut = if all {
            self.rest.len()
        } else {
            match self.cut_point() {
                Some(c) => c,
                None => return Ok(String::new()),
            }
        };
        let out: String = self.rest.drain(..cut).collect();
        let separator =
            !all && self.preserve_separator && out.chars().last().is_some_and(char::is_whitespace);
        (self.normalize)(&out)
            .map(|mut spoken| {
                if separator
                    && !spoken.is_empty()
                    && !spoken.chars().last().is_some_and(char::is_whitespace)
                {
                    spoken.push(' ');
                }
                spoken
            })
            .map_err(|e| (out, e))
    }

    /// Text received but not released yet.
    pub fn pending(&self) -> &str {
        &self.rest
    }

    fn language_ranges(&self, text: &str) -> Option<Vec<(usize, usize)>> {
        if self.english_spans {
            super::spans::stream_ranges(text).ok()
        } else {
            Some(Vec::new())
        }
    }

    fn cut_point(&self) -> Option<usize> {
        let rest = self.rest.as_str();
        // Holding back the last characters is not enough: an abbreviation can lie across the cut. So all
        // abbreviation spans are marked first, and a sentence end only counts if its period belongs to none.
        let mut protected: Vec<(usize, usize)> = Vec::new();
        for rule in RULES.iter().filter(|r| r.may_match(rest)) {
            protected.extend(
                rule.rx
                    .find_iter(rest)
                    .ok()?
                    .iter()
                    .map(|m| (m.start(), m.end())),
            );
        }
        // a period after a number is mostly no sentence end either: list number, ordinal ("am 3. Oktober")
        for rx in [&LIST_PERIOD, &NUMBER_PERIOD] {
            protected.extend(
                rx.find_iter(rest)
                    .ok()?
                    .iter()
                    .map(|m| (m.start(), m.end())),
            );
        }
        // never cut formula blocks; a block still open protects everything to the end
        for (opening, closing) in [("$$", "$$"), ("\\[", "\\]")] {
            let mut i = 0;
            while let Some(a) = rest.get(i..).and_then(|s| s.find(opening)).map(|a| a + i) {
                let from = a + opening.len();
                let end = match rest[from..].find(closing) {
                    Some(b) => from + b + closing.len(),
                    None => rest.len() + 1,
                };
                protected.push((a, end));
                i = end;
            }
        }
        protected.extend(self.language_ranges(rest)?);
        // this close to the end something could still grow
        if rest.chars().count() < HOLD_BACK {
            return None;
        }
        let limit = back_chars(rest, rest.len(), HOLD_BACK);
        let mut cut = None;
        for m in SENTENCE_END.find_iter(rest).ok()? {
            if m.end() > limit {
                break;
            }
            if protected
                .iter()
                .any(|&(a, b)| a <= m.start() && m.start() < b)
            {
                continue; // the period belongs to an abbreviation
            }
            cut = Some(m.end());
        }
        cut.filter(|&c| c > 0)
    }
}
