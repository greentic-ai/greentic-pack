use std::fmt;

use anyhow::{Result, anyhow, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RepoPackKind {
    SourceProvider,
    Scanner,
    Signing,
    Attestation,
    PolicyEngine,
    OciProvider,
    BillingProvider,
    SearchProvider,
    RecommendationProvider,
}

impl fmt::Display for RepoPackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceProvider => "source-provider",
            Self::Scanner => "scanner",
            Self::Signing => "signing",
            Self::Attestation => "attestation",
            Self::PolicyEngine => "policy-engine",
            Self::OciProvider => "oci-provider",
            Self::BillingProvider => "billing-provider",
            Self::SearchProvider => "search-provider",
            Self::RecommendationProvider => "recommendation-provider",
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RepoCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scan: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signing: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attestation: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub oci: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub billing: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reco: Vec<String>,
}

impl RepoCapabilities {
    fn validate(&self) -> Result<()> {
        let validate_list = |label: &str, entries: &[String]| -> Result<()> {
            for entry in entries {
                if entry.trim().is_empty() {
                    bail!("{label} capability values must not be empty");
                }
            }
            Ok(())
        };

        validate_list("source", &self.source)?;
        validate_list("scan", &self.scan)?;
        validate_list("signing", &self.signing)?;
        validate_list("attestation", &self.attestation)?;
        validate_list("policy", &self.policy)?;
        validate_list("oci", &self.oci)?;
        validate_list("billing", &self.billing)?;
        validate_list("search", &self.search)?;
        validate_list("reco", &self.reco)?;
        Ok(())
    }

