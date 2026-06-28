use greentic_types::secrets::{SecretKey, SecretRequirement};
use serde_json::Value;

fn req(key: &str) -> SecretRequirement {
    let mut r = SecretRequirement::default();
    r.key = SecretKey::new(key).expect("valid key");
    r.required = true;
    r
}

#[test]
fn secrets_policy_is_byo_required_list() {
    let reqs = vec![req("llm/deepseek"), req("tavily/api_key")];
    let bytes = packc::agent_pack::secrets_policy_sidecar_bytes(&reqs)
        .unwrap()
        .expect("non-empty");
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let entries = v["requirements"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["key"], "llm/deepseek");
    assert_eq!(entries[0]["required"], true); // present + true (canonical never skips)
    assert_eq!(entries[0]["policy"], "byo-required"); // flattened share tag
}

#[test]
fn secrets_policy_empty_is_none() {
    assert!(
        packc::agent_pack::secrets_policy_sidecar_bytes(&[])
            .unwrap()
            .is_none()
    );
}

/// Golden compare: the serialized entry for a single minimal byo-required
/// requirement must exactly match the designer's flattened SecretPolicyEntry
/// output. Both sides flatten the identical canonical SecretRequirement +
/// SecretSharePolicy tag layer, so byte equality is expected.
#[test]
fn secrets_policy_golden_byo_required_entry() {
    let reqs = vec![req("llm/deepseek")];
    let bytes = packc::agent_pack::secrets_policy_sidecar_bytes(&reqs)
        .unwrap()
        .expect("non-empty");
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let entry = &v["requirements"][0];
    // Minimal requirement: key + required=true (canonical never skips required),
    // plus the policy tag from SecretSharePolicy::ByoRequired (kebab-case).
    let expected: Value = serde_json::json!({
        "key": "llm/deepseek",
        "required": true,
        "policy": "byo-required"
    });
    assert_eq!(
        entry,
        &expected,
        "golden entry mismatch — actual bytes: {}",
        String::from_utf8_lossy(&bytes)
    );
}
