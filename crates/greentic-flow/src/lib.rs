use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod loader {
    use super::FlowDocument;
    use anyhow::{anyhow, Result};

    pub fn load_ygtc_from_str(id: &str, source: &str) -> Result<FlowDocument> {
        if id.is_empty() {
            return Err(anyhow!("flow id must not be empty"));
        }
        Ok(FlowDocument::new(id, source))
    }
}

pub mod resolver {
    use super::FlowIr;
    use anyhow::Result;
    use serde::Serialize;

    #[derive(Debug, Default, Serialize)]
    pub struct ResolvedParameters {
        pub parameters: serde_json::Value,
    }

    pub fn resolve_parameters(ir: &FlowIr) -> Result<ResolvedParameters> {
        let params = serde_json::json!({ "flow_id": ir.id });
        Ok(ResolvedParameters { parameters: params })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocument {
    pub id: String,
    pub source: String,
}

impl FlowDocument {
    pub fn new(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
        }
    }

    pub fn to_ir(&self) -> Result<FlowIr> {
        Ok(FlowIr {
            id: self.id.clone(),
            source: self.source.clone(),
            schema: serde_json::json!({ "kind": "stub" }),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowIr {
    pub id: String,
    pub source: String,
    pub schema: serde_json::Value,
}

impl FlowIr {
    pub fn schema_doc(&self) -> SchemaDoc {
        SchemaDoc {
            flow_id: self.id.clone(),
            schema: self.schema.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDoc {
    pub flow_id: String,
    pub schema: serde_json::Value,
}
