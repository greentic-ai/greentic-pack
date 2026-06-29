//! Helper for resolving `ext://<id>#component` references.
//!
//! # Resolution flow
//!
//! 1. Parse the `ext://<id>#component` ref — error if malformed.
//! 2. Look up `<id>` in `pack.extensions.json` via [`read_extensions_file`].
//! 3. Acquire the extension's `.gtxpack` from its declared `file://` or bare local source.
//! 4. Read `component.json` from the ZIP to obtain the component asset path and expected digest.
//! 5. Read the component wasm asset from the ZIP.
//! 6. Verify SHA-256 digest — error on mismatch.
//! 7. Return the extracted bytes.
//!
//! # `component.json` schema (Phase 2 sidecar)
//!
//! The resolver reads a packc-owned `component.json` sidecar at the root of the
//! `.gtxpack` ZIP. It is written alongside the canonical, store-validated `describe.json`
//! (describe-v2) manifest, which itself cannot carry this metadata (its schema root is
//! `additionalProperties: false`).
//!
//! ```json
//! {
//!   "component": {
//!     "id": "greentic.component-http",
//!     "asset": "component.wasm",
//!     "digest": "sha256:<hex>"
//!   }
//! }
//! ```
//!
//! Fields:
//! - `component.id`     — the store id; informational (the resolver does not enforce
//!   `id == extension_id`).
//! - `component.asset`  — ZIP entry name of the runtime wasm. For a `ComponentExtension`
//!   producer this is `component.wasm` at the root; other producers may use arbitrary
//!   paths such as `assets/component-<name>.wasm`.
//! - `component.digest` — `sha256:<hex>` digest of the wasm bytes.
//!
//! The Phase-2 producer (`greentic-component store publish`) must emit exactly this shape.

#![forbid(unsafe_code)]

use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;

use crate::extension_refs::{
    ExtensionDependency, ExtensionDependencySource, PackExtensionsFile, read_extensions_file,
};

/// Environment variable naming the store base URL used to acquire `store://`
/// extension artifacts (same env the Phase-2 producer publishes against).
const STORE_URL_ENV: &str = "GREENTIC_STORE_URL";

/// Parsed form of `ext://<id>#component`.
#[derive(Debug, Clone)]
pub struct ExtRef {
    /// Extension id (the `<id>` segment).
    pub extension_id: String,
}

/// Parse an `ext://<id>#component` reference.
///
/// Returns an error if the ref is malformed (wrong scheme, missing `#component` fragment,
/// or empty id).
pub fn parse_ext_ref(raw: &str) -> Result<ExtRef> {
    let rest = raw.strip_prefix("ext://").ok_or_else(|| {
        anyhow::anyhow!("ext:// component ref must start with 'ext://' (got '{raw}')")
    })?;
    let (id, fragment) = rest.split_once('#').ok_or_else(|| {
        anyhow::anyhow!(
            "ext:// component ref must have the form 'ext://<id>#component' (got '{raw}')"
        )
    })?;
    if fragment != "component" {
        bail!("ext:// component ref fragment must be '#component' (got '#{fragment}')");
    }
    if id.trim().is_empty() {
        bail!("ext:// component ref extension id must not be empty (got '{raw}')");
    }
    Ok(ExtRef {
        extension_id: id.to_string(),
    })
}

/// Embedded-component descriptor from the `component.json` sidecar inside a `.gtxpack` ZIP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtxpackComponentSidecar {
    /// Embedded runtime component descriptor.
    pub component: GtxpackComponentEntry,
}

/// Single embedded component entry in `component.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtxpackComponentEntry {
    /// Extension id — should match `pack.extensions.json`.
    pub id: String,
    /// Path inside the ZIP to the runtime wasm asset (e.g. `assets/component-foo.wasm`).
    pub asset: String,
    /// SHA-256 digest of the wasm bytes (`sha256:<hex>`).
    pub digest: String,
}

/// Parsed form of a `store://<name>@<version>` extension source ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRef {
    /// Extension name (the `<name>` segment).
    pub name: String,
    /// Explicit version (the `<version>` segment). Required in Phase 3a.
    pub version: String,
}

