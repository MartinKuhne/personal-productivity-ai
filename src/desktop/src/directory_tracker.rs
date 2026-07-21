use crate::file_events::{BusReader, FileEvent, FileEventKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Tracks known directories by consuming [`FileEvent`]s from the event bus.
///
/// The tracker responds to two kinds of events:
/// - `DirDiscovered` / `DirRemoved` — explicit directory lifecycle events
///   published by the initial scan and the UI.
/// - `Discovered` (file) — the immediate parent directory is added as a
///   safety net, covering edge cases where a file appears without a
///   preceding `DirDiscovered`.
///
/// The tracker is the single source of truth for "all directories that
/// exist in the content libraries". It replaces the scattered `all_dirs`
/// management that previously lived in `FileEventProcessor`, `app.rs`,
/// and `modals.rs`.
pub struct DirectoryTracker {
    reader: BusReader<FileEvent>,
    dirs: HashSet<PathBuf>,
}

impl DirectoryTracker {
    /// Create a new tracker that reads events from the given bus reader.
    pub fn new(reader: BusReader<FileEvent>) -> Self {
        Self {
            reader,
            dirs: HashSet::new(),
        }
    }

    /// Drain all pending events from the bus and update internal state.
    ///
    /// Returns `true` if the directory set changed.
    pub fn process_events(&mut self) -> bool {
        let mut changed = false;

        while let Ok(event) = self.reader.try_recv() {
            match event.kind {
                FileEventKind::DirDiscovered => {
                    if self.dirs.insert(event.path) {
                        changed = true;
                    }
                }
                FileEventKind::DirRemoved => {
                    if self.remove_dir_and_descendants(&event.path) {
                        changed = true;
                    }
                }
                FileEventKind::Discovered => {
                    // Safety net: ensure parent directory is tracked.
                    if let Some(parent) = event.path.parent() {
                        if self.dirs.insert(parent.to_path_buf()) {
                            changed = true;
                        }
                    }
                }
                FileEventKind::Removed | FileEventKind::Updated => {
                    // No directory state change for file removals or updates.
                }
            }
        }

        changed
    }

    /// Remove a directory and all of its descendants from the set.
    fn remove_dir_and_descendants(&mut self, base: &Path) -> bool {
        let mut changed = false;
        // Collect paths to remove first to avoid borrow issues.
        let to_remove: Vec<PathBuf> = self
            .dirs
            .iter()
            .filter(|p| *p == base || p.starts_with(base))
            .cloned()
            .collect();
        for p in to_remove {
            self.dirs.remove(&p);
            changed = true;
        }
        changed
    }

    /// All known directories, sorted for deterministic display.
    pub fn dirs_sorted(&self) -> Vec<PathBuf> {
        let mut sorted: Vec<PathBuf> = self.dirs.iter().cloned().collect();
        sorted.sort();
        sorted
    }

    /// Check if a path is a known directory.
    pub fn contains(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    /// Number of tracked directories.
    pub fn len(&self) -> usize {
        self.dirs.len()
    }

    /// Check whether the directory set is empty.
    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;
    use std::time::Duration;

    fn make_bus_and_tracker() -> (Bus<FileEvent>, DirectoryTracker) {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let tracker = DirectoryTracker::new(reader);
        (bus, tracker)
    }

    #[test]
    fn test_dir_discovered_adds_directory() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));

        assert!(tracker.process_events());
        assert!(tracker.contains(Path::new("/a")));
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_dir_discovered_idempotent() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));

        assert!(tracker.process_events());
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_dir_removed_removes_directory() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        tracker.process_events();
        assert_eq!(tracker.len(), 1);

        bus.publish(FileEvent::dir_removed(PathBuf::from("/a")));
        assert!(tracker.process_events());
        assert!(!tracker.contains(Path::new("/a")));
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_dir_removed_cascades_to_descendants() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a/b")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a/b/c")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/d")));
        tracker.process_events();
        assert_eq!(tracker.len(), 4);

        bus.publish(FileEvent::dir_removed(PathBuf::from("/a")));
        assert!(tracker.process_events());
        assert!(!tracker.contains(Path::new("/a")));
        assert!(!tracker.contains(Path::new("/a/b")));
        assert!(!tracker.contains(Path::new("/a/b/c")));
        assert!(tracker.contains(Path::new("/d")));
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_dir_removed_unknown_is_noop() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_removed(PathBuf::from("/nonexistent")));
        assert!(!tracker.process_events());
    }

    #[test]
    fn test_file_discovered_adds_parent_directory() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::discovered(PathBuf::from("/a/b/file.md")));

        assert!(tracker.process_events());
        assert!(tracker.contains(Path::new("/a/b")));
        // The grandparent /a should NOT be added (matching current behavior).
        assert!(!tracker.contains(Path::new("/a")));
    }

    #[test]
    fn test_file_discovered_idempotent_for_parent() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::discovered(PathBuf::from("/a/b/file.md")));
        bus.publish(FileEvent::discovered(PathBuf::from("/a/b/other.md")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a/b")));

        assert!(tracker.process_events());
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_dirs_sorted_returns_sorted() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/c")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/b")));
        tracker.process_events();

        let sorted = tracker.dirs_sorted();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0], PathBuf::from("/a"));
        assert_eq!(sorted[1], PathBuf::from("/b"));
        assert_eq!(sorted[2], PathBuf::from("/c"));
    }

    #[test]
    fn test_file_removed_or_updated_does_not_affect_dirs() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        tracker.process_events();

        bus.publish(FileEvent::removed(PathBuf::from("/a/file.md")));
        bus.publish(FileEvent::updated(PathBuf::from("/a/file.md")));
        assert!(!tracker.process_events());
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_initial_state_is_empty() {
        let (_bus, tracker) = make_bus_and_tracker();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_process_events_handles_multiple_events_in_order() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/b")));
        bus.publish(FileEvent::dir_removed(PathBuf::from("/a")));

        assert!(tracker.process_events());
        assert!(!tracker.contains(Path::new("/a")));
        assert!(tracker.contains(Path::new("/b")));
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_dir_removed_is_idempotent() {
        let (bus, mut tracker) = make_bus_and_tracker();
        bus.publish(FileEvent::dir_discovered(PathBuf::from("/a")));
        tracker.process_events();

        bus.publish(FileEvent::dir_removed(PathBuf::from("/a")));
        tracker.process_events();

        bus.publish(FileEvent::dir_removed(PathBuf::from("/a")));
        assert!(!tracker.process_events());
    }
}
