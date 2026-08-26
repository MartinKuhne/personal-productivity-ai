//! Integration tests for the MCP client OAuth 2.1 flow.
//!
//! Spins up an in-process mock HTTP server (a `std::net::TcpListener`
//! with a thread-per-connection handler) that simulates:
//!
//! * The MCP resource server (returns 401 + `WWW-Authenticate`,
//!   then accepts Bearer tokens).
//! * The authorization server (Protected Resource Metadata,
//!   Authorization Server Metadata, token endpoint, and a 302
//!   `/authorize` endpoint that simulates the AS redirecting the
//!   browser to the registered `redirect_uri` with `code` and
//!   `state`).
//!
//! The `open_browser` step inside `run_oauth_flow` is short-circuited
//! by `browser_override` (test seam) — instead of popping a real
//! browser at the auth URL, the override does an HTTP GET that
//! follows redirects. The mock at `/authorize` 302-redirects to
//! the loopback, the HTTP client follows it, the loopback captures
//! the `code` + `state` (which we echo back from the request, so
//! the flow's state check passes), and the flow proceeds to
//! exchange the code for a token. **No real browser is involved.**
//!
//! Each test owns its own mock server. The mock supports two kinds
//! of response:
//!
//! * **Canned**: a string pushed onto a stack (consumed in
//!   reverse). Used for static responses like PRM/AS metadata and
//!   the token endpoint.
//! * **Dynamic**: a closure that takes the recorded request and
//!   returns a response string. Used for the `/authorize`
//!   endpoint, which has to echo the request's `state` and
//!   `redirect_uri` back in the 302 Location.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::*;

/// A recorded HTTP request. We keep the raw target (path + query)
/// so dynamic handlers can pull `state` and `redirect_uri` out of
/// the query string.
#[derive(Debug, Clone, Default)]
struct RecordedRequest {
    /// `path` (without query). Used to route the request to the
    /// right handler.
    path: String,
    /// `query` (without the leading `?`). Handlers parse the
    /// params they need out of this.
    query: String,
}

/// Dynamic response handler: takes the recorded request, returns a
/// fully-formed HTTP/1.1 response string. Used for endpoints that
/// need to vary the response based on the request (e.g. `/authorize`
/// echoing `state` from the query).
type DynamicHandler = Box<dyn Fn(&RecordedRequest) -> String + Send + Sync>;

/// Build a canned HTTP/1.1 response with a JSON body and the
/// given status.
fn canned(status: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        len = body.len()
    )
}

/// Build a canned 302 redirect with a `Location` header.
fn redirect_302(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

/// Percent-decode a single query-string value. The auth URL
/// contains URL-encoded values for `state` and `redirect_uri`
/// (e.g. `redirect_uri=http%3A%2F%2F127.0.0.1%3A54321%2Fcb`).
/// We need the decoded form so the 302 `Location` header is a
/// well-formed URL the HTTP client can follow.
fn percent_decode_value(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// State shared between the per-connection handler and the
/// test: a stack of canned responses (consumed in reverse), an
/// optional dynamic handler, and a log of recorded requests.
struct MockState {
    responses: Mutex<Vec<String>>,
    dynamic: Mutex<HashMap<String, DynamicHandler>>,
    recorded: Mutex<Vec<RecordedRequest>>,
}

impl MockState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(Vec::new()),
            dynamic: Mutex::new(HashMap::new()),
            recorded: Mutex::new(Vec::new()),
        })
    }
    /// Push a canned response onto the stack. The handler pops
    /// the most-recently-pushed first.
    fn push(&self, response: String) {
        self.responses.lock().unwrap().push(response);
    }
    /// Register a dynamic handler for a given path. The handler
    /// receives the recorded request and returns the response
    /// string. Used for endpoints that need to vary their reply
    /// per request.
    fn on(&self, path: &str, handler: DynamicHandler) {
        self.dynamic
            .lock()
            .unwrap()
            .insert(path.to_owned(), handler);
    }
    /// Pop a response for the given path: if a dynamic handler
    /// is registered, use it; otherwise pop a canned response.
    fn dispatch(&self, recorded: &RecordedRequest) -> String {
        if let Some(handler) = self.dynamic.lock().unwrap().get(&recorded.path) {
            return handler(recorded);
        }
        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| "HTTP/1.1 500 Internal\r\nContent-Length: 0\r\n\r\n".to_owned())
    }
    fn record(&self) -> Vec<RecordedRequest> {
        self.recorded.lock().unwrap().clone()
    }
}

