use assert_cmd::prelude::*;
use greentic_types::{ComponentCapabilities, ComponentProfiles};
use packc::config::{ComponentConfig, FlowConfig, FlowKindLabel, PackConfig};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn update_syncs_components_and_flows() {
    let temp = tempfile::tempdir().expect("temp dir");
    let pack_dir = temp.path().join("demo-pack");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"));
    cmd.current_dir(workspace_root());
    cmd.args([
        "new",
        "demo-pack",
        "--dir",
        pack_dir.to_str().unwrap(),
        "--log",
        "warn",
    ]);
    cmd.assert().success();

    let components_dir = pack_dir.join("components");
    fs::write(components_dir.join("alpha.wasm"), [0x00, 0x61, 0x73, 0x6d]).unwrap();
    fs::write(components_dir.join("beta.wasm"), [0x00, 0x61, 0x73, 0x6d]).unwrap();

    let flows_dir = pack_dir.join("flows");
    let extra_flow = flows_dir.join("secondary.ygtc");
    fs::write(
        &extra_flow,
        r#"id: secondary
title: Secondary
description: Another flow
type: messaging
start: start

nodes:
  start:
    templating.handlebars:
      text: "hi"
    routing:
      - out: true
"#,
    )
    .unwrap();

    let pack_yaml = pack_dir.join("pack.yaml");
    let mut cfg: PackConfig =
        serde_yaml_bw::from_str(&fs::read_to_string(&pack_yaml).unwrap()).unwrap();
    cfg.components.push(ComponentConfig {
        id: "old.component".to_string(),
        version: "0.1.0".to_string(),
        world: "greentic:component/stub".to_string(),
        supports: vec![FlowKindLabel::Messaging],
        profiles: ComponentProfiles {
            default: Some("default".to_string()),
            supported: vec!["default".to_string()],
        },
        capabilities: ComponentCapabilities::default(),
        wasm: PathBuf::from("components/old.wasm"),
        operations: Vec::new(),
        config_schema: None,
        resources: None,
        configurators: None,
    });
    cfg.flows.push(FlowConfig {
        id: "old_flow".to_string(),
        file: PathBuf::from("flows/old.ygtc"),
        tags: vec!["legacy".to_string()],
        entrypoints: vec!["legacy".to_string()],
        subscribes_to: Vec::new(),
    });
    fs::write(&pack_yaml, serde_yaml_bw::to_string(&cfg).unwrap()).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"));
    cmd.current_dir(workspace_root());
    cmd.args([
        "update",
        "--in",
        pack_dir.to_str().unwrap(),
        "--log",
        "warn",
    ]);
    cmd.assert().success();

    let cfg: PackConfig =
        serde_yaml_bw::from_str(&fs::read_to_string(&pack_yaml).unwrap()).unwrap();

    let component_ids: Vec<_> = cfg.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(component_ids, vec!["alpha", "beta"]);
    let component_paths: Vec<_> = cfg
        .components
        .iter()
        .map(|c| c.wasm.to_string_lossy().to_string())
        .collect();
    assert_eq!(
        component_paths,
        vec!["components/alpha.wasm", "components/beta.wasm"]
    );

    let flow_ids: Vec<_> = cfg.flows.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(flow_ids, vec!["main", "secondary"]);
    let flow_paths: Vec<_> = cfg
        .flows
        .iter()
        .map(|f| f.file.to_string_lossy().to_string())
        .collect();
    assert_eq!(flow_paths, vec!["flows/main.ygtc", "flows/secondary.ygtc"]);

    let secondary = cfg
        .flows
        .iter()
        .find(|f| f.id == "secondary")
        .expect("secondary flow");
    assert_eq!(secondary.tags, vec!["default".to_string()]);
    assert!(
        secondary.entrypoints.contains(&"default".to_string()),
        "secondary flow should have an entrypoint"
    );
}
