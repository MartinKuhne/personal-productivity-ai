//! Batch-processing subsystem — coordinator, dialog, discoverer, executor, file matcher, prompts, and types.

pub mod coordinator;
pub mod dialog;
pub mod discoverer;
pub mod executor;
pub mod file_matcher;
pub mod prompts;
pub mod types;

pub use file_matcher::*;
pub use prompts::*;
pub use types::*;
