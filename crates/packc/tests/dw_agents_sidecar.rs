use std::collections::BTreeMap;

use serde_json::json;

#[test]
fn dw_agents_sidecar_is_bare_map_json() {
    let mut agents = BTreeMap::new();
    agents.insert(
        "greeter".to_string(),
        json!({
            "agent_id": "greeter",
            "system_prompt": "You are a helpful greeter.",
            "tools": [],
            "llm": { "provider": "openai", "model": "gpt-4o-mini" }
        }),
    );
    let bytes = packc::agent_pack::dw_agents_sidecar_bytes(&agents)
        .unwrap()
        .expect("non-empty map yields Some");
    // Faithful shape: a bare object keyed by agent_id; round-trips into the
    // runtime's BTreeMap<String, AgentConfig> (greentic_aw_runtime).
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("deserialize as Value");
    assert!(parsed.is_object(), "sidecar must be a JSON object");
    assert_eq!(
        parsed["greeter"]["agent_id"], "greeter",
        "agent_id must be preserved under the greeter key"
    );
    assert_eq!(
        parsed["greeter"]["llm"]["model"], "gpt-4o-mini",
        "llm.model must be preserved"
    );
}

#[test]
fn dw_agents_sidecar_empty_is_none() {
    let agents = BTreeMap::new();
    assert!(
        packc::agent_pack::dw_agents_sidecar_bytes(&agents)
            .unwrap()
            .is_none()
    );
}
