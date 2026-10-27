//! The secret-entry and multi-line cells of the `UIF-VER-004` matrix
//! (§3.2, `UIF-ATOM-002c` and `UIF-ATOM-002e`).
//!
//! Split out of [`gallery`] because `gallery.rs` is already past the
//! file-size limit the project sets; [`gallery::matrix`] calls
//! [`secret_cells`] back so the matrix stays one list in one fixed order.
//!
//! # What these cells are for
//!
//! Both controls are the `002a` atom seen from a different angle, so the
//! cells are built through the atom's own resolver —
//! [`text_input::resolve_input_style`] at the new [`InputKind`]s — rather
//! than through a second paint path that could drift from it. What each
//! group adds on top of the atom's ordinary state cells is the thing the
//! spec item actually owns:
//!
//! * **password state cells** — the five §3.2 states at both rungs, with
//!   the reveal toggle's slot recorded in every one of them;
//! * **password reveal cells** — the reveal policy as a matrix: masked /
//!   revealed against focused / unfocused / disabled / read-only, each
//!   recording `masked_len`, whether a `revealed_value` exists, whether
//!   the toggle could act, and the two rectangles that must not move;
//! * **password tier cells** — a real [`PasswordController`] driven and
//!   its redacted transcript recorded, so the gallery shows `••••` where
//!   the value would be;
//! * **text-area state cells** — the same five states on the multi-line
//!   kind, proving the slot partition and the cell matrix are unchanged;
//! * **text-area growth cells** — the `min_rows`/`max_rows` solver across
//!   a content ladder, with the height, the ceiling and the scroll
//!   surplus recorded per rung;
//! * **text-area tier cells** — a real [`TextAreaController`] driven,
//!   showing that `Ctrl+Enter` sends without settling and that `Escape`
//!   restores the first-edit snapshot.
//!
//! # The reveal cells carry their own proof of width invariance
//!
//! The `002b` invariant is that the text area and the trailing slot are
//! functions of `(bounds, kind)` alone. A reveal changes what the editor
//! *draws* — iced substitutes `•` per grapheme, so the glyph run is a
//! different width — which is exactly why the invariant has to be stated
//! against the partition rather than against the run. Every reveal cell
//! records both rectangles so the sidecar can assert they are identical
//! to the masked cell's.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_secret_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, RUNGS, Rung, StyleEntry, atom_sample_at,
    paint_frame, snapshot_states, state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::widget::button_widget::Layer;
use crate::widget::password_host::{PasswordController, mirror};
use crate::widget::password_model::{PasswordEmission, PasswordInputModel, SecretInput};
use crate::widget::text_area_host::TextAreaController;
use crate::widget::text_area_model::{Growth, LineCol, TextAreaEmission, TextAreaModel};
use crate::widget::text_input::{self, InputKind};
use crate::widget::text_input_controller::TextInputController;
use iced::Rectangle;

/// The secret being typed in every sample cell.
///
/// Deliberately something that would be obvious if it ever reached a
/// snapshot: the reveal cells assert the opposite.
const SAMPLE_SECRET: &str = "hunter2-supersecret";

/// A [`VisualState`] with `base` and `focused` written out for the
/// const tables below.
const fn vs(base: BaseState, focused: bool) -> VisualState {
    VisualState { base, focused }
}

/// The atom's own cell for `(kind, state, rung)`, rebuilt here so this
/// module can extend it without reaching into `gallery`'s private
/// helpers.
///
/// Paint comes from the same resolver the single-line field uses, which
/// is the point: a password field that painted differently from a text
/// entry would be a second control wearing the same id.
fn atom_cell(kind: InputKind, state: VisualState, rung: Rung) -> Cell {
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
                slot_text(text_input::slot_content(kind, false, invalid)),
            ),
            StyleEntry::text(
                "slot.value",
                slot_text(text_input::slot_content(kind, true, invalid)),
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

/// An [`InputKind`] as a cell-id slug.
///
/// Mirrors the gallery's own private helper; duplicated rather than
/// widened because making a private helper `pub(crate)` for one caller is
/// a worse trade than five lines here.
fn slug_kind(kind: InputKind) -> &'static str {
    match kind {
        InputKind::TextEntry => "text-entry",
        InputKind::Search => "search",
        InputKind::DropdownTrigger => "dropdown-trigger",
        InputKind::Password => "password",
        InputKind::TextArea => "text-area",
    }
}

