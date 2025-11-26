use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use schemars::schema_for;

use packc::manifest::PackSpec;

fn main() -> Result<()> {
    let schema = schema_for!(PackSpec);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    fs::create_dir_all(&dir).context("failed to create schema directory")?;

    let json_path = dir.join("pack.v1.schema.json");
    let yaml_path = dir.join("pack.v1.schema.yaml");

    let json = serde_json::to_string_pretty(&schema)?;
    fs::write(&json_path, json)
        .with_context(|| format!("failed to write {}", json_path.display()))?;

    let yaml = serde_yaml_bw::to_string(&schema)?;
    fs::write(&yaml_path, yaml)
        .with_context(|| format!("failed to write {}", yaml_path.display()))?;

    Ok(())
}
