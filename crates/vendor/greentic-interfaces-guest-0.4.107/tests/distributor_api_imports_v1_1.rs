#![cfg(feature = "distributor-api-v1-1-imports")]

use greentic_interfaces_guest::distributor_api_v1_1::DistributorApiImportsV1_1;
use greentic_interfaces_guest::distributor_api_v1_1::ResolveComponentRequest;

#[test]
fn distributor_imports_v1_1_are_callable() {
    let client = DistributorApiImportsV1_1::new();
    let req = ResolveComponentRequest {
        tenant_id: "tenant-a".to_string(),
        environment_id: "env-a".to_string(),
        pack_id: "pack-a".to_string(),
        component_id: "component-a".to_string(),
        version: "1.0.0".to_string(),
        extra: "{}".to_string(),
    };

    let tenant = req.tenant_id.clone();
    let env = req.environment_id.clone();
    let pack = req.pack_id.clone();
    let digest = "sha256:deadbeef";

    if cfg!(target_arch = "wasm32") {
        let resp = client.resolve_component(&req);
        let _ = resp.secret_requirements;

        let _ = client.resolve_ref("oci://example.com/greentic/component:v1");
        let _ = client.get_by_digest(digest);

        let _ = client.get_pack_status(&tenant, &env, &pack);
        let pack_resp = client.get_pack_status_v2(&tenant, &env, &pack);
        let _ = pack_resp.secret_requirements;
        client.warm_pack(&tenant, &env, &pack);
    } else {
        let _ = &req;
        let _resolve: fn(
            &DistributorApiImportsV1_1,
            &ResolveComponentRequest,
        )
            -> greentic_interfaces_guest::distributor_api_v1_1::ResolveComponentResponse =
            DistributorApiImportsV1_1::resolve_component;
        let _resolve_ref: fn(&DistributorApiImportsV1_1, &str)
            -> greentic_interfaces_guest::distributor_api_v1_1::ResolveRefResponse =
            DistributorApiImportsV1_1::resolve_ref;
        let _get_by_digest: fn(
            &DistributorApiImportsV1_1,
            &str,
        )
            -> greentic_interfaces_guest::distributor_api_v1_1::ArtifactSource =
            DistributorApiImportsV1_1::get_by_digest;
        let _get_status: fn(&DistributorApiImportsV1_1, &String, &String, &String) -> String =
            DistributorApiImportsV1_1::get_pack_status;
        let _get_status_v2: fn(
            &DistributorApiImportsV1_1,
            &String,
            &String,
            &String,
        )
            -> greentic_interfaces_guest::distributor_api_v1_1::PackStatusResponse =
            DistributorApiImportsV1_1::get_pack_status_v2;
        let _warm: fn(&DistributorApiImportsV1_1, &String, &String, &String) =
            DistributorApiImportsV1_1::warm_pack;
        let _ = (
            _resolve,
            _resolve_ref,
            _get_by_digest,
            _get_status,
            _get_status_v2,
            _warm,
        );
    }
}
