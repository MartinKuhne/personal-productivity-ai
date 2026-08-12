//! MCP client manager — owns transport sessions, OAuth token store, and
//! the per-server `needs_auth` flags the tools dialog renders.

use crate::config::{AppConfig, McpServerConfig, McpServerEntry, get_config_path};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::oauth::TokenStore;
use super::session::{DEFAULT_REQUEST_TIMEOUT, McpClientSession, McpToolDescriptor};
use super::tool_source::DynamicToolSource;

/// Manager for MCP server client sessions, transport execution, and
/// tool dispatching.
///
/// Configured via [`McpClients::update_config`]. Sessions for
/// configured servers are created lazily on first
/// [`McpClients::call_tool`]. Removed servers are shut down on
/// the next `update_config`.
///
/// OAuth 2.1 authorization (MCP spec §4) is optional and turned on
/// by installing a [`TokenStore`] via
/// [`McpClients::set_token_store`]. When a store is installed
/// and the configured server has no static `Authorization` header,
/// the session triggers the OAuth flow on a 401 with
/// `WWW-Authenticate` and caches the resulting access token in the
/// store.
pub struct McpClients {
    state: Mutex<InnerState>,
}

struct InnerState {
    servers: HashMap<String, McpServerEntry>,
    /// In-memory OAuth auth-needed flags. NOT persisted to YAML.
    needs_auth: HashMap<String, bool>,
    sessions: HashMap<String, Arc<McpClientSession>>,
    /// Optional OAuth 2.1 token store. When `None`, OAuth is
    /// disabled: the client will not run the authorization flow,
    /// even if the server returns 401 with `WWW-Authenticate`.
    token_store: Option<Arc<TokenStore>>,
}

