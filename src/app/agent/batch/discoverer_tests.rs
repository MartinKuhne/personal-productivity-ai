//! Tests for [`Discoverer`] and its implementations (`agent::batch::discoverer`).

use super::*;

#[test]
fn test_file_matcher_discoverer() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "").unwrap();
    std::fs::write(dir.path().join("b.md"), "").unwrap();
    std::fs::write(dir.path().join("c.txt"), "").unwrap();

    let discoverer = FileMatcherDiscoverer {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
    };
    let files = discoverer.discover().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_directory_discoverer() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub1")).unwrap();
    std::fs::create_dir(dir.path().join("sub2")).unwrap();
    std::fs::write(dir.path().join("file.md"), "").unwrap();

    let discoverer = DirectoryDiscoverer {
        directory: dir.path().to_path_buf(),
    };
    let dirs = discoverer.discover().unwrap();
    assert_eq!(dirs.len(), 2);
}

#[test]
fn test_from_config_file_mode() {
    let config = BatchConfig {
        directory: PathBuf::from("/tmp"),
        pattern: "*.md".to_string(),
        prompt_path: PathBuf::from("/tmp/prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };
    let _discoverer = Discoverer::from_config(&config);
}

#[test]
fn test_from_config_directory_mode() {
    let config = BatchConfig {
        directory: PathBuf::from("/tmp"),
        pattern: String::new(),
        prompt_path: PathBuf::from("/tmp/prompt.md"),
        mode: BatchMode::Directory,
        concurrency: 4,
    };
    let _discoverer = Discoverer::from_config(&config);
}

#[test]
fn test_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let discoverer = FileMatcherDiscoverer {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
    };
    let files = discoverer.discover().unwrap();
    assert!(files.is_empty());
}
