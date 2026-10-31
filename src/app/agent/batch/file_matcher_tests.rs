//! Tests for [`find_matching_files`] and [`find_subdirectories`] (`agent::batch::file_matcher`).

use super::*;
use std::fs;

#[test]
fn test_find_matching_files_glob_pattern() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("readme.md"), "").unwrap();
    fs::write(dir.path().join("notes.md"), "").unwrap();
    fs::write(dir.path().join("image.png"), "").unwrap();
    fs::write(dir.path().join("draft.txt"), "").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/other.md"), "").unwrap();

    let files = find_matching_files(dir.path(), "*.md").unwrap();
    assert_eq!(files.len(), 3);
    assert!(files.iter().any(|f| f.ends_with("readme.md")));
    assert!(files.iter().any(|f| f.ends_with("notes.md")));
    assert!(files.iter().any(|f| f.ends_with("other.md")));
}

#[test]
fn test_find_matching_files_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("data.json"), "").unwrap();

    let files = find_matching_files(dir.path(), "*.md").unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_find_matching_files_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let files = find_matching_files(dir.path(), "*.md").unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_find_subdirectories() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub1")).unwrap();
    fs::create_dir(dir.path().join("sub2")).unwrap();
    fs::write(dir.path().join("file.md"), "").unwrap();

    let subdirs = find_subdirectories(dir.path());
    assert_eq!(subdirs.len(), 2);
    assert!(subdirs.iter().any(|d| d.ends_with("sub1")));
    assert!(subdirs.iter().any(|d| d.ends_with("sub2")));
}

#[test]
fn test_find_subdirectories_no_subdirs() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file.md"), "").unwrap();

    let subdirs = find_subdirectories(dir.path());
    assert!(subdirs.is_empty());
}

#[test]
fn test_find_subdirectories_non_existent_dir() {
    let subdirs = find_subdirectories(Path::new("/nonexistent/path"));
    assert!(subdirs.is_empty());
}
