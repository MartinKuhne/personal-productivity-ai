//! Tests for [`McpClientSession`] (`agent::lib::mcp::session`).
//! Unit tests for the stdio MCP subprocess transport.
//!
//! The "no visible console window" assertion cannot be observed
//! programmatically without Win32 FFI (the std API exposes a
//! [`CommandExt::creation_flags`] setter but no public getter).
//! Per `AGENTS.md` §10 ("If the issue cannot be reproduced at
//! the unit or integration level ... document why ... and add
//! the closest possible deterministic test"), we cover the
//! helper's contract — program/args/env/stdio set correctly and
//! the command is spawnable — and rely on a manual visual check
//! on Windows for the actual no-window behaviour. The
//! `cmd.creation_flags(CREATE_NO_WINDOW)` line itself is the
//! contract; see `doc/adr/cmd-substitution.md` for rationale.

use super::*;
use std::collections::HashMap;

#[test]
fn build_stdio_command_sets_program_args_env_and_pipes_stdio() {
    let env: HashMap<String, String> =
        std::iter::once(("FASTMD_MCP_TEST_VAR".to_string(), "hello".to_string())).collect();
    let args = vec!["--version".to_string()];
    let cmd = build_stdio_command("cargo", &args, &env);

    assert_eq!(cmd.get_program(), "cargo");
    let got_args: Vec<std::ffi::OsString> = cmd.get_args().map(std::ffi::OsString::from).collect();
    assert_eq!(
        got_args,
        vec![std::ffi::OsString::from("--version")],
        "args should pass through verbatim",
    );

    // The env we explicitly set must be present. Other entries
    // from the inherited environment are also expected and we
    // don't enumerate them.
    let envs: Vec<(String, String)> = cmd
        .get_envs()
        .filter_map(|(k, v)| {
            // `Command::get_envs` yields `(&OsStr, Option<&OsStr>)`;
            // `v` is `None` for an explicit unset. Apply `?` only
            // to the outer Option so the closure returns
            // `Option<(String, String)>` and `filter_map` drops
            // entries with non-UTF-8 keys or unset values.
            let key = k.to_str()?.to_owned();
            let val = v?.to_str()?.to_owned();
            Some((key, val))
        })
        .collect();
    assert!(
        envs.iter()
            .any(|(k, v)| k == "FASTMD_MCP_TEST_VAR" && v == "hello"),
        "env override should be present, got {envs:?}",
    );
}

#[test]
fn build_stdio_command_produces_a_spawnable_command() {
    // `cargo --version` is available on every CI/dev environment
    // for this Rust project and exits quickly with a known banner.
    // It is the closest portable "long-lived enough to spawn and
    // reap" smoke target without taking a hard dependency on a
    // particular shell.
    let mut cmd = build_stdio_command("cargo", &["--version".to_string()], &HashMap::new());
    let output = cmd
        .output()
        .expect("cargo --version should spawn successfully");
    assert!(
        output.status.success(),
        "cargo --version should exit 0; got {:?}",
        output.status,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cargo"),
        "cargo --version should print a 'cargo' banner; got: {stdout}",
    );
}

#[test]
fn build_stdio_command_is_idempotent_under_repeated_calls() {
    // Defends against a future refactor that accidentally caches
    // or mutates a shared Command between calls. Each call must
    // return a fresh Command with the requested program/args.
    let env = HashMap::new();
    let cmd_a = build_stdio_command("cargo", &["--version".to_string()], &env);
    let cmd_b = build_stdio_command("node", &[], &env);
    assert_eq!(cmd_a.get_program(), "cargo");
    assert_eq!(cmd_b.get_program(), "node");
}

#[test]
fn is_valid_session_id_accepts_visible_ascii() {
    let cases = [
        ("abc123-XYZ_~", true),
        ("!\"#$%&'()*+,-./012", true),
        ("", false),
        ("has space", false),
        ("has\tdelimiter", false),
        ("has\u{7f}del", false),
    ];
    for (id, expected) in cases {
        assert_eq!(is_valid_session_id(id), expected, "id '{id}'");
    }
}

#[test]
fn redact_session_id_shows_len_and_prefix() {
    let cases = [
        ("abcdefgh", "len=8:abcd"),
        ("ab", "len=2:ab"),
        ("", "len=0:"),
    ];
    for (id, expected) in cases {
        assert_eq!(redact_session_id(id), expected, "id '{id}'");
    }
}

#[test]
fn extract_result_success_and_error_branches() {
    let ok = serde_json::json!({"jsonrpc":"2.0","result":{"x":1},"id":1});
    assert_eq!(
        McpClientSession::extract_result("srv", "m", ok).unwrap(),
        serde_json::json!({"x":1})
    );
    let bad_jsonrpc = serde_json::json!({"result":{}});
    assert!(
        McpClientSession::extract_result("srv", "m", bad_jsonrpc)
            .unwrap_err()
            .to_string()
            .contains("not a JSON-RPC")
    );
    let with_error = serde_json::json!({"jsonrpc":"2.0","error":{"code":-32600,"message":"oops"}});
    let err = McpClientSession::extract_result("srv", "m", with_error).unwrap_err();
    assert!(err.to_string().contains("oops"));
    let missing_both = serde_json::json!({"jsonrpc":"2.0","id":1});
    assert!(
        McpClientSession::extract_result("srv", "m", missing_both)
            .unwrap_err()
            .to_string()
            .contains("neither result nor error")
    );
}

#[test]
fn session_pure_helpers_resource_uri_and_auth() {
    let sse = McpServerConfig::Sse {
        url: "https://example.com/mcp".to_string(),
        headers: HashMap::new(),
        oauth: None,
    };
    let sess = McpClientSession::new(sse, None);
    assert_eq!(
        sess.resource_uri().as_deref(),
        Some("https://example.com/mcp")
    );
    assert!(!sess.has_static_authorization());
    assert!(sess.oauth_config_scopes().is_empty());
    assert_eq!(sess.config_label(), "http:https://example.com/mcp");
    // case-insensitive Authorization header
    let mut h = HashMap::new();
    h.insert("Authorization".to_string(), "Bearer x".to_string());
    let sse_auth = McpServerConfig::Sse {
        url: "https://example.com/mcp".to_string(),
        headers: h,
        oauth: None,
    };
    let sess2 = McpClientSession::new(sse_auth, None);
    assert!(sess2.has_static_authorization());
    let h2: HashMap<String, String> = [("authorization".to_string(), "Bearer y".to_string())]
        .into_iter()
        .collect();
    let sse_auth2 = McpServerConfig::Sse {
        url: "https://example.com/mcp".to_string(),
        headers: h2,
        oauth: None,
    };
    let sess3 = McpClientSession::new(sse_auth2, None);
    assert!(sess3.has_static_authorization(), "case-insensitive");
    let stdio = McpServerConfig::Stdio {
        command: "echo".to_string(),
        args: vec![],
        env: HashMap::new(),
    };
    let sess_stdio = McpClientSession::new(stdio, None);
    assert_eq!(sess_stdio.resource_uri(), None);
    assert_eq!(sess_stdio.config_label(), "stdio:echo");
    assert!(!sess_stdio.has_static_authorization());
}

#[test]
fn take_unauthorized_flag_resets() {
    let sse = McpServerConfig::Sse {
        url: "https://example.com/mcp".to_string(),
        headers: HashMap::new(),
        oauth: None,
    };
    let sess = McpClientSession::new(sse, None);
    assert!(!sess.take_unauthorized_flag());
    {
        let mut st = sess.state.lock().unwrap();
        st.last_call_saw_unauthorized = true;
    }
    assert!(sess.take_unauthorized_flag());
    assert!(!sess.take_unauthorized_flag(), "reset after take");
}

#[test]
fn protocol_version_and_server_info_initially_none() {
    let sse = McpServerConfig::Sse {
        url: "https://example.com/mcp".to_string(),
        headers: HashMap::new(),
        oauth: None,
    };
    let sess = McpClientSession::new(sse, None);
    assert!(sess.protocol_version().is_none());
    assert!(sess.server_capabilities().is_none());
    assert!(sess.server_info().is_none());
}

#[test]
fn wait_for_exit_returns_true_early_on_fast_process() {
    let mut cmd = build_stdio_command("cargo", &["--version".to_string()], &HashMap::new());
    let mut child = cmd.spawn().expect("spawn cargo --version");
    let start = std::time::Instant::now();
    let exited = wait_for_exit(&mut child, std::time::Duration::from_secs(1));
    let elapsed = start.elapsed();
    assert!(exited, "process should have exited cleanly");
    assert!(
        elapsed < std::time::Duration::from_millis(800),
        "should return as soon as child exits, not sleep full 1s; elapsed={elapsed:?}"
    );
}

