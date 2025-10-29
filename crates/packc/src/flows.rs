use crate::manifest::PackSpec;
use anyhow::{Context, Result};
use greentic_flow::{loader, resolve, to_ir};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FlowAsset {
    pub id: String,
    pub relative_path: PathBuf,
    pub raw: String,
    pub sha256: String,
    pub parameters: Value,
}

pub fn load_flows(pack_dir: &Path, spec: &PackSpec) -> Result<Vec<FlowAsset>> {
    let mut flows = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let schema_path = flow_schema_path();

    for entry in &spec.flow_files {
        let relative_path = PathBuf::from(entry);
        let absolute_path = pack_dir.join(&relative_path);

        let raw = fs::read_to_string(&absolute_path)
            .with_context(|| format!("failed to read flow {}", absolute_path.display()))?;

        let flow_id = derive_flow_id(&relative_path);
        if !seen_ids.insert(flow_id.clone()) {
            anyhow::bail!("duplicate flow id detected: {}", flow_id);
        }

        let document = loader::load_ygtc_from_str(&raw, &schema_path)
            .with_context(|| format!("failed to parse flow {}", relative_path.display()))?;
        let ir = to_ir(document).with_context(|| format!("failed to lower flow {}", flow_id))?;
        let resolved_parameters = resolve::resolve_parameters(
            &ir.parameters,
            &ir.parameters,
            &format!("flows.{}", flow_id),
        )
        .with_context(|| format!("failed to resolve parameters for {}", flow_id))?;
        let flow_identifier = if ir.id.trim().is_empty() {
            flow_id.clone()
        } else {
            ir.id.clone()
        };

        let digest = Sha256::digest(raw.as_bytes());
        let sha256 = hex::encode(digest);

        flows.push(FlowAsset {
            id: flow_identifier,
            relative_path,
            raw,
            sha256,
            parameters: resolved_parameters,
        });
    }

    flows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(flows)
}

fn flow_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schemas")
        .join("ygtc.flow.schema.json")
}

fn derive_flow_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace(std::path::MAIN_SEPARATOR, "/"))
        .unwrap_or_else(|| "flow".to_string())
}
