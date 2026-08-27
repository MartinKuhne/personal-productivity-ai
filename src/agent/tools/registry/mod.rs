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
pub mod errors;
pub mod groups;
pub mod pagination;

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;

pub use crate::tools::cache;
pub use crate::tools::cache::{
    CACHE_TTL, CURSOR_EXPIRED_ERROR, CachedWebDocument, FINAL_PAGE_HINT, MAX_CACHE_ENTRIES,
    SearchEmailItem, ToolCache,
};
pub use crate::tools::cursor;
pub use crate::tools::cursor::{CursorPage, CursorSessionManager, PagedDataset};
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
use std::collections::{BTreeMap, HashMap};
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
    /// The single source of truth for group error status
    group_errors: HashMap<ToolGroupId, ToolGroupError>,
    /// MCP client manager — owns transport, sessions, and OAuth.
    mcp_manager: Arc<McpClients>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("group_errors", &self.group_errors)
            .field("mcp_manager", &"<McpClients>")
            .finish()
    }
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
            group_errors: HashMap::new(),
            mcp_manager,
        };
        builtin::register_all_builtins(&mut mgr);
        mgr
    }

    // ---- Registration ----

    /// Register a tool from a [`RegisteredTool`] entry. The
    /// entry's [`crate::tools::ToolDescriptor`] is the source of metadata; the
    /// group is taken from the descriptor so the call site doesn't
    /// have to know which family a tool belongs to.
    pub fn register(&mut self, entry: RegisteredTool) {
        self.tools.insert(entry.descriptor.name.to_string(), entry);
    }

    /// Convenience for registering a newly-discovered MCP tool. Wraps
    /// the dynamic descriptor and executor in a [`RegisteredTool`] and
    /// inserts it into the catalog.
    pub fn register_mcp_tool(
        &mut self,
        server_name: impl Into<String>,
        tool_name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) {
        let server_name = server_name.into();
        let remote_name = tool_name.into();
        let prefixed_name = format!("{}/{}", server_name, remote_name);

        let adapter = Arc::new(McpToolAdapter::from_dynamic_source(
            &server_name,
            &prefixed_name,
            &remote_name,
            description,
            parameters,
            self.mcp_manager.clone(),
        ));
        let descriptor = adapter.descriptor().clone();
        self.tools.insert(
            prefixed_name,
            RegisteredTool {
                descriptor: Arc::new(descriptor),
                executor: adapter,
            },
        );
    }

    // ---- Catalog queries (used by the agent loop) ----

    /// Look up a tool executor by name.
    pub fn get_executor(&self, name: &str) -> Option<Arc<dyn crate::tools::Tool>> {
        self.tools.get(name).map(|t| t.executor.clone())
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
        self.tools.get(name).map(|t| t.descriptor.group.clone())
    }

    pub fn group_of(&self, name: &str) -> Option<ToolGroupId> {
        self.tools.get(name).map(|t| t.descriptor.group.clone())
    }
    /// Parallel-safety classification of a tool. Unknown names are
    /// conservatively classified as [`Safety::Mutating`].
    pub fn safety_of(&self, name: &str) -> Safety {
        self.descriptor(name)
            .map(|d| d.safety)
            .unwrap_or(Safety::Mutating)
    }

    /// Execute a tool by name (called by `ToolExecutor`).
    pub fn execute_tool(
        &self,
        ctx: &crate::tools::context::ToolContext,
        name: &str,
        args: &str,
    ) -> Result<serde_json::Value, crate::tools::ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| crate::tools::ToolError::new(format!("Tool {name} not found.")))?;
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

    /// Build the JSON-Schema tool list for the LLM, honouring both
    /// the statically declared feature-flag requirements and the
    /// prompt-content gating rules.
    pub fn schema_len(&self, config: &crate::config::AgentConfig, prompt: &str) -> usize {
        self.tools
            .keys()
            .filter(|name| self.schema_fragment(name, config, prompt).is_some())
            .count()
    }

    /// Retrieve the full JSON schema array for all enabled tools.
    /// This is the payload sent to the LLM.
    pub fn get_schema(
        &self,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> serde_json::Value {
        let mut tools_schema = Vec::new();

        // 1) All internal and discovered tools.
        for tool_name in self.tools.keys() {
            if let Some(mut fragment) = self.schema_fragment(tool_name, config, prompt) {
                // If it's a dynamic MCP tool, the executor might have additional schema validation rules
                if let Some(mcp) = self.tools.get(tool_name) {
                    #[allow(clippy::collapsible_if)]
                    if let ToolGroupId::Mcp(server_name) = &mcp.descriptor.group {
                        fragment["mcp_server"] = serde_json::json!(server_name);
                    }
                }
                tools_schema.push(fragment);
            }
        }

        // Return array of objects.
        serde_json::Value::Array(tools_schema)
    }

    /// Returns the exact number of bytes the tool schema will consume
    /// for a single tool. Used by the tools dialog UI to display a
    /// cost/budget hint.
    pub fn tool_char_count(
        &self,
        name: &str,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> Option<usize> {
        let entry = self.schema_fragment(name, config, prompt)?;
        Some(serde_json::to_string(&entry).map(|s| s.len()).unwrap_or(0))
    }

    // ---- Group State (used by UI) ----

    /// Return a dynamically computed snapshot of the UI state for the tools dialog.
    pub fn groups(&self, config: &crate::config::AgentConfig) -> Vec<ToolGroupState> {
        let mut groups: HashMap<ToolGroupId, ToolGroupState> = HashMap::new();

        // 1) Group all known tools by their group_id.
        for (tool_name, tool) in &self.tools {
            let group_id = &tool.descriptor.group;
                let entry = groups.entry(group_id.clone()).or_insert_with(|| {
                    let name = match group_id {
                        ToolGroupId::Internal(g) => g.display_name().to_owned(),
                        ToolGroupId::Mcp(name) => name.clone(),
                    };
                    let kind = match group_id {
                        ToolGroupId::Internal(_) => ToolGroupKind::Internal,
                        ToolGroupId::Mcp(n) => match config.mcp_servers.get(n).map(|e| e.config()) {
                            Some(McpServerConfig::Sse { .. }) => ToolGroupKind::McpRemote,
                            _ => ToolGroupKind::McpStdio,
                        },
                    };
                    ToolGroupState {
                        id: group_id.clone(),
                        display_name: name,
                        kind,
                        enabled: crate::tools::descriptor::group_enabled(config, group_id),
                        needs_auth: match group_id {
                            ToolGroupId::Mcp(n) => self.mcp_manager.needs_auth_now(n),
                            _ => false,
                        },
                        tool_names: Vec::new(),
                        parallel_safe: true,
                        last_error: self.group_errors.get(group_id).cloned(),
                    }
                });
                entry.tool_names.push(tool_name.clone());
                if tool.descriptor.safety != Safety::ReadOnly {
                    entry.parallel_safe = false;
                }
        }

        // Sort tool names for stable UI.
        for s in groups.values_mut() {
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
            groups.entry(id.clone()).or_insert_with(|| ToolGroupState {
                id: id.clone(),
                display_name: name.clone(),
                kind: transport_kind,
                enabled: entry.is_enabled(),
                needs_auth: self.mcp_manager.needs_auth_now(name),
                tool_names: Vec::new(),
                parallel_safe: true,
                last_error: self.group_errors.get(&id).cloned(),
            });
        }

        let mut res: Vec<_> = groups.into_values().collect();
        // Keep MCP groups sorted by name, but internal tools at the top.
        res.sort_by(|a, b| match (&a.id, &b.id) {
            (ToolGroupId::Internal(ia), ToolGroupId::Internal(ib)) => ia.cmp(ib),
            (ToolGroupId::Internal(_), ToolGroupId::Mcp(_)) => std::cmp::Ordering::Less,
            (ToolGroupId::Mcp(_), ToolGroupId::Internal(_)) => std::cmp::Ordering::Greater,
            (ToolGroupId::Mcp(na), ToolGroupId::Mcp(nb)) => na.cmp(nb),
        });
        res
    }

    /// Single-group lookup.
    pub fn group(&self, id: &ToolGroupId, config: &crate::config::AgentConfig) -> Option<ToolGroupState> {
        self.groups(config).into_iter().find(|g| g.id == *id)
    }

    pub fn mcp_manager(&self) -> Arc<McpClients> {
        self.mcp_manager.clone()
    }

    pub async fn init_mcp_on_startup(&mut self, config: &AgentConfig) {
        self.mcp_manager.update_config(config);
        self.refresh_mcp_tools(config).await;
    }

    pub fn update_and_refresh(&mut self, config: &crate::config::AgentConfig) {
        self.mcp_manager.update_config(config);
        crate::tools::blocking::block_on(async { self.refresh_mcp_tools(config).await });
    }

    pub fn get_tools_schema(
        &self,
        config: &crate::config::AgentConfig,
        prompt: &str,
    ) -> serde_json::Value {
        self.get_schema(config, prompt)
    }

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

    /// Clear any persistent error associated with this group.
    pub fn clear_error(&mut self, group_id: &ToolGroupId) {
        self.group_errors.remove(group_id);
    }

    /// Record a persistent error against this group (so the UI can
    /// show a ⚠ icon instead of silently hiding the tools).
    pub fn record_error(&mut self, group_id: &ToolGroupId, err: ToolGroupError) {
        self.group_errors.insert(group_id.clone(), err);
    }

    // ---- MCP lifecycle ----

    /// Reconciles `self.tools` with `config.mcp_servers` by calling
    /// `tools/list` on any valid servers.
    ///
    /// This MUST be called asynchronously from a task or inside `block_on`.
    pub async fn refresh_mcp_tools(&mut self, config: &crate::config::AgentConfig) {
        // Remove tools that came from a server whose config is gone or
        // whose config changed.
        let valid_servers: Vec<String> = config.mcp_servers.keys().cloned().collect();
        self.tools.retain(|_name, entry| {
            match &entry.descriptor.group {
                ToolGroupId::Mcp(server) => valid_servers.contains(server),
                _ => true, // keep internal tools
            }
        });

        for server_name in valid_servers {
            match self.mcp_manager.discover_tools(&server_name).await {
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
                    // dialog can surface it.
                    self.record_error(
                        &ToolGroupId::Mcp(server_name.clone()),
                        ToolGroupError::now(
                            ToolErrorKind::Discovery,
                            format!("tools/list failed: {e}"),
                        ),
                    );
                    continue; // Do not abort discovery for remaining servers
                }
            }
        }
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
            Err(e) => ToolOutcome::Err(e),
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
