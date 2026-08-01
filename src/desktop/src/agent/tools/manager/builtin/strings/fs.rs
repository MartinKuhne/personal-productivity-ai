//! User-visible description strings for the filesystem tool family.

// --- replace_text ---

pub const REPLACE_TEXT_DESCRIPTION: &str =
    "Replace exact occurrences of old_string with new_string in a file.";

// --- grep ---

pub const GREP_DESCRIPTION: &str = "Search for a query string case-insensitively across all Markdown files in the workspace. Returns at most 200 matching lines; when the result is truncated, refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.";

pub const FIELD_GREP_INPUT_QUERY: &str = "The search term.";

pub const FIELD_GREP_RESPONSE_MATCHES: &str = "Match lines (`virtual/path:line - content`), capped at `DEFAULT_GREP_MAX_RESULTS` matches. Set to `\"No matches found.\"` when the query matched nothing.";

pub const FIELD_GREP_RESPONSE_TOTAL: &str = "Total number of matches found across all libraries, including any beyond the result cap. Lets the caller tell when `matches` was truncated.";

pub const FIELD_GREP_RESPONSE_TRUNCATED: &str = "True when `matches` was truncated because the query matched more than `DEFAULT_GREP_MAX_RESULTS` lines.";

// --- read_tags ---

pub const READ_TAGS_DESCRIPTION: &str =
    "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.";

// --- list_files_by_tag ---

pub const LIST_FILES_BY_TAG_DESCRIPTION: &str = "List Markdown files that contain a specific tag in their front-matter. Results are returned as a JSON array, paginated across all configured libraries (default page size 20); every response includes the total number of matching files so the caller can drive follow-up page requests.";

pub const FIELD_LIST_FILES_BY_TAG_INPUT_PAGE: &str =
    "1-indexed page number. Defaults to `1` if omitted.";

pub const FIELD_LIST_FILES_BY_TAG_INPUT_PAGE_SIZE: &str =
    "Number of files to return per page. Defaults to `20` if omitted.";

pub const FIELD_LIST_FILES_BY_TAG_RESPONSE_FILES: &str = "JSON array of virtual file paths for the requested page (no library prefix is applied when the result is empty).";

pub const FIELD_LIST_FILES_BY_TAG_RESPONSE_TOTAL: &str = "Total number of files matching the tag, across all pages. This is returned on every response so the caller can size follow-up page requests without having to read the whole library.";

pub const FIELD_LIST_FILES_BY_TAG_RESPONSE_HINT: &str = "When the requested `page` is past the end, this field is set to a human-readable hint explaining why `files` is empty. When the page is in range, the field is `None`.";

// --- list_files ---

pub const LIST_FILES_DESCRIPTION: &str = "List Markdown files in a directory (not recursive). Results are returned as a JSON array, paginated (default page size 20); every response includes the total number of files in the directory so the caller can drive follow-up page requests. With `path` set to \"/\" or \".\" returns the configured content libraries.";

pub const FIELD_LIST_FILES_INPUT_PAGE: &str = "1-indexed page number. Defaults to `1` if omitted.";

pub const FIELD_LIST_FILES_INPUT_PAGE_SIZE: &str =
    "Number of files to return per page. Defaults to `20` if omitted.";

pub const FIELD_LIST_FILES_RESPONSE_FILES: &str =
    "JSON array of virtual file paths for the requested page.";

pub const FIELD_LIST_FILES_RESPONSE_TOTAL: &str = "Total number of files in the requested directory (non-recursive), across all pages. Returned on every response so the caller can size follow-up page requests.";

pub const FIELD_LIST_FILES_RESPONSE_HINT: &str = "When the requested `page` is past the end, this field is set to a human-readable hint. `None` when the page is in range.";

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
