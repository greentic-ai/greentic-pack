use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn wizard_run_emit_answers_writes_envelope() {
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("answers.json");
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--schema-version")
        .arg("1.2.3")
        .arg("--emit-answers")
        .arg(&answers_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wizard run");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"0\n")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "wizard run should succeed");

    let doc: Value =
        serde_json::from_slice(&fs::read(&answers_path).expect("read emitted answers"))
            .expect("parse emitted answers");
    assert_eq!(
        doc.get("wizard_id").and_then(Value::as_str),
        Some("greentic-pack.wizard.run")
    );
    assert_eq!(
        doc.get("schema_id").and_then(Value::as_str),
        Some("greentic-pack.wizard.answers")
    );
    assert_eq!(
        doc.get("schema_version").and_then(Value::as_str),
        Some("1.2.3")
    );
    assert!(doc.get("answers").and_then(Value::as_object).is_some());
    assert!(doc.get("locks").and_then(Value::as_object).is_some());
}

#[test]
fn wizard_validate_with_migrate_reemits_document() {
    let temp = TempDir::new().expect("tempdir");
    let input_path = temp.path().join("old_answers.json");
    let output_path = temp.path().join("migrated_answers.json");
    fs::write(
        &input_path,
        r#"{"answers":{"pack_dir":"."},"locks":{"legacy":"x"}}"#,
    )
    .expect("write old answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&input_path)
        .arg("--migrate")
        .arg("--schema-version")
        .arg("2.0.0")
        .arg("--emit-answers")
        .arg(&output_path)
        .output()
        .expect("run wizard validate");
    assert!(output.status.success(), "wizard validate should succeed");

    let doc: Value =
        serde_json::from_slice(&fs::read(&output_path).expect("read migrated answers"))
            .expect("parse migrated answers");
    assert_eq!(
        doc.get("wizard_id").and_then(Value::as_str),
        Some("greentic-pack.wizard.run")
    );
    assert_eq!(
        doc.get("schema_id").and_then(Value::as_str),
        Some("greentic-pack.wizard.answers")
    );
    assert_eq!(
        doc.get("schema_version").and_then(Value::as_str),
        Some("2.0.0")
    );
}

#[test]
fn wizard_apply_answers_runs_pipeline() {
    let _guard = env_lock().lock().expect("env lock");
    let temp = TempDir::new().expect("tempdir");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let answers_path = temp.path().join("apply_answers.json");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{"pack_dir":".","run_doctor":true,"run_build":true,"sign":false},
  "locks":{}
}"#,
    )
    .expect("write answers file");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
    }
    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard apply");
    unsafe {
        std::env::remove_var("GREENTIC_PACK_WIZARD_SELF_EXE");
    }
    assert!(output.status.success(), "wizard apply should succeed");

    let calls = fs::read_to_string(&log_path).expect("read call log");
    assert!(calls.contains("doctor --in ."));
    assert!(calls.contains("build --in ."));
}

#[test]
fn wizard_apply_answers_with_sign_runs_sign_step() {
    let _guard = env_lock().lock().expect("env lock");
    let temp = TempDir::new().expect("tempdir");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let answers_path = temp.path().join("apply_sign_answers.json");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{
    "pack_dir":".",
    "run_doctor":true,
    "run_build":true,
    "sign":true,
    "sign_key_path":"./test-sign-key.pem"
  },
  "locks":{}
}"#,
    )
    .expect("write answers file");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
    }
    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard apply");
    unsafe {
        std::env::remove_var("GREENTIC_PACK_WIZARD_SELF_EXE");
    }
    assert!(output.status.success(), "wizard apply should succeed");

    let calls = fs::read_to_string(&log_path).expect("read call log");
    assert!(calls.contains("doctor --in ."));
    assert!(calls.contains("build --in ."));
    assert!(calls.contains("sign --pack . --key ./test-sign-key.pem"));
}

