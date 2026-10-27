//! The segmented-strip cells of the `UIF-VER-004` matrix
//! (§3.1b, `UIF-ATOM-001b`).
//!
//! Built through [`gallery`]'s own vocabulary; [`gallery::matrix`] calls
//! [`segmented_cells`] back so the matrix remains one list in one fixed
//! order.
//!
//! # Forty cells
//!
//! Two subjects × two polarities × two height rungs × the five §3.3
//! state columns:
//!
//! * `toggle` — the one-segment strip a `ToggleButton` is, in its `off`
//!   and `on` polarities;
//! * `strip` — a three-segment control with the sampled segment in the
//!   given polarity and its neighbours at rest, which is the only way to
//!   show the *joined* strip rather than a lone box.
//!
//! The two subjects are not redundant. The strip cells are where the
//! zero-shift claim is visible: every segment carries the reserved `✓`
//! slot, so the neighbours' labels sit where they will sit once the
//! selection moves onto them.
//!
//! # The state belongs to the sampled segment
//!
//! A strip has one pointer, so a cell names which segment the state is
//! being applied to. The sampled segment carries the column's paint; the
//! others stay at `Rest`. Auditing only the sampled segment keeps the
//! measurement about the cell the spec prints rather than about three
//! segments that happen to be adjacent to it.
//!
//! # The fill is not audited against the mount
//!
//! A selected segment's `surface.active` measures `1.3089:1` against
//! `surface.raised`, and every surface rung in this palette sits in the
//! same band against its neighbours — `surface.hover` on `surface.raised`
//! is `1.1525:1`. Surfaces are not indicator material and none of them
//! clears `3.00:1`. §3.1b makes the selection signal the
//! `accent.border` perimeter (`4.7922:1`) and the `✓` (`7.6562:1`), and
//! those are what this cell measures. Auditing the fill would report a
//! failure against a floor the token layer never claimed for surfaces.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_segmented_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::widget::button_widget::Layer;
use crate::widget::segmented::{self, SegmentPolarity, SegmentStyle};
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

/// The two height rungs §3.1b prices, in spec order.
const HEIGHTS: [f32; 2] = [segmented::HEIGHT_COMPACT, segmented::HEIGHT_STANDARD];

/// A nominal label width for the geometry entries.
///
/// A harness figure rather than a visual magnitude: the rule under test
/// is the *relationship* between the reserved `✓` slot, the padding and
/// the label, and one representative width shows the strip widening past
/// its floor without the cell having to measure real text.
const SAMPLE_LABEL_WIDTH: f32 = 64.0;

/// The two subjects the matrix samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripSubject {
    /// The one-segment strip a `ToggleButton` is.
    Toggle,
    /// A three-segment control, sampled at its middle segment.
    Strip,
}

impl StripSubject {
    /// The subject's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Strip => "strip",
        }
    }

    /// How many segments this subject's strip has.
    #[must_use]
    pub const fn segment_count(self) -> usize {
        match self {
            Self::Toggle => 1,
            Self::Strip => 3,
        }
    }

    /// Which segment the cell samples.
    #[must_use]
    pub const fn sampled(self) -> usize {
        match self {
            Self::Toggle => 0,
            Self::Strip => 1,
        }
    }
}

/// A [`SegmentPolarity`] as a manifest value.
fn polarity_slug(polarity: SegmentPolarity) -> &'static str {
    match polarity {
        SegmentPolarity::Unselected => "off",
        SegmentPolarity::Selected => "on",
    }
}

/// A height rung as a cell-id slug.
fn height_slug(height: f32) -> &'static str {
    if (height - segmented::HEIGHT_COMPACT).abs() < f32::EPSILON {
        "compact"
    } else {
        "standard"
    }
}

/// The strip's box for one rung, centred in the sample column.
fn strip_box(subject: StripSubject, height: f32, sample: Rectangle) -> Rectangle {
    let labels = vec![SAMPLE_LABEL_WIDTH; subject.segment_count()];
    let width = segmented::strip_width(&labels).max(1.0);
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
        Size::new(width, height),
    )
}

/// One segment's own layer: its fill and perimeter in one quad.
fn segment_layer(bounds: Rectangle, style: &SegmentStyle) -> Layer {
    Layer {
        bounds,
        fill: style.paint.background,
        border: Border {
            color: style.paint.border_color,
            width: style.paint.border_width,
            radius: segmented::RADIUS.into(),
        },
    }
}

