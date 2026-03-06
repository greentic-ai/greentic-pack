#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use greentic_distributor_client::{DistClient, DistOptions};
use greentic_types::{WizardId, WizardMode, WizardTarget};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use tokio::runtime::Handle;

use crate::cli::wizard_i18n::WizardI18n;
use crate::runtime::{NetworkPolicy, RuntimeContext};

const DEFAULT_DOCS_CATALOG_PATH: &str = "docs/extensions_capability_packs.catalog.v1.json";
pub(crate) const DEFAULT_EXTENSION_CATALOG_DOWNLOAD_URL: &str = "https://github.com/greenticai/greentic-pack/blob/master/docs/extensions_capability_packs.catalog.v1.json";
const EMBEDDED_DEFAULT_CATALOG_JSON: &str =
    include_str!("../../../../docs/extensions_capability_packs.catalog.v1.json");

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExtensionCatalog {
    pub(crate) extension_types: Vec<ExtensionType>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExtensionType {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) canonical_extension_key: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    name_key: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    description_key: Option<String>,
    #[serde(default)]
    pub(crate) templates: Vec<ExtensionTemplate>,
    #[serde(default)]
    pub(crate) edit_questions: Vec<CatalogQuestion>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExtensionTemplate {
    pub(crate) id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    name_key: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    description_key: Option<String>,
    #[serde(default)]
    pub(crate) plan: Vec<TemplatePlanStep>,
    #[serde(default)]
    pub(crate) qa_questions: Vec<CatalogQuestion>,
}

impl ExtensionType {
    pub(crate) fn canonical_extension_key(&self) -> &str {
        if let Some(value) = self.canonical_extension_key.as_deref() {
            return value;
        }
        "greentic.ext.capabilities.v1"
    }

    pub(crate) fn display_name(&self, i18n: &WizardI18n) -> String {
        resolve_catalog_text(
            i18n,
            self.name_key.as_deref(),
            self.name.as_deref(),
            &self.id,
        )
    }

    pub(crate) fn display_description(&self, i18n: &WizardI18n) -> String {
        resolve_catalog_text(
            i18n,
            self.description_key.as_deref(),
            self.description.as_deref(),
            "",
        )
    }
}

impl ExtensionTemplate {
    pub(crate) fn display_name(&self, i18n: &WizardI18n) -> String {
        resolve_catalog_text(
            i18n,
            self.name_key.as_deref(),
            self.name.as_deref(),
            &self.id,
        )
    }

    pub(crate) fn display_description(&self, i18n: &WizardI18n) -> String {
        resolve_catalog_text(
            i18n,
            self.description_key.as_deref(),
            self.description.as_deref(),
            "",
        )
    }
}

fn resolve_catalog_text(
    i18n: &WizardI18n,
    key: Option<&str>,
    raw: Option<&str>,
    fallback: &str,
) -> String {
    if let Some(key) = key {
        let resolved = i18n.t(key);
        if !resolved.starts_with("??") {
            return resolved;
        }
    }
    raw.unwrap_or(fallback).to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CatalogQuestion {
    pub(crate) id: String,
    pub(crate) title_key: String,
    #[serde(default)]
    pub(crate) description_key: Option<String>,
    #[serde(default)]
    pub(crate) default: Option<String>,
    #[serde(default)]
    pub(crate) kind: CatalogQuestionKind,
    #[serde(default)]
    pub(crate) choices: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
#[allow(dead_code)]
pub(crate) enum TemplatePlanStep {
    EnsureDir {
        paths: Vec<String>,
    },
    WriteFiles {
        files: BTreeMap<String, String>,
    },
    WriteBinaryFiles {
        files: BTreeMap<String, String>,
    },
    RunCli {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Delegate {
        target: WizardTarget,
        id: WizardId,
        mode: WizardMode,
        #[serde(default)]
        prefilled_answers: BTreeMap<String, Value>,
        #[serde(default)]
        output_map: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CatalogQuestionKind {
    #[default]
    String,
    Enum,
    Boolean,
    Integer,
}

pub(crate) fn load_extension_catalog(
    catalog_ref: &str,
    runtime: Option<&RuntimeContext>,
) -> Result<ExtensionCatalog> {
    if let Some(path) = catalog_ref.strip_prefix("fixture://") {
        let bytes = match path {
            "extensions.json" => FIXTURE_EXTENSIONS_JSON.as_bytes().to_vec(),
            other => return Err(anyhow!("unknown fixture catalog `{other}`")),
        };
        return parse_catalog_bytes(&bytes);
    }

    if let Some(path) = catalog_ref.strip_prefix("file://") {
        let bytes = load_catalog_file_bytes(path, runtime)?;
        return parse_catalog_bytes(&bytes);
    }

    if catalog_ref.starts_with("https://") || catalog_ref.starts_with("http://") {
        let bytes = load_catalog_url_bytes(catalog_ref, runtime)?;
        return parse_catalog_bytes(&bytes);
    }

    if catalog_ref.starts_with("oci://") {
        let Some(runtime) = runtime else {
            return Err(anyhow!("runtime required for oci catalog loading"));
        };
        return load_catalog_from_oci(catalog_ref, runtime);
    }

    Err(anyhow!(
        "unsupported catalog ref scheme; expected fixture://, file://, http(s)://, or oci://"
    ))
}

fn load_catalog_file_bytes(path: &str, runtime: Option<&RuntimeContext>) -> Result<Vec<u8>> {
    let _ = runtime;

    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if should_fallback_to_embedded_catalog(path) => {
            let _ = err;
            Ok(EMBEDDED_DEFAULT_CATALOG_JSON.as_bytes().to_vec())
        }
        Err(err) => Err(err).with_context(|| format!("read catalog file {path}")),
    }
}

fn should_fallback_to_embedded_catalog(path: &str) -> bool {
    Path::new(path) == Path::new(DEFAULT_DOCS_CATALOG_PATH)
}

fn load_catalog_url_bytes(catalog_ref: &str, runtime: Option<&RuntimeContext>) -> Result<Vec<u8>> {
    let normalized = normalize_catalog_url(catalog_ref);
    let use_embedded_fallback = is_default_catalog_download_url(catalog_ref);

    if runtime
        .map(|ctx| ctx.network_policy() == NetworkPolicy::Offline)
        .unwrap_or(false)
    {
        if use_embedded_fallback {
            return Ok(EMBEDDED_DEFAULT_CATALOG_JSON.as_bytes().to_vec());
        }
        return Err(anyhow!(
            "network operation blocked in offline mode: download extension catalog"
        ));
    }

    match fetch_catalog_url_bytes(&normalized) {
        Ok(bytes) => Ok(bytes),
        Err(err) if use_embedded_fallback => {
            let _ = err;
            Ok(EMBEDDED_DEFAULT_CATALOG_JSON.as_bytes().to_vec())
        }
        Err(err) => Err(err),
    }
}

fn fetch_catalog_url_bytes(url: &str) -> Result<Vec<u8>> {
    let url = url.to_string();
    thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_millis(800))
            .timeout(Duration::from_secs(3))
            .build()
            .context("build HTTP client for extension catalog")?;
        let response = client
            .get(&url)
            .send()
            .with_context(|| format!("request extension catalog {url}"))?;
        if response.status() != StatusCode::OK {
            return Err(anyhow!(
                "request extension catalog {} failed with status {}",
                url,
                response.status()
            ));
        }
        let bytes = response
            .bytes()
            .with_context(|| format!("read extension catalog response {url}"))?;
        Ok(bytes.to_vec())
    })
    .join()
    .map_err(|_| anyhow!("catalog download thread panicked"))?
}

fn normalize_catalog_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://github.com/")
        && let Some((repo, path)) = rest.split_once("/blob/")
        && let Some((branch, file_path)) = path.split_once('/')
    {
        return format!("https://raw.githubusercontent.com/{repo}/{branch}/{file_path}");
    }
    url.to_string()
}

fn is_default_catalog_download_url(url: &str) -> bool {
    normalize_catalog_url(url) == normalize_catalog_url(DEFAULT_EXTENSION_CATALOG_DOWNLOAD_URL)
}

fn parse_catalog_bytes(bytes: &[u8]) -> Result<ExtensionCatalog> {
    let mut catalog: ExtensionCatalog =
        serde_json::from_slice(bytes).context("decode extension catalog json")?;
    if !catalog
        .extension_types
        .iter()
        .any(|item| item.id == "custom-scaffold")
    {
        catalog.extension_types.push(ExtensionType {
            id: "custom-scaffold".to_string(),
            canonical_extension_key: Some("greentic.ext.capabilities.v1".to_string()),
            name: Some("Custom extension".to_string()),
            name_key: None,
            description: Some("Scaffold only".to_string()),
            description_key: None,
            templates: vec![default_custom_template()],
            edit_questions: vec![default_edit_question("entry_label", "custom")],
        });
    }
    for extension_type in &mut catalog.extension_types {
        if extension_type.templates.is_empty() {
            extension_type.templates.push(default_generic_template());
        }
        if extension_type.edit_questions.is_empty() {
            extension_type
                .edit_questions
                .push(default_edit_question("entry_label", &extension_type.id));
        }
    }
    Ok(catalog)
}

fn default_generic_template() -> ExtensionTemplate {
    ExtensionTemplate {
        id: "default".to_string(),
        name: Some("Default".to_string()),
        name_key: None,
        description: Some("Baseline scaffold".to_string()),
        description_key: None,
        plan: vec![
            TemplatePlanStep::EnsureDir {
                paths: vec!["flows".to_string(), "components".to_string(), "i18n".to_string()],
            },
            TemplatePlanStep::WriteFiles {
                files: BTreeMap::from([
                    (
                        "README.md".to_string(),
                        "# {{extension_type_name}} extension\n\n{{not_implemented}}\n".to_string(),
                    ),
                    (
                        "pack.yaml".to_string(),
                        "pack_id: {{extension_type_id}}.extension\nversion: 0.1.0\nkind: application\npublisher: Greentic\n\ncomponents: []\nflows: []\ndependencies: []\nassets: []\n".to_string(),
                    ),
                ]),
            },
        ],
        qa_questions: vec![CatalogQuestion {
            id: "display_name".to_string(),
            title_key: "wizard.qa.display_name".to_string(),
            description_key: None,
            default: Some("My extension".to_string()),
            kind: CatalogQuestionKind::String,
            choices: Vec::new(),
        }],
    }
}

fn default_custom_template() -> ExtensionTemplate {
    ExtensionTemplate {
        id: "custom-scaffold-template".to_string(),
        name: Some("Custom scaffold template".to_string()),
        name_key: None,
        description: Some("Create a minimal custom extension skeleton".to_string()),
        description_key: None,
        plan: vec![
            TemplatePlanStep::EnsureDir {
                paths: vec!["flows".to_string(), "components".to_string(), "i18n".to_string()],
            },
            TemplatePlanStep::WriteFiles {
                files: BTreeMap::from([
                    (
                        "README.md".to_string(),
                        "# {{extension_type_name}} ({{template_name}})\n\nCanonical key: {{canonical_extension_key}}\n\nNext steps:\n- Add components under ./components\n- Add flows under ./flows\n".to_string(),
                    ),
                    (
                        "pack.yaml".to_string(),
                        "pack_id: custom.extension\nversion: 0.1.0\nkind: application\npublisher: Greentic\n\ncomponents: []\nflows: []\ndependencies: []\nassets: []\n".to_string(),
                    ),
                    (
                        "flows/main.ygtc".to_string(),
                        "id: main\ntype: messaging\nnodes: {}\n".to_string(),
                    ),
                ]),
            },
        ],
        qa_questions: vec![CatalogQuestion {
            id: "display_name".to_string(),
            title_key: "wizard.qa.display_name".to_string(),
            description_key: None,
            default: Some("Custom extension".to_string()),
            kind: CatalogQuestionKind::String,
            choices: Vec::new(),
        }],
    }
}

fn default_edit_question(id: &str, default: &str) -> CatalogQuestion {
    CatalogQuestion {
        id: id.to_string(),
        title_key: "wizard.update_extension_pack.edit.entry_label".to_string(),
        description_key: None,
        default: Some(default.to_string()),
        kind: CatalogQuestionKind::String,
        choices: Vec::new(),
    }
}

fn load_catalog_from_oci(catalog_ref: &str, runtime: &RuntimeContext) -> Result<ExtensionCatalog> {
    let offline = runtime.network_policy() == NetworkPolicy::Offline;
    let cache_dir = runtime.cache_dir().join("wizard/catalogs");
    let handle = Handle::try_current().context("catalog resolution requires a Tokio runtime")?;
    let bytes = block_on(&handle, async {
        let dist = DistClient::new(DistOptions {
            cache_dir,
            allow_tags: true,
            offline,
            allow_insecure_local_http: false,
            ..DistOptions::default()
        });

        let resolved = if offline {
            dist.ensure_cached(catalog_ref)
                .await
                .context("catalog ref not cached in offline mode")?
        } else {
            dist.resolve_ref(catalog_ref)
                .await
                .context("failed to fetch catalog ref")?
        };
        let cache_path = resolved
            .cache_path
            .as_ref()
            .ok_or_else(|| anyhow!("catalog ref resolved without cache path"))?;
        std::fs::read(cache_path)
            .with_context(|| format!("failed to read catalog cache {}", cache_path.display()))
    })?;

    parse_catalog_bytes(&bytes)
}

fn block_on<F, T, E>(handle: &Handle, fut: F) -> std::result::Result<T, E>
where
    F: std::future::Future<Output = std::result::Result<T, E>>,
{
    tokio::task::block_in_place(|| handle.block_on(fut))
}

const FIXTURE_EXTENSIONS_JSON: &str = include_str!("../../tests/fixtures/wizard/extensions.json");

#[cfg(test)]
mod tests {
    use super::load_extension_catalog;

    #[test]
    fn default_docs_catalog_ref_loads_from_embedded_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_cwd = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(temp.path()).expect("switch to temp cwd");
        let catalog = load_extension_catalog(
            "file://docs/extensions_capability_packs.catalog.v1.json",
            None,
        )
        .expect("embedded default catalog should load");
        std::env::set_current_dir(original_cwd).expect("restore cwd");

        assert!(
            catalog
                .extension_types
                .iter()
                .any(|extension_type| extension_type.id == "control")
        );
    }
}
