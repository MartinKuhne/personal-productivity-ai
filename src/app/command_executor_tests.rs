//! Unit tests for centralized user-command execution.
//!
//! Every test drives [`AppOrchestrator::apply_user_command`] directly, so the
//! command → state mapping is exercised without a UI ([FMD-068]).

use crate::config::{AppConfig, ContentLibrary};

#[test]
fn prompt_discovery_returns_tagged_prompt_candidates_from_content_libraries() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("summarize.md"),
        "---\ntitle: Summarize\ntags: [prompt]\n---\nSummarize the selected files.",
    )
    .unwrap();
    let config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: temp.path().to_string_lossy().into_owned(),
            name: "Test Library".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..AppConfig::default()
    };

    let prompts = crate::agent::batch::prompts::discover_prompts(&config);

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].display_name, "Test Library / summarize.md");
    assert_eq!(prompts[0].content, "Summarize the selected files.");
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

use crate::agent::tools::registry::{
    InternalToolGroup, ToolErrorKind, ToolGroupError, ToolGroupId,
};
use crate::background::logs::BackgroundLogs;
use crate::bus::core::Bus;
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::directory::DirectoryChanged;
use crate::bus::events::file::FileChanged;
use crate::bus::events::typed::{
    BackgroundEvent, BackgroundEventSender, McpAuthEvent, ProcessEvent,
};
use crate::bus::events::user_command::UserCommand;
use crate::config::{ConfigStorageHandler, InMemoryConfigStorage, McpServerConfig, McpServerEntry};
use crate::markdown::table_width::DeficitStrategy;
use crate::orchestrator::AppOrchestrator;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::Workspace;
use arc_swap::ArcSwap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Build an isolated `AppOrchestrator` with fresh buses and a real (idle)
/// `AgentSession` driver thread.
///
/// Mirrors the harness in `orchestrator_tests`; that copy is private to the
/// `orchestrator::tests` module and cannot be shared across sidecars.
fn harness() -> AppOrchestrator {
    let file_event_bus: Bus<FileChanged> = Bus::new();
    let dir_event_bus: Bus<DirectoryChanged> = Bus::new();
    let agent_event_bus: Bus<SeamAgentEvent> = Bus::new();
    let (bg_tx, bg_rx_broadcast) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(bg_tx);
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
        user_command_bus: Bus::new(),
        user_command_reader: None,
    }
}

/// A [`ConfigStorageHandler`] whose writes always fail, so the
/// "failed to persist AppConfig" branches are reachable without touching disk.
struct FailingConfigStorage;

impl ConfigStorageHandler for FailingConfigStorage {
    fn save_config(&self, _config: &AppConfig) -> Result<PathBuf, String> {
        Err("simulated storage failure".to_string())
    }
}

/// Block until the user-file-operation worker reports completion and hand the
/// event to the orchestrator, so queued operations are fully applied.
fn finish_user_file_operation(orch: &mut AppOrchestrator) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!timeout.is_zero(), "file operation completion timed out");
        match orch.rx.recv_timeout(timeout) {
            Ok(BackgroundEvent::Process(event @ ProcessEvent::UserFileOperationCompleted(_))) => {
                orch.handle_process_event(event);
                return;
            }
            Ok(BackgroundEvent::Process(event)) => orch.handle_process_event(event),
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                panic!("file operation completion timed out")
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("file operation worker went away")
            }
        }
    }
}

/// Register an MCP server entry on the orchestrator's config.
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

// ---------------------------------------------------------------------------
// Agent commands (surface B / C)
// ---------------------------------------------------------------------------

#[test]
fn run_agent_starts_a_session_and_reveals_results() {
    let mut orch = harness();
    assert!(orch.agent.current_session_id().is_none());

    orch.apply_user_command(UserCommand::RunAgent("summarise".to_string()));

    assert!(
        orch.agent.current_session_id().is_some(),
        "RunAgent must mint a session id"
    );
    assert!(
        orch.agent.state().running,
        "RunAgent must mark the agent running"
    );
    assert!(orch.agent_panel_state.show_results);
    // A first session resets the transcript rather than appending the prompt.
    assert!(
        !orch.agent_transcript.content.contains("summarise"),
        "a new session must clear the transcript, got: {}",
        orch.agent_transcript.content
    );
}

#[test]
fn run_agent_appends_prompt_to_transcript_on_continuation() {
    let mut orch = harness();
    orch.apply_user_command(UserCommand::RunAgent("first".to_string()));
    let first_session = orch.agent.current_session_id();
    assert!(first_session.is_some());

    orch.apply_user_command(UserCommand::RunAgent("second".to_string()));

    assert_eq!(
        orch.agent.current_session_id(),
        first_session,
        "a continuation reuses the current session id"
    );
    assert!(
        orch.agent_transcript.content.contains("second"),
        "the continuation prompt must be appended, got: {}",
        orch.agent_transcript.content
    );
}

#[test]
fn show_models_lists_configured_models_and_marks_done() {
    let mut orch = harness();
    // `format_models_list` renders the map KEY, the cost and the use cases.
    // `LlmConfig.model` (the provider model id) is deliberately NOT part of the
    // catalogue line, and an unpriced model reports cost 0 (`get_cost`).
    orch.config.models.insert(
        "zeta".to_string(),
        crate::config::LlmConfig {
            model: "zeta-model".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "k".to_string(),
            cost: Some(7),
            use_case: vec!["chat".to_string(), "vision".to_string()],
        },
    );
    orch.config.models.insert(
        "alpha".to_string(),
        crate::config::LlmConfig {
            model: "alpha-model".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "k".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    orch.apply_user_command(UserCommand::ShowModels);

    assert_eq!(orch.agent.state().status, "Done");
    assert!(orch.agent_panel_state.show_results);
    let response = &orch.agent.state().response;
    assert_eq!(
        response,
        &format!(
            "{}- alpha [cost: {}, use_case: chat]\n- zeta [cost: 7, use_case: chat, vision]\n",
            crate::ui::strings::MODELS_LIST_HEADER,
            crate::ui::strings::COST_LABEL_UNKNOWN
        ),
        "the catalogue is one sorted `- <key> [cost: <cost>, use_case: <cases>]` \
         line per model behind the header"
    );
    assert!(
        !response.contains("alpha-model"),
        "the provider model id is not rendered, only the config key: {response}"
    );
}

#[test]
fn show_deprecated_model_message_reports_error_status() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ShowDeprecatedModelMessage);

    assert_eq!(orch.agent.state().status, "Error");
    assert_eq!(
        orch.agent.state().response,
        crate::ui::strings::DEPRECATED_MODEL_MESSAGE
    );
    assert!(orch.agent_panel_state.show_results);
}