/// The control-level focus ring drawn outside the whole strip.
fn focus_ring_layer(bounds: Rectangle, ring: Option<(Color, f32)>) -> Option<Layer> {
    let (color, width) = ring?;
    if width <= 0.0 {
        return None;
    }
    Some(Layer {
        bounds: Rectangle::new(
            Point::new(bounds.x - width, bounds.y - width),
            Size::new(bounds.width + width * 2.0, bounds.height + width * 2.0),
        ),
        fill: Color::TRANSPARENT,
        border: Border {
            color,
            width,
            radius: (segmented::RADIUS + width).into(),
        },
    })
}

/// The segmented-strip cell for one `(subject, polarity, height, state)`
/// quadruple (`UIF-ATOM-001b`).
///
/// Resolved through [`segmented::resolve_style`] — the same pure function
/// a widget paints from — and drawn as the layers it describes. Nothing
/// here is a colour the gallery chose.
#[must_use]
pub fn segmented_cell(
    subject: StripSubject,
    polarity: SegmentPolarity,
    height: f32,
    state: VisualState,
) -> Cell {
    let palette = Palette::dark();
    let style = segmented::resolve_style(polarity, state, &palette);

    let sample = atom_sample_at(height);
    let strip = strip_box(subject, height, sample);
    let labels = vec![SAMPLE_LABEL_WIDTH; subject.segment_count()];
    let mount = palette.surface_raised;
    let ring = segmented::resolve_strip_ring(state, mount, &palette);

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    for index in 0..subject.segment_count() {
        let bounds = segmented::segment_bounds(strip, index, &labels);
        let layer = if index == subject.sampled() {
            segment_layer(bounds, &style)
        } else {
            segment_layer(
                bounds,
                &segmented::resolve_style(SegmentPolarity::Unselected, COLUMNS[0], &palette),
            )
        };
        layers.push(("segment", layer));
    }
    if let Some(layer) = focus_ring_layer(strip, ring) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![
        AuditPair {
            channel: "segment.border",
            foreground: style.paint.border_color,
            background: style.paint.background,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "segment.text",
            foreground: style.paint.foreground,
            background: style.paint.background,
            role: ContrastRole::Text,
        },
    ];
    if style.shows_check {
        audit.push(AuditPair {
            channel: "segment.check",
            foreground: style.check,
            background: style.paint.background,
            role: ContrastRole::Chrome,
        });
    }
    if let Some((color, _)) = ring {
        audit.push(AuditPair {
            channel: "strip.focus-ring",
            foreground: color,
            background: mount,
            role: ContrastRole::Chrome,
        });
    }

    let segment = segmented::segment_bounds(strip, subject.sampled(), &labels);

    Cell {
        id: format!(
            "{}/{}/{}-{}",
            subject.slug(),
            polarity_slug(polarity),
            height_slug(height),
            state_slug(state)
        ),
        kind: CellKind::Atom,
        subject: if subject == StripSubject::Toggle {
            "toggle-button"
        } else {
            "segmented-button"
        },
        spec: "UIF-ATOM-001b",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("subject", subject.slug()),
            StyleEntry::text("polarity", polarity_slug(polarity)),
            StyleEntry::number("segments", subject.segment_count() as f32),
            StyleEntry::number("sampled", subject.sampled() as f32),
            StyleEntry::colour("fill", style.paint.background),
            StyleEntry::colour("text", style.paint.foreground),
            StyleEntry::colour("border.color", style.paint.border_color),
            StyleEntry::number("border.width", style.paint.border_width),
            StyleEntry::colour("check.color", style.check),
            StyleEntry::text("check.shown", style.shows_check.to_string()),
            StyleEntry::number("strip.radius", segmented::RADIUS),
            StyleEntry::number("strip.perimeter-width", segmented::PERIMETER_WIDTH),
            StyleEntry::number("strip.height", height),
            StyleEntry::number("strip.width", strip.width),
            StyleEntry::number("segment.width", segment.width),
            StyleEntry::number("segment.check-slot", segmented::CHECK_SLOT_WIDTH),
            StyleEntry::number(
                "strip.focus-ring.width",
                ring.map(|(_, width)| width).unwrap_or(0.0),
            ),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Every segmented-strip cell: two subjects × two polarities × two rungs
/// × the five §3.3 columns, in that nesting order.
#[must_use]
pub fn segmented_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for subject in [StripSubject::Toggle, StripSubject::Strip] {
        for polarity in [SegmentPolarity::Unselected, SegmentPolarity::Selected] {
            for height in HEIGHTS {
                for state in COLUMNS {
                    cells.push(segmented_cell(subject, polarity, height, state));
                }
            }
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_segmented_tests.rs"]
mod gallery_segmented_tests;
