use std::collections::BTreeSet;
use std::path::PathBuf;

use greentic_types::ComponentId;
use greentic_types::pack::extensions::component_manifests::{
    ComponentManifestIndexV1, EXT_COMPONENT_MANIFEST_INDEX_V1,
};
use greentic_types::pack::extensions::component_sources::{
    ComponentSourcesV1, EXT_COMPONENT_SOURCES_V1,
};
use greentic_types::pack_manifest::{ExtensionInline, PackDependency, PackManifest};
const EXT_BUILD_MODE_ID: &str = "greentic.pack-mode.v1";
const CAP_OAUTH_BROKER_V1: &str = "greentic.cap.oauth.broker.v1";
const CAP_OAUTH_CARD_V1: &str = "greentic.cap.oauth.card.v1";
const CAP_OAUTH_TOKEN_VALIDATION_V1: &str = "greentic.cap.oauth.token_validation.v1";
use greentic_types::provider::ProviderDecl;
use greentic_types::validate::{
    Diagnostic, PackValidator, Severity, ValidationReport, validate_pack_manifest_core,
};
use serde_json::Value;

use crate::PackLoad;

#[derive(Clone, Debug, Default)]
pub struct ValidateCtx {
    pub pack_paths: BTreeSet<String>,
    pub sbom_paths: BTreeSet<String>,
    pub referenced_paths: BTreeSet<String>,
    pub pack_root: Option<PathBuf>,
    pub prod_build: bool,
}

impl ValidateCtx {
    pub fn from_pack_load(load: &PackLoad) -> Self {
        let prod_build = is_production_pack(load);
        let pack_paths = load.files.keys().cloned().collect();
        let sbom_paths = load.sbom.iter().map(|entry| entry.path.clone()).collect();

        let mut referenced_paths = BTreeSet::new();
        for flow in &load.manifest.flows {
            referenced_paths.insert(flow.file_yaml.clone());
            referenced_paths.insert(flow.file_json.clone());
        }
        for component in &load.manifest.components {
            referenced_paths.insert(component.file_wasm.clone());
            if let Some(schema_file) = component.schema_file.as_ref() {
                referenced_paths.insert(schema_file.clone());
            }
            if let Some(manifest_file) = component.manifest_file.as_ref() {
                referenced_paths.insert(manifest_file.clone());
            }
        }
        if let Some(manifest) = load.gpack_manifest.as_ref()
            && let Some(value) = manifest
                .extensions
                .as_ref()
                .and_then(|exts| exts.get(EXT_COMPONENT_MANIFEST_INDEX_V1))
                .and_then(|ext| ext.inline.as_ref())
                .and_then(|inline| match inline {
                    ExtensionInline::Other(value) => Some(value),
                    _ => None,
                })
            && let Ok(index) = ComponentManifestIndexV1::from_extension_value(value)
        {
            for entry in index.entries {
                referenced_paths.insert(entry.manifest_file);
            }
        }

        Self {
            pack_paths,
            sbom_paths,
            referenced_paths,
            pack_root: None,
            prod_build,
        }
    }
}

pub fn run_validators(
    manifest: &PackManifest,
    _ctx: &ValidateCtx,
    validators: &[Box<dyn PackValidator>],
) -> ValidationReport {
    let mut report = ValidationReport {
        pack_id: Some(manifest.pack_id.clone()),
        pack_version: Some(manifest.version.clone()),
        diagnostics: Vec::new(),
    };

    report
        .diagnostics
        .extend(validate_pack_manifest_core(manifest));

    for validator in validators {
        if validator.applies(manifest) {
            report.diagnostics.extend(validator.validate(manifest));
        }
    }

    report
}

#[derive(Clone, Debug)]
pub struct ReferencedFilesExistValidator {
    ctx: ValidateCtx,
}

impl ReferencedFilesExistValidator {
    pub fn new(ctx: ValidateCtx) -> Self {
        Self { ctx }
    }
}

