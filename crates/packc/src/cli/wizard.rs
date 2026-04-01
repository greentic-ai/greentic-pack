#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use clap::{Args, Subcommand};
use greentic_qa_lib::{WizardDriver, WizardFrontend, WizardRunConfig};
use greentic_types::pack::extensions::capabilities::CapabilitiesExtensionV1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_yaml_bw::{Mapping, Value as YamlValue};

use crate::cli::add_extension::{
    CapabilityOfferSpec, ensure_capabilities_extension, inject_capability_offer_spec,
};
use crate::cli::wizard_catalog::{
    CatalogQuestion, CatalogQuestionKind, DEFAULT_EXTENSION_CATALOG_DOWNLOAD_URL, ExtensionCatalog,
    ExtensionTemplate, ExtensionType, TemplatePlanStep, load_extension_catalog,
};
use crate::cli::wizard_i18n::{WizardI18n, detect_requested_locale};
use crate::cli::wizard_ui;
use crate::extensions::{CAPABILITIES_EXTENSION_KEY, DEPLOYER_EXTENSION_KEY};
use crate::runtime::RuntimeContext;

const PACK_WIZARD_ID: &str = "greentic-pack.wizard.run";
const PACK_WIZARD_SCHEMA_ID: &str = "greentic-pack.wizard.answers";
const PACK_WIZARD_SCHEMA_VERSION: &str = "1.0.0";
const DEFAULT_EXTENSION_CATALOG_REF: &str =
    "file://docs/extensions_capability_packs.catalog.v1.json";
static FORCED_WIZARD_SCHEMA: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Args, Default)]
pub struct WizardArgs {
    /// Load AnswerDocument JSON and run in non-interactive mode (implicit `run`)
    #[arg(long, value_name = "FILE")]
    pub answers: Option<PathBuf>,
    /// Write AnswerDocument JSON after run (implicit `run`)
    #[arg(long = "emit-answers", value_name = "FILE")]
    pub emit_answers: Option<PathBuf>,
    /// Pin schema version (default: 1.0.0) (implicit `run`)
    #[arg(long = "schema-version", value_name = "VER")]
    pub schema_version: Option<String>,
    /// Allow migrating older AnswerDocument versions (implicit `run`)
    #[arg(long, default_value_t = false)]
    pub migrate: bool,
    /// Record choices without running side effects (implicit `run`)
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    #[command(subcommand)]
    pub command: Option<WizardCommand>,
}

#[derive(Debug, Subcommand)]
pub enum WizardCommand {
    /// Run wizard interactively (default when no subcommand is passed)
    Run(WizardRunArgs),
    /// Validate AnswerDocument input without running side effects
    Validate(WizardValidateArgs),
    /// Apply AnswerDocument input (doctor/build/sign side effects)
    Apply(WizardApplyArgs),
}

#[derive(Debug, Args, Default)]
pub struct WizardRunArgs {
    /// Load AnswerDocument JSON and run in non-interactive mode
    #[arg(long, value_name = "FILE")]
    pub answers: Option<PathBuf>,
    /// Write AnswerDocument JSON after run
    #[arg(long = "emit-answers", value_name = "FILE")]
    pub emit_answers: Option<PathBuf>,
    /// Pin schema version (default: 1.0.0)
    #[arg(long = "schema-version", value_name = "VER")]
    pub schema_version: Option<String>,
    /// Allow migrating older AnswerDocument versions to current target version
    #[arg(long, default_value_t = false)]
    pub migrate: bool,
    /// Record choices without running side effects (for later `wizard apply --answers`)
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct WizardValidateArgs {
    /// Input AnswerDocument JSON
    #[arg(long, value_name = "FILE")]
    pub answers: PathBuf,
    /// Write migrated/normalized AnswerDocument JSON
    #[arg(long = "emit-answers", value_name = "FILE")]
    pub emit_answers: Option<PathBuf>,
    /// Pin schema version (default: 1.0.0)
    #[arg(long = "schema-version", value_name = "VER")]
    pub schema_version: Option<String>,
    /// Allow migrating older AnswerDocument versions to current target version
    #[arg(long, default_value_t = false)]
    pub migrate: bool,
}

#[derive(Debug, Args)]
pub struct WizardApplyArgs {
    /// Input AnswerDocument JSON
    #[arg(long, value_name = "FILE")]
    pub answers: PathBuf,
    /// Write migrated/normalized AnswerDocument JSON
    #[arg(long = "emit-answers", value_name = "FILE")]
    pub emit_answers: Option<PathBuf>,
    /// Pin schema version (default: 1.0.0)
    #[arg(long = "schema-version", value_name = "VER")]
    pub schema_version: Option<String>,
    /// Allow migrating older AnswerDocument versions to current target version
    #[arg(long, default_value_t = false)]
    pub migrate: bool,
}

#[derive(Clone, Copy)]
enum MainChoice {
    CreateApplicationPack,
    UpdateApplicationPack,
    CreateExtensionPack,
    UpdateExtensionPack,
    AddExtension,
    Exit,
}

#[derive(Clone, Copy)]
enum SubmenuAction {
    Back,
    MainMenu,
}

#[derive(Clone, Copy)]
enum RunMode {
    Harness,
    Cli,
}

#[derive(Default)]
struct WizardSession {
    sign_key_path: Option<String>,
    last_pack_dir: Option<PathBuf>,
    dry_run_delegate_pack_dir: Option<PathBuf>,
    create_pack_id: Option<String>,
    create_pack_scaffold: bool,
    dry_run: bool,
    run_delegate_flow: bool,
    run_delegate_component: bool,
    run_doctor: bool,
    run_build: bool,
    flow_wizard_answers: Option<Value>,
    component_wizard_answers: Option<Value>,
    selected_actions: Vec<String>,
    extension_operation: Option<ExtensionOperationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtensionOperationRecord {
    operation: String,
    catalog_ref: String,
    extension_type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    template_id: Option<String>,
    #[serde(default)]
    template_qa_answers: BTreeMap<String, String>,
    #[serde(default)]
    edit_answers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WizardAnswerDocument {
    wizard_id: String,
    schema_id: String,
    schema_version: String,
    locale: String,
    #[serde(default)]
    answers: BTreeMap<String, Value>,
    #[serde(default)]
    locks: BTreeMap<String, Value>,
}

#[derive(Debug)]
struct WizardExecutionPlan {
    pack_dir: PathBuf,
    create_pack_id: Option<String>,
    create_pack_scaffold: bool,
    run_delegate_flow: bool,
    run_delegate_component: bool,
    run_doctor: bool,
    run_build: bool,
    flow_wizard_answers: Option<Value>,
    component_wizard_answers: Option<Value>,
    sign_key_path: Option<String>,
    extension_operation: Option<ExtensionOperationRecord>,
}

struct FlowSchemaContext {
    pack_dir: Option<PathBuf>,
    flow_wizard_answers: Option<Value>,
}

pub(crate) fn set_forced_schema_flag(requested: bool) {
    FORCED_WIZARD_SCHEMA.store(requested, Ordering::Relaxed);
}

fn consume_forced_schema_flag() -> bool {
    FORCED_WIZARD_SCHEMA.swap(false, Ordering::Relaxed)
}

pub fn handle(
    args: WizardArgs,
    runtime: &RuntimeContext,
    requested_locale: Option<&str>,
) -> Result<()> {
    let implicit_run_args = WizardRunArgs {
        answers: args.answers,
        emit_answers: args.emit_answers,
        schema_version: args.schema_version,
        migrate: args.migrate,
        dry_run: args.dry_run,
    };
    let schema_requested = consume_forced_schema_flag();
    match args.command {
        None => run_interactive_command(
            implicit_run_args,
            runtime,
            requested_locale,
            schema_requested,
        ),
        Some(WizardCommand::Run(cmd)) => {
            run_interactive_command(cmd, runtime, requested_locale, schema_requested)
        }
        Some(WizardCommand::Validate(cmd)) => run_validate_command(cmd, requested_locale),
        Some(WizardCommand::Apply(cmd)) => run_apply_command(cmd, requested_locale),
    }
}

pub fn run_with_io<R: BufRead, W: Write>(input: &mut R, output: &mut W) -> Result<()> {
    run_with_mode(
        input,
        output,
        detect_requested_locale().as_deref(),
        RunMode::Harness,
        None,
        false,
    )?;
    Ok(())
}

pub fn run_with_io_and_locale<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    requested_locale: Option<&str>,
) -> Result<()> {
    run_with_mode(
        input,
        output,
        requested_locale,
        RunMode::Harness,
        None,
        false,
    )?;
    Ok(())
}

pub fn run_cli_with_io_and_locale<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    requested_locale: Option<&str>,
) -> Result<()> {
    run_with_mode(input, output, requested_locale, RunMode::Cli, None, false)?;
    Ok(())
}

fn run_with_mode<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    requested_locale: Option<&str>,
    mode: RunMode,
    runtime: Option<&RuntimeContext>,
    dry_run: bool,
) -> Result<WizardSession> {
    let i18n = WizardI18n::new(requested_locale);
    let mut session = WizardSession {
        dry_run,
        ..WizardSession::default()
    };

    loop {
        let choice = ask_main_menu(input, output, &i18n)?;
        match choice {
            MainChoice::CreateApplicationPack => {
                session
                    .selected_actions
                    .push("main.create_application_pack".to_string());
                match mode {
                    RunMode::Harness => {
                        let _ = ask_placeholder_submenu(
                            input,
                            output,
                            &i18n,
                            "wizard.create_application_pack.title",
                        )?;
                    }
                    RunMode::Cli => {
                        run_create_application_pack(input, output, &i18n, &mut session)?;
                    }
                }
            }
            MainChoice::UpdateApplicationPack => {
                session
                    .selected_actions
                    .push("main.update_application_pack".to_string());
                match mode {
                    RunMode::Harness => {
                        let _ = ask_placeholder_submenu(
                            input,
                            output,
                            &i18n,
                            "wizard.update_application_pack.title",
                        )?;
                    }
                    RunMode::Cli => {
                        run_update_application_pack(input, output, &i18n, &mut session)?;
                    }
                }
            }
            MainChoice::CreateExtensionPack => {
                session
                    .selected_actions
                    .push("main.create_extension_pack".to_string());
                match mode {
                    RunMode::Harness => {
                        let _ = ask_placeholder_submenu(
                            input,
                            output,
                            &i18n,
                            "wizard.create_extension_pack.title",
                        )?;
                    }
                    RunMode::Cli => {
                        run_create_extension_pack(input, output, &i18n, runtime, &mut session)?;
                    }
                }
            }
            MainChoice::UpdateExtensionPack => {
                session
                    .selected_actions
                    .push("main.update_extension_pack".to_string());
                match mode {
                    RunMode::Harness => {
                        let _ = ask_placeholder_submenu(
                            input,
                            output,
                            &i18n,
                            "wizard.update_extension_pack.title",
                        )?;
                    }
                    RunMode::Cli => {
                        run_update_extension_pack(input, output, &i18n, &mut session, runtime)?;
                    }
                }
            }
            MainChoice::AddExtension => {
                session
                    .selected_actions
                    .push("main.add_extension".to_string());
                match mode {
                    RunMode::Harness => {
                        let _ = ask_placeholder_submenu(
                            input,
                            output,
                            &i18n,
                            "wizard.main.option.add_extension",
                        )?;
                    }
                    RunMode::Cli => {
                        run_add_extension(input, output, &i18n, &mut session, runtime)?;
                    }
                }
            }
            MainChoice::Exit => {
                session.selected_actions.push("main.exit".to_string());
                return Ok(session);
            }
        }
    }
}

