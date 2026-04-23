//! Dispatcher for the `greentic-pack info` subcommand.
//!
//! Reads a `.gtpack` archive, projects it into an
//! [`InfoReport`](super::info::InfoReport), and prints either a human-readable
//! summary or a JSON document (sorted keys, pretty-printed).
//!
//! Exit-code mapping (applied by [`run_with_cli`](super::run_with_cli) when
//! this dispatcher returns an error):
//! * `2` — path validation failed (missing file, not a regular file, or the
//!   extension is not `.gtpack`).
//! * `3` — strict mode requested but the pack is unsigned or its signature is
//!   invalid.
//! * `1` — any other failure (archive corruption, I/O error, ...).
//!
//! Errors returned from this module use stable message prefixes so the caller
//! can cheaply classify them. The prefixes come from the `cli.info.error.*`
//! i18n keys (English template), which keeps the programmatic check aligned
//! with the user-facing text.

use std::path::Path;

use anyhow::{Result, anyhow};
use greentic_pack::{SigningPolicy, open_pack};
use serde_json::Value;

use super::info::InfoReport;
use super::info::report::{SignatureInfo, SignatureStatus};
use super::inspect::InspectFormat;

/// Stable error-message prefix emitted when the user-supplied path is not a
/// readable `.gtpack` file. Matched in `run_with_cli` to set exit code `2`.
pub(crate) const ERR_NOT_A_PACK: &str = "not a .gtpack file";

/// Stable error-message prefix emitted when `--strict` rejected an unsigned or
/// invalidly-signed pack. Matched in `run_with_cli` to set exit code `3`.
pub(crate) const ERR_STRICT_UNSIGNED: &str = "Signature verification failed";

/// Entry point for `greentic-pack info <PATH> [--format human|json] [--strict]`.
pub fn handle(path: &Path, format: InspectFormat, strict: bool) -> Result<()> {
    validate_path(path)?;

    let policy = if strict {
        SigningPolicy::Strict
    } else {
        SigningPolicy::DevOk
    };

    let load = open_pack(path, policy).map_err(|e| {
        // `open_pack` in strict mode rejects unsigned packs for the legacy
        // manifest variant — re-emit with the strict-unsigned prefix so the
        // caller maps to exit code 3.
        if strict {
            anyhow!("{ERR_STRICT_UNSIGNED}: {}", e.message)
        } else {
            anyhow!("Failed to read pack: {}", e.message)
        }
    })?;

    // The newer `Gpack` manifest branch in `open_pack_inner` does not enforce
    // `SigningPolicy::Strict` — it downgrades to a warning and leaves
    // `signature_ok == false`. Make `--strict` behave consistently across both
    // manifest variants by rejecting any unsigned / invalidly-signed load here.
    if strict && !load.report.signature_ok {
        return Err(anyhow!(
            "{ERR_STRICT_UNSIGNED}: pack is unsigned or signature is invalid"
        ));
    }

    let sig = if load.report.signature_ok {
        SignatureInfo {
            status: SignatureStatus::Signed,
            key_id: None,
        }
    } else {
        SignatureInfo {
            status: SignatureStatus::Unsigned,
            key_id: None,
        }
    };

    let report = InfoReport::from_pack_meta_and_signature(&load.manifest.meta, sig);

    match format {
        InspectFormat::Json => {
            let value: Value = serde_json::to_value(&report)?;
            let sorted = sort_json(value);
            println!("{}", serde_json::to_string_pretty(&sorted)?);
        }
        InspectFormat::Human => {
            print!("{}", super::info::human::render(&report));
        }
    }

    Ok(())
}

/// Validate the user-supplied path before opening the archive. Returns an
/// error whose message begins with [`ERR_NOT_A_PACK`] on any failure so the
/// caller can map the outcome to exit code 2.
fn validate_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!(
            "{ERR_NOT_A_PACK}: {} (no such file)",
            path.display()
        ));
    }
    if !path.is_file() {
        return Err(anyhow!(
            "{ERR_NOT_A_PACK}: {} (not a regular file)",
            path.display()
        ));
    }
    let ext_ok = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("gtpack"))
        .unwrap_or(false);
    if !ext_ok {
        return Err(anyhow!(
            "{ERR_NOT_A_PACK}: {} (expected .gtpack extension)",
            path.display()
        ));
    }
    Ok(())
}

/// Recursively sort JSON object keys so the machine-readable output is
/// deterministic across platforms and serde_json versions.
fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_rejects_missing() {
        let err = validate_path(Path::new("/definitely/not/here.gtpack")).unwrap_err();
        assert!(err.to_string().starts_with(ERR_NOT_A_PACK));
    }

    #[test]
    fn validate_path_rejects_wrong_extension() {
        // Cargo.toml exists inside the crate, so the `exists()` + `is_file()`
        // checks pass and we exercise the extension branch.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let err = validate_path(&manifest).unwrap_err();
        let msg = err.to_string();
        assert!(msg.starts_with(ERR_NOT_A_PACK));
        assert!(msg.contains("expected .gtpack extension"));
    }

    #[test]
    fn sort_json_orders_keys_recursively() {
        let v: Value = serde_json::json!({
            "b": 1,
            "a": { "z": 2, "y": 3 },
            "c": [{ "n": 1, "m": 2 }],
        });
        let sorted = sort_json(v);
        let rendered = serde_json::to_string(&sorted).unwrap();
        // All sibling keys appear in ascending order after sorting.
        let a_idx = rendered.find("\"a\"").unwrap();
        let b_idx = rendered.find("\"b\"").unwrap();
        let c_idx = rendered.find("\"c\"").unwrap();
        assert!(a_idx < b_idx && b_idx < c_idx);
        let y_idx = rendered.find("\"y\"").unwrap();
        let z_idx = rendered.find("\"z\"").unwrap();
        assert!(y_idx < z_idx);
        let m_idx = rendered.find("\"m\"").unwrap();
        let n_idx = rendered.find("\"n\"").unwrap();
        assert!(m_idx < n_idx);
    }
}