#[test]
fn cancel_agent_clears_running_flag_and_reports_abort() {
    let mut orch = harness();
    orch.apply_user_command(UserCommand::RunAgent("go".to_string()));
    assert!(orch.agent.state().running);

    orch.apply_user_command(UserCommand::CancelAgent);

    assert!(
        !orch.agent.state().running,
        "CancelAgent must clear running"
    );
    assert_eq!(orch.agent.state().status, "Aborted by user.");
}

#[test]
fn queue_agent_prompt_appends_to_pending_prompts() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::QueueAgentPrompt("queued one".to_string()));
    orch.apply_user_command(UserCommand::QueueAgentPrompt("queued two".to_string()));

    assert_eq!(
        orch.agent.state().pending_prompts,
        vec!["queued one".to_string(), "queued two".to_string()],
        "prompts must queue in submission order"
    );
    assert!(
        orch.agent.current_session_id().is_none(),
        "queueing must not start a session"
    );
}

#[test]
fn clear_agent_session_resets_panel_transcript_and_history() {
    let mut orch = harness();
    orch.apply_user_command(UserCommand::RunAgent("go".to_string()));
    orch.agent_panel_state.scroll_to_id = Some("heading".to_string());
    orch.agent.set_response("answer".to_string());
    orch.agent.set_thinking("reasoning".to_string());
    orch.agent
        .set_history(Some(vec![serde_json::json!({"role": "user"})]));
    assert!(orch.agent.state().running);

    orch.apply_user_command(UserCommand::ClearAgentSession);

    assert!(!orch.agent_panel_state.show_results);
    assert_eq!(orch.agent_panel_state.scroll_to_id, None);
    assert_eq!(orch.agent.state().history, None);
    assert_eq!(orch.agent.state().response, "");
    assert_eq!(orch.agent.state().thinking, "");
    assert!(orch.agent_transcript.content.is_empty());
    assert!(
        !orch.agent.state().running,
        "a running session must be cancelled by ClearAgentSession"
    );
}

#[test]
fn clear_agent_session_is_a_noop_when_no_session_is_running() {
    let mut orch = harness();
    orch.agent.set_response("stale".to_string());

    orch.apply_user_command(UserCommand::ClearAgentSession);

    assert_eq!(orch.agent.state().response, "");
    assert!(!orch.agent.state().running);
}

#[test]
fn apply_task_toggles_rewrites_checkboxes_in_transcript() {
    let mut orch = harness();
    orch.agent_transcript.content = "- [ ] first\n- [ ] second\n".to_string();

    orch.apply_user_command(UserCommand::ApplyTaskToggles {
        toggles: vec![(0, true), (1, false)],
    });

    assert_eq!(
        orch.agent_transcript.content, "- [x] first\n- [ ] second\n",
        "only the first task must be ticked"
    );
}

#[test]
fn clear_command_input_empties_the_input_buffer() {
    let mut orch = harness();
    orch.agent_panel_state.command_input = "/models".to_string();

    orch.apply_user_command(UserCommand::ClearCommandInput);

    assert_eq!(orch.agent_panel_state.command_input, "");
}

// ---------------------------------------------------------------------------
// Tabs (surface C)
// ---------------------------------------------------------------------------

#[test]
fn close_all_tabs_empties_the_tab_bar_and_selection() {
    let mut orch = harness();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    orch.tabs.open_tab(a.clone());
    orch.tabs.open_tab(b.clone());
    orch.selection.selected_file = Some(b.clone());

    orch.apply_user_command(UserCommand::CloseAllTabs);

    assert!(orch.tabs.tabs.is_empty());
    assert_eq!(orch.selection.selected_file, None);
}

#[test]
fn close_tab_reconciles_selection_for_every_branch() {
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");

    struct Case {
        name: &'static str,
        tabs: Vec<PathBuf>,
        selected: Option<PathBuf>,
        index: usize,
        expected_tabs: Vec<PathBuf>,
        expected_selected: Option<PathBuf>,
    }

    let cases = vec![
        Case {
            name: "selection survives when the closed tab is another one",
            tabs: vec![a.clone(), b.clone()],
            selected: Some(a.clone()),
            index: 1,
            expected_tabs: vec![a.clone()],
            expected_selected: Some(a.clone()),
        },
        Case {
            name: "selection falls back to the last tab when its own tab closed",
            tabs: vec![a.clone(), b.clone()],
            selected: Some(a.clone()),
            index: 0,
            expected_tabs: vec![b.clone()],
            expected_selected: Some(b.clone()),
        },
        Case {
            name: "no selection is back-filled from the remaining tabs",
            tabs: vec![a.clone(), b.clone()],
            selected: None,
            index: 0,
            expected_tabs: vec![b.clone()],
            expected_selected: Some(b.clone()),
        },
        Case {
            name: "closing the only tab clears the selection",
            tabs: vec![a.clone()],
            selected: None,
            index: 0,
            expected_tabs: vec![],
            expected_selected: None,
        },
        Case {
            name: "a stale index closes nothing and keeps the selection",
            tabs: vec![a.clone()],
            selected: Some(a.clone()),
            index: 7,
            expected_tabs: vec![a.clone()],
            expected_selected: Some(a.clone()),
        },
    ];

    for case in cases {
        let mut orch = harness();
        for tab in &case.tabs {
            orch.tabs.open_tab(tab.clone());
        }
        orch.selection.selected_file = case.selected.clone();

        orch.apply_user_command(UserCommand::CloseTab(case.index));

        assert_eq!(orch.tabs.tabs, case.expected_tabs, "case: {}", case.name);
        assert_eq!(
            orch.selection.selected_file, case.expected_selected,
            "case: {}",
            case.name
        );
    }
}

#[test]
fn close_other_tabs_keeps_only_the_named_tab() {
    let mut orch = harness();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    let c = PathBuf::from("c.md");
    orch.tabs.open_tab(a.clone());
    orch.tabs.open_tab(b.clone());
    orch.tabs.open_tab(c.clone());

    orch.apply_user_command(UserCommand::CloseOtherTabs(1));

    assert_eq!(orch.tabs.tabs, vec![b.clone()]);
    assert_eq!(orch.selection.selected_file, Some(b));
}