impl PackValidator for ReferencedFilesExistValidator {
    fn id(&self) -> &'static str {
        "pack.referenced-files-exist"
    }

    fn applies(&self, _manifest: &PackManifest) -> bool {
        true
    }

    fn validate(&self, _manifest: &PackManifest) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for path in &self.ctx.referenced_paths {
            if self.ctx.prod_build && is_flow_source_path(path) {
                continue;
            }
            let missing_in_pack = !self.ctx.pack_paths.contains(path);
            let missing_in_sbom = !self.ctx.sbom_paths.contains(path);

            if missing_in_pack || missing_in_sbom {
                let message = if missing_in_pack && missing_in_sbom {
                    "Referenced file is missing from the pack archive and SBOM."
                } else if missing_in_pack {
                    "Referenced file is missing from the pack archive."
                } else {
                    "Referenced file is missing from the SBOM."
                };
                diagnostics.push(missing_file_diagnostic(
                    "PACK_MISSING_FILE",
                    message,
                    Some(path.clone()),
                ));
            }
        }

        diagnostics
    }
}

#[derive(Clone, Debug)]
pub struct SbomConsistencyValidator {
    ctx: ValidateCtx,
}

impl SbomConsistencyValidator {
    pub fn new(ctx: ValidateCtx) -> Self {
        Self { ctx }
    }
}

impl PackValidator for SbomConsistencyValidator {
    fn id(&self) -> &'static str {
        "pack.sbom-consistency"
    }

    fn applies(&self, _manifest: &PackManifest) -> bool {
        true
    }

    fn validate(&self, _manifest: &PackManifest) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let manifest_path = "manifest.cbor";

        if !self.ctx.pack_paths.contains(manifest_path)
            || !self.ctx.sbom_paths.contains(manifest_path)
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "PACK_MISSING_MANIFEST_CBOR".to_string(),
                message: "manifest.cbor must be present in the pack and listed in the SBOM."
                    .to_string(),
                path: Some(manifest_path.to_string()),
                hint: Some(
                    "Rebuild the pack so manifest.cbor is included in the SBOM.".to_string(),
                ),
                data: Value::Null,
            });
        }

        for path in &self.ctx.sbom_paths {
            if !self.ctx.pack_paths.contains(path) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "PACK_SBOM_DANGLING_PATH".to_string(),
                    message: "SBOM entry references a path missing from the pack archive."
                        .to_string(),
                    path: Some(path.clone()),
                    hint: Some("Remove stale SBOM entries or rebuild the pack.".to_string()),
                    data: Value::Null,
                });
            }
        }

        diagnostics
    }
}

#[derive(Clone, Debug)]
pub struct ProviderReferencesExistValidator {
    ctx: ValidateCtx,
}

impl ProviderReferencesExistValidator {
    pub fn new(ctx: ValidateCtx) -> Self {
        Self { ctx }
    }
}

impl PackValidator for ProviderReferencesExistValidator {
    fn id(&self) -> &'static str {
        "pack.provider-references-exist"
    }

    fn applies(&self, manifest: &PackManifest) -> bool {
        manifest.provider_extension_inline().is_some()
    }

    fn validate(&self, manifest: &PackManifest) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for provider in providers_from_manifest(manifest) {
            let config_path = provider.config_schema_ref.as_str();
            if !config_path.is_empty() {
                diagnostics.extend(check_pack_path(
                    &self.ctx,
                    config_path,
                    "provider config schema",
                ));
            }
            if let Some(docs_path) = provider.docs_ref.as_deref()
                && !docs_path.is_empty()
            {
                diagnostics.extend(check_pack_path(&self.ctx, docs_path, "provider docs"));
            }
        }

        diagnostics
    }
}

#[derive(Clone, Debug)]
pub struct SecretRequirementsValidator;

