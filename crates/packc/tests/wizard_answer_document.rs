use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::{Value, json};
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
fn wizard_run_emit_answers_records_extension_operation() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("extension_answers.json");
    let pack_dir = temp.path().join("control-pack");
    let input = format!(
        "3\nfixture://extensions.json\n7\n1\n{}\n\n\ncontrol-entry\n0\n\n\n\n\n0\n0\n\n2\n0\n",
        pack_dir.display()
    );

    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
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
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "wizard run should succeed");

    let doc: Value = serde_json::from_slice(&fs::read(&answers_path).expect("read answers"))
        .expect("parse answers");
    let answers = doc
        .get("answers")
        .and_then(Value::as_object)
        .expect("answers object");
    assert_eq!(
        answers.get("extension_operation").and_then(Value::as_str),
        Some("create_extension_pack")
    );
    assert_eq!(
        answers.get("extension_catalog_ref").and_then(Value::as_str),
        Some("fixture://extensions.json")
    );
    assert_eq!(
        answers.get("extension_type_id").and_then(Value::as_str),
        Some("control")
    );
    assert_eq!(
        answers.get("extension_template_id").and_then(Value::as_str),
        Some("control-basic")
    );
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
    let _guard = env_guard();
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
    let _guard = env_guard();
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
    let _guard = env_guard();
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
    let _guard = env_guard();
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

#[test]
fn wizard_run_with_invalid_answers_file_writes_localized_error_and_continues() {
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("invalid_answers.json");
    fs::write(
        &answers_path,
        r#"{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{"pack_dir":".","run_build":"yes"},
  "locks":{}
}"#,
    )
    .expect("write invalid answers file");

    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("--answers")
        .arg(&answers_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wizard --answers");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"0\n")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");

    assert!(
        output.status.success(),
        "wizard should continue interactively after invalid answers"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Wizard answers error: answers.run_build must be a boolean"));
    assert!(stdout.contains("Main Menu"));
    assert!(stdout.contains("0) Exit"));
    assert!(
        String::from_utf8_lossy(&output.stderr).trim().is_empty(),
        "wizard should keep the error in wizard output, not stderr"
    );
}

#[test]
fn wizard_run_dry_run_records_choices_without_side_effects() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("pack");
    let answers_path = temp.path().join("dry_run_answers.json");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");
    fs::create_dir_all(&pack_dir).expect("create pack dir");
    write_script(
        &flow_exe,
        r#"#!/usr/bin/env bash
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--emit-answers" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ -n "$out" ]; then
  printf '{"flow":"ok"}' > "$out"
fi
exit 0
"#,
    );
    write_script(
        &component_exe,
        r#"#!/usr/bin/env bash
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--qa-answers-out" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ -n "$out" ]; then
  printf '{"component":"ok"}' > "$out"
fi
exit 0
"#,
    );

    let input = format!(
        "2\n{}\n1\n2\n2\n2\n3\n2\n4\n./demo-sign.pem\nM\n0\n",
        pack_dir.display()
    );
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
        .arg("--emit-answers")
        .arg(&answers_path)
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wizard run dry-run");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(
        output.status.success(),
        "wizard run --dry-run should succeed"
    );

    let doc: Value = serde_json::from_slice(&fs::read(&answers_path).expect("read answers"))
        .expect("parse answers");
    let answers = doc
        .get("answers")
        .and_then(Value::as_object)
        .expect("answers object");
    assert_eq!(
        answers.get("pack_dir").and_then(Value::as_str),
        Some(pack_dir.to_string_lossy().as_ref())
    );
    assert_eq!(
        answers.get("run_delegate_flow").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers
            .get("run_delegate_component")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers.get("run_doctor").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers.get("run_build").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(answers.get("sign").and_then(Value::as_bool), Some(true));
    assert_eq!(
        answers.get("sign_key_path").and_then(Value::as_str),
        Some("./demo-sign.pem")
    );
    assert_eq!(answers.get("dry_run").and_then(Value::as_bool), Some(true));
    assert_eq!(
        answers.get("mode").and_then(Value::as_str),
        Some("interactive-dry-run")
    );
    assert_eq!(
        answers
            .get("flow_wizard_answers")
            .and_then(Value::as_object)
            .and_then(|v| v.get("flow"))
            .and_then(Value::as_str),
        Some("ok")
    );
    assert_eq!(
        answers
            .get("component_wizard_answers")
            .and_then(Value::as_object)
            .and_then(|v| v.get("component"))
            .and_then(Value::as_str),
        Some("ok")
    );
    let selected = answers
        .get("selected_actions")
        .and_then(Value::as_array)
        .expect("selected_actions array");
    let selected_values = selected
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(selected_values.contains(&"main.update_application_pack"));
    assert!(selected_values.contains(&"update_application_pack.edit_flows"));
    assert!(selected_values.contains(&"update_application_pack.add_edit_components"));
    assert!(selected_values.contains(&"update_application_pack.run_update_validate"));
    assert!(selected_values.contains(&"update_application_pack.sign"));
}

