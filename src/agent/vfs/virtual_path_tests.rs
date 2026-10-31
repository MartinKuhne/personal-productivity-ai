//! Tests for [`VirtualPath`] parsing, resolution and writability (`agent::vfs::virtual_path`).

use super::*;

fn make_lib(root: &str, name: &str, readonly: bool) -> ContentLibrary {
    ContentLibrary {
        root_folder: root.to_string(),
        name: name.to_string(),
        kind: "text".to_string(),
        readonly,
        priority: 0,
    }
}

#[test]
fn test_parse_valid() {
    let vp = VirtualPath::parse("TestLib/sub/file.md").unwrap();
    assert_eq!(vp.library, "TestLib");
    assert_eq!(vp.sub_path, PathBuf::from("sub/file.md"));
}

#[test]
fn test_parse_nested_subpath() {
    let vp = VirtualPath::parse("TestLib/a/b/c/file.md").unwrap();
    assert_eq!(vp.library, "TestLib");
    assert_eq!(vp.sub_path, PathBuf::from("a/b/c/file.md"));
}

#[test]
fn test_parse_empty() {
    let err = VirtualPath::parse("").unwrap_err();
    assert_eq!(err, VirtualPathError::EmptyPath);
}

#[test]
fn test_parse_no_separator() {
    let err = VirtualPath::parse("justaname").unwrap_err();
    assert_eq!(
        err,
        VirtualPathError::InvalidFormat(
            "missing '/' separator between library name and sub-path".to_string()
        )
    );
}

#[test]
fn test_parse_empty_library() {
    let err = VirtualPath::parse("/sub/file.md").unwrap_err();
    assert_eq!(
        err,
        VirtualPathError::InvalidFormat("library name is empty".to_string())
    );
}

#[test]
fn test_parse_empty_subpath() {
    let err = VirtualPath::parse("TestLib/").unwrap_err();
    assert_eq!(
        err,
        VirtualPathError::InvalidFormat("sub-path is empty".to_string())
    );
}

#[test]
fn test_parse_traversal_dotdot() {
    let err = VirtualPath::parse("TestLib/../secret").unwrap_err();
    assert_eq!(err, VirtualPathError::TraversalDetected);
}

#[test]
fn test_parse_traversal_dotdot_nested() {
    let err = VirtualPath::parse("TestLib/sub/../../secret").unwrap_err();
    assert_eq!(err, VirtualPathError::TraversalDetected);
}

#[test]
fn test_parse_backslash_normalization() {
    let vp = VirtualPath::parse("TestLib\\sub\\file.md").unwrap();
    assert_eq!(vp.library, "TestLib");
    assert_eq!(vp.sub_path, PathBuf::from("sub/file.md"));
}

#[test]
fn test_resolve_success() {
    let libs = vec![
        make_lib("C:/lib1", "Lib1", false),
        make_lib("C:/lib2", "Lib2", false),
    ];
    let vp = VirtualPath::parse("Lib2/notes/todo.md").unwrap();
    let path = vp.resolve(&libs).unwrap();
    assert_eq!(path, PathBuf::from("C:/lib2/notes/todo.md"));
}

#[test]
fn test_resolve_not_found() {
    let libs = vec![make_lib("C:/lib1", "Lib1", false)];
    let vp = VirtualPath::parse("UnknownLib/file.md").unwrap();
    let err = vp.resolve(&libs).unwrap_err();
    assert_eq!(
        err,
        VirtualPathError::LibraryNotFound("UnknownLib".to_string())
    );
}

#[test]
fn test_is_writable_true() {
    let libs = vec![make_lib("C:/lib1", "Lib1", false)];
    let vp = VirtualPath::parse("Lib1/file.md").unwrap();
    assert!(vp.is_writable(&libs).unwrap());
}

#[test]
fn test_is_writable_false() {
    let libs = vec![make_lib("C:/lib1", "Lib1", true)];
    let vp = VirtualPath::parse("Lib1/file.md").unwrap();
    assert!(!vp.is_writable(&libs).unwrap());
}

#[test]
fn test_display() {
    let vp = VirtualPath {
        library: "MyLib".to_string(),
        sub_path: PathBuf::from("path/to/note.md"),
    };
    assert_eq!(format!("{vp}"), "MyLib/path/to/note.md");
}
