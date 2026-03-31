#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use anyhow::{Context, anyhow};
use sha2::{Digest, Sha256};
use std::fs;

/// Resolve request for a component reference.
#[derive(Clone, Debug)]
pub struct ResolveReq {
    pub component_id: String,
    pub reference: String,
    pub expected_digest: String,
    pub abi_version: String,
    pub world: Option<String>,
    pub component_version: Option<String>,
}

/// Resolved component data + minimal typed metadata.
#[derive(Clone, Debug)]
pub struct ResolvedComponent {
    pub bytes: Vec<u8>,
    pub resolved_digest: String,
    pub component_id: String,
    pub abi_version: String,
    pub world: Option<String>,
    pub component_version: Option<String>,
    pub source_path: Option<PathBuf>,
}

/// Host-side resolver for component references.
pub trait ComponentResolver {
    fn resolve(&self, req: ResolveReq) -> Result<ResolvedComponent>;
}

/// Fixture-backed resolver for offline tests.
#[derive(Clone, Debug)]
pub struct FixtureResolver {
    root: PathBuf,
}

impl FixtureResolver {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn resolve_component_path(&self, component_id: &str) -> PathBuf {
        self.root
            .join("components")
            .join(component_id)
            .join("component.wasm")
    }
}

impl ComponentResolver for FixtureResolver {
    fn resolve(&self, req: ResolveReq) -> Result<ResolvedComponent> {
        let component_id = req.component_id.clone();
        let path = self.resolve_component_path(&component_id);
        if !path.exists() {
            return Err(anyhow!(
                "fixture component {} not found at {}",
                component_id,
                path.display()
            ));
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        if !req.expected_digest.is_empty() && req.expected_digest != digest {
            return Err(anyhow!(
                "fixture component digest mismatch for {} (expected {}, got {})",
                component_id,
                req.expected_digest,
                digest
            ));
        }
        Ok(ResolvedComponent {
            bytes,
            resolved_digest: digest,
            component_id,
            abi_version: req.abi_version,
            world: req.world,
            component_version: req.component_version,
            source_path: Some(path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn req(expected_digest: String) -> ResolveReq {
        ResolveReq {
            component_id: "demo".to_string(),
            reference: "fixture://demo".to_string(),
            expected_digest,
            abi_version: "0.1.0".to_string(),
            world: Some("demo-world".to_string()),
            component_version: Some("1.2.3".to_string()),
        }
    }

    #[test]
    fn fixture_resolver_reads_component_and_preserves_metadata() {
        let root = tempdir().expect("tempdir");
        let component_dir = root.path().join("components/demo");
        std::fs::create_dir_all(&component_dir).expect("create fixture dir");
        let bytes = b"wasm bytes";
        std::fs::write(component_dir.join("component.wasm"), bytes).expect("write wasm");
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));

        let resolved = FixtureResolver::new(root.path().to_path_buf())
            .resolve(req(digest.clone()))
            .expect("fixture component should resolve");

        assert_eq!(resolved.bytes, bytes);
        assert_eq!(resolved.resolved_digest, digest);
        assert_eq!(resolved.world.as_deref(), Some("demo-world"));
        assert_eq!(resolved.component_version.as_deref(), Some("1.2.3"));
        assert_eq!(
            resolved.source_path.as_deref(),
            Some(component_dir.join("component.wasm").as_path())
        );
    }

    #[test]
    fn fixture_resolver_rejects_digest_mismatches() {
        let root = tempdir().expect("tempdir");
        let component_dir = root.path().join("components/demo");
        std::fs::create_dir_all(&component_dir).expect("create fixture dir");
        std::fs::write(component_dir.join("component.wasm"), b"bytes").expect("write wasm");

        let err = FixtureResolver::new(root.path().to_path_buf())
            .resolve(req("sha256:not-the-real-digest".to_string()))
            .expect_err("digest mismatch should fail");

        assert!(err.to_string().contains("digest mismatch"));
    }

    #[test]
    fn fixture_resolver_reports_missing_components() {
        let root = tempdir().expect("tempdir");

        let err = FixtureResolver::new(root.path().to_path_buf())
            .resolve(req(String::new()))
            .expect_err("missing fixture should fail");

        assert!(err.to_string().contains("fixture component demo not found"));
    }
}
