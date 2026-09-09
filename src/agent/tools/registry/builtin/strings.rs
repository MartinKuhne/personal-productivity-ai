//! User-visible description strings for every built-in tool — the single source of truth for the LLM-facing tool description and per-field schema description.
//!
//! Every const lives at the top level of this module. The `Tool::description()` impls in the
//! sibling `builtin/*.rs` files and the `#[schemars(description = ...)]` attributes on the
//! DTO fields in `crate::tools::dtos` and `crate::tools::csv_db::schema`
//! both reference the consts here. Editing a string in this module is the only place to
//! change what the LLM sees for that tool.
//!
//! Consts are grouped by tool family with `// --- <family> ---` headers purely for
//! human readability; every const is at the same Rust scope. Family-prefixed const
//! names (e.g. `FIELD_BROWSER_NAVIGATE_INPUT_URL`) already disambiguate cross-family
//! references, so no submodule is needed and adding a new tool no longer requires
//! creating a new file.
//!
//! Style: descriptions follow ASD-STE100 Simplified Technical English where possible.
//! Short sentences. One instruction per sentence. Imperative verbs. Consistent terms:
//! `note` is a Markdown file in the workspace. `virtual path` is a workspace path.
//! `cursor` is an opaque page token. REQ: TOOL-001..TOOL-010 (tool behaviour).

// --- paging (offset/limit) — canonical across list-paginated tools ---

/// `offset` parameter description. Used on every list-paginated tool's `offset` field.
pub const FIELD_OFFSET_DESCRIPTION: &str =
    "Give the number of items to skip. Use 0 for the first page. Default: 0.";

/// `limit` parameter description. Per-tool default limits are stated in each tool description.
pub const FIELD_LIMIT_DESCRIPTION: &str = "Give the maximum number of items to return. If you omit this value, the tool uses its default limit.";

/// `total` response field description. Used on every list-paginated tool's `total` field.
pub const FIELD_TOTAL_DESCRIPTION: &str = "The total number of items on all pages.";

/// `hint` response field description. Used on every list-paginated tool's `hint` field.
pub const FIELD_HINT_DESCRIPTION: &str =
    "Status data for the page. It appears on the last page or when there are no results.";

/// `results` response field description for `search_email`: a JSON array of the
/// matching emails on this page, one `{ client, preview }` object per email.
pub const FIELD_SEARCH_EMAIL_RESULTS_DESCRIPTION: &str = "The emails on this page. Each item has `client` and `preview`. Use the email `id` with `get_email_by_id` to read the full email.";

/// `preview` response field description for `SearchEmailItem`: partial email preview.
pub const FIELD_SEARCH_EMAIL_ITEM_PREVIEW_DESC: &str =
    "Partial email content. Use `get_email_by_id` with the email `id` to read the full email.";

/// `errors` response field description for `search_email`: per-account failures.
pub const FIELD_SEARCH_EMAIL_ERRORS_DESCRIPTION: &str =
    "Failure notes for each account. Empty when all accounts work.";

// --- cursor — canonical for cursor-paginated tools ---

/// Standard cursor-based pagination description paragraph (TOOL-028).
#[allow(dead_code)]
pub const CURSOR_PAGINATION_CANONICAL_DESCRIPTION: &str = "This tool uses cursor pages. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`.";

/// `cursor` field description. Used on both the input and output `cursor` fields of
/// cursor-paginated tools because the LLM passes back whatever the tool returned.
pub const FIELD_CURSOR_DESCRIPTION: &str = "Omit this field on the first call. To get the next page, give back the `cursor` value unchanged.";

/// Description for `search_email` (TOOL-026a).
pub const SEARCH_EMAIL_CANONICAL_DESCRIPTION: &str = "Search emails by keyword, folder, date, sender, recipient, or status. Give one or more filters. The tool returns a maximum of 32 emails per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`.";

/// Description for `web_fetch` cursor-based pagination (TOOL-026b).
pub const WEB_FETCH_CURSOR_DESCRIPTION: &str = "Get a URL and change its content to Markdown. The tool returns a maximum of 64 lines per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. To read from the start again, use `force_refetch`.";

/// Hint string emitted on the final page of a cursor pagination (TOOL-025).
pub const FINAL_PAGE_HINT: &str = "Final page.";

/// Hint string emitted by `list_notes_by_tag` when no files match the requested tag (TOOL-026d).
pub const NO_MATCHING_TAGGED_FILES_HINT: &str = "No matching tagged files found.";

