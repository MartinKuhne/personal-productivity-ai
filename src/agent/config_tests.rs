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

// ---------------------------------------------------------------------------
// D2: cost ranking — an unpriced model is unknown, never free
// ---------------------------------------------------------------------------

/// A model with an explicit (`Some`) or missing (`None`) price for one use case.
fn model_with_cost(model: &str, cost: Option<i32>, use_case: &str) -> LlmConfig {
    LlmConfig {
        model: model.to_string(),
        api_url: "http://api".to_string(),
        api_key: "k".to_string(),
        cost,
        use_case: vec![use_case.to_string()],
    }
}

#[test]
fn cost_rank_orders_an_unknown_price_after_every_priced_model() {
    let free = model_with_cost("free", Some(0), "chat");
    let pricey = model_with_cost("pricey", Some(1_000), "chat");
    let unknown = model_with_cost("unknown", None, "chat");

    assert_eq!(free.cost_rank(), (0, 0), "an explicit zero keeps its price");
    assert_eq!(
        pricey.cost_rank(),
        (0, 1_000),
        "a priced model keeps its price"
    );
    assert_eq!(
        unknown.cost_rank(),
        (1, 0),
        "an unknown price gets the last group"
    );
    assert!(
        free.cost_rank() < pricey.cost_rank(),
        "a lower price must rank first"
    );
    assert!(
        pricey.cost_rank() < unknown.cost_rank(),
        "an unknown price must rank after every priced model"
    );
    assert_eq!(
        unknown.cost_rank(),
        model_with_cost("other", None, "chat").cost_rank(),
        "all unpriced models must rank equally"
    );
}

#[test]
fn defect_02_an_unpriced_model_ranks_after_every_priced_model() {
    let mut models = HashMap::new();
    models.insert(
        "cheap".to_string(),
        model_with_cost("cheap-model", Some(1), "chat"),
    );
    models.insert(
        "pricey".to_string(),
        model_with_cost("pricey-model", Some(50), "chat"),
    );
    models.insert(
        "unknown".to_string(),
        model_with_cost("unknown-model", None, "chat"),
    );

    let cfg = AgentConfigBuilder::new().with_models(models).build();
    let (key, chosen) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert_eq!(
        key, "cheap",
        "a missing price must not be coerced to 0 and beat an explicit price"
    );
    assert_eq!(chosen.model, "cheap-model");
}

#[test]
fn defect_02_an_explicit_zero_cost_beats_an_unpriced_model() {
    let mut models = HashMap::new();
    models.insert(
        "free".to_string(),
        model_with_cost("free-model", Some(0), "chat"),
    );
    models.insert(
        "unknown".to_string(),
        model_with_cost("unknown-model", None, "chat"),
    );

    let cfg = AgentConfigBuilder::new().with_models(models).build();
    let (key, _) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert_eq!(key, "free", "only a declared price of 0 is free");
}

#[test]
fn defect_02_select_chat_model_skips_an_unpriced_model_when_one_is_priced() {
    let mut models = HashMap::new();
    models.insert(
        "unknown".to_string(),
        model_with_cost("unknown-model", None, "chat"),
    );
    models.insert(
        "cheap".to_string(),
        model_with_cost("cheap-model", Some(2), "chat"),
    );

    let cfg = AgentConfigBuilder::new().with_models(models).build();

    assert_eq!(
        cfg.select_chat_model().expect("a usable chat model").model,
        "cheap-model",
        "the chat fallback must not treat an unpriced model as the cheapest"
    );
}

#[test]
fn defect_02_all_unpriced_models_rank_equally() {
    let mut models = HashMap::new();
    for name in ["a", "b", "c"] {
        models.insert(name.to_string(), model_with_cost(name, None, "chat"));
    }
    let ranks: Vec<_> = models.values().map(LlmConfig::cost_rank).collect();
    assert!(
        ranks.iter().all(|rank| *rank == ranks[0]),
        "unpriced models must rank equally: {ranks:?}"
    );

    let cfg = AgentConfigBuilder::new()
        .with_models(models.clone())
        .build();
    let (key, chosen) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert!(
        models.contains_key(key),
        "the winner must be one of the configured models: {key}"
    );
    assert_eq!(chosen.cost, None, "the winner must be an unpriced model");
}

#[test]
fn defect_02_ties_still_select_the_lowest_priced_group() {
    let mut models = HashMap::new();
    models.insert("a".to_string(), model_with_cost("a-model", Some(7), "chat"));
    models.insert("b".to_string(), model_with_cost("b-model", Some(7), "chat"));
    models.insert("c".to_string(), model_with_cost("c-model", Some(3), "chat"));
    models.insert("d".to_string(), model_with_cost("d-model", None, "chat"));

    let cfg = AgentConfigBuilder::new().with_models(models).build();
    let (key, _) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert_eq!(
        key, "c",
        "the single cheapest priced model wins over a tie and over unknowns"
    );
}

#[test]
fn defect_02_a_pure_tie_resolves_to_one_of_the_tied_models() {
    let mut models = HashMap::new();
    models.insert("a".to_string(), model_with_cost("a-model", Some(7), "chat"));
    models.insert("b".to_string(), model_with_cost("b-model", Some(7), "chat"));

    let cfg = AgentConfigBuilder::new().with_models(models).build();
    let (key, _) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert!(
        key == "a" || key == "b",
        "a tie must resolve to a tied candidate, not a third model: {key}"
    );
}

#[test]
fn defect_02_the_use_case_filter_is_applied_before_the_cost_ranking() {
    let mut models = HashMap::new();
    models.insert(
        "vision".to_string(),
        model_with_cost("vision-model", None, "vision"),
    );
    models.insert(
        "chat".to_string(),
        model_with_cost("chat-model", Some(9), "chat"),
    );

    let cfg = AgentConfigBuilder::new().with_models(models).build();

    let (key, _) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");
    assert_eq!(
        key, "chat",
        "an unpriced vision model must not answer a chat query"
    );

    let (key, _) = cfg
        .model_for_use_case("vision")
        .expect("a vision model is configured");
    assert_eq!(
        key, "vision",
        "the unpriced model is still available for its own use case"
    );

    assert!(
        cfg.model_for_use_case("embeddings").is_none(),
        "an unmatched use case has no winner"
    );
}

#[test]
fn defect_02_an_empty_model_map_has_no_cheapest_model() {
    let cfg = AgentConfigBuilder::new().build();
    assert!(
        cfg.model_for_use_case("chat").is_none(),
        "no configured models means no winner"
    );
    assert!(
        cfg.model_for_use_case("vision").is_none(),
        "an unknown use case has no winner either"
    );
}
