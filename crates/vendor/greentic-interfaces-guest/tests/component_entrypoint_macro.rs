#![cfg(feature = "component-node")]

use greentic_interfaces_guest::component::node::{
    ExecCtx, InvokeResult, LifecycleStatus, NodeError, StreamEvent,
};
use greentic_interfaces_guest::component_entrypoint;

fn describe_payload() -> String {
    r#"{"component":"macro-test"}"#.to_string()
}

fn handle_message(op: String, input: String) -> InvokeResult {
    if op == "fail" {
        InvokeResult::Err(NodeError {
            code: "unavailable".to_string(),
            message: format!("{op}:{input}"),
            retryable: true,
            backoff_ms: None,
            details: None,
        })
    } else {
        InvokeResult::Ok(format!("{op}:{input}"))
    }
}

component_entrypoint!({
    manifest: describe_payload,
    invoke: handle_message,
});

#[test]
fn macro_invocation_compiles_on_host() {
    assert_eq!(
        greentic_interfaces_guest::NODE_EXPORT_PREFIX,
        "greentic:component/node@0.5.0"
    );
    assert!(describe_payload().contains("macro-test"));

    match handle_message("ping".into(), "body".into()) {
        InvokeResult::Ok(body) => assert_eq!(body, "ping:body"),
        InvokeResult::Err(err) => panic!("unexpected invoke error: {err:?}"),
    }
}

#[test]
fn default_stream_mapping_wraps_invoke_result() {
    let ok_events = greentic_interfaces_guest::component_entrypoint::stream_from_invoke_result(
        InvokeResult::Ok("ok-body".to_string()),
    );
    assert!(matches!(
        ok_events.as_slice(),
        [StreamEvent::Progress(0), StreamEvent::Data(body), StreamEvent::Done] if body == "ok-body"
    ));

    let err_events = greentic_interfaces_guest::component_entrypoint::stream_from_invoke_result(
        InvokeResult::Err(NodeError {
            code: "fail".to_string(),
            message: "boom".to_string(),
            retryable: false,
            backoff_ms: Some(5),
            details: None,
        }),
    );
    assert!(matches!(
        err_events.as_slice(),
        [StreamEvent::Progress(0), StreamEvent::Error(msg), StreamEvent::Done] if msg == "boom"
    ));
}

mod override_handlers {
    use super::*;

    fn manifest() -> String {
        r#"{"component":"overrides"}"#.to_string()
    }

    fn invoke_handler(op: String, input: String) -> InvokeResult {
        InvokeResult::Ok(format!("{op}-{input}-override"))
    }

    #[allow(dead_code)]
    fn start(ctx: ExecCtx) -> Result<LifecycleStatus, String> {
        assert!(!ctx.flow_id.is_empty());
        Ok(LifecycleStatus::Ok)
    }

    #[allow(dead_code)]
    fn stop(ctx: ExecCtx, reason: String) -> Result<LifecycleStatus, String> {
        assert!(!ctx.flow_id.is_empty());
        let _ = reason;
        Ok(LifecycleStatus::Ok)
    }

    component_entrypoint!({
        manifest: manifest,
        invoke: invoke_handler,
        invoke_stream: false,
        on_start: start,
        on_stop: stop,
    });

    #[test]
    fn optional_keys_parse_without_wasm_exports() {
        assert!(manifest().contains("overrides"));
        let result = invoke_handler("demo".into(), "data".into());
        assert!(matches!(result, InvokeResult::Ok(body) if body.contains("override")));
    }
}
