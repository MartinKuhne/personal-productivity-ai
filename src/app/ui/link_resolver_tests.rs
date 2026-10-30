//! Unit tests for `link_resolver`.

use super::*;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Create `<dir>/<relative>` (with its parent directories) and return the path.
fn touch(dir: &Path, relative: &str) -> PathBuf {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture directories");
    }
    std::fs::write(&path, "content").expect("create fixture file");
    path
}

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
fn test_wikilink_matches_workspace_files() {
    let cases: Vec<(&str, &str, Vec<PathBuf>, LinkAction)> = vec![
        (
            "stem match",
            "wikilink:Journal-2023",
            vec![
                PathBuf::from("/workspace/Notes/Todo.md"),
                PathBuf::from("/workspace/Personal/Journal-2023.md"),
            ],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Personal/Journal-2023.md"),
                anchor: None,
            },
        ),
        (
            "stem match is case-insensitive",
            "wikilink:Journal-2023",
            vec![PathBuf::from("/workspace/Personal/journal-2023.md")],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Personal/journal-2023.md"),
                anchor: None,
            },
        ),
        (
            "an anchor is carried through the workspace match",
            "wikilink:Goals#Q1",
            vec![PathBuf::from("/workspace/Notes/Goals.md")],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Notes/Goals.md"),
                anchor: Some("Q1".to_string()),
            },
        ),
        (
            "a path-suffix match with the extension",
            "wikilink:Notes/Todo",
            vec![PathBuf::from("/workspace/Notes/Todo.md")],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Notes/Todo.md"),
                anchor: None,
            },
        ),
        (
            "a path-suffix match without the extension",
            "wikilink:Notes/Todo",
            vec![PathBuf::from("/workspace/Notes/Todo")],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Notes/Todo"),
                anchor: None,
            },
        ),
        (
            "surrounding whitespace around target and anchor is trimmed",
            "wikilink:  Goals # Q1 ",
            vec![PathBuf::from("/workspace/Notes/Goals.md")],
            LinkAction::OpenWorkspaceFile {
                path: PathBuf::from("/workspace/Notes/Goals.md"),
                anchor: Some("Q1".to_string()),
            },
        ),
    ];

    for (name, link, ws_files, expected) in cases {
        assert_eq!(
            resolve_link(link, None, &ws_files, &[]),
            expected,
            "[{name}]"
        );
    }
}

#[test]
fn test_relative_file_links_resolve_against_the_current_file() {
    // Real files on disk: this test is about *resolution*, and a resolved link
    // only opens if the target exists (see `defect_01b_...`).
    let dir = tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("Docs").join("Sub"))
        .expect("create fixture directories");
    let chapter1 = touch(dir.path(), "Docs/Chapter1.md");
    let chapter2 = touch(dir.path(), "Docs/Chapter2.md");
    let intro = touch(dir.path(), "Docs/Intro.md");
    let deep = touch(dir.path(), "Docs/Sub/Deep.md");

    let cases: Vec<(&str, &str, PathBuf, LinkAction)> = vec![
        (
            "a sibling file",
            "Chapter2.md",
            chapter1.clone(),
            LinkAction::OpenWorkspaceFile {
                path: normalize_path(&chapter2),
                anchor: None,
            },
        ),
        (
            "a parent-relative file",
            "../Intro.md",
            deep.clone(),
            LinkAction::OpenWorkspaceFile {
                path: normalize_path(&intro),
                anchor: None,
            },
        ),
        (
            "a sibling file with an anchor",
            "Chapter2.md#summary",
            chapter1,
            LinkAction::OpenWorkspaceFile {
                path: normalize_path(&chapter2),
                anchor: Some("summary".to_string()),
            },
        ),
    ];

    for (name, link, current, expected) in cases {
        assert_eq!(
            resolve_link(link, Some(&current), &[], &[]),
            expected,
            "[{name}]"
        );
    }
}

