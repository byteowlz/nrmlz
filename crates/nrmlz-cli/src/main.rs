//! nrmlz command line: normalise text for speech synthesis.
//!
//! ```text
//! nrmlz de "Am 3. Mai um 15:30 Uhr"        # arguments
//! nrmlz de < sentences.txt                 # stdin, one text per line
//! nrmlz de --whole < answer.md             # stdin as one text (Markdown with line breaks)
//! nrmlz de --jsonl --field text < in.jsonl # JSON lines: adds "normalized" next to the field
//! nrmlz de --stream < tokens               # incremental: whole sentences as soon as they are complete
//! ```
//!
//! Settings come, in this order of priority, from command-line flags, a config file given with `--config`,
//! environment variables (`NRMLZ_LEXICON`, `NRMLZ_LEXICON_STAGE`), `./nrmlz.toml`, and
//! `$XDG_CONFIG_HOME/nrmlz/config.toml` (created with defaults on first run).

use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use nrmlz::{Lang, Lexicon, LexiconStage, Profile};
use serde::Deserialize;

const DEFAULT_CONFIG: &str = r#"#:schema https://raw.githubusercontent.com/byteowlz/nrmlz/main/crates/nrmlz-cli/config.schema.json
# nrmlz settings. Command-line flags override these.

# Lexicon file (TOML, JSON or TSV) with terms to replace, e.g. OK = "Okay". Empty: none.
lexicon = ""

# Apply the lexicon "before" normalisation (on the raw text) or "after" it (on the spoken form).
# A stage given inside the lexicon file wins over this one.
lexicon_stage = "before"

# "nrmlz" (default, with all fixes) or "reference" (exactly the Articulino frontend).
profile = "nrmlz"

# Read "3,5" as "drei Komma, fünf" (pause comma) - for models trained on the Articulino frontend.
decimal_pause = false

# Apply the respelling entries of the abbreviation table (Homebrew -> Hohmbruh, OK -> okeh).
respellings = false

# Speech style for TTS models that read letters and codes poorly (off by default):
# English spoken letter names ("Q X" -> "cue ex", "A M" -> "A em").
letter_names = false
# Read codes in pieces ("QX-4471" -> "cue ex four four seven one" / "kuh iks, vierundvierzig einundsiebzig").
code_digits = false
# Phone numbers as one sentence per digit group, for TTS that synthesises each sentence separately.
phone_chunks = false
"#;

#[derive(Parser)]
#[command(
    name = "nrmlz",
    version,
    about = "Text normalisation for speech synthesis"
)]
struct Cli {
    /// Config file (overrides the global and local config files)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Normalise German text
    De(Run),
    /// Normalise English text (US conventions)
    En(Run),
    /// List the supported languages
    Langs,
    /// Print the path of the global config file (and create it if missing)
    ConfigPath,
}

