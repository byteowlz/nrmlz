//! WebAssembly bindings: `normalize(lang, text)`, a lexicon, and a streaming normaliser.
//!
//! ```js
//! import { normalize, Lexicon, Stream } from "./pkg/nrmlz_wasm.js";
//! normalize("de", "Am 3. Mai um 15:30 Uhr");      // "Am dritten Mai um fünfzehn Uhr dreißig"
//! const lex = Lexicon.fromJson('{"terms": {"OK": "Okay"}}');
//! lex.normalize("de", "OK, 5 km/h");
//! ```

use wasm_bindgen::prelude::*;

fn lang(l: &str) -> Result<nrmlz::Lang, JsError> {
    l.parse::<nrmlz::Lang>()
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Normalise `text` in `lang` ("de" or "en"), using the nrmlz profile.
#[wasm_bindgen]
pub fn normalize(lang_code: &str, text: &str) -> Result<String, JsError> {
    Ok(nrmlz::normalize(lang(lang_code)?, text))
}

/// Normalise with explicit German compatibility options. English uses its own US conventions; the profile
/// affects shared lexicon precedence but does not emulate an English Articulino frontend.
#[wasm_bindgen(js_name = normalizeWith)]
pub fn normalize_with(
    lang_code: &str,
    text: &str,
    profile: &str,
    decimal_pause: Option<bool>,
    respellings: Option<bool>,
) -> Result<String, JsError> {
    let mut p: nrmlz::Profile = profile
        .parse()
        .map_err(|e: nrmlz::Error| JsError::new(&e.to_string()))?;
    if let Some(on) = decimal_pause {
        p = p.with_decimal_pause(on);
    }
    if let Some(on) = respellings {
        p = p.with_respellings(on);
    }
    Ok(nrmlz::normalize_with(lang(lang_code)?, text, p, None))
}

/// Compile every pattern now instead of on the first call.
#[wasm_bindgen(js_name = warmUp)]
pub fn warm_up() {
    nrmlz::de::warm_up();
    nrmlz::en::warm_up();
}

/// Library version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// A user lexicon (JSON with the same structure as the TOML format, or TSV).
#[wasm_bindgen]
pub struct Lexicon {
    inner: nrmlz::Lexicon,
}

#[wasm_bindgen]
impl Lexicon {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Lexicon {
        Lexicon {
            inner: nrmlz::Lexicon::new(),
        }
    }

    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<Lexicon, JsError> {
        nrmlz::Lexicon::from_json(json)
            .map(|inner| Lexicon { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen(js_name = fromTsv)]
    pub fn from_tsv(tsv: &str) -> Result<Lexicon, JsError> {
        nrmlz::Lexicon::from_tsv(tsv)
            .map(|inner| Lexicon { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Add a term (case-sensitive unless `caseSensitive` is false).
    pub fn insert(&mut self, term: &str, replacement: &str, case_sensitive: Option<bool>) {
        self.inner.insert_entry(nrmlz::lexicon::LexiconEntry {
            term: term.into(),
            replacement: replacement.into(),
            case_sensitive: case_sensitive.unwrap_or(true),
        });
    }

    /// "before" (default) or "after" normalisation.
    #[wasm_bindgen(js_name = setStage)]
    pub fn set_stage(&mut self, stage: &str) -> Result<(), JsError> {
        self.inner.set_stage(
            stage
                .parse()
                .map_err(|e: nrmlz::Error| JsError::new(&e.to_string()))?,
        );
        Ok(())
    }

    /// Normalise using a chosen profile, preserving final lexicon outputs in the nrmlz profile.
    #[wasm_bindgen(js_name = normalizeWith)]
    pub fn normalize_with(
        &self,
        lang_code: &str,
        text: &str,
        profile: &str,
    ) -> Result<String, JsError> {
        let p = profile
            .parse()
            .map_err(|e: nrmlz::Error| JsError::new(&e.to_string()))?;
        Ok(nrmlz::normalize_with(
            lang(lang_code)?,
            text,
            p,
            Some(&self.inner),
        ))
    }

    /// Replace the terms only.
    pub fn apply(&self, text: &str) -> String {
        self.inner.apply(text)
    }

    /// Normalise with this lexicon at its stage.
    pub fn normalize(&self, lang_code: &str, text: &str) -> Result<String, JsError> {
        Ok(nrmlz::normalize_with_lexicon(
            lang(lang_code)?,
            text,
            &self.inner,
        ))
    }
}

impl Default for Lexicon {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming German normaliser: `push` returns whole sentences as soon as they are complete.
#[wasm_bindgen]
pub struct Stream {
    inner: nrmlz::de::StreamNormalizer,
}

#[wasm_bindgen]
impl Stream {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Stream {
        Stream {
            inner: nrmlz::de::StreamNormalizer::new(),
        }
    }

    #[wasm_bindgen(js_name = withProfile)]
    pub fn with_profile(profile: &str) -> Result<Stream, JsError> {
        let p = profile
            .parse()
            .map_err(|e: nrmlz::Error| JsError::new(&e.to_string()))?;
        Ok(Stream {
            inner: nrmlz::de::StreamNormalizer::with_profile(p),
        })
    }

    pub fn push(&mut self, text: &str) -> String {
        self.inner.push(text)
    }

    pub fn flush(&mut self) -> String {
        self.inner.flush("")
    }
}

impl Default for Stream {
    fn default() -> Self {
        Self::new()
    }
}
