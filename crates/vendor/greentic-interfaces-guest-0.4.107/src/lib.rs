#![deny(unsafe_code)]
#![warn(missing_docs, clippy::unwrap_used, clippy::expect_used)]
//! Guest-facing bindings and mappers without host-world leakage.

pub mod bindings;

#[cfg(all(not(target_arch = "wasm32"), feature = "host-bridge"))]
pub mod host_bridge;

#[cfg(feature = "component-node")]
#[doc(hidden)]
pub mod component_entrypoint;

#[cfg(feature = "component-node")]
pub use component_entrypoint::{NODE_EXPORT_PREFIX, stream_from_invoke_result};

#[cfg(feature = "distributor-api-imports")]
mod distributor_api_imports;
#[cfg(feature = "distributor-api-v1-1-imports")]
mod distributor_api_imports_v1_1;

/// Component exports for `greentic:component/component@0.5.0`.
#[cfg(feature = "component-node")]
pub mod component {
    pub use crate::bindings::greentic_component_0_5_0_component::exports::greentic::component::*;
}

/// Component exports for `greentic:component/component@0.6.0` (world `component`).
///
/// Enable feature `component-v0-6` and export your implementation:
///
/// ```rust
/// # use greentic_interfaces_guest::component_v0_6::node;
/// # struct MyImpl;
/// # impl node::Guest for MyImpl {
/// #     fn describe() -> node::ComponentDescriptor {
/// #         node::ComponentDescriptor {
/// #             name: "demo".into(),
/// #             version: "0.1.0".into(),
/// #             summary: None,
/// #             capabilities: vec![],
/// #             ops: vec![],
/// #             schemas: vec![],
/// #             setup: None,
/// #         }
/// #     }
/// #     fn invoke(_op: String, _envelope: node::InvocationEnvelope) -> Result<node::InvocationResult, node::NodeError> {
/// #         Ok(node::InvocationResult {
/// #             ok: true,
/// #             output_cbor: vec![],
/// #             output_metadata_cbor: None,
/// #         })
/// #     }
/// # }
/// greentic_interfaces_guest::export_component_v060!(MyImpl);
/// ```
#[cfg(feature = "component-v0-6")]
pub mod component_v0_6 {
    pub use crate::bindings::greentic_component_0_6_0_component::exports::greentic::component::*;
}

