use std::fs;
use tempfile::tempdir;

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
