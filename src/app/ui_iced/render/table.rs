//! Markdown table mapping and the iced `Table` widget ([TASK-015]).
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
//! * `Column::width(Length)` — `:650-653`; a `Length::Fixed` column
//!   constrains its cell's layout max width to exactly that value
//!   (`:302-311`), which is what keeps the FTWA assignment honoured.
//! * `padding_x` / `separator_x` — `:162-190`; the inter-cell gap is
//!   `2 * padding + separator` (`:258-259`), the identity this adapter's
//!   [`COLUMN_SPACING`](crate::ui_iced::table_width::COLUMN_SPACING)
//!   mapping is built on.
//! * Style carries only the two separator colours — `:676-681` — so the
//!   the legacy UI table's perimeter stroke is painted by wrapping the table in a
//!   bordered container instead.
//!
//! # What the document model does not carry
//!
//! GFM's alignment row (`|:---|:--:|---:|`) is consumed by
//! `pulldown-cmark` and never reaches `RenderEvent::Table`
//! (`src/app/markdown/parser.rs:383-397` pushes the head row and the
//! body rows only). Column alignment is therefore **not ported** — every
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
use iced::widget::{container, scrollable, table, text};
use iced::{Background, Element, Length};

/// Border width of the table's perimeter frame.
///
/// Ported from `TABLE_PERIMETER_STROKE`
/// (`src/app/ui/render/table/configured.rs:36-39`), which drew a 1 px
/// `border_strong` line around the whole grid.
pub const TABLE_PERIMETER_WIDTH: f32 = 1.0;

/// Row-index parity that receives the striped background.
///
/// the legacy UI striped `row_idx % 2 == 1`
/// (`src/app/ui/render/table/configured.rs:77`).
pub const STRIPE_PARITY: usize = 1;

/// Height reserved for a blank cell, so it still holds its row open.
///
/// The legacy UI cell used `ui.text_style_height(&TextStyle::Body)` for this
/// (`src/app/ui/render/table/cell.rs:81`).
pub const EMPTY_CELL_HEIGHT: f32 = 18.0;

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
    /// §3.6 fallback: the table cannot fit and needs a horizontal scroll.
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
                        // header is distinguished by colour — the iced
                        // stand-in for the legacy UI's header emphasis.
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
/// wrapped in a horizontal `scrollable` pinned to its solved width — the
/// iced equivalent of the §3.6 fallback in
/// `src/app/ui/render/table/configured.rs:122-144`.
#[must_use]
pub fn table_widget(model: &TableModel) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let columns = (0..model.column_count())
        .map(|index| {
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
                cell_element(Some(&header), false),
                move |row: TableRowData| {
                    let striped = TableModel::is_striped(row.index);
                    cell_element(row.cells.get(index), striped)
                },
            )
            .width(column_length(
                model.column_widths.get(index).copied().unwrap_or(0.0),
            ))
        })
        .collect::<Vec<_>>();

    let grid = table::table(columns, model.body_rows_data())
        .padding_x(CELL_PADDING_X)
        .padding_y(CELL_PADDING_Y)
        .separator_x(0.0)
        .separator_y(0.0)
        .width(if model.needs_horizontal_scroll {
            Length::Fixed(model.total_width.max(0.0))
        } else {
            Length::Shrink
        });

    let framed: Element<'static, Message> = if model.needs_horizontal_scroll {
        // `Scrollable::horizontal` sets `Direction::Horizontal` —
        // `iced_widget-0.14.2/src/scrollable.rs:133-135`.
        scrollable(grid).horizontal().into()
    } else {
        grid.into()
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

/// Renders one cell.
///
/// A cell with no content allocates [`EMPTY_CELL_HEIGHT`] so a blank cell
/// still holds its row open — the same reason the legacy UI cell used
/// `allocate_space` rather than nothing (`cell.rs:83-94`).
#[must_use]
pub fn cell_element(cell: Option<&TableCellModel>, striped: bool) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();
    let spans: &[SpanModel] = cell.map_or(&[][..], |cell| cell.spans.as_slice());

    let content: Element<'static, Message> = if spans.is_empty() {
        text(String::new()).size(BODY_SIZE).into()
    } else {
        super::inline::rich_text_element(spans, None)
    };

    container(content)
        .width(Length::Fill)
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
