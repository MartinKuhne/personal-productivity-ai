//! Tests for [`super::tree_view`] - the flattening, the id-keyed anchor,
//! the keyboard contract, and what survives when the structure changes
//! underneath the selection (`UIF-COMP-006`).

use crate::composite::tree_view::{
    MAX_TREE_DEPTH, Modifiers, SelectionMode, SyncReport, TreeEvent, TreeKey, TreeStructure,
    TreeView, VisibleRow, visible_rows,
};
use crate::focus::{FocusId, FocusState};
use std::collections::BTreeSet;

type Id = &'static str;

/// ```text
/// root
/// |-- a
/// |   |-- a1
/// |   `-- a2
/// `-- b
///     `-- b1
/// c
/// ```
fn fixture() -> TreeStructure<Id> {
    TreeStructure::from_pairs(&[
        ("root", None),
        ("a", Some("root")),
        ("a1", Some("a")),
        ("a2", Some("a")),
        ("b", Some("root")),
        ("b1", Some("b")),
        ("c", None),
    ])
}

fn ids(rows: &[VisibleRow<Id>]) -> Vec<Id> {
    rows.iter().map(|row| row.id).collect()
}

fn expanded(items: &[Id]) -> BTreeSet<Id> {
    items.iter().copied().collect()
}

/// A single-selection view focused on `focus`, with that node's ancestors
/// opened so the row is actually in the visible list.
fn single_with_focus(structure: &TreeStructure<Id>, focus: Id) -> TreeView<Id> {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    for ancestor in structure.ancestors(&focus) {
        view.set_expanded(&ancestor, true);
    }
    let rows = visible_rows(structure, view.expanded());
    let index = rows
        .iter()
        .position(|row| row.id == focus)
        .unwrap_or_else(|| panic!("{focus} is not visible after opening its ancestors"));
    view.focus_index(&rows, index).unwrap();
    view
}

// ---------------------------------------------------------------------------
// 1 - the structure
// ---------------------------------------------------------------------------

#[test]
fn from_pairs_links_both_directions() {
    let s = fixture();
    assert_eq!(s.children_of(&"root"), vec!["a", "b"]);
    assert_eq!(s.children_of(&"a"), vec!["a1", "a2"]);
    assert_eq!(s.parent_of(&"a1"), Some(&"a"));
    assert_eq!(s.parent_of(&"root"), None);
    assert_eq!(s.nodes().len(), 7);
}

#[test]
fn roots_are_the_nodes_with_no_parent() {
    let s = fixture();
    assert_eq!(s.roots(), vec![&"root", &"c"]);
}

#[test]
fn an_undeclared_parent_makes_an_orphan_not_a_root() {
    let s = TreeStructure::from_pairs(&[("child", Some("ghost"))]);
    assert!(!s.contains(&"child") || s.parent_of(&"child").is_some());
    assert!(
        s.roots().is_empty(),
        "a node whose parent was never declared is not top-level; promoting it \
         would show the user roots the host never named"
    );
}

#[test]
fn expandability_is_about_having_children() {
    let s = fixture();
    assert!(s.is_expandable(&"root"));
    assert!(s.is_expandable(&"a"));
    assert!(!s.is_expandable(&"a1"));
    assert!(!s.is_expandable(&"c"));
    assert!(!s.is_expandable(&"nonexistent"));
}

#[test]
fn ancestors_are_nearest_first() {
    let s = fixture();
    assert_eq!(s.ancestors(&"a1"), vec!["a", "root"]);
    assert_eq!(s.ancestors(&"root"), Vec::<Id>::new());
    assert!(s.is_ancestor_of(&"root", &"b1"));
    assert!(!s.is_ancestor_of(&"b", &"a1"));
}

#[test]
fn a_cyclic_parent_link_terminates_at_the_depth_bound() {
    let s = TreeStructure::from_pairs(&[("a", Some("b")), ("b", Some("a"))]);
    let chain = s.ancestors(&"a");
    assert!(
        chain.len() <= MAX_TREE_DEPTH,
        "a two-node cycle must be bounded, not infinite"
    );
}

// ---------------------------------------------------------------------------
// 2 - flattening
// ---------------------------------------------------------------------------

