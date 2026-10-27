//! Gallery cells for the structured grid (`UIF-COMP-007`, decision **D-8**).
//!
//! These live outside `gallery.rs` because the matrix file is already past
//! the module size limit `RUST-053` sets. They also are not the same shape
//! as the cells beside them: a table's interesting claims are about
//! *three separate resolutions* that have to agree at the pixel, and none
//! of them is a picture.
//!
//! * **The band.** Decision **D-8** puts cell rendering in the framework,
//!   so the gallery has to show what a cell resolves to on each of the
//!   four bands - and specifically that the grid line is *promoted* on the
//!   two bands where `border.subtle` would vanish under it.
//! * **The width.** The column resolver is a pure function of the specs
//!   plus the available width, so the gallery renders one cell per regime
//!   and reports the widths it produced. The numbers are the evidence:
//!   `40 / 90 / 80` at a deficit is not something a screenshot can
//!   establish.
//! * **The header.** The sort target and the resize handle are disjoint
//!   regions, and the gallery draws both so the gap between them is
//!   visible rather than implied.
//!
//! The grid line is deliberately **not** in a cell's `audit` list. It is a
//! hairline separator that carries no information on its own - the band
//! and the text do - so `UIF-TOK-006` rule 2's `3.0:1` floor is not the
//! rule it answers to. This crate holds it to its own documented floor
//! ([`table_cell::MIN_GRID_LINE_CONTRAST_ON_BAND`]) and reports the
//! measured ratio as a number instead, so the disagreement stays visible
//! rather than being either hidden or forced into a role it fails.
//!
//! Every cell is built from the same pure resolvers the renderer calls, so
//! a cell is a measurement rather than a mock-up.
//!
//! Unit tests live in the sibling `gallery_table_tests.rs` sidecar.

use crate::composite::table_cell::{self, CellBand, CellStyle};
use crate::composite::table_column::{ColumnLayout, ColumnSpec, resolve_widths};
use crate::composite::table_header::{self, SortDirection};
use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame};
use crate::token::color::Palette;
use crate::token::{shape, size, space};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// A table wide enough for three columns to resolve without scrolling.
const TABLE_WIDTH: f32 = space::LG * 30.0;

/// The body viewport height a cell is sampled at.
const BODY_HEIGHT: f32 = size::CONTROL_MD * 6.0;

/// The three columns every cell here is built from.
///
/// Chosen so the four width regimes land on round, checkable numbers:
/// the floors sum to `180`, the targets to `240`.
fn regime_specs() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::proportional(0, "A", 1.0)
            .with_min(40.0)
            .with_max(400.0),
        ColumnSpec::content(1, "B", 120.0).with_min(60.0),
        ColumnSpec::fixed(2, "C", 80.0),
    ]
}

/// A transparent, hairline-stroked marker for a band the resolver
/// reserved but does not paint.
fn marker(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: iced::Border {
            color: colour,
            width: shape::BORDER_HAIRLINE,
            radius: 0.0_f32.into(),
        },
    }
}

/// A filled layer.
fn fill(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: colour,
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0_f32.into(),
        },
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

// ---------------------------------------------------------------------------
// The cell band - UIF-COMP-007, decision D-8
// ---------------------------------------------------------------------------

