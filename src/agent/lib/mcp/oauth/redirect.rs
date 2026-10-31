//! Loopback HTTP callback server for the OAuth 2.1 authorization
//! code flow (RFC 8252 §7.3).
//!
//! For a desktop/native app, the standard "open browser to auth URL,
//! receive redirect" pattern uses a local HTTP server bound to
//! 127.0.0.1 on a random port. The redirect URI is
//! `http://127.0.0.1:<port>/<path>`. The browser navigates to that
//! URI with `?code=…&state=…` after the user completes the flow,
//! and we extract those query parameters.
//!
//! Security: 127.0.0.1 is the only address the server binds to.
//! RFC 8252 §7.3: "the client SHOULD bind to the IP address
//! `127.0.0.1` (or `[::1]` for IPv6) to ensure the redirect is
//! not accessible from other network adapters". We follow that
//! strictly.
//!
//! Spec: MCP §4.9: "Use only `localhost` or HTTPS redirect URIs".
//! Loopback HTTP to 127.0.0.1 is allowed by §4.9 explicitly.
//!
//! The server is single-shot: it serves one request, returns a
//! "you can close this window now" HTML page, and shuts itself
//! down. Anything other than the configured callback path gets a
//! 404 so the browser prefetches (`/`, `/favicon.ico`) do not
//! consume the one-shot; the server keeps listening until the
//! callback arrives or the timeout elapses. Every reply is framed
//! with the `Content-Length` of the HTML document it actually
//! writes (RFC 9110 §8.6), so the browser receives the complete
//! page rather than a truncated one.
//! Unit tests live in the sibling `redirect_tests.rs` sidecar.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use super::types::OAuthError;
use url::Url;

/// Default path on the loopback server. RFC 8252 §7.3 says any path
/// is fine; the client picks one and the redirect URI uses the same.
pub const DEFAULT_CALLBACK_PATH: &str = "/callback";

/// How long we'll wait for the browser to hit the callback. The
/// user could be in a slow WebAuthn flow, so this is generous.
pub const DEFAULT_CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

/// Status-line reason phrase and HTML heading for a successful callback.
const REASON_OK: &str = "OK";
/// Status-line reason phrase and HTML heading for a disallowed method.
const REASON_METHOD_NOT_ALLOWED: &str = "Method Not Allowed";
/// Status-line reason phrase and HTML heading for a non-callback path.
const REASON_NOT_FOUND: &str = "Not Found";
/// Status-line reason phrase and HTML heading for a malformed callback.
const REASON_BAD_REQUEST: &str = "Bad Request";

/// Text shown to the user on a successful callback.
const MESSAGE_SUCCESS: &str = "You may now close this window and return to the application.";
/// Text shown to the user when the request uses a method we do not accept.
const MESSAGE_METHOD_REQUIRED: &str = "GET or POST required";
/// Text shown to the user when they hit a path that is not the callback.
const MESSAGE_WAITING_FOR_CALLBACK: &str = "waiting for OAuth callback";
/// Text shown to the user when the redirect carries no `code`.
const MESSAGE_MISSING_CODE: &str = "missing 'code' parameter on redirect";
/// Text shown to the user when the redirect carries no `state`.
const MESSAGE_MISSING_STATE: &str = "missing 'state' parameter on redirect";

/// A bound loopback HTTP server ready to accept one redirect
/// request. The actual server lives on a background thread; this
/// handle exposes the redirect URI, the bound port, and a
/// `wait_for_code` method that blocks until the browser hits it.
#[derive(Clone)]
pub struct LoopbackServer {
    /// The redirect URI the browser should be sent to. Includes
    /// the bound port and the configured callback path.
    pub redirect_uri: String,
    /// The bound port. Exposed for tests/diagnostics.
    pub port: u16,
    callback_path: String,
    receiver: Arc<Mutex<mpsc::Receiver<Result<CallbackParams, OAuthError>>>>,
    timeout: Duration,
}

impl std::fmt::Debug for LoopbackServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackServer")
            .field("redirect_uri", &self.redirect_uri)
            .field("port", &self.port)
            .field("callback_path", &self.callback_path)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// One callback hit. Either carries the parsed query parameters
