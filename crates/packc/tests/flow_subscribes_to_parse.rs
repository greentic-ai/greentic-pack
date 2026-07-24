//! `pack.yaml` `flows[].subscribes_to` deserializes onto `FlowConfig`.

use packc::config::load_pack_config;
use std::fs;
use tempfile::tempdir;

#[test]
fn flow_subscribes_to_is_parsed() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("pack.yaml"),
        r#"
pack_id: acme.orders
name: Orders
version: 0.1.0
kind: application
publisher: acme
flows:
  - id: main
    file: flows/main.ygtc
    subscribes_to:
      - orders.created
      - orders.shipped
"#,
    )
    .unwrap();

    let cfg = load_pack_config(dir.path()).expect("pack.yaml parses");
    let flow = cfg
        .flows
        .iter()
        .find(|f| f.id == "main")
        .expect("main flow");
    assert_eq!(flow.subscribes_to, vec!["orders.created", "orders.shipped"]);
}

#[test]
fn flow_without_subscribes_to_defaults_empty() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("pack.yaml"),
        r#"
pack_id: acme.orders
name: Orders
version: 0.1.0
kind: application
publisher: acme
flows:
  - id: main
    file: flows/main.ygtc
"#,
    )
    .unwrap();

    let cfg = load_pack_config(dir.path()).expect("pack.yaml parses");
    let flow = cfg
        .flows
        .iter()
        .find(|f| f.id == "main")
        .expect("main flow");
    assert!(flow.subscribes_to.is_empty());
}
