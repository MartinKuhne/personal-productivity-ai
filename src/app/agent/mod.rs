//! AI agent desktop application subsystem — prompt building, batch processing, and shared agent session runtime.

pub use fastmd_agent::*;

pub mod batch;
pub mod prompts;
pub(crate) mod session;

pub use session::{
    BrowserSession, PageHandle, PdfBackingTracker, SessionError, spawn_config_subscription,
};
