//! Application configuration types and persistence — JMAP, CalDAV, CardDAV clients, content libraries, model settings.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (CONFIG-001..CONFIG-008; CONFIG-009 superseded by VFS-004/009) for the full specification.
//!
//! The VFS domain types ([`VirtualPath`], [`VirtualPathError`], and the
//! behaviour on [`ContentLibrary`]) now live under
//! [`crate::app::vfs`] and are re-exported here for backwards
//! compatibility — prefer importing from `crate::app::vfs` in new
//! code. See [`app/vfs/SPEC.md`](../app/vfs/SPEC.md).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[deprecated(note = "import directly from crate::app::vfs")]
pub use crate::app::vfs::{
    ContentLibraryExt, VirtualPath, VirtualPathError, library_display_label,
};

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
pub struct TrelloClient {
    pub token: String,
    #[serde(rename = "apiKey")]
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

fn default_use_case() -> Vec<String> {
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

fn default_feature_flags() -> HashMap<String, bool> {
    let mut m = HashMap::new();
    // When enabled, tool call responses include full data in logs (may be verbose).
    // When disabled (default), only basic success/failure is logged for privacy.
    m.insert("toolCallDebugMode".to_string(), false);
    m
}

fn default_max_tokens() -> u32 {
    32768
}

fn default_true() -> bool {
    true
}

fn default_table_width_strategy() -> String {
    "waterfill".to_string()
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
    /// Enable or disable the headless-browser automation tools
    /// (`browser_navigate`, `browser_click`, ...). Off by default
    /// for the "no system access" posture described in the README;
    /// the user must opt in to launch a Firefox subprocess
    /// (BRWS-CONF-001).
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

/// Discord bot configuration.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscordConfig {
    /// Bot token from the Discord Developer Portal.
    #[serde(default)]
    pub bot_token: Option<String>,
    /// Channel IDs where the bot should respond (empty = all channels where mentioned).
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    /// Guild IDs where the bot is active (empty = all guilds).
    #[serde(default)]
    pub allowed_guilds: Vec<String>,
    /// Enable slash command registration.
    #[serde(default = "default_true")]
    pub register_commands: bool,
    /// Default system prompt for the LLM.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Maximum conversation history length (number of messages).
    #[serde(default = "default_discord_history_len")]
    pub max_history: usize,
    /// Per-user rate limit (requests per minute).
    #[serde(default = "default_discord_rate_limit")]
    pub rate_limit_per_minute: u32,
}

fn default_discord_history_len() -> usize {
    20
}

fn default_discord_rate_limit() -> u32 {
    10
}

/// Per-tool-group settings for the browser tool family.
///
/// Lives under `config.browser`. All fields are optional so an
/// empty `browser: {}` block (or a missing one entirely) is a
/// valid configuration — sensible defaults are filled in by
/// [`BrowserConfig::resolve`] and used by the
/// [`crate::app::browser::BrowserSession`].
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BrowserConfig {
    /// Directory the LLM is allowed to write screenshots to. An
    /// empty string means "use the first content library's
    /// `browser-screenshots/` subfolder" (BRWS-CONF-002). The
    /// directory is created on first write.
    #[serde(default)]
    pub screenshot_dir: String,
    /// `true` (default) runs Firefox headlessly. Set to `false`
    /// to debug the browser visually — the user will see the
    /// Firefox window open.
    #[serde(default = "default_true")]
    pub headless: bool,
    /// Browser channel. Only `firefox` is wired up in this
    /// revision; other values fail at session launch with a
    /// `Discovery` error (BRWS-CONF-003).
    #[serde(default = "default_browser_type")]
    pub browser_type: String,
    /// Close the Firefox subprocess after this many seconds of
    /// tool-call silence. `0` disables the idle timeout. The
    /// persisted cookie file is reloaded on the next launch, so
    /// the user stays logged in across idle periods
    /// (BRWS-CONF-004).
    #[serde(default = "default_browser_idle_timeout")]
    pub idle_timeout_seconds: u64,
    /// Per-page navigation timeout. Defaults to 30 seconds.
    #[serde(default = "default_browser_page_load_timeout")]
    pub page_load_timeout_ms: u64,
    /// Path to the Playwright `storage_state` JSON file. The
    /// default is `%APPDATA%\fastmd\browser-storage.json` on
    /// Windows and the XDG `~/.config/fastmd/` equivalent
    /// elsewhere. Cookies and local storage are saved here on
    /// every mutating tool call (debounced) and reloaded on the
    /// next session launch (BRWS-CONF-005).
    #[serde(default)]
    pub storage_state_path: String,
}

/// Default browser channel. Firefox-only for v1; `browser_type`
/// is kept as a string so we can add others without a schema
/// change.
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
    /// Fill in any empty fields with defaults that depend on
    /// environment (e.g. `%APPDATA%`) or on the content library
    /// list. Called by [`crate::app::browser::BrowserSession`]
    /// right before the first launch so we don't pay the
    /// environment lookup at config-load time.
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

/// [`BrowserConfig`] with every empty string filled in with a
/// concrete path. Pass this to the session so the rest of the
/// code never has to think about "is this a default?".
#[derive(Clone, Debug)]
pub struct ResolvedBrowserConfig {
    /// Absolute path; created on first write.
    pub screenshot_dir: std::path::PathBuf,
    /// `true` for headless.
    pub headless: bool,
    /// `"firefox"` today; future values surface as a
    /// `Discovery` error at launch.
    pub browser_type: String,
    /// Idle timeout in seconds (`0` = never).
    pub idle_timeout_seconds: u64,
    /// Per-page navigation timeout in milliseconds.
    pub page_load_timeout_ms: u64,
    /// Absolute path to the Playwright `storage_state` JSON.
    pub storage_state_path: std::path::PathBuf,
}

fn default_screenshot_dir(content_libraries: &[ContentLibrary]) -> std::path::PathBuf {
    content_libraries
        .first()
        .map(|lib| std::path::PathBuf::from(&lib.root_folder).join("browser-screenshots"))
        .unwrap_or_else(|| std::path::PathBuf::from("browser-screenshots"))
}

fn default_storage_state_path() -> std::path::PathBuf {
    // %APPDATA% on Windows, $XDG_CONFIG_HOME or ~/.config on
    // Unix. Failing that, the current working directory.
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

/// Optional OAuth 2.1 configuration for an HTTP MCP server. Used by
/// the MCP client to perform the authorization code flow (with
/// PKCE) per `doc/distill/mcp.md` §4.
///
/// If this block is absent, the client still attempts the OAuth
/// flow on a 401 with `WWW-Authenticate`; the difference is that
/// without a pre-registered `client_id`, the client will register
/// dynamically (RFC 7591) or surface an error if the server
/// doesn't advertise a `registration_endpoint`. Supplying a
/// `client_id` short-circuits the discovery + registration steps
/// and lets the client sign in immediately.
#[derive(serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct McpOAuthConfig {
    /// Pre-registered client identifier. If set, the client
    /// skips Dynamic Client Registration (RFC 7591) and uses
    /// this value as `client_id` on the authorization and token
    /// requests.
    #[serde(default)]
    pub client_id: Option<String>,
    /// Pre-registered client secret. Only required for
    /// confidential clients (when the AS metadata lists
    /// `client_secret_basic` or `client_secret_post` in
    /// `token_endpoint_auth_methods_supported`). Public clients
    /// (`token_endpoint_auth_method = "none"`) do not get a
    /// secret.
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Scopes the client will request. Per spec §4.5 the client
    /// SHOULD use the `scope` from the initial `WWW-Authenticate`
    /// header, falling back to `scopes_supported` from the
    /// resource metadata. Explicitly listing scopes here forces
    /// the client to request them in addition to the discovered
    /// set; the union is requested.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Optional redirect URI for the OAuth 2.1 flow. If set, this
    /// URI will be used instead of the default loopback redirect
    /// (`http://127.0.0.1:<random_port>/callback`). This is
    /// required for providers like Atlassian that require the
    /// redirect URI to be pre-registered in the app settings.
    /// The URI must use `http://127.0.0.1` or `http://localhost`
    /// with a specific port and path (e.g. `http://127.0.0.1:8080/callback`).
    #[serde(default)]
    pub redirect_uri: Option<String>,
}

impl std::fmt::Debug for McpOAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpOAuthConfig")
            .field("client_id", &self.client_id)
            // The secret is sensitive; we never log it.
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scopes", &self.scopes)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

/// Single source of truth for a configured MCP server: the per-server
/// `enabled` flag plus the transport/auth config, flattened into one
/// YAML block per server.
///
/// `enabled` defaults to `true` so existing YAMLs that omit the key
/// keep the previous behaviour (CONFIG-012). Toggling the flag in
/// the UI flips the bool in place; the transport and auth fields of
/// the same entry are preserved.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct McpServerEntry {
    /// Whether this server's tools are offered to the LLM.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Transport + auth config (stdio or sse), flattened into the
    /// same YAML block as `enabled`.
    #[serde(flatten)]
    pub config: McpServerConfig,
}

impl McpServerEntry {
    /// Whether the server is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The server's transport + auth config.
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }
}

