use greentic_distributor_client::{DistClient, DistOptions};
use greentic_types::cbor::canonical;
use greentic_types::pack::extensions::component_sources::EXT_COMPONENT_SOURCES_V1;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};
use greentic_types::schemas::component::v0_6_0::{
    ComponentDescribe, ComponentInfo, ComponentOperation, ComponentRunInput, ComponentRunOutput,
    schema_hash,
};
use greentic_types::{PackManifest, decode_pack_manifest, encode_pack_manifest};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;
use zip::CompressionMethod;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn write_pack(dir: &Path) {
    let flows_dir = dir.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir");
    let flow_path = flows_dir.join("main.ygtc");
    fs::write(
        &flow_path,
        r#"id: main
type: messaging
start: templates
nodes:
  templates:
    component.exec:
      component: ai.greentic.component-templates
      operation: handle_message
      input:
        text: "hi"
    routing:
      - out: true
"#,
    )
    .expect("flow file");

    let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let summary = json!({
        "schema_version": 1,
        "flow": "main.ygtc",
        "nodes": {
            "templates": {
                "component_id": "ai.greentic.component-templates",
                "source": {
                    "kind": "oci",
                    "ref": "oci://ghcr.io/greenticai/components/templates@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "digest": digest
            }
        }
    });
    fs::write(
        flow_path.with_extension("ygtc.resolve.summary.json"),
        serde_json::to_vec_pretty(&summary).expect("encode summary"),
    )
    .expect("summary file");

    let pack_yaml = r#"pack_id: dev.local.hello-pack
version: 0.1.0
kind: application
publisher: Greentic
components: []
flows:
  - id: main
    file: flows/main.ygtc
    tags: [default]
    entrypoints: [default]
"#;
    fs::write(dir.join("pack.yaml"), pack_yaml).expect("pack.yaml");
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
    outcomes: Vec::new(),
    };
    let bytes = canonical::to_canonical_cbor_allow_floats(&describe).expect("encode describe");
    let describe_path = format!("{}.describe.cbor", wasm_path.display());
    fs::write(describe_path, bytes).expect("write describe cache");
}

fn cache_component(cache_dir: &Path, digest: &str) {
    let wasm_bytes = b"cached-component";
    let seed_path = cache_dir.join("seed-component.wasm");
    fs::create_dir_all(cache_dir).expect("cache root");
    fs::write(&seed_path, wasm_bytes).expect("write seed wasm");

    let dist = DistClient::new(DistOptions {
        cache_dir: cache_dir.to_path_buf(),
        allow_tags: true,
        offline: false,
        allow_insecure_local_http: false,
        ..DistOptions::default()
    });
    let source = dist
        .parse_source(&format!("file://{}", seed_path.display()))
        .expect("parse source");
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let descriptor = rt
        .block_on(dist.resolve(source, greentic_distributor_client::ResolvePolicy))
        .expect("resolve source");
    let cached = rt
        .block_on(dist.fetch(&descriptor, greentic_distributor_client::CachePolicy))
        .expect("seed cache");

    let actual_digest = cached.descriptor.digest.clone();
    let actual_wasm_path = cached.cache_path.expect("cache path");
    write_describe_sidecar(
        &actual_wasm_path,
        "ai.greentic.component-templates",
        "0.1.0",
    );

    if digest != actual_digest {
        let alias_dir = cache_dir.join(digest.trim_start_matches("sha256:"));
        fs::create_dir_all(&alias_dir).expect("alias cache dir");
        let alias_wasm_path = alias_dir.join("component.wasm");
        fs::write(&alias_wasm_path, wasm_bytes).expect("write alias wasm");
        write_describe_sidecar(&alias_wasm_path, "ai.greentic.component-templates", "0.1.0");

        let actual_entry_path = component_cache_entry_path(cache_dir, &actual_digest);
        let actual_entry_bytes = fs::read(&actual_entry_path).expect("read actual cache entry");
        let mut alias_entry: Value =
            serde_json::from_slice(&actual_entry_bytes).expect("parse actual cache entry");
        alias_entry["cache_key"] = Value::String(digest.to_string());
        alias_entry["digest"] = Value::String(digest.to_string());
        alias_entry["local_path"] = Value::String(alias_wasm_path.display().to_string());
        alias_entry["raw_ref"] = Value::String(format!("file://{}", alias_wasm_path.display()));
        alias_entry["canonical_ref"] =
            Value::String(format!("file://{}@{}", alias_wasm_path.display(), digest));
        alias_entry["source_snapshot"]["raw_ref"] =
            Value::String(format!("file://{}", alias_wasm_path.display()));
        alias_entry["source_snapshot"]["canonical_ref"] =
            Value::String(format!("file://{}@{}", alias_wasm_path.display(), digest));

        let alias_entry_path = component_cache_entry_path(cache_dir, digest);
        if let Some(parent) = alias_entry_path.parent() {
            fs::create_dir_all(parent).expect("alias entry parent");
        }
        fs::write(
            alias_entry_path,
            serde_json::to_vec_pretty(&alias_entry).expect("alias entry json"),
        )
        .expect("write alias cache entry");
    }
}