#[test]
fn wait_for_exit_returns_false_when_child_keeps_running() {
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("ping");
    #[cfg(windows)]
    cmd.args(["127.0.0.1", "-n", "4"]);
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new("sleep");
    #[cfg(not(windows))]
    cmd.arg("4");
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn long-running process");
    let start = std::time::Instant::now();
    let exited = wait_for_exit(&mut child, std::time::Duration::from_millis(100));
    let elapsed = start.elapsed();
    assert!(!exited, "process should still be running");
    assert!(
        elapsed >= std::time::Duration::from_millis(90),
        "should poll until timeout; elapsed={elapsed:?}"
    );
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn shutdown_stdio_transport_fast_exit() {
    let mut cmd = build_stdio_command("cargo", &["--version".to_string()], &HashMap::new());
    let child = cmd.spawn().expect("spawn cargo --version");
    let line_rx = Arc::new(Mutex::new(std::sync::mpsc::channel().1));
    let transport = StdioTransport {
        child: Arc::new(Mutex::new(Some(child))),
        stdin: Arc::new(Mutex::new(None)),
        line_rx,
    };
    let start = std::time::Instant::now();
    McpClientSession::shutdown_stdio_transport(transport);
    assert!(
        start.elapsed() < std::time::Duration::from_millis(800),
        "clean exit should complete well before 1s"
    );
}

#[test]
fn shutdown_stdio_transport_times_out_and_terminates() {
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("ping");
    #[cfg(windows)]
    cmd.args(["127.0.0.1", "-n", "6"]);
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new("sleep");
    #[cfg(not(windows))]
    cmd.arg("6");
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    let child = cmd.spawn().expect("spawn sleep process");
    let line_rx = Arc::new(Mutex::new(std::sync::mpsc::channel().1));
    let transport = StdioTransport {
        child: Arc::new(Mutex::new(Some(child))),
        stdin: Arc::new(Mutex::new(None)),
        line_rx,
    };
    let start = std::time::Instant::now();
    McpClientSession::shutdown_stdio_transport(transport);
    let elapsed = start.elapsed();
    assert!(
        elapsed >= std::time::Duration::from_millis(950)
            && elapsed < std::time::Duration::from_millis(2500),
        "shutdown should wait ~1s then terminate immediately without waiting further; elapsed={elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// Loopback HTTP double for the Streamable-HTTP transport
// ---------------------------------------------------------------------------

/// A single request captured by [`MockHttp`].
#[derive(Clone, Debug)]
struct MockRequest {
    /// HTTP method from the request line (`POST`, `GET`, `DELETE`).
    method: String,
    /// Raw header block (request line and terminating blank line
    /// excluded), one `Name: value` per line.
    headers: String,
    /// Request body; empty when the client sent no `Content-Length`.
    body: String,
}

impl MockRequest {
    /// Case-insensitive header lookup over [`MockRequest::headers`].
    fn header(&self, name: &str) -> Option<String> {
        let prefix = format!("{}:", name.to_ascii_lowercase());
        self.headers.lines().find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .starts_with(&prefix)
                .then(|| line[prefix.len()..].trim().to_owned())
        })
    }
}

/// Canned reply served by [`MockHttp`].
struct MockReply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

impl MockReply {
    /// `200 OK` with a JSON body.
    fn json(body: String) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type", "application/json".to_owned())],
            body,
        }
    }

    /// `202 Accepted`, no body — the spec's answer to a notification.
    fn accepted() -> Self {
        Self {
            status: 202,
            headers: Vec::new(),
            body: String::new(),
        }
    }

    /// Arbitrary status / headers / body.
    fn with(status: u16, headers: Vec<(&'static str, String)>, body: impl Into<String>) -> Self {
        Self {
            status,
            headers,
            body: body.into(),
        }
    }
}

/// Reason phrase for the handful of statuses the tests emit. The client
/// only looks at the numeric code, so anything else can be generic.
fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Mock Status",
    }
}

/// Read one HTTP request from `stream`. Returns `None` on EOF or a
/// short read so the mock's accept loop can stop.
fn read_http_request(stream: &mut std::net::TcpStream) -> Option<MockRequest> {
    use std::io::{BufRead, Read};
    let mut reader = std::io::BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    let method = request_line
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).ok()?;
        if read == 0 || line.trim_end().is_empty() {
            break;
        }
        headers.push_str(&line);
    }
    let len = headers
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split_once(':').map(|(_, v)| v.trim()))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; len];
    if len > 0 {
        reader.read_exact(&mut body).ok()?;
    }
    Some(MockRequest {
        method,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Serialize a [`MockReply`] into the exact bytes written to the socket.
fn render_response(reply: &MockReply) -> String {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        reply.status,
        reason_phrase(reply.status)
    );
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        reply.body.len()
    ));
    head.push_str(&reply.body);
    head
}

/// Hand-rolled loopback HTTP server for the Streamable-HTTP transport.
///
/// Deliberately NOT the wiremock helper in `oauth/test_support.rs`: that
/// one keeps a current-thread tokio runtime `enter()`ed on the test
/// thread, which makes the `reqwest::blocking` calls used by
/// [`McpClientSession::http_session_delete`] and the OAuth flow panic.
/// A raw `TcpListener` on a plain OS thread carries no runtime, so both
/// the async and the blocking code paths can be driven from one harness.
struct MockHttp {
    url: String,
    requests: Arc<Mutex<Vec<MockRequest>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl MockHttp {
    /// Serve requests until [`MockHttp`] is dropped. `responder` sees the
    /// zero-based request index and the parsed request.
    fn spawn(responder: impl Fn(usize, &MockRequest) -> MockReply + Send + 'static) -> Self {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock MCP HTTP server");
        let port = listener.local_addr().expect("mock addr").port();
        listener
            .set_nonblocking(true)
            .expect("non-blocking mock listener");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut n = 0usize;
                loop {
                    if stop.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // Accepted sockets inherit non-blocking mode
                            // on Windows; the reader below expects a
                            // blocking socket.
                            let _ = stream.set_nonblocking(false);
                            let Some(req) = read_http_request(&mut stream) else {
                                break;
                            };
                            let reply = responder(n, &req);
                            requests.lock().expect("mock recorder").push(req);
                            n += 1;
                            let rendered = render_response(&reply);
                            let _ = stream.write_all(rendered.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(2));
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            requests,
            stop,
        }
    }

    /// Every request served so far, in arrival order.
    fn requests(&self) -> Vec<MockRequest> {
        self.requests.lock().expect("mock recorder").clone()
    }
}

impl Drop for MockHttp {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// JSON-RPC `method` of a request body, or `""` when it is not JSON.
fn rpc_method(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

/// Echo the request's `id` back with `result`.
fn rpc_ok(req: &MockRequest, result: serde_json::Value) -> MockReply {
    let id = serde_json::from_str::<serde_json::Value>(&req.body)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or_else(|| serde_json::json!(1));
    MockReply::json(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string())
}

/// A `protocolVersion` the client accepts.
fn init_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {},
        "serverInfo": { "name": "mock", "version": "0.1" },
    })
}

/// Session wired to an HTTP endpoint with OAuth disabled.
fn http_session(url: &str) -> McpClientSession {
    McpClientSession::new(
        McpServerConfig::Sse {
            url: url.to_owned(),
            headers: HashMap::new(),
            oauth: None,
        },
        None,
    )
}

/// Session wired to an HTTP endpoint with a static `Authorization` header.
fn http_session_with_auth(url: &str) -> McpClientSession {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer static".to_string());
    McpClientSession::new(
        McpServerConfig::Sse {
            url: url.to_owned(),
            headers,
            oauth: None,
        },
        None,
    )
}