#[test]
fn close_other_tabs_with_a_stale_index_changes_nothing() {
    let mut orch = harness();
    let a = PathBuf::from("a.md");
    orch.tabs.open_tab(a.clone());
    orch.selection.selected_file = Some(a.clone());

    orch.apply_user_command(UserCommand::CloseOtherTabs(4));

    assert_eq!(orch.tabs.tabs, vec![a.clone()]);
    assert_eq!(
        orch.selection.selected_file,
        Some(a),
        "a stale index must not touch the selection"
    );
}

// ---------------------------------------------------------------------------
// TOC, dialogs, panels (surfaces A / D)
// ---------------------------------------------------------------------------

#[test]
fn scroll_to_header_records_the_target_heading_id() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ScrollToHeader("section-two".to_string()));

    assert_eq!(
        orch.tabs.scroll_to_header_id,
        Some("section-two".to_string())
    );
}

/// Reads the three dialog flags a dialog command is expected to touch, as
/// `(batch, tools, about)`.
type DialogFlags = fn(&Dialogs) -> (bool, bool, bool);

#[test]
fn open_dialog_commands_set_the_matching_dialog_flag_only() {
    let cases: Vec<(&str, UserCommand, DialogFlags)> = vec![
        ("OpenBatchDialog", UserCommand::OpenBatchDialog, |d| {
            (
                d.batch_dialog_open,
                d.tools_dialog_open,
                d.about_dialog_open,
            )
        }),
        ("OpenToolsDialog", UserCommand::OpenToolsDialog, |d| {
            (
                d.batch_dialog_open,
                d.tools_dialog_open,
                d.about_dialog_open,
            )
        }),
        ("OpenAboutDialog", UserCommand::OpenAboutDialog, |d| {
            (
                d.batch_dialog_open,
                d.tools_dialog_open,
                d.about_dialog_open,
            )
        }),
    ];

    for (name, cmd, flags) in cases {
        let mut orch = harness();
        let (before_batch, before_tools, before_about) = flags(&orch.dialogs);
        assert!(
            !(before_batch || before_tools || before_about),
            "{name}: no dialog may start open"
        );

        orch.apply_user_command(cmd);

        let (batch, tools, about) = flags(&orch.dialogs);
        assert!(
            batch || tools || about,
            "{name}: exactly one dialog must be open afterwards"
        );
        assert_eq!(batch, name == "OpenBatchDialog", "{name}: batch flag");
        assert_eq!(tools, name == "OpenToolsDialog", "{name}: tools flag");
        assert_eq!(about, name == "OpenAboutDialog", "{name}: about flag");
    }
}

#[test]
fn toggle_background_logs_sets_the_requested_visibility() {
    let mut orch = harness();
    assert!(!orch.background_manager.lock().unwrap().show_background_logs);

    orch.apply_user_command(UserCommand::ToggleBackgroundLogs(true));
    assert!(orch.background_manager.lock().unwrap().show_background_logs);

    orch.apply_user_command(UserCommand::ToggleBackgroundLogs(false));
    assert!(!orch.background_manager.lock().unwrap().show_background_logs);
}

#[test]
fn toggle_agent_debug_window_sets_the_requested_visibility() {
    let mut orch = harness();
    assert!(!orch.agent_panel_state.show_debug_window);

    orch.apply_user_command(UserCommand::ToggleAgentDebugWindow(true));
    assert!(orch.agent_panel_state.show_debug_window);

    orch.apply_user_command(UserCommand::ToggleAgentDebugWindow(false));
    assert!(!orch.agent_panel_state.show_debug_window);
}

#[test]
fn toggle_debug_window_command_mirrors_the_agent_debug_command() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ToggleDebugWindow(true));
    assert!(orch.agent_panel_state.show_debug_window);

    orch.apply_user_command(UserCommand::ToggleDebugWindow(false));
    assert!(!orch.agent_panel_state.show_debug_window);
}

#[test]
fn toggle_agent_debug_window_shortcut_flips_the_current_state() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ToggleAgentDebugWindowShortcut);
    assert!(orch.agent_panel_state.show_debug_window);

    orch.apply_user_command(UserCommand::ToggleAgentDebugWindowShortcut);
    assert!(!orch.agent_panel_state.show_debug_window);
}

// ---------------------------------------------------------------------------
// Debug window (surface H)
// ---------------------------------------------------------------------------

