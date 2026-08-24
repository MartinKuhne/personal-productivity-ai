//! Domain-specific configuration for the agent module.
//!
//! Defines the configuration types that the agent consumes: LLM models, tool
//! groups, MCP servers, integration clients (JMAP, CalDAV, CardDAV, Trello,
//! SearXNG), runtime feature flags, CSV database path, resolved browser
//! config, content libraries, and the config path.
//!
//! Unit tests live in the sibling `config_tests.rs` sidecar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Integration Clients
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct JmapClient {
    pub url: String,
    pub token: String,
}

impl std::fmt::Debug for JmapClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JmapClient")
            .field("url", &self.url)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CalDavClient {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for CalDavClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CalDavClient")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TrelloClient {
    pub token: String,
    #[serde(alias = "apiKey", alias = "api_key", alias = "key")]
    pub api_key: String,
}

impl std::fmt::Debug for TrelloClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrelloClient")
            .field("token", &"[REDACTED]")
            .field("apiKey", &"[REDACTED]")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// LLM Model Config
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct LlmConfig {
    /// The literal model ID to pass to the API (e.g. `google/gemini-2.5-flash:free`).
    pub model: String,
    /// API endpoint URL.
    pub api_url: String,
    pub api_key: String,
    /// Cost for auto-model selection (lower = preferred). Default 0.
    #[serde(default)]
    pub cost: Option<i32>,
    /// Use cases for this model (e.g. "chat", "vision", "embeddings").
    #[serde(
        default = "default_use_case",
        alias = "capabilities",
        deserialize_with = "deserialize_use_case_or_capabilities"
    )]
    pub use_case: Vec<String>,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("model", &self.model)
            .field("api_url", &self.api_url)
            .field("api_key", &"[REDACTED]")
            .field("cost", &self.cost)
            .field("use_case", &self.use_case)
            .finish()
    }
}

fn deserialize_use_case_or_capabilities<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::String(s) => Ok(vec![s]),
        StringOrVec::Vec(v) => Ok(v),
    }
}

pub fn default_use_case() -> Vec<String> {
    vec!["chat".to_string()]
}

impl LlmConfig {
    pub fn get_cost(&self) -> i32 {
        self.cost.unwrap_or(0)
    }
    pub fn has_use_case(&self, use_case: impl AsRef<str>) -> bool {
        let uc_ref = use_case.as_ref();
        self.use_case.iter().any(|u| u == uc_ref)
    }

    pub fn has_vision(&self) -> bool {
        self.has_use_case("vision")
    }
}

// ---------------------------------------------------------------------------
// Content Library & VFS Trait
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ContentLibrary {
    pub root_folder: String,
    pub name: String,
    pub kind: String,
    #[serde(default = "default_readonly")]
    pub readonly: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_readonly() -> bool {
    true
}

/// Behaviour that callers need on a [`ContentLibrary`] to participate in
/// the VFS domain: containment checks, sub-path resolution,
/// read-only enforcement, and the user-facing display label.
pub trait ContentLibraryExt {
    fn display_label_for(&self, path: &Path) -> Option<String>;
    fn contains_path(&self, path: &Path) -> bool;
    fn resolve(&self, sub: &Path) -> PathBuf;
    fn is_writable(&self) -> bool;
    fn root_path(&self) -> PathBuf;
}

impl ContentLibraryExt for ContentLibrary {
    fn display_label_for(&self, path: &Path) -> Option<String> {
        let root = Path::new(&self.root_folder);
        let rel = path.strip_prefix(root).ok()?;
        let joined = Path::new(&self.name).join(rel);
        let mut label = joined.to_string_lossy().into_owned();
        if label.ends_with('\\') || label.ends_with('/') {
            label.pop();
        }
        Some(label.replace('\\', "/"))
    }

    fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(&self.root_folder)
    }

    fn resolve(&self, sub: &Path) -> PathBuf {
        PathBuf::from(&self.root_folder).join(sub)
    }

    fn is_writable(&self) -> bool {
        !self.readonly
    }

    fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.root_folder)
    }
}

