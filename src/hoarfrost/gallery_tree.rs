//! Gallery cells for the tree subsystem (`UIF-COMP-006`).
//!
//! These live outside `gallery.rs` and `gallery_composite.rs` because the
//! matrix file is already past the module size limit `RUST-053` sets, and
//! because the tree's evidence is not the same shape as the composites'
//! beside it: a tree row carries **five** independently promoted marks,
//! and the interesting claim is not what each one resolves to but that
//! each one still reads on whichever band the row happens to be painted
//! over. Decision **D-8** puts that rendering in the framework, so the
//! gallery has to be able to show it.
//!
//! Every cell here is built from the same pure resolvers the renderer
//! calls - [`tree_row::row_slots`], [`tree_row::resolve_style`],
//! [`VirtualMetrics`] - so a cell is a measurement, not a picture: the
//! numbers in `resolved` are what the widget would have used.
//!
//! Unit tests live in the sibling `gallery_tree_tests.rs` sidecar.

use crate::composite::list_tile::Leading;
use crate::composite::tree_row::{self, TreeRowStyle};
use crate::composite::virtual_list::{RowRung, VirtualMetrics};
use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::space;
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// A sample row width wide enough for the nominal indent tier.
const WIDE_ROW: f32 = space::XL * 12.0;

/// A row width that forces the indent into the compressed and collapsed
/// tiers, composed from tokens rather than written as a bare number:
/// the label floor plus the fixed prefix plus four minimum steps.
const NARROW_ROW: f32 = space::XL * 4.0 + space::LG * 4.0;

/// The viewport a virtualised list is sampled at.
const SAMPLE_VIEWPORT: f32 = space::XL * 10.0;

