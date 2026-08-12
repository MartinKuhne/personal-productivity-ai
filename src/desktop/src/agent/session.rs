//! Agent session manager — lifecycle and UI-visible state for a single LLM agent session (status, response, thinking, history, token usage).
//!
//! Unit tests live in the sibling `session_tests.rs` sidecar.

use crate::agent::config::AgentConfig;
use crate::agent::events::AgentPrompt;
use crate::app::events::AgentEvent as SeamAgentEvent;
use crate::app::session::BrowserSession;
use crate::bus::core::Bus;
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
/// - Holds the agent's domain [`AgentConfig`] (no direct
///   `Bus<ConfigArrived>` subscription — the orchestrator hands us
///   the projected config via [`Self::set_agent_config`]).
/// - Provides `submit_prompt` to enqueue work for the long-lived
///   driver thread.
/// - Exposes read-only `AgentState` for UI rendering
/// - Exposes `event_bus()` for UI to subscribe to `Bus<AgentEvent>`
pub struct AgentSession {
    state: AgentState,
    cancel_flag: Option<Arc<AtomicBool>>,
    /// Shared cell holding the agent's domain config. The UI thread
    /// writes via [`Self::set_agent_config`]; the driver thread reads
    /// it (and projects to the global `AppConfig` for the tool
    /// context) when building each `AgentContext` (research.md §3,
    /// migration step 10).
    agent_config_provider: Arc<std::sync::RwLock<AgentConfig>>,
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
    /// Long-lived headless browser session, shared with the
    /// tool executor. Lazily launches a browser on first use.
    /// Owned by the application, not the agent — sessions
    /// survive across agent turns so cookies persist (BRWS-001).
    /// When the `browser` Cargo feature is off the session is
    /// a stub that returns [`crate::app::session::SessionError::Disabled`]
    /// on every page-handle request; the `browser_*` tools are
    /// not registered.
    browser_session: Arc<BrowserSession>,
}

impl AgentSession {
    /// Create a new empty [`AgentSessionBuilder`]. The builder is
    /// the recommended way to construct a session; the legacy
    /// `AgentSession::new` constructor is kept for the
    /// orchestrator's hot path.
    pub fn builder() -> AgentSessionBuilder {
        AgentSessionBuilder::new()
    }

    /// Build a session manager with a shared cell for the agent's
    /// domain config. The orchestrator writes the cell via
    /// [`Self::set_agent_config`]; the driver reads it per session.
    pub fn new(
        file_event_bus: Bus<crate::bus::events::file::FileEvent>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
        tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    ) -> Self {
        Self::builder()
            .with_file_event_bus(file_event_bus)
            .with_browser_session(browser_session)
            .with_pdf_backing(pdf_backing)
            .with_tool_manager(tool_manager)
            .build()
    }

    /// Construct an `AgentSession` with an initial domain config.
    /// The session holds a private cell containing the config; updates
    /// arrive via [`Self::set_agent_config`].
    pub fn new_with_agent_config(
        initial_config: AgentConfig,
        file_event_bus: Bus<crate::bus::events::file::FileEvent>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
        tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    ) -> Self {
        Self::builder()
            .with_agent_config(initial_config)
            .with_file_event_bus(file_event_bus)
            .with_browser_session(browser_session)
            .with_pdf_backing(pdf_backing)
            .with_tool_manager(tool_manager)
            .build()
    }

