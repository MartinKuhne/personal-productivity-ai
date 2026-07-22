//! Input/output data-transfer objects for every tool — `serde` and `JsonSchema` derives for LLM argument serialisation.

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
    /// The search term.
    pub query: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct GrepResponse {
    pub matches: String,
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
    /// 1-indexed page number. Defaults to `1` if omitted.
    pub page: Option<usize>,
    /// Number of files to return per page. Defaults to `20` if omitted.
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListFilesByTagResponse {
    /// JSON array of virtual file paths for the requested page (no
    /// library prefix is applied when the result is empty).
    #[serde(default)]
    pub files: Vec<String>,
    /// Total number of files matching the tag, across all pages. This
    /// is returned on every response so the caller can size follow-up
    /// page requests without having to read the whole library.
    pub total: usize,
    /// When the requested `page` is past the end, this field is set
    /// to a human-readable hint explaining why `files` is empty. When
    /// the page is in range, the field is `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ListFilesInput {
    pub path: String,
    /// 1-indexed page number. Defaults to `1` if omitted.
    pub page: Option<usize>,
    /// Number of files to return per page. Defaults to `20` if omitted.
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListFilesResponse {
    /// JSON array of virtual file paths for the requested page.
    #[serde(default)]
    pub files: Vec<String>,
    /// Total number of files in the requested directory (non-recursive),
    /// across all pages. Returned on every response so the caller can
    /// size follow-up page requests.
    pub total: usize,
    /// When the requested `page` is past the end, this field is set
    /// to a human-readable hint. `None` when the page is in range.
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
    /// The search term.
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
    /// Full-text search keyword. Matches against subject, body, and
    /// common headers (From, To, Cc, etc.) per JMAP `text` filter rules.
    pub keyword: Option<String>,
    /// Optional mailbox/folder name (e.g. "Inbox", "Sent"). Looked up
    /// case-insensitively against the server's mailbox list.
    pub folder: Option<String>,
    /// Inclusive lower bound on `receivedAt` (ISO `YYYY-MM-DD` or full
    /// RFC 3339 timestamp).
    pub start_date: Option<String>,
    /// Inclusive upper bound on `receivedAt` (ISO `YYYY-MM-DD` or full
    /// RFC 3339 timestamp).
    pub end_date: Option<String>,
    /// Filter by the `From` header (substring match per JMAP).
    pub from: Option<String>,
    /// Filter by the `To` header (substring match per JMAP).
    pub to: Option<String>,
    /// If `Some(true)`, only return unread email. If `Some(false)`, only
    /// return email that has been read.
    pub is_unread: Option<bool>,
    /// If `Some(true)`, only return flagged/starred email. If
    /// `Some(false)`, only return email that is not flagged.
    pub is_flagged: Option<bool>,
    /// 1-indexed page number. Defaults to `1` if omitted.
    pub page: Option<usize>,
    /// Number of results per page. Defaults to `10` if omitted. The
    /// total number of matching emails across all pages is returned
    /// in the `total` field.
    pub page_size: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchEmailResponse {
    pub results: String,
    /// Total number of matching emails across all pages. Use this
    /// together with `page` / `page_size` to drive follow-up page
    /// requests.
    pub total: usize,
    /// When the requested page is past the end, this field is set
    /// to a human-readable hint. `None` when the page is in range.
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
