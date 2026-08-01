//! Agent session manager — lifecycle and UI-visible state for a single LLM agent session (status, response, thinking, history, token usage).

use crate::agent::AgentContext;
use crate::app::browser::BrowserSession;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::messages::TokenUsageInfo;
use crate::bus::events::typed::{AgentEvent, BackgroundEvent};
use crate::config::AppConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Agent state exposed to UI components.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub running: bool,
    pub status: String,
    pub thinking: String,
    pub response: String,
    /// Stable string id of the heading the agent-results panel
    /// should scroll to. The UI layer maps it to an
    /// `egui::Id` at render time.
    pub scroll_to_id: Option<String>,
    pub history: Option<Vec<Value>>,
    pub token_usage: Option<TokenUsageInfo>,
    pub total_usage: TokenUsageInfo,
}

/// Manages the lifecycle and state of a single LLM agent session.
///
/// Responsibilities:
/// - Owns agent state (status, thinking, response, history, token usage)
/// - Subscribes to the [`crate::bus::events::config::ConfigArrived`] bus so the
///   [`AppConfig`] used by `start_session` is the one published at
///   startup, not a value captured at construction time.
/// - Provides `start_session` to launch a new agent thread
/// - Handles incoming [`BackgroundEvent::Agent`] events to update state
/// - Supports cancellation via cancel flag
/// - Exposes read-only `AgentState` for UI rendering
pub struct AgentSessionManager {
    state: AgentState,
    cancel_flag: Option<Arc<AtomicBool>>,
    config: AppConfig,
    /// Reader for the configuration-arrival bus. Subscribed during
    /// `new` so the publish that happens after construction is
    /// observed. Drained on every UI frame by
    /// [`Self::drain_config`].
    config_reader: Option<BusReader<ConfigArrived>>,
    /// Tracks whether the bus has delivered the config yet so
    /// config-derived work uses the published `AppConfig` instead
    /// of the default placeholder.
    config_arrived: bool,
    pub command_input: String,
    show_results: bool,
    /// Long-lived headless Firefox session, shared with the
    /// tool executor. Lazily launches a browser on first use.
    /// Owned by the application, not the agent — sessions
    /// survive across agent turns so cookies persist (BRWS-001).
    browser_session: Arc<BrowserSession>,
}

impl AgentSessionManager {
    /// Subscribe to the configuration-arrival bus and return an
    /// empty manager. The bus is the source of truth for the
    /// [`AppConfig`] used by `start_session`; until the bus
    /// delivers its event that call falls back to
    /// [`AppConfig::default`].
    pub fn new(config_bus: Bus<ConfigArrived>, browser_session: Arc<BrowserSession>) -> Self {
        Self {
            state: AgentState {
                running: false,
                status: String::new(),
                thinking: String::new(),
                response: String::new(),
                scroll_to_id: None,
                history: None,
                token_usage: None,
                total_usage: TokenUsageInfo::default(),
            },
            cancel_flag: None,
            config: AppConfig::default(),
            // Subscribe before returning so the publish that
            // happens immediately afterwards (in main / tests) is
            // observed by this reader.
            config_reader: Some(config_bus.subscribe()),
            config_arrived: false,
            command_input: String::new(),
            show_results: false,
            browser_session,
        }
    }

    /// Test helper: build a manager whose [`AppConfig`] is set
    /// immediately, without going through the bus. Mirrors the old
    /// `new(config)` signature for callers that just want a
    /// populated manager (existing test fixtures).
    #[doc(hidden)]
    pub fn new_for_test(config: AppConfig, browser_session: Arc<BrowserSession>) -> Self {
        Self {
            state: AgentState {
                running: false,
                status: String::new(),
                thinking: String::new(),
                response: String::new(),
                scroll_to_id: None,
                history: None,
                token_usage: None,
                total_usage: TokenUsageInfo::default(),
            },
            cancel_flag: None,
            config,
            config_reader: None,
            config_arrived: true,
            command_input: String::new(),
            show_results: false,
            browser_session,
        }
    }

    /// Drain one event from the configuration bus (non-blocking
    /// `try_recv`). If a [`ConfigArrived`] event is observed, the
    /// stored [`AppConfig`] is updated. Returns `true` if the
    /// config was updated by this call.
    ///
    /// Called once per frame from the UI's
    /// [`FastMdApp::update_ui`](crate::ui::FastMdApp::update_ui)
    /// path. The reader is taken on the first success so the
    /// per-frame cost is a single `Option::None` check after the
    /// initial delivery.
    pub fn drain_config(&mut self) -> bool {
        let Some(reader) = self.config_reader.as_ref() else {
            return false;
        };
        match reader.try_recv() {
            Ok(event) => {
                self.config = event.config;
                self.config_arrived = true;
                tracing::info!(
                    name = "config.arrived",
                    "AgentSessionManager received configuration"
                );
                // Drop the reader — we only care about the first
                // arrival (no hot reload).
                self.config_reader = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Bus is gone; stop trying to drain.
                self.config_reader = None;
                false
            }
        }
    }

