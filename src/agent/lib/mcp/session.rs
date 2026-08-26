//! Persistent MCP client sessions.
//!
//! One [`McpClientSession`] per server name, lazily created on first
//! [`McpClients::call_tool`](super::McpClients::call_tool) and
//! cached for the lifetime of the manager. Each session:
//!
//! * Negotiates protocol version on first use via the JSON-RPC
//!   `initialize` request and the spec-mandated
//!   `notifications/initialized` follow-up.
//! * For stdio: spawns a persistent subprocess, owns the `Child` plus
//!   its stdin/stdout. The spec requires newline-delimited JSON-RPC
//!   messages, so we read line-by-line from a background thread. If
//!   the subprocess dies, the session is reset to Uninitialized so
//!   the next call performs a fresh handshake against a
//!   freshly-spawned server.
//! * For HTTP: sends a POST per call with the `Accept`,
//!   `MCP-Protocol-Version`, and (after init) `MCP-Session-Id` headers
//!   required by the Streamable HTTP transport. A 404 from the server
//!   drops the cached session id and resets to Uninitialized so the
//!   next call re-initializes.
//! * Enforces a per-request timeout
//!   ([`DEFAULT_REQUEST_TIMEOUT`]); on timeout, sends
//!   `notifications/cancelled` per spec §5.1 and surfaces the error
//!   to the caller. The manager exposes a `call_tool_with_timeout`
//!   variant for callers that need a per-call override (spec §2.5).
//! * For HTTP, sends a `DELETE` to the endpoint with
//!   `MCP-Session-Id` on shutdown per spec §3.4, accepting `405` as
//!   a server-managed-lifetime acknowledgement.
//! * Receives server-pushed notifications (e.g. progress) over the
//!   background reader or an SSE response stream and dispatches them
//!   to a tracing logger. SSE-framed HTTP response bodies are
//!   handled by [`super::sse`]. See the module-level docs on
//!   [`super`] for the full compliance matrix.

use super::error::McpError;
use super::oauth::{
    OAuthFlowInputs, PreRegisteredClient, TokenStore, WwwAuthenticateChallenge,
    parse_bearer_challenge, refresh, run_flow,
};
use crate::config::McpServerConfig;
use std::io::Write;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};

/// Build a [`reqwest::Client`] for MCP HTTP requests (async).
///
/// * Default 4xx/5xx handling — reqwest's async client does not
///   raise on 4xx/5xx by default, so the response is always
///   available for body inspection. This is what drives the OAuth
///   retry path on 401/403 and the body-content diagnostic
///   messages.
/// * `timeout(timeout)` — global request timeout. Per-request
///   overrides can still be set on the [`RequestBuilder`][rb]
///   via `RequestBuilder::timeout`.
///
/// [rb]: reqwest::RequestBuilder
fn mcp_client(timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest::Client builder should not fail with default configuration")
}

/// Build a [`reqwest::blocking::Client`] for the rare sync HTTP
/// path (session `DELETE` on shutdown). The blocking client panics
/// if constructed inside a tokio runtime, so callers must invoke
/// it from a plain thread — see [`McpClientSession::shutdown`].
fn mcp_blocking_client(timeout: std::time::Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest::blocking::Client builder should not fail with default configuration")
}

