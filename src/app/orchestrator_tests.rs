//! Orchestrator unit tests — error and corner-case paths for the
//! app-level orchestrator dispatchers that previously had zero direct
//! coverage (close-test-gaps P2).

use super::*;
use crate::background::{BackgroundLogs, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{BackgroundEvent, FsEvent, McpAuthEvent, ProcessEvent};
use crate::config::AppConfig;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, Tabs, TextBuffer};
use crate::workspace::watcher::{DirectoryTracker, FileEventProcessor};
use crate::workspace::Tags;
use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Build an isolated `AppOrchestrator` with fresh buses and a real
/// (idle) `AgentSession` driver thread.
fn harness() -> AppOrchestrator {
    let file_event_bus: Bus<FileEvent> = Bus::new();
    let agent_event_bus: Bus<SeamAgentEvent> = Bus::new();
    let (bg_tx_raw, bg_rx): (_, Receiver<BackgroundEvent>) = channel();
    let tx = BackgroundEventSender::new(bg_tx_raw);

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
        submit_prompt: None,
        text_buffer: TextBuffer::new(),
        inline_editor_enabled: false,
        background_manager: Arc::new(Mutex::new(BackgroundLogs::new())),
        config: AppConfig::default(),
        config_reader: None,
        pending_file_load: None,
        finished_watcher_slot: Arc::new(Mutex::new(None)),
        tool_context,
        agent_event_bus,
        agent_event_reader: Some(agent_event_reader),
        agent_event_lagged: false,
        agent_transcript: AgentTranscript::new(Uuid::nil()),
        agent_panel_state: AgentPanelState::new(),
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

    orch.handle_process_event(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Ok("# Title\n\nBody".to_string()),
    });

    assert_eq!(orch.tabs.loaded_path, Some(path));
    assert!(orch.tabs.current_markdown.contains("Body"));
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
        assert!(orch_mgr
            .get_logs()
            .iter()
            .any(|l| l.message.contains("Failed to load file")));
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
    assert!(orch
        .agent_transcript
        .content
        .contains(LAG_TRUNCATION_MARKER));
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
