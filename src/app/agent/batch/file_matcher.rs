//! Glob-based file discovery for batch File mode and immediate-subdirectory discovery for batch Directory mode.
//!
//! Unit tests live in the sibling `file_matcher_tests.rs` sidecar.

use glob::Pattern;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Finds all files matching the glob pattern within the directory (recursive).
/// Used for File mode batch processing.
pub fn find_matching_files(directory: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let pat = Pattern::new(pattern).map_err(|e| format!("Invalid glob pattern: {}", e))?;
    let mut matches = Vec::new();

    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = path.strip_prefix(directory).unwrap_or(path);
        let relative_str = relative.to_string_lossy();
        if pat.matches(&relative_str) {
            matches.push(path.to_path_buf());
        }
    }

    matches.sort();
    Ok(matches)
}

/// Finds all immediate subdirectories of the given directory.
/// Used for Directory mode batch processing.
pub fn find_subdirectories(directory: &Path) -> Vec<PathBuf> {
    let mut subdirs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            }
        }
    }

    subdirs.sort();
    subdirs
}

#[cfg(test)]
#[path = "file_matcher_tests.rs"]
mod tests;
