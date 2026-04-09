use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use greentic_pack::builder::{ComponentArtifact, FlowBundle, PackBuilder, PackMeta};
use greentic_pack::messaging::{MessagingAdapter, MessagingAdapterKind, MessagingSection};
use semver::Version;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn inspect_reports_messaging_adapter() {
    let (temp_dir, pack_path, adapter_name) = build_pack_with_messaging();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "inspect",
            "--pack",
            pack_path.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let payload: Value = serde_json::from_slice(&output).expect("valid json");
    let adapters = payload
        .get("manifest")
        .and_then(|m| m.get("meta"))
        .and_then(|meta| meta.get("messaging"))
        .and_then(|val| val.get("adapters"))
        .and_then(|val| val.as_array())
        .expect("adapters array present");
    assert!(
        adapters.iter().any(|adapter| {
            adapter
                .get("name")
                .and_then(|val| val.as_str())
                .map(|name| name == adapter_name)
                .unwrap_or(false)
        }),
        "inspect output should include adapter name"
    );

    drop(temp_dir);
}

#[test]
fn inspect_json_reports_messaging_section() {
    let (temp_dir, pack_path, adapter_name) = build_pack_with_messaging();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "inspect",
            "--pack",
            pack_path.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let payload: Value = serde_json::from_slice(&output).expect("valid json");
    let messaging = payload
        .get("manifest")
        .and_then(|m| m.get("meta"))
        .and_then(|meta| meta.get("messaging"))
        .and_then(|val| val.as_object())
        .expect("messaging section present");
    let adapters = messaging
        .get("adapters")
        .and_then(|val| val.as_array())
        .expect("adapters array present");
    let adapter = adapters
        .iter()
        .find(|entry| {
            entry
                .get("name")
                .and_then(|val| val.as_str())
                .map(|name| name == adapter_name)
                .unwrap_or(false)
        })
        .expect("adapter is listed");
    assert_eq!(
        adapter.get("kind").and_then(|val| val.as_str()),
        Some("ingress")
    );
    assert_eq!(
        adapter.get("component").and_then(|val| val.as_str()),
        Some("demo.component")
    );

    drop(temp_dir);
}

#[test]
fn inspect_accepts_positional_path() {
    let (temp_dir, pack_path, _adapter_name) = build_pack_with_messaging();

    Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "inspect",
            pack_path.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .assert()
        .success();

    Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "doctor",
            pack_path.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .assert()
        .success();

    drop(temp_dir);
}

#[test]
fn doctor_warns_on_unsupported_manifest() {
    let (temp_dir, pack_path, _adapter_name) = build_pack_with_messaging();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "doctor",
            "--pack",
            pack_path.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .output()
        .expect("run doctor");
    assert!(output.status.success(), "doctor should succeed");

    let payload: Value = serde_json::from_slice(&output.stdout).expect("valid json");
    let diagnostics = payload
        .get("validation")
        .and_then(|val| val.get("diagnostics"))
        .and_then(|val| val.as_array())
        .expect("validation diagnostics present");
    assert!(
        diagnostics.iter().any(|diag| {
            diag.get("code")
                .and_then(|val| val.as_str())
                .map(|code| code == "PACK_MANIFEST_UNSUPPORTED")
                .unwrap_or(false)
        }),
        "expected unsupported manifest warning diagnostic"
    );

    drop(temp_dir);
}

#[test]
fn inspect_doctor_uses_dev_binary_overrides_for_flow_and_component_checks() {
    let (temp_dir, pack_path, _adapter_name) = build_pack_with_messaging();
    let flow_exe = temp_dir.path().join("greentic-flow-dev");
    let component_exe = temp_dir.path().join("greentic-component-dev");
    let flow_log = temp_dir.path().join("flow.log");
    let component_log = temp_dir.path().join("component.log");

    write_script(
        &flow_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"$*\" > \"{}\"\n\
cat >/dev/null\n\
printf '{{\"ok\":true}}\\n'\n\
exit 0\n",
            flow_log.display()
        ),
    );
    write_script(
        &component_exe,
        &format!(
            "#!/usr/bin/env bash\n\
echo \"$*\" > \"{}\"\n\
exit 0\n",
            component_log.display()
        ),
    );

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .env("GREENTIC_FLOW_DEV_BIN", &flow_exe)
        .env("GREENTIC_COMPONENT_DEV_BIN", &component_exe)
        .args(["doctor", "--pack", pack_path.to_str().unwrap(), "--json"])
        .output()
        .expect("run doctor");
    assert!(output.status.success(), "doctor should succeed");

    let flow_args = fs::read_to_string(&flow_log).expect("read flow log");
    assert!(flow_args.contains("doctor --json --stdin"));
    let component_args = fs::read_to_string(&component_log).expect("read component log");
    assert!(component_args.contains("doctor"));
    assert!(component_args.contains("--manifest"));

    drop(temp_dir);
}

fn build_pack_with_messaging() -> (TempDir, PathBuf, String) {
    let adapter_name = "demo-adapter".to_string();
    let temp = TempDir::new().expect("temp dir");
    let wasm = temp.path().join("component.wasm");
    fs::write(&wasm, [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]).unwrap();

    let flow_json = json!({
        "id": "main",
        "kind": "flow/v1",
        "entry": "start",
        "nodes": []
    });
    let flow = FlowBundle {
        id: "main".into(),
        kind: "flow/v1".into(),
        entry: "start".into(),
        yaml: "id: main\nentry: start\n".into(),
        json: flow_json.clone(),
        hash_blake3: blake3::hash(&serde_json::to_vec(&flow_json).unwrap())
            .to_hex()
            .to_string(),
        nodes: Vec::new(),
    };

    let meta = PackMeta {
        pack_version: greentic_pack::builder::PACK_VERSION,
        pack_id: "demo.inspect".into(),
        version: Version::parse("0.1.0").unwrap(),
        name: "Inspect Demo".into(),
        kind: None,
        description: None,
        authors: vec!["Greentic".into()],
        license: None,
        homepage: None,
        support: None,
        vendor: None,
        imports: Vec::new(),
        entry_flows: vec!["main".into()],
        created_at_utc: "2024-01-01T00:00:00Z".into(),
        events: None,
        repo: None,
        messaging: Some(MessagingSection {
            adapters: Some(vec![MessagingAdapter {
                name: adapter_name.clone(),
                kind: MessagingAdapterKind::Ingress,
                component: "demo.component".into(),
                default_flow: Some("main".into()),
                custom_flow: None,
                capabilities: None,
            }]),
        }),
        interfaces: Vec::new(),
        annotations: serde_json::Map::new(),
        distribution: None,
        components: Vec::new(),
    };

    let component = ComponentArtifact {
        name: "demo.component".into(),
        version: Version::parse("1.0.0").unwrap(),
        wasm_path: wasm.clone(),
        schema_json: None,
        manifest_json: Some(
            json!({
                "name": "demo.component",
                "artifacts": [],
                "hashes": {},
                "describe_export": true,
                "config_schema": {}
            })
            .to_string(),
        ),
        capabilities: None,
        world: Some("greentic:demo@1.0.0".into()),
        hash_blake3: None,
    };

    let pack_path = temp.path().join("demo.gtpack");
    let builder = PackBuilder::new(meta)
        .with_flow(flow)
        .with_component(component);
    builder.build(&pack_path).expect("build pack");

    // sanity: the builder adds signatures in dev mode, keep files alive until inspect runs
    assert!(pack_path.exists());
    assert!(fs::read(&pack_path).is_ok());

    (temp, pack_path, adapter_name)
}

fn write_script(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}
