//! Orchestrator unit tests — error and corner-case paths for the
//! app-level orchestrator dispatchers.

use super::*;
use crate::agent::events::{
    AgentDebugEntry, AgentStatus, DebugEntryKind, DebugEntryRow, TokenUsageInfo, ToolSideEffect,
};
use crate::agent::tools::registry::{ToolErrorKind, ToolGroupId};
use crate::background::LogCategory;
use crate::background::logs::BackgroundLogs;
use crate::bus::core::Bus;
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::directory::{DirectoryChanged, DirectoryChangedKind};
use crate::bus::events::file::{FileChanged, FileChangedKind};
use crate::bus::events::typed::{FsEvent, McpAuthEvent, ProcessEvent};
use crate::config::{AppConfig, McpServerConfig, McpServerEntry};
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::Workspace;
use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Build an isolated `AppOrchestrator` with fresh buses and a real
/// (idle) `AgentSession` driver thread.
fn harness() -> AppOrchestrator {
    let file_event_bus: Bus<FileChanged> = Bus::new();
    let dir_event_bus: Bus<DirectoryChanged> = Bus::new();
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
    // The projection's file state is applied through the orchestrator's own
    // reader (`apply_file_changed`), matching the production wiring; the
    // directory reader is the drain for the dir bus.
    let workspace = Workspace::new(None, Some(dir_event_bus.subscribe()));
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
        user_file_operation_worker: crate::background::UserFileOperationWorker::spawn(tx.clone()),
        tx,
        file_event_bus,
        dir_event_bus,
        file_event_reader: Some(file_event_reader),
        workspace,
        pdf_backing_tracker,
        tags: Tags::new(),
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
        pending_editor_open: None,
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

fn finish_user_file_operation(orch: &mut AppOrchestrator) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!timeout.is_zero(), "file operation completion timed out");
        match orch.rx.recv_timeout(timeout) {
            Ok(crate::bus::events::typed::BackgroundEvent::Process(
                event @ crate::bus::events::typed::ProcessEvent::UserFileOperationCompleted(_),
            )) => {
                orch.handle_process_event(event);
                return;
            }
            Ok(crate::bus::events::typed::BackgroundEvent::Process(event)) => {
                orch.handle_process_event(event);
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                panic!("file operation completion timed out")
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("background channel disconnected before file operation completed")
            }
        }
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
        .publish(FileChanged::discovered(vec![md.clone(), png.clone()]));

    let changed = orch.process_file_events();

    assert!(changed, "process_file_events should report change");
    assert!(orch.workspace.contains_file(&md));
    assert!(!orch.workspace.contains_file(&png));
    assert!(orch.selection.tree_dirty);
    // Parent directory of the discovered workspace file is tracked.
    assert!(orch.workspace.contains_dir(&parent));
}

#[test]
fn apply_directory_changed_cascades_delete_to_descendants() {
    // [REQ-320..322] A `[DirectoryChanged][Deleted]` must remove the
    // directory **and all descendants** from the workspace projection,
    // leaving sibling directories untouched.
    let mut orch = harness();
    let root = PathBuf::from("/lib/docs");
    let child = root.join("sub");
    let other = PathBuf::from("/lib/other");
    let descendant_file = child.join("note.md");
    orch.workspace.add_dir(root.clone());
    orch.workspace.add_dir(child.clone());
    orch.workspace.add_dir(other.clone());
    orch.workspace.add_file(descendant_file.clone());
    orch.tabs.open_tab(descendant_file.clone());
    orch.tags
        .add_tags(descendant_file.clone(), vec!["temporary".to_string()]);
    orch.selection.tree_dirty = false;

    let removed = DirectoryChanged {
        path: root.clone(),
        kind: DirectoryChangedKind::Deleted,
    };
    orch.apply_directory_changed(removed.clone());
    // Applying the same event again (a duplicate delivery, e.g. from a
    // second subscriber) must be a no-op, not a double cascade error.
    orch.apply_directory_changed(removed);

    assert!(!orch.workspace.contains_dir(&root));
    assert!(!orch.workspace.contains_dir(&child));
    assert!(orch.workspace.contains_dir(&other));
    assert!(!orch.workspace.contains_file(&descendant_file));
    assert!(!orch.tabs.tabs.contains(&descendant_file));
    assert!(!orch.tags.file_tags().contains_key(&descendant_file));
    assert!(!orch.tags.all_tags().contains("temporary"));
    assert!(orch.selection.tree_dirty);
}

#[test]
fn renaming_directory_publishes_only_directory_events() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let old_path = temp.path().join("old-name");
    let new_path = temp.path().join("new-name");
    std::fs::create_dir(&old_path).unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::ConfirmRename {
            path: old_path.clone(),
            new_name: "new-name".to_string(),
        },
    );

    assert!(std::fs::metadata(&new_path).unwrap().is_dir());
    assert!(file_reader.try_recv().is_err());
    let deleted = dir_reader.try_recv().unwrap();
    let added = dir_reader.try_recv().unwrap();
    assert_eq!(deleted.path, old_path);
    assert_eq!(deleted.kind, DirectoryChangedKind::Deleted);
    assert_eq!(added.path, new_path);
    assert_eq!(added.kind, DirectoryChangedKind::Added);
}

#[test]
fn moving_directory_publishes_only_directory_events() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let old_path = temp.path().join("old-name");
    let destination = temp.path().join("destination");
    let new_path = destination.join("old-name");
    std::fs::create_dir(&old_path).unwrap();
    std::fs::create_dir(&destination).unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(crate::bus::events::user_command::UserCommand::ConfirmMove {
        file: old_path.clone(),
        destination,
    });
    finish_user_file_operation(&mut orch);

    assert!(std::fs::metadata(&new_path).unwrap().is_dir());
    assert!(file_reader.try_recv().is_err());
    let deleted = dir_reader.try_recv().unwrap();
    let added = dir_reader.try_recv().unwrap();
    assert_eq!(deleted.path, old_path);
    assert_eq!(deleted.kind, DirectoryChangedKind::Deleted);
    assert_eq!(added.path, new_path);
    assert_eq!(added.kind, DirectoryChangedKind::Added);
}

#[test]
fn moving_file_publishes_only_file_events() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let old_path = temp.path().join("old-name.md");
    let destination = temp.path().join("destination");
    let new_path = destination.join("old-name.md");
    std::fs::write(&old_path, "content").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(crate::bus::events::user_command::UserCommand::ConfirmMove {
        file: old_path.clone(),
        destination,
    });
    finish_user_file_operation(&mut orch);

    let deleted = file_reader.try_recv().unwrap();
    let added = file_reader.try_recv().unwrap();
    assert_eq!(deleted.kind, FileChangedKind::Deleted);
    assert_eq!(deleted.files[0].path, old_path);
    assert_eq!(added.kind, FileChangedKind::Added);
    assert_eq!(added.files[0].path, new_path);
    assert!(dir_reader.try_recv().is_err());
}

