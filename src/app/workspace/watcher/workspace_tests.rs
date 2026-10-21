//! Unit tests for the [`Workspace`] projection (sidecar of `workspace.rs`).

use super::*;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::directory::DirectoryEventProducer;
use crate::bus::events::file::FileEventProducer;
use std::time::Duration;

/// One file bus + one directory bus (one bus per event), with a
/// [`Workspace`] subscribed to both — the production wiring shape.
struct TestHarness {
    file_bus: Bus<FileChanged>,
    dir_bus: Bus<DirectoryChanged>,
    file_producer: FileEventProducer,
    dir_producer: DirectoryEventProducer,
    workspace: Workspace,
    /// Tap on the directory bus created before any publish (see `recv_dir`).
    dir_tap: BusReader<DirectoryChanged>,
}

impl TestHarness {
    fn new() -> Self {
        let file_bus: Bus<FileChanged> = Bus::new();
        let dir_bus: Bus<DirectoryChanged> = Bus::new();
        let file_producer = FileEventProducer::new(file_bus.clone());
        let dir_producer = DirectoryEventProducer::new(dir_bus.clone());
        let workspace = Workspace::new(Some(file_bus.subscribe()), Some(dir_bus.subscribe()));
        // A wire-tap reader created **before** any publish: broadcast
        // readers only see events sent after they subscribe, so the tap
        // must exist from the start to observe the producer round trip.
        let dir_tap = dir_bus.subscribe();
        Self {
            file_bus,
            dir_bus,
            file_producer,
            dir_producer,
            workspace,
            dir_tap,
        }
    }

    fn recv_dir(&self) -> DirectoryChanged {
        self.dir_tap
            .recv_timeout(Duration::from_millis(100))
            .expect("directory event published")
    }
}

#[test]
fn test_file_changed_added_populates_files() {
    let mut h = TestHarness::new();

    h.file_bus.publish(FileChanged::discovered(vec![
        PathBuf::from("libs/docs/a.md"),
        PathBuf::from("libs/notes/b.md"),
    ]));

    assert!(h.workspace.process_file_changed());
    assert_eq!(h.workspace.all_files.len(), 2);
    assert!(h.workspace.contains_file(Path::new("libs/docs/a.md")));
    // Parent-directory safety net: the parent of each added file is tracked.
    assert!(h.workspace.contains_dir(Path::new("libs/docs")));
    assert!(h.workspace.contains_dir(Path::new("libs/notes")));
}

#[test]
fn test_file_changed_deleted_removes_files() {
    let mut h = TestHarness::new();
    h.workspace.add_file(PathBuf::from("keep.md"));
    h.workspace.add_file(PathBuf::from("remove.md"));

    h.file_bus
        .publish(FileChanged::removed_one(PathBuf::from("remove.md")));

    assert!(h.workspace.process_file_changed());
    assert_eq!(h.workspace.all_files.len(), 1);
    assert!(h.workspace.contains_file(Path::new("keep.md")));
    assert!(!h.workspace.contains_file(Path::new("remove.md")));
}

#[test]
fn test_add_file_duplicate_returns_false() {
    let mut h = TestHarness::new();
    assert!(h.workspace.add_file(PathBuf::from("a.md")));
    assert!(!h.workspace.add_file(PathBuf::from("a.md")));
    assert_eq!(h.workspace.all_files.len(), 1);
}

#[test]
fn test_add_dir_and_contains_and_remove() {
    let mut h = TestHarness::new();
    assert!(h.workspace.add_dir(PathBuf::from("docs")));
    assert!(!h.workspace.add_dir(PathBuf::from("docs")));
    assert!(h.workspace.contains_dir(Path::new("docs")));
    assert!(!h.workspace.contains_dir(Path::new("other")));
    assert!(h.workspace.remove_dir(Path::new("docs")));
    assert!(!h.workspace.remove_dir(Path::new("docs")));
    assert!(h.workspace.all_dirs.is_empty());
}

#[test]
fn test_remove_file_missing_returns_false() {
    let mut h = TestHarness::new();
    assert!(!h.workspace.remove_file(Path::new("missing.md")));
}

