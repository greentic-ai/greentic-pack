use greentic_interfaces_guest::component_v0_6::node;

struct Impl;

impl node::Guest for Impl {
    fn describe() -> node::ComponentDescriptor {
        node::ComponentDescriptor {
            name: "demo".into(),
            version: "0.1.0".into(),
            summary: None,
            capabilities: vec![],
            ops: vec![],
            schemas: vec![],
            setup: None,
        }
    }

    fn invoke(
        _op: String,
        _envelope: node::InvocationEnvelope,
    ) -> Result<node::InvocationResult, node::NodeError> {
        Ok(node::InvocationResult {
            ok: true,
            output_cbor: vec![],
            output_metadata_cbor: None,
        })
    }
}

greentic_interfaces_guest::export_component_v060!(Impl);

fn main() {}
