//! Drains file events from the bus to maintain running lists of all known files and directories.

use crate::bus::core::BusReader;
use crate::bus::events::file::{FileEvent, FileEventKind};
use std::collections::HashSet;
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
    /// Set for O(1) membership checks on all_files
    all_files_set: HashSet<PathBuf>,
    /// Accumulated list of all directories (populated during initial scan)
    pub all_dirs: Vec<PathBuf>,
    /// Set for O(1) membership checks on all_dirs
    all_dirs_set: HashSet<PathBuf>,
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
            all_files_set: HashSet::new(),
            all_dirs: Vec::new(),
            all_dirs_set: HashSet::new(),
            indexing_finished: false,
            indexing_finished_handled: false,
        }
    }

    /// Add a file path, returning true if it was newly added.
    pub fn add_file(&mut self, path: PathBuf) -> bool {
        if self.all_files_set.insert(path.clone()) {
            self.all_files.push(path);
            true
        } else {
            false
        }
    }

    /// Add a directory path, returning true if it was newly added.
    pub fn add_dir(&mut self, path: PathBuf) -> bool {
        if self.all_dirs_set.insert(path.clone()) {
            self.all_dirs.push(path);
            true
        } else {
            false
        }
    }

    /// Remove a file path, returning true if it was present.
    pub fn remove_file(&mut self, path: &PathBuf) -> bool {
        if self.all_files_set.remove(path) {
            self.all_files.retain(|fp| fp != path);
            true
        } else {
            false
        }
    }

    /// Remove a directory path, returning true if it was present.
    pub fn remove_dir(&mut self, path: &PathBuf) -> bool {
        if self.all_dirs_set.remove(path) {
            self.all_dirs.retain(|dp| dp != path);
            true
        } else {
            false
        }
    }

    /// O(1) membership check for files.
    pub fn contains_file(&self, path: &PathBuf) -> bool {
        self.all_files_set.contains(path)
    }

    /// O(1) membership check for directories.
    pub fn contains_dir(&self, path: &PathBuf) -> bool {
        self.all_dirs_set.contains(path)
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
                        if self.add_file(p.clone()) {
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
                        self.remove_file(p);
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
        self.all_files_set.contains(path)
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
    use crate::bus::core::Bus;

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

        processor.add_file(PathBuf::from("keep.md"));
        processor.add_file(PathBuf::from("remove.md"));

        bus.publish(FileEvent::removed_one(PathBuf::from("remove.md")));

        assert!(processor.process_events());
        assert_eq!(processor.all_files.len(), 1);
        assert!(processor.contains_file(&PathBuf::from("keep.md")));
    }

    #[test]
    fn test_add_file_duplicate_returns_false() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        assert!(p.add_file(PathBuf::from("a.md")));
        assert!(!p.add_file(PathBuf::from("a.md")));
        assert_eq!(p.all_files.len(), 1);
    }

    #[test]
    fn test_add_dir_and_contains_and_remove() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        assert!(p.add_dir(PathBuf::from("docs")));
        assert!(!p.add_dir(PathBuf::from("docs")));
        assert!(p.contains_dir(&PathBuf::from("docs")));
        assert!(!p.contains_dir(&PathBuf::from("other")));
        assert!(p.remove_dir(&PathBuf::from("docs")));
        assert!(!p.remove_dir(&PathBuf::from("docs")));
        assert!(p.all_dirs.is_empty());
    }

    #[test]
    fn test_remove_file_missing_returns_false() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        assert!(!p.remove_file(&PathBuf::from("missing.md")));
    }

    #[test]
    fn test_is_workspace_and_sorted() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        p.add_file(PathBuf::from("b.md"));
        p.add_file(PathBuf::from("a.md"));
        p.add_dir(PathBuf::from("z"));
        p.add_dir(PathBuf::from("a"));
        assert!(p.is_workspace_file(&PathBuf::from("a.md")));
        assert!(!p.is_workspace_file(&PathBuf::from("x.md")));
        assert_eq!(
            p.files_sorted(),
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
        );
        assert_eq!(
            p.dirs_sorted(),
            vec![PathBuf::from("a"), PathBuf::from("z")]
        );
    }

    #[test]
    fn test_process_events_updated_and_dir_ignored() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        bus.publish(FileEvent {
            kind: FileEventKind::Updated,
            paths: vec![PathBuf::from("x.md")],
        });
        assert!(p.process_events());
        bus.publish(FileEvent {
            kind: FileEventKind::DirDiscovered,
            paths: vec![PathBuf::from("docs")],
        });
        bus.publish(FileEvent {
            kind: FileEventKind::DirRemoved,
            paths: vec![PathBuf::from("docs")],
        });
        // Dir events do not set needs_reload, so false when only those queued
        assert!(!p.process_events());
        // No events -> false
        assert!(!p.process_events());
    }

    #[test]
    fn test_process_discovered_duplicate_no_reload() {
        let bus = Bus::new();
        let mut p = FileEventProcessor::new(bus.subscribe());
        p.add_file(PathBuf::from("a.md"));
        bus.publish(FileEvent::discovered_one(PathBuf::from("a.md")));
        // duplicate add returns false, so needs_reload stays false
        assert!(!p.process_events());
    }
}
