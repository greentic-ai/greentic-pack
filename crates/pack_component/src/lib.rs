#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
extern crate alloc;

mod data;

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};
use greentic_interfaces::pack_export::{
    A2AItem, FlowInfo, PackExport, PrepareResult, RunResult, SchemaDoc,
};
use serde::Deserialize;
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;

/// Return the embedded pack manifest as CBOR bytes.
pub fn manifest_cbor() -> &'static [u8] {
    data::MANIFEST_CBOR
}

/// Decode the embedded pack manifest into a `serde_json::Value`.
pub fn manifest_value() -> Value {
    serde_cbor::from_slice(data::MANIFEST_CBOR)
        .expect("generated manifest bytes should always be valid CBOR")
}

/// Decode the embedded manifest into a strongly-typed `T`.
pub fn manifest_as<T>() -> T
where
    T: for<'de> Deserialize<'de>,
{
    serde_cbor::from_slice(data::MANIFEST_CBOR)
        .expect("generated manifest matches the requested type")
}

/// Access the embedded flow sources as `(id, raw_ygtc)` tuples.
pub fn flows() -> &'static [(&'static str, &'static str)] {
    data::FLOWS
}

/// Access the embedded templates as `(logical_path, bytes)` tuples.
pub fn templates() -> &'static [(&'static str, &'static [u8])] {
    data::TEMPLATES
}

/// Lookup a template payload by logical path.
pub fn template_by_path(path: &str) -> Option<&'static [u8]> {
    data::TEMPLATES
        .iter()
        .find(|(logical, _)| *logical == path)
        .map(|(_, bytes)| *bytes)
}

/// Component instance implementing the `greentic:pack-export` interface.
#[derive(Debug, Default)]
pub struct Component;

impl PackExport for Component {
    fn list_flows(&self) -> Vec<FlowInfo> {
        flows()
            .iter()
            .map(|(id, _)| FlowInfo {
                id: (*id).to_string(),
                human_name: None,
                description: None,
            })
            .collect()
    }

    fn get_flow_schema(&self, flow_id: &str) -> Option<SchemaDoc> {
        flows()
            .iter()
            .find(|(id, _)| *id == flow_id)
            .map(|(id, _)| SchemaDoc {
                flow_id: (*id).to_string(),
                schema_json: serde_json::json!({}),
            })
    }

    fn prepare_flow(&self, flow_id: &str) -> PrepareResult {
        if flows().iter().any(|(id, _)| *id == flow_id) {
            PrepareResult {
                status: "ok".into(),
                error: None,
            }
        } else {
            PrepareResult {
                status: "error".into(),
                error: Some(format!("unknown flow: {flow_id}")),
            }
        }
    }

    fn run_flow(&self, flow_id: &str, _input: Value) -> RunResult {
        if flows().iter().any(|(id, _)| *id == flow_id) {
            RunResult {
                status: "error".into(),
                output: None,
                error: Some("not-implemented-in-M1".into()),
            }
        } else {
            RunResult {
                status: "error".into(),
                output: None,
                error: Some(format!("unknown flow: {flow_id}")),
            }
        }
    }

    fn a2a_search(&self, _query: &str) -> Vec<A2AItem> {
        Vec::new()
    }
}

/// Convenience helper for host environments that want an owned component.
pub fn component() -> Component {
    Component::default()
}

// Export simple C ABI shims for the stub interface so a Wasm harness can
// exercise the component without native bindings.
#[no_mangle]
pub extern "C" fn greentic_pack_export__list_flows(json_buffer: *mut u8, len: usize) -> usize {
    let component = Component::default();
    let flows = component.list_flows();
    write_json_response(&flows, json_buffer, len)
}

#[no_mangle]
pub extern "C" fn greentic_pack_export__prepare_flow(
    flow_id_ptr: *const u8,
    flow_id_len: usize,
    json_buffer: *mut u8,
    len: usize,
) -> usize {
    let component = Component::default();
    let flow_id = unsafe { slice_to_str(flow_id_ptr, flow_id_len) };
    let result = component.prepare_flow(flow_id);
    write_json_response(&result, json_buffer, len)
}

#[no_mangle]
pub extern "C" fn greentic_pack_export__run_flow(
    flow_id_ptr: *const u8,
    flow_id_len: usize,
    json_buffer: *mut u8,
    len: usize,
) -> usize {
    let component = Component::default();
    let flow_id = unsafe { slice_to_str(flow_id_ptr, flow_id_len) };
    let result = component.run_flow(flow_id, serde_json::Value::Null);
    write_json_response(&result, json_buffer, len)
}

#[no_mangle]
pub extern "C" fn greentic_pack_export__a2a_search(json_buffer: *mut u8, len: usize) -> usize {
    let component = Component::default();
    let items = component.a2a_search("");
    write_json_response(&items, json_buffer, len)
}

fn write_json_response<T: serde::Serialize>(value: &T, buffer: *mut u8, len: usize) -> usize {
    let json = serde_json::to_vec(value).expect("serialisation succeeds");
    if buffer.is_null() || len == 0 {
        return json.len();
    }

    let copy_len = core::cmp::min(json.len(), len);
    unsafe {
        core::ptr::copy_nonoverlapping(json.as_ptr(), buffer, copy_len);
    }
    copy_len
}

unsafe fn slice_to_str<'a>(ptr: *const u8, len: usize) -> &'a str {
    let bytes = core::slice::from_raw_parts(ptr, len);
    core::str::from_utf8(bytes).expect("flow id is valid utf-8")
}