    fn has_for_kind(&self, kind: &RepoPackKind) -> bool {
        match kind {
            RepoPackKind::SourceProvider => !self.source.is_empty(),
            RepoPackKind::Scanner => !self.scan.is_empty(),
            RepoPackKind::Signing => !self.signing.is_empty(),
            RepoPackKind::Attestation => !self.attestation.is_empty(),
            RepoPackKind::PolicyEngine => !self.policy.is_empty(),
            RepoPackKind::OciProvider => !self.oci.is_empty(),
            RepoPackKind::BillingProvider => !self.billing.is_empty(),
            RepoPackKind::SearchProvider => !self.search.is_empty(),
            RepoPackKind::RecommendationProvider => !self.reco.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RepoBindings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scan: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signing: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attestation: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub oci: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub billing: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<RepoBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reco: Vec<RepoBinding>,
}

impl RepoBindings {
    fn validate(&self) -> Result<()> {
        let validate_list = |label: &str, entries: &[RepoBinding]| -> Result<()> {
            for entry in entries {
                entry.validate(label)?;
            }
            Ok(())
        };
        validate_list("source", &self.source)?;
        validate_list("scan", &self.scan)?;
        validate_list("signing", &self.signing)?;
        validate_list("attestation", &self.attestation)?;
        validate_list("policy", &self.policy)?;
        validate_list("oci", &self.oci)?;
        validate_list("billing", &self.billing)?;
        validate_list("search", &self.search)?;
        validate_list("reco", &self.reco)?;
        Ok(())
    }

    fn has_for_kind(&self, kind: &RepoPackKind) -> bool {
        match kind {
            RepoPackKind::SourceProvider => !self.source.is_empty(),
            RepoPackKind::Scanner => !self.scan.is_empty(),
            RepoPackKind::Signing => !self.signing.is_empty(),
            RepoPackKind::Attestation => !self.attestation.is_empty(),
            RepoPackKind::PolicyEngine => !self.policy.is_empty(),
            RepoPackKind::OciProvider => !self.oci.is_empty(),
            RepoPackKind::BillingProvider => !self.billing.is_empty(),
            RepoPackKind::SearchProvider => !self.search.is_empty(),
            RepoPackKind::RecommendationProvider => !self.reco.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct RepoBinding {
    pub package: String,
    pub world: String,
    pub version: String,
    #[serde(alias = "component_id")]
    pub component: String,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl RepoBinding {
    pub fn validate(&self, label: &str) -> Result<()> {
        if self.package.trim().is_empty() {
            bail!("bindings.{label}[].package is required");
        }
        if self.world.trim().is_empty() {
            bail!("bindings.{label}[].world is required");
        }
        if self.version.trim().is_empty() {
            bail!("bindings.{label}[].version is required");
        }
        if self.component.trim().is_empty() {
            bail!("bindings.{label}[].component is required");
        }
        if self.entrypoint.trim().is_empty() {
            bail!("bindings.{label}[].entrypoint is required");
        }
        if let Some(profile) = &self.profile
            && profile.trim().is_empty()
        {
            bail!("bindings.{label}[].profile must not be empty when present");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct RepoPackSection {
    pub kind: RepoPackKind,
    #[serde(default)]
    pub capabilities: RepoCapabilities,
    #[serde(default)]
    pub bindings: RepoBindings,
}

impl RepoPackSection {
    pub fn validate(&self) -> Result<()> {
        self.capabilities.validate()?;
        self.bindings.validate()?;

        ensure_capability_keys_match_kind(&self.capabilities, &self.kind)?;
        ensure_binding_keys_match_kind(&self.bindings, &self.kind)?;

        if !self.capabilities.has_for_kind(&self.kind) {
            bail!(
                "capabilities for role {} must include at least one entry",
                self.kind
            );
        }

        if !self.bindings.has_for_kind(&self.kind) {
            bail!(
                "bindings for role {} must include at least one entry",
                self.kind
            );
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct InterfaceBinding {
    pub package: String,
    pub world: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl InterfaceBinding {
    pub fn validate(&self, label: &str) -> Result<()> {
        if self.package.trim().is_empty() {
            bail!("{label}[].package is required");
        }
        if self.world.trim().is_empty() {
            bail!("{label}[].world is required");
        }
        if self.version.trim().is_empty() {
            bail!("{label}[].version is required");
        }
        Ok(())
    }
}

fn ensure_capability_keys_match_kind(caps: &RepoCapabilities, kind: &RepoPackKind) -> Result<()> {
    let unexpected = |label: &str| {
        anyhow!(
            "capabilities for {} may not include `{label}`; use the {} key instead",
            kind,
            expected_capability_key(kind)
        )
    };

    match kind {
        RepoPackKind::SourceProvider => {
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Scanner => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Signing => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Attestation => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::PolicyEngine => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::OciProvider => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::BillingProvider => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::SearchProvider => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::RecommendationProvider => {
            if !caps.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !caps.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !caps.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !caps.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !caps.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !caps.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !caps.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !caps.search.is_empty() {
                return Err(unexpected("search"));
            }
        }
    }
    Ok(())
}

fn ensure_binding_keys_match_kind(bindings: &RepoBindings, kind: &RepoPackKind) -> Result<()> {
    let unexpected = |label: &str| {
        anyhow!(
            "bindings for {} may not include `{label}`; use the {} key instead",
            kind,
            expected_capability_key(kind)
        )
    };

    match kind {
        RepoPackKind::SourceProvider => {
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Scanner => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Signing => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::Attestation => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::PolicyEngine => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::OciProvider => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::BillingProvider => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::SearchProvider => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.reco.is_empty() {
                return Err(unexpected("reco"));
            }
        }
        RepoPackKind::RecommendationProvider => {
            if !bindings.source.is_empty() {
                return Err(unexpected("source"));
            }
            if !bindings.scan.is_empty() {
                return Err(unexpected("scan"));
            }
            if !bindings.signing.is_empty() {
                return Err(unexpected("signing"));
            }
            if !bindings.attestation.is_empty() {
                return Err(unexpected("attestation"));
            }
            if !bindings.policy.is_empty() {
                return Err(unexpected("policy"));
            }
            if !bindings.oci.is_empty() {
                return Err(unexpected("oci"));
            }
            if !bindings.billing.is_empty() {
                return Err(unexpected("billing"));
            }
            if !bindings.search.is_empty() {
                return Err(unexpected("search"));
            }
        }
    }

    Ok(())
}

fn expected_capability_key(kind: &RepoPackKind) -> &'static str {
    match kind {
        RepoPackKind::SourceProvider => "source",
        RepoPackKind::Scanner => "scan",
        RepoPackKind::Signing => "signing",
        RepoPackKind::Attestation => "attestation",
        RepoPackKind::PolicyEngine => "policy",
        RepoPackKind::OciProvider => "oci",
        RepoPackKind::BillingProvider => "billing",
        RepoPackKind::SearchProvider => "search",
        RepoPackKind::RecommendationProvider => "reco",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> RepoBinding {
        RepoBinding {
            package: "greentic:scanner".to_string(),
            world: "scan".to_string(),
            version: "0.1.0".to_string(),
            component: "scanner.component".to_string(),
            entrypoint: "scan".to_string(),
            profile: Some("default".to_string()),
        }
    }

    #[test]
    fn validate_accepts_matching_kind_capabilities_and_bindings() {
        let section = RepoPackSection {
            kind: RepoPackKind::Scanner,
            capabilities: RepoCapabilities {
                scan: vec!["repos".to_string()],
                ..Default::default()
            },
            bindings: RepoBindings {
                scan: vec![binding()],
                ..Default::default()
            },
        };

        section
            .validate()
            .expect("matching scanner config should pass");
    }

    #[test]
    fn validate_rejects_unexpected_capability_key_for_kind() {
        let section = RepoPackSection {
            kind: RepoPackKind::Scanner,
            capabilities: RepoCapabilities {
                source: vec!["git".to_string()],
                scan: vec!["repos".to_string()],
                ..Default::default()
            },
            bindings: RepoBindings {
                scan: vec![binding()],
                ..Default::default()
            },
        };

        let err = section
            .validate()
            .expect_err("source capabilities should be invalid for scanner");

        assert!(err.to_string().contains("may not include `source`"));
    }

    #[test]
    fn binding_validation_rejects_blank_optional_profile() {
        let mut binding = binding();
        binding.profile = Some(" ".to_string());

        let err = binding
            .validate("scan")
            .expect_err("blank profile should fail");

        assert!(err.to_string().contains("profile must not be empty"));
    }

    #[test]
    fn display_uses_kebab_case_names() {
        assert_eq!(RepoPackKind::PolicyEngine.to_string(), "policy-engine");
        assert_eq!(
            RepoPackKind::RecommendationProvider.to_string(),
            "recommendation-provider"
        );
    }

    fn all_kinds() -> Vec<(RepoPackKind, &'static str)> {
        vec![
            (RepoPackKind::SourceProvider, "source"),
            (RepoPackKind::Scanner, "scan"),
            (RepoPackKind::Signing, "signing"),
            (RepoPackKind::Attestation, "attestation"),
            (RepoPackKind::PolicyEngine, "policy"),
            (RepoPackKind::OciProvider, "oci"),
            (RepoPackKind::BillingProvider, "billing"),
            (RepoPackKind::SearchProvider, "search"),
            (RepoPackKind::RecommendationProvider, "reco"),
        ]
    }

    fn capabilities_for(key: &str) -> RepoCapabilities {
        let mut caps = RepoCapabilities::default();
        match key {
            "source" => caps.source.push("git".to_string()),
            "scan" => caps.scan.push("repos".to_string()),
            "signing" => caps.signing.push("sign".to_string()),
            "attestation" => caps.attestation.push("attest".to_string()),
            "policy" => caps.policy.push("enforce".to_string()),
            "oci" => caps.oci.push("push".to_string()),
            "billing" => caps.billing.push("invoice".to_string()),
            "search" => caps.search.push("query".to_string()),
            "reco" => caps.reco.push("recommend".to_string()),
            other => panic!("unexpected capability key {other}"),
        }
        caps
    }

    fn bindings_for(key: &str) -> RepoBindings {
        let mut bindings = RepoBindings::default();
        match key {
            "source" => bindings.source.push(binding()),
            "scan" => bindings.scan.push(binding()),
            "signing" => bindings.signing.push(binding()),
            "attestation" => bindings.attestation.push(binding()),
            "policy" => bindings.policy.push(binding()),
            "oci" => bindings.oci.push(binding()),
            "billing" => bindings.billing.push(binding()),
            "search" => bindings.search.push(binding()),
            "reco" => bindings.reco.push(binding()),
            other => panic!("unexpected binding key {other}"),
        }
        bindings
    }

    #[test]
    fn validate_accepts_all_supported_repo_kinds() {
        for (kind, key) in all_kinds() {
            RepoPackSection {
                kind,
                capabilities: capabilities_for(key),
                bindings: bindings_for(key),
            }
            .validate()
            .unwrap_or_else(|err| panic!("kind {key} should validate: {err}"));
        }
    }

    #[test]
    fn validate_requires_capabilities_for_selected_kind() {
        for (kind, key) in all_kinds() {
            let err = RepoPackSection {
                kind,
                capabilities: RepoCapabilities::default(),
                bindings: bindings_for(key),
            }
            .validate()
            .expect_err("missing capabilities should fail");

            assert!(err.to_string().contains("must include at least one entry"));
        }
    }

    #[test]
    fn validate_requires_bindings_for_selected_kind() {
        for (kind, key) in all_kinds() {
            let err = RepoPackSection {
                kind,
                capabilities: capabilities_for(key),
                bindings: RepoBindings::default(),
            }
            .validate()
            .expect_err("missing bindings should fail");

            assert!(err.to_string().contains("must include at least one entry"));
        }
    }
}
