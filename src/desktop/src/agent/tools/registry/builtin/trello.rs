//! Trello API tools for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolConfigSpec;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::strings;

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct TrelloEmptyInput {}

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct TrelloIdInput {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct TrelloCreateCardInput {
    #[serde(rename = "idList")]
    pub id_list: String,
    pub name: String,
    pub desc: Option<String>,
    #[serde(rename = "idLabels")]
    pub id_labels: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct TrelloUpdateCardInput {
    pub id: String,
    pub name: Option<String>,
    pub desc: Option<String>,
    #[serde(rename = "idList")]
    pub id_list: Option<String>,
}

/// Spec for the Trello family. Enabled when the trello group is on
/// AND `trello_client` is configured.
fn trello_spec() -> ToolConfigSpec {
    let group = crate::agent::tools::registry::groups::ToolGroupId::Internal(
        crate::agent::tools::registry::groups::InternalToolGroup::Trello,
    );
    ToolConfigSpec::group_plus_trello(group)
}

fn build_trello_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> crate::agent::tools::descriptor::ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = crate::agent::tools::registry::groups::ToolGroupId::Internal(
        crate::agent::tools::registry::groups::InternalToolGroup::Trello,
    );
    crate::agent::tools::descriptor::ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        trello_spec(),
        group,
    )
}

/// Pull the [`crate::config::TrelloClientConfig`] out of [`ToolContext`] and
/// delegate the call to the protocol-layer client.
fn trello_request(
    ctx: &ToolContext,
    method: reqwest::Method,
    endpoint: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let client_config = ctx
        .config
        .trello_client
        .as_ref()
        .ok_or_else(|| "Trello configuration missing".to_string())?;
    crate::integrations::trello::trello_request(client_config, method, endpoint, body)
}

pub(crate) struct TrelloGetBoardsTool;
impl Tool for TrelloGetBoardsTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloEmptyInput>(
                "trello_get_boards",
                strings::GET_BOARDS_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let _input: TrelloEmptyInput = serde_json::from_str(args).unwrap_or(TrelloEmptyInput {});
        trello_request(ctx, reqwest::Method::GET, "/members/me/boards", None)
    }
}

pub(crate) struct TrelloGetBoardTool;
impl Tool for TrelloGetBoardTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloIdInput>(
                "trello_get_board",
                strings::GET_BOARD_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
        trello_request(
            ctx,
            reqwest::Method::GET,
            &format!("/boards/{}", input.id),
            None,
        )
    }
}

pub(crate) struct TrelloGetListsTool;
impl Tool for TrelloGetListsTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloIdInput>(
                "trello_get_lists",
                strings::GET_LISTS_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
        trello_request(
            ctx,
            reqwest::Method::GET,
            &format!("/boards/{}/lists", input.id),
            None,
        )
    }
}

pub(crate) struct TrelloGetCardsTool;
impl Tool for TrelloGetCardsTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloIdInput>(
                "trello_get_cards",
                strings::GET_CARDS_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
        trello_request(
            ctx,
            reqwest::Method::GET,
            &format!("/lists/{}/cards", input.id),
            None,
        )
    }
}

pub(crate) struct TrelloCreateCardTool;
impl Tool for TrelloCreateCardTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloCreateCardInput>(
                "trello_create_card",
                strings::CREATE_CARD_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let mut input: TrelloCreateCardInput =
            serde_json::from_str(args).map_err(|e| e.to_string())?;

        // Try to attach 'FastMD' label
        if let Ok(list_val) = trello_request(
            ctx,
            reqwest::Method::GET,
            &format!("/lists/{}", input.id_list),
            None,
        ) && let Some(id_board) = list_val.get("idBoard").and_then(|v| v.as_str())
            && let Ok(labels_val) = trello_request(
                ctx,
                reqwest::Method::GET,
                &format!("/boards/{}/labels", id_board),
                None,
            )
        {
            let mut fastmd_label_id = None;
            if let Some(labels) = labels_val.as_array() {
                for label in labels {
                    if let (Some(name), Some(id)) = (
                        label.get("name").and_then(|v| v.as_str()),
                        label.get("id").and_then(|v| v.as_str()),
                    ) && name == "FastMD"
                    {
                        fastmd_label_id = Some(id.to_string());
                        break;
                    }
                }
            }

            if fastmd_label_id.is_none() {
                let create_label_body = serde_json::json!({
                    "name": "FastMD",
                    "color": "blue",
                    "idBoard": id_board
                });
                if let Ok(new_label_val) = trello_request(
                    ctx,
                    reqwest::Method::POST,
                    "/labels",
                    Some(&create_label_body),
                ) && let Some(id) = new_label_val.get("id").and_then(|v| v.as_str())
                {
                    fastmd_label_id = Some(id.to_string());
                }
            }

            if let Some(label_id) = fastmd_label_id {
                let mut labels = input.id_labels.unwrap_or_default();
                labels.push(label_id);
                input.id_labels = Some(labels);
            }
        }

        let body = serde_json::to_value(input).unwrap();
        trello_request(ctx, reqwest::Method::POST, "/cards", Some(&body))
    }
}

pub(crate) struct TrelloUpdateCardTool;
impl Tool for TrelloUpdateCardTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloUpdateCardInput>(
                "trello_update_card",
                strings::UPDATE_CARD_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: TrelloUpdateCardInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
        let id = input.id.clone();
        let body = serde_json::to_value(input).unwrap();
        trello_request(
            ctx,
            reqwest::Method::PUT,
            &format!("/cards/{}", id),
            Some(&body),
        )
    }
}

pub(crate) struct TrelloDeleteCardTool;
impl Tool for TrelloDeleteCardTool {
    fn descriptor(&self) -> &crate::agent::tools::descriptor::ToolDescriptor {
        static D: OnceLock<crate::agent::tools::descriptor::ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_trello_descriptor::<TrelloIdInput>(
                "trello_delete_card",
                strings::DELETE_CARD_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
        trello_request(
            ctx,
            reqwest::Method::DELETE,
            &format!("/cards/{}", input.id),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ToolGroupsConfig, TrelloClient};

    /// Spec-level test mirroring the previous `trello_tools_enabled`
    /// behaviour: the spec must gate the tool on the group flag AND
    /// the presence of `trello_client`.
    #[test]
    fn test_trello_spec_gating() {
        let mut config = AppConfig {
            tool_groups: ToolGroupsConfig {
                trello: true,
                ..Default::default()
            },
            ..Default::default()
        };
        config.trello_client = None;
        assert!(!trello_spec().is_enabled_for(&config, ""));

        config.trello_client = Some(TrelloClient {
            token: "t".to_string(),
            api_key: "s".to_string(),
        });
        assert!(trello_spec().is_enabled_for(&config, ""));

        config.tool_groups.trello = false;
        assert!(!trello_spec().is_enabled_for(&config, ""));
    }
}