/// Ergonomic conversion so existing test sites that construct a
/// `McpServerConfig` directly can pass it into the new
/// `HashMap<String, McpServerEntry>` field with a one-character
/// `.into()`. Newly-built entries default to `enabled: true`.
impl From<McpServerConfig> for McpServerEntry {
    fn from(config: McpServerConfig) -> Self {
        Self {
            enabled: true,
            config,
        }
    }
}

/// Configuration for an external MCP (Model Context Protocol) server connection.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerConfig {
    /// Local subprocess transport over standard input/output.
    Stdio {
        /// Executable command name or absolute path.
        command: String,
        /// Command line arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables for the subprocess.
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// Remote Server-Sent Events transport.
    Sse {
        /// HTTP/HTTPS URL for the SSE endpoint.
        url: String,
        /// Optional HTTP headers (e.g. authorization tokens).
        /// If this map contains an `Authorization` entry, the
        /// client uses it verbatim and does NOT run the OAuth
        /// flow — useful for static API keys, internal servers,
        /// or pre-obtained bearer tokens. If absent or empty,
        /// the client runs the OAuth 2.1 flow on a 401 with
        /// `WWW-Authenticate` per `doc/distill/mcp.md` §4.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// OAuth 2.1 client configuration. Optional; see
        /// [`McpOAuthConfig`]. Only consulted if the static
        /// `Authorization` header is absent.
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub user_address: Option<String>,
    #[serde(default)]
    pub user_birthdate: Option<String>,
    #[serde(default)]
    pub user_gender: Option<String>,
    #[serde(default)]
    pub system_prompt_extension: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, LlmConfig>,
    #[serde(default)]
    pub searxng_url: Option<String>,
    #[serde(default)]
    pub jmap_clients: HashMap<String, JmapClient>,
    #[serde(default)]
    pub caldav_clients: HashMap<String, CalDavClient>,
    #[serde(default)]
    pub content_libraries: Vec<ContentLibrary>,
    /// PDF converter command template (AGENT-009).
    #[serde(default)]
    pub pdf_converter_command: Option<Vec<String>>,
    /// Enable built-in inline text editor (CONFIG-001). Default: false.
    #[serde(default)]
    pub inline_editor_enabled: bool,
    /// Override default storage location for CSV databases.
    #[serde(default)]
    pub csv_db_path: Option<String>,
    /// Runtime feature flags. Map of feature name to enabled/disabled.
    #[serde(default = "default_feature_flags")]
    pub feature_flags: HashMap<String, bool>,
    /// Maximum tokens for LLM responses (AGENT-010). Default: 32768.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Configuration for enabling/disabling tool groups.
    #[serde(default)]
    pub tool_groups: ToolGroupsConfig,
    /// Configured external MCP servers by server name. Each entry
    /// carries its own `enabled` flag (CONFIG-012) plus the
    /// transport/auth config flattened underneath.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerEntry>,
    /// Table width algorithm for deficit regime. Default: "proportional".
    /// Options: "proportional" (fast, O(|S|)), "waterfill" (better G1, O(K log |S|)).
    #[serde(default = "default_table_width_strategy")]
    pub table_width_strategy: String,
    /// Browser automation settings (BRWS-CONF-001..005). Ignored
    /// unless `tool_groups.browser == true`. Default fields are
    /// filled in by [`BrowserConfig::resolve`] at session launch.
    #[serde(default)]
    pub browser: BrowserConfig,
    /// Trello client configuration (token and secret).
    #[serde(default)]
    pub trello_client: Option<TrelloClient>,

    /// Discord bot configuration.
    #[serde(default)]
    pub discord: Option<DiscordConfig>,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("user_name", &self.user_name)
            .field("user_address", &self.user_address)
            .field("user_birthdate", &self.user_birthdate)
            .field("user_gender", &self.user_gender)
            .field("system_prompt_extension", &self.system_prompt_extension)
            .field("models", &self.models)
            .field("searxng_url", &self.searxng_url)
            .field("jmap_clients", &self.jmap_clients)
            .field("caldav_clients", &self.caldav_clients)
            .field("content_libraries", &self.content_libraries)
            .field("pdf_converter_command", &self.pdf_converter_command)
            .field("inline_editor_enabled", &self.inline_editor_enabled)
            .field("csv_db_path", &self.csv_db_path)
            .field("feature_flags", &self.feature_flags)
            .field("max_tokens", &self.max_tokens)
            .field("tool_groups", &self.tool_groups)
            .field("mcp_servers", &self.mcp_servers)
            .field("table_width_strategy", &self.table_width_strategy)
            .field("trello_client", &self.trello_client)
            .field("discord", &self.discord)
            .finish()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            user_name: None,
            user_address: None,
            user_birthdate: None,
            user_gender: None,
            system_prompt_extension: None,
            models: HashMap::new(),
            searxng_url: Some("http://localhost:8090".to_string()),
            jmap_clients: HashMap::new(),
            caldav_clients: HashMap::new(),
            content_libraries: Vec::new(),
            pdf_converter_command: None,
            inline_editor_enabled: false,
            csv_db_path: None,
            feature_flags: default_feature_flags(),
            max_tokens: default_max_tokens(),
            tool_groups: ToolGroupsConfig::default(),
            mcp_servers: HashMap::new(),
            table_width_strategy: default_table_width_strategy(),
            browser: BrowserConfig::default(),
            trello_client: None,
            discord: None,
        }
    }
}

