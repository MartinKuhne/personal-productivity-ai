//! Tests for the VFS trait implementations: the in-memory mock resolver and the real filesystem resolver.

use super::*;

#[test]
fn test_mock_virtual_file_system_file_ops() {
    let vfs = MockVirtualFileSystem::new();
    let file_path = PathBuf::from("/notes/todo.md");

    assert!(!vfs.file_exists(&file_path));
    vfs.write(&file_path, b"Initial content").unwrap();
    assert!(vfs.file_exists(&file_path));
    assert!(vfs.dir_exists("/notes"));

    let content = vfs.read_to_string(&file_path).unwrap();
    assert_eq!(content, "Initial content");

    vfs.append(&file_path, b"\nAppended line").unwrap();
    let updated = vfs.read_to_string(&file_path).unwrap();
    assert_eq!(updated, "Initial content\nAppended line");

    let meta = vfs.metadata(&file_path).unwrap();
    assert!(meta.is_file);
    assert!(!meta.is_dir);
    assert_eq!(meta.len, updated.len() as u64);

    vfs.remove_file(&file_path).unwrap();
    assert!(!vfs.file_exists(&file_path));
}

#[test]
fn test_mock_virtual_file_system_dir_ops() {
    let vfs = MockVirtualFileSystem::new();
    let dir_path = PathBuf::from("/system/Skills/Note");
    vfs.create_dir_all(&dir_path).unwrap();

    assert!(vfs.dir_exists("/system"));
    assert!(vfs.dir_exists("/system/Skills"));
    assert!(vfs.dir_exists(&dir_path));

    let file_path = dir_path.join("Skill.md");
    vfs.write(&file_path, b"# Skill").unwrap();

    let entries = vfs.read_dir(&dir_path).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, file_path);
    assert!(entries[0].is_file);
}

#[test]
fn test_mock_virtual_file_system_rename_and_copy() {
    let vfs = MockVirtualFileSystem::new();
    let from = PathBuf::from("/a/doc.md");
    let to_copy = PathBuf::from("/b/doc_copy.md");
    let to_rename = PathBuf::from("/c/doc_renamed.md");

    vfs.write(&from, b"Hello VFS").unwrap();
    vfs.copy(&from, &to_copy).unwrap();
    assert!(vfs.file_exists(&from));
    assert!(vfs.file_exists(&to_copy));

    vfs.rename(&to_copy, &to_rename).unwrap();
    assert!(!vfs.file_exists(&to_copy));
    assert!(vfs.file_exists(&to_rename));
}

#[test]
fn test_mock_rename_does_not_mutate_when_destination_parent_creation_fails() {
    let vfs = MockVirtualFileSystem::new();
    let from = PathBuf::from("/source/doc.md");
    let to = PathBuf::from("/destination/doc.md");
    vfs.write(&from, b"content").unwrap();
    *vfs.create_dir_all_err.lock().unwrap() = Some("permission denied");

    assert!(vfs.rename(&from, &to).is_err());
    assert!(vfs.file_exists(&from));
    assert!(!vfs.file_exists(&to));
}

#[test]
fn test_mock_error_overrides_rename_copy_remove() {
    let vfs = MockVirtualFileSystem::new();
    let a = PathBuf::from("/a.md");
    let b = PathBuf::from("/b.md");
    vfs.write(&a, b"x").unwrap();

    *vfs.rename_err.lock().unwrap() = Some("rename blocked");
    assert!(vfs.rename(&a, &b).is_err());

    *vfs.copy_err.lock().unwrap() = Some("copy blocked");
    assert!(vfs.copy(&a, &b).is_err());

    *vfs.remove_file_err.lock().unwrap() = Some("remove blocked");
    assert!(vfs.remove_file(&a).is_err());

    // clear overrides so the happy path still works
    *vfs.rename_err.lock().unwrap() = None;
    *vfs.copy_err.lock().unwrap() = None;
    *vfs.remove_file_err.lock().unwrap() = None;
    assert!(vfs.rename(&a, &b).is_ok());
}

#[test]
fn test_mock_read_to_string_invalid_utf8() {
    let vfs = MockVirtualFileSystem::new();
    let p = PathBuf::from("/blob.md");
    vfs.add_file(&p, vec![0xff, 0xfe, 0x00, 0x01]);

    let err = vfs.read_to_string(&p).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_mock_not_found_paths() {
    let vfs = MockVirtualFileSystem::new();
    let missing = PathBuf::from("/nope.md");

    assert_eq!(
        vfs.read_to_string(&missing).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        vfs.metadata(&missing).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        vfs.rename(&missing, &PathBuf::from("/x.md"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        vfs.remove_file(&missing).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        vfs.copy(&missing, &PathBuf::from("/x.md"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
}

#[test]
fn test_mock_read_dir_mixed_sorted() {
    let vfs = MockVirtualFileSystem::new();
    vfs.add_file("/dir/f.md", b"x");
    vfs.add_file("/dir/a.md", b"y");
    vfs.add_dir("/dir/sub");

    let entries = vfs.read_dir(Path::new("/dir")).unwrap();
    let paths: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from("/dir/f.md")));
    assert!(paths.contains(&PathBuf::from("/dir/a.md")));
    assert!(paths.contains(&PathBuf::from("/dir/sub")));
    assert_eq!(paths.len(), 3);
    assert!(entries.iter().all(|e| e.is_file || e.is_dir));
}

#[test]
fn test_vfs_resolver_real_io_happy_path() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("notes");
    let resolver = VfsResolver::new(std::sync::Arc::new(crate::config::AgentConfig::default()));
    let file = base.join("a.md");

    resolver.create_dir_all(&base).unwrap();
    resolver.write(&file, b"hello").unwrap();
    assert_eq!(resolver.read_to_string(&file).unwrap(), "hello");

    resolver.append(&file, b" world").unwrap();
    assert_eq!(resolver.read_to_string(&file).unwrap(), "hello world");

    let meta = resolver.metadata(&file).unwrap();
    assert!(meta.is_file);
    assert!(!meta.is_dir);
    assert_eq!(meta.len, "hello world".len() as u64);

    let dir_meta = resolver.metadata(&base).unwrap();
    assert!(dir_meta.is_dir);

    let entries = resolver.read_dir(&base).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_file);
}

#[test]
fn test_vfs_resolver_rename_copy_remove() {
    let dir = tempfile::tempdir().unwrap();
    let resolver = VfsResolver::new(std::sync::Arc::new(crate::config::AgentConfig::default()));
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    let c = dir.path().join("c.md");

    resolver.write(&a, b"content").unwrap();
    resolver.copy(&a, &b).unwrap();
    assert!(b.exists());
    resolver.rename(&b, &c).unwrap();
    assert!(!b.exists());
    assert!(c.exists());
    resolver.remove_file(&c).unwrap();
    assert!(!c.exists());
    // removing a missing file yields an error
    assert!(resolver.remove_file(&c).is_err());
}
