use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;
use walkdir::WalkDir;

const MSG_PROVIDER_MISSING: &str = "MSG_NO_PROVIDER_DECL";
const WORKSPACE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn workspace_root() -> PathBuf {
    Path::new(WORKSPACE_ROOT).join("..").join("..")
}

fn fixture_dir(name: &str) -> PathBuf {
    workspace_root()
        .join("crates")
        .join("packc")
        .join("tests")
        .join("fixtures")
        .join("packs")
        .join(name)
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let src = fixture_dir(name);
    let temp = tempdir().expect("tempdir");
    let dest = temp.path().join(name);
    fs::create_dir_all(&dest).expect("create destination");
    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        let relative = entry.path().strip_prefix(&src).expect("relative path");
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).expect("create dir");
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
    (temp, dest)
}

fn add_schema(pack_dir: &Path, kind: &str, provider_id: &str) {
    let schema_dir = pack_dir.join("schemas").join(kind).join(provider_id);
    fs::create_dir_all(&schema_dir).expect("create schema dir");
    let schema_path = schema_dir.join("config.schema.json");
    fs::write(&schema_path, "{}").expect("write schema");
}

fn add_extension_and_run_validator(kind: &str, provider_id: &str, validator_ref: &str) {
    if std::env::var("GREENTIC_TEST_OCI").is_err() {
        eprintln!("GREENTIC_TEST_OCI not set; skipping extension validator smoke test");
        return;
    }

    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    add_schema(&pack_dir, kind, provider_id);

    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack").to_path_buf();
    Command::new(bin.clone())
        .args([
            "add-extension",
            "provider",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            provider_id,
            "--kind",
            kind,
            "--title",
            "Smoke Provider",
            "--description",
            "Auto-added for validator tests",
            "--route",
            "diagnostics",
            "--flow",
            "main",
        ])
        .assert()
        .success();

    let mut doctor_cmd = Command::new(bin.clone());
    let output = doctor_cmd
        .args([
            "doctor",
            "--in",
            pack_dir.to_str().unwrap(),
            "--validate",
            "--validator-pack",
            validator_ref,
            "--no-flow-doctor",
            "--no-component-doctor",
            "--json",
        ])
        .output()
        .expect("run doctor");

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains(MSG_PROVIDER_MISSING),
        "provider error present:\n{combined}"
    );
}

#[test]
fn messaging_validator_smoke() {
    add_extension_and_run_validator(
        "messaging",
        "messaging.dummy",
        "oci://ghcr.io/greenticai/validators/messaging:stable",
    );
}

#[test]
fn events_validator_smoke() {
    add_extension_and_run_validator(
        "events",
        "events.dummy",
        "oci://ghcr.io/greenticai/validators/events:stable",
    );
}

#[test]
fn add_extension_updates_pack_yaml_in_place() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");

    Command::new(bin)
        .args([
            "add-extension",
            "provider",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "messaging.dummy",
            "--kind",
            "messaging",
            "--title",
            "Smoke Provider",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack.yaml");
    assert!(
        updated.contains("messaging.dummy"),
        "provider entry missing from pack.yaml"
    );
}

#[test]
fn add_deployer_extension_updates_pack_yaml_and_sidecar() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    fs::create_dir_all(pack_dir.join("flows")).expect("create flows dir");
    fs::write(pack_dir.join("flows/generate.ygtc"), "id: generate\n").expect("write flow");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");

    Command::new(bin)
        .args([
            "add-extension",
            "deployer",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--contract-id",
            "greentic.deployer.v1",
            "--op",
            "generate",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack.yaml");
    assert!(
        updated.contains("greentic.deployer.v1"),
        "deployer extension missing from pack.yaml"
    );

    let sidecar =
        fs::read_to_string(pack_dir.join("extensions/deployer.json")).expect("read sidecar");
    assert!(sidecar.contains("\"deployer_extension\""));
    assert!(sidecar.contains("\"contract\": \"greentic.deployer.v1\""));
}

#[test]
fn add_dependency_updates_pack_extensions_json() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");
    let ref_path = pack_dir.join("pack.yaml");
    let reference = format!("file://{}", ref_path.display());

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &reference,
        ])
        .assert()
        .success();

    let edited =
        fs::read_to_string(pack_dir.join("pack.extensions.json")).expect("read pack.extensions");
    assert!(edited.contains("\"id\": \"greentic.deployer.v1\""));
    assert!(edited.contains("\"role\": \"deployer\""));
    assert!(edited.contains(&reference));
}

#[test]
fn extensions_lock_pins_dependency_refs() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");
    let ref_path = pack_dir.join("pack.yaml");
    let reference = format!("file://{}", ref_path.display());

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &reference,
        ])
        .assert()
        .success();

    Command::new(bin)
        .args(["extensions-lock", "--in", pack_dir.to_str().unwrap()])
        .assert()
        .success();

    let locked = fs::read_to_string(pack_dir.join("pack.extensions.lock.json"))
        .expect("read pack.extensions.lock.json");
    assert!(locked.contains("\"id\": \"greentic.deployer.v1\""));
    assert!(locked.contains("\"source_ref\""));
    assert!(locked.contains("\"digest\": \"sha256:"));
}

#[test]
fn lint_rejects_extensions_lock_drift() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");
    let first_ref = format!("file://{}", pack_dir.join("pack.yaml").display());
    let second_ref = format!("file://{}", pack_dir.join("README.md").display());

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &first_ref,
        ])
        .assert()
        .success();

    Command::new(bin)
        .args(["extensions-lock", "--in", pack_dir.to_str().unwrap()])
        .assert()
        .success();

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &second_ref,
        ])
        .assert()
        .success();

    Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .args(["lint", "--in", pack_dir.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("does not match pack.extensions.json ref"));
}

#[test]
fn doctor_json_reports_extensions_lock_drift() {
    let (_tmp, pack_dir) = copy_fixture("valid-minimal");
    let bin = assert_cmd::cargo::cargo_bin!("greentic-pack");
    let first_ref = format!("file://{}", pack_dir.join("pack.yaml").display());
    let second_ref = format!("file://{}", pack_dir.join("README.md").display());

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &first_ref,
        ])
        .assert()
        .success();

    Command::new(bin)
        .args(["extensions-lock", "--in", pack_dir.to_str().unwrap()])
        .assert()
        .success();

    Command::new(bin)
        .args([
            "add-extension",
            "dependency",
            "--pack-dir",
            pack_dir.to_str().unwrap(),
            "--id",
            "greentic.deployer.v1",
            "--role",
            "deployer",
            "--ref",
            &second_ref,
        ])
        .assert()
        .success();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .args([
            "doctor",
            "--in",
            pack_dir.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .output()
        .expect("run doctor");

    assert!(
        !output.status.success(),
        "doctor should fail for stale extension lock"
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid doctor json");
    let diagnostics = payload
        .get("validation")
        .and_then(|value| value.get("diagnostics"))
        .and_then(|value| value.as_array())
        .expect("validation diagnostics");
    assert!(diagnostics.iter().any(|diag| {
        diag.get("code")
            .and_then(|value| value.as_str())
            .map(|code| code == "PACK_EXTENSION_DEPENDENCY_LOCK_STALE")
            .unwrap_or(false)
    }));
}
