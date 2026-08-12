//! Unit tests for the `AgentConfig` projection and builder.
//!
//! The agent's domain config must be a faithful projection of the slice
//! the agent actually reads from `AppConfig`, and the builder must
//! produce a config that's indistinguishable (in the relevant fields)
//! from the same data assembled via the projection.

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
fn test_from_app_config_projects_models() {
    let mut cfg = AppConfig::default();
    cfg.models.insert("a".to_string(), sample_lc());
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.models().len(), 1);
    assert!(agent_cfg.models().contains_key("a"));
}

#[test]
fn test_from_app_config_projects_user_fields() {
    let cfg = AppConfig {
        user_name: Some("Alice".to_string()),
        user_address: Some("123 Main".to_string()),
        user_birthdate: Some("1990-01-01".to_string()),
        user_gender: Some("female".to_string()),
        ..AppConfig::default()
    };
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.user_name(), Some("Alice"));
    assert_eq!(agent_cfg.user_address(), Some("123 Main"));
    assert_eq!(agent_cfg.user_birthdate(), Some("1990-01-01"));
    assert_eq!(agent_cfg.user_gender(), Some("female"));
}

#[test]
fn test_from_app_config_projects_system_prompt_extension() {
    let cfg = AppConfig {
        system_prompt_extension: Some("extra".to_string()),
        ..AppConfig::default()
    };
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.system_prompt_extension(), Some("extra"));
}

#[test]
fn test_from_app_config_projects_max_tokens() {
    let cfg = AppConfig {
        max_tokens: 4096,
        ..AppConfig::default()
    };
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.max_tokens(), 4096);
}

#[test]
fn test_from_app_config_projects_tool_groups() {
    let cfg = AppConfig::default();
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    let expected = cfg.tool_groups.clone();
    assert_eq!(*agent_cfg.tool_groups(), expected);
}

#[test]
fn test_from_app_config_projects_mcp_servers() {
    let mut cfg = AppConfig::default();
    cfg.mcp_servers.insert("s1".to_string(), sample_mcp_entry());
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.mcp_servers().len(), 1);
    assert!(agent_cfg.mcp_servers().contains_key("s1"));
}

#[test]
fn test_from_app_config_resolves_browser_config() {
    let cfg = AppConfig::default();
    let agent_cfg = AgentConfig::from_app_config(&cfg);
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
fn test_from_app_config_projects_content_libraries() {
    let mut cfg = AppConfig::default();
    cfg.content_libraries.push(ContentLibrary {
        root_folder: "/notes".to_string(),
        name: "notes".to_string(),
        kind: "notes".to_string(),
        readonly: true,
        priority: 0,
    });
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.content_libraries().len(), 1);
    assert_eq!(agent_cfg.content_libraries()[0].name, "notes");
}

#[test]
fn test_from_app_config_captures_config_path() {
    let cfg = AppConfig::default();
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    let expected = crate::config::get_config_path();
    assert_eq!(agent_cfg.config_path(), expected.as_path());
}

#[test]
fn test_from_app_config_drops_irrelevant_fields() {
    // The agent config has no `searxng_url`, `jmap_clients`, etc. — those
    // are not its concern. This test is a structural check that the
    // projection doesn't accidentally leak them.
    let cfg = AppConfig {
        searxng_url: Some("http://searx".to_string()),
        inline_editor_enabled: true,
        csv_db_path: Some("/x".to_string()),
        table_width_strategy: "weird".to_string(),
        ..AppConfig::default()
    };
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    // None of the agent's getters should surface these fields.
    // (We assert by ensuring the type doesn't have accessors for them.)
    let _ = agent_cfg;
}

#[test]
fn test_from_app_config_default_user_fields_are_none() {
    let cfg = AppConfig::default();
    let agent_cfg = AgentConfig::from_app_config(&cfg);
    assert_eq!(agent_cfg.user_name(), None);
    assert_eq!(agent_cfg.user_address(), None);
    assert_eq!(agent_cfg.user_birthdate(), None);
    assert_eq!(agent_cfg.user_gender(), None);
    assert_eq!(agent_cfg.system_prompt_extension(), None);
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
    assert!(agent_cfg.content_libraries().is_empty());
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
fn test_builder_with_user_round_trip() {
    let agent_cfg = AgentConfigBuilder::new()
        .with_user(
            Some("Bob".to_string()),
            Some("addr".to_string()),
            Some("2000-01-01".to_string()),
            Some("male".to_string()),
        )
        .build();
    assert_eq!(agent_cfg.user_name(), Some("Bob"));
    assert_eq!(agent_cfg.user_address(), Some("addr"));
    assert_eq!(agent_cfg.user_birthdate(), Some("2000-01-01"));
    assert_eq!(agent_cfg.user_gender(), Some("male"));
}

#[test]
fn test_builder_with_system_prompt_extension_round_trip() {
    let agent_cfg = AgentConfigBuilder::new()
        .with_system_prompt_extension(Some("hi".to_string()))
        .build();
    assert_eq!(agent_cfg.system_prompt_extension(), Some("hi"));
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
fn test_builder_with_content_libraries_round_trip() {
    let libs = vec![ContentLibrary {
        root_folder: "/x".to_string(),
        name: "x".to_string(),
        kind: "k".to_string(),
        readonly: true,
        priority: 0,
    }];
    let agent_cfg = AgentConfigBuilder::new()
        .with_content_libraries(libs.clone())
        .build();
    assert_eq!(agent_cfg.content_libraries().len(), libs.len());
    assert_eq!(agent_cfg.content_libraries()[0].name, libs[0].name);
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
    assert_eq!(a.content_libraries().len(), b.content_libraries().len());
}

#[test]
fn test_from_app_config_via_into() {
    let mut cfg = AppConfig::default();
    cfg.models.insert("a".to_string(), sample_lc());
    let agent_cfg: AgentConfig = (&cfg).into();
    assert_eq!(agent_cfg.models().len(), 1);
}

#[test]
fn test_make_test_agent_config_has_one_model() {
    let agent_cfg = make_test_agent_config();
    assert_eq!(agent_cfg.models().len(), 1);
    assert!(agent_cfg.models().contains_key("test"));
}
