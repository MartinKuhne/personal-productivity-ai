//! The split-button cells of the `UIF-VER-004` matrix
//! (§3.1c, `UIF-ATOM-001c`).
//!
//! Built through [`gallery`]'s own vocabulary; [`gallery::matrix`] calls
//! [`split_button_cells`] back so the matrix remains one list in one
//! fixed order.
//!
//! # Forty cells
//!
//! Two variants × two zones × two heights × the five §3.3 state columns.
//! The zone is its own axis because §3.1c's whole claim is that the control
//! is *two* hit targets, and a matrix that sampled only "the control" could
//! not show that. Each cell paints the sampled half in the column's state
//! and the other half at `Rest`, so the hover-independence the host pins in
//! tests is visible in the frame as well.
//!
//! # The seam is recorded but not audited
//!
//! §3.1c resolves the `1px` internal divider per variant: `border.strong`
//! on `Secondary` (`4.2379:1` on `surface.raised`) and `text.strong` on
//! `Primary` (`4.8020:1` on `accent.fill`), because `border.strong`
//! against `accent.fill` measures `1.0876:1` and `text.muted` `1.8363:1`
//! — both named as failures.
//!
//! The seam is carried in `resolved` as `divider.color` and
//! `divider.contrast-on-fill` and deliberately kept out of `audit`. That
//! is not because the number is bad — it is legible on both variants now —
//! but because a 1px seam is not a foreground. Scoring it against the
//! text-contrast floors the audit applies would judge the wrong kind of
//! thing, so the figure is reported rather than judged.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_split_button_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::widget::button::ButtonVariant;
use crate::widget::button_widget::Layer;
use crate::widget::split_button::{self, SplitZone};
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

/// The two variants §3.1c allows, in spec order.
const VARIANTS: [ButtonVariant; 2] = [ButtonVariant::Primary, ButtonVariant::Secondary];

/// The two height rungs §3.1c prices, in spec order.
const HEIGHTS: [f32; 2] = [split_button::HEIGHT_COMPACT, split_button::HEIGHT_STANDARD];

/// A nominal label width for the geometry entries.
const SAMPLE_LABEL_WIDTH: f32 = 72.0;

/// What the label-half cell records about its seam.
const NOTE_PRIMARY_ZONE_SECONDARY: &str = "seam is border.strong at 4.2379:1 on \
    surface.raised; recorded, not audited \u{2014} a 1px seam is not a foreground";

/// What the label-half cell records about its seam on a Primary fill.
const NOTE_PRIMARY_ZONE_PRIMARY: &str = "seam is text.strong at 4.8020:1 on \
    accent.fill, per UIF-TOK-006 rule 4; recorded, not audited";

/// What the menu-half cell records about its chevron ink.
const NOTE_MENU_ZONE: &str = "chevron takes the menu half's own foreground, as \
    §3.1c names it; the combobox muted-glyph precedent fails on a Primary fill";

/// A [`SplitZone`] as a cell-id slug.
fn zone_slug(zone: SplitZone) -> &'static str {
    match zone {
        SplitZone::Primary => "label",
        SplitZone::Menu => "chevron",
        SplitZone::Outside => "outside",
    }
}

/// A [`ButtonVariant`] as a cell-id slug.
fn variant_slug(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "primary",
        ButtonVariant::Secondary => "secondary",
        ButtonVariant::Ghost => "ghost",
        ButtonVariant::Danger => "danger",
    }
}

/// A height rung as a cell-id slug.
fn height_slug(height: f32) -> &'static str {
    if (height - split_button::HEIGHT_COMPACT).abs() < f32::EPSILON {
        "compact"
    } else {
        "standard"
    }
}

/// The control's box for one label width, centred in the sample column.
fn control_box(height: f32, sample: Rectangle) -> Rectangle {
    let width = split_button::control_width(SAMPLE_LABEL_WIDTH);
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

/// One half's own layer: its fill and perimeter in one quad.
fn half_layer(bounds: Rectangle, style: &crate::widget::button::ButtonStyle) -> Layer {
    Layer {
        bounds,
        fill: style.background,
        border: Border {
            color: style.border_color,
            width: style.border_width,
            radius: split_button::RADIUS.into(),
        },
    }
}

/// The divider's own layer, drawn at the seam in the variant's own token.
fn divider_layer(control: Rectangle, variant: ButtonVariant) -> Layer {
    Layer {
        bounds: split_button::divider_bounds(control),
        fill: split_button::divider_color(variant, &Palette::dark()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
    }
}

/// The label-half note for one variant's seam.
fn seam_note(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => NOTE_PRIMARY_ZONE_PRIMARY,
        ButtonVariant::Secondary => NOTE_PRIMARY_ZONE_SECONDARY,
        ButtonVariant::Ghost | ButtonVariant::Danger => NOTE_PRIMARY_ZONE_SECONDARY,
    }
}

/// The control-level focus ring drawn outside the whole control.
fn focus_ring_layer(bounds: Rectangle, color: Color, width: f32) -> Option<Layer> {
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
            radius: (split_button::RADIUS + width).into(),
        },
    })
}