#[tokio::test]
async fn http_handshake_captures_session_id_and_replays_headers() {
    let mock = MockHttp::spawn(|_n, req| match rpc_method(&req.body).as_str() {
        "initialize" => {
            let mut reply = rpc_ok(req, init_result());
            reply
                .headers
                .push(("MCP-Session-Id", "sess-abc".to_owned()));
            reply
        }
        "notifications/initialized" => MockReply::accepted(),
        _ => rpc_ok(req, serde_json::json!({})),
    });
    let sess = http_session(&mock.url);

    sess.ensure_initialized().await.expect("handshake succeeds");
    assert_eq!(
        sess.protocol_version().as_deref(),
        Some(PROTOCOL_VERSION),
        "negotiated protocol version is cached on the session"
    );
    assert_eq!(
        sess.server_capabilities(),
        Some(serde_json::json!({})),
        "capabilities are cached on the session"
    );
    sess.ping().await.expect("ping against a healthy mock");

    let reqs = mock.requests();
    assert_eq!(
        reqs.len(),
        3,
        "initialize + initialized notification + ping, got {:?}",
        reqs.iter().map(|r| rpc_method(&r.body)).collect::<Vec<_>>()
    );
    assert_eq!(reqs[0].method, "POST", "initialize is a POST");
    let init_body: serde_json::Value =
        serde_json::from_str(&reqs[0].body).expect("initialize body is JSON");
    // `protocolVersion` is an `initialize` *parameter*, so `do_initialize`
    // puts it inside the JSON-RPC `params` object; the envelope itself only
    // carries `jsonrpc`/`id`/`method`/`params`.
    assert_eq!(
        init_body["params"]["protocolVersion"],
        serde_json::json!(PROTOCOL_VERSION),
        "client advertises its own protocol version inside the initialize params"
    );
    assert_eq!(
        init_body.get("protocolVersion"),
        None,
        "the JSON-RPC envelope has no top-level protocolVersion, got: {}",
        reqs[0].body
    );
    assert!(
        reqs[1].body.contains("notifications/initialized"),
        "spec requires the initialized notification before any other request, got: {}",
        reqs[1].body
    );
    assert_eq!(
        reqs[0].header("mcp-session-id"),
        None,
        "no session id is sent on the handshake itself"
    );
    assert_eq!(
        reqs[2].header("mcp-session-id").as_deref(),
        Some("sess-abc"),
        "captured session id is replayed on later requests"
    );
    assert_eq!(
        reqs[2].header("mcp-protocol-version").as_deref(),
        Some(PROTOCOL_VERSION),
        "negotiated version is replayed on later requests"
    );
    assert!(
        reqs[2]
            .header("accept")
            .unwrap_or_default()
            .contains("text/event-stream"),
        "Accept must offer both JSON and SSE, got: {:?}",
        reqs[2].header("accept")
    );
    // Second call must not re-run the handshake.
    sess.ping().await.expect("second ping");
    assert_eq!(
        mock.requests().len(),
        4,
        "ensure_initialized is idempotent while the session is Active"
    );
}

#[tokio::test]
async fn http_init_with_unsupported_protocol_version_disconnects() {
    let mock = MockHttp::spawn(|_n, req| {
        if rpc_method(&req.body) == "notifications/initialized" {
            return MockReply::accepted();
        }
        rpc_ok(
            req,
            serde_json::json!({
                "protocolVersion": "2024-01-01",
                "capabilities": {},
                "serverInfo": { "name": "old-server" },
            }),
        )
    });
    let sess = http_session(&mock.url);

    let err = sess
        .ensure_initialized()
        .await
        .expect_err("an unsupported protocol version must be refused");
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported protocol version '2024-01-01'"),
        "error names the offending version: {msg}"
    );
    let state = sess.state.lock().expect("state lock");
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "session is reset so a later call can re-initialize"
    );
    assert!(
        state.session_id.is_none(),
        "no session id survives the disconnect"
    );
}

#[tokio::test]
async fn http_session_id_outside_visible_ascii_is_dropped() {
    let mock = MockHttp::spawn(|_n, req| {
        if rpc_method(&req.body) == "notifications/initialized" {
            return MockReply::accepted();
        }
        let mut reply = rpc_ok(req, init_result());
        reply
            .headers
            .push(("MCP-Session-Id", "bad id with space".to_owned()));
        reply
    });
    let sess = http_session(&mock.url);

    sess.ensure_initialized()
        .await
        .expect("handshake still succeeds; only the id is rejected");
    let state = sess.state.lock().expect("state lock");
    assert!(
        state.session_id.is_none(),
        "spec §3.4 session ids are 0x21..0x7E only; junk must not be cached"
    );
}

#[tokio::test]
async fn http_404_resets_session_and_probe_finds_legacy_endpoint() {
    let mock = MockHttp::spawn(|_n, req| match req.method.as_str() {
        "GET" => MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            "event: endpoint\ndata: /sse/123\n\n",
        ),
        _ => MockReply::with(
            404,
            vec![("Content-Type", "text/plain".to_owned())],
            "no such session",
        ),
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("a 404 endpoint cannot serve the modern transport");
    let msg = err.to_string();
    assert!(
        msg.contains("pre-2025-03 HTTP+SSE transport"),
        "the legacy transport must be named: {msg}"
    );
    assert!(
        msg.contains("/sse/123"),
        "the discovered endpoint URL is surfaced: {msg}"
    );
    let state = sess.state.lock().expect("state lock");
    assert!(
        state.session_id.is_none(),
        "a 404 drops the cached session id"
    );
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "a 404 resets the session for re-initialization"
    );
}

#[tokio::test]
async fn http_400_probe_get_with_unexpected_content_type_reports_both_codes() {
    let mock = MockHttp::spawn(|_n, req| match req.method.as_str() {
        "GET" => MockReply::with(
            200,
            vec![("Content-Type", "application/json".to_owned())],
            "{}",
        ),
        _ => MockReply::with(400, vec![], "bad request"),
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("transport negotiation must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("modern POST returned 400"),
        "the POST status is reported: {msg}"
    );
    assert!(
        msg.contains("probe GET returned 200 with unexpected Content-Type 'application/json'"),
        "the unexpected probe content type is reported: {msg}"
    );
}

#[tokio::test]
async fn http_405_probe_get_error_status_reports_both_codes() {
    let mock = MockHttp::spawn(|_n, req| match req.method.as_str() {
        "GET" => MockReply::with(500, vec![], "probe exploded"),
        _ => MockReply::with(405, vec![], "method not allowed"),
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("transport negotiation must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("modern POST returned 405") && msg.contains("probe GET returned 500"),
        "both statuses are reported: {msg}"
    );
    assert!(
        msg.contains("probe exploded"),
        "the probe body is included for diagnosis: {msg}"
    );
}

#[tokio::test]
async fn http_400_probe_get_without_endpoint_event_reports_negotiation_failure() {
    let mock = MockHttp::spawn(|_n, req| match req.method.as_str() {
        "GET" => MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            "event: message\ndata: {\"hello\":1}\n\n",
        ),
        _ => MockReply::with(400, vec![], "bad request"),
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("no endpoint event means no legacy transport either");
    let msg = err.to_string();
    assert!(
        msg.contains("no `event: endpoint` was found"),
        "the missing endpoint event is reported: {msg}"
    );
}

#[tokio::test]
async fn http_500_surfaces_plain_transport_error() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(500, vec![], "boom"));
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("a 5xx is an error");
    let msg = err.to_string();
    assert!(
        msg.contains("HTTP 500: boom"),
        "status and body are surfaced verbatim: {msg}"
    );
    assert!(
        msg.contains("initialize:"),
        "the failing method is annotated on the error: {msg}"
    );
}

#[tokio::test]
async fn http_200_with_invalid_json_is_reported() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::json("this is not json".to_owned()));
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("a non-JSON body cannot be a JSON-RPC response");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid JSON-RPC response"),
        "the parse failure is named: {msg}"
    );
    assert!(msg.contains("this is not json"), "body echoed: {msg}");
}

#[tokio::test]
async fn http_sse_response_is_walked_and_event_id_cached() {
    let mock = MockHttp::spawn(|_n, req| {
        let id = serde_json::from_str::<serde_json::Value>(&req.body)
            .ok()
            .and_then(|v| v.get("id").cloned())
            .unwrap_or_else(|| serde_json::json!(1));
        let result = if rpc_method(&req.body) == "initialize" {
            init_result()
        } else {
            serde_json::json!({})
        };
        let body = format!(
            "id: evt-1\nevent: message\ndata: {}\n\n",
            serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })
        );
        MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            body,
        )
    });
    let sess = http_session(&mock.url);

    let result = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect("an SSE-framed response is unwrapped to its result");
    assert_eq!(
        result,
        serde_json::json!({}),
        "the SSE walk returns the JSON-RPC result object"
    );
    let state = sess.state.lock().expect("state lock");
    assert_eq!(
        state.last_event_id.as_deref(),
        Some("evt-1"),
        "the SSE event id is cached for a future Last-Event-ID resume"
    );
}