#[test]
fn deleting_directory_publishes_only_directory_event() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("directory");
    std::fs::create_dir(&path).unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();
    orch.apply_user_command(crate::bus::events::user_command::UserCommand::Delete(
        path.clone(),
    ));
    finish_user_file_operation(&mut orch);

    assert!(file_reader.try_recv().is_err());
    let deleted = dir_reader.try_recv().unwrap();
    assert_eq!(deleted.path, path);
    assert_eq!(deleted.kind, DirectoryChangedKind::Deleted);
}

#[test]
fn deleting_file_publishes_only_file_event() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("note.md");
    std::fs::write(&path, "content").unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();
    orch.apply_user_command(crate::bus::events::user_command::UserCommand::Delete(
        path.clone(),
    ));
    finish_user_file_operation(&mut orch);

    let deleted = file_reader.try_recv().unwrap();
    assert_eq!(deleted.kind, FileChangedKind::Deleted);
    assert_eq!(deleted.files[0].path, path);
    assert!(dir_reader.try_recv().is_err());
}

#[test]
fn creating_directory_publishes_only_directory_event() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("created");
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::ConfirmCreateDirectory {
            parent: temp.path().to_path_buf(),
            name: "created".to_string(),
        },
    );
    finish_user_file_operation(&mut orch);

    assert!(std::fs::metadata(&path).unwrap().is_dir());
    let added = dir_reader.try_recv().unwrap();
    assert_eq!(added.path, path);
    assert_eq!(added.kind, DirectoryChangedKind::Added);
    assert!(file_reader.try_recv().is_err());
}

#[test]
fn creating_document_publishes_only_file_event() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("created.md");
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::ConfirmCreateDocument {
            parent: temp.path().to_path_buf(),
            name: "created".to_string(),
        },
    );
    finish_user_file_operation(&mut orch);

    assert!(path.exists());
    let added = file_reader.try_recv().unwrap();
    assert_eq!(added.kind, FileChangedKind::Added);
    assert_eq!(added.files[0].path, path);
    assert!(dir_reader.try_recv().is_err());
}

#[test]
fn renaming_file_publishes_only_file_events() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    let old_path = temp.path().join("old-name.md");
    let new_path = temp.path().join("new-name.md");
    std::fs::write(&old_path, "content").unwrap();
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_user_command(
        crate::bus::events::user_command::UserCommand::ConfirmRename {
            path: old_path.clone(),
            new_name: "new-name".to_string(),
        },
    );

    let deleted = file_reader.try_recv().unwrap();
    let added = file_reader.try_recv().unwrap();
    assert_eq!(deleted.kind, FileChangedKind::Deleted);
    assert_eq!(deleted.files[0].path, old_path);
    assert_eq!(added.kind, FileChangedKind::Added);
    assert_eq!(added.files[0].path, new_path);
    assert!(dir_reader.try_recv().is_err());
}

