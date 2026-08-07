//! Shared tool cache — holds paginated tool state across calls.
//!
//! Used by `web_fetch` to cache fetched Markdown bodies, and by
//! `search_email` to cache server result sets so cursor-based
//! pagination does not re-query JMAP. The cache is process-local,
//! `Mutex`-guarded, and lazily evicts entries after [`CACHE_TTL`]
//! on every access. A soft cap of [`MAX_CACHE_ENTRIES`] is enforced
//! with FIFO eviction once exceeded.
//!
//! Single-process desktop app only. Multi-process / cross-restart
//! caching is out of scope.

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// TTL for cache entries shared by `web_fetch` and `search_email`.
/// 30 minutes. Reviewed in `doc/planning/tool-paging-audit-and-migration.md`
/// as part of the cursor-based `search_email` migration; extended from the
/// original 5-minute value to reduce spurious "Cursor expired or unknown"
/// errors in long-running agent turns.
pub const CACHE_TTL: Duration = Duration::from_secs(1800);

/// Soft cap on the number of cache entries. When exceeded, the
/// oldest-inserted entry is evicted (FIFO) on the next insert.
pub const MAX_CACHE_ENTRIES: usize = 1024;

/// One cache entry. Variants correspond to tools that participate
/// in the shared cache.
#[derive(Clone)]
pub enum CacheEntry {
    /// `web_fetch` result. The URL is the cache key, storing the cursor UUID.
    WebFetch { cursor: String },
    /// `web_fetch` full content stored under cursor UUID.
    WebFetchContent {
        content: String,
        response_headers: HashMap<String, String>,
        fetched_at: Instant,
        cursor_offset: usize,
        total_lines: usize,
    },
    /// `search_email` result. The cursor is the cache key.
    SearchEmail(SearchEmailCacheEntry),
}

/// One `search_email` cache entry. The cache stores the full server
/// result set so subsequent cursor calls slice from memory.
#[derive(Clone)]
pub struct SearchEmailCacheEntry {
    /// The full server result set, in the order the server returned.
    pub items: Vec<SearchEmailItem>,
    /// The number of items already returned to the LLM. The next call
    /// returns items starting at this offset.
    pub cursor_offset: usize,
    /// Total number of items in the result set, captured at first fetch.
    pub total: usize,
    /// Insertion time, for TTL eviction.
    pub fetched_at: Instant,
    /// Per-client error messages collected during the first fetch.
    pub errors: Vec<String>,
}

/// One `search_email` item: the JMAP client name and the simplified
/// email JSON value.
#[derive(Clone)]
pub struct SearchEmailItem {
    pub client: String,
    pub email: Value,
}

/// Process-local shared cache. Use [`cache`] to get the singleton.
pub struct ToolCache {
    state: Mutex<CacheState>,
}

struct CacheState {
    entries: HashMap<String, CacheEntry>,
    /// Insertion order, used for FIFO eviction when the cap is exceeded.
    insertion_order: VecDeque<String>,
}

static TOOL_CACHE: LazyLock<ToolCache> = LazyLock::new(ToolCache::new);

impl Default for ToolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                insertion_order: VecDeque::new(),
            }),
        }
    }

    /// Get a clone of the entry if it exists and is not expired. Triggers
    /// lazy eviction of any expired entries.
    pub fn get(&self, key: &str) -> Option<CacheEntry> {
        let mut state = self.state.lock().expect("cache mutex poisoned");
        state.evict_expired_locked();
        if let Some(entry) = state.entries.get(key).cloned() {
            if !is_expired(&entry, &state.entries) {
                return Some(entry);
            }
            // Expired: remove and report miss.
            state.entries.remove(key);
            state.insertion_order.retain(|k| k != key);
        }
        None
    }

    /// Insert or replace the entry under the given key. Enforces the
    /// FIFO cap on new keys.
    pub fn put(&self, key: String, value: CacheEntry) {
        let mut state = self.state.lock().expect("cache mutex poisoned");
        state.evict_expired_locked();
        if !state.entries.contains_key(&key) {
            // New key: enforce FIFO cap before inserting.
            while state.entries.len() >= MAX_CACHE_ENTRIES {
                if let Some(oldest) = state.insertion_order.pop_front() {
                    state.entries.remove(&oldest);
                } else {
                    break;
                }
            }
            state.insertion_order.push_back(key.clone());
        }
        state.entries.insert(key, value);
    }

    /// Remove the entry under the given key.
    pub fn invalidate(&self, key: &str) {
        let mut state = self.state.lock().expect("cache mutex poisoned");
        state.entries.remove(key);
        state.insertion_order.retain(|k| k != key);
    }

    /// Current number of live entries (post-eviction). Exposed for tests.
    pub fn len(&self) -> usize {
        let state = self.state.lock().expect("cache mutex poisoned");
        state.entries.len()
    }

    /// Whether the cache is empty. Exposed for tests.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl CacheState {
    fn evict_expired_locked(&mut self) {
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| is_expired(e, &self.entries))
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired {
            self.entries.remove(&k);
            self.insertion_order.retain(|x| x != &k);
        }
    }
}

