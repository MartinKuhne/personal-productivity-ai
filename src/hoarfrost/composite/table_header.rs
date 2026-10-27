//! Header chrome for the structured grid: the sticky title row, the sort
//! indicator, the column-resize grab, and the hit-test separation that
//! keeps the last two from fighting (`UIF-COMP-007`).
//!
//! # The collision this module exists to settle
//!
//! §3B.7 asks for two things in the same 32px band: the whole header cell
//! is a **sort click target**, and a `10px` handle **centred on the
//! divider** starts a resize. Those two regions overlap by five pixels on
//! either side of every divider, and the overlap is exactly where a user
//! aiming for one lands on the other. A table that resolves the overlap by
//! whichever region was registered last in the widget tree re-sorts the
//! user's data when they meant to widen a column — the worst possible
//! failure, because it is silent and destroys the ordering they were
//! looking at.
//!
//! The rule here is **resize wins, and it is checked first**. It is
//! checked first because the resize band is the smaller and more precisely
//! aimed target: a user who puts the cursor within five pixels of a
//! divider meant the divider. A user who meant to sort aimed at the title,
//! which is nowhere near the divider. The asymmetry is what makes
//! resize-first correct rather than merely convenient.
//!
//! Two consequences follow, and both are tested:
//!
//! * The **sort target is the header cell minus the resize band**, so the
//!   two regions never overlap in the model either. A hit test cannot
//!   disagree with what is drawn, because the drawn regions *are* the hit
//!   regions.
//! * The handle **straddles** the divider, reaching five pixels into the
//!   next column's cell. A naive per-column hit test would hand that strip
//!   to the neighbour and sort it. [`press_at`] therefore scans every
//!   divider before it scans any sort target, across the whole header
//!   rather than within one cell.
//!
//! # What the spec left open
//!
//! §3B.7 gives the band height, the grab width and the indicator glyphs,
//! and stops there. Recorded decisions:
//!
//! * **The final column is never resizable.** Its trailing edge is the
//!   table's own border, not a divider between two columns; dragging it
//!   would move the container edge, which the table does not own. The
//!   last column can still be narrowed by dragging the divider before it.
//! * **The indicator is `size.icon.sm` (`14px`) inset `space.xs` from the
//!   trailing edge of the sort target**, not from the cell edge — so it
//!   can never be painted inside the resize band.
//! * **The title is `type.caption` at `Weight::SemiBold`.** §3B.7 prints
//!   "12px SemiBold"; G-12 notes the scale has no 12px SemiBold *emphasis*
//!   rung, but `CAPTION` lists SemiBold among its `allowed_weights`, so
//!   this is a permitted weight on the rung rather than a composition
//!   outside it — the same reading that lets a menu section heading wear
//!   it (§4.8.1).
//! * **A press commits to its target and is never re-tested mid-drag.**
//!   See [`HeaderPress`] and the freeze rule there.
//!
//! Unit tests live in the sibling `table_header_tests.rs` sidecar.

use crate::composite::table_column::{CellAlign, ColId, ColumnLayout, ColumnSpec};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::token::{shape, size, space, typography};
use iced::{Color, Point, Rectangle};

/// The sticky header's height — §3B.7's `height: 32px`.
pub const HEADER_HEIGHT: f32 = size::CONTROL_MD;

/// The resize grab's width — §3B.7's `10px (size.splitter.grab)`.
pub const RESIZE_GRAB: f32 = size::SPLITTER_GRAB;

/// How far the handle reaches on either side of the divider.
pub const RESIZE_REACH: f32 = RESIZE_GRAB / 2.0;

/// The header's bottom border weight — §3B.7's `1px border.strong`.
pub const HEADER_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The header title's horizontal padding — the same `space.md` a body
/// cell uses, so a heading lines up with the column it heads.
pub const CELL_TITLE_PADDING_X: f32 = space::MD;

/// The sort indicator's box — [`size::ICON_SM`].
///
/// The spec gives the glyph but not its size; `icon.sm` is the rung the
/// rest of the chrome uses for an inline mark beside a caption-weight
/// label.
pub const SORT_INDICATOR_SIZE: f32 = size::ICON_SM;

/// The gap between the title and the sort indicator.
pub const SORT_INDICATOR_GAP: f32 = space::XS;

/// The header title's type rung.
pub const TITLE_RUNG: &typography::TypeRung = &typography::CAPTION;

/// The header title's weight — §3B.7's "SemiBold".
pub const TITLE_WEIGHT: typography::Weight = typography::Weight::SemiBold;

