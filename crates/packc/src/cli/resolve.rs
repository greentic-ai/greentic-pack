#![forbid(unsafe_code)]

use std::collections::{BTreeMap, btree_map::Entry};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use crate::cli::ext_resolver::resolve_ext_component_with_dist;
use crate::config::load_pack_config;
use crate::flow_resolve::{
    ensure_sidecar_exists, read_flow_resolve_summary_for_flow, strip_file_uri_prefix,
};
use crate::runtime::RuntimeContext;
use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use greentic_distributor_client::{DistClient, DistOptions};
use greentic_flow::compile_ygtc_str;
use greentic_pack::pack_lock::{LockedComponent, PackLockV1, write_pack_lock};
use greentic_pack::resolver::{ComponentResolver, ResolveReq, ResolvedComponent};
use greentic_types::cbor::canonical;
use greentic_types::flow_resolve_summary::{
    FlowResolveSummarySourceRefV1, FlowResolveSummaryV1, resolve_summary_path_for_flow,
    write_flow_resolve_summary,
};
use greentic_types::schemas::component::v0_6_0::{ComponentDescribe, schema_hash};
use hex;
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;
use wasmtime::Engine;
use wasmtime::component::{Component as WasmtimeComponent, Linker};

use crate::component_host_stubs::{
    DescribeHostState, add_describe_host_imports, stub_remaining_imports,
};

#[derive(Debug, Args)]
pub struct ResolveArgs {
    /// Pack root directory containing pack.yaml.
    #[arg(long = "in", value_name = "DIR", default_value = ".")]
    pub input: PathBuf,

    /// Output path for pack.lock.cbor (default: pack.lock.cbor under pack root).
    #[arg(long = "lock", value_name = "FILE")]
    pub lock: Option<PathBuf>,
}

pub async fn handle(args: ResolveArgs, runtime: &RuntimeContext, emit_path: bool) -> Result<()> {
    let pack_dir = args
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve pack dir {}", args.input.display()))?;
    let lock_path = resolve_lock_path(&pack_dir, args.lock.as_deref());

    let config = load_pack_config(&pack_dir)?;
    let mut entries: BTreeMap<String, LockedComponent> = BTreeMap::new();
    for flow in &config.flows {
        let compiled = compile_flow(&pack_dir, flow)?;
        ensure_sidecar_exists(&pack_dir, flow, &compiled, false)?;
        let summary = read_flow_resolve_summary_for_flow(&pack_dir, flow)?;
        collect_from_summary(&pack_dir, flow, &summary, &mut entries)?;
    }

    let mut id_remap: BTreeMap<String, String> = BTreeMap::new();
    if !entries.is_empty() {
        let resolver = PackResolver::new(runtime, pack_dir.clone())?;
        let engine = Engine::default();
        let mut rekeyed = BTreeMap::new();
        for (key, mut component) in entries {
            populate_component_contract(&engine, &resolver, &mut component).await?;
            if component.component_id != key {
                id_remap.insert(key, component.component_id.clone());
            }
            rekeyed.insert(component.component_id.clone(), component);
        }
        entries = rekeyed;
    }

    if !id_remap.is_empty() {
        for flow in &config.flows {
            let flow_path = flow.file.clone();
            let summary_path = resolve_summary_path_for_flow(&flow_path);
            if summary_path.exists() {
                let mut summary = read_flow_resolve_summary_for_flow(&pack_dir, flow)?;
                let mut changed = false;
                for node in summary.nodes.values_mut() {
                    let old_id = node.component_id.as_str().to_string();
                    if let Some(new_id) = id_remap.get(&old_id) {
                        node.component_id = new_id.parse().unwrap_or(node.component_id.clone());
                        changed = true;
                    }
                }
                if changed {
                    write_flow_resolve_summary(&summary_path, &summary)
                        .map_err(|e| anyhow!("{e}"))?;
                }
            }
        }
    }

    let lock = PackLockV1::new(entries);
    write_pack_lock(&lock_path, &lock)?;
    if emit_path {
        eprintln!(
            "{}",
            crate::cli_i18n::tf("cli.common.wrote_path", &[&lock_path.display().to_string()])
        );
    }

    Ok(())
}

