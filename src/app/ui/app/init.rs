//! Initialisation of `FastMdApp` — the egui theme setup and the two
//! constructors (`new` for the real eframe creation context, `empty_state`
//! for tests). All one-time wiring (background `Task`, agent session,
//! directory tracker, persisted-UI state) lives here so the per-frame
//! `update` and `render` modules can read from an already-initialised
//! `FastMdApp`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::agent::AgentSession;
use crate::app::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tags, TextBuffer};
use crate::background::{BackgroundLogs, Task};
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::AppConfig;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::workspace::watcher::{DirectoryTracker, FileEventProcessor};

use super::{FastMdApp, PERSISTED_UI_STATE_KEY};

impl FastMdApp {
    /// Purpose: Pin the egui context to the dark theme with the FastMD brand
    /// palette (RGB(9, 9, 11) surfaces, indigo selection, 8px window corners,
    /// 4px widget corners, bright off-white text).
    /// Inputs: `ctx` - The egui context whose theme is to be configured.
    /// Outputs: None.
    /// Purity: Impure (mutates the egui context's options).
    /// Preconditions: `ctx` is a valid egui context.
    /// Postconditions: The dark theme is the active theme, and the dark
    /// theme's visuals match the FastMD palette so UI-002 (dark color
    /// scheme) holds even if the system preference reports light mode.
    ///
    /// egui 0.35 split the global style into a Dark and a Light theme,
    /// picked at runtime by `ThemePreference` (default `System`).
    /// `set_visuals` writes to the *currently active* theme only, so on
    /// systems that report a light-mode preference the next frame can
    /// flip the active theme back to the default light visuals and the
    /// carefully tuned dark background is lost. Forcing the dark theme
    /// and applying the visuals to the dark theme explicitly makes the
    /// dark color scheme the source of truth.
    pub fn configure_dark_theme(ctx: &egui::Context) {
        ctx.set_theme(egui::Theme::Dark);

        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.panel_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.selection.bg_fill = egui::Color32::from_rgb(99, 102, 241);
        visuals.window_corner_radius = 8.0.into();
        visuals.widgets.noninteractive.corner_radius = 4.0.into();
        visuals.widgets.inactive.corner_radius = 4.0.into();
        visuals.widgets.hovered.corner_radius = 4.0.into();
        visuals.widgets.active.corner_radius = 4.0.into();

        let bright_text = egui::Color32::from_gray(210);
        visuals.widgets.noninteractive.fg_stroke.color = bright_text;
        visuals.widgets.inactive.fg_stroke.color = bright_text;
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;

        // Apply to the dark theme explicitly so the visuals persist
        // even if the active theme ever flips to Light.
        ctx.set_visuals_of(egui::Theme::Dark, visuals);
    }

