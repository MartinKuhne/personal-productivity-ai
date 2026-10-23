//! The badge cells of the `UIF-VER-004` matrix (§3.8, `UIF-ATOM-008`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`badge_cells`] back, so the
//! matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # Fifty cells
//!
//! The full `(kind × tone)` cross product §3.8 defines:
//!
//! * the dot in each of the five tones — 5 cells;
//! * the count pill at five representative values (`0`, `1`, `42`, `99`,
//!   `150`) in each tone — 25 cells. The values are chosen at the
//!   boundaries: the empty case, the narrowest case, an ordinary case,
//!   the ceiling itself, and one above it where the fold fires;
//! * the status chip with each of the four mandatory glyphs in each tone
//!   — 20 cells.
//!
//! The cross product is deliberate. A badge has no state column, so the
//! only variation the audit can be given is *which claim in which tone*,
//! and every one of those fifty pairs is a contrast measurement the
//! spec's own table implies.
//!
//! # What each cell paints
//!
//! The badge's own box as one layer — fill and perimeter together, the
//! same shape a widget would hand the renderer — built from
//! [`badge::resolve_style`] rather than from the gallery guessing at it.
//! The count's digits and the chip's glyph are **not** in the frame:
//! they are font runs, and recording a filled rectangle in their place
//! would put a quad in the manifest that no renderer was ever told was a
//! `9`. Their colours are still carried in `resolved` and still go
//! through the audit, so nothing about the pairing goes unmeasured.
//!
//! # The mount is `surface.raised`, and that is the conservative choice
//!
//! §3.8 puts the chip on `surface.raised`, and the dot and pill are
//! pinned to controls whose own surfaces are no darker. The gallery
//! audits the badge's fill against `surface.raised` for every kind,
//! which is the surface the spec names rather than one it was inferred
//! for. Where a badge is mounted on something lighter — a dot on a
//! `surface.hover` toolbar button, say — the ratio only gets better,
//! since the badge fill is the darker of the pair.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_badge_tests.rs` sidecar.

use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::widget::badge::{self, pill_width};
use crate::widget::badge_model::{BadgeKind, BadgeModel, BadgeTone, StatusIcon};
use crate::widget::button_widget::Layer;
use iced::{Border, Point, Rectangle};

/// Every tone, in declaration order.
const TONES: [BadgeTone; 5] = [
    BadgeTone::Neutral,
    BadgeTone::Accent,
    BadgeTone::Success,
    BadgeTone::Warning,
    BadgeTone::Danger,
];

/// Every mandatory status glyph, in §3.8 order.
const ICONS: [StatusIcon; 4] = [
    StatusIcon::Success,
    StatusIcon::Warning,
    StatusIcon::Danger,
    StatusIcon::Info,
];

/// The count values the matrix samples, in order.
///
/// Boundary-picked rather than round: `0` is the empty case, `1` the
/// narrowest, `42` an ordinary mid-range count, `99` the ceiling that
/// must *not* fold, and `150` the first value that must.
const COUNT_VALUES: [u32; 5] = [0, 1, 42, 99, 150];

/// A nominal text width for the pill and chip geometry entries.
///
/// A harness figure rather than a visual magnitude: the width rule is
/// about the *relationship* between padding, glyph box and content, and
/// one representative width shows the pill widening past its floor
/// without the cell having to measure real text.
const SAMPLE_TEXT_WIDTH: f32 = 44.0;

/// A nominal glyph width for the chip's leading mark.
const SAMPLE_GLYPH_WIDTH: f32 = 12.0;

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

/// A [`BadgeTone`] as a cell-id slug.
fn tone_slug(tone: BadgeTone) -> &'static str {
    match tone {
        BadgeTone::Neutral => "neutral",
        BadgeTone::Accent => "accent",
        BadgeTone::Success => "success",
        BadgeTone::Warning => "warning",
        BadgeTone::Danger => "danger",
    }
}

/// A [`StatusIcon`] as a cell-id slug.
fn icon_slug(icon: StatusIcon) -> &'static str {
    match icon {
        StatusIcon::Success => "ok",
        StatusIcon::Warning => "warn",
        StatusIcon::Danger => "fail",
        StatusIcon::Info => "info",
    }
}

/// A [`BadgeKind`] as the manifest's slug for what the cell is.
fn kind_slug(kind: &BadgeKind) -> &'static str {
    match kind {
        BadgeKind::Dot => "dot",
        BadgeKind::Count(_) => "count",
        BadgeKind::Status { .. } => "chip",
    }
}

/// The badge box for one resolved style, centred in the sample.
fn badge_box(style: &badge::BadgeStyle, content_width: f32, sample: Rectangle) -> Rectangle {
    let width = if style.shows_glyph {
        crate::widget::badge::chip_width(style, SAMPLE_GLYPH_WIDTH, content_width)
    } else {
        pill_width(style, content_width)
    };
    Rectangle::new(
        Point::new(
            sample.x + space_centre(sample.width, width),
            sample.y + (sample.height - style.height) / 2.0,
        ),
        iced::Size::new(width, style.height),
    )
}

/// Centre `inner` inside `outer` on one axis.
fn space_centre(outer: f32, inner: f32) -> f32 {
    if inner >= outer {
        0.0
    } else {
        (outer - inner) / 2.0
    }
}

