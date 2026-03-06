#[test]
fn guest_component_config_shape_is_visible() {
    use greentic_interfaces_guest::component::node::{Config, DisplayMode};

    let cfg = Config {
        title: "demo".into(),
        description: Some("component description".into()),
        layout: DisplayMode::Grid,
        max_items: 5,
        tags: None,
        connection_id: None,
    };

    assert_eq!(cfg.layout, DisplayMode::Grid);
}
