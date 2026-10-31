//! Tests for the OAuth authorization-code flow and token refresh (`agent::lib::mcp::oauth::flow`).

use super::super::test_support::{MockHttpServer, MockResponse, RecordedRequest};
use super::*;

/// What a `build_resource_uri` case must produce.
enum ExpectedUri<'a> {
    /// The exact resulting URI.
    Exact(&'a str),
    /// A successful parse whose result contains this fragment.
    Contains(&'a str),
    /// The input must be rejected.
    Rejected,
}

#[test]
fn build_resource_uri_trims_strips_and_validates_each_input() {
    let cases = [
        (
            "query and fragment are stripped",
            "https://mcp.example.com/mcp?token=secret#frag",
            ExpectedUri::Exact("https://mcp.example.com/mcp"),
        ),
        (
            "a bare origin is kept verbatim",
            "https://mcp.example.com",
            ExpectedUri::Exact("https://mcp.example.com"),
        ),
        (
            "surrounding whitespace is trimmed",
            "  https://mcp.example.com/mcp  ",
            ExpectedUri::Exact("https://mcp.example.com/mcp"),
        ),
        (
            "a trailing fragment is stripped",
            "https://mcp.example.com/mcp#frag?q=1",
            ExpectedUri::Contains("/mcp"),
        ),
        (
            "ftp is not an http scheme",
            "ftp://mcp.example.com",
            ExpectedUri::Rejected,
        ),
        (
            "a bare word is not a URL",
            "not-a-url",
            ExpectedUri::Rejected,
        ),
        (
            "ftp is rejected on the trim path too",
            "ftp://mcp.example.com",
            ExpectedUri::Rejected,
        ),
        ("an empty input is rejected", "", ExpectedUri::Rejected),
    ];
    for (label, input, expected) in cases {
        match expected {
            ExpectedUri::Exact(expected) => {
                assert_eq!(
                    build_resource_uri(input).unwrap(),
                    expected,
                    "case '{label}'"
                );
            }
            ExpectedUri::Contains(fragment) => {
                let uri = build_resource_uri(input).unwrap();
                assert!(
                    uri.contains(fragment),
                    "case '{label}': expected '{fragment}' in: {uri}"
                );
            }
            ExpectedUri::Rejected => {
                assert!(
                    build_resource_uri(input).is_err(),
                    "case '{label}': expected an error for '{input}'"
                );
            }
        }
    }
}

/// A `ProtectedResourceMetadata` with the given `scopes_supported` and the
/// standard example resource / authorization-server URLs.
fn prm_with_scopes(scopes: &[&str]) -> ProtectedResourceMetadata {
    ProtectedResourceMetadata {
        resource: "https://mcp.example.com".to_owned(),
        authorization_servers: vec!["https://auth.example.com".to_owned()],
        scopes_supported: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        bearer_methods_supported: vec![],
        resource_name: None,
        resource_documentation: None,
        resource_policy_uri: None,
        resource_tos_uri: None,
    }
}

/// The expected outcome of a `pick_scope` case.
enum ExpectedScope<'a> {
    /// No scope at all.
    None,
    /// The exact scope string.
    Exact(&'a str),
    /// The whitespace-separated parts must contain every listed token.
    Parts(&'a [&'a str]),
    /// The scope string must contain every listed substring.
    Substrings(&'a [&'a str]),
    /// The first list's tokens must appear exactly once (deduplication) and the
    /// second list's tokens must appear at least once.
    Deduplicated(&'a [&'a str], &'a [&'a str]),
}

/// One `pick_scope` case: the WWW-Authenticate `scope` parameter (if any), the
/// metadata `scopes_supported`, the caller's extra scopes and the expectation.
struct ScopeCase<'a> {
    label: &'a str,
    challenge: Option<&'a str>,
    scopes_supported: &'a [&'a str],
    extra: &'a [&'a str],
    expected: ExpectedScope<'a>,
}

/// Assert a `pick_scope` case table.
fn assert_picked_scope(cases: Vec<ScopeCase<'_>>) {
    for case in cases {
        let challenge = case.challenge.map(|scope| WwwAuthenticateChallenge {
            scheme: "Bearer".to_owned(),
            params: vec![("scope".to_owned(), scope.to_owned())],
        });
        let rm = prm_with_scopes(case.scopes_supported);
        let extras: Vec<String> = case.extra.iter().map(|scope| (*scope).to_owned()).collect();
        let scope = pick_scope(challenge.as_ref(), &rm, &extras);
        let scope_of = |what: &str| {
            scope
                .clone()
                .unwrap_or_else(|| panic!("case '{}': expected {what}", case.label))
        };
        match case.expected {
            ExpectedScope::None => assert!(
                scope.is_none(),
                "case '{}': expected no scope, got: {scope:?}",
                case.label
            ),
            ExpectedScope::Exact(expected) => {
                assert_eq!(scope.as_deref(), Some(expected), "case '{}'", case.label)
            }
            ExpectedScope::Parts(tokens) => {
                let scope = scope_of("a scope");
                let parts: Vec<&str> = scope.split_whitespace().collect();
                for token in tokens {
                    assert!(
                        parts.contains(token),
                        "case '{}': expected the part '{token}' in: {scope}",
                        case.label
                    );
                }
            }
            ExpectedScope::Substrings(fragments) => {
                let scope = scope_of("a scope");
                for fragment in fragments {
                    assert!(
                        scope.contains(fragment),
                        "case '{}': expected '{fragment}' in: {scope}",
                        case.label
                    );
                }
            }
            ExpectedScope::Deduplicated(once, present) => {
                let scope = scope_of("a scope");
                let parts: Vec<&str> = scope.split_whitespace().collect();
                for token in once {
                    let hits = parts.iter().filter(|part| *part == token).count();
                    assert_eq!(
                        hits, 1,
                        "case '{}': '{token}' must appear exactly once in: {scope}",
                        case.label
                    );
                }
                for token in present {
                    assert!(
                        parts.contains(token),
                        "case '{}': expected the part '{token}' in: {scope}",
                        case.label
                    );
                }
            }
        }
    }
}

