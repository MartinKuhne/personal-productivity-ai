//! Tests for `config/config.rs`.

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
    let _config = load_config_from_path(&config_path, Some(&dir.path().join("system")));

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
    let yaml = "system_prompt_extension: \"Test extension\"\n";
    std::fs::write(&config_path, yaml).unwrap();

    let config = load_config_from_path(&config_path, Some(&dir.path().join("system")));
    assert_eq!(
        config.system_prompt_extension,
        Some("Test extension".to_string())
    );
}

#[test]
fn test_load_config_invalid_file() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("fastmd").join("config.yaml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, "invalid: yaml: [").unwrap();

    let config = load_config_from_path(&config_path, Some(&dir.path().join("system")));
    // Should return default
    assert!(config.system_prompt_extension.is_none());
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

/// Regression: `load_config_from_path` takes the path as an argument
/// and never reads the process-wide `APPDATA` env var, so tests cannot
/// interfere with one another through shared environment state. This
/// test pins that isolation by loading N independent config files
/// from N threads simultaneously and asserting every thread sees its
/// own data.
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
                let yaml = format!("system_prompt_extension: \"Extension{i}\"\n");
                std::fs::write(&config_path, yaml).unwrap();

                // Maximise the race window: every worker waits here
                // so all N `load_config_from_path` calls happen
                // concurrently.
                barrier.wait();

                let config = load_config_from_path(&config_path, Some(&dir.join("system")));
                assert_eq!(
                    config.system_prompt_extension,
                    Some(format!("Extension{i}")),
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
fn deficit_strategy_default_is_hybrid() {
    // `default_table_width_strategy` in `config.rs` returns "hybrid",
    // and `DeficitStrategy::from_config`'s fallback is the same
    // variant, so a fresh `AppConfig::default()` resolves to
    // HybridMinPenaltyWaterFill. Kept in sync; if either side
    // changes, this test will fail loudly.
    let config = AppConfig::default();
    assert_eq!(
        config.deficit_strategy(),
        crate::markdown::table_width::DeficitStrategy::HybridMinPenaltyWaterFill
    );
    // Round-trip: to_config and from_config compose back to the same
    // variant, so a persisted config with the default value
    // deserialises to the same strategy.
    let s = crate::markdown::table_width::DeficitStrategy::HybridMinPenaltyWaterFill.to_config();
    assert_eq!(s, "hybrid");
    assert_eq!(
        crate::markdown::table_width::DeficitStrategy::from_config(s),
        crate::markdown::table_width::DeficitStrategy::HybridMinPenaltyWaterFill
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
        crate::markdown::table_width::DeficitStrategy::ProportionalToSlack
    );
}

#[test]
fn deficit_strategy_parses_survey_algorithm_strings() {
    // Each of the three survey algorithms (doc §2.10, §2.13, §2.14)
    // must parse from its canonical config string. This guards the
    // round-trip with the top-bar combobox.
    use crate::markdown::table_width::DeficitStrategy;
    for (raw, expected) in [
        ("ratio", DeficitStrategy::WaterFillRatio),
        ("lagrange", DeficitStrategy::LagrangePenalty),
        ("hybrid", DeficitStrategy::HybridMinPenaltyWaterFill),
        // Aliases for ergonomics — the combobox writes the canonical
        // form, but a user editing the YAML may use any of these.
        ("waterfill-ratio", DeficitStrategy::WaterFillRatio),
        ("lagrange-penalty", DeficitStrategy::LagrangePenalty),
        (
            "hybrid-min-penalty",
            DeficitStrategy::HybridMinPenaltyWaterFill,
        ),
    ] {
        let config = AppConfig {
            table_width_strategy: raw.to_string(),
            ..AppConfig::default()
        };
        assert_eq!(
            config.deficit_strategy(),
            expected,
            "config string {raw:?} must parse to {expected:?}"
        );
    }
}

#[test]
fn test_select_chat_model_preferred() {
    let mut config = AppConfig::default();
    config.models.insert(
        "other".to_string(),
        LlmConfig {
            model: "other".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(10),
            use_case: vec!["code".to_string()],
        },
    );
    config.models.insert(
        "chat_model".to_string(),
        LlmConfig {
            model: "chat-preferred".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(5),
            use_case: vec!["chat".to_string()],
        },
    );

    let selected = config.select_chat_model().unwrap();
    assert_eq!(selected.model, "chat-preferred");
}

#[test]
fn test_select_chat_model_fallback() {
    let mut config = AppConfig::default();
    config.models.insert(
        "only_model".to_string(),
        LlmConfig {
            model: "fallback-model".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(10),
            use_case: vec![], // no "chat"
        },
    );

    let selected = config.select_chat_model().unwrap();
    assert_eq!(selected.model, "fallback-model");
}

#[test]
fn test_select_chat_model_empty() {
    let config = AppConfig::default(); // models is empty by default
    let res = config.select_chat_model();
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("No LLM models"));
}

#[test]
fn test_select_chat_model_default_key_rejected() {
    let mut config = AppConfig::default();
    config.models.insert(
        "chat_model".to_string(),
        LlmConfig {
            model: "chat-model".to_string(),
            api_url: "http://a".to_string(),
            api_key: "your-api-key-here".to_string(),
            cost: Some(5),
            use_case: vec!["chat".to_string()],
        },
    );

    let res = config.select_chat_model();
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("API key not set"));

    config.models.get_mut("chat_model").unwrap().api_key = "".to_string();
    let res2 = config.select_chat_model();
    assert!(res2.is_err());
    assert!(res2.unwrap_err().contains("API key not set"));
}

#[test]
fn test_select_chat_model_explicit_override() {
    let mut config = AppConfig::default();
    config.models.insert(
        "model_a".to_string(),
        LlmConfig {
            model: "model-a".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(1),
            use_case: vec!["chat".to_string()],
        },
    );
    config.models.insert(
        "model_b".to_string(),
        LlmConfig {
            model: "model-b".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(10),
            use_case: vec!["chat".to_string()],
        },
    );

    // Default without explicit selection picks lowest cost (model_a)
    assert_eq!(config.current_chat_model_key().as_deref(), Some("model_a"));
    assert_eq!(config.select_chat_model().unwrap().model, "model-a");

    // Setting selected_chat_model overrides cost preference
    config.selected_chat_model = Some("model_b".to_string());
    assert_eq!(config.current_chat_model_key().as_deref(), Some("model_b"));
    assert_eq!(config.select_chat_model().unwrap().model, "model-b");
}

#[test]
fn test_from_app_config_projects_models() {
    let mut cfg = AppConfig::default();
    cfg.models.insert(
        "a".to_string(),
        LlmConfig {
            model: "m".to_string(),
            api_url: "http://api".to_string(),
            api_key: "k".to_string(),
            cost: Some(0),
            use_case: vec!["chat".to_string()],
        },
    );
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
    cfg.mcp_servers.insert(
        "s1".to_string(),
        McpServerEntry {
            enabled: true,
            config: McpServerConfig::Stdio {
                command: "cmd".to_string(),
                args: vec![],
                env: std::collections::HashMap::new(),
            },
        },
    );
    let agent_cfg = cfg.to_agent_config();
    assert_eq!(agent_cfg.mcp_servers().len(), 1);
    assert!(agent_cfg.mcp_servers().contains_key("s1"));
}

#[test]
fn test_from_app_config_resolves_browser_config() {
    let cfg = AppConfig::default();
    let agent_cfg = cfg.to_agent_config();
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
    let cfg = AppConfig {
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
    let _ = agent_cfg;
}

#[test]
fn test_selected_chat_model_is_not_persisted() {
    let config = AppConfig {
        selected_chat_model: Some("test-model".to_string()),
        ..Default::default()
    };

    // Serialization should omit selected_chat_model
    let serialized = serde_norway::to_string(&config).expect("serialization should succeed");
    assert!(
        !serialized.contains("selected_chat_model"),
        "selected_chat_model must not be present in serialized config: {serialized}"
    );

    // Deserialization should ignore selected_chat_model if present in YAML
    let yaml_with_selected = "selected_chat_model: test-model\nmax_tokens: 4096\n";
    let deserialized: AppConfig =
        serde_norway::from_str(yaml_with_selected).expect("deserialization should succeed");
    assert_eq!(
        deserialized.selected_chat_model, None,
        "selected_chat_model must not be loaded from configuration file"
    );
}

#[test]
fn test_chat_model_switching_runtime_lifecycle() {
    let mut config = AppConfig::default();
    config.models.insert(
        "cheap".to_string(),
        LlmConfig {
            model: "cheap-model".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(1),
            use_case: vec!["chat".to_string()],
        },
    );
    config.models.insert(
        "expensive".to_string(),
        LlmConfig {
            model: "expensive-model".to_string(),
            api_url: "http://a".to_string(),
            api_key: "valid-key".to_string(),
            cost: Some(100),
            use_case: vec!["chat".to_string()],
        },
    );

    // Initial state: default picks lowest cost (cheap)
    assert_eq!(config.current_chat_model_key().as_deref(), Some("cheap"));
    assert_eq!(config.select_chat_model().unwrap().model, "cheap-model");
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.selected_chat_model(), None);
    assert_eq!(agent_cfg.select_chat_model().unwrap().model, "cheap-model");

    // Switch to expensive model at runtime
    config.selected_chat_model = Some("expensive".to_string());
    assert_eq!(
        config.current_chat_model_key().as_deref(),
        Some("expensive")
    );
    assert_eq!(config.select_chat_model().unwrap().model, "expensive-model");
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.selected_chat_model(), Some("expensive"));
    assert_eq!(
        agent_cfg.select_chat_model().unwrap().model,
        "expensive-model"
    );

    // Switch back to cheap model at runtime
    config.selected_chat_model = Some("cheap".to_string());
    assert_eq!(config.current_chat_model_key().as_deref(), Some("cheap"));
    assert_eq!(config.select_chat_model().unwrap().model, "cheap-model");
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.selected_chat_model(), Some("cheap"));
    assert_eq!(agent_cfg.select_chat_model().unwrap().model, "cheap-model");

    // Reset selection to None (auto selection)
    config.selected_chat_model = None;
    assert_eq!(config.current_chat_model_key().as_deref(), Some("cheap"));
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.selected_chat_model(), None);
    assert_eq!(agent_cfg.select_chat_model().unwrap().model, "cheap-model");
}

