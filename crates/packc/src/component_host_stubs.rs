#![forbid(unsafe_code)]

use anyhow::Result;
use wasmtime::component::Linker;
use wasmtime_wasi::p2::add_to_linker_sync;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wasmtime_wasi_tls::{
    LinkOptions as WasiTlsLinkOptions, WasiTls, WasiTlsCtx, WasiTlsCtxBuilder,
};

pub struct DescribeHostState {
    table: ResourceTable,
    wasi: WasiCtx,
    wasi_http: WasiHttpCtx,
    wasi_tls: WasiTlsCtx,
}

impl Default for DescribeHostState {
    fn default() -> Self {
        // Describe-only paths should be offline-safe: no inherited env/args,
        // no preopened directories, and no ambient stdio requirements.
        let mut wasi = WasiCtxBuilder::new();
        Self {
            table: ResourceTable::new(),
            wasi: wasi.build(),
            wasi_http: WasiHttpCtx::new(),
            wasi_tls: WasiTlsCtxBuilder::new().build(),
        }
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
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.wasi_http
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

pub fn add_describe_host_imports(linker: &mut Linker<DescribeHostState>) -> Result<()> {
    // Some WASI helper registrars may re-export overlapping interface names
    // (for example `wasi:io/*`) across preview2, TLS, and HTTP worlds.
    linker.allow_shadowing(true);

    add_to_linker_sync(linker)
        .map_err(|err| anyhow::anyhow!("register wasi preview2 describe host stubs: {err}"))?;

    let mut tls_options = WasiTlsLinkOptions::default();
    tls_options.tls(true);
    wasmtime_wasi_tls::add_to_linker(linker, &mut tls_options, |host: &mut DescribeHostState| {
        WasiTls::new(&host.wasi_tls, &mut host.table)
    })
    .map_err(|err| anyhow::anyhow!("register wasi tls describe host stubs: {err}"))?;

    wasmtime_wasi_http::add_only_http_to_linker_sync(linker)
        .map_err(|err| anyhow::anyhow!("register wasi http describe host stubs: {err}"))?;

    // NOTE: greentic host interfaces (state-store, secrets-store, http-client,
    // interfaces-types, etc.) are NOT registered here. They are handled by
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
