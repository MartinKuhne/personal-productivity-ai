//! Framework-owned cell geometry and paint resolution for the structured
//! grid (`UIF-COMP-007`, decision **D-8**).
//!
//! Decision **D-8** puts the table's *cell* rendering in the framework,
//! not merely the container around it. That distinction matters because a
//! table whose cells are laid out by the host is a table whose alignment,
//! padding and grid lines differ on every surface that embeds one — and
//! `fastmd`'s own Tools dialog is the migration target that would
//! inherit the inconsistency. Everything a cell needs to be drawn
//! correctly is therefore computed here from the
//! [`ColumnLayout`] the pure resolver
//! produced, and the host supplies content and nothing else.
//!
//! # What is decided here
//!
//! * **Padding.** §3B.7 prints `[4px, 12px]` for a body row. The pair
//!   is read as `[vertical, horizontal]`, the CSS order the figure's
//!   shape implies, giving [`CELL_PADDING_Y`] = `space.xs` and
//!   [`CELL_PADDING_X`] = `space.md`.
//! * **Alignment.** [`CellAlign::place`] does the placement; this module
//!   only hands it the box. Overflowing content is **clipped at the
//!   leading edge of the box rather than shifted toward the alignment**,
//!   because centring or right-aligning a string that does not fit hides
//!   its beginning — the wrong end to lose.
//! * **Grid-line legibility.** §3B.7 names `border.subtle` for the
//!   column dividers and row separators and prices what happens when a
//!   row is painted on a band: the hairline promotes to `border.strong`
//!   whenever its contrast against the band drops below the `1.15:1`
//!   visibility floor. Recomputed from raw sRGB, `border.subtle` against
//!   `surface.hover` is **1.1012:1** and against `surface.active`
//!   **1.0314:1**, so both promote; against `surface.base` (**1.3495:1**)
//!   and `surface.raised` (**1.2691:1**) it stays. That is
//!   [`MIN_GRID_LINE_CONTRAST_ON_BAND`], and the promotion is measured
//!   rather than asserted: see [`grid_line_contrast`].
//!
//! The floor is deliberately low: a grid line is a separator, not a
//! border, and promoting it to a full 3:1 chrome requirement would turn
//! every table into a wireframe cage.
//!
//! [`ColumnLayout`]: crate::composite::table_column::ColumnLayout
//!
//! Unit tests live in the sibling `table_cell_tests.rs` sidecar.

use crate::composite::table_column::{CellAlign, ColumnLayout};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::focus::{FocusIndicator, resolve_inset_outline_for_background};
use iced::{Color, Rectangle};

/// Horizontal padding inside a body cell — the `12px` of §3B.7's
/// `[4px, 12px]`.
pub const CELL_PADDING_X: f32 = space::MD;

/// Vertical padding inside a body cell — the `4px` of §3B.7's
/// `[4px, 12px]`.
pub const CELL_PADDING_Y: f32 = space::XS;

/// The minimum body row height — §3B.7's `min_height: 32px`.
pub const BODY_ROW_MIN_HEIGHT: f32 = size::CONTROL_MD;

/// The weight of every grid line — §3B.7's `1px border.subtle`.
pub const GRID_LINE_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The lowest contrast a grid line may have against the band it is
/// painted over before it is promoted to `border.strong`.
///
/// **A spec-prescribed figure.** §3B.7 states the floor directly —
/// "automatically promote to `border.strong` … whenever the resting
/// hairline contrast against the row band drops below the `1.15:1`
/// visibility floor" — and §2.5's `RestingDivider` note repeats it. The
/// value is therefore read off the spec rather than chosen here.
///
/// It is deliberately low because a grid line is a separator, not a
/// border: promoting it to the full `3:1` chrome requirement would turn
/// every table into a wireframe cage. It sits between the two bands the
/// hairline survives on (`surface.base` at `1.3495`, `surface.raised` at
/// `1.2691`, both kept) and the two it is lost under (`surface.hover` at
/// `1.1012`, `surface.active` at `1.0314`, both promoted) — exactly the
/// four figures §3B.7 prints.
pub const MIN_GRID_LINE_CONTRAST_ON_BAND: f32 = 1.15;

