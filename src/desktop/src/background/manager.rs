use crate::background::models::{BackgroundLogEntry, LogCategory};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub const MAX_LOG_ENTRIES: usize = 10_000;

pub struct BackgroundProcessManager {
    logs: VecDeque<BackgroundLogEntry>,
    pub filter_category: Option<LogCategory>,
    pub search_text: String,
    pub auto_scroll: bool,
}

impl Default for BackgroundProcessManager {
    fn default() -> Self {
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            filter_category: None,
            search_text: String::new(),
            auto_scroll: true,
        }
    }
}

impl BackgroundProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_log(&mut self, entry: BackgroundLogEntry) {
        if self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(entry);
    }

    pub fn get_logs(&self) -> &VecDeque<BackgroundLogEntry> {
        &self.logs
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn save_logs(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = File::create(path)?;
        for log in &self.logs {
            let line = format!(
                "[{}] [{}] {}\n",
                log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                log.category,
                log.message
            );
            let _ = file.write_all(line.as_bytes());
        }
        Ok(())
    }
}

pub type SharedProcessManager = Arc<Mutex<BackgroundProcessManager>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(msg: &str) -> BackgroundLogEntry {
        BackgroundLogEntry::new(LogCategory::Indexer, msg.to_string())
    }

    #[test]
    fn test_push_log_adds_entry() {
        let mut mgr = BackgroundProcessManager::new();
        mgr.push_log(make_entry("test"));
        assert_eq!(mgr.get_logs().len(), 1);
    }

    #[test]
    fn test_push_log_overflow_evicts_oldest() {
        let mut mgr = BackgroundProcessManager::new();
        for i in 0..MAX_LOG_ENTRIES + 10 {
            mgr.push_log(make_entry(&format!("entry {}", i)));
        }
        assert_eq!(mgr.get_logs().len(), MAX_LOG_ENTRIES);
        let first = mgr.get_logs().front().unwrap();
        assert!(first.message.contains("entry 10"));
    }

    #[test]
    fn test_clear_logs_empties() {
        let mut mgr = BackgroundProcessManager::new();
        mgr.push_log(make_entry("test"));
        mgr.clear_logs();
        assert!(mgr.get_logs().is_empty());
    }

    #[test]
    fn test_save_logs_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("logs/test.log");

        let mut mgr = BackgroundProcessManager::new();
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
    fn test_filter_category_none_shows_all() {
        let mut mgr = BackgroundProcessManager::new();
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

    #[test]
    fn test_search_text_filters() {
        let mut mgr = BackgroundProcessManager::new();
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
        let mgr = BackgroundProcessManager::new();
        assert!(mgr.auto_scroll);
    }
}
