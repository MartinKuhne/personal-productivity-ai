//! Input/output data-transfer objects for every tool â€” `serde` and `JsonSchema` derives for LLM argument serialisation.
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
pub struct SearchNotesInput {
    #[schemars(description = strings::FIELD_SEARCH_NOTES_INPUT_QUERY)]
    pub query: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchNotesResponse {
    #[schemars(description = strings::FIELD_SEARCH_NOTES_RESPONSE_MATCHES)]
    pub matches: String,
    #[schemars(description = strings::FIELD_SEARCH_NOTES_RESPONSE_TOTAL)]
    pub total: usize,
    #[schemars(description = strings::FIELD_SEARCH_NOTES_RESPONSE_TRUNCATED)]
    pub truncated: bool,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadTagsInput {}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadTagsResponse {
    pub tags: Vec<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ListNotesByTagInput {
    pub tag: String,
    #[schemars(description = strings::FIELD_OFFSET_DESCRIPTION)]
    pub offset: Option<usize>,
    #[schemars(description = strings::FIELD_LIMIT_DESCRIPTION)]
    pub limit: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListNotesByTagResponse {
    #[schemars(description = strings::FIELD_LIST_NOTES_BY_TAG_RESPONSE_FILES)]
    #[serde(default)]
    pub files: Vec<String>,
    #[schemars(description = strings::FIELD_TOTAL_DESCRIPTION)]
    pub total: usize,
    #[schemars(description = strings::FIELD_HINT_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ListNotesInput {
    pub path: String,
    #[schemars(description = strings::FIELD_OFFSET_DESCRIPTION)]
    pub offset: Option<usize>,
    #[schemars(description = strings::FIELD_LIMIT_DESCRIPTION)]
    pub limit: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ListNotesResponse {
    #[schemars(description = strings::FIELD_LIST_NOTES_RESPONSE_FILES)]
    #[serde(default)]
    pub files: Vec<String>,
    #[schemars(description = strings::FIELD_TOTAL_DESCRIPTION)]
    pub total: usize,
    #[schemars(description = strings::FIELD_HINT_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct ReadNoteInput {
    pub path: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct ReadNoteResponse {
    pub content: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WindowNoteInput {
    pub path: String,
    #[schemars(description = strings::FIELD_OFFSET_DESCRIPTION)]
    pub offset: Option<usize>,
    #[schemars(description = strings::FIELD_LIMIT_DESCRIPTION)]
    pub limit: Option<usize>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WindowNoteResponse {
    pub content: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct CreateNoteInput {
    pub path: String,
    pub content: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct CreateNoteResponse {
    pub result: String,
    pub size_bytes: u64,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct InsertIntoNoteInput {
    pub path: String,
    /// 0-indexed position in the file at which to insert `lines`.
    /// `offset == 0` inserts at the top; `offset == lines.len()` appends.
    pub offset: usize,
    pub lines: Vec<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct InsertIntoNoteResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebFetchInput {
    pub url: String,
    #[serde(default)]
    pub headers: bool,
    #[serde(default)]
    pub force_refetch: bool,
    #[schemars(description = strings::FIELD_CURSOR_DESCRIPTION)]
    #[serde(default)]
    pub cursor: Option<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WebFetchResponse {
    pub content: String,
    #[schemars(description = strings::FIELD_WEB_FETCH_RESPONSE_TOTAL_LINES)]
    pub total_lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[schemars(description = strings::FIELD_HINT_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<std::collections::HashMap<String, String>>,
    #[schemars(description = strings::FIELD_WEB_FETCH_RESPONSE_FROM_CACHE)]
    pub from_cache: bool,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebSearchInput {
    #[schemars(description = strings::FIELD_WEB_SEARCH_INPUT_QUERY)]
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

#[derive(Deserialize, Serialize, Debug, JsonSchema)]
pub struct AddCalendarItemInput {
    #[schemars(description = strings::FIELD_CALENDAR_SUMMARY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_START_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_END_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_DESCRIPTION_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_LOCATION_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}
#[derive(Serialize, Debug, JsonSchema, PartialEq)]
pub struct AddCalendarItemResponse {
    pub result: String,
}

#[derive(Deserialize, Serialize, Debug, JsonSchema)]
pub struct UpdateCalendarItemInput {
    #[schemars(description = strings::FIELD_CALENDAR_HREF_DESC)]
    pub id: String,
    #[schemars(description = strings::FIELD_CALENDAR_SUMMARY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_START_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_END_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_DESCRIPTION_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[schemars(description = strings::FIELD_CALENDAR_LOCATION_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
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
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_KEYWORD)]
    pub keyword: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_FOLDER)]
    pub folder: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_START_DATE)]
    pub start_date: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_END_DATE)]
    pub end_date: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_FROM)]
    pub from: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_TO)]
    pub to: Option<String>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_IS_UNREAD)]
    pub is_unread: Option<bool>,
    #[schemars(description = strings::FIELD_SEARCH_EMAIL_INPUT_IS_FLAGGED)]
    pub is_flagged: Option<bool>,
    #[schemars(description = strings::FIELD_CURSOR_DESCRIPTION)]
    pub cursor: Option<String>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct SearchEmailResponse {
    pub results: String,
    #[schemars(description = strings::FIELD_TOTAL_DESCRIPTION)]
    pub total: usize,
    #[schemars(description = strings::FIELD_CURSOR_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[schemars(description = strings::FIELD_HINT_DESCRIPTION)]
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

#[derive(Deserialize, Serialize, Debug, JsonSchema)]
pub struct AddressInput {
    #[schemars(description = strings::FIELD_ADDRESS_TYPE_DESC)]
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub addr_type: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_STREET_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_CITY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_REGION_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_POSTAL_CODE_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_COUNTRY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_PO_BOX_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_box: Option<String>,
    #[schemars(description = strings::FIELD_ADDRESS_EXT_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, JsonSchema)]
pub struct AddContactInput {
    #[schemars(description = strings::FIELD_CONTACT_NAME_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_EMAIL_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_PHONE_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_COMPANY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_TITLE_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_NOTES_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_BIRTHDAY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_ADDRESSES_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<AddressInput>>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct AddContactResponse {
    pub result: String,
}

#[derive(Deserialize, Serialize, Debug, JsonSchema)]
pub struct UpdateContactInput {
    #[schemars(description = strings::FIELD_CONTACT_HREF_DESC)]
    #[serde(skip_serializing)]
    pub id: String,
    #[schemars(description = strings::FIELD_CONTACT_NAME_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_EMAIL_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_PHONE_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_COMPANY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_TITLE_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_NOTES_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_BIRTHDAY_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,
    #[schemars(description = strings::FIELD_CONTACT_ADDRESSES_DESC)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<AddressInput>>,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct UpdateContactResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct DeleteContactInput {
    /// The href of the contact to delete. Use the value returned by
    /// `get_contact` or `search_contact`.
    pub id: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct DeleteContactResponse {
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
pub struct PatchNoteInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct PatchNoteResponse {
    pub result: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct WebDelegateInput {
    pub instruction: String,
}
#[derive(Serialize, Debug, JsonSchema)]
pub struct WebDelegateResponse {
    pub result: String,
    /// Structured trace of sub-agent tool calls (FR-014, SC-006).
    #[serde(default)]
    pub tool_calls: Vec<crate::agent::events::DelegateToolCall>,
}

// ---------------------------------------------------------------------------
// Browser automation tools (BRWS-001..008)
// ---------------------------------------------------------------------------

/// `browser_navigate` input â€” drive the persistent page to a URL.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserNavigateInput {
    #[schemars(description = strings::FIELD_BROWSER_NAVIGATE_INPUT_URL)]
    pub url: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserNavigateResponse {
    #[schemars(description = strings::FIELD_BROWSER_NAVIGATE_RESPONSE_URL)]
    pub url: String,
    #[schemars(description = strings::FIELD_BROWSER_NAVIGATE_RESPONSE_TITLE)]
    pub title: String,
}

/// `browser_get_page_state` input â€” no parameters.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserGetPageStateInput {}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserGetPageStateResponse {
    #[schemars(description = strings::FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_URL)]
    pub url: String,
    #[schemars(description = strings::FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TITLE)]
    pub title: String,
    #[schemars(description = strings::FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_ELEMENTS)]
    pub elements: String,
    #[schemars(description = strings::FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TOTAL)]
    pub total: usize,
}

/// `browser_click` input â€” CSS selector for a single element.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserClickInput {
    #[schemars(description = strings::FIELD_BROWSER_CLICK_INPUT_SELECTOR)]
    pub selector: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserClickResponse {
    pub result: String,
}

/// `browser_fill_input` input â€” selector + text.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserFillInputInput {
    #[schemars(description = strings::FIELD_BROWSER_FILL_INPUT_INPUT_SELECTOR)]
    pub selector: String,
    #[schemars(description = strings::FIELD_BROWSER_FILL_INPUT_INPUT_TEXT)]
    pub text: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserFillInputResponse {
    pub result: String,
}

/// `browser_select_dropdown` input â€” selector + option value.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserSelectDropdownInput {
    #[schemars(description = strings::FIELD_BROWSER_SELECT_DROPDOWN_INPUT_SELECTOR)]
    pub selector: String,
    #[schemars(description = strings::FIELD_BROWSER_SELECT_DROPDOWN_INPUT_VALUE)]
    pub value: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserSelectDropdownResponse {
    pub result: String,
}

/// `browser_press_key` input â€” keyboard key.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserPressKeyInput {
    #[schemars(description = strings::FIELD_BROWSER_PRESS_KEY_INPUT_KEY)]
    pub key: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserPressKeyResponse {
    pub result: String,
}

/// `browser_evaluate_js` input â€” arbitrary JS expression.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserEvaluateJsInput {
    #[schemars(description = strings::FIELD_BROWSER_EVALUATE_JS_INPUT_SCRIPT)]
    pub script: String,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserEvaluateJsResponse {
    /// JSON-encoded result of the script. May be `null`.
    pub result: String,
}

/// `browser_screenshot` input â€” restricted filename.
#[derive(Deserialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserScreenshotInput {
    #[schemars(description = strings::FIELD_BROWSER_SCREENSHOT_INPUT_FILENAME)]
    pub filename: String,
    #[serde(default)]
    #[schemars(description = strings::FIELD_BROWSER_SCREENSHOT_INPUT_FULL_PAGE)]
    pub full_page: bool,
}
#[derive(Serialize, Debug, JsonSchema)]
#[cfg(feature = "browser")]
pub struct BrowserScreenshotResponse {
    #[schemars(description = strings::FIELD_BROWSER_SCREENSHOT_RESPONSE_PATH)]
    pub path: String,
    #[schemars(description = strings::FIELD_BROWSER_SCREENSHOT_RESPONSE_BYTES)]
    pub bytes: usize,
}
