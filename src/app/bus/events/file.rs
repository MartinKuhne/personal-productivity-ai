//! `[FileChanged]` events — payload type of the `Bus<FileChanged>` channel.
//!
//! One event type per bus: this module defines the file-change event
//! ([REQ-330..334]) and the thin [`FileEventProducer`] for publishing file
//! changes into the file bus. Directory lifecycle events publish on the
//! separate `Bus<DirectoryChanged>`; their payload and
//! [`crate::bus::events::directory::DirectoryEventProducer`] live in
//! [`crate::bus::events::directory`].
//!
//! Unit tests live in the sibling `file_tests.rs` sidecar.

use crate::bus::core::Bus;
use std::path::{Component, PathBuf};

/// The `Kind` property of a `[FileChanged]` event ([REQ-332]): one kind per
/// event, describing every file in the `[Files]` list.
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
        let is_root = path.has_root()
            && path
                .components()
                .all(|component| matches!(component, Component::Prefix(_) | Component::RootDir));
        let file_name = if is_root {
            String::new()
        } else {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        Self { path, file_name }
    }
}

/// A `[FileChanged]` event ([REQ-330..334]): one event-level `Kind`
/// ([REQ-332]) describing every file in the `[Files]` list ([REQ-331]); each
/// file carries its own `Path` ([REQ-333]) and `FileName` ([REQ-334]).
///
/// The design is kind-homogeneous: a producer that observes several changes
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

    /// A `[FileChanged][Added]` batch ([REQ-332]: `Kind` = Added).
    pub fn discovered(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self {
            kind: FileChangedKind::Added,
            files,
        }
    }

    /// A single-file `[FileChanged][Added]` event.
    pub fn discovered_one(path: PathBuf) -> Self {
        Self::discovered(vec![path])
    }

    /// A `[FileChanged][Changed]` batch ([REQ-332]: `Kind` = Changed).
    pub fn updated(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self {
            kind: FileChangedKind::Changed,
            files,
        }
    }

    /// A single-file `[FileChanged][Changed]` event.
    pub fn updated_one(path: PathBuf) -> Self {
        Self::updated(vec![path])
    }

    /// A `[FileChanged][Deleted]` batch ([REQ-332]: `Kind` = Deleted).
    pub fn removed(paths: Vec<PathBuf>) -> Self {
        let files = paths.into_iter().map(FileEntry::from_path).collect();
        Self {
            kind: FileChangedKind::Deleted,
            files,
        }
    }

    /// A single-file `[FileChanged][Deleted]` event.
    pub fn removed_one(path: PathBuf) -> Self {
        Self::removed(vec![path])
    }
}

/// A thin handle for publishing [`FileChanged`] events from code that
/// mutates the filesystem (UI handlers, tool implementations, the agent).
///
/// Owns a `Bus<FileChanged>` clone (which is itself `Arc`-backed inside
/// `tokio::sync::broadcast::Sender`), so the producer is `'static` and
/// cheap to construct or clone. The `Clone` impl is what makes the
/// lifetime-free `ToolContext` work: a producer cloned out of the context
/// still publishes into the same channel.
#[derive(Clone)]
pub struct FileEventProducer {
    bus: Bus<FileChanged>,
}

impl FileEventProducer {
    /// Create the producer over the file bus.
    pub fn new(bus: Bus<FileChanged>) -> Self {
        Self { bus }
    }

    /// Publish a `[FileChanged][Added]` event for one file.
    pub fn publish_discovered(&self, path: &std::path::Path) {
        self.bus
            .publish(FileChanged::discovered_one(path.to_path_buf()));
    }

    /// Publish a `[FileChanged][Changed]` event for one file.
    pub fn publish_updated(&self, path: &std::path::Path) {
        self.bus
            .publish(FileChanged::updated_one(path.to_path_buf()));
    }

    /// Publish a `[FileChanged][Deleted]` event for one file.
    pub fn publish_removed(&self, path: &std::path::Path) {
        self.bus
            .publish(FileChanged::removed_one(path.to_path_buf()));
    }

    /// Publish a rename as two kind-homogeneous events ([REQ-404]):
    /// `[FileChanged][Deleted]` for the old name followed by
    /// `[FileChanged][Added]` for the new name.
    pub fn publish_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.publish_removed(old);
        self.publish_discovered(new);
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