#[test]
fn wizard_apply_replays_recorded_delegate_and_pipeline_actions() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("pack");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");
    let answers_path = temp.path().join("answers.json");
    fs::create_dir_all(&pack_dir).expect("create pack dir");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"self:$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &flow_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"flow:$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &component_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"component:$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "run_delegate_flow":true,
    "run_delegate_component":true,
    "run_doctor":true,
    "run_build":true,
    "sign":true,
    "sign_key_path":"./demo-sign.pem"
  }},
  "locks":{{}}
}}"#,
            pack_dir.display()
        ),
    )
    .expect("write answers");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
        std::env::set_var("GREENTIC_FLOW_BIN", flow_exe.as_os_str());
        std::env::set_var("GREENTIC_COMPONENT_BIN", component_exe.as_os_str());
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
        std::env::remove_var("GREENTIC_FLOW_BIN");
        std::env::remove_var("GREENTIC_COMPONENT_BIN");
    }
    assert!(output.status.success(), "wizard apply should succeed");

    let calls = fs::read_to_string(&log_path).expect("read call log");
    assert!(calls.contains("flow:wizard ."));
    assert!(calls.contains("component:wizard"));
    assert!(calls.contains("self:doctor --in"));
    assert!(calls.contains("self:build --in"));
    assert!(calls.contains("self:sign --pack"));
}

#[test]
fn wizard_apply_with_nested_answers_can_scaffold_and_replay_subwizards() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("new-pack");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");
    let answers_path = temp.path().join("answers.json");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"self:$*\" >> \"{}\"\n\
if [ \"$1\" = \"new\" ] && [ \"$2\" = \"--dir\" ]; then\n\
  mkdir -p \"$3\"\n\
fi\n\
exit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &flow_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"flow:$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &component_exe,
        &format!(
            "#!/usr/bin/env bash\necho \"component:$*\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    );
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "create_pack_scaffold":true,
    "create_pack_id":"my-pack",
    "run_delegate_flow":true,
    "run_delegate_component":true,
    "flow_wizard_answers":{{"flow":"ok"}},
    "component_wizard_answers":{{"component":"ok"}},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display()
        ),
    )
    .expect("write answers");

    unsafe {
        std::env::set_var("GREENTIC_PACK_WIZARD_SELF_EXE", self_exe.as_os_str());
        std::env::set_var("GREENTIC_FLOW_BIN", flow_exe.as_os_str());
        std::env::set_var("GREENTIC_COMPONENT_BIN", component_exe.as_os_str());
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
        std::env::remove_var("GREENTIC_FLOW_BIN");
        std::env::remove_var("GREENTIC_COMPONENT_BIN");
    }
    assert!(output.status.success(), "wizard apply should succeed");
    assert!(pack_dir.is_dir(), "apply should scaffold pack directory");

    let calls = fs::read_to_string(&log_path).expect("read call log");
    assert!(calls.contains("self:new --dir"));
    assert!(calls.contains("flow:wizard . --execution execute --answers-file"));
    assert!(calls.contains("component:wizard --project-root . --execution execute --qa-answers"));
    assert!(calls.contains("self:update --in"));
    assert!(calls.contains("self:doctor --in"));
    assert!(calls.contains("self:build --in"));
    let update_idx = calls.find("self:update --in").expect("update call");
    let doctor_idx = calls.find("self:doctor --in").expect("doctor call");
    let resolve_idx = calls.find("self:resolve --in").expect("resolve call");
    let build_idx = calls.find("self:build --in").expect("build call");
    assert!(
        update_idx < doctor_idx,
        "update should happen before doctor"
    );
    assert!(
        doctor_idx < resolve_idx,
        "doctor should happen before resolve"
    );
    assert!(
        resolve_idx < build_idx,
        "resolve should happen before build"
    );
}

