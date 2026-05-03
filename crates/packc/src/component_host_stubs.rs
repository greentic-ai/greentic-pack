#![forbid(unsafe_code)]

use anyhow::Result;
use wasmtime::component::Linker;
use wasmtime_wasi::p2::add_to_linker_sync;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::WasiHttpCtx;
use wasmtime_wasi_http::p2::{
    HttpError, HttpResult, WasiHttpCtxView, WasiHttpHooks, WasiHttpView,
    add_only_http_to_linker_sync,
};
use wasmtime_wasi_http::p2::{body, types};

pub struct DescribeHostState {
    table: ResourceTable,
    wasi: WasiCtx,
    wasi_http: WasiHttpCtx,
    http_hooks: DescribeHttpHooks,
}

#[derive(Default)]
struct DescribeHttpHooks;

impl Default for DescribeHostState {
    fn default() -> Self {
        // Describe-only paths should be offline-safe: no inherited env/args,
        // no preopened directories, and no ambient stdio requirements.
        let mut wasi = WasiCtxBuilder::new();
        Self {
            table: ResourceTable::new(),
            wasi: wasi.build(),
            wasi_http: WasiHttpCtx::new(),
            http_hooks: DescribeHttpHooks,
        }
    }
}

impl WasiHttpHooks for DescribeHttpHooks {
    fn send_request(
        &mut self,
        _request: hyper::Request<body::HyperOutgoingBody>,
        _config: types::OutgoingRequestConfig,
    ) -> HttpResult<types::HostFutureIncomingResponse> {
        Err(HttpError::trap(wasmtime::Error::msg(
            "wasi:http outgoing requests are unavailable in describe-only host stubs",
        )))
    }
}

impl WasiView for DescribeHostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            table: &mut self.table,
            ctx: &mut self.wasi,
        }
    }
}

impl WasiHttpView for DescribeHostState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.wasi_http,
            table: &mut self.table,
            hooks: &mut self.http_hooks,
        }
    }
}

pub fn add_describe_host_imports(linker: &mut Linker<DescribeHostState>) -> Result<()> {
    // Some WASI helper registrars may re-export overlapping interface names
    // (for example `wasi:io/*`) across preview2, TLS, and HTTP worlds.
    linker.allow_shadowing(true);

    add_to_linker_sync(linker)
        .map_err(|err| anyhow::anyhow!("register wasi preview2 describe host stubs: {err}"))?;

    add_only_http_to_linker_sync(linker)
        .map_err(|err| anyhow::anyhow!("register wasi http describe host stubs: {err}"))?;

    // NOTE: greentic host interfaces (state-store, secrets-store, http-client,
    // interfaces-types, TLS, etc.) are NOT registered here. They are handled by
    // `stub_remaining_imports` which uses `define_unknown_imports_as_traps` to
    // provide trap stubs for any component imports not already in the linker.
    // This avoids having to maintain an exhaustive list of every greentic
    // interface version a component might import.
    Ok(())
}

/// Stub any component imports not already registered in the linker as traps.
///
/// This must be called AFTER `add_describe_host_imports` so that real WASI
/// implementations take priority. All remaining imports (greentic host
/// interfaces like secrets-store, http-client, etc.) become traps — safe
/// because the describe-only code path never invokes them.
pub fn stub_remaining_imports(
    linker: &mut Linker<DescribeHostState>,
    component: &wasmtime::component::Component,
) -> Result<()> {
    linker
        .define_unknown_imports_as_traps(component)
        .map_err(|err| anyhow::anyhow!("stub remaining component imports: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_host_state_exposes_wasi_and_http_views() {
        let mut state = DescribeHostState::default();

        let ctx_view = state.ctx();
        let _ = ctx_view.table;
        let _ = ctx_view.ctx;

        let http_view = state.http();
        let _ = http_view.table;
        let _ = http_view.ctx;
    }

    #[test]
    fn describe_host_imports_register_without_error() {
        let engine = wasmtime::Engine::default();
        let mut linker = Linker::new(&engine);
        add_describe_host_imports(&mut linker).expect("host imports should register");
    }
}
