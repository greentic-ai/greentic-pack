#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use serde_json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::manifest::{self, PackSignature};
use crate::signing::signer;

#[derive(Debug, Parser)]
pub struct SignArgs {
    /// Path to the pack directory containing pack.toml
    #[arg(long = "pack", value_name = "DIR")]
    pub pack: PathBuf,

    /// Ed25519 private key in PKCS#8 PEM format
    #[arg(long = "key", value_name = "FILE")]
    pub key: PathBuf,

    /// Optional override for the signature key identifier
    #[arg(long = "kid", value_name = "ID")]
    pub key_id: Option<String>,

    /// When set, writes the updated manifest to the provided path instead of in-place
    #[arg(long = "out", value_name = "FILE")]
    pub out: Option<PathBuf>,
}

pub fn handle(args: SignArgs, json: bool) -> Result<()> {
    let SignArgs {
        pack,
        key,
        key_id,
        out,
    } = args;

    let pack_dir = pack
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", pack.display()))?;

    let private_key =
        fs::read_to_string(&key).with_context(|| format!("failed to read {}", key.display()))?;

    let target_manifest_path = match &out {
        Some(path) => path.clone(),
        None => manifest::manifest_path(&pack_dir)?,
    };

    let outcome = signer::sign_pack(&pack_dir, &private_key, key_id.as_deref())?;

    manifest::write_signature(&pack_dir, &outcome.signature, out.as_deref())?;

    if json {
        print_json(&outcome.signature, &target_manifest_path)?;
    } else {
        print_human(&outcome.signature, &target_manifest_path)?;
    }

    Ok(())
}

fn print_human(signature: &PackSignature, manifest_path: &Path) -> Result<()> {
    let created_at = signature
        .created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| signature.created_at.to_string());

    println!(
        "signed pack manifest\n  manifest: {}\n  key_id: {}\n  digest: {}\n  created_at: {}",
        manifest_path.display(),
        signature.key_id,
        signature.digest,
        created_at
    );

    Ok(())
}

fn print_json(signature: &PackSignature, manifest_path: &Path) -> Result<()> {
    #[derive(Serialize)]
    struct Payload<'a> {
        manifest: &'a Path,
        key_id: &'a str,
        alg: &'a str,
        digest: &'a str,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
        sig: &'a str,
    }

    let payload = Payload {
        manifest: manifest_path,
        key_id: &signature.key_id,
        alg: &signature.alg,
        digest: &signature.digest,
        created_at: signature.created_at,
        sig: &signature.sig,
    };

    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}