#[test]
fn process_file_events_removed_closes_tab_and_rebuilds_tags() {
    let mut orch = harness();
    let path = PathBuf::from("/tmp/notes.md");
    orch.tabs.open_tab(path.clone());
    orch.selection.select_file(path.clone());

    orch.file_event_bus
        .publish(FileChanged::removed_one(path.clone()));
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
        .publish(FileChanged::updated_one(path.clone()));
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

    assert!(orch.workspace.indexing_finished);
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
fn tag_results_for_pdf_paths_do_not_add_them_to_the_workspace() {
    let mut orch = harness();
    let path = PathBuf::from("report.pdf");

    orch.handle_fs_event(FsEvent::FileParsed {
        path: path.clone(),
        tags: Vec::new(),
    });
    orch.handle_fs_event(FsEvent::FileModified {
        path: path.clone(),
        tags: vec!["from-header".into()],
    });

    assert!(!orch.workspace.contains_file(&path));
    assert_eq!(
        orch.tags.file_tags().get(&path),
        Some(&vec!["from-header".into()])
    );
}

#[test]
fn tag_resynchronization_replaces_stale_file_and_unique_tag_state() {
    let mut orch = harness();
    let stale_path = PathBuf::from("stale.md");
    let current_path = PathBuf::from("current.md");
    orch.tags
        .add_tags(stale_path.clone(), vec!["stale-only".into()]);

    orch.handle_fs_event(FsEvent::TagsResynchronized {
        files: vec![(current_path.clone(), vec!["current-only".into()])],
    });

    assert!(!orch.tags.file_tags().contains_key(&stale_path));
    assert_eq!(
        orch.tags.file_tags().get(&current_path),
        Some(&vec!["current-only".into()])
    );
    assert!(!orch.tags.all_tags().contains("stale-only"));
    assert!(orch.tags.all_tags().contains("current-only"));
}

#[test]
fn handle_fs_event_finished_marks_indexing() {
    let mut orch = harness();
    orch.handle_fs_event(FsEvent::FinishedWithoutWatcher);
    assert!(orch.workspace.indexing_finished);
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
fn handle_process_event_drops_editor_open_superseded_by_another_path() {
    let mut orch = harness();
    let old_path = PathBuf::from("/tmp/old-editor.md");
    let current_path = PathBuf::from("/tmp/current-editor.md");
    orch.pending_editor_open = Some(current_path.clone());

    orch.handle_process_event(ProcessEvent::UserFileOperationCompleted(
        crate::background::UserFileOperationResult::EditorOpened {
            path: old_path,
            content: Ok(Some("stale contents".to_string())),
        },
    ));

    assert_eq!(orch.pending_editor_open, Some(current_path));
    assert_eq!(orch.tabs.loaded_path, None);
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
fn start_batch_uses_its_command_prompt_snapshot_not_dialog_state_or_disk() {
    let mut orch = harness();
    let temp = tempfile::tempdir().unwrap();
    orch.dialogs.batch_dialog_config.available_prompts =
        vec![crate::agent::batch::types::PromptInfo {
            path: temp.path().join("stale-dialog-prompt.md"),
            display_name: "Stale dialog prompt".to_string(),
            library_name: "test".to_string(),
            content: "stale dialog prompt text".to_string(),
        }];
    orch.dialogs.batch_dialog_config.selected_prompt_idx = Some(0);
    let config = crate::agent::batch::types::BatchConfig {
        directory: temp.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: temp.path().join("missing-prompt.md"),
        mode: crate::agent::batch::types::BatchMode::File,
        concurrency: 1,
    };

    orch.apply_user_command(crate::bus::events::user_command::UserCommand::StartBatch {
        config,
        prompt_text: "command-selected prompt text".to_string(),
    });

    assert!(
        orch.dialogs.batch_handle.is_some(),
        "a self-contained batch command must not need a prompt-file read or dialog back-reference"
    );
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

// ---------------------------------------------------------------------------
// Drain loops — closed / disconnected readers must detach themselves
// ---------------------------------------------------------------------------

/// A closed `Bus<UserCommand>` reader must be released instead of being
/// polled forever ([FMD-064]).
#[test]
fn drain_user_command_bus_releases_a_closed_reader() {
    let mut orch = harness();
    orch.user_command_reader = Some(BusReader::detached());

    orch.drain_user_command_bus();

    assert!(
        orch.user_command_reader.is_none(),
        "a closed reader must be detached so the drain loop stops polling it"
    );
}

/// With no reader attached the drain must be a no-op rather than a panic, and
/// commands published to the bus must not be applied.
#[test]
fn drain_user_command_bus_without_reader_is_a_noop() {
    let mut orch = harness();
    orch.user_command_reader = None;
    orch.agent_panel_state.debug_search_text = "untouched".to_string();
    orch.user_command_bus
        .publish(UserCommand::SetDebugSearchText("ignored".to_string()));

    orch.drain_user_command_bus();

    assert_eq!(orch.agent_panel_state.debug_search_text, "untouched");
}

/// Every queued command must be collected and applied, in publication order,
/// after the reader borrow is released ([FMD-066], [FMD-069]).
#[test]
fn drain_user_command_bus_applies_every_queued_command() {
    let mut orch = harness();
    let reader = orch.user_command_bus.subscribe();
    orch.user_command_reader = Some(reader);
    orch.user_command_bus
        .publish(UserCommand::SetDebugSearchText("first".to_string()));
    orch.user_command_bus
        .publish(UserCommand::SelectTagFilter(Some("tag".to_string())));
    orch.user_command_bus
        .publish(UserCommand::SetDebugSearchText("second".to_string()));

    orch.drain_user_command_bus();

    assert_eq!(
        orch.agent_panel_state.debug_search_text, "second",
        "the last queued command must win, proving every event was drained in order"
    );
    assert_eq!(
        orch.tags.selected_tag,
        Some("tag".to_string()),
        "the interleaved command must also have been applied"
    );
    assert!(
        orch.user_command_reader.is_some(),
        "an open bus must keep its reader attached"
    );
}

/// A disconnected config reader must be released, and the config must stay
/// untouched ([FMD-064]).
#[test]
fn drain_config_bus_releases_a_disconnected_reader() {
    let mut orch = harness();
    orch.config_reader = Some(BusReader::detached());

    orch.drain_config_bus();

    assert!(
        orch.config_reader.is_none(),
        "a disconnected reader must be detached"
    );
    assert!(
        orch.content_libraries.is_empty(),
        "no config may be projected when nothing arrived"
    );
}

/// The inert catch-up reader must not make `process_file_events` report a
/// change ([REQ-330..334]).
#[test]
fn process_file_events_with_an_inert_reader_reports_no_change() {
    let mut orch = harness();
    orch.file_event_reader = Some(BusReader::detached());
    orch.selection.tree_dirty = false;

    let changed = orch.process_file_events();

    assert!(!changed, "a closed catch-up reader yields no events");
    assert!(!orch.selection.tree_dirty);
}

/// A closed agent-event reader must be released so the frame loop stops
/// polling it ([FMD-064]).
#[test]
fn drain_agent_event_bus_releases_a_closed_reader() {
    let mut orch = harness();
    orch.agent_event_reader = Some(BusReader::detached());

    orch.drain_agent_event_bus();

    assert!(
        orch.agent_event_reader.is_none(),
        "a closed reader must be detached so the drain loop stops polling it"
    );
}

#[test]
fn drain_agent_event_bus_without_reader_is_a_noop() {
    let mut orch = harness();
    orch.agent_event_reader = None;
    orch.agent_event_bus.publish(SeamAgentEvent::ContentDelta {
        session_id: Uuid::new_v4(),
        text: "unobserved".to_string(),
    });

    orch.drain_agent_event_bus();

    assert!(
        orch.agent_transcript.content.is_empty(),
        "without a reader nothing may be projected"
    );
}

/// A `SessionStarted` for the session the transcript already renders must keep
/// the accumulated content (only a *new* session resets it).
#[test]
fn drain_agent_event_bus_keeps_transcript_for_the_current_session() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.agent_transcript = AgentTranscript::new(session_id);
    orch.agent_transcript.content.push_str("accumulated");
    orch.agent_event_bus
        .publish(SeamAgentEvent::SessionStarted { session_id });

    orch.drain_agent_event_bus();

    assert_eq!(orch.agent_transcript.session_id, session_id);
    assert!(
        orch.agent_transcript.content.contains("accumulated"),
        "re-delivering the start of the current session must not wipe the transcript"
    );
}

/// `Status` drives the agent status label and doubles as the re-sync point
/// after a lag ([FMD-067]).
#[test]
fn drain_agent_event_bus_status_updates_label_and_resyncs_after_lag() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.agent_event_lagged = true;
    orch.agent_event_bus.publish(SeamAgentEvent::Status {
        session_id,
        status: AgentStatus::AwaitingLlm,
    });

    orch.drain_agent_event_bus();

    assert_eq!(
        orch.agent.state().status,
        AgentStatus::AwaitingLlm.display_string()
    );
    assert!(
        !orch.agent_event_lagged,
        "a Status boundary is a re-sync point after Lagged(n)"
    );

    orch.agent_event_bus.publish(SeamAgentEvent::Status {
        session_id,
        status: AgentStatus::ExecutingTools,
    });
    orch.drain_agent_event_bus();

    assert_eq!(
        orch.agent.state().status,
        AgentStatus::ExecutingTools.display_string()
    );
}

/// A failed session must stop the run and drop queued prompts so one failure
/// cannot cascade ([FMD-062]).
#[test]
fn drain_agent_event_bus_failed_stops_run_and_drops_queue() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.agent.set_running(true);
    orch.agent.queue_prompt("queued prompt".to_string());
    orch.agent_event_bus.publish(SeamAgentEvent::Failed {
        session_id,
        error: "llm exploded".to_string(),
    });

    orch.drain_agent_event_bus();

    assert!(!orch.agent.state().running);
    assert_eq!(orch.agent.state().status, "Error: llm exploded");
    assert_eq!(
        orch.agent.queued_prompt_count(),
        0,
        "queued prompts must not run after a failure"
    );
}

/// Token usage accumulates across turns; `prompt_tokens` is a running maximum
/// because each turn resends the whole conversation.
#[test]
fn drain_agent_event_bus_token_usage_accumulates() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.agent_event_bus.publish(SeamAgentEvent::TokenUsage {
        session_id,
        usage: TokenUsageInfo {
            prompt_tokens: 100,
            completion_tokens: 5,
            total_tokens: 105,
            cached_tokens: Some(2),
            reasoning_tokens: None,
        },
    });

    orch.drain_agent_event_bus();

    assert_eq!(orch.agent.state().total_usage.total_tokens, 105);
    assert_eq!(orch.agent.state().total_usage.prompt_tokens, 100);
    assert_eq!(orch.agent.state().total_usage.cached_tokens, Some(2));
    assert_eq!(
        orch.agent
            .state()
            .token_usage
            .as_ref()
            .map(|u| u.completion_tokens),
        Some(5),
        "the last-turn snapshot must be stored"
    );

    orch.agent_event_bus.publish(SeamAgentEvent::TokenUsage {
        session_id,
        usage: TokenUsageInfo {
            prompt_tokens: 40,
            completion_tokens: 3,
            total_tokens: 43,
            cached_tokens: None,
            reasoning_tokens: Some(1),
        },
    });
    orch.drain_agent_event_bus();

    assert_eq!(
        orch.agent.state().total_usage.prompt_tokens,
        100,
        "prompt_tokens keeps the running maximum, not the last value"
    );
    assert_eq!(orch.agent.state().total_usage.total_tokens, 148);
    assert_eq!(orch.agent.state().total_usage.reasoning_tokens, Some(1));
}

