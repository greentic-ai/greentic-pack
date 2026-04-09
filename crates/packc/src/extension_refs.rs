#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackExtensionsFile {
    pub version: u32,
    #[serde(default)]
    pub extensions: Vec<ExtensionDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDependency {
    pub id: String,
    pub role: String,
    pub source: ExtensionDependencySource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDependencySource {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub allow_tags: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackExtensionsLockFile {
    pub version: u32,
    #[serde(default)]
    pub extensions: Vec<LockedExtensionDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedExtensionDependency {
    pub id: String,
    pub role: String,
    pub source_ref: String,
    pub resolved_ref: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

impl PackExtensionsFile {
    pub fn new(extensions: Vec<ExtensionDependency>) -> Self {
        Self {
            version: 1,
            extensions,
        }
    }
}

impl PackExtensionsLockFile {
    pub fn new(extensions: Vec<LockedExtensionDependency>) -> Self {
        Self {
            version: 1,
            extensions,
        }
    }
}

pub fn read_extensions_file(path: &Path) -> Result<PackExtensionsFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let file: PackExtensionsFile =
        serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))?;
    validate_extensions_file(&file)?;
    Ok(file)
}

pub fn write_extensions_file(path: &Path, file: &PackExtensionsFile) -> Result<()> {
    validate_extensions_file(file)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(file).context("serialize pack.extensions.json")?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn read_extensions_lock_file(path: &Path) -> Result<PackExtensionsLockFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let file: PackExtensionsLockFile =
        serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))?;
    validate_extensions_lock_file(&file)?;
    Ok(file)
}

