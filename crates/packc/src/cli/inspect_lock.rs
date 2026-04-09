#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use greentic_pack::pack_lock::read_pack_lock;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Args)]
pub struct InspectLockArgs {
    /// Pack root directory containing pack.yaml.
    #[arg(long = "in", value_name = "DIR", default_value = ".")]
    pub input: PathBuf,

    /// Path to pack.lock.cbor (default: pack.lock.cbor under pack root).
    #[arg(long = "lock", value_name = "FILE")]
    pub lock: Option<PathBuf>,
}

pub fn handle(args: InspectLockArgs) -> Result<()> {
    let pack_dir = args
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve pack dir {}", args.input.display()))?;
    let lock_path = resolve_lock_path(&pack_dir, args.lock.as_deref());
    let lock = read_pack_lock(&lock_path)
        .with_context(|| format!("failed to read {}", lock_path.display()))?;

    let json = to_sorted_json(&lock)?;
    println!("{json}");
    Ok(())
}

fn resolve_lock_path(pack_dir: &Path, override_path: Option<&Path>) -> PathBuf {
    match override_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => pack_dir.join(path),
        None => pack_dir.join("pack.lock.cbor"),
    }
}

fn to_sorted_json<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).context("encode lock to json")?;
    let sorted = sort_json(value);
    serde_json::to_string_pretty(&sorted).context("serialize json")
}

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
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn resolve_lock_path_defaults_under_pack_dir() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(
            resolve_lock_path(dir.path(), None),
            dir.path().join("pack.lock.cbor")
        );
    }

    #[test]
    fn resolve_lock_path_joins_relative_override() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(
            resolve_lock_path(dir.path(), Some(Path::new("nested/pack.lock.cbor"))),
            dir.path().join("nested/pack.lock.cbor")
        );
    }

    #[test]
    fn resolve_lock_path_keeps_absolute_override() {
        let path = PathBuf::from("/tmp/pack.lock.cbor");
        assert_eq!(
            resolve_lock_path(Path::new("/repo"), Some(path.as_path())),
            path
        );
    }

    #[test]
    fn to_sorted_json_sorts_nested_object_keys() {
        let json = to_sorted_json(&json!({
            "z": 1,
            "a": { "d": true, "b": false },
            "m": [ { "y": 2, "x": 1 } ]
        }))
        .expect("serialize");

        let a_pos = json.find("\"a\"").expect("a");
        let m_pos = json.find("\"m\"").expect("m");
        let z_pos = json.find("\"z\"").expect("z");
        assert!(
            a_pos < m_pos && m_pos < z_pos,
            "top-level keys should be sorted"
        );

        let b_pos = json.find("\"b\"").expect("b");
        let d_pos = json.find("\"d\"").expect("d");
        assert!(b_pos < d_pos, "nested keys should be sorted");
    }
}
