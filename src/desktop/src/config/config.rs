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

pub mod bus;
pub use bus::{CONFIG_ARRIVAL_TIMEOUT, ConfigArrived, config_bus};

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
        #[serde(default)]
        headers: HashMap<String, String>,
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
            Self::Sse { url, headers } => {
                let redacted_headers: HashMap<_, _> = headers
                    .keys()
                    .map(|k| (k.clone(), "[REDACTED]".to_string()))
                    .collect();
                f.debug_struct("McpServerConfig::Sse")
                    .field("url", url)
                    .field("headers", &redacted_headers)
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
    /// Configured external MCP servers by server name.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    /// Table width algorithm for deficit regime. Default: "proportional".
    /// Options: "proportional" (fast, O(|S|)), "waterfill" (better G1, O(K log |S|)).
    #[serde(default = "default_table_width_strategy")]
    pub table_width_strategy: String,
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
            match serde_yml::from_str::<AppConfig>(&content) {
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
        if let Ok(yaml_str) = serde_yml::to_string(&default_config) {
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
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
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
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
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
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
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
        let cfg: ToolGroupsConfig = serde_yml::from_str(yaml).unwrap();
        assert!(cfg.filesystem);
        assert!(cfg.web);
        assert!(cfg.email);
        assert!(cfg.contacts);
        assert!(cfg.calendar);
        assert!(cfg.csv_db);
        assert!(cfg.weather);

        let default_cfg = ToolGroupsConfig::default();
        assert_eq!(cfg, default_cfg);
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
        let stdio_cfg: McpServerConfig = serde_yml::from_str(stdio_yaml).unwrap();
        let debug_stdio = format!("{:?}", stdio_cfg);
        assert!(debug_stdio.contains("McpServerConfig::Stdio"));
        assert!(debug_stdio.contains("npx"));

        let sse_yaml = r#"
transport: sse
url: https://mcp.example.com/sse
headers:
  Authorization: Bearer supersecrettoken
"#;
        let sse_cfg: McpServerConfig = serde_yml::from_str(sse_yaml).unwrap();
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
