//! Shared runtime infrastructure — the long-lived headless browser
//! session and the PDF-backing tracker that both the application
//! orchestrator and the LLM agent depend on.
//!
//! Both types live in one module so the dependency direction stays
//! straight: the orchestrator constructs them and hands them to the
//! agent through `AgentContext` / `ToolContext`, and never reaches
//! back. The agent sees a single `crate::app::session` dependency
//! rather than two unrelated submodules, which keeps the crate's
//! layering intact — top-level `app/orchestrator` owns the agent, and
//! the agent does not know about concrete `app/` submodules.
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

pub(crate) mod browser_session;
pub mod bus_observer;
pub(crate) mod config_subscriber;
pub(crate) mod pdf_backing_tracker;

pub use browser_session::{BrowserSession, PageHandle, SessionError};
pub use config_subscriber::spawn_config_subscription;
pub use pdf_backing_tracker::PdfBackingTracker;

#[cfg(all(test, feature = "browser"))]
#[path = "browser_tests.rs"]
mod browser_tests;