fn run_interactive_command(
    cmd: WizardRunArgs,
    runtime: &RuntimeContext,
    requested_locale: Option<&str>,
    schema_requested: bool,
) -> Result<()> {
    if maybe_print_answer_schema(&cmd, schema_requested)? {
        return Ok(());
    }
    let target_schema_version = target_schema_version(cmd.schema_version.as_deref())?;
    let locale = resolved_locale(requested_locale);
    if let Some(path) = cmd.answers.as_deref() {
        let initial_result = (|| -> Result<()> {
            let doc =
                load_answer_document(path, &target_schema_version, cmd.migrate, requested_locale)?;
            validate_answer_document(&doc)?;
            if !cmd.dry_run {
                apply_answer_document(&doc)?;
            }
            if let Some(out) = cmd.emit_answers.as_deref() {
                write_answer_document(out, &doc)?;
            }
            Ok(())
        })();
        if initial_result.is_ok() {
            return Ok(());
        }

        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        let i18n = WizardI18n::new(requested_locale);
        wizard_ui::render_line(
            &mut output,
            &format!(
                "{}: {}",
                i18n.t("wizard.error.answer_document_failed"),
                initial_result.expect_err("initial wizard answers error")
            ),
        )?;
        let session = run_with_mode(
            &mut input,
            &mut output,
            requested_locale,
            RunMode::Cli,
            Some(runtime),
            cmd.dry_run,
        )?;
        if let Some(path) = cmd.emit_answers.as_deref() {
            let doc = answer_document_from_session(&session, &locale, &target_schema_version)?;
            write_answer_document(path, &doc)?;
        }
        return Ok(());
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let session = run_with_mode(
        &mut input,
        &mut output,
        requested_locale,
        RunMode::Cli,
        Some(runtime),
        cmd.dry_run,
    )?;
    if let Some(path) = cmd.emit_answers.as_deref() {
        let doc = answer_document_from_session(&session, &locale, &target_schema_version)?;
        write_answer_document(path, &doc)?;
    }
    Ok(())
}

fn maybe_print_answer_schema(cmd: &WizardRunArgs, schema_requested: bool) -> Result<bool> {
    if !schema_requested {
        return Ok(false);
    }
    let target_schema_version = target_schema_version(cmd.schema_version.as_deref())?;
    let flow_context = cmd.answers.as_deref().and_then(|path| {
        load_answer_document(path, &target_schema_version, cmd.migrate, None)
            .ok()
            .and_then(|doc| execution_plan_from_answers(&doc.answers).ok())
            .map(|plan| FlowSchemaContext {
                pack_dir: Some(plan.pack_dir),
                flow_wizard_answers: plan.flow_wizard_answers,
            })
    });
    let schema = wizard_answer_schema(&target_schema_version, flow_context.as_ref())?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &schema).context("write wizard schema")?;
    wizard_ui::render_text(&mut output, "\n").context("write wizard schema newline")?;
    Ok(true)
}

fn run_validate_command(cmd: WizardValidateArgs, requested_locale: Option<&str>) -> Result<()> {
    let target_schema_version = target_schema_version(cmd.schema_version.as_deref())?;
    let doc = load_answer_document(
        &cmd.answers,
        &target_schema_version,
        cmd.migrate,
        requested_locale,
    )?;
    validate_answer_document(&doc)?;
    if let Some(path) = cmd.emit_answers.as_deref() {
        write_answer_document(path, &doc)?;
    }
    Ok(())
}

fn run_apply_command(cmd: WizardApplyArgs, requested_locale: Option<&str>) -> Result<()> {
    let target_schema_version = target_schema_version(cmd.schema_version.as_deref())?;
    let doc = load_answer_document(
        &cmd.answers,
        &target_schema_version,
        cmd.migrate,
        requested_locale,
    )?;
    validate_answer_document(&doc)?;
    apply_answer_document(&doc)?;
    if let Some(path) = cmd.emit_answers.as_deref() {
        write_answer_document(path, &doc)?;
    }
    Ok(())
}

fn target_schema_version(schema_version: Option<&str>) -> Result<String> {
    let version = schema_version.unwrap_or(PACK_WIZARD_SCHEMA_VERSION).trim();
    if version.is_empty() {
        return Err(anyhow!("schema version must not be empty"));
    }
    Ok(version.to_string())
}

fn resolved_locale(requested_locale: Option<&str>) -> String {
    let i18n = WizardI18n::new(requested_locale);
    i18n.qa_i18n_config()
        .locale
        .unwrap_or_else(|| "en-GB".to_string())
}

fn load_answer_document(
    path: &Path,
    target_schema_version: &str,
    migrate: bool,
    requested_locale: Option<&str>,
) -> Result<WizardAnswerDocument> {
    let raw = fs::read(path).with_context(|| format!("read answers file {}", path.display()))?;
    let parsed: Value = serde_json::from_slice(&raw)
        .with_context(|| format!("decode answers json {}", path.display()))?;
    normalize_answer_document(parsed, target_schema_version, migrate, requested_locale)
}

fn normalize_answer_document(
    parsed: Value,
    target_schema_version: &str,
    migrate: bool,
    requested_locale: Option<&str>,
) -> Result<WizardAnswerDocument> {
    let mut obj = parsed
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("answers document root must be a JSON object"))?;

    let mut wizard_id = obj
        .remove("wizard_id")
        .and_then(|v| v.as_str().map(ToString::to_string));
    let mut schema_id = obj
        .remove("schema_id")
        .and_then(|v| v.as_str().map(ToString::to_string));
    let mut schema_version = obj
        .remove("schema_version")
        .and_then(|v| v.as_str().map(ToString::to_string));
    let locale = obj
        .remove("locale")
        .and_then(|v| v.as_str().map(ToString::to_string))
        .unwrap_or_else(|| resolved_locale(requested_locale));

    if wizard_id.is_none() || schema_id.is_none() || schema_version.is_none() {
        if !migrate {
            return Err(anyhow!(
                "answers document missing wizard/schema identity; rerun with --migrate"
            ));
        }
        wizard_id.get_or_insert_with(|| PACK_WIZARD_ID.to_string());
        schema_id.get_or_insert_with(|| PACK_WIZARD_SCHEMA_ID.to_string());
        schema_version.get_or_insert_with(|| PACK_WIZARD_SCHEMA_VERSION.to_string());
    }

    if schema_version.as_deref() != Some(target_schema_version) {
        if !migrate {
            return Err(anyhow!(
                "answers schema_version '{}' does not match target '{}'; rerun with --migrate",
                schema_version.as_deref().unwrap_or_default(),
                target_schema_version
            ));
        }
        schema_version = Some(target_schema_version.to_string());
    }

    let answers_value = obj.remove("answers").unwrap_or_else(|| json!({}));
    let locks_value = obj.remove("locks").unwrap_or_else(|| json!({}));
    let answers = json_object_to_btreemap(answers_value, "answers")?;
    let locks = json_object_to_btreemap(locks_value, "locks")?;

    Ok(WizardAnswerDocument {
        wizard_id: wizard_id.unwrap_or_else(|| PACK_WIZARD_ID.to_string()),
        schema_id: schema_id.unwrap_or_else(|| PACK_WIZARD_SCHEMA_ID.to_string()),
        schema_version: schema_version.unwrap_or_else(|| target_schema_version.to_string()),
        locale,
        answers,
        locks,
    })
}

fn json_object_to_btreemap(value: Value, field: &str) -> Result<BTreeMap<String, Value>> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("{field} must be a JSON object"))?;
    Ok(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

fn write_answer_document(path: &Path, doc: &WizardAnswerDocument) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create answers output directory {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(doc).context("serialize answers document")?;
    fs::write(path, bytes).with_context(|| format!("write answers file {}", path.display()))?;
    Ok(())
}

fn answer_document_from_session(
    session: &WizardSession,
    locale: &str,
    schema_version: &str,
) -> Result<WizardAnswerDocument> {
    let pack_dir = match session.last_pack_dir.as_deref() {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let mut answers = BTreeMap::new();
    answers.insert(
        "pack_dir".to_string(),
        Value::String(pack_dir.display().to_string()),
    );
    if session.create_pack_scaffold {
        answers.insert("create_pack_scaffold".to_string(), Value::Bool(true));
    }
    if let Some(pack_id) = session.create_pack_id.as_deref() {
        answers.insert(
            "create_pack_id".to_string(),
            Value::String(pack_id.to_string()),
        );
    }
    answers.insert(
        "run_delegate_flow".to_string(),
        Value::Bool(session.run_delegate_flow),
    );
    answers.insert(
        "run_delegate_component".to_string(),
        Value::Bool(session.run_delegate_component),
    );
    answers.insert("run_doctor".to_string(), Value::Bool(session.run_doctor));
    answers.insert("run_build".to_string(), Value::Bool(session.run_build));
    answers.insert(
        "mode".to_string(),
        Value::String(if session.dry_run {
            "interactive-dry-run".to_string()
        } else {
            "interactive".to_string()
        }),
    );
    answers.insert("dry_run".to_string(), Value::Bool(session.dry_run));
    answers.insert(
        "selected_actions".to_string(),
        Value::Array(
            session
                .selected_actions
                .iter()
                .map(|item| Value::String(item.clone()))
                .collect(),
        ),
    );
    if let Some(flow_answers) = session.flow_wizard_answers.as_ref() {
        answers.insert("flow_wizard_answers".to_string(), flow_answers.clone());
    }
    if let Some(component_answers) = session.component_wizard_answers.as_ref() {
        answers.insert(
            "component_wizard_answers".to_string(),
            component_answers.clone(),
        );
    }
    if let Some(extension) = session.extension_operation.as_ref() {
        answers.insert(
            "extension_operation".to_string(),
            Value::String(extension.operation.clone()),
        );
        answers.insert(
            "extension_catalog_ref".to_string(),
            Value::String(extension.catalog_ref.clone()),
        );
        answers.insert(
            "extension_type_id".to_string(),
            Value::String(extension.extension_type_id.clone()),
        );
        if let Some(template_id) = extension.template_id.as_ref() {
            answers.insert(
                "extension_template_id".to_string(),
                Value::String(template_id.clone()),
            );
        }
        answers.insert(
            "extension_template_qa_answers".to_string(),
            string_map_to_json_value(&extension.template_qa_answers),
        );
        answers.insert(
            "extension_edit_answers".to_string(),
            string_map_to_json_value(&extension.edit_answers),
        );
    }
    if let Some(key) = session.sign_key_path.as_deref() {
        answers.insert("sign".to_string(), Value::Bool(true));
        answers.insert("sign_key_path".to_string(), Value::String(key.to_string()));
    } else {
        answers.insert("sign".to_string(), Value::Bool(false));
    }
    Ok(WizardAnswerDocument {
        wizard_id: PACK_WIZARD_ID.to_string(),
        schema_id: PACK_WIZARD_SCHEMA_ID.to_string(),
        schema_version: schema_version.to_string(),
        locale: locale.to_string(),
        answers,
        locks: BTreeMap::new(),
    })
}

fn wizard_answer_schema(
    schema_version: &str,
    flow_context: Option<&FlowSchemaContext>,
) -> Result<Value> {
    let flow_runtime_schema = load_flow_wizard_runtime_schema(flow_context)?;
    let component_modes = [
        "create",
        "add_operation",
        "update_operation",
        "build_test",
        "doctor",
    ];
    let component_mode_refs = component_modes
        .iter()
        .map(|mode| Value::String(format!("#/$defs/greentic_component_wizard_{mode}")))
        .collect::<Vec<_>>();

    let mut defs = serde_json::Map::new();
    defs.insert(
        "greentic_flow_wizard_runtime_schema".to_string(),
        flow_runtime_schema,
    );
    defs.insert(
        "greentic_flow_wizard_generic_schema".to_string(),
        generic_flow_wizard_schema(),
    );
    defs.insert(
        "greentic_flow_step_answers".to_string(),
        flow_step_answers_schema(),
    );
    defs.insert(
        "greentic_flow_wizard_action".to_string(),
        flow_wizard_action_schema(),
    );
    for mode in component_modes {
        defs.insert(
            format!("greentic_component_wizard_{mode}"),
            load_component_wizard_schema(mode)?,
        );
    }
    defs.insert(
        "greentic_component_wizard_any_mode".to_string(),
        json!({
            "description": "Any greentic-component wizard answer document supported by greentic-pack replay.",
            "oneOf": component_mode_refs
                .iter()
                .map(|reference| json!({ "$ref": reference }))
                .collect::<Vec<_>>(),
        }),
    );

    Ok(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://greenticai.github.io/greentic-pack/schemas/wizard.answers.schema.json",
        "title": "greentic-pack wizard answers",
        "type": "object",
        "additionalProperties": false,
        "$comment": "Nested flow step answers are component-specific. Resolve those contracts by calling `greentic-flow component-schema <file/oci/repo/store>.wasm [--mode default|setup|update|remove]` and pass the resulting schema through to greentic-flow when composing flow wizard answers.",
        "properties": {
            "wizard_id": {
                "type": "string",
                "const": PACK_WIZARD_ID
            },
            "schema_id": {
                "type": "string",
                "const": PACK_WIZARD_SCHEMA_ID
            },
            "schema_version": {
                "type": "string",
                "const": schema_version
            },
            "locale": {
                "type": "string"
            },
            "answers": pack_wizard_answers_schema(),
            "locks": {
                "type": "object",
                "additionalProperties": true
            }
        },
        "required": ["wizard_id", "schema_id", "schema_version", "answers"],
        "$defs": Value::Object(defs),
    }))
}

fn pack_wizard_answers_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pack_dir": { "type": "string" },
            "create_pack_scaffold": { "type": "boolean" },
            "create_pack_id": { "type": "string" },
            "run_delegate_flow": { "type": "boolean" },
            "run_delegate_component": { "type": "boolean" },
            "run_doctor": { "type": "boolean" },
            "run_build": { "type": "boolean" },
            "dry_run": { "type": "boolean" },
            "mode": { "type": "string" },
            "sign": { "type": "boolean" },
            "sign_key_path": { "type": "string" },
            "selected_actions": {
                "type": "array",
                "items": { "type": "string" }
            },
            "flow_wizard_answers": {
                "description": "Nested greentic-flow wizard answers. The generic plan contract is provided here, and the current greentic-flow runtime schema is embedded under #/$defs/greentic_flow_wizard_runtime_schema.",
                "anyOf": [
                    { "$ref": "#/$defs/greentic_flow_wizard_generic_schema" },
                    { "$ref": "#/$defs/greentic_flow_wizard_runtime_schema" }
                ]
            },
            "component_wizard_answers": {
                "description": "Nested greentic-component wizard answers for component-level replay inside greentic-pack.",
                "$ref": "#/$defs/greentic_component_wizard_any_mode"
            },
            "extension_operation": { "type": "string" },
            "extension_catalog_ref": { "type": "string" },
            "extension_type_id": { "type": "string" },
            "extension_template_id": { "type": "string" },
            "extension_template_qa_answers": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            },
            "extension_edit_answers": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        },
        "required": ["pack_dir"]
    })
}

