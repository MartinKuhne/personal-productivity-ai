//! Agent session manager — lifecycle and UI-visible state for a single LLM agent session (status, response, thinking, history, token usage).
//!
//! Unit tests live in the sibling `manager_tests.rs` sidecar.

use crate::agent::AgentContext;
use crate::agent::events::{AgentEvent as SeamAgentEvent, AgentPrompt};
use crate::app::session::BrowserSession;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::debug::AgentDebugEntry;
use crate::bus::events::messages::TokenUsageInfo;
use crate::config::AppConfig;
use serde_json::Value;
use std::sync::mpsc::{self, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
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
    /// Shared config cell — the UI thread writes via `drain_config` /
    /// `set_config`; the driver thread reads when building each
    /// `AgentContext` (research.md §3, migration step 10).
    shared_config: Arc<std::sync::RwLock<AppConfig>>,
    /// Reader for the configuration-arrival bus. Subscribed during
    /// `new` so the publish that happens after construction is
    /// observed. Drained on every UI frame by
    /// [`Self::drain_config`].
    config_reader: Option<BusReader<ConfigArrived>>,
    /// Tracks whether the bus has delivered the config yet so
    /// config-derived work uses the published `AppConfig` instead
    /// of the default placeholder.
    config_arrived: bool,
    /// Uuid of the currently-active (or most-recently-started) agent
    /// session. Set when `submit_prompt` mints a new `Uuid::new_v4()`.
    /// Replaces the old `session_counter: usize` (migration step 10,
    /// FR-008).
    current_session_id: Option<Uuid>,
    /// Agent→UI structured event bus (new seam path). Cloned into the
    /// agent context on each `start_session`; the UI subscribes via
    /// [`Self::event_bus`] (migration step 2).
    agent_event_bus: Bus<SeamAgentEvent>,
    /// UI→agent prompt channel. The UI calls [`Self::submit_prompt`]
    /// which sends an [`AgentPrompt`] on this sender; the long-lived
    /// driver thread owns the `Receiver` and blocks on `recv()`
    /// (research.md §3, migration step 10).
    prompt_tx: Sender<AgentPrompt>,
    /// Handle to the long-lived driver thread. Spawned once in
    /// [`Self::new`] and joined on drop. The driver processes prompts
    /// sequentially — one session at a time (research.md §3).
    driver_handle: Option<JoinHandle<()>>,
    /// Long-lived headless Firefox session, shared with the
    /// tool executor. Lazily launches a browser on first use.
    /// Owned by the application, not the agent — sessions
    /// survive across agent turns so cookies persist (BRWS-001).
    /// When the `browser` Cargo feature is off the session is
    /// a stub that returns [`crate::app::session::SessionError::Disabled`]
    /// on every page-handle request; the `browser_*` tools are
    /// not registered.
    browser_session: Arc<BrowserSession>,
}

impl AgentSessionManager {
    /// Subscribe to the configuration-arrival bus and return an
    /// empty manager. The bus is the source of truth for the
    pub fn new(
        config_bus: Bus<ConfigArrived>,
        file_event_bus: Bus<crate::bus::events::file::FileEvent>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
        tool_manager: Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
    ) -> Self {
        let agent_event_bus = Bus::new();
        let (prompt_tx, prompt_rx) = mpsc::channel::<AgentPrompt>();
        // Shared config cell — the UI thread updates it via `drain_config` /
        // `set_config`; the driver reads it when building each `AgentContext`.
        let shared_config = Arc::new(std::sync::RwLock::new(AppConfig::default()));
        let driver_handle = spawn_driver(
            prompt_rx,
            agent_event_bus.clone(),
            shared_config.clone(),
            file_event_bus,
            browser_session.clone(),
            pdf_backing.clone(),
            tool_manager.clone(),
        );
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
            shared_config,
            // Subscribe before returning so the publish that
            // happens immediately afterwards (in main / tests) is
            // observed by this reader.
            config_reader: Some(config_bus.subscribe()),
            config_arrived: false,
            current_session_id: None,
            agent_event_bus,
            prompt_tx,
            driver_handle: Some(driver_handle),
            browser_session,
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
        let agent_event_bus = Bus::new();
        let (prompt_tx, prompt_rx) = mpsc::channel::<AgentPrompt>();
        let shared_config = Arc::new(std::sync::RwLock::new(config.clone()));
        let driver_handle = spawn_driver(
            prompt_rx,
            agent_event_bus.clone(),
            shared_config.clone(),
            crate::bus::core::Bus::new(),
            browser_session.clone(),
            Arc::new(crate::app::session::PdfBackingTracker::new()),
            tool_manager.clone(),
        );
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
            shared_config,
            config_reader: None,
            config_arrived: true,
            current_session_id: None,
            agent_event_bus,
            prompt_tx,
            driver_handle: Some(driver_handle),
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
                self.config = event.config.clone();
                *self
                    .shared_config
                    .write()
                    .expect("shared_config lock poisoned") = event.config;
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
        self.config = config.clone();
        *self
            .shared_config
            .write()
            .expect("shared_config lock poisoned") = config;
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

    /// Returns the `Uuid` of the currently-active (or most-recently-started)
    /// agent session. `None` before the first `start_session` call (migration
    /// step 10, FR-008).
    pub fn current_session_id(&self) -> Option<Uuid> {
        self.current_session_id
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
        self.current_session_id = None;
    }

    /// Cancel any running agent session.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.cancel_flag.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
        self.state.running = false;
        self.state.status = "Aborted by user.".to_string();
    }

