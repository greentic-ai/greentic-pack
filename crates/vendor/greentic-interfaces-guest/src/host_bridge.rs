#![cfg(all(not(target_arch = "wasm32"), feature = "host-bridge"))]

//! Host-side helpers that depend on `greentic-interfaces` (disabled for wasm).

pub use greentic_interfaces::mappers;
