//! Orchestrator unit tests — error and corner-case paths for the
//! app-level orchestrator dispatchers.

use super::*;
use crate::background::LogCategory;
use crate::background::logs::BackgroundLogs;
use crate::bus::core::Bus;
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{FsEvent, McpAuthEvent, ProcessEvent};
use crate::config::AppConfig;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::{DirectoryTracker, FileEventProcessor};
use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Build an isolated `AppOrchestrator` with fresh buses and a real
/// (idle) `AgentSession` driver thread.
fn harness() -> AppOrchestrator {
    let file_event_bus: Bus<FileEvent> = Bus::new();
    let agent_event_bus: Bus<SeamAgentEvent> = Bus::new();
    // Use broadcast channel for tx (matching production BackgroundEventSender)
    let (bg_tx, bg_rx_broadcast) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(bg_tx);
    // Use std mpsc for rx (orchestrator expects this type)
    // Bridge: forward from broadcast receiver to std mpsc sender
    let (bg_tx_std, bg_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime for bridge");
        rt.block_on(async move {
            let mut rx = bg_rx_broadcast;
            while let Ok(event) = rx.recv().await {
                if bg_tx_std.send(event).is_err() {
                    break;
                }
            }
        });
    });

    let file_event_reader = file_event_bus.subscribe();
    let file_processor = FileEventProcessor::new(file_event_bus.subscribe());
    let directory_tracker = DirectoryTracker::new(file_event_bus.subscribe());
    let pdf_backing_tracker = crate::agent::session::PdfBackingTracker::new();

    let tool_context = Arc::new(ArcSwap::from_pointee(crate::agent::AgentToolContext::new(
        crate::agent::tools::registry::ToolRegistry::new(),
    )));

    let observer_factory: crate::agent::events::AgentObserverFactory =
        Arc::new(|_session_id| std::sync::Arc::new(crate::agent::events::RecordingObserver::new()));

    let agent = crate::agent::AgentSession::builder()
        .with_file_observer(std::sync::Arc::new(
            crate::agent::session::bus_observer::AppFileObserver::new(file_event_bus.clone()),
        ))
        .with_observer_factory(observer_factory)
        .with_tool_context(tool_context.clone())
        .build();

    let agent_event_reader = agent_event_bus.subscribe();

    AppOrchestrator {
        content_libraries: Vec::new(),
        rx: bg_rx,
        tx,
        file_event_bus,
        file_event_reader: Some(file_event_reader),
        file_processor,
        pdf_backing_tracker,
        tags: Tags::new(),
        directory_tracker,
        selection: FileSelection::new(),
        tabs: Tabs::new(),
        _watcher: None,
        agent,
        dialogs: Dialogs::new(),
        text_buffer: TextBuffer::new(),
        inline_editor_enabled: false,
        background_manager: Arc::new(Mutex::new(BackgroundLogs::new())),
        config: AppConfig::default(),
        config_storage: std::sync::Arc::new(crate::config::NoopConfigStorage),
        config_reader: None,
        pending_file_load: None,
        finished_watcher_slot: Arc::new(Mutex::new(None)),
        tool_context,
        agent_event_bus,
        agent_event_reader: Some(agent_event_reader),
        agent_event_lagged: false,
        agent_transcript: AgentTranscript::new(Uuid::nil()),
        agent_panel_state: AgentPanelState::new(),
        user_command_bus: crate::bus::core::Bus::new(),
        user_command_reader: None,
    }
}

#[test]
fn is_workspace_file_matches_known_extensions_case_insensitively() {
    assert!(AppOrchestrator::is_workspace_file(Path::new("a.md")));
    assert!(AppOrchestrator::is_workspace_file(Path::new("a.MD")));
    assert!(AppOrchestrator::is_workspace_file(Path::new("a.MarkDown")));
    assert!(AppOrchestrator::is_workspace_file(Path::new("a.txt")));
    assert!(!AppOrchestrator::is_workspace_file(Path::new("a.exe")));
    assert!(!AppOrchestrator::is_workspace_file(Path::new(
        "noextension"
    )));
    assert!(!AppOrchestrator::is_workspace_file(Path::new("a.pdf")));
}

