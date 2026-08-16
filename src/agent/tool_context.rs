//! Bundle the agent loop, executor, prompt builder, and UI dialog
//! share via `Arc<ArcSwap<AgentToolContext>>`.
//!
//! `AgentToolContext` is the catalog-level bundle that swaps
//! atomically on `ConfigArrived` events and MCP discovery. Today
//! it carries just the [`ToolRegistry`]; the bundle exists so
//! future services (a pre-projected [`AgentConfig`](crate::config::AgentConfig),
//! a typed credential handle)
//! can be added without re-plumbing every consumer.
//!
//! Consumers that just need the catalog should reach for
//! [`AgentToolContext::registry`]. Callers that need to mutate the catalog
//! (e.g. recording per-group errors after a tool call) clone
//! the bundle via `ArcSwap::rcu`, mutate, and let `rcu` swap it
//! back in.
//!
//! Unit tests live in the sibling `tool_context_tests.rs` sidecar.

use crate::tools::registry::ToolRegistry;

#[derive(Clone, Debug)]
pub struct AgentToolContext {
    /// The LLM-facing tool catalog. Built once at startup; cloned
    /// on every `ArcSwap` swap.
    pub registry: ToolRegistry,
}

impl AgentToolContext {
    /// Build a fresh bundle from a registry.
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }
}
