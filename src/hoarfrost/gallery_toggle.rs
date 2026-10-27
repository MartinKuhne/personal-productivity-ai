//! The toggle-family cells of the `UIF-VER-004` matrix (§3.3,
//! `UIF-ATOM-003`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`toggle_cells`] back, so
//! the matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # Thirty cells
//!
//! Three controls × two polarities × the five columns §3.3 prints:
//! Rest, Hovered, Pressed, Focused and Disabled. The Focused column is
//! `Rest + ring`, which is how the spec words that cell ("Base + `2px`
//! … outer focus ring") and how [`crate::state`] models focus as
//! additive rather than as a sixth base state.
//!
//! # What each cell paints
//!
//! The indicator, the mark where the mark is geometry, and the ring —
//! the same three layers a widget would hand the renderer, built by
//! [`toggle`]’s own layer functions rather than by the gallery guessing
//! at them.
//!
//! The checkbox's `✓` is deliberately **not** in the frame. It is a
//! font glyph, and recording a filled rectangle in its place would put
//! a quad in the manifest that no renderer was ever told was a tick.
//! The mark's colour is still carried in `resolved.mark` and still goes
//! through the audit, so nothing about the pairing goes unmeasured — the
//! frame simply does not claim to have drawn something it cannot draw.
//!
//! # The label's audit background is a stated choice
//!
//! A toggle's label sits on the container, not on the indicator, so the
//! cell audits it against `surface.overlay` — the surface the gallery
//! mounts atoms on, and the darker of the two card surfaces a settings
//! row or dialog is drawn against, which makes it the conservative
//! choice. That is a stated assumption about the mount rather than a
//! fact about every mount. The indicator pairs, which *are* the atom's
//! own business, are audited against the indicator's own fill.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_toggle_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, Rung, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::token::space;
use crate::widget::button_widget::Layer;
use crate::widget::toggle::{self, ToggleControl, TogglePolarity};
use iced::Point;

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

/// The three controls, in §3.3 order.
const CONTROLS: [ToggleControl; 3] = [
    ToggleControl::Checkbox,
    ToggleControl::Radio,
    ToggleControl::Switch,
];

/// The two polarity rows, in spec order.
const POLARITIES: [TogglePolarity; 2] = [TogglePolarity::Off, TogglePolarity::On];

/// A nominal label width for the geometry entries.
///
/// A harness figure rather than a visual magnitude: the hit-target rule
/// is about the *relationship* between indicator, gap and label, and one
/// representative width shows the row extending across a label without
/// the cell having to measure real text. The no-label case is reported
/// alongside it, so both ends of the rule are visible.
const SAMPLE_LABEL_WIDTH: f32 = 96.0;

/// A [`ToggleControl`] as a cell-id slug.
fn control_slug(control: ToggleControl) -> &'static str {
    match control {
        ToggleControl::Checkbox => "checkbox",
        ToggleControl::Radio => "radio",
        ToggleControl::Switch => "switch",
    }
}

/// A [`TogglePolarity`] as a cell-id slug.
fn polarity_slug(polarity: TogglePolarity) -> &'static str {
    match polarity {
        TogglePolarity::Off => "off",
        TogglePolarity::On => "on",
    }
}

/// How this control's mark reaches the screen.
///
/// Recorded because the audit measures a colour that three different
/// things consume in three different ways, and a reader of the manifest
/// should be able to tell which one a row is.
fn mark_rendered(control: ToggleControl) -> &'static str {
    match control {
        ToggleControl::Checkbox => "font-glyph",
        ToggleControl::Radio => "inner-dot",
        ToggleControl::Switch => "thumb",
    }
}

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

/// The toggle cell for one `(control, polarity, state)` triple
/// (`UIF-ATOM-003`).
///
/// Resolved through [`toggle::resolve_style`] — the same pure function a
/// widget would paint from — and drawn as the layers that function's
/// companion builders produce. Nothing in this cell is a colour the
/// gallery chose: every value comes off the resolver.
#[must_use]
pub fn toggle_cell(control: ToggleControl, polarity: TogglePolarity, state: VisualState) -> Cell {
    let palette = Palette::dark();
    let style = toggle::resolve_style(polarity, state, &palette);

    let sample = atom_sample_at(Rung::Standard.height());
    let (_, indicator_h) = toggle::indicator_size(control);
    let indicator = toggle::indicator_rect(
        control,
        Point::new(space::SM, (sample.height - indicator_h) / 2.0),
    );

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    layers.push((
        "indicator",
        toggle::indicator_layer(indicator, control, &style),
    ));
    if let Some(mark) = toggle::mark_layer(indicator, control, polarity, &style) {
        layers.push(("mark", mark));
    }
    if let Some(ring) = toggle::focus_ring_layer(indicator, control, &style) {
        layers.push(("focus-ring", ring));
    }

    let mut audit = vec![
        AuditPair {
            channel: "indicator.border",
            foreground: style.indicator_border,
            background: style.indicator_fill,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "indicator.mark",
            foreground: style.mark,
            background: style.indicator_fill,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "label",
            foreground: style.label,
            background: palette.surface_overlay,
            role: ContrastRole::Text,
        },
    ];
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "focus-ring",
            foreground: style.focus_ring_color,
            background: style.indicator_fill,
            role: ContrastRole::Chrome,
        });
    }

    let (hit_w, hit_h) = toggle::hit_size(control, SAMPLE_LABEL_WIDTH);
    let (bare_w, _) = toggle::hit_size(control, 0.0);

    Cell {
        id: format!(
            "{}/{}/{}",
            control_slug(control),
            polarity_slug(polarity),
            state_slug(state)
        ),
        kind: CellKind::Atom,
        subject: control_slug(control),
        spec: "UIF-ATOM-003",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("indicator.fill", style.indicator_fill),
            StyleEntry::colour("indicator.border.color", style.indicator_border),
            StyleEntry::number("indicator.border.width", style.indicator_border_width),
            StyleEntry::number("indicator.radius", toggle::indicator_radius(control)),
            StyleEntry::colour("mark", style.mark),
            StyleEntry::text("mark.rendered", mark_rendered(control)),
            StyleEntry::colour("label", style.label),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::text(
                "chrome.class",
                chrome_class_label(toggle::indicator_chrome_class(
                    style.indicator_border,
                    style.indicator_fill,
                    &palette,
                )),
            ),
            StyleEntry::number("geometry.indicator.width", indicator.width),
            StyleEntry::number("geometry.indicator.height", indicator.height),
            StyleEntry::number("geometry.label-gap", toggle::LABEL_GAP),
            StyleEntry::number("geometry.hit.width", hit_w),
            StyleEntry::number("geometry.hit.width-no-label", bare_w),
            StyleEntry::number("geometry.hit.height", hit_h),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Every toggle cell: three controls × two polarities × the five §3.3
/// columns, in that nesting order.
#[must_use]
pub fn toggle_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for control in CONTROLS {
        for polarity in POLARITIES {
            for state in COLUMNS {
                cells.push(toggle_cell(control, polarity, state));
            }
        }
    }
    cells
}

#[cfg(test)]
#[path = "gallery_toggle_tests.rs"]
mod gallery_toggle_tests;
