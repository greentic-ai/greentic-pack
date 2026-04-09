use std::collections::BTreeSet;
use std::fmt;

use anyhow::{Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
pub struct EventsSection {
    #[serde(default)]
    pub providers: Vec<EventProviderSpec>,
}

impl EventsSection {
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for provider in &self.providers {
            provider.validate()?;
            if !seen.insert(provider.name.clone()) {
                bail!("duplicate events provider name: {}", provider.name);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct EventProviderSpec {
    pub name: String,
    pub kind: EventProviderKind,
    pub component: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_flow: Option<String>,
    #[serde(default)]
    pub capabilities: EventProviderCapabilities,
}

impl EventProviderSpec {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("events.providers[].name is required");
        }
        if self.component.trim().is_empty() {
            bail!(
                "events.providers[{}].component must not be empty",
                self.name
            );
        }
        for topic in &self.capabilities.topics {
            if topic.trim().is_empty() {
                bail!(
                    "events.providers[{}].capabilities.topics may not contain empty entries",
                    self.name
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
pub struct EventProviderCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reliability: Option<ReliabilityKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<OrderingKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventProviderKind {
    Broker,
    Source,
    Sink,
    Bridge,
}

impl fmt::Display for EventProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Broker => "broker",
            Self::Source => "source",
            Self::Sink => "sink",
            Self::Bridge => "bridge",
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum TransportKind {
    Nats,
    Kafka,
    Sqs,
    Webhook,
    Email,
    Other(String),
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nats => f.write_str("nats"),
            Self::Kafka => f.write_str("kafka"),
            Self::Sqs => f.write_str("sqs"),
            Self::Webhook => f.write_str("webhook"),
            Self::Email => f.write_str("email"),
            Self::Other(value) => f.write_str(value),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityKind {
    AtMostOnce,
    AtLeastOnce,
    EffectivelyOnce,
}

impl fmt::Display for ReliabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::AtMostOnce => "at_most_once",
            Self::AtLeastOnce => "at_least_once",
            Self::EffectivelyOnce => "effectively_once",
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderingKind {
    None,
    PerKey,
    Global,
}

impl fmt::Display for OrderingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::None => "none",
            Self::PerKey => "per_key",
            Self::Global => "global",
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_provider() -> EventProviderSpec {
        EventProviderSpec {
            name: "orders".to_string(),
            kind: EventProviderKind::Broker,
            component: "broker.component".to_string(),
            default_flow: Some("flow.default".to_string()),
            custom_flow: None,
            capabilities: EventProviderCapabilities {
                transport: Some(TransportKind::Kafka),
                reliability: Some(ReliabilityKind::AtLeastOnce),
                ordering: Some(OrderingKind::PerKey),
                topics: vec!["orders.created".to_string()],
            },
        }
    }

    #[test]
    fn validate_accepts_unique_provider_with_topics() {
        let section = EventsSection {
            providers: vec![valid_provider()],
        };

        section.validate().expect("valid events section");
    }

    #[test]
    fn validate_rejects_duplicate_provider_names() {
        let provider = valid_provider();
        let section = EventsSection {
            providers: vec![provider.clone(), provider],
        };

        let err = section.validate().expect_err("duplicate names should fail");
        assert!(err.to_string().contains("duplicate events provider name"));
    }

    #[test]
    fn validate_rejects_blank_topic_entries() {
        let mut provider = valid_provider();
        provider.capabilities.topics.push("   ".to_string());

        let err = EventsSection {
            providers: vec![provider],
        }
        .validate()
        .expect_err("blank topics should fail");

        assert!(
            err.to_string()
                .contains("topics may not contain empty entries")
        );
    }

    #[test]
    fn display_formats_enum_values() {
        assert_eq!(EventProviderKind::Sink.to_string(), "sink");
        assert_eq!(TransportKind::Other("sns".to_string()).to_string(), "sns");
        assert_eq!(
            ReliabilityKind::EffectivelyOnce.to_string(),
            "effectively_once"
        );
        assert_eq!(OrderingKind::Global.to_string(), "global");
    }
}
