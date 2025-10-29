use anyhow::{Context, Result};
use greentic_flow::{loader, resolver, FlowIr};
use greentic_types::PackSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FlowAsset {
    pub id: String,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub raw: String,
    pub sha256: String,
    pub ir: FlowIr,
    pub parameters: Value,
}

pub fn load_flows(pack_dir: &Path, spec: &PackSpec) -> Result<Vec<FlowAsset>> {
    let mut flows = Vec::new();
    let mut seen_ids = BTreeSet::new();

    for entry in &spec.flow_files {
        let relative_path = PathBuf::from(entry);
        let absolute_path = pack_dir.join(&relative_path);

        let raw = fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read flow {}", absolute_path.display()))?;

        let flow_id = derive_flow_id(&relative_path);
        if !seen_ids.insert(flow_id.clone()) {
            anyhow::bail!("duplicate flow id detected: {}", flow_id);
        }

        let document = loader::load_ygtc_from_str(&flow_id, &raw)
            .with_context(|| format!("failed to parse flow {}", relative_path.display()))?;
        let ir = document
            .to_ir()
            .with_context(|| format!("failed to lower flow {}", flow_id))?;
        let resolved = resolver::resolve_parameters(&ir)
            .with_context(|| format!("failed to resolve parameters for {}", flow_id))?;

        let digest = Sha256::digest(raw.as_bytes());
        let sha256 = hex::encode(digest);

        flows.push(FlowAsset {
            id: flow_id,
            relative_path,
            absolute_path,
            raw,
            sha256,
            ir,
            parameters: resolved.parameters,
        });
    }

    flows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(flows)
}

fn derive_flow_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace(std::path::MAIN_SEPARATOR, "/"))
        .unwrap_or_else(|| "flow".to_string())
}
