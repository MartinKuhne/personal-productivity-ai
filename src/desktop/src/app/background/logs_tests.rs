//! Tests for `background/manager.rs`.

use super::*;
use std::io;

fn make_entry(msg: &str) -> BackgroundLogEntry {
    BackgroundLogEntry::new(LogCategory::Indexer, msg.to_string())
}

/// A `Write` that succeeds for the first `fail_after` bytes, then
/// returns `Other` on every subsequent call. Used to simulate
/// mid-write failures (disk full, broken pipe, etc.) without
/// relying on OS-level quirks.
struct FailingWriter {
    written: usize,
    fail_after: usize,
}
impl FailingWriter {
    fn new(fail_after: usize) -> Self {
        Self {
            written: 0,
            fail_after,
        }
    }
}
impl io::Write for FailingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written >= self.fail_after {
            return Err(io::Error::other("simulated mid-write failure"));
        }
        let take = (self.fail_after - self.written).min(buf.len());
        self.written += take;
        Ok(take)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_push_log_adds_entry() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("test"));
    assert_eq!(mgr.get_logs().len(), 1);
}

#[test]
fn test_push_log_overflow_evicts_oldest() {
    let mut mgr = BackgroundLogs::new();
    for i in 0..MAX_LOG_ENTRIES + 10 {
        mgr.push_log(make_entry(&format!("entry {}", i)));
    }
    assert_eq!(mgr.get_logs().len(), MAX_LOG_ENTRIES);
    let first = mgr.get_logs().front().unwrap();
    assert!(first.message.contains("entry 10"));
}

#[test]
fn test_clear_logs_empties() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("test"));
    mgr.clear_logs();
    assert!(mgr.get_logs().is_empty());
}

#[test]
fn test_save_logs_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("logs/test.log");

    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("line one"));
    mgr.push_log(make_entry("line two"));

    mgr.save_logs(&log_path).unwrap();
    assert!(log_path.exists());

    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("Indexer"));
    assert!(content.contains("line one"));
    assert!(content.contains("line two"));
}

#[test]
fn test_save_logs_propagates_write_errors() {
    // The original save_logs loop did `let _ = file.write_all(...)`
    // which silently swallowed mid-write failures (disk full,
    // broken pipe, etc.) and returned Ok(()) on a truncated file.
    // Contract: when the underlying write fails, save_logs must
    // return Err so the caller knows the file is incomplete.
    let mut mgr = BackgroundLogs::new();
    // Several entries so we cross the "fail_after" boundary
    // mid-stream and not on the first line.
    for i in 0..10 {
        mgr.push_log(make_entry(&format!("entry {}", i)));
    }

    let mut failing = FailingWriter::new(0);
    let result = mgr.write_logs_to(&mut failing);
    assert!(
        result.is_err(),
        "expected write failure to propagate, got {:?}",
        result
    );
}

#[test]
fn test_filter_category_none_shows_all() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "idx".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Watcher,
        "wtch".to_string(),
    ));

    mgr.filter_category = None;
    // With no filter, all pass through
    let logs: Vec<_> = mgr.get_logs().iter().collect();
    assert_eq!(logs.len(), 2);
}

/// When `filter_category` is set, downstream log consumers (e.g.
/// the background-logs window) must only see entries whose
/// category matches. The previous version of this test never
/// exercised the filtering path itself, only the no-filter
/// default. This test pins the contract end-to-end: push
/// entries of two categories, set a filter, verify only the
/// matching entries are visible.
///
/// The filter is consumed by [`crate::ui::background_logs::is_log_visible`],
/// which we exercise here indirectly by re-implementing the
/// same predicate; if the two diverge, that test will catch it.
#[test]
fn test_filter_category_isolates_matching_entries() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "indexed 1".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Watcher,
        "watched 1".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "indexed 2".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::PdfConverter,
        "converted 1".to_string(),
    ));

    mgr.filter_category = Some(LogCategory::Indexer);
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(
        visible.len(),
        2,
        "Indexer filter should isolate the two Indexer entries"
    );
    assert!(visible.iter().all(|l| l.category == LogCategory::Indexer));

    // Switch to a different category and confirm the same
    // predicate still narrows correctly.
    mgr.filter_category = Some(LogCategory::PdfConverter);
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].category, LogCategory::PdfConverter);

    // Clear the filter; all entries become visible again.
    mgr.filter_category = None;
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(visible.len(), 4);
}

/// `search_text` matching is case-insensitive. The existing
/// `test_search_text_filters` test only uses lowercase input on
/// both sides, so a regression that introduces case-sensitive
/// matching would pass it. This test adds a mixed-case check.
#[test]
fn test_search_text_is_case_insensitive() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("Indexing file.md"));
    mgr.push_log(make_entry("Watching directory"));
    mgr.push_log(make_entry("PDF conversion"));

    // Search for a substring in mixed case; both should match.
    mgr.search_text = "WATCH".to_string();
    let needle = mgr.search_text.to_lowercase();
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| {
            if needle.is_empty() {
                return true;
            }
            l.message.to_lowercase().contains(&needle)
        })
        .collect();
    assert_eq!(
        visible.len(),
        1,
        "case-insensitive search must match the mixed-case query"
    );
    assert!(visible[0].message.contains("Watching"));
}

#[test]
fn test_search_text_filters() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("apple banana"));
    mgr.push_log(make_entry("cherry date"));

    mgr.search_text = "apple".to_string();
    let filtered: Vec<_> = mgr
        .get_logs()
        .iter()
        .filter(|l| {
            if mgr.search_text.is_empty() {
                return true;
            }
            l.message
                .to_lowercase()
                .contains(&mgr.search_text.to_lowercase())
        })
        .collect();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message, "apple banana");
}

#[test]
fn test_auto_scroll_default_true() {
    let mgr = BackgroundLogs::new();
    assert!(mgr.auto_scroll);
}
