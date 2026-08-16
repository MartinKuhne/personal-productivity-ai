//! Tool-group taxonomy and per-group state. See `manager::mod` for the
//! owning [`ToolRegistry`](super::ToolRegistry).

use super::errors::ToolGroupError;

/// Stable identifier for a tool group. Sorted lexicographically for
/// deterministic UI rendering.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolGroupId {
    /// One of the seven built-in tool families.
    Internal(InternalToolGroup),
    /// A configured MCP server (name is the map key in
    /// [`AgentConfig::mcp_servers`](crate::config::AgentConfig::mcp_servers)).
    Mcp(String),
}

/// The eight built-in tool families. Mirrors
/// [`ToolGroupsConfig`](crate::config::ToolGroupsConfig).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InternalToolGroup {
    Filesystem,
    Web,
    Browser,
    Email,
    Contacts,
    Calendar,
    CsvDb,
    Weather,
    Trello,
}

impl InternalToolGroup {
    /// Human-readable name for the UI.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Filesystem => "Filesystem",
            Self::Web => "Web",
            Self::Browser => "Browser",
            Self::Email => "Email",
            Self::Contacts => "Contacts",
            Self::Calendar => "Calendar",
            Self::CsvDb => "CSV Database",
            Self::Weather => "Weather",
            Self::Trello => "Trello",
        }
    }
}

/// Whether a group is built into the binary or provided by an MCP server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolGroupKind {
    Internal,
    McpStdio,
    McpRemote,
}

/// Per-group state, rebuilt by
/// [`ToolRegistry::refresh_state`](super::ToolRegistry::refresh_state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolGroupState {
    /// Stable identifier.
    pub id: ToolGroupId,
    /// Display name (the MCP server name for `Mcp` groups; the family
    /// name for `Internal` groups).
    pub display_name: String,
    /// Internal vs MCP.
    pub kind: ToolGroupKind,
    /// Whether the group is currently enabled (read from
    /// [`AgentConfig::tool_groups`](crate::config::AgentConfig::tool_groups)
    /// or
    /// [`McpServerEntry::enabled`](crate::config::McpServerEntry::enabled)).
    pub enabled: bool,
    /// For MCP groups: whether the server has been observed to
    /// require authentication (a 401 was received). The flag is
    /// set by the MCP client when a 401 is observed; cleared by the
    /// dialog's `Forget` link. Read from
    /// [`McpClients::needs_auth_now`](crate::lib::mcp::McpClients::needs_auth_now)
    /// so the manager's own state is the source of truth at
    /// runtime.
    pub needs_auth: bool,
    /// Names of the tools in this group, sorted alphabetically.
    pub tool_names: Vec<String>,
    /// `true` iff every tool in the group has
    /// [`Safety::ReadOnly`](super::super::Safety::ReadOnly).
    pub parallel_safe: bool,
    /// Most recent error, if any. Cleared by
    /// [`ToolRegistry::clear_error`](super::ToolRegistry::clear_error) or
    /// by a successful `Execution` (per TOOL-021).
    pub last_error: Option<ToolGroupError>,
}

impl ToolGroupState {
    /// Sum of the per-tool JSON-Schema lengths (in characters) of every
    /// enabled tool in the group — i.e. the bytes the group contributes
    /// to the LLM `tools` array (per TOOL-015/016).
    ///
    /// `char_count_for_tool` is supplied by the caller because the
    /// `ToolGroupState` is intentionally egui- and JSON-free.
    pub fn prompt_char_count(&self, char_count_for_tool: &dyn Fn(&str) -> Option<usize>) -> usize {
        self.tool_names
            .iter()
            .filter_map(|n| char_count_for_tool(n))
            .sum()
    }
}
