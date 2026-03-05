use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};

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
        None
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
    assert!(calls.contains("flow:wizard edit --flow flows/main.ygtc"));
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
    assert!(calls.contains("flow:wizard edit --flow flows/main.ygtc"));
    assert!(calls.contains("component:wizard --project-root . --execution execute --qa-answers"));
    assert!(calls.contains("self:doctor --in"));
    assert!(calls.contains("self:build --in"));
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
while [ \"$#\" -gt 0 ]; do\n\
  if [ \"$1\" = \"--emit-answers\" ]; then emit=\"$2\"; shift 2; continue; fi\n\
  if [ \"$1\" = \"--answers-file\" ]; then answers=\"$2\"; shift 2; continue; fi\n\
  shift\n\
done\n\
if [ -n \"$emit\" ]; then printf '{{\"flow\":\"dry-run\"}}' > \"$emit\"; fi\n\
if [ -n \"$answers\" ]; then touch \"$PWD/flow.replayed\"; fi\n\
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
        None
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
    assert!(pack_dir.join("component.replayed").exists());
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

fn env_guard() -> MutexGuard<'static, ()> {
    match env_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
