//! Tests for [`Tabs`] (`ui::tabs`).

use super::*;

#[test]
fn test_new() {
    let manager = Tabs::new();
    assert!(manager.loaded_path.is_none());
    assert!(manager.current_yaml.is_none());
    assert_eq!(manager.current_markdown, "");
    assert!(manager.tabs.is_empty());
    assert!(manager.toc.is_empty());
    assert!(manager.scroll_to_header_id.is_none());
    assert!(manager.scroll_to_search.is_none());
}

#[test]
fn test_open_tab() {
    let mut manager = Tabs::new();
    let path = PathBuf::from("test.md");
    manager.open_tab(path.clone());
    assert!(manager.tabs.contains(&path));
}

#[test]
fn test_open_tab_no_duplicates() {
    let mut manager = Tabs::new();
    let path = PathBuf::from("test.md");
    manager.open_tab(path.clone());
    manager.open_tab(path.clone());
    assert_eq!(manager.tabs.len(), 1);
}

#[test]
fn test_close_tab() {
    let mut manager = Tabs::new();
    let path = PathBuf::from("test.md");
    manager.open_tab(path.clone());
    manager.close_tab(&path);
    assert!(!manager.tabs.contains(&path));
}

#[test]
fn test_clear_content() {
    let mut manager = Tabs::new();
    manager.loaded_path = Some(PathBuf::from("test.md"));
    manager.current_markdown = "# Test".to_string();
    manager.clear_content();
    assert!(manager.loaded_path.is_none());
    assert_eq!(manager.current_markdown, "");
}

#[test]
fn test_toc_entry_id_is_stable_string() {
    // The manager itself never touches Id. Verify
    // the contract: ToCEntry.id is a plain String.
    let e = ToCEntry::new("Intro", 1, "Intro");
    assert_eq!(e.id, "Intro");
    assert_eq!(e.title, "Intro");
    assert_eq!(e.level, 1);

    // The second occurrence gets a unique id.
    let dup = ToCEntry::new("Intro", 1, "Intro#1");
    assert_ne!(e.id, dup.id);
}

#[test]
fn test_scroll_to_header_id_is_optional_string() {
    // Confirm the field is an framework-free Option<String>.
    let mut m = Tabs::new();
    assert_eq!(m.scroll_to_header_id, None);
    m.scroll_to_header_id = Some("Intro".to_string());
    assert_eq!(m.scroll_to_header_id.as_deref(), Some("Intro"));
    m.scroll_to_header_id = None;
    assert_eq!(m.scroll_to_header_id, None);
}

#[test]
fn test_close_tab_missing_path_is_noop() {
    // Closing a tab that was never opened must be a silent no-op
    // (no panic, no spurious removal of other tabs).
    let mut manager = Tabs::new();
    let open = PathBuf::from("a.md");
    manager.open_tab(open.clone());
    manager.close_tab(&PathBuf::from("never-opened.md"));
    assert_eq!(manager.tabs.len(), 1);
    assert!(manager.tabs.contains(&open));
}

#[test]
fn test_tab_titles_path_without_file_name_uses_empty() {
    // A path with no file name (e.g. a root "/") falls back to an
    // empty title via `unwrap_or_default`.
    let mut manager = Tabs::new();
    manager.open_tab(PathBuf::from("/"));
    let titles = manager.tab_titles().to_vec();
    assert_eq!(titles.len(), 1);
    assert_eq!(titles[0], "");
}

#[test]
fn test_tab_titles_mixed_and_cached() {
    let mut manager = Tabs::new();
    manager.open_tab(PathBuf::from("/"));
    manager.open_tab(PathBuf::from("dir/notes.md"));
    let titles = manager.tab_titles().to_vec();
    assert_eq!(titles, vec!["".to_string(), "notes.md".to_string()]);

    // A second call returns the cached slice with the same values.
    let again = manager.tab_titles().to_vec();
    assert_eq!(titles, again);
}

#[test]
fn test_clear_content_invalidates_heading_ids_cache() {
    let mut manager = Tabs::new();
    manager.current_markdown = "# Intro\n\n# Intro\n\n# Body".to_string();
    let first = manager.heading_ids().to_vec();
    assert_eq!(first.len(), 3);
    assert_eq!(first[1], "Intro#1");

    // clear_content resets the markdown and invalidates the cache.
    manager.clear_content();
    assert!(manager.heading_ids().is_empty());
}

#[test]
fn test_tab_strip_reuses_arc_until_tabs_change() {
    let mut manager = Tabs::new();
    manager.open_tab(PathBuf::from("a.md"));
    manager.open_tab(PathBuf::from("dir/b.md"));

    let (paths, titles) = manager.tab_strip();
    assert_eq!(paths.as_slice(), &manager.tabs);
    assert_eq!(titles.as_slice(), ["a.md".to_string(), "b.md".to_string()]);

    // A second call returns the *same* Arc allocations (pointer bump).
    let (paths2, titles2) = manager.tab_strip();
    assert!(Arc::ptr_eq(&paths, &paths2));
    assert!(Arc::ptr_eq(&titles, &titles2));

    // Mutating `tabs` directly (bypassing invalidation) rebuilds.
    manager.tabs.push(PathBuf::from("c.md"));
    let (paths3, _) = manager.tab_strip();
    assert!(!Arc::ptr_eq(&paths, &paths3));
    assert_eq!(paths3.len(), 3);
}

