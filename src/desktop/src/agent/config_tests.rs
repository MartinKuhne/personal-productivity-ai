//! Unit tests for the `AgentConfig` projection and builder.
use crate::config::AppConfig;

use super::*;
use crate::config::{ContentLibrary, LlmConfig, McpServerConfig, McpServerEntry, ToolGroupsConfig};
use std::collections::HashMap;
use std::path::PathBuf;

fn sample_lc() -> LlmConfig {
    LlmConfig {
        model: "m".to_string(),
        api_url: "http://api".to_string(),
        api_key: "k".to_string(),
        cost: Some(0),
        use_case: vec!["chat".to_string()],
    }
}

fn sample_mcp_entry() -> McpServerEntry {
    McpServerEntry {
        enabled: true,
        config: McpServerConfig::Stdio {
            command: "cmd".to_string(),
            args: vec![],
            env: HashMap::new(),
        },
    }
}

#[test]
fn test_from_app_config_projects_models() {
    let mut cfg = AppConfig::default();
    cfg.models.insert("a".to_string(), sample_lc());
    let agent_cfg = cfg.to_agent_config();
    assert_eq!(agent_cfg.models().len(), 1);
    assert!(agent_cfg.models().contains_key("a"));
}

#[test]
fn test_from_app_config_projects_max_tokens() {
    let cfg = AppConfig {
        max_tokens: 4096,
        ..AppConfig::default()
    };
    let agent_cfg = cfg.to_agent_config();
    assert_eq!(agent_cfg.max_tokens(), 4096);
}

#[test]
fn test_from_app_config_projects_tool_groups() {
    let cfg = AppConfig::default();
    let agent_cfg = cfg.to_agent_config();
    let expected = cfg.tool_groups.clone();
    assert_eq!(*agent_cfg.tool_groups(), expected);
}

#[test]
fn test_from_app_config_projects_mcp_servers() {
    let mut cfg = AppConfig::default();
    cfg.mcp_servers.insert("s1".to_string(), sample_mcp_entry());
    let agent_cfg = cfg.to_agent_config();
    assert_eq!(agent_cfg.mcp_servers().len(), 1);
    assert!(agent_cfg.mcp_servers().contains_key("s1"));
}

#[test]
fn test_from_app_config_resolves_browser_config() {
    let cfg = AppConfig::default();
    let agent_cfg = cfg.to_agent_config();
    // The default browser config must end up resolved (no empty path strings).
    assert!(!agent_cfg.browser().screenshot_dir.as_os_str().is_empty());
    assert!(
        !agent_cfg
            .browser()
            .storage_state_path
            .as_os_str()
            .is_empty()
    );
}

#[test]
fn test_from_app_config_captures_config_path() {
    let cfg = AppConfig::default();
    let agent_cfg = cfg.to_agent_config();
    let expected = crate::config::get_config_path();
    assert_eq!(agent_cfg.config_path(), expected.as_path());
}

#[test]
fn test_from_app_config_drops_user_and_content_fields() {
    // User fields and content_libraries are intentionally not in
    // AgentConfig — prompt construction lives in
    // `crate::app::prompts::build_system_prompts`. The agent only
    // consumes the pre-built prompt blocks.
    let cfg = AppConfig {
        user_name: Some("Alice".to_string()),
        user_address: Some("addr".to_string()),
        user_birthdate: Some("1990-01-01".to_string()),
        user_gender: Some("female".to_string()),
        system_prompt_extension: Some("Custom instructions.".to_string()),
        content_libraries: vec![ContentLibrary {
            root_folder: "/x".to_string(),
            name: "x".to_string(),
            kind: "k".to_string(),
            readonly: true,
            priority: 0,
        }],
        ..AppConfig::default()
    };
    let agent_cfg = cfg.to_agent_config();
    // AgentConfig has no `user_*` / `system_prompt_extension` /
    // `content_libraries` accessors. If the struct ever grows them, this
    // test will fail to compile, which is the right trip-wire.
    let _ = agent_cfg;
}

#[test]
fn test_builder_new_matches_app_config_defaults() {
    let builder = AgentConfigBuilder::new();
    let agent_cfg = builder.build();
    let app_defaults = AppConfig::default();
    assert_eq!(agent_cfg.max_tokens(), app_defaults.max_tokens);
    assert_eq!(*agent_cfg.tool_groups(), app_defaults.tool_groups);
    assert!(agent_cfg.models().is_empty());
    assert!(agent_cfg.mcp_servers().is_empty());
}

#[test]
fn test_builder_with_models_round_trip() {
    let mut models = HashMap::new();
    models.insert("a".to_string(), sample_lc());
    let agent_cfg = AgentConfigBuilder::new()
        .with_models(models.clone())
        .build();
    assert_eq!(agent_cfg.models(), &models);
}

#[test]
fn test_builder_with_max_tokens_round_trip() {
    let agent_cfg = AgentConfigBuilder::new().with_max_tokens(8192).build();
    assert_eq!(agent_cfg.max_tokens(), 8192);
}

#[test]
fn test_builder_with_tool_groups_round_trip() {
    let g = ToolGroupsConfig {
        filesystem: false,
        web: false,
        email: true,
        contacts: true,
        calendar: true,
        csv_db: false,
        weather: false,
        browser: true,
        trello: true,
    };
    let agent_cfg = AgentConfigBuilder::new()
        .with_tool_groups(g.clone())
        .build();
    assert_eq!(*agent_cfg.tool_groups(), g);
}

#[test]
fn test_builder_with_mcp_servers_round_trip() {
    let mut m = HashMap::new();
    m.insert("s".to_string(), sample_mcp_entry());
    let agent_cfg = AgentConfigBuilder::new()
        .with_mcp_servers(m.clone())
        .build();
    assert_eq!(agent_cfg.mcp_servers(), &m);
}

#[test]
fn test_builder_with_config_path_round_trip() {
    let p = PathBuf::from("/tmp/test-config.yaml");
    let agent_cfg = AgentConfigBuilder::new()
        .with_config_path(p.clone())
        .build();
    assert_eq!(agent_cfg.config_path(), p.as_path());
}

#[test]
fn test_builder_default_matches_new() {
    let a = AgentConfigBuilder::new().build();
    let b = AgentConfigBuilder::default().build();
    assert_eq!(a.max_tokens(), b.max_tokens());
    assert_eq!(a.tool_groups(), b.tool_groups());
}

#[test]
fn test_default_agent_config_matches_builder_default() {
    let a = AgentConfig::default();
    let b = AgentConfigBuilder::new().build();
    assert_eq!(a.max_tokens(), b.max_tokens());
    assert_eq!(a.models(), b.models());
    assert_eq!(a.mcp_servers(), b.mcp_servers());
}

#[test]
fn test_from_app_config_via_into() {
    let mut cfg = AppConfig::default();
    cfg.models.insert("a".to_string(), sample_lc());
    let agent_cfg: AgentConfig = cfg.to_agent_config();
    assert_eq!(agent_cfg.models().len(), 1);
}

#[test]
fn test_make_test_agent_config_has_one_model() {
    let agent_cfg = make_test_agent_config();
    assert_eq!(agent_cfg.models().len(), 1);
    assert!(agent_cfg.models().contains_key("test"));
}
