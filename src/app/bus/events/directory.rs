//! `[DirectoryChanged]` events — payload type of the `Bus<DirectoryChanged>`
//! channel.
//!
//! One event type per bus: this module defines the directory-change event
//! ([REQ-320..322]) and the thin [`DirectoryEventProducer`] for publishing
//! directory changes into the directory bus. File changes ride the separate
//! `Bus<FileChanged>` ([`crate::bus::events::file`]).

use crate::bus::core::Bus;
use std::path::PathBuf;

/// The `Kind` property of a `[DirectoryChanged]` event ([REQ-322]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryChangedKind {
    /// A not-previously-known directory was enumerated or created.
    Added,
    /// A known directory was deleted (consumers cascade to descendants).
    Deleted,
}

/// A `[DirectoryChanged]` event ([REQ-320..322]): one directory per event,
/// with a fully qualified path ([REQ-321]) and an event-level `Kind`
/// ([REQ-322]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryChanged {
    /// [REQ-321] The fully qualified path of the directory.
    pub path: PathBuf,
    /// [REQ-322] Whether the directory was added or deleted.
    pub kind: DirectoryChangedKind,
}

/// A thin handle for publishing [`DirectoryChanged`] events from code that
/// mutates the directory layout (the crawler, the watcher, UI handlers).
///
/// Owns a `Bus<DirectoryChanged>` clone (which is itself `Arc`-backed
/// inside `tokio::sync::broadcast::Sender`), so the producer is `'static`.
#[derive(Clone)]
pub struct DirectoryEventProducer {
    bus: Bus<DirectoryChanged>,
}

impl DirectoryEventProducer {
    pub fn new(bus: Bus<DirectoryChanged>) -> Self {
        Self { bus }
    }

    pub fn publish_discovered(&self, path: &std::path::Path) {
        self.bus.publish(DirectoryChanged {
            path: path.to_path_buf(),
            kind: DirectoryChangedKind::Added,
        });
    }

    pub fn publish_removed(&self, path: &std::path::Path) {
        self.bus.publish(DirectoryChanged {
            path: path.to_path_buf(),
            kind: DirectoryChangedKind::Deleted,
        });
    }

    /// [REQ-404] Applied to the directory layout: a rename surfaces as a
    /// `[DirectoryChanged][Deleted]` event for the old name followed by a
    /// `[DirectoryChanged][Added]` event for the new name.
    pub fn publish_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.publish_removed(old);
        self.publish_discovered(new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::events::file::{FileChanged, FileChangedKind};
    use std::time::Duration;

    #[test]
    fn test_directory_changed_shape() {
        // [REQ-320]/[REQ-321]/[REQ-322]: one directory per event, fully
        // qualified path, Kind ∈ {Added, Deleted}.
        let added = DirectoryChanged {
            path: PathBuf::from("/libs/docs/subdir"),
            kind: DirectoryChangedKind::Added,
        };
        assert_eq!(added.path, PathBuf::from("/libs/docs/subdir"));
        assert_eq!(added.kind, DirectoryChangedKind::Added);
    }

    #[test]
    fn test_producer_publishes_discovered() {
        let bus: Bus<DirectoryChanged> = Bus::new();
        let reader = bus.subscribe();
        let producer = DirectoryEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/newdir");

        producer.publish_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.path, path);
        assert_eq!(event.kind, DirectoryChangedKind::Added);
    }

    #[test]
    fn test_producer_publishes_removed() {
        let bus: Bus<DirectoryChanged> = Bus::new();
        let reader = bus.subscribe();
        let producer = DirectoryEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/olddir");

        producer.publish_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.path, path);
        assert_eq!(event.kind, DirectoryChangedKind::Deleted);
    }

    #[test]
    fn test_producer_publishes_rename_as_deleted_plus_added() {
        let bus: Bus<DirectoryChanged> = Bus::new();
        let reader = bus.subscribe();
        let producer = DirectoryEventProducer::new(bus.clone());
        let old = PathBuf::from("/tmp/olddir");
        let new = PathBuf::from("/tmp/newdir");

        producer.publish_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, DirectoryChangedKind::Deleted);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, DirectoryChangedKind::Added);
    }

    #[test]
    fn test_directory_buses_are_independent() {
        // One bus per event: publishing a directory event must not be
        // visible to any file-bus subscriber and vice versa — the two
        // transports are distinct channel instances.
        let file_bus: Bus<FileChanged> = Bus::new();
        let dir_bus: Bus<DirectoryChanged> = Bus::new();
        let file_reader = file_bus.subscribe();
        let dir_reader = dir_bus.subscribe();

        dir_bus.publish(DirectoryChanged {
            path: PathBuf::from("/tmp/dir"),
            kind: DirectoryChangedKind::Added,
        });
        file_bus.publish(FileChanged::discovered_one(PathBuf::from("/tmp/a.md")));

        assert!(matches!(
            file_reader.recv_timeout(Duration::from_millis(100)),
            Ok(FileChanged {
                kind: FileChangedKind::Added,
                ..
            })
        ));
        assert!(matches!(
            dir_reader.recv_timeout(Duration::from_millis(100)),
            Ok(DirectoryChanged {
                kind: DirectoryChangedKind::Added,
                ..
            })
        ));
        // Each bus delivered exactly its own event.
        assert!(file_reader.try_recv().is_err());
        assert!(dir_reader.try_recv().is_err());
    }
}