pub fn write_extensions_lock_file(path: &Path, file: &PackExtensionsLockFile) -> Result<()> {
    validate_extensions_lock_file(file)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(file).context("serialize pack.extensions.lock.json")?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn validate_extensions_file(file: &PackExtensionsFile) -> Result<()> {
    if file.version != 1 {
        bail!("pack.extensions.json version must be 1");
    }
    let mut seen = BTreeMap::new();
    for extension in &file.extensions {
        if extension.id.trim().is_empty() {
            bail!("pack.extensions.json extension id must not be empty");
        }
        if extension.role.trim().is_empty() {
            bail!(
                "pack.extensions.json extension `{}` role must not be empty",
                extension.id
            );
        }
        if extension.source.kind.trim().is_empty() {
            bail!(
                "pack.extensions.json extension `{}` source.kind must not be empty",
                extension.id
            );
        }
        if extension.source.reference.trim().is_empty() {
            bail!(
                "pack.extensions.json extension `{}` source.ref must not be empty",
                extension.id
            );
        }
        validate_reference_kind(
            &extension.source.kind,
            &extension.source.reference,
            extension.source.allow_tags,
        )?;
        if let Some(previous_role) = seen.insert(extension.id.as_str(), extension.role.as_str()) {
            bail!(
                "pack.extensions.json extension `{}` is duplicated (roles `{previous_role}` and `{}`)",
                extension.id,
                extension.role
            );
        }
    }
    Ok(())
}

pub fn validate_extensions_lock_file(file: &PackExtensionsLockFile) -> Result<()> {
    if file.version != 1 {
        bail!("pack.extensions.lock.json version must be 1");
    }
    let mut seen = BTreeMap::new();
    for extension in &file.extensions {
        if extension.id.trim().is_empty() {
            bail!("pack.extensions.lock.json extension id must not be empty");
        }
        if extension.role.trim().is_empty() {
            bail!(
                "pack.extensions.lock.json extension `{}` role must not be empty",
                extension.id
            );
        }
        if extension.source_ref.trim().is_empty() {
            bail!(
                "pack.extensions.lock.json extension `{}` source_ref must not be empty",
                extension.id
            );
        }
        if extension.resolved_ref.trim().is_empty() {
            bail!(
                "pack.extensions.lock.json extension `{}` resolved_ref must not be empty",
                extension.id
            );
        }
        if !extension.digest.starts_with("sha256:") {
            bail!(
                "pack.extensions.lock.json extension `{}` digest must start with sha256:",
                extension.id
            );
        }
        if let Some(previous_role) = seen.insert(extension.id.as_str(), extension.role.as_str()) {
            bail!(
                "pack.extensions.lock.json extension `{}` is duplicated (roles `{previous_role}` and `{}`)",
                extension.id,
                extension.role
            );
        }
    }
    Ok(())
}

pub fn validate_extensions_lock_alignment(
    source: &PackExtensionsFile,
    lock: &PackExtensionsLockFile,
) -> Result<()> {
    let source_by_id = source
        .extensions
        .iter()
        .map(|extension| (extension.id.as_str(), extension))
        .collect::<BTreeMap<_, _>>();
    let lock_by_id = lock
        .extensions
        .iter()
        .map(|extension| (extension.id.as_str(), extension))
        .collect::<BTreeMap<_, _>>();

    for (id, source_extension) in &source_by_id {
        let Some(lock_extension) = lock_by_id.get(id) else {
            bail!(
                "pack.extensions.lock.json is missing extension `{id}` present in pack.extensions.json"
            );
        };
        if lock_extension.role != source_extension.role {
            bail!(
                "pack.extensions.lock.json extension `{id}` role `{}` does not match pack.extensions.json role `{}`",
                lock_extension.role,
                source_extension.role
            );
        }
        if lock_extension.source_ref != source_extension.source.reference {
            bail!(
                "pack.extensions.lock.json extension `{id}` source_ref `{}` does not match pack.extensions.json ref `{}`",
                lock_extension.source_ref,
                source_extension.source.reference
            );
        }
    }

    for id in lock_by_id.keys() {
        if !source_by_id.contains_key(id) {
            bail!(
                "pack.extensions.lock.json contains extension `{id}` that is not present in pack.extensions.json"
            );
        }
    }

    Ok(())
}

pub fn default_extensions_file_path(pack_dir: &Path) -> PathBuf {
    pack_dir.join("pack.extensions.json")
}

pub fn default_extensions_lock_file_path(pack_dir: &Path) -> PathBuf {
    pack_dir.join("pack.extensions.lock.json")
}

pub fn infer_reference_kind(reference: &str) -> Result<String> {
    let normalized = reference.trim();
    if normalized.starts_with("oci://") {
        return Ok("oci".to_string());
    }
    if normalized.starts_with("file://") {
        return Ok("file".to_string());
    }
    if normalized.starts_with("http://") || normalized.starts_with("https://") {
        return Ok("http".to_string());
    }
    if normalized.starts_with("repo://") {
        return Ok("repo".to_string());
    }
    if normalized.starts_with("store://") {
        return Ok("store".to_string());
    }
    bail!("unsupported extension source ref scheme: {reference}");
}

pub fn pin_reference(reference: &str, digest: &str) -> String {
    if let Some(rest) = reference.strip_prefix("oci://") {
        return format!("oci://{}@{}", strip_tag_or_digest(rest), digest);
    }
    if let Some(rest) = reference.strip_prefix("repo://") {
        return format!("repo://{}@{}", strip_tag_or_digest(rest), digest);
    }
    if let Some(rest) = reference.strip_prefix("store://") {
        return format!("store://{}@{}", strip_tag_or_digest(rest), digest);
    }
    reference.to_string()
}

fn strip_tag_or_digest(reference: &str) -> &str {
    if let Some((repo, _)) = reference.rsplit_once('@') {
        return repo;
    }
    let last_slash = reference.rfind('/');
    let last_colon = reference.rfind(':');
    if let (Some(slash), Some(colon)) = (last_slash, last_colon)
        && colon > slash
    {
        return &reference[..colon];
    }
    reference
}

fn validate_reference_kind(kind: &str, reference: &str, allow_tags: bool) -> Result<()> {
    let expected_kind = infer_reference_kind(reference)?;
    if kind != expected_kind {
        bail!(
            "pack.extensions.json source.kind `{kind}` does not match ref scheme `{expected_kind}`"
        );
    }
    if matches!(kind, "oci" | "repo" | "store") && !allow_tags && !reference.contains("@sha256:") {
        bail!(
            "pack.extensions.json ref `{reference}` must be digest-pinned or set allow_tags=true"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_reference_rewrites_oci_tag_to_digest() {
        let pinned = pin_reference("oci://ghcr.io/acme/demo:latest", "sha256:abcd");
        assert_eq!(pinned, "oci://ghcr.io/acme/demo@sha256:abcd");
    }

    #[test]
    fn validate_extensions_file_rejects_unpinned_oci_without_allow_tags() {
        let file = PackExtensionsFile::new(vec![ExtensionDependency {
            id: "greentic.deployer.v1".to_string(),
            role: "deployer".to_string(),
            source: ExtensionDependencySource {
                kind: "oci".to_string(),
                reference: "oci://ghcr.io/acme/demo:latest".to_string(),
                allow_tags: false,
            },
        }]);
        let err = validate_extensions_file(&file).expect_err("should reject tag ref");
        assert!(err.to_string().contains("must be digest-pinned"));
    }

    #[test]
    fn validate_extensions_lock_alignment_rejects_source_ref_drift() {
        let source = PackExtensionsFile::new(vec![ExtensionDependency {
            id: "greentic.deployer.v1".to_string(),
            role: "deployer".to_string(),
            source: ExtensionDependencySource {
                kind: "file".to_string(),
                reference: "file:///tmp/a.json".to_string(),
                allow_tags: false,
            },
        }]);
        let lock = PackExtensionsLockFile::new(vec![LockedExtensionDependency {
            id: "greentic.deployer.v1".to_string(),
            role: "deployer".to_string(),
            source_ref: "file:///tmp/b.json".to_string(),
            resolved_ref: "file:///tmp/b.json".to_string(),
            digest: "sha256:abcd".to_string(),
            media_type: None,
            size_bytes: None,
        }]);

        let err = validate_extensions_lock_alignment(&source, &lock).expect_err("should reject");
        assert!(
            err.to_string()
                .contains("does not match pack.extensions.json ref")
        );
    }
}
