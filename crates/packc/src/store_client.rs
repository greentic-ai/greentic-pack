//! Minimal store client for publishing AgenticWorker packs. Mirrors
//! greentic-designer `store::client::publish_agentic_worker`.

use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};

use crate::agent_pack::{DescribeMeta, agentic_worker_describe};

/// Build the `{describe, artifactSha256}` upload metadata for a pack. The
/// describe's `manifestSha256` and the top-level `artifactSha256` are BOTH
/// the lowercase-hex sha256 of `pack_bytes` (the final .gtpack).
pub fn publish_metadata(
    pack_bytes: &[u8],
    id: &str,
    name: &str,
    version: &str,
    summary: &str,
) -> Result<serde_json::Value> {
    let sha = hex::encode(Sha256::digest(pack_bytes));
    let describe = agentic_worker_describe(&DescribeMeta {
        id: id.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        summary: summary.to_string(),
        manifest_sha256: sha.clone(),
    });
    Ok(serde_json::json!({ "describe": describe, "artifactSha256": sha }))
}

/// POST the pack to `{store}/api/v1/agentic-workers` (multipart
/// metadata+artifact, Bearer token). `409` is surfaced as a non-fatal
/// "already published" message by the caller.
pub async fn publish_agentic_worker(
    store_url: &str,
    token: &str,
    metadata: &serde_json::Value,
    pack_bytes: Vec<u8>,
) -> Result<serde_json::Value> {
    let url = format!("{}/api/v1/agentic-workers", store_url.trim_end_matches('/'));
    let metadata_str = serde_json::to_string(metadata).context("encode metadata")?;
    let part = reqwest::multipart::Part::bytes(pack_bytes)
        .file_name("artifact.gtpack")
        .mime_str("application/zip")?;
    let form = reqwest::multipart::Form::new()
        .text("metadata", metadata_str)
        .part("artifact", part);
    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .context("POST agentic-workers")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::CONFLICT {
        return Err(anyhow!("already published (409): {body}"));
    }
    if !status.is_success() {
        return Err(anyhow!("store returned {}: {body}", status.as_u16()));
    }
    Ok(serde_json::from_str(&body).unwrap_or(serde_json::json!({ "raw": body })))
}
