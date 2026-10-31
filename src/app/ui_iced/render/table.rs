//! Markdown table mapping and the iced `Table` widget.
//!
//! Two layers, split so the width solving is testable without building any
//! widget:
//!
//! * [`table_model`] runs the reused pure FTWA solver through
//!   [`crate::ui_iced::table_width`] and maps the solved geometry plus
//!   the parser's cell content into [`TableModel`] / [`TableCellModel`]
//!   descriptors.
//! * [`table_widget`] turns those descriptors into an
//!   `iced::widget::table`, pinning each column to the solved width.
//!
//! # Verified iced `Table` API (`iced_widget-0.14.2/src/table.rs`)
//!
//! * `table(columns, rows)` — `table.rs:18-28`; `Table::new` — `:82-146`
//! * `column(header, view)` — `:34-49`; the view is called once per row
//!   with the row's data, which is why the row payload carries its own
//!   index — the view signature has no row index of its own, and the
//!   striped-row parity needs it.
//! * `Column::width(Length)` — `:650-653`; a `Length::Fixed` column clamps
//!   its cell's layout limits to that width (`:302-311`) — **but only for a
//!   cell that is not fluid itself**, and never for the first column, which
//!   iced rewrites (see the traps below).
//! * `padding_x` / `separator_x` — `:162-190`; the inter-cell gap is
//!   `2 * padding + separator` (`:258-259`), the identity this adapter's
//!   [`COLUMN_SPACING`](crate::ui_iced::table_width::COLUMN_SPACING)
//!   mapping is built on.
//! * Style carries only the two separator colours — `:676-681` — so the
//!   perimeter stroke around the grid is painted by wrapping the table in a
//!   bordered container instead.
//!
//! # The two iced `Table` traps this adapter has to respect
//!
//! Both were found the hard way as defect #17, "a markdown table renders as an
//! empty table", and both are invisible to a test that only *constructs* the
//! widget — they show up in `Table::layout`, so `table_tests.rs` measures it.
//!
//! * **An all-`Fixed` column set is not honoured as written.** `Table::new`
//!   folds the column widths with `Length::enclose` (`:103`), which turns
//!   `Shrink` into a fluid length and nothing else
//!   (`iced_core-0.14.0/src/length.rs:61-66`), so a table whose columns are
//!   all `Fixed` folds to `Shrink` and `:129-133` then silently rewrites the
//!   **first** column to `Length::Fill`. That column is laid out in the second
//!   pass, whose resolved size comes from the cell's own intrinsic width
//!   (`:399-407`: `self.width.fluid()` is `Shrink` for both `Shrink` and
//!   `Fixed`, `iced_core-0.14.0/src/length.rs:51-56`), capped at the leftover
//!   space — so a cell that pins itself to its column's solved width keeps
//!   exactly that width, and [`scroll_width`] exists to make sure the leftover
//!   really is that wide.
//! * **A cell must never declare a fluid width under a fixed column.** The
//!   first pass skips every cell whose own width `is_fill()` (`:293`) without
//!   recording a measured maximum for its column, so `metrics.columns` stays
//!   all-zero. The second pass then lays each fixed column out under
//!   `Limits::width(Fixed(w))`, and `Limits::width` clamps the maximum **down**
//!   to the recorded zero (`iced_core-0.14.0/src/layout/limits.rs:60-65`):
//!   columns 1..n collapse to 0 px, column 0 swallows the viewport and the
//!   table reads as empty. [`cell_element`] therefore hands a cell its own
//!   column's `Length` rather than `Length::Fill`.
//!
//! A table with no columns must not reach the widget at all: `Table::layout`
//! divides by the column count (`:241`) and panics.
//!
//! # What the document model does not carry
//!
//! GFM's alignment row (`|:---|:--:|---:|`) is consumed by
//! `pulldown-cmark` and never reaches `RenderEvent::Table`
//! (`src/app/markdown/parser.rs:383-397` pushes the head row and the
//! body rows only). Column alignment is therefore **not available** — every
//! column is left-aligned. Flagged as spec drift per [RUST-041].
//!
//! Unit tests live in the sibling `table_tests.rs` sidecar.

use super::inline::{BODY_SIZE, SpanModel, spans_from_inline};
use crate::markdown::model::InlineElem;
use crate::markdown::table_layout::TableLayout;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;
use crate::ui_iced::table_width::{
    CELL_PADDING_X, CELL_PADDING_Y, IcedTextMeasurer, TablePadding, column_length, column_widths,
    solve_table_layout,
};
use hoarfrost::token::{shape, typography};
use iced::widget::{Space, container, scrollable, table, text};
use iced::{Background, Element, Length};