// --- filesystem (fs) ---

/// Virtual workspace path of a note. Shared by note tools that take a `path`.
pub const FIELD_NOTE_PATH_DESCRIPTION: &str = "The virtual path of the note. Example: `notes/plan.md`. Use `/` or `.` for the library root where permitted.";

/// Content of a note file. Shared by note tools that take full file text.
pub const FIELD_NOTE_CONTENT_DESCRIPTION: &str = "The full text of the note in Markdown format.";

// --- patch_note ---

pub const PATCH_NOTE_DESCRIPTION: &str = "Change a note. Replace exact text with new text. The tool changes all exact matches. Use `read_note` first to see the exact text. This change is permanent.";

pub const FIELD_PATCH_NOTE_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_PATCH_NOTE_INPUT_OLD_STRING: &str =
    "The exact text to replace. Copy it from `read_note`. The text must match exactly.";

pub const FIELD_PATCH_NOTE_INPUT_NEW_STRING: &str = "The new text. It replaces the old text.";

// --- search_notes ---

pub const SEARCH_NOTES_DESCRIPTION: &str = "Search notes for exact text. Use this tool for exact words, not for ideas. For ideas and concepts, use `vector_search`. The tool returns a maximum of 64 matching lines per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`.";

pub const FIELD_SEARCH_NOTES_INPUT_QUERY: &str = "Give the text to find. Use exact words.";

pub const FIELD_SEARCH_NOTES_RESPONSE_MATCHES: &str =
    "Matching lines for this page. Contains `\"No matches found.\"` when there are no matches.";

pub const FIELD_SEARCH_NOTES_RESPONSE_TOTAL: &str =
    "Total number of matching lines in all libraries.";

// --- read_tags ---

pub const READ_TAGS_DESCRIPTION: &str = "Get all tags from note headers. Use the result with `list_notes_by_tag` to list notes for a tag. No input is necessary.";

// --- list_notes_by_tag ---

pub const LIST_NOTES_BY_TAG_DESCRIPTION: &str = "List notes that contain a tag in the header. Give the tag name. The tool returns a maximum of 64 file names per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`. To see all tags first, use `read_tags`.";

pub const FIELD_LIST_NOTES_BY_TAG_INPUT_TAG: &str =
    "The tag name to find. Omit the `#` prefix. Example: `project`.";

pub const FIELD_LIST_NOTES_BY_TAG_RESPONSE_FILES: &str = "Virtual paths of notes on this page.";

// --- list_notes ---

pub const LIST_NOTES_DESCRIPTION: &str = "List notes in a folder. Use path `/` or `.` to list content libraries. Use `offset` and `limit` to get pages. Default: `offset=0`, `limit=100`. To find notes by content, use `search_notes`. To find notes by tag, use `list_notes_by_tag`.";

pub const FIELD_LIST_NOTES_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_LIST_NOTES_RESPONSE_FILES: &str = "Virtual paths of notes on this page.";

// --- read_note ---

pub const READ_NOTE_DESCRIPTION: &str = "Read a full note. Give the virtual path. Do not use this tool for `User.md` files. The system already gives you these files. To see a summary only, use `read_yaml_header`. To read part of a file, use `window_note`.";

pub const FIELD_READ_NOTE_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_READ_NOTE_RESPONSE_CONTENT: &str = "The full text of the note.";

// --- window_note ---

pub const WINDOW_NOTE_DESCRIPTION: &str = "Read lines from a note. Give the path, the first line (`offset`), and the line count (`limit`). The first line is line 0. Default: `offset=0`, `limit=100`. If `offset` is past the end, the tool returns empty content. If `limit` is too large, the tool returns the remaining lines. Do not use this tool for `User.md` files. Pairs with `read_note`.";

pub const FIELD_WINDOW_NOTE_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_WINDOW_NOTE_INPUT_OFFSET: &str =
    "The first line to read. Use 0 for the first line.";

pub const FIELD_WINDOW_NOTE_INPUT_LIMIT: &str =
    "The maximum number of lines to read. Default: 100.";

pub const FIELD_WINDOW_NOTE_RESPONSE_CONTENT: &str = "The requested lines of the note.";

// --- create_note ---

pub const CREATE_NOTE_DESCRIPTION: &str = "Make a new note file with the given content. Give the path and the content. The tool fails if the file already exists. This tool only makes new files. It does not change files.";

