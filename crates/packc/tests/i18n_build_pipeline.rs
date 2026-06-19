#![cfg(unix)]
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
    //    The answer document must be wrapped in the wizard envelope.
    let answers = root.join("answers.json");
    fs::write(
        &answers,
        serde_json::to_string(&serde_json::json!({
            "wizard_id": "greentic-pack.wizard.run",
            "schema_id": "greentic-pack.wizard.answers",
            "schema_version": "1.0.0",
            "locale": "en-GB",
            "answers": {
                "pack_dir": pack_dir.to_str().unwrap(),
                "run_build": true,
                "langs": ["id", "ja"]
            },
            "locks": {}
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

    // 3. Locate the produced .gtpack archive (name may vary).
    let dist_dir = pack_dir.join("dist");
    let gtpack = fs::read_dir(&dist_dir)
        .unwrap_or_else(|err| panic!("dist dir missing at {}: {err}", dist_dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "gtpack")
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no .gtpack found in {}", dist_dir.display()));

    // 4. Assert the locale files are inside the produced .gtpack.
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
