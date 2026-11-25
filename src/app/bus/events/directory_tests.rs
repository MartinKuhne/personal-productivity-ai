//! Tests for [`DirectoryChanged`] and [`DirectoryEventProducer`] (`bus::events::directory`).

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
