//! The structured grid's state model: row selection keyed by id, cell
//! focus, keyboard traversal, stable sorting, the column-resize gesture,
//! and the sticky-header geometry (`UIF-COMP-007`).
//!
//! This module owns *decisions*, not data. The host keeps its rows; the
//! view keeps the ordering request, the selection, the focused cell and
//! the in-flight resize, and hands back the events §3B.7 names. That
//! split is what lets the two hardest requirements here be stated as
//! facts rather than hoped for:
//!
//! * **Sorting must not change what is selected.** The selection is a
//!   `BTreeSet<Id>`, keyed by identity, and the sort produces a
//!   permutation of the host's row list rather than a mutation of it.
//!   No reordering can touch a set of ids, so the invariant is
//!   structural. See [`sorted_rows`] and the test
//!   `selection_survives_a_sort`.
//! * **The sort must be stable.** Descending is a stable sort with a
//!   *reversed comparator*, never an ascending result run backwards —
//!   reversing flips the ties, which would make two rows that compare
//!   equal swap order depending on the sort direction. That is exactly
//!   the kind of thing a user reads as the data being wrong.
//!
//! # Reuse rather than reinvention
//!
//! Selection and the anchor are the **tree's** rules ([`tree_view`]),
//! deliberately: [`SelectionMode`], [`Modifiers`] and the
//! anchor-is-an-id-never-an-index discipline are imported, not copied. A
//! user who has learned shift-select in the file tree has learned it in
//! the table. A second, subtly different set of anchor rules would be a
//! worse interface no matter how well each one worked on its own.
//!
//! Row virtualisation is [`virtual_list`]'s, and the sticky header is a
//! **separate band that does not participate in row recycling**: the
//! header is one rectangle laid out from the same [`ColumnLayout`] the
//! body uses, so column alignment during horizontal scroll is not a
//! synchronisation problem to be solved but a property of both layers
//! reading the same numbers.
//!
//! # The keyboard table
//!
//! | Key | Body focus | Header focus |
//! |-----|----------|--------------|
//! | `Up` / `Down` | move row, column held | — |
//! | `Left` / `Right` | move column, row held | move column |
//! | `Home` / `End` | first / last column | first / last column |
//! | `Ctrl+Home` / `Ctrl+End` | first / last cell | first / last column |
//! | `PageUp` / `PageDown` | move one realised page | — |
//! | `Shift` + any row move | extend from the anchor | no selection change |
//! | `Enter` | `CellAction::Activate` | cycle sort |
//! | `Space` | toggle row selection | cycle sort |
//! | `Ctrl+A` | select all | select all |
//!
//! Movement **clamps at the ends rather than wrapping**, which is the
//! tree's own decision and is kept so for the same reason: in a grid,
//! wrapping from the last row to the first loses the user's place
//! completely, because there is no surrounding context that tells them
//! the jump happened.
//!
//! [`tree_view`]: crate::composite::tree_view
//!
//! Unit tests live in the sibling `table_view_tests.rs` sidecar.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::binding::{self, Cancel, Commit, GestureSlot, Live};
use crate::composite::table_cell::{self, CellBand};
use crate::composite::table_column::{self, ColId, ColumnLayout, ColumnSpec};
use crate::composite::table_header::{self, HeaderPress, SortDirection};
use crate::composite::tree_view::{Modifiers, SelectionMode};
use crate::composite::virtual_list::{self, VirtualMetrics};
use iced::{Point, Rectangle};

/// The body row height — §3B.7's `min_height: 32px`, taken from the
/// cell module so the two cannot drift apart.
pub const ROW_HEIGHT: f32 = table_cell::BODY_ROW_MIN_HEIGHT;

/// What a keyboard activation on a cell means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellAction {
    /// `Enter` — open, edit or press whatever the cell holds.
    Activate,
    /// `Space` — toggle the row's selected state.
    Toggle,
}

/// The cell the keyboard is currently on.
///
/// Keyed by identity, exactly as the anchor is. A stored index would be
/// invalidated by the next sort, and the first arrow key after a sort
/// would move focus from a row the user can no longer see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableFocus<Id> {
    /// A header cell — focusable so the keyboard can cycle the sort.
    Header {
        /// The focused column.
        column: ColId,
    },
    /// A body cell.
    Body {
        /// The focused row.
        row: Id,
        /// The focused column.
        column: ColId,
    },
}

