use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use greentic_types::pack::extensions::capabilities::{
    CapabilityHookAppliesToV1, CapabilityOfferV1, CapabilityProviderRefV1, CapabilitySetupV1,
};
use greentic_types::provider::{ProviderDecl, ProviderRuntimeRef};
use serde_json::{Value as JsonValue, json};
use serde_yaml_bw::{self, Mapping, Sequence, Value as YamlValue};
use walkdir::WalkDir;

use crate::config::PackConfig;
use crate::extension_refs::{
    ExtensionDependency, ExtensionDependencySource, PackExtensionsFile,
    default_extensions_file_path, infer_reference_kind, read_extensions_file,
    write_extensions_file,
};

pub const PROVIDER_RUNTIME_WORLD: &str = "greentic:provider/schema-core@1.0.0";
const PROVIDER_EXTENSION_KEY: &str = "greentic.provider-extension.v1";
const PROVIDER_EXTENSION_PATH: [&str; 3] = ["greentic", "provider-extension", "v1"];
const CAPABILITIES_EXTENSION_KEY: &str = "greentic.ext.capabilities.v1";
const DEPLOYER_EXTENSION_KEY: &str = "greentic.deployer.v1";

#[derive(Debug, Subcommand)]
pub enum AddExtensionCommand {
    /// Add or update the provider extension entry.
    Provider(ProviderArgs),
    /// Add or update a capability offer entry.
    Capability(CapabilityArgs),
    /// Add or update a generic deployer extension entry.
    Deployer(DeployerArgs),
    /// Add or update an external extension dependency ref in pack.extensions.json.
    Dependency(DependencyArgs),
}

#[derive(Debug, Args)]
pub struct ProviderArgs {
    /// Path to a pack source directory containing pack.yaml.
    #[arg(long = "pack-dir", value_name = "DIR")]
    pub pack_dir: PathBuf,

    /// Print what would change without writing files.
    #[arg(long)]
    pub dry_run: bool,

    /// Provider identifier to add or update.
    #[arg(long = "id", value_name = "PROVIDER_ID")]
    pub provider_id: String,

    /// Provider kind (e.g. messaging, events).
    #[arg(long = "kind", value_name = "KIND")]
    pub kind: String,

    /// Optional provider title to store in metadata.
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,

    /// Optional description to store in metadata.
    #[arg(long, value_name = "DESCRIPTION")]
    pub description: Option<String>,
    /// Optional validator reference for generated provider metadata.
    #[arg(long = "validator-ref", value_name = "VALIDATOR_REF")]
    pub validator_ref: Option<String>,
    /// Optional validator digest for strict pinning.
    #[arg(long = "validator-digest", value_name = "DIGEST")]
    pub validator_digest: Option<String>,

    /// Convenience route hint (if schema supports route<->flow binding).
    #[arg(long = "route", value_name = "ROUTE")]
    pub route: Option<String>,

    /// Convenience flow hint (if schema supports route<->flow binding).
    #[arg(long = "flow", value_name = "FLOW")]
    pub flow: Option<String>,
}

#[derive(Debug, Args)]
pub struct CapabilityArgs {
    /// Path to a pack source directory containing pack.yaml.
    #[arg(long = "pack-dir", value_name = "DIR")]
    pub pack_dir: PathBuf,

    /// Print what would change without writing files.
    #[arg(long)]
    pub dry_run: bool,

    /// Stable offer id.
    #[arg(long = "offer-id", value_name = "ID")]
    pub offer_id: String,

    /// Capability identifier.
    #[arg(long = "cap-id", value_name = "CAP_ID")]
    pub cap_id: String,

    /// Capability contract version.
    #[arg(long, default_value = "v1")]
    pub version: String,

    /// Provider component reference.
    #[arg(long = "component-ref", value_name = "COMPONENT")]
    pub component_ref: String,

    /// Provider operation.
    #[arg(long = "op", value_name = "OP")]
    pub op: String,

    /// Selection priority (ascending).
    #[arg(long, default_value_t = 0)]
    pub priority: i32,

