//! Tool manager — the single source of truth for the LLM tool catalog,
//! per-group state, error tracking, and parallel-safety classification.
//! Replaces the previous `ToolRegistry` (TOOL-014..024).
//!
//! The manager owns:
//! - the catalog of built-in and MCP-discovered tools,
//! - the per-group enable / parallel-safe / error state,
//! - the [`McpClientManager`]
//!   used to discover and call MCP tools.
//!
//! Free functions at the bottom of this file keep the same signatures
//! as the previous `registry::mod` free functions so the agent loop
//! and tests don't notice the refactor.

pub mod builtin;
pub mod cache;
pub mod errors;
pub mod groups;
pub mod pagination;

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;

pub use errors::{ToolErrorKind, ToolGroupError};
pub use groups::{InternalToolGroup, ToolGroupId, ToolGroupKind, ToolGroupState};
pub use pagination::paginate_in_range;

use crate::agent::tools::context::ToolContext;
use crate::agent::tools::mcp::{McpClientManager, McpToolDescriptor};
use crate::agent::tools::{Safety, Tool};
use crate::app::background::{BackgroundLogEntry, LogCategory};
use crate::bus::config::CONFIG_ARRIVAL_TIMEOUT;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::typed::BackgroundEvent;
use crate::config::{AppConfig, McpServerConfig};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::Sender;

/// Central catalog of agent tools, both built-in and dynamic MCP tools,
/// plus per-group state for the UI.
pub struct ToolManager {
    /// Registered tools keyed by name.
    tools: BTreeMap<String, Arc<dyn Tool>>,
    /// Reverse index: which group owns each tool? Built at registration.
    tool_to_group: BTreeMap<String, ToolGroupId>,
    /// Per-group state, rebuilt by [`ToolManager::refresh_state`].
    group_state: BTreeMap<ToolGroupId, ToolGroupState>,
    /// MCP client manager — owns transport, sessions, and OAuth.
    mcp_manager: Arc<McpClientManager>,
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolManager {
    /// Create a new manager and register every built-in tool.
    pub fn new() -> Self {
        let mcp_manager = Arc::new(McpClientManager::new());
        let mut mgr = Self {
            tools: BTreeMap::new(),
            tool_to_group: BTreeMap::new(),
            group_state: BTreeMap::new(),
            mcp_manager,
        };
        builtin::register_all_builtins(&mut mgr);
        mgr
    }

    // ---- Registration ----

    /// Register a built-in tool with the given group.
    pub fn register_builtin(&mut self, group: InternalToolGroup, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tool_to_group
            .insert(name.clone(), ToolGroupId::Internal(group));
        self.tools.insert(name, Arc::from(tool));
    }

    /// Register a dynamic MCP tool into this manager.
    pub fn register_mcp_tool(
        &mut self,
        server_name: impl Into<String>,
        tool_name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) {
        let server_name = server_name.into();
        let tool_name = tool_name.into();
        let adapter = crate::agent::tools::mcp::McpToolAdapter::from_dynamic_source(
            &server_name,
            &tool_name,
            description,
            parameters,
            self.mcp_manager.clone(),
        );
        self.tool_to_group
            .insert(tool_name.clone(), ToolGroupId::Mcp(server_name));
        self.tools.insert(tool_name, Arc::new(adapter));
    }

    // ---- Catalog queries (used by the agent loop) ----

    /// Look up a tool by name.
    pub fn tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Look up the group that owns a given tool. Returns `None` for
    /// unknown tools.
    pub fn tool_group(&self, name: &str) -> Option<ToolGroupId> {
        self.tool_to_group.get(name).cloned()
    }

    /// Parallel-safety classification of a tool. Unknown names are
    /// conservatively classified as [`Safety::Mutating`].
    pub fn safety_of(&self, name: &str) -> Safety {
        self.tools
            .get(name)
            .map(|t| t.safety())
            .unwrap_or(Safety::Mutating)
    }

    /// Execute a tool by name.
    pub fn execute(
        &self,
        ctx: &ToolContext,
        name: &str,
        args: &str,
    ) -> Result<serde_json::Value, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool {name} not found."))?;
        tool.execute(ctx, args)
    }

