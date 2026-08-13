//! CardDAV contact tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolConfigSpec;
use crate::agent::tools::dtos;
use std::sync::OnceLock;

use super::strings;

/// Spec for the contact family. Enabled when the contacts group is
/// on AND either the JMAP or the CalDAV backend is configured —
/// whichever the `useDAVForContacts` feature flag selects. The
/// feature-flag plumbing lives in
/// [`crate::agent::tools::descriptor::ConfigPredicate::DavOrJmapClients`].
fn contact_spec() -> ToolConfigSpec {
    ToolConfigSpec::group_plus_dav_or_jmap(
        crate::agent::tools::registry::groups::ToolGroupId::Internal(
            crate::agent::tools::registry::groups::InternalToolGroup::Contacts,
        ),
    )
}

fn build_contact_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> crate::agent::tools::descriptor::ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = crate::agent::tools::registry::groups::ToolGroupId::Internal(
        crate::agent::tools::registry::groups::InternalToolGroup::Contacts,
    );
    crate::agent::tools::descriptor::ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        contact_spec(),
        group,
    )
}

/// Tool that searches contacts by keyword.
pub(crate) struct SearchContactTool;
impl Tool for SearchContactTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_contact_descriptor::<dtos::SearchContactInput>(
                "search_contact",
                strings::SEARCH_CONTACT_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::card::tool_search_contact(&ctx.config, &input.keyword).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that adds a new contact.
pub(crate) struct AddContactTool;
impl Tool for AddContactTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_contact_descriptor::<dtos::AddContactInput>(
                "add_contact",
                strings::ADD_CONTACT_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let contact_json = serde_json::to_string(&input)
            .map_err(|e| format!("Failed to serialize input: {}", e))?;
        crate::integrations::dav::card::tool_add_contact(&ctx.config, &contact_json).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets contact details by ID.
pub(crate) struct GetContactTool;
impl Tool for GetContactTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_contact_descriptor::<dtos::GetContactInput>(
                "get_contact",
                strings::GET_CONTACT_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::card::tool_get_contact(&ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that updates an existing contact.
pub(crate) struct UpdateContactTool;
impl Tool for UpdateContactTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_contact_descriptor::<dtos::UpdateContactInput>(
                "update_contact",
                strings::UPDATE_CONTACT_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::UpdateContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let contact_json = serde_json::to_string(&input)
            .map_err(|e| format!("Failed to serialize input: {}", e))?;
        crate::integrations::dav::card::tool_update_contact(&ctx.config, &input.id, &contact_json)
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}

/// Tool that deletes a contact by ID.
///
/// **Currently disabled.** The function, DTO, and registration are
/// kept so the tool can be re-enabled in a future release; until then
/// `is_enabled` returns `false` so the LLM never sees the tool. To
/// re-enable, replace the `ConfigPredicate::Never` requirement below
/// with `ConfigPredicate::DavOrJmapClients`.
pub(crate) struct DeleteContactTool;
impl Tool for DeleteContactTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        use crate::agent::tools::descriptor::ConfigPredicate;
        use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            let group = ToolGroupId::Internal(InternalToolGroup::Contacts);
            let spec = ToolConfigSpec {
                group: Some(group.clone()),
                requires: vec![ConfigPredicate::Never],
                prompt_rule: None,
            };
            crate::agent::tools::descriptor::ToolDescriptor::new::<dtos::DeleteContactInput>(
                "delete_contact",
                strings::DELETE_CONTACT_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
                spec,
                group,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::card::tool_delete_contact(&ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