    /// Mark offer as requiring setup.
    #[arg(long = "requires-setup", default_value_t = false)]
    pub requires_setup: bool,

    /// Pack-relative QA spec ref (required when --requires-setup).
    #[arg(long = "qa-ref", value_name = "REF")]
    pub qa_ref: Option<String>,

    /// Exact operation names for hook applicability (repeatable).
    #[arg(long = "hook-op-name", value_name = "OP_NAME")]
    pub hook_op_names: Vec<String>,
}

#[derive(Debug, Args)]
pub struct DeployerArgs {
    /// Path to a pack source directory containing pack.yaml.
    #[arg(long = "pack-dir", value_name = "DIR")]
    pub pack_dir: PathBuf,

    /// Print what would change without writing files.
    #[arg(long)]
    pub dry_run: bool,

    /// Deployer contract identifier.
    #[arg(long = "contract-id", value_name = "CONTRACT")]
    pub contract_id: String,

    /// Supported deployer operation (repeatable).
    #[arg(long = "op", value_name = "OP")]
    pub ops: Vec<String>,

    /// Optional explicit flow ref mapping (`op=flows/path.ygtc`), repeatable.
    #[arg(long = "flow-ref", value_name = "OP=PATH")]
    pub flow_refs: Vec<String>,
}

#[derive(Debug, Args)]
pub struct DependencyArgs {
    /// Path to a pack source directory containing pack.yaml.
    #[arg(long = "pack-dir", value_name = "DIR")]
    pub pack_dir: PathBuf,

    /// Print what would change without writing files.
    #[arg(long)]
    pub dry_run: bool,

    /// Logical dependency id.
    #[arg(long = "id", value_name = "ID")]
    pub id: String,

    /// Logical dependency role (for example `deployer`).
    #[arg(long = "role", value_name = "ROLE")]
    pub role: String,

    /// Source reference, for example `oci://...` or `file://...`.
    #[arg(long = "ref", value_name = "REF")]
    pub reference: String,

