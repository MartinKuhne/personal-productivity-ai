//! Tests for [`OAuthClient`] registration strategies (`agent::lib::mcp::oauth::client`).

use super::*;

fn as_metadata() -> AuthorizationServerMetadata {
    AuthorizationServerMetadata {
        issuer: "https://auth.example.com".to_owned(),
        authorization_endpoint: "https://auth.example.com/authorize".to_owned(),
        token_endpoint: "https://auth.example.com/token".to_owned(),
        jwks_uri: None,
        registration_endpoint: None,
        scopes_supported: vec!["read".to_owned()],
        response_types_supported: vec!["code".to_owned()],
        code_challenge_methods_supported: vec!["S256".to_owned()],
        grant_types_supported: vec!["authorization_code".to_owned()],
        token_endpoint_auth_methods_supported: vec!["none".to_owned()],
        client_id_metadata_document_supported: false,
        service_documentation: None,
        ui_locales_supported: vec![],
    }
}

#[test]
fn pre_registered_takes_priority_over_dcr() {
    let pre = PreRegisteredClient {
        client_id: "preset".to_owned(),
        client_secret: Some("topsecret".to_owned()),
    };
    let mut meta = as_metadata();
    // Even with a registration endpoint, pre-registered wins.
    meta.registration_endpoint = Some("https://auth.example.com/register".to_owned());
    let client =
        OAuthClient::resolve(Some(&pre), &meta, "https://mcp.example.com").expect("must resolve");
    assert_eq!(client.client_id, "preset");
    assert_eq!(client.client_secret.as_deref(), Some("topsecret"));
}

#[test]
fn no_registration_and_no_pre_registered_returns_error() {
    let meta = as_metadata();
    let err = OAuthClient::resolve(None, &meta, "https://mcp.example.com")
        .expect_err("must fail without any path");
    match err {
        OAuthError::ClientRegistration(msg) => {
            assert!(msg.contains("no usable client registration strategy"));
        }
        other => panic!("expected ClientRegistration, got {other:?}"),
    }
}

#[test]
fn from_stored_returns_supplied_client_id() {
    let meta = as_metadata();
    let client = OAuthClient::from_stored(
        "abc".to_owned(),
        "https://auth.example.com".to_owned(),
        &meta,
        None,
    )
    .expect("must succeed");
    assert_eq!(client.client_id, "abc");
    assert!(client.client_secret.is_none());
}

#[test]
fn from_stored_uses_pre_registered_when_ids_match() {
    let meta = as_metadata();
    let pre = PreRegisteredClient {
        client_id: "abc".to_owned(),
        client_secret: Some("xyz".to_owned()),
    };
    let client = OAuthClient::from_stored(
        "abc".to_owned(),
        "https://auth.example.com".to_owned(),
        &meta,
        Some(&pre),
    )
    .expect("must succeed");
    assert_eq!(client.client_secret.as_deref(), Some("xyz"));
}

