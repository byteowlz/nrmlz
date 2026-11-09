//! Byte identity with the pinned Python frontend on the committed corpora, without Python: the expected
//! outputs (`tests/corpus/*.expected.jsonl`) were written by the reference (`just expected`).

use std::path::Path;

use nrmlz::de::{try_normalize_with, Profile, StreamNormalizer};

fn try_normalize(t: &str) -> Result<String, nrmlz::Error> {
    try_normalize_with(t, Profile::REFERENCE)
}
use serde_json::Value;

fn load(name: &str) -> Vec<(Value, Value)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let inputs = std::fs::read_to_string(dir.join(format!("{name}.jsonl"))).unwrap();
    let expected = std::fs::read_to_string(dir.join(format!("{name}.expected.jsonl"))).unwrap();
    let mut exp = expected.lines();
    let meta: Value = serde_json::from_str(exp.next().unwrap()).unwrap();
    assert_eq!(
        meta["reference"]["german_py_sha256"],
        "5b95a317521bfdf865c580bc85f20b7ecf4ac07381c6e9f1040ba6e51da100f5",
        "expected outputs come from another frontend version"
    );
    inputs
        .lines()
        .zip(exp)
        .map(|(i, e)| {
            (
                serde_json::from_str(i).unwrap(),
                serde_json::from_str(e).unwrap(),
            )
        })
        .collect()
}

fn as_value(r: Result<String, nrmlz::Error>) -> (Value, Value) {
    match r {
        Ok(s) => (Value::String(s), Value::Null),
        Err(nrmlz::Error::NumberTooLarge) => (Value::Null, Value::String("OverflowError".into())),
        Err(e) => panic!("unexpected error {e}"),
    }
}

fn check(name: &str) {
    let cases = load(name);
    let mut failures = Vec::new();
    let (mut idempotent, mut checked) = (0, 0);
    for (input, exp) in &cases {
        let text = input["text"].as_str().unwrap();
        let (out, err) = as_value(try_normalize(text));
        let e_out = exp.get("out").cloned().unwrap_or(Value::Null);
        let e_err = exp.get("error").cloned().unwrap_or(Value::Null);
        if out != e_out || err != e_err {
            failures.push(format!(
                "{}: {text:?}\n  expected {e_out} {e_err}\n  got      {out} {err}",
                exp["id"]
            ));
            continue;
        }
        if let Some(o) = out.as_str() {
            let (out2, err2) = as_value(try_normalize(o));
            let e2 = (
                exp.get("out2").cloned().unwrap_or(Value::Null),
                exp.get("error2").cloned().unwrap_or(Value::Null),
            );
            if (out2.clone(), err2) != e2 {
                failures.push(format!("{} second pass: {o:?}", exp["id"]));
            }
            // idempotence wherever the Python frontend is idempotent
            if exp.get("out2") == Some(&out) {
                checked += 1;
                if out2 == out {
                    idempotent += 1;
                }
            }
        }
        // streaming in chunks of 7 characters, cut points included
        let mut s = StreamNormalizer::with_profile(Profile::REFERENCE);
        let chars: Vec<char> = text.chars().collect();
        let mut parts = Vec::new();
        let mut stream_err = Value::Null;
        for chunk in chars.chunks(7) {
            match s.try_push(&chunk.iter().collect::<String>()) {
                Ok(p) => parts.push(p),
                Err(_) => stream_err = Value::String("OverflowError".into()),
            }
            if !stream_err.is_null() {
                break;
            }
        }
        if stream_err.is_null() {
            match s.try_flush("") {
                Ok(p) => parts.push(p),
                Err(_) => stream_err = Value::String("OverflowError".into()),
            }
        }
        let streamed = if stream_err.is_null() {
            Value::String(parts.join("␞"))
        } else {
            Value::Null
        };
        if streamed != exp["stream7"]
            || stream_err != exp.get("stream7_error").cloned().unwrap_or(Value::Null)
        {
            failures.push(format!(
                "{} stream: {text:?}\n  expected {}\n  got      {streamed}",
                exp["id"], exp["stream7"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    assert_eq!(
        idempotent, checked,
        "idempotence where Python is idempotent"
    );
    eprintln!(
        "{name}: {} cases identical, idempotent on {checked}",
        cases.len()
    );
}

#[test]
fn rules_corpus_matches_python() {
    check("rules-de");
}

#[test]
fn stress_corpus_matches_python() {
    check("stress-de");
}
