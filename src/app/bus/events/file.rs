//! File-system events — payload types for the `Bus<FileEvent>` channel.
//!
//! These are produced by code that mutates the filesystem (the initial
//! library scan, the `notify`-based watcher, UI handlers, tool
//! implementations, the agent) and consumed by code that needs to react
//! to those changes (the tag manager, directory tree, indexer, etc.).
//!
//! Two events are defined here, matching the specification vocabulary
//! ([REQ-320..334]): a `[DirectoryChanged]` event ([`DirectoryChanged`])
//! carried one directory at a time, and a `[FileChanged]` event
//! ([`FileChanged`]) carrying a `[Files]` list. [`FileEvent`] is the
//! transport union the two share on the bus.
//!
//! The transport itself lives in [`crate::bus::core`]; this module
//! defines only the event payloads and a thin [`FileEventProducer`]
//! convenience for the common publish operations.

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

/// The `Kind` property of a `[FileChanged]` event ([REQ-333]; previously
/// REC-332 in the first revision).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangedKind {
    /// A not-previously-known file was enumerated or created.
    Added,
    /// A known file was deleted.
    Deleted,
    /// A known file changed its content.
    Changed,
}

/// One file inside the `[Files]` list of a `[FileChanged]` event, with its
/// per-file properties ([REQ-333] `Path`, [REQ-334] `FileName`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// [REQ-333] The fully qualified path of the file.
    pub path: PathBuf,
    /// [REQ-334] The file name component of `path`.
    pub file_name: String,
}

impl FileEntry {
    /// Builds an entry deriving `file_name` from `path` ([REQ-334]); a path
    /// with no final component (a filesystem root) yields an empty string.
    pub fn from_path(path: PathBuf) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self { path, file_name }
    }
}

/// A `[FileChanged]` event ([REQ-330..334]): one event-level `Kind`
/// ([REQ-332]) describing every file in the `[Files]` list ([REQ-331]); each
/// file carries its own `Path` ([REQ-333]) and `FileName` ([REQ-334]).
///
/// The design is kind-homogeneous: an observer that observes several changes
/// of different kinds (e.g. a rename's delete+create pair, [REQ-404])
/// publishes separate events per kind rather than mixing kinds in one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChanged {
    /// [REQ-332] The single kind describing every entry in `files`.
    pub kind: FileChangedKind,
    /// [REQ-331] The files this event notifies about.
    pub files: Vec<FileEntry>,
}

impl FileChanged {
    /// Fully qualified paths of the `[Files]` list, aligned by index.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }
}

/// Transport union for the file-system event bus (`Bus<FileEvent>`): every
/// producer publishes one of the two spec events, every consumer matches the
/// variant it manages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// A `[FileChanged]` event ([REQ-330..334]).
    FileChanged(FileChanged),
    /// A `[DirectoryChanged]` event ([REQ-320..322]).
    DirectoryChanged(DirectoryChanged),
}

impl FileEvent {
    /// A `[FileChanged][Added]` batch ([REQ-330..332]: `Kind` = Added).
    pub fn discovered(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self::FileChanged(FileChanged {
            kind: FileChangedKind::Added,
            files,
        })
    }

    pub fn discovered_one(path: PathBuf) -> Self {
        Self::discovered(vec![path])
    }

    /// A `[FileChanged][Changed]` batch ([REQ-332]: `Kind` = Changed).
    pub fn updated(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self::FileChanged(FileChanged {
            kind: FileChangedKind::Changed,
            files,
        })
    }

    pub fn updated_one(path: PathBuf) -> Self {
        Self::updated(vec![path])
    }

    /// A `[FileChanged][Deleted]` batch ([REQ-332]: `Kind` = Deleted).
    pub fn removed(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self::FileChanged(FileChanged {
            kind: FileChangedKind::Deleted,
            files,
        })
    }

    pub fn removed_one(path: PathBuf) -> Self {
        Self::removed(vec![path])
    }

    /// A `[DirectoryChanged][Added]` event ([REQ-320..322]): one directory
    /// per event, `Kind` = Added.
    pub fn dir_discovered_one(path: PathBuf) -> Self {
        Self::DirectoryChanged(DirectoryChanged {
            path,
            kind: DirectoryChangedKind::Added,
        })
    }

    /// A `[DirectoryChanged][Deleted]` event ([REQ-320..322]): one directory
    /// per event, `Kind` = Deleted.
    pub fn dir_removed_one(path: PathBuf) -> Self {
        Self::DirectoryChanged(DirectoryChanged {
            path,
            kind: DirectoryChangedKind::Deleted,
        })
    }

    /// Fully qualified paths of the payload if it is a `[FileChanged]`
    /// event; empty for directory events.
    pub fn file_paths(&self) -> Vec<PathBuf> {
        match self {
            FileEvent::FileChanged(changed) => changed.paths(),
            FileEvent::DirectoryChanged(_) => Vec::new(),
        }
    }
}

/// A thin handle for publishing [`FileEvent`]s from code that mutates
/// the filesystem (UI handlers, tool implementations, the agent, etc.).
///
/// Owns a `Bus<FileEvent>` clone (which is itself `Arc`-backed inside
/// `tokio::sync::broadcast::Sender`), so the producer is `'static`
/// and cheap to construct or clone. The `Clone` impl is what makes
/// the lifetime-free `ToolContext` rewrite work: a producer cloned
/// out of the context still publishes into the same channel.
#[derive(Clone)]
pub struct FileEventProducer {
    bus: Bus<FileEvent>,
}

