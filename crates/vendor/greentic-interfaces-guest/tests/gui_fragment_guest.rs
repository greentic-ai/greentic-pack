#![cfg(feature = "gui-fragment")]

#[test]
fn guest_bindings_expose_fragment_context() {
    use greentic_interfaces_guest::gui_fragment::FragmentContext;

    let ctx = FragmentContext {
        tenant_ctx: "tenant-json".to_string(),
        user_ctx: "user-json".to_string(),
        route: "/fragments".to_string(),
        session_id: "sess-guest".to_string(),
    };

    assert_eq!(ctx.user_ctx, "user-json");
    assert_eq!(ctx.route, "/fragments");
}