#[test]
fn process_file_events_discovered_filters_workspace_files() {
    let mut orch = harness();
    let md = PathBuf::from("/lib/notes.md");
    let png = PathBuf::from("/lib/img.png");
    let parent = PathBuf::from("/lib");
    orch.file_event_bus
        .publish(FileEvent::discovered(vec![md.clone(), png.clone()]));

    let changed = orch.process_file_events();

    assert!(changed, "process_file_events should report change");
    assert!(orch.file_processor.contains_file(&md));
    assert!(!orch.file_processor.contains_file(&png));
    assert!(orch.selection.tree_dirty);
    // Parent directory of the discovered workspace file is tracked.
    assert!(orch.directory_tracker.contains(&parent));
}

#[test]
fn process_file_events_removed_closes_tab_and_rebuilds_tags() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.tabs.open_tab(path.clone());
    orch.selection.select_file(path.clone());

    orch.file_event_bus
        .publish(FileEvent::removed_one(path.clone()));
    let changed = orch.process_file_events();

    assert!(changed);
    assert!(!orch.tabs.tabs.contains(&path));
    assert!(orch.selection.selected_file().is_none());
    assert!(orch.tabs.current_markdown.is_empty());
}

#[test]
fn process_file_events_updated_unloads_loaded_path_when_editor_closed() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.tabs.loaded_path = Some(path.clone());
    // text_buffer.is_open defaults to false.

    orch.file_event_bus
        .publish(FileEvent::updated_one(path.clone()));
    orch.process_file_events();

    assert!(orch.tabs.loaded_path.is_none());
}

#[test]
fn process_file_events_empty_is_noop() {
    let mut orch = harness();
    let changed = orch.process_file_events();
    assert!(!changed);
    assert!(orch.tabs.tabs.is_empty());
}

#[test]
fn close_tabs_for_removed_files_retargets_selection_to_last() {
    let mut orch = harness();
    let a = PathBuf::from("/tmp/a.md");
    let b = PathBuf::from("/tmp/b.md");
    orch.tabs.open_tab(a.clone());
    orch.tabs.open_tab(b.clone());
    orch.selection.select_file(a.clone());

    orch.close_tabs_for_removed_files(std::slice::from_ref(&a));

    assert!(!orch.tabs.tabs.contains(&a));
    assert_eq!(orch.selection.selected_file(), Some(&b));
}

#[test]
fn close_tabs_for_removed_files_clears_content_when_none_left() {
    let mut orch = harness();
    let a = PathBuf::from("/tmp/a.md");
    orch.tabs.open_tab(a.clone());
    orch.tabs.current_markdown = "# Body".to_string();
    orch.tabs.current_yaml = Some(serde_norway::Value::String("T".into()));

    orch.close_tabs_for_removed_files(std::slice::from_ref(&a));

    assert!(orch.tabs.tabs.is_empty());
    assert!(orch.selection.selected_file().is_none());
    assert!(orch.tabs.current_markdown.is_empty());
    assert!(orch.tabs.current_yaml.is_none());
}

#[test]
fn drain_config_bus_updates_agent_and_ui_state() {
    let mut orch = harness();
    let config_bus: Bus<ConfigArrived> = Bus::new();
    orch.config_reader = Some(config_bus.subscribe());

    let cfg = AppConfig {
        inline_editor_enabled: true,
        ..AppConfig::default()
    };
    config_bus.publish(ConfigArrived::new(cfg.clone()));

    orch.drain_config_bus();

    assert!(orch.inline_editor_enabled);
    assert_eq!(orch.content_libraries.len(), cfg.content_libraries.len());
    assert!(orch.selection.tree_dirty);
}

#[test]
fn drain_config_bus_empty_leaves_reader_attached() {
    let mut orch = harness();
    let config_bus: Bus<ConfigArrived> = Bus::new();
    orch.config_reader = Some(config_bus.subscribe());

    orch.drain_config_bus();

    assert!(orch.config_reader.is_some());
}

