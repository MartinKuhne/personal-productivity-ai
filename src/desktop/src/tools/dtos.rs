use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "status", rename_all = "lowercase")]
    pub enum ToolResponse<T> {
        Success { data: T },
        Error { message: String },
    }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GrepInput { /// The search term.
pub query: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GrepResponse { pub matches: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ReadTagsInput {}
    #[derive(Serialize, Debug, JsonSchema)] pub struct ReadTagsResponse { pub tags_found: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ListFilesByTagInput { pub tag: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ListFilesByTagResponse { pub files: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ListFilesInput { pub path: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ListFilesResponse { pub files: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ReadFileInput { pub path: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ReadFileResponse { pub content: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ReadFileLinesInput { pub path: String, pub start_line: usize, pub end_line: usize }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ReadFileLinesResponse { pub content: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct CreateFileInput { pub path: String, pub content: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct CreateFileResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct InsertLinesInput { pub path: String, pub line_index: usize, pub lines: Vec<String> }
    #[derive(Serialize, Debug, JsonSchema)] pub struct InsertLinesResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct DeleteLinesInput { pub path: String, pub start_line: usize, pub end_line: usize }
    #[derive(Serialize, Debug, JsonSchema)] pub struct DeleteLinesResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct WebFetchInput { pub url: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct WebFetchResponse { pub content: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct WebSearchInput { /// The search term.
pub query: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct WebSearchResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ReadYamlHeaderInput { pub path: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ReadYamlHeaderResponse { pub content: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct WriteYamlHeaderInput { pub path: String, pub title: Option<String>, pub summary: Option<String>, pub tags: Option<Vec<String>>, #[serde(rename="header-date")] pub header_date: Option<String> }
    #[derive(Serialize, Debug, JsonSchema)] pub struct WriteYamlHeaderResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct SearchCalendarInput { pub keyword: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct SearchCalendarResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetCalendarInput { pub start_date: String, pub end_date: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetCalendarResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetCalendarItemInput { pub href: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetCalendarItemResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct AddCalendarItemInput { pub item_json: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct AddCalendarItemResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct UpdateCalendarItemInput { pub id: String, pub update_json: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct UpdateCalendarItemResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct DeleteCalendarItemInput { pub id: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct DeleteCalendarItemResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct SearchEmailInput { pub keyword: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct SearchEmailResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetEmailByIdInput { pub id: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetEmailByIdResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetEmailInput { pub start_date: Option<String>, pub end_date: Option<String>, pub sender: Option<String>, pub recipient: Option<String>, pub is_unread: Option<bool>, pub is_flagged: Option<bool> }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetEmailResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct SendEmailInput { pub to: String, pub subject: String, pub body: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct SendEmailResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct DeleteEmailInput { pub id: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct DeleteEmailResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct SearchContactInput { pub keyword: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct SearchContactResponse { pub results: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetContactInput { pub id: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetContactResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct AddContactInput { pub contact_json: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct AddContactResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct GetWeatherInput { pub location: String, pub date_range: Option<String> }
    #[derive(Serialize, Debug, JsonSchema)] pub struct GetWeatherResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct ReplaceTextInput { pub path: String, pub old_string: String, pub new_string: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct ReplaceTextResponse { pub result: String }

    #[derive(Deserialize, Debug, JsonSchema)] pub struct WebDelegateInput { pub instruction: String }
    #[derive(Serialize, Debug, JsonSchema)] pub struct WebDelegateResponse { pub result: String }