/// Debug entries are accumulated for the debug window; `ToolSideEffect` is
/// currently ignored by the orchestrator.
#[test]
fn drain_agent_event_bus_records_debug_entries() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    let file_reader = orch.file_event_bus.subscribe();
    orch.agent_event_bus.publish(SeamAgentEvent::DebugEntry {
        session_id,
        entry: AgentDebugEntry {
            turn: 1,
            timestamp: chrono::Local::now(),
            kind: DebugEntryKind::Outgoing,
            summary: "outgoing turn".to_string(),
            content: Some(serde_json::json!({ "role": "user" })),
            row_type: DebugEntryRow::Entry,
        },
    });
    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id,
            effect: ToolSideEffect::FileChanged {
                path: PathBuf::from("/tmp/touched.md"),
            },
        });

    orch.drain_agent_event_bus();

    let entries = &orch.agent.state().debug_entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].summary, "outgoing turn");
    assert_eq!(entries[0].kind, DebugEntryKind::Outgoing);
    assert!(
        file_reader.try_recv().is_err(),
        "drain_agent_event_bus currently ignores ToolSideEffect and must not \
         republish it as a file change"
    );
}

/// A queued prompt must be started as soon as the previous session finishes,
/// and a continuation appends the prompt to the existing transcript.
#[test]
fn drain_agent_event_bus_starts_the_queued_prompt_on_session_finished() {
    let mut orch = harness();
    let session_id = Uuid::new_v4();
    orch.start_agent_session("first prompt".to_string());
    assert!(
        orch.agent.current_session_id().is_some(),
        "the first submission mints the session id"
    );
    orch.agent.queue_prompt("second prompt".to_string());
    orch.agent_event_bus
        .publish(SeamAgentEvent::SessionFinished {
            session_id,
            history: vec![],
        });

    orch.drain_agent_event_bus();

    assert_eq!(
        orch.agent.queued_prompt_count(),
        0,
        "the queued prompt must have been taken and submitted"
    );
    assert!(
        orch.agent_transcript.content.contains("second prompt"),
        "a continuation appends the prompt text to the existing transcript"
    );
}

// ---------------------------------------------------------------------------
// handle_fs_event / handle_process_event
// ---------------------------------------------------------------------------

/// `FsEvent::FileDeleted` must drop the file from every projection the
/// orchestrator owns: workspace, tags, tabs, selection and the open document.
#[test]
fn handle_fs_event_file_deleted_clears_tab_selection_and_tags() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/gone.md");
    let other = PathBuf::from("/lib/kept.md");
    orch.workspace.add_file(path.clone());
    orch.tags.add_tags(
        path.clone(),
        vec!["doomed".to_string(), "shared".to_string()],
    );
    orch.tags
        .add_tags(other.clone(), vec!["shared".to_string()]);
    orch.tabs.open_tab(path.clone());
    orch.tabs.open_tab(other.clone());
    orch.tabs.loaded_path = Some(path.clone());
    orch.selection.select_file(path.clone());
    orch.selection.selected_files.insert(path.clone());
    orch.selection.tree_dirty = false;

    orch.handle_fs_event(FsEvent::FileDeleted { path: path.clone() });

    assert!(!orch.workspace.contains_file(&path));
    assert!(!orch.tags.file_tags().contains_key(&path));
    assert!(!orch.tags.all_tags().contains("doomed"));
    assert!(
        orch.tags.all_tags().contains("shared"),
        "a tag still used by another file must survive"
    );
    assert!(!orch.tabs.tabs.contains(&path));
    assert!(orch.tabs.tabs.contains(&other));
    assert_eq!(
        orch.selection.selected_file(),
        Some(&other),
        "selection must retarget to a remaining tab"
    );
    assert!(
        !orch.selection.selected_files.contains(&path),
        "the multi-selection must not keep a removed path"
    );
    assert!(
        orch.tabs.loaded_path.is_none(),
        "the deleted document must be unloaded"
    );
    assert!(orch.selection.tree_dirty);
}

/// Deleting the only open tab must clear the rendered document entirely.
#[test]
fn handle_fs_event_file_deleted_clears_content_when_no_tabs_remain() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/gone.md");
    orch.tabs.open_tab(path.clone());
    orch.selection.select_file(path.clone());
    orch.tabs.current_markdown = "# Body".to_string();

    orch.handle_fs_event(FsEvent::FileDeleted { path: path.clone() });

    assert!(orch.tabs.tabs.is_empty());
    assert!(orch.selection.selected_file().is_none());
    assert!(orch.tabs.current_markdown.is_empty());
}

/// `FsEvent::Finished` marks indexing complete even when no watcher was handed
/// back through the finished-watcher slot.
#[test]
fn handle_fs_event_finished_without_a_watcher_still_marks_indexing() {
    let mut orch = harness();
    orch.workspace.indexing_finished = false;
    orch.tags
        .add_tags(PathBuf::from("/lib/a.md"), vec!["t".to_string()]);
    orch.selection.tree_dirty = false;

    orch.handle_fs_event(FsEvent::Finished);

    assert!(orch.workspace.indexing_finished);
    assert!(orch.selection.tree_dirty);
    assert!(
        orch.tags.all_tags().contains("t"),
        "the tag index is rebuilt, not discarded"
    );
    assert!(
        orch.task_take_finished_watcher().is_none(),
        "an empty finished-watcher slot yields nothing"
    );
}

