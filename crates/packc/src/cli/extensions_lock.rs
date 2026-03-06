#![forbid(unsafe_code)]

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use greentic_distributor_client::{DistClient, DistOptions};
use tokio::runtime::Handle;

use crate::extension_refs::{
    LockedExtensionDependency, PackExtensionsLockFile, default_extensions_file_path,
    default_extensions_lock_file_path, pin_reference, read_extensions_file,
    write_extensions_lock_file,
};
use crate::runtime::RuntimeContext;

#[derive(Debug, Args)]
pub struct ExtensionsLockArgs {
    /// Pack root directory containing pack.yaml / pack.extensions.json.
    #[arg(long = "in", value_name = "DIR", default_value = ".")]
    pub input: PathBuf,

    /// Override source file path (default: <pack_dir>/pack.extensions.json).
    #[arg(long = "file", value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Override output path (default: <pack_dir>/pack.extensions.lock.json).
    #[arg(long = "out", value_name = "FILE")]
    pub out: Option<PathBuf>,
}

pub async fn handle(
    args: ExtensionsLockArgs,
    runtime: &RuntimeContext,
    emit_path: bool,
) -> Result<()> {
    let pack_dir = args
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve pack dir {}", args.input.display()))?;
    let source_path = resolve_path(
        &pack_dir,
        args.file.as_deref(),
        default_extensions_file_path(&pack_dir),
    );
    let out_path = resolve_path(
        &pack_dir,
        args.out.as_deref(),
        default_extensions_lock_file_path(&pack_dir),
    );

    let source = read_extensions_file(&source_path)?;
    let mut locked = Vec::with_capacity(source.extensions.len());
    let handle = Handle::try_current().context("extension locking requires a Tokio runtime")?;

    for extension in source.extensions {
        let dist = DistClient::new(DistOptions {
            cache_dir: runtime.cache_dir(),
            allow_tags: extension.source.allow_tags,
            offline: runtime.network_policy().is_offline(),
            allow_insecure_local_http: false,
            ..DistOptions::default()
        });
        let resolved = block_on(&handle, dist.resolve_ref(&extension.source.reference))
            .with_context(|| format!("resolve extension ref {}", extension.source.reference))?;
        let digest = if resolved.resolved_digest.is_empty() {
            resolved.digest.clone()
        } else {
            resolved.resolved_digest.clone()
        };
        let resolved_ref = pin_reference(&extension.source.reference, &digest);
        locked.push(LockedExtensionDependency {
            id: extension.id,
            role: extension.role,
            source_ref: extension.source.reference,
            resolved_ref,
            digest,
            media_type: resolved.content_type.clone(),
            size_bytes: resolved.content_length,
        });
    }

    let lock = PackExtensionsLockFile::new(locked);
    write_extensions_lock_file(&out_path, &lock)?;
    if emit_path {
        eprintln!(
            "{}",
            crate::cli_i18n::tf("cli.common.wrote_path", &[&out_path.display().to_string()])
        );
    }
    Ok(())
}

fn resolve_path(pack_dir: &Path, override_path: Option<&Path>, default: PathBuf) -> PathBuf {
    match override_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => pack_dir.join(path),
        None => default,
    }
}

fn block_on<F, T, E>(handle: &Handle, fut: F) -> std::result::Result<T, E>
where
    F: Future<Output = std::result::Result<T, E>>,
{
    tokio::task::block_in_place(|| handle.block_on(fut))
}

trait OfflineCheck {
    fn is_offline(&self) -> bool;
}

impl OfflineCheck for crate::runtime::NetworkPolicy {
    fn is_offline(&self) -> bool {
        matches!(self, crate::runtime::NetworkPolicy::Offline)
    }
}
