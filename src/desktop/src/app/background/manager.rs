//! Background log manager — bounded ring buffer of log entries with filtering/search state for the UI panel.
//!
//! Unit tests live in the sibling `manager_tests.rs` sidecar.

use crate::app::background::models::{BackgroundLogEntry, LogCategory};
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
    pub show_background_logs: bool,
}

impl Default for BackgroundProcessManager {
    fn default() -> Self {
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            filter_category: None,
            search_text: String::new(),
            auto_scroll: true,
            show_background_logs: false,
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
            std::fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        self.write_logs_to(&mut file)
    }

    /// Serialize the log buffer into the provided writer.
    ///
    /// Splitting this out from `save_logs` makes the write path unit-
    /// testable with a mock `Write` (see `FailingWriter` in the test
    /// module) and ensures that mid-write failures propagate instead
    /// of being silently swallowed. The previous implementation did
    /// `let _ = file.write_all(line.as_bytes())` per line, which
    /// returned `Ok(())` on a truncated file when the disk filled up
    /// or the writer was closed.
    pub(crate) fn write_logs_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for log in &self.logs {
            let line = format!(
                "[{}] [{}] {}\n",
                log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                log.category,
                log.message
            );
            writer.write_all(line.as_bytes())?;
        }
        Ok(())
    }
}

pub type SharedProcessManager = Arc<Mutex<BackgroundProcessManager>>;

// ---------------------------------------------------------------------------
// Tests live in the sibling `manager_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