/// A `LogEntry` from a background worker lands in the shared background log
/// ([NFR-001], [FMD-071]).
#[test]
fn handle_process_event_log_entry_lands_in_background_logs() {
    let mut orch = harness();

    orch.handle_process_event(ProcessEvent::LogEntry(BackgroundLogEntry::new(
        LogCategory::LlmTools,
        "worker says hello".to_string(),
    )));

    let logs = orch.background_manager.lock().unwrap();
    assert!(
        logs.get_logs()
            .iter()
            .any(|entry| entry.message == "worker says hello"),
        "the entry must be pushed to the shared log buffer"
    );
}

/// An inline editor-open result must open the tab and project the contents.
#[test]
fn handle_process_event_editor_opened_inline_populates_the_document() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/opened.md");
    orch.pending_editor_open = Some(path.clone());

    orch.handle_user_file_operation_result(UserFileOperationResult::EditorOpened {
        path: path.clone(),
        content: Ok(Some("# Opened\n\nBody".to_string())),
    });

    assert!(orch.tabs.tabs.contains(&path), "a tab must be opened");
    assert_eq!(orch.tabs.loaded_path, Some(path.clone()));
    assert!(
        orch.text_buffer.content.contains("# Opened"),
        "the editor buffer must receive the file contents"
    );
    assert!(
        orch.pending_editor_open.is_none(),
        "the pending marker must be released once the result lands"
    );
    assert!(orch.text_buffer.is_open);
}

/// A system-editor launch reports `Ok(None)`: the pending marker is released
/// but nothing is read into the document.
#[test]
fn handle_process_event_editor_opened_without_content_projects_nothing() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/external.md");
    orch.pending_editor_open = Some(path.clone());

    orch.handle_user_file_operation_result(UserFileOperationResult::EditorOpened {
        path: path.clone(),
        content: Ok(None),
    });

    assert!(orch.pending_editor_open.is_none());
    assert!(
        orch.tabs.tabs.is_empty(),
        "an external editor launch must not open an inline tab"
    );
    assert!(orch.tabs.loaded_path.is_none());
    assert!(!orch.text_buffer.is_open);
}

/// Re-opening a path that is already an open tab must not duplicate the tab.
#[test]
fn handle_process_event_editor_opened_reuses_an_existing_tab() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/already-open.md");
    orch.tabs.open_tab(path.clone());
    orch.pending_editor_open = Some(path.clone());

    orch.handle_user_file_operation_result(UserFileOperationResult::EditorOpened {
        path: path.clone(),
        content: Ok(Some("body".to_string())),
    });

    assert_eq!(
        orch.tabs.tabs.iter().filter(|t| *t == &path).count(),
        1,
        "the tab must not be opened twice"
    );
}

// ---------------------------------------------------------------------------
// handle_user_file_operation_result
// ---------------------------------------------------------------------------

/// Register an MCP server entry so the registry lists its group.
fn with_mcp_server(mut orch: AppOrchestrator, name: &str) -> AppOrchestrator {
    orch.config.mcp_servers.insert(
        name.to_string(),
        McpServerEntry::from(McpServerConfig::Stdio {
            command: "test-server".to_string(),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
        }),
    );
    orch
}

/// One failure arm of `handle_user_file_operation_result`.
struct OperationFailureCase {
    /// Case name, embedded in every assertion message.
    name: &'static str,
    /// `pending_editor_open` to seed; the editor-open arm ignores results for a
    /// superseded path.
    pending_editor_open: Option<&'static str>,
    /// Completion reported by the background worker.
    result: UserFileOperationResult,
    /// Substring the background log entry must contain.
    expected: &'static str,
}

/// Every failure arm must surface as a background-log entry carrying the
/// worker's message and error text ([NFR-001], [NFR-004]).
#[test]
fn handle_user_file_operation_result_failures_are_logged() {
    let path = PathBuf::from("/lib/note.md");
    let destination = PathBuf::from("/lib/other/note.md");
    let cases = vec![
        OperationFailureCase {
            name: "prompt read",
            pending_editor_open: None,
            result: UserFileOperationResult::PromptLoaded {
                content: Err("prompt file vanished".to_string()),
                target_dir: None,
                target_file: None,
            },
            expected: "Failed to read prompt file: prompt file vanished",
        },
        OperationFailureCase {
            name: "editor open",
            pending_editor_open: Some("/lib/note.md"),
            result: UserFileOperationResult::EditorOpened {
                path: path.clone(),
                content: Err("no editor configured".to_string()),
            },
            expected: "Failed to open /lib/note.md in editor: no editor configured",
        },
        OperationFailureCase {
            name: "editor save",
            pending_editor_open: None,
            result: UserFileOperationResult::EditorSaved {
                path: path.clone(),
                saved_content: "body".to_string(),
                result: Err("read-only volume".to_string()),
            },
            expected: "Failed to save /lib/note.md: read-only volume",
        },
        OperationFailureCase {
            name: "move",
            pending_editor_open: None,
            result: UserFileOperationResult::Moved {
                source: path.clone(),
                destination: destination.clone(),
                is_directory: false,
                result: Err("access denied".to_string()),
            },
            expected: "Failed to move /lib/note.md to /lib/other/note.md: access denied",
        },
        OperationFailureCase {
            name: "delete",
            pending_editor_open: None,
            result: UserFileOperationResult::Deleted {
                path: path.clone(),
                is_directory: false,
                result: Err("locked by another process".to_string()),
            },
            expected: "Failed to delete /lib/note.md to trash: locked by another process",
        },
        OperationFailureCase {
            name: "directory create",
            pending_editor_open: None,
            result: UserFileOperationResult::DirectoryCreated {
                path: PathBuf::from("/lib/exists"),
                result: Err("already exists".to_string()),
            },
            expected: "Failed to create directory /lib/exists: already exists",
        },
        OperationFailureCase {
            name: "document create",
            pending_editor_open: None,
            result: UserFileOperationResult::DocumentCreated {
                path: Err("no space left".to_string()),
            },
            expected: "Failed to create Markdown document: no space left",
        },
        OperationFailureCase {
            name: "pdf export",
            pending_editor_open: None,
            result: UserFileOperationResult::PdfSaved {
                markdown_path: path.clone(),
                result: Err("typst not found".to_string()),
            },
            expected: "Failed to export /lib/note.md as PDF: typst not found",
        },
        OperationFailureCase {
            name: "reveal",
            pending_editor_open: None,
            result: UserFileOperationResult::Revealed {
                path: path.clone(),
                result: Err("no shell available".to_string()),
            },
            expected: "Failed to reveal /lib/note.md in Explorer: no shell available",
        },
    ];

    for case in cases {
        let mut orch = harness();
        orch.pending_editor_open = case.pending_editor_open.map(PathBuf::from);

        orch.handle_user_file_operation_result(case.result);

        let logs = orch.background_manager.lock().unwrap();
        assert!(
            logs.get_logs()
                .iter()
                .any(|entry| entry.message == case.expected),
            "case '{}': expected a background log entry {:?}, got {:?}",
            case.name,
            case.expected,
            logs.get_logs()
                .iter()
                .map(|entry| entry.message.clone())
                .collect::<Vec<_>>()
        );
    }
}