impl AppConfig {
    /// Parse `table_width_strategy` into the enum used by the FTWA algorithm.
    /// Returns `DeficitStrategy::BreakpointWaterFill` for "waterfill",
    /// `DeficitStrategy::ProportionalToSlack` for everything else.
    pub fn deficit_strategy(&self) -> crate::ui::table_width::DeficitStrategy {
        crate::ui::table_width::DeficitStrategy::from_config(&self.table_width_strategy)
    }

    /// Find the best model for a given use_case (lowest cost among matches).
    pub fn model_for_use_case(&self, use_case: impl AsRef<str>) -> Option<(&String, &LlmConfig)> {
        let uc_ref = use_case.as_ref();
        self.models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(uc_ref))
            .min_by_key(|(_, cfg)| cfg.get_cost())
    }

    /// Find all models tied for the minimum cost for a given use_case.
    pub fn models_for_use_case_min_cost(
        &self,
        use_case: impl AsRef<str>,
    ) -> Vec<(&String, &LlmConfig)> {
        let uc_ref = use_case.as_ref();
        let candidates: Vec<_> = self
            .models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(uc_ref))
            .collect();
        let min_cost = match candidates.iter().map(|(_, cfg)| cfg.get_cost()).min() {
            Some(c) => c,
            None => return Vec::new(),
        };
        candidates
            .into_iter()
            .filter(|(_, cfg)| cfg.get_cost() == min_cost)
            .collect()
    }

    /// Validate configuration, returning a list of warnings.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check models have valid use_case values
        let valid_use_cases = ["chat", "embeddings", "vision"];
        for (key, cfg) in &self.models {
            for uc in &cfg.use_case {
                if !valid_use_cases.contains(&uc.as_str()) {
                    warnings.push(format!("Model '{}' has unknown use_case: '{}'", key, uc));
                }
            }
        }

        // Check at least one chat model exists when models are configured
        if !self.models.is_empty() && !self.models.values().any(|m| m.has_use_case("chat")) {
            warnings.push("No model configured with 'chat' use_case".to_string());
        }

        warnings
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("fastmd").join("config.yaml")
    } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
        PathBuf::from(user_profile).join(".fastmd.yaml")
    } else {
        PathBuf::from(".fastmd.yaml")
    }
}