/// A [`SlotContent`] as the manifest names it.
fn slot_text(slot: text_input::SlotContent) -> String {
    match slot {
        text_input::SlotContent::Empty => "empty".to_string(),
        text_input::SlotContent::Passive(glyph) => format!("passive {glyph}"),
        text_input::SlotContent::Actionable(glyph) => format!("actionable {glyph}"),
        text_input::SlotContent::Toggle(glyph) => format!("toggle {glyph}"),
    }
}

/// A rectangle rendered compactly for a resolved-value entry.
fn rect_text(rectangle: Rectangle) -> String {
    format!(
        "{} {} {} {}",
        rectangle.x, rectangle.y, rectangle.width, rectangle.height
    )
}

// ---------------------------------------------------------------------------
// Password: state cells
// ---------------------------------------------------------------------------

/// The password field across every reachable state at both rungs
/// (`UIF-ATOM-002c`).
#[must_use]
pub fn password_state_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for rung in RUNGS {
        for state in snapshot_states() {
            let mut cell = atom_cell(InputKind::Password, state, rung);
            cell.spec = "UIF-ATOM-002c";
            cell.subject = "password";
            cell.id = format!("password/{}{}", state_slug(state), rung.slug());
            cell.resolved.push(StyleEntry::text(
                "reveal.slot",
                slot_text(text_input::slot_content(InputKind::Password, false, false)),
            ));
            cells.push(cell);
        }
    }
    cells
}

// ---------------------------------------------------------------------------
// Password: reveal policy cells
// ---------------------------------------------------------------------------

/// One row of the reveal matrix: a focus/enable posture against a
/// requested reveal.
struct RevealCase {
    slug: &'static str,
    focused: bool,
    disabled: bool,
    read_only: bool,
    want_revealed: bool,
}

const REVEAL_CASES: &[RevealCase] = &[
    RevealCase {
        slug: "masked-unfocused",
        focused: false,
        disabled: false,
        read_only: false,
        want_revealed: false,
    },
    RevealCase {
        slug: "reveal-refused-unfocused",
        focused: false,
        disabled: false,
        read_only: false,
        want_revealed: true,
    },
    RevealCase {
        slug: "masked-focused",
        focused: true,
        disabled: false,
        read_only: false,
        want_revealed: false,
    },
    RevealCase {
        slug: "revealed-focused",
        focused: true,
        disabled: false,
        read_only: false,
        want_revealed: true,
    },
    RevealCase {
        slug: "reveal-refused-disabled",
        focused: true,
        disabled: true,
        read_only: false,
        want_revealed: true,
    },
    RevealCase {
        slug: "reveal-refused-readonly",
        focused: true,
        disabled: false,
        read_only: true,
        want_revealed: true,
    },
];

/// The reveal policy as a cell per posture (`UIF-ATOM-002c`).
///
/// Each cell records the model's own answers — `masked_len`, whether a
/// `revealed_value` exists, whether the toggle could act — read out of a
/// real [`PasswordInputModel`] after the posture has been applied, plus
/// the two rectangles that must be identical whether the secret is shown
/// or hidden.
#[must_use]
pub fn password_reveal_cells() -> Vec<Cell> {
    let mut cells = Vec::new();

    for case in REVEAL_CASES {
        let mut model = PasswordInputModel::new()
            .with_placeholder("Password")
            .with_value(SecretInput::new(SAMPLE_SECRET))
            .with_disabled(case.disabled)
            .with_read_only(case.read_only);
        let _ = model.note_focus(case.focused);

        let mut controller = PasswordController::new();
        if case.want_revealed {
            controller.toggle_reveal(&mut model);
        }
        controller.observe(&model);

        let state = if case.disabled {
            vs(BaseState::Disabled, false)
        } else if case.focused {
            vs(BaseState::Rest, true)
        } else {
            vs(BaseState::Rest, false)
        };
        let mut cell = atom_cell(InputKind::Password, state, Rung::Standard);
        cell.subject = "password";
        cell.id = format!("password/reveal/{}", case.slug);
        cell.spec = "UIF-ATOM-002c";
        cell.resolved.extend([
            StyleEntry::text("reveal.state", model.is_revealed().to_string()),
            StyleEntry::text("reveal.can", model.can_reveal().to_string()),
            StyleEntry::text("reveal.masked-len", model.masked_len().to_string()),
            StyleEntry::text(
                "reveal.value-present",
                model.revealed_value().is_some().to_string(),
            ),
            StyleEntry::text("reveal.toggles", controller.reveals().to_string()),
            StyleEntry::text("reveal.refusals", controller.refusals().to_string()),
            StyleEntry::text(
                "reveal.mirror-clearable",
                mirror(&model).clearable.to_string(),
            ),
            StyleEntry::text("reveal.debug-redacted", {
                (!format!("{model:?}").contains(SAMPLE_SECRET)).to_string()
            }),
        ]);
        cells.push(cell);
    }

    cells
}