    /// Snapshot of the current agent config.
    pub fn agent_config(&self) -> AgentConfig {
        self.agent_config_provider
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// Replace the stored agent config. Subsequent sessions use the
    /// new value; in-flight sessions finish with the value they were
    /// built with.
    pub fn set_agent_config(&self, agent_config: AgentConfig) {
        *self
            .agent_config_provider
            .write()
            .expect("agent_config lock poisoned") = agent_config;
    }

    /// Apply a transformation to the stored config.
    pub fn replace_agent_config(&self, f: impl FnOnce(&AgentConfig) -> AgentConfig) {
        let mut guard = self
            .agent_config_provider
            .write()
            .expect("agent_config lock poisoned");
        *guard = f(&guard);
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

/// Builder for [`AgentSession`].
///
/// Replaces the previous multi-arg `new` and the bus-driven init path.
/// The bus-driven path is gone — the agent receives its domain config
/// via [`Self::with_agent_config`] or a shared cell via
/// [`Self::with_agent_config_provider`]. The orchestrator (which already
/// drains `Bus<ConfigArrived>`) is the projection site.
pub struct AgentSessionBuilder {
    file_event_bus: Option<Bus<crate::bus::events::file::FileEvent>>,
    browser_session: Option<Arc<BrowserSession>>,
    pdf_backing: Option<Arc<crate::app::session::PdfBackingTracker>>,
    tool_manager: Option<Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>>,
    initial_agent_config: Option<AgentConfig>,
    agent_config_provider: Option<Arc<std::sync::RwLock<AgentConfig>>>,
}

impl AgentSessionBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            file_event_bus: None,
            browser_session: None,
            pdf_backing: None,
            tool_manager: None,
            initial_agent_config: None,
            agent_config_provider: None,
        }
    }

    /// Set the file event bus.
    pub fn with_file_event_bus(mut self, bus: Bus<crate::bus::events::file::FileEvent>) -> Self {
        self.file_event_bus = Some(bus);
        self
    }

    /// Set the long-lived headless browser session.
    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.browser_session = Some(browser_session);
        self
    }

    /// Set the shared PDF-backing tracker.
    pub fn with_pdf_backing(
        mut self,
        pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
    ) -> Self {
        self.pdf_backing = Some(pdf_backing);
        self
    }

    /// Set the tool registry.
    pub fn with_tool_manager(
        mut self,
        tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    ) -> Self {
        self.tool_manager = Some(tool_manager);
        self
    }

    /// Seed the agent config cell with an initial value.
    pub fn with_agent_config(mut self, cfg: AgentConfig) -> Self {
        self.initial_agent_config = Some(cfg);
        self
    }

    /// Hand the agent config cell directly. Use this when the UI thread
    /// already owns a cell and will push updates to it.
    pub fn with_agent_config_provider(
        mut self,
        provider: Arc<std::sync::RwLock<AgentConfig>>,
    ) -> Self {
        self.agent_config_provider = Some(provider);
        self
    }

    /// Build the [`AgentSession`]. Panics if any required field is missing.
    pub fn build(self) -> AgentSession {
        let file_event_bus = self
            .file_event_bus
            .expect("AgentSession requires with_file_event_bus");
        let browser_session = self
            .browser_session
            .expect("AgentSession requires with_browser_session");
        let pdf_backing = self
            .pdf_backing
            .expect("AgentSession requires with_pdf_backing");
        let tool_manager = self
            .tool_manager
            .expect("AgentSession requires with_tool_manager");
        let agent_config_provider = self.agent_config_provider.unwrap_or_else(|| {
            Arc::new(std::sync::RwLock::new(
                self.initial_agent_config.unwrap_or_default(),
            ))
        });
        let agent_event_bus = Bus::new();
        let (prompt_tx, prompt_rx) = mpsc::channel::<AgentPrompt>();
        let driver_handle = spawn_driver(
            prompt_rx,
            agent_event_bus.clone(),
            agent_config_provider.clone(),
            file_event_bus,
            browser_session.clone(),
            pdf_backing,
            tool_manager,
        );
        AgentSession {
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
            agent_config_provider,
            current_session_id: None,
            agent_event_bus,
            prompt_tx,
            driver_handle: Some(driver_handle),
            browser_session,
        }
    }
}