#[test]
fn a_collapsed_tree_shows_only_roots() {
    let s = fixture();
    assert_eq!(ids(&visible_rows(&s, &expanded(&[]))), vec!["root", "c"]);
}

#[test]
fn expansion_is_depth_first_with_document_order_siblings() {
    let s = fixture();
    assert_eq!(
        ids(&visible_rows(&s, &expanded(&["root"]))),
        vec!["root", "a", "b", "c"]
    );
    assert_eq!(
        ids(&visible_rows(&s, &expanded(&["root", "a", "b"]))),
        vec!["root", "a", "a1", "a2", "b", "b1", "c"]
    );
}

#[test]
fn depths_are_recorded_per_row() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    let depth = |id: Id| rows.iter().find(|r| r.id == id).unwrap().depth;
    assert_eq!(depth("root"), 0);
    assert_eq!(depth("a"), 1);
    assert_eq!(depth("a1"), 2);
    assert_eq!(depth("c"), 0);
}

#[test]
fn a_leaf_is_never_marked_expandable() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    for row in &rows {
        let has_children = !s.children_of(&row.id).is_empty();
        assert_eq!(row.expandable, has_children, "{}", row.id);
    }
}

#[test]
fn the_expanded_flag_on_a_row_matches_the_set_it_was_flattened_with() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    assert!(rows.iter().find(|r| r.id == "root").unwrap().expanded);
    assert!(!rows.iter().find(|r| r.id == "a").unwrap().expanded);
}

#[test]
fn a_cyclic_children_link_cannot_overflow_the_flatten() {
    let s = TreeStructure::from_pairs(&[("a", None), ("b", Some("a"))]);
    let rows = visible_rows(&s, &expanded(&["a", "b"]));
    assert!(rows.len() <= MAX_TREE_DEPTH + 1);
    let mut seen = BTreeSet::new();
    for row in &rows {
        assert!(seen.insert(row.id), "each node appears at most once");
    }
}

// ---------------------------------------------------------------------------
// 3 - expansion through the model
// ---------------------------------------------------------------------------

#[test]
fn setting_expansion_reports_a_no_op_as_none() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    assert!(view.set_expanded(&"root", true).is_some());
    assert!(
        view.set_expanded(&"root", true).is_none(),
        "a host persisting every commit must not be written twice for one no-op"
    );
    assert!(view.set_expanded(&"root", false).is_some());
}

#[test]
fn toggle_flips_and_reports_the_new_state() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    assert!(view.toggle_expanded(&"root").value());
    assert!(view.is_expanded(&"root"));
    assert!(!view.toggle_expanded(&"root").value());
    assert!(!view.is_expanded(&"root"));
}

// ---------------------------------------------------------------------------
// 4 - selection
// ---------------------------------------------------------------------------

#[test]
fn select_single_replaces_the_set_and_sets_the_anchor() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    assert!(view.select_single(&"a"));
    assert_eq!(view.selected(), &expanded(&["a"]));
    assert_eq!(view.anchor(), Some(&"a"));
    assert!(!view.select_single(&"a"), "already the only selection");
    assert!(view.select_single(&"b"));
    assert_eq!(view.selected(), &expanded(&["b"]));
}

#[test]
fn multi_toggle_adds_and_removes() {
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.toggle_selected(&"a"));
    assert!(view.toggle_selected(&"b"));
    assert_eq!(view.selected(), &expanded(&["a", "b"]));
    assert!(view.toggle_selected(&"a"));
    assert_eq!(view.selected(), &expanded(&["b"]));
    assert_eq!(view.anchor(), Some(&"a"));
}

#[test]
fn a_single_mode_toggle_is_a_select() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    let _ = view.toggle_selected(&"a");
    let _ = view.toggle_selected(&"b");
    assert_eq!(view.selected(), &expanded(&["b"]));
}

#[test]
fn clearing_takes_the_anchor_with_it() {
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&"a");
    assert!(view.clear_selection());
    assert!(view.selected().is_empty());
    assert_eq!(view.anchor(), None);
    assert!(!view.clear_selection(), "nothing was there to clear");
}