#[test]
fn test_is_workspace_and_sorted_views() {
    let mut h = TestHarness::new();
    h.workspace.add_file(PathBuf::from("b.md"));
    h.workspace.add_file(PathBuf::from("a.md"));
    h.workspace.add_dir(PathBuf::from("z"));
    h.workspace.add_dir(PathBuf::from("a"));
    assert!(h.workspace.is_workspace_file(Path::new("a.md")));
    assert!(!h.workspace.is_workspace_file(Path::new("x.md")));
    assert_eq!(
        h.workspace.files_sorted(),
        vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
    );
    assert_eq!(
        h.workspace.dirs_sorted(),
        vec![PathBuf::from("a"), PathBuf::from("z")]
    );
}

#[test]
fn test_changed_kind_signals_reload_without_state_change() {
    let mut h = TestHarness::new();
    h.workspace.add_file(PathBuf::from("a.md"));
    h.file_bus
        .publish(FileChanged::updated_one(PathBuf::from("a.md")));
    // The Changed kind signals reload; the file set is unchanged.
    assert!(h.workspace.process_file_changed());
    assert_eq!(h.workspace.all_files.len(), 1);
}

#[test]
fn test_added_file_repeated_delivery_is_idempotent() {
    let mut h = TestHarness::new();
    h.file_bus
        .publish(FileChanged::discovered_one(PathBuf::from("a.md")));

    assert!(h.workspace.process_file_changed());
    // A duplicate delivery of the same event (e.g. the message bridge
    // applying the event on top of the projection's own drain) must be a
    // no-op: the membership set, not the delivery count, is the truth.
    h.file_bus
        .publish(FileChanged::discovered_one(PathBuf::from("a.md")));
    assert!(
        !h.workspace.process_file_changed(),
        "no new file => no reload signal"
    );
    assert_eq!(h.workspace.all_files.len(), 1);
}

// ── Directory-event drain ──

#[test]
fn test_directory_added_tracks_directory() {
    let mut h = TestHarness::new();
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/libs"),
        kind: DirectoryChangedKind::Added,
    });
    assert!(h.workspace.process_directory_changed());
    assert!(h.workspace.contains_dir(Path::new("/libs")));
    assert_eq!(h.workspace.all_dirs.len(), 1);
}

#[test]
fn test_directory_added_is_idempotent() {
    let mut h = TestHarness::new();
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/libs"),
        kind: DirectoryChangedKind::Added,
    });

    assert!(h.workspace.process_directory_changed());
    assert!(!h.workspace.process_directory_changed());
    assert_eq!(h.workspace.all_dirs.len(), 1);
}

#[test]
fn test_directory_added_false_return_when_already_known() {
    let mut h = TestHarness::new();
    h.workspace.add_dir(PathBuf::from("/libs"));
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/libs"),
        kind: DirectoryChangedKind::Added,
    });
    assert!(!h.workspace.process_directory_changed());
    assert_eq!(h.workspace.all_dirs.len(), 1);
}

#[test]
fn test_directory_deleted_removes_directory() {
    let mut h = TestHarness::new();
    h.workspace.add_dir(PathBuf::from("/libs"));
    assert_eq!(h.workspace.all_dirs.len(), 1);

    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/libs"),
        kind: DirectoryChangedKind::Deleted,
    });
    assert!(h.workspace.process_directory_changed());
    assert!(!h.workspace.contains_dir(Path::new("/libs")));
    assert_eq!(h.workspace.all_dirs.len(), 0);
}

#[test]
fn test_directory_deleted_cascades_to_descendants() {
    let mut h = TestHarness::new();
    h.workspace.add_dir(PathBuf::from("/a"));
    h.workspace.add_dir(PathBuf::from("/a/b"));
    h.workspace.add_dir(PathBuf::from("/a/b/c"));
    h.workspace.add_dir(PathBuf::from("/d"));
    h.workspace.add_file(PathBuf::from("/a/b/note.md"));
    h.workspace.add_file(PathBuf::from("/d/keep.md"));
    assert_eq!(h.workspace.all_dirs.len(), 4);
    assert_eq!(h.workspace.all_files.len(), 2);

    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/a"),
        kind: DirectoryChangedKind::Deleted,
    });
    assert!(h.workspace.process_directory_changed());
    assert!(!h.workspace.contains_dir(Path::new("/a")));
    assert!(!h.workspace.contains_dir(Path::new("/a/b")));
    assert!(!h.workspace.contains_dir(Path::new("/a/b/c")));
    assert!(h.workspace.contains_dir(Path::new("/d")));
    assert_eq!(h.workspace.all_dirs.len(), 1);
    assert!(!h.workspace.contains_file(Path::new("/a/b/note.md")));
    assert!(h.workspace.contains_file(Path::new("/d/keep.md")));
    assert_eq!(h.workspace.all_files.len(), 1);
}

