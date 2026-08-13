//! Agent context — bundles all inputs (config, channels, file bus, active file/dir, prompt, cancel flag, history) for an agent session.

use crate::agent::config::AgentConfig;
use crate::agent::events::AgentEventObserver;
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
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
    /// Domain-specific configuration for the agent's run loop (LLM,
    /// system prompt, tool groups, MCP, browser, content libraries).
    /// Projected from the global `AppConfig` by the orchestrator via
    /// `AppConfig::to_agent_config`.
    pub agent_config: AgentConfig,
    pub file_observer: std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged>,
    /// Agent→UI structured observer. The agent calls methods here to emit events.
    pub observer: Arc<dyn AgentEventObserver>,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub prompt: String,
    /// Pre-assembled system-prompt message blocks, built by the
    /// submitter via [`crate::app::prompts::build_system_prompts`].
    /// The agent run loop forwards these as `role=system` messages
    /// before the user turn; it does not construct them itself.
    pub system_prompts: Vec<String>,
    pub cancel_flag: Arc<AtomicBool>,
    pub history: Option<Vec<Value>>,
    /// Optional model name override. When set, `LLMClient::from_agent_config` uses this model
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
    pub tool_call_policy: std::sync::Arc<dyn crate::agent::tools::policy::ToolCallPolicy>,
    /// Shared PDF-backing tracker — gives tools access to
    /// the set of Markdown files that have a `.pdf` sibling.
    /// Shared tool cache — held as an `Arc` so the executor
    /// constructed from this context can pass it into every
    /// `ToolContext` (which now takes the cache by value).
    /// Today this is the process-wide singleton from
    /// [`crate::agent::tools::registry::cache::cache`], but
    /// the field is independent so a future test or alt
    /// orchestrator can inject a private cache.
    pub cache: Arc<crate::agent::tools::registry::cache::ToolCache>,
    /// Catalog-level bundle. The agent loop, executor, and prompt
    /// builder all snapshot this per turn. Swapped atomically on
    /// `ConfigArrived` and MCP discovery. See
    /// [`crate::agent::AgentToolContext`].
    pub tool_context: std::sync::Arc<arc_swap::ArcSwap<crate::agent::AgentToolContext>>,
    pub uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator>,
    pub extensions: crate::agent::tools::extensions::Extensions,
}

pub struct AgentContextBuilder {
    agent_config: AgentConfig,
    session_id: Uuid,
    prompt: String,

    file_observer: Option<std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged>>,
    observer: Option<Arc<dyn AgentEventObserver>>,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: HashSet<PathBuf>,
    cancel_flag: Option<Arc<AtomicBool>>,
    history: Option<Vec<Value>>,
    model_name: Option<String>,
    system_prompts: Option<Vec<String>>,
    tool_call_policy: Option<std::sync::Arc<dyn crate::agent::tools::policy::ToolCallPolicy>>,
    cache: Option<Arc<crate::agent::tools::registry::cache::ToolCache>>,
    tool_context: Option<Arc<arc_swap::ArcSwap<crate::agent::AgentToolContext>>>,
    uuid_gen: Option<Arc<dyn crate::utils::uuid::UuidGenerator>>,
    extensions: crate::agent::tools::extensions::Extensions,
}

impl AgentContextBuilder {
    pub fn new(agent_config: AgentConfig, session_id: Uuid, prompt: String) -> Self {
        Self {
            agent_config,
            session_id,
            prompt,
            file_observer: None,
            observer: None,
            active_file: None,
            active_dir: None,
            selected_files: HashSet::new(),
            cancel_flag: None,
            history: None,
            system_prompts: None,
            model_name: None,
            tool_call_policy: None,
            cache: None,
            tool_context: None,
            uuid_gen: None,
            extensions: crate::agent::tools::extensions::Extensions::new(),
        }
    }

    pub fn with_file_observer(mut self, file_observer: std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged>) -> Self {
        self.file_observer = Some(file_observer);
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

    /// Set the pre-built system-prompt blocks. The submitter
    /// (UI submit path or batch executor) calls
    /// [`crate::app::prompts::build_system_prompts`] and hands the
    /// result here. Required at [`Self::build`] time.
    pub fn with_system_prompts(mut self, system_prompts: Vec<String>) -> Self {
        self.system_prompts = Some(system_prompts);
        self
    }

    pub fn with_tool_call_policy(mut self, policy: std::sync::Arc<dyn crate::agent::tools::policy::ToolCallPolicy>) -> Self {
        self.tool_call_policy = Some(policy);
        self
    }

    pub fn with_cache(
        mut self,
        cache: Arc<crate::agent::tools::registry::cache::ToolCache>,
    ) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_tool_context(
        mut self,
        tool_context: Arc<arc_swap::ArcSwap<crate::agent::AgentToolContext>>,
    ) -> Self {
        self.tool_context = Some(tool_context);
        self
    }

    pub fn with_uuid_gen(mut self, uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator>) -> Self {
        self.uuid_gen = Some(uuid_gen);
        self
    }

    pub fn with_extension<T: Send + Sync + 'static>(mut self, extension: Arc<T>) -> Self {
        self.extensions.insert(extension);
        self
    }

    pub fn with_extensions(mut self, extensions: crate::agent::tools::extensions::Extensions) -> Self {
        self.extensions = extensions;
        self
    }

    pub fn build(self) -> AgentContext {
        AgentContext {
            agent_config: self.agent_config,
            session_id: self.session_id,
            prompt: self.prompt,
            file_observer: self.file_observer.unwrap_or_else(|| std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver)),
            observer: self.observer.expect("observer is required"),
            active_file: self.active_file,
            active_dir: self.active_dir,
            tool_call_policy: self.tool_call_policy.unwrap_or_else(|| std::sync::Arc::new(crate::agent::tools::policy::DefaultToolCallPolicy)),
            selected_files: self.selected_files,
            system_prompts: self
                .system_prompts
                .expect("system_prompts is required (use with_system_prompts)"),
            cancel_flag: self
                .cancel_flag
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
            history: self.history,
            model_name: self.model_name,
            cache: self.cache.unwrap_or_else(|| {
                Arc::new(crate::agent::tools::registry::cache::ToolCache::new())
            }),
            tool_context: self.tool_context.unwrap_or_else(|| {
                Arc::new(arc_swap::ArcSwap::from_pointee(
                    crate::agent::AgentToolContext::new(
                        crate::agent::tools::registry::ToolRegistry::new(),
                    ),
                ))
            }),
            uuid_gen: self
                .uuid_gen
                .unwrap_or_else(|| Arc::new(crate::utils::uuid::SystemUuidGenerator)),
            extensions: self.extensions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    #[test]
    fn test_agent_context_creation() {
        let agent_config = AgentConfig::default();
        
                let ctx =
            AgentContextBuilder::new(agent_config.clone(), Uuid::new_v4(), "hello".to_string())
                .with_file_observer(std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver))
                .with_observer(Arc::new(crate::app::events::BusAgentEventObserver::new(
                    Uuid::new_v4(),
                    crate::bus::core::Bus::new(),
                )))
                .with_active_paths(Some(PathBuf::from("test.md")), None)
                .with_system_prompts(Vec::new())
                
                .build();
        assert_eq!(ctx.agent_config.models(), agent_config.models());
        assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
        assert_eq!(ctx.prompt, "hello");
    }
}
