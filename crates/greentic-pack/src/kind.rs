use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported pack kinds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    Application,
    /// Greentic Digital Worker application pack — emitted by
    /// `greentic-dw-pack-builder` from a `DwApplicationPackSpec`.
    /// Distinct from `Application` so deployers can apply DW-specific
    /// installation rules (per-tenant scope, agent flow wiring, etc.).
    DwApplication,
    SourceProvider,
    Scanner,
    Signing,
    Attestation,
    PolicyEngine,
    OciProvider,
    BillingProvider,
    SearchProvider,
    RecommendationProvider,
    DistributionBundle,
    /// Reserved for future phases; validation should reject this kind for now.
    RolloutStrategy,
}

impl PackKind {
    pub fn validate_allowed(&self) -> anyhow::Result<()> {
        if matches!(self, PackKind::RolloutStrategy) {
            anyhow::bail!("kind `rollout-strategy` is reserved and must not be used");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PackKind;

    #[test]
    fn validate_allowed_accepts_supported_kind() {
        PackKind::Application
            .validate_allowed()
            .expect("application should be allowed");
    }

    #[test]
    fn validate_allowed_accepts_dw_application() {
        PackKind::DwApplication
            .validate_allowed()
            .expect("dw-application should be allowed");
    }

    #[test]
    fn validate_allowed_rejects_reserved_kind() {
        let err = PackKind::RolloutStrategy
            .validate_allowed()
            .expect_err("reserved kind should fail");

        assert!(err.to_string().contains("rollout-strategy"));
    }

    #[test]
    fn dw_application_serializes_as_kebab_case() {
        let json = serde_json::to_string(&PackKind::DwApplication).expect("serialize");
        assert_eq!(json, "\"dw-application\"");
    }

    #[test]
    fn dw_application_round_trips_through_serde() {
        let kind: PackKind = serde_json::from_str("\"dw-application\"").expect("deserialize");
        assert_eq!(kind, PackKind::DwApplication);
    }
}
