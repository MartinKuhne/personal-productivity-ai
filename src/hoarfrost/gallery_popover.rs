//! The teaching-popover cells of the `UIF-VER-004` matrix (§3.7,
//! `UIF-ATOM-007`).
//!
//! Split out of [`gallery`] the same way [`crate::gallery_badge`] was, so
//! [`gallery::matrix`] still owns one list in one fixed order.
//!
//! # Six cells
//!
//! The card has no interaction states of its own — its *buttons* have
//! those, and they resolve through
//! [`dialog_button`](crate::surface::dialog_button) like every other
//! footer control — so the two axes that actually change what is painted
//! are the width regime and the footer shape:
//!
//! * three width regimes through [`clamp_width`]: a measurement below the
//!   floor (raised to `WIDTH_MIN`), one inside the band (passed through),
//!   and one above the ceiling (lowered to `WIDTH_MAX`);
//! * two footer shapes: with a primary action, and without one, where the
//!   framework-supplied [`DISMISS_LABEL`] button is all that is left.
//!
//! # Why the width regime is a cell and not a footnote
//!
//! §3.7 gives the card a *range*, and a range is only a rule if the ends
//! are enforced. Each regime cell records the measurement that went in and
//! the width that came out, so a resolver that stopped clamping changes
//! the snapshot rather than silently painting a `520px` callout.
//!
//! # The audit reproduces §3.7's printed figure
//!
//! Three channels, all measured against the card's own `surface.overlay`
//! fill, which is the pairing the section's contrast column is describing:
//!
//! * `popover.border-on-card` — `accent.border` on `surface.overlay`,
//!   recomputed at **`5.9394:1`** against the `3.0:1` rule 2 floor;
//! * `popover.title-on-card` — `text.strong` on `surface.overlay`,
//!   **`17.7168:1** against the `4.5:1` rule 1 floor;
//! * `popover.body-on-card` — `text.primary` on `surface.overlay`,
//!   **`11.7180:1** against the same floor.
//!
//! The card's fill is deliberately **not** audited against the surface
//! behind it. `surface.overlay` sits `1.1230:1` from `surface.base`, and
//! a popover is not required to be visible against its own background by
//! contrast — the perimeter does that work, which is why §3.7 gives the
//! card an `accent.border` edge rather than a heavier fill.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_popover_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame, viewport,
};
use crate::surface::popover::{self, DISMISS_LABEL, PopoverStyle, clamp_width};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::widget::button_widget::Layer;
use iced::{Border, Rectangle};

/// A nominal card height for a callout with a footer.
///
/// A harness figure rather than a visual magnitude: §3.7 fixes the width
/// band and the padding, and lets the height follow the prose. One
/// representative height per footer shape lets the frame be drawn without
/// the gallery laying out real text.
const CARD_HEIGHT_WITH_FOOTER: f32 = 132.0;

/// A nominal card height for a callout whose footer is dismiss-only.
const CARD_HEIGHT_DISMISS_ONLY: f32 = 104.0;

/// The width regimes the matrix samples, in order.
///
/// Chosen at the ends of §3.7's band rather than at round numbers: the
/// point of the cell is what the clamp *did*.
const WIDTH_REGIMES: [(&str, f32); 3] = [
    ("under-min", 180.0),
    ("in-band", 320.0),
    ("over-max", 520.0),
];

/// A [`ChromeClass`] as the manifest names it.
fn chrome_class_label(class: ChromeClass) -> &'static str {
    match class {
        ChromeClass::Judged => "judged",
        ChromeClass::CoincidentWithFill => "coincident-with-fill",
        ChromeClass::UnpaintedBackground => "unpainted-background",
        ChromeClass::RestingDivider => "resting-divider",
    }
}

/// The card rectangle, centred in the viewport.
fn card_rect(width: f32, height: f32) -> Rectangle {
    let view = viewport();
    Rectangle::new(
        iced::Point::new(
            view.x
                + if width >= view.width {
                    0.0
                } else {
                    (view.width - width) / 2.0
                },
            view.y
                + if height >= view.height {
                    0.0
                } else {
                    (view.height - height) / 2.0
                },
        ),
        iced::Size::new(width, height),
    )
}