#[test]
fn from_stored_ignores_a_pre_registered_client_with_another_id() {
    let meta = as_metadata();
    let pre = PreRegisteredClient {
        client_id: "other".to_owned(),
        client_secret: Some("xyz".to_owned()),
    };
    let client = OAuthClient::from_stored(
        "abc".to_owned(),
        "https://auth.example.com".to_owned(),
        &meta,
        Some(&pre),
    )
    .expect("must succeed");
    assert_eq!(
        client.client_id, "abc",
        "the stored id must win when the ids differ"
    );
    assert!(
        client.client_secret.is_none(),
        "another client's secret must not be attached"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Dynamic Client Registration (RFC 7591)
// ─────────────────────────────────────────────────────────────────────────────

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A loopback HTTP double for the registration endpoint. Every request is
/// answered with the canned response and recorded as `(target, body)`.
struct MockRegistrationServer {
    port: u16,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockRegistrationServer {
    fn start(response: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registration server");
        let port = listener.local_addr().expect("mock addr").port();
        let requests: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let requests_for_thread = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let Some((target, body)) = read_request(&mut stream) else {
                    continue;
                };
                requests_for_thread
                    .lock()
                    .expect("lock requests")
                    .push((target, body));
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, requests }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn requests(&self) -> Vec<(String, String)> {
        self.requests.lock().expect("lock requests").clone()
    }
}

/// Index just past the `\r\n\r\n` that ends the header block.
fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Read one HTTP request, returning its request target and body.
fn read_request(stream: &mut std::net::TcpStream) -> Option<(String, String)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        match stream.read(&mut chunk).ok()? {
            0 => return None,
            n => {
                buf.extend_from_slice(&chunk[..n]);
                if let Some(end) = header_end(&buf) {
                    break end;
                }
            }
        }
    };
    let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
    let content_length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while buf.len() < end + content_length {
        match stream.read(&mut chunk).ok()? {
            0 => break,
            n => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let request_line = String::from_utf8_lossy(&buf[..end])
        .lines()
        .next()?
        .to_owned();
    let target = request_line.split_whitespace().nth(1)?.to_owned();
    Some((target, String::from_utf8_lossy(&buf[end..]).into_owned()))
}

/// A canned HTTP response.
fn canned(status: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn json_canned(status: u16, reason: &str, body: &str) -> String {
    canned(status, reason, "application/json", body)
}

/// The shared metadata document with a registration endpoint attached.
fn with_registration(endpoint: &str) -> AuthorizationServerMetadata {
    let mut meta = as_metadata();
    meta.registration_endpoint = Some(endpoint.to_owned());
    meta
}

/// Call the private `register_dynamic` against a mock at `/register`.
fn register_at(server: &MockRegistrationServer) -> Result<OAuthClient, OAuthError> {
    let meta = with_registration(&server.url("/register"));
    register_dynamic(&meta, "https://mcp.example.com/mcp")
}

#[test]
fn register_dynamic_posts_the_rfc7591_request() {
    let server = MockRegistrationServer::start(json_canned(
        201,
        "Created",
        r#"{"client_id":"dyn-1","client_secret":"dyn-secret","redirect_uris":["http://127.0.0.1/callback"]}"#,
    ));

    let client = register_at(&server).expect("registration should succeed");
    assert_eq!(client.client_id, "dyn-1");
    assert_eq!(client.client_secret.as_deref(), Some("dyn-secret"));
    assert!(client.extra_headers.is_empty());

    let requests = server.requests();
    assert_eq!(requests.len(), 1, "exactly one registration request");
    let (target, body) = &requests[0];
    assert_eq!(target, "/register");
    assert!(
        body.contains(r#""redirect_uris":["http://127.0.0.1/callback"]"#),
        "got: {body}"
    );
    assert!(
        body.contains(r#""token_endpoint_auth_method":"none""#),
        "the client must ask to be public, got: {body}"
    );
    assert!(
        body.contains(r#""grant_types":["authorization_code","refresh_token"]"#),
        "got: {body}"
    );
    assert!(
        body.contains(r#""software_id":"#),
        "the client info must be sent as software_id, got: {body}"
    );
}

#[test]
fn resolve_uses_dynamic_registration_when_advertised() {
    let server = MockRegistrationServer::start(json_canned(200, "OK", r#"{"client_id":"dyn-2"}"#));

    let meta = with_registration(&server.url("/register"));
    let client = OAuthClient::resolve(None, &meta, "https://mcp.example.com/mcp")
        .expect("DCR should be chosen");
    assert_eq!(client.client_id, "dyn-2");
    assert!(client.client_secret.is_none());
}

#[test]
fn register_dynamic_maps_an_oauth_error_envelope() {
    let server = MockRegistrationServer::start(json_canned(
        400,
        "Bad Request",
        r#"{"error":"invalid_redirect_uri","error_description":"loopback not allowed"}"#,
    ));

    match register_at(&server).expect_err("the server rejected the redirect URI") {
        OAuthError::OAuth { endpoint, body } => {
            assert_eq!(endpoint, "registration");
            assert_eq!(body.error, "invalid_redirect_uri");
            assert_eq!(
                body.error_description.as_deref(),
                Some("loopback not allowed")
            );
        }
        other => panic!("expected an OAuth envelope, got {other:?}"),
    }
}

#[test]
fn register_dynamic_reports_a_non_json_error_body() {
    let server = MockRegistrationServer::start(canned(
        500,
        "Internal Server Error",
        "text/plain",
        "registration exploded",
    ));

    match register_at(&server).expect_err("the server failed") {
        OAuthError::ClientRegistration(msg) => {
            assert!(
                msg.contains("registration endpoint returned error"),
                "got: {msg}"
            );
            assert!(msg.contains("registration exploded"), "got: {msg}");
        }
        other => panic!("expected ClientRegistration, got {other:?}"),
    }
}

#[test]
fn register_dynamic_reports_an_error_envelope_returned_with_200() {
    let server =
        MockRegistrationServer::start(json_canned(200, "OK", r#"{"error":"invalid_client"}"#));

    match register_at(&server).expect_err("an error envelope is never a registration") {
        OAuthError::OAuth { endpoint, body } => {
            assert_eq!(endpoint, "registration");
            assert_eq!(body.error, "invalid_client");
        }
        other => panic!("expected an OAuth envelope, got {other:?}"),
    }
}

#[test]
fn register_dynamic_reports_malformed_json() {
    let server = MockRegistrationServer::start(json_canned(200, "OK", "{\"client_id\":"));

    match register_at(&server).expect_err("truncated json") {
        OAuthError::Protocol(msg) => {
            assert!(msg.contains("registration response json"), "got: {msg}");
        }
        other => panic!("expected Protocol, got {other:?}"),
    }
}

#[test]
fn register_dynamic_reports_an_unexpected_response_shape() {
    let server = MockRegistrationServer::start(json_canned(200, "OK", r#"{"token":"nope"}"#));

    match register_at(&server).expect_err("no client_id in the document") {
        OAuthError::Protocol(msg) => {
            assert!(msg.contains("registration response shape"), "got: {msg}");
        }
        other => panic!("expected Protocol, got {other:?}"),
    }
}

#[test]
fn register_dynamic_rejects_an_empty_client_id() {
    let server = MockRegistrationServer::start(json_canned(200, "OK", r#"{"client_id":""}"#));

    match register_at(&server).expect_err("an empty client_id is unusable") {
        OAuthError::ClientRegistration(msg) => {
            assert!(msg.contains("missing client_id"), "got: {msg}");
        }
        other => panic!("expected ClientRegistration, got {other:?}"),
    }
}

#[test]
fn register_dynamic_refuses_a_plain_http_registration_endpoint() {
    let meta = with_registration("http://auth.example.com/register");
    match register_dynamic(&meta, "https://mcp.example.com/mcp")
        .expect_err("plain http is not allowed")
    {
        OAuthError::SpecViolation(msg) => {
            assert!(msg.contains("registration endpoint"), "got: {msg}");
            assert!(msg.contains("not https"), "got: {msg}");
        }
        other => panic!("expected SpecViolation, got {other:?}"),
    }
}

#[test]
fn register_dynamic_requires_a_registration_endpoint() {
    let meta = as_metadata();
    match register_dynamic(&meta, "https://mcp.example.com/mcp")
        .expect_err("nothing to register against")
    {
        OAuthError::ClientRegistration(msg) => {
            assert!(msg.contains("no registration_endpoint"), "got: {msg}");
        }
        other => panic!("expected ClientRegistration, got {other:?}"),
    }
}

#[test]
fn register_dynamic_reports_a_transport_failure() {
    // Nothing listens on port 1, so the request fails before any HTTP happens.
    let meta = with_registration("http://127.0.0.1:1/register");
    match register_dynamic(&meta, "https://mcp.example.com/mcp")
        .expect_err("the endpoint is unreachable")
    {
        OAuthError::ClientRegistration(msg) => {
            assert!(msg.contains("registration request failed"), "got: {msg}");
        }
        other => panic!("expected ClientRegistration, got {other:?}"),
    }
}