fn component_cache_entry_path(cache_dir: &Path, digest: &str) -> PathBuf {
    let normalized = digest.trim_start_matches("sha256:");
    let prefix_len = normalized.len().min(2);
    let (prefix, rest) = normalized.split_at(prefix_len);
    cache_dir
        .join("artifacts")
        .join("sha256")
        .join(prefix)
        .join(rest)
        .join("entry.json")
}

fn rewrite_manifest_without_component_sources(src: &Path, dest: &Path) {
    let src_file = File::open(src).expect("open source gtpack");
    let mut archive = ZipArchive::new(src_file).expect("open zip");
    let dest_file = File::create(dest).expect("create dest gtpack");
    let mut writer = ZipWriter::new(dest_file);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("zip entry");
        let name = entry.name().to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read zip entry");
        if name == "manifest.cbor" {
            let mut manifest: PackManifest = decode_pack_manifest(&buf).expect("decode manifest");
            if let Some(ext) = manifest.extensions.as_mut() {
                ext.remove(EXT_COMPONENT_SOURCES_V1);
            }
            buf = encode_pack_manifest(&manifest).expect("encode manifest");
        }

        let mut options = FileOptions::<()>::default();
        options = options.compression_method(match entry.compression() {
            CompressionMethod::Stored => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        });
        writer.start_file(name, options).expect("start file");
        writer.write_all(&buf).expect("write file");
    }

    writer.finish().expect("finish zip");
}

#[test]
fn doctor_accepts_remote_component_sources() {
    let temp = tempdir().expect("temp dir");
    write_pack(temp.path());
    let gtpack_out = temp.path().join("dist/pack.gtpack");
    let cache_dir = temp.path().join("cache");
    cache_component(
        &cache_dir,
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .env("GREENTIC_PACK_USE_DESCRIBE_CACHE", "1")
        .args([
            "build",
            "--in",
            temp.path().to_str().unwrap(),
            "--allow-pack-schema",
            "--gtpack-out",
            gtpack_out.to_str().unwrap(),
            "--cache-dir",
            cache_dir.to_str().unwrap(),
            "--offline",
            "--bundle",
            "none",
            "--no-update",
            "--log",
            "warn",
        ])
        .output()
        .expect("run packc build");
    assert!(
        output.status.success(),
        "packc build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let doctor = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "doctor",
            "--pack",
            gtpack_out.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .output()
        .expect("run doctor");
    assert!(
        doctor.status.success(),
        "doctor failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
}

#[test]
fn doctor_rejects_missing_component_sources() {
    let temp = tempdir().expect("temp dir");
    write_pack(temp.path());
    let gtpack_out = temp.path().join("dist/pack.gtpack");
    let cache_dir = temp.path().join("cache");
    cache_component(
        &cache_dir,
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .env("GREENTIC_PACK_USE_DESCRIBE_CACHE", "1")
        .args([
            "build",
            "--in",
            temp.path().to_str().unwrap(),
            "--allow-pack-schema",
            "--gtpack-out",
            gtpack_out.to_str().unwrap(),
            "--cache-dir",
            cache_dir.to_str().unwrap(),
            "--offline",
            "--bundle",
            "none",
            "--no-update",
            "--log",
            "warn",
        ])
        .output()
        .expect("run packc build");
    assert!(
        output.status.success(),
        "packc build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let broken = temp.path().join("dist/broken.gtpack");
    rewrite_manifest_without_component_sources(&gtpack_out, &broken);

    let doctor = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "doctor",
            "--pack",
            broken.to_str().unwrap(),
            "--json",
            "--no-flow-doctor",
            "--no-component-doctor",
        ])
        .output()
        .expect("run doctor");
    assert!(
        !doctor.status.success(),
        "expected doctor to fail for missing component sources"
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&doctor.stdout).expect("valid json output");
    let diagnostics = payload
        .get("validation")
        .and_then(|val| val.get("diagnostics"))
        .and_then(|val| val.as_array())
        .expect("validation diagnostics present");
    assert!(
        diagnostics.iter().any(|diag| {
            diag.get("code")
                .and_then(|val| val.as_str())
                .map(|code| code == "PACK_MISSING_COMPONENT_REFERENCE")
                .unwrap_or(false)
        }),
        "expected missing component reference diagnostic"
    );
}
