//! Integration: building an application pack with agents auto-derives the
//! credential setup.yaml + secret-requirements.json into the .gtpack.

use std::io::{Read, Write};
use std::path::Path;

fn write_file(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Build a minimal `.gtxpack` containing only a `describe.json` with the
/// Tavily tool's secret requirements.
fn make_tavily_gtxpack(dir: &Path) -> std::path::PathBuf {
    let describe = r#"{"contributions":{"tools":[
      {"name":"tavily_search","secret_requirements":[
        {"key":"tavily/api_key","required":true,"description":"Tavily web-search API key.","format":"text"}]}
    ]}}"#;
    let path = dir.join("greentic.tavily.gtxpack");
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        zip.start_file("describe.json", zip::write::FileOptions::<()>::default())
            .unwrap();
        zip.write_all(describe.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    std::fs::write(&path, buf).unwrap();
    path
}

/// Read one named entry from a `.gtpack` ZIP, returning `None` if absent.
fn read_zip_entry(gtpack: &Path, name: &str) -> Option<String> {
    let bytes = std::fs::read(gtpack).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut entry = archive.by_name(name).ok()?;
    let mut content = String::new();
    entry.read_to_string(&mut content).ok()?;
    Some(content)
}

#[test]
fn build_derives_setup_yaml_for_agent_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("pack");
    let gtx = make_tavily_gtxpack(tmp.path());

    write_file(
        &pack.join("pack.yaml"),
        r#"pack_id: demo
version: 0.1.0
kind: application
publisher: Test
components: []
dependencies: []
flows: []
agents:
  a:
    agent_id: a
    llm:
      provider: deepseek
      model: deepseek-chat
      credential_ref: deepseek
    tools:
      - extension_id: greentic.tavily
        tool_name: tavily_search
"#,
    );

    // Build a valid pack.extensions.json (version + role + source.kind + "ref" key).
    let ext_file_ref = format!("file://{}", gtx.display());
    let extensions_json = serde_json::json!({
        "version": 1,
        "extensions": [{
            "id": "greentic.tavily",
            "role": "tool",
            "source": {
                "kind": "file",
                "ref": ext_file_ref
            }
        }]
    });
    write_file(
        &pack.join("pack.extensions.json"),
        &extensions_json.to_string(),
    );

    let gtpack_out = tmp.path().join("demo.gtpack");

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(&pack)
        .args([
            "build",
            "--in",
            pack.to_str().unwrap(),
            "--no-update",
            "--gtpack-out",
            gtpack_out.to_str().unwrap(),
        ])
        .output()
        .expect("run packc build");

    assert!(
        output.status.success(),
        "packc build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let setup = read_zip_entry(&gtpack_out, "assets/setup.yaml")
        .expect("assets/setup.yaml must be present in the produced .gtpack");
    let spec: serde_json::Value =
        serde_yaml_bw::from_str(&setup).expect("assets/setup.yaml must be valid YAML");
    let question_names: Vec<&str> = spec["questions"]
        .as_array()
        .expect("setup.yaml must have a questions array")
        .iter()
        .map(|question| question["name"].as_str().unwrap())
        .collect();
    assert!(
        question_names.contains(&"deepseek"),
        "LLM credential question must be present (got: {question_names:?})"
    );
    assert!(
        question_names.contains(&"api_key"),
        "tool api_key question must be present (got: {question_names:?})"
    );

    let requirements_json = read_zip_entry(&gtpack_out, "assets/secret-requirements.json")
        .expect("assets/secret-requirements.json must be present in the produced .gtpack");
    assert!(
        requirements_json.contains("llm/deepseek"),
        "LLM secret key must appear in secret-requirements.json"
    );
    assert!(
        requirements_json.contains("tavily/api_key"),
        "tool secret key must appear in secret-requirements.json"
    );
}

/// When the pack source ships a hand-authored `assets/secret-requirements.json`,
/// the generator must NOT overwrite it — the hand-authored file wins.
#[test]
fn hand_authored_secret_requirements_wins_over_generated() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("pack");
    let gtx = make_tavily_gtxpack(tmp.path());

    write_file(
        &pack.join("pack.yaml"),
        r#"pack_id: demo
version: 0.1.0
kind: application
publisher: Test
components: []
dependencies: []
flows: []
agents:
  a:
    agent_id: a
    llm:
      provider: deepseek
      model: deepseek-chat
      credential_ref: deepseek
    tools:
      - extension_id: greentic.tavily
        tool_name: tavily_search
"#,
    );

    // Build a valid pack.extensions.json.
    let ext_file_ref = format!("file://{}", gtx.display());
    let extensions_json = serde_json::json!({
        "version": 1,
        "extensions": [{
            "id": "greentic.tavily",
            "role": "tool",
            "source": {
                "kind": "file",
                "ref": ext_file_ref
            }
        }]
    });
    write_file(
        &pack.join("pack.extensions.json"),
        &extensions_json.to_string(),
    );

    // Hand-authored secret-requirements.json with a distinctive marker.
    let hand_authored_content = r#"[{"key":"manual/override_marker","required":true,"description":"Hand-authored override"}]"#;
    write_file(
        &pack.join("assets/secret-requirements.json"),
        hand_authored_content,
    );

    let gtpack_out = tmp.path().join("demo.gtpack");

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(&pack)
        .args([
            "build",
            "--in",
            pack.to_str().unwrap(),
            "--no-update",
            "--gtpack-out",
            gtpack_out.to_str().unwrap(),
        ])
        .output()
        .expect("run packc build");

    assert!(
        output.status.success(),
        "packc build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let requirements_json = read_zip_entry(&gtpack_out, "assets/secret-requirements.json")
        .expect("assets/secret-requirements.json must be present in the produced .gtpack");

    assert!(
        requirements_json.contains("manual/override_marker"),
        "hand-authored marker must survive in the .gtpack (got: {requirements_json})"
    );
    assert!(
        !requirements_json.contains("llm/"),
        "generated llm/ key must NOT appear when hand-authored file wins (got: {requirements_json})"
    );
}
