//! Canonical LLM-facing strings for offset/limit pagination.
//!
//! Used by the list-paginated tools (`list_files`, `list_files_by_tag`,
//! `web_fetch`). These are the single source of truth for the
//! `offset`, `limit`, `total`, and `hint` field descriptions. Every
//! list-paginated tool MUST reference these consts so the LLM sees
//! the same wording across tools.

/// `offset` parameter description. Used on every list-paginated
/// tool's `offset` field.
pub const FIELD_OFFSET_DESCRIPTION: &str =
    "Number of items to skip from the start (0-indexed). Default 0.";

/// `limit` parameter description. The default value is substituted
/// per tool via the per-family domain sentence.
pub const FIELD_LIMIT_DESCRIPTION: &str = "Number of items to return. Default {N}.";

/// `total` response field description. Used on every list-paginated
/// tool's `total` field.
pub const FIELD_TOTAL_DESCRIPTION: &str = "Number of items across all pages.";

/// `hint` response field description. Used on every list-paginated
/// tool's `hint` field.
pub const FIELD_HINT_DESCRIPTION: &str = "Set to a message when the offset is past the end of the result or there are no matches. Absent otherwise.";

/// Canonical description paragraph for list-paginated tools. Each
/// tool's `*_DESCRIPTION` const appends a one-sentence domain
/// sentence. Combined length MUST stay under the plan's 60-word cap.
pub const CANONICAL_DESCRIPTION: &str = "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise).";
