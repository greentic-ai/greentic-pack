use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn dry_run_weather_demo_succeeds() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    cmd.current_dir(workspace_root());
    cmd.args([
        "build",
        "--in",
        "examples/weather-demo",
        "--dry-run",
        "--log",
        "warn",
    ]);
    cmd.assert().success();
}

#[test]
fn dry_run_rejects_missing_manifest() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    cmd.current_dir(workspace_root());
    cmd.args(["build", "--in", "examples", "--dry-run"]);
    cmd.assert().failure();
}

#[test]
fn scaffold_minimal_pack_builds() {
    let temp = tempdir().expect("temp dir");
    let pack_dir = temp.path().join("demo-pack");

    let mut scaffold = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    scaffold.current_dir(workspace_root());
    scaffold.args([
        "new",
        "demo-pack",
        "--dir",
        pack_dir.to_str().unwrap(),
        "--log",
        "warn",
    ]);
    scaffold.assert().success();

    assert!(pack_dir.join("pack.yaml").exists(), "pack.yaml missing");
    assert!(
        pack_dir.join("flows").join("welcome.ygtc").exists(),
        "flow file missing"
    );

    let mut build = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    build.current_dir(workspace_root());
    build.args([
        "build",
        "--in",
        pack_dir.to_str().unwrap(),
        "--dry-run",
        "--log",
        "warn",
    ]);
    build.assert().success();
}

#[test]
fn scaffold_with_sign_generates_keys() {
    let temp = tempdir().expect("temp dir");
    let pack_dir = temp.path().join("signed-pack");

    let mut scaffold = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    scaffold.current_dir(workspace_root());
    scaffold.args([
        "new",
        "signed-pack",
        "--dir",
        pack_dir.to_str().unwrap(),
        "--sign",
        "--log",
        "warn",
    ]);
    scaffold.assert().success();

    let private_key =
        fs::read_to_string(pack_dir.join("keys/dev_ed25519.sk")).expect("private key present");
    assert!(
        private_key.contains("PRIVATE KEY"),
        "private key should be PEM"
    );

    let public_key =
        fs::read_to_string(pack_dir.join("keys/dev_ed25519.pk")).expect("public key present");
    assert!(
        public_key.contains("PUBLIC KEY"),
        "public key should be PEM"
    );
}

#[test]
fn build_outputs_gtpack_archive() {
    let temp = tempdir().expect("temp dir");
    let base = temp.path();
    let wasm_target_installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.lines().any(|line| line.trim() == "wasm32-waspi2"))
        .unwrap_or(false);
    if !wasm_target_installed {
        eprintln!("skipping gtpack archive test; wasm32-waspi2 target missing");
        return;
    }
    let wasm = base.join("pack.wasm");
    let manifest = base.join("manifest.cbor");
    let sbom = base.join("sbom.cdx.json");
    let gtpack = base.join("pack.gtpack");

    let mut build = Command::new(assert_cmd::cargo::cargo_bin!("packc"));
    build.current_dir(workspace_root());
    build.args([
        "build",
        "--in",
        "examples/weather-demo",
        "--out",
        wasm.to_str().unwrap(),
        "--manifest",
        manifest.to_str().unwrap(),
        "--sbom",
        sbom.to_str().unwrap(),
        "--gtpack-out",
        gtpack.to_str().unwrap(),
        "--log",
        "warn",
    ]);
    build.assert().success();

    let mut inspect = Command::new("cargo");
    inspect.current_dir(workspace_root());
    inspect.args([
        "run",
        "-p",
        "greentic-pack",
        "--bin",
        "gtpack-inspect",
        "--",
        "--policy",
        "devok",
        "--json",
        gtpack.to_str().unwrap(),
    ]);
    let output = inspect
        .output()
        .expect("gtpack-inspect should run successfully");
    assert!(output.status.success(), "gtpack-inspect failed");
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("gtpack-inspect produced valid JSON");
    let sbom_entries = report
        .get("sbom")
        .and_then(Value::as_array)
        .expect("sbom array present");
    assert!(
        sbom_entries.iter().all(|entry| {
            entry
                .get("media_type")
                .and_then(Value::as_str)
                .map(|val| !val.is_empty())
                .unwrap_or(false)
        }),
        "sbom entries must expose media_type"
    );
}