fn compile_flow(pack_dir: &Path, flow: &crate::config::FlowConfig) -> Result<greentic_types::Flow> {
    let flow_path = if flow.file.is_absolute() {
        flow.file.clone()
    } else {
        pack_dir.join(&flow.file)
    };
    let yaml_src = fs::read_to_string(&flow_path)
        .with_context(|| format!("failed to read flow {}", flow_path.display()))?;
    compile_ygtc_str(&yaml_src)
        .with_context(|| format!("failed to compile flow {}", flow_path.display()))
}

fn resolve_lock_path(pack_dir: &Path, override_path: Option<&Path>) -> PathBuf {
    match override_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => pack_dir.join(path),
        None => pack_dir.join("pack.lock.cbor"),
    }
}

fn collect_from_summary(
    pack_dir: &Path,
    flow: &crate::config::FlowConfig,
    doc: &FlowResolveSummaryV1,
    out: &mut BTreeMap<String, LockedComponent>,
) -> Result<()> {
    let mut seen: BTreeMap<String, LockedComponent> = BTreeMap::new();

    for resolve in doc.nodes.values() {
        let source_ref = &resolve.source;
        let (reference, digest) = match source_ref {
            FlowResolveSummarySourceRefV1::Local { path } => {
                let abs = normalize_local(pack_dir, flow, path)?;
                (
                    format!("file://{}", abs.to_string_lossy()),
                    resolve.digest.clone(),
                )
            }
            FlowResolveSummarySourceRefV1::Oci { .. }
            | FlowResolveSummarySourceRefV1::Repo { .. }
            | FlowResolveSummarySourceRefV1::Store { .. } => {
                (format_reference(source_ref), resolve.digest.clone())
            }
            FlowResolveSummarySourceRefV1::Ext { r#ref } => {
                // ext:// refs carry the raw ref string; digest is deferred to PackResolver.
                (r#ref.clone(), resolve.digest.clone())
            }
        };
        let component_id = resolve.component_id.clone();
        let key = component_id.as_str().to_string();
        let reference_for_insert = reference.clone();
        let digest_for_insert = digest.clone();
        let world = resolve.manifest.as_ref().map(|meta| meta.world.clone());
        let component_version = resolve
            .manifest
            .as_ref()
            .map(|meta| meta.version.to_string());
        match seen.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(LockedComponent {
                    component_id: component_id.as_str().to_string(),
                    r#ref: Some(reference_for_insert),
                    abi_version: "0.6.0".to_string(),
                    resolved_digest: digest_for_insert,
                    describe_hash: String::new(),
                    operations: Vec::new(),
                    world,
                    component_version,
                    role: None,
                });
            }
            Entry::Occupied(entry) => {
                let existing = entry.get();
                if existing.r#ref.as_deref() != Some(reference.as_str())
                    || existing.resolved_digest != digest
                {
                    bail!(
                        "component {} resolved by nodes points to different artifacts ({}@{} vs {}@{})",
                        component_id.as_str(),
                        existing.r#ref.as_deref().unwrap_or("unknown-ref"),
                        existing.resolved_digest,
                        reference,
                        digest
                    );
                }
            }
        }
    }

    out.extend(seen);

    Ok(())
}