pub const FIELD_CREATE_NOTE_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_CREATE_NOTE_INPUT_CONTENT: &str = FIELD_NOTE_CONTENT_DESCRIPTION;

// --- insert_into_note ---

pub const INSERT_INTO_NOTE_DESCRIPTION: &str = "Add lines to a note at a line number. Use `offset=0` to add lines at the top. Use `offset=lines.len()` to add lines at the end. The tool fails if `offset` is larger than the line count. This change is permanent.";

pub const FIELD_INSERT_INTO_NOTE_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_INSERT_INTO_NOTE_INPUT_OFFSET: &str =
    "The line number for the insert. Use 0 for the top. Use the line count for the end.";

pub const FIELD_INSERT_INTO_NOTE_INPUT_LINES: &str = "The lines to add. Give one string per line.";

// --- move_note ---

pub const MOVE_NOTE_DESCRIPTION: &str = "Move or rename a note. Give the source path and the target path. The tool fails if the target file already exists. This change is permanent.";

pub const FIELD_MOVE_NOTE_INPUT_SOURCE: &str = "Virtual path of the source note to move.";

pub const FIELD_MOVE_NOTE_INPUT_TARGET: &str = "Virtual path of the target location for the note.";

/// Result text of a move operation.
pub const FIELD_MOVE_NOTE_RESPONSE_RESULT: &str = "Result data for the operation.";

// --- web ---

// --- web_delegate ---

pub const WEB_DELEGATE_DESCRIPTION: &str = "Ask a sub-agent to find one fact on the web. Use this tool when the answer is one word, one sentence, or one short list. Examples: \"What is the full name of the current president of the United States?\" or \"List every compact SUV make and model sold in California in 2026.\" This tool uses less context. Do not use this tool for open research. Do not use it for unclear questions. Do not use it to crawl many pages. Do not use it to retry a failed `web_search`.";

pub const FIELD_WEB_DELEGATE_INPUT_INSTRUCTION: &str =
    "One clear question for the sub-agent. Include all facts that the sub-agent needs.";

// --- web_fetch ---

pub const WEB_FETCH_DESCRIPTION: &str = WEB_FETCH_CURSOR_DESCRIPTION;

pub const FIELD_WEB_FETCH_INPUT_URL: &str =
    "The full URL to read. Example: `https://example.com/page`.";

pub const FIELD_WEB_FETCH_INPUT_HEADERS: &str =
    "Set to `true` to include the HTTP response headers. Default: `false`.";

pub const FIELD_WEB_FETCH_INPUT_FORCE_REFETCH: &str =
    "Set to `true` to read from the source again and start at the first page. Default: `false`.";

pub const FIELD_WEB_FETCH_RESPONSE_CONTENT: &str =
    "The page content in Markdown format for this page.";

pub const FIELD_WEB_FETCH_RESPONSE_TOTAL_LINES: &str =
    "Total number of Markdown lines in the fetched body.";

pub const FIELD_WEB_FETCH_RESPONSE_FROM_CACHE: &str =
    "Set to `true` when the response comes from cache.";

/// Warning code emitted when headless browser rendering fails and triggers HTTP fallback.
pub const TOOL_W001_BROWSER_FETCH_FAILED: &str = "TOOL-W001";

/// Error code emitted when headless browser execution exceeds the safety timeout.
pub const TOOL_E001_BROWSER_TIMEOUT: &str = "TOOL-E001";

/// Standard CLI flag to run Chromium in modern headless mode.
pub const CHROME_FLAG_HEADLESS: &str = "--headless=new";

/// Standard CLI flag to disable hardware GPU acceleration in headless mode.
pub const CHROME_FLAG_DISABLE_GPU: &str = "--disable-gpu";

/// Standard CLI flag to skip the initial welcome dialog.
pub const CHROME_FLAG_NO_FIRST_RUN: &str = "--no-first-run";

/// Standard CLI flag to bypass default browser check prompt.
pub const CHROME_FLAG_NO_DEFAULT_BROWSER_CHECK: &str = "--no-default-browser-check";

/// Standard CLI flag to enforce incognito / ephemeral browsing session.
pub const CHROME_FLAG_INCOGNITO: &str = "--incognito";

/// Standard CLI flag to disable background network activity.
pub const CHROME_FLAG_DISABLE_BACKGROUND_NETWORKING: &str = "--disable-background-networking";