#[derive(clap::Args)]
struct Run {
    /// Text to normalise; without it, stdin is read
    text: Vec<String>,
    /// Read stdin as one text instead of one text per line
    #[arg(long, conflicts_with_all = ["jsonl", "stream"])]
    whole: bool,
    /// Read JSON lines from stdin and add the normalised text as "normalized"
    #[arg(long)]
    jsonl: bool,
    /// JSON field with the input text (with --jsonl)
    #[arg(long, default_value = "text")]
    field: String,
    /// Stream stdin: print whole sentences, normalised, as soon as they are complete
    #[arg(long, conflicts_with = "jsonl")]
    stream: bool,
    /// Lexicon file (.toml, .json or .tsv)
    #[arg(long)]
    lexicon: Option<PathBuf>,
    /// When to apply the lexicon: before or after normalisation
    #[arg(long)]
    lexicon_stage: Option<String>,
    /// Do not apply a lexicon from the config
    #[arg(long)]
    no_lexicon: bool,
    /// Profile: nrmlz (default) or reference (exactly the Articulino frontend), with +fix/-fix terms
    #[arg(long)]
    profile: Option<String>,
    /// Pause comma after "Komma" ("drei Komma, fünf")
    #[arg(long)]
    decimal_pause: bool,
    /// Apply the respelling entries of the abbreviation table (Homebrew -> Hohmbruh)
    #[arg(long)]
    respellings: bool,
    /// Fold typography a speech front-end cannot tokenise („“ -> ", – -> -, … -> ...)
    #[arg(long)]
    tts_safe: bool,
    /// Speech style: spoken English letter names ("Q X" -> "cue ex", "A M" -> "A em")
    #[arg(long)]
    letter_names: bool,
    /// Speech style: read codes in pieces ("QX-4471" -> "cue ex four four seven one")
    #[arg(long)]
    code_digits: bool,
    /// Speech style: phone numbers as one sentence per digit group ("four one five. Five five five. ...")
    #[arg(long)]
    phone_chunks: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Settings {
    #[serde(default)]
    lexicon: String,
    #[serde(default)]
    lexicon_stage: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    decimal_pause: Option<bool>,
    #[serde(default)]
    respellings: Option<bool>,
    #[serde(default)]
    letter_names: Option<bool>,
    #[serde(default)]
    code_digits: Option<bool>,
    #[serde(default)]
    phone_chunks: Option<bool>,
}

fn global_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
    Some(base.join("nrmlz").join("config.toml"))
}

fn ensure_global_config() -> Option<PathBuf> {
    let path = global_config_path()?;
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG);
    }
    Some(path)
}

fn load_settings(explicit: Option<&Path>) -> Result<Settings, String> {
    let mut builder = config::Config::builder();
    if let Some(global) = ensure_global_config() {
        builder = builder.add_source(config::File::from(global).required(false));
    }
    builder = builder.add_source(config::File::with_name("nrmlz.toml").required(false));
    builder = builder.add_source(config::Environment::with_prefix("NRMLZ"));
    if let Some(p) = explicit {
        builder = builder.add_source(config::File::from(p.to_path_buf()).required(true));
    }
    let cfg = builder.build().map_err(|e| format!("config: {e}"))?;
    cfg.try_deserialize().map_err(|e| format!("config: {e}"))
}

struct Pipeline {
    lang: Lang,
    profile: Profile,
    lexicon: Option<Lexicon>,
    tts_safe: bool,
    style: nrmlz::SpeechStyle,
}

impl Pipeline {
    fn run(&self, text: &str) -> String {
        let out = nrmlz::normalize_styled(
            self.lang,
            text,
            self.profile,
            self.lexicon.as_ref(),
            self.style,
        );
        if self.tts_safe {
            nrmlz::tts_safe(&out)
        } else {
            out
        }
    }
}

fn build_profile(run: &Run, settings: &Settings) -> Result<Profile, String> {
    let name = run
        .profile
        .clone()
        .or_else(|| settings.profile.clone())
        .unwrap_or_else(|| "nrmlz".into());
    let mut p: Profile = name.parse().map_err(|e: nrmlz::Error| e.to_string())?;
    if run.decimal_pause || settings.decimal_pause == Some(true) {
        p = p.with_decimal_pause(true);
    }
    if run.respellings || settings.respellings == Some(true) {
        p = p.with_respellings(true);
    }
    Ok(p)
}

