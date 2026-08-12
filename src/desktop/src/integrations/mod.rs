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
/// manager) and the [`McpClients`](crate::integrations::mcp::McpClients)
/// that owns them. The LLM-tool-loop glue that exposes
/// MCP-discovered tools to the agent tool registry lives in
/// [`crate::agent::tools::mcp`].
pub mod mcp;

/// Weather integration — Nominatim (geocoding) + the US National
/// Weather Service (api.weather.gov) forecast. The LLM-tool-loop
/// adapter that exposes `get_weather` as a `Tool` impl lives in
/// crate::agent::tools::registry::builtin::weather.
pub mod weather;

/// Trello integration — REST client over `https://api.trello.com/1`.
/// The LLM-tool-loop adapters (`trello_get_boards`, `trello_create_card`,
/// …) live in
/// crate::agent::tools::registry::builtin::trello.
pub mod trello;

/// DAV integration — CalDAV (RFC 4791) + CardDAV (RFC 6352) over HTTP,
/// backed by the `fast_dav_rs` SDK. The LLM-tool-loop adapters
/// (`search_calendar`, `add_contact`, …) live in
/// crate::agent::tools::registry::builtin::caldav and
/// crate::agent::tools::registry::builtin::carddav.
pub mod dav;
