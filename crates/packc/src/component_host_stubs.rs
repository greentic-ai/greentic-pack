#![forbid(unsafe_code)]

use anyhow::Result;
use greentic_interfaces_wasmtime::host_helpers::v1::state_store::{
    self as state_store_v1, StateStoreError, StateStoreHost,
};
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
        wasi.allow_ip_name_lookup(true);
        wasi.inherit_network();
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

impl StateStoreHost for DescribeHostState {
    fn read(
        &mut self,
        _key: state_store_v1::StateKey,
        _ctx: Option<state_store_v1::TenantCtx>,
    ) -> std::result::Result<Vec<u8>, StateStoreError> {
        Ok(Vec::new())
    }

    fn write(
        &mut self,
        _key: state_store_v1::StateKey,
        _bytes: Vec<u8>,
        _ctx: Option<state_store_v1::TenantCtx>,
    ) -> std::result::Result<state_store_v1::OpAck, StateStoreError> {
        Ok(state_store_v1::OpAck::Ok)
    }

    fn delete(
        &mut self,
        _key: state_store_v1::StateKey,
        _ctx: Option<state_store_v1::TenantCtx>,
    ) -> std::result::Result<state_store_v1::OpAck, StateStoreError> {
        Ok(state_store_v1::OpAck::Ok)
    }
}

pub fn add_describe_host_imports(linker: &mut Linker<DescribeHostState>) -> Result<()> {
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

    state_store_v1::add_state_store_to_linker(linker, |host: &mut DescribeHostState| host)
        .map_err(|err| anyhow::anyhow!("register state-store@1.0.0 describe host stub: {err}"))?;
    Ok(())
}