// ---------------------------------------------------------------------------
// Tool Groups Config
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Configuration options for enabling or disabling specific tool groups.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ToolGroupsConfig {
    /// Enable or disable filesystem tools.
    #[serde(default = "default_true")]
    pub filesystem: bool,
    /// Enable or disable web tools.
    #[serde(default = "default_true")]
    pub web: bool,
    /// Enable or disable email tools.
    #[serde(default = "default_true")]
    pub email: bool,
    /// Enable or disable contacts tools.
    #[serde(default = "default_true")]
    pub contacts: bool,
    /// Enable or disable calendar tools.
    #[serde(default = "default_true")]
    pub calendar: bool,
    /// Enable or disable CSV database tools.
    #[serde(default = "default_true")]
    pub csv_db: bool,
    /// Enable or disable weather tools.
    #[serde(default = "default_true")]
    pub weather: bool,
    /// Enable or disable headless-browser automation tools.
    #[serde(default)]
    pub browser: bool,
    /// Enable or disable Trello agent tools.
    #[serde(default = "default_true")]
    pub trello: bool,
}

impl Default for ToolGroupsConfig {
    fn default() -> Self {
        Self {
            filesystem: true,
            web: true,
            email: true,
            contacts: true,
            calendar: true,
            csv_db: true,
            weather: true,
            browser: false,
            trello: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Browser Config
// ---------------------------------------------------------------------------

/// Per-tool-group settings for the browser tool family.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BrowserConfig {
    #[serde(default)]
    pub screenshot_dir: String,
    #[serde(default = "default_true")]
    pub headless: bool,
    #[serde(default = "default_browser_type")]
    pub browser_type: String,
    #[serde(default = "default_browser_idle_timeout")]
    pub idle_timeout_seconds: u64,
    #[serde(default = "default_browser_page_load_timeout")]
    pub page_load_timeout_ms: u64,
    #[serde(default)]
    pub storage_state_path: String,
}

fn default_browser_type() -> String {
    "firefox".to_string()
}

fn default_browser_idle_timeout() -> u64 {
    300
}

fn default_browser_page_load_timeout() -> u64 {
    30_000
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            screenshot_dir: String::new(),
            headless: true,
            browser_type: default_browser_type(),
            idle_timeout_seconds: default_browser_idle_timeout(),
            page_load_timeout_ms: default_browser_page_load_timeout(),
            storage_state_path: String::new(),
        }
    }
}

impl BrowserConfig {
    pub fn resolve(&self, content_libraries: &[ContentLibrary]) -> ResolvedBrowserConfig {
        let screenshot_dir = if self.screenshot_dir.is_empty() {
            default_screenshot_dir(content_libraries)
        } else {
            std::path::PathBuf::from(&self.screenshot_dir)
        };
        let storage_state_path = if self.storage_state_path.is_empty() {
            default_storage_state_path()
        } else {
            std::path::PathBuf::from(&self.storage_state_path)
        };
        ResolvedBrowserConfig {
            screenshot_dir,
            headless: self.headless,
            browser_type: self.browser_type.clone(),
            idle_timeout_seconds: self.idle_timeout_seconds,
            page_load_timeout_ms: self.page_load_timeout_ms,
            storage_state_path,
        }
    }
}

/// [`BrowserConfig`] with every empty string filled in with a concrete path.
#[derive(Clone, Debug)]
pub struct ResolvedBrowserConfig {
    pub screenshot_dir: std::path::PathBuf,
    pub headless: bool,
    pub browser_type: String,
    pub idle_timeout_seconds: u64,
    pub page_load_timeout_ms: u64,
    pub storage_state_path: std::path::PathBuf,
}

fn default_screenshot_dir(content_libraries: &[ContentLibrary]) -> std::path::PathBuf {
    content_libraries
        .first()
        .map(|lib| std::path::PathBuf::from(&lib.root_folder).join("browser-screenshots"))
        .unwrap_or_else(|| std::path::PathBuf::from("browser-screenshots"))
}