#[test]
fn pick_scope_prefers_the_challenge_then_the_metadata_then_the_extras() {
    assert_picked_scope(vec![
        ScopeCase {
            label: "the WWW-Authenticate scope is used first",
            challenge: Some("read"),
            scopes_supported: &["read", "write"],
            extra: &["write", "admin"],
            expected: ExpectedScope::Parts(&["read", "write", "admin"]),
        },
        ScopeCase {
            label: "without a challenge scopes_supported is used",
            challenge: None,
            scopes_supported: &["read", "write"],
            extra: &[],
            expected: ExpectedScope::Exact("read write"),
        },
        ScopeCase {
            label: "no scopes anywhere yields None",
            challenge: None,
            scopes_supported: &[],
            extra: &[],
            expected: ExpectedScope::None,
        },
        ScopeCase {
            label: "extra scopes are deduplicated against the metadata",
            challenge: None,
            scopes_supported: &["read"],
            extra: &["read", "write"],
            expected: ExpectedScope::Deduplicated(&["read"], &["write"]),
        },
        ScopeCase {
            // An empty challenge scope is ignored; the metadata and the extras
            // are used instead.
            label: "a blank challenge scope falls back to the metadata",
            challenge: Some("   "),
            scopes_supported: &["read"],
            extra: &["extra"],
            expected: ExpectedScope::Substrings(&["read", "extra"]),
        },
    ]);
}

/// `refresh` must exchange the stored refresh token at the token
/// endpoint (no browser round-trip) and update the store. The
/// token request must carry the `resource` param (MCP-013) and
/// the caller's scopes.
///
/// Disabled by default: this test hangs in our CI environment.
/// The hang is in the OAuth refresh path (likely the
/// loopback/discovery HTTP calls) and exceeds the 60-second
/// default test timeout. Re-enable locally with
/// `cargo test -- --ignored refresh_exchanges_stored_refresh_token`
/// and bisect before relying on it in CI.
#[test]
#[ignore = "hangs in CI; OAuth refresh path exceeds 60s test timeout"]
fn refresh_exchanges_stored_refresh_token_without_browser() {
    let server = mock_oauth_server();
    let origin = server.origin.clone();
    let resource = format!("{origin}/mcp");

    let store = TokenStore::in_memory();
    let existing = StoredToken {
        access_token: "expired".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_at: Some(0),
        refresh_token: Some("rt-1".to_owned()),
        scope: vec!["read".to_owned()],
        client_id: Some("client-1".to_owned()),
        issuer: Some(origin.clone()),
    };
    store.put(&resource, existing.clone());

    let inputs = OAuthFlowInputs {
        mcp_server_url: resource.clone(),
        www_authenticate: None,
        extra_scopes: vec!["read".to_owned()],
        timeout: None,
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "client-1".to_owned(),
            client_secret: None,
        }),
        loopback_override: None,
        browser_override: None,
    };

    let output = refresh(&inputs, &store, &existing).expect("refresh should succeed");
    assert_eq!(output.token.access_token, "fresh-access");

    let stored = store
        .get(&resource)
        .expect("store should hold the refreshed token");
    assert_eq!(stored.access_token, "fresh-access");
    assert_eq!(stored.refresh_token.as_deref(), Some("rt-2"));
    assert_eq!(stored.client_id.as_deref(), Some("client-1"));

    let recorded = server.recorded.lock().expect("lock recorded");
    let token_req = recorded
        .iter()
        .find(|r| r.method == "POST" && r.path == "/token")
        .expect("a token request must have been recorded");
    assert!(
        token_req.body.contains("grant_type=refresh_token"),
        "must use the refresh grant, got body: {}",
        token_req.body
    );
    assert!(token_req.body.contains("refresh_token=rt-1"));
    assert!(token_req.body.contains("client_id=client-1"));
    assert!(token_req.body.contains("scope=read"));
    // MCP-013: the `resource` parameter must be present on the
    // token request, pointing at the MCP server URL.
    assert!(
        token_req.body.contains("resource=http%3A%2F%2F127.0.0.1"),
        "token request must carry the resource param, got body: {}",
        token_req.body
    );
}

/// A stored token without a refresh token cannot be refreshed;
/// `refresh` must report that rather than starting a browser
/// flow.
#[test]
fn refresh_requires_stored_refresh_token() {
    let store = TokenStore::in_memory();
    let existing = StoredToken {
        access_token: "expired".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_at: Some(0),
        refresh_token: None,
        scope: vec![],
        client_id: Some("client-1".to_owned()),
        issuer: Some("https://auth.example.com".to_owned()),
    };
    let inputs = OAuthFlowInputs {
        mcp_server_url: "https://mcp.example.com/mcp".to_owned(),
        www_authenticate: None,
        extra_scopes: vec![],
        timeout: None,
        pre_registered_client: None,
        loopback_override: None,
        browser_override: None,
    };
    let err = refresh(&inputs, &store, &existing).expect_err("refresh must fail");
    assert!(matches!(err, OAuthError::RefreshFailed(_)));
}

