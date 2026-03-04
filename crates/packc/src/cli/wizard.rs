#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use clap::{Args, Subcommand};
use greentic_qa_lib::{WizardDriver, WizardFrontend, WizardRunConfig};
use greentic_types::ExtensionRef;
use greentic_types::WizardStep;
use greentic_types::pack_manifest::ExtensionInline;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cli::wizard_catalog::{
    CatalogQuestion, CatalogQuestionKind, ExtensionCatalog, ExtensionTemplate, ExtensionType,
    load_extension_catalog,
};
use crate::cli::wizard_i18n::{WizardI18n, detect_requested_locale};
use crate::cli::wizard_ui;
use crate::runtime::RuntimeContext;

const PACK_WIZARD_ID: &str = "greentic-pack.wizard.run";
const PACK_WIZARD_SCHEMA_ID: &str = "greentic-pack.wizard.answers";
const PACK_WIZARD_SCHEMA_VERSION: &str = "1.0.0";

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
    match args.command {
        None => run_interactive_command(implicit_run_args, runtime, requested_locale),
        Some(WizardCommand::Run(cmd)) => run_interactive_command(cmd, runtime, requested_locale),
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
) -> Result<()> {
    let target_schema_version = target_schema_version(cmd.schema_version.as_deref())?;
    let locale = resolved_locale(requested_locale);
    if let Some(path) = cmd.answers.as_deref() {
        let doc =
            load_answer_document(path, &target_schema_version, cmd.migrate, requested_locale)?;
        validate_answer_document(&doc)?;
        if !cmd.dry_run {
            apply_answer_document(&doc)?;
        }
        if let Some(out) = cmd.emit_answers.as_deref() {
            write_answer_document(out, &doc)?;
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
    if !plan.create_pack_scaffold && !plan.pack_dir.is_dir() {
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
    let catalog_ref = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.create_ext.catalog_ref",
        "wizard.create_extension_pack.ask_catalog_ref",
        Some("wizard.create_extension_pack.ask_catalog_ref_help"),
        Some("oci://ghcr.io/greenticai/catalogs/extensions:latest"),
    )?;

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
    if session.dry_run {
        wizard_ui::render_line(output, &i18n.t("wizard.dry_run.skipping_template_apply"))?;
    } else if let Err(err) =
        apply_template_plan(&template, &pack_dir_path, selected, i18n, &qa_answers)
    {
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
) -> Result<()> {
    fs::create_dir_all(pack_dir)
        .with_context(|| format!("create extension pack dir {}", pack_dir.display()))?;
    for step in &template.plan {
        match step {
            WizardStep::EnsureDir { paths } => {
                for rel in paths {
                    let target = pack_dir.join(rel);
                    fs::create_dir_all(&target)
                        .with_context(|| format!("create directory {}", target.display()))?;
                }
            }
            WizardStep::WriteFiles { files } => {
                for (rel, content) in files {
                    let target = pack_dir.join(rel);
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
                    );
                    fs::write(&target, rendered)
                        .with_context(|| format!("write file {}", target.display()))?;
                }
            }
            WizardStep::RunCli { command, args } => {
                let (rendered_command, rendered_args) = render_run_cli_invocation(
                    command,
                    args,
                    extension_type,
                    template,
                    i18n,
                    qa_answers,
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
            WizardStep::Delegate { target, .. } => {
                let ok = match target {
                    greentic_types::WizardTarget::Flow => {
                        run_delegate("greentic-flow", &["wizard", "."], pack_dir)
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

fn render_template_content(
    content: &str,
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
) -> String {
    render_template_string(content, extension_type, template, i18n, qa_answers)
}

fn render_template_string(
    raw: &str,
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
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
    rendered
}

fn render_run_cli_invocation(
    command: &str,
    args: &[String],
    extension_type: &ExtensionType,
    template: &ExtensionTemplate,
    i18n: &WizardI18n,
    qa_answers: &BTreeMap<String, String>,
) -> Result<(String, Vec<String>)> {
    let rendered_command =
        render_template_string(command, extension_type, template, i18n, qa_answers);
    validate_run_cli_token(&rendered_command, "command", true)?;

    let mut rendered_args = Vec::with_capacity(args.len());
    for (idx, arg) in args.iter().enumerate() {
        let rendered = render_template_string(arg, extension_type, template, i18n, qa_answers);
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
    for question in &extension_type.edit_questions {
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
    answers: &BTreeMap<String, String>,
) -> Result<()> {
    let dir = pack_dir.join("extensions");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(format!("{}.json", extension_type.id));
    let payload = json!({
        "extension_type": extension_type.id,
        "answers": answers,
    });
    let bytes =
        serde_json::to_vec_pretty(&payload).context("serialize extension edit answers payload")?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    merge_extension_answers_into_pack_yaml(pack_dir, extension_type, answers)?;
    Ok(())
}

fn merge_extension_answers_into_pack_yaml(
    pack_dir: &Path,
    extension_type: &ExtensionType,
    answers: &BTreeMap<String, String>,
) -> Result<()> {
    let pack_yaml = pack_dir.join("pack.yaml");
    if !pack_yaml.exists() {
        return Ok(());
    }

    let mut cfg = crate::config::load_pack_config(pack_dir)?;
    let key = format!("greentic.wizard.{}.v1", extension_type.id);
    let inline_payload = json!({
        "extension_type": extension_type.id,
        "answers": answers,
    });

    let mut extensions = cfg.extensions.unwrap_or_default();
    extensions.insert(
        key.clone(),
        ExtensionRef {
            kind: key,
            version: "v1".to_string(),
            location: None,
            digest: None,
            inline: Some(ExtensionInline::Other(inline_payload)),
        },
    );
    cfg.extensions = Some(extensions);

    let serialized = serde_yaml_bw::to_string(&cfg).context("serialize updated pack.yaml")?;
    fs::write(&pack_yaml, serialized).with_context(|| format!("write {}", pack_yaml.display()))?;
    Ok(())
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
    let catalog_ref = ask_text(
        input,
        output,
        i18n,
        "pack.wizard.update_ext.catalog_ref",
        "wizard.update_extension_pack.ask_catalog_ref",
        Some("wizard.update_extension_pack.ask_catalog_ref_help"),
        Some("oci://ghcr.io/greenticai/catalogs/extensions:latest"),
    )?;

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
                if !session.dry_run {
                    persist_extension_edit_answers(&pack_dir_path, selected, &answers)?;
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
    if let Some(current_exe) = std::env::current_exe().ok()
        && let Some(exe_dir) = current_exe.parent()
    {
        let local_bin = exe_dir.join(binary);
        if local_bin.exists() {
            return run_process(&local_bin, args, Some(cwd)).unwrap_or(false);
        }
    }

    if let Some(override_bin) = delegate_override_binary(binary)
        && override_bin.exists()
    {
        return run_process(&override_bin, args, Some(cwd)).unwrap_or(false);
    }

    if should_prefer_monorepo_delegate(binary)
        && let Some(dev_bin) = monorepo_delegate_binary(binary)
        && dev_bin.exists()
    {
        return run_process(&dev_bin, args, Some(cwd)).unwrap_or(false);
    }

    Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_delegate_owned(binary: &str, args: &[String], cwd: &Path) -> bool {
    let argv = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_delegate(binary, &argv, cwd)
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

fn run_flow_delegate_for_session(session: &mut WizardSession, pack_dir: &Path) -> bool {
    if !session.dry_run {
        return run_delegate("greentic-flow", &["wizard", "."], pack_dir);
    }
    let answers_path = temp_answers_path("greentic-flow-wizard-answers");
    let args = vec![
        "wizard".to_string(),
        ".".to_string(),
        "--dry-run".to_string(),
        "--emit-answers".to_string(),
        answers_path.display().to_string(),
    ];
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
        let args = vec![
            "wizard".to_string(),
            ".".to_string(),
            "--answers-file".to_string(),
            answers_path.display().to_string(),
        ];
        let ok = run_delegate_owned("greentic-flow", &args, pack_dir);
        let _ = fs::remove_file(&answers_path);
        return ok;
    }
    run_delegate("greentic-flow", &["wizard", "."], pack_dir)
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

fn delegate_override_binary(binary: &str) -> Option<PathBuf> {
    let key = match binary {
        "greentic-flow" => "GREENTIC_FLOW_BIN",
        "greentic-component" => "GREENTIC_COMPONENT_BIN",
        _ => return None,
    };
    env::var_os(key).map(PathBuf::from)
}

fn monorepo_delegate_binary(binary: &str) -> Option<PathBuf> {
    if binary != "greentic-flow" {
        return None;
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?.parent()?;
    let sibling_root = repo_root.join("../greentic-flow");
    for rel in ["target/debug/greentic-flow", "target/release/greentic-flow"] {
        let candidate = sibling_root.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn should_prefer_monorepo_delegate(binary: &str) -> bool {
    if binary != "greentic-flow" {
        return false;
    }
    let Some(path_bin) = resolve_from_path(binary) else {
        return false;
    };
    let path_str = path_bin.to_string_lossy();
    path_str.contains("/.cargo/bin/greentic-flow")
}

fn resolve_from_path(binary: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
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
            "0" => Ok(Self::Exit),
            _ => Err(anyhow!("invalid main selection `{choice}`")),
        }
    }
}
