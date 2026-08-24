//! Generic cursor session manager for paginated tool results (TOOL-006, TOOL-025..032, TOOL-047).
//! Unit tests live in the sibling `cursor_tests.rs` sidecar.

use crate::utils::uuid::UuidGenerator;
use mini_moka::sync::Cache;
use std::time::Duration;

/// Default TTL for cursor sessions (30 minutes per TOOL-030).
pub const DEFAULT_CURSOR_TTL: Duration = Duration::from_secs(1800);

/// Default capacity for cursor sessions (1024 entries per TOOL-030).
pub const DEFAULT_CURSOR_CAPACITY: u64 = 1024;

/// A single page slice returned by cursor pagination.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CursorPage<T> {
    /// Items in this page slice.
    pub items: Vec<T>,
    /// Next cursor to continue fetching, or `None` if this was the last page.
    pub cursor: Option<String>,
    /// Total items across all pages.
    pub total: usize,
    /// Informative terminal hint when pagination is complete.
    pub hint: Option<String>,
}

/// In-memory dataset held for an active cursor session.
#[derive(Clone, Debug)]
pub struct PagedDataset<T> {
    /// All items in the result set.
    pub items: Vec<T>,
    /// Next 0-indexed offset to slice from.
    pub cursor_offset: usize,
    /// Total item count.
    pub total: usize,
}

/// Generic, thread-safe cursor session manager backed by `mini_moka`.
#[derive(Clone, Debug)]
pub struct CursorSessionManager<T: Clone + Send + Sync + 'static> {
    sessions: Cache<String, PagedDataset<T>>,
    page_size: usize,
    final_page_hint: &'static str,
    expired_error: &'static str,
}

impl<T: Clone + Send + Sync + 'static> CursorSessionManager<T> {
    /// Create a new cursor session manager with default TTL and capacity.
    pub fn new(
        page_size: usize,
        final_page_hint: &'static str,
        expired_error: &'static str,
    ) -> Self {
        Self::with_options(
            page_size,
            DEFAULT_CURSOR_TTL,
            DEFAULT_CURSOR_CAPACITY,
            final_page_hint,
            expired_error,
        )
    }

    /// Create a new cursor session manager with custom options.
    pub fn with_options(
        page_size: usize,
        ttl: Duration,
        capacity: u64,
        final_page_hint: &'static str,
        expired_error: &'static str,
    ) -> Self {
        let sessions = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(ttl)
            .build();
        Self {
            sessions,
            page_size,
            final_page_hint,
            expired_error,
        }
    }

    /// Create a new cursor session from a full result set.
    ///
    /// If `items.len() <= page_size`, no session is stored in the cache,
    /// `cursor` is `None`, and the final page hint is returned.
    /// If `items.len() > page_size`, the first page is returned with a new
    /// cursor UUID, and remaining items are cached under that cursor.
    pub fn create_session(&self, items: Vec<T>, uuid_gen: &dyn UuidGenerator) -> CursorPage<T> {
        let total = items.len();
        if total == 0 {
            return CursorPage {
                items: Vec::new(),
                cursor: None,
                total: 0,
                hint: Some(self.final_page_hint.to_string()),
            };
        }

        if total <= self.page_size {
            return CursorPage {
                items,
                cursor: None,
                total,
                hint: Some(self.final_page_hint.to_string()),
            };
        }

        let cursor_id = format!("c_{}", &uuid_gen.new_v4().simple().to_string()[..8]);
        let page_items = items[..self.page_size].to_vec();
        let dataset = PagedDataset {
            items,
            cursor_offset: self.page_size,
            total,
        };
        self.sessions.insert(cursor_id.clone(), dataset);

        CursorPage {
            items: page_items,
            cursor: Some(cursor_id),
            total,
            hint: None,
        }
    }

    /// Fetch the next page for an active cursor.
    ///
    /// Returns `Err` if the cursor is unknown or expired.
    pub fn next_page(&self, cursor: &str) -> Result<CursorPage<T>, String> {
        let cursor_key = cursor.to_string();
        let entry = self
            .sessions
            .get(&cursor_key)
            .ok_or_else(|| self.expired_error.to_string())?;

        let offset = entry.cursor_offset;
        let total = entry.total;
        if offset >= total {
            self.sessions.invalidate(&cursor_key);
            return Err(self.expired_error.to_string());
        }

        let end = (offset + self.page_size).min(total);
        let page_items = entry.items[offset..end].to_vec();

        if end >= total {
            // Final page reached: invalidate the session
            self.sessions.invalidate(&cursor_key);
            Ok(CursorPage {
                items: page_items,
                cursor: None,
                total,
                hint: Some(self.final_page_hint.to_string()),
            })
        } else {
            // Update session offset
            let updated = PagedDataset {
                items: entry.items,
                cursor_offset: end,
                total,
            };
            self.sessions.insert(cursor_key, updated);
            Ok(CursorPage {
                items: page_items,
                cursor: Some(cursor.to_string()),
                total,
                hint: None,
            })
        }
    }

    /// Explicitly invalidate a cursor.
    pub fn invalidate(&self, cursor: &str) {
        let cursor_key = cursor.to_string();
        self.sessions.invalidate(&cursor_key);
    }

    /// Current number of active sessions.
    pub fn len(&self) -> u64 {
        self.sessions.entry_count()
    }

    /// Whether there are no active sessions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