#[test]
fn test_system_library_default_name() {
    let config = AppConfig::default();
    assert_eq!(config.system_library_name, None);
    assert_eq!(config.system_library_display_name(), "System");
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.system_library_name(), None);
    assert_eq!(agent_cfg.system_library_display_name(), "System");
}

#[test]
fn test_system_library_custom_name() {
    let config = AppConfig {
        system_library_name: Some("Personal Knowledge".to_string()),
        ..Default::default()
    };
    assert_eq!(config.system_library_display_name(), "Personal Knowledge");
    let agent_cfg = config.to_agent_config();
    assert_eq!(agent_cfg.system_library_name(), Some("Personal Knowledge"));
    assert_eq!(
        agent_cfg.system_library_display_name(),
        "Personal Knowledge"
    );
}

#[test]
fn test_system_library_yaml_roundtrip() {
    let yaml = r#"
system_library_name: "Knowledge Base"
"#;
    let config: AppConfig = serde_norway::from_str(yaml).unwrap();
    assert_eq!(
        config.system_library_name.as_deref(),
        Some("Knowledge Base")
    );
    assert_eq!(config.system_library_display_name(), "Knowledge Base");
}

#[test]
fn test_system_library_dir_creation() {
    let dir = tempdir().unwrap();
    let sys_dir = AppConfig::ensure_system_library_dir_at(dir.path()).unwrap();
    assert!(sys_dir.exists());
    assert!(sys_dir.is_dir());

    let conv_dir = AppConfig::ensure_conversations_dir_at(dir.path()).unwrap();
    assert!(conv_dir.exists());
    assert!(conv_dir.is_dir());
    assert_eq!(conv_dir, sys_dir.join("Conversations"));
}

