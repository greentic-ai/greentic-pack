#![cfg(feature = "worker-v1")]

use greentic_interfaces_host::worker::{
    HostWorkerMessage, HostWorkerRequest, HostWorkerResponse,
    exports::greentic::worker::worker_api::WorkerRequest as WitWorkerRequest,
};
use greentic_types::TenantCtx;
use serde_json::json;

fn roundtrip_req(req: HostWorkerRequest) {
    let wit: WitWorkerRequest = req.clone().try_into().expect("host->wit");
    let _back: HostWorkerRequest = wit.try_into().expect("wit->host");
}

fn roundtrip_resp(resp: HostWorkerResponse) {
    let wit = (<HostWorkerResponse as TryInto<
        greentic_interfaces_host::worker::exports::greentic::worker::worker_api::WorkerResponse,
    >>::try_into(resp.clone()))
    .expect("host->wit");
    let _back: HostWorkerResponse = wit.try_into().expect("wit->host");
}

#[test]
fn host_worker_types_convert() {
    let tenant = TenantCtx {
        env: "prod".try_into().unwrap(),
        tenant: "tenant-1".try_into().unwrap(),
        tenant_id: "tenant-1".try_into().unwrap(),
        team: None,
        team_id: None,
        user: None,
        user_id: None,
        attributes: Default::default(),
        session_id: None,
        flow_id: None,
        node_id: None,
        provider_id: None,
        trace_id: None,
        i18n_id: None,
        correlation_id: None,
        deadline: None,
        attempt: 0,
        idempotency_key: None,
        impersonation: None,
    };

    let req = HostWorkerRequest {
        version: "1.0".into(),
        tenant: tenant.clone(),
        worker_id: "demo-worker".into(),
        payload: json!({"key":"value"}),
        timestamp_utc: "2025-01-01T00:00:00Z".into(),
        correlation_id: Some("corr-1".into()),
        session_id: None,
        thread_id: None,
    };
    roundtrip_req(req);

    let resp = HostWorkerResponse {
        version: "1.0".into(),
        tenant,
        worker_id: "demo-worker".into(),
        timestamp_utc: "2025-01-01T00:00:00Z".into(),
        messages: vec![HostWorkerMessage {
            kind: "text".into(),
            payload: json!({"msg":"hi"}),
        }],
        correlation_id: Some("corr-1".into()),
        session_id: None,
        thread_id: None,
    };
    roundtrip_resp(resp);
}
