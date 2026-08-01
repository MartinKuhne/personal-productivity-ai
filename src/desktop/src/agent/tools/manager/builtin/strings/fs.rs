//! User-visible description strings for the filesystem tool family.

use super::paging;

// --- replace_text ---

pub const REPLACE_TEXT_DESCRIPTION: &str =
    "Replace exact occurrences of old_string with new_string in a file.";

// --- grep ---

pub const GREP_DESCRIPTION: &str = "Search for a query string case-insensitively across all Markdown files in the workspace. Returns at most 200 matching lines; when the result is truncated, refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.";

pub const FIELD_GREP_INPUT_QUERY: &str = "The search term.";

pub const FIELD_GREP_RESPONSE_MATCHES: &str = "Match lines (`virtual/path:line - content`), capped at 200 matches. Set to `\"No matches found.\"` when the query matched nothing.";

pub const FIELD_GREP_RESPONSE_TOTAL: &str = "Total number of matches found across all libraries, including any beyond the result cap. Lets the caller tell when `matches` was truncated.";

pub const FIELD_GREP_RESPONSE_TRUNCATED: &str =
    "True when `matches` was truncated because the query matched more than 200 lines.";

// --- read_tags ---

pub const READ_TAGS_DESCRIPTION: &str =
    "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.";

// --- list_files_by_tag ---

pub const LIST_FILES_BY_TAG_DESCRIPTION: &str = "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise). Lists Markdown files that contain the given tag in their front-matter. Defaults: `offset=0`, `limit=100`.";

pub const FIELD_LIST_FILES_BY_TAG_RESPONSE_FILES: &str = "JSON array of virtual file paths for the requested slice (no library prefix is applied when the result is empty).";

// --- list_files ---

pub const LIST_FILES_DESCRIPTION: &str = "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise). Lists Markdown files in a directory (not recursive). With `path` set to `/` or `.` returns the configured content libraries. Defaults: `offset=0`, `limit=100`.";

pub const FIELD_LIST_FILES_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested slice.";

// --- read_file ---

pub const READ_FILE_DESCRIPTION: &str = "Read the entire text contents of a file at the specified path. Prefer using the read_yaml_header tool if just a document summary is needed.";

// --- read_file_lines ---

pub const READ_FILE_LINES_DESCRIPTION: &str = "Read specific lines from a file (1-indexed).";

// --- create_file ---

pub const CREATE_FILE_DESCRIPTION: &str =
    "Create a new file at the specified path with the provided content.";

// --- insert_lines ---

pub const INSERT_LINES_DESCRIPTION: &str =
    "Insert lines into a file at a specific 1-indexed line index.";

// --- delete_lines ---

pub const DELETE_LINES_DESCRIPTION: &str =
    "Delete specific lines from a file (1-indexed, inclusive).";

// Suppress the unused-import warning when the binary is built
// without any caller needing the `paging` module — the description
// strings above already inline its `CANONICAL_DESCRIPTION` text.
#[allow(dead_code)]
const _PAGING_REF: &str = paging::CANONICAL_DESCRIPTION;
