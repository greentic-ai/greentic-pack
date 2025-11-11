use crate::flows::FlowAsset;
use crate::templates::TemplateAsset;
use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use greentic_types::{Signature as SharedSignature, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use toml::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackSignature {
    pub alg: String,
    pub key_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub digest: String,
    pub sig: String,
}

impl PackSignature {
    pub const ED25519: &'static str = "ed25519";

    /// Converts this signature into the shared `greentic-types` representation.
    pub fn to_shared(&self) -> Result<SharedSignature> {
        if self.alg.to_ascii_lowercase() != Self::ED25519 {
            anyhow::bail!("unsupported algorithm {}", self.alg);
        }

        let raw = URL_SAFE_NO_PAD
            .decode(self.sig.as_bytes())
            .map_err(|err| anyhow!("invalid signature encoding: {err}"))?;

        Ok(SharedSignature::new(
            self.key_id.clone(),
            SignatureAlgorithm::Ed25519,
            raw,
        ))
    }
}

pub fn find_manifest_path(pack_dir: &Path) -> Option<PathBuf> {
    MANIFEST_CANDIDATES
        .iter()
        .map(|name| pack_dir.join(name))
        .find(|candidate| candidate.exists())
}

pub fn manifest_path(pack_dir: &Path) -> Result<PathBuf> {
    find_manifest_path(pack_dir)
        .ok_or_else(|| anyhow!("pack manifest not found in {}", pack_dir.display()))
}

pub fn is_pack_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            MANIFEST_CANDIDATES
                .iter()
                .any(|candidate| candidate == &name)
        })
        .unwrap_or(false)
}

pub fn read_manifest_without_signature(path: &Path) -> Result<Vec<u8>> {
    let mut doc = load_manifest_value(path)?;
    strip_signature(&mut doc);
    let serialized =
        toml::to_string(&doc).map_err(|err| anyhow!("failed to serialise manifest: {err}"))?;
    Ok(serialized.into_bytes())
}

pub fn read_signature(pack_dir: &Path) -> Result<Option<PackSignature>> {
    let Some(path) = find_manifest_path(pack_dir) else {
        return Ok(None);
    };

    let doc = load_manifest_value(&path)?;
    signature_from_doc(&doc)
}

pub fn write_signature(
    pack_dir: &Path,
    signature: &PackSignature,
    out_path: Option<&Path>,
) -> Result<()> {
    let manifest_path = manifest_path(pack_dir)?;
    let mut doc = load_manifest_value(&manifest_path)?;
    set_signature(&mut doc, signature)?;

    let target_path = out_path.unwrap_or(&manifest_path);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|err| anyhow!("failed to serialise manifest: {err}"))?;
    fs::write(target_path, serialized.as_bytes())
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(())
}

fn load_manifest_value(path: &Path) -> Result<Value> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let table: toml::value::Table =
        toml::from_str(&source).with_context(|| format!("{} is not valid TOML", path.display()))?;
    Ok(Value::Table(table))
}

fn set_signature(doc: &mut Value, signature: &PackSignature) -> Result<()> {
    let table = doc
        .as_table_mut()
        .ok_or_else(|| anyhow!("pack manifest must be a table"))?;

    let greentic_entry = table
        .entry("greentic".to_string())
        .or_insert_with(|| Value::Table(toml::map::Map::new()));

    let greentic_table = greentic_entry
        .as_table_mut()
        .ok_or_else(|| anyhow!("[greentic] must be a table"))?;

    let signature_value = Value::try_from(signature.clone())
        .map_err(|err| anyhow!("failed to serialise signature: {err}"))?;

    greentic_table.insert("signature".to_string(), signature_value);
    Ok(())
}

fn strip_signature(doc: &mut Value) {
    let Some(table) = doc.as_table_mut() else {
        return;
    };

    if let Some(greentic) = table.get_mut("greentic")
        && let Some(section) = greentic.as_table_mut()
    {
        section.remove("signature");
        if section.is_empty() {
            table.remove("greentic");
        }
    }
}

fn signature_from_doc(doc: &Value) -> Result<Option<PackSignature>> {
    let table = doc
        .as_table()
        .ok_or_else(|| anyhow!("pack manifest must be a table"))?;

    let Some(greentic) = table.get("greentic") else {
        return Ok(None);
    };

    let greentic_table = greentic
        .as_table()
        .ok_or_else(|| anyhow!("[greentic] must be a table"))?;

    let Some(signature_value) = greentic_table.get("signature") else {
        return Ok(None);
    };

    let signature: PackSignature = signature_value
        .clone()
        .try_into()
        .map_err(|err| anyhow!("invalid greentic.signature block: {err}"))?;

    Ok(Some(signature))
}

const MANIFEST_CANDIDATES: [&str; 2] = ["pack.toml", "greentic-pack.toml"];

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
