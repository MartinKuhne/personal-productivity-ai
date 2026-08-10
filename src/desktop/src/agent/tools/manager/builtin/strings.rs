//! User-visible description strings for every built-in tool â€” the single source of truth for the LLM-facing tool description and per-field schema description.
//!
//! Every const lives at the top level of this module. The `Tool::description()` impls in the
//! sibling `builtin/*.rs` files and the `#[schemars(description = ...)]` attributes on the
//! DTO fields in `crate::agent::tools::dtos` and `crate::agent::tools::csv_db::schema`
//! both reference the consts here. Editing a string in this module is the only place to
//! change what the LLM sees for that tool.
//!
//! Consts are grouped by tool family with `// --- <family> ---` headers purely for
//! human readability; every const is at the same Rust scope. Family-prefixed const
//! names (e.g. `FIELD_BROWSER_NAVIGATE_INPUT_URL`) already disambiguate cross-family
//! references, so no submodule is needed and adding a new tool no longer requires
//! creating a new file.

// --- paging (offset/limit) â€” canonical across list-paginated tools ---

/// `offset` parameter description. Used on every list-paginated tool's `offset` field.
pub const FIELD_OFFSET_DESCRIPTION: &str =
    "Specify the number of items to skip from the start (0-indexed). Default: 0.";

/// `limit` parameter description. The default value is substituted per tool via the
/// per-family domain sentence.
pub const FIELD_LIMIT_DESCRIPTION: &str = "Specify the number of items to return. Default: {N}.";

/// `total` response field description. Used on every list-paginated tool's `total` field.
pub const FIELD_TOTAL_DESCRIPTION: &str = "The total number of items across all pages.";

/// `hint` response field description. Used on every list-paginated tool's `hint` field.
pub const FIELD_HINT_DESCRIPTION: &str =
    "Displays a message when the offset exceeds total results or when no matches exist.";

// --- cursor â€” canonical for cursor-paginated tools ---

/// `cursor` field description. Used on both the input and output `cursor` fields of
/// `search_email` and `web_fetch` because the LLM passes back whatever the tool returned.
pub const FIELD_CURSOR_DESCRIPTION: &str = "Pass this pagination token back unchanged to get the next page. The tool generates this token on the first call.";

/// Description for `search_email`. Replaces the offset/limit canonical paragraph
/// because the cursor flow is fundamentally different. The full tool description is
/// this paragraph plus a per-family domain sentence.
pub const SEARCH_EMAIL_CANONICAL_DESCRIPTION: &str = "Search emails by keyword, folder, date range, sender, recipient, or status. You must provide at least one filter. The tool returns up to 100 matching emails and a cursor token for pagination.";

/// Description for `web_fetch` cursor-based pagination. The tool returns up to 100
/// lines of Markdown content and a cursor token for pagination.
pub const WEB_FETCH_CURSOR_DESCRIPTION: &str = "Fetch a URL and convert the content to Markdown. Returns up to 100 lines and a cursor token for pagination. Use the cursor to fetch the next page. Use force_refetch=true to bypass.";

/// Hint string emitted on the final page of a `web_fetch` cursor pagination.
pub const WEB_FETCH_FINAL_PAGE_HINT: &str = "Final page.";

// --- filesystem (fs) ---

// --- patch_note ---

pub const PATCH_NOTE_DESCRIPTION: &str = "Patch a markdown-formatted note by replacing exact occurrences of target text with replacement text.";

// --- search_notes ---

pub const SEARCH_NOTES_DESCRIPTION: &str = "Search markdown-formatted notes for text. The tool returns up to 200 matching lines. If the tool truncates results, refine your query or use a sub-agent.";

pub const FIELD_SEARCH_NOTES_INPUT_QUERY: &str = "Specify the search term.";

pub const FIELD_SEARCH_NOTES_RESPONSE_MATCHES: &str = "Contains matching lines up to 200 results. Returns `\"No matches found.\"` when no matches exist.";

pub const FIELD_SEARCH_NOTES_RESPONSE_TOTAL: &str =
    "Total number of matching lines found across all libraries.";

pub const FIELD_SEARCH_NOTES_RESPONSE_TRUNCATED: &str =
    "Set to `true` when total matches exceed 200 lines.";

// --- read_tags ---

pub const READ_TAGS_DESCRIPTION: &str =
    "Get all unique tags from front-matter headers in workspace Markdown files.";

// --- list_notes_by_tag ---

pub const LIST_NOTES_BY_TAG_DESCRIPTION: &str = "Return a paginated list of markdown-formatted notes that contain a tag in their front-matter. Default parameters: `offset=0`, `limit=100`.";

pub const FIELD_LIST_NOTES_BY_TAG_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested page slice.";

