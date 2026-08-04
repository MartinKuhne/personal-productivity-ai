//! User-visible description strings for the JMAP email tool family.

pub const SEARCH_EMAIL_DESCRIPTION: &str = super::cursor::SEARCH_EMAIL_CANONICAL_DESCRIPTION;

pub const GET_EMAIL_BY_ID_DESCRIPTION: &str = "Get an email by its unique ID.";

pub const SEND_EMAIL_DESCRIPTION: &str = "Send an email message.";

pub const FIELD_SEARCH_EMAIL_INPUT_KEYWORD: &str =
    "Search email subjects, bodies, and headers using text keywords.";

pub const FIELD_SEARCH_EMAIL_INPUT_FOLDER: &str =
    "Specify an optional mailbox folder name (such as Inbox or Sent).";

pub const FIELD_SEARCH_EMAIL_INPUT_START_DATE: &str =
    "Specify an inclusive start date for received emails (ISO YYYY-MM-DD or RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_END_DATE: &str =
    "Specify an inclusive end date for received emails (ISO YYYY-MM-DD or RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_FROM: &str = "Filter emails by sender address substring.";

pub const FIELD_SEARCH_EMAIL_INPUT_TO: &str = "Filter emails by recipient address substring.";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_UNREAD: &str =
    "Set to `true` to return only unread emails, or `false` to return read emails.";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_FLAGGED: &str =
    "Set to `true` to return only flagged emails, or `false` to return unflagged emails.";
