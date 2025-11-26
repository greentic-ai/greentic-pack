use std::fmt;

use anyhow::{Result, bail};
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
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct RepoBinding {
    pub world: String,
    pub component_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl RepoBinding {
    fn validate(&self, label: &str) -> Result<()> {
        if self.world.trim().is_empty() {
            bail!("bindings.{label}[].world is required");
        }
        if self.component_id.trim().is_empty() {
            bail!("bindings.{label}[].component_id is required");
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
