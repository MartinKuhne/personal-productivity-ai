//! Unit tests for the OpenAI-compatible embeddings client.

use super::*;
use crate::agent::config::LlmConfig;
use async_openai::config::Config;

fn llm_config(model: &str, api_url: &str, cost: i32, use_case: &[&str]) -> LlmConfig {
    LlmConfig {
        model: model.to_string(),
        api_url: api_url.to_string(),
        api_key: "test-key".to_string(),
        cost: Some(cost),
        use_case: use_case.iter().map(|u| u.to_string()).collect(),
    }
}

#[test]
fn from_config_returns_none_without_embeddings_model() {
    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        llm_config("gpt-4", "https://api.example.com/v1", 0, &["chat"]),
    );
    assert!(EmbeddingClient::from_config(&config).is_none());
}

#[test]
fn from_config_selects_lowest_cost_embeddings_model() {
    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        llm_config("gpt-4", "https://api.example.com/v1", 0, &["chat"]),
    );
    config.models.insert(
        "emb-expensive".to_string(),
        llm_config("embed-a", "http://localhost:9998/v1", 2, &["embeddings"]),
    );
    config.models.insert(
        "emb-cheap".to_string(),
        llm_config("embed-b", "http://localhost:9999/v1/", 1, &["embeddings"]),
    );
    let client = EmbeddingClient::from_config(&config).unwrap();
    assert_eq!(client.model_name(), "embed-b");
    assert_eq!(
        client.client.config().api_base(),
        "http://localhost:9999/v1"
    );
}

#[test]
fn from_config_trims_trailing_slash_and_quotes_from_api_base() {
    let mut config = AppConfig::default();
    config.models.insert(
        "emb".to_string(),
        llm_config(
            "embed-a",
            "\"http://localhost:9999/v1/\"",
            0,
            &["embeddings"],
        ),
    );
    let client = EmbeddingClient::from_config(&config).unwrap();
    assert_eq!(
        client.client.config().api_base(),
        "http://localhost:9999/v1"
    );
}

#[test]
fn embed_empty_input_returns_empty_without_network() {
    let mut config = AppConfig::default();
    config.models.insert(
        "emb".to_string(),
        llm_config("embed-a", "http://localhost:9999/v1", 0, &["embeddings"]),
    );
    let client = EmbeddingClient::from_config(&config).unwrap();
    assert_eq!(client.embed(Vec::new()).unwrap(), Vec::<Vec<f32>>::new());
}

#[test]
fn order_by_index_reorders_out_of_order_embeddings() {
    let data = vec![
        Embedding {
            index: 2,
            object: "embedding".to_string(),
            embedding: vec![2.0],
        },
        Embedding {
            index: 0,
            object: "embedding".to_string(),
            embedding: vec![0.0],
        },
        Embedding {
            index: 1,
            object: "embedding".to_string(),
            embedding: vec![1.0],
        },
    ];
    assert_eq!(order_by_index(data), vec![vec![0.0], vec![1.0], vec![2.0]]);
}
