//! Sidecar unit tests for `ui/app/mod.rs` ([TASK-029b]).
//!
//! The legacy immediate-mode frame-loop tests (update/render passes,
//! legacy-context theme pinning, font-scale compounding) retired with the
//! the legacy `App` impl; the iced reducer carries that coverage in
//! `src/app/ui_iced/update_tests.rs`. What remains here exercises the
//! framework-free data definitions that survived the swap.

use super::*;
use crate::config::AppConfig;

#[test]
fn test_treenode_new() {
    let node = TreeNode::new("Docs".to_string(), PathBuf::from("/docs"), true);
    assert_eq!(node.name, "Docs");
    assert_eq!(node.path, PathBuf::from("/docs"));
    assert!(node.is_dir);
    assert!(node.children.is_empty());
}

#[test]
fn test_treenode_collect_dirs() {
    let mut root = TreeNode::new("root".to_string(), PathBuf::from("/root"), true);
    let mut child = TreeNode::new("child".to_string(), PathBuf::from("/root/child"), true);
    child.children.insert(
        "leaf.md".to_string(),
        TreeNode::new(
            "leaf.md".to_string(),
            PathBuf::from("/root/child/leaf.md"),
            false,
        ),
    );
    root.children.insert("child".to_string(), child);

    let mut dirs = HashSet::new();
    root.collect_dirs(&mut dirs);
    assert!(dirs.contains(&PathBuf::from("/root")));
    assert!(dirs.contains(&PathBuf::from("/root/child")));
    assert!(!dirs.contains(&PathBuf::from("/root/child/leaf.md")));
}

#[test]
fn test_tags_tracks_tags_correctly() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    app.orchestrator.tags.add_tags(
        PathBuf::from("file1.md"),
        vec!["rust".to_string(), "ui".to_string()],
    );
    app.orchestrator.tags.add_tags(
        PathBuf::from("file2.md"),
        vec!["rust".to_string(), "testing".to_string()],
    );

    assert_eq!(app.tags().all_tags().len(), 3);
    assert!(app.tags().all_tags().contains("rust"));
    assert!(app.tags().all_tags().contains("ui"));
    assert!(app.tags().all_tags().contains("testing"));
}

#[test]
fn test_generate_format_prompt_carries_date() {
    let prompt = generate_format_prompt("2026-10-18T00:00:00Z");
    assert!(prompt.contains("2026-10-18T00:00:00Z"));
    assert!(prompt.contains("header-date:"));
    assert!(prompt.contains("title: A brief title"));
}
