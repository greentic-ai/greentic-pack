use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

/// Normalise `candidate` so it stays under `root` after resolving `..`.
/// Rejects absolute inputs and any traversal that escapes `root`.
pub fn normalize_under_root(root: &Path, candidate: &Path) -> Result<PathBuf> {
    if candidate.is_absolute() {
        anyhow::bail!("absolute paths are not allowed: {}", candidate.display());
    }

    let canon_root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut normalized = canon_root.clone();
    let base_depth = canon_root.components().count();

    for comp in candidate.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if normalized.components().count() <= base_depth {
                    anyhow::bail!(
                        "path escapes root ({}): {}",
                        canon_root.display(),
                        candidate.display()
                    );
                }
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir => {
                anyhow::bail!("invalid path component in {}", candidate.display());
            }
        }
    }

    if !normalized.starts_with(&canon_root) {
        anyhow::bail!(
            "path escapes root ({}): {}",
            canon_root.display(),
            candidate.display()
        );
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_under_root;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn normalizes_nested_path_under_root() {
        let root = tempdir().expect("tempdir");
        let canon_root = root.path().canonicalize().expect("canonical root");
        let normalized =
            normalize_under_root(root.path(), Path::new("./assets/../assets/file.txt"))
                .expect("path under root should succeed");

        assert_eq!(normalized, canon_root.join("assets/file.txt"));
    }

    #[test]
    fn rejects_absolute_candidate_paths() {
        let root = tempdir().expect("tempdir");
        let err = normalize_under_root(root.path(), Path::new("/tmp/outside"))
            .expect_err("absolute paths should fail");

        assert!(err.to_string().contains("absolute paths are not allowed"));
    }

    #[test]
    fn rejects_parent_traversal_above_root() {
        let root = tempdir().expect("tempdir");
        let err = normalize_under_root(root.path(), Path::new("../../escape.txt"))
            .expect_err("escaping path should fail");

        assert!(err.to_string().contains("path escapes root"));
    }
}