#[test]
fn encode_form_percent_encodes_keys_and_values() {
    let form = vec![("a b", "c+d".to_owned()), ("e&f", "g/h".to_owned())];
    let enc = encode_form(&form);
    // percent_encode uses application/x-www-form-urlencoded (space -> +)
    assert!(enc.contains("a+b="), "got {enc}");
    assert!(enc.contains("c%2Bd"), "got {enc}");
    assert!(enc.contains("e%26f="), "got {enc}");
    assert!(enc.contains("g%2Fh"), "got {enc}");
}

/// Minimal OAuth server double: PRM discovery, AS metadata, and
/// a token endpoint that mints `fresh-access`.
fn mock_oauth_server() -> MockHttpServer {
    MockHttpServer::start(move |req: &RecordedRequest, origin: &str| {
        if req.method == "GET"
            && (req.path == "/.well-known/oauth-protected-resource"
                || req.path == "/.well-known/oauth-protected-resource/mcp")
        {
            MockResponse::json(
                "HTTP/1.1 200 OK",
                format!(
                    r#"{{"resource":"{origin}/mcp","authorization_servers":["{origin}"],"scopes_supported":["read"]}}"#
                ),
            )
        } else if req.method == "GET"
            && (req.path == "/.well-known/oauth-authorization-server"
                || req.path == "/.well-known/openid-configuration")
        {
            MockResponse::json(
                "HTTP/1.1 200 OK",
                format!(
                    r#"{{"issuer":"{origin}","authorization_endpoint":"{origin}/auth","token_endpoint":"{origin}/token","code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"]}}"#
                ),
            )
        } else if req.method == "POST" && req.path == "/token" {
            MockResponse::json(
                "HTTP/1.1 200 OK",
                r#"{"access_token":"fresh-access","token_type":"Bearer","expires_in":3600,"refresh_token":"rt-2","scope":"read"}"#,
            )
        } else {
            MockResponse::json("HTTP/1.1 404 Not Found", "{}")
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// In-process OAuth server double
//
// `MockHttpServer` (wiremock) cannot be used here: it keeps a current-thread
// tokio runtime entered on the test thread, which dead-locks the
// `reqwest::blocking` calls the flow makes. The double below is a plain
// `std::net::TcpListener` on loopback, so no external network is involved.
// ─────────────────────────────────────────────────────────────────────────────

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::thread;

/// One request captured by [`MockOAuthServer`].
#[derive(Debug, Clone)]
struct Hit {
    method: String,
    path: String,
    query: String,
    body: String,
}

/// A loopback HTTP double for the OAuth endpoints. A responder closure decides
/// each reply and every request is recorded so tests can assert on the exact
/// form encoding the flow produced.
struct MockOAuthServer {
    port: u16,
    hits: Arc<Mutex<Vec<Hit>>>,
}

impl MockOAuthServer {
    fn start<F>(respond: F) -> Self
    where
        F: Fn(&Hit, &str) -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock oauth server");
        let port = listener.local_addr().expect("mock addr").port();
        let origin = format!("http://127.0.0.1:{port}");
        let hits: Arc<Mutex<Vec<Hit>>> = Arc::new(Mutex::new(Vec::new()));
        let hits_for_thread = Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let Some(hit) = read_hit(&mut stream) else {
                    continue;
                };
                let response = respond(&hit, &origin);
                hits_for_thread.lock().expect("lock hits").push(hit);
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, hits }
    }

    /// Absolute URL for a path on this mock. `url("")` is the origin.
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn hits(&self) -> Vec<Hit> {
        self.hits.lock().expect("lock hits").clone()
    }

    /// The first recorded request for `method` + `path`.
    fn only(&self, method: &str, path: &str) -> Hit {
        self.hits()
            .into_iter()
            .find(|h| h.method == method && h.path == path)
            .unwrap_or_else(|| panic!("no {method} {path} request; got {:?}", self.hits()))
    }
}

/// Index just past the `\r\n\r\n` that ends the header block.
fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Read one HTTP request from a mock connection.
fn read_hit(stream: &mut std::net::TcpStream) -> Option<Hit> {
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
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_owned(), query.to_owned()),
        None => (target.clone(), String::new()),
    };
    Some(Hit {
        method,
        path,
        query,
        body: String::from_utf8_lossy(&buf[end..]).into_owned(),
    })
}

/// A JSON response the discovery helpers accept.
fn json_response(status: u16, reason: &str, body: &str) -> String {
    response(status, reason, "application/json", body)
}

