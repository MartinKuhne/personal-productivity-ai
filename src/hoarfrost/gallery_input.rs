//! The text-input cells of the `UIF-VER-004` matrix (`UIF-ATOM-002`,
//! `UIF-ATOM-002a`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`text_input_cells`] and
//! [`text_input_tier_cells`] back, so the matrix is still one list in one
//! fixed order — the split moved the code, not the artifact.
//!
//! # Two families
//!
//! * [`text_input_cells`] and [`text_input_cell_at`] — the
//!   `(kind × state)` grid, plus the §3.2 `32px` / `28px`
//!   compact-toolbar rung. The invalid-and-focused combination the atom
//!   exists to settle is in here: the error keeps the perimeter and focus
//!   relocates to a ring drawn outside the control.
//! * [`text_input_tier_cells`] — the `000a` editing outcomes in
//!   [`TEXT_INPUT_TIERS`], each driven through a real
//!   [`TextInputController`] so the cell is a transcript of what the
//!   wire carried rather than a caption beside it.
//!
//! `slot_label` and the kind slug moved with this family: both exist to
//! name a [`SlotContent`] or an [`InputKind`] inside a cell id, and this
//! is the only family that emits either.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::binding::Live;
use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, REST_STATE, RUNGS, Rung, StyleEntry,
    atom_sample_at, paint_frame, rect_text, snapshot_states, state_slug,
};
use crate::state::BaseState;
use crate::state::VisualState;
use crate::token::color::Palette;
use crate::widget::button_widget::Layer;
use crate::widget::text_input::{self, InputKind, SlotContent};
use crate::widget::text_input_controller::TextInputController;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::TextInputModel;

/// A [`SlotContent`] as the manifest names it.
fn slot_label(slot: SlotContent) -> String {
    match slot {
        SlotContent::Empty => "empty".to_string(),
        SlotContent::Passive(glyph) => format!("passive {glyph}"),
        SlotContent::Actionable(glyph) => format!("actionable {glyph}"),
        SlotContent::Toggle(glyph) => format!("toggle {glyph}"),
    }
}

/// An [`InputKind`] as a cell-id slug.
///
/// `pub(crate)` because the gallery's own test sidecar asserts the id
/// shape; nothing outside the crate should depend on it.
pub(crate) fn slug_kind(kind: InputKind) -> &'static str {
    match kind {
        InputKind::TextEntry => "text-entry",
        InputKind::Search => "search",
        InputKind::DropdownTrigger => "dropdown-trigger",
        InputKind::Password => "password",
        InputKind::TextArea => "text-area",
    }
}

/// The text input cell for one `(kind, state)` pair (`UIF-ATOM-002`).
///
/// Includes the invalid-and-focused combination the atom exists to settle:
/// the error keeps the perimeter and focus relocates to a ring drawn
/// outside the control, so neither signal is lost. The two rectangles the
/// zero-shift invariant is about — the text area and the reserved trailing
/// slot — are recorded per cell and are functions of `(bounds, kind)`
/// alone, which is why they are identical across every slot content.
#[must_use]
pub fn text_input_cell(kind: InputKind, state: VisualState) -> Cell {
    text_input_cell_at(kind, state, Rung::Standard)
}

/// The text input cell for one `(kind, state, rung)` triple
/// (`UIF-ATOM-002`, §3.2's `32px` / `28px` compact-toolbar pair).
///
/// Includes the invalid-and-focused combination the atom exists to settle:
/// the error keeps the perimeter and focus relocates to a ring drawn
/// outside the control, so neither signal is lost. The two rectangles the
/// zero-shift invariant is about — the text area and the reserved trailing
/// slot — are recorded per cell and are functions of `(bounds, kind)`
/// alone, which is why they are identical across every slot content.
#[must_use]
pub fn text_input_cell_at(kind: InputKind, state: VisualState, rung: Rung) -> Cell {
    let palette = Palette::dark();
    let style = text_input::resolve_input_style(kind, state, &palette);
    let bounds = atom_sample_at(rung.height());
    let invalid = state.base == BaseState::Invalid;

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = text_input::background_layer(bounds, &style) {
        layers.push(("background", layer));
    }
    if let Some(layer) = text_input::focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![
        AuditPair {
            channel: "value",
            foreground: style.text,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "placeholder",
            foreground: style.placeholder,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "icon",
            foreground: style.icon,
            background: style.background,
            role: ContrastRole::Text,
        },
    ];

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
            "input/{}/{}{}",
            slug_kind(kind),
            state_slug(state),
            rung.slug()
        ),
        kind: CellKind::Atom,
        subject: "input",
        spec: "UIF-ATOM-002",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("value", style.text),
            StyleEntry::colour("placeholder", style.placeholder),
            StyleEntry::colour("icon", style.icon),
            StyleEntry::colour("selection.background", style.selection_background),
            StyleEntry::colour("selection.foreground", style.selection_foreground),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::text(
                "slot.empty",
                slot_label(text_input::slot_content(kind, false, invalid)),
            ),
            StyleEntry::text(
                "slot.value",
                slot_label(text_input::slot_content(kind, true, invalid)),
            ),
            StyleEntry::number("slot.width", kind.trailing_slot_width()),
            StyleEntry::text("error-border", style.is_error_border(&palette).to_string()),
            StyleEntry::text(
                "geometry.text-area",
                rect_text(text_input::text_area_bounds(bounds, kind)),
            ),
            StyleEntry::text(
                "geometry.trailing-slot",
                rect_text(text_input::trailing_slot_bounds(bounds, kind)),
            ),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::text("geometry.height-token", rung.token()),
            StyleEntry::number("geometry.radius", text_input::RADIUS),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// The three §3.2 kinds in the order the spec names them.
