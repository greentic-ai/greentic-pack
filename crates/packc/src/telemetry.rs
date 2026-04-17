use anyhow::Result;
use greentic_config_types::{TelemetryConfig, TelemetryExporterKind};
pub use greentic_telemetry::with_task_local;
use greentic_telemetry::{
    TelemetryConfig as ServiceTelemetryConfig, TelemetryCtx,
    export::{ExportConfig, ExportMode, Sampling},
    init_telemetry_auto, init_telemetry_from_config, set_current_telemetry_ctx,
};
use greentic_types::TenantCtx;

/// Install the default Greentic telemetry stack for the given service.
pub fn install(service_name: &str) -> Result<()> {
    init_telemetry_auto(ServiceTelemetryConfig {
        service_name: service_name.to_string(),
    })
}

/// Install telemetry honoring greentic-config telemetry settings.
pub fn install_with_config(service_name: &str, cfg: &TelemetryConfig) -> Result<()> {
    if !cfg.enabled || matches!(cfg.exporter, TelemetryExporterKind::None) {
        return Ok(());
    }

    let export = match cfg.exporter {
        TelemetryExporterKind::Otlp => {
            let mut export = ExportConfig::default();
            export.mode = ExportMode::OtlpGrpc;
            export.endpoint = cfg.endpoint.clone();
            export.sampling = Sampling::TraceIdRatio(cfg.sampling as f64);
            export.compression = None;
            export
        }
        TelemetryExporterKind::Stdout => {
            let mut export = ExportConfig::default();
            export.mode = ExportMode::JsonStdout;
            export.endpoint = None;
            export.sampling = Sampling::TraceIdRatio(cfg.sampling as f64);
            export.compression = None;
            export
        }
        TelemetryExporterKind::None => unreachable!("handled above"),
    };

    init_telemetry_from_config(
        ServiceTelemetryConfig {
            service_name: service_name.to_string(),
        },
        export,
    )
}

/// Map the provided tenant context into the task-local telemetry slot.
pub fn set_current_tenant_ctx(ctx: &TenantCtx) {
    let mut telemetry = TelemetryCtx::new(ctx.tenant_id.as_ref());

    if let Some(session) = ctx.session_id() {
        telemetry = telemetry.with_session(session);
    }
    if let Some(flow) = ctx.flow_id() {
        telemetry = telemetry.with_flow(flow);
    }
    if let Some(node) = ctx.node_id() {
        telemetry = telemetry.with_node(node);
    }
    if let Some(provider) = ctx.provider_id() {
        telemetry = telemetry.with_provider(provider);
    }

    set_current_telemetry_ctx(telemetry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use greentic_config_types::TelemetryConfig;
    use greentic_types::{EnvId, TenantCtx, TenantId};
    use std::str::FromStr;

    #[test]
    fn install_with_config_is_noop_when_disabled() {
        let cfg = TelemetryConfig {
            enabled: false,
            exporter: TelemetryExporterKind::Otlp,
            endpoint: Some("http://localhost:4317".to_string()),
            sampling: 1.0,
        };

        install_with_config("packc-test", &cfg).expect("disabled config should be a no-op");
    }

    #[test]
    fn install_with_config_is_noop_for_none_exporter() {
        let cfg = TelemetryConfig {
            enabled: true,
            exporter: TelemetryExporterKind::None,
            endpoint: None,
            sampling: 0.25,
        };

        install_with_config("packc-test", &cfg).expect("none exporter should be a no-op");
    }

    #[test]
    fn set_current_tenant_ctx_accepts_full_context() {
        let tenant = TenantId::from_str("tenant-a").expect("tenant");
        let env = EnvId::from_str("dev").expect("env");
        let ctx = TenantCtx::new(env, tenant)
            .with_session("sess-123")
            .with_flow("flow.main")
            .with_node("node-1")
            .with_provider("provider-1");

        set_current_tenant_ctx(&ctx);
    }
}
