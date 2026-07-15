//! Integration tests for `cli::publish_agent::run`.
//!
//! Exercises the end-to-end `run` path: file read, metadata build, HTTP
//! publish, and the error cases (missing file, server failure).

#![forbid(unsafe_code)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use packc::cli::publish_agent::PublishAgentArgs;

/// Drain an HTTP/1.1 request (headers + body) from a TCP stream.
fn drain_request(stream: &mut std::net::TcpStream) {
    let mut headers = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return,
            Ok(_) => {
                headers.push(byte[0]);
                if headers.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    let header_str = String::from_utf8_lossy(&headers);
    let content_length: usize = header_str
        .lines()
        .find_map(|line| {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                lower.split(':').nth(1)?.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let mut remaining = content_length;
    let mut buf = [0u8; 4096];
    while remaining > 0 {
        let n = std::cmp::min(buf.len(), remaining);
        match stream.read(&mut buf[..n]) {
            Ok(0) => break,
            Ok(read) => remaining -= read,
            Err(_) => break,
        }
    }
}

/// Spawn a single-connection HTTP/1.1 server returning `status` + `body`.
fn spawn_mock(status: u16, reason: &str, body: &str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let base = format!("http://{addr}");
    let resp_line = format!("HTTP/1.1 {status} {reason}");
    let resp_body = body.to_string();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            drain_request(&mut stream);
            let header = format!(
                "{resp_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                resp_body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(resp_body.as_bytes());
            let _ = stream.flush();
        }
    });
    (base, handle)
}

// ─── cli::publish_agent::run ────────────────────────────────────────────────

#[tokio::test]
async fn run_happy_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pack_path = tmp.path().join("test.gtpack");
    std::fs::write(&pack_path, b"fake-pack-bytes").expect("write pack");

    let (base, server) = spawn_mock(200, "OK", r#"{"status":"created"}"#);

    let args = PublishAgentArgs {
        pack: pack_path,
        store_url: Some(base),
        token: Some("test-token".to_string()),
        id: "test.bot".to_string(),
        name: "Test Bot".to_string(),
        version: "0.1.0".to_string(),
        summary: "A test bot".to_string(),
    };
    let result = packc::cli::publish_agent::run(args).await;
    server.join().expect("server thread");
    result.expect("happy path should succeed");
}

#[tokio::test]
async fn run_pack_file_not_found() {
    let args = PublishAgentArgs {
        pack: PathBuf::from("/nonexistent/path/test.gtpack"),
        store_url: Some("http://127.0.0.1:1".to_string()),
        token: Some("tok".to_string()),
        id: "test.bot".to_string(),
        name: "Bot".to_string(),
        version: "0.1.0".to_string(),
        summary: String::new(),
    };
    let err = packc::cli::publish_agent::run(args)
        .await
        .expect_err("missing file should fail");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("read") || msg.contains("No such file"),
        "expected file-read error, got: {msg}"
    );
}

#[tokio::test]
async fn run_server_409_propagates_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pack_path = tmp.path().join("test.gtpack");
    std::fs::write(&pack_path, b"dup-pack").expect("write pack");

    let (base, server) = spawn_mock(409, "Conflict", "already exists");

    let args = PublishAgentArgs {
        pack: pack_path,
        store_url: Some(base),
        token: Some("tok".to_string()),
        id: "test.bot".to_string(),
        name: "Bot".to_string(),
        version: "0.1.0".to_string(),
        summary: String::new(),
    };
    let err = packc::cli::publish_agent::run(args)
        .await
        .expect_err("409 should propagate");
    server.join().expect("server thread");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("already published"),
        "expected 'already published', got: {msg}"
    );
}