async fn populate_component_contract(
    engine: &Engine,
    resolver: &dyn ComponentResolver,
    component: &mut LockedComponent,
) -> Result<()> {
    if is_builtin_component(component.component_id.as_str()) {
        component.describe_hash = "0".repeat(64);
        component.operations.clear();
        component.role = Some("builtin".to_string());
        if component.component_version.is_none() {
            component.component_version = Some("0.0.0".to_string());
        }
        return Ok(());
    }

    let reference = component
        .r#ref
        .as_ref()
        .ok_or_else(|| anyhow!("component {} missing ref", component.component_id))?;
    let resolved = resolver.resolve(ResolveReq {
        component_id: component.component_id.clone(),
        reference: reference.clone(),
        expected_digest: component.resolved_digest.clone(),
        abi_version: component.abi_version.clone(),
        world: component.world.clone(),
        component_version: component.component_version.clone(),
    })?;
    let bytes = resolved.bytes;
    component.resolved_digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    let use_describe_cache =
        std::env::var("GREENTIC_PACK_USE_DESCRIBE_CACHE").is_ok() || cfg!(test);
    let describe = match describe_component(engine, &bytes) {
        Ok(describe) => describe,
        Err(err) => {
            if let Some(describe) = load_describe_from_cache_path(resolved.source_path.as_deref())?
            {
                describe
            } else if is_state_store_tenant_ctx_abi_mismatch(&err)
                || is_known_host_linker_gap(&err)
                || is_missing_descriptor_instance(&err)
            {
                // Temporary compat fallback: keep resolve/build working for components
                // whose state-store import still mismatches host stubs (19 vs 18 fields).
                component.describe_hash = component
                    .resolved_digest
                    .strip_prefix("sha256:")
                    .unwrap_or(component.resolved_digest.as_str())
                    .to_string();
                component.operations.clear();
                component.role = Some("unknown".to_string());
                if component.component_version.is_none() {
                    component.component_version = Some("0.0.0".to_string());
                }
                return Ok(());
            } else if use_describe_cache {
                return Err(err).context("describe failed and no describe cache present");
            } else {
                return Err(err);
            }
        }
    };

    if describe.info.id != component.component_id {
        eprintln!(
            "warning: component {} describe id mismatch (expected {}, got {}); using describe id",
            component.component_id, component.component_id, describe.info.id
        );
        component.component_id = describe.info.id.clone();
    }

    let describe_hash = compute_describe_hash(&describe)?;
    let mut operations: Vec<_> = describe
        .operations
        .iter()
        .map(|op| {
            let hash = schema_hash(&op.input.schema, &op.output.schema, &describe.config_schema)
                .map_err(|err| anyhow!("schema_hash for {}: {}", op.id, err))?;
            Ok((op.id.clone(), hash))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(
            |(operation_id, schema_hash)| greentic_pack::pack_lock::LockedOperation {
                operation_id,
                schema_hash,
            },
        )
        .collect();
    operations.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));

    component.describe_hash = describe_hash;
    component.operations = operations;
    component.role = Some(describe.info.role);
    component.component_version = Some(describe.info.version);
    Ok(())
}

/// Delegates to the single source of truth in `build` so resolve and build
/// agree on which flow nodes are runner builtins (incl. `dw.agent` and the
/// other dispatch kinds), rather than maintaining a second list that can drift.
fn is_builtin_component(component_id: &str) -> bool {
    crate::build::is_builtin_component_id(component_id)
}

struct PackResolver {
    runtime: RuntimeContext,
    dist: DistClient,
    pack_dir: PathBuf,
}

impl PackResolver {
    fn new(runtime: &RuntimeContext, pack_dir: PathBuf) -> Result<Self> {
        let dist = DistClient::new(DistOptions {
            cache_dir: runtime.cache_dir(),
            allow_tags: true,
            offline: runtime.network_policy() == crate::runtime::NetworkPolicy::Offline,
            allow_insecure_local_http: false,
            ..DistOptions::default()
        });
        Ok(Self {
            runtime: runtime.clone(),
            dist,
            pack_dir,
        })
    }
}

