//! Desktop application library for FastMd — a markdown knowledge-base manager with agent, tooling, and UI.

pub mod agent;
pub mod app;
pub mod bus;
pub mod markdown;
pub mod ui;
pub mod utils;

#[path = "app/background/mod.rs"]
pub mod background;

#[path = "config/config.rs"]
pub mod config;

#[path = "agent/tools/mod.rs"]
pub mod tools;

pub use agent::error;
pub use app::background_task;
pub use app::batch;
pub use app::document;
pub use ui::editor_egui;

pub use agent::run_agent;
pub use app::background_task::Task;
pub use app::print::{PrintJob, execute_print_blocking};
pub use app::watcher::{DirectoryTracker, FileEventProcessor, FileWatcher};
pub use app::{
    Cursor, DialogManager, PanelLayout, PersistedUiState, Selection, SelectionManager, TabManager,
    TagManager, TextBuffer, UndoStack, VirtualPath, VirtualPathError,
};
pub use bus::config::{CONFIG_ARRIVAL_TIMEOUT, config_bus};
pub use bus::core::{Bus, BusReader};
pub use bus::events::{
    AgentEvent, BackgroundEvent, BackgroundMessage, ConfigArrived, FileEvent, FileEventKind,
    FileEventProducer, FsEvent, ProcessEvent, TokenUsageInfo,
};
pub use bus::router::{BusRouter, ChannelWorker, spawn_path_worker};
pub use config::{AppConfig, get_config_path, load_config};
pub use error::AgentError;
pub use markdown::ToCEntry;
pub use tools::{execute_tool, get_tools_schema};
pub use ui::FastMdApp;
pub use utils::extract_tags_from_file;
