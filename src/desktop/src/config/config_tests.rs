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
