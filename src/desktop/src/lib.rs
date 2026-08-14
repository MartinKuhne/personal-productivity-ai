//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.
//!
//! The crate exposes its subsystems as `pub mod` so internal code and
//! external binaries can reach every type through its full module path
//! (`fastmd::agent::lib::mcp`, `fastmd::ui::render`, etc.). The
//! `pub use` re-exports at the bottom of this file are the only items
//! that are also reachable through a shorter root-level path; every
//! other name must be reached through its subsystem module.
//!
//! The re-exports are the externally consumed API surface — the only
//! names used by the bundled `main.rs` and integration `tests/` via
//! the short `fastmd::Name` path. Every other type is one module hop
//! away, which is the documented convention for new code (see
//! `src/desktop/AGENTS.md §5`).
//!
//! Adding a new subsystem re-export: prefer the full module path. Only
//! add a root-level `pub use` when an external consumer actually uses
//! the short path and the type is conceptually a top-level entry point
//! (e.g. the `App` impl, the configuration bus's first event).

pub use fastmd_agent as agent;
pub mod app;
pub mod bus;
pub mod integrations;
pub mod markdown;
#[cfg(feature = "pdf-export")]
#[path = "lib/pdf/mod.rs"]
pub mod pdf;
pub mod ui;
pub mod utils;

#[path = "config/config.rs"]
pub mod config;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------
//
// The list below is the entire root-level short-path surface. Every entry
// has at least one external consumer (a binary in `src/bin/`, the main
// `src/main.rs`, or a file in `tests/`) that uses the short
// `fastmd::Name` form. Types that are only used inside the crate are
// reachable through their `pub mod` path only — see the module-level docs
// above.

/// Re-export the `mcp` subsystem module under the crate root so
/// `fastmd::mcp::oauth::...` works for short-path consumers
/// (e.g. `tests/mcp_oauth.rs` historically reached it that way). The
/// canonical path is `fastmd::agent::lib::mcp`; the short alias
/// stays for backward compatibility.
pub use agent::lib::mcp;

/// The `ConfigArrived` event is the first event the main binary
/// publishes onto the configuration bus after constructing the
/// `FastMdApp`, so it is the only bus payload type exposed at the
/// crate root.
pub use bus::events::ConfigArrived;

/// The `eframe::App` impl that `main.rs` hands to `eframe::run_native`.
pub use ui::FastMdApp;
