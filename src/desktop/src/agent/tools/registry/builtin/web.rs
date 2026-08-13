//! Web tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that delegates web searches and fetches to a sub-agent.
#[derive(ToolDescriptor)]
#[tool(
    name = "web_delegate",
    desc = strings::WEB_DELEGATE_DESCRIPTION,
    input = dtos::WebDelegateInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Web,
    execute_with = execute_web_delegate,
)]
pub(crate) struct WebDelegateTool;
fn execute_web_delegate(
    _self: &WebDelegateTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::WebDelegateInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::web::tool_web_delegate(&ctx.config, &input.instruction, &ctx.cache()).map(
        |r| serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
    )
}

/// Tool that fetches content from a URL and converts to Markdown.
#[derive(ToolDescriptor)]
#[tool(
    name = "web_fetch",
    desc = strings::WEB_FETCH_DESCRIPTION,
    input = dtos::WebFetchInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Web,
    execute_with = execute_web_fetch,
)]
pub(crate) struct WebFetchTool;
fn execute_web_fetch(
    _self: &WebFetchTool,
    _ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::WebFetchInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::web::tool_web_fetch(&input, &_ctx.cache(), _ctx.uuid_gen().as_ref()).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that searches the web using SearXNG.
///
/// The descriptor carries [`crate::app::tool_specs::web_search_spec`]
/// so the tool is hidden when no `searxng_url` is configured.
/// Because [`crate::agent::tools::Tool::is_enabled`] now derives
/// from the spec, the duplicated if config.searxng_url.is_some() {branch in `execute` is gone — `is_enabled` is the single source
/// of truth. The single `as_deref().ok_or_else` below just unwraps
/// the value the function needs; if the spec is ever bypassed it
/// surfaces a clear error instead of panicking.
#[derive(ToolDescriptor)]
#[tool(
    name = "web_search",
    desc = strings::WEB_SEARCH_DESCRIPTION,
    input = dtos::WebSearchInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Web,
    config = crate::app::tool_specs::web_search_spec(),
    execute_with = execute_web_search,
)]
pub(crate) struct WebSearchTool;
fn execute_web_search(
    _self: &WebSearchTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::WebSearchInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let url = ctx
        .config
        .searxng_url
        .as_deref()
        .ok_or_else(|| "web_search is disabled (no SearXNG URL configured).".to_string())?;
    crate::agent::tools::web::tool_web_search(url, &input.query).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the web family.
pub(crate) struct WebProvider;
impl ToolProvider for WebProvider {
    fn id(&self) -> &'static str {
        "web"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Web)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(WebDelegateTool),
            registered(WebFetchTool),
            registered(WebSearchTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
