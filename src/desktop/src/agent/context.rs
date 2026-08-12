//! Agent context — bundles all inputs (config, channels, file bus, active file/dir, prompt, cancel flag, history) for an agent session.

use crate::agent::events::AgentEvent;
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
    /// Agent→UI structured event bus. The agent publishes structured
    /// `AgentEvent`s here; the UI subscribes via `BusReader`.
    pub agent_event_bus: Bus<AgentEvent>,
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
    pub tool_manager: Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,
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
        let ctx = AgentContext {
            config: config.clone(),
            file_event_bus: bus,
            agent_event_bus: Bus::new(),
            active_file: Some(PathBuf::from("test.md")),
            active_dir: None,
            selected_files: HashSet::new(),
            prompt: "hello".to_string(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            history: None,
            model_name: None,
            session_id: Uuid::new_v4(),
            browser_session: browser,
            pdf_backing: Arc::new(crate::app::session::PdfBackingTracker::new()),
            cache: Arc::new(crate::agent::tools::registry::cache::ToolCache::new()),
            tool_manager: Arc::new(std::sync::RwLock::new(
                crate::agent::tools::registry::ToolRegistry::new(),
            )),
            uuid_gen: Arc::new(crate::utils::uuid::SystemUuidGenerator),
        };
        assert_eq!(ctx.config.models, config.models);
        assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
        assert_eq!(ctx.prompt, "hello");
    }
}
