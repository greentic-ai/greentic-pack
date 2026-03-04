#![allow(clippy::all)]
#![allow(missing_docs)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Rust bindings generated from the Greentic WIT worlds for guest usage.
pub mod generated {
    include!(concat!(
        env!("GREENTIC_INTERFACES_GUEST_BINDINGS"),
        "/mod.rs"
    ));
}

pub use generated::*;
