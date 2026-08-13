//! Trello API tools for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_get_boards",
    desc = strings::GET_BOARDS_DESCRIPTION,
    input = TrelloEmptyInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_get_boards,
)]
pub(crate) struct TrelloGetBoardsTool;
fn execute_trello_get_boards(
    _self: &TrelloGetBoardsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let _input: TrelloEmptyInput = serde_json::from_str(args).unwrap_or(TrelloEmptyInput {});
    trello_request(ctx, reqwest::Method::GET, "/members/me/boards", None)
}

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_get_board",
    desc = strings::GET_BOARD_DESCRIPTION,
    input = TrelloIdInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_get_board,
)]
pub(crate) struct TrelloGetBoardTool;
fn execute_trello_get_board(
    _self: &TrelloGetBoardTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
    trello_request(
        ctx,
        reqwest::Method::GET,
        &format!("/boards/{}", input.id),
        None,
    )
}

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_get_lists",
    desc = strings::GET_LISTS_DESCRIPTION,
    input = TrelloIdInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_get_lists,
)]
pub(crate) struct TrelloGetListsTool;
fn execute_trello_get_lists(
    _self: &TrelloGetListsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
    trello_request(
        ctx,
        reqwest::Method::GET,
        &format!("/boards/{}/lists", input.id),
        None,
    )
}

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_get_cards",
    desc = strings::GET_CARDS_DESCRIPTION,
    input = TrelloIdInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_get_cards,
)]
pub(crate) struct TrelloGetCardsTool;
fn execute_trello_get_cards(
    _self: &TrelloGetCardsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
    trello_request(
        ctx,
        reqwest::Method::GET,
        &format!("/lists/{}/cards", input.id),
        None,
    )
}

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_create_card",
    desc = strings::CREATE_CARD_DESCRIPTION,
    input = TrelloCreateCardInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_create_card,
)]
pub(crate) struct TrelloCreateCardTool;
fn execute_trello_create_card(
    _self: &TrelloCreateCardTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let mut input: TrelloCreateCardInput = serde_json::from_str(args).map_err(|e| e.to_string())?;

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

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_update_card",
    desc = strings::UPDATE_CARD_DESCRIPTION,
    input = TrelloUpdateCardInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_update_card,
)]
pub(crate) struct TrelloUpdateCardTool;
fn execute_trello_update_card(
    _self: &TrelloUpdateCardTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
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

#[derive(ToolDescriptor)]
#[tool(
    name = "trello_delete_card",
    desc = strings::DELETE_CARD_DESCRIPTION,
    input = TrelloIdInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Trello,
    config = crate::app::tool_specs::trello_spec(),
    execute_with = execute_trello_delete_card,
)]
pub(crate) struct TrelloDeleteCardTool;
fn execute_trello_delete_card(
    _self: &TrelloDeleteCardTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: TrelloIdInput = serde_json::from_str(args).map_err(|e| e.to_string())?;
    trello_request(
        ctx,
        reqwest::Method::DELETE,
        &format!("/cards/{}", input.id),
        None,
    )
}

/// Self-registering provider for the Trello family.
pub(crate) struct TrelloProvider;
impl ToolProvider for TrelloProvider {
    fn id(&self) -> &'static str {
        "trello"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Trello)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(TrelloGetBoardsTool),
            registered(TrelloGetBoardTool),
            registered(TrelloGetListsTool),
            registered(TrelloGetCardsTool),
            registered(TrelloCreateCardTool),
            registered(TrelloUpdateCardTool),
            registered(TrelloDeleteCardTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
