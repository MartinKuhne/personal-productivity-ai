//! Domain-specific configuration for the agent module.
//!
//! The agent module does not need (and should not depend on) the entire
//! [`crate::config::AppConfig`]. [`AgentConfig`] is the projected slice that
//! the orchestrator hands to the agent: LLM models, tool groups, MCP servers,
//! integration clients that the tools read at execute time (JMAP, CalDAV,
//! CardDAV, Trello, SearXNG), runtime feature flags, the CSV database path,
//! the resolved browser config, and the config path for the "API key not
//! set" error.
//!
//! User identity (`user_name`, `user_address`, `user_birthdate`,
//! `user_gender`), the system-prompt extension, and the content-library
//! list are intentionally **not** in [`AgentConfig`]. They drive
//! system-prompt construction, which lives outside the agent module
//! (see [`crate::app::prompts::build_system_prompts`]). The agent only
//! consumes the pre-built prompts.
//!
//! Construction:
//! - [`AgentConfig::from_app_config`] — projection from the global config
//!   (the canonical orchestrator-side constructor).
//! - [`AgentConfigBuilder`] — fluent builder for tests and per-session
//!   overrides.
//!
//! The projection is the only place that knows the difference between
//! "configured" (`BrowserConfig` with empty strings) and "resolved"
//! ([`crate::config::ResolvedBrowserConfig`] with concrete paths). Inside
//! the agent module, only the resolved form is used.
//!
//! Unit tests live in the sibling `config_tests.rs` sidecar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{
    CalDavClient, ContentLibrary, JmapClient, LlmConfig, McpServerEntry, ResolvedBrowserConfig,
    ToolGroupsConfig, TrelloClient, get_config_path,
};

/// Domain-specific configuration for the agent module.
///
/// Built via [`crate::config::AppConfig::to_agent_config`] (orchestrator seam) or
/// [`AgentConfigBuilder`] (tests / per-session overrides). Every field is
/// the slice the agent actually consumes; fields the agent doesn't read
/// (user identity, system-prompt extension,
/// `inline_editor_enabled`, `pdf_converter_command`, `table_width_strategy`,
/// `discord`) are intentionally not present.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub(crate) models: HashMap<String, LlmConfig>,
    pub(crate) max_tokens: u32,
    pub(crate) tool_groups: ToolGroupsConfig,
    pub(crate) mcp_servers: HashMap<String, McpServerEntry>,
    pub(crate) browser: ResolvedBrowserConfig,
    pub(crate) config_path: PathBuf,
    // Tool execution reads these; the agent loop doesn't, but the
    // `is_enabled` check on the registered tools does. Cloning them
    // here keeps `ToolContext` self-contained — the executor's parallel
    // workers don't need to chase a back-pointer to the global config.
    pub(crate) jmap_clients: HashMap<String, JmapClient>,
    pub(crate) caldav_clients: HashMap<String, CalDavClient>,
    pub(crate) trello_client: Option<TrelloClient>,
    pub(crate) searxng_url: Option<String>,
    pub(crate) csv_db_path: Option<String>,
    pub(crate) feature_flags: HashMap<String, bool>,
    pub(crate) content_libraries: Vec<ContentLibrary>,
}

impl AgentConfig {

    /// Map of configured LLM models by name.
    pub fn models(&self) -> &HashMap<String, LlmConfig> {
        &self.models
    }

    /// Maximum tokens for LLM responses.
    pub fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    /// Per-tool-group enable flags.
    pub fn tool_groups(&self) -> &ToolGroupsConfig {
        &self.tool_groups
    }

    /// Configured external MCP servers by name.
    pub fn mcp_servers(&self) -> &HashMap<String, McpServerEntry> {
        &self.mcp_servers
    }

    /// Resolved browser configuration (paths filled in by the projection).
    pub fn browser(&self) -> &ResolvedBrowserConfig {
        &self.browser
    }

    /// Path to the on-disk config file. Used for the "API key not set"
    /// error message so the user knows where to fix it.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Configured JMAP email clients, keyed by name.
    pub fn jmap_clients(&self) -> &HashMap<String, JmapClient> {
        &self.jmap_clients
    }

    /// Configured CalDAV clients, keyed by name.
    pub fn caldav_clients(&self) -> &HashMap<String, CalDavClient> {
        &self.caldav_clients
    }

    /// Configured Trello client, if any.
    pub fn trello_client(&self) -> Option<&TrelloClient> {
        self.trello_client.as_ref()
    }

    /// URL of the local SearXNG instance, if configured.
    pub fn searxng_url(&self) -> Option<&str> {
        self.searxng_url.as_deref()
    }

    /// Override path for the CSV database directory.
    pub fn csv_db_path(&self) -> Option<&str> {
        self.csv_db_path.as_deref()
    }

    /// Runtime feature flags. Today the agent reads `toolCallDebugMode`
    /// (for verbose tool-call logging) and `useDAVForContacts` (CardDAV
    /// fallback in the contact tools).
    pub fn feature_flags(&self) -> &HashMap<String, bool> {
        &self.feature_flags
    }

    /// Configured content libraries for VFS path resolution.
    pub fn content_libraries(&self) -> &[ContentLibrary] {
        &self.content_libraries
    }

