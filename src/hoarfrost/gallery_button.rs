//! The button cells of the `UIF-VER-004` matrix (`UIF-ATOM-001`, `UIF-ATOM-001a`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`button_cells`] and
//! [`button_controlled_cells`] back, so the matrix is still one list in
//! one fixed order — the split moved the code, not the artifact.
//!
//! # Two families
//!
//! * [`button_cells`] — the base `(variant × state)` grid: one cell per
//!   [`ButtonVariant`] at every reachable [`VisualState`], each resolved
//!   through `button::resolve_style` and drawn as the two layers
//!   `button_widget` would paint.
//! * [`button_controlled_cells`] — the `000b` additions that grid cannot
//!   reach: the busy cell, the collapsed cell, and one cell per binding
//!   tier in [`BUTTON_TIERS`].
//!
//! The tier cells are driven through a real [`ButtonController`] rather
//! than described beside one. Each tier is produced by the emission the
//! controller actually makes, and the cell's note line says how a user
//! produces it — so a tier no gesture can reach would show up as a cell
//! that cannot be built.
//!
//! `cell_height` and the variant slug are this family's own helpers and
//! moved with it; the helpers other split modules share stayed in
//! `gallery`.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, REST_STATE, RUNGS, Rung, StyleEntry,
    StyleValue, atom_sample_at, paint_frame, snapshot_states, state_slug,
};
use crate::state::VisualState;
use crate::token::color::Palette;
use crate::widget::button::{self, ButtonVariant};
use crate::widget::button_controller::ActivationSource;
use crate::widget::button_controller::ButtonController;
use crate::widget::button_controller::ButtonEmission;
use crate::widget::button_model::ButtonDensity;
use crate::widget::button_model::ButtonModel;
use crate::widget::button_model::SPINNER_SLOT;
use crate::widget::button_widget::Layer;
use crate::widget::button_widget::background_layer as button_background_layer;
use crate::widget::button_widget::focus_ring_layer as button_focus_ring_layer;

/// The button cell for one `(variant, state)` pair (`UIF-ATOM-001`).
///
/// Resolved through [`button::resolve_style`] — the same pure function the
/// widget paints from — and drawn as the two layers `button_widget`
/// produces for that style: the fill-and-border layer, then the focus ring
/// outside it. A Ghost at rest resolves a fully transparent fill and
/// border, which is the cell the spec prints as `transparent`; the frame
/// records that by naming the quad `transparent` rather than by the
/// gallery deciding to leave it out.
#[must_use]
pub fn button_cell(variant: ButtonVariant, state: VisualState) -> Cell {
    button_cell_at(variant, state, Rung::Standard)
}

/// The button cell for one `(variant, state, rung)` triple
/// (`UIF-ATOM-001`, §3.1's two control heights).
///
/// Resolved through [`button::resolve_style`] — the same pure function the
/// widget paints from — and drawn as the two layers `button_widget`
/// produces for that style: the fill-and-border layer, then the focus ring
/// outside it. A Ghost at rest resolves a fully transparent fill and
/// border, which is the cell the spec prints as `transparent`; the frame
/// records that by naming the quad `transparent` rather than by the
/// gallery deciding to leave it out.
#[must_use]
pub fn button_cell_at(variant: ButtonVariant, state: VisualState, rung: Rung) -> Cell {
    let palette = Palette::dark();
    let style = button::resolve_style(variant, state, &palette);
    let bounds = atom_sample_at(rung.height());

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = button_background_layer(bounds, &style) {
        layers.push(("background", layer));
    }
    if let Some(layer) = button_focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![AuditPair {
        channel: "label",
        foreground: style.foreground,
        background: style.background,
        role: ContrastRole::Text,
    }];

    if style.border_width > 0.0 {
        audit.push(AuditPair {
            channel: "border",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "ring",
            foreground: style.focus_ring_color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!(
            "button/{}/{}{}",
            slug(variant),
            state_slug(state),
            rung.slug()
        ),
        kind: CellKind::Atom,
        subject: "button",
        spec: "UIF-ATOM-001",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("foreground", style.foreground),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::text("geometry.height-token", rung.token()),
            StyleEntry::number("geometry.radius", button::RADIUS),
            StyleEntry::number("geometry.padding-x", button::HORIZONTAL_PADDING),
            StyleEntry::text(
                "transparent-background",
                style.is_transparent_background().to_string(),
            ),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// The four §3.1 variants in the order the spec prints them.
pub const BUTTON_VARIANTS: [ButtonVariant; 4] = [
    ButtonVariant::Primary,
    ButtonVariant::Secondary,
    ButtonVariant::Ghost,
    ButtonVariant::Danger,
];

/// Every button cell: four variants across every reachable state, at both
/// control-height rungs (§3.1's `28px` / `32px` pair).
#[must_use]
pub fn button_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            for rung in RUNGS {
                cells.push(button_cell_at(variant, state, rung));
            }
        }
    }
    cells
}

/// A [`ButtonVariant`] as a cell-id slug.
///
/// `pub(crate)` because the gallery's own test sidecar asserts the id
/// shape; nothing outside the crate should depend on it.
pub(crate) fn slug(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "primary",
        ButtonVariant::Secondary => "secondary",
        ButtonVariant::Ghost => "ghost",
        ButtonVariant::Danger => "danger",
    }
}

