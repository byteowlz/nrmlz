//! Differential test: runs the pinned Python Articulino frontend and the Rust port on the same inputs and
//! requires byte-identical output. This is nrmlz's acceptance gate.
//!
//! ```text
//! difftest [--config difftest.local.toml] [--only NAME]... [--stream N] [--numbers] [--out target/difftest]
//! ```
//!
//! Local paths (the reference interpreter, private corpora) live in `difftest.local.toml`, never in tracked
//! files; see `difftest.example.toml`. The committed corpora under `tests/corpus/` always run.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use nrmlz::de::{Fix, Profile};
use serde::Deserialize;

/// The gate compares the frozen reference profile with Python; the nrmlz profile is checked by --deviations.
const REFERENCE: Profile = Profile::REFERENCE;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Config {
    /// Python interpreter of the reference environment (articulino_onnx and num2words installed).
    python: PathBuf,
    #[serde(default)]
    corpus: Vec<CorpusSpec>,
}

#[derive(Deserialize, Clone)]
struct CorpusSpec {
    name: String,
    /// One file or a glob over file names in one directory (`sentences-*.jsonl`).
    path: String,
    /// jsonl (one object per line), json (an array, see `pointer`), lines (one text per line)
    #[serde(default = "default_format")]
    format: String,
    /// Object key holding the text (jsonl, or json arrays of objects).
    #[serde(default)]
    field: Option<String>,
    /// JSON pointer to the array inside a json file ("" = the root).
    #[serde(default)]
    pointer: String,
}

fn default_format() -> String {
    "jsonl".into()
}

struct Item {
    corpus: String,
    text: String,
}

fn expand_path(path: &str) -> Vec<PathBuf> {
    let path = if let Some(rest) = path.strip_prefix("~/") {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
    } else {
        PathBuf::from(path)
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    if !name.contains('*') {
        return vec![path];
    }
    let (pre, post) = name.split_once('*').unwrap();
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    n.starts_with(pre) && n.ends_with(post) && n.len() >= pre.len() + post.len()
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn text_of(v: &Value, field: &Option<String>) -> Option<String> {
    match (v, field) {
        (Value::String(s), _) => Some(s.clone()),
        (Value::Object(o), Some(f)) => o.get(f).and_then(|t| t.as_str()).map(str::to_string),
        (Value::Object(o), None) => o.get("text").and_then(|t| t.as_str()).map(str::to_string),
        _ => None,
    }
}

fn load(spec: &CorpusSpec) -> Result<Vec<Item>, String> {
    let files = expand_path(&spec.path);
    if files.is_empty() {
        return Err(format!("{}: no files match {}", spec.name, spec.path));
    }
    let mut texts = Vec::new();
    for f in files {
        let raw = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        match spec.format.as_str() {
            "jsonl" => {
                for line in raw.lines().filter(|l| !l.trim().is_empty()) {
                    let v: Value =
                        serde_json::from_str(line).map_err(|e| format!("{}: {e}", f.display()))?;
                    texts.extend(text_of(&v, &spec.field));
                }
            }
            "json" => {
                let v: Value =
                    serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", f.display()))?;
                let arr = v
                    .pointer(&spec.pointer)
                    .and_then(|a| a.as_array())
                    .ok_or("pointer is no array")?;
                texts.extend(arr.iter().filter_map(|x| text_of(x, &spec.field)));
            }
            "gold-tsv" => {
                for line in raw.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    let columns: Vec<&str> = line.split('\t').collect();
                    if columns.len() != 4 {
                        return Err(format!("{}: malformed gold row", f.display()));
                    }
                    texts.push(columns[1].to_string());
                }
            }
            "lines" => texts.extend(raw.lines().filter(|l| !l.is_empty()).map(str::to_string)),
            other => return Err(format!("unknown format {other}")),
        }
    }
    let mut seen = std::collections::HashSet::new();
    Ok(texts
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .map(|text| Item {
            corpus: spec.name.clone(),
            text,
        })
        .collect())
}

/// Run the reference on `inputs` (JSON objects), return its metadata line and one result per input.
fn run_reference(python: &Path, args: &[&str], inputs: &[Value]) -> (Value, Vec<Value>) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("py/reference.py");
    let mut child = Command::new(python)
        .arg("-I")
        .arg(&script)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot start {}: {e}", python.display()));
    let mut stdin = child.stdin.take().unwrap();
    let payload: String = inputs.iter().map(|v| v.to_string() + "\n").collect();
    let writer = std::thread::spawn(move || {
        stdin.write_all(payload.as_bytes()).unwrap();
    });
    let reader = BufReader::new(child.stdout.take().unwrap());
    let mut lines = reader.lines();
    let meta: Value =
        serde_json::from_str(&lines.next().expect("reference wrote nothing").unwrap()).unwrap();
    assert_eq!(
        meta["reference"]["german_py_sha256"],
        "5b95a317521bfdf865c580bc85f20b7ecf4ac07381c6e9f1040ba6e51da100f5",
        "reference source changed"
    );
    assert_eq!(
        meta["reference"]["num2words"], "0.5.14",
        "reference dependency changed"
    );
    let results: Vec<Value> = lines
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect();
    writer.join().unwrap();
    assert!(child.wait().unwrap().success(), "reference failed");
    assert_eq!(
        results.len(),
        inputs.len(),
        "reference answered {} of {} inputs",
        results.len(),
        inputs.len()
    );
    (meta["reference"].clone(), results)
}

