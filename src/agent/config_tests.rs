//! Unit tests for the `AgentConfig` builder and defaults.

use super::*;
use crate::config::{LlmConfig, McpServerConfig, McpServerEntry, ToolGroupsConfig};
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
fn test_builder_new_defaults() {
    let builder = AgentConfigBuilder::new();
    let agent_cfg = builder.build();
    assert_eq!(agent_cfg.max_tokens(), 32768);
    assert_eq!(*agent_cfg.tool_groups(), ToolGroupsConfig::default());
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
fn test_make_test_agent_config_has_one_model() {
    let agent_cfg = make_test_agent_config();
    assert_eq!(agent_cfg.models().len(), 1);
    assert!(agent_cfg.models().contains_key("test"));
}

#[test]
fn test_builder_with_selected_chat_model_round_trip() {
    let agent_cfg = AgentConfigBuilder::new()
        .with_selected_chat_model(Some("custom-model".to_string()))
        .build();
    assert_eq!(agent_cfg.selected_chat_model(), Some("custom-model"));
}

#[test]
fn test_agent_config_select_chat_model_explicit() {
    let mut models = HashMap::new();
    models.insert(
        "cheap".to_string(),
        LlmConfig {
            model: "cheap-model".to_string(),
            api_url: "http://api".to_string(),
            api_key: "k".to_string(),
            cost: Some(1),
            use_case: vec!["chat".to_string()],
        },
    );
    models.insert(
        "expensive".to_string(),
        LlmConfig {
            model: "expensive-model".to_string(),
            api_url: "http://api".to_string(),
            api_key: "k".to_string(),
            cost: Some(100),
            use_case: vec!["chat".to_string()],
        },
    );

    let default_cfg = AgentConfigBuilder::new()
        .with_models(models.clone())
        .build();
    assert_eq!(
        default_cfg.select_chat_model().unwrap().model,
        "cheap-model"
    );

    let override_cfg = AgentConfigBuilder::new()
        .with_models(models)
        .with_selected_chat_model(Some("expensive".to_string()))
        .build();
    assert_eq!(
        override_cfg.select_chat_model().unwrap().model,
        "expensive-model"
    );
}
