//! File-system events — payload types for the `Bus<FileEvent>` channel.
//!
//! These are produced by code that mutates the filesystem (the initial
//! library scan, the `notify`-based watcher, UI handlers, tool
//! implementations, the agent) and consumed by code that needs to react
//! to those changes (the tag manager, directory tree, indexer, etc.).
//!
//! The transport itself lives in [`crate::bus::core`]; this module
//! defines only the event payloads and a thin [`FileEventProducer`]
//! convenience for the common publish operations.

use crate::bus::core::Bus;
use std::path::PathBuf;

/// What happened to a file-system entry in one of the configured content
/// libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Discovered,
    Updated,
    Removed,
    DirDiscovered,
    DirRemoved,
}

/// A single file-system event published to the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEvent {
    pub kind: FileEventKind,
    pub paths: Vec<PathBuf>,
}

impl FileEvent {
    pub fn discovered(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Discovered,
            paths,
        }
    }

    pub fn discovered_one(path: PathBuf) -> Self {
        Self::discovered(vec![path])
    }

    pub fn updated(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Updated,
            paths,
        }
    }

    pub fn updated_one(path: PathBuf) -> Self {
        Self::updated(vec![path])
    }

    pub fn removed(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Removed,
            paths,
        }
    }

    pub fn removed_one(path: PathBuf) -> Self {
        Self::removed(vec![path])
    }

    pub fn dir_discovered(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::DirDiscovered,
            paths,
        }
    }

    pub fn dir_discovered_one(path: PathBuf) -> Self {
        Self::dir_discovered(vec![path])
    }

    pub fn dir_removed(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::DirRemoved,
            paths,
        }
    }

    pub fn dir_removed_one(path: PathBuf) -> Self {
        Self::dir_removed(vec![path])
    }
}

/// A thin handle for publishing [`FileEvent`]s from code that mutates
/// the filesystem (UI handlers, tool implementations, the agent, etc.).
pub struct FileEventProducer<'a> {
    bus: &'a Bus<FileEvent>,
}

impl<'a> FileEventProducer<'a> {
    pub fn new(bus: &'a Bus<FileEvent>) -> Self {
        Self { bus }
    }

    pub fn publish_discovered(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::discovered_one(path.to_path_buf()));
    }

    pub fn publish_updated(&self, path: &std::path::Path) {
        self.bus.publish(FileEvent::updated_one(path.to_path_buf()));
    }

    pub fn publish_removed(&self, path: &std::path::Path) {
        self.bus.publish(FileEvent::removed_one(path.to_path_buf()));
    }

    pub fn publish_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.bus.publish(FileEvent::removed_one(old.to_path_buf()));
        self.bus
            .publish(FileEvent::discovered_one(new.to_path_buf()));
    }

    pub fn publish_dir_discovered(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_discovered_one(path.to_path_buf()));
    }

    pub fn publish_dir_removed(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_removed_one(path.to_path_buf()));
    }

    pub fn publish_dir_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_removed_one(old.to_path_buf()));
        self.bus
            .publish(FileEvent::dir_discovered_one(new.to_path_buf()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_file_event_constructors() {
        let p = PathBuf::from("docs/notes.md");
        let d = FileEvent::discovered_one(p.clone());
        assert_eq!(d.kind, FileEventKind::Discovered);
        assert_eq!(d.paths, vec![p]);

        let p = PathBuf::from("docs/notes.md");
        let u = FileEvent::updated_one(p.clone());
        assert_eq!(u.kind, FileEventKind::Updated);

        let p = PathBuf::from("docs/notes.md");
        let r = FileEvent::removed_one(p.clone());
        assert_eq!(r.kind, FileEventKind::Removed);
    }

    #[test]
    fn test_dir_event_constructors() {
        let p = PathBuf::from("docs/subdir");
        let d = FileEvent::dir_discovered_one(p.clone());
        assert_eq!(d.kind, FileEventKind::DirDiscovered);
        assert_eq!(d.paths, vec![p]);

        let p = PathBuf::from("docs/subdir");
        let r = FileEvent::dir_removed_one(p.clone());
        assert_eq!(r.kind, FileEventKind::DirRemoved);
        assert_eq!(r.paths, vec![p]);
    }

    #[test]
    fn test_file_event_bus_delivery() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let path = PathBuf::from("a/b/c.md");
        bus.publish(FileEvent::discovered_one(path.clone()));
        bus.publish(FileEvent::updated_one(path.clone()));
        bus.publish(FileEvent::removed_one(path.clone()));

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::Discovered);
        assert_eq!(e1.paths, vec![path]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::Updated);
        let e3 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e3.kind, FileEventKind::Removed);
    }

    #[test]
    fn test_producer_publishes_discovered_for_new_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/new.md");

        producer.publish_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Discovered);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_updated_for_existing_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/existing.md");

        producer.publish_updated(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Updated);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/gone.md");

        producer.publish_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Removed);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_rename_as_removed_plus_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let old = PathBuf::from("/tmp/old.md");
        let new = PathBuf::from("/tmp/new.md");

        producer.publish_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::Removed);
        assert_eq!(e1.paths, vec![old]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::Discovered);
        assert_eq!(e2.paths, vec![new]);
    }

    #[test]
    fn test_producer_publishes_dir_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/newdir");

        producer.publish_dir_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::DirDiscovered);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_dir_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/olddir");

        producer.publish_dir_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::DirRemoved);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_dir_rename() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let old = PathBuf::from("/tmp/olddir");
        let new = PathBuf::from("/tmp/newdir");

        producer.publish_dir_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::DirRemoved);
        assert_eq!(e1.paths, vec![old]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::DirDiscovered);
        assert_eq!(e2.paths, vec![new]);
    }
}
