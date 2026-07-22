//! Agent context — bundles all inputs (config, channels, file bus, active file/dir, prompt, cancel flag, history) for an agent session.

use crate::config::AppConfig;
use crate::file_events::Bus;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{atomic::AtomicBool, Arc};

/// Consolidated context for running an agent session.
///
/// This struct replaces the 11 separate parameters previously passed to `run_agent`.
/// It groups related data and reduces the interface to a single argument (PSD-002).
pub struct AgentContext {
    pub config: AppConfig,
    pub tx_gui: Sender<crate::messages::BackgroundMessage>,
    pub file_event_bus: Bus<crate::file_events::FileEvent>,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub prompt: String,
    pub cancel_flag: Arc<AtomicBool>,
    pub history: Option<Vec<Value>>,
    pub current_response: String,
}

impl AgentContext {
    /// Build a new AgentContext from constituent parts.
    pub fn new(
        config: AppConfig,
        tx_gui: Sender<crate::messages::BackgroundMessage>,
        file_event_bus: Bus<crate::file_events::FileEvent>,
        active_file: Option<PathBuf>,
        active_dir: Option<PathBuf>,
        selected_files: HashSet<PathBuf>,
        prompt: String,
        cancel_flag: Arc<AtomicBool>,
        history: Option<Vec<Value>>,
        current_response: String,
    ) -> Self {
        Self {
            config,
            tx_gui,
            file_event_bus,
            active_file,
            active_dir,
            selected_files,
            prompt,
            cancel_flag,
            history,
            current_response,
        }
    }
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
        let ctx = AgentContext::new(
            config.clone(),
            tx,
            bus,
            Some(PathBuf::from("test.md")),
            None,
            HashSet::new(),
            "hello".to_string(),
            Arc::new(AtomicBool::new(false)),
            None,
            String::new(),
        );
        assert_eq!(ctx.config.models, config.models);
        assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
        assert_eq!(ctx.prompt, "hello");
    }
}
