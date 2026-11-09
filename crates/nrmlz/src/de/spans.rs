//! Explicit language spans; no automatic language guessing. Balanced non-nested [en]...[/en] spans are
//! normalised by the English pipeline and protected from the German stages. Tags are retained for TTS
//! engines' language routing. Invalid/nested tags are left untouched by this stage.

use crate::protected::ProtectedText;
use crate::pyre::{re, LazyRe, R};

static EN_SPAN: LazyRe = re!(r"\[en\]((?:(?!\[/?en\]).)*?)\[/en\]", "is");
static EN_OPEN: LazyRe = re!(r"\[en\]", "i");

pub(crate) fn protect(text: &str) -> R<(String, ProtectedText)> {
    let mut protected = ProtectedText::new(text);
    let matched = EN_SPAN.find_iter(text)?.len();
    if matched != EN_OPEN.find_iter(text)?.len()
        || matched != text.to_ascii_lowercase().matches("[/en]").count()
    {
        return Ok((text.to_string(), protected));
    }
    let out = EN_SPAN.sub_fn(text, |m| {
        // Already-normalised text stays in its explicitly selected language, tags included.
        let spoken = crate::en::try_normalize(m.g(1))?;
        Ok(protected.insert(format!("[en]{spoken}[/en]")))
    })?;
    Ok((out, protected))
}

/// Protect complete and still-open English blocks from the stream's sentence cutter.
pub(crate) fn stream_ranges(text: &str) -> R<Vec<(usize, usize)>> {
    let lower = text.to_ascii_lowercase(); // ASCII-only case conversion keeps UTF-8 byte offsets
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(open) = EN_OPEN.search(&text[from..])? {
        let start = from + open.start();
        let opening_end = from + open.end();
        let end = lower[opening_end..]
            .find("[/en]")
            .map_or(text.len() + 1, |i| opening_end + i + "[/en]".len());
        ranges.push((start, end));
        if end > text.len() {
            break;
        }
        from = end;
    }
    Ok(ranges)
}
