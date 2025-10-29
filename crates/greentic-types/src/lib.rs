use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("pack id must not be empty".into());
        }
        if self.version.is_empty() {
            return Err("pack version must not be empty".into());
        }
        Ok(())
    }
}