#[test]
fn debug_window_settings_are_applied_verbatim() {
    let mut orch = harness();
    orch.agent
        .state_mut()
        .debug_entries
        .push(crate::agent::events::AgentDebugEntry {
            turn: 1,
            timestamp: chrono::Local::now(),
            kind: crate::agent::events::DebugEntryKind::Incoming,
            summary: "response".to_string(),
            content: Some(serde_json::json!({"ok": true})),
            row_type: crate::agent::events::DebugEntryRow::Entry,
        });
    assert!(!orch.agent.state().debug_entries.is_empty());

    orch.apply_user_command(UserCommand::SetDebugJsonRows(12));
    orch.apply_user_command(UserCommand::SetDebugSearchText("tool".to_string()));
    orch.apply_user_command(UserCommand::SetDebugAutoScroll(false));

    assert_eq!(orch.agent_panel_state.debug_json_rows, 12);
    assert_eq!(orch.agent_panel_state.debug_search_text, "tool");
    assert!(!orch.agent_panel_state.debug_auto_scroll);

    orch.apply_user_command(UserCommand::ClearDebugEntries);
    assert!(
        orch.agent.state().debug_entries.is_empty(),
        "ClearDebugEntries must empty the accumulated entries"
    );
}

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[test]
fn select_chat_model_updates_config_and_agent_config() {
    let mut orch = harness();
    orch.config.models.insert(
        "chosen".to_string(),
        crate::config::LlmConfig {
            model: "chosen-model".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "k".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    orch.apply_user_command(UserCommand::SelectChatModel("chosen".to_string()));

    assert_eq!(
        orch.config.selected_chat_model.as_deref(),
        Some("chosen"),
        "the selection must be projected onto the app config"
    );
    assert_eq!(
        orch.agent.agent_config().selected_chat_model(),
        Some("chosen"),
        "the agent must receive the new selection"
    );
}

#[test]
fn select_chat_model_with_the_current_name_is_a_noop() {
    let mut orch = harness();
    orch.config.selected_chat_model = Some("same".to_string());
    let before = orch.agent.agent_config();

    orch.apply_user_command(UserCommand::SelectChatModel("same".to_string()));

    assert_eq!(orch.config.selected_chat_model.as_deref(), Some("same"));
    assert_eq!(
        orch.agent.agent_config().selected_chat_model(),
        before.selected_chat_model(),
        "re-selecting the active model must not push a new agent config"
    );
}

#[test]
fn change_table_width_strategy_persists_the_new_value() {
    let mut orch = harness();
    let storage = Arc::new(InMemoryConfigStorage::new());
    orch.config_storage = storage.clone();
    let before = orch.config.table_width_strategy.clone();

    orch.apply_user_command(UserCommand::ChangeTableWidthStrategy(
        DeficitStrategy::LagrangePenalty,
    ));

    assert_eq!(
        orch.config.table_width_strategy,
        DeficitStrategy::LagrangePenalty.to_config()
    );
    assert_ne!(
        orch.config.table_width_strategy, before,
        "the default strategy must actually change"
    );
    let saved = storage.saved_configs();
    assert_eq!(saved.len(), 1, "the change must be persisted exactly once");
    assert_eq!(
        saved[0].table_width_strategy,
        DeficitStrategy::LagrangePenalty.to_config()
    );
}

#[test]
fn change_table_width_strategy_still_applies_when_persistence_fails() {
    let mut orch = harness();
    orch.config_storage = Arc::new(FailingConfigStorage);

    orch.apply_user_command(UserCommand::ChangeTableWidthStrategy(
        DeficitStrategy::WaterFillRatio,
    ));

    assert_eq!(
        orch.config.table_width_strategy,
        DeficitStrategy::WaterFillRatio.to_config(),
        "a failed save must not roll back the in-memory strategy"
    );
}

#[test]
fn change_table_width_strategy_skips_the_write_when_unchanged() {
    let mut orch = harness();
    let storage = Arc::new(InMemoryConfigStorage::new());
    orch.config_storage = storage.clone();
    orch.config.table_width_strategy = DeficitStrategy::ProportionalToSlack.to_config().to_string();

    orch.apply_user_command(UserCommand::ChangeTableWidthStrategy(
        DeficitStrategy::ProportionalToSlack,
    ));

    assert!(
        storage.saved_configs().is_empty(),
        "an unchanged strategy must not trigger a save"
    );
}

#[test]
fn select_tag_filter_sets_and_clears_the_active_filter() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::SelectTagFilter(Some("research".to_string())));
    assert_eq!(orch.tags.selected_tag.as_deref(), Some("research"));

    orch.apply_user_command(UserCommand::SelectTagFilter(None));
    assert_eq!(orch.tags.selected_tag, None);
}

// ---------------------------------------------------------------------------
// Selection (surface E)
// ---------------------------------------------------------------------------

#[test]
fn select_file_single_mode_replaces_the_selection_and_opens_a_tab() {
    let mut orch = harness();
    let first = PathBuf::from("dir/first.md");
    let second = PathBuf::from("dir/second.md");

    orch.apply_user_command(UserCommand::SelectFile {
        path: first.clone(),
        multi: false,
    });
    orch.apply_user_command(UserCommand::SelectFile {
        path: second.clone(),
        multi: false,
    });

    assert_eq!(orch.selection.selected_file, Some(second.clone()));
    assert_eq!(orch.selection.selected_files.len(), 1);
    assert!(orch.selection.selected_files.contains(&second));
    assert_eq!(orch.selection.selected_dir, Some(PathBuf::from("dir")));
    assert_eq!(orch.tabs.tabs, vec![first, second]);
}

#[test]
fn select_file_multi_mode_adds_then_removes_without_touching_other_tabs() {
    let mut orch = harness();
    let a = PathBuf::from("dir/a.md");
    let b = PathBuf::from("dir/b.md");

    orch.apply_user_command(UserCommand::SelectFile {
        path: a.clone(),
        multi: true,
    });
    orch.apply_user_command(UserCommand::SelectFile {
        path: b.clone(),
        multi: true,
    });
    assert_eq!(orch.selection.selected_file, Some(b.clone()));
    assert_eq!(orch.selection.selected_files.len(), 2);

    // Removing the *active* file falls back to whichever path remains.
    orch.apply_user_command(UserCommand::SelectFile {
        path: b.clone(),
        multi: true,
    });
    assert_eq!(orch.selection.selected_file, Some(a.clone()));
    let only_a: std::collections::HashSet<PathBuf> = [a.clone()].into_iter().collect();
    assert_eq!(orch.selection.selected_files, only_a);

    // Removing a non-active file keeps the active selection.
    let c = PathBuf::from("dir/c.md");
    orch.apply_user_command(UserCommand::SelectFile {
        path: c.clone(),
        multi: true,
    });
    orch.apply_user_command(UserCommand::SelectFile {
        path: c.clone(),
        multi: true,
    });
    assert_eq!(orch.selection.selected_file, Some(a));
    assert_eq!(orch.tabs.tabs.len(), 3, "every visit opens a tab");
}

#[test]
fn select_file_without_a_parent_clears_selected_dir() {
    let mut orch = harness();
    orch.selection.selected_dir = Some(PathBuf::from("keep"));

    // `Path::new("bare.md").parent()` is `Some("")`, so a truly parent-less
    // path is needed to reach the `None` branch of `path.parent()`.
    orch.apply_user_command(UserCommand::SelectFile {
        path: PathBuf::new(),
        multi: false,
    });

    assert_eq!(orch.selection.selected_file, Some(PathBuf::new()));
    assert_eq!(
        orch.selection.selected_dir, None,
        "a path with no parent clears the directory selection"
    );
}

