use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use greentic_pack::builder::PackManifest;
use greentic_pack::plan::infer_base_deployment_plan;
use greentic_pack::reader::{SigningPolicy, open_pack};
use greentic_types::component::ComponentManifest;
use greentic_types::{EnvId, TenantCtx, TenantId};
use zip::ZipArchive;

use super::PlanArgs;
use crate::input::materialize_pack_path;

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