/// Standard CLI flag to disable profile synchronization.
pub const CHROME_FLAG_DISABLE_SYNC: &str = "--disable-sync";

/// Standard CLI flag to disable default pre-installed apps.
pub const CHROME_FLAG_DISABLE_DEFAULT_APPS: &str = "--disable-default-apps";

/// Standard CLI flag to disable all browser extensions.
pub const CHROME_FLAG_DISABLE_EXTENSIONS: &str = "--disable-extensions";

/// Standard CLI flag to mute any audio output.
pub const CHROME_FLAG_MUTE_AUDIO: &str = "--mute-audio";

/// Standard CLI flag to dump the rendered DOM HTML to stdout and exit.
pub const CHROME_FLAG_DUMP_DOM: &str = "--dump-dom";

/// Prefix for setting the user data directory.
pub const CHROME_FLAG_USER_DATA_DIR_PREFIX: &str = "--user-data-dir=";

/// Prefix for setting the virtual time budget in milliseconds.
pub const CHROME_FLAG_VIRTUAL_TIME_BUDGET_PREFIX: &str = "--virtual-time-budget=";

// --- web_search ---

pub const WEB_SEARCH_DESCRIPTION: &str = "Search the web. Give a search phrase. The tool returns a maximum of 32 results per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`. To read a result page, use `web_fetch`. For one fact with low context use, use `web_delegate`.";

pub const FIELD_WEB_SEARCH_INPUT_QUERY: &str = "Give the search phrase.";

pub const FIELD_WEB_SEARCH_RESPONSE_RESULTS: &str = "Search results for this page in text format.";

// --- jmap (email) ---

pub const SEARCH_EMAIL_DESCRIPTION: &str = SEARCH_EMAIL_CANONICAL_DESCRIPTION;

pub const GET_EMAIL_BY_ID_DESCRIPTION: &str = "Read one full email. Give the email ID. Use `search_email` first to find the ID. Returns the subject, body, and headers.";

pub const SEND_EMAIL_DESCRIPTION: &str = "Send a real email. Give the recipient, subject, and body. This action is permanent. It sends the email immediately. Confirm the recipient and the content before you call this tool.";

pub const FIELD_GET_EMAIL_BY_ID_INPUT_ID: &str = "The email ID from a `search_email` result.";

pub const FIELD_SEND_EMAIL_INPUT_TO: &str = "The recipient email address.";

pub const FIELD_SEND_EMAIL_INPUT_SUBJECT: &str = "The email subject line.";

pub const FIELD_SEND_EMAIL_INPUT_BODY: &str = "The email body text.";

pub const FIELD_SEARCH_EMAIL_INPUT_KEYWORD: &str =
    "Search email subjects, bodies, and headers using text keywords.";

pub const FIELD_SEARCH_EMAIL_INPUT_FOLDER: &str =
    "Optional mailbox folder name (such as Inbox or Sent).";

pub const FIELD_SEARCH_EMAIL_INPUT_START_DATE: &str =
    "Inclusive start date for received emails (ISO YYYY-MM-DD or RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_END_DATE: &str =
    "Inclusive end date for received emails (ISO YYYY-MM-DD or RFC 3339 timestamp).";

pub const FIELD_SEARCH_EMAIL_INPUT_FROM: &str = "Filter emails by sender address substring.";

pub const FIELD_SEARCH_EMAIL_INPUT_TO: &str = "Filter emails by recipient address substring.";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_UNREAD: &str =
    "Set to `true` to return only unread emails, or `false` to return read emails.";

pub const FIELD_SEARCH_EMAIL_INPUT_IS_FLAGGED: &str =
    "Set to `true` to return only flagged emails, or `false` to return unflagged emails.";

// --- caldav (calendar) ---

pub const SEARCH_CALENDAR_DESCRIPTION: &str = "Search calendar events by keyword. The tool returns a maximum of 32 events per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`. To search by date, use `get_calendar`.";

pub const GET_CALENDAR_DESCRIPTION: &str = "Get calendar events in a date range. Give a start date and an end date. Use ISO format `YYYY-MM-DD`. The tool returns a maximum of 32 events per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`.";

pub const GET_CALENDAR_ITEM_DESCRIPTION: &str = "Get one calendar event. Give the `href` or `id` from a search result. Returns the full event data.";

