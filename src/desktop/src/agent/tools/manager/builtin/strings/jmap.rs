//! User-visible description strings for the JMAP email tool family.

pub const SEARCH_EMAIL_DESCRIPTION: &str = super::cursor::SEARCH_EMAIL_CANONICAL_DESCRIPTION;

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