fn is_expired(entry: &CacheEntry, entries: &std::collections::HashMap<String, CacheEntry>) -> bool {
    match entry {
        CacheEntry::WebFetch { cursor } => {
            // Only check content expiration if the content exists
            if let Some(CacheEntry::WebFetchContent { fetched_at, .. }) = entries.get(cursor) {
                fetched_at.elapsed() >= CACHE_TTL
            } else {
                // Content doesn't exist yet, don't expire the WebFetch entry
                // The content will be populated on first fetch
                false
            }
        }
        CacheEntry::WebFetchContent { fetched_at, .. } => fetched_at.elapsed() >= CACHE_TTL,
        CacheEntry::SearchEmail(e) => e.fetched_at.elapsed() >= CACHE_TTL,
    }
}

/// Get the global cache singleton.
pub fn cache() -> &'static ToolCache {
    &TOOL_CACHE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn web_entry(cursor: &str) -> CacheEntry {
        CacheEntry::WebFetch {
            cursor: cursor.to_string(),
        }
    }

    #[test]
    fn put_and_get_round_trip() {
        let cache = ToolCache::new();
        cache.put("test:put_and_get".to_string(), web_entry("test-cursor"));
        let entry = cache.get("test:put_and_get");
        match entry {
            Some(CacheEntry::WebFetch { cursor }) => assert_eq!(cursor, "test-cursor"),
            _ => panic!("expected WebFetch entry"),
        }
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = ToolCache::new();
        cache.put("test:invalidate".to_string(), web_entry("x"));
        cache.invalidate("test:invalidate");
        assert!(cache.get("test:invalidate").is_none());
    }

    #[test]
    fn get_after_ttl_returns_none() {
        let cache = ToolCache::new();
        // Construct an entry whose fetched_at is well past CACHE_TTL.
        let stale = CacheEntry::WebFetchContent {
            content: "stale".to_string(),
            response_headers: HashMap::new(),
            fetched_at: Instant::now() - CACHE_TTL - Duration::from_secs(10),
            cursor_offset: 0,
            total_lines: 1,
        };
        cache.put("test:stale_content".to_string(), stale);
        // Put the WebFetch entry pointing to the stale content
        cache.put(
            "test:stale".to_string(),
            CacheEntry::WebFetch {
                cursor: "test:stale_content".to_string(),
            },
        );
        // After TTL, the content is expired, so the content entry should be None
        assert!(cache.get("test:stale_content").is_none());
        // And the eviction sweep removed the content from the map.
        assert!(cache.get("test:stale_content").is_none());
    }

    #[test]
    fn put_at_cap_evicts_oldest() {
        let cache = ToolCache::new();
        // Pre-fill to MAX_CACHE_ENTRIES with sentinel keys we can recognize.
        for i in 0..MAX_CACHE_ENTRIES {
            cache.put(
                format!("test:cap:{i}"),
                CacheEntry::WebFetch {
                    cursor: format!("cursor{i}"),
                },
            );
        }
        // One more insert should evict the oldest (the first sentinel).
        cache.put(
            "test:cap:overflow".to_string(),
            CacheEntry::WebFetch {
                cursor: "overflow".to_string(),
            },
        );
        assert_eq!(cache.len(), MAX_CACHE_ENTRIES);
        assert!(cache.get("test:cap:0").is_none());
        assert!(cache.get("test:cap:overflow").is_some());
    }

    #[test]
    fn replace_existing_key_does_not_grow() {
        let cache = ToolCache::new();
        cache.put(
            "test:replace".to_string(),
            CacheEntry::WebFetch {
                cursor: "v1".to_string(),
            },
        );
        cache.put(
            "test:replace".to_string(),
            CacheEntry::WebFetch {
                cursor: "v2".to_string(),
            },
        );
        let entry = cache.get("test:replace");
        match entry {
            Some(CacheEntry::WebFetch { cursor }) => assert_eq!(cursor, "v2"),
            _ => panic!("expected WebFetch entry"),
        }
    }
}
