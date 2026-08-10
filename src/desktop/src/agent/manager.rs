//! Agent session manager — lifecycle and UI-visible state for a single LLM agent session (status, response, thinking, history, token usage).
//!
//! Unit tests live in the sibling `manager_tests.rs` sidecar.

use crate::agent::AgentContext;
use crate::agent::events::AgentEvent as SeamAgentEvent;
use crate::app::session::BrowserSession;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::debug::AgentDebugEntry;
use crate::bus::events::messages::TokenUsageInfo;
use crate::config::AppConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

/// Agent state exposed to UI components.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub running: bool,
    pub status: String,
    pub thinking: String,
    pub response: String,
    pub history: Option<Vec<Value>>,
    pub token_usage: Option<TokenUsageInfo>,
    pub total_usage: TokenUsageInfo,
    /// Queue of prompts submitted while the agent is running.
    /// These will be processed sequentially after the current prompt finishes.
    pub pending_prompts: Vec<String>,
    /// Accumulated debug entries from all agent sessions. Never cleared;
    /// session-boundary rows delimit sessions.
    pub debug_entries: Vec<AgentDebugEntry>,
}

/// Manages the lifecycle and state of a single LLM agent session.
///
/// Responsibilities:
/// - Owns agent state (status, thinking, response, history, token usage)
/// - Subscribes to the [`crate::bus::events::config::ConfigArrived`] bus so the
///   [`AppConfig`] used by `start_session` is the one published at
///   startup, not a value captured at construction time.
/// - Provides `start_session` to launch a new agent thread
/// - Exposes read-only `AgentState` for UI rendering
/// - Exposes `event_bus()` for UI to subscribe to `Bus<AgentEvent>`
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
    session_counter: usize,
    /// Agent→UI structured event bus (new seam path). Cloned into the
    /// agent context on each `start_session`; the UI subscribes via
    /// [`Self::event_bus`] (migration step 2).
    agent_event_bus: Bus<SeamAgentEvent>,
    /// Long-lived headless Firefox session, shared with the
    /// tool executor. Lazily launches a browser on first use.
    /// Owned by the application, not the agent — sessions
    /// survive across agent turns so cookies persist (BRWS-001).
    /// When the `browser` Cargo feature is off the session is
    /// a stub that returns [`crate::app::session::SessionError::Disabled`]
    /// on every page-handle request; the `browser_*` tools are
    /// not registered.
    browser_session: Arc<BrowserSession>,
    pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
    tool_manager: Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
}

impl AgentSessionManager {
    /// Subscribe to the configuration-arrival bus and return an
    /// empty manager. The bus is the source of truth for the
    pub fn new(
        config_bus: Bus<ConfigArrived>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
        tool_manager: Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
    ) -> Self {
        Self {
            state: AgentState {
                running: false,
                status: String::new(),
                thinking: String::new(),
                response: String::new(),
                history: None,
                token_usage: None,
                total_usage: TokenUsageInfo::default(),
                pending_prompts: Vec::new(),
                debug_entries: Vec::new(),
            },
            cancel_flag: None,
            config: AppConfig::default(),
            // Subscribe before returning so the publish that
            // happens immediately afterwards (in main / tests) is
            // observed by this reader.
            config_reader: Some(config_bus.subscribe()),
            config_arrived: false,
            session_counter: 0,
            agent_event_bus: Bus::new(),
            browser_session,
            pdf_backing,
            tool_manager,
        }
    }

    /// Test helper: build a manager whose [`AppConfig`] is set
    /// immediately, without going through the bus. Mirrors the old
    /// `new(config)` signature for callers that just want a
    /// populated manager (existing test fixtures).
    #[doc(hidden)]
    pub fn new_for_test(config: AppConfig, browser_session: Arc<BrowserSession>) -> Self {
        let tool_manager = Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        ));
        Self {
            state: AgentState {
                running: false,
                status: String::new(),
                thinking: String::new(),
                response: String::new(),
                history: None,
                token_usage: None,
                total_usage: TokenUsageInfo::default(),
                pending_prompts: Vec::new(),
                debug_entries: Vec::new(),
            },
            cancel_flag: None,
            config,
            config_reader: None,
            config_arrived: true,
            session_counter: 0,
            agent_event_bus: Bus::new(),
            browser_session,
            pdf_backing: Arc::new(crate::app::session::PdfBackingTracker::new()),
            tool_manager,
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

    /// Return a clone of the agent→UI event bus so the UI can subscribe
    /// via `BusReader<AgentEvent>` and drain structured agent events each
    /// frame (migration step 2, FR-010).
    pub fn event_bus(&self) -> Bus<SeamAgentEvent> {
        self.agent_event_bus.clone()
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

    /// Set the running flag.
    pub fn set_running(&mut self, running: bool) {
        self.state.running = running;
    }

    /// Set the conversation history.
    pub fn set_history(&mut self, history: Option<Vec<Value>>) {
        self.state.history = history;
    }

    /// Queue a prompt to be processed after the current one finishes.
    pub fn queue_prompt(&mut self, prompt: String) {
        self.state.pending_prompts.push(prompt);
    }

    /// Take the next queued prompt, if any.
    pub fn take_next_queued_prompt(&mut self) -> Option<String> {
        if self.state.pending_prompts.is_empty() {
            None
        } else {
            Some(self.state.pending_prompts.remove(0))
        }
    }

    /// Get the number of queued prompts.
    pub fn queued_prompt_count(&self) -> usize {
        self.state.pending_prompts.len()
    }

    /// Clear all queued prompts (used on session failure to avoid
    /// cascading errors).
    pub fn clear_queued_prompts(&mut self) {
        self.state.pending_prompts.clear();
    }

    /// Apply a `TokenUsageInfo` update to the accumulative totals and
    /// last-turn snapshot. Called by the orchestrator's
    /// `drain_agent_event_bus` when a `TokenUsage` event arrives.
    pub fn apply_token_usage(&mut self, info: TokenUsageInfo) {
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
    }

    /// Push a debug entry to the accumulated list. Called by the
    /// orchestrator's `drain_agent_event_bus` when a `DebugEntry` event arrives.
    pub fn push_debug_entry(&mut self, entry: crate::bus::events::debug::AgentDebugEntry) {
        self.state.debug_entries.push(entry);
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
    /// The agent publishes structured `AgentEvent`s on the `Bus<AgentEvent>`
    /// owned by this manager; the UI subscribes via [`Self::event_bus`].
    pub fn start_session(
        &mut self,
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
        self.session_counter += 1;
        let session_id = Uuid::new_v4();

        // Build context
        let ctx = AgentContext {
            config: self.config.clone(),
            file_event_bus,
            agent_event_bus: self.agent_event_bus.clone(),
            active_file,
            active_dir,
            selected_files,
            prompt: prompt.clone(),
            cancel_flag,
            history: self.state.history.clone(),
            model_name: None,
            session_id,
            browser_session: self.browser_session.clone(),
            pdf_backing: self.pdf_backing.clone(),
            tool_manager: self.tool_manager.clone(),
            uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
        };

        std::thread::spawn(move || {
            crate::agent::run_agent(ctx);
        });
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `manager_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
