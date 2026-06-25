//! Integration tests for `store://` acquisition in the `ext://<id>#component`
//! resolver (WS-D Phase 3a).
//!
//! Covers:
//! - `extract_and_verify_bytes` (the in-memory split of the file:// path).
//! - `store://` ref parsing (valid / malformed / missing version).
//! - store artifact URL building.
//! - end-to-end store acquisition against a tiny in-process HTTP server
//!   (download → extract → digest-verify), plus the offline cache-miss error.

#![forbid(unsafe_code)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::Path;
use std::thread;

use packc::cli::ext_resolver::{
    GtxpackComponentEntry, GtxpackComponentSidecar, StoreRef, download_store_artifact,
    extract_and_verify_bytes, parse_store_ref, resolve_ext_component_with_dist, store_artifact_url,
};
use packc::extension_refs::{
    ExtensionDependency, ExtensionDependencySource, PackExtensionsFile, write_extensions_file,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::ZipWriter;
use zip::write::{ExtendedFileOptions, FileOptions};

/// 8-byte WebAssembly magic header — enough to be a recognisable stub.
const STUB_WASM: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

/// Build minimal extension `.gtxpack` ZIP bytes in memory containing the wasm
/// asset and a `component.json` sidecar. Returns (zip_bytes, wasm_digest).
fn build_gtxpack_bytes(
    extension_id: &str,
    wasm_asset_path: &str,
    wasm_bytes: &[u8],
) -> (Vec<u8>, String) {
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(wasm_bytes)));
    let sidecar = GtxpackComponentSidecar {
        component: GtxpackComponentEntry {
            id: extension_id.to_string(),
            asset: wasm_asset_path.to_string(),
            digest: digest.clone(),
        },
    };
    let component_json = serde_json::to_vec_pretty(&sidecar).expect("serialize component.json");

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let options = FileOptions::<ExtendedFileOptions>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("component.json", options.clone())
            .expect("start component.json");
        zip.write_all(&component_json)
            .expect("write component.json");
        zip.start_file(wasm_asset_path, options.clone())
            .expect("start wasm asset");
        zip.write_all(wasm_bytes).expect("write wasm asset");
        zip.finish().expect("finish zip");
    }
    (buf.into_inner(), digest)
}

/// Build the same fixture but WITHOUT a `component.json` sidecar.
fn build_gtxpack_bytes_no_sidecar(wasm_asset_path: &str, wasm_bytes: &[u8]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let options = FileOptions::<ExtendedFileOptions>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(wasm_asset_path, options.clone())
            .expect("start wasm asset");
        zip.write_all(wasm_bytes).expect("write wasm asset");
        zip.finish().expect("finish zip");
    }
    buf.into_inner()
}

// ─── extract_and_verify_bytes ────────────────────────────────────────────────

#[test]
fn extract_and_verify_bytes_happy() {
    let (zip_bytes, expected_digest) =
        build_gtxpack_bytes("greentic.test-ext", "component.wasm", STUB_WASM);
    let (bytes, digest) =
        extract_and_verify_bytes("greentic.test-ext", &zip_bytes).expect("extract");
    assert_eq!(bytes, STUB_WASM);
    assert_eq!(digest, expected_digest);
}

#[test]
fn extract_and_verify_bytes_missing_component_json() {
    let zip_bytes = build_gtxpack_bytes_no_sidecar("component.wasm", STUB_WASM);
    let err = extract_and_verify_bytes("greentic.test-ext", &zip_bytes)
        .expect_err("missing component.json");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("does not embed a runtime component") || msg.contains("component.json"),
        "got: {msg}"
    );
}

#[test]
fn extract_and_verify_bytes_missing_asset() {
    // Sidecar points at an asset path that is not in the ZIP.
    let sidecar = GtxpackComponentSidecar {
        component: GtxpackComponentEntry {
            id: "greentic.test-ext".to_string(),
            asset: "does/not/exist.wasm".to_string(),
            digest: format!("sha256:{}", hex::encode(Sha256::digest(STUB_WASM))),
        },
    };
    let component_json = serde_json::to_vec_pretty(&sidecar).expect("serialize");
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let options = FileOptions::<ExtendedFileOptions>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("component.json", options.clone())
            .expect("start");
        zip.write_all(&component_json).expect("write");
        zip.start_file("component.wasm", options.clone())
            .expect("start");
        zip.write_all(STUB_WASM).expect("write");
        zip.finish().expect("finish");
    }
    let zip_bytes = buf.into_inner();
    let err = extract_and_verify_bytes("greentic.test-ext", &zip_bytes).expect_err("missing asset");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("not found") || msg.contains("asset"),
        "got: {msg}"
    );
}

