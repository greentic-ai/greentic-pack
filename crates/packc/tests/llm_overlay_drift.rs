//! Guards against greentic-llm gaining a provider the overlay author has not
//! triaged. Every `ProviderKind` must be either curated in `llm_overlay` or
//! listed here as intentionally minimal (functional question, no polish).

use greentic_llm::capabilities::ProviderKind;
use packc::setup_gen::llm_overlay;

/// Providers we knowingly ship without curated docs_url/placeholder. Adding a
/// provider to greentic-llm forces a choice: curate it in `llm_overlay`, or add
/// its id here.
const MINIMAL_OK: &[&str] = &[
    "ollama",
    "llamafile",
    "bedrock",
    "azure",
    "azure-foundry",
    "huggingface",
    "together",
    "moonshot",
    "minimax",
    "hyperbolic",
    "galadriel",
    "mira",
    "zai",
    "xiaomimimo",
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