#[test]
fn test_directory_deleted_unknown_is_noop() {
    let mut h = TestHarness::new();
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/nonexistent"),
        kind: DirectoryChangedKind::Deleted,
    });
    assert!(!h.workspace.process_directory_changed());
}

#[test]
fn test_event_producers_round_trip() {
    // Producers → buses → drains: the full publication path for one
    // Added and one Deleted event per bus.
    let mut h = TestHarness::new();

    // File lane: the file producer publishes `[FileChanged][Added]`; the
    // drain sees the file plus its parent-directory safety net.
    h.file_producer.publish_discovered(Path::new("x.md"));
    assert!(h.workspace.process_file_changed());
    assert!(h.workspace.contains_file(Path::new("x.md")));

    // The file producer publishes `[FileChanged][Deleted]`.
    h.file_producer.publish_removed(Path::new("x.md"));
    assert!(h.workspace.process_file_changed());
    assert!(!h.workspace.contains_file(Path::new("x.md")));

    // Directory lane: the dir producer publishes `[DirectoryChanged]`
    // with the literal kinds on the wire.
    h.dir_producer.publish_discovered(Path::new("/newdir"));
    h.workspace.process_directory_changed();
    assert!(h.workspace.contains_dir(Path::new("/newdir")));

    h.dir_producer.publish_removed(Path::new("/newdir"));
    h.workspace.process_directory_changed();
    assert!(!h.workspace.contains_dir(Path::new("/newdir")));

    // The wire carried the literal kinds in order: Added then Deleted.
    assert!(matches!(
        h.recv_dir(),
        DirectoryChanged {
            kind: DirectoryChangedKind::Added,
            ..
        }
    ));
    assert!(matches!(
        h.recv_dir(),
        DirectoryChanged {
            kind: DirectoryChangedKind::Deleted,
            ..
        }
    ));
}

#[test]
fn test_file_events_do_not_affect_dirs_without_parent_net() {
    let mut h = TestHarness::new();
    h.workspace.add_dir(PathBuf::from("/a"));

    h.file_bus
        .publish(FileChanged::removed_one(PathBuf::from("/a/file.md")));
    h.workspace.process_file_changed();
    h.file_bus
        .publish(FileChanged::updated_one(PathBuf::from("/a/file.md")));
    h.workspace.process_file_changed();
    // File Deleted/Changed events alone must not change the directory set.
    assert_eq!(h.workspace.all_dirs.len(), 1);
    assert!(h.workspace.contains_dir(Path::new("/a")));
}

#[test]
fn test_mixed_drains_handle_events_in_order() {
    let mut h = TestHarness::new();
    let dir_added = DirectoryChanged {
        path: PathBuf::from("/a"),
        kind: DirectoryChangedKind::Added,
    };
    h.dir_bus.publish(dir_added);
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/b"),
        kind: DirectoryChangedKind::Added,
    });
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("/a"),
        kind: DirectoryChangedKind::Deleted,
    });

    assert!(h.workspace.process_directory_changed());
    assert!(!h.workspace.contains_dir(Path::new("/a")));
    assert!(h.workspace.contains_dir(Path::new("/b")));
    assert_eq!(h.workspace.all_dirs.len(), 1);
}

#[test]
fn test_one_bus_per_event_isolation() {
    // One bus per event: publishing a file event must not surface on the
    // directory drain and vice versa — the two transports are distinct.
    let mut h = TestHarness::new();
    h.file_bus
        .publish(FileChanged::discovered_one(PathBuf::from("x.md")));
    h.dir_bus.publish(DirectoryChanged {
        path: PathBuf::from("xdir"),
        kind: DirectoryChangedKind::Added,
    });

    assert!(h.workspace.process_directory_changed());
    assert!(h.workspace.contains_dir(Path::new("xdir")));
    assert!(!h.workspace.contains_file(Path::new("x.md")));

    assert!(h.workspace.process_file_changed());
    assert!(h.workspace.contains_file(Path::new("x.md")));
}
