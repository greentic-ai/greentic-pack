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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_adapter() -> MessagingAdapter {
        MessagingAdapter {
            name: "inbox".to_string(),
            kind: MessagingAdapterKind::Ingress,
            component: "component.messaging".to_string(),
            default_flow: Some("flow.main".to_string()),
            custom_flow: None,
            capabilities: Some(MessagingAdapterCapabilities {
                direction: vec!["ingress".to_string()],
                features: vec!["retry".to_string()],
            }),
        }
    }

    #[test]
    fn validate_accepts_unique_adapter() {
        MessagingSection {
            adapters: Some(vec![valid_adapter()]),
        }
        .validate()
        .expect("valid adapter should pass");
    }

    #[test]
    fn validate_rejects_duplicate_adapter_names() {
        let adapter = valid_adapter();
        let err = MessagingSection {
            adapters: Some(vec![adapter.clone(), adapter]),
        }
        .validate()
        .expect_err("duplicate names should fail");

        assert!(err.to_string().contains("duplicate messaging adapter name"));
    }

    #[test]
    fn validate_rejects_blank_feature_entry() {
        let mut adapter = valid_adapter();
        adapter
            .capabilities
            .as_mut()
            .expect("caps")
            .features
            .push(" ".to_string());

        let err = MessagingSection {
            adapters: Some(vec![adapter]),
        }
        .validate()
        .expect_err("blank feature should fail");

        assert!(err.to_string().contains("capabilities.features"));
    }
}
