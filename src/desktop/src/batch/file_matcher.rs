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

        let relative = path
            .strip_prefix(directory)
            .expect("WalkDir entries are always under the root directory");
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
mod tests {
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
}
