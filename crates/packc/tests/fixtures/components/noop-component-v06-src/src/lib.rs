#![deny(unsafe_code)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use greentic_interfaces_guest::component_v0_6::{component_i18n, component_qa, node};

#[cfg(target_arch = "wasm32")]
struct NoopComponent;

#[cfg(target_arch = "wasm32")]
impl node::Guest for NoopComponent {
    fn describe() -> node::ComponentDescriptor {
        node::ComponentDescriptor {
            name: "noop-component-v06".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            summary: None,
            capabilities: vec![],
            ops: vec![],
            schemas: vec![],
            setup: None,
        }
    }

    fn invoke(
        _operation: String,
        _envelope: node::InvocationEnvelope,
    ) -> Result<node::InvocationResult, node::NodeError> {
        Ok(node::InvocationResult {
            ok: true,
            output_cbor: vec![],
            output_metadata_cbor: None,
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl component_qa::Guest for NoopComponent {
    fn qa_spec(_mode: component_qa::QaMode) -> Vec<u8> {
        Vec::new()
    }

    fn apply_answers(
        _mode: component_qa::QaMode,
        current_config: Vec<u8>,
        _answers: Vec<u8>,
    ) -> Vec<u8> {
        current_config
    }
}

#[cfg(target_arch = "wasm32")]
impl component_i18n::Guest for NoopComponent {
    fn i18n_keys() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(target_arch = "wasm32")]
greentic_interfaces_guest::export_component_v060!(
    NoopComponent,
    component_qa: NoopComponent,
    component_i18n: NoopComponent,
);
