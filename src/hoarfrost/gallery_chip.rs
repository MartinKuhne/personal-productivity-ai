//! The chip cells of the `UIF-VER-004` matrix (§3.6, `UIF-ATOM-006`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`chip_cells`] back, so the
//! matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # Forty cells
//!
//! Four chip shapes × two height rungs × the five §3.3 state columns:
//!
//! * `assist` — the action chip, no selection;
//! * `filter-off` / `filter-on` — the toggle chip in each polarity, the
//!   only shape with a selected skin and a leading `✓`;
//! * `input` — the removable token chip, which reserves the trailing
//!   `24 × 24` `×` slot.
//!
//! The four shapes rather than the three `ChipVariant`s because §3.6's
//! table is keyed on *selected versus not*, and a filter chip in each
//! polarity is where that key actually lands. A non-removable input chip
//! is deliberately absent: it differs from `input` by one reserved slot
//! and no colour, so it would add a cell without adding a measurement.
//!
//! # What each cell paints
//!
//! The chip's own box as one layer — fill and perimeter together — built
//! from [`chip::resolve_style`] and [`chip::with_variant`] rather than
//! from the gallery guessing at them, plus the focus ring when one is
//! drawn. The label and the `✓` / `×` glyphs are **not** in the frame:
//! they are font runs, and recording a filled rectangle in their place
//! would put a quad in the manifest that no renderer was ever told was a
//! `✓`. Their colours are still carried in `resolved` and still go
//! through the audit, so nothing about the pairing goes unmeasured.
//!
//! # The fill is audited against the mount only when it is a signal
//!
//! An unselected chip's fill *is* the surface it sits on: `surface.raised`
//! on `surface.raised` measures `1.00:1`, and calling that a failure
//! would be claiming the chip cannot be seen when the perimeter says it
//! can. §3.6 makes the **selected** chip's `accent.fill` the selection
//! signal, so that fill — and only that fill, and only where it differs
//! from the mount — owes the rule 2 threshold. It measures `3.8965:1`
//! at rest and `3.3082:1` pressed, against a floor of `3.00:1`.
//!
//! The perimeter is audited against the mount for every state. The
//! disabled chip's `border.subtle` perimeter classifies as
//! [`ChromeClass::RestingDivider`] and is exempt, which is the correct
//! verdict: a disabled chip is not signalling, and `UIF-TOK-006` rule 2
//! governs active chrome.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_chip_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::widget::button_widget::Layer;
use crate::widget::chip::{self, ChipPolarity, ChipStyle};
use crate::widget::chip_model::ChipHeight;
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

/// The two height rungs §3.6 prices, in spec order.
const HEIGHTS: [ChipHeight; 2] = [ChipHeight::Compact, ChipHeight::Bar];

/// The four chip shapes the matrix samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipShape {
    /// §3.6's `Assist` — an action chip with nothing to select.
    Assist,
    /// §3.6's `Filter { selected: false }`.
    FilterOff,
    /// §3.6's `Filter { selected: true }`.
    FilterOn,
    /// §3.6's `Input { removable: true }`.
    Input,
}

impl ChipShape {
    /// The shape's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Assist => "assist",
            Self::FilterOff => "filter-off",
            Self::FilterOn => "filter-on",
            Self::Input => "input",
        }
    }

    /// The polarity this shape resolves to.
    #[must_use]
    pub const fn polarity(self) -> ChipPolarity {
        match self {
            Self::FilterOn => ChipPolarity::Selected,
            Self::Assist | Self::FilterOff | Self::Input => ChipPolarity::Unselected,
        }
    }

    /// Whether this shape takes the pill radius.
    #[must_use]
    pub const fn is_assist(self) -> bool {
        matches!(self, Self::Assist)
    }

    /// Whether this shape reserves the trailing `×` slot.
    #[must_use]
    pub const fn is_removable(self) -> bool {
        matches!(self, Self::Input)
    }
}

/// A nominal label width for the geometry entries.
///
/// A harness figure rather than a visual magnitude: the width rule is
/// about the *relationship* between padding, glyph box, remove slot and
/// label, and one representative width shows the chip widening past its
/// floor without the cell having to measure real text.
const SAMPLE_LABEL_WIDTH: f32 = 72.0;

/// A [`ChromeClass`] as the manifest names it.
///
/// Spelled out here rather than read off the class because
/// [`ChromeClass`] carries no `label()` of its own — the class is a
/// measurement verdict, and this is the manifest's rendering of it.
fn chrome_class_label(class: ChromeClass) -> &'static str {
    match class {
        ChromeClass::Judged => "judged",
        ChromeClass::CoincidentWithFill => "coincident-with-fill",
        ChromeClass::UnpaintedBackground => "unpainted-background",
        ChromeClass::RestingDivider => "resting-divider",
    }
}

/// The chip box for one resolved style, centred in the sample.
fn chip_box(style: &ChipStyle, sample: Rectangle) -> Rectangle {
    let leading = if style.shows_check {
        chip::GLYPH_SIZE
    } else {
        0.0
    };
    let width = chip::chip_width(style, leading, SAMPLE_LABEL_WIDTH);
    Rectangle::new(
        Point::new(
            sample.x
                + if width >= sample.width {
                    0.0
                } else {
                    (sample.width - width) / 2.0
                },
            sample.y,
        ),
        Size::new(width, style.height),
    )
}

