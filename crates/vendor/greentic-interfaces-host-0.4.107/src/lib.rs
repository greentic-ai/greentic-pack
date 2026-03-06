#![deny(unsafe_code)]
#![warn(missing_docs, clippy::unwrap_used, clippy::expect_used)]
//! Host-facing bindings and mappers re-exported from `greentic-interfaces`.
//! `greentic:component@0.6.0` exports descriptor/schema/runtime/qa/i18n and is bound via
//! [`component_v0_6`].

#[cfg(target_arch = "wasm32")]
compile_error!("greentic-interfaces-host is intended for native host targets.");

pub use greentic_interfaces::{bindings, mappers, validate};

/// Host bindings for `greentic:component/component@0.6.0` (world `component-v0-v6-v0`).
pub mod component_v0_6 {
    pub use greentic_interfaces::component_v0_6::*;

    /// Back-compat alias for older host helper code/tests.
    pub type ComponentV0V6V0 = Component;
}

/// Component control and exports.
pub mod component {
    /// Component ABI with descriptor/schema/runtime/qa/i18n exports `greentic:component/component@0.6.0`.
    pub mod v0_6 {
        pub use greentic_interfaces::component_v0_6::*;
    }
    /// Component ABI with config surface `greentic:component/component@0.5.0`.
    pub mod v0_5 {
        pub use greentic_interfaces::component_v0_5::*;
    }
    /// Component ABI with optional config schema export `greentic:component/component-configurable@0.5.0`.
    pub mod v0_5_configurable {
        pub use greentic_interfaces::component_configurable_v0_5::*;
    }
    /// Compatibility exports for `greentic:component/component@0.4.0`.
    pub mod v0_4 {
        pub use greentic_interfaces::component_v0_4::*;
    }
    /// Generic component invocation world `greentic:component-v1/component-host@0.1.0`.
    pub mod v1 {
        pub use greentic_interfaces::component_v1::*;
        pub use greentic_interfaces::mappers::ComponentOutcome;
        pub use greentic_interfaces::mappers::ComponentOutcomeStatus;
    }
    /// Describe-only schema export world `greentic:component/component@1.0.0`.
    pub mod describe_v1 {
        pub use greentic_interfaces::component_describe_v1::*;
    }
    /// Lifecycle hooks world `greentic:lifecycle/component-lifecycle@1.0.0`.
    pub mod lifecycle_v1 {
        pub use greentic_interfaces::component_lifecycle_v1::*;
    }
}

/// Runner host bundle `greentic:host@1.0.0`.
pub mod runner_host_v1 {
    pub use greentic_interfaces::runner_host_v1::*;
}

/// Operator hook contracts `greentic:operator/hook-provider@1.0.0`.
pub mod operator_v1 {
    pub use greentic_interfaces::operator_hook_provider_v1::*;
}

/// Pack exporters.
pub mod pack_exports {
    /// Pack exports `0.2.0` world.
    pub mod v0_2 {
        pub use greentic_interfaces::pack_export_v0_2::*;
    }
    /// Pack exports `0.4.0` world.
    pub mod v0_4 {
        pub use greentic_interfaces::pack_export_v0_4::*;
    }
    /// Pack metadata/flow discovery world `greentic:pack-export-v1/pack-host@0.1.0`.
    pub mod v1 {
        pub use greentic_interfaces::mappers::{
            FlowDescriptor as HostFlowDescriptor, PackDescriptor as HostPackDescriptor,
        };
        pub use greentic_interfaces::pack_export_v1::*;
    }
}

/// Core types.
pub mod types {
    /// Shared flow/component fundamentals `greentic:common-types/common@0.1.0`.
    pub mod common_v0_1 {
        pub use greentic_interfaces::common_types_v0_1::*;
    }
    /// Core type defs for the 0.2 line.
    pub mod types_core_v0_2 {
        pub use greentic_interfaces::types_core_v0_2::*;
    }
    /// Core type defs for the 0.4 line.
    pub mod types_core_v0_4 {
        pub use greentic_interfaces::types_core_v0_4::*;
    }
}