    /// Submit a prompt to the long-lived driver thread (migration step 10,
    /// research.md §3, FR-008/FR-009). The UI mints a `session_id: Uuid`
    /// for a new session or reuses an existing one for a continuation
    /// prompt. The driver builds a per-session `AgentContext` and runs
    /// `run_agent_inner` inline (no double-spawn).
    ///
    /// This replaces the old `start_session` spawn-per-prompt entry.
    pub fn submit_prompt(&mut self, mut prompt: AgentPrompt) {
        let session_id = prompt.session_id;
        self.current_session_id = Some(session_id);
        self.state.running = true;
        self.state.status = "Initializing agent...".to_string();
        self.state.thinking.clear();
        self.state.response.clear();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());
        prompt.cancel_flag = cancel_flag;
        let _ = self.prompt_tx.send(prompt);
    }
}

/// Spawn the long-lived driver thread (research.md §3, migration step 10).
///
/// Owns the `Receiver<AgentPrompt>` and blocks on `recv()`. On each prompt,
/// builds a per-session `AgentContext` from the prompt + shared resources and
/// runs `run_agent` inline (no double-spawn). The driver processes prompts
/// sequentially — one session at a time.
fn spawn_driver(
    prompt_rx: std::sync::mpsc::Receiver<AgentPrompt>,
    agent_event_bus: Bus<SeamAgentEvent>,
    shared_config: Arc<std::sync::RwLock<AppConfig>>,
    file_event_bus: Bus<crate::bus::events::file::FileEvent>,
    browser_session: Arc<BrowserSession>,
    pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
    tool_manager: Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        // Per-session history cache. Keyed by `session_id` so continuation
        // prompts reuse the same conversation history (FR-009).
        let mut session_histories: std::collections::HashMap<Uuid, Option<Vec<Value>>> =
            std::collections::HashMap::new();
        while let Ok(prompt) = prompt_rx.recv() {
            let session_id = prompt.session_id;
            let config = shared_config.read().map(|c| c.clone()).unwrap_or_default();
            let history = session_histories.get(&session_id).cloned().flatten();
            let ctx = AgentContext {
                config,
                file_event_bus: file_event_bus.clone(),
                agent_event_bus: agent_event_bus.clone(),
                active_file: prompt.active_file,
                active_dir: prompt.active_dir,
                selected_files: prompt.selected_files,
                prompt: prompt.text,
                cancel_flag: prompt.cancel_flag,
                history: history.clone(),
                model_name: None,
                session_id,
                browser_session: browser_session.clone(),
                pdf_backing: pdf_backing.clone(),
                cache: std::sync::Arc::new(crate::agent::tools::manager::cache::ToolCache::new()),
                tool_manager: tool_manager.clone(),
                uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
            };
            crate::agent::run_agent(ctx);
            // After the session finishes, stash its history for continuation
            // prompts (FR-009). The history is updated by the `SessionFinished`
            // event; here we keep the pre-run history — the orchestrator
            // stores the updated history on the UI side via `set_history`.
            session_histories.insert(session_id, history);
        }
    })
}

impl Drop for AgentSessionManager {
    fn drop(&mut self) {
        // Drop the sender first to disconnect the channel, causing the
        // driver's `recv()` to return `Err` and the driver loop to exit.
        // Then join to ensure the thread terminates before shared
        // resources (`browser_session`, `tool_manager`, etc.) are dropped.
        self.prompt_tx = mpsc::channel::<AgentPrompt>().0;
        if let Some(handle) = self.driver_handle.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `manager_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