impl PackValidator for SecretRequirementsValidator {
    fn id(&self) -> &'static str {
        "pack.secret-requirements-invalid"
    }

    fn applies(&self, manifest: &PackManifest) -> bool {
        !manifest.secret_requirements.is_empty()
    }

    fn validate(&self, manifest: &PackManifest) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (idx, requirement) in manifest.secret_requirements.iter().enumerate() {
            let key = requirement.key.as_str();
            if requirement.scope.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "PACK_SECRET_REQUIREMENTS_INVALID".to_string(),
                    message: format!("secret requirement `{}` is missing a scope", key),
                    path: Some(format!("secretRequirements[{idx}]")),
                    hint: Some(
                        "Provide env/tenant values in secretRequirements or pass --default-secret-scope when building."
                            .to_string(),
                    ),
                    data: Value::Null,
                });
                continue;
            }
            let scope = requirement.scope.as_ref().unwrap();
            if scope.env.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "PACK_SECRET_REQUIREMENTS_INVALID".to_string(),
                    message: format!("secret requirement `{}` has an empty env scope", key),
                    path: Some(format!("secretRequirements[{idx}].scope.env")),
                    hint: Some(
                        "Ensure the secret scope includes a valid environment identifier."
                            .to_string(),
                    ),
                    data: Value::Null,
                });
            }
            if scope.tenant.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "PACK_SECRET_REQUIREMENTS_INVALID".to_string(),
                    message: format!("secret requirement `{}` has an empty tenant scope", key),
                    path: Some(format!("secretRequirements[{idx}].scope.tenant")),
                    hint: Some(
                        "Ensure the secret scope includes a valid tenant identifier.".to_string(),
                    ),
                    data: Value::Null,
                });
            }
        }
        diagnostics
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComponentReferencesExistValidator;

impl PackValidator for ComponentReferencesExistValidator {
    fn id(&self) -> &'static str {
        "pack.component-references-exist"
    }

    fn applies(&self, _manifest: &PackManifest) -> bool {
        true
    }

    fn validate(&self, manifest: &PackManifest) -> Vec<Diagnostic> {
        let mut known: BTreeSet<ComponentId> = BTreeSet::new();
        for component in &manifest.components {
            known.insert(component.id.clone());
        }

        let mut source_ids: BTreeSet<ComponentId> = BTreeSet::new();
        if let Some(value) = manifest
            .extensions
            .as_ref()
            .and_then(|exts| exts.get(EXT_COMPONENT_SOURCES_V1))
            .and_then(|ext| ext.inline.as_ref())
            .and_then(|inline| match inline {
                ExtensionInline::Other(value) => Some(value),
                _ => None,
            })
            && let Ok(cs) = ComponentSourcesV1::from_extension_value(value)
        {
            for entry in cs.components {
                if let Some(id) = entry.component_id {
                    source_ids.insert(id);
                }
            }
        }

        let mut diagnostics = Vec::new();
        for flow in &manifest.flows {
            for (node_id, node) in &flow.flow.nodes {
                if node.component.pack_alias.is_some() {
                    continue;
                }
                let component_id = &node.component.id;
                if !known.contains(component_id) && !source_ids.contains(component_id) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "PACK_MISSING_COMPONENT_REFERENCE".to_string(),
                        message: format!(
                            "Flow references component '{}' missing from manifest/component sources.",
                            component_id
                        ),
                        path: Some(format!(
                            "flows.{}.nodes.{}.component",
                            flow.id.as_str(),
                            node_id.as_str()
                        )),
                        hint: Some(
                            "Add the component to the pack manifest or component sources."
                                .to_string(),
                        ),
                        data: Value::Null,
                    });
                }
            }
        }

        diagnostics
    }
}

#[derive(Clone, Debug, Default)]
pub struct OauthCapabilityRequirementsValidator;

impl PackValidator for OauthCapabilityRequirementsValidator {
    fn id(&self) -> &'static str {
        "pack.oauth-capability-requirements"
    }

    fn applies(&self, _manifest: &PackManifest) -> bool {
        true
    }

    fn validate(&self, manifest: &PackManifest) -> Vec<Diagnostic> {
        let required_capabilities = dependency_required_capabilities(&manifest.dependencies);
        let mut diagnostics = Vec::new();
        for flow in &manifest.flows {
            diagnostics.extend(oauth_capability_requirement_diagnostics_for_flow(
                flow.id.as_str(),
                &flow.flow,
                &required_capabilities,
            ));
        }
        diagnostics
    }
}