/// Win32 `CREATE_NO_WINDOW` flag for [`CreateProcessW`].
///
/// Passed to [`std::os::windows::process::CommandExt::creation_flags`]
/// to prevent a console-subsystem child from popping a visible
/// console window when launched from a GUI-subsystem parent. The
/// child still receives a (hidden) console buffer so stdout/stderr
/// redirection to pipes keeps working — only the visible window is
/// suppressed. See `doc/adr/cmd-substitution.md` for the rationale.
///
/// [`CreateProcessW`]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build the [`std::process::Command`] used to spawn a stdio MCP
/// server.
///
/// On Windows the helper applies [`CREATE_NO_WINDOW`] so the child
/// process does not flash a console window when our release binary
/// is launched as a GUI-subsystem application
/// (`#![windows_subsystem = "windows"]` in `src/main.rs`). Stdin /
/// stdout / stderr are still piped — only the visible window is
/// suppressed. On non-Windows platforms the helper is a no-op
/// beyond the stdio wiring; the `creation_flags` call is gated
/// `#[cfg(windows)]`.
///
/// Extracted from [`McpClientSession::ensure_stdio_transport_locked`]
/// as a free function so it can be unit-tested without spinning up a
/// full session. See `doc/adr/cmd-substitution.md` for the full
/// decision record.
fn build_stdio_command(
    program: &str,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Protocol version this client advertises. Pinned to the spec version
/// the rest of this module targets. Bump together with the spec.
pub const PROTOCOL_VERSION: &str = "2025-11-25";

/// MCP client identity sent in the `initialize` `clientInfo`.
pub const CLIENT_NAME: &str = "fastmd";
/// The fastmd client version reported to MCP servers.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Display name for the client (spec §2.2: "clientInfo.title").
/// Surfaces in server-side UIs that show the connected client.
pub const CLIENT_TITLE: &str = "FastMD";

/// Short description of the client (spec §2.2:
/// "clientInfo.description"). One sentence; servers may use it in
/// diagnostics or admin UIs.
pub const CLIENT_DESCRIPTION: &str =
    "FastMD is a markdown knowledge-base manager with an LLM agent loop.";

/// HTTPS URL for the client project (spec §2.2: "clientInfo.websiteUrl").
/// Must be `https://` per the spec; servers may display it as a
/// clickable link in their admin UI.
pub const CLIENT_WEBSITE_URL: &str = "https://github.com/ppai/fastmd";

/// Default per-request timeout. Spec says clients SHOULD establish
/// timeouts and SHOULD always enforce a maximum regardless of
/// progress notifications. 60s is generous for a single JSON-RPC
/// round-trip; per-tool overrides can be added later.
pub const DEFAULT_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Hard upper bound on any single request, regardless of caller
/// overrides. Per spec §2.5: "Clients SHOULD always enforce a
/// maximum timeout regardless of progress." Anything longer is
/// almost certainly a hung server. 10 minutes is a soft cap — long
/// enough for batch-style tools, short enough that a stuck call
/// can't wedge the agent loop indefinitely.
pub const MAX_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Protocol versions this client accepts during version
/// negotiation (spec §2.2 / §2.6). The newest version is first;
/// the client sends `PROTOCOL_VERSION` on the wire, but if the
/// server returns a different value in this list we still accept
/// the session. Anything outside the list is rejected with a
/// disconnect.
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[PROTOCOL_VERSION];

/// A persistent MCP client session for a single server.
///
/// Created lazily by [`McpClients`](super::McpClients); you
/// usually don't construct one directly. Thread-safe: the inner state
/// is guarded by a single `Mutex`.
pub struct McpClientSession {
    config: McpServerConfig,
    state: Mutex<SessionState>,
    /// OAuth 2.1 token store. When `Some`, the session attaches a
    /// bearer token to every HTTP request (unless the config
    /// supplies a static `Authorization` header) and runs the
    /// authorization flow on a 401 with `WWW-Authenticate`.
    /// `None` disables OAuth entirely.
    token_store: Option<Arc<TokenStore>>,
}

struct SessionState {
    phase: SessionPhase,
    /// Protocol version echoed back by the server in the init response,
    /// or `None` if not yet initialized.
    protocol_version: Option<String>,
    /// Server-declared capabilities from the init response.
    server_capabilities: Option<serde_json::Value>,
    /// Server identity (`serverInfo`) from the init response.
    server_info: Option<serde_json::Value>,
    /// `MCP-Session-Id` returned in the init response (HTTP only).
    session_id: Option<String>,
    /// Set to `true` when the most recent HTTP call observed a 401
    /// (or 403 step-up challenge). The manager reads this via
    /// [`McpClientSession::take_unauthorized_flag`] after every
    /// call so it can update the
    /// [`McpServerEntry::needs_auth`](crate::config::McpServerEntry::needs_auth)
    /// flag and surface the `Authenticate` button in the Tools dialog.
    last_call_saw_unauthorized: bool,
    /// Monotonic per-session JSON-RPC id. Spec allows string or
    /// integer; we use `u64` starting at 1.
    next_id: u64,
    /// Live stdio transport, if any. Present only for stdio servers
    /// that have been spawned.
    stdio: Option<StdioTransport>,
    /// Active progress tokens for in-flight requests. Maps the
    /// token (as a `String`, since spec tokens are string or int)
    /// to the highest `progress` value seen so far. Spec §5.3:
    /// "Progress values MUST increase with each notification";
    /// "The client MUST stop tracking progress after the operation
    /// completes." Entries are removed when the originating
    /// request's response arrives.
    progress_tokens: std::collections::HashMap<String, f64>,
    /// Most-recent SSE event id seen on the session's response
    /// stream (spec §3.3). Used to resume a dropped stream with
    /// the `Last-Event-ID` header. Persists across requests on
    /// the same session; reset on session restart.
    last_event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionPhase {
    /// Never initialized, or initialization has not yet been
    /// attempted, or a prior round-trip left the session in a state
    /// that requires re-initialization (e.g. the stdio subprocess
    /// died or the HTTP session was reset by a 404).
    Uninitialized,
    /// `initialize` round-tripped and `notifications/initialized` was
    /// sent. Ready for regular requests.
    Active,
}

struct StdioTransport {
    /// The child process. Wrapped in `Option<Mutex<…>>` so we can
    /// `take()` it on shutdown for `wait()`/`kill()` while the rest
    /// of the transport is read-only.
    child: Arc<Mutex<Option<Child>>>,
    /// Serialized writer for stdin.
    stdin: Arc<Mutex<ChildStdin>>,
    /// Channel of lines from a background reader thread, wrapped
    /// in `Arc<Mutex<…>>` so the call-site can `Arc::clone` the
    /// receiver and use it without holding the session-level
    /// lock for the duration of a read. EOF is signaled by the
    /// sender being dropped (the reader thread exits naturally
    /// when the child's stdout closes).
    line_rx: Arc<Mutex<std::sync::mpsc::Receiver<std::io::Result<String>>>>,
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Best-effort cleanup if `shutdown` wasn't called: kill the
        // child and let the OS reap it. We don't wait synchronously
        // (Drop is sync); the child handle will be released as
        // soon as the process exits.
        if let Some(mut child) = self.child.lock().ok().and_then(|mut g| g.take()) {
            let _ = child.kill();
            // Reap in the background so the OS handle doesn't leak.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

impl McpClientSession {
    /// Build a new session bound to a given server config. No I/O
    /// happens until [`McpClientSession::ensure_initialized`] (or
    /// [`McpClientSession::call_request`]) is called.
    ///
    /// `token_store` is `Some` for HTTP servers that should run
    /// the OAuth 2.1 authorization flow. Pass `None` to disable
    /// OAuth (stdio transport always uses `None`).
    pub fn new(config: McpServerConfig, token_store: Option<Arc<TokenStore>>) -> Self {
        Self {
            config,
            state: Mutex::new(SessionState {
                phase: SessionPhase::Uninitialized,
                protocol_version: None,
                server_capabilities: None,
                server_info: None,
                session_id: None,
                next_id: 1,
                stdio: None,
                progress_tokens: std::collections::HashMap::new(),
                last_event_id: None,
                last_call_saw_unauthorized: false,
            }),
            token_store,
        }
    }

    /// Returns the previous value of the internal `last_call_saw_unauthorized`
    /// flag, resetting it to `false`. The manager calls this after every
    /// MCP call so it can update the in-memory `needs_auth` flag
    /// (see [`McpClients::mark_needs_auth`](super::McpClients::mark_needs_auth)).
    /// Returns `false` if the lock is poisoned.
    pub fn take_unauthorized_flag(&self) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        std::mem::replace(&mut state.last_call_saw_unauthorized, false)
    }

    /// Replace the OAuth token store on this session. Used by
    /// tests; production code wires the store at construction.
    pub fn set_token_store(&mut self, store: Option<Arc<TokenStore>>) {
        self.token_store = store;
    }

    /// Returns the resource URI used as the `resource` parameter
    /// on the OAuth authorization and token requests. For HTTP
    /// servers this is the URL from the config; for stdio it's
    /// not applicable.
    fn resource_uri(&self) -> Option<String> {
        match &self.config {
            McpServerConfig::Sse { url, .. } => Some(url.clone()),
            McpServerConfig::Stdio { .. } => None,
        }
    }

    /// True if the configured server supplies a static
    /// `Authorization` header. In that case the session uses it
    /// verbatim and does NOT run the OAuth flow.
    fn has_static_authorization(&self) -> bool {
        match &self.config {
            McpServerConfig::Sse { headers, .. } => headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("authorization")),
            McpServerConfig::Stdio { .. } => false,
        }
    }

    /// Scopes declared in the server's explicit OAuth config block
    /// (MCP-012, spec §4.5). These are always requested in addition
    /// to whatever the server discovers. Empty for stdio transport
    /// and for HTTP servers without an OAuth block.
    fn oauth_config_scopes(&self) -> Vec<String> {
        match &self.config {
            McpServerConfig::Sse { oauth, .. } => {
                oauth.as_ref().map(|c| c.scopes.clone()).unwrap_or_default()
            }
            McpServerConfig::Stdio { .. } => Vec::new(),
        }
    }

    /// Build the [`OAuthFlowInputs`] for a (re-)authorization
    /// attempt. Config scopes are merged with the caller-supplied
    /// `extra_scopes` (deduplicated) so the flow always requests the
    /// union per spec §4.5, and the pre-registered client from the
    /// config is attached so refresh skips re-registration.
    fn oauth_flow_inputs(
        &self,
        resource: String,
        challenge: Option<WwwAuthenticateChallenge>,
        extra_scopes: Vec<String>,
    ) -> OAuthFlowInputs {
        let mut scopes = self.oauth_config_scopes();
        for s in &extra_scopes {
            if !scopes.iter().any(|x| x == s) {
                scopes.push(s.clone());
            }
        }
        let redirect_uri = self.oauth_redirect_uri();
        let loopback_override = redirect_uri.as_ref().and_then(|uri| {
            // Parse the redirect URI to extract the port and path for the loopback server
            super::parse_redirect_uri(uri).ok()
        });
        OAuthFlowInputs {
            mcp_server_url: resource,
            www_authenticate: challenge,
            extra_scopes: scopes,
            timeout: None,
            pre_registered_client: self.pre_registered_client(),
            loopback_override,
            browser_override: None,
        }
    }

    /// Server name from the config, useful for error messages.
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Negotiated protocol version (after init), or `None` if not yet
    /// initialized.
    pub fn protocol_version(&self) -> Option<String> {
        self.state.lock().ok()?.protocol_version.clone()
    }

    /// Server-declared capabilities (after init), or `None` if not yet
    /// initialized.
    pub fn server_capabilities(&self) -> Option<serde_json::Value> {
        self.state.lock().ok()?.server_capabilities.clone()
    }

    /// Server identity (after init), or `None` if not yet initialized.
    pub fn server_info(&self) -> Option<serde_json::Value> {
        self.state.lock().ok()?.server_info.clone()
    }

    /// Send a graceful shutdown to the underlying transport, if any.
    /// Safe to call multiple times.
    pub fn shutdown(&self) {
        tracing::info!(
            server = %self.config_label(),
            "shutting down MCP session"
        );
        // Step 1 (HTTP only): per spec §3.4, the client SHOULD send
        // a DELETE to the MCP endpoint with the `MCP-Session-Id`
        // header when the session is no longer needed. The server
        // MAY respond with `405 Method Not Allowed` (server-managed
        // lifetime) — we accept that as success. Network errors are
        // logged but don't fail shutdown.
        //
        // The DELETE uses [`mcp_blocking_client`] (a
        // `reqwest::blocking::Client`) which panics if constructed
        // inside a tokio runtime. We spawn a plain OS thread to
        // keep `shutdown` callable from any context — sync agent
        // loop, tokio config subscriber, or `Drop`.
        if let Some((url, headers, sid)) = self.http_session_endpoint() {
            std::thread::spawn(move || {
                if let Err(e) = Self::http_session_delete(&url, &headers, &sid) {
                    tracing::warn!(
                        server = "shutdown",
                        error = %e,
                        "MCP HTTP DELETE on shutdown failed; continuing with local teardown"
                    );
                }
            });
            // Drop the cached session id so a re-init later doesn't
            // try to reuse a deleted session.
            if let Ok(mut state) = self.state.lock() {
                state.session_id = None;
            }
        }

        // Step 2 (stdio): close stdin, wait briefly, kill on grace
        // timeout. We can't partially-move fields out of
        // `StdioTransport` because of its Drop impl, so we hand the
        // whole transport to a helper that owns it.
        let transport = match self.state.lock() {
            Ok(mut s) => s.stdio.take(),
            Err(_) => None,
        };
        if let Some(transport) = transport {
            Self::shutdown_stdio_transport(transport);
        }
    }

    /// If the session is HTTP and has a `MCP-Session-Id`, return
    /// the URL, user-configured headers, and session id; otherwise
    /// `None`. Used to issue the spec §3.4 DELETE.
    fn http_session_endpoint(
        &self,
    ) -> Option<(String, std::collections::HashMap<String, String>, String)> {
        let state = self.state.lock().ok()?;
        match &self.config {
            McpServerConfig::Sse { url, headers, .. } => {
                let sid = state.session_id.clone()?;
                Some((url.clone(), headers.clone(), sid))
            }
            McpServerConfig::Stdio { .. } => None,
        }
    }

    /// Spec §3.5 backcompat probe. Triggered when the server
    /// returns 400/404/405 on the modern POST. Sends a GET to
    /// the same URL with `Accept: text/event-stream` and looks
    /// for an `event: endpoint\ndata: <url>` event. If found,
    /// the server speaks the pre-2025-03 HTTP+SSE transport —
    /// full legacy transport support is a future round; today
    /// we surface a clear error naming the discovered endpoint
    /// URL. If the GET also fails or returns no endpoint event,
    /// we surface a "could not negotiate transport" error with
    /// both status codes.
    pub async fn probe_legacy_transport(
        url: &str,
        headers: &std::collections::HashMap<String, String>,
        post_code: u16,
        post_body: &str,
    ) -> McpError {
        // reqwest's async client does not raise on 4xx/5xx by
        // default, so the response body is always available for
        // diagnostic messages. The timeout is on the client.
        let client = mcp_client(DEFAULT_REQUEST_TIMEOUT);
        let mut req = client.get(url).header("Accept", "text/event-stream");
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return McpError::transport(
                    format!("HTTP server '{url}'"),
                    format!(
                        "modern POST returned {post_code}; backcompat probe GET also failed: {e}"
                    ),
                );
            }
        };
        let get_code = resp.status().as_u16();
        if get_code >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return McpError::transport(
                format!("HTTP server '{url}'"),
                format!(
                    "transport negotiation failed: modern POST returned {post_code}, backcompat probe GET returned {get_code}; body: {body}"
                ),
            );
        }
        let content_type = resp
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !content_type.contains("text/event-stream") {
            return McpError::transport(
                format!("HTTP server '{url}'"),
                format!(
                    "transport negotiation failed: modern POST returned {post_code}, backcompat probe GET returned {get_code} with unexpected Content-Type '{content_type}'; body: {post_body}"
                ),
            );
        }
        let body = resp.text().await.unwrap_or_default();
        let events = super::sse::parse_sse_body(&body);
        for event in events {
            if event.event.as_deref() == Some("endpoint") && !event.data.is_empty() {
                // Legacy transport detected. Full support is a
                // future round; surface a clear, actionable
                // error that names the discovered endpoint URL.
                return McpError::transport(
                    format!("HTTP server '{url}'"),
                    format!(
                        "server speaks the pre-2025-03 HTTP+SSE transport (endpoint: {}); this client only supports the modern Streamable HTTP transport. Consider upgrading the server or downgrading the protocol version.",
                        event.data
                    ),
                );
            }
        }
        McpError::transport(
            format!("HTTP server '{url}'"),
            format!(
                "transport negotiation failed: modern POST returned {post_code}, backcompat probe GET returned {get_code} but no `event: endpoint` was found; body: {body}"
            ),
        )
    }

    /// Issue the spec §3.4 DELETE against the MCP endpoint with
    /// `MCP-Session-Id`. Treated as success on 2xx (200/202) and on
    /// `405 Method Not Allowed` (server-managed lifetime).
    /// Everything else is reported via the returned
    /// [`McpError`].
    pub fn http_session_delete(
        url: &str,
        headers: &std::collections::HashMap<String, String>,
        session_id: &str,
    ) -> Result<(), McpError> {
        // reqwest's blocking client does not raise on 4xx/5xx by
        // default. The 405 acknowledgement path is therefore
        // expressed as a status-code check on the response.
        let client = mcp_blocking_client(DEFAULT_REQUEST_TIMEOUT);
        let mut req = client
            .delete(url)
            .header("MCP-Session-Id", session_id)
            .header("Accept", "application/json, text/event-stream");
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        match req.send() {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if code == 405 {
                    // Spec: server MAY respond with 405 to indicate it
                    // manages session lifetime. That's an acknowledgement,
                    // not an error.
                    tracing::debug!(url = %url, "MCP server acknowledged DELETE with 405 (server-managed lifetime)");
                    return Ok(());
                }
                if code >= 400 {
                    let body = resp.text().unwrap_or_default();
                    return Err(McpError::transport(
                        format!("HTTP server '{url}'"),
                        format!("DELETE on shutdown returned HTTP {code}: {body}"),
                    ));
                }
                tracing::debug!(
                    url = %url,
                    session_id = %redact_session_id(session_id),
                    "sent MCP session DELETE on shutdown"
                );
                Ok(())
            }
            Err(e) => Err(McpError::transport(
                format!("HTTP server '{url}'"),
                format!("DELETE on shutdown failed: {e}"),
            )),
        }
    }

    fn shutdown_stdio_transport(mut transport: StdioTransport) {
        // Spec §2.4 stdio shutdown:
        //   1. Close input stream to the server process
        //   2. Wait for the server to exit
        //   3. Send SIGTERM if the server does not exit within a
        //      reasonable time
        //   4. Send SIGKILL if the server still does not exit
        //
        // We implement this as four discrete steps, each with a
        // short grace period. The total worst-case shutdown time
        // is bounded by the sum of the grace periods (~4s).

        // Step 1: close stdin. Since `Arc::get_mut` succeeds when
        // there's exactly one strong reference and we hold the
        // only one, we can take the inner Mutex out, drop the lock
        // guard (which drops the ChildStdin), and close the pipe.
        if let Ok(stdin_guard) = transport.stdin.lock() {
            drop(stdin_guard);
        }

        let Some(child_arc) = Arc::get_mut(&mut transport.child) else {
            return;
        };
        let Some(mut child) = child_arc.get_mut().ok().and_then(|g| g.take()) else {
            return;
        };

        // Step 2: wait briefly for a clean exit. A well-behaved
        // server notices the closed stdin and shuts down within
        // ~100ms.
        if wait_for_exit(&mut child, std::time::Duration::from_secs(2)) {
            return;
        }

        // Step 3: SIGTERM (Unix) or direct kill (Windows — no
        // SIGTERM equivalent). On Unix we spawn `kill -TERM` to
        // give the server a chance to flush and exit cleanly
        // before escalating.
        Self::send_sigterm(&mut child);
        if wait_for_exit(&mut child, std::time::Duration::from_secs(2)) {
            return;
        }

        // Step 4: SIGKILL. `Child::kill()` is the cross-platform
        // hard-kill (TerminateProcess on Windows, SIGKILL on
        // Unix). Reap the child to avoid a zombie.
        let _ = child.kill();
        let _ = child.wait();
        // When `transport` is dropped here, the line_rx Receiver
        // is dropped (the reader thread sees the channel close
        // and exits), and any leftover stdin Arc references are
        // dropped.
    }

    /// Send SIGTERM to the child process. Unix-only; no-op on
    /// Windows (where there is no SIGTERM equivalent and the
    /// caller will fall straight through to `Child::kill()`).
    /// Implemented by spawning the standard `kill` command rather
    /// than pulling in a direct `libc` dependency.
    #[cfg(unix)]
    fn send_sigterm(child: &mut std::process::Child) {
        let pid = child.id();
        if pid == 0 {
            return;
        }
        // Best-effort. If `kill` isn't on PATH (extremely rare on
        // Unix) the call fails silently and we fall through to
        // SIGKILL on the next step.
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }

    /// No-op on Windows — there is no SIGTERM. The caller's next
    /// step is `Child::kill()` which uses TerminateProcess.
    #[cfg(not(unix))]
    fn send_sigterm(_child: &mut std::process::Child) {}

    /// Initialize the session if it is not yet active. Idempotent.
    pub async fn ensure_initialized(&self) -> Result<(), McpError> {
        // Fast path: already active.
        if let Ok(state) = self.state.lock()
            && state.phase == SessionPhase::Active
        {
            return Ok(());
        }

        // Slow path: do the init handshake.
        let init_response = self.do_initialize().await?;

        // Per spec §2.2 / §2.6: if the server returns a
        // `protocolVersion` we don't support, we MUST disconnect
        // (and SHOULD refuse to talk to it). The handshake
        // itself completes first, so the server sees a clean
        // `initialize` request and can log our version; the
        // disconnect happens locally.
        let mut state = self.lock_state()?;
        if let Some(value) = init_response.get("result") {
            let server_version = value.get("protocolVersion").and_then(|v| v.as_str());
            state.protocol_version = server_version.map(str::to_owned);
            state.server_capabilities = value.get("capabilities").cloned();
            state.server_info = value.get("serverInfo").cloned();

            if let Some(v) = server_version
                && !SUPPORTED_PROTOCOL_VERSIONS.contains(&v)
            {
                let err = McpError::transport(
                    format!("server '{}'", self.config_label()),
                    format!(
                        "unsupported protocol version '{v}' (supported: {:?}) per spec §2.6",
                        SUPPORTED_PROTOCOL_VERSIONS
                    ),
                );
                // Drop the lock before tearing down the
                // transport — `disconnect_after_init` takes it
                // again.
                drop(state);
                self.disconnect_after_init();
                return Err(err);
            }
        }
        state.phase = SessionPhase::Active;
        Ok(())
    }

    /// Tear down the session after a failed version negotiation
    /// (or any other post-init fatal error). For stdio, kills the
    /// subprocess. For HTTP, drops the session id so the next
    /// call re-initializes from scratch.
    fn disconnect_after_init(&self) {
        if let Ok(mut state) = self.state.lock() {
            tracing::warn!(
                server = %self.config_label(),
                "disconnecting MCP session after init failure (subprocess killed, session id dropped)"
            );
            state.phase = SessionPhase::Uninitialized;
            state.session_id = None;
            if let Some(transport) = state.stdio.take()
                && let Some(mut child) = transport.child.lock().ok().and_then(|mut g| g.take())
            {
                let _ = child.kill();
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
        }
    }

    /// Send a JSON-RPC request (`method` + `params`) and return the
    /// decoded `result` value. Performs the init handshake lazily on
    /// first call. Uses [`DEFAULT_REQUEST_TIMEOUT`].
    pub async fn call_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        self.call_request_with_timeout(method, params, DEFAULT_REQUEST_TIMEOUT)
            .await
    }

    /// Same as [`McpClientSession::call_request`] but with a
    /// caller-supplied per-call timeout. The spec (§2.5) says SDKs
    /// SHOULD allow per-request timeout configuration; the manager
    /// exposes the same override as
    /// [`McpClients::call_tool_with_timeout`](super::McpClients::call_tool_with_timeout).
    pub async fn call_request_with_timeout(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, McpError> {
        self.ensure_initialized().await?;
        let id = {
            let mut state = self.lock_state()?;
            let id = state.next_id;
            // Per spec: id MUST be representable as a JSON integer
            // and SHOULD be unique per session. u64 is fine in
            // practice; we wrap on overflow rather than panic.
            state.next_id = state.next_id.wrapping_add(1);
            id
        };
        // Per spec §2.5: "Clients SHOULD always enforce a maximum
        // timeout regardless of progress." Cap the caller's
        // override so a buggy caller can't request a 24-hour
        // timeout and wedge the agent.
        let bounded_timeout = timeout.min(MAX_REQUEST_TIMEOUT);
        let response = self
            .send_request(id, method, params, bounded_timeout)
            .await?;
        Self::extract_result(&self.config_label(), method, response)
    }

    // ----- internal: init handshake -----

    /// Send a `ping` request to the server and verify the response.
    /// Spec (basic/utilities/ping): the receiver MUST respond with
    /// an empty result. Per spec, used to verify the server is
    /// still responsive.
    pub async fn ping(&self) -> Result<(), McpError> {
        let result = self.call_request("ping", serde_json::json!({})).await?;
        // Validate the empty-result shape. Anything else is
        // protocol-non-compliant.
        match &result {
            serde_json::Value::Object(map) if map.is_empty() => Ok(()),
            serde_json::Value::Null => Ok(()),
            other => Err(McpError::transport(
                format!("server '{}'", self.config_label()),
                format!("ping returned non-empty result: {other}"),
            )),
        }
    }

    async fn do_initialize(&self) -> Result<serde_json::Value, McpError> {
        // Build the `initialize` request. We declare no special
        // capabilities (no roots, no sampling, no elicitation).
        // clientInfo carries the spec-optional `title`,
        // `description`, and `websiteUrl` fields (spec §2.2) so
        // servers that surface connected-client metadata have
        // something to show.
        let init_params = serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": CLIENT_NAME,
                "version": CLIENT_VERSION,
                "title": CLIENT_TITLE,
                "description": CLIENT_DESCRIPTION,
                "websiteUrl": CLIENT_WEBSITE_URL,
            }
        });
        // Allocate the init id from the same monotonic counter the
        // rest of the session uses, and bump it. Previously this was
        // hard-coded to 1, which meant the first post-handshake
        // request re-used id 1 and any mock that asserts on the
        // next-id (`test_stdio_session_handshake_and_call` in
        // `mcp/tests.rs`) saw a duplicate.
        let init_id: u64 = {
            let mut state = self.lock_state()?;
            let id = state.next_id;
            // Per spec: id MUST be representable as a JSON integer
            // and SHOULD be unique per session. u64 is fine in
            // practice; we wrap on overflow rather than panic.
            state.next_id = state.next_id.wrapping_add(1);
            id
        };
        let init_response = self
            .send_request(init_id, "initialize", init_params, DEFAULT_REQUEST_TIMEOUT)
            .await?;
        Self::extract_result(&self.config_label(), "initialize", init_response.clone())?;

        // Send `notifications/initialized`. It's a notification
        // (no `id`, no response expected). Spec: client MUST send
        // this before any other request.
        let initialized_note = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_notification(&initialized_note).await?;

        // Log the negotiated contract so an operator can
        // confirm at a glance which protocol version and server
        // they're talking to.
        if let Some(result) = init_response.get("result") {
            let protocol_version = result.get("protocolVersion").and_then(|v| v.as_str());
            let server_name = result
                .get("serverInfo")
                .and_then(|s| s.get("name"))
                .and_then(|v| v.as_str());
            let server_version = result
                .get("serverInfo")
                .and_then(|s| s.get("version"))
                .and_then(|v| v.as_str());
            tracing::info!(
                server = %self.config_label(),
                protocol_version = ?protocol_version,
                server_name = ?server_name,
                server_version = ?server_version,
                "MCP session initialized"
            );
        }

        Ok(init_response)
    }

    // ----- internal: send_request / send_notification -----

    async fn send_request(
        &self,
        id: u64,
        method: &str,
        params: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, McpError> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let start = std::time::Instant::now();
        tracing::debug!(
            server = %self.config_label(),
            request_id = id,
            method = %method,
            timeout = ?timeout,
            "sending JSON-RPC request"
        );
        let result = match &self.config {
            McpServerConfig::Stdio { .. } => self.stdio_request(id, payload, timeout),
            McpServerConfig::Sse { .. } => self.http_request(id, payload, timeout).await,
        };
        let elapsed = start.elapsed();
        // Spec §5.3: "The client MUST stop tracking progress
        // after the operation completes." Completion includes
        // failure — even a timed-out request can't legitimately
        // be still emitting progress, so we drop any tracked
        // tokens here on both success and error paths.
        self.drop_progress_tokens();
        match &result {
            Ok(_) => {
                tracing::debug!(
                    server = %self.config_label(),
                    request_id = id,
                    method = %method,
                    elapsed = ?elapsed,
                    "JSON-RPC request completed"
                );
            }
            Err(e) => {
                tracing::debug!(
                    server = %self.config_label(),
                    request_id = id,
                    method = %method,
                    elapsed = ?elapsed,
                    error = %e,
                    "JSON-RPC request failed"
                );
            }
        }
        result.map_err(|mut e| {
            // Annotate which method failed, for easier debugging.
            e.message = format!("{method}: {}", e.message);
            e
        })
    }

    async fn send_notification(&self, payload: &serde_json::Value) -> Result<(), McpError> {
        match &self.config {
            McpServerConfig::Stdio { .. } => self.stdio_write(payload),
            McpServerConfig::Sse { .. } => self.http_notification(payload).await,
        }
    }

    // ----- internal: stdio transport -----

    fn stdio_request(
        &self,
        request_id: u64,
        payload: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, McpError> {
        // Make sure the transport is up.
        {
            let mut state = self.lock_state()?;
            self.ensure_stdio_transport_locked(&mut state)?;
        }

        // Write the request. We hold only the stdin mutex for the
        // duration of the write so concurrent writers (e.g. a
        // cancellation notification while a read is pending) don't
        // interleave bytes.
        let write_result = {
            let state = self.lock_state()?;
            let transport = state
                .stdio
                .as_ref()
                .expect("ensure_stdio_transport_locked just installed one");
            let mut stdin = transport
                .stdin
                .lock()
                .map_err(|_| McpError::transport("stdio", "stdin lock poisoned"))?;
            Self::write_json_line(&mut *stdin, &payload)
        };
        if let Err(e) = write_result {
            return Err(self.mark_stdio_dead("write", e));
        }

        // Now wait for the response. Spec: client SHOULD establish
        // per-request timeouts. We loop on the channel, skipping
        // server→client notifications and looking for a response
        // whose id matches ours.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                // Best-effort cancellation. Spec §5.1: on timeout,
                // send `notifications/cancelled` with the request
                // id and a reason, then stop waiting for the
                // response. We log but don't fail if the cancel
                // write itself fails (the subprocess may already
                // be hung).
                let cancel_payload = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/cancelled",
                    "params": {
                        "requestId": request_id,
                        "reason": "client timeout",
                    }
                });
                if let Err(e) = self.stdio_write(&cancel_payload) {
                    tracing::warn!(
                        server = %self.config_label(),
                        request_id,
                        error = %e,
                        "failed to send notifications/cancelled after timeout"
                    );
                }
                // Give the subprocess a brief window to drain the
                // cancel from its stdin pipe before we mark the
                // transport dead (which kills the child). Without
                // this grace period a fast `mark_stdio_dead` can
                // race the read on the child side and the cancel
                // never makes it into the captured stream. 50ms is
                // a tight, well-bounded delay: it's a fraction of
                // any realistic request timeout and only runs on
                // the unhappy timeout path.
                std::thread::sleep(std::time::Duration::from_millis(50));
                // Return the timeout error directly. The previous
                // implementation fell through to `recv_stdio_line`,
                // which would surface a generic "no live transport"
                // or "server closed stdout" error after
                // `mark_stdio_dead` killed the child — that hid
                // the real reason (timeout) from the caller.
                return Err(self.mark_stdio_dead(
                    "timeout",
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("request id {request_id} timed out after {timeout:?}"),
                    ),
                ));
            }
            let remaining = deadline.saturating_duration_since(now);
            let Some(line) = self.recv_stdio_line(remaining)? else {
                // Inner read window elapsed without data; loop
                // back so the outer deadline check above fires
                // and we go through the cancel-write path.
                continue;
            };
            let value: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    self.mark_stdio_dead(
                        "parse",
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("invalid JSON-RPC line: {e}; body: {line}"),
                        ),
                    );
                    continue;
                }
            };
            // Server→client notification: no id, has method.
            if value.get("id").is_none() && value.get("method").is_some() {
                let method = value
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_owned();
                let params = value
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                self.handle_server_notification(&super::sse::SseNotification { method, params });
                continue;
            }
            // Look for a response matching our id.
            let matches = match value.get("id") {
                Some(serde_json::Value::Number(n)) => n.as_u64() == Some(request_id),
                Some(serde_json::Value::String(s)) => s.parse::<u64>().ok() == Some(request_id),
                _ => false,
            };
            if matches {
                return Ok(value);
            }
            // Some other id (protocol error). Per the cancellation
            // spec, we SHOULD ignore unexpected frames; keep
            // looping.
        }
    }

    fn recv_stdio_line(&self, timeout: std::time::Duration) -> Result<Option<String>, McpError> {
        // Grab an Arc handle to the receiver so the session-level
        // lock is released before we sleep.
        let rx = {
            let state = self.lock_state()?;
            let transport = state.stdio.as_ref().ok_or_else(|| {
                McpError::transport(
                    format!("stdio server '{}'", self.config_label()),
                    "no live stdio transport",
                )
            })?;
            Arc::clone(&transport.line_rx)
        };
        let deadline = std::time::Instant::now() + timeout;
        let sleep_step = std::time::Duration::from_millis(5);
        loop {
            let recv = rx
                .lock()
                .map_err(|_| McpError::transport("stdio", "line receiver lock poisoned"))?
                .try_recv();
            match recv {
                // The inner Ok is `std::io::Result<String>`; convert
                // any read error into a transport error.
                Ok(Ok(line)) => return Ok(Some(line)),
                Ok(Err(io_err)) => {
                    return Err(self.mark_stdio_dead("read", io_err));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        // Inner timeout: signal "no data within the
                        // window" by returning `None`. The caller
                        // owns the outer deadline and is responsible
                        // for writing the cancellation notification
                        // before the child is killed. Returning an
                        // error here would short-circuit that path
                        // and the cancel would never be sent.
                        return Ok(None);
                    }
                    let remaining = deadline.saturating_duration_since(now);
                    std::thread::sleep(sleep_step.min(remaining));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.mark_stdio_dead(
                        "read",
                        std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "stdio reader thread disconnected (server closed stdout)",
                        ),
                    );
                    return Err(McpError::transport(
                        format!("stdio server '{}'", self.config_label()),
                        "server closed stdout before responding (EOF)",
                    ));
                }
            }
        }
    }

    fn stdio_write(&self, payload: &serde_json::Value) -> Result<(), McpError> {
        // Ensure the transport is up; the transport's stdin
        // mutex serializes concurrent writers.
        {
            let mut state = self.lock_state()?;
            self.ensure_stdio_transport_locked(&mut state)?;
        }
        let transport = {
            let state = self.lock_state()?;
            state
                .stdio
                .as_ref()
                .ok_or_else(|| {
                    McpError::transport(
                        format!("stdio server '{}'", self.config_label()),
                        "no live stdio transport",
                    )
                })?
                .stdin
                .clone()
        };
        let mut stdin = transport
            .lock()
            .map_err(|_| McpError::transport("stdio", "stdin lock poisoned"))?;
        if let Err(e) = Self::write_json_line(&mut *stdin, payload) {
            tracing::warn!(
                server = %self.config_label(),
                method = %payload.get("method").and_then(|v| v.as_str()).unwrap_or("?"),
                error = %e,
                "stdio write failed; transport will be marked dead"
            );
            drop(stdin);
            self.mark_stdio_dead("write", e);
        }
        Ok(())
    }

    fn ensure_stdio_transport_locked(&self, state: &mut SessionState) -> Result<(), McpError> {
        if state.stdio.is_some() {
            return Ok(());
        }
        let (command, args, env) = match &self.config {
            McpServerConfig::Stdio { command, args, env } => {
                (command.clone(), args.clone(), env.clone())
            }
            McpServerConfig::Sse { .. } => {
                return Err(McpError::transport(
                    "stdio transport requested for non-stdio server",
                    "internal: ensure_stdio_transport_locked called for SSE config",
                ));
            }
        };
        if command.trim().is_empty() {
            return Err(McpError::transport(
                format!("stdio server '{}'", self.config_label()),
                "empty command path",
            ));
        }
        let executable = crate::utils::path::resolve_executable_path(&command);
        let mut cmd = build_stdio_command(executable.as_ref(), &args, &env);
        let mut child = cmd.spawn().map_err(|e| {
            let hint = if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "failed to spawn '{command}': {e}. Check that '{command}' is installed and reachable on PATH"
                )
            } else {
                format!("failed to spawn '{command}': {e}")
            };
            McpError::transport(
                format!("stdio server '{}'", self.config_label()),
                hint,
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::transport("stdio", "child stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::transport("stdio", "child stdout unavailable"))?;
        // Drain stderr in a background thread so a chatty server
        // doesn't block on a full pipe. We do nothing with the
        // output, but a blocking read on stderr would deadlock the
        // process if the buffer fills. Spec: client SHOULD NOT
        // assume stderr indicates an error.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut s = stderr;
                let mut buf = [0u8; 4096];
                // Read in a real drain loop until the child closes
                // stderr (e.g. on exit) or we hit an I/O error. A
                // single read is not enough — a chatty server could
                // fill the pipe and block.
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => continue,
                    }
                }
            });
        }

        // Spawn a background reader thread that pushes each
        // newline-delimited JSON-RPC message from the child's
        // stdout to a channel. The main session pulls from the
        // channel with a per-request timeout, so a stuck server
        // can't hang us forever.
        let line_rx = spawn_stdio_reader_thread(stdout);

        let pid = child.id();
        tracing::info!(
            server = %self.config_label(),
            command = %command,
            executable = %executable,
            pid = pid,
            "spawned stdio MCP subprocess"
        );

        state.stdio = Some(StdioTransport {
            child: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(Mutex::new(stdin)),
            line_rx,
        });
        Ok(())
    }

    /// Mark the stdio transport as dead. Drops the transport (which
    /// kills the child via its Drop impl) and resets the session
    /// to Uninitialized so the next call re-initializes against a
    /// freshly-spawned server. Returns a transport error describing
    /// the original failure.
    fn mark_stdio_dead(&self, op: &str, e: std::io::Error) -> McpError {
        tracing::warn!(
            server = %self.config_label(),
            op = %op,
            error = %e,
            "stdio transport marked dead; next call will re-initialize"
        );
        if let Ok(mut state) = self.state.lock() {
            if let Some(transport) = state.stdio.take() {
                // Take the child out and kill it; the Drop impl on
                // StdioTransport will also catch this but we want to
                // be explicit and not block.
                if let Some(mut child) = transport.child.lock().ok().and_then(|mut g| g.take()) {
                    let _ = child.kill();
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
            }
            state.phase = SessionPhase::Uninitialized;
        }
        McpError::transport(
            format!("stdio server '{}' {op}", self.config_label()),
            e.to_string(),
        )
    }

    fn write_json_line<W: Write>(w: &mut W, value: &serde_json::Value) -> std::io::Result<()> {
        let s = serde_json::to_string(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        w.write_all(s.as_bytes())?;
        w.write_all(b"\n")?;
        w.flush()
    }

    // ----- internal: HTTP transport -----

    async fn http_request(
        &self,
        id: u64,
        payload: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, McpError> {
        // The HTTP request may need to be retried after an OAuth
        // 401 / step-up 403. We track a per-request retry budget
        // here: at most one OAuth flow per top-level call. A
        // second 401 from the same call surfaces as an error
        // rather than another flow.
        self.http_request_with_oauth(id, payload, timeout, Vec::new(), 0)
            .await
    }

    /// Core HTTP request with optional OAuth retry. `initial_bearer`
    /// is the bearer token to send on the first attempt (typically
    /// the cached token from the store); `extra_scopes` is non-empty
    /// when this is a step-up retry driven by a previous 403.
    /// `oauth_attempts` counts how many OAuth flows have already
    /// been run for this call; we cap at 1 to prevent loops.
    async fn http_request_with_oauth(
        &self,
        id: u64,
        payload: serde_json::Value,
        timeout: std::time::Duration,
        extra_scopes: Vec<String>,
        oauth_attempts: u32,
    ) -> Result<serde_json::Value, McpError> {
        let (url, headers) = match &self.config {
            McpServerConfig::Sse { url, headers, .. } => (url.clone(), headers.clone()),
            McpServerConfig::Stdio { .. } => {
                return Err(McpError::transport(
                    "http transport requested for non-http server",
                    "internal: http_request called for stdio config",
                ));
            }
        };
        if url.trim().is_empty() {
            return Err(McpError::transport(
                format!("HTTP server '{}'", self.config_label()),
                "empty endpoint URL",
            ));
        }

        // Decide which Bearer token (if any) to attach. We use
        // the cached token unless (a) the config supplies a
        // static `Authorization` header, in which case we leave
        // the user-supplied auth alone, or (b) the caller passed
        // `extra_scopes`, in which case this is a step-up retry
        // and we want the fresh token the OAuth flow just minted.
        let bearer = if self.has_static_authorization() {
            None
        } else if let Some(store) = &self.token_store {
            match store.get(self.resource_uri().as_deref().unwrap_or(&url)) {
                // On an OAuth retry we must attach whatever the store
                // now holds — the fresh token just minted by the flow
                // or refresh — regardless of scope arguments (the flow
                // may have run for an empty scope set).
                Some(t) if oauth_attempts > 0 => Some(t.access_token.clone()),
                Some(t)
                    if t.is_expired(
                        std::time::SystemTime::now(),
                        super::oauth::DEFAULT_EXPIRY_SKEW,
                    ) =>
                {
                    // Cached token is past its skew; let the OAuth
                    // flow run by sending no bearer on the first
                    // attempt (the 401 will trigger refresh).
                    None
                }
                Some(t) => Some(t.access_token.clone()),
                None => None,
            }
        } else {
            None
        };

        // Spec: client MUST include both `application/json` and
        // `text/event-stream` in Accept. Spec: client MUST send
        // `MCP-Protocol-Version` on every request after init. Spec:
        // if the server returned an `MCP-Session-Id` during init,
        // client MUST include it on every subsequent request. Spec
        // (timeouts): client SHOULD establish per-request timeouts.
        //
        // reqwest's async client does not raise on 4xx/5xx by
        // default, so the response is always available for body
        // inspection. We branch on the status code so the OAuth retry
        // logic still has access to the WWW-Authenticate header and
        // the response body.
        let client = mcp_client(timeout);
        let mut req = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(token) = &bearer {
            req = req.header("Authorization", &format!("Bearer {token}"));
        }
        {
            let state = self.lock_state()?;
            if let Some(v) = &state.protocol_version {
                req = req.header("MCP-Protocol-Version", v);
            }
            if let Some(sid) = &state.session_id {
                req = req.header("MCP-Session-Id", sid);
            }
        }

        let response = req.json(&payload).send().await;
        // reqwest does not raise on 4xx/5xx by default, so the
        // response is always available. We branch on the status code
        // so the OAuth retry logic still has access to the
        // WWW-Authenticate header and the response body.
        let resp = match response {
            Ok(r) if r.status().as_u16() < 400 => r,
            Ok(r) => {
                let code = r.status().as_u16();
                let www_authenticate_header = r
                    .headers()
                    .get("WWW-Authenticate")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let body = r.text().await.unwrap_or_default();
                // The server has indicated it requires auth.
                // Mark the session so the manager can update
                // McpServerEntry::needs_auth on its way out.
                // This is independent of the OAuth retry below: the
                // flag must latch on every 401/403 from a server
                // without a static `Authorization` header, even if
                // no token store is installed (e.g. during the
                // cold-start ping where the manager hasn't wired
                // OAuth yet). Without this, the Tools dialog
                // wouldn't show the `Authenticate` button for a
                // server that returned 401 before the token store
                // was attached. The OAuth retry itself still
                // requires a token store — gating that on
                // `token_store.is_some()` is intentional.
                if (code == 401 || code == 403)
                    && !self.has_static_authorization()
                    && let Ok(mut state) = self.state.lock()
                {
                    state.last_call_saw_unauthorized = true;
                }
                // OAuth integration: on 401 with WWW-Authenticate,
                // run the authorization flow once and retry. On 403
                // with insufficient_scope, run a step-up flow once
                // and retry. The retry path is gated on (a) no
                // static Authorization (the user opted into OAuth
                // implicitly by omitting the header), (b) a token
                // store is installed, and (c) this call hasn't
                // already retried (oauth_attempts < 1).
                if (code == 401 || code == 403)
                    && !self.has_static_authorization()
                    && self.token_store.is_some()
                    && oauth_attempts < 1
                {
                    let challenge = www_authenticate_header
                        .as_deref()
                        .and_then(parse_bearer_challenge);
                    if let Some(store) = &self.token_store {
                        let resource = self
                            .resource_uri()
                            .ok_or_else(|| McpError::transport("missing resource URI", ""))?;
                        let scopes_for_step_up = if code == 403 {
                            let required = challenge
                                .as_ref()
                                .and_then(|c| c.get("scope"))
                                .map(|s| {
                                    s.split_whitespace().map(str::to_owned).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            // Combine the required scopes with any
                            // the caller asked for.
                            let mut combined = extra_scopes.clone();
                            for s in &required {
                                if !combined.iter().any(|x| x == s) {
                                    combined.push(s.clone());
                                }
                            }
                            combined
                        } else {
                            extra_scopes.clone()
                        };
                        let inputs = self.oauth_flow_inputs(
                            resource.clone(),
                            challenge,
                            scopes_for_step_up.clone(),
                        );
                        // On 401, try a silent refresh with the stored
                        // refresh token before falling back to the full
                        // interactive flow. This covers the common
                        // expired-token case without a browser
                        // round-trip (MCP-018). A 403 step-up always
                        // re-runs the interactive flow.
                        //
                        // `refresh` and `run_flow` use
                        // `reqwest::blocking::Client` internally and
                        // cannot be called from within a tokio
                        // runtime. We bridge with `spawn_blocking`,
                        // which runs the closure on a dedicated
                        // blocking thread where no async context is
                        // active.
                        let mut refreshed = false;
                        if code == 401
                            && let Some(existing) =
                                store.get(&resource).filter(|t| t.refresh_token.is_some())
                        {
                            let inputs_clone = inputs.clone();
                            let store_clone = store.clone();
                            let existing_clone = existing.clone();
                            match tokio::task::spawn_blocking(move || {
                                refresh(&inputs_clone, &store_clone, &existing_clone)
                            })
                            .await
                            {
                                Ok(Ok(_output)) => {
                                    tracing::info!(
                                        server = %self.config_label(),
                                        "OAuth refresh succeeded after 401"
                                    );
                                    refreshed = true;
                                }
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        server = %self.config_label(),
                                        error = %e,
                                        "OAuth refresh failed after 401; falling back to interactive flow"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        server = %self.config_label(),
                                        error = %e,
                                        "OAuth refresh task panicked"
                                    );
                                }
                            }
                        }
                        if refreshed {
                            return Box::pin(self.http_request_with_oauth(
                                id,
                                payload,
                                timeout,
                                scopes_for_step_up,
                                oauth_attempts + 1,
                            ))
                            .await;
                        }
                        let inputs_clone = inputs.clone();
                        let store_clone = store.clone();
                        match tokio::task::spawn_blocking(move || {
                            run_flow(&inputs_clone, &store_clone)
                        })
                        .await
                        {
                            Ok(Ok(_output)) => {
                                // Retry once with the fresh token
                                // (and the step-up scopes if any).
                                return Box::pin(self.http_request_with_oauth(
                                    id,
                                    payload,
                                    timeout,
                                    scopes_for_step_up,
                                    oauth_attempts + 1,
                                ))
                                .await;
                            }
                            Ok(Err(e)) => {
                                let label = if code == 401 {
                                    "OAuth flow failed after 401"
                                } else {
                                    "OAuth step-up failed after 403"
                                };
                                tracing::warn!(
                                    server = %self.config_label(),
                                    error = %e,
                                    "{label}"
                                );
                                return Err(McpError::transport(
                                    format!("HTTP server '{}'", self.config_label()),
                                    format!("{label}: {e}"),
                                ));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    server = %self.config_label(),
                                    error = %e,
                                    "OAuth flow task panicked"
                                );
                                return Err(McpError::transport(
                                    format!("HTTP server '{}'", self.config_label()),
                                    format!("OAuth flow task panicked: {e}"),
                                ));
                            }
                        }
                    }
                }
                if code == 404 {
                    // Spec: server indicates the session has been
                    // terminated. Drop the cached session id and
                    // reset to Uninitialized so the next call
                    // re-issues the handshake.
                    tracing::info!(
                        server = %self.config_label(),
                        "server returned HTTP 404; dropping MCP-Session-Id and resetting for re-init"
                    );
                    if let Ok(mut state) = self.lock_state() {
                        state.session_id = None;
                        state.phase = SessionPhase::Uninitialized;
                    }
                }
                // Spec §3.5: 400/404/405 on the modern POST is
                // the trigger to probe for a pre-2025-03
                // HTTP+SSE server. We try a GET to the same URL
                // looking for an `endpoint` SSE event. If we
                // find one, we know the server is legacy — full
                // legacy transport is a future round; today we
                // surface a clear error naming the discovered
                // endpoint URL so the operator can debug.
                if matches!(code, 400 | 404 | 405) {
                    return Err(Self::probe_legacy_transport(&url, &headers, code, &body).await);
                }
                return Err(McpError::transport(
                    format!("HTTP server '{}'", self.config_label()),
                    format!("HTTP {code}: {body}"),
                ));
            }
            Err(e) => {
                return Err(McpError::transport(
                    format!("HTTP server '{}'", self.config_label()),
                    e.to_string(),
                ));
            }
        };

        // If this was the init response (or any response with a
        // session id header), capture it for future requests. Per
        // spec §3.4: "Session IDs MUST contain only visible ASCII
        // characters (0x21 through 0x7E)." We validate and drop
        // non-conforming ids rather than risk sending junk on
        // subsequent requests.
        if let Some(sid) = resp
            .headers()
            .get("MCP-Session-Id")
            .and_then(|v| v.to_str().ok())
        {
            if is_valid_session_id(sid) {
                tracing::info!(
                    server = %self.config_label(),
                    session_id = %redact_session_id(sid),
                    "captured MCP-Session-Id from server"
                );
                if let Ok(mut state) = self.lock_state() {
                    state.session_id = Some(sid.to_owned());
                }
            } else {
                tracing::warn!(
                    server = %self.config_label(),
                    session_id = %sid,
                    "server returned MCP-Session-Id outside the spec-mandated 0x21..0x7E range; ignoring"
                );
            }
        }

        // Per spec, the server may return either `application/json`
        // (a single JSON-RPC envelope) or `text/event-stream` (an
        // SSE stream of one or more events). We branch on the
        // response's Content-Type header, defaulting to JSON if
        // the server didn't set one. The check has to happen
        // BEFORE we consume `resp` via `into_string()`.
        let content_type = resp
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let body = resp.text().await.map_err(|e| {
            McpError::transport(
                format!("HTTP server '{}'", self.config_label()),
                format!("failed to read response body: {e}"),
            )
        })?;

        if content_type.contains("text/event-stream") {
            let events = super::sse::parse_sse_body(&body);
            let mut on_note = |note: &super::sse::SseNotification| {
                self.handle_server_notification(note);
            };
            let walk = super::sse::walk_for_response(events, id, &mut on_note).map_err(|e| {
                McpError::transport(format!("HTTP server '{}'", self.config_label()), e.message)
            })?;
            // Cache the most recent SSE event id on the session so
            // a future request can resume a dropped stream with
            // `Last-Event-ID` (spec §3.3). Full resumption is
            // still TODO — we currently only persist the id, we
            // don't open a GET to resume.
            if let Some(eid) = walk.last_event_id
                && let Ok(mut state) = self.state.lock()
            {
                state.last_event_id = Some(eid);
            }
            Ok(walk.response)
        } else {
            serde_json::from_str(&body).map_err(|e| {
                McpError::transport(
                    format!("HTTP server '{}'", self.config_label()),
                    format!("invalid JSON-RPC response: {e}; body: {body}"),
                )
            })
        }
    }

    /// Build a [`PreRegisteredClient`] from the static OAuth
    /// config block, if any. Returns `None` for stdio transport
    /// or for HTTP servers without an explicit OAuth block.
    fn pre_registered_client(&self) -> Option<PreRegisteredClient> {
        match &self.config {
            McpServerConfig::Sse { oauth, .. } => {
                let cfg = oauth.as_ref()?;
                let client_id = cfg.client_id.clone()?;
                Some(PreRegisteredClient {
                    client_id,
                    client_secret: cfg.client_secret.clone(),
                })
            }
            McpServerConfig::Stdio { .. } => None,
        }
    }

    /// Get the configured redirect URI from the OAuth config, if any.
    /// Returns `None` if not configured or for stdio transport.
    fn oauth_redirect_uri(&self) -> Option<String> {
        match &self.config {
            McpServerConfig::Sse { oauth, .. } => oauth.as_ref()?.redirect_uri.clone(),
            McpServerConfig::Stdio { .. } => None,
        }
    }

    /// Server-pushed notification (no id, has `method`).
    /// Dispatches `notifications/progress` to the progress tracker
    /// (spec §5.3) and logs everything else via tracing. The
    /// tracking is the SHOULD-level obligation: maintain an active
    /// set of `progressToken`s, validate monotonic progress,
    /// stop tracking when the originating request returns.
    /// Subscription dispatch to the LLM is a future round — the
    /// `Tool::execute` trait is still sync, so progress is
    /// observable today only through the log.
    fn handle_server_notification(&self, note: &super::sse::SseNotification) {
        match note.method.as_str() {
            "notifications/progress" => {
                self.track_progress(&note.params);
            }
            "notifications/message" => {
                tracing::info!(
                    server = %self.config_label(),
                    params = %note.params,
                    "received server log message"
                );
            }
            other => {
                tracing::debug!(
                    server = %self.config_label(),
                    method = %other,
                    "received server notification"
                );
            }
        }
    }

    /// Record a `notifications/progress` event (spec §5.3). The
    /// `progressToken` is registered on first sight and the
    /// highest `progress` value seen is remembered so we can
    /// detect the spec violation "progress values MUST increase
    /// with each notification". Tokens are not removed here —
    /// they're cleared when the originating request's response
    /// arrives (see [`McpClientSession::drop_progress_tokens`]).
    fn track_progress(&self, params: &serde_json::Value) {
        let token = match params.get("progressToken") {
            Some(serde_json::Value::String(s)) => s.clone(),
            // Spec: "Progress tokens MUST be strings or integers."
            // We only key on strings because the typical case is
            // caller-supplied string tokens; integer tokens are
            // tracked in the log but not in the dedup map (the
            // monotonic check wouldn't be meaningful without a
            // shared counter).
            Some(serde_json::Value::Number(n)) => {
                tracing::debug!(
                    server = %self.config_label(),
                    token = %n,
                    "received progress notification with integer token (not deduplicated)"
                );
                return;
            }
            _ => {
                tracing::warn!(
                    server = %self.config_label(),
                    "progress notification missing or non-scalar progressToken; ignoring"
                );
                return;
            }
        };
        let progress = match params.get("progress").and_then(|v| v.as_f64()) {
            Some(p) => p,
            None => {
                tracing::warn!(
                    server = %self.config_label(),
                    token = %token,
                    "progress notification missing numeric 'progress' field; ignoring"
                );
                return;
            }
        };
        if let Ok(mut state) = self.state.lock() {
            if let Some(prev) = state.progress_tokens.get(&token).copied()
                && progress < prev
            {
                tracing::warn!(
                    server = %self.config_label(),
                    token = %token,
                    prev,
                    now = progress,
                    "progress notification went backwards (spec §5.3 says progress MUST increase)"
                );
            }
            state.progress_tokens.insert(token.clone(), progress);
        }
        tracing::debug!(
            server = %self.config_label(),
            token = %token,
            progress,
            total = params.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0),
            "received progress notification"
        );
    }

    /// Forget any progress tokens that might still be associated
    /// with the just-completed request. The spec is silent on how
    /// to map requests to tokens (the server is free to send
    /// notifications for any active request), so we conservatively
    /// clear ALL tracked tokens after a successful response —
    /// any still-relevant tokens will be re-registered by the
    /// next notification. This is a behavior trade-off: it means
    /// interleaved parallel requests would lose their progress
    /// state, but `Tool::execute` is sync, so the tool executor
    /// doesn't run parallel MCP calls today.
    fn drop_progress_tokens(&self) {
        if let Ok(mut state) = self.state.lock()
            && !state.progress_tokens.is_empty()
        {
            tracing::debug!(
                server = %self.config_label(),
                tracked = state.progress_tokens.len(),
                "clearing progress tokens after request completion (spec §5.3)"
            );
            state.progress_tokens.clear();
        }
    }

    async fn http_notification(&self, payload: &serde_json::Value) -> Result<(), McpError> {
        // Spec: notifications are accepted with 202 Accepted and no
        // body. We don't bother inspecting the response; we only
        // surface network errors.
        let (url, headers) = match &self.config {
            McpServerConfig::Sse { url, headers, .. } => (url.clone(), headers.clone()),
            McpServerConfig::Stdio { .. } => {
                return Err(McpError::transport(
                    "http transport requested for non-http server",
                    "internal: http_notification called for stdio config",
                ));
            }
        };
        // Notifications are fire-and-forget per spec: 2xx (typically
        // 202 Accepted) is success, but 4xx/5xx is also swallowed
        // because the spec says clients SHOULD ignore the response.
        // reqwest's async client does not raise on 4xx/5xx by
        // default, so anything that gets an HTTP response lands in
        // `Ok(_)`; only network-level errors produce `Err(_)`.
        let client = mcp_client(DEFAULT_REQUEST_TIMEOUT);
        let mut req = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        {
            let state = self.lock_state()?;
            if let Some(v) = &state.protocol_version {
                req = req.header("MCP-Protocol-Version", v);
            }
            if let Some(sid) = &state.session_id {
                req = req.header("MCP-Session-Id", sid);
            }
        }
        match req.json(payload).send().await {
            Ok(_resp) => Ok(()),
            Err(e) => Err(McpError::transport(
                format!("HTTP server '{}'", self.config_label()),
                format!("failed to send notification: {e}"),
            )),
        }
    }

    // ----- internal: response parsing -----

    pub fn extract_result(
        server: &str,
        method: &str,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        // Spec: response MUST be a JSON-RPC envelope. Reject
        // responses missing the `jsonrpc` discriminator.
        if response.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
            return Err(McpError::transport(
                format!("server '{server}'"),
                format!("response to '{method}' is not a JSON-RPC 2.0 envelope: {response}"),
            ));
        }
        if let Some(err) = response.get("error") {
            return Err(McpError::from_jsonrpc(server, err));
        }
        match response.get("result") {
            Some(v) => Ok(v.clone()),
            None => Err(McpError::transport(
                format!("server '{server}'"),
                format!("response to '{method}' had neither result nor error: {response}"),
            )),
        }
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, SessionState>, McpError> {
        self.state
            .lock()
            .map_err(|_| McpError::transport("session lock poisoned", "session lock poisoned"))
    }

    fn config_label(&self) -> String {
        match &self.config {
            McpServerConfig::Stdio { command, .. } => format!("stdio:{command}"),
            McpServerConfig::Sse { url, .. } => format!("http:{url}"),
        }
    }

    // ----- tool discovery -----

    /// Send `tools/list` to the server and return the descriptors for
    /// every tool the server currently advertises.
    ///
    /// Performs the init handshake lazily on first use (same as
    /// [`McpClientSession::call_request`]). Pagination is best-effort:
    /// if the server returns a `nextCursor`, we follow it and log a
    /// warning if we hit the safety cap.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>, McpError> {
        /// Maximum number of pages to fetch when listing MCP resources.
        const MAX_PAGES: u32 = 16;
        let start = std::time::Instant::now();
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        let mut page: u32 = 0;
        loop {
            page += 1;
            let mut params = serde_json::Map::new();
            if let Some(c) = &cursor {
                params.insert("cursor".to_owned(), serde_json::Value::String(c.clone()));
            }
            let result = self
                .call_request("tools/list", serde_json::Value::Object(params))
                .await?;

            let tools = result
                .get("tools")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    McpError::transport(
                        format!("server '{}'", self.config_label()),
                        "tools/list response missing 'tools' array",
                    )
                })?;

            for tool in tools {
                let name = tool
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::transport(
                            format!("server '{}'", self.config_label()),
                            format!("tool descriptor missing 'name': {tool}"),
                        )
                    })?
                    .to_owned();
                let description = tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                // Per spec (server/tools), the input schema is
                // `inputSchema` (a JSON Schema object). Default to
                // an empty object schema if the server omitted it.
                let input_schema = tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));
                all.push(McpToolDescriptor {
                    name,
                    description,
                    input_schema,
                });
            }

            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(next) if !next.is_empty() => {
                    if page >= MAX_PAGES {
                        tracing::warn!(
                            server = %self.config_label(),
                            pages = page,
                            next_cursor = %next,
                            "MCP server returned more tool pages than we follow; truncating"
                        );
                        break;
                    }
                    cursor = Some(next.to_owned());
                }
                _ => break,
            }
        }
        tracing::info!(
            server = %self.config_label(),
            count = all.len(),
            pages = page,
            elapsed = ?start.elapsed(),
            "discovered MCP tools"
        );
        Ok(all)
    }
}

