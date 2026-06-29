# Auto-Derive Credential Setup Form — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** During `greentic-pack` build, auto-derive `assets/setup.yaml` + `assets/secret-requirements.json` from a pack's `agents[].llm` and the tool extensions each agent uses, so any application pack carries its credential form without hand-authoring.

**Architecture:** A new pure module `crates/packc/src/setup_gen.rs` turns parsed agents + resolved tool secret-requirements into the two asset bodies. A small resolver addition reads `describe.json` from each tool extension's `.gtxpack`. `build.rs::run()` calls the generator just before packaging, respecting a hand-authored override and merging component-derived requirements.

**Tech Stack:** Rust 2024 (rustc 1.95.0), `anyhow`, `serde`, `serde_yaml_bw` (package `serde_yaml_gtc`), `serde_json`, `zip` — all already workspace deps of `packc`. `greentic-llm` only as a `dev-dependency` for the drift-test.

## Global Constraints

- Repo `greentic-pack`, crate `packc`. `#![forbid(unsafe_code)]` — no unsafe.
- No new runtime dependency on `greentic-llm` (heavy via rig-core); it is a `dev-dependency` for the drift-test only.
- Parse `describe.json` with a **minimal local serde struct**, not `greentic-extension-sdk-contract`.
- Emitted `setup.yaml` must deserialize into greentic-setup's `SetupSpec`/`SetupQuestion`; emitted `secret-requirements.json` into `Vec<PackSecretRequirement>`. Field names verbatim: question = `name,title,kind,required,secret,help,group,docs_url,placeholder`; requirement = `key,required,description`.
- Name mapping (must keep the zero-env runtime bridge resolving): LLM question `name = credential_ref`, requirement key `llm/<credential_ref>`; tool question `name = <last path segment of secret key>`, requirement key `<full secret key>`. On a question-name collision across extensions, disambiguate to `<provider>_<key>`.
- Override: a hand-authored `assets/setup.yaml` present in the pack source wins (generator skips it).
- Error: a declared tool extension that cannot be resolved at build time is a hard error. An LLM provider missing from the overlay warns and emits a minimal-but-valid question.
- Run `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean before each commit (CLAUDE.md).

---

## File Structure

- **Create** `crates/packc/src/setup_gen.rs` — all generator logic: output types, describe.json parsing, LLM overlay, question building, `generate()`. Pure (no I/O).
- **Modify** `crates/packc/src/lib.rs` (or `main.rs` module list) — `mod setup_gen;`.
- **Modify** `crates/packc/src/cli/ext_resolver.rs` — add `read_describe_from_gtxpack()` + `resolve_agent_tool_requirements()` (the only I/O part: acquire `.gtxpack`, read `describe.json`).
- **Modify** `crates/packc/src/build.rs` — call the generator in `run()` before `package_gtpack()`.
- **Create** `crates/packc/tests/setup_gen_build.rs` — integration: build a fixture agent pack, assert the gtpack carries the two assets.
- **Modify** `crates/packc/Cargo.toml` — add `greentic-llm` under `[dev-dependencies]`.
- **Modify (greentic-demo)** `crates/agentic-research-tavily-demo/` — remove hand-authored `assets/setup.yaml` + `assets/secret-requirements.json`; rebuild gtpack; parity check.

---

### Task 1: Output types + serialization

**Files:**
- Create: `crates/packc/src/setup_gen.rs`
- Modify: `crates/packc/src/lib.rs` (add `mod setup_gen;` — place beside the other `mod` lines)

**Interfaces:**
- Produces: `SetupQuestionOut`, `SetupSpecOut`, `SecretRequirementOut`, `GeneratedSetup` (used by all later tasks); `SetupSpecOut::to_yaml() -> anyhow::Result<String>`, `serde_json::to_string_pretty` for requirements.

- [ ] **Step 1: Write the failing test**

Add at the bottom of `crates/packc/src/setup_gen.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::setup_spec_serializes -- --nocapture`
Expected: FAIL — `setup_gen` / `SetupSpecOut` not found.

- [ ] **Step 3: Write minimal implementation**

At the top of `crates/packc/src/setup_gen.rs`:

```rust
//! Auto-derive the credential setup form (`assets/setup.yaml`) and
//! `assets/secret-requirements.json` for an application pack from its
//! `agents[].llm` and the secret requirements of the tool extensions the
//! agents use. Pure logic; all I/O (resolving `describe.json`) lives in the
//! caller (`cli::ext_resolver`).

