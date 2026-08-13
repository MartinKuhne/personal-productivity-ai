//! JMAP email tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolDescriptor;
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::sync::{Arc, OnceLock};

use super::strings;

fn build_email_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = ToolGroupId::Internal(InternalToolGroup::Email);
    ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        crate::app::tool_specs::email_spec(),
        group,
    )
}

/// Tool that searches email by keyword, folder, date range, etc.
pub(crate) struct SearchEmailTool;
impl Tool for SearchEmailTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_email_descriptor::<dtos::SearchEmailInput>(
                "search_email",
                strings::SEARCH_EMAIL_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
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
            build_email_descriptor::<dtos::GetEmailByIdInput>(
                "get_email_by_id",
                strings::GET_EMAIL_BY_ID_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
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
            build_email_descriptor::<dtos::SendEmailInput>(
                "send_email",
                strings::SEND_EMAIL_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
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