pub const INPUT_KINDS: [InputKind; 3] = [
    InputKind::TextEntry,
    InputKind::Search,
    InputKind::DropdownTrigger,
];

/// Every text input cell: three kinds across every reachable state, at
/// both control-height rungs.
#[must_use]
pub fn text_input_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            for rung in RUNGS {
                cells.push(text_input_cell_at(kind, state, rung));
            }
        }
    }
    cells
}

/// The editing outcome a `002a` tier cell records, and how a user produces
/// it.
pub const TEXT_INPUT_TIERS: [(&str, &str); 6] = [
    ("changed", "type a character"),
    ("committed-enter", "press Enter"),
    ("committed-blur", "leave the field with an open session"),
    ("cleared", "press the trailing clear glyph"),
    ("cancelled", "press Escape inside an open session"),
    ("focus-changed", "focus the field"),
];

/// The `UIF-ATOM-002a` tier cell for one editing outcome.
///
/// Drives a real [`TextInputController`] and records the tiers it
/// actually emitted, so the cell is a transcript rather than a caption —
/// the discipline `button_tier_cell` sets. The paint underneath is the
/// search field's own `rest` cell, deliberately unchanged: a tier is a
/// fact about the wire, not a visual state, and nothing here may suggest
/// otherwise.
#[must_use]
pub fn text_input_tier_cell(tier: &'static str) -> Cell {
    let mut controller = TextInputController::new();
    controller.observe(&TextInputModel::new("report").with_placeholder("Search reports…"));

    let emissions: Vec<TextInputEmission> = match tier {
        "changed" => {
            controller.note_edit("report 2".to_owned());
            controller
                .take_live()
                .map(|value| vec![TextInputEmission::Changed(Live::new(value))])
                .unwrap_or_default()
        }
        "committed-enter" => {
            controller.note_edit("report 2026".to_owned());
            controller.take_live();
            vec![controller.commit().expect("Enter settles what it holds")]
        }
        "committed-blur" => {
            controller.note_focus(true);
            controller.note_edit("report 2026".to_owned());
            controller.take_live();
            controller.note_focus(false)
        }
        "cleared" => vec![controller.clear().expect("a non-empty field clears")],
        "cancelled" => {
            controller.note_edit("report 2026".to_owned());
            vec![controller.revert().expect("an open session reverts")]
        }
        "focus-changed" => controller.note_focus(true),
        other => panic!("unknown tier {other}"),
    };

    let primary = emissions
        .first()
        .unwrap_or_else(|| panic!("tier {tier} emitted nothing"));
    let rest: Vec<String> = emissions
        .iter()
        .skip(1)
        .map(|emission| format!("{}({})", emission.tier(), emission.value().unwrap_or("")))
        .collect();

    let mut cell = text_input_cell(InputKind::Search, REST_STATE);
    cell.id = format!("input/tier/{tier}");
    cell.spec = "UIF-ATOM-002a";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("tier-class", primary.tier()),
        StyleEntry::text(
            "emitted-value",
            primary.value().unwrap_or("<no value>").to_string(),
        ),
        StyleEntry::text("settles-session", primary.settles().to_string()),
        StyleEntry::text(
            "also-emitted",
            if rest.is_empty() {
                "none".to_string()
            } else {
                rest.join(",")
            },
        ),
        StyleEntry::text("session-open", controller.session_open().to_string()),
        StyleEntry::text(
            "snapshot",
            controller.snapshot().unwrap_or("<none>").to_string(),
        ),
        StyleEntry::text("engine-value", controller.value().to_string()),
        StyleEntry::text("coalesced", controller.coalesced().to_string()),
        StyleEntry::text("superseded", controller.superseded().to_string()),
        StyleEntry::text("clears", controller.clears().to_string()),
    ]);
    // Pre-existing cross-family call, preserved verbatim by the split: this
    // asks the *button* tier table for a text-input tier's note line, so
    // only `cancelled` resolves and every other input tier reads
    // "produced by: unknown". Not this move's business to correct.
    cell.note = Some(format!("produced by: {}", crate::gallery_button::how(tier)));
    cell
}

/// Every `UIF-ATOM-002a` tier cell.
#[must_use]
pub fn text_input_tier_cells() -> Vec<Cell> {
    TEXT_INPUT_TIERS
        .iter()
        .map(|(tier, _)| text_input_tier_cell(tier))
        .collect()
}
