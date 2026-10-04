//! Batch-processing subsystem — coordinator, discoverer, executor, file matcher, prompts, prompt rules, and types.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (BATCH-001..BATCH-014) for the full specification.
//! The batch dialog UI is egui presentation code and lives in [`crate::ui::batch_dialog`].

pub(crate) mod coordinator;
pub(crate) mod discoverer;
pub(crate) mod executor;
pub mod file_matcher;
pub mod prompts;
pub mod types;

pub use coordinator::BatchCoordinator;
pub use executor::{BatchAgentRunParams, BatchJobExecutor, run_agent_blocking};