use serde::Serialize;

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
```

Add `mod setup_gen;` to `crates/packc/src/lib.rs` beside the existing `mod` declarations.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::setup_spec_serializes -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd greentic-pack
git add crates/packc/src/setup_gen.rs crates/packc/src/lib.rs
git commit -m "feat(setup-gen): output types + setup.yaml serialization"
```

---

### Task 2: describe.json parsing → tool secret requirements

**Files:**
- Modify: `crates/packc/src/setup_gen.rs`

**Interfaces:**
- Produces: `pub struct ToolSecretReq { key, required, description, format }` (Deserialize); `pub fn extract_tool_secret_requirements(describe_json: &[u8], used_tool_names: &[String]) -> anyhow::Result<Vec<ToolSecretReq>>` — parses `describe.json`, keeps only `contributions.tools[].secret_requirements` whose tool `name` is in `used_tool_names`, dedupes by `key` (first wins), stable order.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::extracts_and_dedupes -- --nocapture`
Expected: FAIL — `extract_tool_secret_requirements` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `setup_gen.rs` (above the tests module):

```rust
use anyhow::Context;
use serde::Deserialize;

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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p greentic-pack --lib setup_gen::tests:: -- --nocapture`
Expected: PASS (both new tests + Task 1's).

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/setup_gen.rs
git commit -m "feat(setup-gen): parse describe.json tool secret requirements"
```

---

### Task 3: LLM provider overlay

**Files:**
- Modify: `crates/packc/src/setup_gen.rs`