pub fn oauth_capability_requirement_diagnostics_for_flow(
    flow_id: &str,
    flow: &greentic_types::Flow,
    required_capabilities: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (node_id, node) in &flow.nodes {
        let operation = node.component.operation.as_deref().unwrap_or_default();
        let uses_card = operation.starts_with("oauth.card.")
            || value_contains_substring(&node.input.mapping, "oauth.card.")
            || value_contains_substring(&node.input.mapping, CAP_OAUTH_CARD_V1);
        if uses_card && !required_capabilities.contains(CAP_OAUTH_CARD_V1) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "PACK_OAUTH_CARD_CAPABILITY_REQUIRED".to_string(),
                message: format!(
                    "flow node uses OAuth card features but dependencies do not require `{CAP_OAUTH_CARD_V1}`"
                ),
                path: Some(oauth_usage_path(
                    flow_id,
                    node_id.as_str(),
                    operation.starts_with("oauth.card."),
                )),
                hint: Some(
                    "Add `greentic.cap.oauth.card.v1` to dependencies[].required_capabilities."
                        .to_string(),
                ),
                data: Value::Null,
            });
        }

        let calls_get_access_token = operation == "oauth.get_access_token"
            || value_contains_substring(&node.input.mapping, "oauth.get_access_token");
        if calls_get_access_token && !required_capabilities.contains(CAP_OAUTH_BROKER_V1) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "PACK_OAUTH_BROKER_CAPABILITY_REQUIRED".to_string(),
                message: format!(
                    "flow node calls oauth.get_access_token but dependencies do not require `{CAP_OAUTH_BROKER_V1}`"
                ),
                path: Some(oauth_usage_path(
                    flow_id,
                    node_id.as_str(),
                    operation == "oauth.get_access_token",
                )),
                hint: Some(
                    "Add `greentic.cap.oauth.broker.v1` to dependencies[].required_capabilities."
                        .to_string(),
                ),
                data: Value::Null,
            });
        }

        let uses_token_validation = operation.starts_with("oauth.validate")
            || operation.contains("token_validation")
            || value_contains_substring(&node.input.mapping, "oauth.validate")
            || value_contains_substring(&node.input.mapping, "token_validation")
            || value_contains_substring(&node.input.mapping, CAP_OAUTH_TOKEN_VALIDATION_V1);
        if uses_token_validation && !required_capabilities.contains(CAP_OAUTH_TOKEN_VALIDATION_V1) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "PACK_OAUTH_TOKEN_VALIDATION_CAPABILITY_REQUIRED".to_string(),
                message: format!(
                    "flow node uses OAuth token validation but dependencies do not require `{CAP_OAUTH_TOKEN_VALIDATION_V1}`"
                ),
                path: Some(oauth_usage_path(
                    flow_id,
                    node_id.as_str(),
                    operation.starts_with("oauth.validate")
                        || operation.contains("token_validation"),
                )),
                hint: Some(
                    "Add `greentic.cap.oauth.token_validation.v1` to dependencies[].required_capabilities."
                        .to_string(),
                ),
                data: Value::Null,
            });
        }
    }

    diagnostics
}

fn providers_from_manifest(manifest: &PackManifest) -> Vec<ProviderDecl> {
    let mut providers = manifest
        .provider_extension_inline()
        .map(|inline| inline.providers.clone())
        .unwrap_or_default();
    providers.sort_by(|a, b| a.provider_type.cmp(&b.provider_type));
    providers
}

fn check_pack_path(ctx: &ValidateCtx, path: &str, label: &str) -> Vec<Diagnostic> {
    if path.trim().is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    if !ctx.pack_paths.contains(path) {
        diagnostics.push(missing_file_diagnostic(
            "PACK_MISSING_FILE",
            &format!("{label} is missing from the pack archive."),
            Some(path.to_string()),
        ));
    } else if !ctx.sbom_paths.contains(path) {
        diagnostics.push(missing_file_diagnostic(
            "PACK_MISSING_FILE",
            &format!("{label} is missing from the SBOM."),
            Some(path.to_string()),
        ));
    }

    diagnostics
}

