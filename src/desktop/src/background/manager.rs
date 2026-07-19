use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use crate::background::models::{BackgroundLogEntry, LogCategory};
use std::fs::File;
use std::io::Write;

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
            let line = format!("[{}] [{}] {}\n", log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"), log.category, log.message);
            let _ = file.write_all(line.as_bytes());
        }
        Ok(())
    }
}

pub type SharedProcessManager = Arc<Mutex<BackgroundProcessManager>>;
