//! Auto-derive the credential setup form (`assets/setup.yaml`) and
//! `assets/secret-requirements.json` for an application pack from its
//! `agents[].llm` and the secret requirements of the tool extensions the
//! agents use. Pure logic; all I/O (resolving `describe.json`) lives in the
//! caller (`cli::ext_resolver`).

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// One credential question. Field set/names match greentic-setup's
/// `SetupQuestion` reader; `None` optionals are omitted (the reader defaults
/// them), so output stays close to a hand-authored file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetupQuestionOut {
    pub name: String,
    pub title: String,
    pub kind: String, // always "string" for credentials
    pub required: bool,
    pub secret: bool, // always true for credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetupSpecOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub questions: Vec<SetupQuestionOut>,
}

impl SetupSpecOut {
    pub fn to_yaml(&self) -> anyhow::Result<String> {
        serde_yaml_bw::to_string(self).map_err(Into::into)
    }
}

/// One entry of `secret-requirements.json`. `required` is omitted when `true`
/// (the reader defaults it to true), matching hand-authored files.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SecretRequirementOut {
    pub key: String,
    #[serde(skip_serializing_if = "is_true")]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn is_true(b: &bool) -> bool {
    *b
}

/// The two generated asset bodies, ready to write into the pack.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedSetup {
    pub setup_yaml: String,
    pub secret_requirements_json: String,
}

/// One secret a tool needs, as declared in the extension's `describe.json`
/// `contributions.tools[].secret_requirements`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolSecretReq {
    pub key: String,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
}

fn default_required() -> bool {
    true
}

// Minimal view of describe.json — only the fields we consume.
#[derive(Deserialize, Default)]
struct DescribeMinimal {
    #[serde(default)]
    contributions: DescribeContributions,
}

#[derive(Deserialize, Default)]
struct DescribeContributions {
    #[serde(default)]
    tools: Vec<DescribeTool>,
}

#[derive(Deserialize)]
struct DescribeTool {
    #[serde(default)]
    name: String,
    #[serde(default)]
    secret_requirements: Vec<ToolSecretReq>,
}

/// Collect the secret requirements of the named tools from a `describe.json`
/// body, deduped by `key` (first occurrence wins), preserving discovery order.
pub fn extract_tool_secret_requirements(
    describe_json: &[u8],
    used_tool_names: &[String],
) -> anyhow::Result<Vec<ToolSecretReq>> {
    let describe: DescribeMinimal =
        serde_json::from_slice(describe_json).context("parse extension describe.json")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for tool in &describe.contributions.tools {
        if !used_tool_names.iter().any(|t| t == &tool.name) {
            continue;
        }
        for req in &tool.secret_requirements {
            if seen.insert(req.key.clone()) {
                out.push(req.clone());
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAVILY_DESCRIBE: &str = r#"{
      "contributions": {
        "tools": [
          {"name": "tavily_search",  "secret_requirements": [
            {"key": "tavily/api_key", "required": true, "description": "Search key", "format": "text"}]},
          {"name": "tavily_extract", "secret_requirements": [
            {"key": "tavily/api_key", "required": true, "description": "Extract key", "format": "text"}]}
        ]
      }
    }"#;

    #[test]
    fn extracts_and_dedupes_tool_secrets_for_used_tools() {
        let used = vec!["tavily_search".to_string(), "tavily_extract".to_string()];
        let reqs = extract_tool_secret_requirements(TAVILY_DESCRIBE.as_bytes(), &used).unwrap();
        assert_eq!(reqs.len(), 1, "same key on two tools dedupes to one");
        assert_eq!(reqs[0].key, "tavily/api_key");
        assert_eq!(reqs[0].description.as_deref(), Some("Search key"));
    }

    #[test]
    fn ignores_secrets_of_unused_tools() {
        let used = vec!["tavily_extract".to_string()];
        let reqs = extract_tool_secret_requirements(TAVILY_DESCRIBE.as_bytes(), &used).unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].description.as_deref(), Some("Extract key"));
    }

    #[test]
    fn setup_spec_serializes_and_round_trips_with_optional_fields_omitted() {
        let spec = SetupSpecOut {
            title: Some("Demo — credentials".to_string()),
            description: None,
            questions: vec![SetupQuestionOut {
                name: "deepseek".to_string(),
                title: "DeepSeek API key".to_string(),
                kind: "string".to_string(),
                required: true,
                secret: true,
                help: Some("LLM key".to_string()),
                group: Some("LLM".to_string()),
                docs_url: Some("https://platform.deepseek.com".to_string()),
                placeholder: Some("sk-...".to_string()),
            }],
        };
        let yaml = spec.to_yaml().expect("serialize");
        // description (None) must not appear; title (None on question is N/A here)
        assert!(!yaml.contains("description"));
        assert!(yaml.contains("name: deepseek"));
        // Round-trips through a serde_json::Value with the same field names.
        let v: serde_json::Value = serde_yaml_bw::from_str(&yaml).expect("parse");
        assert_eq!(v["questions"][0]["group"], "LLM");
        assert!(v["questions"][0].get("default").is_none());
    }
}