// --- list_notes ---

pub const LIST_NOTES_DESCRIPTION: &str = "Return a paginated list of markdown-formatted notes in a directory. Use path `/` or `.` to list content libraries. Default parameters: `offset=0`, `limit=100`.";

pub const FIELD_LIST_NOTES_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested page slice.";

// --- read_note ---

pub const READ_NOTE_DESCRIPTION: &str = "Read the full text of a markdown-formatted note at a path. Use `read_yaml_header` if you only need a document summary.";

// --- window_note ---

pub const WINDOW_NOTE_DESCRIPTION: &str = "Read a contiguous slice of lines from a markdown-formatted note. `offset` is 0-indexed (`0` is the first line); `limit` is the maximum number of lines to return. An `offset` past the end of the note returns an empty `content`. A `limit` that would overflow the note's line count is clamped to the remainder. Default parameters: `offset=0`, `limit=100`. Pairs with `read_note` when you need the whole note.";

// --- create_note ---

pub const CREATE_NOTE_DESCRIPTION: &str = "Create a new file at the specified path with provided content. Fails if the file already exists â€” this tool can only create new files.";

// --- insert_into_note ---

pub const INSERT_INTO_NOTE_DESCRIPTION: &str = "Insert lines into a markdown-formatted note at a specified 0-indexed offset. `offset=0` inserts at the top of the note; `offset=lines.len()` appends to the end. `offset > lines.len()` returns an error.";

// --- web ---

// --- web_delegate ---

pub const WEB_DELEGATE_DESCRIPTION: &str = "Delegate a specific, unambiguous fact lookup to a sub-agent that searches and fetches the web, then returns a concise answer. Use when the question has a single factual answer — a word, sentence, or short list. Examples: \"What is the full name of the current president of the United States?\" or \"List every compact SUV make and model sold in California in 2026.\" Preferring web_delegate for fact lookups reduces your context usage. Do NOT use web_delegate for open-ended research, ambiguous questions, multi-page crawling, or as a retry after a failed web_search.";

// --- web_fetch ---

pub const WEB_FETCH_DESCRIPTION: &str = WEB_FETCH_CURSOR_DESCRIPTION;

pub const FIELD_WEB_FETCH_RESPONSE_TOTAL_LINES: &str =
    "Total number of Markdown lines in the fetched body.";

pub const FIELD_WEB_FETCH_RESPONSE_FROM_CACHE: &str =
    "Set to `true` when the response comes from cache.";

// --- web_search ---

pub const WEB_SEARCH_DESCRIPTION: &str = "Search the web for information using a query string. Use for ambiguous or open-ended questions, evaluating broad result sets, crawling multiple pages, or when you need to try different search strategies to find what you're looking for.";

pub const FIELD_WEB_SEARCH_INPUT_QUERY: &str = "Specify the search term.";

// --- jmap (email) ---

pub const SEARCH_EMAIL_DESCRIPTION: &str = SEARCH_EMAIL_CANONICAL_DESCRIPTION;

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

// --- caldav (calendar) ---

pub const SEARCH_CALENDAR_DESCRIPTION: &str = "Search calendar items by keyword.";

pub const GET_CALENDAR_DESCRIPTION: &str = "Get calendar items by date range.";

pub const GET_CALENDAR_ITEM_DESCRIPTION: &str = "Get a calendar item by its full href path. Provide the exact href value returned by search or get tools instead of a UUID.";

pub const ADD_CALENDAR_ITEM_DESCRIPTION: &str = "Add a new calendar event to the first configured CalDAV calendar. All fields are optional — only include fields you know. The tool generates a UID automatically.";

pub const UPDATE_CALENDAR_ITEM_DESCRIPTION: &str = "Update an existing calendar event identified by `id` (the href returned by search or get tools). Only the fields you provide are changed; all other event properties are preserved.";

pub const DELETE_CALENDAR_ITEM_DESCRIPTION: &str = "Delete a calendar item.";

pub const FIELD_CALENDAR_HREF_DESC: &str =
    "The full href path of the calendar item, as returned by search or get tools.";

pub const FIELD_CALENDAR_SUMMARY_DESC: &str =
    "Event title/summary. Defaults to 'New Event' if omitted.";

pub const FIELD_CALENDAR_START_DESC: &str =
    "Start datetime in ISO 8601 format (e.g. '2025-01-15T09:00:00' or date-only '2025-01-15').";

pub const FIELD_CALENDAR_END_DESC: &str =
    "End datetime in ISO 8601 format (e.g. '2025-01-15T10:00:00' or date-only '2025-01-15').";

pub const FIELD_CALENDAR_DESCRIPTION_DESC: &str = "Free-text event description or notes.";