/// The split-button cell for one `(variant, zone, height, state)`
/// quadruple (`UIF-ATOM-001c`).
///
/// Resolved through [`split_button::resolve_style`], which routes each
/// half through the button atom — so the fill/foreground lock the button
/// asserts is asserted here by the same code path.
#[must_use]
pub fn split_button_cell(
    variant: ButtonVariant,
    zone: SplitZone,
    height: f32,
    state: VisualState,
) -> Cell {
    let palette = Palette::dark();
    let rest = COLUMNS[0];
    let style = match zone {
        SplitZone::Primary => split_button::resolve_style(variant, state, rest, &palette),
        _ => split_button::resolve_style(variant, rest, state, &palette),
    };

    let sample = atom_sample_at(height);
    let control = control_box(height, sample);
    let primary = split_button::primary_zone(control);
    let menu = split_button::menu_zone(control);
    let sampled_bounds = match zone {
        SplitZone::Primary => primary,
        _ => menu,
    };
    let sampled_style = match zone {
        SplitZone::Primary => style.primary,
        _ => style.menu,
    };

    let mut layers: Vec<(&'static str, Layer)> = vec![
        ("primary-half", half_layer(primary, &style.primary)),
        ("menu-half", half_layer(menu, &style.menu)),
        ("divider", divider_layer(control, variant)),
    ];
    if let Some(layer) = focus_ring_layer(
        control,
        sampled_style.focus_ring_color,
        sampled_style.focus_ring_width,
    ) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![
        AuditPair {
            channel: "half.border",
            foreground: sampled_style.border_color,
            background: sampled_style.background,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "half.text",
            foreground: sampled_style.foreground,
            background: sampled_style.background,
            role: ContrastRole::Text,
        },
    ];
    // The chevron is only painted on the menu half, so only that half's
    // cell judges it. This is where the substitution §3.4's `text.muted`
    // combobox precedent forces (1.84:1 on `accent.fill`) gets measured
    // rather than merely recorded.
    if zone == SplitZone::Menu {
        audit.push(AuditPair {
            channel: "chevron",
            foreground: split_button::chevron_color(&style),
            background: sampled_style.background,
            role: ContrastRole::Chrome,
        });
    }

    let divider_on_fill = contrast_ratio(style.divider, sampled_style.background);

    Cell {
        id: format!(
            "split-button/{}/{}-{}-{}",
            variant_slug(variant),
            zone_slug(zone),
            height_slug(height),
            state_slug(state)
        ),
        kind: CellKind::Atom,
        subject: "split-button",
        spec: "UIF-ATOM-001c",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("variant", variant_slug(variant)),
            StyleEntry::text("zone", zone_slug(zone)),
            StyleEntry::colour("half.fill", sampled_style.background),
            StyleEntry::colour("half.text", sampled_style.foreground),
            StyleEntry::colour("half.border.color", sampled_style.border_color),
            StyleEntry::number("half.border.width", sampled_style.border_width),
            StyleEntry::colour("chevron.color", split_button::chevron_color(&style)),
            StyleEntry::colour("divider.color", style.divider),
            StyleEntry::number("divider.width", split_button::DIVIDER_WIDTH),
            StyleEntry::number("divider.contrast-on-fill", divider_on_fill),
            StyleEntry::number("chevron.zone-width", split_button::CHEVRON_ZONE_WIDTH),
            StyleEntry::number("primary.padding-x", split_button::PRIMARY_PADDING),
            StyleEntry::number("control.height", height),
            StyleEntry::number("control.width", control.width),
            StyleEntry::number("primary.zone.width", primary.width),
            StyleEntry::number("menu.zone.width", menu.width),
            StyleEntry::number("sampled.zone.width", sampled_bounds.width),
            StyleEntry::number("focus-ring.width", sampled_style.focus_ring_width),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: Some(if zone == SplitZone::Primary {
            seam_note(variant).to_string()
        } else {
            NOTE_MENU_ZONE.to_string()
        }),
    }
}

/// Every split-button cell: two variants × two zones × two rungs × the
/// five §3.3 columns, in that nesting order.
#[must_use]
pub fn split_button_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in VARIANTS {
        for zone in [SplitZone::Primary, SplitZone::Menu] {
            for height in HEIGHTS {
                for state in COLUMNS {
                    cells.push(split_button_cell(variant, zone, height, state));
                }
            }
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_split_button_tests.rs"]
mod gallery_split_button_tests;
