//! Tests for the structured grid's state model (`super::table_view`,
//! `UIF-COMP-007`).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::*;
use crate::composite::table_column::{ColumnSpec, resolve_widths};
use crate::composite::table_header::SortDirection;
use crate::composite::tree_view::{Modifiers, SelectionMode};
use iced::{Point, Rectangle};

type Id = u32;

const TOL: f32 = 1e-3;

fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < TOL
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

fn specs() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::fixed(0, "a", 100.0)
            .with_sortable(true)
            .with_resizable(true),
        content(1, 140.0, 40.0)
            .with_sortable(true)
            .with_resizable(true),
        proportional(2, 1.0, 60.0)
            .with_sortable(true)
            .with_resizable(true),
    ]
}

fn rows(count: usize) -> Vec<Id> {
    (0..count as Id).collect()
}

/// A content column with its minimum set, spelled short so the tests read
/// as widths rather than as builder chains.
fn content(id: ColId, preferred: f32, min: f32) -> ColumnSpec {
    ColumnSpec::content(id, "c", preferred).with_min(min)
}

/// A proportional column with its minimum set.
fn proportional(id: ColId, weight: f32, min: f32) -> ColumnSpec {
    ColumnSpec::proportional(id, "c", weight).with_min(min)
}

fn viewport() -> Rectangle {
    rect(0.0, 0.0, 400.0, 320.0)
}

fn body() -> Rectangle {
    TableView::<Id>::body_viewport(viewport())
}

/// Body height is 288, so nine 32px rows fit exactly.
const VISIBLE_ROWS: usize = 9;

struct Fixture {
    specs: Vec<ColumnSpec>,
    layout: ColumnLayout,
    rows: Vec<Id>,
    metrics: VirtualMetrics,
}

impl Fixture {
    fn new(count: usize) -> Self {
        let specs = specs();
        let layout = resolve_widths(&specs, 400.0);
        let rows = rows(count);
        let metrics = TableView::<Id>::metrics(count, body());
        Self {
            specs,
            layout,
            rows,
            metrics,
        }
    }

    fn ctx(&self) -> TableContext<'_, Id> {
        TableContext {
            specs: &self.specs,
            layout: &self.layout,
            rows: &self.rows,
            metrics: &self.metrics,
            scroll_y: 0.0,
        }
    }
}

fn set(ids: &[Id]) -> BTreeSet<Id> {
    ids.iter().copied().collect()
}

fn focused_row(view: &TableView<Id>) -> Option<Id> {
    view.focus().and_then(TableFocus::row).cloned()
}

fn focused_column(view: &TableView<Id>) -> Option<ColId> {
    view.focus().map(TableFocus::column)
}

// ---------------------------------------------------------------------------
// Selection and the anchor
// ---------------------------------------------------------------------------

#[test]
fn select_single_sets_the_selection_and_the_anchor() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.select_single(&3));
    assert_eq!(view.selected(), &set(&[3]));
    assert_eq!(view.anchor(), Some(&3));
    assert!(
        !view.select_single(&3),
        "re-selecting the same row changes nothing"
    );
}

#[test]
fn toggle_in_single_mode_is_a_select() {
    let mut view = TableView::<Id>::new(SelectionMode::Single);
    assert!(view.toggle_selected(&2));
    assert_eq!(view.selected(), &set(&[2]));
    assert!(view.toggle_selected(&5));
    assert_eq!(
        view.selected(),
        &set(&[5]),
        "single mode cannot hold two rows"
    );
}

#[test]
fn toggle_in_multi_mode_adds_and_removes() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&1);
    let _ = view.toggle_selected(&4);
    assert_eq!(view.selected(), &set(&[1, 4]));
    let _ = view.toggle_selected(&1);
    assert_eq!(view.selected(), &set(&[4]));
    assert_eq!(
        view.anchor(),
        Some(&1),
        "the anchor moves even on a removal"
    );
}

