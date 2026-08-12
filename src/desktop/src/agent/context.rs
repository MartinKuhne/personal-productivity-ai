//! Agent context — bundles all inputs (config, channels, file bus, active file/dir, prompt, cancel flag, history) for an agent session.

use crate::agent::events::AgentEventObserver;
use crate::app::session::BrowserSession;
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::config::AppConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};
use uuid::Uuid;

/// Consolidated context for running an agent session.
///
/// Construct with the struct literal `AgentContext { ... }`; the previous
/// `AgentContext::new` was a pass-through forwarder (PSD-004) and was removed.
pub struct AgentContext {
    pub config: AppConfig,
    pub file_event_bus: Bus<FileEvent>,
    /// Agent→UI structured observer. The agent calls methods here to emit events.
    pub observer: Arc<dyn AgentEventObserver>,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub prompt: String,
    pub cancel_flag: Arc<AtomicBool>,
    pub history: Option<Vec<Value>>,
    /// Optional model name override. When set, `LLMClient::from_config` uses this model
    /// directly instead of selecting the cheapest available model.
    pub model_name: Option<String>,
    /// Uuid identity for the session — tags every `AgentEvent` on the
    /// `Bus<AgentEvent>` path (FR-008).
    pub session_id: Uuid,
    /// Long-lived headless Firefox session shared with every
    /// mutating browser tool call. Owned by the application
    /// (one instance per app) and handed to the agent thread
    /// via this field. When the `browser` Cargo feature is off
    /// the session is a stub; the `browser_*` tools are not
    /// registered and the field stays unused.
    pub browser_session: Arc<BrowserSession>,
    /// Shared PDF-backing tracker — gives tools access to
    /// the set of Markdown files that have a `.pdf` sibling.
    pub pdf_backing: Arc<crate::app::session::PdfBackingTracker>,
    /// Shared tool cache — held as an `Arc` so the executor
    /// constructed from this context can pass it into every
    /// `ToolContext` (which now takes the cache by value).
    /// Today this is the process-wide singleton from
    /// [`crate::agent::tools::registry::cache::cache`], but
    /// the field is independent so a future test or alt
    /// orchestrator can inject a private cache.
    pub cache: Arc<crate::agent::tools::registry::cache::ToolCache>,
    pub tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    pub uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::AppConfig;
    use std::path::Path;

    #[test]
    fn test_agent_context_creation() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let browser = Arc::new(crate::app::session::BrowserSession::new(&config));
        let ctx = AgentContextBuilder::new(config.clone(), Uuid::new_v4(), "hello".to_string())
            .with_buses(bus)
            .with_observer(Arc::new(crate::app::events::BusAgentEventObserver::new(Uuid::new_v4(), crate::bus::core::Bus::new())))
            .with_active_paths(Some(PathBuf::from("test.md")), None)
            .with_browser_session(browser)
            .build();
        assert_eq!(ctx.config.models, config.models);
        assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
        assert_eq!(ctx.prompt, "hello");
    }
}

pub struct AgentContextBuilder {
    config: AppConfig,
    session_id: Uuid,
    prompt: String,
    
    file_event_bus: Option<Bus<FileEvent>>,
    observer: Option<Arc<dyn AgentEventObserver>>,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: HashSet<PathBuf>,
    cancel_flag: Option<Arc<AtomicBool>>,
    history: Option<Vec<Value>>,
    model_name: Option<String>,
    browser_session: Option<Arc<BrowserSession>>,
    pdf_backing: Option<Arc<crate::app::session::PdfBackingTracker>>,
    cache: Option<Arc<crate::agent::tools::registry::cache::ToolCache>>,
    tool_manager: Option<Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>>,
    uuid_gen: Option<Arc<dyn crate::utils::uuid::UuidGenerator>>,
}

impl AgentContextBuilder {
    pub fn new(config: AppConfig, session_id: Uuid, prompt: String) -> Self {
        Self {
            config,
            session_id,
            prompt,
            file_event_bus: None,
            observer: None,
            active_file: None,
            active_dir: None,
            selected_files: HashSet::new(),
            cancel_flag: None,
            history: None,
            model_name: None,
            browser_session: None,
            pdf_backing: None,
            cache: None,
            tool_manager: None,
            uuid_gen: None,
        }
    }

    pub fn with_buses(mut self, file_event_bus: Bus<FileEvent>) -> Self {
        self.file_event_bus = Some(file_event_bus);
        self
    }

    pub fn with_observer(mut self, observer: Arc<dyn AgentEventObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub fn with_active_paths(mut self, file: Option<PathBuf>, dir: Option<PathBuf>) -> Self {
        self.active_file = file;
        self.active_dir = dir;
        self
    }

    pub fn with_selected_files(mut self, selected_files: HashSet<PathBuf>) -> Self {
        self.selected_files = selected_files;
        self
    }

    pub fn with_cancel_flag(mut self, cancel_flag: Arc<AtomicBool>) -> Self {
        self.cancel_flag = Some(cancel_flag);
        self
    }

    pub fn with_history(mut self, history: Option<Vec<Value>>) -> Self {
        self.history = history;
        self
    }

    pub fn with_model_name(mut self, model_name: Option<String>) -> Self {
        self.model_name = model_name;
        self
    }

    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.browser_session = Some(browser_session);
        self
    }

    pub fn with_pdf_backing(mut self, pdf_backing: Arc<crate::app::session::PdfBackingTracker>) -> Self {
        self.pdf_backing = Some(pdf_backing);
        self
    }

    pub fn with_cache(mut self, cache: Arc<crate::agent::tools::registry::cache::ToolCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_tool_manager(mut self, tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>) -> Self {
        self.tool_manager = Some(tool_manager);
        self
    }

    pub fn with_uuid_gen(mut self, uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator>) -> Self {
        self.uuid_gen = Some(uuid_gen);
        self
    }

    pub fn build(self) -> AgentContext {
        AgentContext {
            config: self.config,
            session_id: self.session_id,
            prompt: self.prompt,
            file_event_bus: self.file_event_bus.unwrap_or_else(Bus::new),
            observer: self.observer.expect("observer is required"),
            active_file: self.active_file,
            active_dir: self.active_dir,
            selected_files: self.selected_files,
            cancel_flag: self.cancel_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
            history: self.history,
            model_name: self.model_name,
            browser_session: self.browser_session.unwrap_or_else(|| Arc::new(BrowserSession::new(&crate::config::AppConfig::default()))),
            pdf_backing: self.pdf_backing.unwrap_or_else(|| Arc::new(crate::app::session::PdfBackingTracker::new())),
            cache: self.cache.unwrap_or_else(|| Arc::new(crate::agent::tools::registry::cache::ToolCache::new())),
            tool_manager: self.tool_manager.unwrap_or_else(|| Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new()))),
            uuid_gen: self.uuid_gen.unwrap_or_else(|| Arc::new(crate::utils::uuid::SystemUuidGenerator)),
        }
    }
}