    /// Allow tag refs in editable source metadata.
    #[arg(long = "allow-tags", default_value_t = false)]
    pub allow_tags: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CapabilityOfferSpec {
    pub offer_id: String,
    pub cap_id: String,
    pub version: String,
    pub component_ref: String,
    pub op: String,
    pub priority: i32,
    pub requires_setup: bool,
    pub qa_ref: Option<String>,
    pub hook_op_names: Vec<String>,
}

pub fn handle(command: AddExtensionCommand) -> Result<()> {
    match command {
        AddExtensionCommand::Provider(args) => handle_provider(args),
        AddExtensionCommand::Capability(args) => handle_capability(args),
        AddExtensionCommand::Deployer(args) => handle_deployer(args),
        AddExtensionCommand::Dependency(args) => handle_dependency(args),
    }
}

fn handle_provider(args: ProviderArgs) -> Result<()> {
    eprintln!(
        "note: provider extension updates use the legacy schema-core path (`greentic:provider/schema-core@1.0.0`)"
    );
    edit_pack_dir(&args.pack_dir, &args)?;
    Ok(())
}

fn handle_capability(args: CapabilityArgs) -> Result<()> {
    let root = normalize_root(&args.pack_dir)?;
    let pack_yaml = root.join("pack.yaml");
    let (_, contents) = read_pack_yaml(&pack_yaml)?;
    let updated_yaml = inject_capability_offer_spec(&contents, &args.to_spec()?)?;

    if args.dry_run {
        println!("--- dry-run: updated pack.yaml ---");
        println!("{updated_yaml}");
        return Ok(());
    }

    fs::write(&pack_yaml, updated_yaml)
        .with_context(|| format!("write {}", pack_yaml.display()))?;
    println!("capabilities extension updated in {}", pack_yaml.display());
    Ok(())
}

fn handle_deployer(args: DeployerArgs) -> Result<()> {
    let root = normalize_root(&args.pack_dir)?;
    let pack_yaml = root.join("pack.yaml");
    let (_, contents) = read_pack_yaml(&pack_yaml)?;
    let payload = args.to_payload()?;
    let updated_yaml = inject_deployer_extension_payload(&contents, &payload)?;

    if args.dry_run {
        println!("--- dry-run: updated pack.yaml ---");
        println!("{updated_yaml}");
        return Ok(());
    }

    fs::write(&pack_yaml, updated_yaml)
        .with_context(|| format!("write {}", pack_yaml.display()))?;
    write_deployer_extension_sidecar(&root, &payload)?;
    println!("deployer extension updated in {}", pack_yaml.display());
    Ok(())
}

fn handle_dependency(args: DependencyArgs) -> Result<()> {
    let root = normalize_root(&args.pack_dir)?;
    let file_path = default_extensions_file_path(&root);
    let mut file = if file_path.exists() {
        read_extensions_file(&file_path)?
    } else {
        PackExtensionsFile::new(Vec::new())
    };
    let dependency = args.to_dependency()?;

    if let Some(existing) = file
        .extensions
        .iter_mut()
        .find(|item| item.id == dependency.id)
    {
        *existing = dependency;
    } else {
        file.extensions.push(dependency);
        file.extensions
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    if args.dry_run {
        println!("--- dry-run: updated {} ---", file_path.display());
        println!(
            "{}",
            serde_json::to_string_pretty(&file).context("serialize pack.extensions.json")?
        );
        return Ok(());
    }

    write_extensions_file(&file_path, &file)?;
    println!("extension dependency updated in {}", file_path.display());
    Ok(())
}

impl CapabilityArgs {
    fn to_spec(&self) -> Result<CapabilityOfferSpec> {
        if self.requires_setup && self.qa_ref.is_none() {
            anyhow::bail!("--qa-ref is required when --requires-setup is set");
        }
        if let Some(qa_ref) = self.qa_ref.as_ref()
            && qa_ref.trim().is_empty()
        {
            anyhow::bail!("--qa-ref must not be empty");
        }
        Ok(CapabilityOfferSpec {
            offer_id: self.offer_id.clone(),
            cap_id: self.cap_id.clone(),
            version: self.version.clone(),
            component_ref: self.component_ref.clone(),
            op: self.op.clone(),
            priority: self.priority,
            requires_setup: self.requires_setup,
            qa_ref: self.qa_ref.clone(),
            hook_op_names: self.hook_op_names.clone(),
        })
    }
}

impl DeployerArgs {
    fn to_payload(&self) -> Result<JsonValue> {
        let contract_id = self.contract_id.trim();
        if contract_id.is_empty() {
            anyhow::bail!("--contract-id must not be empty");
        }

        let ops = if self.ops.is_empty() {
            vec![
                "generate".to_string(),
                "plan".to_string(),
                "apply".to_string(),
                "destroy".to_string(),
                "status".to_string(),
                "rollback".to_string(),
            ]
        } else {
            self.ops
                .iter()
                .map(|op| op.trim())
                .filter(|op| !op.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        };
        if ops.is_empty() {
            anyhow::bail!("at least one non-empty --op value is required");
        }

        let mut flow_refs = serde_json::Map::new();
        if self.flow_refs.is_empty() {
            for op in &ops {
                flow_refs.insert(op.clone(), JsonValue::String(format!("flows/{op}.ygtc")));
            }
        } else {
            for mapping in &self.flow_refs {
                let (op, path) = mapping
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("--flow-ref must be in OP=PATH form"))?;
                let op = op.trim();
                let path = path.trim();
                if op.is_empty() || path.is_empty() {
                    anyhow::bail!("--flow-ref must not contain empty op or path");
                }
                flow_refs.insert(op.to_string(), JsonValue::String(path.to_string()));
            }
        }

        Ok(json!({
            "version": 1,
            "provides": [{
                "capability": DEPLOYER_EXTENSION_KEY,
                "contract": contract_id,
                "ops": ops,
            }],
            "flow_refs": flow_refs,
        }))
    }
}

impl DependencyArgs {
    fn to_dependency(&self) -> Result<ExtensionDependency> {
        let id = self.id.trim();
        let role = self.role.trim();
        let reference = self.reference.trim();
        if id.is_empty() {
            anyhow::bail!("--id must not be empty");
        }
        if role.is_empty() {
            anyhow::bail!("--role must not be empty");
        }
        if reference.is_empty() {
            anyhow::bail!("--ref must not be empty");
        }
        let kind = infer_reference_kind(reference)?;
        Ok(ExtensionDependency {
            id: id.to_string(),
            role: role.to_string(),
            source: ExtensionDependencySource {
                kind,
                reference: reference.to_string(),
                allow_tags: self.allow_tags,
            },
        })
    }
}

fn edit_pack_dir(pack_dir: &Path, args: &ProviderArgs) -> Result<()> {
    let root = normalize_root(pack_dir)?;
    let pack_yaml = root.join("pack.yaml");
    let (pack_config, contents) = read_pack_yaml(&pack_yaml)?;
    let metadata = ProviderMetadata::from_args(args);
    let updated_yaml = inject_provider_entry(
        &contents,
        &build_provider_decl(args, &root)?,
        metadata,
        &pack_config.version,
    )?;

    if args.dry_run {
        println!("--- dry-run: updated pack.yaml ---");
        println!("{updated_yaml}");
        return Ok(());
    }

    fs::write(&pack_yaml, updated_yaml)
        .with_context(|| format!("write {}", pack_yaml.display()))?;
    println!("provider extension updated in {}", pack_yaml.display());
    Ok(())
}

fn normalize_root(path: &Path) -> Result<PathBuf> {
    let canonical = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(canonical)
}

fn read_pack_yaml(path: &Path) -> Result<(PackConfig, String)> {
    let contents = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let config: PackConfig = serde_yaml_bw::from_str(&contents)
        .with_context(|| format!("{} is not a valid pack.yaml", path.display()))?;
    Ok((config, contents))
}

#[derive(Default)]
struct ProviderMetadata {
    title: Option<String>,
    description: Option<String>,
    route: Option<String>,
    flow: Option<String>,
    validator_ref: Option<String>,
    validator_digest: Option<String>,
}

impl ProviderMetadata {
    fn from_args(args: &ProviderArgs) -> Self {
        Self {
            title: args.title.clone(),
            description: args.description.clone(),
            route: args.route.clone(),
            flow: args.flow.clone(),
            validator_ref: args.validator_ref.clone(),
            validator_digest: args.validator_digest.clone(),
        }
    }
}

fn build_provider_decl(args: &ProviderArgs, root: &Path) -> Result<ProviderDecl> {
    let config_ref = find_config_schema_ref(root, &args.kind, &args.provider_id);
    let capabilities = vec![args.kind.clone()];
    let ops = match args.kind.as_str() {
        "messaging" => vec!["send".to_string(), "receive".to_string()],
        "events" => vec!["emit".to_string(), "subscribe".to_string()],
        _ => vec!["run".to_string()],
    };

    Ok(ProviderDecl {
        provider_type: args.provider_id.clone(),
        provider_id: None,
        capabilities,
        ops,
        config_schema_ref: config_ref,
        state_schema_ref: None,
        runtime: ProviderRuntimeRef {
            component_ref: args.provider_id.clone(),
            export: "provider".to_string(),
            world: PROVIDER_RUNTIME_WORLD.to_string(),
        },
        docs_ref: None,
    })
}

pub(crate) fn inject_provider_entry_for_wizard(
    contents: &str,
    provider_id: &str,
    kind: &str,
    version: &str,
) -> Result<String> {
    let provider = ProviderDecl {
        provider_type: provider_id.to_string(),
        provider_id: None,
        capabilities: vec![kind.to_string()],
        ops: match kind {
            "messaging" => vec!["send".to_string(), "receive".to_string()],
            "events" => vec!["emit".to_string(), "subscribe".to_string()],
            _ => vec!["run".to_string()],
        },
        config_schema_ref: format!("schemas/{kind}/{provider_id}/config.schema.json"),
        state_schema_ref: None,
        runtime: ProviderRuntimeRef {
            component_ref: provider_id.to_string(),
            export: "provider".to_string(),
            world: PROVIDER_RUNTIME_WORLD.to_string(),
        },
        docs_ref: None,
    };
    inject_provider_entry(contents, &provider, ProviderMetadata::default(), version)
}

fn find_config_schema_ref(root: &Path, kind: &str, provider_id: &str) -> String {
    let schemas = root.join("schemas");
    if schemas.exists() {
        let provider_kw = provider_id.to_ascii_lowercase();
        for entry in WalkDir::new(&schemas)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains(&provider_kw)
                && name.contains("config.schema")
                && let Ok(rel) = entry.path().strip_prefix(root)
            {
                return rel
                    .components()
                    .map(|comp| comp.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
            }
        }
    }

    format!("schemas/{}/{}/config.schema.json", kind, provider_id)
}

fn inject_provider_entry(
    contents: &str,
    provider: &ProviderDecl,
    metadata: ProviderMetadata,
    version: &str,
) -> Result<String> {
    let mut document: YamlValue =
        serde_yaml_bw::from_str(contents).context("parse pack.yaml for extension merge")?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("pack.yaml root must be a mapping"))?;
    let extensions = mapping
        .entry(yaml_key("extensions"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extensions_map = extensions
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("extensions must be a mapping"))?;

    let location = detect_extension_location(extensions_map);
    let extension_map = resolve_extension_map(extensions_map, &location)
        .context("locate provider extension slot")?;
    extension_map
        .entry(yaml_key("kind"))
        .or_insert_with(|| YamlValue::String(PROVIDER_EXTENSION_KEY.to_string(), None));
    extension_map
        .entry(yaml_key("version"))
        .or_insert_with(|| YamlValue::String(version.to_string(), None));

    let inline = extension_map
        .entry(yaml_key("inline"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let inline_map = match inline {
        YamlValue::Mapping(map) => map,
        _ => {
            *inline = YamlValue::Mapping(Mapping::new());
            inline.as_mapping_mut().unwrap()
        }
    };

    let providers_key = yaml_key("providers");
    let providers_entry = inline_map
        .entry(providers_key.clone())
        .or_insert_with(|| YamlValue::Sequence(Sequence::default()));
    let providers = match providers_entry {
        YamlValue::Sequence(seq) => seq,
        _ => {
            *providers_entry = YamlValue::Sequence(Sequence::default());
            providers_entry.as_sequence_mut().unwrap()
        }
    };

    let mut provider_value =
        serde_yaml_bw::to_value(provider).context("serialize provider declaration")?;
    if let Some(map) = provider_value.as_mapping_mut() {
        if let Some(title) = metadata.title {
            map.insert(yaml_key("title"), YamlValue::String(title, None));
        }
        if let Some(desc) = metadata.description {
            map.insert(yaml_key("description"), YamlValue::String(desc, None));
        }
        if let Some(route) = metadata.route {
            map.insert(yaml_key("route"), YamlValue::String(route, None));
        }
        if let Some(flow) = metadata.flow {
            map.insert(yaml_key("flow"), YamlValue::String(flow, None));
        }
        if let Some(validator_ref) = metadata.validator_ref {
            map.insert(
                yaml_key("validator_ref"),
                YamlValue::String(validator_ref, None),
            );
        }
        if let Some(validator_digest) = metadata.validator_digest {
            map.insert(
                yaml_key("validator_digest"),
                YamlValue::String(validator_digest, None),
            );
        }
    }
    upsert_provider(providers, provider_value, &provider.provider_type);

    serde_yaml_bw::to_string(&document).context("serialize updated pack.yaml")
}

pub(crate) fn ensure_capabilities_extension(contents: &str) -> Result<String> {
    let mut document: YamlValue =
        serde_yaml_bw::from_str(contents).context("parse pack.yaml for extension merge")?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("pack.yaml root must be a mapping"))?;
    let extensions = mapping
        .entry(yaml_key("extensions"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extensions_map = extensions
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("extensions must be a mapping"))?;
    let extension_slot = extensions_map
        .entry(yaml_key(CAPABILITIES_EXTENSION_KEY))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extension_map = extension_slot
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("capabilities extension slot must be a mapping"))?;
    extension_map
        .entry(yaml_key("kind"))
        .or_insert_with(|| YamlValue::String(CAPABILITIES_EXTENSION_KEY.to_string(), None));
    extension_map
        .entry(yaml_key("version"))
        .or_insert_with(|| YamlValue::String("1.0.0".to_string(), None));

    let inline = extension_map
        .entry(yaml_key("inline"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let inline_map = match inline {
        YamlValue::Mapping(map) => map,
        _ => {
            *inline = YamlValue::Mapping(Mapping::new());
            inline.as_mapping_mut().expect("inline map")
        }
    };
    inline_map
        .entry(yaml_key("schema_version"))
        .or_insert_with(|| YamlValue::Number(1u64.into(), None));

    let offers_entry = inline_map
        .entry(yaml_key("offers"))
        .or_insert_with(|| YamlValue::Sequence(Sequence::default()));
    if !matches!(offers_entry, YamlValue::Sequence(_)) {
        *offers_entry = YamlValue::Sequence(Sequence::default());
    }

    serde_yaml_bw::to_string(&document).context("serialize updated pack.yaml")
}

pub(crate) fn inject_capability_offer_spec(
    contents: &str,
    spec: &CapabilityOfferSpec,
) -> Result<String> {
    let mut document: YamlValue = serde_yaml_bw::from_str(
        &ensure_capabilities_extension(contents).context("prepare capabilities extension")?,
    )
    .context("parse pack.yaml for capability offer merge")?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("pack.yaml root must be a mapping"))?;
    let extensions_map = mapping
        .get_mut(yaml_key("extensions"))
        .and_then(YamlValue::as_mapping_mut)
        .ok_or_else(|| anyhow::anyhow!("extensions must be a mapping"))?;
    let extension_map = extensions_map
        .get_mut(yaml_key(CAPABILITIES_EXTENSION_KEY))
        .and_then(YamlValue::as_mapping_mut)
        .ok_or_else(|| anyhow::anyhow!("capabilities extension slot must be a mapping"))?;
    let inline_map = extension_map
        .get_mut(yaml_key("inline"))
        .and_then(YamlValue::as_mapping_mut)
        .ok_or_else(|| anyhow::anyhow!("capabilities extension inline must be a mapping"))?;
    let offers_entry = inline_map
        .entry(yaml_key("offers"))
        .or_insert_with(|| YamlValue::Sequence(Sequence::default()));
    let offers = match offers_entry {
        YamlValue::Sequence(seq) => seq,
        _ => {
            *offers_entry = YamlValue::Sequence(Sequence::default());
            offers_entry.as_sequence_mut().expect("offers seq")
        }
    };

    let offer = CapabilityOfferV1 {
        offer_id: spec.offer_id.clone(),
        cap_id: spec.cap_id.clone(),
        version: spec.version.clone(),
        provider: CapabilityProviderRefV1 {
            component_ref: spec.component_ref.clone(),
            op: spec.op.clone(),
        },
        scope: None,
        priority: spec.priority,
        requires_setup: spec.requires_setup,
        setup: spec.qa_ref.as_ref().map(|qa_ref| CapabilitySetupV1 {
            qa_ref: qa_ref.clone(),
        }),
        applies_to: (!spec.hook_op_names.is_empty()).then(|| CapabilityHookAppliesToV1 {
            op_names: spec.hook_op_names.clone(),
        }),
    };
    let offer_value =
        serde_yaml_bw::to_value(&offer).context("serialize capability offer payload")?;
    upsert_capability_offer(offers, offer_value, &spec.offer_id);
    sort_capability_offers(offers);

    serde_yaml_bw::to_string(&document).context("serialize updated pack.yaml")
}

fn inject_deployer_extension_payload(contents: &str, payload: &JsonValue) -> Result<String> {
    let mut document: YamlValue = serde_yaml_bw::from_str(contents)
        .context("parse pack.yaml for deployer extension merge")?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("pack.yaml root must be a mapping"))?;
    let extensions = mapping
        .entry(yaml_key("extensions"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extensions_map = extensions
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("extensions must be a mapping"))?;
    let extension_slot = extensions_map
        .entry(yaml_key(DEPLOYER_EXTENSION_KEY))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extension_map = extension_slot
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("deployer extension slot must be a mapping"))?;
    extension_map
        .entry(yaml_key("kind"))
        .or_insert_with(|| YamlValue::String(DEPLOYER_EXTENSION_KEY.to_string(), None));
    extension_map
        .entry(yaml_key("version"))
        .or_insert_with(|| YamlValue::String("1.0.0".to_string(), None));
    extension_map.insert(
        yaml_key("inline"),
        serde_yaml_bw::to_value(payload).context("serialize deployer extension payload")?,
    );

    serde_yaml_bw::to_string(&document).context("serialize updated pack.yaml")
}

fn write_deployer_extension_sidecar(root: &Path, payload: &JsonValue) -> Result<()> {
    let extensions_dir = root.join("extensions");
    fs::create_dir_all(&extensions_dir)
        .with_context(|| format!("create {}", extensions_dir.display()))?;
    let path = extensions_dir.join("deployer.json");
    let bytes = serde_json::to_vec_pretty(&json!({
        "extension_type": "deployer",
        "canonical_extension_key": DEPLOYER_EXTENSION_KEY,
        "source": "add-extension deployer",
        "deployer_extension": payload,
    }))
    .context("serialize deployer extension sidecar")?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn upsert_capability_offer(offers: &mut Vec<YamlValue>, offer: YamlValue, offer_id: &str) {
    for entry in offers.iter_mut() {
        if entry_matches_capability_offer(entry, offer_id) {
            *entry = offer;
            return;
        }
    }
    offers.push(offer);
}

fn sort_capability_offers(offers: &mut [YamlValue]) {
    offers.sort_by(|left, right| {
        capability_offer_id(left)
            .cmp(&capability_offer_id(right))
            .then_with(|| {
                let left_yaml = serde_yaml_bw::to_string(left).unwrap_or_default();
                let right_yaml = serde_yaml_bw::to_string(right).unwrap_or_default();
                left_yaml.cmp(&right_yaml)
            })
    });
}

fn capability_offer_id(entry: &YamlValue) -> String {
    let key = yaml_key("offer_id");
    if let YamlValue::Mapping(map) = entry
        && let Some(YamlValue::String(value, _)) = map.get(&key)
    {
        return value.clone();
    }
    String::new()
}

fn entry_matches_capability_offer(entry: &YamlValue, offer_id: &str) -> bool {
    let key = yaml_key("offer_id");
    if let YamlValue::Mapping(map) = entry
        && let Some(YamlValue::String(value, _)) = map.get(&key)
    {
        return value == offer_id;
    }
    false
}

fn upsert_provider(providers: &mut Vec<YamlValue>, provider: YamlValue, provider_id: &str) {
    for entry in providers.iter_mut() {
        if entry_matches_provider(entry, provider_id) {
            *entry = provider;
            return;
        }
    }
    providers.push(provider);
}

fn entry_matches_provider(entry: &YamlValue, provider_id: &str) -> bool {
    let provider_key = yaml_key("provider_type");
    if let YamlValue::Mapping(map) = entry
        && let Some(YamlValue::String(value, _)) = map.get(&provider_key)
    {
        return value == provider_id;
    }
    false
}

enum ExtensionLocation {
    Flat,
    Nested,
}

fn detect_extension_location(extensions: &Mapping) -> ExtensionLocation {
    let provider_key = yaml_key(PROVIDER_EXTENSION_KEY);
    if extensions.contains_key(&provider_key) {
        return ExtensionLocation::Flat;
    }
    let mut current = extensions;
    for segment in PROVIDER_EXTENSION_PATH
        .iter()
        .take(PROVIDER_EXTENSION_PATH.len() - 1)
    {
        let key = yaml_key(*segment);
        if let Some(next) = current.get(&key).and_then(YamlValue::as_mapping) {
            current = next;
        } else {
            return ExtensionLocation::Flat;
        }
    }
    ExtensionLocation::Nested
}

fn resolve_extension_map<'a>(
    extensions: &'a mut Mapping,
    location: &ExtensionLocation,
) -> Result<&'a mut Mapping> {
    match location {
        ExtensionLocation::Flat => {
            let key = yaml_key(PROVIDER_EXTENSION_KEY);
            let slot = extensions
                .entry(key)
                .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
            slot.as_mapping_mut()
                .ok_or_else(|| anyhow::anyhow!("extension slot must be a mapping"))
        }
        ExtensionLocation::Nested => {
            let mut current_map = extensions;
            for segment in PROVIDER_EXTENSION_PATH.iter() {
                let key = yaml_key(*segment);
                let entry = current_map
                    .entry(key)
                    .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
                current_map = entry
                    .as_mapping_mut()
                    .ok_or_else(|| anyhow::anyhow!("nested extension value must be a mapping"))?;
            }
            Ok(current_map)
        }
    }
}

fn yaml_key(value: impl Into<String>) -> YamlValue {
    YamlValue::String(value.into(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_bw;

    fn sample_flat_yaml() -> String {
        r#"pack_id: demo
version: 0.1.0
extensions:
  greentic.provider-extension.v1:
    kind: greentic.provider-extension.v1
    version: 0.1.0
    inline:
      providers:
        - provider_type: existing
          capabilities: [messaging]
          ops: [send]
          config_schema_ref: schemas/messaging/existing/config.schema.json
          runtime:
            component_ref: existing
            export: provider
            world: greentic:provider/schema-core@1.0.0
"#
        .to_string()
    }

    fn sample_nested_yaml() -> String {
        r#"pack_id: demo
version: 0.1.0
extensions:
  greentic:
    provider-extension:
      v1:
        inline:
          providers: []
"#
        .to_string()
    }

    fn provider_decl() -> ProviderDecl {
        ProviderDecl {
            provider_type: "demo.provider".to_string(),
            provider_id: None,
            capabilities: vec!["messaging".to_string()],
            ops: vec!["send".to_string()],
            config_schema_ref: "schemas/messaging/demo/config.schema.json".to_string(),
            state_schema_ref: None,
            runtime: ProviderRuntimeRef {
                component_ref: "demo.provider".to_string(),
                export: "provider".to_string(),
                world: PROVIDER_RUNTIME_WORLD.to_string(),
            },
            docs_ref: None,
        }
    }

    #[test]
    fn inject_flat_extension() {
        let contents = sample_flat_yaml();
        let updated = inject_provider_entry(
            &contents,
            &provider_decl(),
            ProviderMetadata::default(),
            "0.1.0",
        )
        .unwrap();
        let doc: YamlValue = serde_yaml_bw::from_str(&updated).unwrap();

        let providers = doc["extensions"]["greentic.provider-extension.v1"]["inline"]["providers"]
            .as_sequence()
            .expect("providers list");
        assert!(
            providers
                .iter()
                .any(|entry| entry_matches_provider(entry, "demo.provider"))
        );
    }

    #[test]
    fn inject_nested_extension() {
        let contents = sample_nested_yaml();
        let updated = inject_provider_entry(
            &contents,
            &provider_decl(),
            ProviderMetadata::default(),
            "0.1.0",
        )
        .unwrap();
        let doc: YamlValue = serde_yaml_bw::from_str(&updated).unwrap();

        assert!(
            doc["extensions"]["greentic"]["provider-extension"]["v1"]["inline"]["providers"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|entry| entry_matches_provider(entry, "demo.provider"))
        );
    }
}