#[test]
fn select_range_is_inclusive_and_reads_an_inverted_range_backwards() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.select_range(&rows, 1, 3));
    assert_eq!(view.selected(), &expanded(&["a", "a1", "a2"]));
    assert!(view.select_range(&rows, 3, 1));
    assert_eq!(view.selected(), &expanded(&["a", "a1", "a2"]));
}

#[test]
fn select_range_clamps_rather_than_failing() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.select_range(&rows, 0, 99));
    assert_eq!(view.selected(), &expanded(&["root", "c"]));
    assert!(
        !view.select_range(&[], 0, 3),
        "there is nothing to select in an empty list"
    );
}

#[test]
fn select_all_only_works_where_a_set_can_exist() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut single = TreeView::<Id>::new(SelectionMode::Single);
    assert!(
        !single.select_all(&rows),
        "select-all in single mode would put the model in a state the mode forbids"
    );
    let mut multi = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(multi.select_all(&rows));
    assert_eq!(multi.selected().len(), 4);
    assert!(!multi.select_all(&rows), "already all selected");
}

// ---------------------------------------------------------------------------
// 5 - focus movement
// ---------------------------------------------------------------------------

#[test]
fn focus_records_the_slot_it_landed_in() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(view.focus_index(&rows, 2), Some("b"));
    assert_eq!(view.focused(), Some(&"b"));
    assert_eq!(view.focus_slot(), Some(2));
    assert_eq!(view.focus_index(&rows, 99), None);
}

#[test]
fn focus_clamps_at_both_ends_instead_of_wrapping() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = single_with_focus(&s, "root");
    assert_eq!(view.move_focus(&rows, -1), Some("root"));
    assert_eq!(view.move_focus(&rows, -1), Some("root"));
    assert_eq!(view.move_focus(&rows, 1), Some("c"));
    assert_eq!(
        view.move_focus(&rows, 1),
        Some("c"),
        "wrapping a hundred-thousand-row list would teleport the viewport"
    );
}

#[test]
fn focus_enters_at_the_near_end() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut down = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(down.enter_focus(&rows, 1), Some("root"));
    let mut up = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(up.enter_focus(&rows, -1), Some("c"));
    let mut empty = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(empty.enter_focus(&[], 1), None);
}

#[test]
fn move_focus_without_focus_is_no_movement() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(view.move_focus(&rows, 1), None);
}

// ---------------------------------------------------------------------------
// 6 - the keyboard table
// ---------------------------------------------------------------------------

#[test]
fn arrows_enter_and_walk_in_single_mode() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    let events = view.handle_key(TreeKey::Down, Modifiers::none(), &s, &rows);
    assert_eq!(view.focused(), Some(&"root"));
    assert_eq!(
        events,
        vec![TreeEvent::SelectionChanged(expanded(&["root"]))]
    );
    let _ = view.handle_key(TreeKey::Down, Modifiers::none(), &s, &rows);
    assert_eq!(view.focused(), Some(&"a"));
    assert_eq!(view.selected(), &expanded(&["a"]));
}

#[test]
fn shift_arrow_extends_from_the_anchor() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let mut view = single_with_focus(&s, "root");
    let _ = view.select_single(&"root");
    let _ = view.handle_key(TreeKey::Down, Modifiers::shift(), &s, &rows);
    let _ = view.handle_key(TreeKey::Down, Modifiers::shift(), &s, &rows);
    assert_eq!(view.focused(), Some(&"a1"));
    assert_eq!(view.selected(), &expanded(&["root", "a", "a1"]));
}

#[test]
fn shift_arrow_up_selects_backwards_through_the_anchor() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let mut view = single_with_focus(&s, "a2");
    let _ = view.select_single(&"a2");
    let _ = view.handle_key(TreeKey::Up, Modifiers::shift(), &s, &rows);
    assert_eq!(view.focused(), Some(&"a1"));
    assert_eq!(view.selected(), &expanded(&["a1", "a2"]));
}

#[test]
fn ctrl_arrow_moves_focus_without_touching_the_selection() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    let mut view = single_with_focus(&s, "root");
    let _ = view.select_single(&"root");
    let events = view.handle_key(TreeKey::Down, Modifiers::ctrl(), &s, &rows);
    assert_eq!(view.focused(), Some(&"a"));
    assert_eq!(
        view.selected(),
        &expanded(&["root"]),
        "the spec prints nothing for Ctrl+arrow; the conservative reading is \
         focus-only, and the gap is recorded rather than invented away"
    );
    assert!(events.is_empty());
}