/// The badge's own layer: its fill and its perimeter in one quad.
fn badge_layer(bounds: Rectangle, style: &badge::BadgeStyle) -> Layer {
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

/// The badge cell for one model (`UIF-ATOM-008`).
///
/// Resolved through [`badge::resolve_style`] — the same pure function a
/// renderer would paint from — and drawn as the one layer that function
/// describes. Nothing in this cell is a colour the gallery chose.
#[must_use]
pub fn badge_cell(model: &BadgeModel, id: &str) -> Cell {
    let palette = Palette::dark();
    let style = badge::resolve_style(model, &palette);
    let sample = atom_sample_at(badge::CHIP_HEIGHT);
    let bounds = badge_box(&style, SAMPLE_TEXT_WIDTH, sample);
    let layers = vec![("badge", badge_layer(bounds, &style))];
    let mount = palette.surface_raised;

    let mut audit = Vec::new();
    let mut resolved = vec![
        StyleEntry::text("kind", kind_slug(model.kind())),
        StyleEntry::text("tone", tone_slug(model.tone())),
        StyleEntry::colour("fill", style.fill),
        StyleEntry::colour("border.color", style.border),
        StyleEntry::number("border.width", style.border_width),
        StyleEntry::number("radius", style.radius),
        StyleEntry::number("geometry.height", style.height),
        StyleEntry::number("geometry.min-width", style.min_width),
        StyleEntry::number("geometry.padding-x", style.h_padding),
        StyleEntry::text(
            "chrome.class",
            chrome_class_label(badge::badge_chrome_class(&style, &palette)),
        ),
    ];

    if model.is_empty() {
        // Nothing is painted, so there is nothing to measure. The cell
        // exists to record that a zero count is invisible rather than to
        // hand the audit a transparent-on-surface ratio and call the
        // result a pass.
        resolved.push(StyleEntry::text("painted", "false"));
    } else {
        resolved.push(StyleEntry::text("painted", "true"));
    }

    match model.kind() {
        BadgeKind::Dot => {
            audit.push(AuditPair {
                channel: "badge.fill",
                foreground: style.fill,
                background: mount,
                role: ContrastRole::Chrome,
            });
        }
        BadgeKind::Count(_) => {
            if !model.is_empty() {
                if model.tone().paints_solid() {
                    audit.push(AuditPair {
                        channel: "badge.fill",
                        foreground: style.fill,
                        background: mount,
                        role: ContrastRole::Chrome,
                    });
                } else {
                    audit.push(AuditPair {
                        channel: "badge.border",
                        foreground: style.border,
                        background: mount,
                        role: ContrastRole::Chrome,
                    });
                }
                audit.push(AuditPair {
                    channel: "badge.text",
                    foreground: style.text,
                    background: style.fill,
                    role: ContrastRole::Text,
                });
            }
            resolved.push(StyleEntry::colour("text", style.text));
            resolved.push(StyleEntry::text(
                "fill.solid",
                model.tone().paints_solid().to_string(),
            ));
        }
        BadgeKind::Status { icon, label } => {
            audit.push(AuditPair {
                channel: "badge.border",
                foreground: style.border,
                background: mount,
                role: ContrastRole::Chrome,
            });
            audit.push(AuditPair {
                channel: "badge.text",
                foreground: style.text,
                background: style.fill,
                role: ContrastRole::Text,
            });
            audit.push(AuditPair {
                channel: "badge.glyph",
                foreground: style.glyph,
                background: style.fill,
                role: ContrastRole::Chrome,
            });
            resolved.push(StyleEntry::colour("text", style.text));
            resolved.push(StyleEntry::colour("glyph", style.glyph));
            resolved.push(StyleEntry::text("glyph.char", icon.glyph().to_string()));
            resolved.push(StyleEntry::text("label", (*label).to_string()));
        }
    }

    Cell {
        id: id.to_string(),
        kind: CellKind::Atom,
        subject: "badge",
        spec: "UIF-ATOM-008",
        namer: Namer::Standard,
        resolved,
        frame: crate::gallery::paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Every badge cell: the dot in five tones, the count pill at five
/// values in five tones, and the chip with four glyphs in five tones.
#[must_use]
pub fn badge_cells() -> Vec<Cell> {
    let mut cells = Vec::new();

    for tone in TONES {
        cells.push(badge_cell(
            &BadgeModel::dot().with_tone(tone),
            &format!("badge/dot/{}", tone_slug(tone)),
        ));
    }

    for value in COUNT_VALUES {
        for tone in TONES {
            cells.push(badge_cell(
                &BadgeModel::count(value).with_tone(tone),
                &format!("badge/count/{value}/{}", tone_slug(tone)),
            ));
        }
    }

    for icon in ICONS {
        for tone in TONES {
            cells.push(badge_cell(
                &BadgeModel::status(icon, "Status").with_tone(tone),
                &format!("badge/chip/{}-status/{}", icon_slug(icon), tone_slug(tone)),
            ));
        }
    }

    cells
}

#[cfg(test)]
#[path = "gallery_badge_tests.rs"]
mod gallery_badge_tests;