// ---------------------------------------------------------------------------
// Password: tier cells
// ---------------------------------------------------------------------------

/// The tiers a password field produces, and how each was produced.
const PASSWORD_TIERS: [(&str, &str); 6] = [
    ("changed", "type one character"),
    ("committed", "leave the field with an open session"),
    ("cancelled", "press Escape inside an open session"),
    ("revealed", "press the reveal toggle while focused"),
    ("reveal-refused", "press the reveal toggle while unfocused"),
    ("focus-changed", "focus the field"),
];

/// Drive a real [`PasswordController`] and record what it emitted
/// (`UIF-ATOM-002c`, §3.0.4.2).
///
/// The recorded value is the **masked** rendering, because that is what
/// a host logging its tiers would see.
#[must_use]
pub fn password_tier_cells() -> Vec<Cell> {
    PASSWORD_TIERS
        .iter()
        .map(|(tier, how)| password_tier_cell(tier, how))
        .collect()
}

/// The editing engine a **focused** password field runs, seeded with the
/// value the mirror renders.
///
/// The focus edge is told to the engine, not only to the model. A
/// falling edge is what settles a session, and an engine that was never
/// told it held focus sees no edge on blur and commits nothing — so a
/// cell that recorded focus only on the model would show the
/// `committed` tier as an empty transcript and prove the opposite of
/// what it claims.
fn focused_engine(model: &PasswordInputModel) -> TextInputController {
    let mut engine = TextInputController::new();
    let _ = engine.note_focus(true);
    engine.observe(&mirror(model));
    engine
}

fn password_tier_cell(tier: &'static str, how: &str) -> Cell {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new()
        .with_placeholder("Password")
        .with_value(SecretInput::new("report"));

    let emissions: Vec<PasswordEmission> = match tier {
        "changed" => {
            let _ = model.note_focus(true);
            let mut engine = focused_engine(&model);
            engine.note_edit("report-x".to_owned());
            engine
                .take_live()
                .map(|value| {
                    vec![PasswordEmission::Changed(crate::binding::Live::new(
                        SecretInput::new(value),
                    ))]
                })
                .unwrap_or_default()
        }
        "committed" => {
            let _ = model.note_focus(true);
            let mut engine = focused_engine(&model);
            engine.note_edit("report-2026".to_owned());
            engine
                .note_focus(false)
                .into_iter()
                .filter_map(PasswordController::translate)
                .collect()
        }
        "cancelled" => {
            let _ = model.note_focus(true);
            let mut engine = focused_engine(&model);
            engine.note_edit("report-2026".to_owned());
            engine
                .revert()
                .into_iter()
                .filter_map(PasswordController::translate)
                .collect()
        }
        "revealed" => {
            let _ = model.note_focus(true);
            controller.toggle_reveal(&mut model).emissions
        }
        "reveal-refused" => controller.toggle_reveal(&mut model).emissions,
        "focus-changed" => vec![PasswordEmission::FocusChanged(true)],
        other => panic!("unknown tier {other}"),
    };

    let rest: Vec<String> = emissions
        .iter()
        .skip(1)
        .map(|emission| emission.tier().to_string())
        .collect();

    // A refused reveal is a tier cell whose transcript is empty *on
    // purpose*: the spec's callback reports a change, and a press that
    // changed nothing must not report one. So the cell records the
    // absence instead of asserting a tier that never fired — the empty
    // transcript is the finding, not a missing one.
    let (tier_class, settles, carries_secret, rendering) = match emissions.first() {
        Some(primary) => (
            primary.tier().to_string(),
            primary.settles().to_string(),
            primary.carries_secret().to_string(),
            String::from_iter(format!("{primary:?}").chars().take(96)),
        ),
        None => (
            "none".to_string(),
            "not-emitted".to_string(),
            "false".to_string(),
            "<no tier: press refused>".to_string(),
        ),
    };

    let mut cell = atom_cell(
        InputKind::Password,
        vs(BaseState::Rest, false),
        Rung::Standard,
    );
    cell.subject = "password-tier";
    cell.id = format!("password/tier/{tier}");
    cell.spec = "UIF-ATOM-002c";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("tier-class", tier_class),
        StyleEntry::text("emitted-count", emissions.len().to_string()),
        StyleEntry::text("settles-session", settles),
        StyleEntry::text("carries-secret", carries_secret),
        StyleEntry::text("emitted-rendering", rendering),
        StyleEntry::text(
            "also-emitted",
            if rest.is_empty() {
                "none".to_string()
            } else {
                rest.join(",")
            },
        ),
        StyleEntry::text("reveal.toggles", controller.reveals().to_string()),
        StyleEntry::text("reveal.refusals", controller.refusals().to_string()),
    ]);
    cell.note = Some(format!("produced by: {how}"));
    cell
}

