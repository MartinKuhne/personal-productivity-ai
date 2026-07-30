//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.

pub mod agent;
pub mod app;
pub mod background;
pub mod background_task;
pub mod batch;
pub mod browser;
pub mod config;
pub mod document;
pub mod editor_egui;
pub mod error;
pub mod markdown;
pub mod print;
pub mod tools;
pub mod ui;
pub mod utils;

pub use error::AgentError;

pub use agent::run_agent;
pub use app::watcher::{
    Bus, BusReader, DirectoryTracker, FileEvent, FileEventKind, FileEventProcessor,
    FileEventProducer, FileWatcher,
};
pub use app::{
    BackgroundMessage, Cursor, DialogManager, PanelLayout, PersistedUiState, Selection,
    SelectionManager, TabManager, TagManager, TextBuffer, ToCEntry, TokenUsageInfo, UndoStack,
};
pub use background_task::Task;
pub use config::{AppConfig, VirtualPath, VirtualPathError, get_config_path, load_config};
pub use print::{PrintJob, execute_print_blocking};
pub use tools::{execute_tool, get_tools_schema};
pub use ui::FastMdApp;
pub use utils::extract_tags_from_file;
