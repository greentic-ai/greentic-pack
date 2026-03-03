#![cfg(feature = "gui-fragment")]

#[test]
fn host_reexports_fragment_context() {
    use greentic_interfaces_host::gui_fragment::FragmentContext;

    let ctx = FragmentContext {
        tenant_ctx: "tenant-json".to_string(),
        user_ctx: "user-json".to_string(),
        route: "/billing".to_string(),
        session_id: "sess-42".to_string(),
    };

    assert_eq!(ctx.tenant_ctx, "tenant-json");
    assert_eq!(ctx.route, "/billing");
}
