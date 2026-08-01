//! User-visible description strings for the JMAP email tool family.

pub const SEARCH_EMAIL_DESCRIPTION: &str = "Search email by any combination of keyword, folder (mailbox), date range, sender, recipient, unread status, or flagged status. All filters are combined with AND. At least one filter must be provided. Results are paginated (default page size 10); every response includes the total number of matching emails so the caller can drive follow-up page requests.";

pub const GET_EMAIL_BY_ID_DESCRIPTION: &str = "Get email by id.";

pub const SEND_EMAIL_DESCRIPTION: &str = "Send an email.";

pub const FIELD_SEARCH_EMAIL_INPUT_KEYWORD: &str = "Full-text search keyword. Matches against subject, body, and common headers (From, To, Cc, etc.) per JMAP `text` filter rules.";

pub const FIELD_SEARCH_EMAIL_INPUT_FOLDER: &str = "Optional mailbox/folder name (e.g. \"Inbox\", \"Sent\"). Looked up case-insensitively against the server's mailbox list.";

pub const FIELD_SEARCH_EMAIL_INPUT_START_DATE: &str =
    "Inclusive lower bound on `receivedAt` (ISO `YYYY-MM-DD` or full RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_END_DATE: &str =
    "Inclusive upper bound on `receivedAt` (ISO `YYYY-MM-DD` or full RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_FROM: &str =
    "Filter by the `From` header (substring match per JMAP).";

pub const FIELD_SEARCH_EMAIL_INPUT_TO: &str =
    "Filter by the `To` header (substring match per JMAP).";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_UNREAD: &str = "If `Some(true)`, only return unread email. If `Some(false)`, only return email that has been read.";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_FLAGGED: &str = "If `Some(true)`, only return flagged/starred email. If `Some(false)`, only return email that is not flagged.";

pub const FIELD_SEARCH_EMAIL_INPUT_PAGE: &str =
    "1-indexed page number. Defaults to `1` if omitted.";

pub const FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE: &str = "Number of results per page. Defaults to `10` if omitted. The total number of matching emails across all pages is returned in the `total` field.";

pub const FIELD_SEARCH_EMAIL_RESPONSE_TOTAL: &str = "Total number of matching emails across all pages. Use this together with `page` / `page_size` to drive follow-up page requests.";

pub const FIELD_SEARCH_EMAIL_RESPONSE_HINT: &str = "When the requested page is past the end, this field is set to a human-readable hint. `None` when the page is in range.";
