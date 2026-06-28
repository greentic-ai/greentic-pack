use std::fs;
use std::io::Read as _;
use tempfile::tempdir;
use zip::ZipArchive;

/// A non-`dw-application` pack (plain `kind: application`) must NOT contain
/// the `dw-agents.json` or `secrets-policy.json` sidecars. This test would
/// fail if the emission gate in `build.rs` were accidentally removed.
#[test]
fn application_pack_has_no_dw_sidecars() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("pack.yaml"),
        r#"pack_id: minimal-app
version: 0.1.0
kind: application
publisher: greentic
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

    let mut archive = ZipArchive::new(fs::File::open(&gtpack_out).expect("open gtpack"))
        .expect("parse gtpack as zip");

    assert!(
        archive.by_name("dw-agents.json").is_err(),
        "application pack must not carry dw-agents.json"
    );
    assert!(
        archive.by_name("secrets-policy.json").is_err(),
        "application pack must not carry secrets-policy.json"
    );
}

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

/// A dw-application pack must ship a `secrets-policy.json` sidecar that mirrors
/// its effective `secret-requirements.json` (the LLM + per-tool secrets), each
/// entry `byo-required`. Regression: the policy was previously sourced from the
/// component-only aggregate, which is empty for an agent-only pack, so real
/// agent packs shipped with NO secrets-policy at all. A hand-authored
/// `assets/secret-requirements.json` pins the keys deterministically (offline,
/// no tool-extension resolution required).
#[test]
fn dw_application_gtpack_contains_secrets_policy() {
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
      provider: deepseek
      model: deepseek-chat
      credential_ref: deepseek
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(
        root.join("assets/secret-requirements.json"),
        r#"[{"key":"llm/deepseek"},{"key":"tavily/api_key","description":"Tavily API key"}]"#,
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

    let mut archive = ZipArchive::new(fs::File::open(&gtpack_out).expect("open gtpack"))
        .expect("parse gtpack as zip");

    let mut entry = archive
        .by_name("secrets-policy.json")
        .expect("secrets-policy.json must be present in the dw-application gtpack");
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .expect("read secrets-policy.json");
    drop(entry);

    let policy: serde_json::Value =
        serde_json::from_slice(&bytes).expect("secrets-policy.json must be valid JSON");
    let reqs = policy["requirements"]
        .as_array()
        .expect("requirements must be an array");

    let keys: Vec<&str> = reqs.iter().filter_map(|r| r["key"].as_str()).collect();
    assert!(
        keys.contains(&"llm/deepseek") && keys.contains(&"tavily/api_key"),
        "secrets-policy must mirror the effective secret-requirements keys, got {keys:?}"
    );
    for r in reqs {
        assert_eq!(
            r["policy"], "byo-required",
            "every secrets-policy entry must be byo-required"
        );
        assert_eq!(
            r["required"], true,
            "byo-required entries carry required:true"
        );
    }
}