/// **Defect 01b, fixed.** A plain file link whose target is not a real file used
/// to come back as `OpenWorkspaceFile`, so the reducer opened a tab for a
/// phantom path or launched the system editor on a missing file. It now reports
/// [`LinkAction::NotFound`] carrying the best-guess path.
#[test]
fn defect_01b_a_dead_link_reports_not_found_instead_of_a_phantom_file() {
    let dir = tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("Docs")).expect("create fixture directories");
    let current = touch(dir.path(), "Docs/Chapter1.md");
    let missing = normalize_path(&dir.path().join("Docs").join("Chapter2.md"));

    // 1. A missing sibling of the current file.
    assert_eq!(
        resolve_link("Chapter2.md", Some(&current), &[], &[]),
        LinkAction::NotFound {
            path: missing.clone()
        },
        "[missing sibling] the guess is reported, not opened"
    );

    // 2. A missing sibling with an anchor still reports the path, anchor dropped.
    assert_eq!(
        resolve_link("Chapter2.md#summary", Some(&current), &[], &[]),
        LinkAction::NotFound {
            path: missing.clone()
        },
        "[missing sibling with anchor] nothing is opened, so no anchor is queued"
    );

    // 3. An absolute path that does not exist.
    let absent = dir.path().join("Nowhere").join("Gone.md");
    assert_eq!(
        resolve_link(&absent.to_string_lossy(), Some(&current), &[], &[]),
        LinkAction::NotFound {
            path: normalize_path(&absent)
        },
        "[missing absolute path] an absolute link is not exempt from the check"
    );

    // 4. No current file and no content roots: the relative path is reported
    //    unchanged rather than opened.
    assert_eq!(
        resolve_link("Notes/a.md", None, &[], &[]),
        LinkAction::NotFound {
            path: normalize_path(Path::new("Notes/a.md"))
        },
        "[no context] the link is returned as the guess, unopened"
    );

    // 5. The workspace index is a second source of truth: a listed file opens
    //    even when `exists()` cannot confirm it.
    let indexed = dir.path().join("Docs").join("Indexed.md");
    assert_eq!(
        resolve_link(
            "Indexed.md",
            Some(&current),
            &[normalize_path(&indexed)],
            &[]
        ),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&indexed),
            anchor: None,
        },
        "[indexed file] a workspace listing is enough to accept the candidate"
    );

    // 6. A file that exists under a content root still resolves.
    let root = dir.path().join("Root");
    std::fs::create_dir_all(&root).expect("create fixture directories");
    let under_root = touch(dir.path(), "Root/Note.md");
    assert_eq!(
        resolve_link("Note.md", None, &[], &[root]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&under_root),
            anchor: None,
        },
        "[content root] an existing candidate under a root still opens"
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

#[test]
fn test_resolve_link_dispatches_on_the_link_shape() {
    let cases: Vec<(&str, &str, LinkAction)> = vec![
        ("empty link", "", LinkAction::OpenExternal(String::new())),
        (
            "whitespace only",
            "   ",
            LinkAction::OpenExternal(String::new()),
        ),
        (
            "a bare hash is an empty anchor",
            "#",
            LinkAction::ScrollToAnchor {
                anchor: String::new(),
            },
        ),
        (
            "in-page anchor",
            "#overview",
            LinkAction::ScrollToAnchor {
                anchor: "overview".to_string(),
            },
        ),
        (
            "https url",
            "https://example.com",
            LinkAction::OpenExternal("https://example.com".to_string()),
        ),
        (
            "surrounding whitespace is trimmed before dispatch",
            "  mailto:a@example.com  ",
            LinkAction::OpenExternal("mailto:a@example.com".to_string()),
        ),
    ];

    for (name, link, expected) in cases {
        assert_eq!(resolve_link(link, None, &[], &[]), expected, "[{name}]");
    }
}