/// The popover cell for one `(width regime, footer shape)` pair
/// (`UIF-ATOM-007`).
///
/// Resolved through [`PopoverStyle::resolve`] and [`clamp_width`] — the
/// same pure functions the host paints from. Nothing in this cell is a
/// colour the gallery chose.
#[must_use]
pub fn popover_cell(regime_slug: &str, measured: f32, has_primary: bool) -> Cell {
    let palette = Palette::dark();
    let style = PopoverStyle::resolve(&palette);
    let width = clamp_width(measured);
    let height = if has_primary {
        CARD_HEIGHT_WITH_FOOTER
    } else {
        CARD_HEIGHT_DISMISS_ONLY
    };
    let card = card_rect(width, height);

    let layers: Vec<(&'static str, Layer)> = vec![("card", card_layer(card, &style))];

    Cell {
        id: format!("popover/{regime_slug}/{}", footer_slug(has_primary)),
        kind: CellKind::Composite,
        subject: "popover",
        spec: "UIF-ATOM-007",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("card.fill", style.background),
            StyleEntry::colour("card.border.color", style.border_color),
            StyleEntry::number("card.border.width", style.border_width),
            StyleEntry::number("card.radius", style.radius),
            StyleEntry::number("card.padding", style.padding),
            StyleEntry::colour("title", style.title_color),
            StyleEntry::number("title.size", style.title_size),
            StyleEntry::text("title.weight", weight_slug(style.title_weight)),
            StyleEntry::colour("body", style.body_color),
            StyleEntry::number("body.size", style.body_size),
            StyleEntry::text("body.weight", weight_slug(style.body_weight)),
            StyleEntry::number("card.width.measured", measured),
            StyleEntry::number("card.width.resolved", width),
            StyleEntry::number("card.width.min", style.width_min),
            StyleEntry::number("card.width.max", style.width_max),
            StyleEntry::number("card.height", height),
            StyleEntry::text("footer.shape", footer_shape(has_primary)),
            StyleEntry::text("footer.dismiss-label", DISMISS_LABEL),
            StyleEntry::text(
                "chrome.class",
                chrome_class_label(popup_chrome_class(&style, &palette)),
            ),
        ],
        frame: paint_frame(Namer::Standard, viewport(), &layers),
        audit: vec![
            AuditPair {
                channel: "popover.border-on-card",
                foreground: style.border_color,
                background: style.background,
                role: ContrastRole::Chrome,
            },
            AuditPair {
                channel: "popover.title-on-card",
                foreground: style.title_color,
                background: style.background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "popover.body-on-card",
                foreground: style.body_color,
                background: style.background,
                role: ContrastRole::Text,
            },
        ],
        note: Some(
            "Every figure here is what §3.7 specifies: `surface.overlay` fill, `1px \
             accent.border`, `radius.card`, `space.md` padding, a 280-360px width band, \
             a 13px SemiBold title over a 13px Regular body. The dismiss button's label is \
             framework-supplied: §3.7 names the button but not its word."
                .to_string(),
        ),
    }
}

/// The card's own layer: its fill and its perimeter in one quad.
fn card_layer(bounds: Rectangle, style: &PopoverStyle) -> Layer {
    Layer {
        bounds,
        fill: style.background,
        border: Border {
            color: style.border_color,
            width: style.border_width,
            radius: style.radius.into(),
        },
    }
}

/// The card perimeter's classification.
///
/// A thin alias so this module reads against its own docs rather than
/// reaching into the popover module at the call site.
fn popup_chrome_class(style: &PopoverStyle, palette: &Palette) -> ChromeClass {
    popover::card_chrome_class(style, palette)
}

/// A [`Weight`](crate::token::typography::Weight) as a manifest value.
fn weight_slug(weight: crate::token::typography::Weight) -> &'static str {
    match weight {
        crate::token::typography::Weight::Regular => "regular",
        crate::token::typography::Weight::Medium => "medium",
        crate::token::typography::Weight::SemiBold => "semibold",
        crate::token::typography::Weight::Bold => "bold",
    }
}

/// The footer shape as a cell-id slug.
fn footer_slug(has_primary: bool) -> &'static str {
    if has_primary {
        "with-action"
    } else {
        "dismiss-only"
    }
}

/// The footer shape as a manifest value.
fn footer_shape(has_primary: bool) -> &'static str {
    if has_primary {
        "action+dismiss"
    } else {
        "dismiss"
    }
}

/// Every teaching-popover cell: three width regimes × two footer shapes.
#[must_use]
pub fn popover_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for (slug, measured) in WIDTH_REGIMES {
        for has_primary in [true, false] {
            cells.push(popover_cell(slug, measured, has_primary));
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_popover_tests.rs"]
mod gallery_popover_tests;