// ---------------------------------------------------------------------------
// `UIF-ATOM-001a` — the controlled button's busy, collapse and tier cells
// ---------------------------------------------------------------------------
//
// The cells above snapshot what a button *looks like*. These snapshot what it
// *does*: the busy projection a host puts on the model, the rung a declared
// density collapses to and the tooltip that collapse then owes, and the three
// tiers the activation engine actually emits. The tier cells are produced by
// driving a real [`ButtonController`] rather than by writing the tier name
// down, so a gallery frame cannot claim a behaviour the engine does not have.

/// The gesture outcome a tier cell records, and how a user produces it.
pub const BUTTON_TIERS: [(&str, &str); 3] = [
    ("activated", "release inside the bounds"),
    ("cancelled", "release outside, or Escape"),
    ("cancel-requested", "press while the host reports busy"),
];

/// The `UIF-ATOM-001a` busy cell for one variant.
///
/// The paint is deliberately identical to that variant's `rest` cell.
/// §3.0.4.3 forbids a busy control from resizing or re-skinning itself, so
/// the only thing a busy button gains is the **reserved spinner slot** — and
/// the slot is empty, because the glyph is `UIF-ATOM-009`'s deliverable and
/// this increment reserves space rather than inventing a paint.
#[must_use]
pub fn button_busy_cell(variant: ButtonVariant) -> Cell {
    let model = ButtonModel::new("Saving").busy().cancellable();
    let mut cell = button_cell_at(variant, REST_STATE, Rung::Standard);

    cell.id = format!("button/busy/{}", slug(variant));
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("busy", model.busy.to_string()),
        StyleEntry::text("cancellable", model.cancellable.to_string()),
        StyleEntry::text("pending", "indeterminate"),
        StyleEntry::text("spinner.reserved", "true"),
        StyleEntry::number("spinner.slot-width", SPINNER_SLOT),
        StyleEntry::text("spinner.slot-token", "size.icon.sm"),
        StyleEntry::number("bounds.height", cell_height(&cell)),
    ]);
    cell.note = Some(
        "busy holds the outer bounds and reserves the spinner slot; the glyph itself is \
         UIF-ATOM-009, so the slot renders empty"
            .to_string(),
    );
    cell
}

