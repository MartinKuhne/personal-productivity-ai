//! Agent context — bundles all inputs (config, channels, file bus, active file/dir, prompt, cancel flag, history) for an agent session.

use crate::app::watcher::events::Bus;
use crate::config::AppConfig;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::AtomicBool};

/// Consolidated context for running an agent session.
///
/// This struct replaces the 11 separate parameters previously passed to `run_agent`.
/// It groups related data and reduces the interface to a single argument (PSD-002).
/// Construct with the struct literal `AgentContext { ... }`; the previous
/// `AgentContext::new` was a pass-through forwarder (PSD-004) and was removed.
pub struct AgentContext {
    pub config: AppConfig,
    pub tx_gui: Sender<crate::app::messages::BackgroundMessage>,
    pub file_event_bus: Bus<crate::app::watcher::events::FileEvent>,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub prompt: String,
    pub cancel_flag: Arc<AtomicBool>,
    pub history: Option<Vec<Value>>,
    pub current_response: String,
    /// Optional model name override. When set, `LLMClient::from_config` uses this model
    /// directly instead of selecting the cheapest available model.
    pub model_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::AppConfig;
    use std::path::Path;
    use std::sync::mpsc::channel;

    #[test]
    fn test_agent_context_creation() {
        let (tx, _rx) = channel();
        let config = AppConfig::default();
        let bus = Bus::new();
        let ctx = AgentContext {
            config: config.clone(),
            tx_gui: tx,
            file_event_bus: bus,
            active_file: Some(PathBuf::from("test.md")),
            active_dir: None,
            selected_files: HashSet::new(),
            prompt: "hello".to_string(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            history: None,
            current_response: String::new(),
            model_name: None,
        };
        assert_eq!(ctx.config.models, config.models);
        assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
        assert_eq!(ctx.prompt, "hello");
    }
}