**Interfaces:**
- Produces: `pub struct ProviderOverlay { pub label: String, pub docs_url: String, pub placeholder: String }`; `pub fn llm_overlay(provider: &str) -> Option<ProviderOverlay>`. Used by Task 4. The set of recognised keys is asserted complete by Task 5's drift-test.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn llm_overlay_known_and_unknown() {
        let d = llm_overlay("deepseek").expect("deepseek known");
        assert_eq!(d.label, "DeepSeek");
        assert!(d.docs_url.starts_with("https://"));
        assert!(d.placeholder.starts_with("sk-"));
        assert!(llm_overlay("totally-unknown-provider").is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::llm_overlay_known -- --nocapture`
Expected: FAIL — `llm_overlay` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `setup_gen.rs`:

```rust
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
        "anthropic" => overlay("Anthropic", "https://console.anthropic.com/settings/keys", "sk-ant-..."),
        "deepseek" => overlay("DeepSeek", "https://platform.deepseek.com", "sk-..."),
        "gemini" => overlay("Google Gemini", "https://aistudio.google.com/app/apikey", "AIza..."),
        "cohere" => overlay("Cohere", "https://dashboard.cohere.com/api-keys", "..."),
        "groq" => overlay("Groq", "https://console.groq.com/keys", "gsk_..."),
        "perplexity" => overlay("Perplexity", "https://www.perplexity.ai/settings/api", "pplx-..."),
        "xai" => overlay("xAI", "https://console.x.ai", "xai-..."),
        "mistral" => overlay("Mistral", "https://console.mistral.ai/api-keys", "..."),
        "openrouter" => overlay("OpenRouter", "https://openrouter.ai/keys", "sk-or-..."),
        _ => return None,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::llm_overlay_known -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/setup_gen.rs
git commit -m "feat(setup-gen): LLM provider display overlay"
```

---

### Task 4: `generate()` — assemble setup.yaml + secret-requirements.json

**Files:**
- Modify: `crates/packc/src/setup_gen.rs`

**Interfaces:**
- Consumes: `SetupQuestionOut`, `SetupSpecOut`, `SecretRequirementOut`, `GeneratedSetup`, `ToolSecretReq`, `llm_overlay`.
- Produces: `pub fn generate(pack_id: &str, agents: &BTreeMap<String, serde_json::Value>, tool_reqs_by_ext: &BTreeMap<String, Vec<ToolSecretReq>>, component_reqs: &[SecretRequirementOut]) -> anyhow::Result<Option<GeneratedSetup>>`. Returns `Ok(None)` when there are no credential questions and no requirements (nothing to write). `tool_reqs_by_ext` is keyed by `extension_id`; the resolver (Task 6) builds it. Warnings for unknown providers go to `tracing::warn!`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
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
        let gen = generate("agentic-research-tavily-demo", &tavily_agents(), &tavily_tool_reqs(), &[])
            .unwrap()
            .expect("some output");

        let spec: serde_json::Value = serde_yaml_bw::from_str(&gen.setup_yaml).unwrap();
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

        let reqs: Vec<serde_json::Value> = serde_json::from_str(&gen.secret_requirements_json).unwrap();
        let keys: Vec<&str> = reqs.iter().map(|r| r["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&"llm/deepseek"));
        assert!(keys.contains(&"tavily/api_key"));
    }

    #[test]
    fn generate_disambiguates_colliding_tool_names() {
        let mut tool_reqs = tavily_tool_reqs();
        tool_reqs.insert(
            "other.search".to_string(),
            vec![ToolSecretReq { key: "other/api_key".to_string(), required: true, description: None, format: None }],
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
        let gen = generate("p", &agents, &tool_reqs, &[]).unwrap().unwrap();
        let spec: serde_json::Value = serde_yaml_bw::from_str(&gen.setup_yaml).unwrap();
        let names: Vec<&str> = spec["questions"].as_array().unwrap()
            .iter().map(|q| q["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"tavily_api_key"));
        assert!(names.contains(&"other_api_key"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --lib setup_gen::tests::generate_ -- --nocapture`
Expected: FAIL — `generate` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `setup_gen.rs`:

```rust
use std::collections::BTreeMap;
use tracing::warn;

/// A pending question keyed by its secret key, before collision resolution.
struct Pending {
    secret_key: String,   // canonical secret key (e.g. "llm/deepseek", "tavily/api_key")
    provider: String,     // "llm" provider id, or the tool secret's first segment
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
            warn!(provider, "no LLM overlay; emitting a minimal credential question");
            (format!("{provider} API key"),
             Some("LLM API key for the agentic worker's reasoning loop.".to_string()),
             None, None)
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
        requirement: SecretRequirementOut { key: secret_key, required: true, description: None },
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
        if let Some(cred) = agent.get("llm").and_then(|l| l.get("credential_ref")).and_then(|c| c.as_str()) {
            let provider = agent["llm"].get("provider").and_then(|p| p.as_str()).unwrap_or("");
            let p = llm_question(provider, cred);
            if seen_keys.insert(p.secret_key.clone()) {
                pending.push(p);
            }
        }
        // Tool questions
        if let Some(tools) = agent.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                let ext_id = tool.get("extension_id").and_then(|e| e.as_str()).unwrap_or("");
                let tool_name = tool.get("tool_name").and_then(|n| n.as_str()).unwrap_or("");
                let Some(reqs) = tool_reqs_by_ext.get(ext_id) else { continue };
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
```

- [ ] **Step 4: Run tests + fmt/clippy**

Run: `cargo test -p greentic-pack --lib setup_gen:: -- --nocapture && cargo fmt --all && cargo clippy -p greentic-pack --all-targets -- -D warnings`
Expected: all setup_gen tests PASS; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/setup_gen.rs
git commit -m "feat(setup-gen): generate() assembles setup.yaml + secret-requirements"
```

---

### Task 5: Drift-test against `greentic-llm::ProviderKind::all()`

**Files:**
- Modify: `crates/packc/Cargo.toml` (add `greentic-llm` to `[dev-dependencies]`)
- Create: `crates/packc/tests/llm_overlay_drift.rs`

**Interfaces:**
- Consumes: `greentic_pack::setup_gen::llm_overlay` (ensure `setup_gen` is `pub` in `lib.rs` — change `mod setup_gen;` to `pub mod setup_gen;`); `greentic_llm::capabilities::ProviderKind`.

- [ ] **Step 1: Write the failing test**

Create `crates/packc/tests/llm_overlay_drift.rs`:

```rust
//! Guards against greentic-llm gaining a provider the overlay author has not
//! triaged. Every `ProviderKind` must be either curated in `llm_overlay` or
//! listed here as intentionally minimal (functional question, no polish).

use greentic_llm::capabilities::ProviderKind;
use greentic_pack::setup_gen::llm_overlay;

/// Providers we knowingly ship without curated docs_url/placeholder. Adding a
/// provider to greentic-llm forces a choice: curate it in `llm_overlay`, or add
/// its id here.
const MINIMAL_OK: &[&str] = &[
    "ollama", "llamafile", "bedrock", "azure", "azure-foundry", "huggingface",
    "together", "moonshot", "minimax", "hyperbolic", "galadriel", "mira",
    "zai", "xiaomimimo",
];

#[test]
fn every_provider_kind_is_curated_or_explicitly_minimal() {
    let mut untriaged = Vec::new();
    for kind in ProviderKind::all() {
        let id = kind.as_str();
        if llm_overlay(id).is_none() && !MINIMAL_OK.contains(&id) {
            untriaged.push(id.to_string());
        }
    }
    assert!(
        untriaged.is_empty(),
        "new greentic-llm provider(s) not triaged in setup_gen::llm_overlay or MINIMAL_OK: {untriaged:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails (compile error first)**

Add to `crates/packc/Cargo.toml` under `[dev-dependencies]`:

```toml
greentic-llm = { workspace = true }
```

(If `greentic-llm` is not a workspace dependency, use the version the workspace pins, e.g. `greentic-llm = "1.2.6-research"`; confirm with `cargo tree -p greentic-pack -i greentic-llm` or the root `Cargo.toml`.)

Change `crates/packc/src/lib.rs`: `mod setup_gen;` → `pub mod setup_gen;`.

Run: `cargo test -p greentic-pack --test llm_overlay_drift -- --nocapture`
Expected: PASS if `MINIMAL_OK` + overlay together cover `ProviderKind::all()`; if it FAILS, it lists the untriaged providers — add each to `llm_overlay` (Task 3) or `MINIMAL_OK`. Iterate until green. (This is the intended behaviour of the guard.)

- [ ] **Step 3: Reconcile** — for every id the test reports, either add a curated `llm_overlay` arm or append to `MINIMAL_OK`. Re-run until PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/packc/Cargo.toml crates/packc/src/lib.rs crates/packc/tests/llm_overlay_drift.rs
git commit -m "test(setup-gen): drift-test overlay vs greentic-llm ProviderKind"
```

---

### Task 6: Resolve tool `describe.json` from each extension `.gtxpack`

**Files:**
- Modify: `crates/packc/src/cli/ext_resolver.rs`

**Interfaces:**
- Consumes: existing `lookup_ext_dependency(pack_dir, raw_ref)`, `acquire_extension_bytes(source, cache_dir, offline, handle)`, and the `zip::ZipArchive` pattern from `extract_and_verify_bytes` (reads a named ZIP entry).
- Produces:
  - `pub fn read_describe_from_gtxpack(extension_id: &str, zip_bytes: &[u8]) -> anyhow::Result<Vec<u8>>` — returns the `describe.json` body from the `.gtxpack` ZIP.
  - `pub fn resolve_agent_tool_requirements(pack_dir: &Path, agents: &BTreeMap<String, serde_json::Value>, cache_dir: &Path, offline: bool) -> anyhow::Result<BTreeMap<String, Vec<crate::setup_gen::ToolSecretReq>>>` — for each `(extension_id, [tool_names])` used by agents, acquires the `.gtxpack`, reads `describe.json`, and extracts the used tools' secret requirements. **Errors** if a declared extension is not in `pack.extensions.json` or cannot be acquired.

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)] mod` at the bottom of `ext_resolver.rs`:

```rust
#[cfg(test)]
mod describe_tests {
    use super::*;
    use std::io::Write;

    fn gtxpack_with_describe(describe: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file("describe.json", zip::write::FileOptions::<()>::default()).unwrap();
            zip.write_all(describe.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn reads_describe_json_entry_from_gtxpack() {
        let bytes = gtxpack_with_describe(r#"{"contributions":{"tools":[]}}"#);
        let body = read_describe_from_gtxpack("greentic.tavily", &bytes).unwrap();
        assert!(String::from_utf8_lossy(&body).contains("contributions"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --lib cli::ext_resolver::describe_tests -- --nocapture`
Expected: FAIL — `read_describe_from_gtxpack` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `ext_resolver.rs` (mirror `extract_and_verify_bytes`'s ZIP reading; add `use std::io::Read;` if not present):

```rust
/// Read the `describe.json` sidecar from a `.gtxpack` ZIP.
pub fn read_describe_from_gtxpack(extension_id: &str, zip_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .with_context(|| format!("open extension .gtxpack ZIP for '{extension_id}'"))?;
    let mut file = archive
        .by_name("describe.json")
        .with_context(|| format!("extension '{extension_id}' .gtxpack has no describe.json"))?;
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut body)
        .with_context(|| format!("read describe.json from '{extension_id}' .gtxpack"))?;
    Ok(body)
}

/// For each tool extension used by the agents, acquire its `.gtxpack`, read
/// `describe.json`, and extract the secret requirements of the used tools.
/// Keyed by extension id. Errors on an unresolvable declared extension.
pub fn resolve_agent_tool_requirements(
    pack_dir: &Path,
    agents: &std::collections::BTreeMap<String, serde_json::Value>,
    cache_dir: &Path,
    offline: bool,
) -> anyhow::Result<std::collections::BTreeMap<String, Vec<crate::setup_gen::ToolSecretReq>>> {
    use std::collections::{BTreeMap, BTreeSet};

    // Collect extension_id -> set(tool_name) actually used.
    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for agent in agents.values() {
        let Some(tools) = agent.get("tools").and_then(|t| t.as_array()) else { continue };
        for tool in tools {
            let (Some(ext), Some(name)) = (
                tool.get("extension_id").and_then(|e| e.as_str()),
                tool.get("tool_name").and_then(|n| n.as_str()),
            ) else { continue };
            used.entry(ext.to_string()).or_default().insert(name.to_string());
        }
    }

    let mut out = BTreeMap::new();
    for (ext_id, tool_names) in &used {
        let raw_ref = format!("ext://{ext_id}");
        let (_ext_ref, dep) = lookup_ext_dependency(pack_dir, &raw_ref)
            .with_context(|| format!("resolve tool extension '{ext_id}' for credential form"))?;
        let zip_bytes = acquire_extension_bytes(&dep.source, cache_dir, offline, None)
            .with_context(|| format!("acquire .gtxpack for tool extension '{ext_id}'"))?;
        let describe = read_describe_from_gtxpack(ext_id, &zip_bytes)?;
        let names: Vec<String> = tool_names.iter().cloned().collect();
        let reqs = crate::setup_gen::extract_tool_secret_requirements(&describe, &names)?;
        out.insert(ext_id.clone(), reqs);
    }
    Ok(out)
}
```

> Note: confirm the exact field path of the extension source on `ExtensionDependency` (the struct returned by `lookup_ext_dependency`). The grep showed `acquire_extension_bytes(&source, ...)` takes an `&ExtensionDependencySource`; pass `&dep.source` (adjust the field name if the struct names it differently, e.g. `dep.source` vs `dep.src`). Also confirm `lookup_ext_dependency`, `acquire_extension_bytes` are reachable from this module (same file) — they are private fns in `ext_resolver.rs`.

- [ ] **Step 4: Run test + clippy**

Run: `cargo test -p greentic-pack --lib cli::ext_resolver::describe_tests -- --nocapture && cargo clippy -p greentic-pack --all-targets -- -D warnings`
Expected: PASS; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/cli/ext_resolver.rs
git commit -m "feat(ext-resolver): read describe.json + resolve agent tool secret reqs"
```

---

### Task 7: Hook the generator into `build.rs::run()` + integration test

**Files:**
- Modify: `crates/packc/src/build.rs` (the `if let Some(gtpack_out)` block, around lines 300–317)
- Create: `crates/packc/tests/setup_gen_build.rs`

**Interfaces:**
- Consumes: `crate::setup_gen::{generate, SecretRequirementOut}`, `crate::cli::ext_resolver::resolve_agent_tool_requirements`, existing `config`, `secret_requirements` (component-derived, `Vec<greentic_types::SecretRequirement>`), `opts.pack_dir`, `opts.dev`, `AssetFile`, `build.assets`.

- [ ] **Step 1: Write the failing integration test**

Create `crates/packc/tests/setup_gen_build.rs`. Build a minimal application pack with one agent + a local `file://` tool extension fixture, then assert the produced `.gtpack` contains `assets/setup.yaml` with the LLM + tool questions.

```rust
//! Integration: building an application pack with agents auto-derives the
//! credential setup.yaml + secret-requirements.json into the .gtpack.

use std::io::{Read, Write};
use std::path::Path;

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn make_tavily_gtxpack(dir: &Path) -> std::path::PathBuf {
    let describe = r#"{"contributions":{"tools":[
      {"name":"tavily_search","secret_requirements":[
        {"key":"tavily/api_key","required":true,"description":"Tavily web-search API key.","format":"text"}]}
    ]}}"#;
    let path = dir.join("greentic.tavily.gtxpack");
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        zip.start_file("describe.json", zip::write::FileOptions::<()>::default()).unwrap();
        zip.write_all(describe.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    std::fs::write(&path, buf).unwrap();
    path
}

fn read_zip_entry(gtpack: &Path, name: &str) -> Option<String> {
    let bytes = std::fs::read(gtpack).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = archive.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

#[test]
fn build_derives_setup_yaml_for_agent_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("pack");
    let gtx = make_tavily_gtxpack(tmp.path());

    write(&pack.join("pack.yaml"), &format!(r#"pack_id: demo
version: 0.1.0
kind: application
publisher: Test
components: []
dependencies: []
flows: []
agents:
  a:
    agent_id: a
    llm: {{ provider: deepseek, model: deepseek-chat, credential_ref: deepseek }}
    tools:
      - {{ extension_id: greentic.tavily, tool_name: tavily_search }}
"#));
    write(&pack.join("pack.extensions.json"), &format!(
        r#"{{"extensions":[{{"id":"greentic.tavily","source":{{"reference":"file://{}"}}}}]}}"#,
        gtx.display()
    ));

    // Invoke the build via the library entry point (BuildOptions). Fill in the
    // option fields the same way crates/packc/tests/build_pipeline.rs does
    // (gtpack_out set, dev=false, dry_run=false). See that test for the helper.
    let gtpack_out = tmp.path().join("demo.gtpack");
    greentic_pack_build_helper(&pack, &gtpack_out); // see Step 3 note

    let setup = read_zip_entry(&gtpack_out, "assets/setup.yaml").expect("setup.yaml present");
    let spec: serde_json::Value = serde_yaml_bw::from_str(&setup).unwrap();
    let names: Vec<&str> = spec["questions"].as_array().unwrap()
        .iter().map(|q| q["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"deepseek"));
    assert!(names.contains(&"api_key"));

    let reqs = read_zip_entry(&gtpack_out, "assets/secret-requirements.json").expect("requirements present");
    assert!(reqs.contains("llm/deepseek"));
    assert!(reqs.contains("tavily/api_key"));
}
```

> The `greentic_pack_build_helper` stands in for however `crates/packc/tests/build_pipeline.rs` constructs `BuildOptions` and calls `build::run` (it is `async`; use the same runtime/helper that test uses). Copy that test's setup verbatim rather than inventing a new one. If `build::run` is only reachable via the CLI, drive it with `assert_cmd` exactly as `build_pipeline.rs` does.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-pack --test setup_gen_build -- --nocapture`
Expected: FAIL — `assets/setup.yaml` not present in the gtpack (generator not wired yet).

- [ ] **Step 3: Wire the generator into `run()`**

Replace the existing dev-gated secret-requirements block in `crates/packc/src/build.rs` (lines ~301–310):

```rust
        let mut build = build;
        if opts.dev && !secret_requirements.is_empty() {
            let logical = "secret-requirements.json".to_string();
            let req_path =
                write_secret_requirements_file(&opts.pack_dir, &secret_requirements, &logical)?;
            build.assets.push(AssetFile { logical_path: logical, source: req_path });
        }
```

with a unified block:

```rust
        let mut build = build;

        // Auto-derive the credential setup form from agents + tool extensions.
        if !config.agents.is_empty() {
            let component_reqs: Vec<crate::setup_gen::SecretRequirementOut> = secret_requirements
                .iter()
                .map(|r| crate::setup_gen::SecretRequirementOut {
                    key: r.key.clone(),
                    required: r.required,
                    description: r.description.clone(),
                })
                .collect();

            let cache_dir = opts.pack_dir.join(".packc");
            let tool_reqs = crate::cli::ext_resolver::resolve_agent_tool_requirements(
                &opts.pack_dir,
                &config.agents,
                &cache_dir,
                /* offline */ !opts.bundle && opts.dry_run, // use the build's offline policy; see note
            )?;

            if let Some(gen) =
                crate::setup_gen::generate(&config.pack_id, &config.agents, &tool_reqs, &component_reqs)?
            {
                // Override: a hand-authored assets/setup.yaml in the pack source wins.
                let hand_authored = opts.pack_dir.join("assets/setup.yaml").exists();
                if !hand_authored {
                    let p = opts.pack_dir.join(".packc/setup.yaml");
                    write_bytes(&p, gen.setup_yaml.as_bytes())?;
                    build.assets.push(AssetFile {
                        logical_path: "setup.yaml".to_string(),
                        source: p,
                    });
                }
                let rp = opts.pack_dir.join(".packc/secret-requirements.json");
                write_bytes(&rp, gen.secret_requirements_json.as_bytes())?;
                build.assets.push(AssetFile {
                    logical_path: "secret-requirements.json".to_string(),
                    source: rp,
                });
            }
        } else if opts.dev && !secret_requirements.is_empty() {
            // No agents: preserve the existing dev-only component requirements file.
            let logical = "secret-requirements.json".to_string();
            let req_path =
                write_secret_requirements_file(&opts.pack_dir, &secret_requirements, &logical)?;
            build.assets.push(AssetFile { logical_path: logical, source: req_path });
        }
```

> Confirm field names against the codebase: `config.pack_id` (PackConfig has `pub pack_id: String`), `config.agents` (`BTreeMap<String, serde_json::Value>`), and the field set on `greentic_types::SecretRequirement` (the `secret_requirements` element type) — adjust `r.key/r.required/r.description` to its actual fields. For the `offline` argument, reuse whatever offline/online flag `BuildOptions` already exposes (the same one `collect_lock_component_artifacts`/the dist resolver uses); do not invent a new policy. If `assets/setup.yaml` already lands in `build.assets` via the normal asset-collection path (a hand-authored one), the `hand_authored` check above prevents a duplicate; verify there is no double-insert of the `setup.yaml` logical path in `package_gtpack` (it dedupes by `written_paths`, first wins).

Ensure `crate::cli::ext_resolver` and `crate::setup_gen` are reachable from `build.rs` (add `use` or fully-qualify as shown).

- [ ] **Step 4: Run the integration test + full crate tests + clippy**

Run: `cargo test -p greentic-pack --test setup_gen_build -- --nocapture && cargo test -p greentic-pack --locked && cargo clippy -p greentic-pack --all-targets -- -D warnings`
Expected: new test PASS; no regressions in existing build tests; clippy clean. If a golden/canonical gtpack test changed because non-agent packs are unaffected, confirm the diff is limited to agent packs only.

- [ ] **Step 5: Commit**

```bash
git add crates/packc/src/build.rs crates/packc/tests/setup_gen_build.rs
git commit -m "feat(build): auto-derive credential setup.yaml + secret-requirements for agent packs"
```

---

### Task 8: greentic-demo parity — drop hand-authored setup, prove the generator reproduces it

**Files (repo: `greentic-demo`):**
- Delete: `crates/agentic-research-tavily-demo/assets/setup.yaml`
- Modify: `crates/agentic-research-tavily-demo/build-answer.json` — remove the `assets/setup.yaml` and `assets/secret-requirements.json` entries from `pack_overlay.files[]`
- Rebuild: `demos/agentic-research-tavily-demo.gtpack`

**Prerequisite:** a `greentic-pack` / `gtc` binary built from the Task 1–7 branch must be on `PATH` (the demo build shells out to it via `scripts/package_demos.sh`).

- [ ] **Step 1: Capture the current hand-authored form (baseline)**

```bash
cd greentic-demo
python3 - <<'PY'
import zipfile, sys
z = zipfile.ZipFile("demos/agentic-research-tavily-demo.gtpack")
open("/tmp/baseline-setup.yaml","wb").write(z.read("assets/setup.yaml"))
print("baseline questions captured")
PY
```

- [ ] **Step 2: Remove the hand-authored assets**

```bash
git rm crates/agentic-research-tavily-demo/assets/setup.yaml
```

Edit `crates/agentic-research-tavily-demo/build-answer.json`: delete the two `pack_overlay.files[]` entries whose `path` is `assets/setup.yaml` and `assets/secret-requirements.json`.

- [ ] **Step 3: Rebuild the gtpack with the new generator**

```bash
# Uses the Task 1–7 greentic-pack binary on PATH.
scripts/package_demos.sh   # or the project's documented single-demo rebuild
```

- [ ] **Step 4: Assert semantic parity**

```bash
python3 - <<'PY'
import zipfile, sys
try:
    import yaml
except ImportError:
    sys.exit("pip install pyyaml to run the parity check")
z = zipfile.ZipFile("demos/agentic-research-tavily-demo.gtpack")
gen = yaml.safe_load(z.read("assets/setup.yaml"))
base = yaml.safe_load(open("/tmp/baseline-setup.yaml"))
def norm(spec):
    return sorted(
        {k: q.get(k) for k in ("name","group","secret","required")}
        for q in spec["questions"]
    , key=lambda d: d["name"])
assert norm(gen) == norm(base), f"parity mismatch:\nGEN {norm(gen)}\nBASE {norm(base)}"
names = {q["name"] for q in gen["questions"]}
assert {"deepseek","api_key"} <= names, names
print("PARITY OK:", sorted(names))
PY
```

Expected: `PARITY OK: ['api_key', 'deepseek']` — the generated form carries the same questions (name/group/secret/required) the hand-authored file did.

- [ ] **Step 5: Commit**

```bash
git add crates/agentic-research-tavily-demo/build-answer.json demos/agentic-research-tavily-demo.gtpack
git rm --cached crates/agentic-research-tavily-demo/assets/setup.yaml 2>/dev/null || true
git commit -m "feat(tavily-demo): rely on auto-derived credential setup form (drop hand-authored setup.yaml)"
```

---

## Self-Review

**Spec coverage:**
- Hook in build.rs before package_gtpack → Task 7. ✓
- Derive LLM (every provider) + tool secrets → Tasks 3,4,6. ✓
- Overlay + drift-test for all `ProviderKind` → Tasks 3,5. ✓
- Tool describe via minimal local struct → Task 2. ✓
- Name→secret mapping + collision → Task 4. ✓
- Override (hand-authored wins) → Task 7 (`hand_authored` check) + Task 8 (demo drops it). ✓
- Error on unresolvable extension → Task 6 (`?` on `lookup_ext_dependency`/`acquire_extension_bytes`). ✓
- Tests: unit (1–4), drift (5), integration (7), parity/e2e (8). ✓

**Open confirmations the implementer must resolve against live code (flagged inline):**
- `greentic_types::SecretRequirement` field names (Task 7 conversion).
- `ExtensionDependency` source field name for `acquire_extension_bytes` (Task 6).
- `BuildOptions` offline flag to pass to `resolve_agent_tool_requirements` (Task 7).
- The exact `BuildOptions`/`build::run` construction in `tests/build_pipeline.rs` to reuse for Task 7's integration test.
- Whether `greentic-llm` is a `[workspace.dependencies]` entry (Task 5).

**Type consistency:** `SetupQuestionOut`/`SecretRequirementOut`/`ToolSecretReq`/`GeneratedSetup`/`generate`/`llm_overlay`/`extract_tool_secret_requirements`/`read_describe_from_gtxpack`/`resolve_agent_tool_requirements` are referenced with identical signatures across tasks. ✓

**No placeholders:** every code step carries complete code; the few "confirm against live code" notes are explicit field-name verifications, not deferred design.
