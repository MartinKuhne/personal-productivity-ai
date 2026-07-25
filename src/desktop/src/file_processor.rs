//! Drains file events from the bus to maintain running lists of all known files and directories.

use crate::file_events::{BusReader, FileEvent, FileEventKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Processes file system events from the background indexing and watcher.
///
/// Responsibilities:
/// - Owns the `file_event_reader` (BusReader) for receiving FileEvents
/// - Drains the event bus and updates `all_files` and `all_dirs` collections
/// - Determines if a workspace file has changed (for tab reload triggers)
///
/// This extraction reduces `FastMdApp` complexity and isolates event processing.
pub struct FileEventProcessor {
    /// Reader for file events from the bus
    reader: BusReader<FileEvent>,
    /// Accumulated list of all discovered files (populated during initial scan)
    pub all_files: Vec<PathBuf>,
    /// Set for O(1) membership checks on all_files
    all_files_set: HashSet<PathBuf>,
    /// Accumulated list of all directories (populated during initial scan)
    pub all_dirs: Vec<PathBuf>,
    /// Set for O(1) membership checks on all_dirs
    all_dirs_set: HashSet<PathBuf>,
    /// Whether indexing has finished
    pub indexing_finished: bool,
    /// Whether the indexing_finished event has been handled
    pub indexing_finished_handled: bool,
}

impl FileEventProcessor {
    /// Create a new processor with the given bus reader.
    pub fn new(reader: BusReader<FileEvent>) -> Self {
        Self {
            reader,
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
    pub fn remove_file(&mut self, path: &PathBuf) -> bool {
        if self.all_files_set.remove(path) {
            self.all_files.retain(|fp| fp != path);
            true
        } else {
            false
        }
    }

    /// Remove a directory path, returning true if it was present.
    pub fn remove_dir(&mut self, path: &PathBuf) -> bool {
        if self.all_dirs_set.remove(path) {
            self.all_dirs.retain(|dp| dp != path);
            true
        } else {
            false
        }
    }

    /// O(1) membership check for files.
    pub fn contains_file(&self, path: &PathBuf) -> bool {
        self.all_files_set.contains(path)
    }

    /// O(1) membership check for directories.
    pub fn contains_dir(&self, path: &PathBuf) -> bool {
        self.all_dirs_set.contains(path)
    }

    /// Drain all pending file events from the bus and update internal state.
    ///
    /// Also populates the global [`PDF_BACKED`](crate::utils::markdown::update_pdf_backed)
    /// set so that call-sites can quickly check whether a markdown file is backed by a PDF.
    ///
    /// Returns `true` if any tab needs to be reloaded due to file changes.
    pub fn process_events(&mut self) -> bool {
        let mut needs_reload = false;
        let mut to_add: Vec<PathBuf> = Vec::new();
        let mut to_remove: Vec<PathBuf> = Vec::new();

        while let Ok(event) = self.reader.try_recv() {
            match event.kind {
                FileEventKind::Discovered => {
                    for p in &event.paths {
                        if self.add_file(p.clone()) {
                            needs_reload = true;
                        }
                        Self::update_pdf_backing_for_path(p, &mut to_add, &self.all_files_set);
                    }
                }
                FileEventKind::Updated => {
                    needs_reload = true;
                }
                FileEventKind::Removed => {
                    for p in &event.paths {
                        self.remove_file(p);
                        Self::remove_pdf_backing_for_path(p, &mut to_remove, &self.all_files_set);
                    }
                    needs_reload = true;
                }
                FileEventKind::DirDiscovered | FileEventKind::DirRemoved => {}
            }
        }

        if !to_add.is_empty() || !to_remove.is_empty() {
            crate::utils::markdown::update_pdf_backed(&to_add, &to_remove);
        }

        needs_reload
    }

    /// If `p` is a markdown file whose sibling `.pdf` exists, add it to `to_add`.
    /// If `p` is a PDF whose sibling markdown is in `all_files_set`, add the markdown to `to_add`.
    fn update_pdf_backing_for_path(
        p: &Path,
        to_add: &mut Vec<PathBuf>,
        all_files_set: &HashSet<PathBuf>,
    ) {
        let ext = match p.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return,
        };
        match ext.as_str() {
            "md" | "markdown" => {
                if p.with_extension("pdf").exists() {
                    to_add.push(p.to_path_buf());
                }
            }
            "pdf" => {
                for alt_ext in ["md", "markdown"] {
                    let md_path = p.with_extension(alt_ext);
                    if md_path.exists() && all_files_set.contains(&md_path) {
                        to_add.push(md_path);
                    }
                }
            }
            _ => {}
        }
    }

    /// If `p` is a markdown file, remove it from the PDF-backed set.
    /// If `p` is a PDF whose sibling markdown is in `all_files_set`, remove the markdown.
    fn remove_pdf_backing_for_path(
        p: &Path,
        to_remove: &mut Vec<PathBuf>,
        all_files_set: &HashSet<PathBuf>,
    ) {
        let ext = match p.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return,
        };
        match ext.as_str() {
            "md" | "markdown" => {
                to_remove.push(p.to_path_buf());
            }
            "pdf" => {
                for alt_ext in ["md", "markdown"] {
                    let md_path = p.with_extension(alt_ext);
                    if all_files_set.contains(&md_path) {
                        to_remove.push(md_path);
                    }
                }
            }
            _ => {}
        }
    }

    /// Check if the given path is one of the discovered workspace files.
    pub fn is_workspace_file(&self, path: &PathBuf) -> bool {
        self.all_files_set.contains(path)
    }

    /// Get all discovered files (sorted for deterministic display).
    pub fn files_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_files.clone();
        sorted.sort();
        sorted
    }

    /// Get all discovered directories (sorted).
    pub fn dirs_sorted(&self) -> Vec<PathBuf> {
        let mut sorted = self.all_dirs.clone();
        sorted.sort();
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;

    #[test]
    fn test_processor_receives_discovered() {
        let bus = Bus::new();
        let reader = bus.subscribe();
        let mut processor = FileEventProcessor::new(reader);

        bus.publish(FileEvent::discovered_one(PathBuf::from("a.md")));
        bus.publish(FileEvent::discovered_one(PathBuf::from("b.md")));

        assert!(processor.process_events());
        assert_eq!(processor.all_files.len(), 2);
    }

    #[test]
    fn test_processor_handles_removed() {
        let bus = Bus::new();
        let reader = bus.subscribe();
        let mut processor = FileEventProcessor::new(reader);

        processor.add_file(PathBuf::from("keep.md"));
        processor.add_file(PathBuf::from("remove.md"));

        bus.publish(FileEvent::removed_one(PathBuf::from("remove.md")));

        assert!(processor.process_events());
        assert_eq!(processor.all_files.len(), 1);
        assert!(processor.contains_file(&PathBuf::from("keep.md")));
    }
}