fn generic_flow_wizard_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Generic greentic-flow wizard plan schema embedded by greentic-pack. For a concrete flow plan, also fetch greentic-flow's current runtime schema directly with `greentic-flow wizard <pack> --answers <plan.json> --schema <schema.json>`.",
        "properties": {
            "schema_id": {
                "type": "string",
                "const": "greentic-flow.wizard.plan"
            },
            "schema_version": {
                "type": "string"
            },
            "actions": {
                "type": "array",
                "items": {
                    "$ref": "#/$defs/greentic_flow_wizard_action"
                }
            }
        },
        "required": ["schema_id", "schema_version", "actions"]
    })
}

fn flow_step_answers_schema() -> Value {
    json!({
        "type": "object",
        "description": "Exact step-answer contract resolution is component-specific. Call `greentic-flow component-schema <file/oci/repo/store>.wasm [--mode default|setup|update|remove]` and pass that schema on to greentic-flow when composing nested add-step/update-step/delete-step answers.",
        "$comment": "Resolve per-component step answer schemas via `greentic-flow component-schema <file/oci/repo/store>.wasm [--mode default|setup|update|remove]`.",
        "additionalProperties": true
    })
}

fn flow_step_action_schema(action: &str) -> Value {
    let mut required = vec![json!("action"), json!("flow")];
    if matches!(action, "add-step" | "update-step") {
        required.push(json!("component"));
        required.push(json!("mode"));
    }
    if action == "update-step" {
        required.push(json!("step_id"));
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "action": { "type": "string", "const": action },
            "flow": { "type": "string" },
            "step_id": { "type": "string" },
            "after": { "type": "string" },
            "component": { "type": "string" },
            "mode": {
                "type": "string",
                "enum": ["default", "setup", "update", "remove"]
            },
            "answers": { "$ref": "#/$defs/greentic_flow_step_answers" }
        },
        "required": required
    })
}

fn flow_wizard_action_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "const": "add-flow" },
                    "flow": { "type": "string" },
                    "flow_id": { "type": "string" },
                    "flow_type": { "type": "string" }
                },
                "required": ["action", "flow", "flow_id", "flow_type"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "const": "edit-flow-summary" },
                    "flow": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" }
                },
                "required": ["action", "flow"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "const": "generate-translations" },
                    "locales": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["action", "locales"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "const": "delete-flow" },
                    "flow": { "type": "string" }
                },
                "required": ["action", "flow"]
            },
            flow_step_action_schema("add-step"),
            flow_step_action_schema("update-step"),
            flow_step_action_schema("delete-step")
        ]
    })
}

fn load_flow_wizard_runtime_schema(flow_context: Option<&FlowSchemaContext>) -> Result<Value> {
    let temp = tempfile::tempdir().context("create temp dir for flow wizard schema")?;
    let cwd = flow_context
        .and_then(|ctx| ctx.pack_dir.as_deref())
        .unwrap_or_else(|| temp.path());
    let mut args = vec!["wizard".to_string(), "--schema".to_string()];
    let mut temp_answers_path = None;

    if let Some(ctx) = flow_context
        && let Some(pack_dir) = ctx.pack_dir.as_ref()
    {
        args.push(pack_dir.display().to_string());
        if let Some(flow_answers) = ctx.flow_wizard_answers.as_ref() {
            let answers_path = temp.path().join("flow.answers.json");
            if !write_json_value(&answers_path, flow_answers) {
                return Err(anyhow!(
                    "failed to write temp greentic-flow answers plan {}",
                    answers_path.display()
                ));
            }
            args.push("--answers".to_string());
            args.push(answers_path.display().to_string());
            temp_answers_path = Some(answers_path);
        }
    }

    let result = capture_delegate_json("greentic-flow", &args, cwd)
        .context("failed to fetch nested greentic-flow wizard schema");
    if let Some(path) = temp_answers_path.as_deref() {
        let _ = fs::remove_file(path);
    }
    result
}

fn load_component_wizard_schema(mode: &str) -> Result<Value> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let args = vec![
        "wizard".to_string(),
        "--schema".to_string(),
        "--mode".to_string(),
        mode.to_string(),
    ];
    capture_delegate_json("greentic-component", &args, &cwd)
        .with_context(|| format!("fetch nested greentic-component wizard schema for mode '{mode}'"))
}

fn validate_answer_document(doc: &WizardAnswerDocument) -> Result<()> {
    if doc.wizard_id != PACK_WIZARD_ID {
        return Err(anyhow!(
            "unsupported wizard_id '{}', expected '{}'",
            doc.wizard_id,
            PACK_WIZARD_ID
        ));
    }
    if doc.schema_id != PACK_WIZARD_SCHEMA_ID {
        return Err(anyhow!(
            "unsupported schema_id '{}', expected '{}'",
            doc.schema_id,
            PACK_WIZARD_SCHEMA_ID
        ));
    }
    let plan = execution_plan_from_answers(&doc.answers)?;
    let pack_dir_must_exist = !plan.create_pack_scaffold
        && !matches!(
            plan.extension_operation
                .as_ref()
                .map(|item| item.operation.as_str()),
            Some("create_extension_pack")
        );
    if pack_dir_must_exist && !plan.pack_dir.is_dir() {
        return Err(anyhow!(
            "pack_dir is not an existing directory: {}",
            plan.pack_dir.display()
        ));
    }
    if plan.create_pack_scaffold && plan.create_pack_id.is_none() {
        return Err(anyhow!(
            "create_pack_scaffold=true requires answers.create_pack_id string"
        ));
    }
    if let Some(key) = plan.sign_key_path.as_deref()
        && key.trim().is_empty()
    {
        return Err(anyhow!("sign_key_path must not be empty"));
    }
    if let Some(extension) = plan.extension_operation.as_ref() {
        validate_extension_operation_record(extension)?;
    }
    Ok(())
}

fn apply_answer_document(doc: &WizardAnswerDocument) -> Result<()> {
    let plan = execution_plan_from_answers(&doc.answers)?;
    let self_exe = wizard_self_exe()?;
    if plan.create_pack_scaffold {
        let pack_id = plan
            .create_pack_id
            .as_deref()
            .ok_or_else(|| anyhow!("missing create_pack_id for scaffold apply"))?;
        let scaffold_ok = run_process(
            &self_exe,
            &[
                "new",
                "--dir",
                &plan.pack_dir.display().to_string(),
                pack_id,
            ],
            None,
        )?;
        if !scaffold_ok {
            return Err(anyhow!(
                "wizard apply failed while creating application pack {}",
                plan.pack_dir.display()
            ));
        }
    }
    if let Some(extension) = plan.extension_operation.as_ref() {
        apply_extension_operation(&plan.pack_dir, extension)?;
    }
    if plan.run_delegate_flow {
        let ok = run_flow_delegate_replay(&plan.pack_dir, plan.flow_wizard_answers.as_ref());
        if !ok {
            return Err(anyhow!(
                "wizard apply failed while running flow delegate for {}",
                plan.pack_dir.display()
            ));
        }
    }
    if plan.run_delegate_component {
        let ok =
            run_component_delegate_replay(&plan.pack_dir, plan.component_wizard_answers.as_ref());
        if !ok {
            return Err(anyhow!(
                "wizard apply failed while running component delegate for {}",
                plan.pack_dir.display()
            ));
        }
    }
    if plan.run_doctor || plan.run_build {
        let update_ok = run_process(
            &self_exe,
            &["update", "--in", &plan.pack_dir.display().to_string()],
            None,
        )?;
        if !update_ok {
            return Err(anyhow!(
                "wizard apply failed while syncing pack manifest for {}",
                plan.pack_dir.display()
            ));
        }
    }
    if plan.run_doctor {
        let doctor_ok = run_process(
            &self_exe,
            &["doctor", "--in", &plan.pack_dir.display().to_string()],
            None,
        )?;
        if !doctor_ok {
            return Err(anyhow!(
                "wizard apply failed while running doctor for {}",
                plan.pack_dir.display()
            ));
        }
    }
    if plan.run_build {
        let resolve_ok = run_process(
            &self_exe,
            &["resolve", "--in", &plan.pack_dir.display().to_string()],
            None,
        )?;
        if !resolve_ok {
            return Err(anyhow!(
                "wizard apply failed while running resolve for {}",
                plan.pack_dir.display()
            ));
        }
        let build_ok = run_process(
            &self_exe,
            &["build", "--in", &plan.pack_dir.display().to_string()],
            None,
        )?;
        if !build_ok {
            return Err(anyhow!(
                "wizard apply failed while running build for {}",
                plan.pack_dir.display()
            ));
        }
    }
    if let Some(key_path) = plan.sign_key_path.as_deref() {
        let sign_ok = run_process(
            &self_exe,
            &[
                "sign",
                "--pack",
                &plan.pack_dir.display().to_string(),
                "--key",
                key_path,
            ],
            None,
        )?;
        if !sign_ok {
            return Err(anyhow!(
                "wizard apply failed while signing {}",
                plan.pack_dir.display()
            ));
        }
    }
    Ok(())
}

fn execution_plan_from_answers(answers: &BTreeMap<String, Value>) -> Result<WizardExecutionPlan> {
    let pack_dir_raw = answers
        .get("pack_dir")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("answers.pack_dir must be a string"))?;
    let create_pack_scaffold = answer_bool(answers, "create_pack_scaffold", false)?;
    let create_pack_id = answers
        .get("create_pack_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let run_delegate_flow = answer_bool(answers, "run_delegate_flow", false)?;
    let run_delegate_component = answer_bool(answers, "run_delegate_component", false)?;
    let run_doctor = answer_bool(answers, "run_doctor", true)?;
    let run_build = answer_bool(answers, "run_build", true)?;
    let flow_wizard_answers = answers.get("flow_wizard_answers").cloned();
    let component_wizard_answers = answers.get("component_wizard_answers").cloned();
    let sign = answer_bool(answers, "sign", false)?;
    let sign_key_path = answers
        .get("sign_key_path")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if sign && sign_key_path.is_none() {
        return Err(anyhow!(
            "answers.sign=true requires answers.sign_key_path string"
        ));
    }
    let sign_key_path = if sign { sign_key_path } else { None };
    let extension_operation = parse_extension_operation_record(answers)?;
    Ok(WizardExecutionPlan {
        pack_dir: PathBuf::from(pack_dir_raw),
        create_pack_id,
        create_pack_scaffold,
        run_delegate_flow,
        run_delegate_component,
        run_doctor,
        run_build,
        flow_wizard_answers,
        component_wizard_answers,
        sign_key_path,
        extension_operation,
    })
}

fn answer_bool(answers: &BTreeMap<String, Value>, key: &str, default: bool) -> Result<bool> {
    match answers.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| anyhow!("answers.{key} must be a boolean")),
    }
}

fn string_map_to_json_value(map: &BTreeMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn json_value_to_string_map(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("answers.{field} must be an object"))?;
    let mut map = BTreeMap::new();
    for (key, value) in obj {
        let value = value
            .as_str()
            .ok_or_else(|| anyhow!("answers.{field}.{key} must be a string"))?;
        map.insert(key.clone(), value.to_string());
    }
    Ok(map)
}

fn parse_extension_operation_record(
    answers: &BTreeMap<String, Value>,
) -> Result<Option<ExtensionOperationRecord>> {
    let Some(operation) = answers.get("extension_operation").and_then(Value::as_str) else {
        return Ok(None);
    };
    let catalog_ref = answers
        .get("extension_catalog_ref")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("answers.extension_catalog_ref must be a string"))?;
    let extension_type_id = answers
        .get("extension_type_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("answers.extension_type_id must be a string"))?;
    let template_id = answers
        .get("extension_template_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let template_qa_answers = json_value_to_string_map(
        answers.get("extension_template_qa_answers"),
        "extension_template_qa_answers",
    )?;
    let edit_answers = json_value_to_string_map(
        answers.get("extension_edit_answers"),
        "extension_edit_answers",
    )?;
    Ok(Some(ExtensionOperationRecord {
        operation: operation.to_string(),
        catalog_ref: catalog_ref.to_string(),
        extension_type_id: extension_type_id.to_string(),
        template_id,
        template_qa_answers,
        edit_answers,
    }))
}