/// The direction a column is sorted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Smallest first, `▲`.
    Ascending,
    /// Largest first, `▼`.
    Descending,
}

impl SortDirection {
    /// The glyph §3B.7 prints for this direction.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Ascending => "▲",
            Self::Descending => "▼",
        }
    }

    /// The gallery slug for this direction.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Ascending => "asc",
            Self::Descending => "desc",
        }
    }
}

/// What a press on the header committed to.
///
/// The freeze rule: this is resolved **once, at press**, and the gesture
/// that follows acts on the answer without ever re-testing the cursor
/// against the header. A drag that re-resolved on every move would flip
/// between resizing and sorting as the cursor crossed the five-pixel
/// boundary, and a resize that drifted off its handle would turn into a
/// sort of the column the user was trying to widen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPress {
    /// The divider after column `column` — start a resize.
    Resize {
        /// The column whose trailing divider was grabbed.
        column: usize,
    },
    /// Column `column`'s title area — cycle its sort.
    Sort {
        /// The column whose sort was clicked.
        column: usize,
    },
    /// Nothing actionable was pressed.
    Nothing,
}

/// Whether column `index` may be resized.
///
/// The final column is excluded: its trailing edge is the table's border,
/// not a divider between two columns.
#[must_use]
pub fn column_is_resizable(specs: &[ColumnSpec], index: usize) -> bool {
    index + 1 < specs.len() && specs.get(index).is_some_and(|spec| spec.resizable)
}

/// Whether column `index` is sortable.
#[must_use]
pub fn column_is_sortable(specs: &[ColumnSpec], index: usize) -> bool {
    specs.get(index).is_some_and(|spec| spec.sortable)
}

/// The full rectangle of one header cell.
#[must_use]
pub fn header_cell_rect(layout: &ColumnLayout, index: usize, header: Rectangle) -> Rectangle {
    Rectangle {
        x: layout.x_at(index),
        y: header.y,
        width: layout.width_at(index),
        height: header.height,
    }
}

/// The divider's x position for column `index`'s trailing edge.
#[must_use]
pub fn divider_x(layout: &ColumnLayout, index: usize) -> f32 {
    layout.divider_x(index)
}

/// The resize handle's rectangle — centred on the divider.
///
/// The handle straddles the divider and therefore reaches
/// [`RESIZE_REACH`] into the following column's cell. That is what §3B.7
/// asks for and it is the whole reason the hit test has to be
/// header-wide rather than per-cell.
#[must_use]
pub fn resize_handle_rect(layout: &ColumnLayout, index: usize, header: Rectangle) -> Rectangle {
    let boundary = divider_x(layout, index);
    Rectangle {
        x: boundary - RESIZE_REACH,
        y: header.y,
        width: RESIZE_GRAB,
        height: header.height,
    }
}

/// The sort click target for column `index`.
///
/// The header cell, inset by [`RESIZE_REACH`] on every side a live
/// handle covers. A column whose own trailing divider is grabbable loses
/// five pixels at the right; a column whose *leading* edge is another
/// column's grabbable divider loses five at the left. The result is that
/// sort targets and handles are pairwise disjoint across the whole
/// header, so what is drawn and what is clickable are the same regions
/// and no ordering is load-bearing for correctness.
///
/// A column with no live handle on either side keeps its whole cell.
#[must_use]
pub fn sort_target_rect(
    specs: &[ColumnSpec],
    layout: &ColumnLayout,
    index: usize,
    header: Rectangle,
) -> Rectangle {
    let cell = header_cell_rect(layout, index, header);
    let left = if index > 0 && column_is_resizable(specs, index - 1) {
        RESIZE_REACH
    } else {
        0.0
    };
    let right = if column_is_resizable(specs, index) {
        RESIZE_REACH
    } else {
        0.0
    };
    Rectangle {
        x: cell.x + left,
        width: (cell.width - left - right).max(0.0),
        ..cell
    }
}

/// Where the sort indicator is drawn inside a sort target.
///
/// Pinned to the target's trailing edge — never the cell's — so the mark
/// cannot be painted inside the resize band. A target narrower than the
/// indicator clamps to the target's leading edge rather than pushing the
/// mark out of the header.
#[must_use]
pub fn sort_indicator_rect(target: Rectangle) -> Rectangle {
    let x = (target.x + target.width - SORT_INDICATOR_SIZE - SORT_INDICATOR_GAP).max(target.x);
    Rectangle {
        x,
        y: target.y + (target.height - SORT_INDICATOR_SIZE) / 2.0,
        width: SORT_INDICATOR_SIZE,
        height: SORT_INDICATOR_SIZE,
    }
}