#[test]
fn test_ensure_system_library_present_at() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let mut config = AppConfig::default();
    config.ensure_system_library_present_at(&sys_path);

    assert_eq!(config.content_libraries.len(), 1);
    let lib = &config.content_libraries[0];
    assert_eq!(lib.name, "System");
    assert_eq!(lib.kind, "text");
    assert!(!lib.readonly);
    assert!(sys_path.exists());
    assert!(sys_path.join("Conversations").exists());

    // Calling again with custom name should update the existing library name
    config.system_library_name = Some("Custom System".to_string());
    config.ensure_system_library_present_at(&sys_path);
    assert_eq!(config.content_libraries.len(), 1);
    assert_eq!(config.content_libraries[0].name, "Custom System");
}

#[test]
fn test_ensure_system_library_does_not_repoint_unrelated_system_name() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let unrelated_path = dir.path().join("unrelated");
    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(crate::config::ContentLibrary {
            root_folder: unrelated_path.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 3,
        });

    config.ensure_system_library_present_at(&sys_path);

    assert_eq!(config.content_libraries.len(), 2);
    assert_eq!(config.content_libraries[0].name, "System");
    assert_eq!(
        config.content_libraries[0].root_folder,
        sys_path.to_string_lossy()
    );
    assert!(config.content_libraries[1].readonly);
    assert_eq!(
        config.content_libraries[1].root_folder,
        unrelated_path.to_string_lossy()
    );
}

#[test]
fn test_ensure_system_library_repairs_readonly_root_match() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(crate::config::ContentLibrary {
            root_folder: sys_path.to_string_lossy().to_string(),
            name: "Old System".to_string(),
            kind: "markdown".to_string(),
            readonly: true,
            priority: 4,
        });

    config.ensure_system_library_present_at(&sys_path);

    let lib = &config.content_libraries[0];
    assert_eq!(lib.name, "System");
    assert_eq!(lib.kind, "text");
    assert!(!lib.readonly);
    assert_eq!(lib.priority, 0);
}

#[test]
fn test_skills_directories_creation() {
    let dir = tempdir().unwrap();
    let skills_dir = AppConfig::ensure_skills_dirs_at(dir.path()).unwrap();
    assert!(skills_dir.exists());
    assert!(skills_dir.join("Note").exists());
    assert!(skills_dir.join("Folder").exists());
    assert!(skills_dir.join("Batch").exists());

    assert_eq!(
        AppConfig::get_skills_note_dir_at(dir.path()),
        skills_dir.join("Note")
    );
    assert_eq!(
        AppConfig::get_skills_folder_dir_at(dir.path()),
        skills_dir.join("Folder")
    );
    assert_eq!(
        AppConfig::get_skills_batch_dir_at(dir.path()),
        skills_dir.join("Batch")
    );
}

#[test]
fn test_list_skills_files() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let mut config = AppConfig::default();
    config.ensure_system_library_present_at(&sys_path);

    let note_dir = sys_path.join("Skills").join("Note");
    let folder_dir = sys_path.join("Skills").join("Folder");
    let batch_dir = sys_path.join("Skills").join("Batch");

    // Write sample skill files
    std::fs::write(note_dir.join("Proofread.md"), "Proofread this note.").unwrap();
    std::fs::write(note_dir.join("summarize.txt"), "Summarize this note.").unwrap();
    std::fs::write(note_dir.join(".hidden.md"), "Hidden file").unwrap();

    std::fs::write(folder_dir.join("Index.md"), "Index this folder.").unwrap();
    std::fs::write(batch_dir.join("BulkFormat.md"), "Format all notes.").unwrap();

    let note_skills = config.list_note_skills();
    assert_eq!(note_skills.len(), 3);
    assert_eq!(note_skills[0].name, "FormatMarkdown");
    assert_eq!(note_skills[1].name, "Proofread");
    assert_eq!(note_skills[2].name, "summarize");

    let folder_skills = config.list_folder_skills();
    assert_eq!(folder_skills.len(), 2);
    assert_eq!(folder_skills[0].name, "CreateSummary");
    assert_eq!(folder_skills[1].name, "Index");

    let batch_skills = config.list_batch_skills();
    assert_eq!(batch_skills.len(), 1);
    assert_eq!(batch_skills[0].name, "BulkFormat");
}

#[test]
fn test_ensure_all_system_folders_creation() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("my_system");
    assert!(!sys_path.exists());

    AppConfig::ensure_all_system_folders_at(&sys_path).unwrap();

    assert!(sys_path.exists(), "system root folder must be created");
    assert!(
        sys_path.join("Conversations").exists(),
        "Conversations folder must be created"
    );
    assert!(
        sys_path.join("Skills").exists(),
        "Skills folder must be created"
    );
    assert!(
        sys_path.join("Skills").join("Note").exists(),
        "Skills/Note folder must be created"
    );
    assert!(
        sys_path.join("Skills").join("Folder").exists(),
        "Skills/Folder folder must be created"
    );
    assert!(
        sys_path.join("Skills").join("Batch").exists(),
        "Skills/Batch folder must be created"
    );
}