fn is_flow_source_path(path: &str) -> bool {
    path.starts_with("flows/") && (path.ends_with(".ygtc") || path.ends_with(".json"))
}

fn is_production_pack(load: &PackLoad) -> bool {
    if let Some(manifest) = load.gpack_manifest.as_ref()
        && let Some(extension) = manifest
            .extensions
            .as_ref()
            .and_then(|map| map.get(EXT_BUILD_MODE_ID))
        && let Some(ExtensionInline::Other(value)) = extension.inline.as_ref()
        && let Some(mode) = value.get("mode").and_then(|value| value.as_str())
    {
        return !mode.eq_ignore_ascii_case("dev");
    }
    !load.files.keys().any(|path| path.ends_with(".ygtc"))
}

fn dependency_required_capabilities(dependencies: &[PackDependency]) -> BTreeSet<String> {
    let mut required = BTreeSet::new();
    for dep in dependencies {
        for capability in &dep.required_capabilities {
            let trimmed = capability.trim();
            if !trimmed.is_empty() {
                required.insert(trimmed.to_string());
            }
        }
    }
    required
}

fn value_contains_substring(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(items) => items
            .iter()
            .any(|item| value_contains_substring(item, needle)),
        Value::Object(map) => map
            .iter()
            .any(|(key, item)| key.contains(needle) || value_contains_substring(item, needle)),
        _ => false,
    }
}

fn oauth_usage_path(flow_id: &str, node_id: &str, operation_match: bool) -> String {
    if operation_match {
        format!("flows.{flow_id}.nodes.{node_id}.component.operation")
    } else {
        format!("flows.{flow_id}.nodes.{node_id}.input.mapping")
    }
}

fn missing_file_diagnostic(code: &str, message: &str, path: Option<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: code.to_string(),
        message: message.to_string(),
        path,
        hint: None,
        data: Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CAP_OAUTH_BROKER_V1, CAP_OAUTH_CARD_V1, oauth_capability_requirement_diagnostics_for_flow,
    };
    use std::collections::BTreeSet;

    use greentic_flow::compile_ygtc_str;

    #[test]
    fn oauth_card_usage_requires_card_capability() {
        let flow = compile_ygtc_str(
            r#"
id: auth
type: messaging
start: step
nodes:
  step:
    oauth.card.render:
      title: "Sign in"
    routing:
      - out: true
"#,
        )
        .expect("compile flow");

        let required = BTreeSet::new();
        let diagnostics =
            oauth_capability_requirement_diagnostics_for_flow("auth", &flow, &required);
        assert!(diagnostics.iter().any(|diag| {
            diag.code == "PACK_OAUTH_CARD_CAPABILITY_REQUIRED"
                && diag.path.as_deref() == Some("flows.auth.nodes.step.component.operation")
        }));
    }

    #[test]
    fn oauth_get_access_token_requires_broker_capability() {
        let flow = compile_ygtc_str(
            r#"
id: auth
type: messaging
start: step
nodes:
  step:
    oauth.get_access_token:
      tenant: "demo"
    routing:
      - out: true
"#,
        )
        .expect("compile flow");

        let required = BTreeSet::new();
        let diagnostics =
            oauth_capability_requirement_diagnostics_for_flow("auth", &flow, &required);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.code == "PACK_OAUTH_BROKER_CAPABILITY_REQUIRED")
        );
    }

    #[test]
    fn oauth_usage_passes_when_dependency_capabilities_are_declared() {
        let flow = compile_ygtc_str(
            r#"
id: auth
type: messaging
start: step
nodes:
  step:
    oauth.get_access_token:
      tenant: "demo"
    routing:
      - out: true
"#,
        )
        .expect("compile flow");

        let required = BTreeSet::from([
            CAP_OAUTH_BROKER_V1.to_string(),
            CAP_OAUTH_CARD_V1.to_string(),
        ]);
        let diagnostics =
            oauth_capability_requirement_diagnostics_for_flow("auth", &flow, &required);
        assert!(
            diagnostics.is_empty(),
            "expected no oauth requirement diagnostics, got: {diagnostics:?}"
        );
    }
}