/// v1 host capability contracts.
pub mod secrets {
    /// `greentic:secrets-store/store@1.0.0` host imports.
    pub mod store_v1 {
        pub use greentic_interfaces::secrets_store_v1::*;
    }
}

/// Provider core schema world `greentic:provider-schema-core@1.0.0`.
#[cfg(feature = "provider-core-v1")]
pub mod provider_core_v1 {
    pub use greentic_interfaces::provider_schema_core_v1::*;
}

/// Shared messaging provider metadata/render helpers.
pub mod provider_common {
    pub use greentic_interfaces::bindings::provider_common_0_0_2_common::exports::provider::common::capabilities::*;
    pub use greentic_interfaces::bindings::provider_common_0_0_2_common::exports::provider::common::render::*;
}

/// v1 host capability contracts.
pub mod state {
    pub use greentic_interfaces::state_store_v1::*;
}

/// v1 host capability contracts.
pub mod http_client {
    /// `greentic:http/client@1.0.0` bindings.
    pub use greentic_interfaces::http_client_v1::*;

    /// `greentic:http/client@1.1.0` bindings with request options + tenant context.
    pub mod v1_1 {
        pub use greentic_interfaces::http_client_v1_1::*;
    }
}

/// v1 host capability contracts.
pub mod telemetry {
    pub use greentic_interfaces::telemetry_logger_v1::*;
}

/// v1 host capability contracts.
#[cfg(feature = "oauth-broker-v1")]
pub mod oauth_broker {
    pub use greentic_interfaces::oauth_broker_v1::*;
}

/// v1 OAuth broker client imports.
#[cfg(feature = "oauth-broker-v1")]
pub mod oauth_broker_client {
    pub use greentic_interfaces::oauth_broker_client_v1::*;
}

/// Generic worker ABI world.
#[cfg(feature = "worker-v1")]
pub mod worker {
    use greentic_interfaces::bindings::greentic::interfaces_types::types as interfaces_types;
    use greentic_interfaces::worker_v1::exports::greentic::worker::worker_api::{
        TenantCtx as WitWorkerTenantCtx, WorkerMessage as WitWorkerMessage,
        WorkerRequest as WitWorkerRequest, WorkerResponse as WitWorkerResponse,
    };
    use greentic_interfaces::worker_v1::greentic::types_core::types::{
        Cloud, DeploymentCtx, Platform,
    };
    use greentic_types::{ErrorCode, GreenticError, TenantCtx};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    pub use greentic_interfaces::worker_v1::*;

    type MapperResult<T> = Result<T, GreenticError>;

    fn to_worker_tenant(ctx: TenantCtx) -> MapperResult<WitWorkerTenantCtx> {
        let base = crate::mappers::tenant_ctx_to_wit(ctx)?;
        Ok(WitWorkerTenantCtx {
            tenant: base.tenant,
            team: base.team,
            user: base.user,
            deployment: DeploymentCtx {
                cloud: Cloud::Other,
                region: None,
                platform: Platform::Other,
                runtime: None,
                i18n_id: base.i18n_id.clone(),
            },
            trace_id: base.trace_id,
            i18n_id: base.i18n_id,
            session_id: base.session_id,
            flow_id: base.flow_id,
            node_id: base.node_id,
            provider_id: base.provider_id,
        })
    }

