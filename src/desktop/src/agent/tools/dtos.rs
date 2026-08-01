//! Input/output data-transfer objects for every tool — `serde` and `JsonSchema` derives for LLM argument serialisation.
//!
//! Every user-visible string (tool description, per-field description) lives
//! in the per-family `strings` submodule under `registry/builtin/`. The DTOs
//! here reference those consts via `#[schemars(description = ...)]` so the
//! JSON schema the LLM sees is generated from a single source of truth.

use crate::agent::tools::registry::builtin::strings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ToolResponse<T> {
    Success { data: T },
    Error { message: String },
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GrepInput {
    #[schemars(description = strings::fs::FIELD_GREP_INPUT_QUERY)]
    pub query: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GrepResponse {
    #[schemars(description = strings::fs::FIELD_GREP_RESPONSE_MATCHES)]
    pub matches: String,
    #[schemars(description = strings::fs::FIELD_GREP_RESPONSE_TOTAL)]
    pub total: usize,
    #[schemars(description = strings::fs::FIELD_GREP_RESPONSE_TRUNCATED)]
    pub truncated: bool,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadTagsInput {}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadTagsResponse {
    pub tags: Vec<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ListFilesByTagInput {
    pub tag: String,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_BY_TAG_INPUT_PAGE)]
    pub page: Option<usize>,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_BY_TAG_INPUT_PAGE_SIZE)]
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListFilesByTagResponse {
    #[schemars(description = strings::fs::FIELD_LIST_FILES_BY_TAG_RESPONSE_FILES)]
    #[serde(default)]
    pub files: Vec<String>,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_BY_TAG_RESPONSE_TOTAL)]
    pub total: usize,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_BY_TAG_RESPONSE_HINT)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ListFilesInput {
    pub path: String,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_INPUT_PAGE)]
    pub page: Option<usize>,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_INPUT_PAGE_SIZE)]
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListFilesResponse {
    #[schemars(description = strings::fs::FIELD_LIST_FILES_RESPONSE_FILES)]
    #[serde(default)]
    pub files: Vec<String>,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_RESPONSE_TOTAL)]
    pub total: usize,
    #[schemars(description = strings::fs::FIELD_LIST_FILES_RESPONSE_HINT)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadFileInput {
    pub path: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadFileResponse {
    pub content: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadFileLinesInput {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadFileLinesResponse {
    pub content: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct CreateFileInput {
    pub path: String,
    pub content: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct CreateFileResponse {
    pub result: String,
    pub size_bytes: u64,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct InsertLinesInput {
    pub path: String,
    pub line_index: usize,
    pub lines: Vec<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct InsertLinesResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct DeleteLinesInput {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct DeleteLinesResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebFetchInput {
    pub url: String,
    #[serde(default)]
    pub headers: bool,
    #[serde(default)]
    pub force_refetch: bool,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WebFetchResponse {
    pub content: String,
    pub total_lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<std::collections::HashMap<String, String>>,
    pub from_cache: bool,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebSearchInput {
    #[schemars(description = strings::web::FIELD_WEB_SEARCH_INPUT_QUERY)]
    pub query: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WebSearchResponse {
    pub results: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadYamlHeaderInput {
    pub path: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadYamlHeaderResponse {
    pub content: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WriteYamlHeaderInput {
    pub path: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "header-date")]
    pub header_date: Option<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WriteYamlHeaderResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct SearchCalendarInput {
    pub keyword: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchCalendarResponse {
    pub results: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GetCalendarInput {
    pub start_date: String,
    pub end_date: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GetCalendarResponse {
    pub results: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GetCalendarItemInput {
    pub href: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GetCalendarItemResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct AddCalendarItemInput {
    pub item_json: String,
}
#[derive(Serialize, Debug, JsonSchema, PartialEq)]
pub struct AddCalendarItemResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct UpdateCalendarItemInput {
    pub id: String,
    pub update_json: String,
}
#[derive(Serialize, Debug, JsonSchema, PartialEq)]
pub struct UpdateCalendarItemResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct DeleteCalendarItemInput {
    pub id: String,
}
#[derive(Serialize, Debug, JsonSchema, PartialEq)]
pub struct DeleteCalendarItemResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct SearchEmailInput {
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_KEYWORD)]
    pub keyword: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_FOLDER)]
    pub folder: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_START_DATE)]
    pub start_date: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_END_DATE)]
    pub end_date: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_FROM)]
    pub from: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_TO)]
    pub to: Option<String>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_IS_UNREAD)]
    pub is_unread: Option<bool>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_IS_FLAGGED)]
    pub is_flagged: Option<bool>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_PAGE)]
    pub page: Option<usize>,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE)]
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchEmailResponse {
    pub results: String,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_RESPONSE_TOTAL)]
    pub total: usize,
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_RESPONSE_HINT)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GetEmailByIdInput {
    pub id: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GetEmailByIdResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct SendEmailInput {
    pub to: String,
    pub subject: String,
    pub body: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SendEmailResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct DeleteEmailInput {
    pub id: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct DeleteEmailResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct SearchContactInput {
    pub keyword: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchContactResponse {
    pub results: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GetContactInput {
    pub id: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GetContactResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct AddContactInput {
    pub contact_json: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct AddContactResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct GetWeatherInput {
    pub location: String,
    pub date_range: Option<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GetWeatherResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReplaceTextInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReplaceTextResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebDelegateInput {
    pub instruction: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WebDelegateResponse {
    pub result: String,
}