/// Parse a `store://<name>@<version>` extension source ref.
///
/// Phase 3a requires an explicit version; tag/latest resolution is out of scope,
/// so a missing or empty version is an error.
pub fn parse_store_ref(raw: &str) -> Result<StoreRef> {
    let rest = raw.strip_prefix("store://").ok_or_else(|| {
        anyhow!("store:// extension ref must start with 'store://' (got '{raw}')")
    })?;
    let (name, version) = rest.split_once('@').ok_or_else(|| {
        anyhow!(
            "store:// extension ref must pin a version as 'store://<name>@<version>' (got '{raw}')"
        )
    })?;
    if name.trim().is_empty() {
        bail!("store:// extension ref name must not be empty (got '{raw}')");
    }
    if version.trim().is_empty() {
        bail!("store:// extension ref must pin a non-empty version (got '{raw}')");
    }
    Ok(StoreRef {
        name: name.to_string(),
        version: version.to_string(),
    })
}

/// Resolve an `ext://<id>#component` reference by extracting the wasm from the extension's
/// `.gtxpack` and verifying the digest against the `component.json` sidecar.
///
/// `pack_dir` is the directory containing `pack.extensions.json`.
///
/// This file://-only entry point keeps the Phase-1 signature; network schemes
/// (`store://`/`oci://`) require [`resolve_ext_component_with_dist`].
///
/// Returns the raw wasm bytes and the verified digest string (`sha256:<hex>`).
pub fn resolve_ext_component(pack_dir: &Path, raw_ref: &str) -> Result<(Vec<u8>, String)> {
    let (ext_ref, dep) = lookup_ext_dependency(pack_dir, raw_ref)?;
    let zip_bytes = read_local_extension_source(&dep.source)
        .with_context(|| format!("resolve source for extension '{}'", dep.id))?;
    extract_and_verify_bytes(&ext_ref.extension_id, &zip_bytes)
}

/// Cache/handle-aware entry point that resolves an `ext://<id>#component` ref,
/// acquiring the extension `.gtxpack` over the network when its declared source
/// is `store://` (and, guarded, `oci://`). `file://`/bare sources behave exactly
/// as [`resolve_ext_component`].
///
/// - `cache_dir` is the runtime cache dir (downloaded artifacts are cached under it).
/// - `offline` disables network fetches and forces cache-only resolution.
/// - `handle` is an optional current Tokio runtime handle; the store path uses
///   blocking `reqwest` on a dedicated thread, so it does not require one.
pub fn resolve_ext_component_with_dist(
    pack_dir: &Path,
    raw_ref: &str,
    cache_dir: &Path,
    offline: bool,
    handle: Option<&Handle>,
) -> Result<(Vec<u8>, String)> {
    let (ext_ref, dep) = lookup_ext_dependency(pack_dir, raw_ref)?;
    let zip_bytes = acquire_extension_bytes(&dep.source, cache_dir, offline, handle)
        .with_context(|| format!("acquire source for extension '{}'", dep.id))?;
    extract_and_verify_bytes(&ext_ref.extension_id, &zip_bytes)
}

/// Parse the `ext://` ref and locate the matching dependency in
/// `pack.extensions.json`.
fn lookup_ext_dependency(pack_dir: &Path, raw_ref: &str) -> Result<(ExtRef, ExtensionDependency)> {
    let ext_ref = parse_ext_ref(raw_ref)?;
    let extensions_path = pack_dir.join("pack.extensions.json");
    let extensions = read_extensions_file(&extensions_path)
        .with_context(|| format!("read pack.extensions.json from {}", pack_dir.display()))?;
    let dep = find_extension_dep(&extensions, &ext_ref.extension_id)
        .with_context(|| {
            format!(
                "ext:// component ref names extension '{}' not declared in pack.extensions.json",
                ext_ref.extension_id
            )
        })?
        .clone();
    Ok((ext_ref, dep))
}

fn find_extension_dep<'a>(
    file: &'a PackExtensionsFile,
    id: &str,
) -> Option<&'a ExtensionDependency> {
    file.extensions.iter().find(|dep| dep.id == id)
}

