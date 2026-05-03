use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use greentic_distributor_client::{
    DistClient, DistOptions, ReleaseChannel, ReleaseIndex, ReleaseResolutionContext,
};
use regex::Regex;
use walkdir::WalkDir;

const WORKSPACE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn workspace_root() -> PathBuf {
    Path::new(WORKSPACE_ROOT).join("..").join("..")
}

fn stable_oci_refs_used_in_workspace() -> BTreeSet<String> {
    let root = workspace_root();
    let pattern = Regex::new(r#"oci://[^\s"'<>),}]+:stable\b"#).expect("valid regex");
    let mut refs = BTreeSet::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | "target" | ".cache" | "dist" | "bundle"
            )
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(contents) = fs::read_to_string(entry.path()) else {
            continue;
        };
        refs.extend(pattern.find_iter(&contents).map(|match_| {
            match_
                .as_str()
                .trim_end_matches(['.', ';', ':'])
                .to_string()
        }));
    }

    refs
}

fn stable_release_indexes(cache_dir: &Path) -> Vec<(ReleaseResolutionContext, PathBuf)> {
    let release_index_dir = cache_dir.join("release-index").join("v1").join("stable");
    let Ok(entries) = fs::read_dir(&release_index_dir) else {
        return Vec::new();
    };

    let mut indexes = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let index = serde_json::from_slice::<ReleaseIndex>(&bytes).ok()?;
            if index.channel != ReleaseChannel::Stable {
                return None;
            }
            Some((
                ReleaseResolutionContext {
                    release: index.release,
                    channel: ReleaseChannel::Stable,
                },
                path,
            ))
        })
        .collect::<Vec<_>>();

    indexes
        .sort_by(|(left, _), (right, _)| compare_release_versions(&right.release, &left.release));
    indexes
}

fn compare_release_versions(left: &str, right: &str) -> Ordering {
    let left_segments = numeric_version_segments(left);
    let right_segments = numeric_version_segments(right);
    left_segments
        .cmp(&right_segments)
        .then_with(|| left.cmp(right))
}

fn numeric_version_segments(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect()
}

fn stable_oci_cache_test_is_required() -> bool {
    std::env::var("GREENTIC_REQUIRE_STABLE_OCI_CACHE").is_ok_and(|value| value == "1")
}

#[test]
fn all_stable_oci_refs_used_in_workspace_are_available_locally() {
    let refs = stable_oci_refs_used_in_workspace();
    assert!(!refs.is_empty(), "expected at least one stable OCI ref");

    let options = DistOptions {
        allow_tags: true,
        offline: true,
        allow_insecure_local_http: false,
        ..DistOptions::default()
    };
    let cache_dir = options.cache_dir.clone();
    let dist = DistClient::new(options);

    let release_indexes = stable_release_indexes(&cache_dir);
    if release_indexes.is_empty() {
        let release_index_dir = cache_dir.join("release-index").join("v1").join("stable");
        if stable_oci_cache_test_is_required() {
            panic!(
                "no stable release index is present in the local distribution cache at {}",
                release_index_dir.display()
            );
        }
        eprintln!(
            "skipping stable OCI cache validation: no stable release index is present at {}",
            release_index_dir.display()
        );
        return;
    }

    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    let mut failures = Vec::new();

    for reference in refs {
        let mut resolved = None;
        let mut resolve_errors = Vec::new();
        for (context, path) in &release_indexes {
            match runtime.block_on(dist.resolve_oci_ref_with_context(&reference, context)) {
                Ok(descriptor) => {
                    resolved = Some(descriptor);
                    break;
                }
                Err(err) => {
                    resolve_errors.push(format!("{reference} via {}: {err}", path.display()));
                }
            }
        }

        let Some(descriptor) = resolved else {
            failures.push(format!(
                "{reference} could not be resolved from the local stable release index:\n{}",
                resolve_errors.join("\n")
            ));
            continue;
        };

        if !descriptor.canonical_ref.contains("@sha256:") {
            failures.push(format!(
                "{reference} resolved to non-digest-pinned ref {}",
                descriptor.canonical_ref
            ));
            continue;
        }

        match dist.open_cached(&descriptor.digest) {
            Ok(artifact) => {
                let path = artifact.cache_path.as_ref().unwrap_or(&artifact.local_path);
                match fs::metadata(path) {
                    Ok(metadata) if metadata.len() > 0 => {}
                    Ok(_) => failures.push(format!(
                        "{reference} cached artifact is empty at {}",
                        path.display()
                    )),
                    Err(err) => failures.push(format!(
                        "{reference} cached artifact is missing at {}: {err}",
                        path.display()
                    )),
                }
            }
            Err(err) => failures.push(format!(
                "{reference} resolved to {}, but the artifact could not be opened from the local cache: {err}",
                descriptor.canonical_ref
            )),
        }
    }

    if failures.is_empty() {
        return;
    }

    let message = failures.join("\n");
    if stable_oci_cache_test_is_required() {
        panic!("{message}");
    }

    eprintln!("skipping stable OCI cache validation: local cache is incomplete:\n{message}");
}
