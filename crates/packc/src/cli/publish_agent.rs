use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Args;

#[derive(Debug, Args)]
pub struct PublishAgentArgs {
    /// Path to the built dw-application .gtpack.
    #[arg(value_name = "PACK")]
    pub pack: PathBuf,
    /// Store base URL (default: $GREENTIC_STORE_URL or https://store.greentic.cloud).
    #[arg(long = "store-url")]
    pub store_url: Option<String>,
    /// Bearer token (default: $GREENTIC_STORE_TOKEN).
    #[arg(long = "token")]
    pub token: Option<String>,
    /// Store namespace id (e.g. "<handle>.<pack_id>").
    #[arg(long = "id")]
    pub id: String,
    /// Display name.
    #[arg(long = "name")]
    pub name: String,
    /// Version.
    #[arg(long = "version")]
    pub version: String,
    /// One-line summary.
    #[arg(long = "summary", default_value = "")]
    pub summary: String,
}

pub async fn run(args: PublishAgentArgs) -> Result<()> {
    let store_url = args
        .store_url
        .or_else(|| std::env::var("GREENTIC_STORE_URL").ok())
        .unwrap_or_else(|| "https://store.greentic.cloud".to_string());
    let token = args
        .token
        .or_else(|| std::env::var("GREENTIC_STORE_TOKEN").ok())
        .ok_or_else(|| anyhow!("missing token: pass --token or set GREENTIC_STORE_TOKEN"))?;
    let pack_bytes =
        std::fs::read(&args.pack).with_context(|| format!("read {}", args.pack.display()))?;
    let metadata = crate::store_client::publish_metadata(
        &pack_bytes,
        &args.id,
        &args.name,
        &args.version,
        &args.summary,
    )?;
    let resp =
        crate::store_client::publish_agentic_worker(&store_url, &token, &metadata, pack_bytes)
            .await?;
    println!("published: {}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}
