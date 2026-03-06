#![cfg(feature = "worker-v1")]

use greentic_interfaces_host::worker::exports::greentic::worker::worker_api::{
    WorkerError, WorkerMessage, WorkerRequest, WorkerResponse,
};

#[test]
fn worker_types_are_exposed() {
    let _ = core::mem::size_of::<WorkerRequest>();
    let _ = core::mem::size_of::<WorkerResponse>();
    let _ = core::mem::size_of::<WorkerMessage>();
    let _ = core::mem::size_of::<WorkerError>();
}
