//! Web tool implementations for the tool registry.

use crate::config::AppConfig;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use std::any::TypeId;

use super::json_schema;

/// Tool that delegates web searches and fetches to a sub-agent.
pub(crate) struct WebDelegateTool;
impl Tool for WebDelegateTool {
    fn name(&self) -> &'static str {
        "web_delegate"
    }
    fn description(&self) -> &'static str {
        "Delegate web searches and web fetches to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebDelegateInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::WebDelegateInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.web
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebDelegateInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::web::tool_web_delegate(ctx.config, &input.instruction).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that fetches content from a URL and converts to Markdown.
pub(crate) struct WebFetchTool;
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }
    fn description(&self) -> &'static str {
        "Fetch content from a URL and convert to Markdown. Supports pagination via limit/offset to save context — fetch once, then read sections. Response includes total_lines for pagination. Content is cached for 5 minutes; use force_refetch=true to bypass cache."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebFetchInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::WebFetchInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.web
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, _ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebFetchInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::web::tool_web_fetch(&input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that searches the web using SearXNG.
pub(crate) struct WebSearchTool;
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }
    fn description(&self) -> &'static str {
        "Search the web using SearXNG."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebSearchInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::WebSearchInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.web && config.searxng_url.is_some()
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebSearchInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        if let Some(url) = &ctx.config.searxng_url {
            crate::agent::tools::web::tool_web_search(url, &input.query).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
        } else {
            Err("web_search tool is disabled (no SearXNG URL configured).".to_string())
        }
    }
}
