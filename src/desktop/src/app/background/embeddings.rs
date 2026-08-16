//! OpenAI-compatible embedding client configured from the `embeddings` model (AGENT-005, AGENT-031).
//!
//! Unit tests live in the sibling `embeddings_tests.rs` sidecar.

use crate::config::AppConfig;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::embeddings::{CreateEmbeddingRequestArgs, Embedding, EmbeddingInput};

/// Client that produces text embeddings from the user-configured `embeddings` model.
#[derive(Clone)]
pub struct EmbeddingClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl EmbeddingClient {
    /// Build a client from the lowest-cost model with the `embeddings` use case.
    /// Returns `None` when no model is configured for embeddings.
    pub fn from_config(config: &AppConfig) -> Option<Self> {
        let (_, cfg) = config.model_for_use_case("embeddings")?;
        let api_base = cfg.api_url.trim_matches('"').trim_end_matches('/');
        let openai_config = OpenAIConfig::new()
            .with_api_base(api_base.to_string())
            .with_api_key(cfg.api_key.clone());
        Some(Self {
            client: Client::with_config(openai_config),
            model: cfg.model.clone(),
        })
    }

    /// The configured embedding model ID.
    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// Embed a batch of texts asynchronously. Returns vectors in input order.
    pub async fn embed_async(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(EmbeddingInput::StringArray(texts))
            .build()
            .map_err(|error| error.to_string())?;
        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|error| error.to_string())?;
        Ok(order_by_index(response.data))
    }

    /// Embed a batch of texts synchronously. Returns vectors in input order.
    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(self.embed_async(texts))),
            Err(_) => crate::agent::tools::blocking::block_on(self.embed_async(texts)),
        }
    }
}

/// Return embedding vectors ordered by the API-reported index.
fn order_by_index(mut data: Vec<Embedding>) -> Vec<Vec<f32>> {
    data.sort_by_key(|embedding| embedding.index);
    data.into_iter()
        .map(|embedding| embedding.embedding)
        .collect()
}

#[cfg(test)]
#[path = "embeddings_tests.rs"]
mod tests;
