//! Vector-search service contract and tool integration (TOOL-043, AGENT-031).
//! Unit tests live in the sibling `vector_search_tests.rs` sidecar.

use crate::tools::context::ToolContext;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use crate::tools::{Safety, Tool};
use fastmd_tool_macros::ToolDescriptor;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;

/// A Markdown chunk returned by vector search.
#[derive(Clone, Debug, serde::Serialize)]
pub struct VectorSearchHit {
    /// Source Markdown path.
    pub path: String,
    /// Matching Markdown chunk.
    pub text: String,
    /// Distance reported by the vector index.
    pub distance: f32,
}

/// Background vector-index service supplied by the desktop application.
pub trait VectorSearchService: Send + Sync {
    /// Search indexed Markdown chunks without doing embedding work on the caller thread.
    fn search(&self, query: &str, limit: usize) -> Result<Vec<VectorSearchHit>, String>;
}

/// Extension wrapper used to inject the shared vector-search service.
pub struct VectorSearchExt(pub Arc<dyn VectorSearchService>);

impl ToolContext {
    /// Return the injected vector-search service.
    pub fn vector_search(&self) -> Result<Arc<dyn VectorSearchService>, String> {
        self.extensions
            .get::<VectorSearchExt>()
            .map(|ext| ext.0.clone())
            .ok_or_else(|| "Vector search is not available.".to_string())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VectorSearchInput {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    5
}

const VECTOR_SEARCH_DESCRIPTION: &str = "Search indexed Markdown content by meaning. Vector indexing and embedding run in the background.";

/// LLM tool for searching the optional Markdown vector index.
#[derive(ToolDescriptor)]
#[tool(
    name = "vector_search",
    desc = VECTOR_SEARCH_DESCRIPTION,
    input = VectorSearchInput,
    safety = Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_vector_search,
)]
pub(crate) struct VectorSearchTool;

fn execute_vector_search(
    _tool: &VectorSearchTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: VectorSearchInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {e}"))?;
    if input.query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }
    let limit = input.limit.clamp(1, 20);
    let service = ctx
        .extensions
        .get::<VectorSearchExt>()
        .ok_or_else(|| "Vector search is not available.".to_string())?;
    let hits = service.0.search(input.query.trim(), limit)?;
    serde_json::to_value(hits).map_err(|e| e.to_string())
}

/// Provider for the optional vector-search built-in tool.
pub(crate) struct VectorSearchProvider;

impl ToolProvider for VectorSearchProvider {
    fn id(&self) -> &'static str {
        "vector-search"
    }

    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    }

    fn tools(&self) -> Vec<RegisteredTool> {
        let tool = VectorSearchTool;
        vec![RegisteredTool::new(
            tool.descriptor().clone(),
            Arc::new(tool),
        )]
    }
}

#[cfg(test)]
mod tests;
