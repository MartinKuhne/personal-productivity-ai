//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.

pub mod agent;
pub mod app;
pub mod markdown;
pub mod ui;
pub mod utils;

#[path = "agent/tools/mod.rs"]
pub mod tools;
pub mod config;
pub mod background;
pub mod background_task;
pub mod batch;
pub mod document;
pub mod editor_egui;
pub mod error;

pub use error::AgentError;
pub use agent::run_agent;
pub use tools::{execute_tool, get_tools_schema};
pub use app::watcher::{
    Bus, BusReader, DirectoryTracker, FileEvent, FileEventKind, FileEventProcessor,
    FileEventProducer, FileWatcher,
};
pub use app::{
    BackgroundMessage, Cursor, DialogManager, PanelLayout, PersistedUiState, Selection,
    SelectionManager, TabManager, TagManager, TextBuffer, ToCEntry, TokenUsageInfo, UndoStack,
    VirtualPath, VirtualPathError,
};
pub use app::background_task::Task;
pub use config::{AppConfig, get_config_path, load_config};
pub use app::print::{PrintJob, execute_print_blocking};
pub use ui::FastMdApp;
pub use utils::extract_tags_from_file;
