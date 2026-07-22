//! Agent session manager — lifecycle and UI-visible state for a single LLM agent session (status, response, thinking, history, token usage).

use crate::agent::AgentContext;
use crate::config::AppConfig;
use crate::file_events::Bus;
use crate::messages::{BackgroundMessage, TokenUsageInfo};
use eframe::egui;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Agent state exposed to UI components.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub running: bool,
    pub status: String,
    pub thinking: String,
    pub response: String,
    pub scroll_to_id: Option<egui::Id>,
    pub history: Option<Vec<Value>>,
    pub token_usage: Option<TokenUsageInfo>,
    pub total_usage: TokenUsageInfo,
}

/// Manages the lifecycle and state of a single LLM agent session.
///
/// Responsibilities:
/// - Owns agent state (status, thinking, response, history, token usage)
/// - Provides `start_session` to launch a new agent thread
/// - Handles incoming `BackgroundMessage::Agent*` messages to update state
/// - Supports cancellation via cancel flag
/// - Exposes read-only `AgentState` for UI rendering
pub struct AgentSessionManager {
    state: AgentState,
    cancel_flag: Option<Arc<AtomicBool>>,
    config: AppConfig,
    pub command_input: String,
    show_results: bool,
}

impl AgentSessionManager {
    /// Create a new, empty manager (no active session).
    pub fn new(config: AppConfig) -> Self {
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
            command_input: String::new(),
            show_results: false,
        }
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
    pub fn set_scroll_to_id(&mut self, id: Option<egui::Id>) {
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
        gui_tx: std::sync::mpsc::Sender<BackgroundMessage>,
        prompt: String,
        active_file: Option<PathBuf>,
        active_dir: Option<PathBuf>,
        selected_files: HashSet<PathBuf>,
        file_event_bus: Bus<crate::file_events::FileEvent>,
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
        };

        std::thread::spawn(move || {
            crate::agent::run_agent(ctx);
        });
    }

    /// Consume and handle a single background message from the agent thread.
    /// Returns `true` if the UI should repaint.
    pub fn handle_background_message(&mut self, msg: BackgroundMessage) -> bool {
        match msg {
            BackgroundMessage::AgentStatus(status) => {
                self.state.status = status;
                true
            }
            BackgroundMessage::AgentThinking(thinking) => {
                self.state.thinking = thinking;
                true
            }
            BackgroundMessage::AgentResponse(resp) => {
                self.state.response = resp.clone();
                true
            }
            BackgroundMessage::AgentFinished(history) => {
                self.state.running = false;
                self.state.history = Some(history);
                true
            }
            BackgroundMessage::AgentFailed(err) => {
                self.state.running = false;
                self.state.status = format!("Error: {}", err);
                true
            }
            BackgroundMessage::AgentTokenUsage(info) => {
                // Track peak prompt size across session
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
            _ => false,
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
        let mgr = AgentSessionManager::new(config);
        let state = mgr.state();
        assert!(!state.running);
        assert!(state.status.is_empty());
        assert!(state.response.is_empty());
    }

    #[test]
    fn test_cancel_sets_running_false() {
        let config = AppConfig::default();
        let mut mgr = AgentSessionManager::new(config);
        mgr.state_mut().running = true;
        mgr.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
        mgr.cancel();
        assert!(!mgr.state.running);
        assert!(mgr.state.status.contains("Aborted"));
    }

    #[test]
    fn test_clear_history_resets_fields() {
        let config = AppConfig::default();
        let mut mgr = AgentSessionManager::new(config);
        mgr.state.history = Some(vec![Value::String("old".to_string())]);
        mgr.state.token_usage = Some(TokenUsageInfo {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_tokens: None,
            reasoning_tokens: None,
        });
        mgr.clear_history();
        assert!(mgr.state.history.is_none());
        assert!(mgr.state.token_usage.is_none());
    }
}