    /// `true` once the bus has delivered (or the test path bypassed
    /// it).
    pub fn config_arrived(&self) -> bool {
        self.config_arrived
    }

    /// Update the stored config directly. Bypasses the bus — used
    /// by the UI to keep the agent's config in sync with the
    /// app-wide config that was set during the bus drain.
    pub fn set_config(&mut self, config: AppConfig) {
        self.config = config;
        self.config_arrived = true;
    }

    /// Hand out an `Arc` clone of the headless-browser session
    /// so the UI layer can call [`BrowserSession::tick`]
    /// (idle-timeout) and [`BrowserSession::forget`]
    /// (clean logout) without going through the agent.
    pub fn browser_session(&self) -> Arc<BrowserSession> {
        self.browser_session.clone()
    }

    /// Get a read-only view of the current agent state.
    pub fn state(&self) -> &AgentState {
        &self.state
    }

    /// Get a mutable reference to state (for internal use only).
    /// Prefer specific setters for external mutation.
    pub fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    /// Set the agent status message.
    pub fn set_status(&mut self, status: String) {
        self.state.status = status;
    }

    /// Set the thinking content.
    pub fn set_thinking(&mut self, thinking: String) {
        self.state.thinking = thinking;
    }

    /// Set the response content.
    pub fn set_response(&mut self, response: String) {
        self.state.response = response.clone();
        // Note: we don't keep a separate current_response buffer; the app doesn't need it.
    }

    /// Set the scroll-to-id for the agent UI.
    pub fn set_scroll_to_id(&mut self, id: Option<String>) {
        self.state.scroll_to_id = id;
    }

    /// Set the running flag.
    pub fn set_running(&mut self, running: bool) {
        self.state.running = running;
    }

    pub fn show_results(&self) -> bool {
        self.show_results
    }

    pub fn set_show_results(&mut self, show: bool) {
        self.show_results = show;
    }

    /// Set the conversation history.
    pub fn set_history(&mut self, history: Option<Vec<Value>>) {
        self.state.history = history;
    }

    /// Clear the response and history (for new session).
    pub fn clear_history(&mut self) {
        self.state.history = None;
        self.state.token_usage = None;
        self.state.total_usage = TokenUsageInfo::default();
    }

