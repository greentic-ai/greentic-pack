use crate::BuildArgs;
use crate::embed;
use crate::flows;
use crate::manifest;
use crate::sbom;
use crate::templates;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub pack_dir: PathBuf,
    pub component_out: PathBuf,
    pub manifest_out: PathBuf,
    pub sbom_out: PathBuf,
    pub component_data: PathBuf,
    pub dry_run: bool,
}

impl From<BuildArgs> for BuildOptions {
    fn from(args: BuildArgs) -> Self {
        let pack_dir = normalize(args.input);
        let component_out = normalize(args.component_out);
        let manifest_out = normalize(args.manifest);
        let sbom_out = normalize(args.sbom);
        let default_component_data = pack_dir
            .join(".packc")
            .join("pack_component")
            .join("src")
            .join("data.rs");
        let component_data = args
            .component_data
            .map(normalize)
            .unwrap_or(default_component_data);

        Self {
            pack_dir,
            component_out,
            manifest_out,
            sbom_out,
            component_data,
            dry_run: args.dry_run,
        }
    }
}

pub fn run(opts: &BuildOptions) -> Result<()> {
    info!(
        pack_dir = %opts.pack_dir.display(),
        component_out = %opts.component_out.display(),
        manifest_out = %opts.manifest_out.display(),
        sbom_out = %opts.sbom_out.display(),
        component_data = %opts.component_data.display(),
        dry_run = opts.dry_run,
        "building greentic pack"
    );

    let spec_bundle = manifest::load_spec(&opts.pack_dir)?;
    info!(id = %spec_bundle.spec.id, version = %spec_bundle.spec.version, "loaded pack spec");

    let flows = flows::load_flows(&opts.pack_dir, &spec_bundle.spec)?;
    info!(count = flows.len(), "loaded flows");

    let templates = templates::collect_templates(&opts.pack_dir, &spec_bundle.spec)?;
    info!(count = templates.len(), "collected templates");

    let pack_manifest = manifest::build_manifest(&spec_bundle, &flows, &templates);
    let manifest_bytes = manifest::encode_manifest(&pack_manifest)?;
    info!(len = manifest_bytes.len(), "encoded manifest");

    let component_src = embed::generate_component_data(&manifest_bytes, &flows, &templates)?;
    let sbom_model = sbom::generate(&spec_bundle, &flows, &templates);
    let sbom_json = serde_json::to_string_pretty(&sbom_model)?;

    if opts.dry_run {
        debug!("component_data=\n{}", component_src);
        info!("dry-run complete; no files written");
        return Ok(());
    }

    write_if_changed(&opts.manifest_out, &manifest_bytes)?;
    write_if_changed(&opts.sbom_out, sbom_json.as_bytes())?;
    write_if_changed(&opts.component_data, component_src.as_bytes())?;

    embed::compile_component(&opts.component_data, &opts.component_out)?;

    info!("build complete");
    Ok(())
}

fn normalize(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        cwd.join(path)
    }
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let mut needs_write = true;
    if let Ok(current) = fs::read(path)
        && current == contents
    {
        needs_write = false;
    }

    if needs_write {
        fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
        info!(path = %path.display(), "wrote file");
    } else {
        debug!(path = %path.display(), "unchanged");
    }

    Ok(())
}