#[test]
fn wizard_apply_control_extension_answers_is_deterministic() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("control-pack");
    let answers_path = temp.path().join("control_answers.json");
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "extension_operation":"create_extension_pack",
    "extension_catalog_ref":"fixture://extensions.json",
    "extension_type_id":"control",
    "extension_template_id":"control-basic",
    "extension_template_qa_answers":{{
      "display_name":"Routing ingress control chain",
      "pack_id":"routing.ingress.control.chain"
    }},
    "extension_edit_answers":{{
      "entry_label":"control",
      "create_offer":"false",
      "offer_id":"control-offer",
      "cap_id":"greentic.cap.control.chain.v1",
      "component_ref":"controller",
      "op":"apply",
      "version":"v1",
      "priority":"0",
      "requires_setup":"false",
      "qa_ref":"qa/control-setup.json",
      "hook_op_names":""
    }},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display()
        ),
    )
    .expect("write answers");

    let first = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run first apply");
    assert!(first.status.success(), "first apply should succeed");

    let pack_yaml_before = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack yaml");
    let extension_before =
        fs::read_to_string(pack_dir.join("extensions/control.json")).expect("read extension json");
    assert!(pack_dir.join("flows").is_dir());
    assert!(pack_dir.join("components").is_dir());
    assert!(pack_dir.join("i18n").is_dir());
    assert!(pack_dir.join("assets").is_dir());
    assert!(pack_dir.join("qa").is_dir());
    assert!(pack_dir.join("assets/README.md").exists());
    assert!(pack_dir.join("qa/README.md").exists());
    assert!(pack_yaml_before.contains("greentic.ext.capabilities.v1"));
    assert!(pack_yaml_before.contains("offers: []"));

    let second = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run second apply");
    assert!(second.status.success(), "second apply should succeed");

    let pack_yaml_after = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack yaml");
    let extension_after =
        fs::read_to_string(pack_dir.join("extensions/control.json")).expect("read extension json");
    assert_eq!(
        pack_yaml_before, pack_yaml_after,
        "pack.yaml should be idempotent"
    );
    assert_eq!(
        extension_before, extension_after,
        "extensions/control.json should be idempotent"
    );
}

#[test]
fn wizard_apply_runtime_capability_answers_scaffolds_replayable_pack() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("runtime-pack");
    let answers_path = temp.path().join("runtime_answers.json");
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "extension_operation":"create_extension_pack",
    "extension_catalog_ref":"{}",
    "extension_type_id":"runtime-capability",
    "extension_template_id":"runtime-capability-basic",
    "extension_template_qa_answers":{{
      "display_name":"Runtime capability extension",
      "pack_id":"runtime.capability.extension"
    }},
    "extension_edit_answers":{{
      "entry_label":"runtime",
      "create_offer":"true",
      "offer_id":"runtime-offer",
      "cap_id":"greentic.cap.runtime.execution.v1",
      "component_ref":"runtime",
      "op":"run",
      "version":"v1",
      "priority":"0",
      "requires_setup":"false",
      "qa_ref":"qa/runtime-setup.json",
      "hook_op_names":""
    }},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display(),
            default_catalog_ref()
        ),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run apply");
    assert!(output.status.success(), "runtime apply should succeed");

    assert!(pack_dir.join("assets/runtime/README.md").exists());
    assert!(pack_dir.join("assets/examples/runtime-input.json").exists());
    assert!(pack_dir.join("components/runtime/component.wasm").exists());
    assert!(pack_dir.join("extensions/runtime-capability.json").exists());

    let pack_yaml = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack yaml");
    assert!(pack_yaml.contains("greentic.ext.capabilities.v1"));
    assert!(pack_yaml.contains("runtime-offer"));
    assert!(pack_yaml.contains("id: runtime"));
}