/// The surface a cell is painted on.
///
/// Four bands, because that is how many a row can wear: the resting
/// field, the zebra alternate, the pointer, and the selection. The
/// header's band is `surface.raised` and is owned by
/// [`table_header`](super::table_header), which draws a heavier bottom
/// border over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellBand {
    /// `surface.base` — the table's resting field.
    Base,
    /// `surface.raised` — the zebra alternate.
    Raised,
    /// `surface.hover` — the pointer is on the row.
    Hover,
    /// `surface.active` — the row is selected.
    Selected,
}

impl CellBand {
    /// The fill this band paints.
    #[must_use]
    pub fn colour(self, palette: &Palette) -> Color {
        match self {
            Self::Base => palette.surface_base,
            Self::Raised => palette.surface_raised,
            Self::Hover => palette.surface_hover,
            Self::Selected => palette.surface_active,
        }
    }

    /// The gallery slug for this band.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Raised => "raised",
            Self::Hover => "hover",
            Self::Selected => "selected",
        }
    }

    /// Whether this band is one the row itself paints, as opposed to the
    /// table's resting field.
    #[must_use]
    pub const fn is_painted(self) -> bool {
        !matches!(self, Self::Base)
    }
}

/// Resolve the band a row wears.
///
/// The precedence is the crate's own, restated from
/// [`tree_row::resolve_style`](super::tree_row::resolve_style) rather
/// than reinvented: **selection outranks the pointer**, because selection
/// is a fact about the model and the pointer is a fact about the moment.
/// The zebra stripe is last, and is the only thing that disappears when
/// either of the two above it arrives — a striped table must not be able
/// to argue that a selected row is not selected.
#[must_use]
pub fn row_band(selected: bool, hovered: bool, zebra: bool) -> CellBand {
    if selected {
        CellBand::Selected
    } else if hovered {
        CellBand::Hover
    } else if zebra {
        CellBand::Raised
    } else {
        CellBand::Base
    }
}

/// The text colour a cell paints on a band.
///
/// §3B.7 prices each band's foreground directly: resting rows on
/// `surface.base` / `surface.raised` take `text.primary`, `Hovered` rows
/// on `surface.hover` take `text.strong`, and `Selected` rows on
/// `surface.active` take `text.strong` (`14.29:1`). The hover promotion
/// is the spec's own, not a carry-over — a hovered row is the one the
/// pointer is on, and it is the loudest text in the table.
#[must_use]
pub fn foreground_colour(band: CellBand, palette: &Palette) -> Color {
    match band {
        CellBand::Base | CellBand::Raised => palette.text_primary,
        CellBand::Hover | CellBand::Selected => palette.text_strong,
    }
}

/// The grid-line colour a cell paints on a band.
///
/// `border.subtle` where it can still be seen, `border.strong` where it
/// cannot. See [`MIN_GRID_LINE_CONTRAST_ON_BAND`].
#[must_use]
pub fn grid_line_colour(band: CellBand, palette: &Palette) -> Color {
    let resting = palette.border_subtle;
    if contrast_ratio(resting, band.colour(palette)) >= MIN_GRID_LINE_CONTRAST_ON_BAND {
        resting
    } else {
        palette.border_strong
    }
}

/// The measured contrast of the grid line actually chosen for a band.
///
/// Exposed so the gallery can report the number rather than the token:
/// the claim under test is "the divider is visible", and only the ratio
/// says so.
#[must_use]
pub fn grid_line_contrast(band: CellBand, palette: &Palette) -> f32 {
    contrast_ratio(grid_line_colour(band, palette), band.colour(palette))
}

/// The outline a selected row is drawn with — §3B.7's
/// `1px accent.border`.
///
/// `accent.border` against `surface.active` measures **4.7922:1**, which
/// clears the crate's `3.0` chrome floor, so unlike the grid line this
/// one needs no promotion.
#[must_use]
pub fn selection_outline_colour(palette: &Palette) -> Color {
    palette.accent_border
}

/// The focus ring for a cell on a band.
///
/// The inset placement, because a table cell has no room outside its own
/// rectangle — an outer ring would be drawn over the neighbouring
/// column and break the very alignment the layout guarantees.
#[must_use]
pub fn focus_indicator(band: CellBand, palette: &Palette) -> FocusIndicator {
    resolve_inset_outline_for_background(band.colour(palette), palette)
}

