#![cfg(feature = "oauth-broker-v1")]

use greentic_interfaces_host::oauth_broker::exports::greentic::oauth_broker::broker_v1::{
    Guest, GuestIndices,
};

#[test]
fn oauth_broker_guest_handles_are_exposed() {
    let _ = core::mem::size_of::<GuestIndices>();
    let _ = core::mem::size_of::<Guest>();
}
