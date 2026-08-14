//! `ToolProvider` — a self-registering family of tools.
//!
//! A `ToolProvider` is the unit of registration. Each family
//! (filesystem, web, jmap, …) implements this trait and ships its
//! own list of [`RegisteredTool`]s. The registry constructor walks
//! the list of providers and registers each tool. The MCP
//! integration is just another provider whose `tools()` walks
//! `McpClients::discover_tools`.
//!
//! Existing call sites that register a single tool at a time
//! keep working via the legacy `ToolRegistry::register_builtin`
//! path, which wraps the tool in an implicit single-tool provider.

use std::sync::Arc;

use super::Tool;
use super::descriptor::ToolDescriptor;

/// A tool that has been bound to its [`ToolDescriptor`]. The
/// descriptor is the metadata the agent loop, the prompt builder,
/// and the UI dialog consume; the executor is the dyn-trait object
/// the dispatcher calls at run time.
pub struct RegisteredTool {
    /// Static metadata for the tool. Cheap to clone; may be shared
    /// freely with the UI, the prompt builder, and the schema
    /// fragment builders.
    pub descriptor: Arc<ToolDescriptor>,
    /// Executable instance. Shared across all parallel dispatches
    /// of the same tool inside a single turn.
    pub executor: Arc<dyn Tool>,
}

impl Clone for RegisteredTool {
    fn clone(&self) -> Self {
        Self {
            descriptor: Arc::clone(&self.descriptor),
            executor: Arc::clone(&self.executor),
        }
    }
}

impl std::fmt::Debug for RegisteredTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredTool")
            .field("name", &self.descriptor.name)
            .field("group", &self.descriptor.group)
            .field("safety", &self.descriptor.safety)
            .finish_non_exhaustive()
    }
}

impl RegisteredTool {
    /// Convenience constructor for tests and one-off registration
    /// sites. Production registration goes through a
    /// [`ToolProvider`].
    pub fn new(descriptor: ToolDescriptor, executor: Arc<dyn Tool>) -> Self {
        Self {
            descriptor: Arc::new(descriptor),
            executor,
        }
    }
}

/// Self-registering family of tools. Implementations describe their
/// group, ship a vector of [`RegisteredTool`]s, and optionally
/// refresh against a new `AppConfig` (used by the MCP provider to
/// re-discover tools after a config change).
pub trait ToolProvider: Send + Sync {
    /// Stable identifier for the provider (e.g. `"filesystem"`,
    /// `"jmap"`, `"mcp:<server-name>"`). Used in logs and for
    /// distinguishing multiple providers of the same kind.
    fn id(&self) -> &'static str;

    /// The group this provider's tools belong to.
    fn group(&self) -> crate::agent::tools::registry::groups::ToolGroupId;

    /// The tools this provider currently contributes. Called once
    /// at registry construction and again on
    /// [`ToolProvider::refresh`] after a relevant config change.
    fn tools(&self) -> Vec<RegisteredTool>;

    /// Re-emit `tools()` after a config change. Default is a
    /// no-op; MCP uses this to re-discover tools from the server.
    /// The provider is responsible for clearing any state on the
    /// registry that should be rebuilt — typically done by the
    /// registry's higher-level refresh logic.
    fn refresh(&mut self, _config: &crate::config::AppConfig) {}
}
