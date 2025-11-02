#![forbid(unsafe_code)]

pub mod build;
pub mod cli;
pub mod embed;
pub mod flows;
pub mod manifest;
pub mod sbom;
pub mod signing;
pub mod templates;

pub use cli::BuildArgs;
pub use manifest::PackSignature;
pub use signing::{sign_pack_dir, verify_pack_dir, VerificationError, VerifyOptions};
