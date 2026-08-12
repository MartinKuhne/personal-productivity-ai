//! JMAP email tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::config::AppConfig;
use std::any::TypeId;

use super::json_schema;
use super::strings;

/// Tool that searches email by keyword, folder, date range, etc.
pub(crate) struct SearchEmailTool;
impl Tool for SearchEmailTool {
    fn name(&self) -> &'static str {
        "search_email"
    }
    fn description(&self) -> &'static str {
        strings::SEARCH_EMAIL_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchEmailInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::SearchEmailInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.email && !config.jmap_clients.is_empty()
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchEmailInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::jmap::tool_search_email(
            &ctx.config,
            crate::agent::tools::jmap::SearchEmailFilters {
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
            &ctx.cache,
            ctx.uuid_gen.as_ref(),
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets an email by ID.
pub(crate) struct GetEmailByIdTool;
impl Tool for GetEmailByIdTool {
    fn name(&self) -> &'static str {
        "get_email_by_id"
    }
    fn description(&self) -> &'static str {
        strings::GET_EMAIL_BY_ID_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetEmailByIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GetEmailByIdInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.email && !config.jmap_clients.is_empty()
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetEmailByIdInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::jmap::tool_get_email_by_id(&ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that sends an email via JMAP.
pub(crate) struct SendEmailTool;
impl Tool for SendEmailTool {
    fn name(&self) -> &'static str {
        "send_email"
    }
    fn description(&self) -> &'static str {
        strings::SEND_EMAIL_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SendEmailInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::SendEmailInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.email && !config.jmap_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SendEmailInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::jmap::tool_send_email(
            &ctx.config,
            &input.to,
            &input.subject,
            &input.body,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
