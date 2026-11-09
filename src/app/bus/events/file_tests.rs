//! Unit tests for `[FileChanged]` payloads and the file producer (sidecar
//! of `file.rs`).

use super::*;
use std::time::Duration;

#[test]
fn test_file_changed_carries_per_file_properties() {
    // [REQ-331]/[REQ-333]/[REQ-334]: one `[Files]` list whose entries
    // each carry Path and FileName; a single event-level Kind.
    let e = FileChanged::discovered(vec![
        PathBuf::from("/libs/docs/notes.md"),
        PathBuf::from("/libs/docs/todo.txt"),
    ]);
    assert_eq!(e.kind, FileChangedKind::Added);
    assert_eq!(e.files.len(), 2);
    assert_eq!(e.files[0].path, PathBuf::from("/libs/docs/notes.md"));
    assert_eq!(e.files[0].file_name, "notes.md");
    assert_eq!(e.files[1].path, PathBuf::from("/libs/docs/todo.txt"));
    assert_eq!(e.files[1].file_name, "todo.txt");
    // Index-aligned helper for consumers that only need paths.
    assert_eq!(
        e.paths(),
        vec![
            PathBuf::from("/libs/docs/notes.md"),
            PathBuf::from("/libs/docs/todo.txt")
        ]
    );
}

/// A filesystem root has no final component, so its `FileName` ([REQ-334]) is
/// empty. `/` is a root on every platform and so is the empty path, which is what
/// makes this assertion portable; a Windows drive root is covered by the
/// platform-specific sibling below.
#[test]
fn test_file_entry_unnamed_path_is_empty_file_name() {
    let root = FileEntry::from_path(PathBuf::from("/"));
    assert_eq!(root.file_name, "", "a root path has no file name component");

    let empty = FileEntry::from_path(PathBuf::new());
    assert_eq!(
        empty.file_name, "",
        "an empty path has no file name component"
    );
}

/// A drive root is a prefix plus a root dir, so `from_path`'s root rule names it
/// unnamed - but only where drive prefixes and `\` separators mean what they mean.
#[cfg(windows)]
#[test]
fn test_file_entry_windows_drive_root_is_empty_file_name() {
    let entry = FileEntry::from_path(PathBuf::from("C:\\"));
    assert_eq!(
        entry.file_name, "",
        "a drive root has no file name component"
    );
}

/// Off Windows the same text is an ordinary single-component file name literally
/// named `C:\`, so the entry keeps it. Pinned rather than skipped: `from_path`
/// defers to `std::path`, and a backslash is a legal POSIX file-name character,
/// so normalising it away would truncate real POSIX file names.
#[cfg(not(windows))]
#[test]
fn test_file_entry_keeps_a_windows_style_name_off_windows() {
    let entry = FileEntry::from_path(PathBuf::from("C:\\"));
    assert_eq!(
        entry.file_name, "C:\\",
        "off Windows `C:\\` is one ordinary file name, not a root"
    );
}

#[test]
fn test_file_changed_kinds() {
    assert_eq!(
        FileChanged::updated_one(PathBuf::from("a.md")).kind,
        FileChangedKind::Changed
    );
    assert_eq!(
        FileChanged::removed_one(PathBuf::from("a.md")).kind,
        FileChangedKind::Deleted
    );
}

#[test]
fn test_file_event_bus_delivery() {
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let path = PathBuf::from("a/b/c.md");
    bus.publish(FileChanged::discovered_one(path.clone()));
    bus.publish(FileChanged::updated_one(path.clone()));
    bus.publish(FileChanged::removed_one(path.clone()));

    let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(e1.kind, FileChangedKind::Added);
    let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(e2.kind, FileChangedKind::Changed);
    let e3 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(e3.kind, FileChangedKind::Deleted);
}

#[test]
fn test_producer_publishes_discovered_for_new_file() {
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let producer = FileEventProducer::new(bus.clone());
    let path = PathBuf::from("/tmp/new.md");

    producer.publish_discovered(&path);

    let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(event.kind, FileChangedKind::Added);
    assert_eq!(event.files.len(), 1);
    assert_eq!(event.files[0].path, path);
}

#[test]
fn test_producer_publishes_updated_for_existing_file() {
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let producer = FileEventProducer::new(bus.clone());
    let path = PathBuf::from("/tmp/existing.md");

    producer.publish_updated(&path);

    let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(event.kind, FileChangedKind::Changed);
}

#[test]
fn test_producer_publishes_removed() {
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let producer = FileEventProducer::new(bus.clone());
    let path = PathBuf::from("/tmp/gone.md");

    producer.publish_removed(&path);

    let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(event.kind, FileChangedKind::Deleted);
}

#[test]
fn test_producer_publishes_rename_as_deleted_plus_added() {
    // [REQ-404]: a rename is treated as delete followed by create.
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let producer = FileEventProducer::new(bus.clone());
    let old = PathBuf::from("/tmp/old.md");
    let new = PathBuf::from("/tmp/new.md");

    producer.publish_rename(&old, &new);

    let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(e1.kind, FileChangedKind::Deleted);
    let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(e2.kind, FileChangedKind::Added);
}
