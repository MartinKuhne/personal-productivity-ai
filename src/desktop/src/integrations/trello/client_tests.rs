//! Tests for the Trello REST client (`client.rs`).
//!
//! Sidecar file. Extracted from `client.rs` per AGENTS.md RUST-056 so
//! the implementation module stays focused on production code. Lives
//! in a sibling file so private item access via `super::*` keeps
//! working.
//!
//! Covers:
//!  * URL shape (the API-key + token query string)
//!  * the `trello_http_call` happy path against a local `wiremock` server
//!  * the `trello_http_call` error path (non-2xx status)
//!  * JSON parse failure on a 2xx body
//!  * transport failure (connection refused) on an unreachable port

use super::*;
use crate::config::TrelloClient;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

/// A wiremock server whose backing tokio runtime lives as long as
/// this guard. The runtime owns the hyper task that serves mock
/// responses — drop the guard and the server stops. The `server`
/// field is also returned so tests can register stubs *after*
/// construction, and `uri()` is the base URL the test should hit.
struct WiremockGuard {
    server: MockServer,
    _runtime: tokio::runtime::Runtime,
}

impl WiremockGuard {
    fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        // `block_on` enters the runtime context for the duration of the
        // future; no need for an explicit `runtime.enter()` guard.
        let server = runtime.block_on(MockServer::start());
        Self {
            server,
            _runtime: runtime,
        }
    }

    fn uri(&self) -> String {
        self.server.uri()
    }

    fn register(&self, mock: Mock) {
        self._runtime.block_on(self.server.register(mock));
    }
}

#[test]
fn trello_http_call_returns_parsed_json_on_2xx() {
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET"))
            .and(path("/1/members/me/boards"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(r#"[{"id":"board-1","name":"Personal"}]"#),
            ),
    );

    let url = format!(
        "{}/1/members/me/boards?key=test-api-key&token=test-token",
        mock.uri()
    );
    let res = trello_http_call(reqwest::Method::GET, &url, None).expect("trello_http_call");

    let arr = res.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "board-1");
    assert_eq!(arr[0]["name"], "Personal");
}

#[test]
fn trello_http_call_returns_error_string_on_non_2xx() {
    // The production `trello_http_call` returns an `Err` here because of
    // the non-200 status. We assert the error string contains the status
    // and the response body so the operator can diagnose which endpoint
    // failed.
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(any()).respond_with(
            ResponseTemplate::new(401)
                .insert_header("content-type", "text/plain")
                .set_body_string("invalid token"),
        ),
    );

    let url = format!("{}/1/members/me/boards?key=k&token=t", mock.uri());
    let err =
        trello_http_call(reqwest::Method::GET, &url, None).expect_err("expected non-2xx to fail");
    assert!(
        err.contains("401"),
        "error should mention the 401 status, got: {err}"
    );
    assert!(
        err.contains("invalid token"),
        "error should include the response body, got: {err}"
    );
}

#[test]
fn trello_http_call_returns_error_string_on_invalid_json_body() {
    // Round-trip the body the same way `trello_http_call` does:
    // 2xx + non-JSON body should fail at the `serde_json::from_str`
    // step.
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(any()).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string("not json at all"),
        ),
    );

    let url = format!("{}/1/cards?key=k&token=t", mock.uri());
    let err = trello_http_call(reqwest::Method::GET, &url, None)
        .expect_err("expected JSON parse failure");
    // Production wraps the `serde_json::Error` via `Display`; we just
    // assert a non-empty error so the test does not become a moving
    // target across serde_json versions (the exact wording changes
    // between releases).
    assert!(
        !err.is_empty(),
        "expected non-empty error, got empty string"
    );
}

#[test]
fn trello_http_call_returns_error_string_on_connection_refused() {
    // Bind a listener, capture its port, drop it. The port is now
    // unbound so the next connect() should refuse.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let url = format!("http://127.0.0.1:{port}/1/cards?key=k&token=t");
    let err =
        trello_http_call(reqwest::Method::GET, &url, None).expect_err("expected connect to fail");
    // reqwest surfaces connection refused as a `reqwest::Error` whose
    // Display string contains "connection refused" (or similar) on
    // every platform we ship. We only assert non-empty so the test
    // does not become a moving target across rustls/native-tls
    // versions.
    assert!(
        !err.is_empty(),
        "expected non-empty error, got empty string"
    );
}
