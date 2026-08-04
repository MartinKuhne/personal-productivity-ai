//! Headless Firefox session for the LLM agent.
//!
//! See `doc/planning/browser_tools.md` for the design record and
//! [`crate::config::BrowserConfig`] for the user-facing knobs.
//!
//! Owns a long-lived [`playwright_rs::Browser`] +
//! [`playwright_rs::BrowserContext`] + [`playwright_rs::Page`]
//! triple, lazily launching the browser on first use and
//! reloading cookies from a `storage_state` JSON file on every
//! launch so the LLM stays logged in across app restarts
//! (BRWS-001..005).
//!
//! The module is egui-free: [`BrowserSession`] is owned by the
//! application-domain layer and exposed to the UI / tools as an
//! `Arc<BrowserSession>` so the same page is shared across
//! mutating tool calls inside a single agent turn. Tools that
//! do not need the browser simply ignore the field.

pub mod session;

pub use session::{BrowserSession, PageHandle, SessionError};
