use packc::agent_pack::{DescribeMeta, agentic_worker_describe};

#[test]
fn describe_matches_designer_shape() {
    let meta = DescribeMeta {
        id: "alice.greeter-bot".into(),
        name: "Greeter Bot".into(),
        version: "0.1.0".into(),
        summary: "A helpful greeting bot".into(),
        manifest_sha256: "ab".repeat(32),
    };
    let d = agentic_worker_describe(&meta);
    assert_eq!(d["apiVersion"], "greentic.ai/v2");
    assert_eq!(d["kind"], "AgenticWorker");
    assert_eq!(d["metadata"]["id"], "alice.greeter-bot");
    assert_eq!(
        d["runtime"]["components"]["worker"]["world"],
        "greentic:alice.greeter-bot/extension@0.1.0"
    );
    assert_eq!(
        d["runtime"]["components"]["worker"]["sha256"],
        "0".repeat(64)
    );
    assert_eq!(d["manifestSha256"], "ab".repeat(32));
    assert_eq!(d["contributions"], serde_json::json!({}));
}