#[test]
fn test_list_skills_creates_missing_folders() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: sys_path.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..Default::default()
    };

    // Verify folders do not exist initially
    assert!(!sys_path.join("Skills").join("Note").exists());
    assert!(!sys_path.join("Skills").join("Folder").exists());
    assert!(!sys_path.join("Skills").join("Batch").exists());

    // Calling listing methods should create missing folders per VFS-105
    let notes = config.list_note_skills();
    assert!(notes.is_empty());
    assert!(sys_path.join("Skills").join("Note").exists());

    let folders = config.list_folder_skills();
    assert!(folders.is_empty());
    assert!(sys_path.join("Skills").join("Folder").exists());

    let batches = config.list_batch_skills();
    assert!(batches.is_empty());
    assert!(sys_path.join("Skills").join("Batch").exists());
}

#[test]
fn test_ensure_all_system_folders_seeds_keep_files() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("seed_system");

    AppConfig::ensure_all_system_folders_at(&sys_path).unwrap();

    // Each Skills subfolder must contain a .keep file so the folder is
    // visible in the tree even before the user adds any skill files (VFS-105).
    assert!(
        sys_path.join("Skills").join("Note").join(".keep").exists(),
        "Skills/Note/.keep must be seeded"
    );
    assert!(
        sys_path
            .join("Skills")
            .join("Folder")
            .join(".keep")
            .exists(),
        "Skills/Folder/.keep must be seeded"
    );
    assert!(
        sys_path.join("Skills").join("Batch").join(".keep").exists(),
        "Skills/Batch/.keep must be seeded"
    );
}

#[test]
fn test_ensure_all_system_folders_keep_is_idempotent() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("idempotent_system");

    // First call seeds the files.
    AppConfig::ensure_all_system_folders_at(&sys_path).unwrap();
    // Second call must not error even though .keep files already exist.
    AppConfig::ensure_all_system_folders_at(&sys_path).unwrap();

    assert!(sys_path.join("Skills").join("Note").join(".keep").exists());
}

// ---- VFS-103: system library path resolution & fallbacks ----

#[test]
fn test_system_library_path_appdata_branch() {
    let p = AppConfig::system_library_path_from_env(Some("C:/AppData"), None);
    assert_eq!(p, PathBuf::from("C:/AppData").join("fastmd").join("system"));
}

#[test]
fn test_system_library_path_userprofile_fallback_branch() {
    let p = AppConfig::system_library_path_from_env(None, Some("C:/Users/test"));
    assert_eq!(
        p,
        PathBuf::from("C:/Users/test")
            .join(".fastmd")
            .join("system")
    );
}

#[test]
fn test_system_library_path_relative_fallback_branch() {
    let p = AppConfig::system_library_path_from_env(None, None);
    assert_eq!(p, PathBuf::from("system"));
}

#[test]
fn test_system_library_path_appdata_takes_precedence_over_userprofile() {
    let p = AppConfig::system_library_path_from_env(Some("AD"), Some("UP"));
    assert_eq!(p, PathBuf::from("AD").join("fastmd").join("system"));
}

// ---- VFS-104 / VFS-105 / VFS-110: IO-failure error paths ----

#[test]
fn test_ensure_system_library_dir_at_fails_when_root_is_a_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir");
    std::fs::write(&file_path, "bytes").unwrap();

    let result = AppConfig::ensure_system_library_dir_at(&file_path);
    assert!(
        result.is_err(),
        "must propagate io::Error when the parent is not a directory"
    );
}

#[test]
fn test_ensure_conversations_dir_at_fails_when_root_is_a_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir");
    std::fs::write(&file_path, "bytes").unwrap();

    let result = AppConfig::ensure_conversations_dir_at(&file_path);
    assert!(
        result.is_err(),
        "must propagate io::Error when the parent is not a directory"
    );
}

#[test]
fn test_ensure_all_system_folders_at_fails_when_root_is_a_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir");
    std::fs::write(&file_path, "bytes").unwrap();

    let result = AppConfig::ensure_all_system_folders_at(&file_path);
    assert!(
        result.is_err(),
        "must propagate io::Error when the root is not a directory"
    );
}

// ---- VFS-102: empty / whitespace custom name ----

#[test]
fn test_system_library_display_name_empty_string_is_returned_verbatim() {
    let config = AppConfig {
        system_library_name: Some(String::new()),
        ..Default::default()
    };
    // `unwrap_or("System")` only fires for `None`; an empty `Some`
    // is returned as-is. This pins the current behaviour so any
    // future normalisation is a deliberate, visible change.
    assert_eq!(config.system_library_display_name(), "");
}

#[test]
fn test_system_library_display_name_whitespace_is_returned_verbatim() {
    let config = AppConfig {
        system_library_name: Some("   ".to_string()),
        ..Default::default()
    };
    assert_eq!(config.system_library_display_name(), "   ");
}

// ---- VFS-125 / VFS-126: sample skill content & "only-when-creating" ----

#[test]
fn test_skills_sample_files_seeded_with_required_content() {
    let dir = tempdir().unwrap();
    AppConfig::ensure_skills_dirs_at(dir.path()).unwrap();

    let note_sample = AppConfig::get_skills_note_dir_at(dir.path()).join("FormatMarkdown.md");
    let folder_sample = AppConfig::get_skills_folder_dir_at(dir.path()).join("CreateSummary.md");

    assert!(
        note_sample.exists(),
        "Skills/Note/FormatMarkdown.md must be seeded on creation (VFS-125)"
    );
    assert_eq!(
        std::fs::read_to_string(&note_sample).unwrap(),
        "instructions to format the current note into correct markdown.",
        "VFS-125 sample content must match the spec wording"
    );

    assert!(
        folder_sample.exists(),
        "Skills/Folder/CreateSummary.md must be seeded on creation (VFS-126)"
    );
    assert_eq!(
        std::fs::read_to_string(&folder_sample).unwrap(),
        "Provide a brief summary of the contents of the folder, in the format <filename>: <one sentence summary of the contents>. One line per file",
        "VFS-126 sample content must match the spec wording verbatim"
    );

    // The spec seeds sample skills only into Note and Folder; Batch
    // must contain no sample file.
    let batch_entries: Vec<_> = std::fs::read_dir(AppConfig::get_skills_batch_dir_at(dir.path()))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        !batch_entries.iter().any(|n| n.ends_with(".md")),
        "Skills/Batch must not receive a sample skill file; found: {batch_entries:?}"
    );
}