/// The chip's own layer: its fill and its perimeter in one quad.
fn chip_layer(bounds: Rectangle, style: &ChipStyle) -> Layer {
    Layer {
        bounds,
        fill: style.fill,
        border: Border {
            color: style.border,
            width: style.border_width,
            radius: style.radius.into(),
        },
    }
}

/// The focus ring drawn outside the chip box, when one is drawn.
fn focus_ring_layer(bounds: Rectangle, style: &ChipStyle) -> Option<Layer> {
    if style.focus_ring_width <= 0.0 {
        return None;
    }
    let w = style.focus_ring_width;
    Some(Layer {
        bounds: Rectangle::new(
            Point::new(bounds.x - w, bounds.y - w),
            Size::new(bounds.width + w * 2.0, bounds.height + w * 2.0),
        ),
        fill: Color::TRANSPARENT,
        border: Border {
            color: style.focus_ring_color,
            width: w,
            radius: (style.radius + w).into(),
        },
    })
}

/// The chip cell for one `(shape, height, state)` triple (`UIF-ATOM-006`).
///
/// Resolved through [`chip::resolve_style`] and [`chip::with_variant`] —
/// the same pure functions a widget paints from — and drawn as the layers
/// those functions describe. Nothing in this cell is a colour the gallery
/// chose.
#[must_use]
pub fn chip_cell(shape: ChipShape, height: ChipHeight, state: VisualState) -> Cell {
    let palette = Palette::dark();
    let base = chip::resolve_style(shape.polarity(), height, state, &palette);
    let style = chip::with_variant(base, shape.is_assist(), shape.is_removable());

    let sample = atom_sample_at(height.px());
    let bounds = chip_box(&style, sample);
    let mount = palette.surface_raised;

    let mut layers: Vec<(&'static str, Layer)> = vec![("chip", chip_layer(bounds, &style))];
    if let Some(ring) = focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", ring));
    }

    let mut audit = vec![
        AuditPair {
            channel: "chip.border",
            foreground: style.border,
            background: mount,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "chip.text",
            foreground: style.text,
            background: style.fill,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "chip.glyph",
            foreground: style.glyph,
            background: style.fill,
            role: ContrastRole::Chrome,
        },
    ];

    // Only the selected chip's fill is a signal rather than the surface
    // itself; see the module docs.
    if shape.polarity() == ChipPolarity::Selected && style.fill != mount {
        audit.push(AuditPair {
            channel: "chip.fill",
            foreground: style.fill,
            background: mount,
            role: ContrastRole::Chrome,
        });
    }
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "chip.focus-ring",
            foreground: style.focus_ring_color,
            background: style.fill,
            role: ContrastRole::Chrome,
        });
    }

    let remove = chip::remove_slot(&style, bounds);

    Cell {
        id: format!(
            "chip/{}/{}-{}",
            shape.slug(),
            height_slug(height),
            state_slug(state)
        ),
        kind: CellKind::Atom,
        subject: "chip",
        spec: "UIF-ATOM-006",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("shape", shape.slug()),
            StyleEntry::text("polarity", polarity_slug(shape.polarity())),
            StyleEntry::colour("fill", style.fill),
            StyleEntry::colour("border.color", style.border),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("text", style.text),
            StyleEntry::colour("glyph", style.glyph),
            StyleEntry::text(
                "glyph.kind",
                if style.shows_check { "check" } else { "none" },
            ),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::number("radius", style.radius),
            StyleEntry::text(
                "chrome.class",
                chrome_class_label(chip::chip_chrome_class(&style, &palette)),
            ),
            StyleEntry::number("geometry.height", style.height),
            StyleEntry::number("geometry.padding-x", style.h_padding),
            StyleEntry::number("geometry.label-gap", style.label_gap),
            StyleEntry::number("geometry.remove-target", style.remove_target),
            StyleEntry::number("geometry.width", bounds.width),
            StyleEntry::text(
                "geometry.remove-slot",
                match remove {
                    Some(_) => "reserved",
                    None => "none",
                },
            ),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// A [`ChipHeight`] as a cell-id slug.
fn height_slug(height: ChipHeight) -> &'static str {
    match height {
        ChipHeight::Compact => "compact",
        ChipHeight::Bar => "bar",
    }
}

/// A [`ChipPolarity`] as a manifest value.
fn polarity_slug(polarity: ChipPolarity) -> &'static str {
    match polarity {
        ChipPolarity::Unselected => "unselected",
        ChipPolarity::Selected => "selected",
    }
}

/// Every chip cell: four shapes × two rungs × the five §3.3 columns, in
/// that nesting order.
#[must_use]
pub fn chip_cells() -> Vec<Cell> {
    let shapes = [
        ChipShape::Assist,
        ChipShape::FilterOff,
        ChipShape::FilterOn,
        ChipShape::Input,
    ];
    let mut cells = Vec::new();
    for shape in shapes {
        for height in HEIGHTS {
            for state in COLUMNS {
                cells.push(chip_cell(shape, height, state));
            }
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_chip_tests.rs"]
mod gallery_chip_tests;