impl Default for AgentSessionBuilder {
    fn default() -> Self {
        Self::new()
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
    agent_config_provider: Arc<std::sync::RwLock<AgentConfig>>,
    file_event_bus: Bus<crate::bus::events::file::FileEvent>,
    browser_session: Arc<BrowserSession>,
    pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
    tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        // Per-session history cache. Keyed by `session_id` so continuation
        // prompts reuse the same conversation history (FR-009).
        let mut session_histories: std::collections::HashMap<Uuid, Option<Vec<Value>>> =
            std::collections::HashMap::new();
        while let Ok(prompt) = prompt_rx.recv() {
            let session_id = prompt.session_id;
            let agent_config = agent_config_provider
                .read()
                .map(|c| c.clone())
                .unwrap_or_default();
            // The tool context still needs the global config for the
            // integration-layer tools (JMAP / CalDAV / Trello / SearXNG).
            // We project back via the same path: derive a default
            // `AppConfig`, then layer the agent-relevant fields from
            // `agent_config`. The integration tools only read the maps
            // they need, so the layered value is good enough for them.
            let app_config = Arc::new(integration_app_config(&agent_config));
            let history = session_histories.get(&session_id).cloned().flatten();
            let ctx = crate::agent::context::AgentContextBuilder::new(
                agent_config,
                session_id,
                prompt.text,
            )
            .with_app_config(app_config)
            .with_buses(file_event_bus.clone())
            .with_observer(std::sync::Arc::new(
                crate::app::events::BusAgentEventObserver::new(session_id, agent_event_bus.clone()),
            ))
            .with_active_paths(prompt.active_file, prompt.active_dir)
            .with_selected_files(prompt.selected_files)
            .with_system_prompts(prompt.system_prompts)
            .with_cancel_flag(prompt.cancel_flag)
            .with_history(history.clone())
            .with_browser_session(browser_session.clone())
            .with_pdf_backing(pdf_backing.clone())
            .with_cache(std::sync::Arc::new(
                crate::agent::tools::registry::cache::ToolCache::new(),
            ))
            .with_tool_manager(tool_manager.clone())
            .with_uuid_gen(std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator))
            .build();
            crate::agent::run_agent(ctx);
            // After the session finishes, stash its history for continuation
            // prompts (FR-009). The history is updated by the `SessionFinished`
            // event; here we keep the pre-run history — the orchestrator
            // stores the updated history on the UI side via `set_history`.
            session_histories.insert(session_id, history);
        }
    })
}

/// Derive an `AppConfig` from an `AgentConfig` for the integration-layer
/// tools. The tool context carries `Arc<AppConfig>` because the
/// integration functions still take that type; this helper projects
/// the relevant maps back. The orchestrator's path is the
/// `AgentConfig::from_app_config` mirror — fields the agent doesn't
/// expose (`inline_editor_enabled`, `pdf_converter_command`,
/// `table_width_strategy`, `discord`) are filled with their
/// `AppConfig::default` values.
///
/// In production the orchestrator passes the real `AppConfig` via
/// `AgentContextBuilder::with_app_config`; this helper is the
/// fallback path used by `AgentSession::spawn_driver` when the
/// driver is constructed without an external `AppConfig` (i.e. the
/// `new_with_agent_config` test path).
fn integration_app_config(agent_config: &AgentConfig) -> AppConfig {
    AppConfig {
        models: agent_config.models().clone(),
        max_tokens: agent_config.max_tokens(),
        tool_groups: agent_config.tool_groups().clone(),
        mcp_servers: agent_config.mcp_servers().clone(),
        csv_db_path: agent_config.csv_db_path().map(String::from),
        feature_flags: agent_config.feature_flags().clone(),
        jmap_clients: agent_config.jmap_clients().clone(),
        caldav_clients: agent_config.caldav_clients().clone(),
        trello_client: agent_config.trello_client().cloned(),
        searxng_url: agent_config.searxng_url().map(String::from),
        ..AppConfig::default()
    }
}

impl Drop for AgentSession {
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
// Tests live in the sibling `session_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