fn rust_result(r: Result<String, nrmlz::Error>) -> Value {
    match r {
        Ok(s) => json!({ "out": s }),
        // num2words raises OverflowError where nrmlz reports NumberTooLarge
        Err(nrmlz::Error::NumberTooLarge) => json!({ "error": "OverflowError" }),
        Err(e) => json!({ "error": format!("rust: {e}") }),
    }
}

fn same(py: &Value, rs: &Value, key: &str, err: &str) -> bool {
    py.get(key) == rs.get(key) && py.get(err) == rs.get(err)
}

fn streamed(text: &str, size: usize) -> Result<String, nrmlz::Error> {
    let mut s = nrmlz::de::StreamNormalizer::with_profile(REFERENCE);
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    for chunk in chars.chunks(size) {
        parts.push(s.try_push(&chunk.iter().collect::<String>())?);
    }
    parts.push(s.try_flush("")?);
    Ok(parts.join("␞"))
}

struct Args {
    config: PathBuf,
    only: Vec<String>,
    stream: usize,
    numbers: bool,
    write_expected: bool,
    deviations: bool,
    out: PathBuf,
}

fn parse_args() -> Args {
    let mut a = Args {
        config: PathBuf::from("difftest.local.toml"),
        only: vec![],
        stream: 0,
        numbers: false,
        write_expected: false,
        deviations: false,
        out: PathBuf::from("target/difftest"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--config" => a.config = it.next().expect("--config FILE").into(),
            "--only" => a.only.push(it.next().expect("--only NAME")),
            "--stream" => a.stream = it.next().expect("--stream N").parse().expect("--stream N"),
            "--numbers" => a.numbers = true,
            "--write-expected" => a.write_expected = true,
            "--deviations" => a.deviations = true,
            "--out" => a.out = it.next().expect("--out DIR").into(),
            "-h" | "--help" => {
                println!("difftest [--config FILE] [--only NAME]... [--stream N] [--numbers] [--write-expected] [--deviations] [--out DIR]");
                std::process::exit(0);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn number_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    let mut push = |n: String, kind: &str| {
        let id = cases.len();
        cases.push(json!({ "id": id, "n": n, "kind": kind }));
    };
    for n in 0..=1_000_000u64 {
        push(n.to_string(), "cardinal");
        push(n.to_string(), "ordinal");
    }
    for n in 1000..=2999u64 {
        push(n.to_string(), "year");
    }
    // powers of ten, their neighbours and pseudo-random big numbers up to past the limit (10^606)
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for len in 7..=610usize {
        for k in 0..24 {
            let mut s = String::new();
            for i in 0..len {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let d = match k {
                    0 => u64::from(i == 0),
                    1 => 9,
                    2 => u64::from(i == 0 || i == len - 1),
                    3..=8 => {
                        if i == 0 || x.is_multiple_of(k as u64) {
                            1 + (x % 9)
                        } else {
                            0
                        }
                    }
                    _ => {
                        if i == 0 {
                            1 + x % 9
                        } else {
                            x % 10
                        }
                    }
                };
                s.push(char::from(b'0' + d as u8));
            }
            push(s.clone(), "cardinal");
            push(s, "ordinal");
        }
    }
    cases
}

fn run_numbers(cfg: &Config, out_dir: &Path) -> bool {
    let cases = number_cases();
    eprintln!("numbers: {} cases", cases.len());
    let (meta, py) = run_reference(&cfg.python, &["--numbers"], &cases);
    let mut bad = Vec::new();
    for (c, p) in cases.iter().zip(&py) {
        let n = c["n"].as_str().unwrap();
        let digits: Vec<u8> = n.bytes().map(|b| b - b'0').collect();
        let r = match c["kind"].as_str().unwrap() {
            "cardinal" => nrmlz::num::de::cardinal_digits(&digits),
            "ordinal" => nrmlz::num::de::ordinal_digits(&digits),
            _ => Ok(nrmlz::num::de::year(n.parse().unwrap())),
        };
        let r = rust_result(r);
        if !same(p, &r, "out", "error") {
            bad.push(json!({ "case": c, "python": p, "rust": r }));
        }
    }
    std::fs::create_dir_all(out_dir).unwrap();
    let mut f = std::fs::File::create(out_dir.join("numbers-diffs.jsonl")).unwrap();
    for b in &bad {
        writeln!(f, "{b}").unwrap();
    }
    println!(
        "numbers: {} cases, {} differences (reference {meta})",
        cases.len(),
        bad.len()
    );
    for b in bad.iter().take(10) {
        println!("  {b}");
    }
    bad.is_empty()
}

/// Store the reference outputs of the committed corpora next to them (`*.expected.jsonl`), so that
/// `cargo test` checks byte identity without Python: text mode (out, out2) and streaming in 7-character chunks.
fn write_expected(cfg: &Config) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    for name in ["rules-de", "stress-de"] {
        let path = root.join(format!("{name}.jsonl"));
        let inputs: Vec<Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let (meta, text) = run_reference(&cfg.python, &[], &inputs);
        let (_, stream) = run_reference(&cfg.python, &["--stream", "7"], &inputs);
        let mut out = format!("{}\n", json!({ "reference": meta }));
        for ((i, t), s) in inputs.iter().zip(&text).zip(&stream) {
            let mut e = json!({ "id": i["id"] });
            for k in ["out", "error", "out2", "error2"] {
                if let Some(v) = t.get(k) {
                    e[k] = v.clone();
                }
            }
            e["stream7"] = s.get("out").cloned().unwrap_or(Value::Null);
            if let Some(v) = s.get("error") {
                e["stream7_error"] = v.clone();
            }
            out += &format!("{e}\n");
        }
        std::fs::write(root.join(format!("{name}.expected.jsonl")), out).unwrap();
        println!("{name}: {} expected outputs", inputs.len());
    }
}

#[derive(Default)]
struct Counts {
    items: usize,
    identical: usize,
    second_pass_identical: usize,
    python_idempotent: usize,
    rust_idempotent_where_python: usize,
    errors_matching: usize,
    python_ms: f64,
    rust_ms: f64,
}

fn main() {
    let args = parse_args();
    let raw = std::fs::read_to_string(&args.config).unwrap_or_else(|e| {
        eprintln!("cannot read {}: {e}\ncopy difftest.example.toml to difftest.local.toml and fill in the paths", args.config.display());
        std::process::exit(2);
    });
    let cfg: Config = toml::from_str(&raw).expect("difftest config");
    if args.numbers {
        std::process::exit(if run_numbers(&cfg, &args.out) { 0 } else { 1 });
    }
    if args.write_expected {
        write_expected(&cfg);
        return;
    }
    if args.deviations {
        let items = load_all(&cfg, &args);
        std::process::exit(if report_deviations(&items, &args.out) {
            0
        } else {
            1
        });
    }

    let items = load_all(&cfg, &args);
    run_gate(&cfg, &args, &items);
}

fn load_all(cfg: &Config, args: &Args) -> Vec<Item> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut specs = vec![
        CorpusSpec {
            name: "rules".into(),
            path: root
                .join("tests/corpus/rules-de.jsonl")
                .display()
                .to_string(),
            format: "jsonl".into(),
            field: None,
            pointer: String::new(),
        },
        CorpusSpec {
            name: "stress".into(),
            path: root
                .join("tests/corpus/stress-de.jsonl")
                .display()
                .to_string(),
            format: "jsonl".into(),
            field: None,
            pointer: String::new(),
        },
    ];
    specs.push(CorpusSpec {
        name: "gold-de".into(),
        path: root.join("tests/gold/de.tsv").display().to_string(),
        format: "gold-tsv".into(),
        field: None,
        pointer: String::new(),
    });
    specs.extend(cfg.corpus.iter().cloned());
    if !args.only.is_empty() {
        specs.retain(|s| args.only.contains(&s.name));
    }
    let mut items = Vec::new();
    for s in &specs {
        match load(s) {
            Ok(v) => {
                eprintln!("{}: {} texts", s.name, v.len());
                items.extend(v);
            }
            Err(e) => panic!("required corpus could not be loaded: {e}"),
        }
    }
    assert!(
        !items.is_empty(),
        "no inputs selected; refusing a vacuous pass"
    );
    items
}

/// Every difference between the nrmlz and the reference profile, attributed to the fixes that cause it: a
/// fix is responsible if turning only it off changes the nrmlz output. A difference no single fix explains
/// fails the check.
fn report_deviations(items: &[Item], out: &Path) -> bool {
    let full = Profile::NRMLZ;
    let mut per: BTreeMap<String, BTreeMap<&'static str, usize>> = BTreeMap::new();
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut examples: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for it in items {
        let t = totals.entry(it.corpus.clone()).or_default();
        t.0 += 1;
        let reference = nrmlz::de::normalize_with(&it.text, Profile::REFERENCE);
        let nrmlz_out = nrmlz::de::normalize_with(&it.text, full);
        if reference == nrmlz_out {
            continue;
        }
        t.1 += 1;
        let responsible: Vec<Fix> = Fix::ALL
            .iter()
            .copied()
            .filter(|&f| nrmlz::de::normalize_with(&it.text, full.without(f)) != nrmlz_out)
            .collect();
        if responsible.is_empty() {
            unexplained.push(json!({ "corpus": it.corpus, "input": it.text, "reference": reference, "nrmlz": nrmlz_out }));
        }
        for f in responsible {
            *per.entry(it.corpus.clone())
                .or_default()
                .entry(f.name())
                .or_default() += 1;
            let ex = examples.entry(f.name()).or_default();
            // examples only from the committed corpora (local corpora stay local)
            if ex.len() < 5 && matches!(it.corpus.as_str(), "rules" | "stress" | "gold-de") {
                ex.push(json!({ "input": it.text, "reference": reference, "nrmlz": nrmlz_out }));
            }
        }
    }
    let mut report = String::from(
        "# Deviations of the nrmlz profile from the reference\n\n| corpus | texts | differ | ",
    );
    for f in Fix::ALL {
        report += &format!("{} | ", f.name());
    }
    report += "\n|---|---|---|";
    report += &"---|".repeat(Fix::ALL.len());
    report += "\n";
    for (corpus, (n, d)) in &totals {
        report += &format!("| {corpus} | {n} | {d} | ");
        for f in Fix::ALL {
            report += &format!(
                "{} | ",
                per.get(corpus)
                    .and_then(|m| m.get(f.name()))
                    .copied()
                    .unwrap_or(0)
            );
        }
        report += "\n";
    }
    report += &format!("\nUnexplained differences: {}\n", unexplained.len());
    std::fs::create_dir_all(out.join("deviations")).unwrap();
    std::fs::write(out.join("deviations/report.md"), &report).unwrap();
    let mut f = std::fs::File::create(out.join("deviations/examples.jsonl")).unwrap();
    for (fix, ex) in &examples {
        for e in ex {
            writeln!(f, "{}", json!({ "fix": fix, "example": e })).unwrap();
        }
    }
    for u in &unexplained {
        writeln!(f, "{}", json!({ "unexplained": u })).unwrap();
    }
    println!("{report}");
    unexplained.is_empty()
}

fn run_gate(cfg: &Config, args: &Args, items: &[Item]) {
    let inputs: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, it)| json!({ "id": i, "text": it.text }))
        .collect();
    let stream_arg = args.stream.to_string();
    let py_args: Vec<&str> = if args.stream > 0 {
        vec!["--stream", &stream_arg]
    } else {
        vec![]
    };
    let t = Instant::now();
    let (meta, py) = run_reference(&cfg.python, &py_args, &inputs);
    let py_wall = t.elapsed().as_secs_f64();
    let mut counts: BTreeMap<String, Counts> = BTreeMap::new();

    nrmlz::de::warm_up();
    let mut diffs = Vec::new();
    for (it, p) in items.iter().zip(&py) {
        let c = counts.entry(it.corpus.clone()).or_default();
        c.items += 1;
        c.python_ms += p["ms"].as_f64().unwrap_or(0.0);
        let t = Instant::now();
        let r = if args.stream > 0 {
            rust_result(streamed(&it.text, args.stream))
        } else {
            let mut r = rust_result(nrmlz::de::try_normalize_with(&it.text, REFERENCE));
            if let Some(out) = r.get("out").and_then(|o| o.as_str()).map(str::to_string) {
                let r2 = rust_result(nrmlz::de::try_normalize_with(&out, REFERENCE));
                if let Some(o2) = r2.get("out") {
                    r["out2"] = o2.clone();
                }
                if let Some(e2) = r2.get("error") {
                    r["error2"] = e2.clone();
                }
            }
            r
        };
        c.rust_ms += t.elapsed().as_secs_f64() * 1000.0;
        let first = same(p, &r, "out", "error");
        if first {
            c.identical += 1;
            if p.get("error").is_some() {
                c.errors_matching += 1;
            }
        }
        if args.stream == 0 && first {
            if same(p, &r, "out2", "error2") {
                c.second_pass_identical += 1;
            }
            if p.get("out").is_some() && p.get("out") == p.get("out2") {
                c.python_idempotent += 1;
                if r.get("out") == r.get("out2") {
                    c.rust_idempotent_where_python += 1;
                }
            }
        }
        if !first || (args.stream == 0 && !same(p, &r, "out2", "error2")) {
            diffs.push(json!({ "corpus": it.corpus, "input": it.text, "python": p, "rust": r }));
        }
    }

    let mode = if args.stream > 0 {
        format!("stream-{}", args.stream)
    } else {
        "text".into()
    };
    let dir = args.out.join(&mode);
    std::fs::create_dir_all(&dir).unwrap();
    let mut f = std::fs::File::create(dir.join("diffs.jsonl")).unwrap();
    for d in &diffs {
        writeln!(f, "{d}").unwrap();
    }
    let mut report = format!(
        "# difftest ({mode})\n\nReference: {meta}\nnrmlz {}\n\n| corpus | texts | identical | 2nd pass identical | Python idempotent | Rust idempotent there | matching errors | Python ms/text | Rust ms/text |\n|---|---|---|---|---|---|---|---|---|\n",
        env!("CARGO_PKG_VERSION")
    );
    let mut total = Counts::default();
    for (name, c) in &counts {
        report += &format!(
            "| {name} | {} | {} | {} | {} | {} | {} | {:.3} | {:.3} |\n",
            c.items,
            c.identical,
            c.second_pass_identical,
            c.python_idempotent,
            c.rust_idempotent_where_python,
            c.errors_matching,
            c.python_ms / c.items.max(1) as f64,
            c.rust_ms / c.items.max(1) as f64
        );
        total.items += c.items;
        total.identical += c.identical;
        total.second_pass_identical += c.second_pass_identical;
        total.python_idempotent += c.python_idempotent;
        total.rust_idempotent_where_python += c.rust_idempotent_where_python;
        total.errors_matching += c.errors_matching;
        total.python_ms += c.python_ms;
        total.rust_ms += c.rust_ms;
    }
    report += &format!(
        "| **total** | {} | {} | {} | {} | {} | {} | {:.3} | {:.3} |\n\nDifferences: {} (see diffs.jsonl). Python process wall time {py_wall:.1} s.\n",
        total.items,
        total.identical,
        total.second_pass_identical,
        total.python_idempotent,
        total.rust_idempotent_where_python,
        total.errors_matching,
        total.python_ms / total.items.max(1) as f64,
        total.rust_ms / total.items.max(1) as f64,
        diffs.len()
    );
    std::fs::write(dir.join("report.md"), &report).unwrap();
    println!("{report}");
    for d in diffs.iter().take(15) {
        println!(
            "--- {}\n  in:  {:?}\n  py:  {}\n  rs:  {}",
            d["corpus"], d["input"], d["python"], d["rust"]
        );
    }
    #[cfg(feature = "coverage")]
    {
        let snap = nrmlz::coverage::snapshot();
        let zero: Vec<&(String, u64)> = snap.iter().filter(|(_, n)| *n == 0).collect();
        let mut cov = format!(
            "# rule coverage ({mode})\n\n{} patterns compiled, {} never matched\n\n",
            snap.len(),
            zero.len()
        );
        for (p, n) in &snap {
            cov += &format!("{n:>8}  {p}\n");
        }
        std::fs::write(dir.join("coverage.txt"), &cov).unwrap();
        println!(
            "coverage: {} patterns compiled, {} never matched",
            snap.len(),
            zero.len()
        );
        for (p, _) in zero {
            println!("  never matched: {p}");
        }
    }
    std::process::exit(if diffs.is_empty() { 0 } else { 1 });
}