impl ComponentResolver for PackResolver {
    fn resolve(&self, req: ResolveReq) -> Result<ResolvedComponent> {
        if req.reference.starts_with("file://") {
            let path = strip_file_uri_prefix(&req.reference);
            let bytes = fs::read(path).with_context(|| format!("read {}", path))?;
            return Ok(ResolvedComponent {
                bytes,
                resolved_digest: req.expected_digest,
                component_id: req.component_id,
                abi_version: req.abi_version,
                world: req.world,
                component_version: req.component_version,
                source_path: Some(PathBuf::from(path)),
            });
        }

        if req.reference.starts_with("ext://") {
            let offline = self.runtime.network_policy() == crate::runtime::NetworkPolicy::Offline;
            let handle = Handle::try_current().ok();
            let (bytes, verified_digest) = resolve_ext_component_with_dist(
                &self.pack_dir,
                &req.reference,
                &self.runtime.cache_dir(),
                offline,
                handle.as_ref(),
            )
            .with_context(|| format!("resolve ext:// component ref '{}'", req.reference))?;
            return Ok(ResolvedComponent {
                bytes,
                resolved_digest: verified_digest,
                component_id: req.component_id,
                abi_version: req.abi_version,
                world: req.world,
                component_version: req.component_version,
                source_path: None,
            });
        }

        let handle =
            Handle::try_current().context("component resolution requires a Tokio runtime")?;
        let offline = self.runtime.network_policy() == crate::runtime::NetworkPolicy::Offline;
        let resolved = if offline {
            self.dist
                .open_cached(&req.expected_digest)
                .map_err(|err| anyhow!("offline cache miss for {}: {}", req.reference, err))?
        } else {
            let source = self
                .dist
                .parse_source(&req.reference)
                .map_err(|err| anyhow!("resolve {}: {}", req.reference, err))?;
            let descriptor = block_on(
                &handle,
                self.dist
                    .resolve(source, greentic_distributor_client::ResolvePolicy),
            )
            .map_err(|err| anyhow!("resolve {}: {}", req.reference, err))?;
            block_on(
                &handle,
                self.dist
                    .fetch(&descriptor, greentic_distributor_client::CachePolicy),
            )
            .map_err(|err| anyhow!("resolve {}: {}", req.reference, err))?
        };
        let path = resolved
            .cache_path
            .ok_or_else(|| anyhow!("resolved component missing path for {}", req.reference))?;
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(ResolvedComponent {
            bytes,
            resolved_digest: req.expected_digest,
            component_id: req.component_id,
            abi_version: req.abi_version,
            world: req.world,
            component_version: req.component_version,
            source_path: Some(path),
        })
    }
}

fn block_on<F, T, E>(handle: &Handle, fut: F) -> std::result::Result<T, E>
where
    F: Future<Output = std::result::Result<T, E>>,
{
    tokio::task::block_in_place(|| handle.block_on(fut))
}

fn describe_component(engine: &Engine, bytes: &[u8]) -> Result<ComponentDescribe> {
    describe_component_untyped(engine, bytes)
}

fn describe_component_untyped(engine: &Engine, bytes: &[u8]) -> Result<ComponentDescribe> {
    let component = WasmtimeComponent::from_binary(engine, bytes)
        .map_err(|err| anyhow!("decode component bytes: {err}"))?;
    let mut store = wasmtime::Store::new(engine, DescribeHostState::default());
    let mut linker = Linker::new(engine);
    add_describe_host_imports(&mut linker)?;
    // Stub any remaining imports (secrets-store, http-client, interfaces-types,
    // etc.) that the component needs but describe() never calls at runtime.
    stub_remaining_imports(&mut linker, &component)?;
    let instance = linker
        .instantiate(&mut store, &component)
        .map_err(|err| anyhow!("instantiate component root world: {err}"))?;

    let descriptor = [
        "component-descriptor",
        "greentic:component/component-descriptor",
        "greentic:component/component-descriptor@0.6.0",
    ]
    .iter()
    .find_map(|name| instance.get_export_index(&mut store, None, name))
    .ok_or_else(|| anyhow!("missing exported descriptor instance"))?;
    let describe_export = [
        "describe",
        "greentic:component/component-descriptor@0.6.0#describe",
    ]
    .iter()
    .find_map(|name| instance.get_export_index(&mut store, Some(&descriptor), name))
    .ok_or_else(|| anyhow!("missing exported describe function"))?;
    let describe_func = instance
        .get_typed_func::<(), (Vec<u8>,)>(&mut store, &describe_export)
        .map_err(|err| anyhow!("lookup component-descriptor.describe: {err}"))?;
    let (describe_bytes,) = describe_func
        .call(&mut store, ())
        .map_err(|err| anyhow!("call component-descriptor.describe: {err}"))?;
    canonical::from_cbor(&describe_bytes).context("decode ComponentDescribe")
}