/// Border width of the table's perimeter frame, in logical pixels —
/// `border.hairline` (`UIF-TOK-003`). Value unchanged at `1`.
///
/// A `border_strong` line frames the whole grid.
pub const TABLE_PERIMETER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Row-index parity that receives the striped background.
///
/// Body rows where `index % 2 == 1` are striped.
pub const STRIPE_PARITY: usize = 1;

/// Height reserved for a blank cell, in logical pixels, so it still holds
/// its row open.
///
/// **Movement: 18 → 20 (+2).** `18` is `type.ui_compact`'s line height, but
/// these cells render their content at `type.body` — so a blank cell was
/// reserving a line box one rung smaller than the text beside it, and a row
/// mixing a blank cell with body text still had to grow. Deriving the
/// reservation from the rung the cell actually paints on
/// ([`BODY_SIZE`]'s own line height) makes the blank cell hold exactly the
/// line its neighbours occupy.
///
/// [`BODY_SIZE`]: super::inline::BODY_SIZE
pub const EMPTY_CELL_HEIGHT: f32 = typography::BODY.line_height;

/// One rendered table cell.
#[derive(Clone, Debug, PartialEq)]
pub struct TableCellModel {
    /// The cell's styled runs; empty for a blank cell.
    pub spans: Vec<SpanModel>,
    /// The width FTWA assigned to this cell's column.
    pub width: f32,
    /// Whether this cell belongs to the header row.
    pub is_header: bool,
}

/// A row handed to the iced `Table` column views.
///
/// `table::column`'s view closure receives only the row value
/// (`table.rs:34-49`), so the row index the stripe parity needs travels
/// inside the payload.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRowData {
    /// Zero-based index of this row among the **body** rows (0 = the first
    /// row after the header).
    pub index: usize,
    /// The row's cells, in column order.
    pub cells: Vec<TableCellModel>,
}

/// A solved markdown table, ready to render.
#[derive(Clone, Debug, PartialEq)]
pub struct TableModel {
    /// Rows in document order; row 0 is the header.
    pub rows: Vec<Vec<TableCellModel>>,
    /// Solved per-column widths, in column order.
    pub column_widths: Vec<f32>,
    /// Overflow fallback: the table cannot fit and needs a horizontal scroll.
    pub needs_horizontal_scroll: bool,
    /// Total solved width including the column gutters.
    pub total_width: f32,
}

impl TableModel {
    /// Number of columns the solve produced.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.column_widths.len()
    }

    /// The header row, `None` for an empty table.
    #[must_use]
    pub fn header_row(&self) -> Option<&[TableCellModel]> {
        self.rows.first().map(Vec::as_slice)
    }

    /// The data rows — everything after the header.
    #[must_use]
    pub fn body_rows(&self) -> &[Vec<TableCellModel>] {
        if self.rows.is_empty() {
            &[]
        } else {
            &self.rows[1..]
        }
    }

    /// The body rows tagged with their index, as the `Table` column views
    /// want them.
    #[must_use]
    pub fn body_rows_data(&self) -> Vec<TableRowData> {
        self.body_rows()
            .iter()
            .enumerate()
            .map(|(index, cells)| TableRowData {
                index,
                cells: cells.clone(),
            })
            .collect()
    }

    /// Whether the body row at `index` paints the stripe.
    #[must_use]
    pub fn is_striped(index: usize) -> bool {
        index % 2 == STRIPE_PARITY
    }
}

/// Solves and maps one parser table.
///
/// `cells` is the `RenderEvent::Table` payload. The solver runs over the
/// original `InlineElem` cells (it needs the raw text to measure); the
/// solved per-column width is then attached to every cell in that column
/// before the cell content is mapped to styled spans.
#[must_use]
pub fn table_model(
    cells: &[Vec<Vec<InlineElem>>],
    available_width: f32,
    strategy: DeficitStrategy,
    padding: TablePadding,
    measurer: &IcedTextMeasurer<'_>,
    palette: Palette,
) -> TableModel {
    let layout = solve_table_layout(cells, available_width, strategy, padding, measurer);
    from_layout(&layout, palette)
}