/// D1a: `is_external_scheme` used to compare the scheme byte-for-byte, so an
/// upper-cased scheme fell through to workspace-path resolution and clicking
/// `HTTPS://example.com` tried to open a file instead of a browser.
///
/// The corner case is a Windows drive path: a single letter followed by `:`
/// is a drive letter, not a scheme, so it must stay a workspace path.
#[test]
fn defect_01a_scheme_detection_is_case_insensitive_but_drive_safe() {
    let cases: Vec<(&str, bool)> = vec![
        ("HTTPS://example.com", true),
        ("Http://example.com", true),
        ("HTTPS://Example.COM/Path", true),
        ("https://example.com", true),
        ("MailTo:alice@example.com", true),
        ("FTP://files.example.com", true),
        // A single letter followed by ':' is a Windows drive letter.
        (r"C:\notes\x.md", false),
        (r"c:\notes\x.md", false),
        ("C:/notes/x.md", false),
        (r"D:\x.md", false),
        // `file:` is deliberately not in the external set: the file-link
        // resolver strips the prefix so local URIs open inside FastMD.
        ("file:///C:/notes/x.md", false),
        ("file://C:/notes/x.md", false),
        // Not schemes at all.
        ("Notes/a.md", false),
        ("https:/example.com", false),
    ];

    for (link, expect_external) in cases {
        let is_external = matches!(
            resolve_link(link, None, &[], &[]),
            LinkAction::OpenExternal(_)
        );
        assert_eq!(
            is_external, expect_external,
            "[{link}] expected external={expect_external}"
        );
    }
}

#[test]
fn test_wikilink_without_a_target() {
    assert_eq!(
        resolve_link("wikilink:", None, &[], &[]),
        LinkAction::OpenExternal(String::new()),
        "an empty wikilink is handed to the external handler as the bare target text"
    );
    assert_eq!(
        resolve_link("wikilink:#Section", None, &[], &[]),
        LinkAction::ScrollToAnchor {
            anchor: "Section".to_string(),
        },
        "an anchor-only wikilink scrolls inside the current document"
    );
}

#[test]
fn test_wikilink_without_any_context_is_external() {
    assert_eq!(
        resolve_link("wikilink:Missing", None, &[], &[]),
        LinkAction::OpenExternal("wikilink:Missing".to_string()),
        "with no workspace, current file or content root there is nothing to resolve against"
    );
}

/// A wikilink target written with Windows separators only matches paths that
/// use the same separators, because the suffix probe embeds the target verbatim.
#[cfg(windows)]
#[test]
fn test_wikilink_backslash_target_matches_a_windows_path() {
    let ws_files = vec![PathBuf::from("C:\\workspace\\Notes\\Todo.md")];
    assert_eq!(
        resolve_link("wikilink:Notes\\Todo", None, &ws_files, &[]),
        LinkAction::OpenWorkspaceFile {
            path: PathBuf::from("C:\\workspace\\Notes\\Todo.md"),
            anchor: None,
        },
        "a backslash target matches the backslash suffix probe"
    );
}

/// On POSIX the backslash probe cannot match a `/`-separated path, so the
/// target stays unresolved.
#[cfg(not(windows))]
#[test]
fn test_wikilink_backslash_target_does_not_match_a_posix_path() {
    let ws_files = vec![PathBuf::from("/workspace/Notes/Todo.md")];
    assert_eq!(
        resolve_link("wikilink:Notes\\Todo", None, &ws_files, &[]),
        LinkAction::OpenExternal("wikilink:Notes\\Todo".to_string()),
        "a backslash target must not match a slash-separated workspace path"
    );
}

#[test]
fn test_wikilink_resolves_against_the_filesystem() {
    let dir = tempdir().expect("tempdir");
    let docs = dir.path().join("Docs");
    std::fs::create_dir_all(&docs).expect("create Docs directory");
    let current = docs.join("Chapter1.md");
    let target = touch(dir.path(), "Docs/Target.md");
    let raw = touch(dir.path(), "Docs/raw.txt");
    let sub = touch(dir.path(), "Sub/Note.md");

    assert_eq!(
        resolve_link("wikilink:raw.txt", Some(&current), &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&raw),
            anchor: None,
        },
        "a sibling target that exists verbatim is used as-is"
    );
    assert_eq!(
        resolve_link("wikilink:Target", Some(&current), &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&target),
            anchor: None,
        },
        "a sibling target is probed with the .md extension"
    );
    assert_eq!(
        resolve_link("wikilink:Target#Section", Some(&current), &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&target),
            anchor: Some("Section".to_string()),
        },
        "the anchor must survive filesystem resolution"
    );
    assert_eq!(
        resolve_link("wikilink:Sub/Note", None, &[], &[dir.path().to_path_buf()]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&sub),
            anchor: None,
        },
        "an unresolved sibling is probed under each content root"
    );
}

