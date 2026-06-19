#![forbid(unsafe_code)]
//! Directory scanning, bundle generation, and card ID resolution for i18n extraction.
//!
//! This module handles the directory-level operations: scanning for Adaptive Cards,
//! converting extracted strings to JSON bundles, and writing output files.
//!
//! Ported from `greentic-cards2pack/src/i18n_extract/mod.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;
use walkdir::WalkDir;

use super::extract::{ExtractedString, extract_from_value};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for i18n extraction.
#[derive(Debug, Clone)]
pub struct ExtractConfig {
    /// Directory containing Adaptive Card JSON files.
    pub cards_dir: PathBuf,
    /// Output JSON file path.
    pub output: PathBuf,
    /// Key prefix (default: "card").
    pub prefix: String,
    /// Skip strings that already contain $t() patterns.
    pub skip_i18n_patterns: bool,
}

// ---------------------------------------------------------------------------
// Directory-level entry points
// ---------------------------------------------------------------------------

/// Extract translatable strings from all cards in a directory.
pub fn extract_from_directory(config: &ExtractConfig) -> Result<Vec<ExtractedString>> {
    let mut all_strings = Vec::new();

    for entry in WalkDir::new(&config.cards_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let extension = path.extension().and_then(|ext| ext.to_str());
        if extension.is_none_or(|ext| !ext.eq_ignore_ascii_case("json")) {
            continue;
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let value: Value = serde_json::from_str(&contents)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;

        if !is_adaptive_card(&value) {
            continue;
        }

        let card_id = determine_card_id(path, &value, &config.cards_dir)?;
        let full_prefix = if config.prefix.is_empty() {
            card_id.clone()
        } else {
            format!("{}.{}", config.prefix, card_id)
        };

        let strings =
            extract_from_value(&value, &full_prefix, "", path, config.skip_i18n_patterns);
        all_strings.extend(strings);
    }

    Ok(all_strings)
}

/// Convert extracted strings to a JSON bundle format.
pub fn to_json_bundle(strings: &[ExtractedString]) -> Value {
    let mut map = serde_json::Map::new();
    for s in strings {
        map.insert(s.key.clone(), Value::String(s.value.clone()));
    }
    Value::Object(map)
}

/// Write extracted strings to a JSON file.
pub fn write_bundle(strings: &[ExtractedString], output: &Path) -> Result<()> {
    let json = to_json_bundle(strings);
    let contents = serde_json::to_string_pretty(&json)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output, contents)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Card-level helpers (private)
// ---------------------------------------------------------------------------

/// Check if a JSON value represents an Adaptive Card.
fn is_adaptive_card(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    let card_type = obj.get("type").and_then(|v| v.as_str());
    card_type == Some("AdaptiveCard") || obj.contains_key("body") || obj.contains_key("actions")
}

/// Determine the card ID from the file path or card metadata.
fn determine_card_id(path: &Path, value: &Value, cards_dir: &Path) -> Result<String> {
    // Try greentic.cardId first
    if let Some(card_id) = value
        .get("greentic")
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("cardId"))
        .and_then(|v| v.as_str())
    {
        return Ok(sanitize_key_part(card_id));
    }

    // Fall back to file stem
    let rel_path = path.strip_prefix(cards_dir).unwrap_or(path);
    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine card ID for {}", path.display()))?;

    Ok(sanitize_key_part(stem))
}

/// Sanitize a string for use as a key part.
fn sanitize_key_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// Tests — from mod.rs
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    #[test]
    fn test_sanitize_key_part() {
        assert_eq!(sanitize_key_part("my-card"), "my_card");
        assert_eq!(sanitize_key_part("My Card"), "my_card");
        assert_eq!(sanitize_key_part("card_123"), "card_123");
        assert_eq!(sanitize_key_part("Card.Name"), "card_name");
    }

    #[test]
    fn test_determine_card_id_from_metadata() {
        let card = json!({
            "type": "AdaptiveCard",
            "greentic": { "cardId": "my-custom-id" },
            "body": []
        });
        let id =
            determine_card_id(Path::new("some-filename.json"), &card, Path::new("")).unwrap();
        assert_eq!(id, "my_custom_id");
    }

    #[test]
    fn test_determine_card_id_fallback_to_filename() {
        let card = json!({ "type": "AdaptiveCard", "body": [] });
        let id =
            determine_card_id(Path::new("cards/my-card.json"), &card, Path::new("cards")).unwrap();
        assert_eq!(id, "my_card");
    }

    #[test]
    fn test_to_json_bundle() {
        let strings = vec![
            ExtractedString {
                key: "card.title".to_string(),
                value: "Hello".to_string(),
                source_file: PathBuf::from("test.json"),
                json_path: "body[0].text".to_string(),
            },
            ExtractedString {
                key: "card.button".to_string(),
                value: "Click me".to_string(),
                source_file: PathBuf::from("test.json"),
                json_path: "actions[0].title".to_string(),
            },
        ];

        let bundle = to_json_bundle(&strings);
        assert_eq!(bundle["card.title"], "Hello");
        assert_eq!(bundle["card.button"], "Click me");
    }

    #[test]
    fn test_write_bundle_creates_parent_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let output = tmp.path().join("deep/nested/dir/en.json");
        let strings = vec![ExtractedString {
            key: "k".to_string(),
            value: "v".to_string(),
            source_file: PathBuf::from("test.json"),
            json_path: "text".to_string(),
        }];
        write_bundle(&strings, &output).unwrap();
        assert!(output.is_file());
    }
}
