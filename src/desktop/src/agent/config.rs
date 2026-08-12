//! Domain-specific configuration for the agent module.
//!
//! The agent module does not need (and should not depend on) the entire
//! [`crate::config::AppConfig`]. [`AgentConfig`] is the projected slice that
//! the orchestrator hands to the agent: LLM models, user/system prompt
//! context, tool groups, MCP servers, integration clients that the
//! tools read at execute time (JMAP, CalDAV, CardDAV, Trello, SearXNG),
//! runtime feature flags, the CSV database path, the resolved browser
//! config, and content libraries for VFS resolution.
//!
//! Fields the agent doesn't touch — `inline_editor_enabled`,
//! `pdf_converter_command`, `table_width_strategy`, `discord` — are
//! excluded by the projection.
//!
//! Construction:
//! - [`AgentConfig::from_app_config`] — projection from the global config
//!   (the canonical entry point at the orchestrator seam).
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
    AppConfig, CalDavClient, ContentLibrary, JmapClient, LlmConfig, McpServerEntry,
    ResolvedBrowserConfig, ToolGroupsConfig, TrelloClient, get_config_path,
};

/// Domain-specific configuration for the agent module.
///
/// Built via [`AgentConfig::from_app_config`] (orchestrator seam) or
/// [`AgentConfigBuilder`] (tests / per-session overrides). Every field is
/// the slice the agent actually consumes; fields the agent doesn't read
/// (`inline_editor_enabled`, `pdf_converter_command`,
/// `table_width_strategy`, `discord`) are intentionally not present.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    models: HashMap<String, LlmConfig>,
    user_name: Option<String>,
    user_address: Option<String>,
    user_birthdate: Option<String>,
    user_gender: Option<String>,
    system_prompt_extension: Option<String>,
    max_tokens: u32,
    tool_groups: ToolGroupsConfig,
    mcp_servers: HashMap<String, McpServerEntry>,
    browser: ResolvedBrowserConfig,
    content_libraries: Vec<ContentLibrary>,
    config_path: PathBuf,
    // Tool execution reads these; the agent loop doesn't, but the
    // `is_enabled` check on the registered tools does. Cloning them
    // here keeps `ToolContext` self-contained — the executor's parallel
    // workers don't need to chase a back-pointer to the global config.
    jmap_clients: HashMap<String, JmapClient>,
    caldav_clients: HashMap<String, CalDavClient>,
    trello_client: Option<TrelloClient>,
    searxng_url: Option<String>,
    csv_db_path: Option<String>,
    feature_flags: HashMap<String, bool>,
}

impl AgentConfig {
    /// Project the global [`AppConfig`] into the agent's domain slice.
    ///
    /// Resolves the browser config (filling env-dependent defaults) and
    /// captures the config path for the "API key not set" error message.
    /// The orchestrator calls this once per `ConfigArrived` event.
    pub fn from_app_config(cfg: &AppConfig) -> Self {
        let browser = cfg.browser.resolve(&cfg.content_libraries);
        let config_path = get_config_path();
        Self {
            models: cfg.models.clone(),
            user_name: cfg.user_name.clone(),
            user_address: cfg.user_address.clone(),
            user_birthdate: cfg.user_birthdate.clone(),
            user_gender: cfg.user_gender.clone(),
            system_prompt_extension: cfg.system_prompt_extension.clone(),
            max_tokens: cfg.max_tokens,
            tool_groups: cfg.tool_groups.clone(),
            mcp_servers: cfg.mcp_servers.clone(),
            browser,
            content_libraries: cfg.content_libraries.clone(),
            config_path,
            jmap_clients: cfg.jmap_clients.clone(),
            caldav_clients: cfg.caldav_clients.clone(),
            trello_client: cfg.trello_client.clone(),
            searxng_url: cfg.searxng_url.clone(),
            csv_db_path: cfg.csv_db_path.clone(),
            feature_flags: cfg.feature_flags.clone(),
        }
    }

    /// Map of configured LLM models by name.
    pub fn models(&self) -> &HashMap<String, LlmConfig> {
        &self.models
    }

    /// User's name (used in the dynamic system prompt).
    pub fn user_name(&self) -> Option<&str> {
        self.user_name.as_deref()
    }

    /// User's address.
    pub fn user_address(&self) -> Option<&str> {
        self.user_address.as_deref()
    }

    /// User's birthdate.
    pub fn user_birthdate(&self) -> Option<&str> {
        self.user_birthdate.as_deref()
    }

    /// User's gender.
    pub fn user_gender(&self) -> Option<&str> {
        self.user_gender.as_deref()
    }