impl<Id: Clone> TableFocus<Id> {
    /// The focused column, whichever band the focus is on.
    #[must_use]
    pub fn column(&self) -> ColId {
        match self {
            Self::Header { column } | Self::Body { column, .. } => *column,
        }
    }

    /// The focused row id, if the focus is in the body.
    #[must_use]
    pub fn row(&self) -> Option<&Id> {
        match self {
            Self::Header { .. } => None,
            Self::Body { row, .. } => Some(row),
        }
    }
}

/// One keyboard action the table responds to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableKey {
    /// `Up`.
    Up,
    /// `Down`.
    Down,
    /// `Left`.
    Left,
    /// `Right`.
    Right,
    /// `Home`.
    Home,
    /// `End`.
    End,
    /// `PageUp`.
    PageUp,
    /// `PageDown`.
    PageDown,
    /// `Enter`.
    Activate,
    /// `Space`.
    ToggleSelect,
    /// `Ctrl+A`.
    SelectAll,
}

/// A callback §3B.7 asks the host to be told about.
#[derive(Clone, Debug, PartialEq)]
pub enum TableEvent<Id> {
    /// The row selection changed.
    SelectionChanged(BTreeSet<Id>),
    /// A cell was activated.
    CellAction(Id, ColId, CellAction),
    /// The sort changed; `None` is the unsorted state.
    SortChanged(Option<(ColId, SortDirection)>),
    /// A resize tick, fired on every move while the drag is open.
    ColumnResizeLive(ColId, f32),
    /// A resize settled, fired exactly once.
    ColumnResizeCommit(ColId, f32),
    /// A resize was abandoned and the width reverted.
    ColumnResizeCancel(ColId, f32),
}

/// The rows, columns and scroll state a key or a click is resolved
/// against.
///
/// Bundled because every resolution needs all of it, and a seven-argument
/// method is harder to read than one named thing.
pub struct TableContext<'a, Id> {
    /// The column declarations.
    pub specs: &'a [ColumnSpec],
    /// The resolved column layout.
    pub layout: &'a ColumnLayout,
    /// The rows in their current display order.
    pub rows: &'a [Id],
    /// The row virtualisation metrics for the body viewport.
    pub metrics: &'a VirtualMetrics,
    /// The body's vertical scroll offset.
    pub scroll_y: f32,
}

/// The structured grid's state.
#[derive(Clone, Debug)]
pub struct TableView<Id> {
    mode: SelectionMode,
    selected: BTreeSet<Id>,
    anchor: Option<Id>,
    focus: Option<TableFocus<Id>>,
    sort: Option<(ColId, SortDirection)>,
    resize: ResizeGesture,
}

impl<Id: Clone + Ord + core::fmt::Debug> TableView<Id> {
    /// A table in the given selection mode, with nothing selected,
    /// focused or sorted.
    #[must_use]
    pub fn new(mode: SelectionMode) -> Self {
        Self {
            mode,
            selected: BTreeSet::new(),
            anchor: None,
            focus: None,
            sort: None,
            resize: ResizeGesture::default(),
        }
    }

    /// The selection mode.
    #[must_use]
    pub const fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// The selected row ids.
    #[must_use]
    pub fn selected(&self) -> &BTreeSet<Id> {
        &self.selected
    }

    /// Whether `id` is selected.
    #[must_use]
    pub fn is_selected(&self, id: &Id) -> bool {
        self.selected.contains(id)
    }

    /// The shift-anchor, if one is set.
    #[must_use]
    pub fn anchor(&self) -> Option<&Id> {
        self.anchor.as_ref()
    }

    /// The focused cell.
    #[must_use]
    pub fn focus(&self) -> Option<&TableFocus<Id>> {
        self.focus.as_ref()
    }

    /// The current sort.
    #[must_use]
    pub const fn sort(&self) -> Option<(ColId, SortDirection)> {
        self.sort
    }

    /// Whether a column resize is in flight.
    #[must_use]
    pub fn is_resizing(&self) -> bool {
        self.resize.is_active()
    }

    // -- selection ---------------------------------------------------------

    /// Select one row and make it the anchor.
    #[must_use]
    pub fn select_single(&mut self, id: &Id) -> bool {
        let changed = self.selected.len() != 1 || !self.selected.contains(id);
        self.selected.clear();
        self.selected.insert(id.clone());
        self.anchor = Some(id.clone());
        changed
    }