#[test]
fn select_directory_toggles_expansion_and_clears_file_selection() {
    let mut orch = harness();
    let dir = PathBuf::from("notes");
    orch.selection.selected_file = Some(PathBuf::from("notes/x.md"));
    orch.selection
        .selected_files
        .insert(PathBuf::from("notes/y.md"));
    orch.selection.tree_dirty = false;

    orch.apply_user_command(UserCommand::SelectDirectory {
        path: dir.clone(),
        toggle_expand: true,
    });
    assert!(orch.selection.expanded_dirs.contains(&dir));
    assert!(
        orch.selection.tree_dirty,
        "a tree change must dirty the tree"
    );
    assert_eq!(orch.selection.selected_dir, Some(dir.clone()));
    assert!(orch.selection.selected_files.is_empty());
    assert_eq!(orch.selection.selected_file, None);

    orch.apply_user_command(UserCommand::SelectDirectory {
        path: dir.clone(),
        toggle_expand: true,
    });
    assert!(
        !orch.selection.expanded_dirs.contains(&dir),
        "the second toggle collapses the directory"
    );

    // `toggle_expand: false` means "pure selection" — the expansion set is left
    // exactly as it was. It does NOT mean "collapse".
    orch.apply_user_command(UserCommand::SelectDirectory {
        path: dir.clone(),
        toggle_expand: true,
    });
    assert!(
        orch.selection.expanded_dirs.contains(&dir),
        "the third toggle expands the directory again"
    );
    orch.apply_user_command(UserCommand::SelectDirectory {
        path: dir.clone(),
        toggle_expand: false,
    });
    assert!(
        orch.selection.expanded_dirs.contains(&dir),
        "toggle_expand = false must leave an expanded directory expanded"
    );
    assert_eq!(orch.selection.selected_dir, Some(dir.clone()));

    // Symmetrically, selecting a collapsed directory without toggling must not
    // expand it.
    let collapsed = PathBuf::from("archive");
    orch.apply_user_command(UserCommand::SelectDirectory {
        path: collapsed.clone(),
        toggle_expand: false,
    });
    assert!(
        !orch.selection.expanded_dirs.contains(&collapsed),
        "toggle_expand = false never expands a collapsed directory"
    );
    assert_eq!(orch.selection.selected_dir, Some(collapsed));
}

// ---------------------------------------------------------------------------
// File-tree dialogs (surface E)
// ---------------------------------------------------------------------------

#[test]
fn rename_primes_the_dialog_with_the_stem_as_initial_value() {
    let mut orch = harness();
    let path = PathBuf::from("notes/report.md");

    orch.apply_user_command(UserCommand::Rename(path.clone()));

    assert_eq!(orch.dialogs.file_to_rename, Some(path));
    assert_eq!(orch.dialogs.rename_new_name, "report");
    assert!(orch.dialogs.rename_dialog_open);
}

#[test]
fn move_primes_the_move_dialog() {
    let mut orch = harness();
    let path = PathBuf::from("notes/report.md");

    orch.apply_user_command(UserCommand::Move(path.clone()));

    assert_eq!(orch.dialogs.file_to_move, Some(path));
    assert!(orch.dialogs.move_dialog_open);
}

#[test]
fn create_directory_and_document_prime_their_dialogs() {
    let mut orch = harness();
    let parent = PathBuf::from("notes");

    orch.apply_user_command(UserCommand::CreateDirectory {
        parent: parent.clone(),
    });
    assert_eq!(orch.dialogs.create_dir_parent, Some(parent.clone()));
    assert!(orch.dialogs.create_dir_dialog_open);
    assert!(!orch.dialogs.create_document_dialog_open);

    orch.apply_user_command(UserCommand::CreateDocument {
        parent: parent.clone(),
    });
    assert_eq!(orch.dialogs.create_document_parent, Some(parent));
    assert!(orch.dialogs.create_document_dialog_open);
}

// ---------------------------------------------------------------------------
// Skill prompts and merge (surface E)
// ---------------------------------------------------------------------------

#[test]
fn run_skill_prompt_targets_a_file_when_one_is_given() {
    let mut orch = harness();
    let file = PathBuf::from("notes/target.md");
    let dir = PathBuf::from("notes");
    orch.selection
        .selected_files
        .insert(PathBuf::from("stale.md"));

    orch.apply_user_command(UserCommand::RunSkillPrompt {
        content: "skill body".to_string(),
        target_dir: Some(dir.clone()),
        target_file: Some(file.clone()),
    });

    assert_eq!(orch.selection.selected_file, Some(file.clone()));
    let only_file: std::collections::HashSet<PathBuf> = [file].into_iter().collect();
    assert_eq!(orch.selection.selected_files, only_file);
    assert!(orch.agent.current_session_id().is_some());
    assert!(orch.agent_panel_state.show_results);
}

#[test]
fn run_skill_prompt_falls_back_to_the_target_directory() {
    let mut orch = harness();
    let dir = PathBuf::from("notes");
    orch.selection.selected_file = Some(PathBuf::from("stale.md"));
    orch.selection
        .selected_files
        .insert(PathBuf::from("stale.md"));

    orch.apply_user_command(UserCommand::RunSkillPrompt {
        content: "skill body".to_string(),
        target_dir: Some(dir.clone()),
        target_file: None,
    });

    assert_eq!(orch.selection.selected_dir, Some(dir));
    assert!(orch.selection.selected_files.is_empty());
    assert_eq!(orch.selection.selected_file, None);
    assert!(orch.agent.current_session_id().is_some());
}

#[test]
fn run_skill_prompt_without_targets_keeps_the_selection() {
    let mut orch = harness();
    let kept = PathBuf::from("notes/keep.md");
    orch.selection.selected_file = Some(kept.clone());
    orch.selection.selected_files.insert(kept.clone());

    orch.apply_user_command(UserCommand::RunSkillPrompt {
        content: "skill body".to_string(),
        target_dir: None,
        target_file: None,
    });

    assert_eq!(orch.selection.selected_file, Some(kept));
    assert!(orch.agent.current_session_id().is_some());
}

#[test]
fn merge_prompt_lists_every_selected_file() {
    let mut orch = harness();
    let a = PathBuf::from("notes/a.md");
    let b = PathBuf::from("notes/b.md");

    // The first merge starts a new session, which clears the transcript; the
    // second is a continuation and appends the prompt text.
    orch.apply_user_command(UserCommand::MergePrompt(vec![a.clone(), b.clone()]));
    assert!(orch.agent.current_session_id().is_some());
    assert!(orch.agent_panel_state.show_results);

    orch.apply_user_command(UserCommand::MergePrompt(vec![a.clone(), b.clone()]));
    let transcript = &orch.agent_transcript.content;
    assert!(
        transcript.contains(&a.to_string_lossy().to_string())
            && transcript.contains(&b.to_string_lossy().to_string()),
        "the merge prompt must list both sources, got: {transcript}"
    );
}