#[test]
fn test_create_skills_dir_preserves_existing_sample_files() {
    // "Only when creating": if the subfolder already exists, the
    // sample skill must NOT be (re)written — a user-authored file
    // with the same name must survive untouched.
    let dir = tempdir().unwrap();
    let skills_dir = dir.path().join("system").join("Skills");
    let note_dir = skills_dir.join("Note");
    let folder_dir = skills_dir.join("Folder");
    std::fs::create_dir_all(&note_dir).unwrap();
    std::fs::create_dir_all(&folder_dir).unwrap();

    let user_note_content = "My custom format instructions.";
    let user_folder_content = "My custom summary instructions.";
    std::fs::write(note_dir.join("FormatMarkdown.md"), user_note_content).unwrap();
    std::fs::write(folder_dir.join("CreateSummary.md"), user_folder_content).unwrap();

    AppConfig::create_skills_dir(&skills_dir).unwrap();

    assert_eq!(
        std::fs::read_to_string(note_dir.join("FormatMarkdown.md")).unwrap(),
        user_note_content,
        "existing user FormatMarkdown.md must not be overwritten (VFS-125 only-when-creating)"
    );
    assert_eq!(
        std::fs::read_to_string(folder_dir.join("CreateSummary.md")).unwrap(),
        user_folder_content,
        "existing user CreateSummary.md must not be overwritten (VFS-126 only-when-creating)"
    );
}

#[test]
fn test_create_skills_dir_does_not_seed_sample_into_pre_existing_folder_without_file() {
    // If the subfolder exists but the sample file does not, the
    // "only-when-creating" guard still skips seeding (the guard
    // keys on folder existence, not file existence). Pin this
    // behaviour so a future change that adds the sample to an
    // already-present folder is a deliberate, reviewed change.
    let dir = tempdir().unwrap();
    let skills_dir = dir.path().join("system").join("Skills");
    let note_dir = skills_dir.join("Note");
    std::fs::create_dir_all(&note_dir).unwrap();

    AppConfig::create_skills_dir(&skills_dir).unwrap();

    assert!(
        !note_dir.join("FormatMarkdown.md").exists(),
        "sample must not be seeded into a pre-existing Note folder (VFS-125 only-when-creating)"
    );
}

// ---- VFS-100: no-op when system library already correct ----

#[test]
fn test_ensure_system_library_present_idempotent_when_already_correct() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system");
    let mut config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: sys_path.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..Default::default()
    };

    let libs_before = config.content_libraries.clone();
    config.ensure_system_library_present_at(&sys_path);

    assert_eq!(
        config.content_libraries.len(),
        libs_before.len(),
        "an already-correct System library must not change the library count (VFS-100 no-op)"
    );
    // ContentLibrary does not derive PartialEq; compare the
    // single library field-by-field.
    let lib = &config.content_libraries[0];
    assert_eq!(lib.name, "System");
    assert_eq!(lib.kind, "text");
    assert!(!lib.readonly);
    assert_eq!(lib.priority, 0);
    assert_eq!(
        lib.root_folder,
        sys_path.to_string_lossy().to_string(),
        "root_folder must not be repointed for an already-correct System library"
    );
}

#[test]
#[should_panic(expected = "save_config() was called in a test context")]
fn test_save_config_panics_in_test_context() {
    let cfg = AppConfig::default();
    let _ = crate::config::save_config(&cfg);
}

#[test]
fn test_file_config_storage_saves_to_path() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("subdir").join("test_config.yaml");
    let storage = FileConfigStorage::new(path.clone());
    assert_eq!(storage.path(), path.as_path());

    let cfg = AppConfig {
        table_width_strategy: "waterfill".to_string(),
        ..AppConfig::default()
    };
    let written = storage.save_config(&cfg).unwrap();
    assert_eq!(written, path);
    assert!(path.exists());

    let loaded = crate::config::load_config_from_path(&path, None);
    assert_eq!(loaded.table_width_strategy, "waterfill");
}

#[test]
fn test_noop_config_storage_does_not_create_file() {
    let storage = NoopConfigStorage;
    let cfg = AppConfig::default();
    let res = storage.save_config(&cfg);
    assert!(res.is_ok());
}

#[test]
fn test_in_memory_config_storage_records_history() {
    let storage = InMemoryConfigStorage::new();
    assert!(storage.saved_configs().is_empty());
    assert!(storage.latest_config().is_none());

    let cfg1 = AppConfig {
        table_width_strategy: "waterfill".to_string(),
        ..AppConfig::default()
    };
    storage.save_config(&cfg1).unwrap();

    let cfg2 = AppConfig {
        table_width_strategy: "lagrange".to_string(),
        ..AppConfig::default()
    };
    storage.save_config(&cfg2).unwrap();

    let history = storage.saved_configs();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].table_width_strategy, "waterfill");
    assert_eq!(history[1].table_width_strategy, "lagrange");
    assert_eq!(
        storage.latest_config().unwrap().table_width_strategy,
        "lagrange"
    );
}

// ---- models_for_use_case_min_cost ----

fn chat_model(model: &str, cost: Option<i32>) -> LlmConfig {
    LlmConfig {
        model: model.to_string(),
        api_url: "http://localhost".to_string(),
        api_key: "k".to_string(),
        cost,
        use_case: vec!["chat".to_string()],
    }
}

