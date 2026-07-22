use std::path::{Path, PathBuf};

use std::collections::HashMap;

pub mod virtual_path;
pub use virtual_path::{VirtualPath, VirtualPathError};

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

impl ContentLibrary {
    /// Purpose: Compute the user-facing display label for an absolute path under this library.
    /// Inputs: `path` (the absolute path to localize).
    /// Outputs: `Some(label)` when `path` lives inside `self.root_folder`, otherwise `None`.
    /// Purity: Pure function.
    /// Preconditions: `self.root_folder` should be an absolute path.
    /// Postconditions: The returned label uses `/` separators and `self.name` as the root segment; trailing separators from `Path::join` are trimmed.
    pub fn display_label_for(&self, path: &Path) -> Option<String> {
        let root = Path::new(&self.root_folder);
        let rel = path.strip_prefix(root).ok()?;
        let joined = Path::new(&self.name).join(rel);
        let mut label = joined.to_string_lossy().into_owned();
        if label.ends_with('\\') || label.ends_with('/') {
            label.pop();
        }
        Some(label)
    }

    /// Check if the given absolute path is inside this library's root folder.
    pub fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(&self.root_folder)
    }

    /// Resolve a sub-path relative to this library's root folder.
    pub fn resolve(&self, sub: &Path) -> PathBuf {
        PathBuf::from(&self.root_folder).join(sub)
    }

    /// Returns `true` if this library allows writes.
    pub fn is_writable(&self) -> bool {
        !self.readonly
    }

    /// Returns the root folder as a `PathBuf`.
    pub fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.root_folder)
    }
}

/// Purpose: Find the first library that contains the given absolute path and return its display label.
/// Inputs: `libraries` (slice of registered content libraries), `path` (the absolute path to localize).
/// Outputs: `Some(label)` if any library contains `path`, otherwise `None`.
/// Purity: Pure function.
/// Preconditions: Each library's `root_folder` should be an absolute path.
/// Postconditions: Returns `None` if no library matches.
pub fn library_display_label(libraries: &[ContentLibrary], path: &Path) -> Option<String> {
    libraries.iter().find_map(|lib| lib.display_label_for(path))
}

fn default_readonly() -> bool {
    true
}

fn default_feature_flags() -> HashMap<String, bool> {
    let mut m = HashMap::new();
    // Default to CardDAV for contact operations. Some JMAP providers (notably
    // Fastmail) don't expose the contact data type via JMAP and return
    // `unknownMethod: Unknown object 'JMAPApp::DataType::Contact'`. CardDAV
    // has the same contact data with broader provider support, so it's the
    // safer default. Set this to `false` in your config to opt back into JMAP.
    m.insert("useDAVForContacts".to_string(), true);
    // When enabled, tool call responses include full data in logs (may be verbose).
    // When disabled (default), only basic success/failure is logged for privacy.
    m.insert("toolCallDebugMode".to_string(), false);
    m
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
    /// PDF converter command template (REQ-604b).
    #[serde(default)]
    pub pdf_converter_command: Option<Vec<String>>,
    /// Enable built-in inline text editor (REQ-250). Default: false.
    #[serde(default)]
    pub inline_editor_enabled: bool,
    /// Override default storage location for CSV databases.
    #[serde(default)]
    pub csv_db_path: Option<String>,
    /// Runtime feature flags. Map of feature name to enabled/disabled.
    #[serde(default = "default_feature_flags")]
    pub feature_flags: HashMap<String, bool>,
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
        }
    }
}

