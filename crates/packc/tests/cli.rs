use assert_cmd::prelude::*;
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
