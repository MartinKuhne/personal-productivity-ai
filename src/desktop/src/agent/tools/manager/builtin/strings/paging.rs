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
    "Specify the number of items to skip from the start (0-indexed). Default: 0.";

/// `limit` parameter description. The default value is substituted
/// per tool via the per-family domain sentence.
pub const FIELD_LIMIT_DESCRIPTION: &str = "Specify the number of items to return. Default: {N}.";

/// `total` response field description. Used on every list-paginated
/// tool's `total` field.
pub const FIELD_TOTAL_DESCRIPTION: &str = "The total number of items across all pages.";

/// `hint` response field description. Used on every list-paginated
/// tool's `hint` field.
pub const FIELD_HINT_DESCRIPTION: &str =
    "Displays a message when the offset exceeds total results or when no matches exist.";

/// Canonical description paragraph for list-paginated tools. Each
/// tool's `*_DESCRIPTION` const appends a one-sentence domain
/// sentence. Combined length MUST stay under the plan's 60-word cap.
pub const CANONICAL_DESCRIPTION: &str = "Return a paginated list. Use `offset` to skip items and `limit` to set page size. The response includes `total` items and an optional status `hint`.";