impl AppConfig {
    /// Find the best model for a given use_case (lowest cost among matches).
    pub fn model_for_use_case(&self, use_case: impl AsRef<str>) -> Option<(&String, &LlmConfig)> {
        let uc_ref = use_case.as_ref();
        self.models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(uc_ref))
            .min_by_key(|(_, cfg)| cfg.get_cost())
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

pub fn load_config() -> AppConfig {
    let config_path = get_config_path();
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            match serde_yaml::from_str::<AppConfig>(&content) {
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
        if let Ok(yaml_str) = serde_yaml::to_string(&default_config) {
            let _ = std::fs::write(&config_path, yaml_str);
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
        // CardDAV is the default for contact operations because some JMAP
        // providers (notably Fastmail) don't expose the Contact data type.
        // This makes the registry's contact tools route to CardDAV out of
        // the box.
        assert_eq!(
            config.feature_flags.get("useDAVForContacts").copied(),
            Some(true),
            "useDAVForContacts must default to true so contact lookups don't fail on JMAP servers without a Contact data type"
        );
    }

    #[test]
    fn test_load_config_creates_default_when_missing() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");

        // Temporarily override the config path
        unsafe {
            std::env::set_var("APPDATA", dir.path());
        }

        let _config = load_config();

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
        assert!(warnings
            .iter()
            .any(|w| w.contains("No model configured with 'chat'")));
    }

    #[test]
    fn test_validate_missing_active_model() {
        // Test removed as active model is now deprecated.
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
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
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

        unsafe {
            std::env::set_var("APPDATA", dir.path());
        }

        let config = load_config();
        assert_eq!(config.user_name, Some("TestUser".to_string()));
    }

    #[test]
    fn test_content_library_contains_path_inside() {
        let lib = ContentLibrary {
            root_folder: "C:/lib/one".to_string(),
            name: "One".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        };
        assert!(lib.contains_path(Path::new("C:/lib/one/sub/file.md")));
        assert!(lib.contains_path(Path::new("C:/lib/one")));
    }

    #[test]
    fn test_content_library_contains_path_outside() {
        let lib = ContentLibrary {
            root_folder: "C:/lib/one".to_string(),
            name: "One".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        };
        assert!(!lib.contains_path(Path::new("C:/lib/two/file.md")));
        assert!(!lib.contains_path(Path::new("D:/other/path.md")));
    }

    #[test]
    fn test_content_library_resolve() {
        let lib = ContentLibrary {
            root_folder: "C:/base".to_string(),
            name: "Base".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        };
        assert_eq!(
            lib.resolve(Path::new("sub/file.md")),
            PathBuf::from("C:/base/sub/file.md")
        );
        assert_eq!(lib.resolve(Path::new("")), PathBuf::from("C:/base"));
    }

    #[test]
    fn test_content_library_is_writable() {
        let writable = ContentLibrary {
            root_folder: "C:/w".to_string(),
            name: "W".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        };
        assert!(writable.is_writable());

        let readonly = ContentLibrary {
            root_folder: "C:/r".to_string(),
            name: "R".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        };
        assert!(!readonly.is_writable());
    }

    #[test]
    fn test_content_library_root_path() {
        let lib = ContentLibrary {
            root_folder: "C:/my/library".to_string(),
            name: "Test".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        };
        assert_eq!(lib.root_path(), PathBuf::from("C:/my/library"));
    }

    #[test]
    fn test_content_library_display_label_for_member() {
        let lib = ContentLibrary {
            root_folder: "C:/my/test/dir".to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        };
        let expected = PathBuf::from("TestLib").join("sub/file.md");
        let actual = lib
            .display_label_for(Path::new("C:/my/test/dir/sub/file.md"))
            .expect("path is inside library");
        assert_eq!(actual, expected.to_string_lossy());
        assert_eq!(
            lib.display_label_for(Path::new("C:/my/test/dir")),
            Some("TestLib".to_string())
        );
        assert!(lib
            .display_label_for(Path::new("C:/other/path.md"))
            .is_none());
    }

    #[test]
    fn test_library_display_label_finds_first_match() {
        let libs = vec![
            ContentLibrary {
                root_folder: "C:/lib/one".to_string(),
                name: "One".to_string(),
                kind: "text".to_string(),
                readonly: true,
                priority: 0,
            },
            ContentLibrary {
                root_folder: "C:/lib/two".to_string(),
                name: "Two".to_string(),
                kind: "text".to_string(),
                readonly: true,
                priority: 0,
            },
        ];
        let expected = PathBuf::from("Two").join("note.md");
        let actual = library_display_label(&libs, Path::new("C:/lib/two/note.md"))
            .expect("path is inside a library");
        assert_eq!(actual, expected.to_string_lossy());
        assert!(library_display_label(&libs, Path::new("C:/other/note.md")).is_none());
    }

    #[test]
    fn test_load_config_invalid_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, "invalid: yaml: [").unwrap();

        unsafe {
            std::env::set_var("APPDATA", dir.path());
        }

        let config = load_config();
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
}