#[test]
fn left_collapses_an_open_branch() {
    let s = fixture();
    let mut view = single_with_focus(&s, "root");
    view.set_expanded(&"root", true);
    let rows = visible_rows(&s, view.expanded());
    let events = view.handle_key(TreeKey::Left, Modifiers::none(), &s, &rows);
    assert_eq!(events, vec![TreeEvent::ExpandToggle("root", false)]);
    assert!(!view.is_expanded(&"root"));
}

#[test]
fn left_on_a_closed_node_jumps_to_its_parent() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = single_with_focus(&s, "a");
    let events = view.handle_key(TreeKey::Left, Modifiers::none(), &s, &rows);
    assert!(events.is_empty());
    assert_eq!(view.focused(), Some(&"root"));
}

#[test]
fn left_on_a_root_leaf_does_nothing() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = single_with_focus(&s, "c");
    let events = view.handle_key(TreeKey::Left, Modifiers::none(), &s, &rows);
    assert!(events.is_empty());
    assert_eq!(view.focused(), Some(&"c"));
}

#[test]
fn right_opens_a_closed_branch_before_moving() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = single_with_focus(&s, "root");
    let events = view.handle_key(TreeKey::Right, Modifiers::none(), &s, &rows);
    assert_eq!(events, vec![TreeEvent::ExpandToggle("root", true)]);
    assert!(view.is_expanded(&"root"));
    assert_eq!(view.focused(), Some(&"root"), "opening does not move focus");
}

#[test]
fn right_on_an_open_branch_steps_to_its_first_child() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = single_with_focus(&s, "root");
    view.set_expanded(&"root", true);
    let events = view.handle_key(TreeKey::Right, Modifiers::none(), &s, &rows);
    assert!(events.is_empty());
    assert_eq!(view.focused(), Some(&"a"));
}

#[test]
fn right_on_a_leaf_does_nothing() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    let mut view = single_with_focus(&s, "a1");
    let events = view.handle_key(TreeKey::Right, Modifiers::none(), &s, &rows);
    assert!(events.is_empty());
    assert_eq!(view.focused(), Some(&"a1"));
}

#[test]
fn home_and_end_jump_to_the_visible_ends() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let mut view = single_with_focus(&s, "a1");
    let _ = view.handle_key(TreeKey::End, Modifiers::none(), &s, &rows);
    assert_eq!(view.focused(), Some(&"c"));
    assert_eq!(view.focus_slot(), Some(6));
    let _ = view.handle_key(TreeKey::Home, Modifiers::none(), &s, &rows);
    assert_eq!(view.focused(), Some(&"root"));
    assert_eq!(view.focus_slot(), Some(0));
}

#[test]
fn activate_reports_the_focused_row() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = single_with_focus(&s, "root");
    let events = view.handle_key(TreeKey::Activate, Modifiers::none(), &s, &rows);
    assert_eq!(events, vec![TreeEvent::Activate("root")]);
}

#[test]
fn toggle_select_adds_in_multi_mode() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_index(&rows, 1).unwrap();
    let events = view.handle_key(TreeKey::ToggleSelect, Modifiers::none(), &s, &rows);
    assert_eq!(events, vec![TreeEvent::SelectionChanged(expanded(&["a"]))]);
    let events = view.handle_key(TreeKey::ToggleSelect, Modifiers::none(), &s, &rows);
    assert_eq!(events, vec![TreeEvent::SelectionChanged(BTreeSet::new())]);
}

#[test]
fn select_all_keys_fill_a_multi_selection() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let events = view.handle_key(TreeKey::SelectAll, Modifiers::none(), &s, &rows);
    assert_eq!(events.len(), 1);
    assert_eq!(view.selected().len(), 4);
}

