//! User-visible description strings for the web tool family.

use super::paging;

// --- web_delegate ---

pub const WEB_DELEGATE_DESCRIPTION: &str = "Delegate web searches and fetches to a sub-agent. Provide clear instructions. The sub-agent returns summarized information.";

// --- web_fetch ---

pub const WEB_FETCH_DESCRIPTION: &str = "Fetch a URL and convert the content to Markdown. Use `offset` and `limit` to paginate lines. Default parameters: `offset=0`, `limit=100`.";

pub const FIELD_WEB_FETCH_RESPONSE_TOTAL_LINES: &str =
    "Total number of Markdown lines in the fetched body.";

pub const FIELD_WEB_FETCH_RESPONSE_FROM_CACHE: &str =
    "Set to `true` when the response comes from cache.";

// --- web_search ---

pub const WEB_SEARCH_DESCRIPTION: &str = "Search the web for information using a query string.";

pub const FIELD_WEB_SEARCH_INPUT_QUERY: &str = "Specify the search term.";

// Suppress the unused-import warning when the binary is built
// without any caller needing the `paging` module — the description
// string above already inlines its `CANONICAL_DESCRIPTION` text.
#[allow(dead_code)]
const _PAGING_REF: &str = paging::CANONICAL_DESCRIPTION;
