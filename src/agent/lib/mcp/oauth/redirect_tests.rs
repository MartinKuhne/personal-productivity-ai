//! Tests for the loopback redirect server and redirect-URI parsing (`agent::lib::mcp::oauth::redirect`).

use super::*;
use std::io::Write;
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Takes the error out of a `Result` while keeping a bespoke failure message.
/// `unwrap_err`/`expect_err` both demand `Ok: Debug`, which several of the types
/// under test here do not implement, and `.err().expect(..)` is a clippy
/// `err_expect` violation.
#[track_caller]
fn expect_error<T, E>(result: Result<T, E>, what: &str) -> E {
    match result {
        Ok(_) => panic!("expected an error: {what}"),
        Err(err) => err,
    }
}

#[test]
fn start_binds_to_loopback_and_reports_redirect_uri() {
    let server = start(Some("/cb"), None).expect("bind");
    assert!(server.redirect_uri.starts_with("http://127.0.0.1:"));
    assert!(server.redirect_uri.ends_with("/cb"));
    assert!(server.port > 0);
}

#[test]
fn wait_for_code_returns_code_and_state_on_hit() {
    let server = start(Some("/cb"), Some(Duration::from_secs(5))).expect("bind");
    let url = server.redirect_uri.clone();
    let port = server.port;
    let redirect = server.redirect_uri.clone();
    let handle = std::thread::spawn(move || server.wait_for_code());

    // Send a synthetic GET request.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        stream,
        "GET {redirect}?code=abc&state=xyz&other=zzz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .expect("write");
    // Drain the response so the server can close the socket.
    let mut buf = Vec::new();
    use std::io::Read;
    let _ = stream.read_to_end(&mut buf);

    let params = handle.join().expect("join").expect("callback");
    assert_eq!(params.code, "abc");
    assert_eq!(params.state, "xyz");
    assert_eq!(params.extras, vec![("other".to_owned(), "zzz".to_owned())]);
    // URL was just an example; touch it to silence "unused".
    let _ = url;
}

#[test]
fn non_callback_path_keeps_listening() {
    let server = start(Some("/cb"), Some(Duration::from_secs(5))).expect("bind");
    let port = server.port;
    let redirect = server.redirect_uri.clone();
    let handle = std::thread::spawn(move || server.wait_for_code());

    // First request: hit `/` with a 404. Server should keep
    // listening.
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
        )
        .expect("write");
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
    }
    // Give the server a moment to loop back to accept().
    std::thread::sleep(Duration::from_millis(50));

    // Second request: hit the real callback.
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        write!(
            stream,
            "GET {redirect}?code=the-code&state=the-state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
        )
        .expect("write");
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
    }

    let params = handle.join().expect("join").expect("callback");
    assert_eq!(params.code, "the-code");
    assert_eq!(params.state, "the-state");
}

#[test]
fn missing_code_returns_authorization_denied() {
    let server = start(Some("/cb"), Some(Duration::from_secs(5))).expect("bind");
    let port = server.port;
    let redirect = server.redirect_uri.clone();
    let handle = std::thread::spawn(move || server.wait_for_code());

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        stream,
        "GET {redirect}?error=access_denied&error_description=user+said+no HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .expect("write");
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);

    let err = handle.join().expect("join").expect_err("must error");
    match err {
        OAuthError::AuthorizationDenied(msg) => {
            assert!(msg.contains("access_denied"));
            assert!(msg.contains("user said no"));
        }
        other => panic!("expected AuthorizationDenied, got {other:?}"),
    }
}