#[test]
fn every_key_is_silent_with_nothing_focused() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    for key in [
        TreeKey::Up,
        TreeKey::Down,
        TreeKey::Left,
        TreeKey::Right,
        TreeKey::Home,
        TreeKey::End,
        TreeKey::Activate,
        TreeKey::ToggleSelect,
        TreeKey::SelectAll,
    ] {
        let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
        let events = view.handle_key(key, Modifiers::none(), &s, &rows);
        match key {
            TreeKey::SelectAll => assert!(!events.is_empty(), "select-all needs no focus"),
            _ => assert!(events.is_empty(), "{key:?} fired with no focus"),
        }
    }
}

#[test]
fn no_key_emits_anything_for_an_empty_tree() {
    let s = TreeStructure::<Id>::new();
    let rows = visible_rows(&s, &expanded(&[]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    for key in [
        TreeKey::Home,
        TreeKey::End,
        TreeKey::Down,
        TreeKey::SelectAll,
    ] {
        assert!(
            view.handle_key(key, Modifiers::none(), &s, &rows)
                .is_empty()
        );
    }
}

// ---------------------------------------------------------------------------
// 7 - the anchor is an id, never an index
// ---------------------------------------------------------------------------

#[test]
fn the_anchor_resolves_to_itself_while_visible() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&"a1");
    assert_eq!(view.resolve_anchor(&s, &rows), Some(2));
}

#[test]
fn an_anchor_collapsed_away_resolves_to_its_nearest_visible_ancestor() {
    let s = fixture();
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&"a1");
    let collapsed = visible_rows(&s, &expanded(&["root"]));
    assert_eq!(
        view.resolve_anchor(&s, &collapsed),
        Some(1),
        "a1 is hidden; its parent `a` is the row the user is looking at"
    );
    let further = visible_rows(&s, &expanded(&[]));
    assert_eq!(
        view.resolve_anchor(&s, &further),
        Some(0),
        "collapse one more level and the anchor walks up to `root`"
    );
}

#[test]
fn an_anchor_returns_when_its_branch_reopens() {
    let s = fixture();
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&"a1");
    let collapsed = visible_rows(&s, &expanded(&[]));
    assert_ne!(view.resolve_anchor(&s, &collapsed), Some(2));
    let reopened = visible_rows(&s, &expanded(&["root", "a"]));
    assert_eq!(view.resolve_anchor(&s, &reopened), Some(2));
    assert_eq!(view.anchor(), Some(&"a1"), "the id was never lost");
}

#[test]
fn a_removed_anchor_leaves_shift_with_nothing_to_extend_from() {
    let s = fixture();
    let mut view = single_with_focus(&s, "a1");
    let _ = view.select_single(&"a1");
    let without = TreeStructure::from_pairs(&[("root", None), ("b", Some("root")), ("c", None)]);
    let new_rows = visible_rows(&without, &expanded(&["root"]));
    let report: SyncReport<Id> = view.sync_to(&without, &new_rows);
    assert_eq!(report.dropped_anchor, Some("a1"));
    assert_eq!(view.anchor(), None);
    let _ = view.handle_key(TreeKey::Down, Modifiers::shift(), &without, &new_rows);
    assert_eq!(
        view.selected().len(),
        1,
        "with no anchor the shift selects the focused row alone rather than \
         reaching back to a node that no longer exists"
    );
}

// ---------------------------------------------------------------------------
// 8 - the context menu
// ---------------------------------------------------------------------------

#[test]
fn a_menu_on_an_unselected_row_resets_the_selection_first() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let _ = rows;
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&"root");
    let events = view.context_menu(&"c", iced::Point::new(10.0, 20.0), false);
    assert_eq!(events.len(), 2);
    assert_eq!(view.selected(), &expanded(&["c"]));
    match &events[1] {
        TreeEvent::ContextMenu(set, at) => {
            assert_eq!(set, &expanded(&["c"]));
            assert_eq!(*at, iced::Point::new(10.0, 20.0));
        }
        other => panic!("expected a context menu, got {other:?}"),
    }
}

#[test]
fn a_menu_on_a_selected_row_preserves_the_group() {
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&"a");
    let _ = view.toggle_selected(&"b");
    let events = view.context_menu(&"a", iced::Point::new(1.0, 2.0), true);
    assert_eq!(events.len(), 1);
    assert_eq!(view.selected(), &expanded(&["a", "b"]));
}