/// Regression: a removed-opened-close cycle must re-open the tab. The
/// pre-fix `close_tab_at` mutation left the path in `tabs_set` while
/// dropping it from `tabs`, so `open_tab`'s set guard silently refused
/// to re-push it — "the markdown tab does not open".
#[test]
fn open_after_close_tab_at_reopens() {
    let mut manager = Tabs::new();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    manager.open_tab(a.clone());
    manager.open_tab(b.clone());

    assert_eq!(manager.close_tab_at(0), Some(a.clone()));
    assert!(!manager.tabs.contains(&a));

    // The set must not block the re-open.
    manager.open_tab(a.clone());
    assert!(manager.tabs.contains(&a));
    assert_eq!(manager.tabs.len(), 2);
}

/// Regression: `CloseOtherTabs` must keep the kept tab openable and drop
/// the closed ones from the shadow set.
#[test]
fn keep_only_syncs_set_and_selection_order() {
    let mut manager = Tabs::new();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    let c = PathBuf::from("c.md");
    manager.open_tab(a.clone());
    manager.open_tab(b.clone());
    manager.open_tab(c.clone());

    assert_eq!(manager.keep_only(1), Some(b.clone()));
    assert_eq!(manager.tabs, vec![b.clone()]);
    assert!(manager.tabs_set.contains(&b));

    // The closed paths can be re-opened immediately.
    manager.open_tab(a.clone());
    assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);
}

/// Regression: out-of-range indices are no-ops, not panics.
#[test]
fn keep_only_and_close_tab_at_out_of_range_are_noops() {
    let mut manager = Tabs::new();
    manager.open_tab(PathBuf::from("a.md"));
    assert_eq!(manager.keep_only(5), None);
    assert_eq!(manager.close_tab_at(5), None);
    assert_eq!(manager.tabs.len(), 1);
}

/// Regression: `CloseAllTabs` must clear the shadow set too, otherwise a
/// whole workspace becomes permanently un-openable until restart.
#[test]
fn close_all_and_reopen() {
    let mut manager = Tabs::new();
    let a = PathBuf::from("a.md");
    manager.open_tab(a.clone());
    manager.close_all();
    assert!(manager.tabs.is_empty());
    manager.open_tab(a.clone());
    assert_eq!(manager.tabs, vec![a.clone()]);
    // A second open is still a no-op duplicate guard.
    manager.open_tab(a.clone());
    assert_eq!(manager.tabs.len(), 1);
}

/// Regression: `close_tabs` reports whether anything was closed so the
/// orchestrator's selection retarget only runs on real removals.
#[test]
fn close_tabs_reports_closed_any_and_reopens() {
    let mut manager = Tabs::new();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    manager.open_tab(a.clone());
    manager.open_tab(b.clone());

    // Paths that are not open must not count as closed.
    let never_opened = PathBuf::from("never-opened.md");
    assert!(!manager.close_tabs(&[never_opened, PathBuf::from("/")]));
    assert_eq!(manager.tabs.len(), 2);

    assert!(manager.close_tabs(std::slice::from_ref(&a)));
    assert_eq!(manager.tabs, vec![b.clone()]);
    manager.open_tab(a.clone());
    assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);
}

/// Healing: a manager whose set/vector disagree after a direct mutation
/// still opens (no duplicate) and closes (vector and set in
/// step again).
#[test]
fn open_and_close_heal_set_desync() {
    let mut manager = Tabs::new();
    let a = PathBuf::from("a.md");
    let b = PathBuf::from("b.md");
    manager.open_tab(a.clone());
    manager.open_tab(b.clone());

    // Desync half 1: the set lost `a` but the vector still has it —
    // re-opening must not create a duplicate.
    assert!(manager.tabs_set.remove(&a));
    manager.open_tab(a.clone());
    assert_eq!(
        manager.tabs.iter().position(|p| p == &a),
        Some(0),
        "open must not duplicate"
    );
    assert_eq!(manager.tabs.len(), 2);

    // Desync half 2: closing by index removes the row from *both*
    // bookkeeping sides, so re-opening pushes it again instead of being
    // swallowed by a stale set guard.
    assert_eq!(manager.close_tab_at(0), Some(a.clone()));
    assert!(!manager.tabs_set.contains(&a));
    manager.open_tab(a.clone());
    assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);

    // Closing once removes both bookkeeping sides.
    manager.close_tab(&a);
    assert!(!manager.tabs.contains(&a));
    assert!(!manager.tabs_set.contains(&a));
    assert_eq!(manager.tabs, vec![b.clone()]);
}

#[test]
fn test_heading_ids_arc_reuses_arc_until_content_changes() {
    let mut manager = Tabs::new();
    manager.current_markdown = "# Intro".to_string();
    let first = manager.heading_ids_arc();
    assert_eq!(first.as_slice(), ["Intro".to_string()]);

    let second = manager.heading_ids_arc();
    assert!(
        Arc::ptr_eq(&first, &second),
        "unchanged content must reuse the Arc allocation"
    );

    manager.current_markdown = "# Intro\n\n# Two".to_string();
    let third = manager.heading_ids_arc();
    assert!(!Arc::ptr_eq(&first, &third));
    assert_eq!(third.len(), 2);
}

#[test]
fn test_toc_arc_reuses_arc_until_toc_changes() {
    let mut manager = Tabs::new();
    manager.toc = vec![ToCEntry::new("Intro", 1, "Intro")];
    let first = manager.toc_arc();
    assert_eq!(first.len(), 1);

    let second = manager.toc_arc();
    assert!(
        Arc::ptr_eq(&first, &second),
        "unchanged ToC must reuse the Arc allocation"
    );

    manager.toc.push(ToCEntry::new("Body", 1, "Body"));
    let third = manager.toc_arc();
    assert!(!Arc::ptr_eq(&first, &third));
    assert_eq!(third.len(), 2);
}