/// Every model tied at the lowest cost is returned, so the UI can offer the
/// whole tie instead of picking one arbitrarily.
#[test]
fn test_models_for_use_case_min_cost_returns_every_tied_model() {
    let mut config = AppConfig::default();
    config
        .models
        .insert("cheap-a".to_string(), chat_model("a", Some(1)));
    config
        .models
        .insert("cheap-b".to_string(), chat_model("b", Some(1)));
    config
        .models
        .insert("pricier".to_string(), chat_model("c", Some(7)));

    let mut keys: Vec<&str> = config
        .models_for_use_case_min_cost("chat")
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        vec!["cheap-a", "cheap-b"],
        "only the models tied at the minimum cost may be returned"
    );
}

/// **Defect 02, fixed.** An unpriced model (`cost: None`) means "price unknown",
/// not "free". [`LlmConfig::get_cost`] still reports `0` for display, but
/// `model_for_use_case` and `models_for_use_case_min_cost` rank on
/// [`LlmConfig::cost_rank`], so an unpriced model comes *after* every priced
/// model and no longer ties with a genuinely free (`Some(0)`) one.
#[test]
fn defect_02_app_config_ranks_an_unpriced_model_after_every_priced_one() {
    let mut config = AppConfig::default();
    config
        .models
        .insert("cheap".to_string(), chat_model("a", Some(1)));
    config
        .models
        .insert("pricier".to_string(), chat_model("b", Some(9)));
    config
        .models
        .insert("unpriced".to_string(), chat_model("c", None));

    let mut keys: Vec<&str> = config
        .models_for_use_case_min_cost("chat")
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        vec!["cheap"],
        "an unknown price must not beat a configured one"
    );

    // An explicitly free model wins outright; the unpriced one no longer ties.
    config
        .models
        .insert("free".to_string(), chat_model("d", Some(0)));
    let mut tied: Vec<&str> = config
        .models_for_use_case_min_cost("chat")
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    tied.sort_unstable();
    assert_eq!(
        tied,
        vec!["free"],
        "only a genuinely free model is the minimum-cost match"
    );

    // With no priced model at all the unpriced ones still win, so a config
    // without any `cost:` entries keeps selecting a model.
    let mut unpriced_only = AppConfig::default();
    unpriced_only
        .models
        .insert("one".to_string(), chat_model("a", None));
    unpriced_only
        .models
        .insert("two".to_string(), chat_model("b", None));
    let mut all_unknown: Vec<&str> = unpriced_only
        .models_for_use_case_min_cost("chat")
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    all_unknown.sort_unstable();
    assert_eq!(
        all_unknown,
        vec!["one", "two"],
        "when nothing is priced, everything is tied and still selectable"
    );
}

#[test]
fn test_models_for_use_case_min_cost_single_winner() {
    let mut config = AppConfig::default();
    config
        .models
        .insert("cheap".to_string(), chat_model("a", Some(1)));
    config
        .models
        .insert("pricier".to_string(), chat_model("b", Some(9)));

    let matches = config.models_for_use_case_min_cost("chat");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, "cheap");
    assert_eq!(matches[0].1.model, "a");
}

/// A use case nothing is configured for yields an empty list rather than a
/// fallback to an unrelated model.
#[test]
fn test_models_for_use_case_min_cost_empty_when_nothing_matches() {
    let mut config = AppConfig::default();
    config.models.insert(
        "vision_only".to_string(),
        LlmConfig {
            model: "gpt-4o".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "k".to_string(),
            cost: Some(1),
            use_case: vec!["vision".to_string()],
        },
    );

    assert!(config.models_for_use_case_min_cost("chat").is_empty());
    assert!(
        AppConfig::default()
            .models_for_use_case_min_cost("chat")
            .is_empty()
    );
}

// ---- get_or_create_system_library_at ----

/// The path-taking variant must create the system folders and label the
/// library with the configured display name — without touching the platform
/// default locations ([RUST-024]).
#[test]
fn test_get_or_create_system_library_at_creates_folders_and_labels_library() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system-root");
    let config = AppConfig {
        system_library_name: Some("My Library".to_string()),
        ..AppConfig::default()
    };

    let library = config.get_or_create_system_library_at(&sys_path);

    assert_eq!(
        library.root_folder,
        sys_path.to_string_lossy().to_string(),
        "the library must be rooted at the requested path, not a platform default"
    );
    assert_eq!(library.name, "My Library");
    assert_eq!(library.kind, "text");
    assert!(!library.readonly);
    assert_eq!(library.priority, 0);
    assert!(
        sys_path.join("Skills").join("Note").exists(),
        "the system folders must be created as a side effect"
    );
}

#[test]
fn test_get_or_create_system_library_at_falls_back_to_default_display_name() {
    let dir = tempdir().unwrap();
    let sys_path = dir.path().join("system-root");
    let config = AppConfig::default();

    let library = config.get_or_create_system_library_at(&sys_path);

    assert_eq!(library.name, "System");
}

// ---- list_skill_files_in ----

/// `list_skill_files_in` must degrade to an empty list for anything that is not
/// a readable directory, skip dotfiles, and sort case-insensitively by name.
#[test]
fn test_list_skill_files_in_edge_cases() {
    let dir = tempdir().unwrap();
    let skills = dir.path().join("skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("beta.md"), "b").unwrap();
    std::fs::write(skills.join("Alpha.md"), "a").unwrap();
    std::fs::write(skills.join(".hidden.md"), "h").unwrap();
    std::fs::write(skills.join("no_extension"), "n").unwrap();
    std::fs::create_dir(skills.join("nested")).unwrap();
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    let a_file = dir.path().join("a-file.md");
    std::fs::write(&a_file, "not a dir").unwrap();

    struct Case {
        name: &'static str,
        dir: PathBuf,
        expected: Vec<&'static str>,
    }
    let cases = [
        Case {
            name: "missing directory",
            dir: dir.path().join("does-not-exist"),
            expected: vec![],
        },
        Case {
            name: "path is a file, not a directory",
            dir: a_file.clone(),
            expected: vec![],
        },
        Case {
            name: "empty directory",
            dir: dir.path().join("empty"),
            expected: vec![],
        },
        Case {
            name: "dotfiles and subdirectories are skipped and names sort case-insensitively",
            dir: skills.clone(),
            expected: vec!["Alpha", "beta", "no_extension"],
        },
    ];

    for case in cases {
        let found = AppConfig::list_skill_files_in(&case.dir);
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, case.expected, "case: {}", case.name);
    }

    // The stem of a Markdown file is the display name; the full path is kept.
    let found = AppConfig::list_skill_files_in(&skills);
    assert_eq!(
        found[0].path,
        skills.join("Alpha.md"),
        "the skill must carry the path it was discovered at"
    );
    assert_eq!(
        found[2].name, "no_extension",
        "a file without an extension falls back to its file name"
    );
}