    /// System-prompt extension string injected into the dynamic prompt.
    pub fn system_prompt_extension(&self) -> Option<&str> {
        self.system_prompt_extension.as_deref()
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

    /// Content libraries for VFS resolution and USER.md discovery.
    pub fn content_libraries(&self) -> &[ContentLibrary] {
        &self.content_libraries
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
}

/// Ergonomic projection so callers that already hold an `&AppConfig` can
/// use `.into()` instead of spelling out [`AgentConfig::from_app_config`].
impl From<&AppConfig> for AgentConfig {
    fn from(cfg: &AppConfig) -> Self {
        AgentConfig::from_app_config(cfg)
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfigBuilder::new().build()
    }
}

/// Fluent builder for [`AgentConfig`].
///
/// Used in two places:
/// - the projection helper [`AgentConfig::from_app_config`], which is
///   the canonical orchestrator-side constructor;
/// - tests and per-session overrides that want to assemble a domain
///   config without going through the global [`AppConfig`].
///
/// All fields default to the same values as [`AppConfig::default`] so
/// tests can build minimal configs without spelling every field.
pub struct AgentConfigBuilder {
    models: HashMap<String, LlmConfig>,
    user_name: Option<String>,
    user_address: Option<String>,
    user_birthdate: Option<String>,
    user_gender: Option<String>,
    system_prompt_extension: Option<String>,
    max_tokens: u32,
    tool_groups: ToolGroupsConfig,
    mcp_servers: HashMap<String, McpServerEntry>,
    browser: ResolvedBrowserConfig,
    content_libraries: Vec<ContentLibrary>,
    config_path: PathBuf,
    jmap_clients: HashMap<String, JmapClient>,
    caldav_clients: HashMap<String, CalDavClient>,
    trello_client: Option<TrelloClient>,
    searxng_url: Option<String>,
    csv_db_path: Option<String>,
    feature_flags: HashMap<String, bool>,
}

impl AgentConfigBuilder {
    /// Create a builder populated with the same defaults as
    /// [`AppConfig::default`].
    pub fn new() -> Self {
        let app_defaults = AppConfig::default();
        let browser = app_defaults
            .browser
            .resolve(&app_defaults.content_libraries);
        Self {
            models: HashMap::new(),
            user_name: None,
            user_address: None,
            user_birthdate: None,
            user_gender: None,
            system_prompt_extension: None,
            max_tokens: app_defaults.max_tokens,
            tool_groups: app_defaults.tool_groups.clone(),
            mcp_servers: HashMap::new(),
            browser,
            content_libraries: Vec::new(),
            config_path: get_config_path(),
            jmap_clients: HashMap::new(),
            caldav_clients: HashMap::new(),
            trello_client: None,
            searxng_url: None,
            csv_db_path: None,
            feature_flags: app_defaults.feature_flags.clone(),
        }
    }

    /// Set the LLM model map.
    pub fn with_models(mut self, models: HashMap<String, LlmConfig>) -> Self {
        self.models = models;
        self
    }

    /// Set the user-identifying fields in one call.
    pub fn with_user(
        mut self,
        name: Option<String>,
        address: Option<String>,
        birthdate: Option<String>,
        gender: Option<String>,
    ) -> Self {
        self.user_name = name;
        self.user_address = address;
        self.user_birthdate = birthdate;
        self.user_gender = gender;
        self
    }

    /// Set the system-prompt extension.
    pub fn with_system_prompt_extension(mut self, ext: Option<String>) -> Self {
        self.system_prompt_extension = ext;
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

    /// Set the content libraries.
    pub fn with_content_libraries(mut self, libs: Vec<ContentLibrary>) -> Self {
        self.content_libraries = libs;
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

    /// Build the [`AgentConfig`].
    pub fn build(self) -> AgentConfig {
        AgentConfig {
            models: self.models,
            user_name: self.user_name,
            user_address: self.user_address,
            user_birthdate: self.user_birthdate,
            user_gender: self.user_gender,
            system_prompt_extension: self.system_prompt_extension,
            max_tokens: self.max_tokens,
            tool_groups: self.tool_groups,
            mcp_servers: self.mcp_servers,
            browser: self.browser,
            content_libraries: self.content_libraries,
            config_path: self.config_path,
            jmap_clients: self.jmap_clients,
            caldav_clients: self.caldav_clients,
            trello_client: self.trello_client,
            searxng_url: self.searxng_url,
            csv_db_path: self.csv_db_path,
            feature_flags: self.feature_flags,
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