#[test]
fn clear_selection_takes_the_anchor_with_it() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&2);
    assert!(view.clear_selection());
    assert!(view.selected().is_empty());
    assert_eq!(view.anchor(), None);
    assert!(!view.clear_selection());
}

#[test]
fn select_range_clamps_and_reads_an_inverted_range_backwards() {
    let rows = rows(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.select_range(&rows, 2, 4);
    assert_eq!(view.selected(), &set(&[2, 3, 4]));
    view.select_range(&rows, 7, 5);
    assert_eq!(view.selected(), &set(&[5, 6, 7]));
    view.select_range(&rows, 8, 99);
    assert_eq!(view.selected(), &set(&[8, 9]));
}

#[test]
fn select_range_on_an_empty_table_does_nothing() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(!view.select_range(&[], 0, 5));
    assert!(view.selected().is_empty());
}

#[test]
fn select_all_is_refused_in_single_mode() {
    let rows = rows(4);
    let mut single = TableView::<Id>::new(SelectionMode::Single);
    assert!(!single.select_all(&rows));
    assert!(single.selected().is_empty());

    let mut multi = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(multi.select_all(&rows));
    assert_eq!(multi.selected(), &set(&[0, 1, 2, 3]));
}

#[test]
fn the_anchor_is_an_id_and_follows_its_row_through_a_reorder() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&4);
    assert_eq!(view.resolve_anchor(&rows(10)), Some(4));

    let reversed: Vec<Id> = (0..10 as Id).rev().collect();
    assert_eq!(view.resolve_anchor(&reversed), Some(5));
}

#[test]
fn an_anchor_whose_row_is_gone_resolves_to_none() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&9);
    assert_eq!(view.resolve_anchor(&rows(5)), None);
}

#[test]
fn shift_down_extends_from_the_anchor() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&fixture.rows, 0, 2, Modifiers::none());
    let _ = view.handle_key(TableKey::Down, Modifiers::shift(), &fixture.ctx());
    assert_eq!(view.selected(), &set(&[2, 3]));
    let _ = view.handle_key(TableKey::Down, Modifiers::shift(), &fixture.ctx());
    assert_eq!(view.selected(), &set(&[2, 3, 4]));
}

#[test]
fn shift_up_extends_upwards_from_the_anchor() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&fixture.rows, 0, 5, Modifiers::none());
    let _ = view.handle_key(TableKey::Up, Modifiers::shift(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Up, Modifiers::shift(), &fixture.ctx());
    assert_eq!(view.selected(), &set(&[3, 4, 5]));
}

#[test]
fn shift_with_a_lost_anchor_selects_only_the_focused_row() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&99);
    view.focus_cell(&fixture.rows, 0, 4);
    view.apply_shift_range(&fixture.rows);
    assert_eq!(view.selected(), &set(&[4]));
    assert_eq!(view.anchor(), Some(&4));
}

#[test]
fn shift_without_focus_changes_nothing() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.apply_shift_range(&fixture.rows);
    assert!(view.selected().is_empty());
    assert_eq!(view.anchor(), None);
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

/// Column 0 sorts by a key with deliberate ties, which is what makes the
/// stability assertions mean something.
fn key(id: &Id) -> u32 {
    id % 3
}

fn compare(a: &Id, b: &Id, column: ColId) -> Ordering {
    match column {
        0 => key(a).cmp(&key(b)),
        _ => a.cmp(b),
    }
}

