//! Application configuration types and persistence — JMAP, CalDAV, CardDAV clients, content libraries, model settings.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (CONFIG-001..CONFIG-008; CONFIG-009 superseded by VFS-004/009) for the full specification.
//!
//! The VFS domain types ([`VirtualPath`], [`VirtualPathError`], and the
//! behaviour on [`ContentLibrary`]) now live under
//! [`crate::workspace::vfs`] and are re-exported here for backwards
//! compatibility — prefer importing from `crate::workspace::vfs` in new
//! code. See [`workspace/vfs/SPEC.md`](../workspace/vfs/SPEC.md).
//!
//! Unit tests live in the sibling `config_tests.rs` sidecar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use crate::agent::config::{
    BrowserConfig, CalDavClient, ContentLibrary, ContentLibraryExt, JmapClient, LlmConfig,
    McpOAuthConfig, McpServerConfig, McpServerEntry, ResolvedBrowserConfig, ToolGroupsConfig,
    TrelloClient, default_use_case,
};
pub use crate::workspace::vfs::{VirtualPath, VirtualPathError, library_display_label};

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
    "hybrid".to_string()
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
    /// Substrings blocked on both inbound (user) and outbound (LLM) content.
    /// Matching is case-insensitive. Empty by default.
    #[serde(default)]
    pub blocked_patterns: Vec<String>,
}

fn default_discord_history_len() -> usize {
    20
}

fn default_discord_rate_limit() -> u32 {
    10
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
    /// User-selected active chat model name (overrides auto cost selection).
    /// In-memory runtime state only; not persisted to configuration file.
    #[serde(skip)]
    pub selected_chat_model: Option<String>,
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
    /// Table width algorithm for deficit regime. Default: "hybrid".
    /// Options:
    /// * "proportional" — fast, O(|S|); FTWA v1.
    /// * "waterfill" — better G1, O(K log |S|); FTWA v2.
    /// * "ratio" — water-filling by `max_j / w_j` (doc §2.10).
    /// * "lagrange" — `Σ extraLines_j(w_j)` minimised via Lagrange bisection
    ///   on a per-column penalty (doc §2.13).
    /// * "hybrid" — min-content floor + per-column penalty + slack
    ///   water-fill (doc §2.14). Best G1/G2 trade-off on degenerate
    ///   cell distributions; recommended default.
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

    /// Optional Qdrant vector database URL (e.g. "http://localhost:6334").
    #[serde(default)]
    pub qdrant_url: Option<String>,

    /// Optional Qdrant API key for authenticated endpoints.
    #[serde(default)]
    pub qdrant_api_key: Option<String>,

    /// Optional Qdrant collection name (defaults to "fastmd_chunks").
    #[serde(default)]
    pub qdrant_collection: Option<String>,
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
            .field("selected_chat_model", &self.selected_chat_model)
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
            .field("qdrant_url", &self.qdrant_url)
            .field("qdrant_collection", &self.qdrant_collection)
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
            selected_chat_model: None,
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
            qdrant_url: None,
            qdrant_api_key: None,
            qdrant_collection: None,
        }
    }
}

impl AppConfig {
    /// Parse `table_width_strategy` into the enum used by the FTWA algorithm.
    /// Unknown values fall back to `DeficitStrategy::HybridMinPenaltyWaterFill`
    /// (the default set by `default_table_width_strategy` in `config.rs`).
    pub fn deficit_strategy(&self) -> crate::ui::table_width::DeficitStrategy {
        crate::ui::table_width::DeficitStrategy::from_config(&self.table_width_strategy)
    }

    /// Project the global `AppConfig` into the agent's domain slice.
    ///
    /// Resolves the browser config (filling env-dependent defaults) and
    /// captures the config path for the "API key not set" error message.
    pub fn to_agent_config(&self) -> crate::agent::config::AgentConfig {
        crate::agent::config::AgentConfigBuilder::new()
            .with_models(self.models.clone())
            .with_selected_chat_model(self.selected_chat_model.clone())
            .with_max_tokens(self.max_tokens)
            .with_tool_groups(self.tool_groups.clone())
            .with_mcp_servers(self.mcp_servers.clone())
            .with_browser(self.browser.resolve(&self.content_libraries))
            .with_config_path(get_config_path())
            .with_jmap_clients(self.jmap_clients.clone())
            .with_caldav_clients(self.caldav_clients.clone())
            .with_trello_client(self.trello_client.clone())
            .with_searxng_url(self.searxng_url.clone())
            .with_csv_db_path(self.csv_db_path.clone())
            .with_feature_flags(self.feature_flags.clone())
            .with_content_libraries(self.content_libraries.clone())
            .build()
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
        if candidates.is_empty() {
            return vec![];
        }
        let min_cost = candidates
            .iter()
            .map(|(_, cfg)| cfg.get_cost())
            .min()
            .unwrap();
        candidates
            .into_iter()
            .filter(|(_, cfg)| cfg.get_cost() == min_cost)
            .collect()
    }

    /// Determine the active chat model key: explicit `selected_chat_model` if valid,
    /// falling back to the lowest-cost model with "chat" use_case, then the first model.
    pub fn current_chat_model_key(&self) -> Option<String> {
        if let Some(selected) = &self.selected_chat_model
            && self.models.contains_key(selected)
        {
            return Some(selected.clone());
        }
        self.model_for_use_case("chat")
            .map(|(key, _)| key.clone())
            .or_else(|| self.models.keys().next().cloned())
    }

    /// Select a chat model, preferring the explicitly chosen `selected_chat_model`,
    /// then one configured for the "chat" use case, falling back to the first available model,
    /// and rejecting default/empty keys.
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `config_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