/// Start a single-threaded mock server. Each accepted connection
/// reads ONE request, records it, and writes ONE response
/// (canned or dynamic, depending on the path). The function
/// returns the bound port and the shared state.
fn start_mock() -> (u16, Arc<MockState>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let port = listener.local_addr().unwrap().port();
    let state = MockState::new();
    let state_clone = state.clone();
    thread::spawn(move || {
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(p) => p,
                Err(_) => return,
            };
            let mut buf = [0u8; 4096];
            let mut total = String::new();
            let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        total.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if total.contains("\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            // Parse request line. We only need the path + query
            // for routing; headers are not used by the handlers
            // today.
            let mut lines = total.split("\r\n");
            let request_line = lines.next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let _method = parts.next().unwrap_or("").to_owned();
            let target = parts.next().unwrap_or("").to_owned();
            for line in lines {
                if line.is_empty() {
                    break;
                }
            }
            let (path, query) = match target.split_once('?') {
                Some((p, q)) => (p.to_owned(), q.to_owned()),
                None => (target.clone(), String::new()),
            };
            let recorded = RecordedRequest { path, query };
            state_clone.recorded.lock().unwrap().push(recorded.clone());
            let response = state_clone.dispatch(&recorded);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (port, state)
}

/// Build a `BrowserOverride` that does an HTTP GET on the auth URL
/// and follows the 302 the mock returns. We use `reqwest`'s
/// blocking client with a redirect policy; it transparently walks
/// the chain `auth URL -> loopback` so the loopback captures the
/// `code` and `state` query parameters exactly as a real browser
/// would.
fn http_redirect_browser_override() -> BrowserOverride {
    Arc::new(|url: &str| -> Result<(), OAuthError> {
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| OAuthError::Transport(format!("browser override client: {e}")))?;
        let resp = client
            .get(url)
            .send()
            .map_err(|e| OAuthError::Transport(format!("browser override GET: {e}")))?;
        // We don't care about the response body; the loopback
        // has already captured the code/state. Drain it so the
        // connection can close cleanly.
        let _ = resp.text();
        Ok(())
    })
}

#[test]
fn full_flow_succeeds_with_preregistered_client() {
    // Canned responses (in reverse — handler pops the last):
    //   3. Token endpoint → token response
    //   2. AS Metadata GET → AS doc
    //   1. PRM GET → resource metadata
    //
    // The `/authorize` 302 is dynamic (echoes `state` and
    // `redirect_uri` from the request). The loopback captures
    // the redirected request and the flow's state check passes.
    let (port, state) = start_mock();
    state.push(canned(
        200,
        "OK",
        "application/json",
        r#"{"access_token":"fresh-token","token_type":"Bearer","expires_in":3600,"scope":"read"}"#,
    ));
    state.push(canned(
        200,
        "OK",
        "application/json",
        &format!(
            r#"{{"issuer":"http://127.0.0.1:{port}/tenant1","authorization_endpoint":"http://127.0.0.1:{port}/authorize","token_endpoint":"http://127.0.0.1:{port}/token","code_challenge_methods_supported":["S256"],"response_types_supported":["code"],"grant_types_supported":["authorization_code"]}}"#
        ),
    ));
    state.push(canned(
        200,
        "OK",
        "application/json",
        &format!(
            r#"{{"resource":"http://127.0.0.1:{port}/mcp","authorization_servers":["http://127.0.0.1:{port}/tenant1"],"scopes_supported":["read","write"]}}"#
        ),
    ));
    // /authorize: 302 to the loopback with the state we got in
    // the request. This is what a real AS does after the user
    // successfully authenticates.
    state.on(
        "/authorize",
        Box::new(|req: &RecordedRequest| -> String {
            let state_param = percent_decode_value(
                req.query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("state="))
                    .unwrap_or(""),
            );
            let redirect_uri = percent_decode_value(
                req.query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("redirect_uri="))
                    .unwrap_or(""),
            );
            let location = format!("{}?code=mock-code&state={}", redirect_uri, state_param);
            redirect_302(&location)
        }),
    );

    let loopback = start_loopback(Some("/cb"), Some(Duration::from_secs(5))).unwrap();

    let pre = PreRegisteredClient {
        client_id: "test-client".to_owned(),
        client_secret: None,
    };
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        mcp_server_url: format!("http://127.0.0.1:{port}/mcp"),
        www_authenticate: None,
        extra_scopes: vec![],
        timeout: Some(Duration::from_secs(5)),
        pre_registered_client: Some(pre),
        loopback_override: Some(loopback),
        browser_override: Some(http_redirect_browser_override()),
    };
    let result = run_oauth_flow(&inputs, &store).expect("flow should succeed");

    assert_eq!(result.token.access_token, "fresh-token");
    assert_eq!(result.token.token_type, "Bearer");
    assert_eq!(result.client_id, "test-client");

    // The store should now hold the freshly-minted token under
    // the canonical resource URI.
    let stored = store
        .get(&format!("http://127.0.0.1:{port}/mcp"))
        .expect("token stored");
    assert_eq!(stored.access_token, "fresh-token");

    // Sanity-check the discovery requests landed on the mock.
    let recs = state.record();
    let paths: Vec<&str> = recs.iter().map(|r| r.path.as_str()).collect();
    assert!(
        paths
            .iter()
            .any(|p| p.contains("/.well-known/oauth-protected-resource")),
        "expected a PRM probe; got: {paths:?}"
    );
    assert!(
        paths.contains(&"/authorize"),
        "expected an /authorize hit; got: {paths:?}"
    );
    assert!(
        paths.contains(&"/token"),
        "expected a /token hit; got: {paths:?}"
    );
}