/// A response with an arbitrary content type.
fn response(status: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A 302 that sends the "browser" to the loopback callback.
fn redirect_302(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a query-string value so it can be echoed back in a
/// `Location` header.
fn percent_decode_value(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((h << 4) | l);
            i += 3;
            continue;
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-decoded value of a query parameter, or `""` when absent.
fn query_param(query: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    percent_decode_value(
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix(&prefix))
            .unwrap_or(""),
    )
}

/// Protected Resource Metadata advertising the mock as its authorization server.
fn prm_document(origin: &str) -> String {
    format!(
        r#"{{"resource":"{origin}/mcp","authorization_servers":["{origin}/tenant1"],"scopes_supported":["read"]}}"#
    )
}

/// Authorization Server Metadata with the fields the flow depends on.
fn as_document_full(
    origin: &str,
    authorization_endpoint: &str,
    token_endpoint: &str,
    code_challenge_methods: &str,
    registration_endpoint: Option<&str>,
) -> String {
    let registration = match registration_endpoint {
        Some(url) => format!(r#","registration_endpoint":"{url}""#),
        None => String::new(),
    };
    format!(
        r#"{{"issuer":"{origin}/tenant1","authorization_endpoint":"{authorization_endpoint}","token_endpoint":"{token_endpoint}"{registration},"code_challenge_methods_supported":{code_challenge_methods},"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"token_endpoint_auth_methods_supported":["none"]}}"#
    )
}

/// The standard loopback AS document (S256 advertised, no DCR endpoint).
fn as_document(origin: &str) -> String {
    as_document_full(
        origin,
        &format!("{origin}/authorize"),
        &format!("{origin}/token"),
        r#"["S256"]"#,
        None,
    )
}

/// Serve `prm` and `as_doc` on their well-known paths; 404 everything else.
/// Both builders receive the mock's own origin so the documents always point
/// back at the server that is answering.
fn discovery_mock<P, A>(prm: P, as_doc: A) -> MockOAuthServer
where
    P: Fn(&str) -> String + Send + Sync + 'static,
    A: Fn(&str) -> String + Send + Sync + 'static,
{
    MockOAuthServer::start(move |hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => json_response(200, "OK", &prm(origin)),
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_doc(origin))
        }
        _ => json_response(404, "Not Found", "{}"),
    })
}

/// Discovery plus an `/authorize` that echoes `state` back and a token
/// endpoint that always replies with `token_response`.
fn flow_mock(token_response: String) -> MockOAuthServer {
    MockOAuthServer::start(move |hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_document(origin))
        }
        "/authorize" => redirect_302(&format!(
            "{}?code=mock-code&state={}",
            query_param(&hit.query, "redirect_uri"),
            query_param(&hit.query, "state"),
        )),
        "/token" => json_response(200, "OK", &token_response),
        _ => json_response(404, "Not Found", "{}"),
    })
}

/// Inputs that stop before the browser step (no loopback, no override).
fn base_inputs(origin: &str) -> OAuthFlowInputs {
    OAuthFlowInputs {
        mcp_server_url: format!("{origin}/mcp"),
        www_authenticate: None,
        extra_scopes: vec![],
        timeout: Some(Duration::from_secs(5)),
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "test-client".to_owned(),
            client_secret: None,
        }),
        loopback_override: None,
        browser_override: None,
    }
}

/// A "browser" that follows the mock's 302 so the loopback captures the code.
fn redirect_browser() -> BrowserOverride {
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
        let _ = resp.text();
        Ok(())
    })
}

/// A loopback callback server with a short wait so failure paths stay fast.
fn quick_loopback(timeout: Duration) -> LoopbackServer {
    start(Some("/cb"), Some(timeout)).expect("start loopback")
}

/// A stored token with a refresh token, ready to be refreshed.
fn stored_token(origin: &str) -> StoredToken {
    StoredToken {
        access_token: "expired".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_at: Some(0),
        refresh_token: Some("rt-1".to_owned()),
        scope: vec!["read".to_owned()],
        client_id: Some("stored-client".to_owned()),
        issuer: Some(format!("{origin}/tenant1")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// run_flow — success paths
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_flow_completes_and_caches_the_token() {
    let server = flow_mock(
        r#"{"access_token":"fresh","token_type":"Bearer","expires_in":3600,"refresh_token":"rt-2","scope":"read"}"#
            .to_owned(),
    );
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };

    let output = run_flow(&inputs, &store).expect("flow should succeed");
    assert_eq!(output.token.access_token, "fresh");
    assert_eq!(output.client_id, "test-client");
    assert_eq!(output.issuer, format!("{origin}/tenant1"));
    assert_eq!(output.token.refresh_token.as_deref(), Some("rt-2"));
    assert!(
        store
            .get(&format!("{origin}/mcp"))
            .is_some_and(|t| t.access_token == "fresh"),
        "the minted token must be cached under the resource URI"
    );

    let token_hit = server.only("POST", "/token");
    assert!(
        token_hit.body.contains("grant_type=authorization_code"),
        "got: {}",
        token_hit.body
    );
    assert!(
        token_hit.body.contains("code=mock-code"),
        "got: {}",
        token_hit.body
    );
    assert!(
        token_hit.body.contains("code_verifier="),
        "got: {}",
        token_hit.body
    );
    assert!(
        token_hit.body.contains("client_id=test-client"),
        "got: {}",
        token_hit.body
    );
    assert!(
        !token_hit.body.contains("client_secret="),
        "a public client must not send a secret, got: {}",
        token_hit.body
    );
    assert!(
        token_hit.body.contains("resource=http%3A%2F%2F127.0.0.1"),
        "MCP-013: the resource param must be present, got: {}",
        token_hit.body
    );
    assert!(
        token_hit
            .body
            .contains("redirect_uri=http%3A%2F%2F127.0.0.1"),
        "got: {}",
        token_hit.body
    );
}

#[test]
fn run_flow_sends_the_secret_for_confidential_clients() {
    let server =
        flow_mock(r#"{"access_token":"conf","token_type":"Bearer","expires_in":60}"#.to_owned());
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "conf-client".to_owned(),
            client_secret: Some("s/secret".to_owned()),
        }),
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };

    run_flow(&inputs, &store).expect("flow should succeed");
    let body = server.only("POST", "/token").body;
    assert!(body.contains("client_id=conf-client"), "got: {body}");
    assert!(
        body.contains("client_secret=s%2Fsecret"),
        "the secret must be form-encoded, got: {body}"
    );
}

