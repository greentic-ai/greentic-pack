use crate::bindings::greentic_distributor_api_1_1_0_distributor_api::exports::greentic::distributor_api::distributor
    as exports;
use crate::bindings::greentic_distributor_api_1_1_0_distributor_api_imports::greentic::distributor_api::distributor
    as imports;
use crate::bindings::greentic_distributor_api_1_1_0_distributor_api::greentic::secrets_types::types
    as export_secrets;
use crate::bindings::greentic_distributor_api_1_1_0_distributor_api_imports::greentic::secrets_types::types
    as import_secrets;

/// Thin client for calling `greentic:distributor-api@1.1.0` imports.
#[derive(Clone, Copy, Debug, Default)]
pub struct DistributorApiImportsV1_1;

impl DistributorApiImportsV1_1 {
    /// Creates a new client wrapper.
    pub const fn new() -> Self {
        Self
    }

    /// Resolves a component via the distributor host import.
    pub fn resolve_component(
        &self,
        req: &exports::ResolveComponentRequest,
    ) -> exports::ResolveComponentResponse {
        let import_req = imports::ResolveComponentRequest {
            tenant_id: req.tenant_id.clone(),
            environment_id: req.environment_id.clone(),
            pack_id: req.pack_id.clone(),
            component_id: req.component_id.clone(),
            version: req.version.clone(),
            extra: req.extra.clone(),
        };
        let response = imports::resolve_component(&import_req);
        exports::ResolveComponentResponse {
            component_status: to_export_status(response.component_status),
            digest: response.digest,
            artifact_location: exports::ArtifactLocation {
                kind: response.artifact_location.kind,
                value: response.artifact_location.value,
            },
            signature_summary: exports::SignatureSummary {
                verified: response.signature_summary.verified,
                signer: response.signature_summary.signer,
                extra: response.signature_summary.extra,
            },
            cache_info: exports::CacheInfo {
                size_bytes: response.cache_info.size_bytes,
                last_used_utc: response.cache_info.last_used_utc,
                last_refreshed_utc: response.cache_info.last_refreshed_utc,
            },
            secret_requirements: to_export_secret_requirements(response.secret_requirements),
        }
    }

    /// Resolves a component reference via the distributor host import.
    pub fn resolve_ref(&self, component_ref: &str) -> exports::ResolveRefResponse {
        let response = imports::resolve_ref(component_ref);
        exports::ResolveRefResponse {
            digest: response.digest,
            metadata: exports::ResolveRefMetadata {
                component_status: to_export_status(response.metadata.component_status),
                artifact_location: exports::ArtifactLocation {
                    kind: response.metadata.artifact_location.kind,
                    value: response.metadata.artifact_location.value,
                },
                signature_summary: exports::SignatureSummary {
                    verified: response.metadata.signature_summary.verified,
                    signer: response.metadata.signature_summary.signer,
                    extra: response.metadata.signature_summary.extra,
                },
                cache_info: exports::CacheInfo {
                    size_bytes: response.metadata.cache_info.size_bytes,
                    last_used_utc: response.metadata.cache_info.last_used_utc,
                    last_refreshed_utc: response.metadata.cache_info.last_refreshed_utc,
                },
                secret_requirements: to_export_secret_requirements(
                    response.metadata.secret_requirements,
                ),
            },
        }
    }

    /// Fetches a resolved component artifact by digest.
    pub fn get_by_digest(&self, digest: &str) -> exports::ArtifactSource {
        match imports::get_by_digest(digest) {
            imports::ArtifactSource::Bytes(bytes) => exports::ArtifactSource::Bytes(bytes),
            imports::ArtifactSource::Path(path) => exports::ArtifactSource::Path(path),
        }
    }

    /// Fetches pack status via the distributor host import.
    pub fn get_pack_status(
        &self,
        tenant_id: &exports::TenantId,
        env_id: &exports::DistributorEnvironmentId,
        pack_id: &exports::PackId,
    ) -> String {
        imports::get_pack_status(tenant_id, env_id, pack_id)
    }

    /// Fetches pack status and secret requirements via the distributor host import.
    pub fn get_pack_status_v2(
        &self,
        tenant_id: &exports::TenantId,
        env_id: &exports::DistributorEnvironmentId,
        pack_id: &exports::PackId,
    ) -> exports::PackStatusResponse {
        let response = imports::get_pack_status_v2(tenant_id, env_id, pack_id);
        exports::PackStatusResponse {
            status: response.status,
            secret_requirements: to_export_secret_requirements(response.secret_requirements),
            extra: response.extra,
        }
    }

    /// Warms a pack via the distributor host import.
    pub fn warm_pack(
        &self,
        tenant_id: &exports::TenantId,
        env_id: &exports::DistributorEnvironmentId,
        pack_id: &exports::PackId,
    ) {
        imports::warm_pack(tenant_id, env_id, pack_id);
    }
}

fn to_export_status(status: imports::ComponentStatus) -> exports::ComponentStatus {
    match status {
        imports::ComponentStatus::Pending => exports::ComponentStatus::Pending,
        imports::ComponentStatus::Ready => exports::ComponentStatus::Ready,
        imports::ComponentStatus::Failed => exports::ComponentStatus::Failed,
    }
}

fn to_export_secret_requirements(
    requirements: Vec<import_secrets::SecretRequirement>,
) -> Vec<exports::SecretRequirement> {
    requirements
        .into_iter()
        .map(|req| exports::SecretRequirement {
            key: req.key,
            required: req.required,
            description: req.description,
            scope: req.scope.map(|scope| export_secrets::SecretScope {
                env: scope.env,
                tenant: scope.tenant,
                team: scope.team,
            }),
            format: req.format.map(to_export_secret_format),
            schema: req.schema,
            examples: req.examples,
        })
        .collect()
}

fn to_export_secret_format(format: import_secrets::SecretFormat) -> export_secrets::SecretFormat {
    match format {
        import_secrets::SecretFormat::Bytes => export_secrets::SecretFormat::Bytes,
        import_secrets::SecretFormat::Text => export_secrets::SecretFormat::Text,
        import_secrets::SecretFormat::Json => export_secrets::SecretFormat::Json,
    }
}
