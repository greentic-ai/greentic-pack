//! AgenticWorker (`dw-application`) pack emission: designer-faithful sidecars
//! and the store describe document. All output mirrors greentic-designer's
//! `orchestrate::pack_via_packc` / `orchestrate::dw_publish` byte-for-byte.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

/// Serialize the `agents:` map to the `dw-agents.json` sidecar bytes
/// (bare `{ "<agent_id>": <AgentConfig> }` JSON). Returns `None` when the map is
/// empty so non-agent packs produce byte-identical archives.
pub fn dw_agents_sidecar_bytes(
    agents: &BTreeMap<String, serde_json::Value>,
) -> Result<Option<Vec<u8>>> {
    if agents.is_empty() {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(agents).context("serialize dw-agents.json")?;
    Ok(Some(bytes))
}