    pub fn new(cc: &eframe::CreationContext<'_>, config_bus: Bus<ConfigArrived>) -> Self {
        // egui 0.35 split the global style into a Dark and a Light
        // theme, picked at runtime by `ThemePreference` (default
        // `System`). `set_visuals` writes to the *currently active*
        // theme only, so on systems that report a light-mode
        // preference the next frame can flip the active theme back
        // to the default light visuals and the carefully tuned
        // dark background is lost. Force the dark theme and apply
        // our custom visuals to the dark theme explicitly.
        Self::configure_dark_theme(&cc.egui_ctx);

        // Subscribe before any worker is spawned so the first
        // `ConfigArrived` publish reaches every reader.
        let config_reader = config_bus.subscribe();

        // `Task::new` and `AgentSession::new` each subscribe
        // to the same bus, then defer their own work until the
        // event arrives. The background `Task` spawns a thread that
        // waits on its own reader; the agent's reader is drained on
        // the UI thread in `update_ui`.
        let background_task = Task::new(config_bus.clone());
        background_task
            .tx
            .set_repaint_callback(std::sync::Arc::new({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }));
        // The tools manager subscribes to the same bus and performs
        // the one-time
        let tool_context = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::AgentToolContext::new(crate::agent::tools::registry::ToolRegistry::new()),
        ));
        // Start MCP initialization immediately on the app's initial
        // background thread, so the UI thread never blocks on MCP
        // network I/O at startup.
        crate::app::session::spawn_config_subscription(
            tool_context.clone(),
            config_bus.clone(),
            background_task.tx.clone(),
        );
        // The file-watcher thread writes the `RecommendedWatcher`
        // handle into this slot before sending `FsEvent::Finished`;
        // the UI takes ownership from `task_take_finished_watcher`
        // in `handle_fs_event`.
        let finished_watcher_slot = background_task.finished_watcher.clone();
        let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
        let background_manager = Arc::new(Mutex::new(BackgroundLogs::new()));
        // One BrowserSession for the whole app lifetime; shared
        // with the agent and (read-only) with the Tools dialog
        // so the UI can call `tick()` / `forget()`. Lazily
        // launches a Firefox process on first browser tool call.
        let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing_tracker = crate::app::session::PdfBackingTracker::new();
        // The agent no longer subscribes to `Bus<ConfigArrived>`.
        // The orchestrator projects `AppConfig -> AgentConfig`
        // and pushes the result into the agent's domain-config
        // cell. We hand `FastMdApp` a `Sender<ConfigArrived>`-style
        // channel? No — we keep `config_bus` and `config_reader`
        // so the UI can still drain the bus; the `AgentSession`
        // itself is built without a `config_bus` parameter.
        let agent_event_bus = Bus::new();
        let agent_event_reader = agent_event_bus.subscribe();
        let agent_event_bus_clone = agent_event_bus.clone();
        let observer_factory: crate::agent::events::AgentObserverFactory =
            std::sync::Arc::new(move |session_id| {
                std::sync::Arc::new(crate::app::events::BusAgentEventObserver::new(
                    session_id,
                    agent_event_bus_clone.clone(),
                ))
            });

        let agent_builder = AgentSession::builder()
            .with_observer_factory(observer_factory)
            .with_file_observer(std::sync::Arc::new(
                crate::app::session::bus_observer::AppFileObserver::new(
                    background_task.file_event_bus.clone(),
                ),
            ))
            .with_extension(browser_session.clone())
            .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(
                browser_session.clone(),
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

        let event_bus = background_task.file_event_bus;
        let dir_tracker = DirectoryTracker::new(event_bus.subscribe());

        let mut persisted_ui_state: PersistedUiState = cc
            .storage
            .and_then(|s| s.get_string(PERSISTED_UI_STATE_KEY))
            .map(|json| {
                serde_json::from_str(&json).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to parse persisted UI state, using defaults");
                    PersistedUiState::default()
                })
            })
            .unwrap_or_default();

        // Schema migration: state written by the pre-fix build
        // has no `schema_version` field (deserialises to 0) and
        // its `font_size_scale` actually holds the absolute
        // OS-reported ppp — treating it as a multiplier on top
        // of the same ppp was the historical compounding bug.
        // Clear the field on the first launch after the fix so
        // the user starts at the OS default; other fields
        // (panel widths, expanded dirs) are preserved.
        if persisted_ui_state.schema_version < crate::app::persisted::CURRENT_SCHEMA_VERSION {
            tracing::info!(
                from = persisted_ui_state.schema_version,
                to = crate::app::persisted::CURRENT_SCHEMA_VERSION,
                "migrating persisted UI state: clearing legacy font_size_scale"
            );
            persisted_ui_state.font_size_scale = None;
            persisted_ui_state.schema_version = crate::app::persisted::CURRENT_SCHEMA_VERSION;
        }

        let mut layout = PanelLayout::new();
        if let Some(w) = persisted_ui_state.left_panel_width {
            layout.left_panel_width = Some(w);
        }
        if let Some(w) = persisted_ui_state.right_panel_width {
            layout.right_panel_width = Some(w);
        }

        let mut selection = FileSelection::new();
        for dir in &persisted_ui_state.expanded_dirs {
            selection.expanded_dirs.insert(dir.clone());
        }

        let dialogs = Dialogs::new();

        Self {
            orchestrator: crate::app::orchestrator::AppOrchestrator {
                content_libraries: Vec::new(),
                rx: background_task.rx,
                tx: background_task.tx,
                file_event_reader: Some(event_bus.subscribe()),
                file_event_bus: event_bus,
                file_processor,
                pdf_backing_tracker,
                directory_tracker: dir_tracker,
                tags: Tags::new(),
                selection,
                tabs: crate::app::tabs::Tabs::new(),
                _watcher: None,
                agent,
                dialogs,
                submit_prompt: None,
                text_buffer: TextBuffer::new(),
                inline_editor_enabled: false,
                background_manager,
                config: AppConfig::default(),
                config_reader: Some(config_reader),
                pending_file_load: None,
                finished_watcher_slot,
                tool_context: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
                    crate::agent::AgentToolContext::new(
                        crate::agent::tools::registry::ToolRegistry::new(),
                    ),
                )),
                agent_event_bus,
                agent_event_reader: Some(agent_event_reader),
                agent_event_lagged: false,
                agent_transcript: AgentTranscript::new(uuid::Uuid::nil()),
                agent_panel_state: AgentPanelState::new(),
            },
            layout,
            cached_tree_rows: None,
            persisted_ui_state,
            persisted_font_applied: false,
            os_baseline_ppp: None,
            applied_font_scale: 1.0,
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
        // bus-driven init path. This keeps a single code path for
        // tests that don't need a real `CreationContext`.
        let bus = crate::bus::config::config_bus();
        bus.publish(ConfigArrived::new(config.clone()));
        Self::empty_state_via_bus(bus, config)
    }

    /// Build an `empty_state` `FastMdApp` by publishing `config` into
    /// a private bus and draining it synchronously. This is the test
    /// counterpart of [`Self::new`]: no `CreationContext` is needed
    /// because we don't apply egui visuals.
    fn empty_state_via_bus(bus: Bus<ConfigArrived>, config: crate::config::AppConfig) -> Self {
        let background_task = Task::new(bus.clone());
        // The `empty_state` test path creates the background `Task`
        // (so it subscribes to the config bus) but uses its own
        // fresh `tx`/`rx` channel for the UI side. As a result the
        // file-watcher thread never has a chance to deposit a
        // `RecommendedWatcher` into the slot the UI knows about, so
        // we point the slot at a fresh empty one.
        let finished_watcher_slot = background_task.finished_watcher.clone();
        let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
        let background_manager = Arc::new(Mutex::new(BackgroundLogs::new()));
        let test_browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing_tracker = crate::app::session::PdfBackingTracker::new();
        let agent_event_bus = Bus::new();
        let agent_event_reader = agent_event_bus.subscribe();
        let agent_event_bus_clone = agent_event_bus.clone();
        let observer_factory: crate::agent::events::AgentObserverFactory =
            std::sync::Arc::new(move |session_id| {
                std::sync::Arc::new(crate::app::events::BusAgentEventObserver::new(
                    session_id,
                    agent_event_bus_clone.clone(),
                ))
            });

        let agent_builder = AgentSession::builder()
            .with_observer_factory(observer_factory)
            .with_file_observer(std::sync::Arc::new(
                crate::app::session::bus_observer::AppFileObserver::new(
                    background_task.file_event_bus.clone(),
                ),
            ))
            .with_extension(test_browser_session.clone())
            .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(
                test_browser_session,
            )))
            .with_extension(Arc::new(pdf_backing_tracker.clone()))
            .with_tool_call_policy(Arc::new(pdf_backing_tracker.clone()))
            .with_tool_context(Arc::new(arc_swap::ArcSwap::from_pointee(
                crate::agent::AgentToolContext::new(
                    crate::agent::tools::registry::ToolRegistry::new(),
                ),
            )));
        #[cfg(feature = "vector-search")]
        let agent_builder = agent_builder.with_extension(Arc::new(
            crate::agent::tools::vector_search::VectorSearchExt(
                background_task.vector_search_service.clone(),
            ),
        ));
        let agent = agent_builder.build();
        agent.set_agent_config(config.to_agent_config());

        let event_bus = background_task.file_event_bus;
        let dir_tracker = DirectoryTracker::new(event_bus.subscribe());

        let selection = FileSelection::new();
        let mut dialogs = Dialogs::new();
        let batch_dialog_config = crate::app::batch::types::BatchDialogConfig {
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

        Self {
            orchestrator: crate::app::orchestrator::AppOrchestrator {
                content_libraries,
                rx: background_task.rx,
                tx: background_task.tx,
                file_event_bus: event_bus.clone(),
                file_event_reader: Some(event_bus.subscribe()),
                file_processor,
                pdf_backing_tracker,
                tags: Tags::new(),
                selection,
                tabs: crate::app::tabs::Tabs::new(),
                _watcher: None,
                agent,
                dialogs,
                submit_prompt: None,
                text_buffer: TextBuffer::new(),
                inline_editor_enabled,
                background_manager,
                directory_tracker: dir_tracker,
                config,
                config_reader: None,
                pending_file_load: None,
                finished_watcher_slot,
                tool_context: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
                    crate::agent::AgentToolContext::new(
                        crate::agent::tools::registry::ToolRegistry::new(),
                    ),
                )),
                agent_event_bus,
                agent_event_reader: Some(agent_event_reader),
                agent_event_lagged: false,
                agent_transcript: AgentTranscript::new(uuid::Uuid::nil()),
                agent_panel_state: AgentPanelState::new(),
            },
            layout: PanelLayout::new(),
            cached_tree_rows: None,
            persisted_ui_state: PersistedUiState::default(),
            persisted_font_applied: false,
            os_baseline_ppp: None,
            applied_font_scale: 1.0,
        }
    }
}
