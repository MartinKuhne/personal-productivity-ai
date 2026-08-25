//! LLM-callable tools — the `Tool` trait, the `ToolRegistry` (catalog +
//! per-group state + error tracking), and implementations for
//! filesystem, web, email, and CSV.
//!
//! Protocol-layer code for the external service integrations
//! (CalDAV, CardDAV, MCP, Trello, Weather) lives under
//! [`crate::lib`] and is referenced from the `Tool`
//! adapters in [`registry::builtin`].
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (TOOL-001..TOOL-010, TOOL-014..024) for the full specification.

pub mod blocking;
pub mod browser;
pub mod cache;
pub mod context;
pub mod csv_db;
pub mod cursor;
pub mod descriptor;
#[cfg(test)]
mod descriptor_tests;
pub mod dispatcher;
#[cfg(test)]
mod dispatcher_tests;
pub mod dtos;
pub mod extensions;
pub mod filesystem;
pub mod jmap;
pub mod mcp;
pub mod observer;
pub mod policy;
pub mod provider;
#[cfg(test)]
mod provider_tests;
pub mod registry;
pub mod specs;
#[cfg(feature = "vector-search")]
pub mod vector_search;
pub mod vfs;
pub mod web;
pub mod yaml_header;

// Property-based tests for the LLM↔tool DTO trust boundary.
// Phase 1 of the fuzzing plan (`doc/planning/fuzzing.md`).
#[cfg(test)]
mod dtos_proptests;

// Property-based tests for the `ToolRegistry::execute_tool`
// dispatch — every (name, args) pair the LLM emits. Phase 1 of
// the fuzzing plan (`doc/planning/fuzzing.md`).
#[cfg(test)]
mod tool_call_dispatch_proptests;

use context::ToolContext;
use descriptor::ToolDescriptor;
use std::any::TypeId;

/// Whether a tool mutates user state and therefore cannot be dispatched
/// in parallel with other mutating tools.
///
/// `Tool::safety` lets [`crate::tool_executor::ToolExecutor`]
/// classify tools without matching on string names. The default
/// classification is [`Safety::Mutating`] (the conservative choice for
/// any tool that has not yet been audited).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    /// Read-only / idempotent. Multiple safe tool calls may run in
    /// parallel within a single agent turn.
    ReadOnly,
    /// Mutates user-visible state (filesystem, calendar, contacts, email,
    /// ...). Calls run sequentially in the order the LLM emitted them.
    Mutating,
}

/// LLM-callable tool. Implementations return a `'static`
/// [`ToolDescriptor`] from [`Tool::descriptor`]; the metadata
/// methods default to reading from the descriptor. The default
/// `is_enabled` consults the descriptor's
/// [`descriptor::ToolConfigSpec`] and the current prompt; tools
/// with non-trivial prompt rules (e.g. the CSV family's TOOL-001
/// gate) override the default.
pub trait Tool: Send + Sync {
    /// Static metadata for this tool. Returned by reference so the
    /// descriptor can be cached in a `OnceLock` and shared with
    /// the agent loop, the prompt builder, and the UI dialog
    /// without an allocation per call.
    fn descriptor(&self) -> &ToolDescriptor;

    /// Tool name as it appears to the LLM. Defaults to
    /// `self.descriptor().name`.
    fn name(&self) -> &str {
        &self.descriptor().name
    }

    /// Tool description shown to the LLM. Defaults to
    /// `self.descriptor().description`.
    fn description(&self) -> &str {
        &self.descriptor().description
    }

    /// Compile-time identifier of the input DTO. Defaults to
    /// `self.descriptor().input_type`.
    fn input_type(&self) -> TypeId {
        self.descriptor().input_type
    }

    /// JSON Schema for the tool's input. Defaults to a clone of
    /// `self.descriptor().parameters_schema`.
    fn parameters_schema(&self) -> serde_json::Value {
        self.descriptor().parameters_schema.clone()
    }

    /// Whether the tool should currently be offered to the LLM.
    /// Defaults to evaluating `self.descriptor().config` against
    /// the live `AgentConfig` and the current prompt.
    fn is_enabled(&self, config: &crate::config::AgentConfig, prompt: &str) -> bool {
        self.descriptor().config.is_enabled_for(config, prompt)
    }

    /// Classification used by the agent loop to decide parallel vs.
    /// sequential dispatch. Defaults to
    /// `self.descriptor().safety`.
    fn safety(&self) -> Safety {
        self.descriptor().safety
    }

    /// Run the tool with the given JSON arguments. Errors are
    /// returned as [`ToolError`]; success as a serialised JSON
    /// value.
    fn execute(&self, ctx: &ToolContext, input_json: &str) -> Result<serde_json::Value, ToolError>;
}

pub use cache::{
    CACHE_TTL, CURSOR_EXPIRED_ERROR, CachedWebDocument, FINAL_PAGE_HINT, MAX_CACHE_ENTRIES,
    SearchEmailItem, ToolCache, cache,
};
pub use context::ToolContext as ToolContextType;
pub use cursor::{CursorPage, CursorSessionManager, PagedDataset};
pub use descriptor::ToolDescriptor as ToolDescriptorType;
pub use dispatcher::{ToolDispatcher, ToolError, ToolOutcome, ToolServices};
pub use provider::{RegisteredTool, ToolProvider};
pub use registry::execute_tool;
