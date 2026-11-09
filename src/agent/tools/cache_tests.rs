//! Tests for the shared tool cache: document round-trips, shallow clone semantics and spec constants.

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