#[test]
fn no_sort_leaves_the_order_alone() {
    let input = rows(6);
    let out = sorted_rows(&input, None, compare);
    assert_eq!(out, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(
        input,
        vec![0, 1, 2, 3, 4, 5],
        "the input must not be mutated"
    );
}

#[test]
fn ascending_is_stable() {
    let input = rows(9);
    let out = sorted_rows(&input, Some((0, SortDirection::Ascending)), compare);
    assert_eq!(out, vec![0, 3, 6, 1, 4, 7, 2, 5, 8]);
}

#[test]
fn descending_is_stable_and_not_a_reversed_ascending() {
    let input = rows(9);
    let out = sorted_rows(&input, Some((0, SortDirection::Descending)), compare);
    // Ties keep their original relative order. Reversing an ascending
    // result would give [8, 5, 2, 7, 4, 1, 6, 3, 0] instead.
    assert_eq!(out, vec![2, 5, 8, 1, 4, 7, 0, 3, 6]);
    let ascending = sorted_rows(&input, Some((0, SortDirection::Ascending)), compare);
    let mut reversed_ascending = ascending.clone();
    reversed_ascending.reverse();
    assert_ne!(out, reversed_ascending);
}

#[test]
fn sorting_does_not_change_which_rows_are_selected() {
    let input = rows(9);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.toggle_selected(&1);
    let _ = view.toggle_selected(&4);
    let _ = view.toggle_selected(&8);

    let before = view.selected().clone();
    let ascending = sorted_rows(&input, Some((0, SortDirection::Ascending)), compare);
    let descending = sorted_rows(&input, Some((0, SortDirection::Descending)), compare);

    assert_eq!(view.selected(), &before);
    let mut ascending_ids = ascending.clone();
    ascending_ids.sort();
    assert_eq!(
        ascending_ids,
        rows(9),
        "a sort is a permutation, never a filter"
    );
    let mut descending_ids = descending.clone();
    descending_ids.sort();
    assert_eq!(descending_ids, rows(9));
    assert_eq!(view.selected(), &set(&[1, 4, 8]));
}

#[test]
fn sorting_does_not_change_the_anchor_identity() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&7);
    let _ = sorted_rows(&rows(9), Some((0, SortDirection::Descending)), compare);
    assert_eq!(view.anchor(), Some(&7));
}

#[test]
fn shift_select_after_a_sort_uses_the_new_positions() {
    let specs = specs();
    let layout = resolve_widths(&specs, 400.0);
    let metrics = TableView::<Id>::metrics(9, body());
    let sorted = sorted_rows(&rows(9), Some((0, SortDirection::Ascending)), compare);
    let ctx = TableContext {
        specs: &specs,
        layout: &layout,
        rows: &sorted,
        metrics: &metrics,
        scroll_y: 0.0,
    };
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&sorted, 0, 0, Modifiers::none());
    let _ = view.handle_key(TableKey::Down, Modifiers::shift(), &ctx);
    assert_eq!(view.selected(), &set(&[0, 3]));
}

// ---------------------------------------------------------------------------
// Keyboard traversal
// ---------------------------------------------------------------------------

#[test]
fn down_and_up_move_the_row_and_hold_the_column() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 1, 0);
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(1));
    assert_eq!(focused_column(&view), Some(1));
    let _ = view.handle_key(TableKey::Up, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0));
    assert_eq!(focused_column(&view), Some(1));
}

#[test]
fn left_and_right_move_the_column_and_hold_the_row() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 1, 4);
    let _ = view.handle_key(TableKey::Right, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_column(&view), Some(2));
    assert_eq!(focused_row(&view), Some(4));
    let _ = view.handle_key(TableKey::Left, Modifiers::none(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Left, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_column(&view), Some(0));
    assert_eq!(focused_row(&view), Some(4));
}

#[test]
fn home_and_end_move_to_the_edge_columns() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 1, 3);
    let _ = view.handle_key(TableKey::End, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_column(&view), Some(2));
    let _ = view.handle_key(TableKey::Home, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_column(&view), Some(0));
    assert_eq!(
        focused_row(&view),
        Some(3),
        "Home moves the column, not the row"
    );
}

#[test]
fn ctrl_home_and_ctrl_end_move_to_the_edge_cells() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 1, 3);
    let _ = view.handle_key(TableKey::End, Modifiers::ctrl(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(9));
    let _ = view.handle_key(TableKey::Home, Modifiers::ctrl(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0));
}

#[test]
fn page_keys_move_one_realised_page() {
    let fixture = Fixture::new(40);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 0, 0);
    let _ = view.handle_key(TableKey::PageDown, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(VISIBLE_ROWS as Id));
    let _ = view.handle_key(TableKey::PageUp, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0));
}

