//! The [`Workspace`] projection — the single source of truth for the file
//! and directory state of the content libraries.
//!
//! `Workspace` drains the two file-system event buses — `Bus<FileChanged>`
//! for file events ([REQ-330..334]) and `Bus<DirectoryChanged>` for
//! directory events ([REQ-320..322]) — and maintains the running views
//! every other subsystem reads:
//!
//! | View | Source event | Maintained by |
//! | --- | --- | --- |
//! | [`Workspace::all_files`] | `[FileChanged][Added]` / `[Deleted]` | [`Workspace::process_file_changed`] |
//! | [`Workspace::all_dirs`] | `[DirectoryChanged][Added]` / `[Deleted]`, plus the parent-directory safety net on `[FileChanged][Added]` | both drains |
//!
//! Deletions of a directory cascade to its descendants:
//! [`Self::remove_dir_and_descendants`] removes the subtree from both the
//! ordered lists and membership sets. Directory and file indexes are
//! maintained together in this projection.
//!
//! All items in this module are framework-free and unit-tested without
//! driving the UI; the UI layer treats the type as pure state-holding data.
//! Unit tests live in the sibling `workspace_tests.rs` sidecar.

use crate::bus::core::BusReader;
use crate::bus::events::directory::{DirectoryChanged, DirectoryChangedKind};
use crate::bus::events::file::{FileChanged, FileChangedKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct Workspace {
    /// Catch-up reader for `[FileChanged]` events. `Some` only on paths
    /// where the drain owns the state application (unit tests, legacy
    /// wiring); under the iced bridge the reducer applies each event
    /// through `AppOrchestrator::apply_file_changed` instead, so the
    /// reader stays `None` — a live reader would apply every event twice.
    file_reader: Option<BusReader<FileChanged>>,
    /// Active reader for `[DirectoryChanged]` events. Directory state is
    /// only applied through this drain (or, under the iced bridge,
    /// `AppOrchestrator::apply_directory_changed` for the message-carried
    /// events — the drain stays `Some` as the catch-up backstop).
    dir_reader: Option<BusReader<DirectoryChanged>>,
    /// Accumulated list of all discovered files (enumeration order).
    pub all_files: Vec<PathBuf>,
    /// Set for O(1) membership checks on `all_files`.
    all_files_set: HashSet<PathBuf>,
    /// Accumulated list of all known directories (enumeration order).
    pub all_dirs: Vec<PathBuf>,
    /// Set for O(1) membership checks on `all_dirs`.
    all_dirs_set: HashSet<PathBuf>,
    /// Whether the initial indexing has finished.
    pub indexing_finished: bool,
    /// Whether the `indexing_finished` notification has been handled.
    pub indexing_finished_handled: bool,
}

impl Workspace {
    /// Create a new projection.
    ///
    /// * `file_reader` — `Some` when the projection itself owns the file
    ///   state application; `None` when a bridge/orchestrator applies
    ///   events through its own path (avoids double application).
    /// * `dir_reader` — `Some` to keep the directory state hydrated even
    ///   when directory events are not delivered as messages (legacy
    ///   wiring); `None` when the bridge applies every directory event.
    pub fn new(
        file_reader: Option<BusReader<FileChanged>>,
        dir_reader: Option<BusReader<DirectoryChanged>>,
    ) -> Self {
        Self {
            file_reader,
            dir_reader,
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
    pub fn remove_file(&mut self, path: &Path) -> bool {
        if self.all_files_set.remove(path) {
            self.all_files.retain(|fp| fp.as_path() != path);
            true
        } else {
            false
        }
    }

    /// Remove a directory path (exact match only), returning true if it was
    /// present. Use [`Self::remove_dir_and_descendants`] for the cascade
    /// semantics of a `[DirectoryChanged][Deleted]` event.
    pub fn remove_dir(&mut self, path: &Path) -> bool {
        if self.all_dirs_set.remove(path) {
            self.all_dirs.retain(|dp| dp.as_path() != path);
            true
        } else {
            false
        }
    }

    /// Remove a directory and all descendant directories and files from the
    /// projection (the cascade semantics of a `[DirectoryChanged][Deleted]`
    /// event). Returns true when any workspace state changed.
    pub fn remove_dir_and_descendants(&mut self, base: &Path) -> bool {
        let to_remove: Vec<PathBuf> = self
            .all_dirs_set
            .iter()
            .filter(|p| p.as_path() == base || p.starts_with(base))
            .cloned()
            .collect();
        let mut changed = false;
        for p in to_remove {
            self.all_dirs_set.remove(&p);
            self.all_dirs.retain(|dp| dp.as_path() != p.as_path());
            changed = true;
        }
        let files_to_remove: Vec<PathBuf> = self
            .all_files_set
            .iter()
            .filter(|p| p.starts_with(base))
            .cloned()
            .collect();
        for p in files_to_remove {
            self.remove_file(&p);
            changed = true;
        }
        changed
    }

    /// O(1) membership check for files.
    pub fn contains_file(&self, path: &Path) -> bool {
        self.all_files_set.contains(path)
    }

    /// O(1) membership check for directories.
    pub fn contains_dir(&self, path: &Path) -> bool {
        self.all_dirs_set.contains(path)
    }

    /// Drain all pending `[FileChanged]` events into the projection.
    ///
    /// Returns `true` if any tab needs to be reloaded due to file changes.
    /// The `[FileChanged][Added]` arm also tracks each file's parent
    /// directory — the safety net for producers that emit file creations
    /// without a preceding `[DirectoryChanged][Added]` (e.g. the watcher).
    /// No-op (returns `false`) when no reader was wired — the bridge-owned
    /// application path does not use this drain.
    pub fn process_file_changed(&mut self) -> bool {
        let Some(reader) = self.file_reader.as_mut() else {
            return false;
        };
        // Collect first, apply afterwards: the drain borrows the reader
        // mutably, so the state mutations must wait until the borrow ends.
        let mut backlog: Vec<FileChanged> = Vec::new();
        while let Ok(event) = reader.try_recv() {
            backlog.push(event);
        }

        let mut needs_reload = false;
        for event in backlog {
            match event.kind {
                FileChangedKind::Added => {
                    for entry in event.files {
                        if self.add_file(entry.path.clone()) {
                            needs_reload = true;
                        }
                        // Parent-directory safety net (absorbed tracking):
                        // track the parent even without a preceding
                        // `[DirectoryChanged][Added]` event.
                        if let Some(parent) = entry.path.parent() {
                            let _ = self.add_dir(parent.to_path_buf());
                        }
                    }
                }
                FileChangedKind::Changed => {
                    needs_reload = true;
                }
                FileChangedKind::Deleted => {
                    for entry in event.files {
                        self.remove_file(&entry.path);
                    }
                    needs_reload = true;
                }
            }
        }

        needs_reload
    }

    /// Drain all pending `[DirectoryChanged]` events into the projection.
    ///
    /// Returns `true` if the directory state changed. No-op (returns
    /// `false`) when no reader was wired.
    pub fn process_directory_changed(&mut self) -> bool {
        let Some(reader) = self.dir_reader.as_mut() else {
            return false;
        };
        // Collect first, apply afterwards (see `process_file_changed`).
        let mut backlog: Vec<DirectoryChanged> = Vec::new();
        while let Ok(event) = reader.try_recv() {
            backlog.push(event);
        }

        let mut changed = false;
        for event in backlog {
            match event.kind {
                DirectoryChangedKind::Added => {
                    if self.add_dir(event.path) {
                        changed = true;
                    }
                }
                DirectoryChangedKind::Deleted => {
                    if self.remove_dir_and_descendants(&event.path) {
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    /// Check if the given path is one of the discovered workspace files.
    pub fn is_workspace_file(&self, path: &Path) -> bool {
        self.all_files_set.contains(path)
    }

    /// Get all discovered files (sorted for deterministic display).
    pub fn files_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_files.clone();
        sorted.sort();
        sorted
    }

    /// Get all known directories (sorted).
    pub fn dirs_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_dirs.clone();
        sorted.sort();
        sorted
    }
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