// ---------------------------------------------------------------------------
// Text area: state cells
// ---------------------------------------------------------------------------

/// The multi-line kind across every reachable state at both rungs
/// (`UIF-ATOM-002e`).
///
/// The paint is the atom's, unchanged: the multi-line field differs in
/// row height and wrapping, not in its cell matrix.
#[must_use]
pub fn text_area_state_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for rung in RUNGS {
        for state in snapshot_states() {
            let mut cell = atom_cell(InputKind::TextArea, state, rung);
            cell.spec = "UIF-ATOM-002e";
            cell.subject = "text-area";
            cell.id = format!("text-area/{}{}", state_slug(state), rung.slug());
            cell.resolved.push(StyleEntry::text(
                "well.min-rows",
                Growth::default().min_rows.to_string(),
            ));
            cells.push(cell);
        }
    }
    cells
}

// ---------------------------------------------------------------------------
// Text area: growth cells
// ---------------------------------------------------------------------------

/// A growth band and the content ladder sampled against it.
const GROWTH_BANDS: [(u32, Option<u32>); 3] = [(1, Some(3)), (3, None), (2, Some(6))];
const GROWTH_LADDER: [u32; 6] = [1, 2, 3, 4, 6, 9];

/// The `min_rows` / `max_rows` solver across a ladder of content sizes
/// (`UIF-ATOM-002e`).
///
/// Each cell carries the height the well would take, whether it had room
/// left to grow, and the scroll surplus the ceiling produced — the three
/// numbers a host needs and the three places a hand-written solver would
/// get the arithmetic wrong.
#[must_use]
pub fn text_area_growth_cells() -> Vec<Cell> {
    let mut cells = Vec::new();

    for (min_rows, max_rows) in GROWTH_BANDS {
        for lines in GROWTH_LADDER {
            let band = Growth::new(min_rows, max_rows);
            let text = (0..lines)
                .map(|n| format!("line {n}"))
                .collect::<Vec<_>>()
                .join("\n");
            let model = TextAreaModel::new()
                .with_growth(band)
                .with_text(text)
                .with_disabled(false);

            let mut cell = atom_cell(
                InputKind::TextArea,
                vs(BaseState::Rest, false),
                Rung::Standard,
            );
            cell.subject = "text-area-growth";
            cell.id = format!(
                "text-area/growth/{}-{}-lines{}",
                band_label(min_rows, max_rows),
                lines,
                ""
            );
            cell.spec = "UIF-ATOM-002e";
            cell.resolved.extend([
                StyleEntry::text("band.min-rows", min_rows.to_string()),
                StyleEntry::text(
                    "band.max-rows",
                    max_rows.map_or("none".to_string(), |v| v.to_string()),
                ),
                StyleEntry::number("band.min-height", band.min_height()),
                StyleEntry::number(
                    "band.max-height",
                    if band.max_height().is_finite() {
                        band.max_height()
                    } else {
                        -1.0
                    },
                ),
                StyleEntry::text("content.lines", model.line_count().to_string()),
                StyleEntry::number("content.height", model.content_height()),
                StyleEntry::number("well.height", model.height()),
                StyleEntry::number("well.visible", model.visible_text_height()),
                StyleEntry::number("well.max-scroll", model.max_scroll()),
                StyleEntry::text("well.scrollable", model.is_scrollable().to_string()),
                StyleEntry::text("well.can-grow", band.can_grow(lines).to_string()),
            ]);
            cells.push(cell);
        }
    }

    cells
}

