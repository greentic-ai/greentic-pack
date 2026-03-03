#[test]
fn distributor_api_v1_1_types_are_available() {
    use greentic_interfaces_host::distributor_api::v1_1::{
        ArtifactLocation, ArtifactSource, CacheInfo, ComponentStatus, ResolveRefMetadata,
        ResolveRefResponse, SignatureSummary,
    };
    use greentic_interfaces_host::distributor_api::{
        ComponentResolver, ResolvedArtifact, ResolvedComponent,
    };

    let metadata = ResolveRefMetadata {
        component_status: ComponentStatus::Ready,
        artifact_location: ArtifactLocation {
            kind: "file".into(),
            value: "/tmp/component.wasm".into(),
        },
        signature_summary: SignatureSummary {
            verified: true,
            signer: "test-signer".into(),
            extra: "{}".into(),
        },
        cache_info: CacheInfo {
            size_bytes: 0,
            last_used_utc: "2024-01-01T00:00:00Z".into(),
            last_refreshed_utc: "2024-01-01T00:00:00Z".into(),
        },
        secret_requirements: Vec::new(),
    };

    let response = ResolveRefResponse {
        digest: "sha256:deadbeef".into(),
        metadata,
    };

    struct DummyResolver;

    impl ComponentResolver for DummyResolver {
        fn resolve_ref(&self, component_ref: &str) -> ResolvedComponent {
            ResolvedComponent {
                digest: component_ref.to_string(),
                metadata: ResolveRefMetadata {
                    component_status: ComponentStatus::Ready,
                    artifact_location: ArtifactLocation {
                        kind: "file".into(),
                        value: "/tmp/component.wasm".into(),
                    },
                    signature_summary: SignatureSummary {
                        verified: true,
                        signer: "test-signer".into(),
                        extra: "{}".into(),
                    },
                    cache_info: CacheInfo {
                        size_bytes: 0,
                        last_used_utc: "2024-01-01T00:00:00Z".into(),
                        last_refreshed_utc: "2024-01-01T00:00:00Z".into(),
                    },
                    secret_requirements: Vec::new(),
                },
            }
        }

        fn fetch_digest(&self, _digest: &str) -> ResolvedArtifact {
            ResolvedArtifact::Path("/tmp/component.wasm".into())
        }
    }

    let _ = response.digest;
    let _ = ArtifactSource::Path("/tmp/component.wasm".into());
    let _ = DummyResolver.fetch_digest("sha256:deadbeef");
}