pub const FIELD_CALENDAR_LOCATION_DESC: &str = "Event location (e.g. room name, address).";

// --- carddav (contacts) ---

pub const FIELD_CONTACT_HREF_DESC: &str =
    "The href of the contact, as returned by `get_contact` or `search_contact`.";

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

pub const SEARCH_CONTACT_DESCRIPTION: &str = "Search contacts by keyword.";

pub const ADD_CONTACT_DESCRIPTION: &str = "Add a new contact to the configured CardDAV addressbook. All fields are optional — only include fields you know. The created resource is returned with its href, Location, and ETag.";

pub const GET_CONTACT_DESCRIPTION: &str = "Get a contact by its unique ID. The response includes the \
raw vCard plus structured fields: `fn_name`, `email`, `tel`, `org`, \
`bday` (ISO `YYYY-MM-DD`), and `addresses` (array of postal-address \
objects with the same shape as `add_contact`).";

pub const UPDATE_CONTACT_DESCRIPTION: &str = "Update an existing contact at the given `id` (the href returned by `get_contact` or `search_contact`). Only the fields you provide are touched; every other vCard property on the contact (N, NICKNAME, URL, X-*, …) is preserved verbatim, so this is safe to call with a partial payload. The contact's vCard `UID` is preserved across the update so the addressbook href stays stable. `addresses` is list-valued: when provided, the new list replaces every existing ADR on the contact. The update is performed with `If-Match` so concurrent edits are detected — if the server returns 412 the caller should re-`get_contact` and retry.";

pub const DELETE_CONTACT_DESCRIPTION: &str = "Delete the contact at the given `id` (the href returned by \
`get_contact` or `search_contact`). A 404 (already absent) is treated as a \
successful no-op so the call is idempotent. Other non-2xx responses are \
propagated as errors with the truncated response body for diagnosis.";

// --- csv (database) ---

pub const CREATE_CSV_DESCRIPTION: &str = "Create a CSV database. Specify the column headers.";

pub const LIST_CSV_DESCRIPTION: &str = "List all CSV databases.";

pub const ADD_ROWS_DESCRIPTION: &str = "Add rows to a CSV database.";

pub const DELETE_ROWS_DESCRIPTION: &str = "Delete rows from a CSV database. Specify an expression.";

pub const QUERY_DESCRIPTION: &str =
    "Query a CSV database. Specify an expression or an aggregate function.";

pub const FIELD_CREATE_CSV_INPUT_DB_NAME: &str = "Specify a unique name for the new CSV database.";

pub const FIELD_CREATE_CSV_INPUT_HEADERS: &str =
    "Specify the column headers for the new CSV database. Use sequential order.";

pub const FIELD_ADD_ROWS_INPUT_DB_NAME: &str = "Specify the name of the target CSV database.";

pub const FIELD_ADD_ROWS_INPUT_ROWS: &str = "Specify the rows as JSON objects. Map the header names to the string values. The system saves the missing keys as empty strings.";

pub const FIELD_DELETE_ROWS_INPUT_DB_NAME: &str = "Specify the name of the target CSV database.";

pub const FIELD_DELETE_ROWS_INPUT_PREDICATE: &str = "Specify an expression to evaluate each row. The tool deletes the rows where the expression returns true.";

pub const FIELD_QUERY_REQUEST_DB_NAME: &str = "Specify the name of the target CSV database.";

pub const FIELD_QUERY_REQUEST_PREDICATE: &str = "Specify an expression to filter the rows. If you do not specify the expression, the tool evaluates all rows.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_COL: &str =
    "Specify the column to aggregate. Specify this value when you set `aggregate_func`.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_FUNC: &str =
    "Specify the aggregate function. Use `sum`, `average`, or `count`.";

// --- yaml (front-matter) ---

// --- read_yaml_header ---

pub const READ_YAML_HEADER_DESCRIPTION: &str = "Read the YAML header from a Markdown file. Use this tool to inspect a document summary before reading the full file.";

// --- write_yaml_header ---

pub const WRITE_YAML_HEADER_DESCRIPTION: &str =
    "Write or update YAML header data in a Markdown file.";

// --- weather ---

pub const GET_WEATHER_DESCRIPTION: &str = "Get current weather conditions and forecasts for a location (~7 days). You can optionally filter forecasts by date (`YYYY-MM-DD`).";

// --- trello ---

pub const GET_BOARDS_DESCRIPTION: &str = "Fetch all Trello boards for the authenticated user.";
pub const GET_BOARD_DESCRIPTION: &str = "Fetch details of a Trello board by its ID.";
pub const GET_LISTS_DESCRIPTION: &str = "Fetch all lists in a Trello board by its ID.";
pub const GET_CARDS_DESCRIPTION: &str = "Fetch all cards in a Trello list by its ID.";
pub const CREATE_CARD_DESCRIPTION: &str = "Create a new card in a specific Trello list. Make sure the card specifies what needs to be accomplished, when, how, and by whom. Be sure to set an estimated priority and include any relevant context or links (like email or website references) in the description.";
pub const UPDATE_CARD_DESCRIPTION: &str =
    "Update an existing Trello card (e.g. name, description, move to list).";
