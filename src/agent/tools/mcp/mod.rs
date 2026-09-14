//! MCP tool family — the LLM-tool-loop adapter that turns tools
//! advertised by an MCP server into [`Tool`](super::Tool) implementations.
//!
//! The actual MCP wire-protocol client (transports, sessions, OAuth,
//! manager) lives in
//! [`crate::lib::mcp`]. This module only owns the
//! [`McpToolAdapter`] glue that lets the LLM tool loop discover and
//! call MCP-provided tools, plus the in-process auth actions the
//! tools dialog needs to surface.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (MCP-001..MCP-021) for the
//! full MCP requirements set covering both the protocol layer
//! (in `crate::lib::mcp`) and the LLM-tool-loop glue
//! (this module).

mod adapter;

pub use adapter::McpToolAdapter;

#[cfg(test)]
mod adapter_tests;