/// The `UIF-ATOM-001a` density-collapse cell for one variant.
///
/// Records the rung a declared `Compact` density resolves to, the label mode
/// that rung forces, the tooltip that mode therefore makes mandatory, and
/// the zero vertical shift §3.0.4.4 fixes.
#[must_use]
pub fn button_collapsed_cell(variant: ButtonVariant) -> Cell {
    let model = ButtonModel::new("Save")
        .with_variant(variant)
        .with_density(ButtonDensity::Compact)
        .with_tooltip("Save this document");
    let layout = model.responsive();
    let mut cell = button_cell_at(variant, REST_STATE, Rung::Compact);

    cell.id = format!("button/collapsed/{}", slug(variant));
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("density", "compact"),
        StyleEntry::text("breakpoint", format!("{:?}", layout.breakpoint())),
        StyleEntry::text("label-mode", format!("{:?}", layout.label_mode())),
        StyleEntry::text(
            "icon-only",
            model.is_icon_only(layout.label_mode()).to_string(),
        ),
        StyleEntry::text("tooltip.required", "true"),
        StyleEntry::text(
            "tooltip.present",
            model
                .tooltip_for(layout.label_mode())
                .unwrap_or("<MISSING>")
                .to_string(),
        ),
        StyleEntry::number("padding-x", layout.horizontal_padding()),
        StyleEntry::number("vertical-shift", layout.vertical_shift()),
    ]);
    cell.note = Some(
        "the collapsed rung drops the label and therefore owes a tooltip (UIF-ATOM-007); \
         the rung shift is 0px by construction"
            .to_string(),
    );
    cell
}

/// The `UIF-ATOM-001a` tier cell for one gesture outcome.
///
/// Drives a real [`ButtonController`] and records the tier it actually
/// emitted, so the cell is a transcript rather than a caption.
#[must_use]
pub fn button_tier_cell(tier: &'static str) -> Cell {
    let idle = ButtonModel::new("Save").cancellable();
    let busy = ButtonModel::new("Saving").busy().cancellable();

    let mut controller = ButtonController::new();
    let emission = match tier {
        "activated" => {
            controller.begin(&idle, ActivationSource::Pointer);
            controller
                .commit(&idle, ActivationSource::Pointer)
                .expect("a live button commits")
        }
        "cancelled" => {
            controller.begin(&idle, ActivationSource::Pointer);
            controller.cancel(&idle).expect("an open gesture cancels")
        }
        "cancel-requested" => controller
            .begin(&busy, ActivationSource::Pointer)
            .expect("a busy cancellable button requests"),
        other => panic!("unknown tier {other}"),
    };

    let mut cell = button_cell_at(ButtonVariant::Primary, REST_STATE, Rung::Standard);
    cell.id = format!("button/tier/{tier}");
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text(
            "dispatched",
            matches!(emission, ButtonEmission::Activated(_)).to_string(),
        ),
        StyleEntry::text(
            "restores-activation",
            match emission.settlement() {
                Some(settlement) => format!("{:?}", settlement.activated),
                None => "n/a".to_string(),
            },
        ),
        StyleEntry::text(
            "pending-at-settle",
            format!("{:?}", emission.settlement().map(|s| s.pending)),
        ),
        StyleEntry::text("fired-count", controller.fired().to_string()),
        StyleEntry::text("suppressed-count", controller.suppressed().to_string()),
    ]);
    cell.note = Some(format!("produced by: {}", how(tier)));
    cell
}

/// How a user produces one of the [`BUTTON_TIERS`] outcomes.
///
/// `pub(crate)` only because the split preserved an existing cross-family
/// call: `gallery_input::text_input_tier_cell` asks this function for its
/// note line, so it reads `BUTTON_TIERS` for a *text-input* tier name and
/// answers `"unknown"` for every tier that is not a button's. That is the
/// behaviour the gallery had before this file existed and is deliberately
/// unchanged here — see the note at that call site.
pub(crate) fn how(tier: &'static str) -> &'static str {
    BUTTON_TIERS
        .iter()
        .find(|(name, _)| *name == tier)
        .map(|(_, how)| *how)
        .unwrap_or("unknown")
}

/// The height a cell recorded, so a busy cell can assert it did not change.
fn cell_height(cell: &Cell) -> f32 {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == "geometry.height")
        .and_then(|entry| match entry.value {
            StyleValue::Number(value) => Some(value),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// Every `UIF-ATOM-001a` button cell: busy and collapsed per variant, plus
/// the three activation tiers.
#[must_use]
pub fn button_controlled_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in BUTTON_VARIANTS {
        cells.push(button_busy_cell(variant));
        cells.push(button_collapsed_cell(variant));
    }
    for (tier, _) in BUTTON_TIERS {
        cells.push(button_tier_cell(tier));
    }
    cells
}
