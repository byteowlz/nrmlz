//! English throughput on the independent gold inputs, not on PolyNorm.
use std::time::Instant;

fn main() {
    let t = Instant::now();
    nrmlz::en::warm_up();
    println!(
        "English warm-up: {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let raw = include_str!("../../../tests/gold/en.tsv");
    let inputs: Vec<&str> = raw
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(1))
        .collect();
    let rounds = 20;
    let t = Instant::now();
    for _ in 0..rounds {
        for text in &inputs {
            std::hint::black_box(nrmlz::normalize_en(text));
        }
    }
    let total = t.elapsed().as_secs_f64();
    let chars: usize = inputs.iter().map(|t| t.chars().count()).sum();
    println!(
        "English: {} texts x {rounds}, {:.3} ms/text, {:.2} MB/s",
        inputs.len(),
        total * 1000.0 / (inputs.len() * rounds) as f64,
        (chars * rounds) as f64 / total / 1e6
    );
}