fn default_storage_state_path() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        if let Ok(roaming) = std::env::var("APPDATA") {
            return std::path::PathBuf::from(roaming)
                .join("fastmd")
                .join("browser-storage.json");
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(xdg)
                .join("fastmd")
                .join("browser-storage.json");
        }
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home)
                .join(".config")
                .join("fastmd")
                .join("browser-storage.json");
        }
    }
    std::path::PathBuf::from("browser-storage.json")
}

// ---------------------------------------------------------------------------
// MCP Server Config & OAuth
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct McpOAuthConfig {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub redirect_uri: Option<String>,
}

impl std::fmt::Debug for McpOAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpOAuthConfig")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scopes", &self.scopes)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct McpServerEntry {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub config: McpServerConfig,
}

impl McpServerEntry {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }
}

impl From<McpServerConfig> for McpServerEntry {
    fn from(config: McpServerConfig) -> Self {
        Self {
            enabled: true,
            config,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        oauth: Option<McpOAuthConfig>,
    },
}

impl std::fmt::Debug for McpServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdio { command, args, env } => f
                .debug_struct("McpServerConfig::Stdio")
                .field("command", command)
                .field("args", args)
                .field("env", env)
                .finish(),
            Self::Sse {
                url,
                headers,
                oauth,
            } => {
                let redacted_headers: HashMap<_, _> = headers
                    .keys()
                    .map(|k| (k.clone(), "[REDACTED]".to_string()))
                    .collect();
                f.debug_struct("McpServerConfig::Sse")
                    .field("url", url)
                    .field("headers", &redacted_headers)
                    .field("oauth", oauth)
                    .finish()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Agent Config
// ---------------------------------------------------------------------------

/// Domain-specific configuration for the agent module.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub(crate) models: HashMap<String, LlmConfig>,
    pub(crate) selected_chat_model: Option<String>,
    pub(crate) max_tokens: u32,
    pub(crate) tool_groups: ToolGroupsConfig,
    pub(crate) mcp_servers: HashMap<String, McpServerEntry>,
    pub(crate) browser: ResolvedBrowserConfig,
    pub(crate) config_path: PathBuf,
    pub(crate) jmap_clients: HashMap<String, JmapClient>,
    pub(crate) caldav_clients: HashMap<String, CalDavClient>,
    pub(crate) trello_client: Option<TrelloClient>,
    pub(crate) searxng_url: Option<String>,
    pub(crate) csv_db_path: Option<String>,
    pub(crate) feature_flags: HashMap<String, bool>,
    pub(crate) content_libraries: Vec<ContentLibrary>,
    pub(crate) system_library_name: Option<String>,
}

impl AgentConfig {
    pub fn models(&self) -> &HashMap<String, LlmConfig> {
        &self.models
    }

    pub fn selected_chat_model(&self) -> Option<&str> {
        self.selected_chat_model.as_deref()
    }

    pub fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    pub fn tool_groups(&self) -> &ToolGroupsConfig {
        &self.tool_groups
    }

    pub fn mcp_servers(&self) -> &HashMap<String, McpServerEntry> {
        &self.mcp_servers
    }

    pub fn browser(&self) -> &ResolvedBrowserConfig {
        &self.browser
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn jmap_clients(&self) -> &HashMap<String, JmapClient> {
        &self.jmap_clients
    }

    pub fn caldav_clients(&self) -> &HashMap<String, CalDavClient> {
        &self.caldav_clients
    }

    pub fn trello_client(&self) -> Option<&TrelloClient> {
        self.trello_client.as_ref()
    }

    pub fn searxng_url(&self) -> Option<&str> {
        self.searxng_url.as_deref()
    }

    pub fn csv_db_path(&self) -> Option<&str> {
        self.csv_db_path.as_deref()
    }

    pub fn feature_flags(&self) -> &HashMap<String, bool> {
        &self.feature_flags
    }

    pub fn content_libraries(&self) -> &[ContentLibrary] {
        &self.content_libraries
    }

    pub fn system_library_name(&self) -> Option<&str> {
        self.system_library_name.as_deref()
    }

    pub fn system_library_display_name(&self) -> &str {
        self.system_library_name.as_deref().unwrap_or("System")
    }

    pub fn model_for_use_case(&self, use_case: impl AsRef<str>) -> Option<(&String, &LlmConfig)> {
        let uc_ref = use_case.as_ref();
        self.models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(uc_ref))
            .min_by_key(|(_, cfg)| cfg.get_cost())
    }

