//! JMAP email tool implementations for the tool registry.

use crate::config::AppConfig;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use std::any::TypeId;

use super::json_schema;

/// Tool that searches email by keyword, folder, date range, etc.
pub(crate) struct SearchEmailTool;
impl Tool for SearchEmailTool {
    fn name(&self) -> &'static str {
        "search_email"
    }
    fn description(&self) -> &'static str {
        "Search email by any combination of keyword, folder (mailbox), date range, sender, recipient, unread status, or flagged status. All filters are combined with AND. At least one filter must be provided. Results are paginated (default page size 10); every response includes the total number of matching emails so the caller can drive follow-up page requests."
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
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input.page_size.unwrap_or(10).max(1);
        crate::agent::tools::jmap::tool_search_email(
            ctx.config,
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
            crate::agent::tools::jmap::SearchEmailPagination { page, page_size },
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
        "Get email by id."
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
        crate::agent::tools::jmap::tool_get_email_by_id(ctx.config, &input.id).map(|r| {
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
        "Send an email."
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
        crate::agent::tools::jmap::tool_send_email(ctx.config, &input.to, &input.subject, &input.body).map(
            |r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            },
        )
    }
}