impl Default for McpClients {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClients {
    /// Creates a new [`McpClients`] with no OAuth support.
    /// Call [`McpClients::set_token_store`] before the first
    /// call to enable the OAuth 2.1 flow.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InnerState {
                servers: HashMap::new(),
                needs_auth: HashMap::new(),
                sessions: HashMap::new(),
                token_store: None,
            }),
        }
    }

    /// Install (or replace) the OAuth 2.1 token store used by HTTP
    /// sessions. New sessions pick up the store on creation;
    /// existing sessions get a reference at the time of their next
    /// HTTP call. Passing `None` disables OAuth.
    pub fn set_token_store(&self, store: Option<Arc<TokenStore>>) {
        if let Ok(mut state) = self.state.lock() {
            state.token_store = store;
        }
    }

    /// Snapshot of the currently-installed token store, if any.
    /// Exposed for tests and for code that wants to pre-warm a
    /// token by running the flow outside the session.
    pub fn token_store(&self) -> Option<Arc<TokenStore>> {
        self.state.lock().ok().and_then(|s| s.token_store.clone())
    }

    /// Update manager configuration with active MCP servers. Any
    /// previously-cached sessions for servers that are no longer
    /// present in `config` are shut down and dropped.
    pub fn update_config(&self, config: &AppConfig) {
        let mut state = match self.state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let new_servers = config.mcp_servers.clone();

        // Shut down and drop sessions whose server is gone or whose
        // config changed (cheap correctness: a different command or
        // URL is a different server).
        let stale: Vec<String> = state
            .sessions
            .keys()
            .filter(|name| {
                !new_servers.contains_key(*name)
                    || new_servers.get(*name) != state.servers.get(*name)
            })
            .cloned()
            .collect();
        for name in stale {
            if let Some(session) = state.sessions.remove(&name) {
                tracing::info!(
                    server = %name,
                    "dropping MCP session: server removed from config or config changed"
                );
                session.shutdown();
            }
        }

        // Log the diff so an operator can see what changed.
        let added: Vec<&String> = new_servers
            .keys()
            .filter(|k| !state.servers.contains_key(*k))
            .collect();
        let removed: Vec<&String> = state
            .servers
            .keys()
            .filter(|k| !new_servers.contains_key(*k))
            .collect();
        let changed: Vec<&String> = state
            .servers
            .keys()
            .filter(|k| {
                new_servers.contains_key(*k) && new_servers.get(*k) != state.servers.get(*k)
            })
            .collect();
        if !added.is_empty() || !removed.is_empty() || !changed.is_empty() {
            tracing::info!(
                added = added.len(),
                removed = removed.len(),
                changed = changed.len(),
                "MCP config updated"
            );
        }

        state.servers = new_servers.clone();

        // Initialize/clean up in-memory needs_auth flags for current servers.
        state
            .needs_auth
            .retain(|name, _| new_servers.contains_key(name));
        for name in new_servers.keys() {
            state.needs_auth.entry(name.clone()).or_insert(false);
        }
    }

    /// Eagerly initialize the session for a given server. Returns
    /// the cached [`McpClientSession`] so callers can inspect the
    /// negotiated protocol version, server info, etc. Idempotent.
    pub fn initialize_server(&self, server_name: &str) -> Result<Arc<McpClientSession>, String> {
        let session = self.get_or_create_session(server_name)?;
        session.ensure_initialized().map_err(|e| e.to_string())?;
        Ok(session)
    }

    /// Discover the tools advertised by a single MCP server by
    /// running `tools/list` against it. Performs the init handshake
    /// lazily if the session is not yet active.
    ///
    /// Returns one [`McpToolDescriptor`] per tool the server
    /// currently exposes. Errors (transport, JSON-RPC, or
    /// validation) are surfaced so callers can decide whether to
    /// skip the server or surface the failure.
    pub fn discover_tools(&self, server_name: &str) -> Result<Vec<McpToolDescriptor>, String> {
        let session = self.get_or_create_session(server_name)?;
        session.list_tools().map_err(|e| e.to_string())
    }

    /// Health-check a single MCP server by issuing a `ping` request.
    /// Performs the init handshake lazily. Used to verify the
    /// server is still alive before kicking off a long call.
    pub fn ping(&self, server_name: &str) -> Result<(), String> {
        let session = self.get_or_create_session(server_name)?;
        session.ping().map_err(|e| e.to_string())
    }

    /// Execute a tool call (`tools/call`) on the specified MCP server.
    /// Performs the init handshake lazily on first use.
    pub fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_tool_with_timeout(server_name, tool_name, arguments, DEFAULT_REQUEST_TIMEOUT)
    }

    /// Same as [`McpClients::call_tool`] but with a
    /// caller-supplied per-call timeout. Spec §2.5: "SDKs SHOULD
    /// allow per-request timeout configuration." This is the
    /// recommended entry point for tools whose expected runtime
    /// varies widely (e.g. long batch jobs vs. quick lookups).
    pub fn call_tool_with_timeout(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, String> {
        let session = self.get_or_create_session(server_name)?;

        let start = std::time::Instant::now();
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });
        let result = session.call_request_with_timeout("tools/call", params, timeout);
        let elapsed = start.elapsed();

        match &result {
            Ok(value) => {
                tracing::info!(
                    server = %server_name,
                    tool = %tool_name,
                    elapsed = ?elapsed,
                    timeout = ?timeout,
                    "MCP tool execution completed successfully"
                );
                Ok(value.clone())
            }
            Err(err) => {
                tracing::error!(
                    server = %server_name,
                    tool = %tool_name,
                    elapsed = ?elapsed,
                    timeout = ?timeout,
                    error = %err,
                    "MCP tool execution failed"
                );
                Err(err.to_string())
            }
        }
    }

    /// Gracefully shut down all active sessions. Safe to call
    /// multiple times.
    pub fn shutdown(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        for (_, session) in state.sessions.drain() {
            session.shutdown();
        }
    }

    /// Snapshot of the currently-configured server names.
    pub fn configured_servers(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|s| s.servers.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether the given server config is eligible for the OAuth 2.1
    /// flow. Returns `true` only for `McpServerConfig::Sse` servers
    /// with no static `Authorization` header (per MCP-020).
    pub fn needs_authentication(config: &McpServerConfig) -> bool {
        match config {
            McpServerConfig::Stdio { .. } => false,
            McpServerConfig::Sse { headers, .. } => !headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("authorization")),
        }
    }

    /// Trigger the OAuth 2.1 flow for a server by issuing a probe
    /// request. The existing 401→OAuth path in
    /// [`McpClientSession`] handles the round-trip transparently.
    ///
    /// Returns:
    /// - `Ok(())` if a token is now available, or the server does
    ///   not need auth.
    /// - `Err(msg)` if the server is not configured, if the server
    ///   is not eligible for the OAuth flow (stdio or static-auth
    ///   sse), or if the probe failed.
    ///
    /// Per MCP-021, callers should translate the error into a
    /// `ToolGroupError { kind: Authentication, ... }` via
    /// `ToolRegistry::record_error`.
    pub fn authenticate(&self, server_name: &str) -> Result<(), String> {
        // Look up the server config.
        let cfg = {
            let state = self
                .state
                .lock()
                .map_err(|e| format!("Failed to lock MCP manager state: {e}"))?;
            state
                .servers
                .get(server_name)
                .map(|e| e.config.clone())
                .ok_or_else(|| format!("MCP server '{server_name}' is not configured."))?
        };
        if !Self::needs_authentication(&cfg) {
            return Err(format!(
                "server '{server_name}' does not require authentication (stdio transport or static Authorization header present)"
            ));
        }

        // Install token store if not already installed (lazy init on first authenticate).
        if self.token_store().is_none() {
            let config_dir = get_config_path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            let store = TokenStore::open(&config_dir).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to open MCP token store, using in-memory store");
                TokenStore::in_memory()
            });
            self.set_token_store(Some(Arc::new(store)));
        }

        // Recreate the session with the new token store so it picks up OAuth.
        {
            let mut state = self
                .state
                .lock()
                .map_err(|e| format!("Failed to lock MCP manager state: {e}"))?;
            if let Some(session) = state.sessions.remove(server_name) {
                session.shutdown();
            }
        }

        // Probe by calling `tools/list` — the existing 401→OAuth
        // path in `McpClientSession` will run the flow transparently.
        let result = self
            .discover_tools(server_name)
            .map(|_| ())
            .map_err(|e| format!("OAuth flow for '{server_name}' failed: {e}"));
        // Whether the flow succeeded or failed, the server told us
        // it needs auth (else we wouldn't be here). Make sure the
        // entry's flag is set so the dialog button stays visible.
        if result.is_err() {
            self.mark_needs_auth(server_name, true);
        }
        result
    }

    /// Read the in-memory `needs_auth` flag. This is NOT persisted
    /// to YAML — it only lives in the manager's runtime state.
    /// Used by the [`crate::agent::tools::registry::ToolRegistry`] to populate
    /// [`ToolGroupState::needs_auth`](crate::agent::tools::registry::ToolGroupState::needs_auth).
    pub fn needs_auth_now(&self, server_name: &str) -> bool {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.needs_auth.get(server_name).copied())
            .unwrap_or(false)
    }

    /// Set or clear the in-memory `needs_auth` flag. Used by the
    /// dialog's `Forget` link and by [`McpClients::authenticate`].
    /// This is NOT persisted to YAML — it only lives in the manager's
    /// runtime state.
    pub fn mark_needs_auth(&self, server_name: &str, needs_auth: bool) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.needs_auth.insert(server_name.to_owned(), needs_auth);
    }

    fn get_or_create_session(&self, server_name: &str) -> Result<Arc<McpClientSession>, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("Failed to lock MCP manager state: {e}"))?;
        if let Some(session) = state.sessions.get(server_name) {
            return Ok(session.clone());
        }
        let cfg = state
            .servers
            .get(server_name)
            .map(|entry| entry.config.clone())
            .ok_or_else(|| format!("MCP server '{server_name}' is not configured."))?;
        let store = state.token_store.clone();
        let session = Arc::new(McpClientSession::new(cfg, store));
        state
            .sessions
            .insert(server_name.to_owned(), session.clone());
        Ok(session)
    }
}

impl Drop for McpClients {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl DynamicToolSource for McpClients {
    fn configured_servers(&self) -> Vec<String> {
        McpClients::configured_servers(self)
    }
    fn discover_tools(&self, server: &str) -> Result<Vec<McpToolDescriptor>, String> {
        McpClients::discover_tools(self, server)
    }
    fn update_config(&self, config: &AppConfig) {
        McpClients::update_config(self, config);
    }
    fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        McpClients::call_tool(self, server, tool_name, arguments)
    }
}
