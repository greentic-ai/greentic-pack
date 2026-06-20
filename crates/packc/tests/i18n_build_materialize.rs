#![cfg(unix)]
//! Integration tests for `materialize_i18n`.
//!
//! These live in an integration-test crate (not inside `src/`) so that
//! `#![forbid(unsafe_code)]` in the library does NOT apply here.  The tests
//! use `unsafe { std::env::set_var(...) }` to wire a stub translator binary,
//! which is intentional and safe when protected by env_lock().

use packc::i18n_build::materialize_i18n;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn write_card(pack_root: &Path) {
    let cards = pack_root.join("assets/cards");
    fs::create_dir_all(&cards).unwrap();
    fs::write(
        cards.join("hello.json"),
        r#"{"type":"AdaptiveCard","body":[{"type":"TextBlock","text":"Hello"}]}"#,
    )
    .unwrap();
}

/// A stub translator: copies en.json to <lang>.json so we can assert wiring
/// without a real LLM. Written as a shell script and pointed at via env.
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

/// A translator stub that always fails (exits 1, writes nothing).
/// This is used to test that when a translator is found but fails,
/// the build succeeds and no locale files are written.
fn install_failing_translator(dir: &Path) -> std::path::PathBuf {
    let script = dir.join("failing-translator.sh");
    fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn empty_langs_is_a_noop() {
    let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    write_card(tmp.path());
    materialize_i18n(tmp.path(), &[]);
    assert!(!tmp.path().join("assets/i18n").exists());
}

#[test]
fn failing_translator_writes_no_lang_files() {
    let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    write_card(tmp.path());
    let failing = install_failing_translator(tmp.path());
    // SAFETY: protected by env_lock() guard.
    unsafe { std::env::set_var("GREENTIC_I18N_TRANSLATOR_BIN", &failing); }
    materialize_i18n(tmp.path(), &["id".to_string()]);
    unsafe { std::env::remove_var("GREENTIC_I18N_TRANSLATOR_BIN"); }
    // The translator was found but failed → no locale file, build stays non-fatal.
    assert!(!tmp.path().join("assets/i18n/id.json").exists());
}

#[test]
fn stub_translator_produces_lang_files_and_manifest() {
    let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    write_card(tmp.path());
    let stub = install_stub_translator(tmp.path());
    // SAFETY: protected by env_lock() guard.
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
