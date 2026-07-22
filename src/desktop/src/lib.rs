//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.

pub mod agent;
pub mod background;
pub mod background_task;
pub mod batch;
pub mod browser;
pub mod config;
pub mod document;
pub mod editor;
pub mod error;
pub mod file_events;
pub mod directory_tracker;
pub mod file_processor;
pub mod messages;
pub mod print;
pub mod tag_manager;
pub mod tools;
pub mod ui;
pub mod utils;

pub use error::AgentError;

pub use agent::run_agent;
pub use background_task::Task;
pub use config::{AppConfig, VirtualPath, VirtualPathError, get_config_path, load_config};
pub use messages::BackgroundMessage;
pub use print::{PrintJob, execute_print_blocking};
pub use tag_manager::TagManager;
pub use tools::{execute_tool, get_tools_schema};
pub use ui::FastMdApp;
pub use utils::{extract_tags_from_file, parse_front_matter};