fn run_create_extension_pack<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    runtime: Option<&RuntimeContext>,
    session: &mut WizardSession,
) -> Result<()> {
    session
        .selected_actions
        .push("create_extension_pack.start".to_string());
    let catalog_ref = prompt_for_extension_catalog_ref(input, output, i18n)?;

    let catalog = match load_extension_catalog(catalog_ref.trim(), runtime) {
        Ok(value) => value,
        Err(err) => {
            wizard_ui::render_line(
                output,
                &format!("{}: {}", i18n.t("wizard.error.catalog_load_failed"), err),
            )?;
            let nav = ask_failure_nav(input, output, i18n)?;
            if matches!(nav, SubmenuAction::MainMenu) {
                return Ok(());
            }
            return Ok(());
        }
    };

    let type_choice = ask_extension_type(input, output, i18n, &catalog)?;
    if type_choice == "0" || type_choice.eq_ignore_ascii_case("m") {
        return Ok(());
    }

    let selected = catalog
        .extension_types
        .iter()
        .find(|item| item.id == type_choice)
        .ok_or_else(|| anyhow!("selected extension type not found"))?;

    let template = match ask_extension_template(input, output, i18n, selected)? {
        Some(template) => template,
        None => return Ok(()),
    };

    wizard_ui::render_line(
        output,
        &format!(
            "{} {} / {}",
            i18n.t("wizard.create_extension_pack.selected_type"),
            selected.id,
            template.id
        ),
    )?;

    let default_dir = format!("./{}-extension", selected.id.replace('/', "-"));
    let pack_dir = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.create_ext.pack_dir",
        "wizard.create_extension_pack.ask_pack_dir",
        Some("wizard.create_extension_pack.ask_pack_dir_help"),
        Some(&default_dir),
    )?;
    let pack_dir_path = PathBuf::from(pack_dir.trim());
    session.last_pack_dir = Some(pack_dir_path.clone());
    let qa_answers = ask_template_qa_answers(input, output, i18n, &template)?;
    let edit_answers = ask_extension_edit_answers(input, output, i18n, selected)?;
    session.extension_operation = Some(ExtensionOperationRecord {
        operation: "create_extension_pack".to_string(),
        catalog_ref: catalog_ref.trim().to_string(),
        extension_type_id: selected.id.clone(),
        template_id: Some(template.id.clone()),
        template_qa_answers: qa_answers.clone(),
        edit_answers: edit_answers.clone(),
    });
    if session.dry_run {
        wizard_ui::render_line(output, &i18n.t("wizard.dry_run.skipping_template_apply"))?;
    } else {
        if let Err(err) = apply_template_plan(
            &template,
            &pack_dir_path,
            selected,
            i18n,
            &qa_answers,
            &edit_answers,
        ) {
            wizard_ui::render_line(
                output,
                &format!("{}: {err}", i18n.t("wizard.error.template_apply_failed")),
            )?;
            let nav = ask_failure_nav(input, output, i18n)?;
            if matches!(nav, SubmenuAction::MainMenu) {
                return Ok(());
            }
            return Ok(());
        }
        persist_extension_state(
            &pack_dir_path,
            selected,
            &session
                .extension_operation
                .clone()
                .expect("extension operation recorded"),
        )?;
    }

    let self_exe = wizard_self_exe()?;
    let finalized = run_update_validate_sequence(
        input,
        output,
        i18n,
        session,
        &self_exe,
        &pack_dir_path,
        true,
        "wizard.progress.running_finalize",
    )?;
    if !finalized {
        let _ = ask_failure_nav(input, output, i18n)?;
    }
    Ok(())
}

fn ask_extension_type<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    catalog: &ExtensionCatalog,
) -> Result<String> {
    let mut choices = catalog
        .extension_types
        .iter()
        .enumerate()
        .map(|(idx, ext)| {
            (
                (idx + 1).to_string(),
                format!(
                    "{} - {}",
                    ext.display_name(i18n),
                    ext.display_description(i18n)
                ),
                ext.id.clone(),
            )
        })
        .collect::<Vec<_>>();

    let mut menu_choices = choices
        .iter()
        .map(|(menu_id, label, _)| (menu_id.clone(), label.clone()))
        .collect::<Vec<_>>();
    menu_choices.push(("0".to_string(), i18n.t("wizard.nav.back")));
    menu_choices.push(("M".to_string(), i18n.t("wizard.nav.main_menu")));

    let choice = ask_enum_custom_labels_owned(
        input,
        output,
        i18n,
        "pack.wizard.create_ext.type",
        "wizard.create_extension_pack.type_menu.title",
        Some("wizard.create_extension_pack.type_menu.description"),
        &menu_choices,
        "M",
    )?;

    if choice == "0" || choice.eq_ignore_ascii_case("m") {
        return Ok(choice);
    }

    let selected = choices
        .iter_mut()
        .find(|(menu_id, _, _)| menu_id == &choice)
        .map(|(_, _, id)| id.clone())
        .ok_or_else(|| anyhow!("invalid extension type selection"))?;
    Ok(selected)
}

fn ask_extension_template<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    extension_type: &ExtensionType,
) -> Result<Option<ExtensionTemplate>> {
    if extension_type.templates.is_empty() {
        return Err(anyhow!("extension type has no templates"));
    }

    let choices = extension_type
        .templates
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            (
                (idx + 1).to_string(),
                format!(
                    "{} - {}",
                    item.display_name(i18n),
                    item.display_description(i18n)
                ),
                item,
            )
        })
        .collect::<Vec<_>>();

    let mut menu_choices = choices
        .iter()
        .map(|(menu_id, label, _)| (menu_id.clone(), label.clone()))
        .collect::<Vec<_>>();
    menu_choices.push(("0".to_string(), i18n.t("wizard.nav.back")));
    menu_choices.push(("M".to_string(), i18n.t("wizard.nav.main_menu")));

    let choice = ask_enum_custom_labels_owned(
        input,
        output,
        i18n,
        "pack.wizard.create_ext.template",
        "wizard.create_extension_pack.template_menu.title",
        Some("wizard.create_extension_pack.template_menu.description"),
        &menu_choices,
        "M",
    )?;

    if choice == "0" || choice.eq_ignore_ascii_case("m") {
        return Ok(None);
    }

    let selected = choices
        .iter()
        .find(|(menu_id, _, _)| menu_id == &choice)
        .map(|(_, _, template)| (*template).clone())
        .ok_or_else(|| anyhow!("invalid extension template selection"))?;
    Ok(Some(selected))
}

fn apply_template_plan(
    template: &ExtensionTemplate,
    pack_dir: &Path,
    extension_type: &ExtensionType,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<()> {
    ensure_extension_pack_base_scaffold(pack_dir)?;
    for step in &template.plan {
        match step {
            TemplatePlanStep::EnsureDir { paths } => {
                for rel in paths {
                    let target = pack_dir.join(render_template_string(
                        rel,
                        extension_type,
                        template,
                        i18n,
                        qa_answers,
                        edit_answers,
                    ));
                    fs::create_dir_all(&target)
                        .with_context(|| format!("create directory {}", target.display()))?;
                }
            }
            TemplatePlanStep::WriteFiles { files } => {
                for (rel, content) in files {
                    let target = pack_dir.join(render_template_string(
                        rel,
                        extension_type,
                        template,
                        i18n,
                        qa_answers,
                        edit_answers,
                    ));
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("create parent directory {}", parent.display())
                        })?;
                    }
                    let rendered = render_template_content(
                        content,
                        extension_type,
                        template,
                        i18n,
                        qa_answers,
                        edit_answers,
                    );
                    fs::write(&target, rendered)
                        .with_context(|| format!("write file {}", target.display()))?;
                }
            }
            TemplatePlanStep::WriteBinaryFiles { files } => {
                for (rel, encoded) in files {
                    let target = pack_dir.join(render_template_string(
                        rel,
                        extension_type,
                        template,
                        i18n,
                        qa_answers,
                        edit_answers,
                    ));
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("create parent directory {}", parent.display())
                        })?;
                    }
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .with_context(|| {
                            format!("decode base64 binary scaffold for {}", target.display())
                        })?;
                    fs::write(&target, bytes)
                        .with_context(|| format!("write file {}", target.display()))?;
                }
            }
            TemplatePlanStep::RunCli { command, args } => {
                let (rendered_command, rendered_args) = render_run_cli_invocation(
                    command,
                    args,
                    extension_type,
                    template,
                    i18n,
                    qa_answers,
                    edit_answers,
                )?;
                let argv = rendered_args.iter().map(String::as_str).collect::<Vec<_>>();
                let ok = run_process(Path::new(&rendered_command), &argv, Some(pack_dir))
                    .unwrap_or(false);
                if !ok {
                    return Err(anyhow!(
                        "template run_cli step failed: {} {:?}",
                        rendered_command,
                        rendered_args
                    ));
                }
            }
            TemplatePlanStep::Delegate { target, .. } => {
                let ok = match target {
                    greentic_types::WizardTarget::Flow => {
                        let args = flow_delegate_args(pack_dir);
                        run_delegate_owned("greentic-flow", &args, pack_dir)
                    }
                    greentic_types::WizardTarget::Component => {
                        run_delegate("greentic-component", &["wizard"], pack_dir)
                    }
                    _ => false,
                };
                if !ok {
                    return Err(anyhow!(
                        "template delegate step failed for target {:?}",
                        target
                    ));
                }
            }
        }
    }
    Ok(())
}

fn ensure_extension_pack_base_scaffold(pack_dir: &Path) -> Result<()> {
    fs::create_dir_all(pack_dir)
        .with_context(|| format!("create extension pack dir {}", pack_dir.display()))?;

    for rel in ["flows", "components", "i18n", "assets", "qa", "extensions"] {
        let target = pack_dir.join(rel);
        fs::create_dir_all(&target)
            .with_context(|| format!("create directory {}", target.display()))?;
    }

    for (rel, contents) in [
        ("assets/README.md", "Add extension assets here.\n"),
        ("qa/README.md", "Add extension QA/setup documents here.\n"),
    ] {
        let target = pack_dir.join(rel);
        if !target.exists() {
            fs::write(&target, contents)
                .with_context(|| format!("write file {}", target.display()))?;
        }
    }

    Ok(())
}

fn render_template_content(
    content: &str,
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> String {
    render_template_string(
        content,
        extension_type,
        template,
        i18n,
        qa_answers,
        edit_answers,
    )
}

fn render_template_string(
    raw: &str,
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> String {
    let mut rendered = raw
        .replace("{{extension_type_id}}", &extension_type.id)
        .replace(
            "{{extension_type_name}}",
            &extension_type.display_name(i18n),
        )
        .replace("{{template_id}}", &template.id)
        .replace("{{template_name}}", &template.display_name(i18n))
        .replace(
            "{{canonical_extension_key}}",
            extension_type.canonical_extension_key(),
        )
        .replace(
            "{{not_implemented}}",
            &i18n.t("wizard.shared.not_implemented"),
        );
    for (key, value) in qa_answers {
        rendered = rendered.replace(&format!("{{{{qa.{key}}}}}"), value);
    }
    for (key, value) in edit_answers {
        rendered = rendered.replace(&format!("{{{{edit.{key}}}}}"), value);
    }
    rendered
}

fn render_run_cli_invocation(
    command: &str,
    args: &[String],
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<(String, Vec<String>)> {
    let rendered_command = render_template_string(
        command,
        extension_type,
        template,
        i18n,
        qa_answers,
        edit_answers,
    );
    validate_run_cli_token(&rendered_command, "command", true)?;

    let mut rendered_args = Vec::with_capacity(args.len());
    for (idx, arg) in args.iter().enumerate() {
        let rendered = render_template_string(
            arg,
            extension_type,
            template,
            i18n,
            qa_answers,
            edit_answers,
        );
        validate_run_cli_token(&rendered, &format!("arg[{idx}]"), false)?;
        rendered_args.push(rendered);
    }
    Ok((rendered_command, rendered_args))
}

fn validate_run_cli_token(value: &str, field: &str, require_single_word: bool) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!(
            "template run_cli {field} resolved to an empty value"
        ));
    }
    if value.contains("{{") || value.contains("}}") {
        return Err(anyhow!(
            "template run_cli {field} contains unresolved placeholders: {value}"
        ));
    }
    if value
        .chars()
        .any(|ch| ch == '\0' || ch == '\n' || ch == '\r' || ch.is_control())
    {
        return Err(anyhow!(
            "template run_cli {field} contains control characters"
        ));
    }
    if require_single_word && value.chars().any(char::is_whitespace) {
        return Err(anyhow!(
            "template run_cli {field} must not contain whitespace"
        ));
    }
    Ok(())
}

fn ask_template_qa_answers<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    template: &ExtensionTemplate,
) -> Result<BTreeMap<String, String>> {
    let mut answers = BTreeMap::new();
    for question in &template.qa_questions {
        let value = ask_catalog_question(
            input,
            output,
            i18n,
            &format!("pack.wizard.create_ext.qa.{}", question.id),
            question,
        )?;
        answers.insert(question.id.clone(), value);
    }
    Ok(answers)
}

