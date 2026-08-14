//! Tool manager — the single source of truth for the LLM tool catalog,
//! per-group state, error tracking, and parallel-safety classification.
//! Replaces the previous `ToolRegistry` (TOOL-014..024).
//!
//! The manager owns:
//! - the catalog of built-in and MCP-discovered tools,
//! - the per-group enable / parallel-safe / error state,
//! - the [`McpClients`]
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

use crate::config::AgentConfig;
use crate::config::McpServerConfig;
use crate::lib::mcp::{McpClients, McpToolDescriptor};
use crate::tools::RegisteredTool;
use crate::tools::context::ToolContext;
use crate::tools::mcp::McpToolAdapter;
use crate::tools::{Safety, Tool, ToolDispatcher, ToolOutcome};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Central catalog of agent tools, both built-in and dynamic MCP tools,
/// plus per-group state for the UI.
#[derive(Clone)]
pub struct ToolRegistry {
    /// Registered tools keyed by name. Each entry pairs the static
    /// [`crate::tools::ToolDescriptor`] (used by the LLM
    /// schema, the UI dialog, and the prompt char-count) with the
    /// `Arc<dyn Tool>` executor (used by the dispatcher at run
    /// time). See [`crate::tools::RegisteredTool`].
    tools: BTreeMap<String, RegisteredTool>,
    /// Reverse index: which group owns each tool? Built at registration.
    tool_to_group: BTreeMap<String, ToolGroupId>,
    /// Per-group state, rebuilt by [`ToolRegistry::refresh_state`].
    group_state: BTreeMap<ToolGroupId, ToolGroupState>,
    /// MCP client manager — owns transport, sessions, and OAuth.
    mcp_manager: Arc<McpClients>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new manager and register every built-in tool.
    pub fn new() -> Self {
        let mcp_manager = Arc::new(McpClients::new());
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

    /// Register a tool from a [`RegisteredTool`] entry. The
    /// entry's [`crate::tools::ToolDescriptor`] is the source of metadata; the
    /// group is taken from the descriptor so the call site doesn't
    /// have to know which family a tool belongs to. The legacy
    /// `Box<dyn Tool>` registration path is gone — providers hand
    /// the registry pre-built entries.
    pub fn register_registered_tool(&mut self, entry: RegisteredTool) {
        let name = entry.descriptor.name.to_string();
        let group = entry.descriptor.group.clone();
        self.tool_to_group.insert(name.clone(), group);
        self.tools.insert(name, entry);
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
        let adapter = Arc::new(McpToolAdapter::from_dynamic_source(
            &server_name,
            &tool_name,
            description,
            parameters,
            self.mcp_manager.clone(),
        ));
        let descriptor = adapter.descriptor().clone();
        self.tool_to_group
            .insert(tool_name.clone(), ToolGroupId::Mcp(server_name));
        self.tools.insert(
            tool_name,
            RegisteredTool {
                descriptor: Arc::new(descriptor),
                executor: adapter,
            },
        );
    }

    // ---- Catalog queries (used by the agent loop) ----

    /// Look up a tool executor by name.
    pub fn tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.executor.as_ref())
    }

    /// Look up the static [`crate::tools::ToolDescriptor`]
    /// for a tool by name. Used by the LLM schema, the UI dialog,
    /// and the prompt char-count.
    pub fn descriptor(&self, name: &str) -> Option<&crate::tools::ToolDescriptor> {
        self.tools.get(name).map(|t| t.descriptor.as_ref())
    }

    /// Look up the group that owns a given tool. Returns `None` for
    /// unknown tools.
    pub fn tool_group(&self, name: &str) -> Option<ToolGroupId> {
        self.tool_to_group.get(name).cloned()
    }

    /// Parallel-safety classification of a tool. Unknown names are
    /// conservatively classified as [`Safety::Mutating`].
    pub fn safety_of(&self, name: &str) -> Safety {
        self.descriptor(name)
            .map(|d| d.safety)
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
        tool.executor.execute(ctx, args)
    }

    /// Build a single JSON-Schema entry (`{"type":"function",
    /// "function":{"name","description","parameters"}}`) for the
    /// given tool descriptor, or `None` if the tool is not
    /// currently enabled. The fragment is the source of truth for
    /// both [`ToolRegistry::get_schema`] and
    /// [`ToolRegistry::tool_char_count`] — see TOOL-015.
    pub fn schema_fragment(
        &self,
        name: &str,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> Option<serde_json::Value> {
        let entry = self.tools.get(name)?;
        if !entry.executor.is_enabled(config, prompt) {
            return None;
        }
        let mut params = entry.descriptor.parameters_schema.clone();
        if params.get("properties").is_none() {
            params["properties"] = serde_json::Value::Object(Default::default());
        }
        Some(serde_json::json!({
            "type": "function",
            "function": {
                "name": entry.descriptor.name,
                "description": entry.descriptor.description,
                "parameters": params
            }
        }))
    }

    /// Build the JSON-Schema tool list for the LLM, honouring both
    /// per-group enable flags and the prompt-content rule in
    /// [`Tool::is_enabled`].
    pub fn get_schema(
        &self,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> serde_json::Value {
        let mut tools = Vec::new();
        for name in self.tools.keys() {
            if let Some(fragment) = self.schema_fragment(name, config, prompt) {
                tools.push(fragment);
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
            .filter(|(_, t)| t.descriptor.safety == Safety::ReadOnly)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Length (in characters) of the JSON-Schema entry that a single
    /// tool contributes to the LLM `tools` array, or `None` if the
    /// tool is not currently enabled (per [`Tool::is_enabled`]).
    /// Per TOOL-015.
    pub fn tool_char_count(
        &self,
        name: &str,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> Option<usize> {
        let entry = self.schema_fragment(name, config, prompt)?;
        Some(serde_json::to_string(&entry).map(|s| s.len()).unwrap_or(0))
    }

    // ---- Group state (used by the UI dialog) ----

    /// Recompute the per-group view from the current `AppConfig` and
    /// the current catalog. Cheap (no I/O). Called on every config
    /// change and on every dialog open.
    pub fn refresh_state(&mut self, config: &crate::config::AgentConfig) {
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
            let safety = self
                .tools
                .get(tool_name)
                .map(|t| t.descriptor.safety)
                .unwrap_or(Safety::Mutating);
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

    pub fn groups_snapshot(&mut self, config: &AgentConfig) -> Vec<ToolGroupState> {
        self.refresh_state(config);
        self.groups()
    }

    pub fn mcp_manager(&self) -> Arc<McpClients> {
        self.mcp_manager.clone()
    }

    pub fn init_mcp_on_startup(&mut self, config: &AgentConfig) {
        self.mcp_manager.update_config(config);
        self.refresh_mcp_tools(config);
    }

    pub fn update_and_refresh(&mut self, config: &crate::config::AgentConfig) {
        self.mcp_manager.update_config(config);
        self.refresh_mcp_tools(config);
        self.refresh_state(config);
    }

    pub fn get_tools_schema(
        &mut self,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> serde_json::Value {
        self.mcp_manager.update_config(config);
        self.refresh_mcp_tools(config);
        self.refresh_state(config);
        self.get_schema(config, prompt)
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
    /// supplied `AgentConfig`. The change takes effect on the next
    /// [`ToolRegistry::get_schema`] call.
    pub fn set_group_enabled(&self, config: &mut AgentConfig, id: &ToolGroupId, enabled: bool) {
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
    /// error is cleared by passing `None` via [`ToolRegistry::clear_error`].
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
    pub fn refresh_mcp_tools(&mut self, config: &crate::config::AgentConfig) {
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

    /// Snapshot of the tool descriptors the manager currently knows
    /// about, useful for tests and the debug overlay.
    pub fn tool_descriptors(&self) -> Vec<McpToolDescriptor> {
        self.tools
            .values()
            .map(|t| McpToolDescriptor {
                name: t.descriptor.name.to_string(),
                description: t.descriptor.description.to_string(),
                input_schema: t.descriptor.parameters_schema.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ToolDispatcher impl — the registry acts as the dispatch surface
// for the executor and for any tool that needs to invoke another
// tool by name.
// ---------------------------------------------------------------------------

impl ToolDispatcher for ToolRegistry {
    fn dispatch(&self, name: &str, args: &str, ctx: &ToolContext) -> ToolOutcome {
        let tool = match self.tools.get(name) {
            Some(t) => t,
            None => return ToolOutcome::err(format!("Tool {name} not found.")),
        };
        match tool.executor.execute(ctx, args) {
            Ok(value) => ToolOutcome::ok(value),
            Err(message) => ToolOutcome::err(message),
        }
    }

    fn safety(&self, name: &str) -> Safety {
        self.safety_of(name)
    }
}

// ---------------------------------------------------------------------------
// Group-enabled helpers — thin wrappers over the AppConfig fields so the
// `set_group_enabled` method can be a single `match`.
// ---------------------------------------------------------------------------

fn is_internal_group_enabled(config: &crate::config::AgentConfig, g: InternalToolGroup) -> bool {
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
        Trello => config.tool_groups.trello,
    }
}

fn set_internal_group_enabled(config: &mut AgentConfig, g: InternalToolGroup, on: bool) {
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
        Trello => config.tool_groups.trello = on,
    }
}

pub fn execute_tool(
    dispatcher: &dyn ToolDispatcher,
    ctx: &ToolContext,
    name: &str,
    args_str: &str,
) -> String {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let debug_mode = ctx
        .config
        .feature_flags()
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false);

    tracing::info!(name = "tool.manager.call", tool_name = %name, args = %args_str, "Executing tool call");
    let start_time = std::time::Instant::now();

    // Phase 1: dispatch through the dispatcher, catching any panic so
    // a buggy tool cannot kill the agent loop. The executor handles
    // the per-group error-recording phase (see `agent::tool_executor`).
    let result_raw: Result<serde_json::Value, String> =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatcher.dispatch(name, args_str, ctx).into_json_result()
        }))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };
            Err(format!("Tool {name} panicked: {msg}"))
        });

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
            crate::tools::dtos::ToolResponse::Success { data }
        }
        Err(err) => {
            tracing::error!(name = "tool.manager.failed", tool_name = %name, elapsed = ?elapsed, error = %err, "Tool execution failed. Operator should verify tool inputs.");
            crate::tools::dtos::ToolResponse::Error { message: err }
        }
    };

    serde_json::to_string(&response_dto).unwrap_or_else(|_| {
        r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string()
    })
}