/// Load the application configuration from the platform-default location
/// (resolved via [`get_config_path`]). Thin wrapper around the path-based
/// loader for production callers.
pub fn load_config() -> AppConfig {
    load_config_from_path(&get_config_path())
}

/// Persist the supplied configuration to the platform-default
/// location. Returns the path written on success. The parent
/// directory is created if it does not exist.
pub fn save_config(config: &AppConfig) -> Result<PathBuf, String> {
    save_config_to_path(config, &get_config_path())
}

/// Persist the supplied configuration to an explicit path. See
/// [`save_config`] for the platform-default wrapper.
pub fn save_config_to_path(config: &AppConfig, path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create config parent dir {}: {e}",
                parent.display()
            )
        })?;
    }
    let yaml = serde_norway::to_string(config)
        .map_err(|e| format!("failed to serialise AppConfig to YAML: {e}"))?;
    std::fs::write(path, yaml)
        .map_err(|e| format!("failed to write config to {}: {e}", path.display()))?;
    Ok(path.to_path_buf())
}

/// Load the application configuration from an explicit file path.
///
/// Behaviour:
/// - If the file exists and parses cleanly, return the parsed config.
/// - If the file exists but is unreadable or unparseable, log a `tracing`
///   error and fall back to [`AppConfig::default`].
/// - If the file does not exist, create the parent directory and write a
///   default-config YAML so subsequent loads succeed, then return the
///   default.
///
/// Taking an explicit path (rather than reading `APPDATA` from the
/// environment) makes the function deterministic for tests and prevents
/// parallel tests from racing on the process-wide environment.
pub(crate) fn load_config_from_path(config_path: &Path) -> AppConfig {
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(config_path) {
            match serde_norway::from_str::<AppConfig>(&content) {
                Ok(config) => return config,
                Err(err) => {
                    tracing::error!(
                        name = "config.parse.failed",
                        path = %config_path.display(),
                        error = %err,
                        "Failed to parse config file. Using default configuration."
                    );
                }
            }
        } else {
            tracing::error!(
                name = "config.read.failed",
                path = %config_path.display(),
                "Failed to read config file from disk. Using default configuration. Likely cause: missing file, incorrect permissions, or disk error. Operator should ensure the file exists and is readable."
            );
        }
    } else {
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_config = AppConfig::default();
        if let Ok(yaml_str) = serde_norway::to_string(&default_config) {
            let _ = std::fs::write(config_path, yaml_str);
        }
    }
    AppConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.pdf_converter_command.is_none());
        assert!(!config.inline_editor_enabled);
    }

    #[test]
    fn test_load_config_creates_default_when_missing() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");

        // No global env mutation: pass the path explicitly so this test
        // cannot race with any other test that also touches APPDATA.
        let _config = load_config_from_path(&config_path);

        // Config file should have been created
        assert!(config_path.exists());
    }

    #[test]
    fn test_llm_config_defaults() {
        let cfg = LlmConfig {
            model: "test".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: None,
            use_case: default_use_case(),
        };
        assert_eq!(cfg.get_cost(), 0);
        assert!(cfg.has_use_case("chat"));
        assert!(!cfg.has_use_case("vision"));
    }

    #[test]
    fn test_llm_config_has_use_case() {
        let cfg = LlmConfig {
            model: "multi".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(5),
            use_case: vec!["chat".to_string(), "vision".to_string()],
        };
        assert!(cfg.has_use_case("chat"));
        assert!(cfg.has_use_case("vision"));
        assert!(!cfg.has_use_case("embeddings"));
        assert_eq!(cfg.get_cost(), 5);
    }

    #[test]
    fn test_model_for_use_case_returns_lowest_cost() {
        let mut config = AppConfig::default();
        config.models.insert(
            "expensive".to_string(),
            LlmConfig {
                model: "expensive-model".to_string(),
                api_url: "http://a".to_string(),
                api_key: "k".to_string(),
                cost: Some(10),
                use_case: vec!["chat".to_string()],
            },
        );
        config.models.insert(
            "cheap".to_string(),
            LlmConfig {
                model: "cheap-model".to_string(),
                api_url: "http://b".to_string(),
                api_key: "k".to_string(),
                cost: Some(1),
                use_case: vec!["chat".to_string()],
            },
        );
        let (key, _cfg) = config.model_for_use_case("chat").unwrap();
        assert_eq!(key, "cheap");
    }

    #[test]
    fn test_model_for_use_case_none_when_no_match() {
        let mut config = AppConfig::default();
        config.models.insert(
            "chat_only".to_string(),
            LlmConfig {
                model: "chat-model".to_string(),
                api_url: "http://a".to_string(),
                api_key: "k".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );
        assert!(config.model_for_use_case("vision").is_none());
    }

    #[test]
    fn test_model_for_use_case_vision() {
        let mut config = AppConfig::default();
        config.models.insert(
            "vision_model".to_string(),
            LlmConfig {
                model: "gpt-4o".to_string(),
                api_url: "http://a".to_string(),
                api_key: "k".to_string(),
                cost: Some(5),
                use_case: vec!["chat".to_string(), "vision".to_string()],
            },
        );
        let (key, _cfg) = config.model_for_use_case("vision").unwrap();
        assert_eq!(key, "vision_model");
    }

    #[test]
    fn test_validate_valid_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_empty());
    }

    #[test]
    fn test_validate_unknown_use_case() {
        let mut config = AppConfig::default();
        config.models.insert(
            "bad".to_string(),
            LlmConfig {
                model: "bad".to_string(),
                api_url: "http://a".to_string(),
                api_key: "k".to_string(),
                cost: None,
                use_case: vec!["chat".to_string(), "invalid".to_string()],
            },
        );
        let warnings = config.validate();
        assert!(warnings.iter().any(|w| w.contains("unknown use_case")));
    }

    #[test]
    fn test_validate_no_chat_model() {
        let mut config = AppConfig::default();
        config.models.insert(
            "embed".to_string(),
            LlmConfig {
                model: "embed".to_string(),
                api_url: "http://a".to_string(),
                api_key: "k".to_string(),
                cost: None,
                use_case: vec!["embeddings".to_string()],
            },
        );
        let warnings = config.validate();
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("No model configured with 'chat'"))
        );
    }

    #[test]
    fn test_backward_compat_old_field_names() {
        let yaml = r#"
model: "test"
models:
  legacy_model:
    model: "old-model-name"
    api_url: "http://old-endpoint"
    api_key: "old-key"
    capabilities: "chat"
"#;
        let config: AppConfig = serde_norway::from_str(yaml).unwrap();
        let m = config.models.get("legacy_model").unwrap();
        // Old field names should deserialize without issues
        assert_eq!(m.model, "old-model-name");
        assert_eq!(m.api_url, "http://old-endpoint");
    }

    #[test]
    fn test_new_field_names() {
        let yaml = r#"
model: "test"
models:
  new_model:
    model: "new-model-name"
    api_url: "http://new-endpoint"
    api_key: "new-key"
    cost: 3
    use_case:
      - chat
      - vision
"#;
        let config: AppConfig = serde_norway::from_str(yaml).unwrap();
        let m = config.models.get("new_model").unwrap();
        assert_eq!(m.model, "new-model-name");
        assert_eq!(m.api_url, "http://new-endpoint");
        assert_eq!(m.get_cost(), 3);
        assert!(m.has_use_case("chat"));
        assert!(m.has_use_case("vision"));
    }

    #[test]
    fn test_config_with_pdf_converter() {
        let yaml = r#"
model: "test"
pdf_converter_command:
  - pandoc
  - "-f"
  - pdf
  - "-o"
  - "{output}"
  - "{input}"
inline_editor_enabled: true
"#;
        let config: AppConfig = serde_norway::from_str(yaml).unwrap();
        assert!(config.pdf_converter_command.is_some());
        let cmd = config.pdf_converter_command.unwrap();
        assert_eq!(cmd[0], "pandoc");
        assert!(config.inline_editor_enabled);
    }

    #[test]
    fn test_load_config_valid_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let yaml = r#"