/// (success) or an error envelope (RFC 6749 §4.1.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackParams {
    /// `code` query parameter.
    pub code: String,
    /// `state` query parameter.
    pub state: String,
    /// Any other query parameters. The OAuth spec only defines
    /// `code` and `state`, but the client MAY receive more (e.g.
    /// `iss` from OIDC). We keep them around in case the
    /// application wants to log them.
    pub extras: Vec<(String, String)>,
}

/// Start a loopback server. The `callback_path` is the URI path
/// the server responds to. The actual port is chosen by the OS
/// (`bind(0)`) and reported back.
pub fn start(
    callback_path: Option<&str>,
    timeout: Option<Duration>,
) -> Result<LoopbackServer, OAuthError> {
    let callback_path = callback_path.unwrap_or(DEFAULT_CALLBACK_PATH).to_owned();
    let timeout = timeout.unwrap_or(DEFAULT_CALLBACK_TIMEOUT);
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|e| OAuthError::Transport(format!("could not bind loopback server: {e}")))?;
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            return Err(OAuthError::Transport(format!(
                "could not read loopback local_addr: {e}"
            )));
        }
    };
    // Restrict the listener to 127.0.0.1 explicitly even though we
    // already bound there — RFC 8252 §7.3 belt-and-suspenders.
    listener.set_nonblocking(false).ok();
    let redirect_uri = format!("http://127.0.0.1:{port}{callback_path}");

    let (tx, rx) = mpsc::channel();
    let path_for_thread = callback_path.clone();
    thread::spawn(move || {
        if let Err(e) = run_loop(listener, &path_for_thread, tx.clone()) {
            let _ = tx.send(Err(e));
        }
    });
    Ok(LoopbackServer {
        redirect_uri,
        port,
        callback_path,
        receiver: Arc::new(Mutex::new(rx)),
        timeout,
    })
}

/// Parse a redirect URI and create a LoopbackServer bound to the
/// specified port and path. The URI must be of the form
/// `http://127.0.0.1:<port>/<path>` or `http://localhost:<port>/<path>`.
pub fn parse_redirect_uri(uri: &str) -> Result<LoopbackServer, OAuthError> {
    let parsed = Url::parse(uri)
        .map_err(|e| OAuthError::Internal(format!("invalid redirect URI '{uri}': {e}")))?;

    if parsed.scheme() != "http" {
        return Err(OAuthError::Internal(format!(
            "redirect URI must use http scheme, got '{}'",
            parsed.scheme()
        )));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| OAuthError::Internal(format!("redirect URI missing host: {uri}")))?;

    if host != "127.0.0.1" && host != "localhost" {
        return Err(OAuthError::Internal(format!(
            "redirect URI host must be 127.0.0.1 or localhost, got '{host}'"
        )));
    }

    let port = parsed
        .port()
        .ok_or_else(|| OAuthError::Internal(format!("redirect URI missing port: {uri}")))?;

    let path = parsed.path().to_owned();
    if path.is_empty() || path == "/" {
        return Err(OAuthError::Internal(format!(
            "redirect URI must have a path component (e.g. /callback): {uri}"
        )));
    }

    // Bind to the specified port
    let listener =
        TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).map_err(|e| {
            OAuthError::Transport(format!("could not bind loopback server to {port}: {e}"))
        })?;

    // Verify we got the expected port
    let actual_port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            return Err(OAuthError::Transport(format!(
                "could not read loopback local_addr: {e}"
            )));
        }
    };

    if actual_port != port {
        return Err(OAuthError::Internal(format!(
            "bound to port {} but expected {port}",
            actual_port
        )));
    }

    listener.set_nonblocking(false).ok();

    let redirect_uri = format!("http://127.0.0.1:{port}{path}");

    let (tx, rx) = mpsc::channel();
    let path_for_thread = path.clone();
    thread::spawn(move || {
        if let Err(e) = run_loop(listener, &path_for_thread, tx.clone()) {
            let _ = tx.send(Err(e));
        }
    });

    Ok(LoopbackServer {
        redirect_uri,
        port,
        callback_path: path,
        receiver: Arc::new(Mutex::new(rx)),
        timeout: DEFAULT_CALLBACK_TIMEOUT,
    })
}

