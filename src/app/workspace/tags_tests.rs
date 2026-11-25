//! Tests for [`Tags`] (`workspace::tags`).

use super::*;

#[test]
fn test_new_is_empty() {
    let tm = Tags::new();
    assert!(tm.file_tags().is_empty());
    assert!(tm.all_tags().is_empty());
    assert!(tm.prompt_paths().is_empty());
}

#[test]
fn test_add_tags_tracks_tags_and_prompts() {
    let mut tm = Tags::new();
    let p1 = PathBuf::from("doc.md");
    let p2 = PathBuf::from("prompt.md");

    tm.add_tags(p1.clone(), vec!["rust".into(), "ui".into()]);
    tm.add_tags(p2.clone(), vec!["prompt".into()]);

    assert_eq!(tm.file_tags().len(), 2);
    assert_eq!(tm.all_tags().len(), 3);
    assert!(tm.all_tags().contains("rust"));
    assert!(tm.all_tags().contains("ui"));
    assert!(tm.all_tags().contains("prompt"));

    assert!(tm.prompt_paths().contains(&p2));
    assert!(!tm.prompt_paths().contains(&p1));
}

#[test]
fn test_add_tags_prompt_case_insensitive() {
    let mut tm = Tags::new();
    let p = PathBuf::from("p.md");
    tm.add_tags(p.clone(), vec!["PROMPT".into()]);
    assert!(tm.prompt_paths().contains(&p));
}

#[test]
fn test_add_tags_removes_old_prompt_status() {
    let mut tm = Tags::new();
    let p = PathBuf::from("was_prompt.md");

    tm.add_tags(p.clone(), vec!["prompt".into()]);
    assert!(tm.prompt_paths().contains(&p));

    // File no longer has prompt tag
    tm.add_tags(p.clone(), vec!["general".into()]);
    assert!(!tm.prompt_paths().contains(&p));
}

#[test]
fn test_remove_file_removes_from_prompt_paths() {
    let mut tm = Tags::new();
    let p = PathBuf::from("prompt.md");
    tm.add_tags(p.clone(), vec!["prompt".into()]);
    assert!(tm.prompt_paths().contains(&p));

    tm.remove_file(&p);
    assert!(!tm.file_tags().contains_key(&p));
    assert!(!tm.prompt_paths().contains(&p));
}

#[test]
fn test_rebuild_evicts_stale_tags() {
    let mut tm = Tags::new();
    let p1 = PathBuf::from("a.md");
    let p2 = PathBuf::from("b.md");

    tm.add_tags(p1.clone(), vec!["rust".into(), "ui".into()]);
    tm.add_tags(p2.clone(), vec!["rust".into(), "testing".into()]);
    assert_eq!(tm.all_tags().len(), 3);

    // Remove p2 and rebuild — the "testing" tag must be evicted.
    tm.remove_file(&p2);
    tm.rebuild();
    assert_eq!(tm.all_tags().len(), 2);
    assert!(tm.all_tags().contains("rust"));
    assert!(tm.all_tags().contains("ui"));
}

#[test]
fn test_rebuild_rebuilds_prompt_paths() {
    let mut tm = Tags::new();
    let p1 = PathBuf::from("prompt.md");
    let p2 = PathBuf::from("regular.md");

    tm.add_tags(p1.clone(), vec!["prompt".into()]);
    tm.add_tags(p2.clone(), vec!["work".into()]);

    // Simulate: add_tags was called before rebuild was needed.
    // Directly manipulate file_tags to simulate stale state.
    tm.file_tags.insert(p2.clone(), vec!["prompt".into()]);
    tm.rebuild();

    assert!(tm.prompt_paths().contains(&p1));
    assert!(tm.prompt_paths().contains(&p2));
}

#[test]
fn test_add_tags_incremental_does_not_evict_stale() {
    // Match current behavior: add_tags is incremental and does NOT
    // remove tags from all_tags that old tag lists no longer carry.
    // The caller must call rebuild() for eviction.
    let mut tm = Tags::new();
    let p = PathBuf::from("doc.md");

    tm.add_tags(p.clone(), vec!["rust".into(), "ui".into()]);
    assert_eq!(tm.all_tags().len(), 2);

    // Re-add with different tags — old tag "ui" should remain
    // until rebuild() is called.
    tm.add_tags(p.clone(), vec!["rust".into()]);
    assert!(
        tm.all_tags().contains("ui"),
        "incremental add_tags should not evict stale tags"
    );

    tm.rebuild();
    assert!(!tm.all_tags().contains("ui"));
}
