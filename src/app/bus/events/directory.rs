//! `[DirectoryChanged]` events — payload type of the `Bus<DirectoryChanged>`
//! channel.
//!
//! One event type per bus: this module defines the directory-change event
//! ([REQ-320..322]) and the thin [`DirectoryEventProducer`] for publishing
//! directory changes into the directory bus. File changes ride the separate
//! `Bus<FileChanged>` ([`crate::bus::events::file`]).
//!
//! Unit tests live in the sibling `directory_tests.rs` sidecar.

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
#[path = "directory_tests.rs"]
mod tests;