/// Read a `file://` or bare-path extension source into ZIP bytes.
///
/// Network schemes are rejected here; callers needing them must use
/// [`acquire_extension_bytes`].
fn read_local_extension_source(source: &ExtensionDependencySource) -> Result<Vec<u8>> {
    let raw = source.reference.as_str();
    if let Some(path) = local_path_for_source(raw) {
        return std::fs::read(&path)
            .with_context(|| format!("read extension .gtxpack at {}", path.display()));
    }
    bail!(
        "ext:// component resolver here only supports file:// or bare local extension sources, got '{raw}' (use the dist-aware resolver for store://)"
    );
}

/// Map a `file://` or bare-path ref to a filesystem path; `None` for any scheme.
fn local_path_for_source(raw: &str) -> Option<PathBuf> {
    if let Some(path_str) = raw.strip_prefix("file://") {
        return Some(PathBuf::from(path_str));
    }
    if !raw.contains("://") {
        return Some(PathBuf::from(raw));
    }
    None
}

/// Acquire the extension `.gtxpack` bytes for any supported source scheme.
///
/// - `file://`/bare → filesystem read (unchanged from Phase 1).
/// - `store://`     → store artifact endpoint GET, cached by archive sha256.
/// - `oci://`       → guarded/deferred (no producer yet) → clear error.
fn acquire_extension_bytes(
    source: &ExtensionDependencySource,
    cache_dir: &Path,
    offline: bool,
    _handle: Option<&Handle>,
) -> Result<Vec<u8>> {
    let raw = source.reference.as_str();
    if local_path_for_source(raw).is_some() {
        return read_local_extension_source(source);
    }
    if raw.starts_with("store://") {
        let store_ref = parse_store_ref(raw)?;
        return acquire_store_extension_bytes(&store_ref, cache_dir, offline);
    }
    if raw.starts_with("oci://") {
        // No producer publishes extensions to OCI yet; the DistClient is
        // wasm/pack media-type centric and would likely reject a `.gtxpack`.
        // Bail with a clear, actionable message rather than a fragile path.
        bail!(
            "oci:// extension acquisition not yet supported (no producer); declare the extension with a store:// or file:// source instead (got '{raw}')"
        );
    }
    bail!("unsupported extension source scheme for ext:// resolution: '{raw}'");
}

/// Acquire (and cache) the extension `.gtxpack` from the store artifact endpoint.
///
/// Reference resolution (the `store://<name>@<version>` parse and the
/// `GREENTIC_STORE_URL` base lookup) stays here; the HTTP transport, digest
/// verification, and on-disk caching are delegated to
/// [`greentic_distributor_client::store_ext::fetch_store_extension`] so that
/// packs and store extensions share a single fetch path.
fn acquire_store_extension_bytes(
    store_ref: &StoreRef,
    cache_dir: &Path,
    offline: bool,
) -> Result<Vec<u8>> {
    // Offline never hits the network, so the store base URL is not needed (and
    // we must not require the env var). Online, resolve it from the environment.
    let store_base = if offline {
        String::new()
    } else {
        std::env::var(STORE_URL_ENV).map_err(|_| {
            anyhow!(
                "{STORE_URL_ENV} is not set; it must name the store base URL to acquire store:// extension '{}@{}'",
                store_ref.name,
                store_ref.version
            )
        })?
    };

    greentic_distributor_client::store_ext::fetch_store_extension(
        &store_base,
        &store_ref.name,
        &store_ref.version,
        cache_dir,
        offline,
    )
    .with_context(|| {
        format!(
            "acquire store extension '{}@{}'",
            store_ref.name, store_ref.version
        )
    })
}

/// Build the store artifact endpoint URL for an extension `(name, version)`.
///
/// Shape: `{base}/api/v1/extensions/{name}/{version}/artifact` (public, no auth).
///
/// Deprecated forwarding shim: the canonical implementation now lives in
/// [`greentic_distributor_client::store_ext::store_artifact_url`], so packs and
/// store extensions share a single transport path.
#[deprecated(
    since = "1.1.1",
    note = "moved to greentic_distributor_client::store_ext; this shim forwards and will be removed in a future release"
)]
pub fn store_artifact_url(store_base: &str, name: &str, version: &str) -> String {
    greentic_distributor_client::store_ext::store_artifact_url(store_base, name, version)
}