    fn from_worker_tenant(ctx: WitWorkerTenantCtx) -> MapperResult<TenantCtx> {
        let team = ctx.team.clone();
        let user = ctx.user.clone();
        let base = interfaces_types::TenantCtx {
            env: "unknown".to_string(),
            tenant: ctx.tenant.clone(),
            tenant_id: ctx.tenant,
            team,
            team_id: ctx.team,
            user,
            user_id: ctx.user,
            trace_id: ctx.trace_id,
            i18n_id: ctx.i18n_id.or(ctx.deployment.i18n_id),
            correlation_id: None,
            session_id: ctx.session_id,
            flow_id: ctx.flow_id,
            node_id: ctx.node_id,
            provider_id: ctx.provider_id,
            deadline_ms: None,
            attempt: 0,
            idempotency_key: None,
            impersonation: None,
            attributes: Vec::new(),
        };
        crate::mappers::tenant_ctx_from_wit(base)
    }

    /// Host-friendly request wrapper for worker invocations (uses `greentic-types` and `serde_json` payloads).
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct HostWorkerRequest {
        /// ABI version identifier (e.g. "1.0").
        pub version: String,
        /// Shared tenant context from `greentic-types`.
        pub tenant: TenantCtx,
        /// Target worker identifier.
        pub worker_id: String,
        /// JSON payload for the worker.
        pub payload: Value,
        /// ISO8601 UTC timestamp of the request.
        pub timestamp_utc: String,
        /// Optional correlation identifier.
        pub correlation_id: Option<String>,
        /// Optional session identifier.
        pub session_id: Option<String>,
        /// Optional thread identifier.
        pub thread_id: Option<String>,
    }

    /// Host-friendly worker message envelope.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct HostWorkerMessage {
        /// Message kind (e.g. "text", "card").
        pub kind: String,
        /// JSON payload content.
        pub payload: Value,
    }

    /// Host-friendly worker response wrapper with typed tenant context.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct HostWorkerResponse {
        /// Mirrors the request version.
        pub version: String,
        /// Shared tenant context from `greentic-types`.
        pub tenant: TenantCtx,
        /// Worker identifier that produced the response.
        pub worker_id: String,
        /// ISO8601 UTC timestamp.
        pub timestamp_utc: String,
        /// Accumulated worker messages.
        pub messages: Vec<HostWorkerMessage>,
        /// Optional correlation identifier.
        pub correlation_id: Option<String>,
        /// Optional session identifier.
        pub session_id: Option<String>,
        /// Optional thread identifier.
        pub thread_id: Option<String>,
    }

    impl TryFrom<HostWorkerMessage> for WitWorkerMessage {
        type Error = GreenticError;

        fn try_from(value: HostWorkerMessage) -> MapperResult<Self> {
            let payload_json = serde_json::to_string(&value.payload)
                .map_err(|err| GreenticError::new(ErrorCode::InvalidInput, err.to_string()))?;
            Ok(Self {
                kind: value.kind,
                payload_json,
            })
        }
    }

    impl TryFrom<WitWorkerMessage> for HostWorkerMessage {
        type Error = GreenticError;

        fn try_from(value: WitWorkerMessage) -> MapperResult<Self> {
            let payload = serde_json::from_str(&value.payload_json).map_err(|err| {
                GreenticError::new(
                    ErrorCode::InvalidInput,
                    format!("invalid worker payload: {err}"),
                )
            })?;
            Ok(Self {
                kind: value.kind,
                payload,
            })
        }
    }

    impl TryFrom<HostWorkerRequest> for WitWorkerRequest {
        type Error = GreenticError;

        fn try_from(value: HostWorkerRequest) -> MapperResult<Self> {
            let payload_json = serde_json::to_string(&value.payload)
                .map_err(|err| GreenticError::new(ErrorCode::InvalidInput, err.to_string()))?;
            Ok(Self {
                version: value.version,
                tenant: to_worker_tenant(value.tenant)?,
                worker_id: value.worker_id,
                correlation_id: value.correlation_id,
                session_id: value.session_id,
                thread_id: value.thread_id,
                payload_json,
                timestamp_utc: value.timestamp_utc,
            })
        }
    }

    impl TryFrom<WitWorkerRequest> for HostWorkerRequest {
        type Error = GreenticError;

        fn try_from(value: WitWorkerRequest) -> MapperResult<Self> {
            let payload: Value = serde_json::from_str(&value.payload_json).map_err(|err| {
                GreenticError::new(
                    ErrorCode::InvalidInput,
                    format!("invalid worker payload: {err}"),
                )
            })?;
            Ok(Self {
                version: value.version,
                tenant: from_worker_tenant(value.tenant)?,
                worker_id: value.worker_id,
                correlation_id: value.correlation_id,
                session_id: value.session_id,
                thread_id: value.thread_id,
                payload,
                timestamp_utc: value.timestamp_utc,
            })
        }
    }

    impl TryFrom<HostWorkerResponse> for WitWorkerResponse {
        type Error = GreenticError;

        fn try_from(value: HostWorkerResponse) -> MapperResult<Self> {
            let messages = value
                .messages
                .into_iter()
                .map(WitWorkerMessage::try_from)
                .collect::<MapperResult<Vec<_>>>()?;
            Ok(Self {
                version: value.version,
                tenant: to_worker_tenant(value.tenant)?,
                worker_id: value.worker_id,
                correlation_id: value.correlation_id,
                session_id: value.session_id,
                thread_id: value.thread_id,
                messages,
                timestamp_utc: value.timestamp_utc,
            })
        }
    }

    impl TryFrom<WitWorkerResponse> for HostWorkerResponse {
        type Error = GreenticError;

        fn try_from(value: WitWorkerResponse) -> MapperResult<Self> {
            let messages = value
                .messages
                .into_iter()
                .map(HostWorkerMessage::try_from)
                .collect::<MapperResult<Vec<_>>>()?;
            Ok(Self {
                version: value.version,
                tenant: from_worker_tenant(value.tenant)?,
                worker_id: value.worker_id,
                correlation_id: value.correlation_id,
                session_id: value.session_id,
                thread_id: value.thread_id,
                messages,
                timestamp_utc: value.timestamp_utc,
            })
        }
    }
}