#[test]
fn drain_background_channel_routes_all_variants() {
    let mut orch = harness();
    orch.tx
        .send(FsEvent::FinishedWithoutWatcher.into())
        .unwrap();
    orch.tx
        .send(BackgroundLogEntry::new(LogCategory::Watcher, "hello".into()).into())
        .unwrap();
    orch.tx
        .send(
            McpAuthEvent::Completed {
                server_name: "srv".into(),
                error: None,
            }
            .into(),
        )
        .unwrap();

    // Give the bridge thread time to forward events from broadcast to std mpsc
    std::thread::sleep(std::time::Duration::from_millis(50));

    orch.drain_background_channel();

    assert!(orch.file_processor.indexing_finished);
    {
        let logs = orch.background_manager.lock().unwrap();
        assert!(logs.get_logs().iter().any(|l| l.message == "hello"));
    }
}

#[test]
fn handle_fs_event_file_modified_clears_stale_loaded_path() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.tabs.loaded_path = Some(path.clone());

    orch.handle_fs_event(FsEvent::FileModified {
        path: path.clone(),
        tags: vec!["tag".into()],
    });

    assert!(orch.tabs.loaded_path.is_none());
    assert!(orch.selection.tree_dirty);
}

#[test]
fn handle_fs_event_finished_marks_indexing() {
    let mut orch = harness();
    orch.handle_fs_event(FsEvent::FinishedWithoutWatcher);
    assert!(orch.file_processor.indexing_finished);
}

#[test]
fn handle_process_event_file_loaded_ok_populates_tabs() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    // The real flow only ever spawns a read for a file that IS selected
    // (`handle_file_selection`), so the projection precondition is a selection.
    orch.selection.select_file(path.clone());

    orch.handle_process_event(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Ok("# Title\n\nBody".to_string()),
    });

    assert_eq!(orch.tabs.loaded_path, Some(path));
    assert!(orch.tabs.current_markdown.contains("Body"));
}

/// Regression: a read that was still in flight when the user moved on must not
/// overwrite the document. Landing it must not repoint `loaded_path` or rebuild
/// `toc` from the abandoned file, which would flip the ToC pane between two
/// files' heading sets with no user action.
#[test]
fn handle_process_event_drops_file_loaded_for_a_deslected_file() {
    let mut orch = harness();
    let abandoned = PathBuf::from("/tmp/abandoned.md");
    let current = PathBuf::from("/tmp/current.md");

    // The user moved from `abandoned` to `current`; the read for `current` is in
    // flight and `pending_file_load` names it.
    orch.tabs.open_tab(current.clone());
    orch.selection.select_file(current.clone());
    orch.pending_file_load = Some(current.clone());

    // The abandoned read lands first.
    orch.handle_process_event(ProcessEvent::FileLoaded {
        path: abandoned,
        content: Ok("# Abandoned\n\nShould never be shown".to_string()),
    });

    assert_eq!(
        orch.tabs.loaded_path, None,
        "a stale landing must not repoint the open document"
    );
    assert!(
        orch.tabs.current_markdown.is_empty(),
        "a stale landing must not overwrite the rendered body"
    );
    assert!(
        orch.tabs.toc.is_empty(),
        "a stale landing must not populate the ToC"
    );
    assert_eq!(
        orch.pending_file_load,
        Some(current),
        "the load still pending for the current selection must survive"
    );
}

/// Regression: release the pending marker for the landing path even when it is
/// not currently selected. Leaving it set would block re-selecting the same
/// file behind a load that has already returned.
#[test]
fn handle_process_event_releases_pending_marker_for_its_own_path() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.pending_file_load = Some(path.clone());
    // The harness starts with nothing selected, which is the precondition here.

    orch.handle_process_event(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Ok("# Title\n\nBody".to_string()),
    });

    assert_eq!(
        orch.pending_file_load, None,
        "the marker for the returned path must be released so a later re-select can load"
    );
    assert_eq!(
        orch.tabs.loaded_path, None,
        "nothing is selected, so nothing may be projected"
    );
}

#[test]
fn handle_process_event_file_loaded_err_closes_and_logs() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.tabs.open_tab(path.clone());
    orch.selection.select_file(path.clone());

    orch.handle_process_event(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Err("boom".to_string()),
    });

    assert!(!orch.tabs.tabs.contains(&path));
    assert!(orch.selection.selected_file().is_none());
    {
        let orch_mgr = orch.background_manager.lock().unwrap();
        assert!(
            orch_mgr
                .get_logs()
                .iter()
                .any(|l| l.message.contains("Failed to load file"))
        );
    }
}