#[test]
fn movement_clamps_at_both_ends_rather_than_wrapping() {
    let fixture = Fixture::new(5);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 0, 0);
    let _ = view.handle_key(TableKey::Up, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0), "Up at the top must not wrap");
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    assert_eq!(
        focused_row(&view),
        Some(4),
        "Down at the bottom must not wrap"
    );
    let _ = view.handle_key(TableKey::Left, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_column(&view), Some(0));
}

#[test]
fn with_no_focus_a_down_key_enters_at_the_first_row() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0));
}

#[test]
fn enter_on_a_body_cell_activates_it() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 1, 3);
    let events = view.handle_key(TableKey::Activate, Modifiers::none(), &fixture.ctx());
    assert_eq!(
        events,
        vec![TableEvent::CellAction(3, 1, CellAction::Activate)]
    );
}

#[test]
fn space_on_a_body_cell_toggles_its_row() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 0, 3);
    let events = view.handle_key(TableKey::ToggleSelect, Modifiers::none(), &fixture.ctx());
    assert_eq!(events, vec![TableEvent::SelectionChanged(set(&[3]))]);
    let events = view.handle_key(TableKey::ToggleSelect, Modifiers::none(), &fixture.ctx());
    assert_eq!(events, vec![TableEvent::SelectionChanged(BTreeSet::new())]);
}

#[test]
fn enter_and_space_on_a_header_cell_cycle_the_sort() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_header(1);
    let events = view.handle_key(TableKey::Activate, Modifiers::none(), &fixture.ctx());
    assert_eq!(
        events,
        vec![TableEvent::SortChanged(Some((1, SortDirection::Ascending)))]
    );
    let events = view.handle_key(TableKey::ToggleSelect, Modifiers::none(), &fixture.ctx());
    assert_eq!(
        events,
        vec![TableEvent::SortChanged(Some((
            1,
            SortDirection::Descending
        )))]
    );
    let events = view.handle_key(TableKey::Activate, Modifiers::none(), &fixture.ctx());
    assert_eq!(events, vec![TableEvent::SortChanged(None)]);
}

#[test]
fn ctrl_a_selects_every_row() {
    let fixture = Fixture::new(4);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let events = view.handle_key(TableKey::SelectAll, Modifiers::ctrl(), &fixture.ctx());
    assert_eq!(
        events,
        vec![TableEvent::SelectionChanged(set(&[0, 1, 2, 3]))]
    );
}

const ALL_KEYS: [TableKey; 11] = [
    TableKey::Up,
    TableKey::Down,
    TableKey::Left,
    TableKey::Right,
    TableKey::Home,
    TableKey::End,
    TableKey::PageUp,
    TableKey::PageDown,
    TableKey::Activate,
    TableKey::ToggleSelect,
    TableKey::SelectAll,
];

#[test]
fn keys_on_a_table_with_no_rows_and_no_columns_produce_nothing() {
    let specs: Vec<ColumnSpec> = Vec::new();
    let layout = resolve_widths(&specs, 400.0);
    let fixture = Fixture {
        specs,
        layout,
        rows: Vec::new(),
        metrics: TableView::<Id>::metrics(0, body()),
    };
    for key in ALL_KEYS {
        let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
        let events = view.handle_key(key, Modifiers::none(), &fixture.ctx());
        assert!(events.is_empty(), "{key:?} produced {events:?} on nothing");
        assert_eq!(
            view.focus(),
            None,
            "{key:?} focused a cell that does not exist"
        );
    }
}

#[test]
fn keys_on_a_table_with_no_rows_stay_inert_for_a_fresh_view_per_key() {
    // Columns exist, rows do not. Each key gets its own view so the assertion is
    // about the key, not about residue: with live columns `Home` legitimately
    // focuses a header cell, and a later `Activate` on that focus would be a
    // real sort rather than a bug.
    let fixture = Fixture::new(0);
    for key in ALL_KEYS {
        let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
        let events = view.handle_key(key, Modifiers::none(), &fixture.ctx());
        assert!(
            events.is_empty(),
            "{key:?} produced {events:?} with no rows to act on"
        );
    }
}

