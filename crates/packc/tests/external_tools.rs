use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use packc::external_tools;
use tempfile::TempDir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
fn write_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, "#!/usr/bin/env bash\nexit 0\n").expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

#[test]
fn resolve_prefers_env_override_over_path() {
    let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let temp = TempDir::new().expect("tempdir");
    let override_bin = temp.path().join("flow-override");
    let path_bin = temp.path().join("greentic-flow");
    write_executable(&override_bin);
    write_executable(&path_bin);

    let old_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", temp.path());
        std::env::set_var("GREENTIC_FLOW_BIN", &override_bin);
    }

    let resolved = external_tools::resolve("greentic-flow").expect("resolved binary");
    assert_eq!(resolved, override_bin);

    unsafe {
        std::env::remove_var("GREENTIC_FLOW_BIN");
        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }
}

#[test]
fn resolve_uses_path_without_monorepo_sibling_fallback() {
    let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let temp = TempDir::new().expect("tempdir");
    let path_bin = temp.path().join("greentic-flow");
    write_executable(&path_bin);

    let old_path = std::env::var_os("PATH");
    unsafe {
        std::env::remove_var("GREENTIC_FLOW_BIN");
        std::env::remove_var("GREENTIC_FLOW_DEV_BIN");
        std::env::set_var("PATH", temp.path());
    }

    let resolved = external_tools::resolve("greentic-flow").expect("resolved binary");
    assert_eq!(resolved, path_bin);

    unsafe {
        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }
}
