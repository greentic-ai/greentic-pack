use std::fs;
use std::io::Read as _;
use tempfile::tempdir;
use zip::ZipArchive;

/// A minimal dw-application pack (agent-only, no flows/components) must build.
#[test]
fn builds_dw_application_kind() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("pack.yaml"),
        r#"pack_id: greeter-agent
version: 0.1.0
kind: dw-application
publisher: greentic
agents:
  greeter:
    agent_id: greeter
    system_prompt: "You are a helpful greeter."
    tools: []
    llm:
      provider: openai
      model: gpt-4o-mini
"#,
    )
    .unwrap();

    let manifest_out = root.join("dist/manifest.cbor");
    let gtpack_out = root.join("dist/pack.gtpack");

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(root)
        .env("GREENTIC_DIST_OFFLINE", "1")
        .args([
            "build",
            "--in",
            root.to_str().expect("root path"),
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
    assert!(manifest_out.exists(), "manifest must be written");
    assert!(gtpack_out.exists(), "gtpack must be written");
}

/// The built gtpack must contain a `dw-agents.json` sidecar whose content is a
/// bare JSON object keyed by agent_id (as required by the runtime's
/// `BTreeMap<String, AgentConfig>` deserialiser).
#[test]
fn dw_application_gtpack_contains_agents_sidecar() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("pack.yaml"),
        r#"pack_id: greeter-agent
version: 0.1.0
kind: dw-application
publisher: greentic
agents:
  greeter:
    agent_id: greeter
    system_prompt: "You are a helpful greeter."
    tools: []
    llm:
      provider: openai
      model: gpt-4o-mini
"#,
    )
    .unwrap();

    let manifest_out = root.join("dist/manifest.cbor");
    let gtpack_out = root.join("dist/pack.gtpack");

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(root)
        .env("GREENTIC_DIST_OFFLINE", "1")
        .args([
            "build",
            "--in",
            root.to_str().expect("root path"),
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
    assert!(gtpack_out.exists(), "gtpack must be written");

    // Open the zip and find dw-agents.json
    let mut archive = ZipArchive::new(fs::File::open(&gtpack_out).expect("open gtpack"))
        .expect("parse gtpack as zip");

    let mut entry = archive
        .by_name("dw-agents.json")
        .expect("dw-agents.json must be present in the dw-application gtpack");

    let mut sidecar_bytes = Vec::new();
    entry
        .read_to_end(&mut sidecar_bytes)
        .expect("read dw-agents.json bytes");

    let parsed: serde_json::Value =
        serde_json::from_slice(&sidecar_bytes).expect("dw-agents.json must be valid JSON");

    assert!(
        parsed.is_object(),
        "dw-agents.json must be a bare JSON object"
    );
    assert!(
        parsed.get("greeter").is_some(),
        "dw-agents.json must contain the 'greeter' agent key"
    );
}