#[test]
fn wait_for_code_times_out() {
    let server = start(Some("/cb"), Some(Duration::from_millis(100))).expect("bind");
    let started = Instant::now();
    let result = server.wait_for_code();
    let elapsed = started.elapsed();
    assert!(matches!(result, Err(OAuthError::CallbackTimeout(_))));
    // We shouldn't be more than ~250ms late.
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
fn form_decode_handles_plus_and_percent() {
    assert_eq!(form_decode("a+b"), "a b");
    assert_eq!(form_decode("a%20b"), "a b");
    assert_eq!(form_decode("a%2Bb"), "a+b");
    assert_eq!(form_decode("plain"), "plain");
    assert_eq!(form_decode(""), "");
}

#[test]
fn form_decode_keeps_invalid_percent_escapes() {
    assert_eq!(form_decode("%zz"), "%zz", "invalid hex digits are literal");
    assert_eq!(form_decode("100%"), "100%", "a trailing percent is literal");
    assert_eq!(form_decode("a%2"), "a%2", "a truncated escape is literal");
    assert_eq!(form_decode("%2f"), "/", "lower-case hex decodes");
    assert_eq!(form_decode("%2F"), "/", "upper-case hex decodes");
    assert_eq!(form_decode("%00a"), "\0a", "a NUL byte decodes losslessly");
}

#[test]
fn hex_val_covers_every_digit_class() {
    assert_eq!(hex_val(b'0'), Some(0));
    assert_eq!(hex_val(b'9'), Some(9));
    assert_eq!(hex_val(b'a'), Some(10));
    assert_eq!(hex_val(b'f'), Some(15));
    assert_eq!(hex_val(b'A'), Some(10));
    assert_eq!(hex_val(b'F'), Some(15));
    assert_eq!(hex_val(b'g'), None);
    assert_eq!(hex_val(b' '), None);
}

#[test]
fn parse_query_splits_pairs_and_normalises_shapes() {
    let q = parse_query("code=a&state=b");
    assert_eq!(q.get("code"), Some(&"a".to_owned()));
    assert_eq!(q.get("state"), Some(&"b".to_owned()));

    assert!(parse_query("").is_empty(), "an empty query has no pairs");

    let q = parse_query("&&flag&&x=1");
    assert_eq!(q.len(), 2, "empty pairs are skipped");
    assert_eq!(q.get("flag"), Some(&String::new()), "a bare key gets \"\"");
    assert_eq!(q.get("x"), Some(&"1".to_owned()));

    let q = parse_query("code=a&code=b");
    assert_eq!(
        q.get("code"),
        Some(&"b".to_owned()),
        "a repeated key keeps the last value"
    );

    let q = parse_query("code=a%20b%2Bc");
    assert_eq!(q.get("code"), Some(&"a b+c".to_owned()));
}

#[test]
fn start_uses_default_callback_path_and_timeout() {
    let server = start(None, None).expect("bind");
    assert!(
        server.redirect_uri.ends_with(DEFAULT_CALLBACK_PATH),
        "expected the default callback path in {}",
        server.redirect_uri
    );
    assert_eq!(server.callback_path, DEFAULT_CALLBACK_PATH);
    assert_eq!(server.timeout, DEFAULT_CALLBACK_TIMEOUT);
}

#[test]
fn loopback_server_debug_lists_binding_details() {
    let server = start(Some("/cb"), Some(Duration::from_secs(3))).expect("bind");
    let text = format!("{server:?}");
    assert!(text.contains("LoopbackServer"), "got: {text}");
    assert!(text.contains(&server.redirect_uri), "got: {text}");
    assert!(text.contains("/cb"), "got: {text}");
}

// ─────────────────────────────────────────────────────────────────────────────
// parse_redirect_uri
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_redirect_uri_rejects_unusable_uris() {
    let cases: Vec<(&str, &str)> = vec![
        ("not a uri", "invalid redirect URI"),
        ("https://127.0.0.1:8765/cb", "must use http scheme"),
        ("ftp://127.0.0.1:8765/cb", "must use http scheme"),
        (
            "http://example.com:8765/cb",
            "host must be 127.0.0.1 or localhost",
        ),
        ("http://127.0.0.1/cb", "missing port"),
        ("http://127.0.0.1:8765", "must have a path component"),
        ("http://127.0.0.1:8765/", "must have a path component"),
    ];

    for (uri, expected) in cases {
        let err = parse_redirect_uri(uri)
            .err()
            .unwrap_or_else(|| panic!("{uri} must be rejected"));
        assert!(
            err.to_string().contains(expected),
            "for {uri}: expected '{expected}', got '{err}'"
        );
    }
}

#[test]
fn parse_redirect_uri_binds_the_requested_port() {
    // Reserve a port, release it, then hand it to the parser so the bind is
    // deterministic without hard-coding a port number.
    let port = {
        let probe = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("probe");
        probe.local_addr().expect("addr").port()
    };

    let server = parse_redirect_uri(&format!("http://localhost:{port}/cb")).expect("bind");
    assert_eq!(server.port, port);
    assert_eq!(
        server.redirect_uri,
        format!("http://127.0.0.1:{port}/cb"),
        "the redirect URI is normalised to the loopback IP"
    );
    assert_eq!(server.callback_path, "/cb");
    assert_eq!(server.timeout, DEFAULT_CALLBACK_TIMEOUT);
}

#[test]
fn parse_redirect_uri_reports_bind_conflict() {
    let occupied = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("bind");
    let port = occupied.local_addr().expect("addr").port();

    let err = expect_error(
        parse_redirect_uri(&format!("http://127.0.0.1:{port}/cb")),
        "the port is still held",
    );
    assert!(
        matches!(err, OAuthError::Transport(_)),
        "expected a transport error, got {err:?}"
    );
    assert!(
        err.to_string().contains("could not bind loopback server"),
        "got: {err}"
    );
}

#[test]
fn parse_redirect_uri_detects_unexpected_bound_port() {
    // Port 0 asks the OS to choose, so the bound port can never match the
    // requested one.
    let err = expect_error(
        parse_redirect_uri("http://127.0.0.1:0/cb"),
        "the OS always picks a real port",
    );
    assert!(err.to_string().contains("bound to port"), "got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// wait_for_code — channel failure modes
// ─────────────────────────────────────────────────────────────────────────────

/// Build a server handle around a caller-supplied receiver so the channel
/// failure modes can be exercised without a background accept thread.
fn server_with_receiver(
    receiver: std::sync::Arc<Mutex<mpsc::Receiver<Result<CallbackParams, OAuthError>>>>,
) -> LoopbackServer {
    LoopbackServer {
        redirect_uri: "http://127.0.0.1:1/callback".to_owned(),
        port: 1,
        callback_path: "/callback".to_owned(),
        receiver,
        timeout: Duration::from_millis(50),
    }
}

#[test]
fn wait_for_code_reports_disconnected_thread() {
    let (tx, rx) = mpsc::channel::<Result<CallbackParams, OAuthError>>();
    drop(tx);

    let err = expect_error(
        server_with_receiver(Arc::new(Mutex::new(rx))).wait_for_code(),
        "a disconnected channel must fail",
    );
    assert!(
        matches!(err, OAuthError::Internal(ref m) if m.contains("disconnected")),
        "got: {err:?}"
    );
}

#[test]
fn wait_for_code_reports_poisoned_lock() {
    let (_tx, rx) = mpsc::channel::<Result<CallbackParams, OAuthError>>();
    let receiver = Arc::new(Mutex::new(rx));

    let poison = Arc::clone(&receiver);
    let handle = std::thread::spawn(move || {
        let _guard = poison.lock().expect("lock");
        panic!("poison the loopback receiver lock");
    });
    assert!(
        handle.join().is_err(),
        "the poisoning thread must panic to poison the mutex"
    );

    let err = expect_error(
        server_with_receiver(receiver).wait_for_code(),
        "a poisoned lock must fail",
    );
    assert!(
        matches!(err, OAuthError::Internal(ref m) if m.contains("poisoned")),
        "got: {err:?}"
    );
}

#[test]
fn wait_for_code_relays_accept_loop_error() {
    let server = start(Some("/cb"), Some(Duration::from_secs(5))).expect("bind");
    let port = server.port;
    let handle = std::thread::spawn(move || server.wait_for_code());

    // Half-close the connection so the accept loop reads an empty request and
    // reports the failure through the channel.
    let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("half-close");

    let err = expect_error(handle.join().expect("join"), "must error");
    assert!(matches!(err, OAuthError::Transport(_)), "got: {err:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_connection — driven directly over a socket pair
// ─────────────────────────────────────────────────────────────────────────────

/// A connected loopback pair: the first socket is the client, the second is
/// handed to [`handle_connection`] so no accept thread is needed.
fn connected_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let client = TcpStream::connect(addr).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    (client, server)
}

/// One HTTP reply as it actually landed on the wire.
struct Reply {
    /// Status code parsed from the `HTTP/1.1 <code> <reason>` status line.
    status: u16,
    /// The `Content-Length` the server declared.
    declared_len: usize,
    /// The payload a spec-conforming client keeps: the first `declared_len`
    /// bytes, and nothing more.
    body: String,
    /// Every payload byte the server wrote, including the bytes past the
    /// declared length that a conforming client never gets to see.
    full_payload: String,
}

/// Read the status line, headers and the *complete* payload of one reply.
///
/// `handle_connection` has already returned (and closed its socket) by the time
/// this runs, so the trailing read reaches EOF instead of blocking.
fn read_reply(client: &mut TcpStream) -> Reply {
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut reader = BufReader::new(client.try_clone().expect("clone"));
    let mut status = 0u16;
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).expect("read line") == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("HTTP/1.1 ") {
            status = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
        if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    let mut payload = Vec::new();
    reader.read_to_end(&mut payload).expect("read payload");
    let kept = payload.len().min(content_length);
    Reply {
        status,
        declared_len: content_length,
        body: String::from_utf8_lossy(&payload[..kept]).into_owned(),
        full_payload: String::from_utf8_lossy(&payload).into_owned(),
    }
}

/// Send one request through [`handle_connection`] and collect both the HTTP
/// reply and the callback outcome.
fn drive(
    request: &str,
    callback_path: &str,
) -> (u16, String, Result<Option<CallbackParams>, OAuthError>) {
    let (reply, result) = drive_reply(request, callback_path);
    (reply.status, reply.body, result)
}

/// Same as [`drive`], but keeps the declared `Content-Length` and the full
/// payload so the framing of the reply can be inspected.
fn drive_reply(
    request: &str,
    callback_path: &str,
) -> (Reply, Result<Option<CallbackParams>, OAuthError>) {
    let (mut client, server) = connected_pair();
    client.write_all(request.as_bytes()).expect("write request");
    client.flush().expect("flush request");
    let result = handle_connection(server, callback_path);
    (read_reply(&mut client), result)
}

/// Unwrap the success case of a [`drive`] result.
fn callback(params: Result<Option<CallbackParams>, OAuthError>) -> CallbackParams {
    match params {
        Ok(Some(params)) => params,
        other => panic!("expected a callback, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DEFECT (shared root cause for the `defect_*` tests below): every reply is
// truncated. `send_response` sizes `Content-Length` from the plain-text
// message it is handed but then writes a full HTML document as the payload:
//
//     len = body_bytes.len()          // length of the message only
//     ...write(header block)          // "Content-Length: <message length>"
//     ...write(html)                  // "<!doctype html>…</html>"
//
// RFC 9110 §8.6 requires Content-Length to be the size of the message payload,
// so a conforming client stops reading at the message length and receives an
// unterminated document — the browser the module doc promises ("returns a
// 'you can close this window now' HTML page", redirect.rs §20-22) never sees
// the message or the closing tags. The tests pin the actual behaviour: the
// declared length equals the message length and `body` is the truncated HTML
// prefix, while `full_payload` shows what the server really wrote.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn defect_handle_connection_accepts_post_with_form_body() {
    // DEFECT: the 200 reply declares Content-Length = 60 (the length of
    // "You may now close this window and return to the application.") while
    // the HTML document is far longer, so the client-visible body is cut off
    // mid-tag and the promise text is never delivered.
    let (reply, result) = drive_reply(
        "POST /cb?code=posted&state=st HTTP/1.1\r\n\
         Host: 127.0.0.1\r\n\
         Content-Length: 7\r\n\
         Connection: close\r\n\r\n\
         x=1&y=2",
        "/cb",
    );

    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.declared_len, 60,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html><head><title>OK</title></head><body><h1",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("close this window"),
        "the promised text sits past the declared length, got: {}",
        reply.body
    );
    assert_eq!(
        reply.full_payload,
        "<!doctype html><html><head><title>OK</title></head><body><h1>OK</h1>\
         <p>You may now close this window and return to the application.</p>\
         </body></html>",
        "the server does write the whole document, it is just miscounted"
    );
    let params = callback(result);
    assert_eq!(params.code, "posted");
    assert_eq!(params.state, "st");
    assert!(params.extras.is_empty());
}

#[test]
fn handle_connection_drains_a_body_larger_than_the_read_buffer() {
    let body = "a".repeat(2_500);
    let request = format!(
        "POST /cb?code=big&state=st HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );

    let (status, _, result) = drive(&request, "/cb");
    assert_eq!(status, 200);
    assert_eq!(callback(result).code, "big");
}

#[test]
fn handle_connection_ignores_a_non_numeric_content_length() {
    let (status, _, result) = drive(
        "GET /cb?code=c&state=s HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: abc\r\n\r\n",
        "/cb",
    );
    assert_eq!(status, 200);
    assert_eq!(callback(result).code, "c");
}

#[test]
fn defect_handle_connection_rejects_other_methods_with_405() {
    // DEFECT: Content-Length is declared as 20 ("GET or POST required") while
    // the HTML document is longer, so the client-visible body is the first 20
    // bytes of the document and the explanation is lost.
    let (reply, result) = drive_reply(
        "DELETE /cb?code=a&state=b HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        "/cb",
    );
    assert_eq!(reply.status, 405);
    assert_eq!(
        reply.declared_len, 20,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("GET or POST required"),
        "the reason text sits past the declared length, got: {}",
        reply.body
    );
    assert!(
        reply.full_payload.contains("GET or POST required"),
        "the server does write the reason, it is just miscounted, got: {}",
        reply.full_payload
    );
    assert!(
        matches!(result, Ok(None)),
        "a wrong method must keep the server listening"
    );
}

#[test]
fn defect_handle_connection_rejects_other_paths_with_404() {
    // DEFECT: Content-Length is declared as 26 ("waiting for OAuth callback")
    // while the HTML document is longer, so the client-visible body stops
    // inside the `<head>` and the message is never delivered.
    let (reply, result) = drive_reply("GET /favicon.ico HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(reply.status, 404);
    assert_eq!(
        reply.declared_len, 26,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html><head",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("waiting for OAuth callback"),
        "the reason text sits past the declared length, got: {}",
        reply.body
    );
    assert!(
        reply.full_payload.contains("waiting for OAuth callback"),
        "the server does write the reason, it is just miscounted, got: {}",
        reply.full_payload
    );
    assert!(matches!(result, Ok(None)));
}

#[test]
fn handle_connection_accepts_absolute_form_request_targets() {
    for target in [
        "http://127.0.0.1:9/cb?code=abs&state=st",
        "https://127.0.0.1:9/cb?code=abs&state=st",
    ] {
        let request = format!("GET {target} HTTP/1.1\r\nHost: h\r\n\r\n");
        let params = callback(drive(&request, "/cb").2);
        assert_eq!(params.code, "abs", "target: {target}");
        assert_eq!(params.state, "st", "target: {target}");
    }
}

#[test]
fn handle_connection_maps_authority_only_target_to_root_path() {
    // An absolute-form target with no path becomes "/" and therefore 404s.
    let (status, _, result) = drive("GET http://127.0.0.1:9 HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(status, 404);
    assert!(matches!(result, Ok(None)));
}

#[test]
fn handle_connection_percent_decodes_query_parameters() {
    let (_, _, result) = drive(
        "GET /cb?code=a%2Fb&state=s%201&iss=op HTTP/1.1\r\nHost: h\r\n\r\n",
        "/cb",
    );
    let params = callback(result);
    assert_eq!(params.code, "a/b");
    assert_eq!(params.state, "s 1");
    assert_eq!(params.extras, vec![("iss".to_owned(), "op".to_owned())]);
}

#[test]
fn defect_handle_connection_rejects_a_missing_code() {
    // DEFECT: Content-Length is declared as 36 ("missing 'code' parameter on
    // redirect") while the HTML document is longer, so the client-visible body
    // is cut off inside the `<title>` and the diagnostic never arrives.
    let (reply, result) = drive_reply("GET /cb?state=st HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(reply.status, 400);
    assert_eq!(
        reply.declared_len, 36,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html><head><title>Ba",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("missing 'code' parameter"),
        "the reason text sits past the declared length, got: {}",
        reply.body
    );
    assert!(
        reply
            .full_payload
            .contains("missing 'code' parameter on redirect"),
        "the server does write the reason, it is just miscounted, got: {}",
        reply.full_payload
    );
    match result {
        Err(OAuthError::AuthorizationDenied(msg)) => assert!(msg.contains("missing 'code'")),
        other => panic!("expected AuthorizationDenied, got {other:?}"),
    }
}

#[test]
fn handle_connection_rejects_an_empty_code() {
    let (status, _, result) = drive("GET /cb?code=&state=st HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(status, 400);
    assert!(
        matches!(result, Err(OAuthError::AuthorizationDenied(_))),
        "got: {result:?}"
    );
}

#[test]
fn defect_handle_connection_rejects_a_missing_state() {
    // DEFECT: Content-Length is declared as 37 ("missing 'state' parameter on
    // redirect") while the HTML document is longer, so the client-visible body
    // is cut off inside the `<title>` and the diagnostic never arrives.
    let (reply, result) = drive_reply("GET /cb?code=c HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(reply.status, 400);
    assert_eq!(
        reply.declared_len, 37,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html><head><title>Bad",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("missing 'state' parameter"),
        "the reason text sits past the declared length, got: {}",
        reply.body
    );
    assert!(
        reply
            .full_payload
            .contains("missing 'state' parameter on redirect"),
        "the server does write the reason, it is just miscounted, got: {}",
        reply.full_payload
    );
    match result {
        Err(OAuthError::AuthorizationDenied(msg)) => assert!(msg.contains("missing 'state'")),
        other => panic!("expected AuthorizationDenied, got {other:?}"),
    }
}

#[test]
fn handle_connection_rejects_an_empty_state() {
    let (status, _, result) = drive("GET /cb?code=c&state= HTTP/1.1\r\nHost: h\r\n\r\n", "/cb");
    assert_eq!(status, 400);
    assert!(
        matches!(result, Err(OAuthError::AuthorizationDenied(_))),
        "got: {result:?}"
    );
}

#[test]
fn defect_handle_connection_relays_authorization_server_error_envelope() {
    // DEFECT: Content-Length is declared as 70 (the length of
    // "Authorization server returned error='server_error'; description='oops'")
    // while the HTML document is longer, so the client-visible body ends right
    // after `<h1>` and the error envelope is never shown to the user. The
    // in-band `OAuthError` still carries the envelope, which is why the flow
    // itself works — but the browser window shows a broken page.
    let (reply, result) = drive_reply(
        "GET /cb?error=server_error&error_description=oops HTTP/1.1\r\nHost: h\r\n\r\n",
        "/cb",
    );
    assert_eq!(reply.status, 400);
    assert_eq!(
        reply.declared_len, 70,
        "Content-Length is the length of the plain message, not the HTML"
    );
    assert_eq!(
        reply.body, "<!doctype html><html><head><title>Bad Request</title></head><body><h1>",
        "the client sees the HTML truncated at the message length, got: {}",
        reply.body
    );
    assert!(
        !reply.body.contains("error='server_error'"),
        "the envelope sits past the declared length, got: {}",
        reply.body
    );
    assert_eq!(
        reply.full_payload.matches("description='oops'").count(),
        1,
        "the description is written exactly once in the document, got: {}",
        reply.full_payload
    );
    assert!(
        reply.full_payload.contains("error='server_error'"),
        "the server does write the error, it is just miscounted, got: {}",
        reply.full_payload
    );
    match result {
        Err(OAuthError::AuthorizationDenied(msg)) => {
            assert_eq!(msg, "server_error: oops", "got: {msg}");
        }
        other => panic!("expected AuthorizationDenied, got {other:?}"),
    }
}

#[test]
fn handle_connection_rejects_an_empty_request() {
    let (client, server) = connected_pair();
    client
        .shutdown(std::net::Shutdown::Write)
        .expect("half-close");

    let err = expect_error(
        handle_connection(server, "/cb"),
        "an empty request must fail",
    );
    assert!(
        matches!(err, OAuthError::Transport(ref m) if m.contains("empty request")),
        "got: {err:?}"
    );
}

#[test]
fn handle_connection_honours_a_custom_callback_path() {
    let (status, _, result) = drive(
        "GET /return?code=c&state=s HTTP/1.1\r\nHost: h\r\n\r\n",
        "/return",
    );
    assert_eq!(status, 200);
    assert_eq!(callback(result).code, "c");
}
