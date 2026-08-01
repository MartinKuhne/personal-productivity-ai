//! User-visible description strings for the web tool family.

use super::paging;

// --- web_delegate ---

pub const WEB_DELEGATE_DESCRIPTION: &str = "Delegate web searches and web fetches to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information.";

// --- web_fetch ---

pub const WEB_FETCH_DESCRIPTION: &str = "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise). Fetches the URL and converts HTML to Markdown. `limit` is the line count, `offset` skips lines. `total_lines` is the line count of the full Markdown body. Cached for 5 minutes; pass `force_refetch=true` to bypass. Defaults: `offset=0` lines, `limit=100` lines.";

pub const FIELD_WEB_FETCH_RESPONSE_TOTAL_LINES: &str = "Total number of Markdown lines in the full fetched body. Use this together with `offset` and `limit` to paginate through the content.";

pub const FIELD_WEB_FETCH_RESPONSE_FROM_CACHE: &str =
    "True when the response was served from the shared cache.";

// --- web_search ---

pub const WEB_SEARCH_DESCRIPTION: &str = "Search the web using SearXNG.";

pub const FIELD_WEB_SEARCH_INPUT_QUERY: &str = "The search term.";

// Suppress the unused-import warning when the binary is built
// without any caller needing the `paging` module — the description
// string above already inlines its `CANONICAL_DESCRIPTION` text.
#[allow(dead_code)]
const _PAGING_REF: &str = paging::CANONICAL_DESCRIPTION;
