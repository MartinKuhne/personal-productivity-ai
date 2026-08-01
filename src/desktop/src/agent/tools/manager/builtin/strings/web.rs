//! User-visible description strings for the web tool family.

// --- web_delegate ---

pub const WEB_DELEGATE_DESCRIPTION: &str = "Delegate web searches and web fetches to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information.";

// --- web_fetch ---

pub const WEB_FETCH_DESCRIPTION: &str = "Fetch content from a URL and convert to Markdown. Supports pagination via limit/offset to save context — fetch once, then read sections. Response includes total_lines for pagination. Content is cached for 5 minutes; use force_refetch=true to bypass cache.";

// --- web_search ---

pub const WEB_SEARCH_DESCRIPTION: &str = "Search the web using SearXNG.";

pub const FIELD_WEB_SEARCH_INPUT_QUERY: &str = "The search term.";