// ---------------------------------------------------------------------------
// File operations (surface F confirmations)
// ---------------------------------------------------------------------------

#[test]
fn confirm_create_directory_creates_the_directory() {
    let temp = tempfile::tempdir().unwrap();
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ConfirmCreateDirectory {
        parent: temp.path().to_path_buf(),
        name: "new-folder".to_string(),
    });
    finish_user_file_operation(&mut orch);

    assert!(temp.path().join("new-folder").is_dir());
}

#[test]
fn confirm_create_directory_rejects_an_unsafe_name() {
    let temp = tempfile::tempdir().unwrap();
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ConfirmCreateDirectory {
        parent: temp.path().to_path_buf(),
        name: "../escape".to_string(),
    });
    finish_user_file_operation(&mut orch);

    let created: Vec<PathBuf> = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        created.is_empty(),
        "an unsafe directory name must create nothing, found: {created:?}"
    );
}

#[test]
fn confirm_create_document_writes_the_new_document() {
    let temp = tempfile::tempdir().unwrap();
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ConfirmCreateDocument {
        parent: temp.path().to_path_buf(),
        name: "new-note".to_string(),
    });
    finish_user_file_operation(&mut orch);

    let created = temp.path().join("new-note.md");
    assert!(created.exists(), "the document must be created on disk");
}

#[test]
fn confirm_create_document_rejects_an_unsafe_name() {
    let temp = tempfile::tempdir().unwrap();
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ConfirmCreateDocument {
        parent: temp.path().to_path_buf(),
        name: "bad/name".to_string(),
    });
    finish_user_file_operation(&mut orch);

    let created: Vec<PathBuf> = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        created.is_empty(),
        "an unsafe document name must create nothing, found: {created:?}"
    );
}

#[test]
fn confirm_move_relocates_the_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("move-me.md");
    std::fs::write(&source, "content").unwrap();
    let destination = temp.path().join("target");
    std::fs::create_dir(&destination).unwrap();
    let mut orch = harness();

    orch.apply_user_command(UserCommand::ConfirmMove {
        file: source.clone(),
        destination: destination.clone(),
    });
    finish_user_file_operation(&mut orch);

    assert!(!source.exists(), "the source must be gone after the move");
    assert!(destination.join("move-me.md").is_file());
}

#[test]
fn open_in_editor_queues_an_inline_open() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("note.md");
    std::fs::write(&file, "# note").unwrap();
    let mut orch = harness();
    orch.inline_editor_enabled = true;

    orch.apply_user_command(UserCommand::OpenInEditor(file.clone()));
    assert_eq!(
        orch.pending_editor_open,
        Some(file.clone()),
        "the open request must be tracked while queued"
    );

    finish_user_file_operation(&mut orch);
    assert_eq!(
        orch.pending_editor_open, None,
        "a completed open must clear the pending marker"
    );
}

// ---------------------------------------------------------------------------
// ConfirmRename
// ---------------------------------------------------------------------------

#[test]
fn confirm_rename_rejects_an_unsafe_name_without_touching_the_file() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("keep.md");
    std::fs::write(&file, "body").unwrap();
    let mut orch = harness();
    orch.selection.selected_file = Some(file.clone());

    orch.apply_user_command(UserCommand::ConfirmRename {
        path: file.clone(),
        new_name: String::new(),
    });

    assert!(file.exists(), "an empty name must not rename anything");
    assert_eq!(orch.selection.selected_file, Some(file));
}

#[test]
fn confirm_rename_reports_a_filesystem_failure_without_updating_state() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("does-not-exist.md");
    let mut orch = harness();
    orch.selection.selected_file = Some(missing.clone());
    orch.tabs.open_tab(missing.clone());

    orch.apply_user_command(UserCommand::ConfirmRename {
        path: missing.clone(),
        new_name: "whatever".to_string(),
    });

    assert_eq!(
        orch.selection.selected_file,
        Some(missing.clone()),
        "a failed rename must leave the selection at the old path"
    );
    assert_eq!(orch.tabs.tabs, vec![missing]);
}

#[test]
fn confirm_rename_updates_tabs_selection_and_workspace_projection() {
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old.md");
    std::fs::write(&old, "---\ntags: [alpha]\n---\nbody").unwrap();
    let new = temp.path().join("renamed.md");
    let mut orch = harness();
    orch.tabs.loaded_path = Some(old.clone());
    orch.tabs.open_tab(old.clone());
    orch.selection.selected_file = Some(old.clone());
    orch.workspace.add_file(old.clone());
    orch.tags.add_tags(old.clone(), vec!["alpha".to_string()]);

    orch.apply_user_command(UserCommand::ConfirmRename {
        path: old.clone(),
        new_name: "renamed".to_string(),
    });

    assert!(new.exists(), "the file must be renamed on disk");
    assert_eq!(
        orch.tabs.loaded_path,
        Some(new.clone()),
        "the loaded tab must follow the rename"
    );
    assert_eq!(orch.tabs.tabs, vec![new.clone()]);
    assert_eq!(orch.selection.selected_file, Some(new.clone()));
    assert!(
        orch.workspace.all_files.contains(&new),
        "the renamed markdown file must stay in the workspace projection"
    );
    assert!(
        !orch.workspace.all_files.contains(&old),
        "the old path must be dropped from the projection"
    );
    assert!(
        !orch.tags.file_tags().contains_key(&old),
        "tags must be removed for the old path"
    );
    assert_eq!(
        orch.tags.file_tags().get(&new),
        Some(&vec!["alpha".to_string()]),
        "tags must be re-extracted for the renamed file"
    );
}

#[test]
fn confirm_rename_carries_the_expanded_directory_state_for_a_directory() {
    let temp = tempfile::tempdir().unwrap();
    let old_dir = temp.path().join("old-dir");
    std::fs::create_dir(&old_dir).unwrap();
    let new_dir = temp.path().join("new-dir");
    let mut orch = harness();
    orch.selection.selected_dir = Some(old_dir.clone());
    orch.selection.expanded_dirs.insert(old_dir.clone());
    orch.workspace.add_dir(old_dir.clone());

    orch.apply_user_command(UserCommand::ConfirmRename {
        path: old_dir.clone(),
        new_name: "new-dir".to_string(),
    });

    assert!(new_dir.is_dir());
    assert_eq!(orch.selection.selected_dir, Some(new_dir.clone()));
    assert!(
        !orch.selection.expanded_dirs.contains(&old_dir),
        "the old directory must no longer be marked expanded"
    );
    assert!(orch.selection.expanded_dirs.contains(&new_dir));
    assert!(orch.workspace.all_dirs.contains(&new_dir));
    assert!(!orch.workspace.all_dirs.contains(&old_dir));
}