/// Maps an already-solved [`TableLayout`] onto the render model.
///
/// Split out so a caller that already holds a layout — a cache, a test
/// fixture — can reuse the mapping.
#[must_use]
pub fn from_layout(layout: &TableLayout, palette: Palette) -> TableModel {
    let widths = column_widths(layout);
    let strong = palette.text_strong;
    let rows = layout
        .rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let is_header = row_index == 0;
            row.iter()
                .enumerate()
                .map(|(column_index, cell)| {
                    let mut spans = spans_from_inline(&cell.content, BODY_SIZE, palette);
                    if is_header {
                        // GFM header cells are not bold in the AST, so the
                        // header is distinguished by colour.
                        for span in &mut spans {
                            span.color = strong;
                        }
                    }
                    TableCellModel {
                        width: widths.get(column_index).copied().unwrap_or(cell.width),
                        spans,
                        is_header,
                    }
                })
                .collect()
        })
        .collect();

    TableModel {
        rows,
        column_widths: widths,
        needs_horizontal_scroll: layout.needs_horizontal_scroll,
        total_width: layout.total_width,
    }
}

/// Renders a solved table.
///
/// The framed block is always allocated; an empty model renders an empty
/// frame. When [`TableModel::needs_horizontal_scroll`] is set the grid is
/// wrapped in a horizontal `scrollable` pinned to its solved width, the
/// overflow fallback for a table that cannot fit.
#[must_use]
pub fn table_widget(model: &TableModel) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let grid = grid_element(model);
    let framed: Element<'static, Message> = match grid {
        // `Scrollable::horizontal` sets `Direction::Horizontal` —
        // `iced_widget-0.14.2/src/scrollable.rs:133-135`.
        Some(grid) if model.needs_horizontal_scroll => scrollable(grid).horizontal().into(),
        Some(grid) => grid,
        None => Space::new().height(0.0).into(),
    };

    container(framed)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            border: iced::Border {
                color: palette.border_strong,
                width: TABLE_PERIMETER_WIDTH,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Builds the iced `Table` itself, `None` when the model has no columns.
///
/// Every column is pinned to its solved width **and every cell is pinned to
/// the same `Length`** — see the module docs for why the two must agree.
fn grid_element(model: &TableModel) -> Option<Element<'static, Message>> {
    if model.column_count() == 0 {
        return None;
    }

    let columns = (0..model.column_count())
        .map(|index| {
            let width = column_length(model.column_widths.get(index).copied().unwrap_or(0.0));
            let header = model
                .header_row()
                .and_then(|row| row.get(index))
                .cloned()
                .unwrap_or(TableCellModel {
                    spans: Vec::new(),
                    width: 0.0,
                    is_header: true,
                });
            table::column(
                cell_element(Some(&header), false, width),
                move |row: TableRowData| {
                    let striped = TableModel::is_striped(row.index);
                    cell_element(row.cells.get(index), striped, width)
                },
            )
            .width(width)
        })
        .collect::<Vec<_>>();

    let grid = table::table(columns, model.body_rows_data())
        .padding_x(CELL_PADDING_X)
        .padding_y(CELL_PADDING_Y)
        .separator_x(0.0)
        .separator_y(0.0)
        .width(if model.needs_horizontal_scroll {
            scroll_width(model)
        } else {
            Length::Shrink
        });

    Some(grid.into())
}

/// The width the grid is pinned to when it needs the horizontal-scroll
/// fallback.
///
/// [`TableModel::total_width`] is the solver's own accounting — columns plus
/// their gutters — and excludes the two outer padding bands the iced `Table`
/// paints around the grid. Pinning to the bare `total_width` would leave the
/// first column `2 * CELL_PADDING_X` narrower than it was solved for, because
/// iced hands that column whatever the fixed table width has left over
/// (module docs), so the bands are added back here.
#[must_use]
pub fn scroll_width(model: &TableModel) -> Length {
    Length::Fixed((model.total_width + 2.0 * CELL_PADDING_X).max(0.0))
}

/// Renders one cell.
///
/// A cell with no content still allocates [`EMPTY_CELL_HEIGHT`] so a blank
/// cell holds its row open.
///
/// `column` is the `Length` the cell's own column is pinned to, and the cell
/// declares exactly the same one: a cell that declared `Length::Fill` under a
/// fixed column was silently skipped by iced's first layout pass and collapsed
/// the whole table (defect #17 — see the module docs).
#[must_use]
pub fn cell_element(
    cell: Option<&TableCellModel>,
    striped: bool,
    column: Length,
) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();
    let spans: &[SpanModel] = cell.map_or(&[][..], |cell| cell.spans.as_slice());

    let content: Element<'static, Message> = if spans.is_empty() {
        text(String::new()).size(BODY_SIZE).into()
    } else {
        super::inline::rich_text_element(spans, None)
    };

    container(content)
        .width(column)
        .height(if spans.is_empty() {
            Length::Fixed(EMPTY_CELL_HEIGHT)
        } else {
            Length::Shrink
        })
        .style(move |_theme| container::Style {
            background: striped.then_some(Background::Color(palette.surface_variant)),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
