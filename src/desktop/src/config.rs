use std::path::PathBuf;

use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct JmapClient {
    pub url: String,
    pub token: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CalDavClient {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LlmConfig {
    pub model: String,
    pub api_url: String,
    pub api_key: String,
    #[serde(default)]
    pub cost: Option<i32>,
    #[serde(default)]
    pub capabilities: Option<String>,
}

impl LlmConfig {
    pub fn get_cost(&self) -> i32 {
        self.cost.unwrap_or(1)
    }
    pub fn get_capabilities(&self) -> String {
        self.capabilities.clone().unwrap_or_else(|| "chat".to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AppConfig {
    pub api_url: String,
    pub model: String,
    pub api_key: String,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub user_address: Option<String>,
    #[serde(default)]
    pub user_age: Option<u32>,
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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_url: "https://openrouter.ai/api/v1".to_string(),
            model: "google/gemini-2.5-flash:free".to_string(),
            api_key: "your-api-key-here".to_string(),
            user_name: None,
            user_address: None,
            user_age: None,
            user_gender: None,
            system_prompt_extension: None,
            models: HashMap::new(),
            searxng_url: Some("http://localhost:8090".to_string()),
            jmap_clients: HashMap::new(),
            caldav_clients: HashMap::new(),
        }
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
            if let Ok(config) = serde_yaml::from_str::<AppConfig>(&content) {
                return config;
            } else {
                eprintln!("Failed to parse config file: {}, using defaults.", config_path.display());
            }
        } else {
            eprintln!("Failed to read config file: {}, using defaults.", config_path.display());
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
        assert_eq!(config.api_url, "https://openrouter.ai/api/v1");
        assert_eq!(config.model, "google/gemini-2.5-flash:free");
        assert_eq!(config.api_key, "your-api-key-here");
    }

    #[test]
    fn test_load_config_creates_default_when_missing() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("fastmd").join("config.yaml");
        
        // Temporarily override the config path
        std::env::set_var("APPDATA", dir.path());
        
        let config = load_config();
        assert_eq!(config.api_url, "https://openrouter.ai/api/v1");
        
        // Config file should have been created
        assert!(config_path.exists());
    }
}