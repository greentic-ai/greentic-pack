use crate::flows::FlowAsset;
use crate::templates::TemplateAsset;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, Deserialize)]
pub struct PackSpec {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub flow_files: Vec<String>,
    #[serde(default)]
    pub template_dirs: Vec<String>,
    #[serde(default)]
    pub imports_required: Vec<String>,
}

impl PackSpec {
    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("pack id must not be empty");
        }
        if self.version.trim().is_empty() {
            anyhow::bail!("pack version must not be empty");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SpecBundle {
    pub spec: PackSpec,
    #[allow(dead_code)]
    pub source: PathBuf,
}

pub fn load_spec(pack_dir: &Path) -> Result<SpecBundle> {
    let manifest_path = pack_dir.join("pack.yaml");
    let contents = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let spec: PackSpec = serde_yaml_bw::from_str(&contents)
        .with_context(|| format!("{} is not a valid PackSpec", manifest_path.display()))?;
    spec.validate()
        .with_context(|| format!("invalid pack spec {}", manifest_path.display()))?;

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
    #[serde(rename = "type")]
    pub flow_type: String,
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
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
            flow_type: flow.flow_type.clone(),
            start: flow.start.clone(),
            source: Some(flow.relative_path.to_string_lossy().to_string()),
            sha256: Some(flow.sha256.clone()),
            size: Some(flow.raw.len() as u64),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{flows, templates};

    fn demo_pack_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples/weather-demo")
    }

    #[test]
    fn weather_demo_manifest_includes_mcp_exec_flow() {
        let pack_dir = demo_pack_dir();
        let spec_bundle = load_spec(&pack_dir).expect("spec loads");
        assert_eq!(spec_bundle.spec.id, "greentic.weather.demo");

        let flows = flows::load_flows(&pack_dir, &spec_bundle.spec).expect("flows load");
        assert_eq!(flows.len(), 1);
        assert!(
            flows[0].raw.contains("mcp.exec"),
            "flow should reference mcp.exec node"
        );

        let templates =
            templates::collect_templates(&pack_dir, &spec_bundle.spec).expect("templates load");
        assert_eq!(templates.len(), 1);

        let manifest = build_manifest(&spec_bundle, &flows, &templates);
        assert_eq!(manifest.flows[0].id, "weather_bot");
        assert_eq!(manifest.flows[0].flow_type, "messaging");
        assert_eq!(manifest.flows[0].start.as_deref(), Some("collect_location"));
        assert_eq!(
            manifest.flows[0].source.as_deref(),
            Some("flows/weather_bot.ygtc")
        );
        assert_eq!(
            manifest.templates[0].logical_path,
            "templates/weather_now.hbs"
        );
        assert_eq!(manifest.imports_required.len(), 2);

        let encoded = encode_manifest(&manifest).expect("manifest encodes");
        assert!(!encoded.is_empty(), "CBOR output should not be empty");
    }
}