fn load_describe_from_cache_path(path: Option<&Path>) -> Result<Option<ComponentDescribe>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let describe_path = PathBuf::from(format!("{}.describe.cbor", path.display()));
    if !describe_path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(&describe_path).with_context(|| format!("read {}", describe_path.display()))?;
    canonical::ensure_canonical(&bytes).context("describe cache must be canonical")?;
    let describe = canonical::from_cbor(&bytes).context("decode ComponentDescribe from cache")?;
    Ok(Some(describe))
}

fn compute_describe_hash(describe: &ComponentDescribe) -> Result<String> {
    let bytes =
        canonical::to_canonical_cbor_allow_floats(describe).context("canonicalize describe")?;
    let digest = Sha256::digest(bytes.as_slice());
    Ok(hex::encode(digest))
}

fn is_state_store_tenant_ctx_abi_mismatch(err: &anyhow::Error) -> bool {
    let text = format!("{:#}", err);
    text.contains("greentic:state/state-store@1.0.0")
        && text.contains("expected record of 19 fields, found 18 fields")
}

fn is_known_host_linker_gap(err: &anyhow::Error) -> bool {
    let text = format!("{:#}", err);
    let missing_impl = text.contains("matching implementation was not found in the linker");
    missing_impl
        && (text.contains("greentic:http/http-client@1.1.0")
            || text.contains("greentic:http/http-client@1.0.0"))
}

fn is_missing_descriptor_instance(err: &anyhow::Error) -> bool {
    format!("{:#}", err).contains("missing exported descriptor instance")
}

fn normalize_local(
    pack_dir: &Path,
    flow: &crate::config::FlowConfig,
    rel: &str,
) -> Result<PathBuf> {
    let flow_path = if flow.file.is_absolute() {
        flow.file.clone()
    } else {
        pack_dir.join(&flow.file)
    };
    let parent = flow_path
        .parent()
        .ok_or_else(|| anyhow!("flow path {} has no parent", flow_path.display()))?;
    let rel = strip_file_uri_prefix(rel);
    Ok(parent.join(rel))
}

