//! Initialisation of `FastMdApp` — the framework-free constructors
//! (`from_parts` for the iced boot path, `empty_state` for tests). All
//! one-time wiring (background `Task`, agent session, directory tracker,
//! persisted-UI state) lives here so consumers read from an
//! already-initialised `FastMdApp`.
//!
//! The boot path ([`crate::ui_iced::app::build_state`]) assembles the
//! orchestrator, layout and persisted snapshot itself and hands them to
//! [`FastMdApp::from_parts`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::background::Task;
use crate::background::logs::BackgroundLogs;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::Workspace;
use fastmd_agent::AgentSession;

use super::FastMdApp;

impl FastMdApp {
    /// Build a `FastMdApp` from already-constructed parts.
    ///
    /// This is the framework-free construction entry point used by the boot
    /// path ([`crate::ui_iced::app::build_state`]): the iced side
    /// assembles the [`crate::orchestrator::AppOrchestrator`], the
    /// [`PanelLayout`] and the persisted snapshot itself. Keeping this
    /// here — beside the other constructors — lets the private fields stay
    /// private while giving the boot path a framework-free way to build
    /// the struct.
    ///
    /// [RUST-020]: pure construction; no side effects.
    pub(crate) fn from_parts(
        orchestrator: crate::orchestrator::AppOrchestrator,
        layout: PanelLayout,
        persisted_ui_state: PersistedUiState,
    ) -> Self {
        Self {
            orchestrator,
            layout,
            cached_tree_rows: None,
            persisted_ui_state,
            search: crate::ui::TreeSearch::new(),
        }
    }

    /// Purpose: Build a `FastMdApp` with all UI state cleared and no background channels.
    /// Inputs: None.
    /// Outputs: `FastMdApp` with every collection empty and every optional set to `None`.
    /// Purity: Constructs a new value; no side effects.
    /// Preconditions: None.
    /// Postconditions: Caller still owns a usable background event sender paired with `rx`.
    pub fn empty_state(config: crate::config::AppConfig) -> Self {
        // Publish the supplied config into a private bus and let
        // `empty_state_via_bus` build the struct through the same
        // bus-driven init path. This keeps a single code path for tests.
        let bus = crate::bus::config::config_bus();
        bus.publish(ConfigArrived::new(config.clone()));
        Self::empty_state_via_bus(bus, config)
    }

    /// Build an `empty_state` `FastMdApp` by publishing `config` into
    /// a private bus and draining it synchronously.
    fn empty_state_via_bus(bus: Bus<ConfigArrived>, config: crate::config::AppConfig) -> Self {
        let background_task = Task::new(bus.clone());
        // The `empty_state` test path creates the background `Task`
        // (so it subscribes to the config bus) but uses its own
        // fresh `tx`/`rx` channel for the UI side. As a result the
        // file-watcher thread never has a chance to deposit a
        // `RecommendedWatcher` into the slot the UI knows about, so
        // we point the slot at a fresh empty one.
        let finished_watcher_slot = background_task.finished_watcher.clone();
        let background_manager = Arc::new(Mutex::new(BackgroundLogs::new()));
        let test_browser_session = std::sync::Arc::new(crate::agent::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing_tracker = crate::agent::session::PdfBackingTracker::new();

        let tool_context = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::AgentToolContext::new(crate::agent::tools::registry::ToolRegistry::new()),
        ));

        let agent_event_bus = Bus::new();
        let agent_event_reader = agent_event_bus.subscribe();
        let agent_event_bus_clone = agent_event_bus.clone();
        let observer_factory: crate::agent::events::AgentObserverFactory =
            std::sync::Arc::new(move |session_id| {
                let bus_observer =
                    std::sync::Arc::new(crate::bus::events::agent::BusAgentEventObserver::new(
                        session_id,
                        agent_event_bus_clone.clone(),
                    ));
                let logger_observer = std::sync::Arc::new(
                    crate::ui::agent::conversation_logger::ConversationLoggerObserver::new(
                        session_id,
                        std::env::temp_dir().join("fastmd_test_conversations"),
                    ),
                );
                std::sync::Arc::new(crate::bus::events::agent::CompositeAgentEventObserver::new(
                    vec![bus_observer, logger_observer],
                ))
            });

        let agent_builder = AgentSession::builder()
            .with_observer_factory(observer_factory)
            .with_file_observer(std::sync::Arc::new(
                crate::agent::session::bus_observer::AppFileObserver::new(
                    background_task.file_event_bus.clone(),
                ),
            ))
            .with_extension(test_browser_session.clone())
            .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(
                test_browser_session,
            )))
            .with_extension(Arc::new(pdf_backing_tracker.clone()))
            .with_tool_call_policy(Arc::new(pdf_backing_tracker.clone()))
            .with_tool_context(tool_context.clone());
        #[cfg(feature = "vector-search")]
        let agent_builder = agent_builder.with_extension(Arc::new(
            crate::agent::tools::vector_search::VectorSearchExt(
                background_task.vector_search_service.clone(),
            ),
        ));
        let agent = agent_builder.build();
        agent.set_agent_config(config.to_agent_config());

        // Extract needed fields from background_task BEFORE moving file_event_bus
        let rx_std = background_task.rx_std();
        let tx = background_task.tx;
        let user_file_operation_worker =
            crate::background::UserFileOperationWorker::spawn(tx.clone());
        let event_bus = background_task.file_event_bus;
        let dir_event_bus = background_task.dir_event_bus;
        // The single source of truth for file/directory state. File events
        // are applied from the orchestrator's own file reader
        // (`apply_file_changed`); the projection keeps a live directory
        // reader as the drain for legacy wiring.
        let workspace = Workspace::new(None, Some(dir_event_bus.subscribe()));

        let selection = FileSelection::new();
        let mut dialogs = Dialogs::new();
        let batch_dialog_config = crate::agent::batch::types::BatchDialogConfig {
            available_dirs: config
                .content_libraries
                .iter()
                .map(|lib| PathBuf::from(&lib.root_folder))
                .collect(),
            ..Default::default()
        };
        dialogs.batch_dialog_config = batch_dialog_config;

        let content_libraries = config.content_libraries.clone();
        let inline_editor_enabled = config.inline_editor_enabled;

        let user_command_bus = crate::bus::core::Bus::new();
        let user_command_reader = user_command_bus.subscribe();

        Self {
            orchestrator: crate::orchestrator::AppOrchestrator {
                content_libraries,
                rx: rx_std,
                tx,
                user_file_operation_worker,
                file_event_bus: event_bus.clone(),
                dir_event_bus: dir_event_bus.clone(),
                file_event_reader: Some(event_bus.subscribe()),
                workspace,
                pdf_backing_tracker,
                tags: Tags::new(),
                selection,
                tabs: Tabs::new(),
                _watcher: None,
                agent,
                dialogs,
                text_buffer: TextBuffer::new(),
                inline_editor_enabled,
                background_manager,
                config,
                config_storage: std::sync::Arc::new(crate::config::NoopConfigStorage),
                config_reader: None,
                pending_file_load: None,
                pending_editor_open: None,
                finished_watcher_slot,
                tool_context,
                agent_event_bus,
                agent_event_reader: Some(agent_event_reader),
                agent_event_lagged: false,
                agent_transcript: AgentTranscript::new(uuid::Uuid::nil()),
                agent_panel_state: AgentPanelState::new(),
                user_command_bus,
                user_command_reader: Some(user_command_reader),
            },
            layout: PanelLayout::new(),
            cached_tree_rows: None,
            persisted_ui_state: PersistedUiState::default(),
            search: crate::ui::TreeSearch::new(),
        }
    }
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