#[test]
fn test_wikilink_falls_back_to_a_sibling_path_when_missing() {
    let dir = tempdir().expect("tempdir");
    let current = dir.path().join("Docs").join("Chapter1.md");

    assert_eq!(
        resolve_link("wikilink:New", Some(&current), &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&dir.path().join("Docs").join("New.md")),
            anchor: None,
        },
        "an unresolved wikilink becomes a sibling .md path next to the current file"
    );
}

#[test]
fn test_file_link_absolute_path_is_used_verbatim() {
    let dir = tempdir().expect("tempdir");
    let target = touch(dir.path(), "abs.md");

    assert_eq!(
        resolve_link(target.to_str().expect("utf-8 temp path"), None, &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&target),
            anchor: None,
        },
        "an absolute path is resolved without any context"
    );
}

#[test]
fn test_file_link_file_scheme_prefixes_are_stripped() {
    let dir = tempdir().expect("tempdir");
    let target = touch(dir.path(), "scheme.md");
    let raw = target.to_str().expect("utf-8 temp path");

    for (name, link) in [
        ("two slashes", format!("file://{raw}")),
        ("three slashes", format!("file:///{raw}")),
    ] {
        assert_eq!(
            resolve_link(&link, None, &[], &[]),
            LinkAction::OpenWorkspaceFile {
                path: normalize_path(&target),
                anchor: None,
            },
            "[{name}]"
        );
    }
}

#[test]
fn test_file_link_with_an_empty_path_resolves_to_the_parent_directory() {
    let dir = tempdir().expect("tempdir");
    let docs = dir.path().join("Docs");
    std::fs::create_dir_all(&docs).expect("create Docs directory");
    let current = docs.join("Chapter1.md");

    assert_eq!(
        resolve_link("file://", Some(&current), &[], &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&docs),
            anchor: None,
        },
        "a bare file:// scheme degenerates to the containing directory"
    );
}

#[test]
fn test_file_link_resolves_relative_to_a_content_root() {
    let dir = tempdir().expect("tempdir");
    let target = touch(dir.path(), "Docs/a.md");

    assert_eq!(
        resolve_link("Docs/a.md", None, &[], &[dir.path().to_path_buf()]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&target),
            anchor: None,
        },
        "a relative link is probed under each content root"
    );
}

#[test]
fn test_file_link_keeps_the_candidate_when_the_workspace_is_listed() {
    let dir = tempdir().expect("tempdir");
    let candidate = dir.path().join("Docs").join("Missing.md");
    let current = dir.path().join("Docs").join("Chapter1.md");
    let ws = vec![candidate.clone()];

    assert_eq!(
        resolve_link("Missing.md#x", Some(&current), &ws, &[]),
        LinkAction::OpenWorkspaceFile {
            path: normalize_path(&candidate),
            anchor: Some("x".to_string()),
        },
        "the workspace listing names this file, which is enough to accept the \
         candidate even when exists() cannot confirm it"
    );
}

#[test]
fn test_file_link_without_any_context_stays_relative() {
    assert_eq!(
        resolve_link("Notes/a.md", None, &[], &[]),
        LinkAction::NotFound {
            path: normalize_path(Path::new("Notes/a.md"))
        },
        "with no current file and no content root the link is reported unchanged \
         as a guess, and is never opened"
    );
}

#[test]
fn test_normalize_path_edge_cases() {
    let cases = [
        ("", ""),
        (".", ""),
        ("./a/./b", "a/b"),
        ("a/../b", "b"),
        ("a/..", ""),
        ("..", ""),
        ("a/../..", ""),
        ("/a/../..", "/"),
        ("/..", "/"),
        ("a/b/../../c", "c"),
    ];

    for (input, expected) in cases {
        assert_eq!(
            normalize_path(Path::new(input)),
            PathBuf::from(expected),
            "normalize_path({input:?})"
        );
    }
}
