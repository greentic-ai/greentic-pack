use crate::manifest::PackSpec;
use anyhow::{Context, Result};
use greentic_flow::flow_bundle::{FlowBundle, load_and_validate_bundle_with_ir};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FlowAsset {
    pub bundle: FlowBundle,
    #[allow(dead_code)]
    pub relative_path: PathBuf,
    pub raw: String,
    pub sha256: String,
}

const FLOW_SCHEMA_JSON: &str = include_str!("../schemas/ygtc.flow.schema.json");

pub fn load_flows(pack_dir: &Path, spec: &PackSpec) -> Result<Vec<FlowAsset>> {
    let mut flows = Vec::new();
    let mut seen_ids = BTreeSet::new();
    ensure_flow_schema(pack_dir)?;

    for entry in &spec.flow_files {
        let relative_path = PathBuf::from(entry);
        let absolute_path = pack_dir.join(&relative_path);

        let raw = fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read flow {}", absolute_path.display()))?;

        let flow_id = derive_flow_id(&relative_path);
        let (bundle, _ir) = load_and_validate_bundle_with_ir(&raw, Some(&absolute_path))
            .with_context(|| format!("failed to parse flow {}", relative_path.display()))?;
        let mut bundle = bundle;
        let flow_identifier = if bundle.id.trim().is_empty() {
            flow_id.clone()
        } else {
            bundle.id.clone()
        };
        if !seen_ids.insert(flow_identifier.clone()) {
            anyhow::bail!("duplicate flow id detected: {}", flow_identifier);
        }
        bundle.id = flow_identifier.clone();
        let digest = Sha256::digest(raw.as_bytes());
        let sha256 = hex::encode(digest);

        flows.push(FlowAsset {
            bundle,
            relative_path,
            raw,
            sha256,
        });
    }

    flows.sort_by(|a, b| a.bundle.id.cmp(&b.bundle.id));
    Ok(flows)
}

fn ensure_flow_schema(pack_dir: &Path) -> Result<PathBuf> {
    let schema_dir = pack_dir.join(".packc").join("schemas");
    let schema_path = schema_dir.join("ygtc.flow.schema.json");
    if !schema_path.exists() {
        fs::create_dir_all(&schema_dir)
            .with_context(|| format!("failed to create schema dir {}", schema_dir.display()))?;
        fs::write(&schema_path, FLOW_SCHEMA_JSON)
            .with_context(|| format!("failed to write schema {}", schema_path.display()))?;
    }
    Ok(schema_path)
}

fn derive_flow_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace(std::path::MAIN_SEPARATOR, "/"))
        .unwrap_or_else(|| "flow".to_string())
}
