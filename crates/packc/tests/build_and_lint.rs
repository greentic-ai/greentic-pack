use assert_cmd::prelude::*;
use greentic_pack::pack_lock::{LockedComponent, PackLockV1, write_pack_lock};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn copy_weather_demo() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir");
    let src = workspace_root().join("examples/weather-demo");
    let dest = temp.path().join("weather-demo");
    std::fs::create_dir_all(&dest).expect("create weather-demo dir");
    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        let rel = entry.path().strip_prefix(&src).expect("relative path");
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).expect("create dir");
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("create parent");
            }
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
    (temp, dest)
}

fn write_weather_summary(pack_dir: &Path) {
    let summary_path = pack_dir.join("flows/weather_bot.ygtc.resolve.summary.json");
    let parent = summary_path
        .parent()
        .expect("summary parent exists")
        .to_path_buf();
    std::fs::create_dir_all(&parent).expect("create flows dir");
    let qa_path = pack_dir.join("components/qa.process/component.wasm");
    let mcp_path = pack_dir.join("components/mcp.exec/component.wasm");
    let templating_path = pack_dir.join("components/templating.handlebars/component.wasm");
    let qa_digest = format!(
        "sha256:{:x}",
        Sha256::digest(std::fs::read(&qa_path).expect("read qa wasm"))
    );
    let mcp_digest = format!(
        "sha256:{:x}",
        Sha256::digest(std::fs::read(&mcp_path).expect("read mcp wasm"))
    );
    let templating_digest = format!(
        "sha256:{:x}",
        Sha256::digest(std::fs::read(&templating_path).expect("read templating wasm"))
    );
    let doc = json!({
        "schema_version": 1,
        "flow": "weather_bot.ygtc",
        "nodes": {
            "collect_location": {
                "component_id": "qa.process",
                "source": { "kind": "local", "path": "../components/qa.process/component.wasm" },
                "digest": qa_digest
            },
            "forecast_weather": {
                "component_id": "mcp.exec",
                "source": { "kind": "local", "path": "../components/mcp.exec/component.wasm" },
                "digest": mcp_digest
            },
            "weather_text": {
                "component_id": "templating.handlebars",
                "source": { "kind": "local", "path": "../components/templating.handlebars/component.wasm" },
                "digest": templating_digest
            }
        }
    });
    std::fs::write(summary_path, serde_json::to_vec_pretty(&doc).unwrap()).expect("write summary");
}

fn write_weather_lock(pack_dir: &Path) {
    let lock_path = pack_dir.join("pack.lock.cbor");
    let mut components = BTreeMap::new();
    components.insert(
        "qa.process".to_string(),
        LockedComponent {
            component_id: "qa.process".to_string(),
            r#ref: Some("file://../components/qa.process/component.wasm".to_string()),
            abi_version: "0.6.0".to_string(),
            resolved_digest: format!("sha256:{}", "a".repeat(64)),
            describe_hash: "b".repeat(64),
            operations: Vec::new(),
            world: None,
            component_version: None,
            role: None,
        },
    );
    components.insert(
        "mcp.exec".to_string(),
        LockedComponent {
            component_id: "mcp.exec".to_string(),
            r#ref: Some("file://../components/mcp.exec/component.wasm".to_string()),
            abi_version: "0.6.0".to_string(),
            resolved_digest: format!("sha256:{}", "c".repeat(64)),
            describe_hash: "d".repeat(64),
            operations: Vec::new(),
            world: None,
            component_version: None,
            role: None,
        },
    );
    components.insert(
        "templating.handlebars".to_string(),
        LockedComponent {
            component_id: "templating.handlebars".to_string(),
            r#ref: Some("file://../components/templating.handlebars/component.wasm".to_string()),
            abi_version: "0.6.0".to_string(),
            resolved_digest: format!("sha256:{}", "e".repeat(64)),
            describe_hash: "f".repeat(64),
            operations: Vec::new(),
            world: None,
            component_version: None,
            role: None,
        },
    );
    let lock = PackLockV1::new(components);
    write_pack_lock(&lock_path, &lock).expect("write pack.lock.cbor");
}

fn add_invalid_static_routes_extension(pack_dir: &Path) {
    let assets_dir = pack_dir.join("assets").join("webchat-gui");
    std::fs::create_dir_all(&assets_dir).expect("create assets dir");
    std::fs::write(assets_dir.join("index.html"), "<html/>").expect("write index");

    let pack_yaml_path = pack_dir.join("pack.yaml");
    let mut pack_yaml = std::fs::read_to_string(&pack_yaml_path).expect("read pack.yaml");
    pack_yaml.push_str(
        r#"
extensions:
  greentic.static-routes.v1:
    kind: greentic.static-routes.v1
    version: 1.0.0
    inline:
      version: 1
      routes:
        - id: webchat-gui
          public_path: /v1/web/webchat/{team}
          source_root: assets/webchat-gui
          scope:
            tenant: false
            team: true
"#,
    );
    std::fs::write(pack_yaml_path, pack_yaml).expect("write pack.yaml");
}

#[test]
fn build_weather_demo_dry_run() {
    let (_pack_temp, pack_dir) = copy_weather_demo();
    write_weather_summary(&pack_dir);
    write_weather_lock(&pack_dir);
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"));
    cmd.current_dir(workspace_root());
    cmd.args([
        "build",
        "--in",
        pack_dir.to_str().unwrap(),
        "--allow-pack-schema",
        "--dry-run",
        "--log",
        "warn",
    ]);
    cmd.assert().success();
}

#[test]
fn lint_weather_demo() {
    let (_pack_temp, pack_dir) = copy_weather_demo();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"));
    cmd.current_dir(workspace_root());
    cmd.args(["lint", "--in", pack_dir.to_str().unwrap(), "--log", "warn"]);
    cmd.assert().success();
}

#[test]
fn new_pack_scaffold() {
    let temp = tempfile::tempdir().expect("temp dir");
    let pack_dir = temp.path().join("demo-pack");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"));
    cmd.current_dir(workspace_root());
    cmd.args([
        "new",
        "demo-pack",
        "--dir",
        pack_dir.to_str().unwrap(),
        "--log",
        "warn",
    ]);
    cmd.assert().success();

    assert!(pack_dir.join("pack.yaml").exists(), "pack.yaml missing");
    assert!(
        pack_dir.join("flows").join("main.ygtc").exists(),
        "flow missing"
    );
    assert!(
        pack_dir.join("components").is_dir(),
        "components directory missing"
    );
    assert!(
        !pack_dir.join("components").join("stub.wasm").exists(),
        "stub component should not be scaffolded"
    );
}

#[test]
fn lint_rejects_invalid_static_routes_extension() {
    let temp = tempfile::tempdir().expect("temp dir");
    let src = workspace_root()
        .join("crates")
        .join("packc")
        .join("tests")
        .join("fixtures")
        .join("packs")
        .join("valid-minimal");
    let pack_dir = temp.path().join("valid-minimal");
    std::fs::create_dir_all(&pack_dir).expect("create pack dir");
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(&src).expect("relative path");
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = pack_dir.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).expect("create target dir");
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("create target parent");
            }
            std::fs::copy(entry.path(), &target).expect("copy fixture entry");
        }
    }
    add_invalid_static_routes_extension(&pack_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args(["lint", "--in", pack_dir.to_str().unwrap(), "--log", "warn"])
        .output()
        .expect("run lint");
    assert!(!output.status.success(), "lint should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scope.team=true"),
        "unexpected stderr: {stderr}"
    );
}