    pub fn select_chat_model(&self) -> Result<&LlmConfig, String> {
        let model_cfg = if let Some(key) = &self.selected_chat_model {
            if let Some(cfg) = self.models.get(key) {
                cfg
            } else if let Some((_key, model_cfg)) = self.model_for_use_case("chat") {
                model_cfg
            } else if let Some(model_cfg) = self.models.values().next() {
                model_cfg
            } else {
                return Err("No LLM models are configured.".to_string());
            }
        } else if let Some((_key, model_cfg)) = self.model_for_use_case("chat") {
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
pub struct AgentConfigBuilder {
    models: HashMap<String, LlmConfig>,
    selected_chat_model: Option<String>,
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
    system_library_name: Option<String>,
}

impl AgentConfigBuilder {
    pub fn new() -> Self {
        let browser = BrowserConfig::default().resolve(&[]);
        Self {
            models: HashMap::new(),
            selected_chat_model: None,
            max_tokens: 32768,
            tool_groups: ToolGroupsConfig::default(),
            mcp_servers: HashMap::new(),
            browser,
            config_path: PathBuf::new(),
            jmap_clients: HashMap::new(),
            caldav_clients: HashMap::new(),
            trello_client: None,
            searxng_url: None,
            csv_db_path: None,
            feature_flags: HashMap::new(),
            content_libraries: Vec::new(),
            system_library_name: None,
        }
    }

    pub fn with_models(mut self, models: HashMap<String, LlmConfig>) -> Self {
        self.models = models;
        self
    }

    pub fn with_selected_chat_model(mut self, model: Option<String>) -> Self {
        self.selected_chat_model = model;
        self
    }

    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    pub fn with_tool_groups(mut self, g: ToolGroupsConfig) -> Self {
        self.tool_groups = g;
        self
    }

    pub fn with_mcp_servers(mut self, m: HashMap<String, McpServerEntry>) -> Self {
        self.mcp_servers = m;
        self
    }

    pub fn with_browser(mut self, b: ResolvedBrowserConfig) -> Self {
        self.browser = b;
        self
    }

    pub fn with_config_path(mut self, p: PathBuf) -> Self {
        self.config_path = p;
        self
    }

    pub fn with_jmap_clients(mut self, c: HashMap<String, JmapClient>) -> Self {
        self.jmap_clients = c;
        self
    }

    pub fn with_caldav_clients(mut self, c: HashMap<String, CalDavClient>) -> Self {
        self.caldav_clients = c;
        self
    }

    pub fn with_trello_client(mut self, c: Option<TrelloClient>) -> Self {
        self.trello_client = c;
        self
    }

    pub fn with_searxng_url(mut self, u: Option<String>) -> Self {
        self.searxng_url = u;
        self
    }

    pub fn with_csv_db_path(mut self, p: Option<String>) -> Self {
        self.csv_db_path = p;
        self
    }

    pub fn with_feature_flags(mut self, f: HashMap<String, bool>) -> Self {
        self.feature_flags = f;
        self
    }

    pub fn with_content_libraries(mut self, l: Vec<ContentLibrary>) -> Self {
        self.content_libraries = l;
        self
    }

    pub fn with_system_library_name(mut self, name: Option<String>) -> Self {
        self.system_library_name = name;
        self
    }

    pub fn build(self) -> AgentConfig {
        AgentConfig {
            models: self.models,
            selected_chat_model: self.selected_chat_model,
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
            system_library_name: self.system_library_name,
        }
    }
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) fn make_test_agent_config() -> AgentConfig {
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

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
