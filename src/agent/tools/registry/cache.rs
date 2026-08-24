//! Shared tool cache and cursor state management (TOOL-006, TOOL-025..032, TOOL-047).
//!
//! Powered by `mini_moka` for lock-optimized in-memory caching with automatic
//! TTL expiration and capacity capping.

use crate::tools::registry::cursor::CursorSessionManager;
use mini_moka::sync::Cache;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

/// TTL for cache entries shared by tools (30 minutes per TOOL-030).
pub const CACHE_TTL: Duration = Duration::from_secs(1800);

/// Capacity cap for tool cache entries (1024 entries per TOOL-030).
pub const MAX_CACHE_ENTRIES: u64 = 1024;

/// One `search_email` item: the JMAP client name and the simplified email JSON value.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Shared in-memory tool cache holding document caches and cursor pagination sessions.
///
/// `Clone` is a cheap shallow clone of the internal `mini_moka` handles.
#[derive(Clone, Debug)]
pub struct ToolCache {
    /// Cache of URL -> fetched Markdown document.
    pub web_documents: Cache<String, CachedWebDocument>,
    /// Line-based pagination sessions for `web_fetch`.
    pub web_lines: CursorSessionManager<String>,
    /// Item-based pagination sessions for `search_email`.
    pub email_sessions: CursorSessionManager<SearchEmailItem>,
    /// Hit-based pagination sessions for `vector_search`.
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
    /// Create a new tool cache with 30-minute TTL and 1024 capacity.
    pub fn new() -> Self {
        let web_documents = Cache::builder()
            .max_capacity(MAX_CACHE_ENTRIES)
            .time_to_live(CACHE_TTL)
            .build();
        let web_lines = CursorSessionManager::new(
            64,
            crate::tools::registry::builtin::strings::WEB_FETCH_FINAL_PAGE_HINT,
            "Cursor expired or unknown; re-run the fetch with no cursor.",
        );
        let email_sessions = CursorSessionManager::new(
            25,
            crate::tools::jmap::email::SEARCH_EMAIL_FINAL_PAGE_HINT,
            "Cursor expired or unknown; re-run the search with no cursor.",
        );
        #[cfg(feature = "vector-search")]
        let vector_sessions = CursorSessionManager::new(
            5,
            "Final page. All matching records returned.",
            "Cursor expired or unknown; re-run the search with no cursor.",
        );
        Self {
            web_documents,
            web_lines,
            email_sessions,
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
}
