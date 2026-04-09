use std::fs;
use std::path::{Path, PathBuf};

use greentic_types::pack_manifest::ExtensionInline;
use std::process::Command;
use tempfile::tempdir;
use walkdir::WalkDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_dir(name: &str) -> PathBuf {
    workspace_root()
        .join("crates")
        .join("packc")
        .join("tests")
        .join("fixtures")
        .join("packs")
        .join(name)
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let src = fixture_dir(name);
    let temp = tempdir().expect("tempdir");
    let dest = temp.path().join(name);
    fs::create_dir_all(&dest).expect("create destination");
    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        let relative = entry.path().strip_prefix(&src).expect("relative path");
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).expect("create dir");
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
    (temp, dest)
}

fn write_static_routes_extension(pack_dir: &Path) {
    let assets_dir = pack_dir.join("assets").join("webchat-gui");
    fs::create_dir_all(&assets_dir).expect("create assets dir");
    fs::write(assets_dir.join("index.html"), "<html/>").expect("write index");

    let pack_yaml_path = pack_dir.join("pack.yaml");
    let mut pack_yaml = fs::read_to_string(&pack_yaml_path).expect("read pack.yaml");
    pack_yaml.push_str(
        r#"
extensions:
  greentic.static-routes.v1:
    kind: greentic.static-routes.v1
    version: 0.4.37
    inline:
      version: 1
      routes:
        - id: webchat-gui
          public_path: /v1/web/webchat/{tenant}
          source_root: assets/webchat-gui
          scope:
            tenant: true
            team: false
          index_file: index.html
          spa_fallback: index.html
"#,
    );
    fs::write(pack_yaml_path, pack_yaml).expect("write pack.yaml");
}

#[test]
fn build_accepts_unknown_static_routes_inline_version_payload() {
    let (_temp, pack_dir) = copy_fixture("valid-minimal");
    write_static_routes_extension(&pack_dir);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("greentic-pack"))
        .current_dir(workspace_root())
        .args([
            "build",
            "--in",
            pack_dir.to_str().unwrap(),
            "--no-update",
            "--dry-run",
            "--log",
            "warn",
        ])
        .output()
        .expect("run build");

    assert!(
        output.status.success(),
        "build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_keeps_outer_extension_version_and_inner_payload_version_separate() {
    let (_temp, pack_dir) = copy_fixture("valid-minimal");
    write_static_routes_extension(&pack_dir);

    let cfg = packc::config::load_pack_config(&pack_dir).expect("load pack config");
    let ext = cfg
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("greentic.static-routes.v1"))
        .expect("static routes extension present");

    assert_eq!(ext.version, "0.4.37");
    let inline = match ext.inline.as_ref() {
        Some(ExtensionInline::Other(value)) => value,
        other => panic!("unexpected inline payload: {other:?}"),
    };
    assert_eq!(inline.get("version"), Some(&serde_json::json!(1)));
}