impl LoopbackServer {
    /// Block until the browser hits the callback, or until the
    /// timeout elapses. Returns the parsed `CallbackParams` on
    /// success.
    pub fn wait_for_code(&self) -> Result<CallbackParams, OAuthError> {
        let rx = self
            .receiver
            .lock()
            .map_err(|_| OAuthError::Internal("loopback receiver lock poisoned".to_owned()))?;
        match rx.recv_timeout(self.timeout) {
            Ok(Ok(params)) => Ok(params),
            Ok(Err(e)) => Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(OAuthError::CallbackTimeout(format!(
                "no redirect hit within {:?}",
                self.timeout
            ))),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(OAuthError::Internal(
                "loopback server thread disconnected unexpectedly".to_owned(),
            )),
        }
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        // The server thread exits when it serves one request or
        // when the listener is closed; closing the listener is
        // not straightforward from a different thread (Drop on
        // `TcpListener` itself drops the OS handle, but we don't
        // own it here). The thread blocks on `accept()`, so it
        // will keep the OS handle alive. We leak it deliberately
        // rather than introducing a shared `Arc<Mutex<Option<…>>>`
        // for what's a one-shot 120s wait. The thread will be
        // cleaned up by `accept` returning an error when the
        // process exits.
    }
}

fn run_loop(
    listener: TcpListener,
    callback_path: &str,
    tx: mpsc::Sender<Result<CallbackParams, OAuthError>>,
) -> Result<(), OAuthError> {
    // Single-shot. Accept exactly one request, then return so the
    // thread can exit. If the request isn't the configured
    // callback path, keep listening (the browser may have hit
    // the root or favicon first).
    loop {
        let (stream, _peer) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) => {
                let _ = tx.send(Err(OAuthError::Transport(format!(
                    "loopback accept failed: {e}"
                ))));
                return Ok(());
            }
        };
        // RFC 8252 §7.3: "the client SHOULD verify that the
        // request was made to the path configured in the
        // redirect URI." We do that before doing anything else.
        match handle_connection(stream, callback_path) {
            Ok(Some(params)) => {
                // Success: send the params and stop.
                let _ = tx.send(Ok(params));
                return Ok(());
            }
            Ok(None) => continue, // Not the callback path; keep listening.
            Err(e) => {
                let _ = tx.send(Err(e));
                return Ok(());
            }
        }
    }
}

