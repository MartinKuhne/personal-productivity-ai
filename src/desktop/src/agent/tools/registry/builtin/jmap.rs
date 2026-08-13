//! JMAP email tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::{ConfigPredicate, ToolConfigSpec, ToolDescriptor};
use crate::agent::tools::dtos;
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::sync::OnceLock;

use super::strings;

/// Build a `ToolConfigSpec` for an email-family tool: enabled iff
/// `tool_groups.email` is on and at least one JMAP client is
/// configured.
fn email_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Email);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::JmapClientsPresent],
        prompt_rule: None,
    }
}

/// Tool that searches email by keyword, folder, date range, etc.
pub(crate) struct SearchEmailTool;
impl Tool for SearchEmailTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            let group = ToolGroupId::Internal(InternalToolGroup::Email);
            ToolDescriptor::new::<dtos::SearchEmailInput>(
                "search_email",
                strings::SEARCH_EMAIL_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
                email_spec(),
                group,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            let group = ToolGroupId::Internal(InternalToolGroup::Email);
            ToolDescriptor::new::<dtos::GetEmailByIdInput>(
                "get_email_by_id",
                strings::GET_EMAIL_BY_ID_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
                email_spec(),
                group,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            let group = ToolGroupId::Internal(InternalToolGroup::Email);
            ToolDescriptor::new::<dtos::SendEmailInput>(
                "send_email",
                strings::SEND_EMAIL_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
                email_spec(),
                group,
            )
        })
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