#[test]
fn wizard_apply_contract_answers_scaffolds_contract_assets() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("contract-pack");
    let answers_path = temp.path().join("contract_answers.json");
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "extension_operation":"create_extension_pack",
    "extension_catalog_ref":"{}",
    "extension_type_id":"contract",
    "extension_template_id":"contract-basic",
    "extension_template_qa_answers":{{
      "display_name":"Contract extension",
      "pack_id":"contract.extension"
    }},
    "extension_edit_answers":{{
      "entry_label":"contract",
      "create_offer":"false",
      "offer_id":"contract-offer",
      "cap_id":"greentic.cap.contract.bundle.v1",
      "component_ref":"contract-hook",
      "op":"validate",
      "version":"v1",
      "priority":"0",
      "requires_setup":"false",
      "qa_ref":"qa/contract-setup.json",
      "hook_op_names":""
    }},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display(),
            default_catalog_ref()
        ),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run apply");
    assert!(output.status.success(), "contract apply should succeed");

    assert!(pack_dir.join("assets/contracts/transitions.yaml").exists());
    assert!(pack_dir.join("assets/contracts/rules.yaml").exists());
    assert!(
        pack_dir
            .join("assets/examples/contract.example.json")
            .exists()
    );
    assert!(
        pack_dir
            .join("components/contract-hook/component.wasm")
            .exists()
    );
    assert!(pack_dir.join("extensions/contract.json").exists());

    let pack_yaml = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack yaml");
    assert!(pack_yaml.contains("offers: []"));
    assert!(pack_yaml.contains("id: contract-hook"));
}

#[test]
fn wizard_apply_ops_answers_scaffolds_ops_bundle() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("ops-pack");
    let answers_path = temp.path().join("ops_answers.json");
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "extension_operation":"create_extension_pack",
    "extension_catalog_ref":"{}",
    "extension_type_id":"ops",
    "extension_template_id":"ops-basic",
    "extension_template_qa_answers":{{
      "display_name":"Ops extension",
      "pack_id":"ops.extension"
    }},
    "extension_edit_answers":{{
      "entry_label":"ops",
      "create_offer":"true",
      "offer_id":"ops-offer",
      "cap_id":"greentic.cap.ops.execution.v1",
      "component_ref":"ops-provider",
      "op":"execute",
      "version":"v1",
      "priority":"0",
      "requires_setup":"false",
      "qa_ref":"qa/ops-setup.json",
      "hook_op_names":"before,after"
    }},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display(),
            default_catalog_ref()
        ),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run apply");
    assert!(output.status.success(), "ops apply should succeed");

    assert!(pack_dir.join("assets/ops/ops.yaml").exists());
    assert!(pack_dir.join("assets/examples/ops-input.json").exists());
    assert!(
        pack_dir
            .join("components/ops-provider/component.wasm")
            .exists()
    );
    assert!(pack_dir.join("extensions/ops.json").exists());

    let pack_yaml = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack yaml");
    assert!(pack_yaml.contains("ops-offer"));
    assert!(pack_yaml.contains("id: ops-provider"));
}

#[test]
fn wizard_run_dry_run_then_apply_deployer_destroy_answers_succeeds() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("pack-wizard-sample.json");
    let pack_dir = temp.path().join("deploy-test");
    let input = format!(
        "3\nn\n11\n1\n{}\n\n\ndeployer\ngreentic.deployer.example.v1\ndeployer\ngenerate,plan,apply,destroy,status,rollback\n2\n0\n",
        pack_dir.display()
    );

    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
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
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "wizard run should succeed");

    let apply = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard apply");
    assert!(apply.status.success(), "wizard apply should succeed");

    let extension =
        fs::read_to_string(pack_dir.join("extensions/deployer.json")).expect("read extension");
    assert!(extension.contains("\"destroy\""));
    assert!(!extension.contains("\"remove\""));
    assert!(pack_dir.join("flows/destroy.ygtc").exists());
    assert!(!pack_dir.join("flows/remove.ygtc").exists());
}