user_name: "TestUser"
"#;
        std::fs::write(&config_path, yaml).unwrap();

        let config = load_config_from_path(&config_path);
        assert_eq!(config.user_name, Some("TestUser".to_string()));
    }

    #[test]
    fn test_load_config_invalid_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, "invalid: yaml: [").unwrap();

        let config = load_config_from_path(&config_path);
        // Should return default
        assert!(config.user_name.is_none());
    }

    #[test]
    fn test_debug_impls() {
        let j = JmapClient {
            url: "a".into(),
            token: "b".into(),
        };
        let s = format!("{:?}", j);
        assert!(s.contains("[REDACTED]"));

        let c = CalDavClient {
            url: "a".into(),
            username: "u".into(),
            password: "p".into(),
        };
        let s = format!("{:?}", c);
        assert!(s.contains("[REDACTED]"));

        let l = LlmConfig {
            model: "m".into(),
            api_url: "a".into(),
            api_key: "k".into(),
            cost: None,
            use_case: vec![],
        };
        let s = format!("{:?}", l);
        assert!(s.contains("[REDACTED]"));

        let cfg = AppConfig::default();
        let s2 = format!("{:?}", cfg);
        assert!(s2.contains("AppConfig"));
    }

    /// Regression: previously the three `test_load_config_*` tests mutated
    /// the process-wide `APPDATA` env var, and `load_config()` read it
    /// back. Under the parallel test runner, another test could overwrite
    /// `APPDATA` between the `set_var` and the `load_config` call, so
    /// `test_load_config_invalid_file` would sometimes load a sibling
    /// test's valid YAML and see `user_name: Some("TestUser")` instead of
    /// the expected `None`.
    ///
    /// The refactor that moved the file I/O behind a path parameter
    /// removed the shared state; this test pins that property by loading
    /// N independent config files from N threads simultaneously and
    /// asserting every thread sees its own data.
    #[test]
    fn test_load_config_from_path_is_isolated() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        const N: usize = 16;
        let dir = tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(N));

        let handles: Vec<_> = (0..N)
            .map(|i| {
                let dir = dir.path().to_path_buf();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let config_path = dir.join(format!("cfg_{i}.yaml"));
                    let yaml = format!("user_name: \"User{i}\"\n");
                    std::fs::write(&config_path, yaml).unwrap();

                    // Maximise the race window: every worker waits here
                    // so all N `load_config_from_path` calls happen
                    // concurrently.
                    barrier.wait();

                    let config = load_config_from_path(&config_path);
                    assert_eq!(
                        config.user_name,
                        Some(format!("User{i}")),
                        "thread {i} saw the wrong config (cross-talk between loads)"
                    );
                })
            })
            .collect();

        for h in handles {
            h.join().expect("worker thread panicked");
        }
    }

    #[test]
    fn test_tool_groups_config_defaults() {
        let yaml = "{}";
        let cfg: ToolGroupsConfig = serde_norway::from_str(yaml).unwrap();
        assert!(cfg.filesystem);
        assert!(cfg.web);
        assert!(cfg.email);
        assert!(cfg.contacts);
        assert!(cfg.calendar);
        assert!(cfg.csv_db);
        assert!(cfg.weather);
        // Browser defaults to OFF (opt-in, BRWS-CONF-001).
        assert!(!cfg.browser);

        let default_cfg = ToolGroupsConfig::default();
        assert_eq!(cfg, default_cfg);
    }

    #[test]
    fn test_browser_config_defaults() {
        let yaml = "{}";
        let cfg: BrowserConfig = serde_norway::from_str(yaml).unwrap();
        assert!(cfg.headless);
        assert_eq!(cfg.browser_type, "firefox");
        assert_eq!(cfg.idle_timeout_seconds, 300);
        assert_eq!(cfg.page_load_timeout_ms, 30_000);
        assert!(cfg.screenshot_dir.is_empty());
        assert!(cfg.storage_state_path.is_empty());
    }

    #[test]
    fn test_browser_config_resolve_fills_defaults() {
        let cfg = BrowserConfig::default();
        let resolved = cfg.resolve(&[]);
        assert!(resolved.headless);
        assert_eq!(resolved.browser_type, "firefox");
        assert_eq!(resolved.idle_timeout_seconds, 300);
        // The screenshot_dir and storage_state_path come from the
        // environment, so we only check that they are non-empty.
        assert!(!resolved.screenshot_dir.as_os_str().is_empty());
        assert!(!resolved.storage_state_path.as_os_str().is_empty());
    }

    #[test]
    fn test_browser_config_resolve_uses_content_library() {
        let dir = tempdir().unwrap();
        let cfg = BrowserConfig::default();
        let libs = vec![ContentLibrary {
            root_folder: dir.path().to_string_lossy().to_string(),
            name: "MyLib".to_string(),
            kind: "markdown".to_string(),
            readonly: false,
            priority: 0,
        }];
        let resolved = cfg.resolve(&libs);
        assert_eq!(
            resolved.screenshot_dir,
            dir.path().join("browser-screenshots")
        );
    }

    #[test]
    fn test_mcp_server_config_deserialization_and_debug_redaction() {
        let stdio_yaml = r#"
transport: stdio
command: npx
args: ["-y", "@modelcontextprotocol/server-memory"]
env:
  API_KEY: "secret"
"#;
        let stdio_cfg: McpServerConfig = serde_norway::from_str(stdio_yaml).unwrap();
        let debug_stdio = format!("{:?}", stdio_cfg);
        assert!(debug_stdio.contains("McpServerConfig::Stdio"));
        assert!(debug_stdio.contains("npx"));

        let sse_yaml = r#"
transport: sse
url: https://mcp.example.com/sse
headers:
  Authorization: Bearer supersecrettoken
"#;
        let sse_cfg: McpServerConfig = serde_norway::from_str(sse_yaml).unwrap();
        let debug_sse = format!("{:?}", sse_cfg);
        assert!(debug_sse.contains("McpServerConfig::Sse"));
        assert!(debug_sse.contains("[REDACTED]"));
        assert!(!debug_sse.contains("supersecrettoken"));
    }

    #[test]
    fn deficit_strategy_default_is_waterfill() {
        let config = AppConfig::default();
        assert_eq!(
            config.deficit_strategy(),
            crate::ui::table_width::DeficitStrategy::BreakpointWaterFill
        );
    }

    // ---- McpServerEntry wrapper (CONFIG-012) ----

    #[test]
    fn test_mcp_server_entry_defaults_enabled_to_true() {
        // Legacy YAML without the `enabled` key must parse as enabled.
        let yaml = r#"
transport: stdio
command: npx
"#;
        let entry: McpServerEntry = serde_norway::from_str(yaml).unwrap();
        assert!(entry.is_enabled(), "legacy YAML must default to enabled");
        match entry.config() {
            McpServerConfig::Stdio { command, .. } => assert_eq!(command, "npx"),
            other => panic!("expected Stdio, got {other:?}"),
        }
    }

    #[test]
    fn test_mcp_server_entry_round_trips_enabled_false() {
        let yaml = r#"
enabled: false
transport: sse
url: https://mcp.example.com/sse
headers: {}
"#;
        let entry: McpServerEntry = serde_norway::from_str(yaml).unwrap();
        assert!(!entry.is_enabled());
        match entry.config() {
            McpServerConfig::Sse { url, .. } => assert_eq!(url, "https://mcp.example.com/sse"),
            other => panic!("expected Sse, got {other:?}"),
        }
        // Round-trip: re-serialise and re-parse, expect same shape.
        let serialised = serde_norway::to_string(&entry).unwrap();
        let re: McpServerEntry = serde_norway::from_str(&serialised).unwrap();
        assert!(!re.is_enabled());
    }

    #[test]
    fn test_mcp_server_entry_from_config_defaults_enabled_true() {
        let entry: McpServerEntry = McpServerConfig::Stdio {
            command: "echo".to_string(),
            args: vec![],
            env: HashMap::new(),
        }
        .into();
        assert!(entry.is_enabled());
    }

    #[test]
    fn test_mcp_server_entry_legacy_yaml_parses() {
        // Legacy YAML without `needs_auth` must parse correctly.
        let yaml = r#"
transport: stdio
command: npx
"#;
        let entry: McpServerEntry = serde_norway::from_str(yaml).unwrap();
        assert!(entry.enabled);
    }

    #[test]
    fn test_mcp_server_entry_redacts_headers_in_debug() {
        // The wrapper inherits the redaction contract from
        // `McpServerConfig::Debug`. Regression guard: a future refactor
        // that adds a new field to `McpServerEntry` must not leak
        // header values via Debug.
        let entry: McpServerEntry = McpServerConfig::Sse {
            url: "https://mcp.example.com/sse".to_string(),
            headers: [(
                "Authorization".to_string(),
                "Bearer supersecrettoken".to_string(),
            )]
            .into_iter()
            .collect(),
            oauth: None,
        }
        .into();
        let debug = format!("{entry:?}");
        assert!(debug.contains("[REDACTED]"), "Debug must redact headers");
        assert!(
            !debug.contains("supersecrettoken"),
            "Debug must not leak tokens"
        );
    }

    #[test]
    fn deficit_strategy_respects_config_value() {
        let config = AppConfig {
            table_width_strategy: "proportional".to_string(),
            ..AppConfig::default()
        };
        assert_eq!(
            config.deficit_strategy(),
            crate::ui::table_width::DeficitStrategy::ProportionalToSlack
        );
    }
}