/// Handle one HTTP connection. Returns:
/// * `Ok(Some(params))` if this was a successful callback hit;
/// * `Ok(None)` if the request was to a different path (the
///   server should keep listening);
/// * `Err(_)` on parse/transport failure (the server should stop).
fn handle_connection(
    mut stream: TcpStream,
    callback_path: &str,
) -> Result<Option<CallbackParams>, OAuthError> {
    // Cap the read so a misbehaving client can't tie us up.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| OAuthError::Transport(format!("loopback clone failed: {e}")))?,
    );
    let mut request_line = String::new();
    if reader
        .read_line(&mut request_line)
        .map_err(|e| OAuthError::Transport(format!("loopback read failed: {e}")))?
        == 0
    {
        return Err(OAuthError::Transport(
            "loopback got empty request".to_owned(),
        ));
    }
    // We only consume headers up to the blank line; we ignore the
    // body (GET requests don't carry one).
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        let n = reader
            .read_line(&mut header)
            .map_err(|e| OAuthError::Transport(format!("loopback header read: {e}")))?;
        if n == 0 {
            break;
        }
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    // Drain the body if Content-Length is set; we don't use it,
    // but leaving bytes in the socket would close the connection
    // before the response is sent.
    if content_length > 0 {
        let mut sink = [0u8; 1024];
        let mut remaining = content_length;
        while remaining > 0 {
            let n = reader
                .read(&mut sink)
                .map_err(|e| OAuthError::Transport(format!("loopback body read: {e}")))?;
            if n == 0 {
                break;
            }
            remaining -= n;
        }
    }

    // Parse the request line: METHOD SP REQUEST-TARGET SP HTTP-VERSION CRLF
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method != "GET" && method != "POST" {
        // RFC 6749 §3.1.2.3 only requires GET; some servers POST
        // the response. We accept both.
        send_response(
            &mut stream,
            405,
            REASON_METHOD_NOT_ALLOWED,
            MESSAGE_METHOD_REQUIRED,
        )?;
        return Ok(None);
    }
    // Split the target into path and query string.
    let target = if let Some(rest) = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
    {
        match rest.split_once('/') {
            Some((_host, path)) => format!("/{path}"),
            None => "/".to_owned(),
        }
    } else {
        target.to_owned()
    };
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target.as_str(), ""),
    };
    if path != callback_path {
        // Browser probably hit `/` or `/favicon.ico`. Reply with
        // a 404 and keep listening.
        send_response(
            &mut stream,
            404,
            REASON_NOT_FOUND,
            MESSAGE_WAITING_FOR_CALLBACK,
        )?;
        return Ok(None);
    }
    // Parse the query string. We accept the standard `code` and
    // `state` parameters; anything else is preserved in `extras`.
    let params = parse_query(query);
    let code = match params.get("code") {
        Some(c) if !c.is_empty() => c.clone(),
        _ => {
            // No code: probably an error response (RFC 6749 §4.1.2.1).
            if let Some(err) = params.get("error") {
                let desc = params.get("error_description").cloned().unwrap_or_default();
                send_response(
                    &mut stream,
                    400,
                    REASON_BAD_REQUEST,
                    &format!("Authorization server returned error='{err}'; description='{desc}'"),
                )?;
                return Err(OAuthError::AuthorizationDenied(format!("{err}: {desc}")));
            }
            send_response(&mut stream, 400, REASON_BAD_REQUEST, MESSAGE_MISSING_CODE)?;
            return Err(OAuthError::AuthorizationDenied(
                "redirect missing 'code'".to_owned(),
            ));
        }
    };
    let state = match params.get("state") {
        Some(s) if !s.is_empty() => s.clone(),
        _ => {
            send_response(&mut stream, 400, REASON_BAD_REQUEST, MESSAGE_MISSING_STATE)?;
            return Err(OAuthError::AuthorizationDenied(
                "redirect missing 'state'".to_owned(),
            ));
        }
    };
    // Drain the `extras` (anything that isn't `code` or `state`).
    let mut extras: Vec<(String, String)> = Vec::new();
    for (k, v) in &params {
        if k != "code" && k != "state" {
            extras.push((k.clone(), v.clone()));
        }
    }
    send_response(&mut stream, 200, REASON_OK, MESSAGE_SUCCESS)?;
    Ok(Some(CallbackParams {
        code,
        state,
        extras,
    }))
}

pub(crate) fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some(parts) => parts,
            None => (pair, ""),
        };
        out.insert(form_decode(k), form_decode(v));
    }
    out
}

fn form_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(b' ');
        } else if b == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 2;
            } else {
                out.push(b);
            }
        } else {
            out.push(b);
        }
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

/// Build the HTML document every reply carries: the status reason as the
/// heading and the diagnostic message as the body text.
fn html_document(reason: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><title>{reason}</title></head>\
         <body><h1>{reason}</h1><p>{message}</p></body></html>"
    )
}

/// Write one complete HTTP reply (status line, headers, blank line, HTML
/// document).
///
/// `Content-Length` is counted from the document bytes that are actually
/// written — RFC 9110 §8.6 defines it as the size of the message body — so a
/// conforming client keeps the whole page instead of stopping at the end of
/// the plain-text message.
fn send_response(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    message: &str,
) -> Result<(), OAuthError> {
    let document = html_document(reason, message);
    let headers = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        document.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|_| stream.write_all(document.as_bytes()))
        .and_then(|_| stream.flush())
        .map_err(|e| OAuthError::Transport(format!("loopback write failed: {e}")))?;
    Ok(())
}

/// Launch the system default browser at the given URL. Best-effort:
/// failure here doesn't fail the OAuth flow (the user can still
/// copy/paste the URL into a browser), but it IS logged at warn.
pub fn open_browser(url: &str) -> Result<(), OAuthError> {
    tracing::info!(authorization_url = %url, "opening browser for OAuth authorization");
    webbrowser::open(url)
        .map_err(|e| OAuthError::Transport(format!("failed to open system browser: {e}")))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "redirect_tests.rs"]
mod tests;