fn ask_extension_edit_answers<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    extension_type: &ExtensionType,
) -> Result<BTreeMap<String, String>> {
    let mut answers = BTreeMap::new();
    let mut create_offer = None;
    let mut requires_setup = None;
    for question in &extension_type.edit_questions {
        let is_offer_field = matches!(
            question.id.as_str(),
            "offer_id"
                | "cap_id"
                | "component_ref"
                | "op"
                | "version"
                | "priority"
                | "requires_setup"
                | "qa_ref"
                | "hook_op_names"
        );
        if is_offer_field && create_offer == Some(false) {
            continue;
        }
        if question.id == "qa_ref" && requires_setup == Some(false) {
            continue;
        }
        let value = ask_catalog_question(
            input,
            output,
            i18n,
            &format!(
                "pack.wizard.update_ext.edit.{}.{}",
                extension_type.id, question.id
            ),
            question,
        )?;
        if question.id == "create_offer" {
            create_offer = Some(value.trim() == "true");
        }
        if question.id == "requires_setup" {
            requires_setup = Some(value.trim() == "true");
        }
        answers.insert(question.id.clone(), value);
    }
    Ok(answers)
}

fn ask_catalog_question<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    form_id: &str,
    question: &CatalogQuestion,
) -> Result<String> {
    match question.kind {
        CatalogQuestionKind::Enum => {
            let choices = question
                .choices
                .iter()
                .enumerate()
                .map(|(idx, choice)| ((idx + 1).to_string(), choice.clone()))
                .collect::<Vec<_>>();
            let mut menu = choices
                .iter()
                .map(|(id, label)| (id.clone(), label.clone()))
                .collect::<Vec<_>>();
            menu.push(("0".to_string(), i18n.t("wizard.nav.back")));
            let default_idx = question
                .default
                .as_deref()
                .and_then(|value| {
                    choices
                        .iter()
                        .find(|(_, label)| label == value)
                        .map(|(idx, _)| idx.as_str())
                })
                .unwrap_or("1");
            let selected = ask_enum_custom_labels_owned(
                input,
                output,
                i18n,
                form_id,
                &question.title_key,
                question.description_key.as_deref(),
                &menu,
                default_idx,
            )?;
            if selected == "0" {
                return Ok(question.default.clone().unwrap_or_default());
            }
            choices
                .iter()
                .find(|(idx, _)| idx == &selected)
                .map(|(_, label)| label.clone())
                .ok_or_else(|| anyhow!("invalid enum selection for {}", question.id))
        }
        CatalogQuestionKind::Boolean => {
            let selected = ask_enum(
                input,
                output,
                i18n,
                form_id,
                &question.title_key,
                question.description_key.as_deref(),
                &[
                    ("1", "wizard.bool.true"),
                    ("2", "wizard.bool.false"),
                    ("0", "wizard.nav.back"),
                ],
                if question.default.as_deref() == Some("false") {
                    "2"
                } else {
                    "1"
                },
            )?;
            match selected.as_str() {
                "1" => Ok("true".to_string()),
                "2" => Ok("false".to_string()),
                "0" => Ok(question
                    .default
                    .clone()
                    .unwrap_or_else(|| "false".to_string())),
                _ => Err(anyhow!("invalid boolean selection")),
            }
        }
        CatalogQuestionKind::Integer => loop {
            let value = ask_text(
                input,
                output,
                i18n,
                form_id,
                &question.title_key,
                question.description_key.as_deref(),
                question.default.as_deref(),
            )?;
            if value.trim().parse::<i64>().is_ok() {
                break Ok(value);
            }
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
        },
        CatalogQuestionKind::String => ask_text(
            input,
            output,
            i18n,
            form_id,
            &question.title_key,
            question.description_key.as_deref(),
            question.default.as_deref(),
        ),
    }
}

fn persist_extension_edit_answers(
    pack_dir: &Path,
    extension_type: &ExtensionType,
    operation: &ExtensionOperationRecord,
) -> Result<()> {
    validate_capability_offer_component_ref(
        pack_dir,
        extension_type,
        &operation.template_qa_answers,
        &operation.edit_answers,
    )?;
    let dir = pack_dir.join("extensions");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(format!("{}.json", extension_type.id));
    let mut payload = json!({
        "extension_type": extension_type.id,
        "canonical_extension_key": extension_type.canonical_extension_key(),
        "operation": operation.operation,
        "catalog_ref": operation.catalog_ref,
        "template_id": operation.template_id,
        "template_qa_answers": operation.template_qa_answers,
        "edit_answers": operation.edit_answers,
    });
    if uses_capabilities_extension(extension_type) {
        payload["capabilities_extension"] = serde_json::to_value(build_capabilities_payload(
            extension_type,
            &operation.template_qa_answers,
            &operation.edit_answers,
        )?)
        .context("serialize capabilities extension payload")?;
    } else if uses_deployer_extension(extension_type) {
        payload["deployer_extension"] = build_deployer_payload(
            extension_type,
            &operation.template_qa_answers,
            &operation.edit_answers,
        )?;
    }
    let bytes =
        serde_json::to_vec_pretty(&payload).context("serialize extension edit answers payload")?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    merge_extension_answers_into_pack_yaml(
        pack_dir,
        extension_type,
        &operation.template_qa_answers,
        &operation.edit_answers,
    )?;
    Ok(())
}

fn merge_extension_answers_into_pack_yaml(
    pack_dir: &Path,
    extension_type: &ExtensionType,
    template_qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<()> {
    if !uses_capabilities_extension(extension_type) {
        if uses_deployer_extension(extension_type) {
            let pack_yaml = pack_dir.join("pack.yaml");
            if !pack_yaml.exists() {
                return Ok(());
            }
            let contents = fs::read_to_string(&pack_yaml)
                .with_context(|| format!("read {}", pack_yaml.display()))?;
            let serialized = inject_deployer_extension_payload(
                &contents,
                &build_deployer_payload(extension_type, template_qa_answers, edit_answers)?,
            )?;
            fs::write(&pack_yaml, serialized)
                .with_context(|| format!("write {}", pack_yaml.display()))?;
        }
        return Ok(());
    }
    let pack_yaml = pack_dir.join("pack.yaml");
    if !pack_yaml.exists() {
        return Ok(());
    }
    let contents =
        fs::read_to_string(&pack_yaml).with_context(|| format!("read {}", pack_yaml.display()))?;
    let capabilities =
        build_capabilities_payload(extension_type, template_qa_answers, edit_answers)?;
    let serialized = if let Some(spec) =
        capability_offer_spec_from_answers(extension_type, template_qa_answers, edit_answers)?
    {
        inject_capability_offer_spec(&contents, &spec)?
    } else {
        ensure_capabilities_extension(&contents)?
    };
    let _ = capabilities;
    fs::write(&pack_yaml, serialized).with_context(|| format!("write {}", pack_yaml.display()))?;
    Ok(())
}

fn validate_capability_offer_component_ref(
    pack_dir: &Path,
    extension_type: &ExtensionType,
    template_qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<()> {
    if !uses_capabilities_extension(extension_type) {
        return Ok(());
    }
    let Some(spec) =
        capability_offer_spec_from_answers(extension_type, template_qa_answers, edit_answers)?
    else {
        return Ok(());
    };
    let pack_yaml = pack_dir.join("pack.yaml");
    if !pack_yaml.exists() {
        return Ok(());
    }
    let config = crate::config::load_pack_config(pack_dir)?;
    if config
        .components
        .iter()
        .any(|item| item.id == spec.component_ref)
    {
        return Ok(());
    }
    Err(anyhow!(
        "capability offer component_ref `{}` does not match any components[].id in pack.yaml; scaffold a component with that id or set create_offer=false",
        spec.component_ref
    ))
}

fn persist_extension_state(
    pack_dir: &Path,
    extension_type: &ExtensionType,
    operation: &ExtensionOperationRecord,
) -> Result<()> {
    persist_extension_edit_answers(pack_dir, extension_type, operation)
}

fn build_capabilities_payload(
    extension_type: &ExtensionType,
    template_qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<CapabilitiesExtensionV1> {
    let offer =
        capability_offer_spec_from_answers(extension_type, template_qa_answers, edit_answers)?.map(
            |spec| greentic_types::pack::extensions::capabilities::CapabilityOfferV1 {
                offer_id: spec.offer_id,
                cap_id: spec.cap_id,
                version: spec.version,
                provider: greentic_types::pack::extensions::capabilities::CapabilityProviderRefV1 {
                    component_ref: spec.component_ref,
                    op: spec.op,
                },
                scope: None,
                priority: spec.priority,
                requires_setup: spec.requires_setup,
                setup: spec.qa_ref.map(|qa_ref| {
                    greentic_types::pack::extensions::capabilities::CapabilitySetupV1 { qa_ref }
                }),
                applies_to: (!spec.hook_op_names.is_empty()).then_some(
                    greentic_types::pack::extensions::capabilities::CapabilityHookAppliesToV1 {
                        op_names: spec.hook_op_names,
                    },
                ),
            },
        );
    Ok(CapabilitiesExtensionV1::new(offer.into_iter().collect()))
}

fn build_deployer_payload(
    _extension_type: &ExtensionType,
    _template_qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<Value> {
    let contract_id = required_answer(edit_answers, "contract_id")?;
    let ops = optional_answer(edit_answers, "supported_ops")
        .unwrap_or_else(|| "generate,plan,apply,destroy,status,rollback".to_string())
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if ops.is_empty() {
        return Err(anyhow!("missing required answer `supported_ops`"));
    }
    let flow_refs = ops
        .iter()
        .map(|op| (op.clone(), Value::String(format!("flows/{op}.ygtc"))))
        .collect::<serde_json::Map<_, _>>();

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

fn capability_offer_spec_from_answers(
    extension_type: &ExtensionType,
    template_qa_answers: &BTreeMap<String, String>,
    edit_answers: &BTreeMap<String, String>,
) -> Result<Option<CapabilityOfferSpec>> {
    let create_offer = match edit_answers.get("create_offer").map(|value| value.trim()) {
        None | Some("") => false,
        Some("true") => true,
        Some("false") => false,
        Some(other) => return Err(anyhow!("invalid create_offer value `{other}`")),
    };
    if !create_offer {
        return Ok(None);
    }

    let offer_id = required_answer(edit_answers, "offer_id")?;
    let cap_id = required_answer(edit_answers, "cap_id")?;
    let component_ref = required_answer(edit_answers, "component_ref")?;
    let op = required_answer(edit_answers, "op")?;
    let version = optional_answer(edit_answers, "version")
        .unwrap_or_else(|| default_capability_version(extension_type));
    let priority = optional_answer(edit_answers, "priority")
        .unwrap_or_else(|| "0".to_string())
        .parse::<i32>()
        .with_context(|| format!("invalid priority for extension type {}", extension_type.id))?;
    let requires_setup = matches!(
        edit_answers.get("requires_setup").map(|value| value.trim()),
        Some("true")
    );
    let qa_ref = if requires_setup {
        optional_answer(edit_answers, "qa_ref")
            .or_else(|| optional_answer(template_qa_answers, "qa_ref"))
    } else {
        None
    };
    if requires_setup && qa_ref.is_none() {
        return Err(anyhow!(
            "extension type {} requires qa_ref when requires_setup=true",
            extension_type.id
        ));
    }
    let hook_op_names = optional_answer(edit_answers, "hook_op_names")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(Some(CapabilityOfferSpec {
        offer_id,
        cap_id,
        version,
        component_ref,
        op,
        priority,
        requires_setup,
        qa_ref,
        hook_op_names,
    }))
}

fn required_answer(answers: &BTreeMap<String, String>, key: &str) -> Result<String> {
    answers
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing required answer `{key}`"))
}

fn optional_answer(answers: &BTreeMap<String, String>, key: &str) -> Option<String> {
    answers
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn default_capability_version(_extension_type: &ExtensionType) -> String {
    "v1".to_string()
}

fn inject_deployer_extension_payload(contents: &str, payload: &Value) -> Result<String> {
    let mut document: YamlValue = serde_yaml_bw::from_str(contents)
        .context("parse pack.yaml for deployer extension merge")?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("pack.yaml root must be a mapping"))?;
    let extensions = mapping
        .entry(yaml_key("extensions"))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extensions_map = extensions
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("extensions must be a mapping"))?;
    let extension_slot = extensions_map
        .entry(yaml_key(DEPLOYER_EXTENSION_KEY))
        .or_insert_with(|| YamlValue::Mapping(Mapping::new()));
    let extension_map = extension_slot
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("deployer extension slot must be a mapping"))?;
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

fn yaml_key(key: &str) -> YamlValue {
    YamlValue::String(key.to_string(), None)
}

fn uses_capabilities_extension(extension_type: &ExtensionType) -> bool {
    extension_type.canonical_extension_key() == CAPABILITIES_EXTENSION_KEY
}

fn uses_deployer_extension(extension_type: &ExtensionType) -> bool {
    extension_type.canonical_extension_key() == DEPLOYER_EXTENSION_KEY
}

fn validate_extension_operation_record(operation: &ExtensionOperationRecord) -> Result<()> {
    match operation.operation.as_str() {
        "create_extension_pack" | "update_extension_pack" | "add_extension" => {}
        other => {
            return Err(anyhow!(
                "unsupported extension operation `{other}` in answers document"
            ));
        }
    }
    if operation.catalog_ref.trim().is_empty() {
        return Err(anyhow!("extension catalog ref must not be empty"));
    }
    if operation.extension_type_id.trim().is_empty() {
        return Err(anyhow!("extension type id must not be empty"));
    }
    if operation.operation == "create_extension_pack" && operation.template_id.is_none() {
        return Err(anyhow!(
            "create_extension_pack requires answers.extension_template_id"
        ));
    }
    Ok(())
}

fn apply_extension_operation(pack_dir: &Path, operation: &ExtensionOperationRecord) -> Result<()> {
    let catalog = load_extension_catalog(&operation.catalog_ref, None)?;
    let extension_type = catalog
        .extension_types
        .iter()
        .find(|item| item.id == operation.extension_type_id)
        .ok_or_else(|| {
            anyhow!(
                "extension type `{}` not found in catalog",
                operation.extension_type_id
            )
        })?;

    if operation.operation == "create_extension_pack" {
        let template_id = operation
            .template_id
            .as_deref()
            .ok_or_else(|| anyhow!("missing template_id for create_extension_pack"))?;
        let template = extension_type
            .templates
            .iter()
            .find(|item| item.id == template_id)
            .ok_or_else(|| anyhow!("template `{template_id}` not found in catalog"))?;
        let i18n = WizardI18n::new(Some("en-GB"));
        apply_template_plan(
            template,
            pack_dir,
            extension_type,
            &i18n,
            &operation.template_qa_answers,
            &operation.edit_answers,
        )?;
    }

    persist_extension_state(pack_dir, extension_type, operation)
}

fn ask_main_menu<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
) -> Result<MainChoice> {
    let choice = ask_enum(
        input,
        output,
        i18n,
        "pack.wizard.main",
        "wizard.main.title",
        Some("wizard.main.description"),
        &[
            ("1", "wizard.main.option.create_application_pack"),
            ("2", "wizard.main.option.update_application_pack"),
            ("3", "wizard.main.option.create_extension_pack"),
            ("4", "wizard.main.option.update_extension_pack"),
            ("5", "wizard.main.option.add_extension"),
            ("0", "wizard.main.option.exit"),
        ],
        "0",
    )?;
    MainChoice::from_choice(&choice)
}

fn ask_placeholder_submenu<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    title_key: &str,
) -> Result<SubmenuAction> {
    let choice = ask_enum(
        input,
        output,
        i18n,
        "pack.wizard.placeholder",
        title_key,
        Some("wizard.shared.not_implemented"),
        &[("0", "wizard.nav.back"), ("M", "wizard.nav.main_menu")],
        "M",
    )?;
    SubmenuAction::from_choice(&choice)
}

fn run_create_application_pack<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
) -> Result<()> {
    session
        .selected_actions
        .push("create_application_pack.start".to_string());
    let pack_id = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.create_app.pack_id",
        "wizard.create_application_pack.ask_pack_id",
        None,
        None,
    )?;

    let pack_dir_default = format!("./{pack_id}");
    let pack_dir = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.create_app.pack_dir",
        "wizard.create_application_pack.ask_pack_dir",
        Some("wizard.create_application_pack.ask_pack_dir_help"),
        Some(&pack_dir_default),
    )?;

    let pack_dir_path = PathBuf::from(pack_dir.trim());
    session.last_pack_dir = Some(pack_dir_path.clone());
    session.create_pack_scaffold = true;
    session.create_pack_id = Some(pack_id.clone());
    let self_exe = wizard_self_exe()?;

    let scaffold_ok = if session.dry_run {
        wizard_ui::render_line(output, &i18n.t("wizard.dry_run.skipping_scaffold"))?;
        let temp_pack_dir = temp_answers_path("greentic-pack-dry-run-pack");
        let ok = run_process(
            &self_exe,
            &[
                "new",
                "--dir",
                &temp_pack_dir.display().to_string(),
                &pack_id,
            ],
            None,
        )?;
        if ok {
            session.dry_run_delegate_pack_dir = Some(temp_pack_dir);
        }
        ok
    } else {
        run_process(
            &self_exe,
            &[
                "new",
                "--dir",
                &pack_dir_path.display().to_string(),
                &pack_id,
            ],
            None,
        )?
    };
    if !scaffold_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.create_app_failed"))?;
        let nav = ask_failure_nav(input, output, i18n)?;
        if matches!(nav, SubmenuAction::MainMenu) {
            return Ok(());
        }
        return Ok(());
    }

    loop {
        let delegate_pack_dir = session
            .dry_run_delegate_pack_dir
            .as_deref()
            .unwrap_or(&pack_dir_path)
            .to_path_buf();
        let setup_choice = ask_enum(
            input,
            output,
            i18n,
            "pack.wizard.create_app.setup",
            "wizard.create_application_pack.setup.title",
            Some("wizard.create_application_pack.setup.description"),
            &[
                (
                    "1",
                    "wizard.create_application_pack.setup.option.edit_flows",
                ),
                (
                    "2",
                    "wizard.create_application_pack.setup.option.add_edit_components",
                ),
                ("3", "wizard.create_application_pack.setup.option.finalize"),
                ("0", "wizard.nav.back"),
                ("M", "wizard.nav.main_menu"),
            ],
            "M",
        )?;

        match setup_choice.as_str() {
            "1" => {
                session.run_delegate_flow = true;
                let delegate_ok = run_flow_delegate_for_session(session, &delegate_pack_dir);
                if !delegate_ok
                    && handle_delegate_failure(
                        input,
                        output,
                        i18n,
                        session,
                        "wizard.error.delegate_flow_failed",
                    )?
                {
                    return Ok(());
                }
            }
            "2" => {
                session.run_delegate_component = true;
                let delegate_ok = run_component_delegate_for_session(session, &delegate_pack_dir);
                if !delegate_ok
                    && handle_delegate_failure(
                        input,
                        output,
                        i18n,
                        session,
                        "wizard.error.delegate_component_failed",
                    )?
                {
                    return Ok(());
                }
            }
            "3" => {
                if finalize_create_app(input, output, i18n, session, &self_exe, &pack_dir_path)? {
                    return Ok(());
                }
            }
            "0" | "M" | "m" => return Ok(()),
            _ => {
                wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            }
        }
    }
}