/// A successful editor save republishes the path as an updated file so the
/// watcher-driven projections catch up ([REQ-330..334]).
#[test]
fn handle_user_file_operation_result_editor_saved_publishes_file_updated() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/saved.md");
    let file_reader = orch.file_event_bus.subscribe();

    orch.handle_user_file_operation_result(UserFileOperationResult::EditorSaved {
        path: path.clone(),
        saved_content: "body".to_string(),
        result: Ok(()),
    });

    let event = file_reader
        .try_recv()
        .expect("an editor save must republish the file");
    assert_eq!(event.kind, FileChangedKind::Changed);
    assert_eq!(event.files[0].path, path);
}

/// A successful delete publishes a directory event for a directory and a file
/// event for a file — never both ([REQ-320..322]).
#[test]
fn handle_user_file_operation_result_deleted_publishes_one_event_per_kind() {
    struct Case {
        name: &'static str,
        is_directory: bool,
    }
    for case in [
        Case {
            name: "directory",
            is_directory: true,
        },
        Case {
            name: "file",
            is_directory: false,
        },
    ] {
        let mut orch = harness();
        let path = PathBuf::from("/lib/target");
        let file_reader = orch.file_event_bus.subscribe();
        let dir_reader = orch.dir_event_bus.subscribe();

        orch.handle_user_file_operation_result(UserFileOperationResult::Deleted {
            path: path.clone(),
            is_directory: case.is_directory,
            result: Ok(()),
        });

        if case.is_directory {
            let event = dir_reader
                .try_recv()
                .unwrap_or_else(|_| panic!("case '{}': expected a directory event", case.name));
            assert_eq!(event.path, path);
            assert_eq!(event.kind, DirectoryChangedKind::Deleted);
            assert!(
                file_reader.try_recv().is_err(),
                "case '{}': a directory delete must not publish a file event",
                case.name
            );
        } else {
            let event = file_reader
                .try_recv()
                .unwrap_or_else(|_| panic!("case '{}': expected a file event", case.name));
            assert_eq!(event.kind, FileChangedKind::Deleted);
            assert_eq!(event.files[0].path, path);
            assert!(
                dir_reader.try_recv().is_err(),
                "case '{}': a file delete must not publish a directory event",
                case.name
            );
        }
    }
}

/// A created directory joins the workspace projection and is announced on the
/// directory bus.
#[test]
fn handle_user_file_operation_result_directory_created_updates_workspace() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/created");
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.handle_user_file_operation_result(UserFileOperationResult::DirectoryCreated {
        path: path.clone(),
        result: Ok(()),
    });

    assert!(
        orch.workspace.contains_dir(&path),
        "the new directory must join the workspace projection"
    );
    let event = dir_reader
        .try_recv()
        .expect("the new directory must be announced");
    assert_eq!(event.path, path);
    assert_eq!(event.kind, DirectoryChangedKind::Added);
    assert!(
        file_reader.try_recv().is_err(),
        "a directory must not be announced on the file bus"
    );
}

/// A created document is announced on the file bus.
#[test]
fn handle_user_file_operation_result_document_created_publishes_file_discovered() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/created.md");
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.handle_user_file_operation_result(UserFileOperationResult::DocumentCreated {
        path: Ok(path.clone()),
    });

    let event = file_reader
        .try_recv()
        .expect("the new document must be announced");
    assert_eq!(event.kind, FileChangedKind::Added);
    assert_eq!(event.files[0].path, path);
    assert!(dir_reader.try_recv().is_err());
}

/// The success arms for PDF export and Explorer reveal carry no further UI
/// side effects.
#[test]
fn handle_user_file_operation_result_export_and_reveal_successes_are_noops() {
    for (name, result) in [
        (
            "pdf export",
            UserFileOperationResult::PdfSaved {
                markdown_path: PathBuf::from("/lib/note.md"),
                result: Ok(PathBuf::from("/lib/note.pdf")),
            },
        ),
        (
            "reveal",
            UserFileOperationResult::Revealed {
                path: PathBuf::from("/lib/note.md"),
                result: Ok(()),
            },
        ),
    ] {
        let mut orch = harness();
        let file_reader = orch.file_event_bus.subscribe();
        let dir_reader = orch.dir_event_bus.subscribe();

        orch.handle_user_file_operation_result(result);

        let logs = orch.background_manager.lock().unwrap();
        assert!(
            logs.get_logs().is_empty(),
            "case '{}': a success must not log an error",
            name
        );
        assert!(
            file_reader.try_recv().is_err(),
            "case '{}': a success must not publish a file event",
            name
        );
        assert!(
            dir_reader.try_recv().is_err(),
            "case '{}': a success must not publish a directory event",
            name
        );
    }
}

// ---------------------------------------------------------------------------
// apply_path_move
// ---------------------------------------------------------------------------

/// A file move must carry the tag index, tab list, selection and expanded
/// state over to the destination path.
#[test]
fn apply_path_move_file_carries_tags_selection_and_expanded_state() {
    let mut orch = harness();
    let source = PathBuf::from("/lib/old.md");
    let destination = PathBuf::from("/lib/new.md");
    let unrelated = PathBuf::from("/lib/other.md");
    orch.workspace.add_file(source.clone());
    orch.tags.add_tags(
        source.clone(),
        vec!["carried".to_string(), "shared".to_string()],
    );
    orch.tags
        .add_tags(unrelated.clone(), vec!["shared".to_string()]);
    orch.tabs.open_tab(source.clone());
    orch.tabs.loaded_path = Some(source.clone());
    orch.selection.select_file(source.clone());
    orch.selection.selected_dir = Some(source.clone());
    orch.selection.expanded_dirs.insert(source.clone());
    orch.selection.tree_dirty = false;

    orch.apply_path_move(&source, &destination, false);

    assert!(!orch.workspace.contains_file(&source));
    assert!(
        orch.workspace.contains_file(&destination),
        "a Markdown destination must be re-added to the workspace"
    );
    assert_eq!(
        orch.tags.file_tags().get(&destination),
        Some(&vec!["carried".to_string(), "shared".to_string()]),
        "the tag entry must move with the file"
    );
    assert!(!orch.tags.file_tags().contains_key(&source));
    assert!(
        orch.tags.all_tags().contains("shared"),
        "a tag still used by another file must survive the move"
    );
    assert!(!orch.tabs.tabs.contains(&source));
    assert!(orch.tabs.tabs.contains(&destination));
    assert_eq!(orch.tabs.loaded_path, Some(destination.clone()));
    assert_eq!(orch.selection.selected_file, Some(destination.clone()));
    assert_eq!(orch.selection.selected_dir, Some(destination.clone()));
    assert!(!orch.selection.expanded_dirs.contains(&source));
    assert!(orch.selection.expanded_dirs.contains(&destination));
    assert!(orch.selection.tree_dirty);
}