#[test]
fn wizard_validate_answers_is_side_effect_free() {
    let _guard = env_lock().lock().expect("env lock");
    let temp = TempDir::new().expect("tempdir");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let answers_path = temp.path().join("validate_answers.json");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{"pack_dir":".","run_doctor":true,"run_build":true,"sign":false},
  "locks":{}
}"#,
    )
    .expect("write answers file");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
    }
    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard validate");
    unsafe {
        std::env::remove_var("GREENTIC_PACK_WIZARD_SELF_EXE");
    }
    assert!(output.status.success(), "wizard validate should succeed");
    assert!(
        !log_path.exists()
            || fs::read_to_string(&log_path)
                .expect("read calls")
                .trim()
                .is_empty()
    );
}

#[test]
fn wizard_run_with_answers_executes_apply_flow() {
    let _guard = env_lock().lock().expect("env lock");
    let temp = TempDir::new().expect("tempdir");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let answers_path = temp.path().join("run_answers.json");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{"pack_dir":".","run_doctor":true,"run_build":true,"sign":false},
  "locks":{}
}"#,
    )
    .expect("write answers file");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
    }
    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard run --answers");
    unsafe {
        std::env::remove_var("GREENTIC_PACK_WIZARD_SELF_EXE");
    }
    assert!(
        output.status.success(),
        "wizard run --answers should succeed"
    );

    let calls = fs::read_to_string(&log_path).expect("read call log");
    assert!(calls.contains("doctor --in ."));
    assert!(calls.contains("build --in ."));
}

#[test]
fn wizard_validate_missing_schema_without_migrate_fails() {
    let temp = TempDir::new().expect("tempdir");
    let input_path = temp.path().join("old_answers.json");
    fs::write(&input_path, r#"{"answers":{"pack_dir":"."},"locks":{}}"#)
        .expect("write old answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&input_path)
        .output()
        .expect("run wizard validate");
    assert!(
        !output.status.success(),
        "validate should fail without --migrate"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rerun with --migrate"));
}

#[test]
fn wizard_validate_schema_mismatch_without_migrate_fails() {
    let temp = TempDir::new().expect("tempdir");
    let input_path = temp.path().join("schema_mismatch_answers.json");
    fs::write(
        &input_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"0.9.0",
  "locale":"en-GB",
  "answers":{"pack_dir":"."},
  "locks":{}
}"#,
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&input_path)
        .arg("--schema-version")
        .arg("1.0.0")
        .output()
        .expect("run validate");
    assert!(
        !output.status.success(),
        "validate should fail on schema mismatch without --migrate"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not match target"));
}

#[test]
fn wizard_validate_schema_mismatch_with_migrate_succeeds() {
    let temp = TempDir::new().expect("tempdir");
    let input_path = temp.path().join("schema_mismatch_answers.json");
    let output_path = temp.path().join("schema_mismatch_migrated.json");
    fs::write(
        &input_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"0.9.0",
  "locale":"en-GB",
  "answers":{"pack_dir":"."},
  "locks":{"k":"v"}
}"#,
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&input_path)
        .arg("--schema-version")
        .arg("1.0.0")
        .arg("--migrate")
        .arg("--emit-answers")
        .arg(&output_path)
        .output()
        .expect("run validate");
    assert!(
        output.status.success(),
        "validate should succeed with migrate on schema mismatch"
    );
    let doc: Value = serde_json::from_slice(&fs::read(&output_path).expect("read migrated"))
        .expect("parse migrated");
    assert_eq!(
        doc.get("schema_version").and_then(Value::as_str),
        Some("1.0.0")
    );
    assert_eq!(
        doc.get("locks")
            .and_then(Value::as_object)
            .and_then(|o| o.get("k"))
            .and_then(Value::as_str),
        Some("v")
    );
}

#[test]
fn wizard_validate_rejects_wrong_wizard_id() {
    let temp = TempDir::new().expect("tempdir");
    let input_path = temp.path().join("wrong_wizard_answers.json");
    fs::write(
        &input_path,
        r#"{
  "wizard_id":"other.wizard",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{"pack_dir":"."},
  "locks":{}
}"#,
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("validate")
        .arg("--answers")
        .arg(&input_path)
        .output()
        .expect("run validate");
    assert!(
        !output.status.success(),
        "validate should fail for unsupported wizard_id"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported wizard_id"));
}

fn write_script(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