#[test]
fn confirm_rename_drops_the_file_from_the_projection_when_the_target_is_not_markdown() {
    let temp = tempfile::tempdir().unwrap();
    // `ConfirmRename` re-appends the original extension, so a `.txt` source
    // renames to a `.txt` target and must not be re-added to the projection.
    let old = temp.path().join("note.txt");
    std::fs::write(&old, "body").unwrap();
    let mut orch = harness();
    orch.workspace.add_file(old.clone());

    orch.apply_user_command(UserCommand::ConfirmRename {
        path: old.clone(),
        new_name: "renamed".to_string(),
    });

    let renamed = temp.path().join("renamed.txt");
    assert!(renamed.exists(), "the rename itself must succeed");
    assert!(
        !orch.workspace.all_files.contains(&old),
        "the old path must be dropped"
    );
    assert!(
        !orch.workspace.all_files.contains(&renamed),
        "a non-markdown target must not be re-added, got: {:?}",
        orch.workspace.all_files
    );
}

// ---------------------------------------------------------------------------
// Tools dialog (surface F)
// ---------------------------------------------------------------------------

/// Reads one internal tool group's enabled flag from the config.
type GroupFlag = fn(&crate::config::ToolGroupsConfig) -> bool;

#[test]
fn set_tool_group_enabled_toggles_every_internal_group() {
    let cases: Vec<(&'static str, InternalToolGroup, GroupFlag)> = vec![
        ("Filesystem", InternalToolGroup::Filesystem, |g| {
            g.filesystem
        }),
        ("Web", InternalToolGroup::Web, |g| g.web),
        ("Browser", InternalToolGroup::Browser, |g| g.browser),
        ("Email", InternalToolGroup::Email, |g| g.email),
        ("Contacts", InternalToolGroup::Contacts, |g| g.contacts),
        ("Calendar", InternalToolGroup::Calendar, |g| g.calendar),
        ("CsvDb", InternalToolGroup::CsvDb, |g| g.csv_db),
        ("Weather", InternalToolGroup::Weather, |g| g.weather),
        ("Trello", InternalToolGroup::Trello, |g| g.trello),
    ];

    for (name, group, read) in cases {
        let mut orch = harness();
        let id = ToolGroupId::Internal(group);

        orch.apply_user_command(UserCommand::SetToolGroupEnabled {
            id: id.clone(),
            enabled: false,
        });
        assert!(
            !read(&orch.config.tool_groups),
            "{name}: disabling must clear the config flag"
        );
        let disabled_projection = orch.config.to_agent_config();
        assert!(
            !read(disabled_projection.tool_groups()),
            "{name}: the agent config projection must agree after disabling"
        );

        orch.apply_user_command(UserCommand::SetToolGroupEnabled { id, enabled: true });
        assert!(
            read(&orch.config.tool_groups),
            "{name}: enabling must set the config flag"
        );
        let enabled_projection = orch.config.to_agent_config();
        assert!(
            read(enabled_projection.tool_groups()),
            "{name}: the agent config projection must agree after enabling"
        );
    }
}

#[test]
fn set_tool_group_enabled_is_visible_through_the_registry_group_state() {
    let mut orch = harness();
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: id.clone(),
        enabled: false,
    });

    let agent_cfg = orch.config.to_agent_config();
    let group = orch
        .tool_context
        .load()
        .registry
        .group(&id, &agent_cfg)
        .expect("the filesystem group is always registered");
    assert!(
        !group.enabled,
        "the registry snapshot must report the group as disabled"
    );
    assert!(
        !group.tool_names.is_empty(),
        "the filesystem group must still list its tools"
    );
}

/// **Regression test for the tool-group toggle never reaching the live registry (defect D1).**
///
/// `ToolRegistry` keeps no enabled state of its own — `groups()`, `group()` and
/// `get_schema()` all derive it from the `AgentConfig` they are handed. The
/// live carrier of that projection is the agent session's config provider, so
/// the handler had to swap it in. Instead it mutated a throwaway copy of the
/// projection inside `tool_context.rcu` and discarded it, so the next agent
/// turn kept offering the tools the user had just switched off. Every
/// pre-existing assertion read `orch.config` — the app-side copy that *was*
/// always updated — which is why none of them caught it.
#[test]
fn defect_01_set_tool_group_enabled_reaches_the_live_agent_projection() {
    let mut orch = harness();
    let id = ToolGroupId::Internal(InternalToolGroup::Web);

    assert!(
        orch.agent.agent_config().tool_groups().web,
        "precondition: a freshly built session starts with the Web group enabled"
    );

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: id.clone(),
        enabled: false,
    });

    let live = orch.agent.agent_config();
    assert!(
        !live.tool_groups().web,
        "the config the next turn hands the registry must see the disable"
    );
    let group = orch
        .tool_context
        .load()
        .registry
        .group(&id, &live)
        .expect("the Web group is part of the builtin catalog");
    assert!(
        !group.enabled,
        "the live registry must report the Web group as disabled"
    );
    assert!(
        !group.tool_names.is_empty(),
        "disabling a group must not remove its tools from the catalog"
    );

    // The other direction must propagate as well.
    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: id.clone(),
        enabled: true,
    });
    let live = orch.agent.agent_config();
    assert!(
        live.tool_groups().web,
        "re-enabling must reach the live projection too"
    );
    assert!(
        orch.tool_context
            .load()
            .registry
            .group(&id, &live)
            .expect("the Web group is part of the builtin catalog")
            .enabled,
        "the live registry must report the Web group as enabled again"
    );
}

