use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Parser, ValueEnum};
use greentic_pack::{SigningPolicy, VerifyReport, builder::PackManifest, open_pack};
use serde_json::json;

#[derive(Parser, Debug)]
#[command(
    name = "gtpack-inspect",
    version,
    about = "Inspect Greentic pack archives"
)]
struct Args {
    /// Path to the .gtpack file
    #[arg(value_name = "FILE")]
    path: PathBuf,

    /// Signature policy to enforce
    #[arg(long, value_enum, default_value_t = PolicyArg::Devok)]
    policy: PolicyArg,

    /// Emit JSON output
    #[arg(long)]
    json: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum PolicyArg {
    Devok,
    Strict,
}

impl From<PolicyArg> for SigningPolicy {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::Devok => SigningPolicy::DevOk,
            PolicyArg::Strict => SigningPolicy::Strict,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let load = open_pack(&args.path, args.policy.into()).map_err(|err| anyhow!(err.message))?;

    if args.json {
        print_json(&load.manifest, &load.report, &load.sbom)?;
    } else {
        print_human(&load.manifest, &load.report, &load.sbom);
    }

    Ok(())
}

fn print_human(
    manifest: &PackManifest,
    report: &VerifyReport,
    sbom: &[greentic_pack::builder::SbomEntry],
) {
    println!(
        "Pack: {} ({})",
        manifest.meta.pack_id, manifest.meta.version
    );
    println!("Flows: {}", manifest.flows.len());
    println!("Components: {}", manifest.components.len());
    println!("SBOM entries: {}", sbom.len());
    println!("Signature OK: {}", report.signature_ok);
    println!("SBOM OK: {}", report.sbom_ok);
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning);
        }
    }
}

fn print_json(
    manifest: &PackManifest,
    report: &VerifyReport,
    sbom: &[greentic_pack::builder::SbomEntry],
) -> Result<()> {
    let payload = json!({
        "manifest": {
            "pack_id": manifest.meta.pack_id,
            "version": manifest.meta.version,
            "flows": manifest.flows.len(),
            "components": manifest.components.len(),
        },
        "report": {
            "signature_ok": report.signature_ok,
            "sbom_ok": report.sbom_ok,
            "warnings": report.warnings,
        },
        "sbom": sbom,
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
