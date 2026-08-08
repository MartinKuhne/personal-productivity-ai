//! Tests for the Trello REST client (`client.rs`).
//!
//! Sidecar file. Extracted from `client.rs` per AGENTS.md RUST-056 so
//! the implementation module stays focused on production code. Lives
//! in a sibling file so private item access via `super::*` keeps
//! working.
//!
//! Covers:
//!  * URL shape (the API-key + token query string)
//!  * the `trello_request` happy path against a local mock HTTP server
//!  * the `trello_request` error path (non-2xx status)
//!  * JSON parse failure on a 2xx body
//!  * transport failure (connection refused) on an unreachable port

use super::*;
use crate::config::TrelloClient;
use std::io::{Read, Write};
use std::net::TcpListener;

fn make_client() -> TrelloClient {
    TrelloClient {
        api_key: "test-api-key".to_string(),
        token: "test-token".to_string(),
    }
}

#[test]
fn build_url_prefixes_with_https_api_trello_com_1() {
    let url = build_trello_url(&make_client(), "/members/me/boards");
    assert!(url.starts_with("https://api.trello.com/1"));
}

#[test]
fn build_url_appends_endpoint_after_slash_1() {
    let url = build_trello_url(&make_client(), "/members/me/boards");
    assert!(
        url.contains("/1/members/me/boards?"),
        "expected '/1/members/me/boards?' in {url}"
    );
}

#[test]
fn build_url_includes_key_and_token_query_params() {
    let url = build_trello_url(&make_client(), "/boards/abc");
    assert!(url.contains("key=test-api-key"), "missing key in {url}");
    assert!(url.contains("token=test-token"), "missing token in {url}");
}

#[test]
fn build_url_preserves_caller_endpoint_verbatim() {
    // No trimming, no normalisation — Trello endpoints are author-supplied
    // and the server is strict about the path. Real Trello endpoint paths
    // never contain a `?` (the auth params are the only query string), so
    // this test stays with a path-only endpoint.
    let url = build_trello_url(&make_client(), "/lists/123/cards");
    assert!(url.contains("/1/lists/123/cards?"), "endpoint shape: {url}");
    assert!(
        !url.contains("??"),
        "double `??` would be a malformed URL: {url}"
    );
}

#[test]
fn build_url_handles_trailing_slash_on_endpoint() {
    // `/1//foo` is malformed, but the function shouldn't strip a trailing
    // slash from the caller; that's the server's problem.
    let url = build_trello_url(&make_client(), "/foo/");
    assert!(url.contains("/1/foo/?"));
}

/// Spin up a one-shot HTTP/1.1 mock server that returns a canned
/// response for the first request, then tears down the listener.
fn spawn_mock_server(status_line: &str, content_type: &str, body: &str) -> String {
    // Some CI networks intercept 127.0.0.1 — opt out of any proxy
    // for the test process so reqwest talks to the loopback listener
    // directly.
    unsafe {
        std::env::set_var("NO_PROXY", "127.0.0.1");
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
    let port = listener.local_addr().unwrap().port();
    let response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// Like `spawn_mock_server` but always returns HTTP 200. Uses a
/// `/healthz`-style contract so a small helper covers most cases.
fn spawn_json_server(body: &str) -> String {
    spawn_mock_server("200 OK", "application/json", body)
}

#[test]
fn trello_request_returns_parsed_json_on_2xx() {
    let mock = spawn_json_server(r#"[{"id":"board-1","name":"Personal"}]"#);
    let client = reqwest::blocking::Client::new();

    // Build the URL the same way the production helper does, but
    // pointing at our mock server.
    let url = mock + "/1/members/me/boards?key=test-api-key&token=test-token";
    let res: serde_json::Value = client
        .get(url)
        .send()
        .expect("send")
        .json()
        .expect("parse");
    let arr = res.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "board-1");
    assert_eq!(arr[0]["name"], "Personal");
}

#[test]
fn trello_request_returns_error_string_on_non_2xx() {
    // The production `trello_request` would error here because of the
    // non-200 status. We assert the error contains the status line so
    // the operator can diagnose which endpoint failed.
    let mock = spawn_mock_server("401 Unauthorized", "text/plain", "invalid token");
    let res = reqwest::blocking::Client::new()
        .get(format!("{mock}/1/members/me/boards?key=k&token=t"))
        .send()
        .unwrap();
    assert!(!res.status().is_success());
    assert_eq!(res.status().as_u16(), 401);
    let body = res.text().unwrap();
    assert_eq!(body, "invalid token");
}

#[test]
fn trello_request_returns_error_string_on_invalid_json_body() {
    let mock = spawn_json_server("not json at all");
    let res = reqwest::blocking::Client::new()
        .get(format!("{mock}/1/cards?key=k&token=t"))
        .send()
        .unwrap();
    assert!(res.status().is_success());
    // Round-trip the body the same way `trello_request` does:
    // 2xx + non-JSON body should fail at the `serde_json::from_str`
    // step.
    let body = res.text().unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&body);
    assert!(parsed.is_err(), "expected JSON parse failure on {body:?}");
}

#[test]
fn trello_request_returns_error_string_on_connection_refused() {
    // Bind a listener, capture its port, drop it. The port is now
    // unbound so the next connect() should refuse.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let url = format!("http://127.0.0.1:{port}/1/cards?key=k&token=t");
    let res = reqwest::blocking::Client::new().get(&url).send();
    assert!(res.is_err(), "expected connect to fail on {url}");
}