#[tokio::test]
async fn http_sse_stream_without_matching_id_errors() {
    let mock = MockHttp::spawn(|_n, _req| {
        MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":99,\"result\":{}}\n\n".to_owned(),
        )
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("a response for another id is not ours");
    let msg = err.to_string();
    assert!(
        msg.contains("stream ended without a JSON-RPC response"),
        "the mismatch surfaces as a stream failure: {msg}"
    );
}

#[tokio::test]
async fn http_401_without_token_store_latches_unauthorized() {
    let mock = MockHttp::spawn(|_n, _req| {
        MockReply::with(
            401,
            vec![("WWW-Authenticate", "Bearer realm=\"mcp\"".to_owned())],
            "unauthorized",
        )
    });
    let sess = http_session(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("401 without a token store cannot be retried");
    assert!(
        err.to_string().contains("HTTP 401"),
        "the status is surfaced: {err}"
    );
    assert!(
        sess.take_unauthorized_flag(),
        "the manager must see the auth signal even with no token store"
    );
    assert!(
        !sess.take_unauthorized_flag(),
        "the flag is consumed by the first read"
    );
}

#[tokio::test]
async fn http_403_with_static_authorization_does_not_latch() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(403, vec![], "forbidden"));
    let sess = http_session_with_auth(&mock.url);

    let err = sess
        .call_request("ping", serde_json::json!({}))
        .await
        .expect_err("403 is an error");
    assert!(
        err.to_string().contains("HTTP 403"),
        "the status is surfaced: {err}"
    );
    assert!(
        !sess.take_unauthorized_flag(),
        "a static Authorization header means the operator opted out of OAuth"
    );
}

// ---------------------------------------------------------------------------
// Spec §3.5 backcompat probe (driven directly against the mock)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn probe_legacy_transport_reports_discovered_endpoint() {
    let mock = MockHttp::spawn(|_n, _req| {
        MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            "event: endpoint\ndata: /legacy/sse?sid=9\n\n".to_owned(),
        )
    });

    let err = McpClientSession::probe_legacy_transport(
        &mock.url,
        &HashMap::new(),
        405,
        "Method Not Allowed",
    )
    .await;
    let msg = err.to_string();
    assert!(
        msg.contains("pre-2025-03 HTTP+SSE transport"),
        "the legacy transport is named: {msg}"
    );
    assert!(
        msg.contains("/legacy/sse?sid=9"),
        "the endpoint URL from the SSE event is included: {msg}"
    );
}

#[tokio::test]
async fn probe_legacy_transport_ignores_empty_endpoint_events() {
    let mock = MockHttp::spawn(|_n, _req| {
        MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            "event: endpoint\ndata: \n\n".to_owned(),
        )
    });

    let err =
        McpClientSession::probe_legacy_transport(&mock.url, &HashMap::new(), 400, "bad").await;
    assert!(
        err.to_string().contains("no `event: endpoint` was found"),
        "an endpoint event with no data is not a usable endpoint: {err}"
    );
}

#[tokio::test]
async fn probe_legacy_transport_forwards_config_headers() {
    let mock = MockHttp::spawn(|_n, req| {
        let seen = req.header("x-trace").unwrap_or_default();
        MockReply::with(
            200,
            vec![("Content-Type", "text/event-stream".to_owned())],
            format!("event: endpoint\ndata: /ep?trace={seen}\n\n"),
        )
    });
    let mut headers = HashMap::new();
    headers.insert("X-Trace".to_string(), "abc".to_string());

    let err = McpClientSession::probe_legacy_transport(&mock.url, &headers, 404, "gone").await;
    let msg = err.to_string();
    assert!(
        msg.contains("/ep?trace=abc"),
        "configured headers are sent on the probe GET: {msg}"
    );
    assert_eq!(
        mock.requests()
            .first()
            .map(|r| r.method.as_str())
            .unwrap_or_default(),
        "GET",
        "the probe uses GET, not POST"
    );
}

// ---------------------------------------------------------------------------
// Spec §3.4 session DELETE
// ---------------------------------------------------------------------------

#[test]
fn http_session_delete_accepts_405_as_acknowledgement() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(405, vec![], "server-managed"));

    let result = McpClientSession::http_session_delete(&mock.url, &HashMap::new(), "sid-1");
    assert!(
        result.is_ok(),
        "405 means the server manages session lifetime: {:?}",
        result.err()
    );
    let requests = mock.requests();
    let req = requests.first().expect("one DELETE");
    assert_eq!(req.method, "DELETE", "spec §3.4 uses DELETE");
    assert_eq!(
        req.header("mcp-session-id").as_deref(),
        Some("sid-1"),
        "the session id header is sent"
    );
}

#[test]
fn http_session_delete_accepts_2xx_and_sends_config_headers() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(200, vec![], ""));
    let mut headers = HashMap::new();
    headers.insert("X-Api-Key".to_string(), "secret".to_string());

    let result = McpClientSession::http_session_delete(&mock.url, &headers, "sid-2");
    assert!(result.is_ok(), "2xx is success: {:?}", result.err());
    let requests = mock.requests();
    let req = requests.first().expect("one DELETE");
    assert_eq!(
        req.header("x-api-key").as_deref(),
        Some("secret"),
        "user headers are attached to the DELETE"
    );
    assert!(
        req.header("accept")
            .unwrap_or_default()
            .contains("text/event-stream"),
        "Accept offers both media types, got: {:?}",
        req.header("accept")
    );
}

#[test]
fn http_session_delete_reports_non_405_error_statuses() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(503, vec![], "unavailable"));

    let err = McpClientSession::http_session_delete(&mock.url, &HashMap::new(), "sid-3")
        .expect_err("5xx is not an acknowledgement");
    let msg = err.to_string();
    assert!(
        msg.contains("DELETE on shutdown returned HTTP 503: unavailable"),
        "status and body are reported: {msg}"
    );
}

#[test]
fn shutdown_drops_http_session_id_and_deletes_server_side() {
    let mock = MockHttp::spawn(|_n, _req| MockReply::with(405, vec![], "server-managed"));
    let sess = http_session(&mock.url);
    {
        let mut state = sess.state.lock().expect("state lock");
        state.session_id = Some("sid-shutdown".to_owned());
        state.phase = SessionPhase::Active;
    }

    sess.shutdown();
    let state = sess.state.lock().expect("state lock");
    assert!(
        state.session_id.is_none(),
        "the cached id is dropped so a re-init does not reuse a deleted session"
    );
    assert_eq!(
        state.phase,
        SessionPhase::Active,
        "shutdown only clears the HTTP session id, it does not reset the phase"
    );
    drop(state);
    // The DELETE runs on a detached thread (it uses the blocking client),
    // so poll briefly instead of asserting immediately. The loop normally
    // exits on the first or second iteration.
    let mut served = mock.requests().len();
    for _ in 0..20 {
        if served > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        served = mock.requests().len();
    }
    let reqs = mock.requests();
    assert_eq!(
        reqs.first().map(|r| r.method.as_str()),
        Some("DELETE"),
        "shutdown issues the spec §3.4 DELETE"
    );
    assert_eq!(
        reqs[0].header("mcp-session-id").as_deref(),
        Some("sid-shutdown"),
        "the DELETE carries the session id that is being closed"
    );
}

#[test]
fn shutdown_on_stdio_session_without_transport_is_a_noop() {
    let sess = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "echo".to_owned(),
            args: vec![],
            env: HashMap::new(),
        },
        None,
    );
    sess.shutdown();
    assert!(
        sess.state.lock().expect("state lock").stdio.is_none(),
        "nothing was spawned, so shutdown must not create a transport"
    );
}

// ---------------------------------------------------------------------------
// Transport guard arms and client builders
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_calls_for_stdio_config_are_rejected() {
    let sess = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "echo".to_owned(),
            args: vec![],
            env: HashMap::new(),
        },
        None,
    );
    let notification_err = sess
        .http_notification(&serde_json::json!({"jsonrpc": "2.0"}))
        .await
        .expect_err("stdio has no HTTP notification path");
    let request_err = sess
        .http_request_with_oauth(
            1,
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
            DEFAULT_REQUEST_TIMEOUT,
            Vec::new(),
            0,
        )
        .await
        .expect_err("stdio has no HTTP request path");

    for (label, err) in [
        ("http_notification", notification_err),
        ("http_request_with_oauth", request_err),
    ] {
        assert!(
            err.to_string()
                .contains("http transport requested for non-http server"),
            "call '{label}': the guard names the mismatch: {err}"
        );
    }
}

