#![forbid(unsafe_code)]
//! Build-time i18n materialisation for packs: extract card strings,
//! translate via greentic-i18n-translator, and write assets/i18n/.

mod extract;

pub use extract::{ExtractConfig, ExtractedString, extract_from_directory, write_bundle};