fn finalize_create_app<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    self_exe: &Path,
    pack_dir_path: &Path,
) -> Result<bool> {
    run_update_validate_sequence(
        input,
        output,
        i18n,
        session,
        self_exe,
        pack_dir_path,
        true,
        "wizard.progress.running_finalize",
    )
}

fn run_update_application_pack<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
) -> Result<()> {
    let pack_dir_path = ask_existing_pack_dir(
        input,
        output,
        i18n,
        "pack.wizard.update_app.pack_dir",
        "wizard.update_application_pack.ask_pack_dir",
        Some("wizard.update_application_pack.ask_pack_dir_help"),
        Some("."),
    )?;
    session.last_pack_dir = Some(pack_dir_path.clone());
    let self_exe = wizard_self_exe()?;

    loop {
        let choice = ask_enum(
            input,
            output,
            i18n,
            "pack.wizard.update_app.menu",
            "wizard.update_application_pack.menu.title",
            Some("wizard.update_application_pack.menu.description"),
            &[
                ("1", "wizard.update_application_pack.menu.option.edit_flows"),
                (
                    "2",
                    "wizard.update_application_pack.menu.option.add_edit_components",
                ),
                (
                    "3",
                    "wizard.update_application_pack.menu.option.run_update_validate",
                ),
                ("4", "wizard.update_application_pack.menu.option.sign"),
                ("0", "wizard.nav.back"),
                ("M", "wizard.nav.main_menu"),
            ],
            "M",
        )?;

        match choice.as_str() {
            "1" => {
                session
                    .selected_actions
                    .push("update_application_pack.edit_flows".to_string());
                session.run_delegate_flow = true;
                let delegate_ok = run_flow_delegate_for_session(session, &pack_dir_path);
                if delegate_ok {
                    let _ = run_update_validate_sequence(
                        input,
                        output,
                        i18n,
                        session,
                        &self_exe,
                        &pack_dir_path,
                        true,
                        "wizard.progress.auto_run_update_validate",
                    )?;
                } else if handle_delegate_failure(
                    input,
                    output,
                    i18n,
                    session,
                    "wizard.error.delegate_flow_failed",
                )? {
                    return Ok(());
                }
            }
            "2" => {
                session
                    .selected_actions
                    .push("update_application_pack.add_edit_components".to_string());
                session.run_delegate_component = true;
                let delegate_ok = run_component_delegate_for_session(session, &pack_dir_path);
                if delegate_ok {
                    let _ = run_update_validate_sequence(
                        input,
                        output,
                        i18n,
                        session,
                        &self_exe,
                        &pack_dir_path,
                        true,
                        "wizard.progress.auto_run_update_validate",
                    )?;
                } else if handle_delegate_failure(
                    input,
                    output,
                    i18n,
                    session,
                    "wizard.error.delegate_component_failed",
                )? {
                    return Ok(());
                }
            }
            "3" => {
                session
                    .selected_actions
                    .push("update_application_pack.run_update_validate".to_string());
                let _ = run_update_validate_sequence(
                    input,
                    output,
                    i18n,
                    session,
                    &self_exe,
                    &pack_dir_path,
                    true,
                    "wizard.progress.running_update_validate",
                )?;
            }
            "4" => {
                session
                    .selected_actions
                    .push("update_application_pack.sign".to_string());
                let _ = run_sign_for_pack(input, output, i18n, session, &self_exe, &pack_dir_path)?;
            }
            "0" | "M" | "m" => return Ok(()),
            _ => {
                wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            }
        }
    }
}

fn run_update_extension_pack<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    runtime: Option<&RuntimeContext>,
) -> Result<()> {
    session
        .selected_actions
        .push("update_extension_pack.start".to_string());
    let pack_dir_path = ask_existing_pack_dir(
        input,
        output,
        i18n,
        "pack.wizard.update_ext.pack_dir",
        "wizard.update_extension_pack.ask_pack_dir",
        Some("wizard.update_extension_pack.ask_pack_dir_help"),
        Some("."),
    )?;
    session.last_pack_dir = Some(pack_dir_path.clone());
    let catalog_ref = prompt_for_extension_catalog_ref(input, output, i18n)?;

    let catalog = match load_extension_catalog(catalog_ref.trim(), runtime) {
        Ok(value) => value,
        Err(err) => {
            wizard_ui::render_line(
                output,
                &format!("{}: {}", i18n.t("wizard.error.catalog_load_failed"), err),
            )?;
            let nav = ask_failure_nav(input, output, i18n)?;
            if matches!(nav, SubmenuAction::MainMenu) {
                return Ok(());
            }
            return Ok(());
        }
    };

    let self_exe = wizard_self_exe()?;

    loop {
        let choice = ask_enum(
            input,
            output,
            i18n,
            "pack.wizard.update_ext.menu",
            "wizard.update_extension_pack.menu.title",
            Some("wizard.update_extension_pack.menu.description"),
            &[
                ("1", "wizard.update_extension_pack.menu.option.edit_entries"),
                ("2", "wizard.update_extension_pack.menu.option.edit_flows"),
                (
                    "3",
                    "wizard.update_extension_pack.menu.option.add_edit_components",
                ),
                (
                    "4",
                    "wizard.update_extension_pack.menu.option.run_update_validate",
                ),
                ("5", "wizard.update_extension_pack.menu.option.sign"),
                ("0", "wizard.nav.back"),
                ("M", "wizard.nav.main_menu"),
            ],
            "M",
        )?;

        match choice.as_str() {
            "1" => {
                let type_choice = ask_extension_type(input, output, i18n, &catalog)?;
                if type_choice == "0" || type_choice.eq_ignore_ascii_case("m") {
                    continue;
                }
                let selected = catalog
                    .extension_types
                    .iter()
                    .find(|item| item.id == type_choice)
                    .ok_or_else(|| anyhow!("selected extension type not found"))?;
                let answers = ask_extension_edit_answers(input, output, i18n, selected)?;
                let operation = ExtensionOperationRecord {
                    operation: "update_extension_pack".to_string(),
                    catalog_ref: catalog_ref.trim().to_string(),
                    extension_type_id: selected.id.clone(),
                    template_id: None,
                    template_qa_answers: BTreeMap::new(),
                    edit_answers: answers.clone(),
                };
                session.extension_operation = Some(operation.clone());
                if !session.dry_run {
                    persist_extension_edit_answers(&pack_dir_path, selected, &operation)?;
                } else {
                    wizard_ui::render_line(
                        output,
                        &i18n.t("wizard.dry_run.skipping_edit_entry_persist"),
                    )?;
                }
                wizard_ui::render_line(
                    output,
                    &format!(
                        "{} {}",
                        i18n.t("wizard.update_extension_pack.edited_entry"),
                        type_choice
                    ),
                )?;
            }
            "2" => {
                session.run_delegate_flow = true;
                let delegate_ok = run_flow_delegate_for_session(session, &pack_dir_path);
                if !delegate_ok
                    && handle_delegate_failure(
                        input,
                        output,
                        i18n,
                        session,
                        "wizard.error.delegate_flow_failed",
                    )?
                {
                    return Ok(());
                }
            }
            "3" => {
                session.run_delegate_component = true;
                let delegate_ok = run_component_delegate_for_session(session, &pack_dir_path);
                if !delegate_ok
                    && handle_delegate_failure(
                        input,
                        output,
                        i18n,
                        session,
                        "wizard.error.delegate_component_failed",
                    )?
                {
                    return Ok(());
                }
            }
            "4" => {
                let _ = run_update_validate_sequence(
                    input,
                    output,
                    i18n,
                    session,
                    &self_exe,
                    &pack_dir_path,
                    true,
                    "wizard.progress.running_update_validate",
                )?;
            }
            "5" => {
                let _ = run_sign_for_pack(input, output, i18n, session, &self_exe, &pack_dir_path)?;
            }
            "0" | "M" | "m" => return Ok(()),
            _ => {
                wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            }
        }
    }
}

