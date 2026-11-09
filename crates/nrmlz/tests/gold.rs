//! Quality gate of the nrmlz profile: the hand-written gold sets (`tests/gold/*.tsv`, expected outputs with a
//! justification per line). Lines of a `gap:` class are known gaps: reported, not failed, and flagged when they
//! start to pass.

use std::collections::BTreeMap;
use std::path::Path;

fn check(lang: nrmlz::Lang, file: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/gold")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut per_class: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut failures, mut fixed_gaps, mut open_gaps) = (Vec::new(), Vec::new(), 0);
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            cols.len(),
            4,
            "{file}:{}: class, input, expected, justification",
            n + 1
        );
        let (class, input, expected) = (cols[0], cols[1], cols[2]);
        assert!(
            !cols[3].trim().is_empty(),
            "{file}:{}: no justification",
            n + 1
        );
        let got = nrmlz::normalize(lang, input);
        let gap = class.starts_with("gap:");
        let e = per_class
            .entry(class.trim_start_matches("gap:").to_string())
            .or_default();
        e.0 += 1;
        match (got == expected, gap) {
            (true, false) => e.1 += 1,
            (true, true) => fixed_gaps.push(format!(
                "{file}:{}: {input:?} passes now; drop the gap: marker",
                n + 1
            )),
            (false, true) => open_gaps += 1,
            (false, false) => failures.push(format!(
                "{file}:{}: {input:?}\n  expected {expected:?}\n  got      {got:?}",
                n + 1
            )),
        }
    }
    let total: usize = per_class.values().map(|v| v.0).sum();
    eprintln!(
        "{file}: {total} lines, {} failures, {open_gaps} known gaps",
        failures.len()
    );
    for (c, (n, ok)) in &per_class {
        eprintln!("  {c:<14} {ok}/{n}");
    }
    assert!(
        failures.is_empty(),
        "{} gold lines fail:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(fixed_gaps.is_empty(), "{}", fixed_gaps.join("\n"));
}

#[test]
fn german_gold_set() {
    check(nrmlz::Lang::De, "de.tsv");
}

#[test]
fn english_gold_set() {
    check(nrmlz::Lang::En, "en.tsv");
}
