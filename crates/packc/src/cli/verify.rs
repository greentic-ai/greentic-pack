#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use serde_json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::manifest::PackSignature;
use crate::signing::{VerifyOptions, verify_pack_dir};

#[derive(Debug, Parser)]
pub struct VerifyArgs {
    /// Path to the pack directory containing pack.toml
    #[arg(long = "pack", value_name = "DIR")]
    pub pack: PathBuf,

    /// Public key to verify against (PKCS#8 PEM)
    #[arg(long = "pub", value_name = "FILE")]
    pub public_key: Option<PathBuf>,

    /// Allow verification to succeed when no signature is present
    #[arg(long = "allow-unsigned")]
    pub allow_unsigned: bool,
}

pub fn handle(args: VerifyArgs, json: bool) -> Result<()> {
    let VerifyArgs {
        pack,
        public_key,
        allow_unsigned,
    } = args;

    let pack_dir = pack
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", pack.display()))?;

    let public_key_pem = match public_key {
        Some(path) => Some(
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        ),
        None => None,
    };

    let signature = verify_pack_dir(
        &pack_dir,
        VerifyOptions {
            public_key_pem: public_key_pem.as_deref(),
            allow_unsigned,
        },
    )?;

    if json {
        print_json(&signature, &pack_dir)?;
    } else {
        print_human(&signature, &pack_dir)?;
    }

    Ok(())
}

fn print_human(signature: &PackSignature, pack_dir: &Path) -> Result<()> {
    if signature.alg == "none" {
        println!(
            "verified pack manifest in {} (unsigned manifest accepted)",
            pack_dir.display()
        );
        return Ok(());
    }

    let created_at = signature
        .created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| signature.created_at.to_string());

    println!(
        "verified pack manifest in {}\n  key_id: {}\n  digest: {}\n  created_at: {}",
        pack_dir.display(),
        signature.key_id,
        signature.digest,
        created_at
    );

    Ok(())
}

fn print_json(signature: &PackSignature, pack_dir: &Path) -> Result<()> {
    #[derive(Serialize)]
    struct Payload<'a> {
        pack: &'a Path,
        alg: &'a str,
        key_id: &'a str,
        digest: &'a str,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
        sig: &'a str,
    }

    let payload = Payload {
        pack: pack_dir,
        alg: &signature.alg,
        key_id: &signature.key_id,
        digest: &signature.digest,
        created_at: signature.created_at,
        sig: &signature.sig,
    };

    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}
