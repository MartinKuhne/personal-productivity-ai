//! User-visible description strings for the filesystem tool family.

use super::paging;

// --- replace_text ---

pub const REPLACE_TEXT_DESCRIPTION: &str =
    "Replace exact occurrences of target text with replacement text in a file.";

// --- grep ---

pub const GREP_DESCRIPTION: &str = "Search Markdown files for text. The tool returns up to 200 matching lines. If the tool truncates results, refine your query or use a sub-agent.";

pub const FIELD_GREP_INPUT_QUERY: &str = "Specify the search term.";

pub const FIELD_GREP_RESPONSE_MATCHES: &str = "Contains matching lines up to 200 results. Returns `\"No matches found.\"` when no matches exist.";

pub const FIELD_GREP_RESPONSE_TOTAL: &str =
    "Total number of matching lines found across all libraries.";

pub const FIELD_GREP_RESPONSE_TRUNCATED: &str =
    "Set to `true` when total matches exceed 200 lines.";

// --- read_tags ---

pub const READ_TAGS_DESCRIPTION: &str =
    "Get all unique tags from front-matter headers in workspace Markdown files.";

// --- list_files_by_tag ---

pub const LIST_FILES_BY_TAG_DESCRIPTION: &str = "Return a paginated list of Markdown files that contain a tag in their front-matter. Default parameters: `offset=0`, `limit=100`.";

pub const FIELD_LIST_FILES_BY_TAG_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested page slice.";

// --- list_files ---

pub const LIST_FILES_DESCRIPTION: &str = "Return a paginated list of Markdown files in a directory. Use path `/` or `.` to list content libraries. Default parameters: `offset=0`, `limit=100`.";

pub const FIELD_LIST_FILES_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested page slice.";

// --- read_file ---

pub const READ_FILE_DESCRIPTION: &str = "Read the full text of a file at a path. Use `read_yaml_header` if you only need a document summary.";

// --- read_file_lines ---

pub const READ_FILE_LINES_DESCRIPTION: &str =
    "Read specific line ranges from a file using 1-indexed line numbers.";

// --- create_file ---

pub const CREATE_FILE_DESCRIPTION: &str =
    "Create a file at the specified path with provided content.";

// --- insert_lines ---

pub const INSERT_LINES_DESCRIPTION: &str =
    "Insert lines into a file at a specified 1-indexed line index.";

// --- delete_lines ---

pub const DELETE_LINES_DESCRIPTION: &str =
    "Delete specific lines from a file using 1-indexed line numbers.";

// Suppress the unused-import warning when the binary is built
// without any caller needing the `paging` module — the description
// strings above already inline its `CANONICAL_DESCRIPTION` text.
#[allow(dead_code)]
const _PAGING_REF: &str = paging::CANONICAL_DESCRIPTION;
