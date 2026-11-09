//! Turns the German rule tables (data/de/*.json, unchanged from the Articulino frontend) into Rust statics.
//! Key order is preserved: the Python frontend sorts by length with a stable sort, so ties keep file order.

use std::{env, fs, path::Path};

use serde_json::Value;

fn lit(s: &str) -> String {
    format!("{s:?}")
}

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("data/de");
    let abbr_path = dir.join("abbreviations.json");
    let extra_path = dir.join("abbreviations-nrmlz.json");
    println!("cargo:rerun-if-changed={}", extra_path.display());
    let units_path = dir.join("units.json");
    println!("cargo:rerun-if-changed={}", abbr_path.display());
    println!("cargo:rerun-if-changed={}", units_path.display());

    let abbr: Value = serde_json::from_str(&fs::read_to_string(&abbr_path).unwrap()).unwrap();
    let units: Value = serde_json::from_str(&fs::read_to_string(&units_path).unwrap()).unwrap();

    let mut out = String::new();
    for (name, key) in [("ABBREVIATIONS", "abbreviations"), ("SYMBOLS", "symbols")] {
        out += &format!("pub static {name}: &[(&str, &str)] = &[\n");
        for (k, v) in abbr[key].as_object().unwrap() {
            out += &format!("    ({}, {}),\n", lit(k), lit(v.as_str().unwrap()));
        }
        out += "];\n";
    }
    let extra: Value = serde_json::from_str(&fs::read_to_string(&extra_path).unwrap()).unwrap();
    for (name, key) in [
        ("NRMLZ_ABBREVIATIONS", "abbreviations"),
        ("NRMLZ_BEFORE_NAME", "before_name"),
        ("NRMLZ_BEFORE_NUMBER", "before_number"),
    ] {
        out += &format!("pub static {name}: &[(&str, &str)] = &[\n");
        for (k, v) in extra[key].as_object().unwrap() {
            out += &format!("    ({}, {}),\n", lit(k), lit(v.as_str().unwrap()));
        }
        out += "];\n";
    }
    out += "/// (unit, singular, plural, gender, space required)\n";
    out += "pub static UNITS: &[(&str, &str, &str, char, bool)] = &[\n";
    for (k, v) in units["units"].as_object().unwrap() {
        let v = v.as_array().unwrap();
        let gender = v[2].as_str().unwrap().chars().next().unwrap();
        let space = v[3].as_i64().unwrap() != 0;
        out += &format!(
            "    ({}, {}, {}, {:?}, {}),\n",
            lit(k),
            lit(v[0].as_str().unwrap()),
            lit(v[1].as_str().unwrap()),
            gender,
            space
        );
    }
    out += "];\n";
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("de_tables.rs"),
        out,
    )
    .unwrap();
    english_tables();
}

fn english_tables() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("data/en");
    let read = |file: &str| -> Value {
        let path = dir.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    };
    let units = read("units.json");
    let abbr = read("abbreviations.json");
    let mut out = String::from("pub static UNITS: &[(&str, &str, &str, bool)] = &[\n");
    for (k, v) in units["units"].as_object().unwrap() {
        out += &format!(
            "({}, {}, {}, {}),\n",
            lit(k),
            lit(v[0].as_str().unwrap()),
            lit(v[1].as_str().unwrap()),
            v[2].as_i64().unwrap() != 0
        );
    }
    out += "];\n";
    for (name, key) in [
        ("ABBREVIATIONS", "abbreviations"),
        ("BEFORE_NAME", "before_name"),
        ("AFTER_NAME", "after_name"),
        ("BEFORE_NUMBER", "before_number"),
        ("SYMBOLS", "symbols"),
    ] {
        out += &format!("pub static {name}: &[(&str, &str)] = &[\n");
        for (k, v) in abbr[key].as_object().unwrap() {
            out += &format!("({}, {}),\n", lit(k), lit(v.as_str().unwrap()));
        }
        out += "];\n";
    }
    for (name, key) in [
        ("ACRONYM_WORDS", "acronym_words"),
        ("ALWAYS_SPELLED", "always_spelled"),
    ] {
        out += &format!("pub static {name}: &[&str] = &[\n");
        for v in abbr[key].as_array().unwrap() {
            out += &format!("{},\n", lit(v.as_str().unwrap()));
        }
        out += "];\n";
    }
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("en_tables.rs"),
        out,
    )
    .unwrap();
}