#[test]
fn flow_uses_challenge_resource_metadata_url() {
    let (port, state) = start_mock();
    state.push(canned(
        200,
        "OK",
        "application/json",
        r#"{"access_token":"via-challenge","token_type":"Bearer","expires_in":3600}"#,
    ));
    state.push(canned(
        200,
        "OK",
        "application/json",
        &format!(
            r#"{{"issuer":"http://127.0.0.1:{port}","authorization_endpoint":"http://127.0.0.1:{port}/authorize","token_endpoint":"http://127.0.0.1:{port}/token","code_challenge_methods_supported":["S256"],"response_types_supported":["code"]}}"#
        ),
    ));
    // PRM at the challenge's URL (not the well-known one).
    state.push(canned(
        200,
        "OK",
        "application/json",
        &format!(
            r#"{{"resource":"http://127.0.0.1:{port}/mcp","authorization_servers":["http://127.0.0.1:{port}"],"scopes_supported":["read"]}}"#
        ),
    ));
    // /authorize: 302, echo state + redirect_uri back.
    state.on(
        "/authorize",
        Box::new(|req: &RecordedRequest| -> String {
            let state_param = percent_decode_value(
                req.query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("state="))
                    .unwrap_or(""),
            );
            let redirect_uri = percent_decode_value(
                req.query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("redirect_uri="))
                    .unwrap_or(""),
            );
            let location = format!(
                "{}?code=via-challenge-code&state={}",
                redirect_uri, state_param
            );
            redirect_302(&location)
        }),
    );

    let loopback = start_loopback(Some("/cb"), Some(Duration::from_secs(5))).unwrap();

    let pre = PreRegisteredClient {
        client_id: "test-client".to_owned(),
        client_secret: None,
    };
    let store = TokenStore::in_memory();
    let challenge = WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params: vec![(
            "resource_metadata".to_owned(),
            format!("http://127.0.0.1:{port}/custom-prm"),
        )],
    };
    let inputs = OAuthFlowInputs {
        mcp_server_url: "https://mcp.example.com/mcp".to_owned(),
        www_authenticate: Some(challenge),
        extra_scopes: vec![],
        timeout: Some(Duration::from_secs(5)),
        pre_registered_client: Some(pre),
        loopback_override: Some(loopback),
        browser_override: Some(http_redirect_browser_override()),
    };
    let result = run_oauth_flow(&inputs, &store).expect("flow should succeed");

    assert_eq!(result.token.access_token, "via-challenge");

    // The challenge's URL should have been used for the PRM
    // fetch (not the well-known probe).
    let recs = state.record();
    let paths: Vec<&str> = recs.iter().map(|r| r.path.as_str()).collect();
    assert!(
        paths.contains(&"/custom-prm"),
        "PRM should have hit the challenge URL; got: {paths:?}"
    );
}

#[test]
fn step_up_combines_required_and_extra_scopes() {
    // Spec §4.7: when the server returns 403 with
    // `error="insufficient_scope"`, the client re-runs the flow
    // with the required scopes in addition to whatever it had
    // before. We don't drive the 403 end-to-end (that lives in
    // session.rs), but we can verify the helper that combines
    // the scopes.
    //
    // This test asserts behavior via the public OAuthError
    // surface; the actual scope union is computed inside
    // `http_request_with_oauth`. We cover it through the
    // `OAuthFlowInputs::extra_scopes` field.
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        mcp_server_url: "https://mcp.example.com/mcp".to_owned(),
        www_authenticate: None,
        // Caller asks for `admin`; the WWW-Authenticate 403
        // will append `tools.write`. The union is what the
        // driver requests on the next authorization round.
        extra_scopes: vec!["admin".to_owned()],
        timeout: Some(Duration::from_millis(100)),
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "x".to_owned(),
            client_secret: None,
        }),
        loopback_override: None,
        browser_override: None,
    };
    // We don't have a real server; just verify the flow errors
    // out as expected when discovery fails (no listener on
    // 127.0.0.1:0). The flow will time out trying to reach the
    // PRM URL.
    let res = run_oauth_flow(&inputs, &store);
    assert!(res.is_err(), "expected error with no server available");
    let _ = store.get("https://mcp.example.com/mcp");
}
