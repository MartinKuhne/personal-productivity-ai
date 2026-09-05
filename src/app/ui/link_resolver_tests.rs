//! Unit tests for `link_resolver`.

use super::*;
use std::path::PathBuf;

#[test]
fn test_in_page_anchor() {
    let action = resolve_link("#overview", None, &[], &[]);
    assert_eq!(
        action,
        LinkAction::ScrollToAnchor {
            anchor: "overview".to_string(),
        }
    );
}

#[test]
fn test_external_http_urls() {
    let cases = [
        "https://example.com",
        "http://rust-lang.org/learn",
        "mailto:alice@example.com",
        "ftp://files.example.com",
    ];
    for url in cases {
        let action = resolve_link(url, None, &[], &[]);
        assert_eq!(action, LinkAction::OpenExternal(url.to_string()));
    }
}

#[test]
fn test_wikilink_resolved_from_workspace_files() {
    let ws_files = vec![
        PathBuf::from("/workspace/Notes/Todo.md"),
        PathBuf::from("/workspace/Personal/Journal-2023.md"),
    ];
    let action = resolve_link("wikilink:Journal-2023", None, &ws_files, &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Personal/Journal-2023.md"),
            anchor: None,
        }
    );
}

#[test]
fn test_wikilink_case_insensitive_match() {
    let ws_files = vec![PathBuf::from("/workspace/Personal/journal-2023.md")];
    let action = resolve_link("wikilink:Journal-2023", None, &ws_files, &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Personal/journal-2023.md"),
            anchor: None,
        }
    );
}

#[test]
fn test_wikilink_with_anchor() {
    let ws_files = vec![PathBuf::from("/workspace/Notes/Goals.md")];
    let action = resolve_link("wikilink:Goals#Q1", None, &ws_files, &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Notes/Goals.md"),
            anchor: Some("Q1".to_string()),
        }
    );
}

#[test]
fn test_relative_file_link_with_current_file() {
    let current = PathBuf::from("/workspace/Docs/Chapter1.md");
    let action = resolve_link("Chapter2.md", Some(&current), &[], &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Docs/Chapter2.md"),
            anchor: None,
        }
    );
}

#[test]
fn test_parent_relative_file_link() {
    let current = PathBuf::from("/workspace/Docs/Sub/Deep.md");
    let action = resolve_link("../Intro.md", Some(&current), &[], &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Docs/Intro.md"),
            anchor: None,
        }
    );
}

#[test]
fn test_relative_file_link_with_anchor() {
    let current = PathBuf::from("/workspace/Docs/Chapter1.md");
    let action = resolve_link("Chapter2.md#summary", Some(&current), &[], &[]);
    assert_eq!(
        action,
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("/workspace/Docs/Chapter2.md"),
            anchor: Some("summary".to_string()),
        }
    );
}

#[test]
fn test_normalize_path_components() {
    let raw = PathBuf::from("/a/b/../c/./d.md");
    let norm = normalize_path(&raw);
    assert_eq!(norm, PathBuf::from("/a/c/d.md"));
}

#[test]
fn test_empty_link_falls_back_to_external() {
    let action = resolve_link("   ", None, &[], &[]);
    assert_eq!(action, LinkAction::OpenExternal(String::new()));
}