/// Moving a non-Markdown file must not add it to the workspace file index.
#[test]
fn apply_path_move_file_with_an_untracked_extension_is_not_added() {
    let mut orch = harness();
    let source = PathBuf::from("/lib/old.txt");
    let destination = PathBuf::from("/lib/new.png");
    orch.workspace.add_file(source.clone());

    orch.apply_path_move(&source, &destination, false);

    assert!(!orch.workspace.contains_file(&source));
    assert!(
        !orch.workspace.contains_file(&destination),
        "an image destination is not a workspace file"
    );
}

/// A move of a path with no tag entry must not invent one at the destination.
#[test]
fn apply_path_move_without_a_tag_entry_creates_none() {
    let mut orch = harness();
    let source = PathBuf::from("/lib/untracked.md");
    let destination = PathBuf::from("/lib/moved.md");

    orch.apply_path_move(&source, &destination, false);

    assert!(
        !orch.tags.file_tags().contains_key(&destination),
        "a file with no tags must not gain an empty entry"
    );
}

/// A directory move republishes on the directory bus only and swaps the
/// workspace directory entry.
#[test]
fn apply_path_move_directory_swaps_workspace_dirs() {
    let mut orch = harness();
    let source = PathBuf::from("/lib/old-dir");
    let destination = PathBuf::from("/lib/new-dir");
    orch.workspace.add_dir(source.clone());
    let file_reader = orch.file_event_bus.subscribe();
    let dir_reader = orch.dir_event_bus.subscribe();

    orch.apply_path_move(&source, &destination, true);

    assert!(!orch.workspace.contains_dir(&source));
    assert!(orch.workspace.contains_dir(&destination));
    assert!(
        file_reader.try_recv().is_err(),
        "a directory move must not publish file events"
    );
    let deleted = dir_reader
        .try_recv()
        .expect("the old directory must be announced as removed");
    let added = dir_reader
        .try_recv()
        .expect("the new directory must be announced as added");
    assert_eq!(deleted.path, source);
    assert_eq!(deleted.kind, DirectoryChangedKind::Deleted);
    assert_eq!(added.path, destination);
    assert_eq!(added.kind, DirectoryChangedKind::Added);
}

// ---------------------------------------------------------------------------
// handle_mcp_auth_event
// ---------------------------------------------------------------------------

/// A failed OAuth flow must clear the in-progress flag *and* record an
/// authentication error on the group row so the dialog can surface it.
#[test]
fn handle_mcp_auth_event_failure_records_authentication_error_on_group() {
    let mut orch = with_mcp_server(harness(), "srv");
    orch.dialogs.set_oauth_in_progress("srv");

    orch.handle_mcp_auth_event(McpAuthEvent::Completed {
        server_name: "srv".to_string(),
        error: Some("token exchange rejected".to_string()),
    });

    assert!(
        !orch.dialogs.is_oauth_in_progress("srv"),
        "the flow has ended, so the in-progress flag must be cleared"
    );
    let bundle = orch.tool_context.load();
    let agent_cfg = orch.config.to_agent_config();
    let group = bundle
        .registry
        .group(&ToolGroupId::Mcp("srv".to_string()), &agent_cfg)
        .expect("a configured MCP server must have a group row");
    let error = group
        .last_error
        .as_ref()
        .expect("a failed OAuth flow must record an error on the group");
    assert_eq!(error.kind, ToolErrorKind::Authentication);
    assert_eq!(error.message, "token exchange rejected");
}

// ---------------------------------------------------------------------------
// close_tabs_for_removed_files
// ---------------------------------------------------------------------------

/// Removing paths that no tab references must leave tabs, selection and the
/// rendered document untouched.
#[test]
fn close_tabs_for_removed_files_without_a_match_leaves_state_untouched() {
    let mut orch = harness();
    let kept = PathBuf::from("/lib/kept.md");
    let gone = PathBuf::from("/lib/never-opened.md");
    orch.tabs.open_tab(kept.clone());
    orch.selection.select_file(kept.clone());
    orch.tabs.current_markdown = "# Body".to_string();

    orch.close_tabs_for_removed_files(std::slice::from_ref(&gone));

    assert_eq!(orch.tabs.tabs, vec![kept.clone()]);
    assert_eq!(orch.selection.selected_file(), Some(&kept));
    assert_eq!(orch.tabs.current_markdown, "# Body");
}

/// With no selected file but open tabs, closing removed files must adopt the
/// last remaining tab.
#[test]
fn close_tabs_for_removed_files_selects_last_tab_when_nothing_was_selected() {
    let mut orch = harness();
    let a = PathBuf::from("/lib/a.md");
    let b = PathBuf::from("/lib/b.md");
    orch.tabs.open_tab(a.clone());
    orch.tabs.open_tab(b.clone());
    orch.selection.selected_file = None;

    orch.close_tabs_for_removed_files(&[PathBuf::from("/lib/absent.md"), a.clone()]);

    assert_eq!(orch.tabs.tabs, vec![b.clone()]);
    assert_eq!(
        orch.selection.selected_file(),
        Some(&b),
        "the last remaining tab becomes the selection"
    );
}

// ---------------------------------------------------------------------------
// Tool side effects re-issued as `FsEvent::FileModified` (FR-007)
// ---------------------------------------------------------------------------

/// Read one `FsEvent` off a dedicated background-channel subscription, naming
/// the failing case instead of hanging the suite.
fn next_fs_event(
    receiver: &mut tokio::sync::broadcast::Receiver<BackgroundEvent>,
    case: &str,
) -> FsEvent {
    match receiver.try_recv() {
        Ok(BackgroundEvent::Fs(ev)) => ev,
        Ok(other) => {
            panic!("{case}: expected an Fs event on the background channel, got {other:?}")
        }
        Err(err) => panic!("{case}: nothing was published on the background channel ({err:?})"),
    }
}

