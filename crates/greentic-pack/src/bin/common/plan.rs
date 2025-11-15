use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use greentic_pack::builder::PackManifest;
use greentic_pack::plan::infer_base_deployment_plan;
use greentic_pack::reader::{SigningPolicy, open_pack};
use greentic_types::component::ComponentManifest;
use greentic_types::{EnvId, TenantCtx, TenantId};
use tempfile::TempDir;
use zip::ZipArchive;

use super::PlanArgs;

const PACKC_ENV: &str = "GREENTIC_PACK_PLAN_PACKC";

pub fn run(args: &PlanArgs) -> Result<()> {
    let (temp, pack_path) = materialize_pack_path(&args.input, args.verbose)?;
    let tenant_ctx = build_tenant_ctx(&args.environment, &args.tenant)?;
    let plan = plan_for_pack(&pack_path, &tenant_ctx, &args.environment)?;

    if args.json {
        println!("{}", serde_json::to_string(&plan)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    }

    drop(temp);
    Ok(())
}

fn plan_for_pack(
    path: &Path,
    tenant: &TenantCtx,
    environment: &str,
) -> Result<greentic_types::deployment::DeploymentPlan> {
    let load = open_pack(path, SigningPolicy::DevOk).map_err(|err| anyhow!(err.message))?;
    let connectors = load.manifest.meta.annotations.get("connectors");
    let components = load_component_manifests(path, &load.manifest)?;

    Ok(infer_base_deployment_plan(
        &load.manifest.meta,
        &load.manifest.flows,
        connectors,
        &components,
        tenant,
        environment,
    ))
}

fn build_tenant_ctx(environment: &str, tenant: &str) -> Result<TenantCtx> {
    let env_id = EnvId::from_str(environment)
        .with_context(|| format!("invalid environment id `{}`", environment))?;
    let tenant_id =
        TenantId::from_str(tenant).with_context(|| format!("invalid tenant id `{}`", tenant))?;
    Ok(TenantCtx::new(env_id, tenant_id))
}

fn materialize_pack_path(input: &Path, verbose: bool) -> Result<(Option<TempDir>, PathBuf)> {
    let metadata =
        fs::metadata(input).with_context(|| format!("unable to read input {}", input.display()))?;
    if metadata.is_file() {
        Ok((None, input.to_path_buf()))
    } else if metadata.is_dir() {
        let (temp, path) = build_pack_from_source(input, verbose)?;
        Ok((Some(temp), path))
    } else {
        bail!(
            "input {} is neither a file nor a directory",
            input.display()
        );
    }
}

fn build_pack_from_source(source: &Path, verbose: bool) -> Result<(TempDir, PathBuf)> {
    let temp = TempDir::new().context("failed to create temporary directory for plan build")?;
    let gtpack_path = temp.path().join("plan.gtpack");
    let wasm_path = temp.path().join("pack.wasm");
    let manifest_path = temp.path().join("manifest.cbor");
    let sbom_path = temp.path().join("sbom.cdx.json");
    let component_data = temp.path().join("data.rs");

    let packc_bin = std::env::var(PACKC_ENV).unwrap_or_else(|_| "packc".to_string());
    let mut cmd = Command::new(packc_bin);
    cmd.arg("build")
        .arg("--in")
        .arg(source)
        .arg("--out")
        .arg(&wasm_path)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--sbom")
        .arg(&sbom_path)
        .arg("--gtpack-out")
        .arg(&gtpack_path)
        .arg("--component-data")
        .arg(&component_data)
        .arg("--log")
        .arg(if verbose { "info" } else { "warn" });

    if !verbose {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    let status = cmd
        .status()
        .context("failed to spawn packc to build temporary .gtpack")?;
    if !status.success() {
        bail!("packc build failed with status {}", status);
    }

    Ok((temp, gtpack_path))
}

fn load_component_manifests(
    pack_path: &Path,
    pack_manifest: &PackManifest,
) -> Result<HashMap<String, ComponentManifest>> {
    let file =
        File::open(pack_path).with_context(|| format!("failed to open {}", pack_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid gtpack archive", pack_path.display()))?;

    let mut manifests = HashMap::new();
    for component in &pack_manifest.components {
        if let Some(manifest_path) = component.manifest_file.as_deref() {
            let mut entry = archive
                .by_name(manifest_path)
                .with_context(|| format!("component manifest `{}` missing", manifest_path))?;
            let manifest: ComponentManifest =
                serde_json::from_reader(&mut entry).with_context(|| {
                    format!("failed to parse component manifest `{}`", manifest_path)
                })?;
            manifests.insert(component.name.clone(), manifest);
        }
    }

    Ok(manifests)
}
