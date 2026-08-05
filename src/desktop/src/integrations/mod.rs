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
