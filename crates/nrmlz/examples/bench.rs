//! Throughput of the German normaliser, overall and per stage, on a JSON-lines corpus.
//!
//!     cargo run --release -p nrmlz --example bench [-- tests/corpus/stress-de.jsonl ...]

use std::time::{Duration, Instant};

use nrmlz::de;

type Stage = fn(&str, de::Profile) -> Result<String, nrmlz::Error>;

fn main() {
    let mut files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        files = vec![
            "tests/corpus/rules-de.jsonl".into(),
            "tests/corpus/stress-de.jsonl".into(),
        ];
    }
    let mut texts = Vec::new();
    for f in &files {
        for line in std::fs::read_to_string(f).expect("corpus").lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                texts.push(t.to_string());
            }
        }
    }
    let chars: usize = texts.iter().map(|t| t.chars().count()).sum();
    let t = Instant::now();
    de::warm_up();
    println!(
        "warm-up (pattern compilation): {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );

    let stages: [(&str, Stage); 7] = [
        ("phone_numbers", de::phone_numbers),
        ("clean_markup", de::clean_markup),
        ("expand_units", de::expand_units),
        ("math_symbols", de::math_symbols),
        ("numbers", de::numbers),
        ("expand_abbreviations", de::expand_abbreviations),
        ("spell_out_letters", de::spell_out_letters),
    ];
    let rounds = 3;
    let mut per_stage = vec![Duration::ZERO; stages.len()];
    for _ in 0..rounds {
        for text in &texts {
            let mut s = text.clone();
            for (i, (_, f)) in stages.iter().enumerate() {
                let t = Instant::now();
                s = f(&s, de::Profile::REFERENCE).unwrap_or_default();
                per_stage[i] += t.elapsed();
            }
        }
    }
    let n = (texts.len() * rounds) as f64;
    for (name, profile) in [
        ("nrmlz", de::Profile::NRMLZ),
        ("reference", de::Profile::REFERENCE),
    ] {
        let t = Instant::now();
        for _ in 0..rounds {
            for text in &texts {
                std::hint::black_box(de::normalize_with(text, profile));
            }
        }
        let total = t.elapsed().as_secs_f64();
        println!(
            "{name:<9} {} texts ({} chars) x {rounds}: {:.3} ms per text, {:.2} MB/s",
            texts.len(),
            chars,
            total * 1000.0 / n,
            (chars * rounds) as f64 / total / 1e6
        );
    }
    println!("reference stages:");
    for (i, (name, _)) in stages.iter().enumerate() {
        println!(
            "  {name:<22} {:.3} ms per text",
            per_stage[i].as_secs_f64() * 1000.0 / n
        );
    }
    #[cfg(feature = "coverage")]
    for (p, d) in nrmlz::coverage::timings().iter().take(25) {
        println!(
            "{:>9.1} ms  {}",
            d.as_secs_f64() * 1000.0,
            &p[..p.len().min(110)]
        );
    }
}
