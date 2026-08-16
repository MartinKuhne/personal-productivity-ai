//! AI agent desktop application subsystem — prompt building, batch processing, and shared agent session runtime.

pub use fastmd_agent::*;

pub mod batch;
pub mod prompts;
pub mod session;

pub use batch::{BatchCoordinator, BatchHandle, BatchJob, BatchJobStatus, BatchMode, BatchResult};
pub use prompts::build_system_prompts;
pub use session::{
    BrowserSession, PageHandle, PdfBackingTracker, SessionError, spawn_config_subscription,
};