/// The MCP branch reads its flag from the projected config's server entry, so
/// it must propagate through the same swap.
#[test]
fn defect_01_disabling_an_mcp_group_reaches_the_live_agent_projection() {
    let mut orch = with_mcp_server(harness(), "known-server");
    let id = ToolGroupId::Mcp("known-server".to_string());

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: id.clone(),
        enabled: false,
    });

    let live = orch.agent.agent_config();
    assert!(
        !live
            .mcp_servers()
            .get("known-server")
            .expect("the configured server must be part of the projected config")
            .enabled,
        "the live projection must carry the disabled MCP entry"
    );
    let state = orch
        .tool_context
        .load()
        .registry
        .group(&id, &live)
        .expect("a configured MCP server must appear in the group list");
    assert!(
        !state.enabled,
        "the live registry must report the MCP group as disabled"
    );
}

/// A command that matches no group is a no-op: it must not disturb the live
/// projection either.
#[test]
fn defect_01_an_unknown_mcp_group_leaves_the_live_projection_untouched() {
    let mut orch = harness();
    let before = orch.agent.agent_config();

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: ToolGroupId::Mcp("ghost".to_string()),
        enabled: false,
    });

    assert_eq!(
        orch.agent.agent_config().tool_groups(),
        before.tool_groups(),
        "a no-op command must not change any group flag in the live projection"
    );
    assert!(
        orch.agent
            .agent_config()
            .mcp_servers()
            .get("ghost")
            .is_none(),
        "a command for an unknown server must not invent an entry in the live projection"
    );
}

#[test]
fn set_tool_group_enabled_for_a_known_mcp_server_flips_its_entry() {
    let mut orch = with_mcp_server(harness(), "known-server");
    assert!(orch.config.mcp_servers["known-server"].enabled);

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: ToolGroupId::Mcp("known-server".to_string()),
        enabled: false,
    });

    assert!(
        !orch.config.mcp_servers["known-server"].enabled,
        "the MCP entry must be disabled"
    );
    let agent_cfg = orch.config.to_agent_config();
    let state = orch
        .tool_context
        .load()
        .registry
        .group(&ToolGroupId::Mcp("known-server".to_string()), &agent_cfg)
        .expect("a configured MCP server must appear in the group list");
    assert!(!state.enabled);
}

#[test]
fn set_tool_group_enabled_for_an_unknown_mcp_server_changes_nothing() {
    let mut orch = harness();
    let before = orch.config.clone();

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: ToolGroupId::Mcp("ghost".to_string()),
        enabled: false,
    });

    assert!(orch.config.mcp_servers.is_empty());
    assert_eq!(orch.config.tool_groups, before.tool_groups);
}

#[test]
fn set_tool_group_enabled_still_applies_when_persistence_fails() {
    let mut orch = harness();
    orch.config_storage = Arc::new(FailingConfigStorage);

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: ToolGroupId::Internal(InternalToolGroup::Weather),
        enabled: false,
    });

    assert!(
        !orch.config.tool_groups.weather,
        "a failed save must not roll back the toggle"
    );
}

#[test]
fn set_tool_group_enabled_persists_the_full_config() {
    let mut orch = harness();
    let storage = Arc::new(InMemoryConfigStorage::new());
    orch.config_storage = storage.clone();

    orch.apply_user_command(UserCommand::SetToolGroupEnabled {
        id: ToolGroupId::Internal(InternalToolGroup::Email),
        enabled: false,
    });

    let saved = storage.saved_configs();
    assert_eq!(saved.len(), 1, "the toggle must be persisted once");
    assert!(!saved[0].tool_groups.email);
}

#[test]
fn clear_tool_group_error_removes_the_recorded_error() {
    let mut orch = with_mcp_server(harness(), "err-server");
    let id = ToolGroupId::Mcp("err-server".to_string());
    orch.tool_context.rcu(|bundle| {
        let mut next = (**bundle).clone();
        next.registry
            .record_error(&id, ToolGroupError::now(ToolErrorKind::Discovery, "boom"));
        next
    });
    let agent_cfg = orch.config.to_agent_config();
    assert!(
        orch.tool_context
            .load()
            .registry
            .group(&id, &agent_cfg)
            .and_then(|g| g.last_error)
            .is_some(),
        "the recorded error must be visible before the clear"
    );

    orch.apply_user_command(UserCommand::ClearToolGroupError(id.clone()));

    let agent_cfg = orch.config.to_agent_config();
    let group = orch
        .tool_context
        .load()
        .registry
        .group(&id, &agent_cfg)
        .expect("the MCP group must still be listed");
    assert_eq!(
        group.last_error, None,
        "the error must be cleared by ClearToolGroupError"
    );
}

#[test]
fn start_mcp_auth_marks_the_flow_in_progress_and_reports_failure() {
    let mut orch = harness();

    orch.apply_user_command(UserCommand::StartMcpAuth("missing-server".to_string()));

    assert!(orch.dialogs.is_oauth_in_progress("missing-server"));
    // The spawned flow fails immediately for an unconfigured server and
    // reports back over the background channel.
    let (server_name, error) = receive_mcp_auth_event(&mut orch);
    assert_eq!(server_name, "missing-server");
    assert!(
        error.is_some(),
        "an unconfigured server must report an auth error"
    );
}

/// Wait for the next `McpAuthEvent::Completed` on the background channel and
/// return its `(server_name, error)` payload.
fn receive_mcp_auth_event(orch: &mut AppOrchestrator) -> (String, Option<String>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!timeout.is_zero(), "the MCP auth event timed out");
        match orch.rx.recv_timeout(timeout) {
            Ok(BackgroundEvent::McpAuth(McpAuthEvent::Completed { server_name, error })) => {
                return (server_name, error);
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!("MCP auth event timed out"),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("the background channel went away")
            }
        }
    }
}

#[test]
fn forget_mcp_auth_clears_the_needs_auth_flag() {
    let mut orch = harness();
    let manager = orch.tool_context.load().registry.mcp_manager();
    manager.mark_needs_auth("srv", true);
    assert!(manager.needs_auth_now("srv"));
    drop(manager);

    orch.apply_user_command(UserCommand::ForgetMcpAuth("srv".to_string()));

    let manager = orch.tool_context.load().registry.mcp_manager();
    assert!(
        !manager.needs_auth_now("srv"),
        "ForgetMcpAuth must clear the stored auth requirement"
    );
}

// ---------------------------------------------------------------------------
// Batch (surface F)
// ---------------------------------------------------------------------------

#[test]
fn cancel_batch_closes_the_dialog() {
    let mut orch = harness();
    orch.dialogs.batch_dialog_open = true;

    orch.apply_user_command(UserCommand::CancelBatch);

    assert!(!orch.dialogs.batch_dialog_open);
}
