use greentic_pack::builder::PackMeta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoReport {
    pub info_schema_version: u32,
    pub name: String,
    pub version: String,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub support: Option<String>,
    pub vendor: Option<String>,
    pub created_at_utc: String,
    pub signature: SignatureInfo,
    pub components: Vec<ComponentInfo>,
    pub entry_flows: Vec<String>,
    pub imports: Vec<ImportInfo>,
    pub interfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    pub status: SignatureStatus,
    pub key_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignatureStatus {
    Signed,
    Unsigned,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub component_id: String,
    pub version: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    pub pack_id: String,
    pub version_req: String,
}

impl InfoReport {
    pub fn from_pack_meta_and_signature(meta: &PackMeta, signature: SignatureInfo) -> Self {
        Self {
            info_schema_version: 1,
            name: meta.name.clone(),
            version: meta.version.to_string(),
            kind: meta.kind.as_ref().map(pack_kind_to_string),
            description: meta.description.clone(),
            authors: meta.authors.clone(),
            license: meta.license.clone(),
            homepage: meta.homepage.clone(),
            support: meta.support.clone(),
            vendor: meta.vendor.clone(),
            created_at_utc: meta.created_at_utc.clone(),
            signature,
            components: meta
                .components
                .iter()
                .map(|c| ComponentInfo {
                    component_id: c.component_id.clone(),
                    version: c.version.clone(),
                    kind: c.kind.clone(),
                })
                .collect(),
            entry_flows: meta.entry_flows.clone(),
            imports: meta
                .imports
                .iter()
                .map(|i| ImportInfo {
                    pack_id: i.pack_id.clone(),
                    version_req: i.version_req.clone(),
                })
                .collect(),
            interfaces: meta
                .interfaces
                .iter()
                .map(|b| format!("{}:{}@{}", b.package, b.world, b.version))
                .collect(),
        }
    }
}

fn pack_kind_to_string(k: &greentic_pack::PackKind) -> String {
    // Relies on PackKind's #[serde(rename_all = "kebab-case")] attribute.
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{:?}", k).to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_pack_meta_projects_all_fields() {
        use greentic_pack::PackKind;
        use greentic_pack::builder::{ComponentDescriptor, ImportRef, PackMeta};
        use greentic_pack::repo::InterfaceBinding;

        let meta = PackMeta {
            pack_version: 1,
            pack_id: "ex".into(),
            version: semver::Version::parse("1.2.3").unwrap(),
            name: "ex".into(),
            kind: Some(PackKind::Application),
            description: Some("demo".into()),
            authors: vec!["Alice".into()],
            license: Some("MIT".into()),
            homepage: None,
            support: None,
            vendor: None,
            imports: vec![ImportRef {
                pack_id: "greentic/core".into(),
                version_req: "^0.6.0".into(),
            }],
            entry_flows: vec!["flows/a.ygtc".into()],
            created_at_utc: "2026-01-01T00:00:00Z".into(),
            events: None,
            repo: None,
            messaging: None,
            interfaces: vec![InterfaceBinding {
                package: "greentic".into(),
                world: "component".into(),
                version: "0.6.0".into(),
                note: None,
            }],
            annotations: Default::default(),
            distribution: None,
            components: vec![ComponentDescriptor {
                component_id: "c".into(),
                version: "0.1.0".into(),
                digest: "sha256:x".into(),
                artifact_path: "components/c.wasm".into(),
                kind: Some("messaging".into()),
                artifact_type: Some("component/wasm".into()),
                tags: vec![],
                platform: None,
                entrypoint: None,
            }],
        };
        let sig = SignatureInfo {
            status: SignatureStatus::Unsigned,
            key_id: None,
        };

        let report = InfoReport::from_pack_meta_and_signature(&meta, sig);

        assert_eq!(report.info_schema_version, 1);
        assert_eq!(report.name, "ex");
        assert_eq!(report.version, "1.2.3");
        assert_eq!(report.kind.as_deref(), Some("application"));
        assert_eq!(report.authors, vec!["Alice".to_string()]);
        assert_eq!(report.license.as_deref(), Some("MIT"));
        assert_eq!(report.created_at_utc, "2026-01-01T00:00:00Z");
        assert_eq!(report.components.len(), 1);
        assert_eq!(report.components[0].component_id, "c");
        assert_eq!(report.components[0].version, "0.1.0");
        assert_eq!(report.components[0].kind.as_deref(), Some("messaging"));
        assert_eq!(report.entry_flows, vec!["flows/a.ygtc".to_string()]);
        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.imports[0].pack_id, "greentic/core");
        assert_eq!(report.imports[0].version_req, "^0.6.0");
        assert_eq!(
            report.interfaces,
            vec!["greentic:component@0.6.0".to_string()]
        );
        assert_eq!(report.signature.status, SignatureStatus::Unsigned);
    }

    #[test]
    fn json_has_schema_version_one() {
        let report = InfoReport {
            info_schema_version: 1,
            name: "x".into(),
            version: "0.1.0".into(),
            kind: None,
            description: None,
            authors: vec![],
            license: None,
            homepage: None,
            support: None,
            vendor: None,
            created_at_utc: "2026-01-01T00:00:00Z".into(),
            signature: SignatureInfo {
                status: SignatureStatus::Unsigned,
                key_id: None,
            },
            components: vec![],
            entry_flows: vec![],
            imports: vec![],
            interfaces: vec![],
        };
        let v: serde_json::Value = serde_json::to_value(&report).unwrap();
        assert_eq!(v["info_schema_version"], 1);
        assert_eq!(v["signature"]["status"], "unsigned");
    }
}