/// A transparent, hairline-stroked marker for a band the resolver
/// reserved but does not paint.
fn marker(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: iced::Border {
            color: colour,
            width: crate::token::shape::BORDER_HAIRLINE,
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

/// One tree-row posture to render.
#[derive(Clone, Copy, Debug)]
pub struct Posture {
    /// The posture's gallery slug.
    pub state: &'static str,
    /// The row's tree depth.
    pub depth: usize,
    /// Whether the row is selected.
    pub selected: bool,
    /// Whether the row carries `special_warning`.
    pub warning: bool,
    /// Whether the row is disabled.
    pub disabled: bool,
    /// The pointer state.
    pub base: BaseState,
    /// The row's width.
    pub width: f32,
    /// The row height rung.
    pub rung: RowRung,
}

// ---------------------------------------------------------------------------
// Tree row - UIF-COMP-006, decision D-8
// ---------------------------------------------------------------------------

/// One rendered tree row (`UIF-COMP-006`).
///
/// The `indent.*` channels are the width-relative policy's output: at a
/// narrow width the same depth reports a smaller `step` and a non-zero
/// `collapsed_levels`, and `label.width` does not move. The colour
/// channels are the D-6 adaptation: on the selection fill the guide,
/// chevron, bar and warning all resolve to `text.strong` because none of
/// their nominal tokens can be seen there.
#[must_use]
pub fn tree_row_cell(posture: &Posture) -> Cell {
    let palette = Palette::dark();
    let row_height = posture.rung.height();
    let bounds = rect(0.0, 0.0, posture.width, row_height);
    let slots = tree_row::row_slots(bounds, posture.depth, Leading::None, true, 0.0);
    let style = tree_row::resolve_style(
        posture.selected,
        posture.disabled,
        posture.warning,
        posture.base,
        palette.surface_raised,
        &palette,
    );
    let band = style.band;

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(band_layer) = tree_row::row_layer(bounds, &style) {
        layers.push(("band", band_layer));
    }
    for (index, guide) in tree_row::guide_layers(&slots, &style)
        .into_iter()
        .enumerate()
    {
        let _ = index;
        layers.push(("guide", guide));
    }
    if let Some(bar) = tree_row::selection_bar_layer(slots.selection_bar, &style) {
        layers.push(("selection-bar", bar));
    }
    layers.push(("slot.indent", marker(slots.indent, palette.border_strong)));
    layers.push(("slot.chevron", marker(slots.chevron, palette.border_strong)));
    layers.push((
        "slot.tile.center",
        marker(slots.tile.center, palette.border_subtle),
    ));

    // The step is read from the tier that actually drew the guides, not
    // derived from the reserved width: in the collapsed tier the width is
    // shared out differently than it is divided back.
    let indent = tree_row::indent_geometry(posture.depth, posture.width);
    let mut resolved = vec![
        StyleEntry::number("geometry.row.height", row_height),
        StyleEntry::number("geometry.row.width", posture.width),
        StyleEntry::number("geometry.indent.step", indent.step),
        StyleEntry::number("geometry.indent.width", slots.indent.width),
        StyleEntry::number("geometry.indent.guides", slots.guides.len() as f32),
        StyleEntry::number(
            "geometry.indent.collapsed_levels",
            indent.collapsed_levels as f32,
        ),
        StyleEntry::number("geometry.chevron.width", slots.chevron.width),
        StyleEntry::number("geometry.selection_bar.width", slots.selection_bar.width),
        StyleEntry::number("geometry.label.width", slots.tile.center.width),
        StyleEntry::text("audit.background", token_label(band)),
        StyleEntry::colour("label", style.label_colour),
        StyleEntry::colour("chevron", style.chevron),
        StyleEntry::colour("guide", style.guide),
    ];
    // A channel is only reported when the renderer actually paints it. A
    // disabled row carries neither the selection bar nor the warning dot,
    // so both resolve fully transparent - auditing a transparent
    // foreground against the band would report a contrast failure for a
    // pixel that never lands.
    let paints_bar = style.selection_bar.a > 0.0;
    let paints_warning = style.warning.a > 0.0;
    if paints_bar {
        resolved.push(StyleEntry::colour("selection_bar", style.selection_bar));
    }
    if paints_warning {
        resolved.push(StyleEntry::colour("warning", style.warning));
    }
    let mut audit = vec![
        AuditPair {
            channel: "label",
            foreground: style.label_colour,
            background: band,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "chevron",
            foreground: style.chevron,
            background: band,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "guide",
            foreground: style.guide,
            background: band,
            role: ContrastRole::Chrome,
        },
    ];
    if paints_bar {
        audit.push(AuditPair {
            channel: "selection_bar",
            foreground: style.selection_bar,
            background: band,
            role: ContrastRole::Chrome,
        });
    }
    if paints_warning {
        audit.push(AuditPair {
            channel: "warning",
            foreground: style.warning,
            background: band,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!("tree-row/{}", posture.state),
        kind: CellKind::Composite,
        subject: "tree-row",
        spec: "UIF-COMP-006",
        namer: Namer::Standard,
        resolved,
        frame: crate::gallery::paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// The tree-row postures the gallery renders.
#[must_use]
pub fn tree_row_cells() -> Vec<Cell> {
    [
        Posture {
            state: "rest/depth-0",
            depth: 0,
            selected: false,
            warning: false,
            disabled: false,
            base: BaseState::Rest,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "rest/depth-3",
            depth: 3,
            selected: false,
            warning: false,
            disabled: false,
            base: BaseState::Rest,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "hovered/depth-2",
            depth: 2,
            selected: false,
            warning: false,
            disabled: false,
            base: BaseState::Hovered,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "pressed/depth-2",
            depth: 2,
            selected: false,
            warning: false,
            disabled: false,
            base: BaseState::Pressed,
            width: WIDE_ROW,
            rung: RowRung::Compact,
        },
        Posture {
            state: "selected/depth-1",
            depth: 1,
            selected: true,
            warning: false,
            disabled: false,
            base: BaseState::Rest,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "selected/depth-4",
            depth: 4,
            selected: true,
            warning: false,
            disabled: false,
            base: BaseState::Hovered,
            width: WIDE_ROW,
            rung: RowRung::Compact,
        },
        Posture {
            state: "selected-warning/depth-2",
            depth: 2,
            selected: true,
            warning: true,
            disabled: false,
            base: BaseState::Rest,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "warning/depth-2",
            depth: 2,
            selected: false,
            warning: true,
            disabled: false,
            base: BaseState::Rest,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "disabled/depth-2",
            depth: 2,
            selected: true,
            warning: true,
            disabled: true,
            base: BaseState::Hovered,
            width: WIDE_ROW,
            rung: RowRung::Standard,
        },
        Posture {
            state: "narrow/depth-9-compressed",
            depth: 9,
            selected: false,
            warning: false,
            disabled: false,
            base: BaseState::Rest,
            width: NARROW_ROW,
            rung: RowRung::Compact,
        },
        Posture {
            state: "narrow/depth-20-collapsed",
            depth: 20,
            selected: true,
            warning: false,
            disabled: false,
            base: BaseState::Rest,
            width: NARROW_ROW,
            rung: RowRung::Compact,
        },
    ]
    .iter()
    .map(tree_row_cell)
    .collect()
}

// ---------------------------------------------------------------------------
// Virtual list - UIF-COMP-006, the recycling bound
// ---------------------------------------------------------------------------

/// One virtualised-list posture (`UIF-COMP-006`).
///
/// The claim in `resolved` is `realized.bound` against `total.rows`: the
/// bound has no term for the total, so a million-row list reports the
/// same realised count as a hundred-thousand-row one. The thumb length
/// is audited as chrome over the track because a thumb sized to the
/// realised rows would read as a nearly-full track and lie about the
/// document's size.
#[must_use]
pub fn virtual_list_cell(total_rows: usize, rung: RowRung, label: &'static str) -> Cell {
    let palette = Palette::dark();
    let metrics = VirtualMetrics::new(total_rows, rung, SAMPLE_VIEWPORT);
    let bounds = rect(0.0, 0.0, WIDE_ROW, SAMPLE_VIEWPORT);
    let row = metrics.row_rect(0, bounds, 0.0);
    let track = rect(
        bounds.width - crate::token::size::HIT_TARGET_MIN,
        0.0,
        crate::token::size::HIT_TARGET_MIN,
        SAMPLE_VIEWPORT,
    );
    let thumb = metrics.thumb_rect(track, 0.0, crate::composite::scroll_view::Axis::Vertical);

    let layers: Vec<(&'static str, Layer)> = vec![
        ("viewport", marker(bounds, palette.border_subtle)),
        ("row", marker(row, palette.border_strong)),
        ("track", marker(track, palette.border_subtle)),
        (
            "thumb",
            Layer {
                bounds: thumb,
                fill: palette.border_strong,
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0_f32.into(),
                },
            },
        ),
    ];

    Cell {
        id: format!("virtual-list/{label}"),
        kind: CellKind::Composite,
        subject: "virtual-list",
        spec: "UIF-COMP-006",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("geometry.row.height", metrics.row_height()),
            StyleEntry::number("geometry.viewport", metrics.viewport_len()),
            StyleEntry::number("geometry.overscan", metrics.overscan() as f32),
            StyleEntry::number("geometry.total.rows", total_rows as f32),
            StyleEntry::number(
                "geometry.realized.bound",
                metrics.realized_count_bound() as f32,
            ),
            StyleEntry::number(
                "geometry.realized.count",
                metrics.realized_count(0.0) as f32,
            ),
            StyleEntry::number("geometry.content.len", metrics.content_len()),
            StyleEntry::number("geometry.thumb.length", metrics.thumb_length()),
        ],
        frame: crate::gallery::paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![AuditPair {
            channel: "thumb",
            foreground: palette.border_strong,
            background: palette.surface_raised,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

/// The virtual-list postures the gallery renders.
#[must_use]
pub fn virtual_list_cells() -> Vec<Cell> {
    vec![
        virtual_list_cell(1_000, RowRung::Compact, "compact/1k"),
        virtual_list_cell(1_000, RowRung::Standard, "standard/1k"),
        virtual_list_cell(100_000, RowRung::Compact, "compact/100k"),
        virtual_list_cell(1_000_000, RowRung::Compact, "compact/1m"),
        virtual_list_cell(1_000_000, RowRung::Standard, "standard/1m"),
    ]
}

/// Every tree-subsystem cell.
#[must_use]
pub fn tree_cells() -> Vec<Cell> {
    let mut cells = tree_row_cells();
    cells.extend(virtual_list_cells());
    cells
}

/// The row style a posture resolves to, for the sidecar's cross-checks.
#[must_use]
pub fn style_for(posture: &Posture, palette: &Palette) -> TreeRowStyle {
    tree_row::resolve_style(
        posture.selected,
        posture.disabled,
        posture.warning,
        posture.base,
        palette.surface_raised,
        palette,
    )
}

fn token_label(colour: Color) -> String {
    Namer::Standard.name(colour)
}

/// The narrow-row width, exposed for the sidecar.
#[must_use]
pub const fn narrow_row() -> f32 {
    NARROW_ROW
}

/// The wide-row width, exposed for the sidecar.
#[must_use]
pub const fn wide_row() -> f32 {
    WIDE_ROW
}

/// The list viewport, exposed for the sidecar.
#[must_use]
pub const fn sample_viewport() -> f32 {
    SAMPLE_VIEWPORT
}

#[cfg(test)]
#[path = "gallery_tree_tests.rs"]
mod gallery_tree_tests;