/// Spec §3.4: "Session IDs MUST contain only visible ASCII
/// characters (0x21 through 0x7E)." Returns `true` if `s` is
/// non-empty and every byte is in range. Used to validate
/// `MCP-Session-Id` before caching it.
pub fn is_valid_session_id(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| (0x21..=0x7E).contains(&b))
}

/// Return a log-safe representation of an `MCP-Session-Id`:
/// `len=8:abcd` — never log the raw value. Session ids are
/// server-assigned opaque tokens but operators reasonably want
/// to confirm "yes, we got one" and "yes, it survived across
/// requests" without leaking the value into log aggregation
/// systems.
fn redact_session_id(s: &str) -> String {
    let len = s.len();
    let prefix: String = s.chars().take(4).collect();
    format!("len={len}:{prefix}")
}

/// Poll `child.try_wait()` for up to `timeout`, returning `true`
/// if the child has exited. Used during the stdio shutdown
/// sequence to give the server a chance to terminate gracefully
/// before we escalate to SIGTERM/SIGKILL.
fn wait_for_exit(child: &mut std::process::Child, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    let step = std::time::Duration::from_millis(20);
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => return true,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return false;
                }
                std::thread::sleep(step);
            }
            Err(_) => return true, // treat as "exited with an error"
        }
    }
}

