//! Shared runtime infrastructure — the long-lived headless browser
//! session and the PDF-backing tracker that both the application
//! orchestrator and the LLM agent depend on.
//!
//! Before this module existed, [`BrowserSession`] lived in
//! `app::browser` (the Playwright wrapper) and [`PdfBackingTracker`]
//! lived in `app::watcher` (the file-watcher plumbing). Both were
//! consumed by the agent layer, which produced a logical
//! `agent → app` cycle that ran the wrong way for the dependency
//! graph the rest of the crate follows (top-level
//! `app/orchestrator` owns the agent; the agent should not
//! know about concrete `app/` submodules).
//!
//! Collapsing the two shared types into a single
//! `app::session` module makes the dependency intent explicit:
//! the orchestrator constructs them, hands them to the agent
//! through `AgentContext` / `ToolContext`, and never reaches back.
//! The agent sees a single `crate::app::session` dependency
//! instead of two unrelated submodules.
//!
//! ## Browser session and the `browser` Cargo feature
//!
//! The browser session and its types are always compiled so the
//! `agent` and `ui` modules can hold an `Arc<BrowserSession>` without
//! sprinkling `#[cfg(feature = "browser")]` across every consumer.
//! The Playwright-backed internals (the [`page`](BrowserSession::page)
//! method, the [`PageHandle`] fields) are gated: when the
//! `browser` feature is off, the session is a zero-sized stub and
//! every operation returns [`SessionError::Disabled`]. The
//! `playwright-rs` dependency is only pulled in when the
//! `browser` feature is enabled.
//!
//! See the layering-inversion review in
//! `doc/planning/desktop-module-boundaries-review.md` for the
//! rationale.

pub mod browser_session;
pub mod bus_observer;
pub mod config_subscriber;
pub mod pdf_backing_tracker;

pub use browser_session::{BrowserSession, PageHandle, SessionError};
pub use config_subscriber::spawn_config_subscription;
pub use pdf_backing_tracker::PdfBackingTracker;

#[cfg(all(test, feature = "browser"))]
#[path = "browser_tests.rs"]
mod browser_tests;
