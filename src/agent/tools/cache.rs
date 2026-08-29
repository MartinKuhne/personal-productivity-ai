//! Shared tool cache and cursor state management (TOOL-006, TOOL-025..033, TOOL-047).
//!
//! Powered by `mini_moka` for lock-optimized in-memory caching with automatic
//! 30-minute TTL expiration and 256 entry capacity capping (TOOL-030).

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

/// One `search_email` item: the JMAP client name and the simplified email JSON value.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct SearchEmailItem {
    /// JMAP account / client identifier.
    pub client: String,
    /// Email body / summary payload.
    pub email: Value,
}

/// A cached web document (HTML-converted Markdown plus headers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedWebDocument {
    /// Converted markdown body.
    pub content: String,
    /// Response headers (if captured).
    pub response_headers: HashMap<String, String>,
}

/// Shared in-memory tool cache holding document caches and cursor pagination sessions (TOOL-030, TOOL-033).
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
    pub calendar_search_sessions: CursorSessionManager<String>,
    /// Calendar range sessions for `get_calendar` (TOOL-026g: 32 events).
    pub calendar_get_sessions: CursorSessionManager<String>,
    /// Contact search sessions for `search_contact` (TOOL-026h: 32 contacts).
    pub contact_search_sessions: CursorSessionManager<String>,
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
mod tests {
    use super::*;

    #[test]
    fn web_document_cache_round_trip() {
        let cache = ToolCache::new();
        let doc = CachedWebDocument {
            content: "test content".to_string(),
            response_headers: HashMap::new(),
        };
        let key = "https://example.com".to_string();
        cache.web_documents.insert(key.clone(), doc.clone());

        let cached = cache.web_documents.get(&key);
        assert_eq!(cached, Some(doc));
    }

    #[test]
    fn clone_is_shallow() {
        let cache = ToolCache::new();
        let doc = CachedWebDocument {
            content: "doc1".to_string(),
            response_headers: HashMap::new(),
        };
        let key = "https://example.com/test".to_string();
        cache.web_documents.insert(key.clone(), doc);

        let cloned = cache.clone();
        assert!(cloned.web_documents.get(&key).is_some());

        cloned.web_documents.invalidate(&key);
        assert!(cache.web_documents.get(&key).is_none());
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = ToolCache::new();
        let doc = CachedWebDocument {
            content: "content".to_string(),
            response_headers: HashMap::new(),
        };
        let key = "https://example.com".to_string();
        cache.web_documents.insert(key.clone(), doc);
        cache.web_documents.invalidate(&key);
        assert!(cache.web_documents.get(&key).is_none());
    }

    #[test]
    fn ttl_and_capacity_constants_match_spec() {
        // TOOL-030 pins a 30-minute TTL and 256-entry cap; any change
        // to these constants is deliberate and must be surfaced.
        assert_eq!(CACHE_TTL, Duration::from_secs(1800));
        assert_eq!(MAX_CACHE_ENTRIES, 256);
        assert_eq!(
            CURSOR_EXPIRED_ERROR,
            "Cursor expired or unknown; re-run the search with no cursor."
        );
        assert_eq!(FINAL_PAGE_HINT, "Final page.");
    }

    #[test]
    fn global_cache_singleton_is_shared() {
        // `cache()` returns the same LazyLock singleton across calls.
        let a = cache();
        let b = cache();
        assert!(std::ptr::eq(a, b));
    }
}
