//! Trello API tools for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::any::TypeId;

use super::json_schema;
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

fn trello_tools_enabled(config: &AppConfig) -> bool {
    let enabled = config.tool_groups.trello && config.trello_client.is_some();
    tracing::info!(
        "TRELLO_TOOLS_ENABLED: config.tool_groups.trello={}, config.trello_client.is_some()={}, result={}",
        config.tool_groups.trello,
        config.trello_client.is_some(),
        enabled
    );
    enabled
}

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

    let url = format!(
        "https://api.trello.com/1{}?key={}&token={}",
        endpoint, client_config.api_key, client_config.token
    );
    let safe_url = format!("https://api.trello.com/1{}", endpoint);

    tracing::debug!(name = "trello.request", method = %method, url = %safe_url, "Sending request to Trello API");

    let client = reqwest::blocking::Client::new();
    let mut req = client.request(method.clone(), &url);
    if let Some(b) = body {
        req = req
            .header("Content-Type", "application/json")
            .body(b.to_string());
    }

    let res = req.send().map_err(|e| {
        tracing::error!(name = "trello.request.error", error = %e, url = %safe_url, "Trello request failed");
        e.to_string()
    })?;

    let status = res.status();
    tracing::debug!(name = "trello.response", status = %status, url = %safe_url, "Received response from Trello API");

    if status.is_success() {
        let text = res.text().map_err(|e| {
            tracing::error!(name = "trello.response.read_error", error = %e, "Failed to read Trello response text");
            e.to_string()
        })?;
        serde_json::from_str(&text).map_err(|e| {
            tracing::error!(name = "trello.response.parse_error", error = %e, text = %text, "Failed to parse Trello JSON");
            e.to_string()
        })
    } else {
        let error_text = res.text().unwrap_or_default();
        tracing::error!(name = "trello.response.status_error", status = %status, url = %safe_url, response = %error_text, "Trello API returned error status");
        Err(format!("Trello API error: {} - {}", status, error_text))
    }
}

pub(crate) struct TrelloGetBoardsTool;
impl Tool for TrelloGetBoardsTool {
    fn name(&self) -> &'static str {
        "trello_get_boards"
    }
    fn description(&self) -> &'static str {
        strings::GET_BOARDS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloEmptyInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloEmptyInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let _input: TrelloEmptyInput = serde_json::from_str(args).unwrap_or(TrelloEmptyInput {});
        trello_request(ctx, reqwest::Method::GET, "/members/me/boards", None)
    }
}

pub(crate) struct TrelloGetBoardTool;
impl Tool for TrelloGetBoardTool {
    fn name(&self) -> &'static str {
        "trello_get_board"
    }
    fn description(&self) -> &'static str {
        strings::GET_BOARD_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloIdInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "trello_get_lists"
    }
    fn description(&self) -> &'static str {
        strings::GET_LISTS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloIdInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "trello_get_cards"
    }
    fn description(&self) -> &'static str {
        strings::GET_CARDS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloIdInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "trello_create_card"
    }
    fn description(&self) -> &'static str {
        strings::CREATE_CARD_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloCreateCardInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloCreateCardInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
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
    fn name(&self) -> &'static str {
        "trello_update_card"
    }
    fn description(&self) -> &'static str {
        strings::UPDATE_CARD_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloUpdateCardInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloUpdateCardInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
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
    fn name(&self) -> &'static str {
        "trello_delete_card"
    }
    fn description(&self) -> &'static str {
        strings::DELETE_CARD_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<TrelloIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<TrelloIdInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        trello_tools_enabled(config)
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

    #[test]
    fn test_trello_tools_enabled() {
        let mut config = AppConfig {
            tool_groups: ToolGroupsConfig {
                trello: true,
                ..Default::default()
            },
            ..Default::default()
        };
        config.trello_client = None;
        assert!(!trello_tools_enabled(&config));

        config.trello_client = Some(TrelloClient {
            token: "t".to_string(),
            api_key: "s".to_string(),
        });
        assert!(trello_tools_enabled(&config));

        config.tool_groups.trello = false;
        assert!(!trello_tools_enabled(&config));
    }
}
