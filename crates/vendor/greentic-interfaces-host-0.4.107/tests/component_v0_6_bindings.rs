use greentic_interfaces_host::component_v0_6;

#[test]
fn component_v0_6_bindings_exist() {
    fn assert_describe<T>(
        component: &component_v0_6::ComponentV0V6V0,
        store: &mut wasmtime::Store<T>,
    ) {
        let _ = component.greentic_component_node().call_describe(store);
    }

    let _ = assert_describe::<()>;
}