// ---- save_config_to_path ----

/// A missing parent chain is created, and the written YAML reloads identically.
#[test]
fn test_save_config_to_path_creates_parents_and_round_trips() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deep").join("nested").join("config.yaml");
    let config = AppConfig {
        system_prompt_extension: Some("be terse".to_string()),
        table_width_strategy: "waterfill".to_string(),
        ..AppConfig::default()
    };

    let written = save_config_to_path(&config, &path).unwrap();

    assert_eq!(written, path);
    assert!(path.exists());
    let reloaded = load_config_from_path(&path, Some(&dir.path().join("system")));
    assert_eq!(
        reloaded.system_prompt_extension.as_deref(),
        Some("be terse")
    );
    assert_eq!(reloaded.table_width_strategy, "waterfill");
}

/// When the parent cannot be created the error must name the directory that
/// failed instead of surfacing a bare write error.
#[test]
fn test_save_config_to_path_reports_an_uncreatable_parent() {
    let dir = tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "i am a file").unwrap();
    let path = blocker.join("config.yaml");

    let err = save_config_to_path(&AppConfig::default(), &path)
        .expect_err("a parent blocked by a regular file cannot be created");

    assert!(
        err.contains("failed to create config parent dir"),
        "unexpected error: {err}"
    );
    assert!(
        !path.exists(),
        "no config file may be written under an uncreatable parent"
    );
}

// ---- load_config_from_path fallbacks ----

/// A path that exists but cannot be read (here: a directory) must fall back to
/// the default config with the system library projected in, and must not
/// overwrite the offending path.
#[test]
fn test_load_config_from_path_falls_back_when_the_file_is_unreadable() {
    let dir = tempdir().unwrap();
    let system_path = dir.path().join("system");
    let config_path = dir.path().join("config.yaml");
    std::fs::create_dir(&config_path).unwrap();

    let config = load_config_from_path(&config_path, Some(&system_path));

    assert_eq!(
        config.system_prompt_extension,
        AppConfig::default().system_prompt_extension
    );
    assert!(
        config_path.is_dir(),
        "the fallback must not replace the unreadable path with a file"
    );
    assert!(
        config
            .content_libraries
            .iter()
            .any(|lib| lib.name == "System"),
        "the system library must still be projected onto the fallback config"
    );
}

/// An unparseable file must also leave the file untouched — the fallback path
/// does not rewrite it, so the user's broken YAML stays visible for repair.
#[test]
fn test_load_config_from_path_does_not_rewrite_an_unparseable_file() {
    let dir = tempdir().unwrap();
    let system_path = dir.path().join("system");
    let config_path = dir.path().join("config.yaml");
    std::fs::write(&config_path, "invalid: yaml: [").unwrap();

    let config = load_config_from_path(&config_path, Some(&system_path));

    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "invalid: yaml: [",
        "a broken config file must not be silently overwritten"
    );
    assert_eq!(config.table_width_strategy, "hybrid");
}

// ---- validate ----

/// Several independent problems must all be reported in one pass so the user
/// can fix the file in a single edit.
#[test]
fn test_validate_collects_multiple_warnings() {
    let mut config = AppConfig::default();
    config.models.insert(
        "odd".to_string(),
        LlmConfig {
            model: "m".to_string(),
            api_url: "http://a".to_string(),
            api_key: "k".to_string(),
            cost: None,
            use_case: vec!["summarise".to_string(), "embeddings".to_string()],
        },
    );
    config.models.insert(
        "also-odd".to_string(),
        LlmConfig {
            model: "m2".to_string(),
            api_url: "http://b".to_string(),
            api_key: "k".to_string(),
            cost: None,
            use_case: vec!["chat-ish".to_string()],
        },
    );

    let warnings = config.validate();

    assert_eq!(
        warnings.len(),
        3,
        "two unknown use_cases plus the missing chat model: {warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("odd") && w.contains("summarise"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("also-odd") && w.contains("chat-ish"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w == "No model configured with 'chat' use_case")
    );
}

// ---- DiscordConfig serde defaults ----

/// A bare `discord:` block must fill in the documented defaults rather than
/// failing to deserialise.
#[test]
fn test_discord_config_serde_defaults() {
    let parsed: DiscordConfig = serde_norway::from_str("{}").unwrap();

    assert!(parsed.bot_token.is_none());
    assert!(parsed.allowed_channels.is_empty());
    assert!(parsed.allowed_guilds.is_empty());
    assert!(
        parsed.register_commands,
        "slash-command registration is on by default"
    );
    assert!(parsed.system_prompt.is_none());
    assert_eq!(parsed.max_history, 20);
    assert_eq!(parsed.rate_limit_per_minute, 10);
    assert!(parsed.blocked_patterns.is_empty());

    // Explicit values must win over the defaults.
    let parsed: DiscordConfig = serde_norway::from_str(
        "register_commands: false\nmax_history: 3\nrate_limit_per_minute: 1\nblocked_patterns: [\"secret\"]\n",
    )
    .unwrap();
    assert!(!parsed.register_commands);
    assert_eq!(parsed.max_history, 3);
    assert_eq!(parsed.rate_limit_per_minute, 1);
    assert_eq!(parsed.blocked_patterns, vec!["secret".to_string()]);
}