#[test]
fn extract_and_verify_bytes_digest_mismatch() {
    let sidecar = GtxpackComponentSidecar {
        component: GtxpackComponentEntry {
            id: "greentic.test-ext".to_string(),
            asset: "component.wasm".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        },
    };
    let component_json = serde_json::to_vec_pretty(&sidecar).expect("serialize");
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let options = FileOptions::<ExtendedFileOptions>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("component.json", options.clone())
            .expect("start");
        zip.write_all(&component_json).expect("write");
        zip.start_file("component.wasm", options.clone())
            .expect("start");
        zip.write_all(STUB_WASM).expect("write");
        zip.finish().expect("finish");
    }
    let zip_bytes = buf.into_inner();
    let err =
        extract_and_verify_bytes("greentic.test-ext", &zip_bytes).expect_err("digest mismatch");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("digest") || msg.contains("mismatch"),
        "got: {msg}"
    );
}

// ─── parse_store_ref ─────────────────────────────────────────────────────────

#[test]
fn parse_store_ref_happy() {
    let parsed = parse_store_ref("store://greentic.http@1.2.0").expect("valid");
    assert_eq!(parsed.name, "greentic.http");
    assert_eq!(parsed.version, "1.2.0");
}

#[test]
fn parse_store_ref_missing_version() {
    let err = parse_store_ref("store://greentic.http").expect_err("no version");
    let msg = err.to_string();
    assert!(msg.contains("version") || msg.contains("@"), "got: {msg}");
}

#[test]
fn parse_store_ref_empty_version() {
    let err = parse_store_ref("store://greentic.http@").expect_err("empty version");
    assert!(err.to_string().contains("version"), "got: {err}");
}

#[test]
fn parse_store_ref_empty_name() {
    let err = parse_store_ref("store://@1.2.0").expect_err("empty name");
    assert!(err.to_string().contains("name"), "got: {err}");
}

#[test]
fn parse_store_ref_wrong_scheme() {
    let err = parse_store_ref("oci://foo@1.0.0").expect_err("wrong scheme");
    assert!(err.to_string().contains("store://"), "got: {err}");
}

// ─── store_artifact_url ──────────────────────────────────────────────────────

#[test]
fn store_artifact_url_builds_expected_path() {
    let url = store_artifact_url("https://store.example", "greentic.http", "1.2.0");
    assert_eq!(
        url,
        "https://store.example/api/v1/extensions/greentic.http/1.2.0/artifact"
    );
}

#[test]
fn store_artifact_url_trims_trailing_slash() {
    let url = store_artifact_url("https://store.example/", "greentic.http", "1.2.0");
    assert_eq!(
        url,
        "https://store.example/api/v1/extensions/greentic.http/1.2.0/artifact"
    );
}

// ─── store acquisition (in-process mock server) ──────────────────────────────

/// A tiny single-connection HTTP/1.1 server that serves the given body with an
/// `x-artifact-sha256` header at any path, then exits. Returns the base URL.
fn spawn_artifact_server(body: Vec<u8>, sha_hex: String) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let base = format!("http://{addr}");
    let handle = thread::spawn(move || {
        // Serve a single connection then return.
        if let Ok((mut stream, _)) = listener.accept() {
            // Drain the request headers (read until CRLFCRLF or EOF).
            let mut req = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                match stream.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        req.push(byte[0]);
                        if req.ends_with(b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nx-artifact-sha256: {}\r\nConnection: close\r\n\r\n",
                body.len(),
                sha_hex
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });
    (base, handle)
}

fn write_store_pack_extensions(pack_dir: &Path, extension_id: &str, store_ref: &str) {
    let file = PackExtensionsFile::new(vec![ExtensionDependency {
        id: extension_id.to_string(),
        role: "capability".to_string(),
        source: ExtensionDependencySource {
            kind: "store".to_string(),
            reference: store_ref.to_string(),
            allow_tags: true,
        },
    }]);
    write_extensions_file(&pack_dir.join("pack.extensions.json"), &file)
        .expect("write pack.extensions.json");
}

// NOTE on env: `GREENTIC_STORE_URL` resolution lives in the public entry
// `resolve_ext_component_with_dist`. Mutating process env is forbidden here
// (`#![forbid(unsafe_code)]` rejects the now-`unsafe` `std::env::set_var`), and
// would race across parallel test threads anyway. We therefore test the
// network seam `download_store_artifact(base, ...)` directly (it takes an
// explicit base URL), and document the env-driven path with an `#[ignore]`d
// live test below.

#[test]
fn store_acquisition_downloads_extracts_and_verifies() {
    let tmp = TempDir::new().expect("tempdir");
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).expect("cache dir");

    let (zip_bytes, expected_digest) =
        build_gtxpack_bytes("greentic.http", "component.wasm", STUB_WASM);
    let archive_sha = hex::encode(Sha256::digest(&zip_bytes));

    let (base, server) = spawn_artifact_server(zip_bytes.clone(), archive_sha);

    let store_ref = StoreRef {
        name: "greentic.http".to_string(),
        version: "1.2.0".to_string(),
    };
    let downloaded =
        download_store_artifact(&base, &store_ref, &cache_dir).expect("download store artifact");
    server.join().expect("server thread");

    assert_eq!(
        downloaded, zip_bytes,
        "downloaded bytes must match served body"
    );

    // Extract + digest-verify the embedded component from the downloaded bytes.
    let (bytes, digest) =
        extract_and_verify_bytes("greentic.http", &downloaded).expect("extract embedded");
    assert_eq!(bytes, STUB_WASM);
    assert_eq!(digest, expected_digest);

    // The artifact is cached under cache_dir/ext-store keyed by archive sha and ref.
    let ext_store = cache_dir.join("ext-store");
    let cached: Vec<_> = std::fs::read_dir(&ext_store)
        .expect("read ext-store")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !cached.is_empty(),
        "expected the downloaded artifact to be cached under {}",
        ext_store.display()
    );
}

