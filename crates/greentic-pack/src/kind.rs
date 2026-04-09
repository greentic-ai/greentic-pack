use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported pack kinds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    Application,
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
    fn validate_allowed_rejects_reserved_kind() {
        let err = PackKind::RolloutStrategy
            .validate_allowed()
            .expect_err("reserved kind should fail");

        assert!(err.to_string().contains("rollout-strategy"));
    }
}
