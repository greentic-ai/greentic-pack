use crate::component::node;

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_node_export_prefix {
    () => {
        "greentic:component/node@0.5.0"
    };
}

/// Prefix for all `greentic:component/node@0.5.0` exports.
pub const NODE_EXPORT_PREFIX: &str = __greentic_component_node_export_prefix!();

#[doc(hidden)]
/// Builds default stream events from an invoke result.
pub fn stream_from_invoke_result(result: node::InvokeResult) -> Vec<node::StreamEvent> {
    match result {
        node::InvokeResult::Ok(body) => vec![
            node::StreamEvent::Progress(0),
            node::StreamEvent::Data(body),
            node::StreamEvent::Done,
        ],
        node::InvokeResult::Err(err) => vec![
            node::StreamEvent::Progress(0),
            node::StreamEvent::Error(err.message),
            node::StreamEvent::Done,
        ],
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_entrypoint_call_on_start {
    (default, $ctx:ident) => {{
        let _ = &$ctx;
        Ok($crate::component::node::LifecycleStatus::Ok)
    }};
    ({ custom $path:path }, $ctx:ident) => {
        $path($ctx)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_entrypoint_call_on_stop {
    (default, $ctx:ident, $reason:ident) => {{
        let _ = &$ctx;
        let _ = &$reason;
        Ok($crate::component::node::LifecycleStatus::Ok)
    }};
    ({ custom $path:path }, $ctx:ident, $reason:ident) => {
        $path($ctx, $reason)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_entrypoint_stream {
    ($result:expr) => {
        $crate::component_entrypoint::stream_from_invoke_result($result)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_entrypoint_expand {
    (
        manifest: $manifest:path,
        invoke: $invoke:path,
        invoke_stream: $invoke_stream:expr,
        on_start: $on_start:tt,
        on_stop: $on_stop:tt,
        lifecycle: $lifecycle:expr
    ) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            use $crate::component::node as component_node;

            #[used]
            #[unsafe(link_section = ".greentic.wasi")]
            static WASI_TARGET_MARKER: [u8; 13] = *b"wasm32-wasip2";

            struct Component;

            impl component_node::Guest for Component {
                fn get_manifest() -> String {
                    $manifest()
                }

                fn on_start(ctx: component_node::ExecCtx) -> Result<component_node::LifecycleStatus, String> {
                    if !$lifecycle {
                        let _ = ctx;
                        return Ok(component_node::LifecycleStatus::Ok);
                    }
                    $crate::__greentic_component_entrypoint_call_on_start!($on_start, ctx)
                }

                fn on_stop(
                    ctx: component_node::ExecCtx,
                    reason: String,
                ) -> Result<component_node::LifecycleStatus, String> {
                    if !$lifecycle {
                        let _ = ctx;
                        let _ = reason;
                        return Ok(component_node::LifecycleStatus::Ok);
                    }
                    $crate::__greentic_component_entrypoint_call_on_stop!($on_stop, ctx, reason)
                }

                fn invoke(ctx: component_node::ExecCtx, op: String, input: String) -> component_node::InvokeResult {
                    let _ = ctx;
                    $invoke(op, input)
                }

                fn invoke_stream(
                    ctx: component_node::ExecCtx,
                    op: String,
                    input: String,
                ) -> Vec<component_node::StreamEvent> {
                    if $invoke_stream {
                        $crate::__greentic_component_entrypoint_stream!(Self::invoke(ctx, op, input))
                    } else {
                        vec![component_node::StreamEvent::Done]
                    }
                }
            }

            use $crate::bindings::greentic_component_0_5_0_component::exports::greentic::component::node as bindings_node;

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#get-manifest"))]
            unsafe extern "C" fn export_get_manifest() -> *mut u8 {
                bindings_node::_export_get_manifest_cabi::<Component>()
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#get-manifest"))]
            unsafe extern "C" fn post_return_get_manifest(arg0: *mut u8) {
                bindings_node::__post_return_get_manifest::<Component>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#on-start"))]
            unsafe extern "C" fn export_on_start(arg0: *mut u8) -> *mut u8 {
                bindings_node::_export_on_start_cabi::<Component>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#on-start"))]
            unsafe extern "C" fn post_return_on_start(arg0: *mut u8) {
                bindings_node::__post_return_on_start::<Component>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#on-stop"))]
            unsafe extern "C" fn export_on_stop(arg0: *mut u8) -> *mut u8 {
                bindings_node::_export_on_stop_cabi::<Component>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#on-stop"))]
            unsafe extern "C" fn post_return_on_stop(arg0: *mut u8) {
                bindings_node::__post_return_on_stop::<Component>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#invoke"))]
            unsafe extern "C" fn export_invoke(arg0: *mut u8) -> *mut u8 {
                bindings_node::_export_invoke_cabi::<Component>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#invoke"))]
            unsafe extern "C" fn post_return_invoke(arg0: *mut u8) {
                bindings_node::__post_return_invoke::<Component>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#invoke-stream"))]
            unsafe extern "C" fn export_invoke_stream(arg0: *mut u8) -> *mut u8 {
                bindings_node::_export_invoke_stream_cabi::<Component>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#invoke-stream"))]
            unsafe extern "C" fn post_return_invoke_stream(arg0: *mut u8) {
                bindings_node::__post_return_invoke_stream::<Component>(arg0);
            }
        };
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __greentic_component_entrypoint_parse {
    (
        @options { }
        {
            manifest: $manifest:path,
            invoke: $invoke:path,
            invoke_stream: $invoke_stream:expr,
            on_start: $on_start:tt,
            on_stop: $on_stop:tt,
            lifecycle: $lifecycle:expr
        }
    ) => {
        $crate::__greentic_component_entrypoint_expand! {
            manifest: $manifest,
            invoke: $invoke,
            invoke_stream: $invoke_stream,
            on_start: $on_start,
            on_stop: $on_stop,
            lifecycle: $lifecycle
        }
    };

    (
        @options { , $($rest:tt)* }
        $state:tt
    ) => {
        $crate::__greentic_component_entrypoint_parse! { @options { $($rest)* } $state }
    };

    (
        @options { invoke_stream: $value:expr $(, $($rest:tt)*)? }
        {
            manifest: $manifest:path,
            invoke: $invoke:path,
            invoke_stream: $invoke_stream:expr,
            on_start: $on_start:tt,
            on_stop: $on_stop:tt,
            lifecycle: $lifecycle:expr
        }
    ) => {
        $crate::__greentic_component_entrypoint_parse! {
            @options { $($($rest)*)? }
            {
                manifest: $manifest,
                invoke: $invoke,
                invoke_stream: $value,
                on_start: $on_start,
                on_stop: $on_stop,
                lifecycle: $lifecycle
            }
        }
    };

    (
        @options { on_start: $value:path $(, $($rest:tt)*)? }
        {
            manifest: $manifest:path,
            invoke: $invoke:path,
            invoke_stream: $invoke_stream:expr,
            on_start: $on_start:tt,
            on_stop: $on_stop:tt,
            lifecycle: $lifecycle:expr
        }
    ) => {
        $crate::__greentic_component_entrypoint_parse! {
            @options { $($($rest)*)? }
            {
                manifest: $manifest,
                invoke: $invoke,
                invoke_stream: $invoke_stream,
                on_start: { custom $value },
                on_stop: $on_stop,
                lifecycle: $lifecycle
            }
        }
    };

    (
        @options { on_stop: $value:path $(, $($rest:tt)*)? }
        {
            manifest: $manifest:path,
            invoke: $invoke:path,
            invoke_stream: $invoke_stream:expr,
            on_start: $on_start:tt,
            on_stop: $on_stop:tt,
            lifecycle: $lifecycle:expr
        }
    ) => {
        $crate::__greentic_component_entrypoint_parse! {
            @options { $($($rest)*)? }
            {
                manifest: $manifest,
                invoke: $invoke,
                invoke_stream: $invoke_stream,
                on_start: $on_start,
                on_stop: { custom $value },
                lifecycle: $lifecycle
            }
        }
    };

    (
        @options { lifecycle: $value:expr $(, $($rest:tt)*)? }
        {
            manifest: $manifest:path,
            invoke: $invoke:path,
            invoke_stream: $invoke_stream:expr,
            on_start: $on_start:tt,
            on_stop: $on_stop:tt,
            lifecycle: $lifecycle:expr
        }
    ) => {
        $crate::__greentic_component_entrypoint_parse! {
            @options { $($($rest)*)? }
            {
                manifest: $manifest,
                invoke: $invoke,
                invoke_stream: $invoke_stream,
                on_start: $on_start,
                on_stop: $on_stop,
                lifecycle: $value
            }
        }
    };

    (@options { $unexpected:ident : $($rest:tt)* } $_state:tt) => {
        compile_error!(concat!("unknown component_entrypoint! option: ", stringify!($unexpected)));
    };

    (@options { $($tokens:tt)+ } $_state:tt) => {
        compile_error!("malformed component_entrypoint! options");
    };
}

/// Export a `greentic:component/node@0.5.0` implementation without handwritten glue.
#[macro_export]
macro_rules! component_entrypoint {
    ({ manifest: $manifest:path, invoke: $invoke:path $(, $($rest:tt)*)? }) => {
        $crate::__greentic_component_entrypoint_parse! {
            @options { $($($rest)*)? }
            {
                manifest: $manifest,
                invoke: $invoke,
                invoke_stream: true,
                on_start: default,
                on_stop: default,
                lifecycle: true
            }
        }
    };

    ({ invoke: $invoke:path, manifest: $manifest:path $(, $($rest:tt)*)? }) => {
        $crate::component_entrypoint!({ manifest: $manifest, invoke: $invoke $(, $($rest)*)? })
    };

    ($other:tt) => {
        compile_error!("component_entrypoint! expects a map like { manifest: describe_payload, invoke: handle_message, ... }");
    };
}

/// Helper macro to export an implementation of `greentic:component/node@0.5.0`.
#[macro_export]
macro_rules! export_component_node {
    ($ty:ty) => {
        const _: () = {
            use $crate::bindings::greentic_component_0_5_0_component::exports::greentic::component::node;

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#get-manifest"))]
            unsafe extern "C" fn export_get_manifest() -> *mut u8 {
                node::_export_get_manifest_cabi::<$ty>()
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#get-manifest"))]
            unsafe extern "C" fn post_return_get_manifest(arg0: *mut u8) {
                node::__post_return_get_manifest::<$ty>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#on-start"))]
            unsafe extern "C" fn export_on_start(arg0: *mut u8) -> *mut u8 {
                node::_export_on_start_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#on-start"))]
            unsafe extern "C" fn post_return_on_start(arg0: *mut u8) {
                node::__post_return_on_start::<$ty>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#on-stop"))]
            unsafe extern "C" fn export_on_stop(arg0: *mut u8) -> *mut u8 {
                node::_export_on_stop_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#on-stop"))]
            unsafe extern "C" fn post_return_on_stop(arg0: *mut u8) {
                node::__post_return_on_stop::<$ty>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#invoke"))]
            unsafe extern "C" fn export_invoke(arg0: *mut u8) -> *mut u8 {
                node::_export_invoke_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#invoke"))]
            unsafe extern "C" fn post_return_invoke(arg0: *mut u8) {
                node::__post_return_invoke::<$ty>(arg0);
            }

            #[unsafe(export_name = concat!($crate::__greentic_component_node_export_prefix!(), "#invoke-stream"))]
            unsafe extern "C" fn export_invoke_stream(arg0: *mut u8) -> *mut u8 {
                node::_export_invoke_stream_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = concat!("cabi_post_", $crate::__greentic_component_node_export_prefix!(), "#invoke-stream"))]
            unsafe extern "C" fn post_return_invoke_stream(arg0: *mut u8) {
                node::__post_return_invoke_stream::<$ty>(arg0);
            }
        };
    };
}