fn build_pipeline(lang: Lang, run: &Run, settings: &Settings) -> Result<Pipeline, String> {
    let path = run.lexicon.clone().or_else(|| {
        (!run.no_lexicon && !settings.lexicon.is_empty()).then(|| PathBuf::from(&settings.lexicon))
    });
    let lexicon = match path {
        Some(p) => {
            let mut lex = Lexicon::from_path(&p).map_err(|e| e.to_string())?;
            let stage = run.lexicon_stage.clone().or_else(|| {
                if lex.has_explicit_stage() {
                    None
                } else {
                    settings.lexicon_stage.clone()
                }
            });
            if let Some(s) = stage {
                lex.set_stage(s.parse::<LexiconStage>().map_err(|e| e.to_string())?);
            }
            Some(lex)
        }
        None => None,
    };
    Ok(Pipeline {
        lang,
        profile: build_profile(run, settings)?,
        lexicon,
        tts_safe: run.tts_safe,
        style: nrmlz::SpeechStyle {
            letter_names: run.letter_names || settings.letter_names == Some(true),
            code_digits: run.code_digits || settings.code_digits == Some(true),
            phone_chunks: run.phone_chunks || settings.phone_chunks == Some(true),
        },
    })
}

fn run_lang(lang: Lang, run: Run, settings: &Settings) -> Result<(), String> {
    let pipeline = build_pipeline(lang, &run, settings)?;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let io_err = |e: io::Error| e.to_string();

    if !run.text.is_empty() {
        writeln!(out, "{}", pipeline.run(&run.text.join(" "))).map_err(io_err)?;
        return Ok(());
    }
    let stdin = io::stdin();
    if run.whole {
        let mut text = String::new();
        stdin.lock().read_to_string(&mut text).map_err(io_err)?;
        writeln!(out, "{}", pipeline.run(&text)).map_err(io_err)?;
    } else if run.stream {
        if lang != Lang::De {
            return Err("--stream is only available for de".into());
        }
        let p = std::sync::Arc::new(pipeline);
        let p2 = p.clone();
        let mut s = nrmlz::de::StreamNormalizer::with_normalizer(move |t| p2.run(t));
        let mut buf = [0u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        let mut lock = stdin.lock();
        loop {
            let n = lock.read(&mut buf).map_err(io_err)?;
            if n == 0 {
                break;
            }
            pending.extend_from_slice(&buf[..n]);
            // feed only complete UTF-8
            let valid = match std::str::from_utf8(&pending) {
                Ok(_) => pending.len(),
                Err(e) => e.valid_up_to(),
            };
            let chunk = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            let released = s.push(&chunk);
            if !released.is_empty() {
                writeln!(out, "{released}").map_err(io_err)?;
                out.flush().map_err(io_err)?;
            }
        }
        let rest = s.flush(&String::from_utf8_lossy(&pending));
        if !rest.trim().is_empty() {
            writeln!(out, "{rest}").map_err(io_err)?;
        }
        drop(p);
    } else if run.jsonl {
        for (n, line) in stdin.lock().lines().enumerate() {
            let line = line.map_err(io_err)?;
            if line.trim().is_empty() {
                continue;
            }
            let mut v: serde_json::Value =
                serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", n + 1))?;
            let text = v
                .get(&run.field)
                .and_then(|t| t.as_str())
                .ok_or_else(|| format!("line {}: no string field {:?}", n + 1, run.field))?
                .to_string();
            v["normalized"] = serde_json::Value::String(pipeline.run(&text));
            writeln!(out, "{v}").map_err(io_err)?;
        }
    } else {
        for line in stdin.lock().lines() {
            writeln!(out, "{}", pipeline.run(&line.map_err(io_err)?)).map_err(io_err)?;
        }
    }
    out.flush().map_err(io_err)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Langs => {
            println!(
                "de\tGerman (nrmlz or frozen reference profile)\nen\tEnglish (US conventions)"
            );
            Ok(())
        }
        Command::ConfigPath => match ensure_global_config() {
            Some(p) => {
                println!("{}", p.display());
                Ok(())
            }
            None => Err("no home directory".into()),
        },
        Command::De(run) => {
            load_settings(cli.config.as_deref()).and_then(|s| run_lang(Lang::De, run, &s))
        }
        Command::En(run) => {
            load_settings(cli.config.as_deref()).and_then(|s| run_lang(Lang::En, run, &s))
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nrmlz: {e}");
            ExitCode::FAILURE
        }
    }
}
