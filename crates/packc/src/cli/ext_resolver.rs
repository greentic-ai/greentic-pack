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

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::extension_refs::{
    ExtensionDependency, ExtensionDependencySource, PackExtensionsFile, read_extensions_file,
};

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

/// Resolve an `ext://<id>#component` reference by extracting the wasm from the extension's
/// `.gtxpack` and verifying the digest against the `component.json` sidecar.
///
/// `pack_dir` is the directory containing `pack.extensions.json`.
///
/// Returns the raw wasm bytes and the verified digest string (`sha256:<hex>`).
pub fn resolve_ext_component(pack_dir: &Path, raw_ref: &str) -> Result<(Vec<u8>, String)> {
    let ext_ref = parse_ext_ref(raw_ref)?;
    let extensions_path = pack_dir.join("pack.extensions.json");
    let extensions = read_extensions_file(&extensions_path)
        .with_context(|| format!("read pack.extensions.json from {}", pack_dir.display()))?;

    let dep = find_extension_dep(&extensions, &ext_ref.extension_id).with_context(|| {
        format!(
            "ext:// component ref names extension '{}' not declared in pack.extensions.json",
            ext_ref.extension_id
        )
    })?;

    let gtxpack_path = resolve_extension_source(&dep.source)
        .with_context(|| format!("resolve source for extension '{}'", dep.id))?;

    extract_and_verify(&ext_ref.extension_id, &gtxpack_path)
}

fn find_extension_dep<'a>(
    file: &'a PackExtensionsFile,
    id: &str,
) -> Option<&'a ExtensionDependency> {
    file.extensions.iter().find(|dep| dep.id == id)
}

/// Convert an extension source to a local filesystem path.
///
/// Phase 1 supports `file://` paths and bare local paths. `oci://`, `store://`, and
/// `repo://` are unsupported until a network-acquisition path is wired in (Phase 3+).
fn resolve_extension_source(source: &ExtensionDependencySource) -> Result<PathBuf> {
    let raw = source.reference.as_str();
    if let Some(path_str) = raw.strip_prefix("file://") {
        return Ok(PathBuf::from(path_str));
    }
    // Bare local path (no scheme) — treat as filesystem path directly.
    if !raw.contains("://") {
        return Ok(PathBuf::from(raw));
    }
    bail!(
        "ext:// component resolver Phase 1 only supports file:// or bare local extension sources, got '{raw}'"
    );
}

/// Open the `.gtxpack` ZIP at `gtxpack_path`, read the `component.json` sidecar + the
/// component wasm asset, verify the digest, and return (wasm_bytes, verified_digest).
fn extract_and_verify(extension_id: &str, gtxpack_path: &Path) -> Result<(Vec<u8>, String)> {
    let zip_bytes = std::fs::read(gtxpack_path)
        .with_context(|| format!("read extension .gtxpack at {}", gtxpack_path.display()))?;
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .with_context(|| format!("open ZIP archive {}", gtxpack_path.display()))?;

    // Read the component.json sidecar.
    let sidecar: GtxpackComponentSidecar = {
        let mut entry = archive.by_name("component.json").map_err(|_| {
            anyhow::anyhow!(
                "extension '{}' does not embed a runtime component: 'component.json' not found in {}",
                extension_id,
                gtxpack_path.display()
            )
        })?;
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("read component.json from {}", gtxpack_path.display()))?;
        serde_json::from_slice(&buf)
            .with_context(|| format!("parse component.json from {}", gtxpack_path.display()))?
    };

    // Validate the component entry.
    let asset_path = sidecar.component.asset.as_str();
    if asset_path.trim().is_empty() {
        bail!(
            "extension '{}' does not embed a runtime component: 'component.json' component.asset is empty",
            extension_id
        );
    }

    // Read the wasm asset.
    let wasm_bytes = {
        let mut entry = archive.by_name(asset_path).map_err(|_| {
            anyhow::anyhow!(
                "extension '{}' does not embed a runtime component: asset '{}' not found in {}",
                extension_id,
                asset_path,
                gtxpack_path.display()
            )
        })?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).with_context(|| {
            format!(
                "read asset '{}' from {}",
                asset_path,
                gtxpack_path.display()
            )
        })?;
        buf
    };

    // Compute actual digest.
    let actual_digest = format!("sha256:{}", hex::encode(Sha256::digest(&wasm_bytes)));

    // Verify against the component.json advertised digest.
    let expected_digest = sidecar.component.digest.as_str();
    if actual_digest != expected_digest {
        bail!(
            "embedded component digest mismatch for extension '{}': component.json advertises '{}' but extracted wasm hashes to '{}'",
            extension_id,
            expected_digest,
            actual_digest
        );
    }

    Ok((wasm_bytes, actual_digest))
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