fn run_add_extension<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    runtime: Option<&RuntimeContext>,
) -> Result<()> {
    session
        .selected_actions
        .push("add_extension.start".to_string());
    let pack_dir_path = ask_existing_pack_dir(
        input,
        output,
        i18n,
        "pack.wizard.add_ext.pack_dir",
        "wizard.update_extension_pack.ask_pack_dir",
        Some("wizard.update_extension_pack.ask_pack_dir_help"),
        Some("."),
    )?;
    session.last_pack_dir = Some(pack_dir_path.clone());
    let catalog_ref = prompt_for_extension_catalog_ref(input, output, i18n)?;

    let catalog = match load_extension_catalog(catalog_ref.trim(), runtime) {
        Ok(value) => value,
        Err(err) => {
            wizard_ui::render_line(
                output,
                &format!("{}: {}", i18n.t("wizard.error.catalog_load_failed"), err),
            )?;
            let nav = ask_failure_nav(input, output, i18n)?;
            if matches!(nav, SubmenuAction::MainMenu) {
                return Ok(());
            }
            return Ok(());
        }
    };

    let type_choice = ask_extension_type(input, output, i18n, &catalog)?;
    if type_choice == "0" || type_choice.eq_ignore_ascii_case("m") {
        return Ok(());
    }
    let selected = catalog
        .extension_types
        .iter()
        .find(|item| item.id == type_choice)
        .ok_or_else(|| anyhow!("selected extension type not found"))?;
    let answers = ask_extension_edit_answers(input, output, i18n, selected)?;
    let operation = ExtensionOperationRecord {
        operation: "add_extension".to_string(),
        catalog_ref: catalog_ref.trim().to_string(),
        extension_type_id: selected.id.clone(),
        template_id: None,
        template_qa_answers: BTreeMap::new(),
        edit_answers: answers.clone(),
    };
    session.extension_operation = Some(operation.clone());
    if !session.dry_run {
        persist_extension_edit_answers(&pack_dir_path, selected, &operation)?;
        wizard_ui::render_line(output, &i18n.t("cli.wizard.updated_pack_yaml"))?;
    } else {
        wizard_ui::render_line(output, &i18n.t("cli.wizard.dry_run.update_pack_yaml"))?;
        let extension_path = pack_dir_path
            .join("extensions")
            .join(format!("{}.json", selected.id));
        let would_write = i18n.t("cli.wizard.dry_run.would_write").replacen(
            "{}",
            &extension_path.display().to_string(),
            1,
        );
        wizard_ui::render_line(output, &would_write)?;
    }
    session
        .selected_actions
        .push("add_extension.edit_entries".to_string());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_update_validate_sequence<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    self_exe: &Path,
    pack_dir_path: &Path,
    prompt_sign_after: bool,
    progress_key: &str,
) -> Result<bool> {
    session.run_doctor = true;
    session.run_build = true;
    session
        .selected_actions
        .push("pipeline.update_validate".to_string());
    if session.dry_run {
        wizard_ui::render_line(output, &i18n.t(progress_key))?;
        wizard_ui::render_line(output, &i18n.t("wizard.progress.running_doctor"))?;
        wizard_ui::render_line(output, &i18n.t("wizard.progress.running_build"))?;
        return if prompt_sign_after {
            run_sign_prompt_after_finalize(input, output, i18n, session, self_exe, pack_dir_path)
        } else {
            Ok(true)
        };
    }

    wizard_ui::render_line(output, &i18n.t(progress_key))?;
    let update_ok = run_process(
        self_exe,
        &["update", "--in", &pack_dir_path.display().to_string()],
        None,
    )?;
    if !update_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.finalize_build_failed"))?;
        return Ok(false);
    }
    wizard_ui::render_line(output, &i18n.t("wizard.progress.running_doctor"))?;
    let doctor_ok = run_process(
        self_exe,
        &["doctor", "--in", &pack_dir_path.display().to_string()],
        None,
    )?;
    if !doctor_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.finalize_doctor_failed"))?;
        return Ok(false);
    }

    let resolve_ok = run_process(
        self_exe,
        &["resolve", "--in", &pack_dir_path.display().to_string()],
        None,
    )?;
    if !resolve_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.finalize_build_failed"))?;
        return Ok(false);
    }

    wizard_ui::render_line(output, &i18n.t("wizard.progress.running_build"))?;
    let build_ok = run_process(
        self_exe,
        &["build", "--in", &pack_dir_path.display().to_string()],
        None,
    )?;
    if !build_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.finalize_build_failed"))?;
        return Ok(false);
    }

    if prompt_sign_after {
        run_sign_prompt_after_finalize(input, output, i18n, session, self_exe, pack_dir_path)
    } else {
        Ok(true)
    }
}

fn run_sign_prompt_after_finalize<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    self_exe: &Path,
    pack_dir_path: &Path,
) -> Result<bool> {
    let sign_choice = ask_enum(
        input,
        output,
        i18n,
        "pack.wizard.sign_prompt",
        "wizard.sign.after_finalize.title",
        Some("wizard.sign.after_finalize.description"),
        &[
            ("1", "wizard.sign.after_finalize.option.sign_now"),
            ("2", "wizard.sign.after_finalize.option.skip"),
            ("0", "wizard.nav.back"),
            ("M", "wizard.nav.main_menu"),
        ],
        "2",
    )?;

    match sign_choice.as_str() {
        "2" => {
            session
                .selected_actions
                .push("pipeline.sign_prompt.skip".to_string());
            Ok(true)
        }
        "M" | "m" => {
            session
                .selected_actions
                .push("pipeline.sign_prompt.main_menu".to_string());
            Ok(true)
        }
        "0" => {
            session
                .selected_actions
                .push("pipeline.sign_prompt.back".to_string());
            Ok(false)
        }
        "1" => run_sign_for_pack(input, output, i18n, session, self_exe, pack_dir_path),
        _ => {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            Ok(false)
        }
    }
}

fn run_sign_for_pack<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &mut WizardSession,
    self_exe: &Path,
    pack_dir_path: &Path,
) -> Result<bool> {
    session.selected_actions.push("pipeline.sign".to_string());
    let key_path = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.sign_key_path",
        "wizard.sign.ask_key_path",
        None,
        session.sign_key_path.as_deref(),
    )?;
    let sign_ok = if session.dry_run {
        wizard_ui::render_line(output, &i18n.t("wizard.dry_run.skipping_sign"))?;
        true
    } else {
        run_process(
            self_exe,
            &[
                "sign",
                "--pack",
                &pack_dir_path.display().to_string(),
                "--key",
                &key_path,
            ],
            None,
        )?
    };
    if !sign_ok {
        wizard_ui::render_line(output, &i18n.t("wizard.error.sign_failed"))?;
        return Ok(false);
    }
    session.sign_key_path = Some(key_path);
    Ok(true)
}

fn ask_failure_nav<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
) -> Result<SubmenuAction> {
    let choice = ask_enum(
        input,
        output,
        i18n,
        "pack.wizard.failure_nav",
        "wizard.failure_nav.title",
        Some("wizard.failure_nav.description"),
        &[("0", "wizard.nav.back"), ("M", "wizard.nav.main_menu")],
        "0",
    )?;
    SubmenuAction::from_choice(&choice)
}

#[allow(clippy::too_many_arguments)]
fn ask_enum<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    form_id: &str,
    title_key: &str,
    description_key: Option<&str>,
    choices: &[(&str, &str)],
    default_on_eof: &str,
) -> Result<String> {
    let mut question = json!({
        "id": "choice",
        "type": "enum",
        "title": i18n.t(title_key),
        "title_i18n": {"key": title_key},
        "required": true,
        "choices": choices.iter().map(|(v, _)| *v).collect::<Vec<_>>(),
    });
    if let Some(description_key) = description_key {
        question["description"] = Value::String(i18n.t(description_key));
        question["description_i18n"] = json!({"key": description_key});
    }

    let spec = json!({
        "id": form_id,
        "title": i18n.t(title_key),
        "version": "1.0.0",
        "description": description_key.map(|key| i18n.t(key)).unwrap_or_default(),
        "progress_policy": {
            "skip_answered": true,
            "autofill_defaults": false,
            "treat_default_as_answered": false,
        },
        "questions": [question],
    });
    let config = WizardRunConfig {
        spec_json: serde_json::to_string(&spec).context("serialize enum QA spec")?,
        initial_answers_json: None,
        frontend: WizardFrontend::Text,
        i18n: i18n.qa_i18n_config(),
        verbose: false,
    };

    let mut driver = WizardDriver::new(config).context("initialize QA enum driver")?;
    loop {
        let payload_raw = driver
            .next_payload_json()
            .context("render QA enum payload")?;
        let payload: Value = serde_json::from_str(&payload_raw).context("parse QA enum payload")?;
        if let Some(text) = payload.get("text").and_then(Value::as_str) {
            render_driver_text(output, text)?;
        }

        if driver.is_complete() {
            break;
        }

        for (value, key) in choices {
            wizard_ui::render_line(output, &format!("{value}) {}", i18n.t(key)))?;
        }

        wizard_ui::render_prompt(output, &i18n.t("wizard.prompt"))?;
        let Some(line) = read_trimmed_line(input)? else {
            return Ok(default_on_eof.to_string());
        };
        let candidate = if line.eq_ignore_ascii_case("m") {
            "M".to_string()
        } else {
            line
        };
        if !choices
            .iter()
            .map(|(value, _)| *value)
            .any(|value| value.eq_ignore_ascii_case(&candidate))
        {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            continue;
        }

        let submit = driver
            .submit_patch_json(&json!({"choice": candidate}).to_string())
            .context("submit QA enum answer")?;
        if submit.status == "error" {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
        }
    }

    let result = driver.finish().context("finish QA enum")?;
    result
        .answer_set
        .answers
        .get("choice")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing enum answer"))
}

#[allow(clippy::too_many_arguments)]
fn ask_enum_custom_labels_owned<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    form_id: &str,
    title_key: &str,
    description_key: Option<&str>,
    choices: &[(String, String)],
    default_on_eof: &str,
) -> Result<String> {
    let mut question = json!({
        "id": "choice",
        "type": "enum",
        "title": i18n.t(title_key),
        "title_i18n": {"key": title_key},
        "required": true,
        "choices": choices.iter().map(|(v, _)| v).collect::<Vec<_>>(),
    });
    if let Some(description_key) = description_key {
        question["description"] = Value::String(i18n.t(description_key));
        question["description_i18n"] = json!({"key": description_key});
    }

    let spec = json!({
        "id": form_id,
        "title": i18n.t(title_key),
        "version": "1.0.0",
        "description": description_key.map(|key| i18n.t(key)).unwrap_or_default(),
        "progress_policy": {
            "skip_answered": true,
            "autofill_defaults": false,
            "treat_default_as_answered": false,
        },
        "questions": [question],
    });
    let config = WizardRunConfig {
        spec_json: serde_json::to_string(&spec).context("serialize custom enum QA spec")?,
        initial_answers_json: None,
        frontend: WizardFrontend::Text,
        i18n: i18n.qa_i18n_config(),
        verbose: false,
    };

    let mut driver = WizardDriver::new(config).context("initialize QA custom enum driver")?;
    loop {
        let payload_raw = driver
            .next_payload_json()
            .context("render QA custom enum payload")?;
        let payload: Value =
            serde_json::from_str(&payload_raw).context("parse QA custom enum payload")?;
        if let Some(text) = payload.get("text").and_then(Value::as_str) {
            render_driver_text(output, text)?;
        }

        if driver.is_complete() {
            break;
        }

        for (value, label) in choices {
            wizard_ui::render_line(output, &format!("{value}) {label}"))?;
        }

        wizard_ui::render_prompt(output, &i18n.t("wizard.prompt"))?;
        let Some(line) = read_trimmed_line(input)? else {
            return Ok(default_on_eof.to_string());
        };
        let candidate = if line.eq_ignore_ascii_case("m") {
            "M".to_string()
        } else {
            line
        };
        if !choices
            .iter()
            .map(|(value, _)| value.as_str())
            .any(|value| value.eq_ignore_ascii_case(&candidate))
        {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
            continue;
        }

        let submit = driver
            .submit_patch_json(&json!({"choice": candidate}).to_string())
            .context("submit QA custom enum answer")?;
        if submit.status == "error" {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
        }
    }

    let result = driver.finish().context("finish QA custom enum")?;
    result
        .answer_set
        .answers
        .get("choice")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing custom enum answer"))
}