pub const ADD_CALENDAR_ITEM_DESCRIPTION: &str = "Add a new calendar event. Only give fields that you know. All fields are optional. Give `client` to select an account. If you omit `client`, the tool uses the primary account. The tool makes a UID.";

pub const UPDATE_CALENDAR_ITEM_DESCRIPTION: &str = "Change a calendar event. Give the `href` or `id`. Only the given fields change. All other fields stay the same. Give `client` to select an account. This change is permanent.";

pub const DELETE_CALENDAR_ITEM_DESCRIPTION: &str = "Delete a calendar event. Give the `href` or `id`. Give `client` to select an account. This action is permanent. It cannot be undone.";

pub const FIELD_CALENDAR_CLIENT_DESC: &str =
    "Optional CalDAV account name to target. If omitted, uses the default account.";

pub const FIELD_CALENDAR_HREF_DESC: &str =
    "The full href path (or id) of the calendar item, as returned by search or get tools.";

pub const FIELD_CALENDAR_SUMMARY_DESC: &str =
    "Event title/summary. Defaults to 'New Event' if omitted.";

pub const FIELD_CALENDAR_START_DESC: &str =
    "Start datetime in ISO 8601 format (e.g. '2025-01-15T09:00:00' or date-only '2025-01-15').";

pub const FIELD_CALENDAR_END_DESC: &str =
    "End datetime in ISO 8601 format (e.g. '2025-01-15T10:00:00' or date-only '2025-01-15').";

pub const FIELD_CALENDAR_DESCRIPTION_DESC: &str = "Free-text event description or notes.";

pub const FIELD_CALENDAR_LOCATION_DESC: &str = "Event location (e.g. room name, address).";

pub const FIELD_SEARCH_CALENDAR_INPUT_KEYWORD: &str =
    "The keyword to find in event titles and notes.";

pub const FIELD_GET_CALENDAR_INPUT_START_DATE: &str =
    "The start of the date range. Use ISO format `YYYY-MM-DD`.";

pub const FIELD_GET_CALENDAR_INPUT_END_DATE: &str =
    "The end of the date range. Use ISO format `YYYY-MM-DD`.";

// --- carddav (contacts) ---

pub const FIELD_CONTACT_CLIENT_DESC: &str =
    "Optional CardDAV account name to target. If omitted, uses the default account.";

pub const FIELD_CONTACT_HREF_DESC: &str =
    "The href (or id) of the contact, as returned by `get_contact` or `search_contact`.";

pub const FIELD_CONTACT_NAME_DESC: &str = "The contact's display name.";

pub const FIELD_CONTACT_EMAIL_DESC: &str = "Email address.";

pub const FIELD_CONTACT_PHONE_DESC: &str = "Phone number.";

pub const FIELD_CONTACT_COMPANY_DESC: &str = "Company or organization name.";

pub const FIELD_CONTACT_TITLE_DESC: &str = "Job title or role.";

pub const FIELD_CONTACT_NOTES_DESC: &str = "Free-form notes about the contact.";

pub const FIELD_CONTACT_BIRTHDAY_DESC: &str = "Birthday as ISO date YYYY-MM-DD.";

pub const FIELD_CONTACT_ADDRESSES_DESC: &str =
    "Array of postal address objects. See `AddressInput` schema for field details.";

pub const FIELD_ADDRESS_TYPE_DESC: &str = "Address type: home, work, or another label.";

pub const FIELD_ADDRESS_STREET_DESC: &str = "Street address.";

pub const FIELD_ADDRESS_CITY_DESC: &str = "City.";

pub const FIELD_ADDRESS_REGION_DESC: &str = "State, province, or region.";

pub const FIELD_ADDRESS_POSTAL_CODE_DESC: &str = "Postal or ZIP code.";

pub const FIELD_ADDRESS_COUNTRY_DESC: &str = "Country name.";

pub const FIELD_ADDRESS_PO_BOX_DESC: &str = "P.O. box number.";

pub const FIELD_ADDRESS_EXT_DESC: &str = "Extended address (e.g. apartment or suite).";

pub const FIELD_SEARCH_CONTACT_INPUT_KEYWORD: &str =
    "The keyword to find in names and contact fields.";

pub const SEARCH_CONTACT_DESCRIPTION: &str = "Search contacts by keyword. Give one keyword. The tool returns a maximum of 32 contacts per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page. The last page has a `hint` field and no `cursor`. To read one contact, use `get_contact`.";