#[test]
fn down_from_the_header_enters_the_body() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_header(2);
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    assert_eq!(focused_row(&view), Some(0));
    assert_eq!(focused_column(&view), Some(2));
}

#[test]
fn shift_on_a_column_move_does_not_touch_the_selection() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&fixture.rows, 0, 1, Modifiers::none());
    let before = view.selected().clone();
    let _ = view.handle_key(TableKey::Right, Modifiers::shift(), &fixture.ctx());
    assert_eq!(view.selected(), &before);
}

#[test]
fn plain_arrows_in_single_mode_follow_the_row() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::Single);
    let _ = view.press_row(&fixture.rows, 0, 2, Modifiers::none());
    let _ = view.handle_key(TableKey::Down, Modifiers::none(), &fixture.ctx());
    assert_eq!(view.selected(), &set(&[3]));
    assert_eq!(view.anchor(), Some(&3));
}

// ---------------------------------------------------------------------------
// Pointer
// ---------------------------------------------------------------------------

#[test]
fn pressing_a_divider_opens_the_resize_and_emits_nothing() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let events = view.press_header(
        &fixture.specs,
        &fixture.layout,
        header,
        Point::new(100.0, 16.0),
    );
    assert!(events.is_empty());
    assert!(view.is_resizing());
    assert_eq!(view.resize.column(), Some(0));
}

#[test]
fn pressing_a_title_cycles_the_sort_and_focuses_the_header() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let events = view.press_header(
        &fixture.specs,
        &fixture.layout,
        header,
        Point::new(50.0, 16.0),
    );
    assert_eq!(
        events,
        vec![TableEvent::SortChanged(Some((0, SortDirection::Ascending)))]
    );
    assert_eq!(view.focus(), Some(&TableFocus::Header { column: 0 }));
    assert!(!view.is_resizing());
}

#[test]
fn a_plain_row_press_selects_and_focuses() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let events = view.press_row(&fixture.rows, 1, 4, Modifiers::none());
    assert_eq!(events, vec![TableEvent::SelectionChanged(set(&[4]))]);
    assert_eq!(focused_row(&view), Some(4));
    assert_eq!(focused_column(&view), Some(1));
}

#[test]
fn a_ctrl_row_press_toggles() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&fixture.rows, 0, 2, Modifiers::none());
    let events = view.press_row(&fixture.rows, 0, 5, Modifiers::ctrl());
    assert_eq!(events, vec![TableEvent::SelectionChanged(set(&[2, 5]))]);
    let events = view.press_row(&fixture.rows, 0, 2, Modifiers::ctrl());
    assert_eq!(events, vec![TableEvent::SelectionChanged(set(&[5]))]);
}

#[test]
fn a_shift_row_press_selects_the_anchor_range() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.press_row(&fixture.rows, 0, 6, Modifiers::none());
    let events = view.press_row(&fixture.rows, 0, 2, Modifiers::shift());
    assert_eq!(
        events,
        vec![TableEvent::SelectionChanged(set(&[2, 3, 4, 5, 6]))]
    );
}

