#![cfg(feature = "distributor-api-imports")]

use greentic_interfaces_guest::distributor_api::DistributorApiImports;
use greentic_interfaces_guest::distributor_api::ResolveComponentRequest;

#[test]
fn distributor_imports_are_callable() {
    let client = DistributorApiImports::new();
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

    if cfg!(target_arch = "wasm32") {
        let resp = client.resolve_component(&req);
        let _ = resp.secret_requirements;

        let _ = client.get_pack_status(&tenant, &env, &pack);
        let pack_resp = client.get_pack_status_v2(&tenant, &env, &pack);
        let _ = pack_resp.secret_requirements;
        client.warm_pack(&tenant, &env, &pack);
    } else {
        let _ = &req;
        let _resolve: fn(
            &DistributorApiImports,
            &ResolveComponentRequest,
        )
            -> greentic_interfaces_guest::distributor_api::ResolveComponentResponse =
            DistributorApiImports::resolve_component;
        let _get_status: fn(&DistributorApiImports, &String, &String, &String) -> String =
            DistributorApiImports::get_pack_status;
        let _get_status_v2: fn(
            &DistributorApiImports,
            &String,
            &String,
            &String,
        )
            -> greentic_interfaces_guest::distributor_api::PackStatusResponse =
            DistributorApiImports::get_pack_status_v2;
        let _warm: fn(&DistributorApiImports, &String, &String, &String) =
            DistributorApiImports::warm_pack;
        let _ = (_resolve, _get_status, _get_status_v2, _warm);
    }
}