    /// Cancel any running agent session.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.cancel_flag.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
        self.state.running = false;
        self.state.status = "Aborted by user.".to_string();
    }

    /// Start a new agent session with the given prompt.
    ///
    /// This spawns a background thread running `crate::agent::run_agent`.
    /// The agent sends messages to `gui_tx`, which should be the app's
    /// main channel (the same channel used for background messages).
    pub fn start_session(
        &mut self,
        gui_tx: std::sync::mpsc::Sender<BackgroundEvent>,
        prompt: String,
        active_file: Option<PathBuf>,
        active_dir: Option<PathBuf>,
        selected_files: HashSet<PathBuf>,
        file_event_bus: Bus<crate::bus::events::file::FileEvent>,
    ) {
        // Reset state for new session
        self.state.running = true;
        self.state.status = "Initializing agent...".to_string();
        self.state.thinking.clear();
        self.state.response.clear();
        self.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
        let cancel_flag = self.cancel_flag.clone().unwrap();

        // Build context
        let ctx = AgentContext {
            config: self.config.clone(),
            tx_gui: gui_tx,
            file_event_bus,
            active_file,
            active_dir,
            selected_files,
            prompt: prompt.clone(),
            cancel_flag,
            history: self.state.history.clone(),
            current_response: self.state.response.clone(),
            model_name: None,
            browser_session: self.browser_session.clone(),
        };

        std::thread::spawn(move || {
            crate::agent::run_agent(ctx);
        });
    }

    /// Consume and handle a single typed [`AgentEvent`] from the
    /// background event channel.
    ///
    /// Returns `true` if the UI should repaint.
    pub fn handle_agent_event(&mut self, event: AgentEvent) -> bool {
        match event {
            AgentEvent::Status(status) => {
                self.state.status = status;
                true
            }
            AgentEvent::Thinking(thinking) => {
                self.state.thinking = thinking;
                true
            }
            AgentEvent::Response(resp) => {
                self.state.response = resp.clone();
                true
            }
            AgentEvent::Finished(history) => {
                self.state.running = false;
                self.state.history = Some(history);
                true
            }
            AgentEvent::Failed(err) => {
                self.state.running = false;
                self.state.status = format!("Error: {}", err);
                true
            }
            AgentEvent::TokenUsage(info) => {
                if info.prompt_tokens > self.state.total_usage.prompt_tokens {
                    self.state.total_usage.prompt_tokens = info.prompt_tokens;
                }
                self.state.total_usage.completion_tokens = self
                    .state
                    .total_usage
                    .completion_tokens
                    .saturating_add(info.completion_tokens);
                self.state.total_usage.total_tokens = self
                    .state
                    .total_usage
                    .total_tokens
                    .saturating_add(info.total_tokens);
                self.state.total_usage.cached_tokens = Some(
                    self.state
                        .total_usage
                        .cached_tokens
                        .unwrap_or(0)
                        .saturating_add(info.cached_tokens.unwrap_or(0)),
                );
                self.state.total_usage.reasoning_tokens = Some(
                    self.state
                        .total_usage
                        .reasoning_tokens
                        .unwrap_or(0)
                        .saturating_add(info.reasoning_tokens.unwrap_or(0)),
                );
                self.state.token_usage = Some(info);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_new_manager_is_empty() {
        let config = AppConfig::default();
        let mgr = AgentSessionManager::new_for_test(
            config,
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );
        let state = mgr.state();
        assert!(!state.running);
        assert!(state.status.is_empty());
        assert!(state.response.is_empty());
    }

    #[test]
    fn test_cancel_sets_running_false() {
        let config = AppConfig::default();
        let mut mgr = AgentSessionManager::new_for_test(
            config,
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );
        mgr.state_mut().running = true;
        mgr.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
        mgr.cancel();
        assert!(!mgr.state.running);
        assert!(mgr.state.status.contains("Aborted"));
    }

    #[test]
    fn test_clear_history_resets_fields() {
        let config = AppConfig::default();
        let mut mgr = AgentSessionManager::new_for_test(
            config,
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );
        mgr.state.history = Some(vec![Value::String("old".to_string())]);
        mgr.state.token_usage = Some(TokenUsageInfo {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        });
        mgr.clear_history();
        assert!(mgr.state.history.is_none());
        assert!(mgr.state.token_usage.is_none());
    }

    /// Regression: the bus-driven constructor must observe the
    /// first published `ConfigArrived` and store it. The second
    /// `drain_config` call (after the reader is dropped) is a
    /// no-op.
    #[test]
    fn test_drain_config_observes_first_event() {
        use crate::bus::config::config_bus;
        use crate::bus::events::config::ConfigArrived;

        let bus = config_bus();
        let mut mgr = AgentSessionManager::new(
            bus.clone(),
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );

        // Before any event: not arrived, default config in use.
        assert!(!mgr.config_arrived());

        let cfg = AppConfig {
            csv_db_path: Some("/tmp/foo".to_string()),
            ..AppConfig::default()
        };
        bus.publish(ConfigArrived::new(cfg.clone()));

        assert!(mgr.drain_config());
        assert!(mgr.config_arrived());
        assert_eq!(mgr.config.csv_db_path, cfg.csv_db_path);

        // Subsequent drain is a no-op (reader dropped after first).
        assert!(!mgr.drain_config());
    }

    /// Regression: `drain_config` returns false when no event has
    /// been published yet, and does not flip the `config_arrived`
    /// flag.
    #[test]
    fn test_drain_config_returns_false_when_empty() {
        let bus = crate::bus::config::config_bus();
        let mut mgr = AgentSessionManager::new(
            bus,
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );

        assert!(!mgr.drain_config());
        assert!(!mgr.config_arrived());
    }

    /// Regression: the production startup order in `main.rs`
    /// must be "construct subscribers first, then publish" —
    /// `tokio::sync::broadcast` only delivers an event to
    /// subscribers that exist at publish time. Reversing the
    /// order silently drops the event and the agent never sees
    /// the loaded config. This test pins that contract: build
    /// the manager (which subscribes), then publish, then drain.
    #[test]
    fn test_construct_then_publish_order_drains_config() {
        let bus = crate::bus::config::config_bus();
        // 1. Construct first — this is the `AgentSessionManager::new`
        //    call inside `FastMdApp::new`. It subscribes here.
        let mut mgr = AgentSessionManager::new(
            bus.clone(),
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );

        // 2. Publish second — this is the line in `main.rs` that
        //    fires `config_bus.publish(...)` after construction.
        //    Because the subscription is already in place, the
        //    broadcast channel delivers the event to this reader.
        let cfg = AppConfig {
            csv_db_path: Some("/tmp/regression".to_string()),
            ..AppConfig::default()
        };
        bus.publish(ConfigArrived::new(cfg.clone()));

        // 3. Drain on the first UI frame.
        assert!(mgr.drain_config());
        assert!(mgr.config_arrived());
        assert_eq!(mgr.config.csv_db_path, cfg.csv_db_path);
    }

    /// Regression: the *reverse* order (publish, then construct)
    /// silently drops the event. We assert this so any future
    /// refactor that flips the order is caught immediately.
    #[test]
    fn test_publish_then_construct_order_drops_event() {
        let bus = crate::bus::config::config_bus();
        bus.publish(ConfigArrived::new(AppConfig::default()));

        // Subscribing after the publish means the broadcast
        // channel won't deliver the event to this reader.
        let mut mgr = AgentSessionManager::new(
            bus,
            Arc::new(crate::app::browser::BrowserSession::new(
                &AppConfig::default(),
            )),
        );

        assert!(!mgr.drain_config());
        assert!(!mgr.config_arrived());
    }
}
