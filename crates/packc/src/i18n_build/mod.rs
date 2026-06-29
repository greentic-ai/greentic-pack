#![forbid(unsafe_code)]
//! Build-time i18n materialisation for packs: extract card strings,
//! translate via greentic-i18n-translator, and write assets/i18n/.

mod bundle;
mod extract;

pub use bundle::{ExtractConfig, extract_from_directory, write_bundle};
pub use extract::ExtractedString;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const TRANSLATOR_BIN: &str = "greentic-i18n-translator";

/// Materialise `assets/i18n/` for `langs` inside `pack_root`. Non-fatal:
/// any problem is reported to stderr and the build proceeds.
pub fn materialize_i18n(pack_root: &Path, langs: &[String]) {
    if langs.is_empty() {
        return;
    }

    let translator = match crate::external_tools::resolve(TRANSLATOR_BIN) {
        Some(path) => path,
        None => {
            eprintln!(
                "[i18n] {TRANSLATOR_BIN} not found; skipping {} language(s): {}",
                langs.len(),
                langs.join(", ")
            );
            return;
        }
    };

    let cards_dir = pack_root.join("assets/cards");
    let i18n_dir = pack_root.join("assets/i18n");
    let en_path = i18n_dir.join("en.json");

    let config = ExtractConfig {
        cards_dir,
        output: en_path.clone(),
        prefix: "card".to_string(),
        skip_i18n_patterns: true,
    };

    let strings = match extract_from_directory(&config) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("[i18n] extraction failed: {err:#}");
            return;
        }
    };
    if strings.is_empty() {
        eprintln!(
            "[i18n] no translatable strings found in {}",
            config.cards_dir.display()
        );
        return;
    }
    if let Err(err) = write_bundle(&strings, &en_path) {
        eprintln!("[i18n] failed to write en.json: {err:#}");
        return;
    }

    let mut available: BTreeSet<String> = BTreeSet::new();
    available.insert("en".to_string());

    for lang in langs {
        if lang == "en" {
            continue;
        }
        let target = i18n_dir.join(format!("{lang}.json"));
        if target.is_file() {
            // Carry-over: author already shipped this locale.
            available.insert(lang.clone());
            continue;
        }
        match translate_to_language(&translator, lang, &en_path) {
            Ok(()) if target.is_file() => {
                available.insert(lang.clone());
            }
            Ok(()) => {
                eprintln!("[i18n] translator reported success for {lang} but wrote no file");
            }
            Err(err) => {
                eprintln!("[i18n] failed to translate {lang}: {err:#}");
            }
        }
    }

    write_manifest(&i18n_dir, &available);
    let codes: Vec<&str> = available.iter().map(String::as_str).collect();
    eprintln!("[i18n] materialised locales: {}", codes.join(", "));
}

fn translate_to_language(translator: &Path, lang: &str, en_bundle: &Path) -> anyhow::Result<()> {
    use anyhow::{Context, bail};

    let work_dir = tempfile::tempdir().context("create translator work dir")?;
    let work_path = work_dir.path();
    if !work_path.join(".git").exists() {
        let _ = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(work_path)
            .output();
    }

    let en_abs = std::fs::canonicalize(en_bundle).unwrap_or_else(|_| en_bundle.to_path_buf());
    let output = Command::new(translator)
        .current_dir(work_path)
        .arg("translate")
        .arg("--langs")
        .arg(lang)
        .arg("--en")
        .arg(&en_abs)
        .arg("--auth-mode")
        .arg("auto")
        .output()
        .context("failed to execute greentic-i18n-translator")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "translator exited non-zero for {lang}: {}",
            stderr.trim_end()
        );
    }
    Ok(())
}

fn write_manifest(i18n_dir: &Path, locales: &BTreeSet<String>) {
    let codes: Vec<&String> = locales.iter().collect();
    match serde_json::to_string_pretty(&codes) {
        Ok(json) => {
            if let Err(err) = std::fs::write(i18n_dir.join("_manifest.json"), json) {
                eprintln!("[i18n] failed to write _manifest.json: {err}");
            }
        }
        Err(err) => eprintln!("[i18n] failed to serialise manifest: {err}"),
    }
}