/// Exports a `greentic:component/node@0.6.0` guest implementation.
#[cfg(feature = "component-v0-6")]
#[macro_export]
macro_rules! export_component_v060 {
    ($ty:ty) => {
        const _: () = {
            use $crate::bindings::greentic_component_0_6_0_component::exports::greentic::component::node;

            #[unsafe(export_name = "greentic:component/node@0.6.0#describe")]
            unsafe extern "C" fn export_component_v060_describe() -> *mut u8 {
                unsafe { node::_export_describe_cabi::<$ty>() }
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.6.0#describe")]
            unsafe extern "C" fn export_component_v060_post_describe(arg0: *mut u8) {
                unsafe { node::__post_return_describe::<$ty>(arg0) }
            }

            #[unsafe(export_name = "greentic:component/node@0.6.0#invoke")]
            unsafe extern "C" fn export_component_v060_invoke(arg0: *mut u8) -> *mut u8 {
                unsafe { node::_export_invoke_cabi::<$ty>(arg0) }
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.6.0#invoke")]
            unsafe extern "C" fn export_component_v060_post_invoke(arg0: *mut u8) {
                unsafe { node::__post_return_invoke::<$ty>(arg0) }
            }
        };
    };
}

/// Legacy component exports for `greentic:component/component@0.4.0`.
#[cfg(feature = "component-node-v0-4")]
pub mod component_v0_4 {
    pub use crate::bindings::greentic_component_0_4_0_component::exports::greentic::component::*;
}

/// Generic component host ABI `greentic:component-v1/component-host@0.1.0`.
#[cfg(feature = "component-v1")]
pub mod component_v1 {
    pub use crate::bindings::greentic_component_v1_0_1_0_component_host::exports::greentic::component_v1::*;
    #[cfg(not(target_arch = "wasm32"))]
    pub use greentic_interfaces::mappers::{ComponentOutcome, ComponentOutcomeStatus};
}

/// Helper macro to export an implementation of `greentic:component/node@0.4.0`.
#[cfg(feature = "component-node-v0-4")]
#[macro_export]
macro_rules! export_component_node_v0_4 {
    ($ty:ty) => {
        const _: () = {
            use $crate::bindings::greentic_component_0_4_0_component::exports::greentic::component::node;

            #[unsafe(export_name = "greentic:component/node@0.4.0#get-manifest")]
            unsafe extern "C" fn export_get_manifest() -> *mut u8 {
                node::_export_get_manifest_cabi::<$ty>()
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#get-manifest")]
            unsafe extern "C" fn post_return_get_manifest(arg0: *mut u8) {
                node::__post_return_get_manifest::<$ty>(arg0);
            }

            #[unsafe(export_name = "greentic:component/node@0.4.0#on-start")]
            unsafe extern "C" fn export_on_start(arg0: *mut u8) -> *mut u8 {
                node::_export_on_start_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#on-start")]
            unsafe extern "C" fn post_return_on_start(arg0: *mut u8) {
                node::__post_return_on_start::<$ty>(arg0);
            }

            #[unsafe(export_name = "greentic:component/node@0.4.0#on-stop")]
            unsafe extern "C" fn export_on_stop(arg0: *mut u8) -> *mut u8 {
                node::_export_on_stop_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#on-stop")]
            unsafe extern "C" fn post_return_on_stop(arg0: *mut u8) {
                node::__post_return_on_stop::<$ty>(arg0);
            }

            #[unsafe(export_name = "greentic:component/node@0.4.0#invoke")]
            unsafe extern "C" fn export_invoke(arg0: *mut u8) -> *mut u8 {
                node::_export_invoke_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#invoke")]
            unsafe extern "C" fn post_return_invoke(arg0: *mut u8) {
                node::__post_return_invoke::<$ty>(arg0);
            }

            #[unsafe(export_name = "greentic:component/node@0.4.0#invoke-stream")]
            unsafe extern "C" fn export_invoke_stream(arg0: *mut u8) -> *mut u8 {
                node::_export_invoke_stream_cabi::<$ty>(arg0)
            }

            #[unsafe(export_name = "cabi_post_greentic:component/node@0.4.0#invoke-stream")]
            unsafe extern "C" fn post_return_invoke(arg0: *mut u8) {
                node::__post_return_invoke_stream::<$ty>(arg0);
            }
        };
    };
}

/// Lifecycle hooks for `greentic:lifecycle/component-lifecycle@1.0.0`.
#[cfg(feature = "lifecycle")]
pub mod lifecycle {
    pub use crate::bindings::greentic_lifecycle_1_0_0_component_lifecycle::exports::greentic::lifecycle::*;
}

/// Secret store imports for `greentic:secrets-store/store@1.0.0`.
#[cfg(feature = "secrets")]
pub mod secrets_store {
    pub use crate::bindings::greentic_secrets_store_1_0_0_store::greentic::secrets_store::secrets_store::*;
}

// Legacy secrets provider protocol removed; use provider-core instead.

/// Provider core schema exports for `greentic:provider-schema-core@1.0.0`.
#[cfg(feature = "provider-core-v1")]
pub mod provider_core {
    pub use crate::bindings::greentic_provider_schema_core_1_0_0_schema_core::exports::greentic::provider_schema_core::schema_core_api::*;
}

/// Operator hook-provider exports for `greentic:operator/hook-provider@1.0.0`.
#[cfg(feature = "operator-hooks-v1")]
pub mod operator_hooks {
    pub use crate::bindings::greentic_operator_1_0_0_hook_provider::exports::greentic::operator::hook_api::*;
}

/// Shared messaging provider metadata/render helpers `provider:common/common@0.0.2`.
#[cfg(feature = "provider-common")]
pub mod provider_common {
    pub use crate::bindings::provider_common_0_0_2_common::exports::provider::common::capabilities::*;
    pub use crate::bindings::provider_common_0_0_2_common::exports::provider::common::render::*;
}

/// State store imports for `greentic:state/store@1.0.0`.
#[cfg(feature = "state-store")]
pub mod state_store {
    pub use crate::bindings::greentic_state_1_0_0_store::greentic::state::state_store::*;
}

/// HTTP client imports for `greentic:http/client@1.0.0`.
#[cfg(feature = "http-client")]
pub mod http_client {
    pub use crate::bindings::greentic_http_1_0_0_client::greentic::http::http_client::*;
}

/// HTTP client imports for `greentic:http/client@1.1.0`.
#[cfg(feature = "http-client-v1-1")]
pub mod http_client_v1_1 {
    pub use crate::bindings::greentic_http_1_1_0_client::greentic::http::http_client::*;
}

/// Telemetry logger imports for `greentic:telemetry/logger@1.0.0`.
#[cfg(feature = "telemetry")]
pub mod telemetry_logger {
    pub use crate::bindings::greentic_telemetry_1_0_0_logger::greentic::telemetry::logger_api::*;
}

/// OAuth broker imports for `greentic:oauth-broker/broker@1.0.0`.
#[cfg(feature = "oauth-broker")]
pub mod oauth_broker {
    pub use crate::bindings::greentic_oauth_broker_1_0_0_broker::exports::greentic::oauth_broker::broker_v1::*;
}

/// OAuth broker client imports for `greentic:oauth-broker/broker-client@1.0.0`.
#[cfg(feature = "oauth-broker")]
pub mod oauth_broker_client {
    pub use crate::bindings::greentic_oauth_broker_1_0_0_broker_client::greentic::oauth_broker::broker_v1::*;
}

/// Generic worker world `greentic:worker/worker@1.0.0`.
#[cfg(feature = "worker")]
pub mod worker {
    pub use crate::bindings::greentic_worker_1_0_0_worker::exports::greentic::worker::worker_api::*;
}

/// GUI fragment world `greentic:gui/gui-fragment@1.0.0`.
#[cfg(feature = "gui-fragment")]
pub mod gui_fragment {
    pub use crate::bindings::greentic_gui_1_0_0_gui_fragment::exports::greentic::gui::fragment_api::*;
}

/// Pack validator world `greentic:pack-validate/pack-validator@0.1.0`.
#[cfg(feature = "pack-validate")]
pub mod pack_validate {
    pub use crate::bindings::greentic_pack_validate_0_1_0_pack_validator::exports::greentic::pack_validate::validator::*;
}

/// Provisioning world `greentic:provision/provision-runner@0.1.0`.
#[cfg(feature = "provision")]
pub mod provision {
    pub use crate::bindings::greentic_provision_0_1_0_provision_runner::exports::greentic::provision::provisioner::*;
}

/// Pack metadata/flow discovery worlds.
#[cfg(any(feature = "pack-export", feature = "pack-export-v1"))]
pub mod pack_exports {
    /// Pack exports `0.2.0` world.
    #[cfg(feature = "pack-export")]
    pub mod v0_2 {
        pub use crate::bindings::greentic_pack_export_0_2_0_pack_exports::exports::greentic::pack_export::*;
    }
    /// Pack exports `0.4.0` world.
    #[cfg(feature = "pack-export")]
    pub mod v0_4 {
        pub use crate::bindings::greentic_pack_export_0_4_0_pack_exports::exports::greentic::pack_export::*;
    }
    /// Pack host metadata world `greentic:pack-export-v1/pack-host@0.1.0`.
    #[cfg(feature = "pack-export-v1")]
    pub mod v1 {
        pub use crate::bindings::greentic_pack_export_v1_0_1_0_pack_host::exports::greentic::pack_export_v1::*;
        #[cfg(not(target_arch = "wasm32"))]
        pub use greentic_interfaces::mappers::{
            FlowDescriptor as GuestFlowDescriptor, PackDescriptor as GuestPackDescriptor,
        };
    }
}

/// Supply-chain provider contracts implemented by components.
#[cfg(any(
    feature = "repo",
    feature = "build",
    feature = "scan",
    feature = "signing",
    feature = "attestation",
    feature = "policy",
    feature = "metadata",
    feature = "oci"
))]
pub mod supply_chain {
    /// Source provider world `greentic:source/source-sync@1.0.0`.
    #[cfg(feature = "repo")]
    pub mod source {
        pub use crate::bindings::greentic_source_1_0_0_source_sync::exports::greentic::source::source_api::*;
    }
    /// Build provider world `greentic:build/builder@1.0.0`.
    #[cfg(feature = "build")]
    pub mod build {
        pub use crate::bindings::greentic_build_1_0_0_builder::exports::greentic::build::builder_api::*;
    }
    /// Scanner world `greentic:scan/scanner@1.0.0`.
    #[cfg(feature = "scan")]
    pub mod scan {
        pub use crate::bindings::greentic_scan_1_0_0_scanner::exports::greentic::scan::scanner_api::*;
    }
    /// Signing world `greentic:signing/signer@1.0.0`.
    #[cfg(feature = "signing")]
    pub mod signing {
        pub use crate::bindings::greentic_signing_1_0_0_signer::exports::greentic::signing::signer_api::*;
    }
    /// Attestation world `greentic:attestation/attester@1.0.0`.
    #[cfg(feature = "attestation")]
    pub mod attestation {
        pub use crate::bindings::greentic_attestation_1_0_0_attester::exports::greentic::attestation::attester_api::*;
    }
    /// Policy evaluation world `greentic:policy/policy-evaluator@1.0.0`.
    #[cfg(feature = "policy")]
    pub mod policy {
        pub use crate::bindings::greentic_policy_1_0_0_policy_evaluator::exports::greentic::policy::policy_api::*;
    }
    /// Metadata store world `greentic:metadata/metadata-store@1.0.0`.
    #[cfg(feature = "metadata")]
    pub mod metadata {
        pub use crate::bindings::greentic_metadata_1_0_0_metadata_store::exports::greentic::metadata::metadata_api::*;
    }
    /// OCI distribution world `greentic:oci/oci-distribution@1.0.0`.
    #[cfg(feature = "oci")]
    pub mod oci {
        pub use crate::bindings::greentic_oci_1_0_0_oci_distribution::exports::greentic::oci::oci_api::*;
    }
}

/// Desired state distribution API (experimental).
#[cfg(feature = "distribution")]
pub mod distribution {
    pub use crate::bindings::greentic_distribution_1_0_0_distribution::exports::greentic::distribution::distribution_api::*;
}

/// Distributor API for resolving pack components (active).
#[cfg(any(feature = "distributor-api", feature = "distributor-api-imports"))]
pub mod distributor_api {
    #[cfg(feature = "distributor-api")]
    pub use crate::bindings::greentic_distributor_api_1_0_0_distributor_api::exports::greentic::distributor_api::distributor::*;

    /// Raw imports generated from `greentic:distributor-api@1.0.0`.
    #[cfg(feature = "distributor-api-imports")]
    pub mod imports {
        pub use crate::bindings::greentic_distributor_api_1_0_0_distributor_api_imports::greentic::distributor_api::distributor::*;
    }

    /// Convenience wrapper around the distributor imports.
    #[cfg(feature = "distributor-api-imports")]
    pub use crate::distributor_api_imports::DistributorApiImports;
}

/// Distributor API for resolving pack components (ref-based v1.1).
#[cfg(any(
    feature = "distributor-api-v1-1",
    feature = "distributor-api-v1-1-imports"
))]
pub mod distributor_api_v1_1 {
    #[cfg(feature = "distributor-api-v1-1")]
    pub use crate::bindings::greentic_distributor_api_1_1_0_distributor_api::exports::greentic::distributor_api::distributor::*;

