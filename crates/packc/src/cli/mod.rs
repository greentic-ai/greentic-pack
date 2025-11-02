#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

use crate::build;

pub mod sign;
pub mod verify;

#[derive(Debug, Parser)]
#[command(name = "packc", about = "Greentic pack builder CLI", version)]
pub struct Cli {
    /// Logging filter (overrides PACKC_LOG)
    #[arg(long = "log", default_value = "info", global = true)]
    pub verbosity: String,

    /// Emit machine-readable JSON output where applicable
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build a pack component and supporting artifacts
    Build(BuildArgs),
    /// Sign a pack manifest using an Ed25519 private key
    Sign(sign::SignArgs),
    /// Verify a pack's manifest signature
    Verify(verify::VerifyArgs),
}

#[derive(Debug, Clone, Parser)]
pub struct BuildArgs {
    /// Root directory of the pack (must contain pack.yaml)
    #[arg(long = "in", value_name = "DIR")]
    pub input: PathBuf,

    /// Output path for the built Wasm component
    #[arg(long = "out", value_name = "FILE", default_value = "dist/pack.wasm")]
    pub component_out: PathBuf,

    /// Output path for the generated manifest (CBOR)
    #[arg(long, value_name = "FILE", default_value = "dist/manifest.cbor")]
    pub manifest: PathBuf,

    /// Output path for the generated SBOM (CycloneDX JSON)
    #[arg(long, value_name = "FILE", default_value = "dist/sbom.cdx.json")]
    pub sbom: PathBuf,

    /// Optional override for the generated component data source file
    #[arg(long = "component-data", value_name = "FILE")]
    pub component_data: Option<PathBuf>,

    /// When set, the command validates input without writing artifacts
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    let env_filter = std::env::var("PACKC_LOG").unwrap_or_else(|_| cli.verbosity.clone());

    let _ = fmt()
        .with_env_filter(EnvFilter::new(env_filter))
        .with_writer(std::io::stderr)
        .try_init();

    match cli.command {
        Command::Build(args) => build::run(&build::BuildOptions::from(args))?,
        Command::Sign(args) => sign::handle(args, cli.json)?,
        Command::Verify(args) => verify::handle(args, cli.json)?,
    }

    Ok(())
}