#[test]
fn a_press_on_a_missing_row_emits_nothing() {
    let fixture = Fixture::new(3);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(
        view.press_row(&fixture.rows, 0, 9, Modifiers::none())
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// The resize gesture
// ---------------------------------------------------------------------------

#[test]
fn a_resize_emits_live_ticks_and_commits_once() {
    // An elastic column, so the drag actually moves the edge. A Fixed column
    // would report its own width back on every tick and prove nothing.
    let mut specs = specs();
    specs[0] = proportional(0, 1.0, 40.0)
        .with_sortable(true)
        .with_resizable(true);
    let layout = resolve_widths(&specs, 400.0);
    let edge = layout.divider_x(0);

    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(&specs, &layout, header, Point::new(edge, 16.0));

    let first = view.drag_resize(&specs, edge + 10.0).expect("a live tick");
    assert!(approx(*first.value(), edge + 10.0));
    let second = view.drag_resize(&specs, edge + 35.0).expect("a live tick");
    assert!(approx(*second.value(), edge + 35.0));

    let commit = view.release_resize().expect("a commit");
    assert!(approx(*commit.value(), edge + 35.0));
    assert!(
        view.release_resize().is_none(),
        "commit must fire exactly once"
    );
    assert!(!view.is_resizing());
}

#[test]
fn cancelling_a_resize_reverts_to_the_width_at_press() {
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(
        &fixture.specs,
        &fixture.layout,
        header,
        Point::new(100.0, 16.0),
    );
    let _ = view.drag_resize(&fixture.specs, 200.0);
    let cancel = view.cancel_resize().expect("a cancel");
    assert!(approx(*cancel.restored(), 100.0));
    assert!(!view.is_resizing());
    assert!(view.cancel_resize().is_none());
}

#[test]
fn a_resize_never_goes_below_the_columns_minimum() {
    // A Proportional column whose minimum sits above where the drag ends, so the
    // clamp is actually observable. A Fixed column could not show this: its floor
    // equals its ceiling, so every drag returns the same number.
    let mut specs = specs();
    specs[0] = proportional(0, 1.0, 80.0)
        .with_sortable(true)
        .with_resizable(true);
    let layout = resolve_widths(&specs, 400.0);
    let start = layout.divider_x(0);
    assert!(
        start > 80.0,
        "the column must start above its own minimum for this to mean anything"
    );

    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(&specs, &layout, header, Point::new(start, 16.0));
    let live = view.drag_resize(&specs, 10.0).expect("a live tick");
    assert!(
        approx(*live.value(), 80.0),
        "the live width must be clamped to the column minimum"
    );
}

#[test]
fn a_resize_never_goes_above_the_columns_maximum() {
    // Same shape, upper bound: a Content column cannot show this either, because
    // its ceiling IS its target and `.with_max()` above the target is discarded.
    let mut specs = specs();
    specs[0] = proportional(0, 1.0, 40.0)
        .with_sortable(true)
        .with_resizable(true)
        .with_max(150.0);
    let layout = resolve_widths(&specs, 400.0);
    let start = layout.divider_x(0);
    assert!(start < 150.0, "the column must start below its own maximum");

    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(&specs, &layout, header, Point::new(start, 16.0));
    let live = view.drag_resize(&specs, 900.0).expect("a live tick");
    assert!(
        approx(*live.value(), 150.0),
        "the live width must be clamped to the column maximum"
    );
}

#[test]
fn a_content_column_cannot_be_dragged_past_its_content_width() {
    // Content's ceiling is its target, so the outward half of the drag is dead
    // while the inward half still works down to the minimum.
    let mut specs = specs();
    specs[0] = content(0, 120.0, 30.0)
        .with_sortable(true)
        .with_resizable(true);
    let layout = resolve_widths(&specs, 400.0);
    let start = layout.divider_x(0);
    assert!(
        approx(start, 120.0),
        "a Content column sits at its content width"
    );

    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(&specs, &layout, header, Point::new(start, 16.0));

    assert!(approx(
        *view
            .drag_resize(&specs, 220.0)
            .expect("a live tick")
            .value(),
        120.0
    ));
    assert!(approx(
        *view.drag_resize(&specs, 50.0).expect("a live tick").value(),
        50.0
    ));
}

#[test]
fn a_fixed_column_offers_no_resize_handle_at_all() {
    // `fixed()` clears `resizable`: "this width never changes" and "the user may
    // drag this edge" are a contradiction. The handle is absent, not inert.
    let specs = vec![
        ColumnSpec::fixed(0, "a", 100.0).with_sortable(true),
        proportional(1, 1.0, 60.0).with_resizable(true),
    ];
    assert!(
        !table_header::column_is_resizable(&specs, 0),
        "a Fixed column must not expose a handle"
    );
    assert!(
        !table_header::column_is_resizable(&specs, 1),
        "and the last column never does either: its trailing edge is the container border"
    );

    let layout = resolve_widths(&specs, 400.0);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let events = view.press_header(&specs, &layout, header, Point::new(100.0, 16.0));
    assert!(
        events.is_empty(),
        "pressing a Fixed column's trailing edge must not open a drag: {events:?}"
    );
    assert!(!view.is_resizing());
}

#[test]
fn a_release_with_no_open_drag_emits_nothing() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.release_resize().is_none());
    assert!(view.cancel_resize().is_none());
    assert!(view.drag_resize(&specs(), 200.0).is_none());
}

