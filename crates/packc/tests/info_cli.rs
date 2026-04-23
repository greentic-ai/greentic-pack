#![forbid(unsafe_code)]
//! CLI integration tests for `greentic-pack info`.
//!
//! The fixture builder mirrors the pattern used in `cli_smoke.rs`: copy the
//! `billing-demo` example into a temp directory, drop describe-sidecar caches
//! (so the build is offline-deterministic), and invoke `greentic-pack build`
//! to produce a real `.gtpack` archive. The resulting pack is unsigned, which
//! is exactly what we need to exercise the signature-status branches and the
//! `--strict` exit-code path.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};
use greentic_types::schemas::component::v0_6_0::{
    ComponentDescribe, ComponentInfo, ComponentOperation, ComponentRunInput, ComponentRunOutput,
    schema_hash,
};
use serde_yaml_bw::Value as YamlValue;
use tempfile::{TempDir, tempdir};
use walkdir::WalkDir;

// --- Fixture helpers (copied from cli_smoke.rs — tests don't DRY across files) ---

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let src_path = entry.path();
        let rel = src_path.strip_prefix(src).expect("relative path");
        let dest_path = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src_path, &dest_path)?;
        }
    }
    Ok(())
}

fn example_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn write_describe_sidecar(wasm_path: &Path, component_id: &str, version: &str) {
    let input_schema = SchemaIr::String {
        min_len: None,
        max_len: None,
        regex: None,
        format: None,
    };
    let output_schema = SchemaIr::String {
        min_len: None,
        max_len: None,
        regex: None,
        format: None,
    };
    let config_schema = SchemaIr::Object {
        properties: BTreeMap::new(),
        required: Vec::new(),
        additional: AdditionalProperties::Forbid,
    };
    let hash = schema_hash(&input_schema, &output_schema, &config_schema).expect("schema hash");
    let operation = ComponentOperation {
        id: "run".to_string(),
        display_name: None,
        input: ComponentRunInput {
            schema: input_schema,
        },
        output: ComponentRunOutput {
            schema: output_schema,
        },
        defaults: BTreeMap::new(),
        redactions: Vec::new(),
        constraints: BTreeMap::new(),
        schema_hash: hash,
    };
    let describe = ComponentDescribe {
        info: ComponentInfo {
            id: component_id.to_string(),
            version: version.to_string(),
            role: "tool".to_string(),
            display_name: None,
        },
        provided_capabilities: Vec::new(),
        required_capabilities: Vec::new(),
        metadata: BTreeMap::new(),
        operations: vec![operation],
        config_schema,
    };
    let bytes = canonical::to_canonical_cbor_allow_floats(&describe).expect("encode describe");
    let describe_path = format!("{}.describe.cbor", wasm_path.display());
    fs::write(describe_path, bytes).expect("write describe cache");
}

fn write_describe_sidecars_from_pack(pack_dir: &Path) {
    let pack_yaml = fs::read_to_string(pack_dir.join("pack.yaml")).expect("read pack.yaml");
    let doc: YamlValue = serde_yaml_bw::from_str(&pack_yaml).expect("parse pack.yaml");
    let components = doc
        .get("components")
        .and_then(|val| val.as_sequence())
        .cloned()
        .unwrap_or_default();
    for comp in components {
        let id = comp
            .get("id")
            .and_then(|val| val.as_str())
            .unwrap_or_default();
        let version = comp
            .get("version")
            .and_then(|val| val.as_str())
            .unwrap_or("0.1.0");
        let wasm = comp
            .get("wasm")
            .and_then(|val| val.as_str())
            .unwrap_or_default();
        if id.is_empty() || wasm.is_empty() {
            continue;
        }
        let path = if Path::new(wasm).is_absolute() {
            PathBuf::from(wasm)
        } else {
            pack_dir.join(wasm)
        };
        write_describe_sidecar(&path, id, version);
    }
}

/// Build an unsigned `.gtpack` from the `billing-demo` example. Returns the
/// absolute path to the produced archive. The temp directory must outlive the
/// returned path (keep `tmp` alive in the test body).
fn build_unsigned_pack(tmp: &TempDir) -> PathBuf {
    let pack_dir = tmp.path().join("pack");
    copy_dir(&example_path("billing-demo"), &pack_dir).expect("copy example");
    write_describe_sidecars_from_pack(&pack_dir);

    let manifest_out = pack_dir.join("dist/manifest.cbor");
    let gtpack_out = pack_dir.join("dist/pack.gtpack");

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(&pack_dir)
        .env("GREENTIC_DIST_OFFLINE", "1")
        .env("GREENTIC_PACK_USE_DESCRIBE_CACHE", "1")
        .args([
            "build",
            "--in",
            pack_dir.to_str().expect("pack dir"),
            "--allow-pack-schema",
            "--manifest",
            manifest_out.to_str().expect("manifest path"),
            "--gtpack-out",
            gtpack_out.to_str().expect("gtpack path"),
        ])
        .output()
        .expect("run greentic-pack build");

    assert!(
        output.status.success(),
        "greentic-pack build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gtpack_out.exists(), "gtpack should be written");
    gtpack_out
}

fn greentic_pack() -> std::process::Command {
    std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
}

// --- Tests ---

#[test]
fn human_output_on_unsigned_pack() {
    let tmp = tempdir().expect("temp dir");
    let pack = build_unsigned_pack(&tmp);
    let out = greentic_pack()
        .args(["info", pack.to_str().expect("pack path")])
        .output()
        .expect("run info");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("unsigned"),
        "stdout did not mention unsigned: {s}"
    );
    // The fixture pack ships at least one component and one flow, so the
    // corresponding sections must be rendered.
    assert!(
        s.contains("Components") || s.contains("Entry flows"),
        "stdout missing components/entry-flows section: {s}"
    );
}

#[test]
fn json_output_roundtrip() {
    let tmp = tempdir().expect("temp dir");
    let pack = build_unsigned_pack(&tmp);
    let out = greentic_pack()
        .args([
            "info",
            pack.to_str().expect("pack path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run info --format json");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout is not valid JSON");
    assert_eq!(v["info_schema_version"], 1);
    assert_eq!(v["signature"]["status"], "unsigned");
    // Required top-level fields from `InfoReport` must appear in the JSON.
    assert!(v.get("name").is_some());
    assert!(v.get("version").is_some());
    assert!(v.get("components").is_some());
}

#[test]
fn strict_on_unsigned_exits_3() {
    let tmp = tempdir().expect("temp dir");
    let pack = build_unsigned_pack(&tmp);
    let out = greentic_pack()
        .args(["info", pack.to_str().expect("pack path"), "--strict"])
        .output()
        .expect("run info --strict");
    assert_eq!(
        out.status.code(),
        Some(3),
        "expected exit 3 on strict unsigned; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_file_exits_2() {
    let out = greentic_pack()
        .args(["info", "/nope/no.gtpack"])
        .output()
        .expect("run info on missing path");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 on missing file; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn wrong_extension_exits_2() {
    // Use the current crate's `Cargo.toml` — guaranteed to exist as a regular
    // file so we exercise the extension-check branch rather than the
    // existence-check branch.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let out = greentic_pack()
        .args(["info", manifest.to_str().expect("manifest path")])
        .output()
        .expect("run info on Cargo.toml");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 on wrong extension; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