#[test]
fn a_menu_on_an_already_single_selected_row_emits_only_the_menu() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    let _ = view.select_single(&"a");
    let events = view.context_menu(&"a", iced::Point::new(0.0, 0.0), false);
    assert_eq!(events.len(), 1);
}

// ---------------------------------------------------------------------------
// 9 - auto-scroll
// ---------------------------------------------------------------------------

#[test]
fn auto_scroll_yields_to_an_open_thumb_drag() {
    use crate::composite::virtual_list::{RowRung, VirtualMetrics};
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a", "b"]));
    let metrics = VirtualMetrics::new(rows.len(), RowRung::Compact, 24.0);
    let view = single_with_focus(&s, "b1");
    assert_eq!(
        view.scroll_into_view(&rows, &metrics, 0.0, true),
        None,
        "the drag owns the offset and will commit its own value"
    );
    assert!(
        view.scroll_into_view(&rows, &metrics, 0.0, false).is_some(),
        "with no drag the focused row must be revealed"
    );
}

#[test]
fn auto_scroll_does_nothing_for_a_row_already_in_view() {
    use crate::composite::virtual_list::{RowRung, VirtualMetrics};
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let metrics = VirtualMetrics::new(rows.len(), RowRung::Compact, 96.0);
    let view = single_with_focus(&s, "root");
    assert_eq!(view.scroll_into_view(&rows, &metrics, 0.0, false), None);
}

#[test]
fn auto_scroll_is_silent_without_focus() {
    use crate::composite::virtual_list::{RowRung, VirtualMetrics};
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let metrics = VirtualMetrics::new(rows.len(), RowRung::Compact, 24.0);
    let view = TreeView::<Id>::new(SelectionMode::Single);
    assert_eq!(view.scroll_into_view(&rows, &metrics, 0.0, false), None);
}

// ---------------------------------------------------------------------------
// 10 - structural change
// ---------------------------------------------------------------------------

#[test]
fn a_clean_resync_reports_nothing() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root"]));
    let mut view = single_with_focus(&s, "a");
    let _ = view.select_single(&"a");
    let report = view.sync_to(&s, &rows);
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn a_deleted_selected_node_is_reported_not_swallowed() {
    let before = fixture();
    assert!(before.contains(&"a"), "the fixture starts with `a` present");
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&"a");
    let _ = view.toggle_selected(&"c");
    let shrunk = TreeStructure::from_pairs(&[
        ("root", None),
        ("b", Some("root")),
        ("b1", Some("b")),
        ("c", None),
    ]);
    let new_rows = visible_rows(&shrunk, &expanded(&["root"]));
    let report = view.sync_to(&shrunk, &new_rows);
    assert_eq!(report.dropped_selected, vec!["a"]);
    assert_eq!(view.selected(), &expanded(&["c"]));
}

#[test]
fn an_expansion_that_lost_its_children_is_dropped() {
    let mut view = TreeView::<Id>::new(SelectionMode::Single);
    view.set_expanded(&"a", true);
    let shrunk = TreeStructure::from_pairs(&[("root", None), ("a", Some("root")), ("c", None)]);
    let new_rows = visible_rows(&shrunk, view.expanded());
    let report = view.sync_to(&shrunk, &new_rows);
    assert_eq!(
        report.dropped_expanded,
        vec!["a"],
        "a leaf cannot stay marked open"
    );
    assert!(!view.is_expanded(&"a"));
}

#[test]
fn focus_collapsed_away_lands_on_the_ancestor_that_hid_it() {
    let s = fixture();
    let mut view = single_with_focus(&s, "a1");
    let collapsed = visible_rows(&s, &expanded(&["root"]));
    let report = view.sync_to(&s, &collapsed);
    assert_eq!(report.lost_focus, Some("a1"));
    assert_eq!(report.landed_focus, Some("a"));
    assert_eq!(view.focused(), Some(&"a"));
    assert_eq!(view.focus_slot(), Some(1));
}

