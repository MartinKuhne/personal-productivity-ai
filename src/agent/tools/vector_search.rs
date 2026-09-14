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
    /// Distance reported by the vector index.
    pub distance: f32,
    /// 0-indexed line offset within the note body (after YAML front matter).
    pub offset: usize,
    /// Number of lines in the chunk.
    pub limit: usize,
    /// Source file lines at the chunk's offset/limit.
    pub content: String,
}

/// Background vector-index service supplied by the desktop application.
pub trait VectorSearchService: Send + Sync {
    /// Search indexed Markdown chunks without doing embedding work on the caller thread.
    fn search(
        &self,
        query: &str,
        limit: usize,
        max_distance: Option<f32>,
    ) -> Result<Vec<VectorSearchHit>, String>;
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
    #[schemars(description = crate::tools::registry::builtin::strings::FIELD_VECTOR_SEARCH_INPUT_QUERY)]
    #[serde(default)]
    query: String,
    #[schemars(description = crate::tools::registry::builtin::strings::FIELD_VECTOR_SEARCH_INPUT_MAX_DISTANCE)]
    #[serde(default)]
    max_distance: Option<f32>,
    #[schemars(description = crate::tools::registry::builtin::strings::FIELD_CURSOR_DESCRIPTION)]
    #[serde(default)]
    cursor: Option<String>,
}

const VECTOR_SEARCH_DESCRIPTION: &str =
    crate::tools::registry::builtin::strings::VECTOR_SEARCH_DESCRIPTION;

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

    if let Some(cursor) = &input.cursor {
        let page = ctx.cache().vector_sessions.next_page(cursor)?;
        return serde_json::to_value(page).map_err(|e| e.to_string());
    }

    if input.query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }
    let service = ctx
        .extensions
        .get::<VectorSearchExt>()
        .ok_or_else(|| "Vector search is not available.".to_string())?;
    let hits = service
        .0
        .search(input.query.trim(), 128, input.max_distance)?;

    if hits.is_empty() {
        let page: crate::tools::cursor::CursorPage<VectorSearchHit> =
            crate::tools::cursor::CursorPage {
                items: Vec::new(),
                count: 0,
                total: 0,
                cursor: None,
                hint: Some(crate::tools::registry::builtin::strings::FINAL_PAGE_HINT.to_string()),
            };
        return serde_json::to_value(page).map_err(|e| e.to_string());
    }

    let page = ctx
        .cache()
        .vector_sessions
        .create_session(hits, ctx.uuid_gen().as_ref());
    serde_json::to_value(page).map_err(|e| e.to_string())
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
#[path = "vector_search_tests.rs"]
mod tests;
