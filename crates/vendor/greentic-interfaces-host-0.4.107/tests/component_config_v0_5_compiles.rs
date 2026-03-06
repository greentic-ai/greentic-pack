#[test]
fn component_config_type_is_available() {
    use greentic_interfaces_host::component::v0_5::exports::greentic::component::node::{
        Config, DisplayMode,
    };

    let cfg = Config {
        title: "example".into(),
        description: None,
        layout: DisplayMode::Stacked,
        max_items: 10,
        tags: Some(vec!["tag-a".into(), "tag-b".into()]),
        connection_id: None,
    };

    assert_eq!(cfg.max_items, 10);
}
