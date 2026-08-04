//! Canonical LLM-facing strings for cursor-based pagination.
//!
//! Used by `search_email`. The cursor is an opaque string that the
//! tool returns and the LLM passes back unchanged. The cache key
//! for the underlying shared `ToolCache` is the same string.

/// `cursor` field description. Used on both the input and output
/// `cursor` fields of `search_email` and `web_fetch` because the LLM passes back
/// whatever the tool returned.
pub const FIELD_CURSOR_DESCRIPTION: &str = "Pass this pagination token back unchanged to get the next page. The tool generates this token on the first call.";

/// Description for `search_email`. Replaces the offset/limit
/// canonical paragraph because the cursor flow is fundamentally
/// different. The full tool description is this paragraph + the
/// domain sentence in `strings::jmap::SEARCH_EMAIL_DOMAIN`.
pub const SEARCH_EMAIL_CANONICAL_DESCRIPTION: &str = "Search emails by keyword, folder, date range, sender, recipient, or status. You must provide at least one filter. The tool returns up to 100 matching emails and a cursor token for pagination.";

/// Description for `web_fetch` cursor-based pagination. The tool returns
/// up to 100 lines of Markdown content and a cursor token for pagination.
pub const WEB_FETCH_CURSOR_DESCRIPTION: &str = "Fetch a URL and convert the content to Markdown. Returns up to 100 lines and a cursor token for pagination. Use the cursor to fetch the next page. Cache lasts 5 minutes; use force_refetch=true to bypass.";

/// Hint string emitted on the final page of a `web_fetch` cursor
/// pagination.
pub const WEB_FETCH_FINAL_PAGE_HINT: &str = "Final page.";