impl FileEventProducer {
    pub fn new(bus: Bus<FileEvent>) -> Self {
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
    fn test_file_changed_carries_per_file_properties() {
        // [REQ-331]/[REQ-333]/[REQ-334]: one `[Files]` list whose entries
        // each carry Path and FileName; a single event-level Kind.
        let e = FileEvent::discovered(vec![
            PathBuf::from("/libs/docs/notes.md"),
            PathBuf::from("/libs/docs/todo.txt"),
        ]);
        let FileEvent::FileChanged(changed) = &e else {
            panic!("file event must be FileChanged");
        };
        assert_eq!(changed.kind, FileChangedKind::Added);
        assert_eq!(changed.files.len(), 2);
        assert_eq!(changed.files[0].path, PathBuf::from("/libs/docs/notes.md"));
        assert_eq!(changed.files[0].file_name, "notes.md");
        assert_eq!(changed.files[1].path, PathBuf::from("/libs/docs/todo.txt"));
        assert_eq!(changed.files[1].file_name, "todo.txt");
        // Index-aligned helper for consumers that only need paths.
        assert_eq!(
            e.file_paths(),
            vec![
                PathBuf::from("/libs/docs/notes.md"),
                PathBuf::from("/libs/docs/todo.txt")
            ]
        );
    }

    #[test]
    fn test_file_entry_unnamed_path_is_empty_file_name() {
        let entry = FileEntry::from_path(PathBuf::from("C:\\"));
        assert_eq!(entry.file_name, "");
    }

    #[test]
    fn test_file_event_kinds() {
        assert!(
            matches!(
                FileEvent::updated_one(PathBuf::from("a.md")),
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Changed,
                    ..
                })
            ),
            "updated_one builds [FileChanged][Changed]"
        );
        assert!(
            matches!(
                FileEvent::removed_one(PathBuf::from("a.md")),
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Deleted,
                    ..
                })
            ),
            "removed_one builds [FileChanged][Deleted]"
        );
    }

    #[test]
    fn test_dir_changed_kind_values() {
        // [REQ-320]/[REQ-321]/[REQ-322]: one directory per event, fully
        // qualified path, Kind ∈ {Added, Deleted}.
        let added = FileEvent::dir_discovered_one(PathBuf::from("/libs/docs/subdir"));
        assert!(matches!(
            &added,
            FileEvent::DirectoryChanged(DirectoryChanged {
                path,
                kind: DirectoryChangedKind::Added,
            }) if path == &PathBuf::from("/libs/docs/subdir")
        ));
        assert!(
            matches!(
                FileEvent::dir_removed_one(PathBuf::from("/libs/docs/subdir")),
                FileEvent::DirectoryChanged(DirectoryChanged {
                    kind: DirectoryChangedKind::Deleted,
                    ..
                })
            ),
            "dir_removed_one builds [DirectoryChanged][Deleted]"
        );
        // Directory events carry no file entries.
        assert!(added.file_paths().is_empty());
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
        assert!(matches!(
            e1,
            FileEvent::FileChanged(FileChanged {
                kind: FileChangedKind::Added,
                ..
            })
        ));
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            e2,
            FileEvent::FileChanged(FileChanged {
                kind: FileChangedKind::Changed,
                ..
            })
        ));
        let e3 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            e3,
            FileEvent::FileChanged(FileChanged {
                kind: FileChangedKind::Deleted,
                ..
            })
        ));
    }

    #[test]
    fn test_producer_publishes_discovered_for_new_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/new.md");

        producer.publish_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            matches!(
                event,
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Added,
                    ref files,
                }) if files.len() == 1 && files[0].path == path
            ),
            "publish_discovered writes [FileChanged][Added]"
        );
    }

    #[test]
    fn test_producer_publishes_updated_for_existing_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/existing.md");

        producer.publish_updated(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            matches!(
                event,
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Changed,
                    ..
                })
            ),
            "publish_updated writes [FileChanged][Changed]"
        );
    }

    #[test]
    fn test_producer_publishes_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/gone.md");

        producer.publish_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            matches!(
                event,
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Deleted,
                    ..
                })
            ),
            "publish_removed writes [FileChanged][Deleted]"
        );
    }

    #[test]
    fn test_producer_publishes_rename_as_removed_plus_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let old = PathBuf::from("/tmp/old.md");
        let new = PathBuf::from("/tmp/new.md");

        producer.publish_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            matches!(
                e1,
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Deleted,
                    ..
                })
            ),
            "rename publishes the delete-half first"
        );
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            matches!(
                e2,
                FileEvent::FileChanged(FileChanged {
                    kind: FileChangedKind::Added,
                    ..
                })
            ),
            "rename then publishes the create-half"
        );
    }

    #[test]
    fn test_producer_publishes_dir_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/newdir");

        producer.publish_dir_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            event,
            FileEvent::DirectoryChanged(DirectoryChanged {
                kind: DirectoryChangedKind::Added,
                ..
            })
        ));
    }

    #[test]
    fn test_producer_publishes_dir_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let path = PathBuf::from("/tmp/olddir");

        producer.publish_dir_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            event,
            FileEvent::DirectoryChanged(DirectoryChanged {
                kind: DirectoryChangedKind::Deleted,
                ..
            })
        ));
    }

    #[test]
    fn test_producer_publishes_dir_rename() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus.clone());
        let old = PathBuf::from("/tmp/olddir");
        let new = PathBuf::from("/tmp/newdir");

        producer.publish_dir_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            e1,
            FileEvent::DirectoryChanged(DirectoryChanged {
                kind: DirectoryChangedKind::Deleted,
                ..
            })
        ));
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(
            e2,
            FileEvent::DirectoryChanged(DirectoryChanged {
                kind: DirectoryChangedKind::Added,
                ..
            })
        ));
    }
}
