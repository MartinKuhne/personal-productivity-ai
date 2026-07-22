//! Drains file events from the bus to maintain running lists of all known files and directories.

use crate::file_events::{BusReader, FileEvent, FileEventKind};
use std::path::PathBuf;

/// Processes file system events from the background indexing and watcher.
///
/// Responsibilities:
/// - Owns the `file_event_reader` (BusReader) for receiving FileEvents
/// - Drains the event bus and updates `all_files` and `all_dirs` collections
/// - Determines if a workspace file has changed (for tab reload triggers)
///
/// This extraction reduces `FastMdApp` complexity and isolates event processing.
pub struct FileEventProcessor {
    /// Reader for file events from the bus
    reader: BusReader<FileEvent>,
    /// Accumulated list of all discovered files (populated during initial scan)
    pub all_files: Vec<PathBuf>,
    /// Accumulated list of all directories (populated during initial scan)
    pub all_dirs: Vec<PathBuf>,
    /// Whether indexing has finished
    pub indexing_finished: bool,
    /// Whether the indexing_finished event has been handled
    pub indexing_finished_handled: bool,
}

impl FileEventProcessor {
    /// Create a new processor with the given bus reader.
    pub fn new(reader: BusReader<FileEvent>) -> Self {
        Self {
            reader,
            all_files: Vec::new(),
            all_dirs: Vec::new(),
            indexing_finished: false,
            indexing_finished_handled: false,
        }
    }

    /// Drain all pending file events from the bus and update internal state.
    ///
    /// Returns `true` if any tab needs to be reloaded due to file changes.
    pub fn process_events(&mut self) -> bool {
        let mut needs_reload = false;

        while let Ok(event) = self.reader.try_recv() {
            match event.kind {
                FileEventKind::Discovered => {
                    for p in &event.paths {
                        if !self.all_files.contains(p) {
                            self.all_files.push(p.clone());
                            needs_reload = true;
                        }
                    }
                }
                FileEventKind::Updated => {
                    // Mark that loaded file may need refresh if it's the active file
                    // The actual reload decision is made by FastMdApp based on loaded_path
                    // We just signal that something changed.
                    needs_reload = true;
                }
                FileEventKind::Removed => {
                    for p in &event.paths {
                        self.all_files.retain(|fp| fp != p);
                    }
                    // Deletion handled by FastMdApp; we just signal change
                    needs_reload = true;
                }
                FileEventKind::DirDiscovered | FileEventKind::DirRemoved => {
                    // Directory events are handled by DirectoryTracker.
                }
            }
        }

        needs_reload
    }

    /// Check if the given path is one of the discovered workspace files.
    pub fn is_workspace_file(&self, path: &PathBuf) -> bool {
        self.all_files.contains(path)
    }

    /// Get all discovered files (sorted for deterministic display).
    pub fn files_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_files.clone();
        sorted.sort();
        sorted
    }

    /// Get all discovered directories (sorted).
    pub fn dirs_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_dirs.clone();
        sorted.sort();
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;

    #[test]
    fn test_processor_receives_discovered() {
        let bus = Bus::new();
        let reader = bus.subscribe();
        let mut processor = FileEventProcessor::new(reader);

        bus.publish(FileEvent::discovered_one(PathBuf::from("a.md")));
        bus.publish(FileEvent::discovered_one(PathBuf::from("b.md")));

        assert!(processor.process_events());
        assert_eq!(processor.all_files.len(), 2);
    }

    #[test]
    fn test_processor_handles_removed() {
        let bus = Bus::new();
        let reader = bus.subscribe();
        let mut processor = FileEventProcessor::new(reader);

        processor.all_files.push(PathBuf::from("keep.md"));
        processor.all_files.push(PathBuf::from("remove.md"));

        bus.publish(FileEvent::removed_one(PathBuf::from("remove.md")));

        assert!(processor.process_events());
        assert_eq!(processor.all_files.len(), 1);
        assert!(processor.all_files.contains(&PathBuf::from("keep.md")));
    }
}
