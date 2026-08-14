//! Batch-processing subsystem — coordinator, discoverer, executor, file matcher, prompts, prompt rules, and types.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (BATCH-001..BATCH-014) for the full specification.
//! The batch dialog UI is egui presentation code and lives in [`crate::ui::batch_dialog`].

pub mod coordinator;
pub mod discoverer;
pub mod executor;
pub mod file_matcher;
pub mod prompt_rules;
pub mod prompts;
pub mod types;

pub use coordinator::BatchCoordinator;
pub use discoverer::Discoverer;
pub use executor::{BatchJobExecutor, run_agent_blocking};
pub use prompts::{discover_prompts, read_prompt_content, resolve_prompts};
pub use types::{
    BatchConfig, BatchHandle, BatchJob, BatchJobStatus, BatchMode, BatchResult, PromptInfo,
};

#[cfg(test)]
mod prompt_rules_tests;