#[test]
fn wizard_apply_deployer_legacy_remove_answers_fail() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("legacy-deployer-pack");
    let answers_path = temp.path().join("legacy_deployer_answers.json");
    fs::write(
        &answers_path,
        format!(
            r#"{{
  "wizard_id":"greentic-pack.wizard.run",
  "schema_id":"greentic-pack.wizard.answers",
  "schema_version":"1.0.0",
  "locale":"en-GB",
  "answers":{{
    "pack_dir":"{}",
    "extension_operation":"create_extension_pack",
    "extension_catalog_ref":"{}",
    "extension_type_id":"deployer",
    "extension_template_id":"deployer-basic",
    "extension_template_qa_answers":{{
      "display_name":"Generic deployer extension",
      "pack_id":"deployer.extension"
    }},
    "extension_edit_answers":{{
      "entry_label":"deployer",
      "contract_id":"greentic.deployer.example.v1",
      "component_ref":"deployer",
      "supported_ops":"generate,plan,apply,remove,status,rollback"
    }},
    "run_doctor":true,
    "run_build":true,
    "sign":false
  }},
  "locks":{{}}
}}"#,
            pack_dir.display(),
            default_catalog_ref()
        ),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .output()
        .expect("run wizard apply");
    assert!(
        !output.status.success(),
        "legacy remove answers should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("flows/remove.ygtc"));
    let extension =
        fs::read_to_string(pack_dir.join("extensions/deployer.json")).expect("read extension");
    assert!(extension.contains("\"remove\""));
    assert!(!extension.contains("\"destroy\""));
}

#[test]
fn wizard_dry_run_flow_child_exit_returns_gracefully_to_pack_menu() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("answers.json");
    let self_exe = temp.path().join("greentic-pack-self");
    let flow_exe = temp.path().join("greentic-flow");

    write_script(
        &self_exe,
        "#!/usr/bin/env bash\nif [ \"$1\" = \"new\" ] && [ \"$2\" = \"--dir\" ]; then mkdir -p \"$3\"; fi\nexit 0\n",
    );
    write_script(&flow_exe, "#!/usr/bin/env bash\nexit 1\n");

    let input = format!(
        "1\nmy-pack\n{}\n1\n0\n0\n",
        temp.path().join("pack").display()
    );
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
        .arg("--emit-answers")
        .arg(&answers_path)
        .env("GREENTIC_PACK_WIZARD_SELF_EXE", &self_exe)
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wizard");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "wizard should return gracefully");
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(rendered.contains("child wizard exited") || rendered.contains("child-wizard"));
}

#[test]
fn wizard_dry_run_component_child_exit_returns_gracefully_to_pack_menu() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let answers_path = temp.path().join("answers.json");
    let self_exe = temp.path().join("greentic-pack-self");
    let component_exe = temp.path().join("greentic-component");

    write_script(
        &self_exe,
        "#!/usr/bin/env bash\nif [ \"$1\" = \"new\" ] && [ \"$2\" = \"--dir\" ]; then mkdir -p \"$3\"; fi\nexit 0\n",
    );
    write_script(&component_exe, "#!/usr/bin/env bash\nexit 1\n");

    let input = format!(
        "1\nmy-pack\n{}\n2\n0\n0\n",
        temp.path().join("pack").display()
    );
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
        .arg("--emit-answers")
        .arg(&answers_path)
        .env("GREENTIC_PACK_WIZARD_SELF_EXE", &self_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wizard");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "wizard should return gracefully");
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(rendered.contains("child wizard exited") || rendered.contains("child-wizard"));
}