    /// Raw imports generated from `greentic:distributor-api@1.1.0`.
    #[cfg(feature = "distributor-api-v1-1-imports")]
    pub mod imports {
        pub use crate::bindings::greentic_distributor_api_1_1_0_distributor_api_imports::greentic::distributor_api::distributor::*;
    }

    /// Convenience wrapper around the distributor imports.
    #[cfg(feature = "distributor-api-v1-1-imports")]
    pub use crate::distributor_api_imports_v1_1::DistributorApiImportsV1_1;
}

/// MCP router exports for multiple protocol snapshots.
#[cfg(any(
    feature = "wasix-mcp-24-11-05-guest",
    feature = "wasix-mcp-25-03-26-guest",
    feature = "wasix-mcp-25-06-18-guest"
))]
pub mod mcp {
    /// `wasix:mcp@24.11.5` snapshot (2024-11-05 spec).
    #[cfg(feature = "wasix-mcp-24-11-05-guest")]
    pub mod v24_11_05 {
        pub use crate::bindings::wasix_mcp_24_11_5_mcp_router::exports::wasix::mcp::router::*;
    }

    /// `wasix:mcp@25.3.26` snapshot with annotations/audio/completions/progress.
    #[cfg(feature = "wasix-mcp-25-03-26-guest")]
    pub mod v25_03_26 {
        pub use crate::bindings::wasix_mcp_25_3_26_mcp_router::exports::wasix::mcp::router::*;
    }

    /// `wasix:mcp@25.6.18` snapshot with structured output/resources/elicitation.
    #[cfg(feature = "wasix-mcp-25-06-18-guest")]
    pub mod v25_06_18 {
        pub use crate::bindings::wasix_mcp_25_6_18_mcp_router::exports::wasix::mcp::router::*;
    }
}

/// UI action handler world `greentic:repo-ui-actions/repo-ui-worker@1.0.0`.
#[cfg(feature = "repo-ui-actions")]
pub mod repo_ui_actions {
    pub use crate::bindings::greentic_repo_ui_actions_1_0_0_repo_ui_worker::exports::greentic::repo_ui_actions::ui_action_api::*;
}

/// Stable alias for OAuth broker imports.
#[cfg(feature = "oauth-broker")]
pub mod oauth {
    pub use super::oauth_broker::*;
}
