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

/// Display metadata for an LLM provider's API-key question. Keyed by the
/// provider id used in `pack.yaml agents[].llm.provider` (matches
/// `greentic_llm::ProviderKind::as_str()`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderOverlay {
    pub label: String,
    pub docs_url: String,
    pub placeholder: String,
}

fn overlay(label: &str, docs_url: &str, placeholder: &str) -> ProviderOverlay {
    ProviderOverlay {
        label: label.to_string(),
        docs_url: docs_url.to_string(),
        placeholder: placeholder.to_string(),
    }
}

/// Polished display metadata for the popular providers. Returns `None` for
/// providers without a curated entry (the caller emits a minimal question).
/// Task 5's drift-test asserts every `greentic_llm::ProviderKind` is either
/// covered here or in that test's explicit minimal allow-list.
pub fn llm_overlay(provider: &str) -> Option<ProviderOverlay> {
    Some(match provider {
        "openai" => overlay("OpenAI", "https://platform.openai.com/api-keys", "sk-..."),
        "anthropic" => overlay(
            "Anthropic",
            "https://console.anthropic.com/settings/keys",
            "sk-ant-...",
        ),
        "deepseek" => overlay("DeepSeek", "https://platform.deepseek.com", "sk-..."),
        "gemini" => overlay(
            "Google Gemini",
            "https://aistudio.google.com/app/apikey",
            "AIza...",
        ),
        "cohere" => overlay("Cohere", "https://dashboard.cohere.com/api-keys", "..."),
        "groq" => overlay("Groq", "https://console.groq.com/keys", "gsk_..."),
        "perplexity" => overlay(
            "Perplexity",
            "https://www.perplexity.ai/settings/api",
            "pplx-...",
        ),
        "xai" => overlay("xAI", "https://console.x.ai", "xai-..."),
        "mistral" => overlay("Mistral", "https://console.mistral.ai/api-keys", "..."),
        "openrouter" => overlay("OpenRouter", "https://openrouter.ai/keys", "sk-or-..."),
        _ => return None,
    })
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

use std::collections::BTreeMap;
use tracing::warn;

/// A pending question keyed by its secret key, before collision resolution.
struct Pending {
    secret_key: String, // canonical secret key (e.g. "llm/deepseek", "tavily/api_key")
    provider: String,   // "llm" provider id, or the tool secret's first segment
    last_segment: String, // default question name (segment after the last "/")
    question: SetupQuestionOut,
    requirement: SecretRequirementOut,
}

fn last_segment(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

fn llm_question(provider: &str, credential_ref: &str) -> Pending {
    let secret_key = format!("llm/{credential_ref}");
    let (title, help, docs_url, placeholder) = match llm_overlay(provider) {
        Some(o) => (
            format!("{} API key", o.label),
            Some("LLM API key for the agentic worker's reasoning loop.".to_string()),
            Some(o.docs_url),
            Some(o.placeholder),
        ),
        None => {
            warn!(
                provider,
                "no LLM overlay; emitting a minimal credential question"
            );
            (
                format!("{provider} API key"),
                Some("LLM API key for the agentic worker's reasoning loop.".to_string()),
                None,
                None,
            )
        }
    };
    Pending {
        secret_key: secret_key.clone(),
        provider: "llm".to_string(),
        last_segment: credential_ref.to_string(),
        question: SetupQuestionOut {
            name: credential_ref.to_string(),
            title,
            kind: "string".to_string(),
            required: true,
            secret: true,
            help,
            group: Some("LLM".to_string()),
            docs_url,
            placeholder,
        },
        requirement: SecretRequirementOut {
            key: secret_key,
            required: true,
            description: None,
        },
    }
}

fn tool_question(req: &ToolSecretReq) -> Pending {
    let provider = req.key.split('/').next().unwrap_or("").to_string();
    let seg = last_segment(&req.key).to_string();
    Pending {
        secret_key: req.key.clone(),
        provider,
        last_segment: seg.clone(),
        question: SetupQuestionOut {
            name: seg.clone(),
            title: format!("{} key", titleize(&seg)),
            kind: "string".to_string(),
            required: req.required,
            secret: true,
            help: req.description.clone(),
            group: Some("Tools".to_string()),
            docs_url: None,
            placeholder: None,
        },
        requirement: SecretRequirementOut {
            key: req.key.clone(),
            required: req.required,
            description: req.description.clone(),
        },
    }
}

fn titleize(s: &str) -> String {
    s.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build setup.yaml + secret-requirements.json from a pack's agents and the
/// resolved tool secret requirements. Returns `None` when there is nothing to
/// emit. Component requirements are merged into secret-requirements.json.
pub fn generate(
    pack_id: &str,
    agents: &BTreeMap<String, serde_json::Value>,
    tool_reqs_by_ext: &BTreeMap<String, Vec<ToolSecretReq>>,
    component_reqs: &[SecretRequirementOut],
) -> anyhow::Result<Option<GeneratedSetup>> {
    let mut pending: Vec<Pending> = Vec::new();
    let mut seen_keys = std::collections::BTreeSet::new();

    for agent in agents.values() {
        // LLM question
        if let Some(cred) = agent
            .get("llm")
            .and_then(|l| l.get("credential_ref"))
            .and_then(|c| c.as_str())
        {
            let provider = agent["llm"]
                .get("provider")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let p = llm_question(provider, cred);
            if seen_keys.insert(p.secret_key.clone()) {
                pending.push(p);
            }
        }
        // Tool questions
        if let Some(tools) = agent.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                let ext_id = tool
                    .get("extension_id")
                    .and_then(|e| e.as_str())
                    .unwrap_or("");
                let tool_name = tool.get("tool_name").and_then(|n| n.as_str()).unwrap_or("");
                let Some(reqs) = tool_reqs_by_ext.get(ext_id) else {
                    continue;
                };
                for req in reqs {
                    // The resolver already filtered to used tools; key-dedupe here.
                    let _ = tool_name;
                    if seen_keys.insert(req.key.clone()) {
                        pending.push(tool_question(req));
                    }
                }
            }
        }
    }

    if pending.is_empty() && component_reqs.is_empty() {
        return Ok(None);
    }

    // Resolve question-name collisions: if two pending share a question name,
    // disambiguate both to "<provider>_<segment>".
    let mut name_counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in &pending {
        *name_counts.entry(p.question.name.clone()).or_default() += 1;
    }
    for p in &mut pending {
        if name_counts.get(&p.question.name).copied().unwrap_or(0) > 1 {
            p.question.name = format!("{}_{}", p.provider, p.last_segment);
        }
    }

    let questions: Vec<SetupQuestionOut> = pending.iter().map(|p| p.question.clone()).collect();
    let mut requirements: Vec<SecretRequirementOut> =
        pending.iter().map(|p| p.requirement.clone()).collect();
    // Merge component-derived requirements (dedupe by key).
    for cr in component_reqs {
        if !requirements.iter().any(|r| r.key == cr.key) {
            requirements.push(cr.clone());
        }
    }

    let spec = SetupSpecOut {
        title: Some(format!("{pack_id} — credentials")),
        description: Some("API keys for the agentic worker and its tools.".to_string()),
        questions,
    };
    Ok(Some(GeneratedSetup {
        setup_yaml: spec.to_yaml()?,
        secret_requirements_json: serde_json::to_string_pretty(&requirements)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn tavily_agents() -> BTreeMap<String, serde_json::Value> {
        let agent = serde_json::json!({
            "agent_id": "demo_assistant",
            "llm": {"provider": "deepseek", "model": "deepseek-chat", "credential_ref": "deepseek"},
            "tools": [
                {"extension_id": "greentic.tavily", "tool_name": "tavily_search"},
                {"extension_id": "greentic.tavily", "tool_name": "tavily_extract"}
            ]
        });
        BTreeMap::from([("demo_assistant".to_string(), agent)])
    }

    fn tavily_tool_reqs() -> BTreeMap<String, Vec<ToolSecretReq>> {
        BTreeMap::from([(
            "greentic.tavily".to_string(),
            vec![ToolSecretReq {
                key: "tavily/api_key".to_string(),
                required: true,
                description: Some("Tavily web-search API key.".to_string()),
                format: Some("text".to_string()),
            }],
        )])
    }

    #[test]
    fn generate_produces_llm_and_tool_questions() {
        let output = generate(
            "agentic-research-tavily-demo",
            &tavily_agents(),
            &tavily_tool_reqs(),
            &[],
        )
        .unwrap()
        .expect("some output");

        let spec: serde_json::Value = serde_yaml_bw::from_str(&output.setup_yaml).unwrap();
        let q = spec["questions"].as_array().unwrap();
        assert_eq!(q.len(), 2, "one LLM + one tool (deduped)");

        let llm = q.iter().find(|x| x["name"] == "deepseek").unwrap();
        assert_eq!(llm["group"], "LLM");
        assert_eq!(llm["title"], "DeepSeek API key");
        assert_eq!(llm["docs_url"], "https://platform.deepseek.com");
        assert_eq!(llm["secret"], true);

        let tool = q.iter().find(|x| x["name"] == "api_key").unwrap();
        assert_eq!(tool["group"], "Tools");
        assert_eq!(tool["help"], "Tavily web-search API key.");

        let reqs: Vec<serde_json::Value> =
            serde_json::from_str(&output.secret_requirements_json).unwrap();
        let keys: Vec<&str> = reqs.iter().map(|r| r["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&"llm/deepseek"));
        assert!(keys.contains(&"tavily/api_key"));
    }

    #[test]
    fn generate_disambiguates_colliding_tool_names() {
        let mut tool_reqs = tavily_tool_reqs();
        tool_reqs.insert(
            "other.search".to_string(),
            vec![ToolSecretReq {
                key: "other/api_key".to_string(),
                required: true,
                description: None,
                format: None,
            }],
        );
        let mut agents = tavily_agents();
        // add a second agent using other.search so both api_key secrets surface
        agents.insert(
            "a2".to_string(),
            serde_json::json!({
                "agent_id": "a2",
                "llm": {"provider": "openai", "credential_ref": "openai"},
                "tools": [{"extension_id": "other.search", "tool_name": "search"}]
            }),
        );
        let output = generate("p", &agents, &tool_reqs, &[]).unwrap().unwrap();
        let spec: serde_json::Value = serde_yaml_bw::from_str(&output.setup_yaml).unwrap();
        let names: Vec<&str> = spec["questions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| q["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"tavily_api_key"));
        assert!(names.contains(&"other_api_key"));
    }

    #[test]
    fn llm_overlay_known_and_unknown() {
        let d = llm_overlay("deepseek").expect("deepseek known");
        assert_eq!(d.label, "DeepSeek");
        assert!(d.docs_url.starts_with("https://"));
        assert!(d.placeholder.starts_with("sk-"));
        assert!(llm_overlay("totally-unknown-provider").is_none());
    }

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