#[test]
fn store_acquisition_rejects_archive_sha_mismatch() {
    let tmp = TempDir::new().expect("tempdir");
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).expect("cache dir");

    let (zip_bytes, _) = build_gtxpack_bytes("greentic.http", "component.wasm", STUB_WASM);
    // Serve a deliberately wrong x-artifact-sha256.
    let (base, server) = spawn_artifact_server(zip_bytes.clone(), "f".repeat(64));

    let store_ref = StoreRef {
        name: "greentic.http".to_string(),
        version: "1.2.0".to_string(),
    };
    let err = download_store_artifact(&base, &store_ref, &cache_dir)
        .expect_err("should reject archive sha mismatch");
    server.join().expect("server thread");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("integrity") || msg.contains("x-artifact-sha256"),
        "error should mention archive integrity; got: {msg}"
    );
}

#[test]
fn store_acquisition_offline_empty_cache_errors() {
    let tmp = TempDir::new().expect("tempdir");
    let pack_dir = tmp.path().join("pack");
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&pack_dir).expect("pack dir");
    std::fs::create_dir_all(&cache_dir).expect("cache dir");

    write_store_pack_extensions(&pack_dir, "greentic.http", "store://greentic.http@1.2.0");

    // Offline never reads GREENTIC_STORE_URL, so no env handling is needed.
    let err = resolve_ext_component_with_dist(
        &pack_dir,
        "ext://greentic.http#component",
        &cache_dir,
        true,
        None,
    )
    .expect_err("offline empty cache should error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("offline") || msg.contains("cache"),
        "error should mention offline/cache; got: {msg}"
    );
}

#[test]
fn store_acquisition_offline_cache_hit_succeeds() {
    let tmp = TempDir::new().expect("tempdir");
    let pack_dir = tmp.path().join("pack");
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&pack_dir).expect("pack dir");
    std::fs::create_dir_all(&cache_dir).expect("cache dir");

    let (zip_bytes, expected_digest) =
        build_gtxpack_bytes("greentic.http", "component.wasm", STUB_WASM);

    // Populate the cache the way an online download would (ref-keyed file).
    let ext_store = cache_dir.join("ext-store");
    std::fs::create_dir_all(&ext_store).expect("ext-store");
    std::fs::write(ext_store.join("greentic.http_1.2.0.gtxpack"), &zip_bytes).expect("seed cache");

    write_store_pack_extensions(&pack_dir, "greentic.http", "store://greentic.http@1.2.0");

    let (bytes, digest) = resolve_ext_component_with_dist(
        &pack_dir,
        "ext://greentic.http#component",
        &cache_dir,
        true,
        None,
    )
    .expect("offline cache hit should succeed");
    assert_eq!(bytes, STUB_WASM);
    assert_eq!(digest, expected_digest);
}

/// Documents the env-driven public path end to end. Ignored because it relies
/// on the process env (`GREENTIC_STORE_URL`), which cannot be mutated under
/// `#![forbid(unsafe_code)]` and would race other tests. Run explicitly with:
/// `GREENTIC_STORE_URL=http://127.0.0.1:PORT cargo test ... -- --ignored`.
#[test]
#[ignore = "requires GREENTIC_STORE_URL pointing at a live store artifact endpoint"]
fn store_env_driven_resolve_live() {
    let tmp = TempDir::new().expect("tempdir");
    let pack_dir = tmp.path().join("pack");
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&pack_dir).expect("pack dir");
    std::fs::create_dir_all(&cache_dir).expect("cache dir");
    write_store_pack_extensions(&pack_dir, "greentic.http", "store://greentic.http@1.2.0");

    let (bytes, digest) = resolve_ext_component_with_dist(
        &pack_dir,
        "ext://greentic.http#component",
        &cache_dir,
        false,
        None,
    )
    .expect("resolve from live store");
    assert!(!bytes.is_empty());
    assert!(digest.starts_with("sha256:"));
}
