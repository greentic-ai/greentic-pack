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

#[cfg(test)]
mod tests {
    use super::*;

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