    /// Toggle a row in the selection. In
    /// [`SelectionMode::Single`] a toggle *is* a select.
    #[must_use]
    pub fn toggle_selected(&mut self, id: &Id) -> bool {
        self.anchor = Some(id.clone());
        if !self.mode.is_multi() {
            return self.select_single(id);
        }
        if self.selected.remove(id) {
            true
        } else {
            self.selected.insert(id.clone());
            true
        }
    }

    /// Clear the selection. The anchor goes with it.
    pub fn clear_selection(&mut self) -> bool {
        let had = !self.selected.is_empty();
        self.selected.clear();
        self.anchor = None;
        had
    }

    /// Select the inclusive range of positions `from..=to`, clamped and
    /// read backwards if inverted — the tree's rule verbatim.
    pub fn select_range(&mut self, rows: &[Id], from: usize, to: usize) -> bool {
        if rows.is_empty() {
            return false;
        }
        let last = rows.len() - 1;
        let start = from.min(last);
        let end = to.min(last);
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        self.selected = rows[lo..=hi].iter().cloned().collect();
        true
    }

    /// Select every visible row.
    pub fn select_all(&mut self, rows: &[Id]) -> bool {
        if !self.mode.is_multi() {
            return false;
        }
        let next: BTreeSet<Id> = rows.iter().cloned().collect();
        let changed = next != self.selected;
        self.selected = next;
        changed
    }

    /// Resolve the anchor to a position in the current row order.
    ///
    /// The anchor is an id, so a sort that moves a row moves the anchor
    /// with it; an anchor whose row has gone away resolves to `None`
    /// rather than to the index it used to occupy.
    #[must_use]
    pub fn resolve_anchor(&self, rows: &[Id]) -> Option<usize> {
        self.anchor
            .as_ref()
            .and_then(|anchor| position_of(rows, anchor))
    }

    /// The `Shift` rule: the selection becomes the range from the
    /// resolved anchor to the focused row.
    fn apply_shift_range(&mut self, rows: &[Id]) {
        let Some(focus) = self.focus.as_ref().and_then(TableFocus::row) else {
            return;
        };
        let Some(to) = position_of(rows, focus) else {
            return;
        };
        match self.resolve_anchor(rows) {
            Some(from) => {
                self.select_range(rows, from, to);
            }
            None => {
                self.selected.clear();
                self.selected.insert(focus.clone());
                self.anchor = Some(focus.clone());
            }
        }
    }

    // -- focus -------------------------------------------------------------

    /// Focus the cell at `row_index` / `column_index`.
    pub fn focus_cell(
        &mut self,
        rows: &[Id],
        column: ColId,
        row_index: usize,
    ) -> Option<TableFocus<Id>> {
        let row = rows.get(row_index)?.clone();
        let focus = TableFocus::Body { row, column };
        self.focus = Some(focus.clone());
        Some(focus)
    }

    /// Focus the header cell for `column`.
    pub fn focus_header(&mut self, column: ColId) -> TableFocus<Id> {
        let focus = TableFocus::Header { column };
        self.focus = Some(focus.clone());
        focus
    }