/// A cell's fully resolved paint.
///
/// Every channel is a colour this module can be measured against the
/// band, so a gallery cell can assert the whole of a cell's appearance
/// without the renderer being involved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellStyle {
    /// The band this cell sits on.
    pub band: CellBand,
    /// The band's fill.
    pub background: Color,
    /// The text colour.
    pub foreground: Color,
    /// The grid-line colour.
    pub grid_line: Color,
    /// The selected-row outline, transparent on an unselected band.
    pub outline: Color,
    /// The focus ring.
    pub focus: FocusIndicator,
}

impl CellStyle {
    /// Resolve every channel for a band.
    #[must_use]
    pub fn resolve(band: CellBand, palette: &Palette) -> Self {
        Self {
            band,
            background: band.colour(palette),
            foreground: foreground_colour(band, palette),
            grid_line: grid_line_colour(band, palette),
            outline: if band == CellBand::Selected {
                selection_outline_colour(palette)
            } else {
                Color::TRANSPARENT
            },
            focus: focus_indicator(band, palette),
        }
    }
}

/// The painted rectangle of one cell.
///
/// The cell owns the full column width — padding is subtracted inside it
/// rather than carved out here — so a row of cells tiles the table with
/// no gaps for a grid line to fall into.
#[must_use]
pub fn cell_rect(layout: &ColumnLayout, column: usize, row: Rectangle) -> Rectangle {
    Rectangle {
        x: layout.x_at(column),
        y: row.y,
        width: layout.width_at(column),
        height: row.height,
    }
}

/// The content box inside a cell, with [`CELL_PADDING_X`] and
/// [`CELL_PADDING_Y`] removed.
///
/// A column narrower than twice the horizontal padding clamps to a
/// zero-width box rather than reporting a negative one: a negative width
/// handed to a layout engine inverts the box and produces geometry that
/// is not merely wrong but wrong in a way that moves other things.
#[must_use]
pub fn content_rect(cell: Rectangle) -> Rectangle {
    let width = (cell.width - CELL_PADDING_X * 2.0).max(0.0);
    let height = (cell.height - CELL_PADDING_Y * 2.0).max(0.0);
    Rectangle {
        x: cell.x + CELL_PADDING_X,
        y: cell.y + CELL_PADDING_Y,
        width,
        height,
    }
}

/// Where content of `content_width` begins inside a cell.
#[must_use]
pub fn content_x(cell: Rectangle, content_width: f32, align: CellAlign) -> f32 {
    let box_rect = content_rect(cell);
    align.place(box_rect.x, box_rect.width, content_width)
}

/// Whether content of `content_width` is clipped by its cell.
#[must_use]
pub fn is_truncated(cell: Rectangle, content_width: f32) -> bool {
    content_width > content_rect(cell).width + f32::EPSILON
}

/// The vertical centre of a cell's content box — where an icon or a
/// single line of text is anchored.
#[must_use]
pub fn content_centre_y(cell: Rectangle) -> f32 {
    let box_rect = content_rect(cell);
    box_rect.y + box_rect.height / 2.0
}

/// The row separator drawn on a cell's bottom edge.
///
/// Returned as a rectangle rather than a stroke so the caller can batch
/// it with the rest of the table's lines without knowing the weight.
#[must_use]
pub fn bottom_separator(cell: Rectangle) -> Rectangle {
    Rectangle {
        x: cell.x,
        y: cell.y + cell.height - GRID_LINE_WIDTH,
        width: cell.width,
        height: GRID_LINE_WIDTH,
    }
}

/// The column divider drawn on a cell's trailing edge.
///
/// `None` for the final column: the table's own right edge is the
/// container's border, and drawing a divider there too would double the
/// stroke at the one place a user reads as the table's boundary.
#[must_use]
pub fn trailing_divider(cell: Rectangle, is_last: bool) -> Option<Rectangle> {
    if is_last || cell.width < GRID_LINE_WIDTH {
        return None;
    }
    Some(Rectangle {
        x: cell.x + cell.width - GRID_LINE_WIDTH,
        y: cell.y,
        width: GRID_LINE_WIDTH,
        height: cell.height,
    })
}

#[cfg(test)]
#[path = "table_cell_tests.rs"]
mod tests;