/// Download (and cache) the extension `.gtxpack` from the store artifact endpoint,
/// verifying the advertised whole-archive digest.
///
/// Deprecated forwarding shim: the canonical implementation now lives in
/// [`greentic_distributor_client::store_ext::fetch_store_extension`]. This shim
/// forwards in online mode (`offline = false`) to preserve the original
/// behaviour and public API.
#[deprecated(
    since = "1.1.1",
    note = "moved to greentic_distributor_client::store_ext; this shim forwards and will be removed in a future release"
)]
pub fn download_store_artifact(
    store_base: &str,
    store_ref: &StoreRef,
    cache_dir: &Path,
) -> Result<Vec<u8>> {
    greentic_distributor_client::store_ext::fetch_store_extension(
        store_base,
        &store_ref.name,
        &store_ref.version,
        cache_dir,
        false,
    )
}

/// Read the `component.json` sidecar + the component wasm asset from `.gtxpack`
/// ZIP `zip_bytes`, verify the digest, and return (wasm_bytes, verified_digest).
pub fn extract_and_verify_bytes(extension_id: &str, zip_bytes: &[u8]) -> Result<(Vec<u8>, String)> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .with_context(|| format!("open extension .gtxpack ZIP for '{extension_id}'"))?;

    // Read the component.json sidecar.
    let sidecar: GtxpackComponentSidecar = {
        let mut entry = archive.by_name("component.json").map_err(|_| {
            anyhow!(
                "extension '{extension_id}' does not embed a runtime component: 'component.json' not found in .gtxpack"
            )
        })?;
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("read component.json for '{extension_id}'"))?;
        serde_json::from_slice(&buf)
            .with_context(|| format!("parse component.json for '{extension_id}'"))?
    };

    // Validate the component entry.
    let asset_path = sidecar.component.asset.as_str();
    if asset_path.trim().is_empty() {
        bail!(
            "extension '{extension_id}' does not embed a runtime component: 'component.json' component.asset is empty"
        );
    }

    // Read the wasm asset.
    let wasm_bytes = {
        let mut entry = archive.by_name(asset_path).map_err(|_| {
            anyhow!(
                "extension '{extension_id}' does not embed a runtime component: asset '{asset_path}' not found in .gtxpack"
            )
        })?;
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("read asset '{asset_path}' for '{extension_id}'"))?;
        buf
    };

    // Compute actual digest.
    let actual_digest = format!("sha256:{}", hex::encode(Sha256::digest(&wasm_bytes)));

    // Verify against the component.json advertised digest.
    let expected_digest = sidecar.component.digest.as_str();
    if actual_digest != expected_digest {
        bail!(
            "embedded component digest mismatch for extension '{extension_id}': component.json advertises '{expected_digest}' but extracted wasm hashes to '{actual_digest}'"
        );
    }

    Ok((wasm_bytes, actual_digest))
}

/// Read the `describe.json` sidecar from a `.gtxpack` ZIP.
///
/// Returns the raw `describe.json` bytes so callers can parse only the fields
/// they need (e.g. via [`crate::setup_gen::extract_tool_secret_requirements`]).
pub fn read_describe_from_gtxpack(extension_id: &str, zip_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .with_context(|| format!("open extension .gtxpack ZIP for '{extension_id}'"))?;
    let mut file = archive
        .by_name("describe.json")
        .with_context(|| format!("extension '{extension_id}' .gtxpack has no describe.json"))?;
    let mut body = Vec::new();
    file.read_to_end(&mut body)
        .with_context(|| format!("read describe.json from '{extension_id}' .gtxpack"))?;
    Ok(body)
}