#[test]
fn the_resized_column_is_frozen_at_press_across_the_whole_drag() {
    // Drag column 0's handle far to the right, past column 1's own
    // handle. The gesture must still be resizing column 0.
    let fixture = Fixture::new(10);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let header = TableView::<Id>::header_rect(viewport());
    let _ = view.press_header(
        &fixture.specs,
        &fixture.layout,
        header,
        Point::new(100.0, 16.0),
    );
    for x in [120.0, 180.0, 240.0, 300.0, 380.0] {
        let _ = view.drag_resize(&fixture.specs, x);
        assert_eq!(
            view.resize.column(),
            Some(0),
            "the drag drifted off column 0 at x={x}"
        );
    }
}

// ---------------------------------------------------------------------------
// Sticky header and virtualisation
// ---------------------------------------------------------------------------

#[test]
fn the_header_and_body_partition_the_viewport_exactly() {
    let band = TableView::<Id>::header_rect(viewport());
    let body = body();
    assert!(approx(band.height, 32.0));
    assert!(approx(band.y, 0.0));
    assert!(approx(body.y, 32.0));
    assert!(approx(band.y + band.height, body.y));
    assert!(approx(body.y + body.height, viewport().height));
    assert!(approx(body.width, viewport().width));
}

#[test]
fn a_viewport_shorter_than_the_header_clamps_the_body_to_zero() {
    let tiny = rect(0.0, 0.0, 400.0, 12.0);
    let body = TableView::<Id>::body_viewport(tiny);
    assert!(approx(body.height, 0.0));
    assert!(body.height >= 0.0);
}

#[test]
fn the_header_and_body_columns_stay_aligned_at_every_scroll_offset() {
    let fixture = Fixture::new(40);
    let band = TableView::<Id>::header_rect(viewport());
    let body = body();
    let mut offset = 0.0;
    while offset <= 400.0 {
        for index in 0..fixture.layout.len() {
            let header_cell = table_header::header_cell_rect(&fixture.layout, index, band);
            let body_cell = table_cell::cell_rect(&fixture.layout, index, body);
            let header_shifted = scrolled_x(header_cell, offset);
            let body_shifted = scrolled_x(body_cell, offset);
            assert!(
                approx(header_shifted.x, body_shifted.x),
                "column {index} drifted at offset {offset}: {} vs {}",
                header_shifted.x,
                body_shifted.x
            );
            assert!(approx(header_shifted.width, body_shifted.width));
        }
        offset += 7.0;
    }
}

#[test]
fn the_row_realisation_bound_does_not_depend_on_the_column_count() {
    let few = resolve_widths(&specs(), 400.0);
    let many = resolve_widths(
        &(0..40u16)
            .map(|index| proportional(index, 1.0, 20.0))
            .collect::<Vec<_>>(),
        400.0,
    );
    let few_metrics = TableView::<Id>::metrics(1000, body());
    let many_metrics = TableView::<Id>::metrics(1000, body());
    assert_eq!(few.len(), 3);
    assert_eq!(many.len(), 40);
    assert_eq!(
        few_metrics.realized_count_bound(),
        many_metrics.realized_count_bound(),
        "columns must not change how many rows are realised"
    );
}