/// Spawn the background reader thread for a stdio subprocess.
/// Returns an `Arc<Mutex<Receiver<…>>>` so the session can grab a
/// clone and use it without holding any other lock for the
/// duration of a read.
fn spawn_stdio_reader_thread(
    stdout: std::process::ChildStdout,
) -> Arc<Mutex<std::sync::mpsc::Receiver<std::io::Result<String>>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF: drop tx to close the channel
                Ok(_) => {
                    // Strip the trailing newline for cleaner
                    // downstream parsing.
                    let trimmed = line.trim_end_matches(['\n', '\r']).to_owned();
                    if tx.send(Ok(trimmed)).is_err() {
                        // Receiver dropped; transport is going away.
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    break;
                }
            }
        }
    });
    Arc::new(Mutex::new(rx))
}

#[cfg(test)]
mod tests {
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
        let got_args: Vec<std::ffi::OsString> =
            cmd.get_args().map(std::ffi::OsString::from).collect();
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
}

/// A tool advertised by an MCP server. Returned by
/// [`McpClientSession::list_tools`] and consumed by the tool registry
/// to expose the tool to the LLM.
#[derive(Debug, Clone)]
pub struct McpToolDescriptor {
    /// Server-side tool name (passed verbatim to `tools/call`).
    pub name: String,
    /// Human-readable description, surfaced to the LLM in the schema.
    pub description: String,
    /// JSON Schema object describing the tool's input.
    pub input_schema: serde_json::Value,
}
#[cfg(test)]
#[path = "session_proptests.rs"]
mod session_proptests;