/// For each tool extension used by the agents, acquire its `.gtxpack`, read
/// `describe.json`, and extract the secret requirements of the used tools.
///
/// Returns a map keyed by extension id. Errors (and propagates via `?`) when a
/// declared tool extension is not found in `pack.extensions.json` or cannot be
/// acquired — no silent skips.
///
/// `pack_dir` must contain `pack.extensions.json`. `cache_dir` and `offline`
/// are threaded through to [`acquire_extension_bytes`] for `store://` sources.
pub fn resolve_agent_tool_requirements(
    pack_dir: &Path,
    agents: &std::collections::BTreeMap<String, serde_json::Value>,
    cache_dir: &Path,
    offline: bool,
) -> Result<std::collections::BTreeMap<String, Vec<crate::setup_gen::ToolSecretReq>>> {
    use std::collections::{BTreeMap, BTreeSet};

    // Collect extension_id -> set(tool_name) actually used across all agents.
    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (agent_name, agent) in agents {
        let Some(tools) = agent.get("tools").and_then(|t| t.as_array()) else {
            continue;
        };
        for tool in tools {
            let (Some(ext_id), Some(tool_name)) = (
                tool.get("extension_id").and_then(|e| e.as_str()),
                tool.get("tool_name").and_then(|n| n.as_str()),
            ) else {
                tracing::warn!(
                    agent = %agent_name,
                    "skipping malformed agent tool entry: missing extension_id or tool_name"
                );
                continue;
            };
            used.entry(ext_id.to_string())
                .or_default()
                .insert(tool_name.to_string());
        }
    }

    let mut out = BTreeMap::new();
    for (ext_id, tool_names) in &used {
        // Reuse lookup_ext_dependency — it requires the #component fragment for
        // the parse_ext_ref validator even though we only need describe.json.
        let raw_ref = format!("ext://{ext_id}#component");
        let (_ext_ref, dep) = lookup_ext_dependency(pack_dir, &raw_ref).with_context(|| {
            format!("resolve tool extension '{ext_id}' for credential form generation")
        })?;
        let zip_bytes = acquire_extension_bytes(&dep.source, cache_dir, offline, None)
            .with_context(|| format!("acquire .gtxpack for tool extension '{ext_id}'"))?;
        let describe_bytes = read_describe_from_gtxpack(ext_id, &zip_bytes)?;
        let names: Vec<String> = tool_names.iter().cloned().collect();
        let secret_requirements =
            crate::setup_gen::extract_tool_secret_requirements(&describe_bytes, &names)?;
        out.insert(ext_id.clone(), secret_requirements);
    }
    Ok(out)
}

#[cfg(test)]
mod describe_tests {
    use super::*;
    use std::io::Write;

    fn gtxpack_with_describe(describe: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("describe.json", zip::write::FileOptions::<()>::default())
                .unwrap();
            zip.write_all(describe.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn reads_describe_json_entry_from_gtxpack() {
        let bytes = gtxpack_with_describe(r#"{"contributions":{"tools":[]}}"#);
        let body = read_describe_from_gtxpack("greentic.tavily", &bytes).unwrap();
        assert!(String::from_utf8_lossy(&body).contains("contributions"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ext_ref_happy() {
        let r = parse_ext_ref("ext://greentic.http#component").expect("valid ref");
        assert_eq!(r.extension_id, "greentic.http");
    }

    #[test]
    fn parse_ext_ref_wrong_scheme() {
        let err = parse_ext_ref("oci://foo#component").expect_err("wrong scheme");
        assert!(err.to_string().contains("ext://"));
    }

    #[test]
    fn parse_ext_ref_no_fragment() {
        let err = parse_ext_ref("ext://greentic.http").expect_err("no fragment");
        assert!(err.to_string().contains("#component"));
    }

    #[test]
    fn parse_ext_ref_wrong_fragment() {
        let err = parse_ext_ref("ext://greentic.http#other").expect_err("wrong fragment");
        assert!(err.to_string().contains("#component"));
    }

    #[test]
    fn parse_ext_ref_empty_id() {
        let err = parse_ext_ref("ext://#component").expect_err("empty id");
        assert!(err.to_string().contains("must not be empty"));
    }
}