#[test]
fn the_row_realisation_bound_does_not_depend_on_the_row_count() {
    let small = TableView::<Id>::metrics(100, body());
    let large = TableView::<Id>::metrics(1_000_000, body());
    assert_eq!(small.realized_count_bound(), large.realized_count_bound());
    assert!(large.realized_count_bound() < VISIBLE_ROWS + 10);
}

#[test]
fn the_body_viewport_height_is_what_the_metrics_are_built_on() {
    let metrics = TableView::<Id>::metrics(100, body());
    assert!(approx(metrics.viewport_len(), body().height));
    assert!(approx(metrics.row_height(), ROW_HEIGHT));
}

#[test]
fn horizontal_scroll_is_reported_only_when_the_columns_exceed_the_viewport() {
    let wide = viewport();
    let fitting = resolve_widths(&specs(), wide.width);
    assert!(!needs_horizontal_scroll(&fitting, wide));

    let tight = rect(0.0, 0.0, 150.0, 320.0);
    let cramped = resolve_widths(&specs(), tight.width);
    assert!(needs_horizontal_scroll(&cramped, tight));
}

// ---------------------------------------------------------------------------
// Bands
// ---------------------------------------------------------------------------

#[test]
fn the_row_band_reflects_selection_over_hover() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    let _ = view.select_single(&3);
    assert_eq!(view.row_band(&3, true, true), CellBand::Selected);
    assert_eq!(view.row_band(&4, true, true), CellBand::Hover);
    assert_eq!(view.row_band(&4, false, true), CellBand::Raised);
    assert_eq!(view.row_band(&4, false, false), CellBand::Base);
}

#[test]
fn the_header_band_is_the_raised_surface() {
    assert_eq!(header_band(), CellBand::Raised);
}

// ---------------------------------------------------------------------------
// Focus plumbing
// ---------------------------------------------------------------------------

#[test]
fn focus_helpers_report_what_they_set() {
    let fixture = Fixture::new(5);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.focus_cell(&fixture.rows, 2, 9).is_none());
    let focus = view.focus_cell(&fixture.rows, 2, 1).expect("a body focus");
    assert_eq!(focus, TableFocus::Body { row: 1, column: 2 });
    assert_eq!(focus.row(), Some(&1));
    assert_eq!(focus.column(), 2);
    let header = view.focus_header(1);
    assert_eq!(header, TableFocus::Header { column: 1 });
    assert_eq!(header.row(), None);
    assert_eq!(header.column(), 1);
}

#[test]
fn edge_column_focus_keeps_the_row() {
    let fixture = Fixture::new(5);
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    view.focus_cell(&fixture.rows, 0, 3);
    let focus = view
        .focus_edge_column(&fixture.specs, false)
        .expect("a focus");
    assert_eq!(focus, TableFocus::Body { row: 3, column: 2 });
}

#[test]
fn edge_column_focus_on_an_empty_column_list_is_none() {
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    assert!(view.focus_edge_column(&[], true).is_none());
    assert!(view.move_column(&[], 1).is_none());
}

#[test]
fn with_no_rows_and_no_columns_nothing_can_be_focused() {
    let specs: Vec<ColumnSpec> = Vec::new();
    let layout = resolve_widths(&specs, 400.0);
    let metrics = TableView::<Id>::metrics(0, body());
    let rows: Vec<Id> = Vec::new();
    let ctx = TableContext {
        specs: &specs,
        layout: &layout,
        rows: &rows,
        metrics: &metrics,
        scroll_y: 0.0,
    };
    let mut view = TableView::<Id>::new(SelectionMode::ExtendedMulti);
    for key in [
        TableKey::Up,
        TableKey::Down,
        TableKey::Left,
        TableKey::Right,
        TableKey::Home,
        TableKey::End,
        TableKey::PageUp,
        TableKey::PageDown,
        TableKey::Activate,
        TableKey::ToggleSelect,
        TableKey::SelectAll,
    ] {
        assert!(view.handle_key(key, Modifiers::none(), &ctx).is_empty());
        assert_eq!(view.focus(), None, "{key:?} focused something from nothing");
    }
}