pub const ADD_CONTACT_DESCRIPTION: &str = "Add a new contact. Only give fields that you know. All fields are optional. Give `client` to select an account. If you omit `client`, the tool uses the primary account. The tool returns the new `href`.";

pub const GET_CONTACT_DESCRIPTION: &str = "Get a contact by its href (or id). Returns structured fields: `fn_name`, `email`, `tel`, `org`, `bday`, `addresses`, and raw `vcard`.";

pub const UPDATE_CONTACT_DESCRIPTION: &str = "Change a contact at the given `href` (or `id`). Give `client` to select an account. Only the given fields change. All other vCard data stays the same. The vCard `UID` stays the same. Thus the addressbook href stays stable. This change is permanent.";

pub const DELETE_CONTACT_DESCRIPTION: &str = "Delete the contact at the given `href` (or `id`). Give `client` to select an account. A 404 (already absent) is a successful no-op. Thus the call is idempotent. A successful delete is permanent.";

// --- csv (database) ---

pub const CREATE_CSV_DESCRIPTION: &str = "Make a new CSV database. Give a unique name and the column headers in order. Example headers: `[\"name\", \"age\"]`. Use `add_rows` next to add data.";

pub const LIST_CSV_DESCRIPTION: &str =
    "List all CSV databases. Use this tool to see names before you query. No input is necessary.";

pub const ADD_ROWS_DESCRIPTION: &str = "Add rows to a CSV database. Give the database name and the rows. Give each row as an object. Use header names as keys. Missing keys become empty text. This change is permanent.";

pub const DELETE_ROWS_DESCRIPTION: &str = "Delete rows from a CSV database. Give the database name and a filter expression. The tool deletes each row where the expression is true. Example: `age > 30`. This change is permanent. It cannot be undone.";

pub const QUERY_DESCRIPTION: &str = "Read rows from a CSV database. Give the database name. Optionally give a filter expression to select rows. Optionally give `aggregate_col` and `aggregate_func` (`sum`, `average`, `count`) to calculate a value. Example filter: `city == \"Berlin\"`. To change data, use `add_rows` or `delete_rows`.";

pub const FIELD_CREATE_CSV_INPUT_DB_NAME: &str = "A unique name for the new CSV database.";

pub const FIELD_CREATE_CSV_INPUT_HEADERS: &str =
    "The column headers in order. Example: `[\"name\", \"age\"]`.";

pub const FIELD_ADD_ROWS_INPUT_DB_NAME: &str = "The name of the target CSV database.";

pub const FIELD_ADD_ROWS_INPUT_ROWS: &str = "The rows as objects. Use header names as keys with text values. Missing keys become empty text.";

pub const FIELD_DELETE_ROWS_INPUT_DB_NAME: &str = "The name of the target CSV database.";

pub const FIELD_DELETE_ROWS_INPUT_PREDICATE: &str = "A filter expression for each row. The tool deletes rows where the expression is true. Example: `age > 30`.";

pub const FIELD_QUERY_REQUEST_DB_NAME: &str = "The name of the target CSV database.";

pub const FIELD_QUERY_REQUEST_PREDICATE: &str = "A filter expression to select rows. If you omit it, the tool reads all rows. Example: `city == \"Berlin\"`.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_COL: &str =
    "The column to calculate. Give this value when you set `aggregate_func`.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_FUNC: &str =
    "The calculate function. Use `sum`, `average`, or `count`.";

// --- yaml (front-matter) ---

// --- read_yaml_header ---

pub const READ_YAML_HEADER_DESCRIPTION: &str = "Read the header of a Markdown file. The header has the title, summary, and tags. Use this tool to see a summary before you read the full file. It uses less context than `read_note`.";

pub const FIELD_READ_YAML_HEADER_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

// --- write_yaml_header ---

pub const WRITE_YAML_HEADER_DESCRIPTION: &str = "Write or change the header of a Markdown file. Give the path. Give only the fields to change: `title`, `summary`, `tags`, or `header-date`. Other header fields stay the same. This change is permanent.";

pub const FIELD_WRITE_YAML_HEADER_INPUT_PATH: &str = FIELD_NOTE_PATH_DESCRIPTION;

pub const FIELD_WRITE_YAML_HEADER_INPUT_TITLE: &str = "The document title.";

pub const FIELD_WRITE_YAML_HEADER_INPUT_SUMMARY: &str = "A short summary of the document.";

