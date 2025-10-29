use crate::flows::FlowAsset;
use crate::templates::TemplateAsset;
use anyhow::{Context, Result};
use greentic_types::PackSpec;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct SpecBundle {
    pub spec: PackSpec,
    pub source: PathBuf,
}

pub fn load_spec(pack_dir: &Path) -> Result<SpecBundle> {
    let manifest_path = pack_dir.join("pack.yaml");
    let contents = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let spec: PackSpec = serde_yaml_bw::from_str(&contents)
        .with_context(|| format!("{} is not a valid PackSpec", manifest_path.display()))?;
    spec.validate()
        .map_err(|msg| anyhow::anyhow!("{}: {}", manifest_path.display(), msg))?;

    Ok(SpecBundle {
        spec,
        source: manifest_path,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub pack_id: String,
    pub version: String,
    pub created_at: String,
    pub flows: Vec<FlowEntry>,
    pub templates: Vec<BlobEntry>,
    pub imports_required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEntry {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobEntry {
    pub logical_path: String,
    pub sha256: String,
    pub size: u64,
}

pub fn build_manifest(
    bundle: &SpecBundle,
    flows: &[FlowAsset],
    templates: &[TemplateAsset],
) -> PackManifest {
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    let flow_entries = flows
        .iter()
        .map(|flow| FlowEntry {
            id: flow.id.clone(),
            path: flow.relative_path.to_string_lossy().to_string(),
            sha256: flow.sha256.clone(),
            size: flow.raw.as_bytes().len() as u64,
            parameters: flow.parameters.clone(),
        })
        .collect();

    let template_entries = templates
        .iter()
        .map(|blob| BlobEntry {
            logical_path: blob.logical_path.clone(),
            sha256: blob.sha256.clone(),
            size: blob.size,
        })
        .collect();

    PackManifest {
        pack_id: bundle.spec.id.clone(),
        version: bundle.spec.version.clone(),
        created_at,
        flows: flow_entries,
        templates: template_entries,
        imports_required: bundle.spec.imports_required.clone(),
    }
}

pub fn encode_manifest(manifest: &PackManifest) -> Result<Vec<u8>> {
    Ok(serde_cbor::to_vec(manifest)?)
}
