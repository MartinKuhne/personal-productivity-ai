//! JMAP email tool implementations for the tool registry.
//!
//! Unit tests live in the sibling `jmap_tests.rs` sidecar.

use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that searches email by keyword, folder, date range, etc.
#[derive(ToolDescriptor)]
#[tool(
    name = "search_email",
    desc = strings::SEARCH_EMAIL_DESCRIPTION,
    input = dtos::SearchEmailInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Email,
    config = crate::tools::specs::email_spec(),
    execute_with = execute_search_email,
)]
pub(crate) struct SearchEmailTool;
fn execute_search_email(
    _self: &SearchEmailTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::SearchEmailInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::tools::jmap::tool_search_email(
        &ctx.config,
        crate::tools::jmap::SearchEmailFilters {
            keyword: input.keyword.as_deref(),
            folder: input.folder.as_deref(),
            start_date: input.start_date.as_deref(),
            end_date: input.end_date.as_deref(),
            from: input.from.as_deref(),
            to: input.to.as_deref(),
            is_unread: input.is_unread,
            is_flagged: input.is_flagged,
        },
        input.cursor,
        &ctx.cache(),
        ctx.uuid_gen().as_ref(),
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that gets an email by ID.
#[derive(ToolDescriptor)]
#[tool(
    name = "get_email_by_id",
    desc = strings::GET_EMAIL_BY_ID_DESCRIPTION,
    input = dtos::GetEmailByIdInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Email,
    config = crate::tools::specs::email_spec(),
    execute_with = execute_get_email_by_id,
)]
pub(crate) struct GetEmailByIdTool;
fn execute_get_email_by_id(
    _self: &GetEmailByIdTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::GetEmailByIdInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::tools::jmap::tool_get_email_by_id(&ctx.config, &input.id).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that sends an email via JMAP.
#[derive(ToolDescriptor)]
#[tool(
    name = "send_email",
    desc = strings::SEND_EMAIL_DESCRIPTION,
    input = dtos::SendEmailInput,
    safety = crate::tools::Safety::Mutating,
    group = Email,
    config = crate::tools::specs::email_spec(),
    execute_with = execute_send_email,
)]
pub(crate) struct SendEmailTool;
fn execute_send_email(
    _self: &SendEmailTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::SendEmailInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::tools::jmap::tool_send_email(&ctx.config, &input.to, &input.subject, &input.body).map(
        |r| serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
    )
}

/// Self-registering provider for the JMAP email family.
pub(crate) struct JmapProvider;
impl ToolProvider for JmapProvider {
    fn id(&self) -> &'static str {
        "jmap"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Email)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(SearchEmailTool),
            registered(GetEmailByIdTool),
            registered(SendEmailTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}

#[cfg(test)]
#[path = "jmap_tests.rs"]
mod tests;
