//! Regression tests for `FileTreeProcessor`. Direct port of `AppTest.kt`.
//!
//! The Kotlin app's test suite is four tests covering the same scenarios;
//! any change to the filter or sort order in `file_node.rs` must update
//! this file in lockstep.

use fastmd_android_egui::{FileNode, FileTreeProcessor};

/// Helper: build a file node with the given name (markdown by default).
fn file(name: &str) -> FileNode {
    FileNode::new(format!("file-{name}"), name, false)
}

/// Helper: build a directory node with the given children.
fn dir(name: &str, children: Vec<FileNode>) -> FileNode {
    let mut n = FileNode::new(format!("dir-{name}"), name, true);
    n.children = children;
    n
}

#[test]
fn filters_out_non_markdown_files() {
    let root = dir(
        "root",
        vec![
            file("image.png"),
            file("document.txt"),
            file("notes.md"),
        ],
    );

    let processed = FileTreeProcessor::process_tree(root).expect("root should be retained");

    assert_eq!(processed.children.len(), 1);
    assert_eq!(processed.children[0].name, "notes.md");
}

#[test]
fn filters_out_empty_directories() {
    let root = dir(
        "root",
        vec![
            dir(
                "EmptyDir",
                vec![file("image.png")], // will be filtered, making the dir empty
            ),
            dir(
                "ValidDir",
                vec![file("valid.md")],
            ),
        ],
    );

    let processed = FileTreeProcessor::process_tree(root).expect("root should be retained");

    assert_eq!(processed.children.len(), 1);
    assert_eq!(processed.children[0].name, "ValidDir");
}

#[test]
fn returns_none_if_root_is_empty_directory() {
    let root = dir("root", vec![file("image.png")]);

    let processed = FileTreeProcessor::process_tree(root);
    assert!(processed.is_none());
}

#[test]
fn sorts_directories_before_files() {
    let root = dir(
        "root",
        vec![
            file("z_file.md"),
            dir("a_dir", vec![file("doc.md")]),
            file("a_file.md"),
            dir("z_dir", vec![file("doc.md")]),
        ],
    );

    let processed = FileTreeProcessor::process_tree(root).expect("root should be retained");
    assert_eq!(processed.children.len(), 4);

    // Directories first, sorted alphabetically.
    assert_eq!(processed.children[0].name, "a_dir");
    assert_eq!(processed.children[1].name, "z_dir");

    // Then files, sorted alphabetically.
    assert_eq!(processed.children[2].name, "a_file.md");
    assert_eq!(processed.children[3].name, "z_file.md");
}
