# greentic-pack i18n build step — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `greentic-pack`'s build materialise `assets/i18n/<lang>.json` + `_manifest.json` for a caller-supplied list of languages, so exported `.gtpack` archives carry the locales the operator selected.

**Architecture:** Add a non-fatal i18n step to `greentic-pack wizard apply`. A new `langs` answers field flows into a `materialize_i18n(pack_root, langs)` call placed after the delegate steps and before `update`/`resolve`/`build`. The step extracts translatable strings from `assets/cards/*.json` into `en.json`, shells out to `greentic-i18n-translator` per language, and writes `_manifest.json`. greentic-pack's existing `assets/**` auto-archiving carries the files into the `.gtpack`. greentic-designer only forwards its selected languages into the build answers.

**Tech Stack:** Rust 2024 (rustc 1.95.0), `serde_json`, `walkdir`, `anyhow`, `tempfile` (dev), `assert_cmd` (dev). External binary `greentic-i18n-translator`.

## Global Constraints

- Rust 2024 edition, pinned rustc 1.95.0; `#![forbid(unsafe_code)]` in every greentic-pack source file.
- Error handling: `anyhow::Result<T>` with `.context()`; `thiserror` for domain errors.
- `materialize_i18n` is **non-fatal**: it never returns `Err` to `apply_answer_document`; every skip/failure is an `eprintln!` to stderr.
- **No auto-install** of the translator (no `cargo binstall`).
- Max ~500 lines per Rust file — `i18n_build` is a directory module split into `mod.rs` + `extract.rs`.
- Translator contract is fixed: `greentic-i18n-translator translate --langs <lang> --en <abs en.json> --auth-mode auto`, run in a per-language temp cwd (`git init --quiet`); it writes `<lang>.json` next to `--en`.
- Manifest format: a JSON array of locale codes, e.g. `["en","id","ja"]`, sorted + deduped, always including `"en"`.
- Do NOT add a `greentic-cards2pack` dependency to greentic-pack — port the focused extractor instead.
- No Claude co-author attribution on commits. Conventional commit messages.

---

### Task 1: Register `greentic-i18n-translator` in external_tools

**Files:**
- Modify: `crates/packc/src/external_tools.rs:26-34` (the `override_binary` match)
- Test: `crates/packc/src/external_tools.rs` (inline `#[cfg(test)]` module — add one)

**Interfaces:**
- Consumes: existing `pub fn resolve(binary: &str) -> Option<PathBuf>`.
- Produces: `resolve("greentic-i18n-translator")` honours `GREENTIC_I18N_TRANSLATOR_BIN` / `GREENTIC_I18N_TRANSLATOR_DEV_BIN`.

- [ ] **Step 1: Write the failing test**

Append to `crates/packc/src/external_tools.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn resolve_honours_translator_env_override() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // SAFETY: single-threaded test; no other thread reads this env var here.
        unsafe { std::env::set_var("GREENTIC_I18N_TRANSLATOR_BIN", tmp.path()); }
        let got = resolve("greentic-i18n-translator");
        unsafe { std::env::remove_var("GREENTIC_I18N_TRANSLATOR_BIN"); }
        assert_eq!(got.as_deref(), Some(tmp.path()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --locked external_tools::tests::resolve_honours_translator_env_override -- --nocapture`
Expected: FAIL — `resolve` returns `None` because the match arm doesn't exist (override returns `None`, and the temp path isn't on PATH).

- [ ] **Step 3: Add the match arm**

In `override_binary`, add the arm before `_ => return None,`:

```rust
        "greentic-i18n-translator" => {
            &["GREENTIC_I18N_TRANSLATOR_BIN", "GREENTIC_I18N_TRANSLATOR_DEV_BIN"]
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p greentic-pack --locked external_tools::tests::resolve_honours_translator_env_override -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/external_tools.rs
git commit -m "feat(pack): resolve greentic-i18n-translator via env override"
```

---

### Task 2: Port the Adaptive-Card string extractor into `i18n_build::extract`

This copies the proven extractor from `greentic-cards2pack` (the source of truth) into greentic-pack so the build step has no cross-crate dependency. The code and its tests are reproduced from `greentic-cards2pack/src/i18n_extract/extractor.rs` and `greentic-cards2pack/src/i18n_extract/mod.rs`.

