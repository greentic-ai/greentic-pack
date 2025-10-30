use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;

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