pub const DELETE_CARD_DESCRIPTION: &str = "Delete a Trello card by its ID.";

// --- browser (headless automation, BRWS-001..008) ---

// --- browser_navigate ---

#[cfg(feature = "browser")]
pub const BROWSER_NAVIGATE_DESCRIPTION: &str = "Navigate the headless browser to a URL. The system preserves page state across calls for multi-step flows. The response returns the final URL and page title.";

// --- browser_get_page_state ---

#[cfg(feature = "browser")]
pub const BROWSER_GET_PAGE_STATE_DESCRIPTION: &str = "Get interactable elements (a, button, input, select, textarea), current URL, and page title. Each element includes a stable agent_id for action tools.";

// --- browser_click ---

#[cfg(feature = "browser")]
pub const BROWSER_CLICK_DESCRIPTION: &str = "Click an element on the page using a CSS selector. Subsequent page state calls reflect the updated DOM.";

// --- browser_fill_input ---

#[cfg(feature = "browser")]
pub const BROWSER_FILL_INPUT_DESCRIPTION: &str = "Fill an input or textarea element with text. This action replaces any existing value. Press Enter using browser_press_key to submit forms.";

// --- browser_select_dropdown ---

#[cfg(feature = "browser")]
pub const BROWSER_SELECT_DROPDOWN_DESCRIPTION: &str =
    "Select an option in a dropdown element using its value attribute.";

// --- browser_press_key ---

#[cfg(feature = "browser")]
pub const BROWSER_PRESS_KEY_DESCRIPTION: &str =
    "Press a keyboard key on the page (such as Enter, Tab, Escape, or ArrowDown).";

// --- browser_evaluate_js ---

#[cfg(feature = "browser")]
pub const BROWSER_EVALUATE_JS_DESCRIPTION: &str = "Evaluate a JavaScript expression in the page context. The tool serializes the return value to JSON.";

// --- browser_screenshot ---

#[cfg(feature = "browser")]
pub const BROWSER_SCREENSHOT_DESCRIPTION: &str = "Save a PNG screenshot of the page to the configured directory. The tool restricts filenames to valid alphanumeric characters (up to 128 characters).";

// --- Field descriptions ---

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_INPUT_URL: &str =
    "Specify the absolute URL to navigate to (such as `https://example.com/login`).";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_RESPONSE_URL: &str =
    "The final URL after navigation. This URL changes if the server redirects the request.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_NAVIGATE_RESPONSE_TITLE: &str =
    "The page `<title>` after navigation. Returns an empty string if the page has no title.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_URL: &str = "The current page URL.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TITLE: &str =
    "The current page `<title>`. Returns an empty string if the page has no title.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_ELEMENTS: &str = "JSON array of interactable elements. Each entry contains `agent_id`, `tag`, `text`, `placeholder`, `name`, and `type`. Use `agent_id` to target specific elements.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TOTAL: &str =
    "Total number of interactable elements on the page.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_CLICK_INPUT_SELECTOR: &str = "Specify the CSS selector for the element to click. Use `agent_id` to build a `:nth-of-type(...)` selector if needed.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_FILL_INPUT_INPUT_SELECTOR: &str =
    "Specify the CSS selector for the input or textarea to fill.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_FILL_INPUT_INPUT_TEXT: &str =
    "Provide the text to insert. Replaces any existing text.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_SELECTOR: &str =
    "Specify the CSS selector for the `<select>` element.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_VALUE: &str =
    "Specify the `value` attribute of the `<option>` to select.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_PRESS_KEY_INPUT_KEY: &str =
    "Specify the key to press (such as `Enter`, `Tab`, `Escape`, or `ArrowDown`).";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_EVALUATE_JS_INPUT_SCRIPT: &str = "Provide a JavaScript expression to evaluate in the page context. The tool serializes the return value to JSON.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_INPUT_FILENAME: &str = "Specify the filename for the PNG. Must match `[A-Za-z0-9._-]{1,128}` without path separators or leading dots.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_INPUT_FULL_PAGE: &str = "Set to `true` to capture the entire scrollable page, or `false` to capture only the current viewport. Default: `false`.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_PATH: &str =
    "Absolute path of the saved PNG screenshot file.";

#[cfg(feature = "browser")]
pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_BYTES: &str =
    "Size of the saved PNG screenshot file in bytes.";