/// Resolve a press against the whole header.
///
/// Every divider is scanned for a handle **before** any sort target is
/// scanned, which is the resize-first rule made executable. See the
/// [module docs](self) for why the order is the behaviour.
#[must_use]
pub fn press_at(
    specs: &[ColumnSpec],
    layout: &ColumnLayout,
    header: Rectangle,
    cursor: Point,
) -> HeaderPress {
    if !contains(header, cursor) {
        return HeaderPress::Nothing;
    }

    for index in 0..layout.len() {
        if column_is_resizable(specs, index)
            && contains(resize_handle_rect(layout, index, header), cursor)
        {
            return HeaderPress::Resize { column: index };
        }
    }

    for index in 0..layout.len() {
        if column_is_sortable(specs, index)
            && contains(sort_target_rect(specs, layout, index, header), cursor)
        {
            return HeaderPress::Sort { column: index };
        }
    }

    HeaderPress::Nothing
}

/// The direction column `index` is currently sorted in, if any.
#[must_use]
pub fn sort_indicator(
    sort: Option<(ColId, SortDirection)>,
    column: ColId,
) -> Option<SortDirection> {
    sort.filter(|(id, _)| *id == column)
        .map(|(_, direction)| direction)
}

/// Apply one click to the sort state.
///
/// §3B.7's cycle is `Ascending -> Descending -> None`. Clicking a
/// different column restarts at `Ascending` rather than continuing the
/// previous column's cycle, because a user switching sort columns wants
/// the ordinary first click, not the second half of a cycle they were
/// never on.
#[must_use]
pub fn cycle_sort(
    current: Option<(ColId, SortDirection)>,
    column: ColId,
) -> Option<(ColId, SortDirection)> {
    match current {
        Some((id, SortDirection::Ascending)) if id == column => {
            Some((column, SortDirection::Descending))
        }
        Some((id, SortDirection::Descending)) if id == column => None,
        _ => Some((column, SortDirection::Ascending)),
    }
}

/// The header's bottom border rectangle.
#[must_use]
pub fn bottom_border_layer_rect(header: Rectangle) -> Rectangle {
    Rectangle {
        x: header.x,
        y: header.y + header.height - HEADER_BORDER_WIDTH,
        width: header.width,
        height: HEADER_BORDER_WIDTH,
    }
}

/// The header band's fill — §3B.7's `surface.raised`.
#[must_use]
pub fn band_colour(palette: &Palette) -> Color {
    palette.surface_raised
}

/// The header title's colour — §3B.7's `text.strong`.
#[must_use]
pub fn title_colour(palette: &Palette) -> Color {
    palette.text_strong
}

/// The sort indicator's colour — §3B.7's `accent.text`.
#[must_use]
pub fn indicator_colour(palette: &Palette) -> Color {
    palette.accent_text
}

/// The header's bottom border colour — §3B.7's `border.strong`.
#[must_use]
pub fn border_colour(palette: &Palette) -> Color {
    palette.border_strong
}

/// The measured contrast of the sort indicator against the header band.
#[must_use]
pub fn indicator_contrast(palette: &Palette) -> f32 {
    contrast_ratio(indicator_colour(palette), band_colour(palette))
}

/// The measured contrast of the header's bottom border against the band.
#[must_use]
pub fn border_contrast(palette: &Palette) -> f32 {
    contrast_ratio(border_colour(palette), band_colour(palette))
}

/// Where content of `content_width` begins inside a header cell.
///
/// Header titles follow the column's own alignment, so a numeric column's
/// heading sits over its digits.
#[must_use]
pub fn title_x(cell: Rectangle, content_width: f32, align: CellAlign) -> f32 {
    let inner_width = (cell.width - CELL_TITLE_PADDING_X * 2.0).max(0.0);
    align.place(cell.x + CELL_TITLE_PADDING_X, inner_width, content_width)
}

/// Whether `rect` covers `point`, half-open on the trailing edges.
fn contains(rect: Rectangle, point: Point) -> bool {
    point.x >= rect.x
        && point.x < rect.x + rect.width
        && point.y >= rect.y
        && point.y < rect.y + rect.height
}

#[cfg(test)]
#[path = "table_header_tests.rs"]
mod tests;