/// One rendered cell band (`UIF-COMP-007`).
///
/// `grid_line.contrast` is the measured ratio of the line actually chosen
/// against the band actually painted. On `hover` and `selected` that line
/// is `border.strong`, promoted because `border.subtle` measures below
/// the floor there; on `base` and `raised` the resting line survives.
#[must_use]
pub fn cell_band_cell(band: CellBand) -> Cell {
    let palette = Palette::dark();
    let style = CellStyle::resolve(band, &palette);
    let layout = resolve_widths(&regime_specs(), TABLE_WIDTH);
    let row = rect(0.0, 0.0, layout.total_width(), BODY_HEIGHT);
    let cell = table_cell::cell_rect(&layout, 0, row);
    let content = table_cell::content_rect(cell);
    let separator = table_cell::bottom_separator(cell);
    let divider = table_cell::trailing_divider(cell, false).unwrap_or(separator);
    let focus = table_cell::focus_indicator(band, &palette);

    let mut layers = vec![
        ("band", fill(cell, style.background)),
        ("separator", fill(separator, style.grid_line)),
        ("divider", fill(divider, style.grid_line)),
        ("content", marker(content, palette.border_strong)),
    ];
    if band == CellBand::Selected {
        layers.push(("outline", marker(cell, style.outline)));
    }
    layers.push(("focus", marker(cell, focus.color)));

    let mut resolved = vec![
        StyleEntry::text("audit.band", band.slug()),
        StyleEntry::colour("band", style.background),
        StyleEntry::colour("text", style.foreground),
        StyleEntry::colour("grid_line", style.grid_line),
        StyleEntry::number(
            "grid_line.contrast",
            table_cell::grid_line_contrast(band, &palette),
        ),
        StyleEntry::number("grid_line.width", table_cell::GRID_LINE_WIDTH),
        StyleEntry::number("padding.x", table_cell::CELL_PADDING_X),
        StyleEntry::number("padding.y", table_cell::CELL_PADDING_Y),
        StyleEntry::number("row.min_height", table_cell::BODY_ROW_MIN_HEIGHT),
        StyleEntry::number("content.width", content.width),
        StyleEntry::number("focus.width", focus.width),
        StyleEntry::text("focus.placement", format!("{:?}", focus.placement)),
    ];
    if band == CellBand::Selected {
        resolved.push(StyleEntry::colour("selection_outline", style.outline));
    }

    let mut audit = vec![AuditPair {
        channel: "text",
        foreground: style.foreground,
        background: style.background,
        role: ContrastRole::Text,
    }];
    if band == CellBand::Selected {
        audit.push(AuditPair {
            channel: "selection_outline",
            foreground: style.outline,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }
    audit.push(AuditPair {
        channel: "focus_ring",
        foreground: focus.color,
        background: style.background,
        role: ContrastRole::Chrome,
    });

    Cell {
        id: format!("table-cell/{}", band.slug()),
        kind: CellKind::Composite,
        subject: "table-cell",
        spec: "UIF-COMP-007",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, row, &layers),
        audit,
        note: None,
    }
}

/// The four cell bands the gallery renders.
#[must_use]
pub fn cell_band_cells() -> Vec<Cell> {
    [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ]
    .iter()
    .map(|&band| cell_band_cell(band))
    .collect()
}

// ---------------------------------------------------------------------------
// Width resolution - UIF-COMP-007, the pure column solver
// ---------------------------------------------------------------------------

/// One width-resolution regime (`UIF-COMP-007`).
///
/// The claim is the width vector itself. Each regime is named for which
/// inequality holds between the available width and the columns' floors
/// and targets, and `scroll` records whether the resolver had to give up
/// and ask for a horizontal scrollbar rather than violate a minimum.
#[must_use]
pub fn width_regime_cell(label: &'static str, available: f32) -> Cell {
    let palette = Palette::dark();
    let specs = regime_specs();
    let layout = resolve_widths(&specs, available);
    let bounds = rect(0.0, 0.0, TABLE_WIDTH, BODY_HEIGHT);

    let mut layers = vec![("viewport", marker(bounds, palette.border_subtle))];
    for index in 0..specs.len() {
        let cell = rect(layout.x_at(index), 0.0, layout.width_at(index), BODY_HEIGHT);
        layers.push(("column", marker(cell, palette.border_strong)));
    }

    let widths: Vec<String> = (0..specs.len())
        .map(|index| format!("{:.1}", layout.width_at(index)))
        .collect();
    let resolved = vec![
        StyleEntry::text("audit.regime", label),
        StyleEntry::number("available", available),
        StyleEntry::number("sum.floors", layout.sum_minimums()),
        StyleEntry::number("total", layout.total_width()),
        StyleEntry::text(
            "scroll",
            if layout.needs_horizontal_scroll() {
                "horizontal"
            } else {
                "none"
            },
        ),
        StyleEntry::text("widths", widths.join(" / ")),
    ];

    Cell {
        id: format!("table-column/{label}"),
        kind: CellKind::Composite,
        subject: "table-column",
        spec: "UIF-COMP-007",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![AuditPair {
            channel: "column_edge",
            foreground: palette.border_strong,
            background: palette.surface_base,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

/// The width regimes the gallery renders.
///
/// `480` leaves surplus for the elastic column, `240` is exactly the sum
/// of targets, `210` is a deficit shared down the two columns that have
/// headroom, and `120` is below the sum of minimums - the case the
/// resolver must terminate on rather than loop.
#[must_use]
pub fn width_regime_cells() -> Vec<Cell> {
    vec![
        width_regime_cell("surplus", 480.0),
        width_regime_cell("at-target", 240.0),
        width_regime_cell("deficit", 210.0),
        width_regime_cell("oversubscribed", 120.0),
    ]
}

// ---------------------------------------------------------------------------
// The header - UIF-COMP-007, sort target vs resize handle
// ---------------------------------------------------------------------------

/// One header posture (`UIF-COMP-007`).
///
/// Both contested regions are drawn: the sort target as a strong marker,
/// the resize handle as a subtle one. `gap` is the signed clearance
/// between them - a negative number would mean the two overlap, which is
/// exactly the bug the inset rule exists to prevent.
#[must_use]
pub fn header_posture_cell(
    label: &'static str,
    index: usize,
    sorted: Option<SortDirection>,
) -> Cell {
    let palette = Palette::dark();
    let specs = regime_specs();
    let layout = resolve_widths(&specs, TABLE_WIDTH);
    let header = rect(0.0, 0.0, TABLE_WIDTH, table_header::HEADER_HEIGHT);
    let cell = table_header::header_cell_rect(&layout, index, header);
    let target = table_header::sort_target_rect(&specs, &layout, index, header);
    let indicator = table_header::sort_indicator_rect(target);
    let handle = if table_header::column_is_resizable(&specs, index) {
        Some(table_header::resize_handle_rect(&layout, index, header))
    } else {
        None
    };

    let mut layers = vec![
        ("band", fill(cell, table_header::band_colour(&palette))),
        ("sort_target", marker(target, palette.border_strong)),
        ("indicator", marker(indicator, palette.accent_border)),
    ];
    if let Some(handle) = handle {
        layers.push(("resize_handle", marker(handle, palette.border_subtle)));
    }

    let gap = match handle {
        Some(handle) => target.x + target.width - handle.x,
        None => f32::INFINITY,
    };

    let resolved = vec![
        StyleEntry::text("audit.posture", label),
        StyleEntry::number("header.height", table_header::HEADER_HEIGHT),
        StyleEntry::number("cell.width", cell.width),
        StyleEntry::number("sort_target.width", target.width),
        StyleEntry::number("indicator.size", table_header::SORT_INDICATOR_SIZE),
        StyleEntry::number("resize.grab", table_header::RESIZE_GRAB),
        StyleEntry::number("resize.gap", if gap.is_finite() { gap } else { -1.0 }),
        StyleEntry::text(
            "resize.handle",
            if handle.is_some() { "present" } else { "none" },
        ),
        StyleEntry::text(
            "sort",
            match sorted {
                Some(direction) => format!("{direction:?}"),
                None => "unsorted".to_string(),
            },
        ),
        StyleEntry::colour("title", table_header::title_colour(&palette)),
        StyleEntry::colour("indicator", table_header::indicator_colour(&palette)),
        StyleEntry::number(
            "indicator.contrast",
            table_header::indicator_contrast(&palette),
        ),
    ];

    Cell {
        id: format!("table-header/{label}"),
        kind: CellKind::Composite,
        subject: "table-header",
        spec: "UIF-COMP-007",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, header, &layers),
        audit: vec![
            AuditPair {
                channel: "title",
                foreground: table_header::title_colour(&palette),
                background: table_header::band_colour(&palette),
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "indicator",
                foreground: table_header::indicator_colour(&palette),
                background: table_header::band_colour(&palette),
                role: ContrastRole::Chrome,
            },
        ],
        note: None,
    }
}

/// The header postures the gallery renders.
///
/// Column 0 has a live handle on its trailing edge and one on its leading
/// edge from its neighbour, so its sort target is inset twice. Column 2 is
/// the last column: no handle at all, and its sort target is the whole
/// cell.
#[must_use]
pub fn header_posture_cells() -> Vec<Cell> {
    vec![
        header_posture_cell("sortable-resizable", 0, None),
        header_posture_cell("sorted-ascending", 1, Some(SortDirection::Ascending)),
        header_posture_cell("sorted-descending", 1, Some(SortDirection::Descending)),
        header_posture_cell("last-column", 2, None),
    ]
}

/// Every structured-grid cell.
#[must_use]
pub fn table_cells() -> Vec<Cell> {
    let mut cells = cell_band_cells();
    cells.extend(width_regime_cells());
    cells.extend(header_posture_cells());
    cells
}

/// The layout the gallery's regimes resolve to, for the sidecar.
#[must_use]
pub fn layout_at(available: f32) -> ColumnLayout {
    resolve_widths(&regime_specs(), available)
}

/// The table width every cell here is built on, exposed for the sidecar.
#[must_use]
pub const fn table_width() -> f32 {
    TABLE_WIDTH
}

#[cfg(test)]
#[path = "gallery_table_tests.rs"]
mod gallery_table_tests;