    /// Move the focused row by `delta`, clamped at both ends.
    pub fn move_row(&mut self, rows: &[Id], delta: i32) -> Option<TableFocus<Id>> {
        if rows.is_empty() {
            return None;
        }
        let last = rows.len() - 1;
        let column = self.focus.as_ref().column_or_first();
        let current = match self.focus.as_ref().and_then(TableFocus::row) {
            Some(row) => position_of(rows, row)?,
            None => {
                let index = if delta >= 0 { 0 } else { last };
                return self.focus_cell(rows, column, index);
            }
        };
        let next = if delta >= 0 {
            current.saturating_add(delta as usize).min(last)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        self.focus_cell(rows, column, next)
    }

    /// Move the focused column by `delta`, clamped at both ends.
    pub fn move_column(&mut self, specs: &[ColumnSpec], delta: i32) -> Option<TableFocus<Id>> {
        if specs.is_empty() {
            return None;
        }
        let last = specs.len() - 1;
        let current = self
            .focus
            .as_ref()
            .map(|focus| {
                specs
                    .iter()
                    .position(|spec| spec.id == focus.column())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let next = if delta >= 0 {
            current.saturating_add(delta as usize).min(last)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        let column = specs[next].id;
        match self.focus.clone() {
            Some(TableFocus::Header { .. }) | None => Some(self.focus_header(column)),
            Some(TableFocus::Body { row, .. }) => {
                let focus = TableFocus::Body { row, column };
                self.focus = Some(focus.clone());
                Some(focus)
            }
        }
    }

    /// Move focus to the first (`true`) or last (`false`) column.
    pub fn focus_edge_column(
        &mut self,
        specs: &[ColumnSpec],
        first: bool,
    ) -> Option<TableFocus<Id>> {
        let spec = if first { specs.first()? } else { specs.last()? };
        let column = spec.id;
        match self.focus.clone() {
            Some(TableFocus::Body { row, .. }) => {
                let focus = TableFocus::Body { row, column };
                self.focus = Some(focus.clone());
                Some(focus)
            }
            _ => Some(self.focus_header(column)),
        }
    }

    // -- keyboard ----------------------------------------------------------

    /// Apply one key and report what the host must be told.
    #[must_use]
    pub fn handle_key(
        &mut self,
        key: TableKey,
        mods: Modifiers,
        ctx: &TableContext<'_, Id>,
    ) -> Vec<TableEvent<Id>> {
        let mut events = Vec::new();
        let rows = ctx.rows;

        match key {
            TableKey::Up | TableKey::Down => {
                let delta = if key == TableKey::Up { -1 } else { 1 };
                if matches!(self.focus, Some(TableFocus::Header { .. })) {
                    if delta > 0 {
                        self.focus_cell(rows, self.focus.as_ref().column_or_first(), 0);
                    }
                    return events;
                }
                if self.move_row(rows, delta).is_none() {
                    return events;
                }
                if mods.shift {
                    self.apply_shift_range(rows);
                    events.push(TableEvent::SelectionChanged(self.selected.clone()));
                } else if !mods.ctrl
                    && !self.mode.is_multi()
                    && let Some(row) = self.focus.as_ref().and_then(TableFocus::row)
                {
                    let row = row.clone();
                    if self.select_single(&row) {
                        events.push(TableEvent::SelectionChanged(self.selected.clone()));
                    }
                }
            }
            TableKey::Left | TableKey::Right => {
                let delta = if key == TableKey::Left { -1 } else { 1 };
                self.move_column(ctx.specs, delta);
            }
            TableKey::Home | TableKey::End => {
                let first = key == TableKey::Home;
                if mods.ctrl {
                    let index = if first {
                        0
                    } else {
                        rows.len().saturating_sub(1)
                    };
                    self.focus_cell(rows, self.focus.as_ref().column_or_first(), index);
                } else {
                    self.focus_edge_column(ctx.specs, first);
                }
                if mods.shift {
                    self.apply_shift_range(rows);
                    events.push(TableEvent::SelectionChanged(self.selected.clone()));
                }
            }
            TableKey::PageUp | TableKey::PageDown => {
                let page = ctx.metrics.visible_range(ctx.scroll_y).len().max(1) as i32;
                let delta = if key == TableKey::PageUp { -page } else { page };
                if self.move_row(rows, delta).is_none() {
                    return events;
                }
                if mods.shift {
                    self.apply_shift_range(rows);
                    events.push(TableEvent::SelectionChanged(self.selected.clone()));
                }
            }
            TableKey::Activate | TableKey::ToggleSelect => {
                let Some(focus) = self.focus.clone() else {
                    return events;
                };
                match focus {
                    TableFocus::Header { column } => {
                        let next = table_header::cycle_sort(self.sort, column);
                        if next != self.sort {
                            self.sort = next;
                            events.push(TableEvent::SortChanged(next));
                        }
                    }
                    TableFocus::Body { row, column } => {
                        if key == TableKey::Activate {
                            events.push(TableEvent::CellAction(row, column, CellAction::Activate));
                        } else if self.toggle_selected(&row) {
                            events.push(TableEvent::SelectionChanged(self.selected.clone()));
                        }
                    }
                }
            }
            TableKey::SelectAll => {
                if self.select_all(rows) {
                    events.push(TableEvent::SelectionChanged(self.selected.clone()));
                }
            }
        }

        events
    }

    // -- pointer -----------------------------------------------------------

    /// Resolve a press on the header band.
    ///
    /// A resize press opens the gesture and emits nothing yet; a sort
    /// press cycles the sort and emits `SortChanged`.
    #[must_use]
    pub fn press_header(
        &mut self,
        specs: &[ColumnSpec],
        layout: &ColumnLayout,
        header: Rectangle,
        cursor: Point,
    ) -> Vec<TableEvent<Id>> {
        let mut events = Vec::new();
        match table_header::press_at(specs, layout, header, cursor) {
            HeaderPress::Resize { column } => {
                if let Some(spec) = specs.get(column) {
                    let id = spec.id;
                    self.resize.begin(layout.width_at(column), cursor.x, id);
                }
            }
            HeaderPress::Sort { column } => {
                if let Some(spec) = specs.get(column) {
                    self.focus_header(spec.id);
                    let next = table_header::cycle_sort(self.sort, spec.id);
                    if next != self.sort {
                        self.sort = next;
                        events.push(TableEvent::SortChanged(next));
                    }
                }
            }
            HeaderPress::Nothing => {}
        }
        events
    }

    /// Resolve a press on a body row.
    #[must_use]
    pub fn press_row(
        &mut self,
        rows: &[Id],
        column: ColId,
        row_index: usize,
        mods: Modifiers,
    ) -> Vec<TableEvent<Id>> {
        let mut events = Vec::new();
        let Some(row) = rows.get(row_index).cloned() else {
            return events;
        };
        self.focus_cell(rows, column, row_index);

        if mods.shift {
            self.apply_shift_range(rows);
            events.push(TableEvent::SelectionChanged(self.selected.clone()));
        } else if mods.ctrl {
            if self.toggle_selected(&row) {
                events.push(TableEvent::SelectionChanged(self.selected.clone()));
            }
        } else if self.select_single(&row) {
            events.push(TableEvent::SelectionChanged(self.selected.clone()));
        }
        events
    }

    // -- resize gesture ----------------------------------------------------

    /// Move the in-flight resize to `pointer_x`, emitting a live tick.
    ///
    /// The new width is clamped to the column's own `[min, max]` before
    /// it is emitted, so a host that writes the live value straight into
    /// its column spec can never see a width the column forbids.
    #[must_use]
    pub fn drag_resize(&mut self, specs: &[ColumnSpec], pointer_x: f32) -> Option<Live<f32>> {
        let (column_id, start_width, start_x) = self.resize.active_parts()?;
        let spec = specs.iter().find(|spec| spec.id == column_id)?;
        let bounds = spec.bounds();
        let raw = start_width + (pointer_x - start_x);
        let width = raw.max(bounds.floor).min(bounds.ceiling);
        self.resize.live(width)
    }

    /// Settle the resize; fires exactly once.
    #[must_use]
    pub fn release_resize(&mut self) -> Option<Commit<f32>> {
        self.resize.commit()
    }

    /// Abandon the resize, reverting to the width captured at press.
    #[must_use]
    pub fn cancel_resize(&mut self) -> Option<Cancel<f32>> {
        self.resize.cancel()
    }

    // -- geometry ----------------------------------------------------------

    /// The sticky header's band inside a viewport.
    #[must_use]
    pub fn header_rect(viewport: Rectangle) -> Rectangle {
        Rectangle {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width,
            height: table_header::HEADER_HEIGHT,
        }
    }

    /// The body viewport: everything below the header band.
    ///
    /// The header's height is subtracted rather than overlapped, so the
    /// row count a viewport realises is computed against the height the
    /// rows actually have and the first row is never hidden behind the
    /// sticky layer.
    #[must_use]
    pub fn body_viewport(viewport: Rectangle) -> Rectangle {
        Rectangle {
            y: viewport.y + table_header::HEADER_HEIGHT,
            height: (viewport.height - table_header::HEADER_HEIGHT).max(0.0),
            ..viewport
        }
    }

    /// Metrics for `row_count` rows in the body viewport.
    ///
    /// Built on the **body** height rather than the whole viewport, so
    /// the sticky header cannot make the table realise one row fewer
    /// than the space below it actually holds.
    #[must_use]
    pub fn metrics(row_count: usize, body: Rectangle) -> VirtualMetrics {
        VirtualMetrics::with_row_height(
            row_count,
            ROW_HEIGHT,
            body.height,
            virtual_list::DEFAULT_OVERSCAN_ROWS,
        )
    }

    /// The band a body row wears, given the table's state.
    #[must_use]
    pub fn row_band(&self, id: &Id, hovered: bool, zebra: bool) -> CellBand {
        table_cell::row_band(self.is_selected(id), hovered, zebra)
    }
}

/// A helper for the many call sites that need "the focused column, or
/// whatever is first" when focus has not been placed yet.
trait ColumnOrFirst {
    fn column_or_first(&self) -> ColId;
}

impl<Id: Clone> ColumnOrFirst for Option<&TableFocus<Id>> {
    fn column_or_first(&self) -> ColId {
        self.map(|focus| focus.column()).unwrap_or(0)
    }
}

/// The in-flight column resize.
///
/// Modelled on [`scroll_view::ThumbDrag`](super::scroll_view::ThumbDrag):
/// the width captured at press is the value a cancel restores, the live
/// ticks go through an open [`GestureSlot`] so exactly one commit can
/// follow, and a stray release with no open drag produces nothing.
#[derive(Clone, Debug)]
pub struct ResizeGesture {
    slot: GestureSlot<f32>,
    column: Option<ColId>,
    start_width: f32,
    start_x: f32,
    current: f32,
}

impl Default for ResizeGesture {
    fn default() -> Self {
        Self {
            slot: GestureSlot::new(),
            column: None,
            start_width: 0.0,
            start_x: 0.0,
            current: 0.0,
        }
    }
}

impl ResizeGesture {
    /// Whether a drag is open.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.slot.is_open() && self.column.is_some()
    }

    /// Open the drag on `column`, capturing the width and the x it
    /// started from.
    ///
    /// Both are captured at press because the drag's delta is measured
    /// from the grab point, not from the last tick: a host that dropped
    /// frames mid-drag would otherwise see the column jump.
    pub fn begin(
        &mut self,
        start_width: f32,
        start_x: f32,
        column: ColId,
    ) -> binding::GestureStart {
        self.column = Some(column);
        self.start_width = start_width;
        self.start_x = start_x;
        self.current = start_width;
        self.slot.begin(&start_width)
    }

    /// Emit a live tick at `width`.
    pub fn live(&mut self, width: f32) -> Option<Live<f32>> {
        let live = self.slot.live(width)?;
        self.current = width;
        Some(live)
    }

    /// Commit the current width, once.
    pub fn commit(&mut self) -> Option<Commit<f32>> {
        let commit = self.slot.commit(self.current)?;
        self.column = None;
        Some(commit)
    }

    /// Revert to the width captured at press.
    pub fn cancel(&mut self) -> Option<Cancel<f32>> {
        let outcome = self.slot.cancel()?;
        self.current = *outcome.restored();
        self.column = None;
        Some(outcome)
    }

    /// The column, the width it started at and the x it was grabbed at.
    fn active_parts(&self) -> Option<(ColId, f32, f32)> {
        if !self.is_active() {
            return None;
        }
        Some((self.column?, self.start_width, self.start_x))
    }

    /// The width the drag currently holds.
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// The column being resized, if a drag is open.
    #[must_use]
    pub fn column(&self) -> Option<ColId> {
        if self.is_active() { self.column } else { None }
    }
}

/// The position of `id` in `rows`.
fn position_of<Id: PartialEq>(rows: &[Id], id: &Id) -> Option<usize> {
    rows.iter().position(|row| row == id)
}

/// Apply the table's sort to `rows` without disturbing anything else.
///
/// The comparator is the host's, applied against the sorted column, and
/// the result is a **stable** permutation: rows that compare equal keep
/// their relative order in both directions. Descending is produced by
/// reversing the comparator, never by reversing an ascending result —
/// the latter flips ties, which would make equal rows trade places
/// depending on a control the user believes only orders them.
#[must_use]
pub fn sorted_rows<Id: Clone>(
    rows: &[Id],
    sort: Option<(ColId, SortDirection)>,
    mut compare: impl FnMut(&Id, &Id, ColId) -> Ordering,
) -> Vec<Id> {
    let mut ordered = rows.to_vec();
    let Some((column, direction)) = sort else {
        return ordered;
    };
    match direction {
        SortDirection::Ascending => ordered.sort_by(|a, b| compare(a, b, column)),
        SortDirection::Descending => ordered.sort_by(|a, b| compare(a, b, column).reverse()),
    }
    ordered
}

/// The horizontal translation a scrolled layer needs.
#[must_use]
pub fn scrolled_x(base: Rectangle, offset_x: f32) -> Rectangle {
    Rectangle {
        x: base.x - offset_x,
        ..base
    }
}

/// Whether the table must scroll sideways to show every column.
#[must_use]
pub fn needs_horizontal_scroll(layout: &ColumnLayout, viewport: Rectangle) -> bool {
    layout.total_width() > viewport.width + table_column::EPS
}

/// The band the header paints — the sticky layer's own fill.
#[must_use]
pub fn header_band() -> CellBand {
    CellBand::Raised
}

#[cfg(test)]
#[path = "table_view_tests.rs"]
mod tests;