    /// Build the JSON-Schema tool list for the LLM, honouring both
    /// per-group enable flags and the prompt-content rule in
    /// [`Tool::is_enabled`].
    pub fn get_schema(&self, config: &AppConfig, prompt: &str) -> serde_json::Value {
        let mut tools = Vec::new();
        for tool in self.tools.values() {
            if tool.is_enabled(config, prompt) {
                let mut params = tool.parameters_schema();
                if params.get("properties").is_none() {
                    params["properties"] = serde_json::Value::Object(Default::default());
                }
                tools.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": params
                    }
                }));
            }
        }
        serde_json::Value::Array(tools)
    }

    /// Names of every tool that is parallel-safe (i.e. classified as
    /// [`Safety::ReadOnly`]). Used by the agent loop to identify the
    /// "safe" set up-front.
    pub fn parallel_safe_tools(&self) -> Vec<String> {
        self.tools
            .iter()
            .filter(|(_, t)| t.safety() == Safety::ReadOnly)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Length (in characters) of the JSON-Schema entry that a single
    /// tool contributes to the LLM `tools` array, or `None` if the
    /// tool is not currently enabled (per [`Tool::is_enabled`]).
    ///
    /// The returned byte count matches what
    /// [`ToolManager::get_schema`] would serialise for the tool —
    /// `{"type":"function","function":{"name","description","parameters"}}`.
    /// Per TOOL-015.
    pub fn tool_char_count(&self, name: &str, config: &AppConfig, prompt: &str) -> Option<usize> {
        let tool = self.tools.get(name)?;
        if !tool.is_enabled(config, prompt) {
            return None;
        }
        let mut params = tool.parameters_schema();
        if params.get("properties").is_none() {
            params["properties"] = serde_json::Value::Object(Default::default());
        }
        let entry = serde_json::json!({
            "type": "function",
            "function": {
                "name": tool.name(),
                "description": tool.description(),
                "parameters": params
            }
        });
        Some(serde_json::to_string(&entry).map(|s| s.len()).unwrap_or(0))
    }

    // ---- Group state (used by the UI dialog) ----

    /// Recompute the per-group view from the current `AppConfig` and
    /// the current catalog. Cheap (no I/O). Called on every config
    /// change and on every dialog open.
    pub fn refresh_state(&mut self, config: &AppConfig) {
        // For each known group, rebuild the state from the catalog
        // and the config. We carry forward the `last_error` from the
        // previous view: a successful `Execution` clears it via
        // `record_error` (see TOOL-021); UI `clear_error` and
        // successful `refresh_mcp_tools` for `Discovery` / `ConfigInvalid`
        // also clear it.
        let prev = std::mem::take(&mut self.group_state);

        // 1) Build a set of every group the catalog knows about.
        let mut next: BTreeMap<ToolGroupId, ToolGroupState> = BTreeMap::new();
        for (tool_name, group_id) in &self.tool_to_group {
            let entry = next.entry(group_id.clone()).or_insert_with(|| {
                let (display_name, kind) = match group_id {
                    ToolGroupId::Internal(g) => {
                        (g.display_name().to_string(), ToolGroupKind::Internal)
                    }
                    ToolGroupId::Mcp(name) => {
                        // Determine transport type from config to show
                        // "MCP (stdio)" vs "MCP (remote)" in the UI.
                        let transport_kind = config
                            .mcp_servers
                            .get(name)
                            .map(|e| match e.config() {
                                McpServerConfig::Stdio { .. } => ToolGroupKind::McpStdio,
                                McpServerConfig::Sse { .. } => ToolGroupKind::McpRemote,
                            })
                            .unwrap_or(ToolGroupKind::McpRemote);
                        (name.clone(), transport_kind)
                    }
                };
                let enabled = match group_id {
                    ToolGroupId::Internal(g) => is_internal_group_enabled(config, *g),
                    ToolGroupId::Mcp(name) => {
                        config.mcp_servers.get(name).is_some_and(|e| e.is_enabled())
                    }
                };
                // `needs_auth` lives in the MCP client manager's own
                // state (set when a 401 is observed). Internal groups
                // never need auth.
                let needs_auth = match group_id {
                    ToolGroupId::Internal(_) => false,
                    ToolGroupId::Mcp(name) => self.mcp_manager.needs_auth_now(name),
                };
                ToolGroupState {
                    id: group_id.clone(),
                    display_name,
                    kind,
                    enabled,
                    needs_auth,
                    tool_names: Vec::new(),
                    parallel_safe: true,
                    last_error: prev.get(group_id).and_then(|s| s.last_error.clone()),
                }
            });
            entry.tool_names.push(tool_name.clone());
            let tool = self.tools.get(tool_name).map(|t| t.as_ref());
            let safety = tool.map(|t| t.safety()).unwrap_or(Safety::Mutating);
            if safety != Safety::ReadOnly {
                entry.parallel_safe = false;
            }
        }

        // Sort tool names for stable UI.
        for s in next.values_mut() {
            s.tool_names.sort();
        }

        // 2) Ensure every configured-but-not-yet-discovered MCP server
        //    also has a row (so the UI shows it even before its first
        //    `tools/list`).
        for (name, entry) in &config.mcp_servers {
            let id = ToolGroupId::Mcp(name.clone());
            let transport_kind = match entry.config() {
                McpServerConfig::Stdio { .. } => ToolGroupKind::McpStdio,
                McpServerConfig::Sse { .. } => ToolGroupKind::McpRemote,
            };
            next.entry(id.clone()).or_insert_with(|| ToolGroupState {
                id,
                display_name: name.clone(),
                kind: transport_kind,
                enabled: entry.is_enabled(),
                needs_auth: self.mcp_manager.needs_auth_now(name),
                tool_names: Vec::new(),
                parallel_safe: true,
                last_error: prev
                    .get(&ToolGroupId::Mcp(name.clone()))
                    .and_then(|s| s.last_error.clone()),
            });
        }

        // 3) Drop `last_error` from groups that no longer exist
        //    (e.g. an MCP server that was removed from the config).
        //    Done by `std::mem::take` above; no further work.

        self.group_state = next;
    }

    /// All groups, sorted deterministically by id.
    pub fn groups(&self) -> Vec<ToolGroupState> {
        self.group_state.values().cloned().collect()
    }

    /// Single-group lookup.
    pub fn group(&self, id: &ToolGroupId) -> Option<&ToolGroupState> {
        self.group_state.get(id)
    }

    // ---- Group mutations (used by the UI dialog) ----

    /// Flip the enabled flag for a group, persisting into the
    /// supplied `AppConfig`. The change takes effect on the next
    /// [`ToolManager::get_schema`] call (after the caller writes
    /// `config` back to `config.yaml`).
    pub fn set_group_enabled(&self, config: &mut AppConfig, id: &ToolGroupId, enabled: bool) {
        match id {
            ToolGroupId::Internal(g) => set_internal_group_enabled(config, *g, enabled),
            ToolGroupId::Mcp(name) => {
                if let Some(entry) = config.mcp_servers.get_mut(name) {
                    entry.enabled = enabled;
                }
            }
        }
    }

    // ---- Error tracking ----

    /// Record a per-group error. Replaces any previous `last_error`
    /// for the same group. Per TOOL-021, a successful `Execution`
    /// error is cleared by passing `None` via [`ToolManager::clear_error`].
    pub fn record_error(&mut self, group: &ToolGroupId, err: ToolGroupError) {
        if let Some(state) = self.group_state.get_mut(group) {
            state.last_error = Some(err);
        }
    }

    /// Clear the recorded error for a group. Intended to be called by
    /// the UI "Restart" link (UI-060) or by the agent loop after a
    /// successful `Execution`.
    pub fn clear_error(&mut self, group: &ToolGroupId) {
        if let Some(state) = self.group_state.get_mut(group) {
            state.last_error = None;
        }
    }

    // ---- MCP lifecycle ----

    /// Refresh the MCP catalog: re-run `tools/list` against every
    /// configured server, register the discovered tools, and record
    /// `Discovery` errors on the affected group when a server fails.
    pub fn refresh_mcp_tools(&mut self, config: &AppConfig) {
        // Remove tools that came from a server whose config is gone or
        // whose config changed.
        let valid_servers: Vec<String> = config.mcp_servers.keys().cloned().collect();
        self.tools.retain(|name, _| {
            match self.tool_to_group.get(name) {
                Some(ToolGroupId::Mcp(server)) => valid_servers.contains(server),
                _ => true, // keep internal tools
            }
        });
        self.tool_to_group.retain(|_name, group| match group {
            ToolGroupId::Mcp(server) => valid_servers.contains(server),
            _ => true,
        });

        for server_name in valid_servers {
            match self.mcp_manager.discover_tools(&server_name) {
                Ok(tools) => {
                    // Clear any prior Discovery error on success.
                    self.clear_error(&ToolGroupId::Mcp(server_name.clone()));
                    for tool in tools {
                        self.register_mcp_tool(
                            server_name.clone(),
                            tool.name.clone(),
                            tool.description,
                            tool.input_schema,
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %e,
                        "Failed to discover tools from MCP server; skipping"
                    );
                    // Record a `Discovery` error on the group so the
                    // dialog can surface it. We need to `refresh_state`
                    // first so the group exists, then record.
                    self.refresh_state(config);
                    self.record_error(
                        &ToolGroupId::Mcp(server_name.clone()),
                        ToolGroupError::now(
                            ToolErrorKind::Discovery,
                            format!("tools/list failed: {e}"),
                        ),
                    );
                    return;
                }
            }
        }
        self.refresh_state(config);
    }

    /// Direct access to the MCP client manager for callers that need
    /// to invoke OAuth flows or inspect session state.
    pub fn mcp_manager(&self) -> &Arc<McpClientManager> {
        &self.mcp_manager
    }

    /// Snapshot of the tool descriptors the manager currently knows
    /// about, useful for tests and the debug overlay.
    pub fn tool_descriptors(&self) -> Vec<McpToolDescriptor> {
        self.tools
            .values()
            .map(|t| McpToolDescriptor {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.parameters_schema(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Group-enabled helpers — thin wrappers over the AppConfig fields so the
// `set_group_enabled` method can be a single `match`.
// ---------------------------------------------------------------------------

fn is_internal_group_enabled(config: &AppConfig, g: InternalToolGroup) -> bool {
    use InternalToolGroup::*;
    match g {
        Filesystem => config.tool_groups.filesystem,
        Web => config.tool_groups.web,
        Browser => config.tool_groups.browser,
        Email => config.tool_groups.email,
        Contacts => config.tool_groups.contacts,
        Calendar => config.tool_groups.calendar,
        CsvDb => config.tool_groups.csv_db,
        Weather => config.tool_groups.weather,
    }
}

fn set_internal_group_enabled(config: &mut AppConfig, g: InternalToolGroup, on: bool) {
    use InternalToolGroup::*;
    match g {
        Filesystem => config.tool_groups.filesystem = on,
        Web => config.tool_groups.web = on,
        Browser => config.tool_groups.browser = on,
        Email => config.tool_groups.email = on,
        Contacts => config.tool_groups.contacts = on,
        Calendar => config.tool_groups.calendar = on,
        CsvDb => config.tool_groups.csv_db = on,
        Weather => config.tool_groups.weather = on,
    }
}

// ---------------------------------------------------------------------------
// Global ToolManager + free functions. These keep the same signatures as
// the previous `registry::mod` free functions so the agent loop and
// `tool_executor` callers don't change at the call site — only the
// import path does.
// ---------------------------------------------------------------------------

static TOOL_MANAGER: std::sync::LazyLock<std::sync::RwLock<ToolManager>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(ToolManager::new()));

/// Snapshot the manager's group view for the UI dialog. Refreshes the
/// view from the supplied `config` first so the snapshot is in sync.
pub fn groups_snapshot(config: &AppConfig) -> Vec<ToolGroupState> {
    if let Ok(mut mgr) = TOOL_MANAGER.write() {
        mgr.refresh_state(config);
        mgr.groups()
    } else {
        Vec::new()
    }
}

/// Per-tool char count helper used by the UI dialog. Returns `None`
/// if the tool is not registered or not currently enabled.
pub fn tool_char_count_for(name: &str, config: &AppConfig, prompt: &str) -> Option<usize> {
    TOOL_MANAGER
        .read()
        .ok()
        .and_then(|m| m.tool_char_count(name, config, prompt))
}

/// Toggle a group's enabled flag in the supplied `AppConfig`. Does
/// not persist to disk — the UI dialog calls
/// [`crate::config::save_config`] after invoking this.
pub fn set_group_enabled(config: &mut AppConfig, id: &ToolGroupId, on: bool) {
    if let Ok(mgr) = TOOL_MANAGER.read() {
        mgr.set_group_enabled(config, id, on);
    }
}

/// Clear the recorded `last_error` for a group. Used by the UI
/// "Restart" link (UI-060).
pub fn clear_error(id: &ToolGroupId) {
    if let Ok(mut mgr) = TOOL_MANAGER.write() {
        mgr.clear_error(id);
    }
}

/// Record an `Authentication`-kind [`ToolGroupError`] on a group.
/// Called by the UI event handler when `McpAuthEvent::Completed`
/// arrives with an error so the Tools dialog row shows ⚠ + Clear.
pub fn record_mcp_error(id: &ToolGroupId, err: ToolGroupError) {
    if let Ok(mut mgr) = TOOL_MANAGER.write() {
        mgr.record_error(id, err);
    }
}

/// Read the entry's `needs_auth` flag. The dialog uses this to
/// decide whether to render the `Authenticate` button. The flag
/// lives in the manager's own state and is set when a 401 is
/// observed on the server.
pub fn mcp_needs_auth_now(server_name: &str) -> bool {
    TOOL_MANAGER
        .read()
        .ok()
        .map(|m| m.mcp_manager.needs_auth_now(server_name))
        .unwrap_or(false)
}

/// Clear the entry's `needs_auth` flag. Used by the dialog's
/// `Forget` link to tell the manager "we no longer need to
/// remember that this server required auth" (e.g. because the
/// user added a static `Authorization` header to the YAML).
pub fn mcp_clear_needs_auth(server_name: &str) {
    if let Ok(mgr) = TOOL_MANAGER.read() {
        mgr.mcp_manager.mark_needs_auth(server_name, false);
    }
}

/// Force a one-time MCP tool discovery for every configured
/// server. Used by the Tools dialog on its first frame so MCP
/// groups show their tools and prompt char count immediately
/// (without this, the MCP catalog is empty until the first
/// `get_tools_schema` call from the agent loop). Do NOT call
/// this on every frame — each call does a `tools/list` per
/// server, which is expensive over the network.
pub fn mcp_refresh_for_dialog(config: &AppConfig) {
    if let Ok(mut mgr) = TOOL_MANAGER.write() {
        mgr.mcp_manager.update_config(config);
        mgr.refresh_mcp_tools(config);
    }
}

/// Direct access to the global MCP client manager. The UI dialog
/// uses this to invoke [`crate::agent::tools::mcp::McpClientManager::authenticate`].
pub fn mcp_manager() -> Arc<McpClientManager> {
    TOOL_MANAGER
        .read()
        .map(|m| m.mcp_manager().clone())
        .unwrap_or_default()
}

/// Initialize the MCP subsystem on app start. Pings every configured
/// server and refreshes the MCP tool catalog. Returns the number of
/// servers that responded successfully.
///
/// OAuth token store is NOT installed here — it will be installed
/// lazily when the user clicks "Authenticate" for a server.
pub fn init_mcp_on_startup(config: &AppConfig) -> usize {
    let Ok(mut mgr) = TOOL_MANAGER.write() else {
        return 0;
    };
    mgr.mcp_manager.update_config(config);
    let ok = mgr.mcp_manager.ping_all_servers();
    mgr.refresh_mcp_tools(config);
    ok
}

/// Subscribe to the configuration-arrival bus and perform the
/// one-time MCP startup init on a background thread.
///
/// The subscription is registered before this returns, so callers may
/// publish the [`ConfigArrived`] event any time afterwards; the
/// spawned thread observes the first arrival (or falls back to
/// [`AppConfig::default`] if no event arrives within
/// [`CONFIG_ARRIVAL_TIMEOUT`]) and then runs the same startup path as
/// [`init_mcp_on_startup`]: it pushes the config into the MCP manager,
/// pings every configured server, and discovers each server's tools.
///
/// All network I/O happens off the UI thread so the window never
/// blocks on MCP servers at startup. A completion log entry is posted
/// to `tx` (the background-event channel) so the result shows up in
/// the UI's background log panel.
pub fn spawn_config_subscription(config_bus: Bus<ConfigArrived>, tx: Sender<BackgroundEvent>) {
    let config_reader = config_bus.subscribe();
    std::thread::spawn(move || {
        let config = match config_reader.recv_timeout(CONFIG_ARRIVAL_TIMEOUT) {
            Ok(event) => event.config,
            Err(_) => {
                tracing::error!(
                    name = "config.arrived.timeout",
                    timeout_ms = CONFIG_ARRIVAL_TIMEOUT.as_millis() as u64,
                    "No ConfigArrived event observed within timeout; using default configuration"
                );
                AppConfig::default()
            }
        };
        let servers_ok = init_mcp_on_startup(&config);
        let _ = tx.send(
            BackgroundLogEntry::new(
                LogCategory::Indexer,
                format!("MCP startup ping complete: {servers_ok} server(s) responded"),
            )
            .into(),
        );
    });
}

/// Look up a tool's [`Safety`] classification by name.
pub fn safety_of(name: &str) -> Safety {
    TOOL_MANAGER.read().unwrap().safety_of(name)
}

/// Retrieve the JSON Schema for all active tools.
pub fn get_tools_schema(config: &AppConfig, prompt: &str) -> serde_json::Value {
    let mut mgr = TOOL_MANAGER.write().unwrap_or_else(|e| e.into_inner());
    mgr.mcp_manager.update_config(config);
    mgr.refresh_mcp_tools(config);
    mgr.refresh_state(config);
    mgr.get_schema(config, prompt)
}

/// Execute a named tool with JSON arguments and return a serialized
/// [`ToolResponse`](crate::agent::tools::dtos::ToolResponse).
///
/// Per TOOL-021, the manager records an `Execution`-kind
/// [`ToolGroupError`] on the tool's group when the call returns
/// `Err`, and clears any prior `Execution` error on success. The
/// record/clear step uses a *separate* write lock from the call
/// itself, so MCP tool calls (which may do network I/O) don't hold
/// the write lock across the network round-trip.
pub fn execute_tool(ctx: &ToolContext, name: &str, args_str: &str) -> String {
    #[cfg(feature = "profiling")]
    puffin::profile_scope!("execute_tool");

    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let debug_mode = ctx
        .config
        .feature_flags
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false);

    tracing::info!(name = "tool.manager.call", tool_name = %name, args = %args_str, "Executing tool call");
    let start_time = std::time::Instant::now();

    // Phase 1: read-locked — do the I/O and snapshot the tool's group.
    let (result_raw, tool_group): (Result<serde_json::Value, String>, Option<ToolGroupId>) =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mgr = TOOL_MANAGER.read().unwrap();
            mgr.mcp_manager.update_config(ctx.config);
            let group = mgr.tool_group(name);
            let result = mgr.execute(ctx, name, args_str);
            (result, group)
        }))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };
            (Err(format!("Tool {name} panicked: {msg}")), None)
        });

    // Phase 2: write-locked — record/clear the Execution error.
    if let Some(group) = &tool_group {
        let mut mgr = TOOL_MANAGER.write().unwrap_or_else(|e| e.into_inner());
        match &result_raw {
            Ok(_) => mgr.clear_error(group),
            Err(msg) => mgr.record_error(
                group,
                ToolGroupError::now(ToolErrorKind::Execution, msg.clone()),
            ),
        }
    }

    let elapsed = start_time.elapsed();
    let response_dto = match result_raw {
        Ok(data) => {
            if debug_mode {
                let data_str = serde_json::to_string(&data)
                    .unwrap_or_else(|_| "<serialization error>".to_string());
                tracing::info!(name = "tool.manager.success", tool_name = %name, elapsed = ?elapsed, data = %data_str, "Tool execution succeeded");
            } else {
                tracing::info!(name = "tool.manager.success", tool_name = %name, elapsed = ?elapsed, "Tool execution succeeded");
            }
            crate::agent::tools::dtos::ToolResponse::Success { data }
        }
        Err(err) => {
            tracing::error!(name = "tool.manager.failed", tool_name = %name, elapsed = ?elapsed, error = %err, "Tool execution failed. Operator should verify tool inputs.");
            crate::agent::tools::dtos::ToolResponse::Error { message: err }
        }
    };

    serde_json::to_string(&response_dto).unwrap_or_else(|_| {
        r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string()
    })
}