#[test]
fn wizard_dry_run_export_and_replay_constructs_pack_flow_component() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("target-pack");
    let answers_path = temp.path().join("dry_run_answers.json");
    let log_path = temp.path().join("calls.log");
    let self_exe = temp.path().join("greentic-pack-self");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");

    write_script(
        &self_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"self:$*\" >> \"{}\"\n\
if [ \"$1\" = \"new\" ] && [ \"$2\" = \"--dir\" ]; then mkdir -p \"$3\"; fi\n\
exit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &flow_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"flow:$*\" >> \"{}\"\n\
emit=\"\"\n\
answers=\"\"\n\
mode=\"\"\n\
while [ \"$#\" -gt 0 ]; do\n\
  if [ \"$1\" = \"--execution\" ]; then mode=\"$2\"; shift 2; continue; fi\n\
  if [ \"$1\" = \"--emit-answers\" ]; then emit=\"$2\"; shift 2; continue; fi\n\
  if [ \"$1\" = \"--answers-file\" ]; then answers=\"$2\"; shift 2; continue; fi\n\
  shift\n\
done\n\
if [ \"$mode\" = \"dry-run\" ] && [ -n \"$emit\" ]; then printf '{{\"flow\":\"dry-run\"}}' > \"$emit\"; fi\n\
if [ \"$mode\" = \"execute\" ] && [ -n \"$answers\" ]; then touch \"$PWD/flow.replayed\"; fi\n\
exit 0\n",
            log_path.display()
        ),
    );
    write_script(
        &component_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"component:$*\" >> \"{}\"\n\
out=\"\"\n\
in=\"\"\n\
while [ \"$#\" -gt 0 ]; do\n\
  if [ \"$1\" = \"--qa-answers-out\" ]; then out=\"$2\"; shift 2; continue; fi\n\
  if [ \"$1\" = \"--qa-answers\" ]; then in=\"$2\"; shift 2; continue; fi\n\
  shift\n\
done\n\
if [ -n \"$out\" ]; then printf '{{\"component\":\"dry-run\"}}' > \"$out\"; fi\n\
if [ -n \"$in\" ]; then touch \"$PWD/component.replayed\"; fi\n\
exit 0\n",
            log_path.display()
        ),
    );

    let input = format!("1\nmy-pack\n{}\n1\n2\n3\n2\n0\n", pack_dir.display());
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("run")
        .arg("--dry-run")
        .arg("--emit-answers")
        .arg(&answers_path)
        .env("GREENTIC_PACK_WIZARD_SELF_EXE", &self_exe)
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dry-run wizard");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait output");
    assert!(output.status.success(), "dry-run should succeed");

    let doc: Value = serde_json::from_slice(&fs::read(&answers_path).expect("read answers"))
        .expect("parse answers");
    let answers = doc
        .get("answers")
        .and_then(Value::as_object)
        .expect("answers object");
    assert_eq!(
        answers.get("create_pack_id").and_then(Value::as_str),
        Some("my-pack")
    );
    assert_eq!(
        answers.get("create_pack_scaffold").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers.get("run_delegate_flow").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers
            .get("run_delegate_component")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers.get("run_doctor").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers.get("run_build").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        answers
            .get("flow_wizard_answers")
            .and_then(Value::as_object)
            .and_then(|v| v.get("flow"))
            .and_then(Value::as_str),
        Some("dry-run")
    );
    assert_eq!(
        answers
            .get("component_wizard_answers")
            .and_then(Value::as_object)
            .and_then(|v| v.get("component"))
            .and_then(Value::as_str),
        Some("dry-run")
    );

    let apply_output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .env("GREENTIC_PACK_WIZARD_SELF_EXE", &self_exe)
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .output()
        .expect("run apply");
    assert!(apply_output.status.success(), "apply should succeed");
    assert!(pack_dir.is_dir(), "pack should be created from answers");
    assert!(pack_dir.join("flow.replayed").exists());
    assert!(pack_dir.join("component.replayed").exists());
    let calls = fs::read_to_string(&log_path).expect("read calls");
    assert!(calls.contains("flow:wizard . --execution dry-run --emit-answers"));
    assert!(calls.contains("flow:wizard . --execution execute --answers-file"));
}

