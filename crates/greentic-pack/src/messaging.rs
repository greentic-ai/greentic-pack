use std::collections::BTreeSet;

use anyhow::{Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
pub struct MessagingSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapters: Option<Vec<MessagingAdapter>>,
}

impl MessagingSection {
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        if let Some(adapters) = &self.adapters {
            for adapter in adapters {
                adapter.validate()?;
                if !seen.insert(adapter.name.clone()) {
                    bail!("duplicate messaging adapter name: {}", adapter.name);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct MessagingAdapter {
    pub name: String,
    pub kind: MessagingAdapterKind,
    pub component: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<MessagingAdapterCapabilities>,
}

impl MessagingAdapter {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("messaging.adapters[].name is required");
        }
        if self.component.trim().is_empty() {
            bail!(
                "messaging.adapters[{}].component must not be empty",
                self.name
            );
        }
        if let Some(cap) = &self.capabilities {
            cap.validate(&self.name)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MessagingAdapterKind {
    Ingress,
    Egress,
    IngressEgress,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct MessagingAdapterCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direction: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

impl MessagingAdapterCapabilities {
    fn validate(&self, name: &str) -> Result<()> {
        for entry in &self.direction {
            if entry.trim().is_empty() {
                bail!(
                    "messaging.adapters[{name}].capabilities.direction must not contain empty values"
                );
            }
        }
        for entry in &self.features {
            if entry.trim().is_empty() {
                bail!(
                    "messaging.adapters[{name}].capabilities.features must not contain empty values"
                );
            }
        }
        Ok(())
    }
}