#[test]
fn run_flow_registers_a_client_dynamically_when_none_is_pre_registered() {
    let server = MockOAuthServer::start(|hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => json_response(
            200,
            "OK",
            &as_document_full(
                origin,
                &format!("{origin}/authorize"),
                &format!("{origin}/token"),
                r#"["S256"]"#,
                Some(&format!("{origin}/register")),
            ),
        ),
        "/register" => json_response(
            201,
            "Created",
            r#"{"client_id":"dyn-1","client_secret":"dyn-secret"}"#,
        ),
        "/authorize" => redirect_302(&format!(
            "{}?code=dcr-code&state={}",
            query_param(&hit.query, "redirect_uri"),
            query_param(&hit.query, "state"),
        )),
        "/token" => json_response(
            200,
            "OK",
            r#"{"access_token":"dcr-token","token_type":"Bearer","expires_in":60}"#,
        ),
        _ => json_response(404, "Not Found", "{}"),
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        pre_registered_client: None,
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };

    let output = run_flow(&inputs, &store).expect("DCR flow should succeed");
    assert_eq!(output.client_id, "dyn-1");
    assert_eq!(output.token.access_token, "dcr-token");

    let register_hit = server.only("POST", "/register");
    assert!(
        register_hit.body.contains("redirect_uris"),
        "got: {}",
        register_hit.body
    );
    assert!(
        register_hit
            .body
            .contains("\"token_endpoint_auth_method\":\"none\""),
        "got: {}",
        register_hit.body
    );
    let token_body = server.only("POST", "/token").body;
    assert!(token_body.contains("client_id=dyn-1"), "got: {token_body}");
    assert!(
        token_body.contains("client_secret=dyn-secret"),
        "got: {token_body}"
    );
}