fn ask_text<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    form_id: &str,
    title_key: &str,
    description_key: Option<&str>,
    default_value: Option<&str>,
) -> Result<String> {
    let mut question = json!({
        "id": "value",
        "type": "string",
        "title": i18n.t(title_key),
        "title_i18n": {"key": title_key},
        "required": true,
    });
    if let Some(description_key) = description_key {
        question["description"] = Value::String(i18n.t(description_key));
        question["description_i18n"] = json!({"key": description_key});
    }
    if let Some(default_value) = default_value {
        question["default_value"] = Value::String(default_value.to_string());
    }

    let spec = json!({
        "id": form_id,
        "title": i18n.t(title_key),
        "version": "1.0.0",
        "description": description_key.map(|key| i18n.t(key)).unwrap_or_default(),
        "progress_policy": {
            "skip_answered": true,
            "autofill_defaults": false,
            "treat_default_as_answered": false,
        },
        "questions": [question],
    });
    let config = WizardRunConfig {
        spec_json: serde_json::to_string(&spec).context("serialize text QA spec")?,
        initial_answers_json: None,
        frontend: WizardFrontend::Text,
        i18n: i18n.qa_i18n_config(),
        verbose: false,
    };

    let mut driver = WizardDriver::new(config).context("initialize QA text driver")?;
    loop {
        let payload_raw = driver
            .next_payload_json()
            .context("render QA text payload")?;
        let payload: Value = serde_json::from_str(&payload_raw).context("parse QA text payload")?;
        if let Some(text) = payload.get("text").and_then(Value::as_str) {
            render_driver_text(output, text)?;
        }

        if driver.is_complete() {
            break;
        }

        wizard_ui::render_prompt(output, &i18n.t("wizard.prompt"))?;
        let Some(line) = read_trimmed_line(input)? else {
            if let Some(default) = default_value {
                return Ok(default.to_string());
            }
            return Err(anyhow!("missing text input"));
        };

        let answer = if line.trim().is_empty() {
            default_value.unwrap_or_default().to_string()
        } else {
            line
        };
        let submit = driver
            .submit_patch_json(&json!({"value": answer}).to_string())
            .context("submit QA text answer")?;
        if submit.status == "error" {
            wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
        }
    }

    let result = driver.finish().context("finish QA text")?;
    result
        .answer_set
        .answers
        .get("value")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing text answer"))
}

fn prompt_for_extension_catalog_ref<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
) -> Result<String> {
    loop {
        wizard_ui::render_line(output, &i18n.t("wizard.extension_catalog.check_newer"))?;
        wizard_ui::render_line(output, &i18n.t("wizard.extension_catalog.check_newer_help"))?;
        wizard_ui::render_prompt(output, &i18n.t("wizard.prompt"))?;

        let Some(line) = read_trimmed_line(input)? else {
            return Ok(DEFAULT_EXTENSION_CATALOG_DOWNLOAD_URL.to_string());
        };
        let trimmed = line.trim();

        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("y")
            || trimmed.eq_ignore_ascii_case("yes")
        {
            return ask_text(
                input,
                output,
                i18n,
                "pack.wizard.extension_catalog.url",
                "wizard.extension_catalog.url",
                Some("wizard.extension_catalog.url_help"),
                Some(DEFAULT_EXTENSION_CATALOG_DOWNLOAD_URL),
            );
        }
        if trimmed.eq_ignore_ascii_case("n") || trimmed.eq_ignore_ascii_case("no") {
            return Ok(DEFAULT_EXTENSION_CATALOG_REF.to_string());
        }
        if looks_like_catalog_ref(trimmed) {
            return Ok(trimmed.to_string());
        }

        wizard_ui::render_line(output, &i18n.t("wizard.error.invalid_selection"))?;
    }
}

fn looks_like_catalog_ref(value: &str) -> bool {
    value.contains("://")
}

fn ask_existing_pack_dir<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    form_id: &str,
    title_key: &str,
    description_key: Option<&str>,
    default_value: Option<&str>,
) -> Result<PathBuf> {
    loop {
        let pack_dir = ask_text(
            input,
            output,
            i18n,
            form_id,
            title_key,
            description_key,
            default_value,
        )?;
        let candidate = PathBuf::from(pack_dir.trim());
        if candidate.is_dir() {
            return Ok(candidate);
        }
        wizard_ui::render_line(
            output,
            &format!(
                "{}: {}",
                i18n.t("wizard.error.invalid_pack_dir"),
                candidate.display()
            ),
        )?;
    }
}

fn run_process(binary: &Path, args: &[&str], cwd: Option<&Path>) -> Result<bool> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let status = cmd
        .status()
        .with_context(|| format!("spawn {}", binary.display()))?;
    Ok(status.success())
}

fn run_delegate(binary: &str, args: &[&str], cwd: &Path) -> bool {
    let resolved = crate::external_tools::resolve(binary).unwrap_or_else(|| PathBuf::from(binary));
    run_process(&resolved, args, Some(cwd)).unwrap_or(false)
}

fn run_delegate_owned(binary: &str, args: &[String], cwd: &Path) -> bool {
    let argv = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_delegate(binary, &argv, cwd)
}

fn capture_delegate_json(binary: &str, args: &[String], cwd: &Path) -> Result<Value> {
    let resolved = crate::external_tools::resolve(binary).unwrap_or_else(|| PathBuf::from(binary));
    let output = Command::new(&resolved)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("spawn {}", resolved.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("{} failed: {}", resolved.display(), stderr.trim()));
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parse json emitted by {}", resolved.display()))
}

fn temp_answers_path(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{prefix}-{}-{stamp}.json", std::process::id()))
}

fn read_json_value(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

fn write_json_value(path: &Path, value: &Value) -> bool {
    serde_json::to_vec_pretty(value)
        .ok()
        .and_then(|bytes| fs::write(path, bytes).ok())
        .is_some()
}

fn flow_delegate_args(_pack_dir: &Path) -> Vec<String> {
    vec!["wizard".to_string(), ".".to_string()]
}

fn run_flow_delegate_for_session(session: &mut WizardSession, pack_dir: &Path) -> bool {
    if !session.dry_run {
        let args = flow_delegate_args(pack_dir);
        return run_delegate_owned("greentic-flow", &args, pack_dir);
    }
    let answers_path = temp_answers_path("greentic-flow-wizard-answers");
    let mut args = flow_delegate_args(pack_dir);
    args.push("--execution".to_string());
    args.push("dry-run".to_string());
    args.push("--emit-answers".to_string());
    args.push(answers_path.display().to_string());
    let ok = run_delegate_owned("greentic-flow", &args, pack_dir);
    if ok {
        session.flow_wizard_answers = read_json_value(&answers_path);
    }
    let _ = fs::remove_file(&answers_path);
    ok
}

fn run_component_delegate_for_session(session: &mut WizardSession, pack_dir: &Path) -> bool {
    if !session.dry_run {
        return run_delegate("greentic-component", &["wizard"], pack_dir);
    }
    let answers_path = temp_answers_path("greentic-component-wizard-answers");
    let args = vec![
        "wizard".to_string(),
        "--project-root".to_string(),
        ".".to_string(),
        "--execution".to_string(),
        "dry-run".to_string(),
        "--qa-answers-out".to_string(),
        answers_path.display().to_string(),
    ];
    let ok = run_delegate_owned("greentic-component", &args, pack_dir);
    if ok {
        session.component_wizard_answers = read_json_value(&answers_path);
    }
    let _ = fs::remove_file(&answers_path);
    ok
}

fn run_flow_delegate_replay(pack_dir: &Path, answers: Option<&Value>) -> bool {
    if let Some(answers) = answers {
        let answers_path = temp_answers_path("greentic-flow-wizard-replay");
        if !write_json_value(&answers_path, answers) {
            return false;
        }
        let mut args = flow_delegate_args(pack_dir);
        args.push("--execution".to_string());
        args.push("execute".to_string());
        args.push("--answers-file".to_string());
        args.push(answers_path.display().to_string());
        let ok = run_delegate_owned("greentic-flow", &args, pack_dir);
        let _ = fs::remove_file(&answers_path);
        return ok;
    }
    let args = flow_delegate_args(pack_dir);
    run_delegate_owned("greentic-flow", &args, pack_dir)
}

fn run_component_delegate_replay(pack_dir: &Path, answers: Option<&Value>) -> bool {
    if let Some(answers) = answers {
        let answers_path = temp_answers_path("greentic-component-wizard-replay");
        if !write_json_value(&answers_path, answers) {
            return false;
        }
        let args = vec![
            "wizard".to_string(),
            "--project-root".to_string(),
            ".".to_string(),
            "--execution".to_string(),
            "execute".to_string(),
            "--qa-answers".to_string(),
            answers_path.display().to_string(),
        ];
        let ok = run_delegate_owned("greentic-component", &args, pack_dir);
        let _ = fs::remove_file(&answers_path);
        return ok;
    }
    run_delegate("greentic-component", &["wizard"], pack_dir)
}

fn handle_delegate_failure<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    i18n: &WizardI18n,
    session: &WizardSession,
    error_key: &str,
) -> Result<bool> {
    if session.dry_run {
        wizard_ui::render_line(output, &i18n.t("wizard.dry_run.child_wizard_returned"))?;
        return Ok(false);
    }
    wizard_ui::render_line(output, &i18n.t(error_key))?;
    if matches!(
        ask_failure_nav(input, output, i18n)?,
        SubmenuAction::MainMenu
    ) {
        return Ok(true);
    }
    Ok(false)
}

fn wizard_self_exe() -> Result<PathBuf> {
    if let Ok(path) = env::var("GREENTIC_PACK_WIZARD_SELF_EXE") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(anyhow!(
            "GREENTIC_PACK_WIZARD_SELF_EXE does not exist: {}",
            candidate.display()
        ));
    }
    std::env::current_exe().context("resolve current executable")
}

fn read_trimmed_line<R: BufRead>(input: &mut R) -> Result<Option<String>> {
    let mut line = String::new();
    let read = input.read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

fn render_driver_text<W: Write>(output: &mut W, text: &str) -> Result<()> {
    let filtered = filter_driver_boilerplate(text);
    if filtered.trim().is_empty() {
        return Ok(());
    }
    wizard_ui::render_text(output, &filtered)?;
    if !filtered.ends_with('\n') {
        wizard_ui::render_text(output, "\n")?;
    }
    Ok(())
}

fn filter_driver_boilerplate(text: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping_visible_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(title) = trimmed.strip_prefix("Title:") {
            let title = title.trim();
            if !title.is_empty() {
                kept.push(title);
            }
            continue;
        }
        if trimmed.starts_with("Description:") || trimmed.starts_with("Required:") {
            continue;
        }
        if trimmed == "All visible questions are answered." {
            continue;
        }
        if trimmed.starts_with("Form:")
            || trimmed.starts_with("Status:")
            || trimmed.starts_with("Help:")
            || trimmed.starts_with("Next question:")
        {
            skipping_visible_block = false;
            continue;
        }
        if trimmed.starts_with("Visible questions:") {
            skipping_visible_block = true;
            continue;
        }
        if skipping_visible_block {
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            skipping_visible_block = false;
        }
        kept.push(line);
    }
    let joined = kept.join("\n");
    joined.trim_matches('\n').to_string()
}

impl SubmenuAction {
    fn from_choice(choice: &str) -> Result<Self> {
        if choice == "0" {
            return Ok(Self::Back);
        }
        if choice.eq_ignore_ascii_case("m") {
            return Ok(Self::MainMenu);
        }
        Err(anyhow!("invalid submenu selection `{choice}`"))
    }
}

impl MainChoice {
    fn from_choice(choice: &str) -> Result<Self> {
        match choice {
            "1" => Ok(Self::CreateApplicationPack),
            "2" => Ok(Self::UpdateApplicationPack),
            "3" => Ok(Self::CreateExtensionPack),
            "4" => Ok(Self::UpdateExtensionPack),
            "5" => Ok(Self::AddExtension),
            "0" => Ok(Self::Exit),
            _ => Err(anyhow!("invalid main selection `{choice}`")),
        }
    }
}
