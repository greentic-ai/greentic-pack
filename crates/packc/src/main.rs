mod build;
mod embed;
mod flows;
mod manifest;
mod sbom;
mod templates;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

fn main() -> Result<()> {
    let cli = Cli::parse();

    let env_filter = std::env::var("PACKC_LOG").unwrap_or_else(|_| cli.verbosity.clone());

    fmt()
        .with_env_filter(EnvFilter::new(env_filter))
        .with_writer(std::io::stderr)
        .init();

    build::run(&build::BuildOptions::from(cli))?;

    Ok(())
}

#[derive(Debug, Parser)]
#[command(name = "packc", about = "Greentic pack builder CLI", version)]
pub struct Cli {
    /// Root directory of the pack (must contain pack.yaml)
    #[arg(long = "in", value_name = "DIR")]
    pub input: std::path::PathBuf,

    /// Output path for the built Wasm component
    #[arg(long = "out", value_name = "FILE", default_value = "dist/pack.wasm")]
    pub component_out: std::path::PathBuf,

    /// Output path for the generated manifest (CBOR)
    #[arg(long, value_name = "FILE", default_value = "dist/manifest.cbor")]
    pub manifest: std::path::PathBuf,

    /// Output path for the generated SBOM (CycloneDX JSON)
    #[arg(long, value_name = "FILE", default_value = "dist/sbom.cdx.json")]
    pub sbom: std::path::PathBuf,

    /// Optional override for the generated component data source file
    #[arg(long = "component-data", value_name = "FILE")]
    pub component_data: Option<std::path::PathBuf>,

    /// When set, the command validates input without writing artifacts
    #[arg(long)]
    pub dry_run: bool,

    /// Logging filter (overrides PACKC_LOG)
    #[arg(long = "log", default_value = "info")]
    pub verbosity: String,
}
