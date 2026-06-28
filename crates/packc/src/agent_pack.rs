//! AgenticWorker (`dw-application`) pack emission: designer-faithful sidecars
//! and the store describe document. All output mirrors greentic-designer's
//! `orchestrate::pack_via_packc` / `orchestrate::dw_publish` byte-for-byte.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use greentic_types::secrets::SecretRequirement;
use serde::Serialize;
use serde_json::{Value, json};

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

/// Mirror of greentic-designer `store::secrets_policy::SecretSharePolicy`.
///
/// Uses `#[serde(tag = "policy", rename_all = "kebab-case")]` to emit the
/// policy as an inlined tag field, matching the designer's wire format exactly.
/// Only `Serialize` is derived — packc emits this sidecar but never reads it back.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "policy", rename_all = "kebab-case")]
pub enum SecretSharePolicy {
    /// Installer must supply their own value; no publisher value ever ships.
    ByoRequired,
    /// Publisher may supply an overridable default (referenced, never inlined).
    DefaultOverridable {
        /// Reference to a publisher secret marked shareable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_ref: Option<String>,
    },
}

/// One requirement: the canonical `SecretRequirement` flattened with its share
/// policy tag. Mirrors `SecretPolicyEntry` in the designer's `store::secrets_policy`.
///
/// `#[serde(flatten)]` on both fields produces the designer's byte-faithful wire
/// format: `{"key":"...","required":true,"policy":"byo-required"}`.
#[derive(Debug, Clone, Serialize)]
pub struct SecretPolicyEntry {
    /// Canonical requirement — key/required and optional description/scope/format/schema/examples.
    #[serde(flatten)]
    pub requirement: SecretRequirement,
    /// Hybrid share policy tag (inlined via `#[serde(tag)]` on the enum).
    #[serde(flatten)]
    pub share: SecretSharePolicy,
}

/// The `secrets-policy.json` document embedded in a published `dw-application` `.gtpack`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AgenticWorkerSecretsPolicy {
    /// One entry per secret the worker needs.
    pub requirements: Vec<SecretPolicyEntry>,
}

/// Build the `secrets-policy.json` sidecar bytes from the canonical secret
/// requirements the pack declares. All entries are assigned the `byo-required`
/// policy — no publisher value ever ships. Returns `None` when `requirements`
/// is empty so packs with no secrets produce a byte-identical archive.
pub fn secrets_policy_sidecar_bytes(requirements: &[SecretRequirement]) -> Result<Option<Vec<u8>>> {
    if requirements.is_empty() {
        return Ok(None);
    }
    let policy = AgenticWorkerSecretsPolicy {
        requirements: requirements
            .iter()
            .map(|req| SecretPolicyEntry {
                requirement: req.clone(),
                share: SecretSharePolicy::ByoRequired,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&policy).context("serialize secrets-policy.json")?;
    Ok(Some(bytes))
}

/// Inputs for the store describe document. `manifest_sha256` is the
/// lowercase-hex SHA-256 of the FINAL `.gtpack` bytes (== `artifactSha256`).
pub struct DescribeMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub manifest_sha256: String,
}

/// Author a `describe-v2` document for an Agentic Worker. Byte-faithful to
/// greentic-designer `orchestrate::dw_publish::agentic_worker_describe`.
pub fn agentic_worker_describe(meta: &DescribeMeta) -> Value {
    json!({
        "apiVersion": "greentic.ai/v2",
        "kind": "AgenticWorker",
        "compat": {
            "min_designer_version": ">=1.2.0",
            "min_runner_version": "^0.12.0",
            "contract_version": "1.2.0"
        },
        "metadata": {
            "id": meta.id,
            "name": meta.name,
            "version": meta.version,
            "summary": meta.summary,
            "author": { "name": "Greentic" },
            "license": "MIT"
        },
        "engine": { "greenticDesigner": ">=1.2.0", "extRuntime": "^1.2.0" },
        "capabilities": { "offered": [], "required": [] },
        "runtime": {
            "memoryLimitMB": 32,
            "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] },
            "components": {
                "worker": {
                    "gtpack": {
                        "file": "worker.wasm",
                        "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                        "pack_id": meta.id,
                        "component_version": meta.version
                    },
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "world": format!("greentic:{}/extension@{}", meta.id, meta.version)
                }
            }
        },
        "contributions": {},
        "manifestSha256": meta.manifest_sha256
    })
}
