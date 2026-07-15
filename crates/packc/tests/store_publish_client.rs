//! Integration tests for `store_client::publish_agentic_worker`.
//!
//! Uses the same in-process `TcpListener` mock-server pattern as
//! `ext_component_store_resolver.rs` to exercise the 200 / 409 / non-2xx
//! code paths without any external service.

#![forbid(unsafe_code)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::thread;

/// Drain an HTTP/1.1 request (headers + body) from a TCP stream so the
/// client (reqwest) does not see a broken pipe before reading the response.
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

/// Spawn a single-connection HTTP/1.1 server that drains the incoming
/// request and responds with the given status code and body, then exits.
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

// ─── publish_agentic_worker ─────────────────────────────────────────────────

#[tokio::test]
async fn publish_200_returns_parsed_json() {
    let resp_body = r#"{"id":"test","status":"ok"}"#;
    let (base, server) = spawn_mock(200, "OK", resp_body);

    let meta =
        packc::store_client::publish_metadata(b"fake-pack", "test.bot", "Bot", "0.1.0", "summary")
            .unwrap();
    let result =
        packc::store_client::publish_agentic_worker(&base, "tok", &meta, b"fake-pack".to_vec())
            .await;

    server.join().expect("server thread");
    let val = result.expect("200 should succeed");
    assert_eq!(val["id"], "test");
    assert_eq!(val["status"], "ok");
}

#[tokio::test]
async fn publish_409_returns_already_published() {
    let (base, server) = spawn_mock(409, "Conflict", "duplicate version");

    let meta =
        packc::store_client::publish_metadata(b"fake-pack", "test.bot", "Bot", "0.1.0", "summary")
            .unwrap();
    let err =
        packc::store_client::publish_agentic_worker(&base, "tok", &meta, b"fake-pack".to_vec())
            .await
            .expect_err("409 should fail");

    server.join().expect("server thread");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("already published"),
        "expected 'already published', got: {msg}"
    );
    assert!(msg.contains("409"), "expected '409' in message, got: {msg}");
}

#[tokio::test]
async fn publish_500_returns_store_error() {
    let (base, server) = spawn_mock(500, "Internal Server Error", "kaboom");

    let meta =
        packc::store_client::publish_metadata(b"fake-pack", "test.bot", "Bot", "0.1.0", "summary")
            .unwrap();
    let err =
        packc::store_client::publish_agentic_worker(&base, "tok", &meta, b"fake-pack".to_vec())
            .await
            .expect_err("500 should fail");

    server.join().expect("server thread");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("store returned 500"),
        "expected 'store returned 500', got: {msg}"
    );
}

#[tokio::test]
async fn publish_200_non_json_falls_back_to_raw() {
    let (base, server) = spawn_mock(200, "OK", "plain text");

    let meta =
        packc::store_client::publish_metadata(b"fake", "test.bot", "Bot", "0.1.0", "").unwrap();
    let val = packc::store_client::publish_agentic_worker(&base, "tok", &meta, b"fake".to_vec())
        .await
        .expect("200 should succeed even with non-JSON body");

    server.join().expect("server thread");
    assert_eq!(val["raw"], "plain text");
}