/// GUI fragment renderers implemented by components.
#[cfg(feature = "gui-fragment")]
pub mod gui_fragment {
    pub use greentic_interfaces::bindings::greentic_gui_1_0_0_gui_fragment::exports::greentic::gui::fragment_api as bindings;
    pub use bindings::FragmentContext;
    pub use bindings::Guest as GuiFragment;
}

/// Supply-chain provider contracts.
pub mod supply_chain {
    /// Source provider world `greentic:source/source-sync@1.0.0`.
    pub mod source {
        pub use greentic_interfaces::bindings::greentic_source_1_0_0_source_sync::exports::greentic::source::source_api::*;
    }
    /// Build provider world `greentic:build/builder@1.0.0`.
    pub mod build {
        pub use greentic_interfaces::bindings::greentic_build_1_0_0_builder::exports::greentic::build::builder_api::*;
    }
    /// Scanner world `greentic:scan/scanner@1.0.0`.
    pub mod scan {
        pub use greentic_interfaces::bindings::greentic_scan_1_0_0_scanner::exports::greentic::scan::scanner_api::*;
    }
    /// Signing world `greentic:signing/signer@1.0.0`.
    pub mod signing {
        pub use greentic_interfaces::bindings::greentic_signing_1_0_0_signer::exports::greentic::signing::signer_api::*;
    }
    /// Attestation world `greentic:attestation/attester@1.0.0`.
    pub mod attestation {
        pub use greentic_interfaces::bindings::greentic_attestation_1_0_0_attester::exports::greentic::attestation::attester_api::*;
    }
    /// Policy evaluation world `greentic:policy/policy-evaluator@1.0.0`.
    pub mod policy {
        pub use greentic_interfaces::bindings::greentic_policy_1_0_0_policy_evaluator::exports::greentic::policy::policy_api::*;
    }
    /// Metadata store world `greentic:metadata/metadata-store@1.0.0`.
    pub mod metadata {
        pub use greentic_interfaces::bindings::greentic_metadata_1_0_0_metadata_store::exports::greentic::metadata::metadata_api::*;
    }
    /// OCI distribution world `greentic:oci/oci-distribution@1.0.0`.
    pub mod oci {
        pub use greentic_interfaces::bindings::greentic_oci_1_0_0_oci_distribution::exports::greentic::oci::oci_api::*;
    }
}

