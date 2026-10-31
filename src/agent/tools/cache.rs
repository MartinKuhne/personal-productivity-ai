//! Shared tool cache and cursor state management (TOOL-006, TOOL-025..033, TOOL-047).
//!
//! Powered by `mini_moka` for lock-optimized in-memory caching with automatic
//! 30-minute TTL expiration and 256 entry capacity capping (TOOL-030).
//!
//! Unit tests live in the sibling `cache_tests.rs` sidecar.

use crate::tools::cursor::CursorSessionManager;
use mini_moka::sync::Cache;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

/// TTL for cache entries shared by tools (30 minutes per TOOL-030).
pub const CACHE_TTL: Duration = Duration::from_secs(1800);

/// Capacity cap for tool cache entries (256 entries per TOOL-030).
pub const MAX_CACHE_ENTRIES: u64 = 256;

/// Standard cursor expired error message (TOOL-031).
pub const CURSOR_EXPIRED_ERROR: &str =
    "Cursor expired or unknown; re-run the search with no cursor.";

/// Standard final page hint string (TOOL-025).
pub const FINAL_PAGE_HINT: &str = "Final page.";

/// One `search_email` item: the JMAP client name and the simplified email preview value.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct SearchEmailItem {
    /// JMAP account / client identifier.
    pub client: String,
    /// Partial email content preview. Use `get_email_by_id` with the email `id` to read the full content.
    #[schemars(
        description = crate::tools::registry::builtin::strings::FIELD_SEARCH_EMAIL_ITEM_PREVIEW_DESC
    )]
    pub preview: Value,
}

/// A cached web document (HTML-converted Markdown plus headers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedWebDocument {
    /// Converted markdown body.
    pub content: String,
    /// Response headers (if captured).
    pub response_headers: HashMap<String, String>,
}

/// Shared in-memory tool cache holding document caches and cursor pagination sessions (TOOL-030, TOOL-049).
///
/// `Clone` is a cheap shallow clone of the internal `mini_moka` handles.
#[derive(Clone, Debug)]
pub struct ToolCache {
    /// Cache of URL -> fetched Markdown document.
    pub web_documents: Cache<String, CachedWebDocument>,
    /// Line-based pagination sessions for `web_fetch` (TOOL-026b: 64 lines).
    pub web_lines: CursorSessionManager<String>,
    /// Match-line pagination sessions for `search_notes` (TOOL-026c: 64 matches).
    pub search_notes_sessions: CursorSessionManager<String>,
    /// File-name pagination sessions for `list_notes_by_tag` (TOOL-026d: 64 files).
    pub list_notes_by_tag_sessions: CursorSessionManager<String>,
    /// Result-item pagination sessions for `web_search` (TOOL-026e: 32 results).
    pub web_search_sessions: CursorSessionManager<String>,
    /// Item-based pagination sessions for `search_email` (TOOL-026a: 32 emails).
    pub email_sessions: CursorSessionManager<SearchEmailItem>,
    /// Calendar search sessions for `search_calendar` (TOOL-026f: 32 events).
    pub calendar_search_sessions: CursorSessionManager<crate::tools::dtos::CalDavEventDetails>,
    /// Calendar range sessions for `get_calendar` (TOOL-026g: 32 events).
    pub calendar_get_sessions: CursorSessionManager<crate::tools::dtos::CalDavEventDetails>,
    /// Contact search sessions for `search_contact` (TOOL-026h: 32 contacts).
    pub contact_search_sessions: CursorSessionManager<crate::tools::dtos::CardDavContactDetails>,
    /// Hit-based pagination sessions for `vector_search` (TOOL-026i: 32 hits).
    #[cfg(feature = "vector-search")]
    pub vector_sessions: CursorSessionManager<crate::tools::vector_search::VectorSearchHit>,
}

static TOOL_CACHE: LazyLock<ToolCache> = LazyLock::new(ToolCache::new);

impl Default for ToolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCache {
    /// Create a new tool cache with 30-minute TTL and 256 capacity (TOOL-030).
    pub fn new() -> Self {
        let web_documents = Cache::builder()
            .max_capacity(MAX_CACHE_ENTRIES)
            .time_to_live(CACHE_TTL)
            .build();
        let web_lines = CursorSessionManager::new(64, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let search_notes_sessions =
            CursorSessionManager::new(64, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let list_notes_by_tag_sessions =
            CursorSessionManager::new(64, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let web_search_sessions =
            CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let email_sessions = CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let calendar_search_sessions =
            CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let calendar_get_sessions =
            CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        let contact_search_sessions =
            CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);
        #[cfg(feature = "vector-search")]
        let vector_sessions = CursorSessionManager::new(32, FINAL_PAGE_HINT, CURSOR_EXPIRED_ERROR);

        Self {
            web_documents,
            web_lines,
            search_notes_sessions,
            list_notes_by_tag_sessions,
            web_search_sessions,
            email_sessions,
            calendar_search_sessions,
            calendar_get_sessions,
            contact_search_sessions,
            #[cfg(feature = "vector-search")]
            vector_sessions,
        }
    }
}

/// Get the global cache singleton.
pub fn cache() -> &'static ToolCache {
    &TOOL_CACHE
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