**Files:**
- Create: `crates/packc/src/i18n_build/mod.rs` (module root; this task only declares `extract` + re-exports)
- Create: `crates/packc/src/i18n_build/extract.rs`
- Modify: `crates/packc/src/lib.rs:11` (add `pub mod i18n_build;` after `pub mod flow_resolve;`)

**Interfaces:**
- Produces:
  - `pub struct ExtractedString { pub key: String, pub value: String, pub source_file: PathBuf, pub json_path: String }`
  - `pub struct ExtractConfig { pub cards_dir: PathBuf, pub output: PathBuf, pub prefix: String, pub skip_i18n_patterns: bool }`
  - `pub fn extract_from_directory(config: &ExtractConfig) -> anyhow::Result<Vec<ExtractedString>>`
  - `pub fn write_bundle(strings: &[ExtractedString], output: &Path) -> anyhow::Result<()>`

- [ ] **Step 1: Create `crates/packc/src/i18n_build/extract.rs`**

Copy the full contents of `greentic-cards2pack/src/i18n_extract/extractor.rs` (the recursive extractor: `extract_from_value`, `extract_translatable_fields`, `extract_container_fields`, `extract_factset`, `extract_choiceset`, `should_extract`, `build_key`, `build_json_path`, plus its `#[cfg(test)]` module) AND the directory/bundle helpers from `greentic-cards2pack/src/i18n_extract/mod.rs` (`ExtractConfig`, `ExtractedString`, `extract_from_directory`, `to_json_bundle`, `write_bundle`, `is_adaptive_card`, `determine_card_id`, `sanitize_key_part`, plus its `#[cfg(test)]` module) into this single file.

Adjust for a single-file module:
- Top of file: `#![forbid(unsafe_code)]` is on the crate root already; do not repeat. Add `use` lines: `use std::fs; use std::path::{Path, PathBuf}; use anyhow::{Context, Result}; use serde_json::Value; use walkdir::WalkDir;`.
- Remove the `mod extractor; mod report;` and `pub use report::generate_report;` lines (report is not ported). Remove the `pub use extractor::extract_from_value;` re-export; `extract_from_value` is now defined in this same file.
- Keep `ExtractedString`, `ExtractConfig`, `extract_from_directory`, `to_json_bundle`, `write_bundle` `pub`.

- [ ] **Step 2: Create `crates/packc/src/i18n_build/mod.rs` (extraction wiring only)**

```rust
#![forbid(unsafe_code)]
//! Build-time i18n materialisation for packs: extract card strings,
//! translate via greentic-i18n-translator, and write assets/i18n/.

mod extract;

pub use extract::{ExtractConfig, ExtractedString, extract_from_directory, write_bundle};
```

- [ ] **Step 3: Register the module**

In `crates/packc/src/lib.rs`, add after `pub mod flow_resolve;` (line 11):

```rust
pub mod i18n_build;
```

- [ ] **Step 4: Run the ported tests to verify they pass**

Run: `cargo test -p greentic-pack --locked i18n_build::extract -- --nocapture`
Expected: PASS — all ported extractor + bundle tests (e.g. `test_extract_from_simple_card`, `test_extract_factset`, `test_write_bundle_creates_parent_dirs`) pass.

- [ ] **Step 5: Verify it compiles cleanly**

Run: `cargo clippy -p greentic-pack --all-targets --locked -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/packc/src/i18n_build/ crates/packc/src/lib.rs
git commit -m "feat(pack): port adaptive-card i18n string extractor into packc"
```

---

### Task 3: Implement `materialize_i18n` (translate + manifest + non-fatal)