pub const FIELD_WRITE_YAML_HEADER_INPUT_TAGS: &str = "The tag list for the document.";

pub const FIELD_WRITE_YAML_HEADER_INPUT_HEADER_DATE: &str =
    "The header date as text. Use ISO format `YYYY-MM-DD` where possible.";

// --- weather ---

pub const GET_WEATHER_DESCRIPTION: &str = "Get weather for a place. Give a city name or coordinates. Returns current conditions and a forecast for 7 days. Optionally give a date (`YYYY-MM-DD`) to filter the forecast.";

pub const FIELD_WEATHER_INPUT_LOCATION: &str =
    "The place name or coordinates. Example: `Berlin` or `52.52,13.41`.";

pub const FIELD_WEATHER_INPUT_DATE_RANGE: &str = "An optional date (`YYYY-MM-DD`) to filter the forecast. If you omit it, the tool returns all 7 days.";

// --- trello ---

pub const GET_BOARDS_DESCRIPTION: &str = "List all Trello boards for the user. Use this tool first to find a board ID. No input is necessary.";
pub const GET_BOARD_DESCRIPTION: &str =
    "Read one Trello board. Give the board ID from `trello_get_boards`. Returns the board data.";
pub const GET_LISTS_DESCRIPTION: &str = "List all lists on a Trello board. Give the board ID. Use the result to find a list ID for `trello_get_cards` or `trello_create_card`.";
pub const GET_CARDS_DESCRIPTION: &str =
    "List all cards in a Trello list. Give the list ID from `trello_get_lists`.";
pub const CREATE_CARD_DESCRIPTION: &str = "Make a new card in a Trello list. Give the list ID (`idList`) and a name. Optionally give a description in Markdown with context and links. This change is permanent.";
pub const UPDATE_CARD_DESCRIPTION: &str = "Change a Trello card. Give the card ID. Give only the fields to change: `name`, `desc`, or `idList` to move the card. This change is permanent.";
pub const DELETE_CARD_DESCRIPTION: &str =
    "Delete a Trello card. Give the card ID. This action is permanent. It cannot be undone.";

pub const FIELD_TRELLO_ID_DESCRIPTION: &str = "The Trello ID of the board, list, or card.";

pub const FIELD_TRELLO_CREATE_ID_LIST_DESCRIPTION: &str =
    "The list ID that holds the new card. Get it from `trello_get_lists`.";

pub const FIELD_TRELLO_CREATE_NAME_DESCRIPTION: &str = "The card name.";

pub const FIELD_TRELLO_CREATE_DESC_DESCRIPTION: &str =
    "The card description in Markdown. Include context and links.";

pub const FIELD_TRELLO_CREATE_ID_LABELS_DESCRIPTION: &str = "Optional label IDs for the card.";

pub const FIELD_TRELLO_UPDATE_ID_DESCRIPTION: &str = "The card ID to change.";

pub const FIELD_TRELLO_UPDATE_NAME_DESCRIPTION: &str = "The new card name.";

pub const FIELD_TRELLO_UPDATE_DESC_DESCRIPTION: &str = "The new card description.";

pub const FIELD_TRELLO_UPDATE_ID_LIST_DESCRIPTION: &str =
    "The new list ID. Use it to move the card.";

// --- vector search ---

/// Description for `vector_search`. Only used with the `vector-search` feature.
#[allow(dead_code)]
pub const VECTOR_SEARCH_DESCRIPTION: &str = "Search notes by meaning and concepts. Give a phrase, question, or summary. Example: `checking account statements from credit union`. For exact words, use `search_notes`. The tool returns a maximum of 32 results per page. Omit `cursor` on the first call. Give the `cursor` back unchanged to get the next page.";

#[allow(dead_code)]
pub const FIELD_VECTOR_SEARCH_INPUT_QUERY: &str = "A phrase, question, or summary. Do not use single keywords. For exact text, use `search_notes`.";

#[allow(dead_code)]
pub const FIELD_VECTOR_SEARCH_INPUT_MAX_DISTANCE: &str = "A value from 0.0 to 2.0. Default: 0.6. Use 0.3 to 0.5 for strict matches. Use 0.8 to 1.0 for broad matches.";

// --- browser (headless automation, BRWS-001..008) ---

// --- browser_navigate ---

