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

/// Regression: previously the three `test_load_config_*` tests mutated
/// the process-wide `APPDATA` env var, and `load_config()` read it
/// back. Under the parallel test runner, another test could overwrite
/// `APPDATA` between the `set_var` and the `load_config` call, so
/// `test_load_config_invalid_file` would sometimes load a sibling
/// test's valid YAML and see another test's value instead of
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
        crate::ui::table_width::DeficitStrategy::HybridMinPenaltyWaterFill
    );
    // Round-trip: to_config and from_config compose back to the same
    // variant, so a persisted config with the default value
    // deserialises to the same strategy.
    let s = crate::ui::table_width::DeficitStrategy::HybridMinPenaltyWaterFill.to_config();
    assert_eq!(s, "hybrid");
    assert_eq!(
        crate::ui::table_width::DeficitStrategy::from_config(s),
        crate::ui::table_width::DeficitStrategy::HybridMinPenaltyWaterFill
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

#[test]
fn deficit_strategy_parses_survey_algorithm_strings() {
    // Each of the three survey algorithms (doc §2.10, §2.13, §2.14)
    // must parse from its canonical config string. This guards the
    // round-trip with the top-bar combobox.
    use crate::ui::table_width::DeficitStrategy;
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

    let mut cfg = AppConfig::default();
    cfg.table_width_strategy = "waterfill".to_string();
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

    let mut cfg1 = AppConfig::default();
    cfg1.table_width_strategy = "waterfill".to_string();
    storage.save_config(&cfg1).unwrap();

    let mut cfg2 = AppConfig::default();
    cfg2.table_width_strategy = "lagrange".to_string();
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
