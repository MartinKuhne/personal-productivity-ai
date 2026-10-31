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
// D2: cost ranking — an unpriced model is free, not unknown
//
// Contract: `cost` is an optional integer whose documented default is `0`
// ([AGENT-005], [CONFIG-006], [CONFIG-008]), and selection picks the lowest
// cost for the use case ([AGENT-030], [AGENT-019]). So `AgentConfig::
// model_for_use_case` ranks on `LlmConfig::get_cost()`: a model nobody priced
// ties with a declared `cost: 0` and beats every priced model.
// An earlier revision ranked an absent price as *unknown* and last, which
// contradicted the spec; the spec is authoritative and the code follows it.
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
fn get_cost_defaults_an_unpriced_model_to_zero() {
    let free = model_with_cost("free", Some(0), "chat");
    let pricey = model_with_cost("pricey", Some(1_000), "chat");
    let unknown = model_with_cost("unknown", None, "chat");

    assert_eq!(free.get_cost(), 0, "an explicit zero is free");
    assert_eq!(
        unknown.get_cost(),
        0,
        "an absent price defaults to 0, per AGENT-005"
    );
    assert_eq!(pricey.get_cost(), 1_000, "a declared price is kept");
    assert_eq!(
        unknown.get_cost(),
        free.get_cost(),
        "an unpriced model must tie with a declared cost of 0"
    );
    assert!(
        free.get_cost() < pricey.get_cost(),
        "a lower price must rank first"
    );
}

/// `cost` defaults to `0` ([AGENT-005]), so a model nobody priced is free and
/// wins cheapest-first selection against every explicitly priced candidate.
#[test]
fn an_unpriced_model_ranks_ahead_of_every_priced_model() {
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
        key, "unknown",
        "an absent price defaults to 0, so it must beat a declared price of 1"
    );
    assert_eq!(chosen.model, "unknown-model");
    assert_eq!(chosen.get_cost(), 0, "the winner is treated as free");
}

/// A declared `cost: 0` and an absent price are the same price, so neither may
/// be preferred over the other; either may win the tie.
#[test]
fn an_explicit_zero_cost_ties_with_an_unpriced_model() {
    let mut models = HashMap::new();
    models.insert(
        "free".to_string(),
        model_with_cost("free-model", Some(0), "chat"),
    );
    models.insert(
        "unknown".to_string(),
        model_with_cost("unknown-model", None, "chat"),
    );

    let cfg = AgentConfigBuilder::new()
        .with_models(models.clone())
        .build();
    let (key, chosen) = cfg
        .model_for_use_case("chat")
        .expect("a chat model is configured");

    assert!(
        key == "free" || key == "unknown",
        "the tie must resolve to one of the two free candidates: {key}"
    );
    assert_eq!(
        chosen.get_cost(),
        0,
        "both candidates cost 0, whichever wins"
    );
    assert_eq!(
        models["free"].get_cost(),
        models["unknown"].get_cost(),
        "a declared 0 and an absent price must be equal"
    );
}

/// [AGENT-030] selects the lowest-cost chat model, and an absent price is the
/// documented default of `0`, so the unpriced model is the correct fallback.
#[test]
fn select_chat_model_prefers_an_unpriced_model_because_cost_defaults_to_zero() {
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
        "unknown-model",
        "the chat fallback must rank the unpriced model at its default cost of 0"
    );
}

/// Every unpriced model defaults to the same cost of `0` ([AGENT-005]), so none
/// of them is preferred over the others on price.
#[test]
fn all_unpriced_models_rank_equally() {
    let mut models = HashMap::new();
    for name in ["a", "b", "c"] {
        models.insert(name.to_string(), model_with_cost(name, None, "chat"));
    }
    let ranks: Vec<i32> = models.values().map(LlmConfig::get_cost).collect();
    assert!(
        ranks.iter().all(|cost| *cost == ranks[0]),
        "unpriced models must rank equally: {ranks:?}"
    );
    assert!(
        ranks.iter().all(|cost| *cost == 0),
        "an absent price must rank at the documented default of 0: {ranks:?}"
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

/// The cheapest *effective* cost wins: the unpriced model ranks at its default
/// of `0`, so it beats both the priced tie at 7 and the priced minimum at 3.
#[test]
fn the_lowest_effective_cost_wins_over_a_priced_tie() {
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
        key, "d",
        "an unpriced model costs 0, so it wins over the priced tie and the priced minimum"
    );
}

/// `HashMap` order decides a tie, so the contract is only that the winner is one
/// of the tied candidates.
#[test]
fn a_pure_tie_resolves_to_one_of_the_tied_models() {
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

/// The use-case filter is applied before price, which matters precisely because
/// an unpriced model now ranks at 0: a free vision model must not answer a chat
/// query.
#[test]
fn the_use_case_filter_is_applied_before_the_cost_ranking() {
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

/// No models means no winner, whatever the ranking rule.
#[test]
fn an_empty_model_map_has_no_cheapest_model() {
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