/// Assert an `FsEvent` is a `FileModified` and hand back its fields.
fn expect_file_modified(ev: FsEvent, case: &str) -> (PathBuf, Vec<String>) {
    match ev {
        FsEvent::FileModified { path, tags } => (path, tags),
        other => panic!("{case}: expected FsEvent::FileModified, got {other:?}"),
    }
}

/// **Regression test for `ToolSideEffect` being silently dropped (defect D4).**
///
/// The seam variant's own doc promises "The UI reissues this as
/// `FsEvent::FileModified` (FR-007)", but the drain arm was an empty `{}`, so
/// the promise was never kept. The side effect now goes back out on the
/// background channel — the transport `FsEvent` actually travels on — where the
/// UI's update loop feeds it to [`AppOrchestrator::handle_fs_event`].
#[test]
fn defect_04_tool_side_effect_is_reissued_as_fs_file_modified() {
    let mut orch = harness();
    // Subscribe before the drain so the published event is observable.
    let mut background = orch.tx.subscribe();
    let path = PathBuf::from("/lib/tool-created.md");
    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileCreated {
                path: path.clone(),
                tags: vec!["prompt".to_string()],
            },
        });

    orch.drain_agent_event_bus();

    let (published_path, published_tags) = expect_file_modified(
        next_fs_event(&mut background, "a created file must be re-published"),
        "the re-issued event must be a file-modified event",
    );
    assert_eq!(
        published_path, path,
        "the created path must be carried through unchanged"
    );
    assert_eq!(
        published_tags,
        vec!["prompt".to_string()],
        "the tags the tool read from the new file must be carried through"
    );
}

/// The re-issued event must be *consumed*, not merely published: feeding it to
/// `handle_fs_event` the way the UI loop does has to update the tag index, the
/// prompt set and the tree-dirty flag.
#[test]
fn defect_04_reissued_side_effect_updates_the_tag_index_when_handled() {
    let mut orch = harness();
    let mut background = orch.tx.subscribe();
    let path = PathBuf::from("/lib/created.md");
    orch.selection.tree_dirty = false;
    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileCreated {
                path: path.clone(),
                tags: vec!["note".to_string(), "prompt".to_string()],
            },
        });

    orch.drain_agent_event_bus();

    let (published_path, published_tags) = expect_file_modified(
        next_fs_event(&mut background, "the side effect must reach the channel"),
        "the re-issued event must be a file-modified event",
    );
    orch.handle_fs_event(FsEvent::FileModified {
        path: published_path,
        tags: published_tags,
    });

    assert_eq!(
        orch.tags
            .file_tags()
            .get(&path)
            .cloned()
            .unwrap_or_default(),
        vec!["note".to_string(), "prompt".to_string()],
        "the tag index must learn the new file's tags"
    );
    assert!(
        orch.tags.prompt_paths().contains(&path),
        "a prompt-tagged creation must enter the prompt set"
    );
    assert!(
        orch.selection.tree_dirty,
        "a file appearing must dirty the tree for a rebuild"
    );
}

/// A `FileChanged` carries no tags, and `Tags::add_tags` *replaces* the entry a
/// path already has — so the index's known tags are carried forward instead of
/// being wiped by an empty list.
#[test]
fn defect_04_a_changed_file_keeps_its_known_tags_instead_of_being_wiped() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/known.md");
    orch.tags
        .add_tags(path.clone(), vec!["keep-me".to_string()]);
    let mut background = orch.tx.subscribe();

    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileChanged { path: path.clone() },
        });

    orch.drain_agent_event_bus();

    let (published_path, published_tags) = expect_file_modified(
        next_fs_event(&mut background, "a changed file must be re-published"),
        "the re-issued event must be a file-modified event",
    );
    assert_eq!(published_path, path, "the changed path must be carried");
    assert_eq!(
        published_tags,
        vec!["keep-me".to_string()],
        "known tags must be carried forward rather than replaced by an empty list"
    );
}

/// A changed file the index has never seen is still a real change: the event is
/// published with an empty tag list rather than being suppressed.
#[test]
fn defect_04_a_changed_file_with_no_known_tags_publishes_an_empty_tag_list() {
    let mut orch = harness();
    let path = PathBuf::from("/lib/unknown.md");
    let mut background = orch.tx.subscribe();

    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileChanged { path: path.clone() },
        });

    orch.drain_agent_event_bus();

    let (published_path, published_tags) = expect_file_modified(
        next_fs_event(
            &mut background,
            "an unindexed change must still be re-published",
        ),
        "the re-issued event must be a file-modified event",
    );
    assert_eq!(published_path, path, "the path must be carried");
    assert!(
        published_tags.is_empty(),
        "nothing is known about this path yet, got: {published_tags:?}"
    );
}

/// Several side effects drained in one pass must be re-published in the order
/// the agent produced them ([FMD-066]).
#[test]
fn defect_04_side_effects_keep_their_order_within_one_drain() {
    let mut orch = harness();
    let mut background = orch.tx.subscribe();
    let first = PathBuf::from("/lib/first.md");
    let second = PathBuf::from("/lib/second.md");
    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileCreated {
                path: first.clone(),
                tags: Vec::new(),
            },
        });
    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileChanged {
                path: second.clone(),
            },
        });

    orch.drain_agent_event_bus();

    let (first_path, _) = expect_file_modified(
        next_fs_event(&mut background, "the first side effect must come first"),
        "first event",
    );
    let (second_path, _) = expect_file_modified(
        next_fs_event(&mut background, "the second side effect must come second"),
        "second event",
    );
    assert_eq!(first_path, first, "publication order must be preserved");
    assert_eq!(second_path, second, "publication order must be preserved");
}

/// A background channel with no subscriber must not take the agent drain down:
/// the failure is logged with an error code and recorded for the UI log panel
/// ([NFR-001], [NFR-003], [NFR-004]).
#[test]
fn defect_04_a_failed_publish_is_logged_instead_of_panicking() {
    let mut orch = harness();
    // A sender whose only receiver has been dropped: every `send` fails.
    let (dead_tx, dead_rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(4);
    drop(dead_rx);
    orch.tx = BackgroundEventSender::new(dead_tx);

    orch.agent_event_bus
        .publish(SeamAgentEvent::ToolSideEffect {
            session_id: Uuid::new_v4(),
            effect: ToolSideEffect::FileChanged {
                path: PathBuf::from("/lib/unreachable.md"),
            },
        });

    orch.drain_agent_event_bus();

    let logs = orch.background_manager.lock().unwrap();
    assert!(
        logs.get_logs()
            .iter()
            .any(|entry| entry.message.contains("tool file side effect")),
        "the publish failure must be recorded in the background log, got: {:?}",
        logs.get_logs()
    );
}