/// Desired state distribution contracts.
pub mod distribution {
    /// `greentic:distribution/distribution@1.0.0`.
    pub mod v1 {
        pub use greentic_interfaces::bindings::greentic_distribution_1_0_0_distribution::exports::greentic::distribution::distribution_api::*;
    }
}

/// Distributor API contracts.
pub mod distributor_api {
    /// `greentic:distributor-api/distributor-api@1.0.0`.
    pub mod v1 {
        pub use greentic_interfaces::bindings::greentic_distributor_api_1_0_0_distributor_api::exports::greentic::distributor_api::distributor::*;
    }
    /// `greentic:distributor-api/distributor-api@1.1.0`.
    pub mod v1_1 {
        pub use greentic_interfaces::bindings::greentic_distributor_api_1_1_0_distributor_api::exports::greentic::distributor_api::distributor::*;
    }

    /// Resolved component metadata returned by a ref-based lookup.
    #[derive(Debug, Clone)]
    pub struct ResolvedComponent {
        /// Digest returned by the distributor (opaque string).
        pub digest: String,
        /// Metadata returned by `resolve-ref`.
        pub metadata: v1_1::ResolveRefMetadata,
    }

    /// Resolved component artifact content.
    #[derive(Debug, Clone, PartialEq)]
    pub enum ResolvedArtifact {
        /// Raw component bytes.
        Bytes(Vec<u8>),
        /// Filesystem path to the component.
        Path(String),
    }

    /// Host-side resolver for ref-based component lookup.
    pub trait ComponentResolver {
        /// Resolve a component reference string to a digest plus metadata.
        fn resolve_ref(&self, component_ref: &str) -> ResolvedComponent;

        /// Fetch a resolved component artifact by digest.
        fn fetch_digest(&self, digest: &str) -> ResolvedArtifact;
    }
}

/// Stable alias for HTTP client imports.
pub mod http {
    pub use super::http_client::*;
}

/// Stable alias for OAuth broker imports.
#[cfg(feature = "oauth-broker-v1")]
pub mod oauth {
    pub use super::oauth_broker::*;
}

/// MCP router surfaces (multiple protocol snapshots).
pub mod mcp {
    /// `wasix:mcp@24.11.5` snapshot (2024-11-05 spec).
    #[cfg(feature = "wasix-mcp-24-11-05-host")]
    pub mod v24_11_05 {
        pub use greentic_interfaces::wasix_mcp_24_11_05::*;
    }

    /// `wasix:mcp@25.3.26` snapshot with annotations/audio/completions/progress.
    #[cfg(feature = "wasix-mcp-25-03-26-host")]
    pub mod v25_03_26 {
        pub use greentic_interfaces::wasix_mcp_25_03_26::*;
    }

    /// `wasix:mcp@25.6.18` snapshot with structured output/resources/elicitation.
    #[cfg(feature = "wasix-mcp-25-06-18-host")]
    pub mod v25_06_18 {
        pub use greentic_interfaces::wasix_mcp_25_06_18::*;
    }
}

/// UI action handler contracts.
pub mod ui_actions {
    /// UI action handler world `greentic:repo-ui-actions/repo-ui-worker@1.0.0`.
    pub mod repo_ui_worker {
        pub use greentic_interfaces::bindings::greentic_repo_ui_actions_1_0_0_repo_ui_worker::exports::greentic::repo_ui_actions::ui_action_api::*;
    }
}
