//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.

pub mod agent;
pub mod background;
pub mod bus;
pub mod command_executor;
pub mod export;
pub mod integrations;
pub mod markdown;
pub(crate) mod orchestrator;
pub mod ui;
pub mod utils;
pub mod workspace;

#[path = "config/config.rs"]
pub mod config;

pub use orchestrator::AppOrchestrator;
