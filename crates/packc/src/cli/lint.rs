#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Parser;
use greentic_flow::compile_ygtc_str;
use greentic_pack::validate::oauth_capability_requirement_diagnostics_for_flow;
use tracing::info;

use crate::config::load_pack_config;

#[derive(Debug, Parser)]
pub struct LintArgs {
    /// Root directory of the pack (must contain pack.yaml)
    #[arg(long = "in", value_name = "DIR")]
    pub input: PathBuf,

    /// Allow OCI component refs in extensions to be tag-based (default requires sha256 digest)
    #[arg(long = "allow-oci-tags", default_value_t = false)]
    pub allow_oci_tags: bool,
}

pub fn handle(args: LintArgs, json: bool) -> Result<()> {
    let pack_dir = normalize(args.input);
    info!(path = %pack_dir.display(), "linting pack");

    let cfg = load_pack_config(&pack_dir)?;
    crate::extensions::validate_components_extension(&cfg.extensions, args.allow_oci_tags)?;

    let required_capabilities: BTreeSet<String> = cfg
        .dependencies
        .iter()
        .flat_map(|dep| dep.required_capabilities.iter())
        .map(|cap| cap.trim().to_string())
        .filter(|cap| !cap.is_empty())
        .collect();

    let mut compiled = 0usize;
    let mut oauth_diagnostics = Vec::new();
    for flow in &cfg.flows {
        let src = std::fs::read_to_string(&flow.file)?;
        let compiled_flow = compile_ygtc_str(&src)?;
        oauth_diagnostics.extend(oauth_capability_requirement_diagnostics_for_flow(
            flow.id.as_str(),
            &compiled_flow,
            &required_capabilities,
        ));
        compiled += 1;
    }

    if !oauth_diagnostics.is_empty() {
        let mut message = String::from("OAuth capability requirement checks failed during lint:\n");
        for diag in oauth_diagnostics {
            if let Some(path) = diag.path.as_deref() {
                message.push_str(&format!("- [{}] {}: {}\n", diag.code, path, diag.message));
            } else {
                message.push_str(&format!("- [{}] {}\n", diag.code, diag.message));
            }
            if let Some(hint) = diag.hint.as_deref() {
                message.push_str(&format!("  hint: {hint}\n"));
            }
        }
        bail!(message.trim_end().to_string());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": crate::cli_i18n::t("cli.status.ok"),
                "pack_id": cfg.pack_id,
                "version": cfg.version,
                "flows": compiled,
                "components": cfg.components.len(),
                "dependencies": cfg.dependencies.len(),
            }))?
        );
    } else {
        println!("{}", crate::cli_i18n::t("cli.lint.ok_header"));
        println!(
            "{}",
            crate::cli_i18n::tf("cli.lint.pack", &[&cfg.pack_id, &cfg.version.to_string()])
        );
        println!(
            "{}",
            crate::cli_i18n::tf("cli.lint.flows", &[&compiled.to_string()])
        );
        println!(
            "{}",
            crate::cli_i18n::tf("cli.lint.components", &[&cfg.components.len().to_string()])
        );
        println!(
            "{}",
            crate::cli_i18n::tf(
                "cli.lint.dependencies",
                &[&cfg.dependencies.len().to_string()]
            )
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