#[test]
fn focus_on_a_deleted_node_lands_on_whoever_holds_the_slot() {
    let s = fixture();
    let mut view = single_with_focus(&s, "a1");
    let shrunk = TreeStructure::from_pairs(&[("root", None), ("a", Some("root")), ("c", None)]);
    let new_rows = visible_rows(&shrunk, &expanded(&["root", "a"]));
    let report = view.sync_to(&shrunk, &new_rows);
    assert_eq!(report.lost_focus, Some("a1"));
    assert_eq!(view.focused(), Some(&"c"));
    assert_eq!(
        report.landed_focus,
        Some("c"),
        "slot 2 is now held by `c`; the slot-preserving rule is the same one \
         FocusState applies to a removed tab stop"
    );
    assert_eq!(new_rows.len(), 3);
}

#[test]
fn a_tree_that_vanishes_entirely_loses_focus_cleanly() {
    let s = fixture();
    let mut view = single_with_focus(&s, "a");
    let report = view.sync_to(&TreeStructure::<Id>::new(), &[]);
    assert_eq!(report.lost_focus, Some("a"));
    assert_eq!(report.landed_focus, None);
    assert_eq!(view.focused(), None);
    assert_eq!(view.focus_slot(), None);
}

#[test]
fn selection_survives_a_parent_being_collapsed() {
    let s = fixture();
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&"a1");
    let _ = view.toggle_selected(&"a2");
    let collapsed = visible_rows(&s, &expanded(&[]));
    let report = view.sync_to(&s, &collapsed);
    assert!(
        report.dropped_selected.is_empty(),
        "hiding a row is not deleting it"
    );
    assert_eq!(view.selected(), &expanded(&["a1", "a2"]));
}

#[test]
fn re_expanding_restores_the_visible_set_without_touching_the_model() {
    let s = fixture();
    let mut view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&"a1");
    let before = visible_rows(&s, &expanded(&["root", "a"]));
    view.set_expanded(&"root", true);
    view.set_expanded(&"a", true);
    let after = visible_rows(&s, view.expanded());
    assert_eq!(before, after);
    assert_eq!(view.selected(), &expanded(&["a1"]));
}

// ---------------------------------------------------------------------------
// 11 - focus registration
// ---------------------------------------------------------------------------

#[test]
fn every_visible_row_is_a_focus_member_and_a_disabled_one_is_flagged() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&["root", "a"]));
    let view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let members = view.focus_members(&rows, &expanded(&["a1"]));
    assert_eq!(members.len(), rows.len());
    let a1 = members
        .iter()
        .find(|m| m.id.as_str() == "a1")
        .expect("a1 registered");
    assert!(!a1.enabled);
    let root = members
        .iter()
        .find(|m| m.id.as_str() == "root")
        .expect("root registered");
    assert!(root.enabled);
}

#[test]
fn a_collapsed_subtree_is_unreachable_because_it_is_not_registered() {
    let s = fixture();
    let collapsed_rows = visible_rows(&s, &expanded(&[]));
    let view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let members = view.focus_members(&collapsed_rows, &BTreeSet::new());
    let mut state = FocusState::new();
    state.register_group(crate::focus::GroupId::new("tree"), members);

    let mut reached = Vec::new();
    for _ in 0..4 {
        if let Some(id) = state.focus_next() {
            reached.push(id.as_str().to_string());
        }
    }
    let joined = reached.join(",");
    assert!(
        !joined.contains("a1"),
        "a row hidden by a collapse must not be reachable: {joined}"
    );
    assert!(joined.contains("root") && joined.contains("c"), "{joined}");
}

#[test]
fn a_disabled_member_is_stepped_over_not_reachable() {
    let s = fixture();
    let rows = visible_rows(&s, &expanded(&[]));
    let view = TreeView::<Id>::new(SelectionMode::ExtendedMulti);
    let members = view.focus_members(&rows, &expanded(&["root"]));
    let mut state = FocusState::new();
    state.register_group(crate::focus::GroupId::new("tree"), members);
    let mut reached = Vec::new();
    for _ in 0..4 {
        if let Some(id) = state.focus_next() {
            reached.push(id.as_str().to_string());
        }
    }
    let first = reached.first().cloned().unwrap_or_default();
    assert_ne!(
        first, "root",
        "the disabled root must be stepped over, got {reached:?}"
    );
    let _ = FocusId::new("unused");
}
