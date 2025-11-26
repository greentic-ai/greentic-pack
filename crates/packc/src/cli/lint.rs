#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use serde_json::json;
use tracing::info;

use crate::{flows, manifest, templates};

#[derive(Debug, Parser)]
pub struct LintArgs {
    /// Root directory of the pack (must contain pack.yaml)
    #[arg(long = "in", value_name = "DIR")]
    pub input: PathBuf,
}

pub fn handle(args: LintArgs, json: bool) -> Result<()> {
    let pack_dir = normalize(args.input);
    info!(path = %pack_dir.display(), "linting pack");

    let spec_bundle = manifest::load_spec(&pack_dir)?;
    let flows = flows::load_flows(&pack_dir, &spec_bundle.spec)?;
    let templates = templates::collect_templates(&pack_dir, &spec_bundle.spec)?;
    let events = spec_bundle
        .spec
        .events
        .as_ref()
        .map(|section| section.providers.len())
        .unwrap_or(0);

    // Building the manifest ensures flow/template metadata is well-formed.
    let _manifest = manifest::build_manifest(&spec_bundle, &flows, &templates);

    if json {
        let payload = json!({
            "status": "ok",
            "pack_id": spec_bundle.spec.id,
            "version": spec_bundle.spec.version,
            "flows": flows.len(),
            "templates": templates.len(),
            "events_providers": events,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "lint ok\n  pack: {}@{}\n  flows: {}\n  templates: {}\n  events.providers: {}",
            spec_bundle.spec.id,
            spec_bundle.spec.version,
            flows.len(),
            templates.len(),
            events
        );
    }

    Ok(())
}

fn normalize(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