#[tokio::test]
async fn http_request_with_empty_url_is_rejected_before_connecting() {
    let sess = http_session("   ");
    let err = sess
        .http_request_with_oauth(
            1,
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
            DEFAULT_REQUEST_TIMEOUT,
            Vec::new(),
            0,
        )
        .await
        .expect_err("an empty endpoint cannot be dialed");
    assert!(
        err.to_string().contains("empty endpoint URL"),
        "the misconfiguration is named: {err}"
    );
}

/// Bind a loopback listener that accepts connections and answers nothing, so the
/// only way a request against it can fail is the client's own configured timeout.
/// The accept loop stops after [`SILENT_SERVER_ACCEPTS`] connections so the helper
/// thread cannot outlive the test binary.
fn silent_server() -> String {
    const SILENT_SERVER_ACCEPTS: usize = 8;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a loopback listener");
    let port = listener.local_addr().expect("listener local_addr").port();
    std::thread::spawn(move || {
        for _ in 0..SILENT_SERVER_ACCEPTS {
            if let Ok((stream, _peer)) = listener.accept() {
                // Hold the socket open without ever writing a response byte.
                std::thread::spawn(move || {
                    let _held = stream;
                    std::thread::sleep(std::time::Duration::from_secs(2));
                });
            }
        }
    });
    format!("http://127.0.0.1:{port}/")
}

#[tokio::test]
async fn mcp_client_applies_the_requested_timeout() {
    let url = silent_server();
    let client = mcp_client(std::time::Duration::from_millis(150));
    let err = match client.get(&url).send().await {
        Ok(resp) => panic!("a server that never answers must not succeed, got: {resp:?}"),
        Err(err) => err,
    };
    assert!(
        err.is_timeout(),
        "expected the configured request timeout to fire, got: {err:?}"
    );
}

#[test]
fn mcp_blocking_client_applies_the_requested_timeout() {
    let url = silent_server();
    let client = mcp_blocking_client(std::time::Duration::from_millis(150));
    let err = match client.get(&url).send() {
        Ok(resp) => panic!("a server that never answers must not succeed, got: {resp:?}"),
        Err(err) => err,
    };
    assert!(
        err.is_timeout(),
        "expected the configured request timeout to fire, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// tools/list pagination
// ---------------------------------------------------------------------------

/// Wrap a `tools/list` responder in a healthy init handshake. `make_page`
/// receives the zero-based page index (the handshake consumes the first
/// two requests) and the request it must answer.
/// A scripted responder for the mock HTTP server: given the zero-based request
/// index and the request, produce the reply.
type PageResponder = Box<dyn Fn(usize, &MockRequest) -> MockReply + Send + 'static>;

fn tools_server(
    make_page: impl Fn(usize, &MockRequest) -> MockReply + Send + 'static,
) -> PageResponder {
    Box::new(
        move |n: usize, req: &MockRequest| match rpc_method(&req.body).as_str() {
            "initialize" => rpc_ok(req, init_result()),
            "notifications/initialized" => MockReply::accepted(),
            _ => make_page(n.saturating_sub(2), req),
        },
    )
}

/// A `tools/list` result page, optionally advertising a `nextCursor`.
fn tools_page(req: &MockRequest, tools: serde_json::Value, next: Option<&str>) -> MockReply {
    let mut result = serde_json::json!({ "tools": tools });
    if let Some(cursor) = next {
        result["nextCursor"] = serde_json::json!(cursor);
    }
    rpc_ok(req, result)
}

#[tokio::test]
async fn list_tools_follows_pagination_up_to_the_page_cap() {
    let mock = MockHttp::spawn(tools_server(|n, req| {
        tools_page(
            req,
            serde_json::json!([{ "name": format!("tool-{n}") }]),
            Some("again"),
        )
    }));
    let sess = http_session(&mock.url);

    let tools = sess.list_tools().await.expect("paginated listing");
    assert_eq!(
        tools.len(),
        16,
        "the client follows at most MAX_PAGES (16) pages before truncating"
    );
    assert_eq!(
        tools.first().map(|t| t.name.as_str()),
        Some("tool-0"),
        "the first page is kept"
    );
    assert_eq!(
        tools.last().map(|t| t.name.as_str()),
        Some("tool-15"),
        "the last accepted page is kept"
    );

    let reqs = mock.requests();
    assert_eq!(reqs.len(), 18, "handshake (2) + 16 tools/list pages");
    let first_page: serde_json::Value = serde_json::from_str(&reqs[2].body).expect("json");
    assert_eq!(
        first_page["params"],
        serde_json::json!({}),
        "the first page is requested without a cursor"
    );
    let second_page: serde_json::Value = serde_json::from_str(&reqs[3].body).expect("json");
    assert_eq!(
        second_page["params"]["cursor"],
        serde_json::json!("again"),
        "the server's cursor is echoed back on the next page"
    );
}

#[tokio::test]
async fn list_tools_stops_on_empty_next_cursor() {
    let mock = MockHttp::spawn(tools_server(|_n, req| {
        tools_page(req, serde_json::json!([{ "name": "only" }]), Some(""))
    }));
    let sess = http_session(&mock.url);

    let tools = sess.list_tools().await.expect("single page listing");
    assert_eq!(
        tools.len(),
        1,
        "an empty cursor means there is no next page"
    );
    assert_eq!(
        mock.requests().len(),
        3,
        "exactly one tools/list request is made"
    );
}

#[tokio::test]
async fn list_tools_requires_a_tools_array() {
    let mock = MockHttp::spawn(tools_server(|_n, req| rpc_ok(req, serde_json::json!({}))));
    let sess = http_session(&mock.url);

    let err = sess.list_tools().await.expect_err("missing tools array");
    assert!(
        err.to_string()
            .contains("tools/list response missing 'tools' array"),
        "the malformed payload is named: {err}"
    );
}

#[tokio::test]
async fn list_tools_rejects_descriptor_without_name() {
    let mock = MockHttp::spawn(tools_server(|_n, req| {
        tools_page(
            req,
            serde_json::json!([{ "description": "nameless" }]),
            None,
        )
    }));
    let sess = http_session(&mock.url);

    let err = sess.list_tools().await.expect_err("nameless descriptor");
    let msg = err.to_string();
    assert!(
        msg.contains("tool descriptor missing 'name'"),
        "the offending descriptor is named: {msg}"
    );
    assert!(
        msg.contains("nameless"),
        "the descriptor payload is echoed for diagnosis: {msg}"
    );
}

#[tokio::test]
async fn list_tools_fills_in_optional_fields() {
    let mock = MockHttp::spawn(tools_server(|_n, req| {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "x": { "type": "string" } },
        });
        tools_page(
            req,
            serde_json::json!([
                { "name": "bare" },
                {
                    "name": "documented",
                    "description": "Does a thing",
                    "inputSchema": schema,
                }
            ]),
            None,
        )
    }));
    let sess = http_session(&mock.url);

    let tools = sess.list_tools().await.expect("listing");
    assert_eq!(tools.len(), 2, "both descriptors are returned");
    assert_eq!(tools[0].name, "bare", "name passes through");
    assert_eq!(
        tools[0].description, "",
        "a missing description defaults to the empty string"
    );
    assert_eq!(
        tools[0].input_schema,
        serde_json::json!({ "type": "object", "properties": {} }),
        "a missing inputSchema defaults to an empty object schema"
    );
    assert_eq!(
        tools[1].description, "Does a thing",
        "description passes through"
    );
    assert_eq!(
        tools[1].input_schema["properties"]["x"]["type"],
        serde_json::json!("string"),
        "a server-supplied schema is preserved verbatim"
    );
}

// ---------------------------------------------------------------------------
// Server notifications and progress tracking (spec §5.3)
// ---------------------------------------------------------------------------

fn notification(method: &str, params: serde_json::Value) -> super::super::sse::SseNotification {
    super::super::sse::SseNotification {
        method: method.to_owned(),
        params,
    }
}

fn tracked(sess: &McpClientSession, token: &str) -> Option<f64> {
    sess.state
        .lock()
        .expect("state lock")
        .progress_tokens
        .get(token)
        .copied()
}