**Files:**
- Modify: `crates/packc/src/i18n_build/mod.rs` (add orchestration)
- Test: `crates/packc/src/i18n_build/mod.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `extract_from_directory`, `write_bundle`, `ExtractConfig` (Task 2); `crate::external_tools::resolve` (Task 1).
- Produces: `pub fn materialize_i18n(pack_root: &Path, langs: &[String])` — writes `pack_root/assets/i18n/en.json`, `<lang>.json`, `_manifest.json`. Never panics, never returns.

- [ ] **Step 1: Write the failing tests**

Add to `crates/packc/src/i18n_build/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::materialize_i18n;
    use std::fs;
    use std::path::Path;

    fn write_card(pack_root: &Path) {
        let cards = pack_root.join("assets/cards");
        fs::create_dir_all(&cards).unwrap();
        fs::write(
            cards.join("hello.json"),
            r#"{"type":"AdaptiveCard","body":[{"type":"TextBlock","text":"Hello"}]}"#,
        )
        .unwrap();
    }

    // A stub translator: copies en.json to <lang>.json so we can assert wiring
    // without a real LLM. Written as a shell script and pointed at via env.
    fn install_stub_translator(dir: &Path) -> std::path::PathBuf {
        let script = dir.join("stub-translator.sh");
        // greentic-i18n-translator translate --langs <lang> --en <path>
        fs::write(
            &script,
            "#!/bin/sh\nlang=\"\"; en=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --langs) lang=\"$2\"; shift 2;; --en) en=\"$2\"; shift 2;; *) shift;; esac; done\ncp \"$en\" \"$(dirname \"$en\")/$lang.json\"\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
        script
    }

    #[test]
    fn empty_langs_is_a_noop() {
        let tmp = tempfile::tempdir().unwrap();
        write_card(tmp.path());
        materialize_i18n(tmp.path(), &[]);
        assert!(!tmp.path().join("assets/i18n").exists());
    }

    #[test]
    fn missing_translator_writes_no_lang_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_card(tmp.path());
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("GREENTIC_I18N_TRANSLATOR_BIN", "/nonexistent/translator-xyz"); }
        materialize_i18n(tmp.path(), &["id".to_string()]);
        unsafe { std::env::remove_var("GREENTIC_I18N_TRANSLATOR_BIN"); }
        assert!(!tmp.path().join("assets/i18n/id.json").exists());
    }

    #[test]
    fn stub_translator_produces_lang_files_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_card(tmp.path());
        let stub = install_stub_translator(tmp.path());
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("GREENTIC_I18N_TRANSLATOR_BIN", &stub); }
        materialize_i18n(tmp.path(), &["id".to_string(), "ja".to_string()]);
        unsafe { std::env::remove_var("GREENTIC_I18N_TRANSLATOR_BIN"); }

        let i18n = tmp.path().join("assets/i18n");
        assert!(i18n.join("en.json").is_file());
        assert!(i18n.join("id.json").is_file());
        assert!(i18n.join("ja.json").is_file());
        let manifest: Vec<String> =
            serde_json::from_str(&fs::read_to_string(i18n.join("_manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest, vec!["en".to_string(), "id".to_string(), "ja".to_string()]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p greentic-pack --locked i18n_build::tests -- --nocapture`
Expected: FAIL — `materialize_i18n` not defined.

- [ ] **Step 3: Implement the orchestration**

Add to `crates/packc/src/i18n_build/mod.rs` (above the test module):

```rust
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
        eprintln!("[i18n] no translatable strings found in {}", config.cards_dir.display());
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

fn translate_to_language(
    translator: &Path,
    lang: &str,
    en_bundle: &Path,
) -> anyhow::Result<()> {
    use anyhow::{Context, bail};

    let work_dir = std::env::temp_dir().join(format!("greentic-pack-translate-{lang}"));
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("create translator work dir for {lang}"))?;
    if !work_dir.join(".git").exists() {
        let _ = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(&work_dir)
            .output();
    }

    let en_abs = std::fs::canonicalize(en_bundle).unwrap_or_else(|_| en_bundle.to_path_buf());
    let output = Command::new(translator)
        .current_dir(&work_dir)
        .arg("translate")
        .arg("--langs")
        .arg(lang)
        .arg("--en")
        .arg(&en_abs)
        .arg("--auth-mode")
        .arg("auto")
        .output()
        .context("failed to execute greentic-i18n-translator")?;

    let _ = std::fs::remove_dir_all(&work_dir);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("translator exited non-zero for {lang}: {}", stderr.trim_end());
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
```

Note: `BTreeSet` keeps the manifest sorted + deduped automatically, so `["en","id","ja"]` is deterministic.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p greentic-pack --locked i18n_build::tests -- --nocapture`
Expected: PASS — all three tests.

- [ ] **Step 5: Lint**

Run: `cargo clippy -p greentic-pack --all-targets --locked -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/packc/src/i18n_build/mod.rs
git commit -m "feat(pack): materialize_i18n translates cards and writes assets/i18n"
```

---

### Task 4: Thread `langs` through wizard answers and call the step

**Files:**
- Modify: `crates/packc/src/cli/wizard.rs` — schema (~L905, in `pack_wizard_answers_schema`), `WizardExecutionPlan` struct (L193-207), plan construction (the function that fills `asset_staging`), and `apply_answer_document` (after delegates ~L1274, before the `run_doctor || run_build` block ~L1275)
- Test: `crates/packc/tests/i18n_build_pipeline.rs` (new integration test)

**Interfaces:**
- Consumes: `crate::i18n_build::materialize_i18n` (Task 3).
- Produces: `wizard apply --answers` accepts `"langs": ["id","ja"]` and writes locale files into the built `.gtpack`.

- [ ] **Step 1: Write the failing integration test**

Create `crates/packc/tests/i18n_build_pipeline.rs`:

```rust
//! Verifies that `wizard apply` with `langs` lands locale files inside the
//! final `.gtpack`, using a stub translator (no real LLM).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn stub_translator(dir: &Path) -> std::path::PathBuf {
    let script = dir.join("stub-translator.sh");
    fs::write(
        &script,
        "#!/bin/sh\nlang=\"\"; en=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --langs) lang=\"$2\"; shift 2;; --en) en=\"$2\"; shift 2;; *) shift;; esac; done\ncp \"$en\" \"$(dirname \"$en\")/$lang.json\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

#[test]
fn wizard_apply_with_langs_packs_locale_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // 1. Scaffold a minimal pack with one Adaptive Card.
    let pack_dir = root.join("demo.pack");
    let exe = env!("CARGO_BIN_EXE_greentic-pack");
    let ok = std::process::Command::new(exe)
        .args(["new", "demo", "--dir", pack_dir.to_str().unwrap()])
        .status()
        .unwrap()
        .success();
    assert!(ok, "pack new failed");
    let cards = pack_dir.join("assets/cards");
    fs::create_dir_all(&cards).unwrap();
    fs::write(
        cards.join("hello.json"),
        r#"{"type":"AdaptiveCard","body":[{"type":"TextBlock","text":"Hello"}]}"#,
    )
    .unwrap();

    // 2. Build via wizard apply with langs + stub translator.
    let answers = root.join("answers.json");
    fs::write(
        &answers,
        serde_json::to_string(&serde_json::json!({
            "pack_dir": pack_dir.to_str().unwrap(),
            "run_build": true,
            "langs": ["id", "ja"]
        }))
        .unwrap(),
    )
    .unwrap();

    let stub = stub_translator(root);
    let status = std::process::Command::new(exe)
        .args(["wizard", "apply", "--answers", answers.to_str().unwrap()])
        .env("GREENTIC_I18N_TRANSLATOR_BIN", &stub)
        .status()
        .unwrap();
    assert!(status.success(), "wizard apply failed");

    // 3. Assert the locale files are inside the produced .gtpack.
    let gtpack = pack_dir.join("dist/demo.gtpack");
    assert!(gtpack.is_file(), "no .gtpack at {}", gtpack.display());
    let file = fs::File::open(&gtpack).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    for want in [
        "assets/i18n/en.json",
        "assets/i18n/id.json",
        "assets/i18n/ja.json",
        "assets/i18n/_manifest.json",
    ] {
        assert!(names.iter().any(|n| n == want), "missing {want} in {names:?}");
    }
}
```

If `zip` is not already a dev-dependency of `crates/packc`, add it: in `crates/packc/Cargo.toml` under `[dev-dependencies]` add `zip = { workspace = true }` (or the same version the crate uses at runtime — check `Cargo.toml`; `zip` is already a normal dependency used by `build.rs`, so `zip.workspace = true` or the existing version string works).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --locked --test i18n_build_pipeline -- --nocapture`
Expected: FAIL — `langs` is ignored, so `.gtpack` has no `assets/i18n/*` (the four asserts fail). The `langs` key may also be rejected by schema validation; either way the test is red.

- [ ] **Step 3: Add `langs` to the answers schema**

In `pack_wizard_answers_schema()`, inside the `properties` object (next to `asset_staging`, around L905-913), add:

```rust
            "langs": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Target locale codes to translate the pack's Adaptive Card strings into during build (e.g. [\"id\",\"ja\"]). Requires greentic-i18n-translator on PATH; missing/failed languages are skipped with a warning."
            },
```

- [ ] **Step 4: Add the field to `WizardExecutionPlan`**

In `struct WizardExecutionPlan` (L193-207), add after `asset_staging`:

```rust
    i18n_langs: Vec<String>,
```

- [ ] **Step 5: Parse `langs` where the plan is built**

Find the function that constructs `WizardExecutionPlan { … asset_staging, }` (it is built from the `WizardAnswerDocument`; search the file for `asset_staging,` inside a struct literal). Immediately before that struct literal, parse the langs array from the answers value (the same `answers`/`doc` object the other fields read from):

```rust
    let i18n_langs: Vec<String> = answers
        .get("langs")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
```

Then add `i18n_langs,` to the struct literal next to `asset_staging,`. (Replace `answers` with the actual identifier in scope — match how `asset_staging` is read.)

- [ ] **Step 6: Call `materialize_i18n` in `apply_answer_document`**

In `apply_answer_document` (L1226), after the component-delegate block ends (~L1274) and before the `if plan.run_doctor || plan.run_build {` block (~L1275), insert:

```rust
    if !plan.i18n_langs.is_empty() {
        // Non-fatal: writes pack_root/assets/i18n/*, reports skips to stderr.
        crate::i18n_build::materialize_i18n(&plan.pack_root, &plan.i18n_langs);
    }
```

- [ ] **Step 7: Run the integration test to verify it passes**

Run: `cargo test -p greentic-pack --locked --test i18n_build_pipeline -- --nocapture`
Expected: PASS — the `.gtpack` contains all four `assets/i18n/*` entries.

- [ ] **Step 8: Lint + full test sweep**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test -p greentic-pack --locked`
Expected: no warnings; all tests pass (confirms no regression in existing wizard tests).

- [ ] **Step 9: Commit**

```bash
git add crates/packc/src/cli/wizard.rs crates/packc/tests/i18n_build_pipeline.rs crates/packc/Cargo.toml
git commit -m "feat(pack): wizard apply translates selected langs into the gtpack"
```

---

### Task 5: greentic-designer forwards selected languages

This task is in the **greentic-designer** repo, not greentic-pack. Create/checkout a feature branch there (e.g. an isolated worktree off the designer's integration branch) before starting.

**Files:**
- Modify: `greentic-designer/src/orchestrate/pack_via_packc/mod.rs` — `fn build_pack_answers()` (L1036) and its call site in `render_via_packc` (the `let pack_answers = build_pack_answers();` line, ~L171)
- Test: `greentic-designer/src/orchestrate/pack_via_packc/mod.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: greentic-pack's new `langs` answers field (Task 4).
- Produces: pack-build answers JSON contains `"langs": [<selected codes>]`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `pack_via_packc/mod.rs`:

```rust
    #[test]
    fn build_pack_answers_includes_langs() {
        let answers = super::build_pack_answers(&["id".to_string(), "ja".to_string()]);
        let langs = answers
            .get("langs")
            .and_then(|v| v.as_array())
            .expect("langs array present");
        let codes: Vec<&str> = langs.iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(codes, vec!["id", "ja"]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-designer --locked build_pack_answers_includes_langs -- --nocapture`
Expected: FAIL — `build_pack_answers` takes no arguments / has no `langs` key.

- [ ] **Step 3: Thread langs into `build_pack_answers`**

Change the signature and body of `build_pack_answers` to accept the langs slice and include it:

```rust
fn build_pack_answers(langs: &[String]) -> Value {
    // ...existing fields unchanged...
    // add to the returned json!({ ... }) object:
    //   "langs": langs,
}
```

Concretely, in the returned `json!({ ... })` literal add a `"langs": langs,` entry (serde_json serialises `&[String]` as a JSON array).

- [ ] **Step 4: Update the call site**

In `render_via_packc`, change `let pack_answers = build_pack_answers();` to:

```rust
    let pack_answers = build_pack_answers(inputs.langs);
```

The existing log line that printed `inputs.langs.len()` may stay (it is now followed by real forwarding) or be removed — leave it; it is harmless and still informative.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p greentic-designer --locked build_pack_answers_includes_langs -- --nocapture`
Expected: PASS

- [ ] **Step 6: Lint + crate tests**

Run: `cargo clippy -p greentic-designer --all-targets -- -D warnings && cargo test -p greentic-designer --locked --lib`
Expected: no warnings; tests pass.

- [ ] **Step 7: Commit (in greentic-designer)**

```bash
git add src/orchestrate/pack_via_packc/mod.rs
git commit -m "feat(packc): forward selected languages to greentic-pack i18n build"
```

---

### Task 6: Documentation

**Files:**
- Modify: `greentic-pack/CLAUDE.md` (build pipeline gains an i18n step)
- Modify: `greentic-pack/docs/internationalise-pack-howto.md` (automated `langs` path)
- Modify: `greentic-designer/CLAUDE.md` (packc pipeline forwards `langs`)
- Modify: workspace root `CLAUDE.md` capability/i18n note + `greentic-docs` `multilanguage-i18n` page (export now translates selected languages into the `.gtpack`)

- [ ] **Step 1: greentic-pack docs**

In `greentic-pack/CLAUDE.md` under "Architecture → Build pipeline", add a bullet: the build step materialises `assets/i18n/<lang>.json` + `_manifest.json` from `assets/cards/*.json` for any `langs` passed to `wizard apply`, via `greentic-i18n-translator` (non-fatal; skipped with a warning when the binary is absent; located via `GREENTIC_I18N_TRANSLATOR_BIN`).

In `greentic-pack/docs/internationalise-pack-howto.md`, add a section "Automated translation during build" describing `wizard apply` with a `langs` array, the translator requirement, and the non-fatal behaviour.

- [ ] **Step 2: greentic-designer + workspace docs**

In `greentic-designer/CLAUDE.md` "Pack Pipeline" section, note that `render_via_packc` forwards the operator-selected `langs` into the greentic-pack build answers (replacing the previous log-only handling); locale files are produced by greentic-pack, not the designer.

In the workspace root `CLAUDE.md` (the `greentic.cap.webchat.i18n.v1` row / i18n notes) and `greentic-docs` `multilanguage-i18n` page, state that exporting from the designer with extra languages now packs the translated locale files into the app `.gtpack`, so webchat-gui's locale picker shows them.

- [ ] **Step 3: Commit**

In each repo touched:

```bash
git add CLAUDE.md docs/
git commit -m "docs: document automated pack i18n translation"
```

---

## Self-Review

**Spec coverage:**
- Answers schema `langs` field → Task 4 Step 3. ✓
- `WizardExecutionPlan.i18n_langs` + parse → Task 4 Steps 4-5. ✓
- Injection after delegates / before build → Task 4 Step 6. ✓
- New `i18n_build` module (extract + translate + manifest) → Tasks 2-3. ✓
- Translator binary registration → Task 1. ✓
- Non-fatal + stderr reporting → Task 3 Step 3 (all `eprintln!`, no `Err` out). ✓
- Carry-over wins → Task 3 Step 3 (`target.is_file()` branch). ✓
- No auto-install → Task 3 resolves-or-warns, never installs. ✓
- Asset auto-archiving (crux) → Task 4 Step 1 integration test asserts archive contents. ✓
- `is_reserved_extra_file` risk → covered by the same archive assertion (en.json + _manifest.json present). ✓
- Designer forwards langs → Task 5. ✓
- Docs → Task 6. ✓
- Out-of-scope (cards2pack dedup, legacy WASM path, runtime overlay) → not tasked, per spec. ✓

**Placeholder scan:** No TBD/TODO; every code step shows full code. Task 2 copies a named source file verbatim (exact path given) — concrete, not a placeholder.

**Type consistency:** `materialize_i18n(pack_root: &Path, langs: &[String])` used identically in Task 3 (def), Task 4 Step 6 (call). `ExtractConfig`/`extract_from_directory`/`write_bundle` signatures match Task 2 exports. `build_pack_answers(langs: &[String]) -> Value` consistent across Task 5 Steps 1/3/4. Manifest is `Vec`/`BTreeSet` of locale codes everywhere.
