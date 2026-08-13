//! CardDAV contact tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::{ToolConfigSpec, ToolDescriptor};
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::sync::{Arc, OnceLock};

use super::strings;

fn build_contact_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = ToolGroupId::Internal(InternalToolGroup::Contacts);
    ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        crate::app::tool_specs::contacts_spec(),
        group,
    )
}

/// Tool that searches contacts by keyword.
pub(crate) struct SearchContactTool;
impl Tool for SearchContactTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
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
/// with `ConfigPredicate::DavOrJmapClients` (via
/// [`crate::app::tool_specs::contacts_spec`]).
pub(crate) struct DeleteContactTool;
impl Tool for DeleteContactTool {
    fn descriptor(&self) -> &ToolDescriptor {
        use crate::agent::tools::descriptor::ConfigPredicate;
        use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            let group = ToolGroupId::Internal(InternalToolGroup::Contacts);
            let spec = ToolConfigSpec {
                group: Some(group.clone()),
                requires: vec![ConfigPredicate::Never],
                prompt_rule: None,
            };
            ToolDescriptor::new::<dtos::DeleteContactInput>(
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

/// Self-registering provider for the CardDAV contacts family.
pub(crate) struct CardDavProvider;
impl ToolProvider for CardDavProvider {
    fn id(&self) -> &'static str {
        "carddav"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Contacts)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(SearchContactTool),
            registered(AddContactTool),
            registered(GetContactTool),
            registered(UpdateContactTool),
            registered(DeleteContactTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