#[cfg(feature = "browser")]
pub const BROWSER_NAVIGATE_DESCRIPTION: &str = "Open a URL in the headless browser. Give a full URL. The page stays active for the next calls. The tool returns the final URL and the page title. Use `browser_get_page_state` next to see the page.";

// --- browser_get_page_state ---

#[cfg(feature = "browser")]
pub const BROWSER_GET_PAGE_STATE_DESCRIPTION: &str = "Read the current page. Returns the URL, title, and all controls (links, buttons, inputs). Each control has a stable `agent_id`. Use the `agent_id` in the next action tools. Call this tool after each navigation or click.";

// --- browser_click ---

#[cfg(feature = "browser")]
pub const BROWSER_CLICK_DESCRIPTION: &str = "Click a control on the page. First call `browser_get_page_state` to get the `agent_id`. Give the CSS selector for the control. The page changes after the click. Call `browser_get_page_state` again to see the new page.";

// --- browser_fill_input ---

#[cfg(feature = "browser")]
pub const BROWSER_FILL_INPUT_DESCRIPTION: &str = "Type text in an input field or text area. First call `browser_get_page_state` to get the `agent_id`. This action replaces all text in the field. To submit a form, press `Enter` with `browser_press_key`.";

// --- browser_select_dropdown ---

#[cfg(feature = "browser")]
pub const BROWSER_SELECT_DROPDOWN_DESCRIPTION: &str = "Select a value in a drop-down list. First call `browser_get_page_state` to get the `agent_id`. Give the CSS selector for the list and the `value` of the option.";

// --- browser_press_key ---

#[cfg(feature = "browser")]
pub const BROWSER_PRESS_KEY_DESCRIPTION: &str = "Press a key on the page. Use `Enter` to submit a form. Use `Escape` to close a dialog. You can also use `Tab` or `ArrowDown`. First read the page with `browser_get_page_state`.";

// --- browser_evaluate_js ---

#[cfg(feature = "browser")]
pub const BROWSER_EVALUATE_JS_DESCRIPTION: &str = "Run a JavaScript expression on the page. Give one expression. The tool returns the result as JSON. Keep the script short and safe.";

// --- browser_screenshot ---

#[cfg(feature = "browser")]
pub const BROWSER_SCREENSHOT_DESCRIPTION: &str = "Save a picture of the page as a PNG file. Give a file name. Use only letters, digits, `.`, `_`, `-`, maximum 128 characters. No folders. No leading dots.";

// --- Field descriptions ---

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_INPUT_URL: &str =
    "The full URL to open (such as `https://example.com/login`).";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_RESPONSE_URL: &str =
    "The final URL after navigation. This URL changes if the server redirects the request.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_RESPONSE_TITLE: &str =
    "The page `<title>` after navigation. Empty when the page has no title.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_URL: &str = "The current page URL.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TITLE: &str =
    "The current page `<title>`. Empty when the page has no title.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_ELEMENTS: &str = "Controls on the page. Each entry has `agent_id`, `tag`, `text`, `placeholder`, `name`, and `type`. Use `agent_id` to select a control.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TOTAL: &str =
    "Total number of controls on the page.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_CLICK_INPUT_SELECTOR: &str = "The CSS selector for the control. Get the `agent_id` from `browser_get_page_state` first, then build the selector.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_FILL_INPUT_INPUT_SELECTOR: &str = "The CSS selector for the input or text area. Get the `agent_id` from `browser_get_page_state` first.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_FILL_INPUT_INPUT_TEXT: &str =
    "The text to type. It replaces all text in the field.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_SELECTOR: &str = "The CSS selector for the `<select>` list. Get the `agent_id` from `browser_get_page_state` first.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_VALUE: &str =
    "The `value` of the `<option>` to select.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_PRESS_KEY_INPUT_KEY: &str =
    "The key to press (such as `Enter`, `Tab`, `Escape`, or `ArrowDown`).";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_EVALUATE_JS_INPUT_SCRIPT: &str =
    "One JavaScript expression to run on the page. The tool returns the result as JSON.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_INPUT_FILENAME: &str =
    "The file name for the PNG. Use `[A-Za-z0-9._-]{1,128}`. No folders. No leading dots.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_INPUT_FULL_PAGE: &str = "Set to `true` to capture the full page, or `false` to capture only the screen. Default: `false`.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_PATH: &str =
    "Absolute path of the saved PNG screenshot file.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_BYTES: &str =
    "Size of the saved PNG screenshot file in bytes.";