    /// Find the best model for a given use_case (lowest cost among matches).
    pub fn model_for_use_case(&self, use_case: impl AsRef<str>) -> Option<(&String, &crate::config::LlmConfig)> {
        let uc_ref = use_case.as_ref();
        self.models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(uc_ref))
            .min_by_key(|(_, cfg)| cfg.get_cost())
    }

    /// Select a chat model, preferring one configured for the "chat" use case,
    /// falling back to the first available model, and rejecting default/empty keys.
    pub fn select_chat_model(&self) -> Result<&crate::config::LlmConfig, String> {
        let model_cfg = if let Some((_key, model_cfg)) = self.model_for_use_case("chat") {
            model_cfg
        } else if let Some(model_cfg) = self.models.values().next() {
            model_cfg
        } else {
            return Err("No LLM models are configured.".to_string());
        };

        if model_cfg.api_key == "your-api-key-here" || model_cfg.api_key.is_empty() {
            return Err("API key not set or invalid.".to_string());
        }

        Ok(model_cfg)
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfigBuilder::new().build()
    }
}

/// Fluent builder for [`AgentConfig`].
///
/// Used for tests and per-session overrides that want to assemble a domain
/// config without going through the global config.
pub struct AgentConfigBuilder {
    models: HashMap<String, LlmConfig>,
    max_tokens: u32,
    tool_groups: ToolGroupsConfig,
    mcp_servers: HashMap<String, McpServerEntry>,
    browser: ResolvedBrowserConfig,
    config_path: PathBuf,
    jmap_clients: HashMap<String, JmapClient>,
    caldav_clients: HashMap<String, CalDavClient>,
    trello_client: Option<TrelloClient>,
    searxng_url: Option<String>,
    csv_db_path: Option<String>,
    feature_flags: HashMap<String, bool>,
    content_libraries: Vec<ContentLibrary>,
}

impl AgentConfigBuilder {
    /// Create a builder populated with defaults.
    pub fn new() -> Self {
        let browser = crate::config::BrowserConfig::default().resolve(&[]);
        Self {
            models: HashMap::new(),
            max_tokens: 32768,
            tool_groups: ToolGroupsConfig::default(),
            mcp_servers: HashMap::new(),
            browser,
            config_path: get_config_path(),
            jmap_clients: HashMap::new(),
            caldav_clients: HashMap::new(),
            trello_client: None,
            searxng_url: None,
            csv_db_path: None,
            feature_flags: HashMap::new(),
            content_libraries: Vec::new(),
        }
    }

    /// Set the LLM model map.
    pub fn with_models(mut self, models: HashMap<String, LlmConfig>) -> Self {
        self.models = models;
        self
    }

    /// Set the max-tokens budget.
    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    /// Set the per-tool-group enable flags.
    pub fn with_tool_groups(mut self, g: ToolGroupsConfig) -> Self {
        self.tool_groups = g;
        self
    }

    /// Set the MCP server map.
    pub fn with_mcp_servers(mut self, m: HashMap<String, McpServerEntry>) -> Self {
        self.mcp_servers = m;
        self
    }

    /// Set the resolved browser configuration.
    pub fn with_browser(mut self, b: ResolvedBrowserConfig) -> Self {
        self.browser = b;
        self
    }

    /// Set the on-disk config path (for the "API key not set" error).
    pub fn with_config_path(mut self, p: PathBuf) -> Self {
        self.config_path = p;
        self
    }

    /// Set the JMAP clients.
    pub fn with_jmap_clients(mut self, c: HashMap<String, JmapClient>) -> Self {
        self.jmap_clients = c;
        self
    }

    /// Set the CalDAV clients.
    pub fn with_caldav_clients(mut self, c: HashMap<String, CalDavClient>) -> Self {
        self.caldav_clients = c;
        self
    }

    /// Set the Trello client.
    pub fn with_trello_client(mut self, c: Option<TrelloClient>) -> Self {
        self.trello_client = c;
        self
    }

    /// Set the SearXNG URL.
    pub fn with_searxng_url(mut self, u: Option<String>) -> Self {
        self.searxng_url = u;
        self
    }

    /// Set the CSV database path override.
    pub fn with_csv_db_path(mut self, p: Option<String>) -> Self {
        self.csv_db_path = p;
        self
    }

    /// Set the feature-flag map.
    pub fn with_feature_flags(mut self, f: HashMap<String, bool>) -> Self {
        self.feature_flags = f;
        self
    }

    /// Set the content libraries.
    pub fn with_content_libraries(mut self, l: Vec<ContentLibrary>) -> Self {
        self.content_libraries = l;
        self
    }

    /// Build the [`AgentConfig`].
    pub fn build(self) -> AgentConfig {
        AgentConfig {
            models: self.models,
            max_tokens: self.max_tokens,
            tool_groups: self.tool_groups,
            mcp_servers: self.mcp_servers,
            browser: self.browser,
            config_path: self.config_path,
            jmap_clients: self.jmap_clients,
            caldav_clients: self.caldav_clients,
            trello_client: self.trello_client,
            searxng_url: self.searxng_url,
            csv_db_path: self.csv_db_path,
            feature_flags: self.feature_flags,
            content_libraries: self.content_libraries,
        }
    }
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test helper: build an `AgentConfig` whose `models` map contains a
/// single test model. Mirrors the `make_config(port)` fixtures the
/// agent's existing tests use so the test-side churn is a one-liner.
#[cfg(test)]
pub(crate) fn make_test_agent_config() -> AgentConfig {
    use crate::config::LlmConfig;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test-model".to_string(),
            api_url: "http://localhost:0".to_string(),
            api_key: "test-key".to_string(),
            cost: Some(0),
            use_case: vec!["chat".to_string()],
        },
    );
    AgentConfigBuilder::new().with_models(models).build()
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `config_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