#[test]
fn handle_mcp_auth_event_completed_success_clears_oauth_idle() {
    let mut orch = harness();
    orch.dialogs.set_oauth_in_progress("srv");
    orch.handle_mcp_auth_event(McpAuthEvent::Completed {
        server_name: "srv".into(),
        error: None,
    });
    // Success path clears the in-progress flag.
    assert!(!orch.dialogs.is_oauth_in_progress("srv"));
}

#[test]
fn drain_agent_event_bus_session_finished_and_content_delta() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.agent_event_bus
        .publish(SeamAgentEvent::SessionStarted { session_id });
    orch.agent_event_bus.publish(SeamAgentEvent::ContentDelta {
        session_id,
        text: "Hello".into(),
    });
    orch.agent_event_bus
        .publish(SeamAgentEvent::SessionFinished {
            session_id,
            history: vec![],
        });

    orch.drain_agent_event_bus();

    assert_eq!(orch.agent_transcript.session_id, session_id);
    assert!(orch.agent_transcript.content.contains("Hello"));
    assert!(!orch.agent.state().running);
    assert!(orch.agent.state().history.is_some());
}

#[test]
fn drain_agent_event_bus_lagged_appends_truncation_marker() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    // Publish enough to overflow the broadcast buffer so the reader lags.
    for i in 0..20_000 {
        orch.agent_event_bus.publish(SeamAgentEvent::Thinking {
            session_id,
            text: format!("t{i}"),
        });
    }

    orch.drain_agent_event_bus();

    assert!(orch.agent_event_lagged);
    assert!(
        orch.agent_transcript
            .content
            .contains(LAG_TRUNCATION_MARKER)
    );
}

#[test]
fn handle_file_selection_spawns_background_load() {
    let mut orch = harness();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.md");
    std::fs::write(&path, "# Loaded\n").unwrap();
    orch.selection.select_file(path.clone());

    orch.handle_file_selection();
    // Allow the background thread to read and send the FileLoaded event.
    for _ in 0..100 {
        orch.drain_background_channel();
        if orch.tabs.loaded_path == Some(path.clone()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(orch.tabs.loaded_path, Some(path.clone()));
    assert!(orch.tabs.current_markdown.contains("Loaded"));
}

#[test]
fn orchestrator_user_commands_persist_via_injected_config_storage() {
    let mut orch = harness();
    let storage = std::sync::Arc::new(crate::config::InMemoryConfigStorage::new());
    orch.config_storage = storage.clone();

    // Table width strategy change
    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::ChangeTableWidthStrategy(
            crate::markdown::table_width::DeficitStrategy::WaterFillRatio,
        ),
    );
    assert_eq!(
        orch.config.deficit_strategy(),
        crate::markdown::table_width::DeficitStrategy::WaterFillRatio
    );
    assert_eq!(
        storage.latest_config().unwrap().deficit_strategy(),
        crate::markdown::table_width::DeficitStrategy::WaterFillRatio
    );

    // Tool group enable/disable
    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::SetToolGroupEnabled {
            id: crate::agent::tools::registry::ToolGroupId::Internal(
                crate::agent::tools::registry::InternalToolGroup::Weather,
            ),
            enabled: false,
        },
    );
    assert!(!orch.config.tool_groups.weather);
    assert!(!storage.latest_config().unwrap().tool_groups.weather);
}

#[test]
fn test_select_file_command_loads_markdown_content() {
    let mut orch = harness();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.md");
    std::fs::write(&path, "# Loaded Markdown\n").unwrap();

    // Select the file via UserCommand::SelectFile
    orch.apply_user_command(crate::bus::events::user_command::UserCommand::SelectFile {
        path: path.clone(),
        multi: false,
    });

    // Regression check: selecting a file must not prematurely set loaded_path
    assert_eq!(
        orch.tabs.loaded_path, None,
        "loaded_path must remain None until file content is actually read into memory"
    );

    // Drive handle_file_selection() the way the application drives it each
    // frame
    orch.handle_file_selection();

    // Drain background channel to receive ProcessEvent::FileLoaded
    for _ in 0..100 {
        orch.drain_background_channel();
        if orch.tabs.loaded_path == Some(path.clone()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(orch.tabs.loaded_path, Some(path.clone()));
    assert!(orch.tabs.current_markdown.contains("Loaded Markdown"));
}
