#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;

pub fn resolve(binary: &str) -> Option<PathBuf> {
    if let Some(override_bin) = override_binary(binary)
        && override_bin.exists()
    {
        return Some(override_bin);
    }
    if let Some(path_bin) = resolve_from_path(binary) {
        return Some(path_bin);
    }
    if let Some(current_exe) = std::env::current_exe().ok()
        && let Some(exe_dir) = current_exe.parent()
    {
        let local_bin = exe_dir.join(binary);
        if local_bin.exists() {
            return Some(local_bin);
        }
    }
    None
}

fn override_binary(binary: &str) -> Option<PathBuf> {
    let keys: &[&str] = match binary {
        "greentic-flow" => &["GREENTIC_FLOW_BIN", "GREENTIC_FLOW_DEV_BIN"],
        "greentic-component" => &["GREENTIC_COMPONENT_BIN", "GREENTIC_COMPONENT_DEV_BIN"],
        "greentic-i18n-translator" => &[
            "GREENTIC_I18N_TRANSLATOR_BIN",
            "GREENTIC_I18N_TRANSLATOR_DEV_BIN",
        ],
        _ => return None,
    };
    keys.iter()
        .find_map(|key| env::var_os(key).map(PathBuf::from))
}

fn resolve_from_path(binary: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