#[test]
fn progress_notifications_are_tracked_per_token() {
    let sess = http_session("https://example.com/mcp");

    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 1.0, "total": 10.0 }),
    ));
    assert_eq!(
        tracked(&sess, "t1"),
        Some(1.0),
        "the first notification registers the token"
    );

    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 2.5 }),
    ));
    assert_eq!(
        tracked(&sess, "t1"),
        Some(2.5),
        "a monotonic increase replaces the stored value"
    );

    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t2", "progress": 0.1 }),
    ));
    assert_eq!(
        tracked(&sess, "t2"),
        Some(0.1),
        "tokens are tracked independently"
    );
}

#[test]
fn progress_notification_going_backwards_keeps_the_high_water_mark() {
    // Spec §5.3: "Progress values MUST increase with each notification." A
    // regressed frame is a server-side violation: it is logged and the stored
    // value stays at the highest `progress` seen, so the monotonicity check for
    // later frames still compares against the furthest progress reached.
    // (Was: the regressed value overwrote the high-water mark, so the next
    // frame was judged against the regression.)
    let sess = http_session("https://example.com/mcp");
    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 9.0 }),
    ));
    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 1.0 }),
    ));
    assert_eq!(
        tracked(&sess, "t1"),
        Some(9.0),
        "the regressed frame is rejected but the token stays tracked at the mark"
    );

    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 9.0 }),
    ));
    assert_eq!(
        tracked(&sess, "t1"),
        Some(9.0),
        "a frame that merely repeats the mark is not an increase"
    );

    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 12.0 }),
    ));
    assert_eq!(
        tracked(&sess, "t1"),
        Some(12.0),
        "a frame past the mark advances it"
    );
}

#[test]
fn progress_notifications_with_unusable_payloads_are_ignored() {
    let sess = http_session("https://example.com/mcp");
    let cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "integer tokens are logged but not deduplicated",
            serde_json::json!({ "progressToken": 7, "progress": 1.0 }),
        ),
        (
            "missing progressToken",
            serde_json::json!({ "progress": 1.0 }),
        ),
        (
            "non-scalar progressToken",
            serde_json::json!({ "progressToken": { "nested": true }, "progress": 1.0 }),
        ),
        (
            "null progressToken",
            serde_json::json!({ "progressToken": null, "progress": 1.0 }),
        ),
        (
            "missing progress",
            serde_json::json!({ "progressToken": "t" }),
        ),
        (
            "non-numeric progress",
            serde_json::json!({ "progressToken": "t", "progress": "fast" }),
        ),
    ];

    for (case, params) in cases {
        sess.handle_server_notification(&notification("notifications/progress", params));
        let state = sess.state.lock().expect("state lock");
        assert!(
            state.progress_tokens.is_empty(),
            "{case}: nothing should be tracked, got {:?}",
            state.progress_tokens
        );
    }
}

#[test]
fn non_progress_notifications_are_only_logged() {
    let sess = http_session("https://example.com/mcp");
    for method in [
        "notifications/message",
        "notifications/resources/updated",
        "notifications/some/unknown/method",
    ] {
        sess.handle_server_notification(&notification(
            method,
            serde_json::json!({ "level": "info" }),
        ));
    }
    let state = sess.state.lock().expect("state lock");
    assert!(
        state.progress_tokens.is_empty(),
        "only progress notifications touch the session state"
    );
}

#[test]
fn drop_progress_tokens_clears_the_active_set() {
    let sess = http_session("https://example.com/mcp");
    sess.handle_server_notification(&notification(
        "notifications/progress",
        serde_json::json!({ "progressToken": "t1", "progress": 1.0 }),
    ));

    sess.drop_progress_tokens();
    assert!(
        sess.state
            .lock()
            .expect("state lock")
            .progress_tokens
            .is_empty(),
        "tokens are forgotten once the request completes"
    );

    // Calling it again takes the `is_empty()` short-circuit; the state
    // must stay empty rather than resurrect anything.
    sess.drop_progress_tokens();
    assert!(
        sess.state
            .lock()
            .expect("state lock")
            .progress_tokens
            .is_empty(),
        "dropping an empty set is a no-op"
    );
}

// ---------------------------------------------------------------------------
// OAuth input plumbing
// ---------------------------------------------------------------------------

/// An HTTP endpoint bound to an ephemeral port, released immediately so
/// the port can be reused as a configured loopback redirect target.
fn free_loopback_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

fn sse_config(oauth: Option<crate::config::McpOAuthConfig>) -> McpServerConfig {
    McpServerConfig::Sse {
        url: "https://mcp.example.com/mcp".to_owned(),
        headers: HashMap::new(),
        oauth,
    }
}

#[test]
fn oauth_flow_inputs_merge_scopes_and_attach_pre_registered_client() {
    let port = free_loopback_port();
    let config = sse_config(Some(crate::config::McpOAuthConfig {
        client_id: Some("cid".to_owned()),
        client_secret: Some("s3cret".to_owned()),
        scopes: vec!["read".to_owned(), "write".to_owned()],
        redirect_uri: Some(format!("http://127.0.0.1:{port}/callback")),
    }));
    let sess = McpClientSession::new(config, None);
    let challenge = parse_bearer_challenge("Bearer realm=\"mcp\",scope=\"extra read\"");
    assert!(challenge.is_some(), "the challenge used below must parse");

    let inputs = sess.oauth_flow_inputs(
        "https://mcp.example.com/mcp".to_owned(),
        challenge.clone(),
        vec!["extra".to_owned(), "read".to_owned(), "new".to_owned()],
    );

    assert_eq!(
        inputs.extra_scopes,
        vec![
            "read".to_owned(),
            "write".to_owned(),
            "extra".to_owned(),
            "new".to_owned()
        ],
        "config scopes come first and caller extras are appended without duplicates"
    );
    assert_eq!(
        inputs.mcp_server_url, "https://mcp.example.com/mcp",
        "the resource URI is passed through"
    );
    assert_eq!(
        inputs.www_authenticate, challenge,
        "the WWW-Authenticate challenge is passed through"
    );
    assert!(
        inputs.timeout.is_none(),
        "the flow uses its default timeout"
    );
    assert!(
        inputs.browser_override.is_none(),
        "the session never overrides the browser"
    );
    let pre = inputs
        .pre_registered_client
        .as_ref()
        .expect("a configured client id skips registration");
    assert_eq!(pre.client_id, "cid", "client id comes from the config");
    assert_eq!(
        pre.client_secret.as_deref(),
        Some("s3cret"),
        "client secret comes from the config"
    );
    let loopback = inputs
        .loopback_override
        .as_ref()
        .expect("a configured redirect URI binds the loopback server");
    assert_eq!(
        loopback.port, port,
        "the loopback binds the configured port"
    );
    assert_eq!(
        loopback.redirect_uri,
        format!("http://127.0.0.1:{port}/callback"),
        "the redirect URI is rebuilt from the parsed parts"
    );
}

#[test]
fn oauth_flow_inputs_without_oauth_block_use_only_extra_scopes() {
    let sess = McpClientSession::new(sse_config(None), None);
    let inputs = sess.oauth_flow_inputs(
        "https://mcp.example.com/mcp".to_owned(),
        None,
        vec!["only".to_owned()],
    );
    assert_eq!(
        inputs.extra_scopes,
        vec!["only".to_owned()],
        "with no config block the caller's scopes are the whole request"
    );
    assert!(
        inputs.pre_registered_client.is_none(),
        "no client id means dynamic registration is required"
    );
    assert!(
        inputs.loopback_override.is_none(),
        "no redirect URI means the flow picks its own loopback port"
    );
    assert!(
        inputs.www_authenticate.is_none(),
        "no challenge was supplied"
    );
}

#[test]
fn oauth_flow_inputs_ignore_unusable_redirect_uris() {
    // `https` is rejected by `parse_redirect_uri` (loopback must be plain
    // http), so the override is dropped but the pre-registered client is
    // still attached.
    let config = sse_config(Some(crate::config::McpOAuthConfig {
        client_id: Some("cid".to_owned()),
        client_secret: None,
        scopes: vec![],
        redirect_uri: Some("https://127.0.0.1:9999/callback".to_owned()),
    }));
    let sess = McpClientSession::new(config, None);

    let inputs = sess.oauth_flow_inputs("https://mcp.example.com/mcp".to_owned(), None, vec![]);
    assert!(
        inputs.loopback_override.is_none(),
        "an unparseable redirect URI must not abort the flow"
    );
    assert_eq!(
        inputs.pre_registered_client.map(|c| c.client_id),
        Some("cid".to_owned()),
        "the client id is independent of the redirect URI"
    );
}

