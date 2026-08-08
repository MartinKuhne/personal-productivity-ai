//! External service integrations.
//!
//! All sub-integrations live behind a Cargo feature. The crate
//! compiles with `default-features` (no integration enabled) and the
//! user opts in per integration:
//!
//! - `discord` — adds the Discord bot (see
//!   `integrations::discord::run_discord_bot`).
//! - `browser` — adds the Playwright-backed browser session and
//!   browser_* agent tools (lives under `crate::app::session` and
//!   `crate::app::browser`, not here).

#[cfg(feature = "discord")]
pub mod discord;

/// MCP (Model Context Protocol) client integration.
///
/// The wire-protocol client (transports, sessions, OAuth 2.1,
/// manager) and the [`McpClientManager`](crate::integrations::mcp::McpClientManager)
/// that owns them. The LLM-tool-loop glue that exposes
/// MCP-discovered tools to the agent tool registry lives in
/// [`crate::agent::tools::mcp`].
pub mod mcp;
