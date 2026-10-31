//! The hyperlink cells of the `UIF-VER-004` matrix (§3.1d,
//! `UIF-ATOM-001d`).
//!
//! Built through [`gallery`]'s own vocabulary; [`gallery::matrix`] calls
//! [`hyperlink_cells`] back so the matrix remains one list in one fixed
//! order.
//!
//! # Ten cells
//!
//! The five §3.3 state columns × the two mounts §3.1d names. The mount
//! is its own axis here and nowhere else in the matrix because §3.1d is
//! the only requirement that prints a single cell against two surfaces —
//! `accent.text` at `10.66:1` on `base` and `9.49:1` on `overlay` —
//! because a hyperlink is the only atom that appears both inside body
//! copy and on a floating surface. Sampling one mount would leave half of
//! the printed pair unmeasured.
//!
//! # The label is audited against what is actually behind it
//!
//! At rest and when disabled the link paints nothing, so the pair that
//! matters is the label against the **mount**, not against a transparent
//! fill. Measuring a label against `Color::TRANSPARENT` is not a
//! measurement at all, and the gallery exempts transparent-backing pairs
//! for exactly that reason — which means passing the raw style through
//! would silently drop the two figures §3.1d actually quotes. Every cell
//! therefore resolves its own backing: the pill where one is painted, the
//! mount where none is.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_hyperlink_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::widget::button_widget::Layer;
use crate::widget::hyperlink::{self, HyperlinkStyle};
use iced::{Border, Color, Point, Rectangle, Size};

/// The five state columns §3.3 prints, in spec order.
const COLUMNS: [VisualState; 5] = [
    VisualState {
        base: BaseState::Rest,
        focused: false,
    },
    VisualState {
        base: BaseState::Hovered,
        focused: false,
    },
    VisualState {
        base: BaseState::Pressed,
        focused: false,
    },
    VisualState {
        base: BaseState::Rest,
        focused: true,
    },
    VisualState {
        base: BaseState::Disabled,
        focused: false,
    },
];

/// One of the surfaces §3.1d quotes the link's ratios against: its name
/// and the palette channel that supplies it.
type Mount = (&'static str, fn(&Palette) -> Color);

/// The two mounts §3.1d quotes its ratios against, in spec order.
const MOUNTS: [Mount; 2] = [
    ("base", |p: &Palette| p.surface_base),
    ("overlay", |p: &Palette| p.surface_overlay),
];

/// A nominal label width for the geometry entries.
const SAMPLE_LABEL_WIDTH: f32 = 88.0;

/// The link's hit box for one label width, centred in the sample column.
fn link_box(sample: Rectangle) -> Rectangle {
    let size = hyperlink::control_size(SAMPLE_LABEL_WIDTH);
    Rectangle::new(
        Point::new(
            sample.x
                + if size.width >= sample.width {
                    0.0
                } else {
                    (sample.width - size.width) / 2.0
                },
            sample.y + (sample.height - size.height) / 2.0,
        ),
        size,
    )
}

/// The pill layer, present only where a fill is actually painted.
fn pill_layer(bounds: Rectangle, style: &HyperlinkStyle) -> Option<Layer> {
    if style.is_transparent_background() {
        return None;
    }
    Some(Layer {
        bounds,
        fill: style.background,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: hyperlink::RADIUS.into(),
        },
    })
}

/// The underline layer, present in every live cell.
fn underline_layer(style: &HyperlinkStyle, bounds: Rectangle) -> Option<Layer> {
    if !style.underlined {
        return None;
    }
    Some(Layer {
        bounds: hyperlink::underline_bounds(bounds),
        fill: style.foreground,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
    })
}

/// The focus ring drawn outside the hit box.
fn focus_ring_layer(bounds: Rectangle, style: &HyperlinkStyle) -> Option<Layer> {
    if style.focus_ring_width <= 0.0 {
        return None;
    }
    let width = style.focus_ring_width;
    Some(Layer {
        bounds: Rectangle::new(
            Point::new(bounds.x - width, bounds.y - width),
            Size::new(bounds.width + width * 2.0, bounds.height + width * 2.0),
        ),
        fill: Color::TRANSPARENT,
        border: Border {
            color: style.focus_ring_color,
            width,
            radius: (hyperlink::RADIUS + width).into(),
        },
    })
}

/// The hyperlink cell for one `(state, mount)` pair (`UIF-ATOM-001d`).
///
/// Resolved through [`hyperlink::resolve_style`] — the same pure
/// function a widget paints from.
#[must_use]
pub fn hyperlink_cell(state: VisualState, mount_name: &'static str, mount: Color) -> Cell {
    let palette = Palette::dark();
    let style = hyperlink::resolve_style(state, mount, &palette);

    let sample = atom_sample_at(hyperlink::MIN_HEIGHT);
    let bounds = link_box(sample);

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = pill_layer(bounds, &style) {
        layers.push(("pill", layer));
    }
    if let Some(layer) = underline_layer(&style, bounds) {
        layers.push(("underline", layer));
    }
    if let Some(layer) = focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }

    let backing = if style.is_transparent_background() {
        mount
    } else {
        style.background
    };

    let mut audit = vec![AuditPair {
        channel: "link.text",
        foreground: style.foreground,
        background: backing,
        role: ContrastRole::Text,
    }];
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "link.focus-ring",
            foreground: style.focus_ring_color,
            background: mount,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!("hyperlink/{mount_name}-{}", state_slug(state)),
        kind: CellKind::Atom,
        subject: "hyperlink",
        spec: "UIF-ATOM-001d",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("mount", mount_name),
            StyleEntry::colour("text", style.foreground),
            StyleEntry::colour("fill", style.background),
            StyleEntry::text(
                "transparent-fill",
                style.is_transparent_background().to_string(),
            ),
            StyleEntry::text("underlined", style.underlined.to_string()),
            StyleEntry::number("underline.width", hyperlink::UNDERLINE_WIDTH),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::number("radius", hyperlink::RADIUS),
            StyleEntry::number("hit.min-height", hyperlink::MIN_HEIGHT),
            StyleEntry::number("hit.height", bounds.height),
            StyleEntry::number("hit.width", bounds.width),
            StyleEntry::number("padding-x", hyperlink::PADDING_X),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Every hyperlink cell: the five §3.3 columns against both mounts §3.1d
/// names, in that nesting order.
#[must_use]
pub fn hyperlink_cells() -> Vec<Cell> {
    let palette = Palette::dark();
    let mut cells = Vec::new();
    for state in COLUMNS {
        for (name, pick) in MOUNTS {
            cells.push(hyperlink_cell(state, name, pick(&palette)));
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_hyperlink_tests.rs"]
mod gallery_hyperlink_tests;