#[test]
fn pre_registered_client_requires_a_client_id() {
    let without_id = sse_config(Some(crate::config::McpOAuthConfig {
        client_id: None,
        client_secret: Some("orphan".to_owned()),
        scopes: vec!["read".to_owned()],
        redirect_uri: Some("http://127.0.0.1:1234/callback".to_owned()),
    }));
    let sess = McpClientSession::new(without_id, None);
    assert!(
        sess.pre_registered_client().is_none(),
        "a secret without a client id is not a pre-registered client"
    );
    assert_eq!(
        sess.oauth_redirect_uri().as_deref(),
        Some("http://127.0.0.1:1234/callback"),
        "the redirect URI is reported independently of the client id"
    );
    assert_eq!(
        sess.oauth_config_scopes(),
        vec!["read".to_owned()],
        "declared scopes are reported"
    );

    let stdio = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "echo".to_owned(),
            args: vec![],
            env: HashMap::new(),
        },
        None,
    );
    assert!(
        stdio.pre_registered_client().is_none(),
        "stdio has no OAuth configuration"
    );
    assert!(
        stdio.oauth_redirect_uri().is_none(),
        "stdio has no redirect URI"
    );
    assert!(
        stdio.oauth_config_scopes().is_empty(),
        "stdio has no declared scopes"
    );
}

// ---------------------------------------------------------------------------
// stdio transport internals
// ---------------------------------------------------------------------------

/// A stdio session with a hand-installed transport, so the subprocess
/// lifecycle can be exercised without a real MCP server.
fn stdio_session_with(transport: StdioTransport) -> McpClientSession {
    let sess = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "test-server".to_owned(),
            args: vec![],
            env: HashMap::new(),
        },
        None,
    );
    sess.state.lock().expect("state lock").stdio = Some(transport);
    sess
}

/// A child that stays alive for several seconds and stays silent on
/// stdout, so its piped stdin remains writable.
fn silent_child() -> std::process::Child {
    #[cfg(windows)]
    let args = vec!["127.0.0.1".to_owned(), "-n".to_owned(), "6".to_owned()];
    #[cfg(windows)]
    let program = "ping";
    #[cfg(not(windows))]
    let args = vec!["6".to_owned()];
    #[cfg(not(windows))]
    let program = "sleep";
    let mut cmd = build_stdio_command(program, &args, &HashMap::new());
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().expect("spawn a silent long-running child")
}

/// Build a transport with a live child, a piped stdin and `lines`
/// pre-queued on the reader channel. The returned sender must be kept
/// alive by the caller, otherwise the channel reports `Disconnected`.
fn hand_built_transport(
    lines: Vec<std::io::Result<String>>,
) -> (
    StdioTransport,
    std::sync::mpsc::Sender<std::io::Result<String>>,
) {
    let mut child = silent_child();
    let stdin = child.stdin.take().expect("piped stdin");
    let (tx, rx) = std::sync::mpsc::channel();
    for line in lines {
        tx.send(line).expect("queue a reader line");
    }
    (
        StdioTransport {
            child: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(Mutex::new(Some(stdin))),
            line_rx: Arc::new(Mutex::new(rx)),
        },
        tx,
    )
}

fn ping_payload(id: u64) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": "ping", "params": {} })
}

#[test]
fn recv_stdio_line_returns_queued_lines_then_none() {
    let (transport, _tx) =
        hand_built_transport(vec![Ok("line-a".to_owned()), Ok("line-b".to_owned())]);
    let sess = stdio_session_with(transport);

    assert_eq!(
        sess.recv_stdio_line(std::time::Duration::from_millis(50))
            .expect("first line")
            .as_deref(),
        Some("line-a"),
        "queued lines are returned in order"
    );
    assert_eq!(
        sess.recv_stdio_line(std::time::Duration::from_millis(50))
            .expect("second line")
            .as_deref(),
        Some("line-b"),
        "the second queued line follows the first"
    );
    assert_eq!(
        sess.recv_stdio_line(std::time::Duration::from_millis(10))
            .expect("empty window"),
        None,
        "an empty read window is signalled with None so the caller owns the deadline"
    );
}

#[test]
fn recv_stdio_line_surfaces_a_reader_io_error() {
    let (transport, _tx) = hand_built_transport(vec![Err(std::io::Error::other("read boom"))]);
    let sess = stdio_session_with(transport);

    let err = sess
        .recv_stdio_line(std::time::Duration::from_millis(50))
        .expect_err("an io error is not a line");
    assert!(
        err.to_string().contains("read boom"),
        "the underlying error is reported: {err}"
    );
    let state = sess.state.lock().expect("state lock");
    assert!(state.stdio.is_none(), "the failing transport is torn down");
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "the next call re-initializes"
    );
}

#[test]
fn recv_stdio_line_reports_a_disconnected_reader_thread() {
    let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
    drop(tx);
    let transport = StdioTransport {
        child: Arc::new(Mutex::new(Some(silent_child()))),
        stdin: Arc::new(Mutex::new(None)),
        line_rx: Arc::new(Mutex::new(rx)),
    };
    let sess = stdio_session_with(transport);

    let err = sess
        .recv_stdio_line(std::time::Duration::from_millis(50))
        .expect_err("a disconnected channel means the child closed stdout");
    assert!(
        err.to_string()
            .contains("server closed stdout before responding (EOF)"),
        "the EOF is reported to the caller: {err}"
    );
}

#[test]
fn recv_stdio_line_without_a_transport_errors() {
    let sess = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "test-server".to_owned(),
            args: vec![],
            env: HashMap::new(),
        },
        None,
    );
    let err = sess
        .recv_stdio_line(std::time::Duration::from_millis(10))
        .expect_err("nothing is running");
    assert!(
        err.to_string().contains("no live stdio transport"),
        "the missing transport is named: {err}"
    );
}

#[test]
fn stdio_request_skips_notifications_and_mismatched_ids() {
    let note = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": { "progressToken": "t", "progress": 1.0 },
    })
    .to_string();
    let other = serde_json::json!({ "jsonrpc": "2.0", "id": 99, "result": {} }).to_string();
    let mine =
        serde_json::json!({ "jsonrpc": "2.0", "id": 5, "result": { "ok": true } }).to_string();
    let (transport, _tx) = hand_built_transport(vec![Ok(note), Ok(other), Ok(mine)]);
    let sess = stdio_session_with(transport);

    let response = sess
        .stdio_request(5, ping_payload(5), std::time::Duration::from_millis(500))
        .expect("the matching frame is returned");
    assert_eq!(
        response["result"]["ok"],
        serde_json::json!(true),
        "the response for our id is returned verbatim"
    );
    assert_eq!(
        tracked(&sess, "t"),
        Some(1.0),
        "an interleaved server notification is dispatched while waiting"
    );
}

#[test]
fn stdio_request_matches_string_ids() {
    let line =
        serde_json::json!({ "jsonrpc": "2.0", "id": "7", "result": { "ok": true } }).to_string();
    let (transport, _tx) = hand_built_transport(vec![Ok(line)]);
    let sess = stdio_session_with(transport);

    let response = sess
        .stdio_request(7, ping_payload(7), std::time::Duration::from_millis(500))
        .expect("string ids are legal JSON-RPC ids");
    assert_eq!(
        response["id"],
        serde_json::json!("7"),
        "the string-id frame is the one we matched"
    );
}

#[test]
fn stdio_request_without_stdin_marks_the_transport_dead() {
    // Contract: a request whose stdin is already gone is a transport failure,
    // so it goes through `mark_stdio_dead` ("Drops the transport … and resets
    // the session to Uninitialized so the next call re-initializes against a
    // freshly-spawned server") like every other stdio failure path (write,
    // timeout, parse). The subprocess is taken and killed, not left running.
    // (Was: the missing-stdin guard used `?` inside the write block, returning
    // from `stdio_request` before the teardown ran, so the dead transport
    // stayed installed forever.)
    let (transport, _tx) = hand_built_transport(Vec::new());
    let child = Arc::clone(&transport.child);
    transport.stdin.lock().expect("stdin lock").take();
    let sess = stdio_session_with(transport);

    let err = sess
        .stdio_request(1, ping_payload(1), std::time::Duration::from_millis(200))
        .expect_err("a closed stdin cannot carry a request");
    assert_eq!(
        err.to_string(),
        "stdio server 'stdio:test-server' write (code -1): stdin closed",
        "the write failure is reported as a transport failure: {err}"
    );
    let state = sess.state.lock().expect("state lock");
    assert!(
        state.stdio.is_none(),
        "the dead transport is dropped so the next call re-spawns a server"
    );
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "the session is flagged for re-initialization"
    );
    assert!(
        child.lock().expect("child lock").is_none(),
        "`mark_stdio_dead` takes the child out to kill it"
    );
}