#[test]
fn wizard_schema_embeds_pack_flow_and_component_contracts() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");

    write_script(
        &flow_exe,
        r#"#!/usr/bin/env bash
printf '%s\n' "$*" > /dev/null
cat <<'JSON'
{"schema_id":"greentic-flow.wizard.plan","schema_version":"2.0.0","type":"object","properties":{"actions":{"type":"array"}}}
JSON
exit 0
"#,
    );
    write_script(
        &component_exe,
        r#"#!/usr/bin/env bash
mode=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--mode" ]; then mode="$2"; shift 2; continue; fi
  shift
done
printf '{"title":"component-%s","type":"object","properties":{"answers":{"type":"object","properties":{"mode":{"const":"%s"}}}}}\n' "$mode" "$mode"
"#,
    );

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("--schema")
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .output()
        .expect("run wizard --schema");
    assert!(output.status.success(), "wizard --schema should succeed");

    let schema: Value = serde_json::from_slice(&output.stdout).expect("parse wizard schema");
    assert_eq!(
        schema
            .get("properties")
            .and_then(|v| v.get("schema_id"))
            .and_then(|v| v.get("const"))
            .and_then(Value::as_str),
        Some("greentic-pack.wizard.answers")
    );
    assert_eq!(
        schema
            .get("properties")
            .and_then(|v| v.get("answers"))
            .and_then(|v| v.get("properties"))
            .and_then(|v| v.get("flow_wizard_answers"))
            .and_then(|v| v.get("anyOf"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        schema
            .get("$defs")
            .and_then(|v| v.get("greentic_flow_wizard_runtime_schema"))
            .and_then(|v| v.get("schema_id"))
            .and_then(Value::as_str),
        Some("greentic-flow.wizard.plan")
    );
    assert_eq!(
        schema
            .get("$defs")
            .and_then(|v| v.get("greentic_component_wizard_create"))
            .and_then(|v| v.get("title"))
            .and_then(Value::as_str),
        Some("component-create")
    );
    assert_eq!(
        schema
            .get("$defs")
            .and_then(|v| v.get("greentic_component_wizard_doctor"))
            .and_then(|v| v.get("title"))
            .and_then(Value::as_str),
        Some("component-doctor")
    );
    let step_comment = schema
        .get("$defs")
        .and_then(|v| v.get("greentic_flow_step_answers"))
        .and_then(|v| v.get("$comment"))
        .and_then(Value::as_str)
        .expect("flow step comment");
    assert!(step_comment.contains("component-schema <file/oci/repo/store>.wasm"));
    assert!(step_comment.contains("default|setup|update|remove"));
}

#[test]
fn wizard_schema_with_answers_uses_pack_specific_flow_schema_and_dev_overrides() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("pack");
    let answers_path = temp.path().join("answers.json");
    let flow_dev_exe = temp.path().join("greentic-flow-dev");
    let component_dev_exe = temp.path().join("greentic-component-dev");
    let flow_log = temp.path().join("flow.log");
    let component_log = temp.path().join("component.log");
    fs::create_dir_all(&pack_dir).expect("create pack dir");

    write_script(
        &flow_dev_exe,
        &format!(
            r#"#!/usr/bin/env bash
echo "$*" > "{}"
if [ "$1" != "wizard" ] || [ "$2" != "--schema" ] || [ "$3" != "{}" ] || [ "$4" != "--answers" ] || [ ! -f "$5" ]; then
  exit 1
fi
cat <<'JSON'
{{"schema_id":"greentic-flow.wizard.plan","schema_version":"2.0.0","title":"pack-specific-flow-schema","properties":{{"actions":{{"type":"array","prefixItems":[{{"action":"from-pack"}}]}}}}}}
JSON
exit 0
"#,
            flow_log.display(),
            pack_dir.display()
        ),
    );
    write_script(
        &component_dev_exe,
        &format!(
            r#"#!/usr/bin/env bash
echo "$*" >> "{}"
mode=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--mode" ]; then mode="$2"; shift 2; continue; fi
  shift
done
printf '{{"title":"component-%s","type":"object"}}\n' "$mode"
"#,
            component_log.display()
        ),
    );

    fs::write(
        &answers_path,
        serde_json::to_vec_pretty(&json!({
            "wizard_id": "greentic-pack.wizard.run",
            "schema_id": "greentic-pack.wizard.answers",
            "schema_version": "1.0.0",
            "locale": "en-GB",
            "answers": {
                "pack_dir": pack_dir,
                "run_delegate_flow": true,
                "run_delegate_component": false,
                "run_doctor": false,
                "run_build": false,
                "sign": false,
                "flow_wizard_answers": {
                    "schema_id": "greentic-flow.wizard.plan",
                    "schema_version": "2.0.0",
                    "actions": [
                        {
                            "action": "add-step",
                            "flow": "flows/order_tracking_flow.ygtc",
                            "component": "components/order-status.wasm",
                            "mode": "setup",
                            "answers": {
                                "tenant": "acme"
                            }
                        }
                    ]
                }
            },
            "locks": {}
        }))
        .expect("serialize answers"),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("--schema")
        .arg("--answers")
        .arg(&answers_path)
        .env("GREENTIC_FLOW_DEV_BIN", &flow_dev_exe)
        .env("GREENTIC_COMPONENT_DEV_BIN", &component_dev_exe)
        .output()
        .expect("run wizard --schema --answers");
    assert!(output.status.success(), "wizard --schema should succeed");

    let schema: Value = serde_json::from_slice(&output.stdout).expect("parse wizard schema");
    assert_eq!(
        schema
            .get("$defs")
            .and_then(|v| v.get("greentic_flow_wizard_runtime_schema"))
            .and_then(|v| v.get("title"))
            .and_then(Value::as_str),
        Some("pack-specific-flow-schema")
    );
    let flow_args = fs::read_to_string(&flow_log).expect("read flow log");
    assert!(flow_args.contains("wizard --schema"));
    assert!(flow_args.contains(&pack_dir.display().to_string()));
    assert!(flow_args.contains("--answers"));
    let component_args = fs::read_to_string(&component_log).expect("read component log");
    assert!(component_args.contains("wizard --schema --mode create"));
}

#[test]
fn wizard_apply_replays_nested_flow_step_and_component_answers_without_dropping_them() {
    let _guard = env_guard();
    let temp = TempDir::new().expect("tempdir");
    let pack_dir = temp.path().join("pack");
    let answers_path = temp.path().join("answers.json");
    let self_exe = temp.path().join("greentic-pack-self");
    let flow_exe = temp.path().join("greentic-flow");
    let component_exe = temp.path().join("greentic-component");
    fs::create_dir_all(&pack_dir).expect("create pack dir");

    write_script(&self_exe, "#!/usr/bin/env bash\nexit 0\n");
    write_script(
        &flow_exe,
        r#"#!/usr/bin/env bash
answers=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--answers-file" ]; then answers="$2"; shift 2; continue; fi
  shift
done
if [ -n "$answers" ]; then cp "$answers" "$PWD/flow.replayed.json"; fi
exit 0
"#,
    );
    write_script(
        &component_exe,
        r#"#!/usr/bin/env bash
answers=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--qa-answers" ]; then answers="$2"; shift 2; continue; fi
  shift
done
if [ -n "$answers" ]; then cp "$answers" "$PWD/component.replayed.json"; fi
exit 0
"#,
    );
    fs::write(
        &answers_path,
        serde_json::to_vec_pretty(&json!({
            "wizard_id": "greentic-pack.wizard.run",
            "schema_id": "greentic-pack.wizard.answers",
            "schema_version": "1.0.0",
            "locale": "en-GB",
            "answers": {
                "pack_dir": pack_dir,
                "run_delegate_flow": true,
                "run_delegate_component": true,
                "run_doctor": false,
                "run_build": false,
                "sign": false,
                "flow_wizard_answers": {
                    "schema_id": "greentic-flow.wizard.plan",
                    "schema_version": "2.0.0",
                    "actions": [
                        {
                            "action": "add-step",
                            "flow": "flows/order_tracking_flow.ygtc",
                            "component": "components/order-status.wasm",
                            "mode": "setup",
                            "answers": {
                                "tenant": "acme",
                                "card": {
                                    "template": "adaptive-card"
                                }
                            }
                        }
                    ]
                },
                "component_wizard_answers": {
                    "wizard_id": "greentic-component.wizard.run",
                    "schema_id": "greentic-component.wizard.run",
                    "schema_version": "1.0.0",
                    "answers": {
                        "mode": "create",
                        "fields": {
                            "component_name": "order-status"
                        }
                    },
                    "locks": {}
                }
            },
            "locks": {}
        }))
        .expect("serialize answers"),
    )
    .expect("write answers");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .arg("wizard")
        .arg("apply")
        .arg("--answers")
        .arg(&answers_path)
        .env("GREENTIC_PACK_WIZARD_SELF_EXE", &self_exe)
        .env("GREENTIC_FLOW_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_BIN", &component_exe)
        .output()
        .expect("run wizard apply");
    assert!(output.status.success(), "wizard apply should succeed");

    let replayed_flow: Value = serde_json::from_slice(
        &fs::read(pack_dir.join("flow.replayed.json")).expect("read replayed flow answers"),
    )
    .expect("parse replayed flow answers");
    assert_eq!(
        replayed_flow
            .get("actions")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("answers"))
            .and_then(|v| v.get("card"))
            .and_then(|v| v.get("template"))
            .and_then(Value::as_str),
        Some("adaptive-card")
    );
    let replayed_component: Value = serde_json::from_slice(
        &fs::read(pack_dir.join("component.replayed.json"))
            .expect("read replayed component answers"),
    )
    .expect("parse replayed component answers");
    assert_eq!(
        replayed_component
            .get("answers")
            .and_then(|v| v.get("fields"))
            .and_then(|v| v.get("component_name"))
            .and_then(Value::as_str),
        Some("order-status")
    );
}

fn write_script(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn default_catalog_ref() -> String {
    format!(
        "file://{}",
        workspace_root()
            .join("docs")
            .join("extensions_capability_packs.catalog.v1.json")
            .display()
    )
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn env_guard() -> MutexGuard<'static, ()> {
    match env_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
