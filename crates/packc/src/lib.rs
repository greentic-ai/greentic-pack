#![forbid(unsafe_code)]

pub mod build;
pub mod cli;
pub mod embed;
pub mod flows;
pub mod manifest;
pub mod mcp;
pub mod new;
pub mod sbom;
pub mod signing;
pub mod telemetry;
pub mod templates;

pub use cli::BuildArgs;
pub use manifest::PackSignature;
pub use signing::{VerificationError, VerifyOptions, sign_pack_dir, verify_pack_dir};
