//! Smoke tests of the `nrmlz` binary (with a throw-away config directory).

use std::io::Write;
use std::process::{Command, Stdio};

fn nrmlz(args: &[&str], stdin: &str) -> (String, bool) {
    let dir = std::env::temp_dir().join(format!("nrmlz-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_nrmlz"))
        .args(args)
        .env("XDG_CONFIG_HOME", &dir)
        .env_remove("NRMLZ_LEXICON")
        .env_remove("NRMLZ_LEXICON_STAGE")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8(out.stdout).unwrap(), out.status.success())
}

#[test]
fn argument_and_lines() {
    assert_eq!(
        nrmlz(&["de", "Am 3. Mai um 15:30 Uhr"], "").0,
        "Am dritten Mai um fünfzehn Uhr dreißig.\n"
    );
    assert_eq!(
        nrmlz(&["de"], "3 kg\n1 h\n").0,
        "drei Kilogramm.\neine Stunde.\n"
    );
}

#[test]
fn whole_jsonl_and_stream() {
    assert_eq!(
        nrmlz(&["de", "--whole"], "# Titel\n\n- 2 Punkte\n").0,
        "Titel. zwei Punkte.\n"
    );
    let (out, ok) = nrmlz(&["de", "--jsonl"], "{\"text\": \"5 km/h\", \"id\": 7}\n");
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["normalized"], "fünf Kilometer pro Stunde.");
    assert_eq!(v["id"], 7);
    let (out, _) = nrmlz(
        &["de", "--stream"],
        "Er kommt am 3. Mai an. Dann z. B. weiter und noch etwas mehr",
    );
    assert_eq!(
        out,
        "Er kommt am dritten Mai an. \nDann zum Beispiel weiter und noch etwas mehr.\n"
    );
}

#[test]
fn english_and_reference_profiles() {
    assert_eq!(
        nrmlz(&["en", "It costs $5.50."], "").0,
        "It costs five dollars and fifty cents.\n"
    );
    assert_eq!(
        nrmlz(&["de", "--profile", "reference", "3,5 kg."], "").0,
        "drei Komma, fünf Kilogramm.\n"
    );
    assert_eq!(
        nrmlz(&["de", "3,5 kg."], "").0,
        "drei Komma fünf Kilogramm.\n"
    );
    assert_eq!(
        nrmlz(&["de", "--decimal-pause", "3,5 kg."], "").0,
        "drei Komma, fünf Kilogramm.\n"
    );
    assert!(!nrmlz(&["en", "--stream"], "hello").1);
}

#[test]
fn lexicon_file_and_stage() {
    let dir = std::env::temp_dir().join(format!("nrmlz-lex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lex = dir.join("terms.toml");
    std::fs::write(&lex, "[terms]\nKubernetes = \"Kubernetis\"\n").unwrap();
    let lex = lex.to_str().unwrap();
    assert_eq!(
        nrmlz(&["de", "--lexicon", lex, "Kubernetes auf 3 Knoten"], "").0,
        "Kubernetis auf drei Knoten.\n"
    );
    assert_eq!(
        nrmlz(
            &[
                "de",
                "--lexicon",
                lex,
                "--lexicon-stage",
                "after",
                "Kubernetes 2"
            ],
            ""
        )
        .0,
        "Kubernetis zwei.\n"
    );
    let (_, ok) = nrmlz(&["de", "--lexicon", "/nonexistent/terms.toml", "x"], "");
    assert!(!ok);
}