#[test]
fn stdio_request_with_a_poisoned_stdin_lock_marks_the_transport_dead() {
    // Contract: a poisoned stdin mutex means the transport can never carry a
    // request again, so it is torn down like any other write failure.
    let (transport, _tx) = hand_built_transport(Vec::new());
    let poison = Arc::clone(&transport.stdin);
    let handle = std::thread::spawn(move || {
        let _guard = poison.lock().expect("lock");
        panic!("poison the stdio stdin lock");
    });
    assert!(
        handle.join().is_err(),
        "the poisoning thread must panic to poison the mutex"
    );
    let child = Arc::clone(&transport.child);
    let sess = stdio_session_with(transport);

    let err = sess
        .stdio_request(1, ping_payload(1), std::time::Duration::from_millis(200))
        .expect_err("a poisoned stdin lock cannot carry a request");
    assert_eq!(
        err.to_string(),
        "stdio server 'stdio:test-server' write (code -1): stdin lock poisoned",
        "the lock failure is reported as a transport failure: {err}"
    );
    let state = sess.state.lock().expect("state lock");
    assert!(state.stdio.is_none(), "the unusable transport is dropped");
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "the next call re-initializes"
    );
    assert!(
        child.lock().expect("child lock").is_none(),
        "the subprocess is taken out to be killed"
    );
}

#[test]
fn stdio_request_times_out_and_reports_the_deadline() {
    let (transport, _tx) = hand_built_transport(Vec::new());
    let sess = stdio_session_with(transport);
    let start = std::time::Instant::now();

    let err = sess
        .stdio_request(3, ping_payload(3), std::time::Duration::from_millis(60))
        .expect_err("no response ever arrives");
    let elapsed = start.elapsed();
    assert!(
        err.to_string().contains("timed out after"),
        "the timeout is the reported cause: {err}"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(55),
        "the caller's deadline is honoured, not short-circuited: {elapsed:?}"
    );
    assert!(
        sess.state.lock().expect("state lock").stdio.is_none(),
        "a timed-out transport is killed so the next call starts fresh"
    );
}

#[test]
fn stdio_request_invalid_json_line_kills_the_transport() {
    let (transport, _tx) = hand_built_transport(vec![Ok("this is not json".to_owned())]);
    let sess = stdio_session_with(transport);

    let err = sess
        .stdio_request(1, ping_payload(1), std::time::Duration::from_millis(200))
        .expect_err("a malformed frame cannot be a response");
    assert!(
        err.to_string().contains("no live stdio transport"),
        "the parse failure tears the transport down first, so the follow-up read reports it: {err}"
    );
    assert!(
        sess.state.lock().expect("state lock").stdio.is_none(),
        "the transport was dropped by the parse failure"
    );
}

#[test]
fn mark_stdio_dead_names_the_failed_operation() {
    let (transport, _tx) = hand_built_transport(Vec::new());
    let sess = stdio_session_with(transport);

    let err = sess.mark_stdio_dead(
        "unit-test",
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe went away"),
    );
    let msg = err.to_string();
    assert!(
        msg.contains("stdio server 'stdio:test-server' unit-test"),
        "the context names the server and the operation: {msg}"
    );
    assert!(
        msg.contains("pipe went away"),
        "the cause is preserved: {msg}"
    );
    let state = sess.state.lock().expect("state lock");
    assert!(state.stdio.is_none(), "the transport is removed");
    assert_eq!(
        state.phase,
        SessionPhase::Uninitialized,
        "the session is flagged for re-initialization"
    );
}

#[test]
fn write_json_line_appends_the_frame_newline() {
    let mut buf: Vec<u8> = Vec::new();
    McpClientSession::write_json_line(&mut buf, &serde_json::json!({ "a": 1 }))
        .expect("writing to a Vec cannot fail");
    let text = String::from_utf8(buf).expect("json is utf-8");
    assert!(
        text.ends_with('\n'),
        "spec requires newline-delimited JSON-RPC: {text:?}"
    );
    assert_eq!(
        text.trim_end(),
        r#"{"a":1}"#,
        "the payload is written verbatim"
    );
}

/// A writer that accepts no bytes at all: `write` reports a zero-length write,
/// which the default `Write::write_all` turns into `ErrorKind::WriteZero`.
struct RefusingWriter;

impl std::io::Write for RefusingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer that fails every write with a fixed error.
struct BrokenPipeWriter;

impl std::io::Write for BrokenPipeWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe went away",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn write_json_line_surfaces_a_failing_writer() {
    // `std::io::Empty` cannot drive this path: it overrides `write_all` to
    // swallow every byte and report success, so a `write_all`-based writer
    // legitimately sees `Ok(())` from it. The failure path needs a writer that
    // actually refuses the bytes.
    let mut accepting = std::io::empty();
    assert!(
        McpClientSession::write_json_line(&mut accepting, &serde_json::json!({ "a": 1 })).is_ok(),
        "std::io::Empty accepts every write_all and must not look like a failure"
    );

    let mut refusing = RefusingWriter;
    let err = McpClientSession::write_json_line(&mut refusing, &serde_json::json!({ "a": 1 }))
        .expect_err("a writer that accepts no bytes must report the short write");
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::WriteZero,
        "the writer's refusal is propagated as an io error"
    );

    let mut broken = BrokenPipeWriter;
    let err = McpClientSession::write_json_line(&mut broken, &serde_json::json!({ "a": 1 }))
        .expect_err("a writer that errors must propagate the error");
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::BrokenPipe,
        "the writer's own error kind is preserved"
    );
    assert!(
        err.to_string().contains("pipe went away"),
        "the writer's message is preserved, got: {err}"
    );
}

#[test]
fn ensure_stdio_transport_rejects_every_unusable_configuration() {
    let cases = vec![
        (
            "a blank command path",
            McpClientSession::new(
                McpServerConfig::Stdio {
                    command: "   ".to_owned(),
                    args: vec![],
                    env: HashMap::new(),
                },
                None,
            ),
            vec!["empty command path"],
        ),
        (
            "a missing binary",
            McpClientSession::new(
                McpServerConfig::Stdio {
                    command: "definitely-not-an-installed-mcp-server-xyz".to_owned(),
                    args: vec![],
                    env: HashMap::new(),
                },
                None,
            ),
            vec![
                "failed to spawn",
                // The NotFound hint tells the operator what to check.
                "installed and reachable on PATH",
            ],
        ),
        (
            "an HTTP server config",
            http_session("https://example.com/mcp"),
            vec!["stdio transport requested for non-stdio server"],
        ),
    ];
    for (label, sess, expected) in cases {
        let mut state = sess.state.lock().expect("state lock");
        let Err(err) = sess.ensure_stdio_transport_locked(&mut state) else {
            panic!("case '{label}': the misconfiguration must be rejected");
        };
        let msg = err.to_string();
        for fragment in expected {
            assert!(
                msg.contains(fragment),
                "case '{label}': the error must mention '{fragment}', got: {msg}"
            );
        }
    }
}

#[test]
fn ensure_stdio_transport_spawns_once_and_is_reused() {
    let sess = McpClientSession::new(
        McpServerConfig::Stdio {
            command: "cargo".to_owned(),
            args: vec!["--version".to_owned()],
            env: HashMap::new(),
        },
        None,
    );
    let mut state = sess.state.lock().expect("state lock");
    sess.ensure_stdio_transport_locked(&mut state)
        .expect("cargo is available in every dev/CI environment");
    let first_pid = state
        .stdio
        .as_ref()
        .expect("transport installed")
        .child
        .lock()
        .expect("child lock")
        .as_ref()
        .expect("child present")
        .id();

    sess.ensure_stdio_transport_locked(&mut state)
        .expect("a live transport is reused");
    let second_pid = state
        .stdio
        .as_ref()
        .expect("transport still installed")
        .child
        .lock()
        .expect("child lock")
        .as_ref()
        .expect("child present")
        .id();
    assert_eq!(
        first_pid, second_pid,
        "the second call must not spawn a second subprocess"
    );
}