// ---- DiscordConfig secret redaction (NFR-009) ----

/// A Discord configuration with every field populated, so a redaction test can
/// tell "the secret is hidden" apart from "the whole struct prints nothing".
fn discord_config_with_token(token: &str) -> DiscordConfig {
    DiscordConfig {
        bot_token: Some(token.to_string()),
        allowed_channels: vec!["channel-123".to_string()],
        allowed_guilds: vec!["guild-456".to_string()],
        register_commands: true,
        system_prompt: Some("Answer briefly.".to_string()),
        max_history: 7,
        rate_limit_per_minute: 3,
        blocked_patterns: vec!["internal-only".to_string()],
    }
}

/// **Regression test for the Discord bot token leaking through `Debug` (defect D2).**
///
/// `DiscordConfig` used to derive `Debug`, so `{:?}` printed `bot_token:
/// Some("<the real token>")` — and because `AppConfig`'s `Debug` forwards to it,
/// any `tracing::debug!("{config:?}")` wrote the credential to the log, which
/// [NFR-009] forbids. The token is now rendered as the repo-wide `[REDACTED]`
/// placeholder while every other field stays readable.
#[test]
fn defect_02_discord_config_debug_redacts_the_bot_token() {
    const SECRET: &str = "MTA5NDIyOTc0MzY3OTAwMDcwNg.GbRaZx.supersecrettokenvalue";
    let config = discord_config_with_token(SECRET);

    let rendered = format!("{config:?}");

    assert!(
        !rendered.contains(SECRET),
        "Debug must never print the bot token, got: {rendered}"
    );
    assert!(
        !rendered.contains("supersecret"),
        "no fragment of the token may survive, got: {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "the token must be replaced by the repo-wide redaction placeholder, got: {rendered}"
    );
    assert!(
        rendered.contains("Some(\"[REDACTED]\")"),
        "a configured token must still be visible as *configured*, got: {rendered}"
    );

    // Every non-secret field must remain readable, otherwise redaction has
    // quietly turned the diagnostic into a black box.
    for expected in [
        "channel-123",
        "guild-456",
        "register_commands: true",
        "Answer briefly.",
        "max_history: 7",
        "rate_limit_per_minute: 3",
        "internal-only",
    ] {
        assert!(
            rendered.contains(expected),
            "Debug must keep `{expected}` readable, got: {rendered}"
        );
    }
    assert!(
        rendered.starts_with("DiscordConfig"),
        "the struct name must still head the dump, got: {rendered}"
    );
}

/// An unconfigured token must not be dressed up as a redacted one: `None` stays
/// `None` so an operator can tell "no token" from "token set".
#[test]
fn defect_02_discord_config_debug_keeps_an_absent_token_as_none() {
    let rendered = format!("{:?}", DiscordConfig::default());

    assert!(
        rendered.contains("bot_token: None"),
        "an absent token must print as None, got: {rendered}"
    );
    assert!(
        !rendered.contains("[REDACTED]"),
        "nothing is redacted when nothing was configured, got: {rendered}"
    );
}

/// An empty configured token is still a configured value: its *presence* is
/// reported, its content is not.
#[test]
fn defect_02_discord_config_debug_redacts_an_empty_token_too() {
    let rendered = format!("{:?}", discord_config_with_token(""));

    assert!(
        rendered.contains("Some(\"[REDACTED]\")"),
        "an empty token must be reported as configured-but-redacted, got: {rendered}"
    );
}

/// The leak that actually mattered: `AppConfig` is what gets logged, and its
/// `Debug` forwards to the Discord config.
#[test]
fn defect_02_app_config_debug_does_not_leak_the_discord_token() {
    const SECRET: &str = "Bot.s3cr3t-t0k3n-value-do-not-log";
    let config = AppConfig {
        discord: Some(discord_config_with_token(SECRET)),
        ..AppConfig::default()
    };

    let rendered = format!("{config:?}");

    assert!(
        !rendered.contains(SECRET),
        "AppConfig's Debug must not carry the Discord token, got: {rendered}"
    );
    assert!(
        rendered.contains("discord: Some(") && rendered.contains("[REDACTED]"),
        "the Discord section must still be present and redacted, got: {rendered}"
    );
}

/// `AppConfig`'s hand-written `Debug` must name every field an operator needs
/// when triaging a bad config — a missing `.field(...)` silently hides a field
/// from diagnostics.
#[test]
fn test_app_config_debug_lists_every_field() {
    let config = AppConfig {
        discord: Some(DiscordConfig {
            bot_token: Some("token".to_string()),
            ..DiscordConfig::default()
        }),
        qdrant_url: Some("http://qdrant:6333".to_string()),
        qdrant_collection: Some("notes".to_string()),
        csv_db_path: Some("C:/db".to_string()),
        pdf_converter_command: Some(vec!["pandoc".to_string(), "{input}".to_string()]),
        ..AppConfig::default()
    };

    let rendered = format!("{config:?}");

    for field in [
        "system_prompt_extension",
        "models",
        "selected_chat_model",
        "searxng_url",
        "jmap_clients",
        "caldav_clients",
        "content_libraries",
        "system_library_name",
        "pdf_converter_command",
        "inline_editor_enabled",
        "csv_db_path",
        "feature_flags",
        "max_tokens",
        "tool_groups",
        "mcp_servers",
        "table_width_strategy",
        "trello_client",
        "discord",
        "qdrant_url",
        "qdrant_collection",
    ] {
        assert!(
            rendered.contains(field),
            "Debug must list the `{field}` field, got: {rendered}"
        );
    }
    assert!(rendered.starts_with("AppConfig"));
}