#[test]
fn run_flow_requests_the_advertised_scope() {
    let server = flow_mock(
        r#"{"access_token":"scoped","token_type":"Bearer","expires_in":60,"scope":"read"}"#
            .to_owned(),
    );
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        extra_scopes: vec!["write".to_owned()],
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };

    run_flow(&inputs, &store).expect("flow should succeed");
    let authorize = server.only("GET", "/authorize");
    // `build_authorization_url` encodes every parameter with `percent_encode`,
    // which implements the `application/x-www-form-urlencoded` rule that RFC
    // 6749 §3.1 points at: a space becomes `+`, not `%20`. `pick_scope` emits
    // the advertised `scopes_supported` first ("read") and appends the
    // de-duplicated `extra_scopes` ("write"), and `scope` is the last parameter
    // of the URL.
    assert!(
        authorize.query.contains("scope=read+write"),
        "the union of advertised and extra scopes must be requested with the \
         form encoding (space as '+'), got: {}",
        authorize.query
    );
    assert!(
        !authorize.query.contains("scope=read%20write"),
        "the builder emits '+' for spaces, never '%20', got: {}",
        authorize.query
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// run_flow — discovery and spec guards
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_flow_refuses_when_pkce_s256_is_not_advertised() {
    let server = discovery_mock(prm_document, |origin: &str| {
        as_document_full(
            origin,
            &format!("{origin}/authorize"),
            &format!("{origin}/token"),
            r#"["plain"]"#,
            None,
        )
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("S256 is mandatory");
    assert!(matches!(err, OAuthError::PkceNotSupported), "got: {err:?}");
}

#[test]
fn run_flow_rejects_an_insecure_endpoint_in_the_as_document() {
    // RFC 8414 §4.9 forbids plain http endpoints; `{origin}` is replaced with
    // the mock's own origin so the other endpoint keeps pointing at the mock.
    let cases = [
        (
            "the authorization endpoint",
            "http://evil.example.com/authorize",
            "{origin}/token",
            "authorization endpoint",
        ),
        (
            "the token endpoint",
            "{origin}/authorize",
            "http://evil.example.com/token",
            "token endpoint",
        ),
    ];
    for (label, authorize, token, expected) in cases {
        let server = discovery_mock(prm_document, move |origin: &str| {
            as_document_full(
                origin,
                &authorize.replace("{origin}", origin),
                &token.replace("{origin}", origin),
                r#"["S256"]"#,
                None,
            )
        });
        let origin = server.url("");
        let store = TokenStore::in_memory();

        // `unwrap_or_else` would hand back the *success* value, so the error is
        // taken with an explicit match to keep the actual output in the message.
        let err = match run_flow(&base_inputs(&origin), &store) {
            Ok(ok) => panic!("case '{label}': plain http must be refused, got {ok:?}"),
            Err(err) => err,
        };
        match err {
            OAuthError::SpecViolation(msg) => {
                assert!(msg.contains(expected), "case '{label}': got: {msg}");
            }
            other => panic!("case '{label}': expected SpecViolation, got {other:?}"),
        }
    }
}

#[test]
fn run_flow_reports_unreachable_resource_metadata() {
    let server = MockOAuthServer::start(|_hit, _origin| json_response(404, "Not Found", "{}"));
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("no metadata anywhere");
    match err {
        OAuthError::ResourceMetadata(msg) => {
            assert!(
                msg.contains("could not fetch Protected Resource Metadata"),
                "got: {msg}"
            );
            assert!(
                msg.contains("/.well-known/oauth-protected-resource/mcp"),
                "both candidates must be reported, got: {msg}"
            );
        }
        other => panic!("expected ResourceMetadata, got {other:?}"),
    }
}

#[test]
fn run_flow_reports_resource_metadata_without_authorization_servers() {
    let server = discovery_mock(
        |origin: &str| format!(r#"{{"resource":"{origin}/mcp","authorization_servers":[]}}"#),
        |_origin: &str| String::new(),
    );
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("no issuer advertised");
    match err {
        OAuthError::ResourceMetadata(msg) => {
            assert!(msg.contains("no authorization_servers"), "got: {msg}");
        }
        other => panic!("expected ResourceMetadata, got {other:?}"),
    }
}

#[test]
fn run_flow_reports_an_unreachable_authorization_server() {
    let server = MockOAuthServer::start(|hit, origin| {
        if hit.path == "/.well-known/oauth-protected-resource/mcp" {
            json_response(200, "OK", &prm_document(origin))
        } else {
            json_response(404, "Not Found", "{}")
        }
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("no AS metadata");
    match err {
        OAuthError::AuthorizationServerMetadata(msg) => {
            assert!(
                msg.contains("no well-known metadata document"),
                "got: {msg}"
            );
            assert!(
                msg.contains("last_error: <none>"),
                "404s are not an error, got: {msg}"
            );
        }
        other => panic!("expected AuthorizationServerMetadata, got {other:?}"),
    }
}

#[test]
fn run_flow_reports_invalid_authorization_server_metadata() {
    let server = MockOAuthServer::start(|hit, origin| {
        if hit.path == "/.well-known/oauth-protected-resource/mcp" {
            json_response(200, "OK", &prm_document(origin))
        } else {
            json_response(200, "OK", r#"{"not":"metadata"}"#)
        }
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("unparsable AS document");
    match err {
        OAuthError::AuthorizationServerMetadata(msg) => {
            assert!(
                msg.contains("last_error: OAuth protocol error"),
                "the parse failure must be reported, got: {msg}"
            );
        }
        other => panic!("expected AuthorizationServerMetadata, got {other:?}"),
    }
}

#[test]
fn run_flow_reports_a_non_json_metadata_response() {
    let server = MockOAuthServer::start(|hit, _origin| {
        if hit.path == "/.well-known/oauth-protected-resource/mcp" {
            response(200, "OK", "text/html", "<html>nope</html>")
        } else {
            json_response(404, "Not Found", "{}")
        }
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = run_flow(&base_inputs(&origin), &store).expect_err("html is not metadata");
    assert!(
        matches!(err, OAuthError::ResourceMetadata(_)),
        "got: {err:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// run_flow — callback and token-exchange failures
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_flow_detects_a_state_mismatch() {
    let server = MockOAuthServer::start(|hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_document(origin))
        }
        "/authorize" => redirect_302(&format!(
            "{}?code=mock-code&state=forged-state",
            query_param(&hit.query, "redirect_uri")
        )),
        _ => json_response(404, "Not Found", "{}"),
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };

    let err = run_flow(&inputs, &store).expect_err("the state must match");
    assert!(matches!(err, OAuthError::StateMismatch), "got: {err:?}");
    assert!(
        server.hits().iter().all(|h| h.path != "/token"),
        "the code must never be exchanged on a state mismatch"
    );
}

#[test]
fn run_flow_times_out_when_no_callback_arrives_whatever_the_browser_does() {
    let server =
        flow_mock(r#"{"access_token":"unused","token_type":"Bearer","expires_in":60}"#.to_owned());
    let origin = server.url("");
    let store = TokenStore::in_memory();
    // A silent "browser" never triggers the loopback, and a failing browser is
    // only logged: the flow still waits for a manual callback, so the
    // observable failure is the callback timeout in both cases.
    let silent_browser: BrowserOverride =
        Arc::new(|_url: &str| -> Result<(), OAuthError> { Ok(()) });
    let broken_browser: BrowserOverride =
        Arc::new(|_url: &str| -> Result<(), OAuthError> { Err(OAuthError::UserCancelled) });
    let cases = [
        ("a browser that does nothing", silent_browser),
        ("a browser that fails", broken_browser),
    ];

    for (label, browser) in cases {
        let inputs = OAuthFlowInputs {
            loopback_override: Some(quick_loopback(Duration::from_millis(150))),
            browser_override: Some(browser),
            ..base_inputs(&origin)
        };

        let err = match run_flow(&inputs, &store) {
            Ok(ok) => panic!("case '{label}': expected no callback, got {ok:?}"),
            Err(err) => err,
        };
        assert!(
            matches!(err, OAuthError::CallbackTimeout(_)),
            "case '{label}': got: {err:?}"
        );
    }
}

/// A mock whose `/token` endpoint answers with a fixed response.
fn token_endpoint_mock(
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> MockOAuthServer {
    let canned = response(status, reason, content_type, body);
    MockOAuthServer::start(move |hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_document(origin))
        }
        "/authorize" => redirect_302(&format!(
            "{}?code=mock-code&state={}",
            query_param(&hit.query, "redirect_uri"),
            query_param(&hit.query, "state"),
        )),
        "/token" => canned.clone(),
        _ => json_response(404, "Not Found", "{}"),
    })
}

/// Run a flow against `server` and return the error it produced.
fn expect_flow_error(server: &MockOAuthServer) -> OAuthError {
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let inputs = OAuthFlowInputs {
        loopback_override: Some(quick_loopback(Duration::from_secs(5))),
        browser_override: Some(redirect_browser()),
        ..base_inputs(&origin)
    };
    run_flow(&inputs, &store).expect_err("the token endpoint is configured to fail")
}

/// The error shape a token-endpoint case must produce, with the fragments the
/// error payload must mention.
#[derive(Clone, Copy)]
enum ExpectedError<'a> {
    /// An OAuth error envelope: the endpoint name, the `error` code and, when
    /// the case pins it, the `error_description`.
    OAuth {
        endpoint: &'a str,
        error: &'a str,
        description: Option<&'a str>,
    },
    /// A transport failure whose message must contain every fragment.
    Transport(&'a [&'a str]),
    /// A protocol failure whose message must contain every fragment.
    Protocol(&'a [&'a str]),
}

/// One canned `/token` response and the error `run_flow` must report for it.
struct TokenCase<'a> {
    label: &'a str,
    status: u16,
    reason: &'a str,
    content_type: &'a str,
    body: &'a str,
    expected: ExpectedError<'a>,
}

/// Assert a token-endpoint case table: each canned response is served by the
/// mock and `run_flow` must fail in the expected way.
fn assert_token_errors(cases: Vec<TokenCase<'_>>) {
    for case in cases {
        let server = token_endpoint_mock(case.status, case.reason, case.content_type, case.body);
        let err = expect_flow_error(&server);
        match (err, case.expected) {
            (
                OAuthError::OAuth { endpoint, body },
                ExpectedError::OAuth {
                    endpoint: expected_endpoint,
                    error,
                    description,
                },
            ) => {
                assert_eq!(endpoint, expected_endpoint, "case '{}'", case.label);
                assert_eq!(body.error, error, "case '{}'", case.label);
                if let Some(expected) = description {
                    assert_eq!(
                        body.error_description.as_deref(),
                        Some(expected),
                        "case '{}'",
                        case.label
                    );
                }
            }
            (OAuthError::Transport(msg), ExpectedError::Transport(fragments))
            | (OAuthError::Protocol(msg), ExpectedError::Protocol(fragments)) => {
                for fragment in fragments {
                    assert!(msg.contains(fragment), "case '{}': got: {msg}", case.label);
                }
            }
            (other, _) => panic!("case '{}': unexpected error shape: {other:?}", case.label),
        }
    }
}

#[test]
fn run_flow_reports_every_token_endpoint_failure_shape() {
    assert_token_errors(vec![
        TokenCase {
            label: "an error envelope with a description",
            status: 400,
            reason: "Bad Request",
            content_type: "application/json",
            body: r#"{"error":"invalid_grant","error_description":"code expired"}"#,
            expected: ExpectedError::OAuth {
                endpoint: "token",
                error: "invalid_grant",
                description: Some("code expired"),
            },
        },
        TokenCase {
            label: "a plain-text failure body",
            status: 403,
            reason: "Forbidden",
            content_type: "text/plain",
            body: "no token for you",
            expected: ExpectedError::Transport(&[
                "HTTP 403",
                "no token for you",
                "grant=authorization_code",
            ]),
        },
        TokenCase {
            label: "malformed token json",
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: "{\"access_token\":",
            expected: ExpectedError::Protocol(&["token response json"]),
        },
        TokenCase {
            label: "an unexpected token shape",
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: r#"{"ok":true}"#,
            expected: ExpectedError::Protocol(&["token response shape"]),
        },
        TokenCase {
            label: "an error envelope returned with 200",
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: r#"{"error":"invalid_request","error_description":"missing code"}"#,
            expected: ExpectedError::OAuth {
                endpoint: "token",
                error: "invalid_request",
                description: None,
            },
        },
        TokenCase {
            label: "a non-Bearer token_type",
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: r#"{"access_token":"x","token_type":"MAC","expires_in":60}"#,
            expected: ExpectedError::Protocol(&["token_type 'MAC'"]),
        },
    ]);
}

// ─────────────────────────────────────────────────────────────────────────────
// refresh
// ─────────────────────────────────────────────────────────────────────────────

/// Discovery plus a token endpoint that mints `fresh-2`.
fn refresh_mock() -> MockOAuthServer {
    MockOAuthServer::start(|hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_document(origin))
        }
        "/token" => json_response(
            200,
            "OK",
            r#"{"access_token":"fresh-2","token_type":"Bearer","expires_in":120,"refresh_token":"rt-2","scope":"read write"}"#,
        ),
        _ => json_response(404, "Not Found", "{}"),
    })
}

#[test]
fn refresh_exchanges_the_stored_refresh_token() {
    let server = refresh_mock();
    let origin = server.url("");
    let resource = format!("{origin}/mcp");
    let store = TokenStore::in_memory();
    let existing = stored_token(&origin);
    store.put(&resource, existing.clone());

    let output = refresh(&base_inputs(&origin), &store, &existing).expect("refresh should succeed");
    assert_eq!(output.token.access_token, "fresh-2");
    assert_eq!(output.client_id, "stored-client");
    assert_eq!(output.issuer, format!("{origin}/tenant1"));
    assert_eq!(
        store
            .get(&resource)
            .expect("refreshed token must be cached")
            .refresh_token
            .as_deref(),
        Some("rt-2")
    );

    let body = server.only("POST", "/token").body;
    assert!(body.contains("grant_type=refresh_token"), "got: {body}");
    assert!(body.contains("refresh_token=rt-1"), "got: {body}");
    assert!(body.contains("client_id=stored-client"), "got: {body}");
    assert!(
        !body.contains("scope="),
        "no scope param when no extra scopes were requested, got: {body}"
    );
    assert!(
        body.contains("resource=http%3A%2F%2F127.0.0.1"),
        "got: {body}"
    );
}

#[test]
fn refresh_uses_the_pre_registered_secret_when_ids_match() {
    let server = refresh_mock();
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let existing = stored_token(&origin);
    let inputs = OAuthFlowInputs {
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "stored-client".to_owned(),
            client_secret: Some("shh".to_owned()),
        }),
        extra_scopes: vec!["admin".to_owned()],
        ..base_inputs(&origin)
    };

    refresh(&inputs, &store, &existing).expect("refresh should succeed");
    let body = server.only("POST", "/token").body;
    assert!(body.contains("client_secret=shh"), "got: {body}");
    assert!(body.contains("scope=admin"), "got: {body}");
}

#[test]
fn refresh_ignores_a_pre_registered_client_with_a_different_id() {
    let server = refresh_mock();
    let origin = server.url("");
    let store = TokenStore::in_memory();
    let existing = stored_token(&origin);
    let inputs = OAuthFlowInputs {
        pre_registered_client: Some(PreRegisteredClient {
            client_id: "someone-else".to_owned(),
            client_secret: Some("shh".to_owned()),
        }),
        ..base_inputs(&origin)
    };

    let output = refresh(&inputs, &store, &existing).expect("refresh should succeed");
    assert_eq!(
        output.client_id, "stored-client",
        "the stored client id must win"
    );
    let body = server.only("POST", "/token").body;
    assert!(
        !body.contains("client_secret="),
        "the mismatched secret must not be sent, got: {body}"
    );
}

#[test]
fn refresh_requires_a_client_id_and_an_issuer_on_the_stored_token() {
    let server = refresh_mock();
    let origin = server.url("");
    let store = TokenStore::in_memory();
    // Each label doubles as the fragment the error must name.
    let cases = [
        (
            "no client_id",
            StoredToken {
                client_id: None,
                ..stored_token(&origin)
            },
        ),
        (
            "no issuer",
            StoredToken {
                issuer: None,
                ..stored_token(&origin)
            },
        ),
    ];

    for (expected, existing) in cases {
        let err = match refresh(&base_inputs(&origin), &store, &existing) {
            Ok(ok) => {
                panic!("a stored token with {expected} cannot be refreshed, got: {ok:?}")
            }
            Err(err) => err,
        };
        match err {
            OAuthError::RefreshFailed(msg) => {
                assert!(msg.contains(expected), "case '{expected}': got: {msg}");
            }
            other => panic!("case '{expected}': expected RefreshFailed, got {other:?}"),
        }
    }
}

#[test]
fn refresh_maps_a_token_endpoint_error() {
    let server = MockOAuthServer::start(|hit: &Hit, origin| match hit.path.as_str() {
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(200, "OK", &prm_document(origin))
        }
        "/.well-known/oauth-authorization-server/tenant1" => {
            json_response(200, "OK", &as_document(origin))
        }
        "/token" => json_response(400, "Bad Request", r#"{"error":"invalid_grant"}"#),
        _ => json_response(404, "Not Found", "{}"),
    });
    let origin = server.url("");
    let store = TokenStore::in_memory();

    let err = refresh(&base_inputs(&origin), &store, &stored_token(&origin))
        .expect_err("the refresh token is revoked");
    match err {
        OAuthError::OAuth { endpoint, body } => {
            assert_eq!(endpoint, "token");
            assert_eq!(body.error, "invalid_grant");
        }
        other => panic!("expected an OAuth envelope, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OAuthFlowInputs plumbing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn flow_inputs_debug_hides_the_browser_override_closure() {
    let plain = base_inputs("https://mcp.example.com");
    let text = format!("{plain:?}");
    assert!(text.contains("OAuthFlowInputs"), "got: {text}");
    assert!(text.contains("browser_override: None"), "got: {text}");

    let with_browser = OAuthFlowInputs {
        browser_override: Some(redirect_browser()),
        ..base_inputs("https://mcp.example.com")
    };
    assert!(
        format!("{with_browser:?}").contains("<fn>"),
        "the closure must be rendered as a placeholder"
    );
}

#[test]
fn flow_inputs_clone_copies_every_field() {
    let original = OAuthFlowInputs {
        extra_scopes: vec!["read".to_owned()],
        browser_override: Some(redirect_browser()),
        ..base_inputs("https://mcp.example.com")
    };
    let cloned = original.clone();

    assert_eq!(cloned.mcp_server_url, original.mcp_server_url);
    assert_eq!(cloned.extra_scopes, original.extra_scopes);
    assert_eq!(cloned.timeout, original.timeout);
    assert!(cloned.browser_override.is_some());
    assert!(cloned.loopback_override.is_none());
    assert_eq!(
        cloned
            .pre_registered_client
            .as_ref()
            .map(|c| c.client_id.as_str()),
        Some("test-client")
    );
}