fn format_reference(source: &FlowResolveSummarySourceRefV1) -> String {
    match source {
        FlowResolveSummarySourceRefV1::Local { path } => path.clone(),
        FlowResolveSummarySourceRefV1::Oci { r#ref } => {
            if r#ref.contains("://") {
                r#ref.clone()
            } else {
                format!("oci://{}", r#ref)
            }
        }
        FlowResolveSummarySourceRefV1::Repo { r#ref } => {
            if r#ref.contains("://") {
                r#ref.clone()
            } else {
                format!("repo://{}", r#ref)
            }
        }
        FlowResolveSummarySourceRefV1::Store { r#ref } => {
            if r#ref.contains("://") {
                r#ref.clone()
            } else {
                format!("store://{}", r#ref)
            }
        }
        FlowResolveSummarySourceRefV1::Ext { r#ref } => r#ref.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::resolve_runtime;
    use greentic_types::ComponentId;
    use greentic_types::flow_resolve::{read_flow_resolve, sidecar_path_for_flow};
    use greentic_types::flow_resolve_summary::{
        FlowResolveSummarySourceRefV1, FlowResolveSummaryV1, NodeResolveSummaryV1,
        read_flow_resolve_summary, resolve_summary_path_for_flow,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_flow() -> crate::config::FlowConfig {
        crate::config::FlowConfig {
            id: "meetingPrep".to_string(),
            file: PathBuf::from("flows/main.ygtc"),
            tags: Vec::new(),
            entrypoints: Vec::new(),
        }
    }

    #[test]
    fn collect_from_summary_dedups_duplicate_component_ids() {
        let flow = sample_flow();
        let pack_dir = PathBuf::from("/tmp");
        let component_id =
            ComponentId::new("ai.greentic.component-adaptive-card").expect("valid component id");

        let mut nodes = BTreeMap::new();
        for name in ["node_one", "node_two"] {
            nodes.insert(
                name.to_string(),
                NodeResolveSummaryV1 {
                    component_id: component_id.clone(),
                    source: FlowResolveSummarySourceRefV1::Oci {
                        r#ref: "oci://ghcr.io/greenticai/components/component-adaptive-card:stable"
                            .to_string(),
                    },
                    digest: format!("sha256:{}", "a".repeat(64)),
                    manifest: None,
                },
            );
        }

        let summary = FlowResolveSummaryV1 {
            schema_version: 1,
            flow: "main.ygtc".to_string(),
            nodes,
        };

        let mut entries = BTreeMap::new();
        collect_from_summary(&pack_dir, &flow, &summary, &mut entries).expect("collect entries");

        assert_eq!(entries.len(), 1);
        let entry = entries.get(component_id.as_str()).expect("component entry");
        assert_eq!(entry.component_id, component_id.as_str());
    }

    #[test]
    fn collect_from_summary_rejects_conflicting_lock_data() {
        let flow = sample_flow();
        let pack_dir = PathBuf::from("/tmp");
        let component_id =
            ComponentId::new("ai.greentic.component-adaptive-card").expect("valid component id");

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "alpha".to_string(),
            NodeResolveSummaryV1 {
                component_id: component_id.clone(),
                source: FlowResolveSummarySourceRefV1::Oci {
                    r#ref: "oci://ghcr.io/greenticai/components/component-adaptive-card:stable"
                        .to_string(),
                },
                digest: format!("sha256:{}", "b".repeat(64)),
                manifest: None,
            },
        );
        nodes.insert(
            "beta".to_string(),
            NodeResolveSummaryV1 {
                component_id: component_id.clone(),
                source: FlowResolveSummarySourceRefV1::Oci {
                    r#ref: "oci://ghcr.io/greenticai/components/component-adaptive-card:stable"
                        .to_string(),
                },
                digest: format!("sha256:{}", "c".repeat(64)),
                manifest: None,
            },
        );

        let summary = FlowResolveSummaryV1 {
            schema_version: 1,
            flow: "main.ygtc".to_string(),
            nodes,
        };

        let mut entries = BTreeMap::new();
        let err = collect_from_summary(&pack_dir, &flow, &summary, &mut entries).unwrap_err();
        assert!(err.to_string().contains("points to different artifacts"));
    }

    #[tokio::test]
    async fn handle_creates_missing_sidecar_before_reading_summary() {
        let temp = TempDir::new().expect("tempdir");
        let pack_dir = temp.path().join("pack");
        fs::create_dir_all(pack_dir.join("flows")).expect("create flows dir");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"pack_id: dev.local.resolve-sidecar
version: 0.1.0
kind: application
publisher: Test
components: []
dependencies: []
flows:
- id: main
  file: flows/main.ygtc
  tags: [default]
  entrypoints: [default]
assets: []
"#,
        )
        .expect("write pack yaml");
        fs::write(
            pack_dir.join("flows/main.ygtc"),
            r#"id: main
type: messaging
start: hello
nodes:
  hello:
    handle_message:
      input: "hi"
    routing:
      - out: true
"#,
        )
        .expect("write flow");

        let runtime = resolve_runtime(Some(temp.path()), None, true, None).expect("runtime");
        handle(
            ResolveArgs {
                input: pack_dir.clone(),
                lock: None,
            },
            &runtime,
            false,
        )
        .await
        .expect("resolve should create sidecar instead of failing");

        let flow_cfg = sample_flow();
        let sidecar_path = sidecar_path_for_flow(&pack_dir.join(&flow_cfg.file));
        let summary_path = resolve_summary_path_for_flow(&pack_dir.join(&flow_cfg.file));
        assert!(
            sidecar_path.exists(),
            "resolve should create the missing sidecar"
        );
        assert!(
            summary_path.exists(),
            "resolve should write a summary for the new sidecar"
        );

        let sidecar = read_flow_resolve(&sidecar_path).expect("read sidecar");
        assert!(sidecar.nodes.is_empty(), "new sidecar should start empty");

        let summary = read_flow_resolve_summary(&summary_path).expect("read summary");
        assert!(
            summary.nodes.is_empty(),
            "summary should reflect the empty sidecar"
        );

        let lock_path = pack_dir.join("pack.lock.cbor");
        assert!(
            lock_path.exists(),
            "resolve should still write pack.lock.cbor"
        );
    }
}