fn band_label(min_rows: u32, max_rows: Option<u32>) -> String {
    format!(
        "{min_rows}-{}",
        max_rows.map_or("inf".to_string(), |v| v.to_string())
    )
}

// ---------------------------------------------------------------------------
// Text area: tier cells
// ---------------------------------------------------------------------------

/// The tiers a multi-line well produces, and how each was produced.
const TEXT_AREA_TIERS: [(&str, &str); 7] = [
    ("changed", "type one character"),
    ("submitted", "press Ctrl+Enter / Cmd+Enter"),
    ("committed-blur", "leave the well with an open session"),
    ("cancelled", "press Escape inside an open session"),
    ("undone", "press Ctrl+Z"),
    ("redone", "press Ctrl+Shift+Z"),
    ("cursor-changed", "move the caret to a new line"),
];

/// Drive a real [`TextAreaController`] and record what it emitted
/// (`UIF-ATOM-002e`, §3.0.4.2).
#[must_use]
pub fn text_area_tier_cells() -> Vec<Cell> {
    TEXT_AREA_TIERS
        .iter()
        .map(|(tier, how)| text_area_tier_cell(tier, how))
        .collect()
}

fn text_area_tier_cell(tier: &'static str, how: &str) -> Cell {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("draft one");
    // Seed the engine with the buffer the well opens holding. Without this
    // the first edit's snapshot is the engine's own empty string rather
    // than the text on screen, and `Escape` would report restoring
    // nothing when the user can see "draft one".
    controller.sync(&model);

    let emissions: Vec<TextAreaEmission> = match tier {
        "changed" => {
            controller.note_focus(&mut model, true);
            controller
                .note_edit(&mut model, "draft one x".to_owned())
                .emissions
        }
        "submitted" => {
            controller.note_focus(&mut model, true);
            controller.note_edit(&mut model, "draft one sent".to_owned());
            controller.submit(&model).emissions
        }
        "committed-blur" => {
            controller.note_focus(&mut model, true);
            controller.note_edit(&mut model, "draft one settled".to_owned());
            controller.note_focus(&mut model, false).emissions
        }
        "cancelled" => {
            controller.note_focus(&mut model, true);
            controller.note_edit(&mut model, "draft one changed".to_owned());
            controller.cancel(&mut model).emissions
        }
        "undone" => {
            controller.note_edit(&mut model, "draft two".to_owned());
            controller.undo(&mut model).emissions
        }
        "redone" => {
            controller.note_edit(&mut model, "draft two".to_owned());
            controller.undo(&mut model);
            controller.redo(&mut model).emissions
        }
        "cursor-changed" => {
            controller
                .note_cursor(&mut model, LineCol::new(0, 4))
                .emissions
        }
        other => panic!("unknown tier {other}"),
    };

    let primary = emissions
        .first()
        .unwrap_or_else(|| panic!("tier {tier} emitted nothing"));
    let rest: Vec<String> = emissions
        .iter()
        .skip(1)
        .map(|emission| emission.tier().to_string())
        .collect();

    let mut cell = atom_cell(
        InputKind::TextArea,
        vs(BaseState::Rest, false),
        Rung::Standard,
    );
    cell.subject = "text-area-tier";
    cell.id = format!("text-area/tier/{tier}");
    cell.spec = "UIF-ATOM-002e";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("tier-class", primary.tier()),
        StyleEntry::text("settles-session", primary.settles().to_string()),
        StyleEntry::text(
            "emitted-value",
            primary.value().unwrap_or("<no value>").to_string(),
        ),
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
        StyleEntry::text("undo-depth", controller.history().undo_depth().to_string()),
        StyleEntry::text("redo-depth", controller.history().redo_depth().to_string()),
        StyleEntry::text("cursor", format!("{:?}", model.cursor())),
    ]);
    cell.note = Some(format!("produced by: {how}"));
    cell
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Every `UIF-ATOM-002c` and `UIF-ATOM-002e` cell, in matrix order.
#[must_use]
pub fn secret_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    cells.extend(password_state_cells());
    cells.extend(password_reveal_cells());
    cells.extend(password_tier_cells());
    cells.extend(text_area_state_cells());
    cells.extend(text_area_growth_cells());
    cells.extend(text_area_tier_cells());
    cells
}

#[cfg(test)]
#[path = "gallery_secret_tests.rs"]
mod gallery_secret_tests;
