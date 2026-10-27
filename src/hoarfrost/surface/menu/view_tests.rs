//! Tests for the §4.9.2 menu row matrix and the [`MenuRow`] widget.
//!
//! Three legs, in the order the claims get harder:
//!
//! 1. **The matrix.** Every cell resolves to the token triple §4.9.2
//!    specifies, and the cells the spec does not print are named as
//!    derived rather than quietly invented.
//! 2. **The contrast audit.** Every `(row state × emitted colour)` pair
//!    the resolver can produce is measured, and the six G-11 traps are
//!    pinned at their measured values — not waived.
//! 3. **The widget.** Layout, paint layers, focus registration and
//!    pointer behaviour, driven through the real iced tree.

use super::*;

use crate::state::{InteractionInput, VisualState, resolve};
use crate::surface::menu::Menu;
use crate::test_helpers::{Blank, Harness, moved};
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, composite_over, contrast_ratio,
};
use crate::token::{shape, size, space, typography};
use crate::widget::button::{ButtonVariant, resolve_style};
use crate::widget::focus::{FocusPlacement, resolve_inset_outline_for_background};
use crate::widget::text_input::{InputKind, resolve_input_style};
use iced::mouse;
use iced::{Color, Rectangle};
use iced_test::renderer::Renderer as HeadlessRenderer;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn palette() -> Palette {
    Palette::dark()
}

/// A plain enabled action row.
fn action() -> Row {
    Row::action("edit", "Edit")
}

/// A disabled action row.
fn disabled_action() -> Row {
    Row::action("locked", "Locked").disabled()
}

/// A destructive action row.
fn destructive() -> Row {
    Row::action("delete", "Delete...").destructive()
}

/// A checked toggle row.
fn checked() -> Row {
    Row::checkable("wrap", "Wrap lines", true)
}

/// An unchecked toggle row.
fn unchecked() -> Row {
    Row::checkable("wrap", "Wrap lines", false)
}

/// A selected radio member.
fn radio_selected() -> Row {
    Row::radio("md", "Markdown", "format", true)
}

/// A submenu parent with a non-empty child.
fn submenu_parent() -> Row {
    Row::submenu(
        "more",
        "More",
        Menu::new("child", vec![Row::action("inner", "Inner")]),
    )
}

fn heading() -> Row {
    Row::heading("File")
}

fn separator() -> Row {
    Row::separator()
}

/// Every row shape the matrix has to survive.
fn every_row() -> Vec<(&'static str, Row)> {
    vec![
        ("action", action()),
        ("disabled action", disabled_action()),
        ("destructive action", destructive()),
        ("checked toggle", checked()),
        ("unchecked toggle", unchecked()),
        ("selected radio", radio_selected()),
        ("submenu parent", submenu_parent()),
        ("heading", heading()),
        ("separator", separator()),
    ]
}

/// The visual state a plain pointer hover produces on an enabled row.
fn hovered_visual() -> VisualState {
    resolve(InteractionInput {
        enabled: true,
        hovered: true,
        pressed: false,
        focused: false,
        invalid: false,
    })
}

/// The visual state a keyboard-focused, unhovered row produces.
fn focused_only_visual() -> VisualState {
    resolve(InteractionInput {
        enabled: true,
        hovered: false,
        pressed: false,
        focused: true,
        invalid: false,
    })
}

/// The visual state a pressed row produces.
fn pressed_visual() -> VisualState {
    resolve(InteractionInput {
        enabled: true,
        hovered: true,
        pressed: true,
        focused: false,
        invalid: false,
    })
}

// ---------------------------------------------------------------------------
// 1. The matrix
// ---------------------------------------------------------------------------

#[test]
fn test_rest_row_is_transparent_with_primary_label() {
    let style = resolve_row_style(&action(), RowState::REST, &palette());

    assert_eq!(style.background, Color::TRANSPARENT);
    assert_eq!(style.label, palette().text_primary);
    assert_eq!(style.trailing, palette().text_muted);
    assert!(
        style.focus_outline.is_none(),
        "a resting row carries no focus indicator at all"
    );
    assert_eq!(style.cursor, mouse::Interaction::Pointer);
}

#[test]
fn test_rest_leading_is_an_accent_only_for_a_state_glyph() {
    let p = palette();

    // A `✓` or `•` is the accent the spec names; an icon or an empty box
    // is quiet.
    assert_eq!(
        resolve_row_style(&checked(), RowState::REST, &p).leading,
        p.accent_text
    );
    assert_eq!(
        resolve_row_style(&radio_selected(), RowState::REST, &p).leading,
        p.accent_text
    );
    assert_eq!(
        resolve_row_style(&unchecked(), RowState::REST, &p).leading,
        p.text_muted
    );
    assert_eq!(
        resolve_row_style(&action(), RowState::REST, &p).leading,
        p.text_muted
    );
    assert_eq!(
        resolve_row_style(
            &Row::action("i", "Icon").with_icon("file"),
            RowState::REST,
            &p
        )
        .leading,
        p.text_muted
    );
}

#[test]
fn test_hovered_row_goes_white_in_every_slot() {
    let style = resolve_row_style(&action(), RowState::HOVERED, &palette());
    let p = palette();

    assert_eq!(style.background, p.surface_hover);
    assert_eq!(style.leading, p.text_strong);
    assert_eq!(style.label, p.text_strong);
    assert_eq!(style.trailing, p.text_strong);
}

#[test]
fn test_pressed_row_uses_the_active_surface() {
    let style = resolve_row_style(&action(), RowState::PRESSED, &palette());

    assert_eq!(style.background, palette().surface_active);
    assert_eq!(style.label, palette().text_strong);
}

#[test]
fn test_keyboard_focused_is_the_hover_fill_plus_an_inset_outline() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let hover = resolve_row_style(&action(), RowState::HOVERED, &p);

    // The outline is additive: the base paint is identical to Hovered's.
    assert_eq!(style.background, hover.background);
    assert_eq!(style.label, hover.label);

    let outline = style
        .focus_outline
        .expect("a focused row carries an outline");
    assert_eq!(outline.width, FOCUS_OUTLINE_WIDTH);
    assert_eq!(outline.width, shape::BORDER_HAIRLINE);
    assert_eq!(outline.placement, FocusPlacement::Inset);
    assert_ne!(outline.color, Color::TRANSPARENT);
    assert_ne!(
        outline.width,
        shape::BORDER_FOCUS,
        "a row takes the 1px inset outline, never the 2px outer-ring stroke"
    );
}

#[test]
fn test_active_submenu_parent_keeps_the_chevron_accented() {
    let p = palette();
    let style = resolve_row_style(&submenu_parent(), RowState::ACTIVE_SUBMENU_PARENT, &p);

    assert_eq!(style.background, p.surface_hover);
    assert_eq!(style.label, p.text_strong);
    assert_eq!(style.trailing, p.accent_text);
}

#[test]
fn test_section_heading_is_muted_with_no_slot_colour() {
    let p = palette();
    let style = resolve_row_style(&heading(), RowState::SECTION_HEADING, &p);

    assert_eq!(style.background, Color::TRANSPARENT);
    assert_eq!(style.label, p.text_muted);
    // No slots exist on a heading, so a slot colour would be a paint
    // waiting to appear by accident.
    assert_eq!(style.leading, Color::TRANSPARENT);
    assert_eq!(style.trailing, Color::TRANSPARENT);
    assert_eq!(style.cursor, mouse::Interaction::None);
}

#[test]
fn test_every_spec_row_names_its_matrix_row() {
    let expected = [
        "Rest (Enabled)",
        "Hovered",
        "Pressed",
        "Keyboard Focused",
        "Active Submenu Parent",
        "Destructive Row (Hovered)",
        "Disabled / Placeholder",
        "Section Heading",
    ];

    let actual: Vec<&str> = RowState::SPEC_ROWS.iter().map(|c| c.matrix_row()).collect();
    assert_eq!(actual, expected.to_vec());
}

#[test]
fn test_derived_rows_are_labelled_as_derived() {
    for cell in RowState::DERIVED_ROWS {
        assert!(
            cell.matrix_row().contains("derived"),
            "a cell §4.9.2 does not print must say so, got {:?}",
            cell.matrix_row()
        );
    }
}

#[test]
fn test_all_covers_spec_plus_derived_with_no_duplicates() {
    assert_eq!(RowState::ALL.len(), RowState::SPEC_ROWS.len() + 4);

    let mut seen = std::collections::HashSet::new();
    for cell in RowState::ALL {
        assert!(
            seen.insert(cell),
            "RowState::ALL carries {:?} more than once",
            cell
        );
    }

    for cell in RowState::SPEC_ROWS
        .iter()
        .chain(RowState::DERIVED_ROWS.iter())
    {
        assert!(RowState::ALL.contains(cell), "{:?} missing from ALL", cell);
    }
}

#[test]
fn test_resolve_is_pure_and_total_over_every_row_and_cell() {
    let p = palette();

    for (name, row) in every_row() {
        for cell in RowState::ALL {
            let first = resolve_row_style(&row, cell, &p);
            let second = resolve_row_style(&row, cell, &p);

            assert_eq!(
                first,
                second,
                "resolve_row_style is not pure for {} x {}",
                name,
                cell.matrix_row()
            );
        }
    }
}

#[test]
fn test_cursor_is_pointer_only_for_an_activatable_row() {
    let p = palette();

    for cell in RowState::ALL {
        let live = !matches!(cell.base, RowBase::Disabled | RowBase::SectionHeading);
        let want = if action().activatable() && live {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        };

        // The cursor column tracks `activatable`, never the base: a
        // disabled row is Default in every focus combination.
        let got = resolve_row_style(&action(), cell, &p).cursor;
        let got_disabled = resolve_row_style(&disabled_action(), cell, &p).cursor;
        let got_heading = resolve_row_style(&heading(), cell, &p).cursor;

        assert_eq!(got, want, "action x {}", cell.matrix_row());
        assert_eq!(got_disabled, mouse::Interaction::None);
        assert_eq!(got_heading, mouse::Interaction::None);
    }
}

// ---------------------------------------------------------------------------
// 2. derive_row_state
// ---------------------------------------------------------------------------

#[test]
fn test_keyboard_focus_on_a_plain_row_is_the_keyboard_focused_cell() {
    assert_eq!(
        derive_row_state(&action(), focused_only_visual(), false),
        RowState::KEYBOARD_FOCUSED
    );
}

#[test]
fn test_a_focused_row_never_derives_to_rest() {
    for row in [action(), destructive(), submenu_parent(), checked()] {
        let cell = derive_row_state(&row, focused_only_visual(), false);
        assert_ne!(
            cell.base,
            RowBase::Rest,
            "a focused {:?} must not paint a bare transparent row",
            row.kind()
        );
        assert!(cell.focused);
    }
}

#[test]
fn test_unattended_enabled_row_rests() {
    let resting = resolve(InteractionInput {
        enabled: true,
        hovered: false,
        pressed: false,
        focused: false,
        invalid: false,
    });

    assert_eq!(derive_row_state(&action(), resting, false), RowState::REST);
}

#[test]
fn test_heading_and_separator_derive_to_section_heading_under_any_pointer() {
    for row in [heading(), separator()] {
        for visual in [hovered_visual(), pressed_visual(), focused_only_visual()] {
            assert_eq!(
                derive_row_state(&row, visual, false),
                RowState::SECTION_HEADING,
                "{:?} must never take the hover fill",
                row.kind()
            );
        }
    }
}

#[test]
fn test_disabled_action_derives_to_disabled_under_any_pointer() {
    let row = disabled_action();

    for visual in [hovered_visual(), pressed_visual(), focused_only_visual()] {
        let cell = derive_row_state(&row, visual, false);
        assert_eq!(cell.base, RowBase::Disabled);
        // `resolve` zeroes the ring for a dead control, so a disabled row
        // cannot arrive focused.
        assert!(!cell.focused);
    }
}

#[test]
fn test_destructive_row_is_destructive_on_hover_and_on_focus() {
    let row = destructive();

    assert_eq!(
        derive_row_state(&row, hovered_visual(), false),
        RowState::DESTRUCTIVE_HOVERED
    );
    assert_eq!(
        derive_row_state(&row, focused_only_visual(), false),
        RowState::DESTRUCTIVE_FOCUSED
    );
}

#[test]
fn test_destructive_pressed_beats_destructive_hover() {
    assert_eq!(
        derive_row_state(&destructive(), pressed_visual(), false),
        RowState::DESTRUCTIVE_PRESSED
    );
}

#[test]
fn test_submenu_parent_is_active_only_when_its_child_is_open() {
    let row = submenu_parent();

    assert_eq!(
        derive_row_state(&row, hovered_visual(), true),
        RowState::ACTIVE_SUBMENU_PARENT
    );
    assert_eq!(
        derive_row_state(&row, hovered_visual(), false),
        RowState::HOVERED,
        "a submenu parent whose child is closed is just a hovered row"
    );
}

#[test]
fn test_submenu_open_does_not_light_a_plain_row() {
    assert_eq!(
        derive_row_state(&action(), hovered_visual(), true),
        RowState::HOVERED,
        "`submenu_open` is only meaningful to a submenu parent"
    );
}

#[test]
fn test_pressed_outranks_submenu_open() {
    assert_eq!(
        derive_row_state(&submenu_parent(), pressed_visual(), true),
        RowState::PRESSED,
        "a committed press outranks the open-child skin"
    );
}

#[test]
fn test_derive_is_total_over_every_row_and_visual_state() {
    let visuals = [
        hovered_visual(),
        pressed_visual(),
        focused_only_visual(),
        resolve(InteractionInput::default()),
    ];

    for (name, row) in every_row() {
        for visual in visuals {
            for open in [false, true] {
                let cell = derive_row_state(&row, visual, open);
                assert!(
                    RowState::ALL.contains(&cell)
                        || cell == RowState::HOVERED
                        || cell == RowState::REST,
                    "{} x {:?} x open={} produced an undefined cell {:?}",
                    name,
                    visual,
                    open,
                    cell
                );
            }
        }
    }
}
// ---------------------------------------------------------------------------
// 3. The contrast audit
// ---------------------------------------------------------------------------
//
// Every `(row state x emitted colour)` pair the resolver can produce is
// measured here, against the surface the pair actually appears on. The
// point of the audit is that the six G-11 traps cannot come back: each is
// pinned at the value INC-14 measured, so a palette retune that reopens
// one fails a named test rather than shipping an unreadable row.

/// The surface a menu card rests on (§4.8.1: `surface.overlay`).
fn card_surface() -> Color {
    palette().surface_overlay
}

/// The background a label actually sits on for a given cell.
///
/// A transparent row background means the label is on the **card**, not
/// on nothing — measuring a transparent colour would report the contrast
/// of a colour that is never painted.
fn effective_background(style: &RowStyle) -> Color {
    if style.is_transparent_background() {
        card_surface()
    } else {
        style.background
    }
}

/// A judged pairing from the audit.
struct AuditRow {
    cell: &'static str,
    channel: &'static str,
    fg: Color,
    bg: Color,
    min: f32,
    measured: f32,
}

impl AuditRow {
    fn passes(&self) -> bool {
        self.measured >= self.min - 1e-3
    }
}

/// Every pair the resolver emits, over every cell it defines.
///
/// The focus outline is audited against the resolved fill (it is stroked
/// on the inside of that fill's own perimeter, so the fill is its
/// adjacent colour), and the three content channels against the
/// effective background.
fn audit_rows() -> Vec<AuditRow> {
    let p = palette();
    let mut rows = Vec::new();

    for cell in RowState::ALL {
        // A heading has no leading or trailing slot, and a separator has
        // no slots at all: those channels are not emitted, so auditing
        // them would be measuring a colour that never reaches the screen.
        let style = resolve_row_style(&action(), cell, &p);
        let bg = effective_background(&style);
        let name = cell.matrix_row();

        let mut channels = vec![("label", style.label, MIN_TEXT_CONTRAST)];

        if cell.base != RowBase::SectionHeading {
            channels.push(("leading", style.leading, MIN_TEXT_CONTRAST));
            if style.trailing != Color::TRANSPARENT {
                channels.push(("trailing", style.trailing, MIN_TEXT_CONTRAST));
            }
        }

        for (channel, fg, min) in channels {
            rows.push(AuditRow {
                cell: name,
                channel,
                fg,
                bg,
                min,
                measured: contrast_ratio(fg, bg),
            });
        }

        if let Some(outline) = style.focus_outline {
            rows.push(AuditRow {
                cell: name,
                channel: "focus_outline",
                fg: outline.color,
                bg: style.background,
                min: MIN_CHROME_CONTRAST,
                measured: contrast_ratio(outline.color, style.background),
            });
        }
    }

    rows
}

#[test]
fn test_every_emitted_pair_clears_the_floor_its_role_carries() {
    // `--nocapture` dumps the whole measured table, so the numbers behind
    // a verdict can be read without re-deriving them by hand.
    for row in audit_rows() {
        println!(
            "{:<26} {:<11} {:>8.4}  floor {:>4.1}  {}",
            row.cell,
            row.channel,
            row.measured,
            row.min,
            if row.passes() { "PASS" } else { "FAIL" }
        );
    }

    let failing: Vec<String> = audit_rows()
        .iter()
        .filter(|r| !r.passes())
        .map(|r| {
            format!(
                "{} / {}: {:.4} < {:.1}",
                r.cell, r.channel, r.measured, r.min
            )
        })
        .collect();

    assert!(
        failing.is_empty(),
        "the menu row contrast audit found unreadable pairs: {:?}",
        failing
    );
}

#[test]
fn test_the_audit_actually_covers_every_cell() {
    let cells: Vec<&str> = audit_rows().iter().map(|r| r.cell).collect();

    for cell in RowState::ALL {
        assert!(
            cells.contains(&cell.matrix_row()),
            "{} produced no audited pair at all",
            cell.matrix_row()
        );
    }

    // Every focused cell must contribute an outline measurement, or the
    // audit would silently skip the trap it exists to catch.
    for cell in RowState::ALL.iter().filter(|c| c.focused) {
        assert!(
            audit_rows()
                .iter()
                .any(|r| r.cell == cell.matrix_row() && r.channel == "focus_outline"),
            "{} has no focus_outline pair",
            cell.matrix_row()
        );
    }
}

#[test]
fn test_trap_1_destructive_hover_uses_the_fill_not_the_text_red() {
    let p = palette();
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_HOVERED, &p);

    assert_eq!(style.background, p.status_danger_fill);
    assert_ne!(style.background, p.status_danger_text);
    assert!(
        contrast_ratio(style.label, style.background) >= MIN_TEXT_CONTRAST,
        "the destructive label must clear the text floor on the fill it paints"
    );
    // The pairing G-11 records, pinned so a token move is noticed.
    assert!(
        (contrast_ratio(p.text_strong, p.status_danger_text) - 3.0271).abs() < 1e-3,
        "text.strong on status.danger.text drifted off the 3.0271 G-11 pairing"
    );
    assert!(
        (contrast_ratio(p.text_strong, p.status_danger_fill) - 5.1270).abs() < 1e-3,
        "text.strong on status.danger_fill drifted off the 5.1270 D-5 target"
    );
}

#[test]
fn test_trap_2_the_outline_on_a_destructive_fill_is_never_the_literal_accent() {
    let p = palette();

    for cell in [RowState::DESTRUCTIVE_HOVERED, RowState::DESTRUCTIVE_FOCUSED] {
        let style = resolve_row_style(&destructive(), cell, &p);

        if cell.focused {
            let outline = style
                .focus_outline
                .expect("a focused destructive row carries an outline");

            assert_ne!(
                outline.color,
                p.accent_border,
                "accent.border on the destructive fill is {} — the outline \
                 must have been resolved, not copied from the matrix",
                contrast_ratio(p.accent_border, style.background)
            );
            assert_eq!(outline.color, p.text_strong);
            assert!(
                contrast_ratio(outline.color, style.background) >= MIN_CHROME_CONTRAST,
                "the destructive outline must clear the 3.0 chrome floor"
            );
            assert_eq!(
                style.background, p.status_danger_fill,
                "§4.9.2 keeps the destructive focused row on the hover fill, \
                 not the pressed fill"
            );
        } else {
            assert!(
                style.focus_outline.is_none(),
                "{} is not focused and must carry no outline",
                cell.matrix_row()
            );
        }
    }

    // The measurements G-11 records, pinned.
    assert!(
        (contrast_ratio(p.accent_border, p.status_danger_fill) - 1.7188).abs() < 1e-3,
        "accent.border on danger_fill drifted off 1.7188"
    );
    assert!(
        (contrast_ratio(p.text_strong, p.status_danger_fill) - 5.1270).abs() < 1e-3,
        "text.strong on danger_fill drifted off 5.1270"
    );
}

#[test]
fn test_the_destructive_pressed_row_paints_the_real_pressed_token() {
    let p = palette();
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_PRESSED, &p);

    // §4.9.2 (revised 2026-10-22) prints this cell against
    // `status.danger.fill_active`, and the resolver paints that token
    // outright. The 85% synthesis this cell used to own is gone, so the
    // fill is opaque and is not a dim of the hover fill.
    assert_eq!(style.background, p.status_danger_fill_active);
    assert_eq!(
        style.background.a, 1.0,
        "the pressed fill must be fully opaque — a synthesised alpha makes \
         this cell's contrast a function of the card underneath it"
    );
    assert_ne!(
        style.background, p.status_danger_fill,
        "pressed must remain distinguishable from hovered"
    );
    assert_ne!(
        style.background, p.status_danger_text,
        "G-11 trap 1 still applies: the fill token is not the text token"
    );

    // The pairing the revision buys: 5.7543 under `text.strong`, against
    // the hover cell's 5.1270.
    assert!(
        (contrast_ratio(p.text_strong, style.background) - 5.7543).abs() < 1e-3,
        "text.strong on status.danger_fill_active drifted off the 5.7543 pairing"
    );
    assert!(
        contrast_ratio(p.text_strong, style.background) >= MIN_TEXT_CONTRAST,
        "the destructive pressed label must clear the text floor"
    );

    // Compositing the fill over the card is now a no-op: what reaches the
    // screen *is* the token. Under the old 85% synthesis these two
    // differed, and only one of them described the row the user saw.
    let composited = composite_over(style.background, card_surface());
    for (channel, got, want) in [
        ("r", composited.r, style.background.r),
        ("g", composited.g, style.background.g),
        ("b", composited.b, style.background.b),
    ] {
        assert!(
            (got - want).abs() < 1e-6,
            "an opaque fill composites to itself on {channel}: {got} vs {want}"
        );
    }
    assert_eq!(composited.a, 1.0);

    // Every slot goes to `text.strong` here — none of them may keep an
    // accent colour that would sit on the danger fill.
    assert_eq!(style.label, p.text_strong);
    assert_eq!(style.leading, p.text_strong);
    assert_eq!(style.trailing, p.text_strong);
}

#[test]
fn test_trap_3_a_disabled_row_paints_no_fill_and_reads_on_the_card() {
    let p = palette();
    let style = resolve_row_style(&disabled_action(), RowState::DISABLED, &p);

    assert!(
        style.is_transparent_background(),
        "a disabled row must paint nothing, so it cannot inherit a hovered \
         ancestor's fill"
    );
    assert!(
        contrast_ratio(style.label, card_surface()) >= MIN_TEXT_CONTRAST,
        "text.disabled on the card measures {}",
        contrast_ratio(style.label, card_surface())
    );

    // The pairing that makes the transparency load-bearing. On the revised
    // palette `surface.hover` clears the text floor at 4.8280, so the
    // load now rests on `surface.active` alone: a disabled row that picked
    // up an active ancestor's fill would read at 4.2510, under 4.5.
    assert!(
        contrast_ratio(p.text_disabled, p.surface_hover) >= MIN_TEXT_CONTRAST,
        "surface.hover used to be load-bearing at 4.4734; if it now fails the \
         floor the ladder changed under this test and the doc needs revisiting"
    );
    assert!(
        contrast_ratio(p.text_disabled, p.surface_active) < MIN_TEXT_CONTRAST,
        "if this ever clears 4.5 the transparency stopped being load-bearing \
         and this test should say so"
    );
}

#[test]
fn test_trap_4_no_slot_carries_accent_text_over_a_destructive_fill() {
    let p = palette();

    for cell in [
        RowState::DESTRUCTIVE_HOVERED,
        RowState::DESTRUCTIVE_FOCUSED,
        RowState::DESTRUCTIVE_PRESSED,
    ] {
        let style = resolve_row_style(&checked().destructive(), cell, &p);

        for (channel, colour) in [
            ("leading", style.leading),
            ("label", style.label),
            ("trailing", style.trailing),
        ] {
            assert_ne!(
                colour,
                p.accent_text,
                "{} on a destructive fill is {} — trap 4 says use text.strong",
                channel,
                contrast_ratio(p.accent_text, p.status_danger_fill)
            );
            assert_eq!(
                colour,
                p.text_strong,
                "{} on {}",
                channel,
                cell.matrix_row()
            );
        }
    }
}

#[test]
fn test_trap_5_the_keyboard_outline_on_a_normal_row_keeps_its_headroom() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let outline = style
        .focus_outline
        .expect("the keyboard focused row carries an outline");
    let measured = contrast_ratio(outline.color, style.background);

    assert_eq!(outline.color, p.accent_border);
    assert_eq!(outline.placement, FocusPlacement::Inset);
    assert!(
        measured >= 5.4427 - 1e-3,
        "the keyboard outline on surface.hover has only {:.4} of headroom over \
         the 3.0 floor; it measured {}, expected 5.4427",
        measured - MIN_CHROME_CONTRAST,
        measured
    );
    // §4.9.2 prints this cell's outline as `5.35:1`. The solver says
    // 5.4427, and the solver is the authority: the spec's column is a
    // rounded restatement of the same pairing, not a different one.
    assert!(
        (measured - 5.4427).abs() < 1e-3,
        "the normal focused row's outline drifted off the 5.4427 the solver \
         produces (the spec prints 5.35)"
    );
}

#[test]
fn test_trap_6_the_row_lift_delta_is_measured_and_recorded() {
    let p = palette();
    let rest = resolve_row_style(&action(), RowState::REST, &p);
    let hover = resolve_row_style(&action(), RowState::HOVERED, &p);

    // The lift is a fill-vs-fill difference no WCAG rule protects, so it
    // is asserted as a delta. If the two ever converge the affordance is
    // gone and this test is the alarm.
    assert!(rest.is_transparent_background());
    assert_eq!(hover.background, p.surface_hover);
    let delta = (contrast_ratio(p.surface_hover, card_surface()) - 1.0).abs();
    assert!(
        (delta - 0.0913).abs() < 1e-3,
        "surface.hover over surface.overlay drifted off the 1.0913 measured on \
         the revised palette"
    );
    assert!(
        (contrast_ratio(p.status_danger_fill, p.surface_hover) - 3.1666).abs() < 1e-3,
        "danger_fill over surface.hover drifted off the 3.1666 measured on the \
         revised palette"
    );
}

#[test]
fn test_the_separator_is_deliberately_below_the_chrome_floor() {
    let p = palette();
    let layer = separator_layer(
        Rectangle::new((0.0, 0.0).into(), (100.0, SEPARATOR_HEIGHT).into()),
        &p,
    );

    assert_eq!(layer.fill, p.border_subtle);
    assert_eq!(layer.bounds.height, SEPARATOR_THICKNESS);
    assert_eq!(
        layer.bounds.y,
        (SEPARATOR_HEIGHT - SEPARATOR_THICKNESS) / 2.0,
        "the rule sits centred in the 9px band"
    );
    // Quiet on purpose: rule 2 of UIF-TOK-006 governs active indicators.
    assert!(
        contrast_ratio(layer.fill, card_surface()) < MIN_CHROME_CONTRAST,
        "a separator that cleared the chrome floor would shout over the \
         grouping it exists to create"
    );
}

/// A judged pairing the audit is allowed to leave failing.
///
/// Kept as a type even though it is empty: an unreadable pairing should
/// fail into a named, argued record a human adds by hand, not into a bare
/// assertion someone satisfies by deleting the threshold.
struct AcceptedFailure {
    cell: &'static str,
    channel: &'static str,
    min: f32,
    measured: f32,
    why: &'static str,
}

fn accepted_failures() -> Vec<AcceptedFailure> {
    Vec::new()
}

#[test]
fn test_the_accepted_failure_ledger_is_empty_and_fully_specified() {
    let accepted = accepted_failures();

    assert!(
        accepted.is_empty(),
        "an accepted contrast failure is a supervisor decision, not a \
         convenience — found {:?}",
        accepted
            .iter()
            .map(|f| format!("{} / {}", f.cell, f.channel))
            .collect::<Vec<_>>()
    );

    for f in &accepted {
        assert!(!f.cell.is_empty(), "an accepted failure must name its cell");
        assert!(f.min > 0.0, "{}: must carry the rule it misses", f.cell);
        assert!(
            f.measured > 0.0 && f.measured < f.min,
            "{} / {}: {:.4} must sit under the {:.1} it is accepted against",
            f.cell,
            f.channel,
            f.measured,
            f.min
        );
        assert!(
            !f.why.is_empty(),
            "{} / {}: needs an argument, not just a number",
            f.cell,
            f.channel
        );
    }
}
// ---------------------------------------------------------------------------
// The audit ledger is self-consistent
// ---------------------------------------------------------------------------

#[test]
fn test_every_audit_row_recomputes_from_the_pair_it_reports() {
    // The ledger is only evidence if the number in it is the number the
    // pair produces. A hand-entered `measured` would let the audit pass
    // while measuring nothing.
    for row in audit_rows() {
        assert!(
            (row.measured - contrast_ratio(row.fg, row.bg)).abs() < 1e-6,
            "{} / {} reports {} but its pair measures {}",
            row.cell,
            row.channel,
            row.measured,
            contrast_ratio(row.fg, row.bg)
        );
    }
}

// ---------------------------------------------------------------------------
// Geometry comes from the tokens, not from numbers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The crate-wide "no synthesised opacity" gate
// ---------------------------------------------------------------------------

/// Every [`VisualState`] the state machine can actually produce, derived
/// by running all 32 [`InteractionInput`] combinations through
/// [`resolve`] rather than written out by hand.
///
/// Deriving it keeps the gate honest: if the state machine gains or loses
/// a reachable cell, the walk changes with it instead of quietly auditing
/// a list that no longer matches.
fn reachable_visual_states() -> Vec<VisualState> {
    let mut out: Vec<VisualState> = Vec::new();
    for enabled in [false, true] {
        for hovered in [false, true] {
            for pressed in [false, true] {
                for focused in [false, true] {
                    for invalid in [false, true] {
                        let state = resolve(InteractionInput {
                            enabled,
                            hovered,
                            pressed,
                            focused,
                            invalid,
                        });
                        if !out.contains(&state) {
                            out.push(state);
                        }
                    }
                }
            }
        }
    }
    out
}

/// **No state fill anywhere in this crate is synthesised from opacity.**
///
/// Before the 2026-10-22 revision there was no token for a pressed CTA or
/// a pressed destructive row, so both were made by hand: paint the rest
/// fill at 85% alpha. The revision supplied `accent.fill_active` and
/// `status.danger.fill_active`; `INC-25` deleted the button atom's copy
/// of the hack and this increment deleted the menu row's — the last one
/// in the crate.
///
/// This walks every state fill every resolver in the crate can emit and
/// asserts each is either fully opaque or fully transparent. A fill in
/// between makes a label's contrast a function of the container it
/// happens to sit on rather than of the fill, which is exactly how the
/// old hack came to have two "correct" measurements and only one of them
/// described the screen.
///
/// The modal scrim is deliberately **not** in this set. `color.surface.scrim`
/// *is* `surface.base` at 55% alpha as a token (§5): the alpha is the
/// specified rule rather than a synthesised state fill, and
/// [`crate::surface::zorder`] pins it in its own tests.
#[test]
fn test_no_state_fill_anywhere_in_the_crate_is_synthesised_from_opacity() {
    let p = palette();
    let states = reachable_visual_states();
    let mut walked = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    let mut check = |at: String, fill: Color| {
        walked += 1;
        if fill.a != 0.0 && fill.a != 1.0 {
            offenders.push(format!("{at} emits alpha {:.2}", fill.a));
        }
    };

    for cell in RowState::ALL {
        check(
            format!("menu row {}", cell.matrix_row()),
            resolve_row_style(&action(), cell, &p).background,
        );
    }

    for variant in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ] {
        for state in &states {
            check(
                format!("button {variant:?} / {state:?}"),
                resolve_style(variant, *state, &p).background,
            );
        }
    }

    for kind in [
        InputKind::TextEntry,
        InputKind::Search,
        InputKind::DropdownTrigger,
    ] {
        for state in &states {
            check(
                format!("input {kind:?} / {state:?}"),
                resolve_input_style(kind, *state, &p).background,
            );
        }
    }

    assert!(
        offenders.is_empty(),
        "opacity-synthesised state fills are banned; found: {offenders:?}"
    );

    let expected = RowState::ALL.len() + 4 * states.len() + 3 * states.len();
    assert_eq!(
        walked, expected,
        "the gate must walk every cell of every resolver it claims to cover"
    );
    assert!(
        states.len() >= 4,
        "the walk is only meaningful if the state machine contributes \
         more than a token few states; got {}",
        states.len()
    );
}

#[test]
fn test_a_normal_row_is_the_control_height_token() {
    assert_eq!(ROW_HEIGHT, size::CONTROL_MD);
    assert_eq!(row_height(&action()), size::CONTROL_MD);
    assert_eq!(row_height(&destructive()), size::CONTROL_MD);
    assert_eq!(row_height(&submenu_parent()), size::CONTROL_MD);
}

#[test]
fn test_a_heading_is_at_least_the_minimum_hit_target() {
    assert_eq!(HEADING_HEIGHT, size::HIT_TARGET_MIN);
    assert!(
        row_height(&heading()) >= size::HIT_TARGET_MIN,
        "a heading is still a thing the pointer points at, so it cannot be \
         smaller than the hit-target token"
    );
}

#[test]
fn test_a_separator_is_a_hairline_plus_its_breathing_room() {
    assert_eq!(SEPARATOR_THICKNESS, shape::BORDER_HAIRLINE);
    assert_eq!(SEPARATOR_HEIGHT, space::SM + shape::BORDER_HAIRLINE);
    assert_eq!(row_height(&separator()), SEPARATOR_HEIGHT);
}

#[test]
fn test_the_row_insets_come_from_the_space_tokens() {
    assert_eq!(ROW_PADDING_X, space::SM + space::XXS);
    assert_eq!(ROW_GAP, space::SM);
}

#[test]
fn test_the_leading_box_is_an_icon_and_the_trailing_type_is_the_caption_size() {
    assert_eq!(LEADING_SLOT, size::ICON_MD);
    assert_eq!(TRAILING_TYPE, typography::CAPTION.size);
}

#[test]
fn test_only_a_heading_gets_vertical_padding() {
    assert_eq!(row_padding_y(&heading()), HEADING_PADDING_Y);
    assert!(row_padding_y(&heading()) > 0.0);

    for row in every_row() {
        if row.1.is_heading() {
            continue;
        }

        assert_eq!(
            row_padding_y(&row.1),
            0.0,
            "{} centres its content inside a fixed band and needs no padding",
            row.0
        );
    }
}

#[test]
fn test_the_row_shape_is_the_widget_radius_token() {
    assert_eq!(ROW_RADIUS, shape::RADIUS_WIDGET);
}

#[test]
fn test_the_outline_width_is_the_hairline_token_not_a_literal() {
    assert_eq!(FOCUS_OUTLINE_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(FOCUS_OUTLINE_WIDTH, FocusPlacement::Inset.stroke());
    assert_ne!(FOCUS_OUTLINE_WIDTH, shape::BORDER_FOCUS);
}

/// The row does not own a colour rule. Its outline is exactly what
/// [`resolve_inset_outline_for_background`] returns for the fill the row
/// just resolved — so a menu row and an application-owned tree row that
/// paint the same background get the same outline, byte for byte.
#[test]
fn test_the_rows_outline_is_the_shared_resolver_verbatim() {
    let p = palette();

    for cell in RowState::ALL.iter().filter(|c| c.focused) {
        let style = resolve_row_style(&action(), *cell, &p);
        let outline = style
            .focus_outline
            .unwrap_or_else(|| panic!("{} carries no outline", cell.matrix_row()));

        assert_eq!(
            outline,
            resolve_inset_outline_for_background(style.background, &p),
            "{}'s outline is not the shared inset rule applied to its own \
             background — the row must not carry a colour rule of its own",
            cell.matrix_row()
        );
    }
}

/// The two focused cells §4.9.2 prints, end to end: fill, outline colour,
/// stroke and placement, in the layer the renderer actually gets.
#[test]
fn test_the_two_focused_cells_paint_as_the_spec_matrix_states() {
    let p = palette();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));

    // Normal focused: surface.hover + 1px accent.border inset.
    let normal = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let normal_layers = paint_layers(&action(), bounds, &normal, &p);
    assert_eq!(normal_layers.len(), 2);
    assert_eq!(normal_layers[0].fill, p.surface_hover);
    assert_eq!(normal_layers[1].border.color, p.accent_border);
    assert_eq!(normal_layers[1].border.width, 1.0);
    assert_eq!(normal_layers[1].bounds, bounds);

    // Destructive focused: status.danger.fill + 1px text.strong inset.
    let danger = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_FOCUSED, &p);
    let destructive_layers = paint_layers(&destructive(), bounds, &danger, &p);
    assert_eq!(destructive_layers.len(), 2);
    assert_eq!(
        destructive_layers[0].fill, p.status_danger_fill,
        "the destructive focused row keeps the hover fill, not the pressed fill"
    );
    assert_eq!(destructive_layers[1].border.color, p.text_strong);
    assert_eq!(destructive_layers[1].border.width, 1.0);
    assert_eq!(destructive_layers[1].bounds, bounds);

    // The two cells differ in fill and in outline colour, and agree on
    // everything the placement carries.
    assert_eq!(
        normal_layers[1].border.width,
        destructive_layers[1].border.width
    );
    assert_ne!(
        normal_layers[1].border.color, destructive_layers[1].border.color,
        "the outline must adapt to the fill, not be one colour for every row"
    );
}

// ---------------------------------------------------------------------------
// Fixtures for the widget leg
// ---------------------------------------------------------------------------

/// A headless harness around one row.
///
/// The body is [`Blank`] so the widget's own paint — fill, ring, rule — is
/// the only thing the frame contains beyond empty content, and the row is
/// driven through the real iced build/layout/operate/update path rather than
/// a stand-in for it.
fn headless_row(row: Row) -> (Harness<String, HeadlessRenderer>, Id) {
    let focus_id = crate::focus::FocusId::new("row-under-test");
    let widget_id = crate::bridge::widget_id(&focus_id);

    let harness = Harness::headless(move || {
        MenuRow::with_body(
            row.clone(),
            Element::new(Blank::new(Size::new(80.0, 16.0))),
            focus_id.clone(),
            |event| match event {
                RowEvent::Activated => "activated".to_owned(),
                RowEvent::Absorbed => "absorbed".to_owned(),
                RowEvent::Hovered => "hovered".to_owned(),
                RowEvent::Unhovered => "unhovered".to_owned(),
            },
        )
        .into()
    });

    (harness, widget_id)
}

/// A point the row definitely covers: it fills the viewport width and its
/// own height band starts at the top of the frame.
fn inside(row: &Row) -> Point {
    Point::new(
        crate::test_helpers::VIEWPORT.width / 2.0,
        row_height(row) / 2.0,
    )
}

/// A point below the row, still inside the frame.
fn outside(row: &Row) -> Point {
    Point::new(
        crate::test_helpers::VIEWPORT.width / 2.0,
        row_height(row) + 50.0,
    )
}

// ---------------------------------------------------------------------------
// Focus registration
// ---------------------------------------------------------------------------

#[test]
fn test_an_activatable_row_registers_itself_with_the_focus_router() {
    let (mut harness, widget_id) = headless_row(action());

    assert_eq!(
        harness.registered(),
        vec![Some(widget_id)],
        "the row must be reachable by Tab, under the id the bridge derives \
         from its FocusId"
    );
}

#[test]
fn test_a_separator_registers_nothing() {
    let (mut harness, _id) = headless_row(separator());

    assert!(
        harness.registered().is_empty(),
        "a separator is not a thing Tab can stop on"
    );
}

#[test]
fn test_a_heading_registers_nothing() {
    let (mut harness, _id) = headless_row(heading());

    assert!(
        harness.registered().is_empty(),
        "a heading labels the rows under it; it is not itself a target"
    );
}

#[test]
fn test_a_disabled_row_registers_nothing() {
    let (mut harness, _id) = headless_row(disabled_action());

    assert!(
        harness.registered().is_empty(),
        "a row that cannot act must not be a Tab stop — the keyboard would \
         land on a dead control"
    );
}

#[test]
fn test_a_focused_row_that_stops_being_navigable_loses_its_registration() {
    let (mut harness, widget_id) = headless_row(action());
    harness.focus(&widget_id);
    assert!(harness.is_focused(&widget_id));

    // The same slot now holds a separator. The row cannot hold focus for a
    // widget the router can no longer traverse to.
    let separator = separator();
    let focus_id = crate::focus::FocusId::new("row-under-test");
    harness.set_view(move || {
        MenuRow::with_body(
            separator.clone(),
            Element::new(Blank::new(Size::new(80.0, 16.0))),
            focus_id.clone(),
            |_| "unused".to_owned(),
        )
        .into()
    });

    assert!(
        !harness.is_focused(&widget_id),
        "a row that is no longer navigable must drop focus rather than keep \
         a ring on an unreachable control"
    );
}

// ---------------------------------------------------------------------------
// Pointer: press, release, and the walk-off
// ---------------------------------------------------------------------------

#[test]
fn test_a_press_inside_the_row_fires_nothing_until_the_release() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    // The press event carries the cursor into the row, so the hover edge
    // lands on this same frame. What must **not** appear is the
    // activation — that belongs to the release alone.
    assert_eq!(
        harness.press(at),
        vec!["hovered".to_owned()],
        "the press is held: firing on press would take the menu's dismissal \
         away from the release"
    );
}

#[test]
fn test_a_release_still_inside_an_activatable_row_activates_it() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    harness.press(at);

    assert_eq!(harness.release(at), vec!["activated".to_owned()]);
}

#[test]
fn test_a_press_walked_off_the_row_before_the_release_never_activates() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(inside(&row));

    assert_eq!(
        harness.release(outside(&row)),
        vec!["unhovered".to_owned()],
        "walking the pointer off the row must cancel the activation: the \
         only thing this frame reports is the pointer leaving"
    );
}

#[test]
fn test_a_press_started_outside_the_row_is_never_activated_by_a_release_inside() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(outside(&row));

    assert_eq!(
        harness.release(inside(&row)),
        vec!["hovered".to_owned()],
        "the row never saw the press, so the release inside it is not its \
         business — the frame reports only that the pointer has arrived"
    );
}

#[test]
fn test_a_release_over_a_heading_reports_only_that_it_absorbed_the_press() {
    let row = heading();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(inside(&row));

    assert_eq!(
        harness.release(inside(&row)),
        vec!["absorbed".to_owned()],
        "a heading takes the press so the card behind it does not treat it \
         as a click-away, and reports nothing else"
    );
}

#[test]
fn test_a_release_over_a_separator_reports_only_that_it_absorbed_the_press() {
    let row = separator();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(inside(&row));

    assert_eq!(
        harness.release(inside(&row)),
        vec!["absorbed".to_owned()],
        "a separator must not dismiss the menu when it is clicked, and must \
         not report an activation"
    );
}

#[test]
fn test_a_release_over_a_disabled_row_reports_only_that_it_absorbed_the_press() {
    let row = disabled_action();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(inside(&row));

    assert_eq!(
        harness.release(inside(&row)),
        vec!["absorbed".to_owned()],
        "a disabled row swallows the click instead of falling through to \
         whatever is behind it"
    );
}

#[test]
fn test_a_destructive_row_activates_on_release_like_any_other() {
    let row = destructive();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    harness.press(at);

    assert_eq!(
        harness.release(at),
        vec!["activated".to_owned()],
        "destructive is a colour, not a different activation rule"
    );
}

// ---------------------------------------------------------------------------
// Pointer: the hover edge (`UIF-PRIN-008` plumbing)
// ---------------------------------------------------------------------------

#[test]
fn test_entering_a_row_reports_hovered_exactly_once() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    assert_eq!(harness.hover(at), vec!["hovered".to_owned()]);

    // The guard is non-vacuous: three more frames with the cursor in the
    // same place must add nothing. A level report here would publish on
    // every frame the pointer sat still and re-solve the host forever.
    assert_eq!(harness.hover(at), Vec::<String>::new());
    assert_eq!(harness.hover(at), Vec::<String>::new());
    assert_eq!(harness.hover(at), Vec::<String>::new());
}

#[test]
fn test_leaving_the_row_reports_unhovered_exactly_once() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());

    harness.hover(inside(&row));

    assert_eq!(harness.hover(outside(&row)), vec!["unhovered".to_owned()]);
    assert_eq!(harness.hover(outside(&row)), Vec::<String>::new());
    assert_eq!(harness.hover(outside(&row)), Vec::<String>::new());
}

#[test]
fn test_re_entering_the_row_reports_the_crossing_again() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());

    let mut seen = Vec::new();
    for _ in 0..2 {
        seen.extend(harness.hover(inside(&row)));
        seen.extend(harness.hover(outside(&row)));
    }

    assert_eq!(
        seen,
        vec![
            "hovered".to_owned(),
            "unhovered".to_owned(),
            "hovered".to_owned(),
            "unhovered".to_owned(),
        ],
        "each crossing reports once and the edge resets on the way out"
    );
}

#[test]
fn test_hover_is_reported_for_every_row_kind_not_just_the_interactive_ones() {
    // The widget reports raw observations and the host judges them, so
    // the corridor can see a pointer sitting on a heading or a disabled
    // row exactly as plainly as one on a submenu trigger.
    for (name, row) in [
        ("action", action()),
        ("submenu parent", submenu_parent()),
        ("heading", heading()),
        ("separator", separator()),
        ("disabled", disabled_action()),
        ("destructive", destructive()),
    ] {
        let (mut harness, _) = headless_row(row.clone());

        assert_eq!(
            harness.hover(inside(&row)),
            vec!["hovered".to_owned()],
            "a {name} must report the pointer arriving"
        );
        assert_eq!(
            harness.hover(outside(&row)),
            vec!["unhovered".to_owned()],
            "a {name} must report the pointer leaving"
        );
    }
}

#[test]
fn test_a_hover_report_does_not_disturb_the_activation_sequence() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    // The pointer arrives, then the click happens inside it. The hover
    // edge is the only thing the arrival says, and the click half is
    // untouched: press still says nothing, release still says activated.
    assert_eq!(harness.hover(at), vec!["hovered".to_owned()]);
    assert_eq!(harness.press(at), Vec::<String>::new());
    assert_eq!(harness.release(at), vec!["activated".to_owned()]);
}

#[test]
fn test_an_unavailable_cursor_is_reported_as_leaving_the_row() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());
    let at = inside(&row);

    harness.hover(at);

    // `Cursor::Unavailable` is `is_over == false`, so a pointer that
    // vanishes mid-hover — window blur, capture taken elsewhere — reads
    // as leaving on the next event through, rather than the row believing
    // it is still hovered forever. The event itself is irrelevant; the
    // harness passes the cursor separately precisely so a test can say
    // "the tree must read the cursor as gone".
    assert_eq!(
        harness.pointer(&[moved(at)], mouse::Cursor::Unavailable),
        vec!["unhovered".to_owned()]
    );
}

// ---------------------------------------------------------------------------
// The real paint path
// ---------------------------------------------------------------------------

#[test]
fn test_the_row_draws_through_a_real_renderer_in_every_state_it_can_be_put_in() {
    let row = action();
    let (mut harness, widget_id) = headless_row(row.clone());
    let at = inside(&row);

    harness.render();
    harness.hover(at);
    harness.render();
    harness.press(at);
    harness.render();
    harness.release(at);
    harness.focus(&widget_id);
    harness.render();
}

#[test]
fn test_a_separator_draws_through_a_real_renderer() {
    let (mut harness, _) = headless_row(separator());

    harness.hover(Point::new(200.0, 4.0));
    harness.render();
}

// ---------------------------------------------------------------------------
// Layers: what the resolver's output becomes
// ---------------------------------------------------------------------------

#[test]
fn test_a_resting_row_paints_no_fill_and_draws_no_ring() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::REST, &p);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));

    assert!(
        paint_layers(&action(), bounds, &style, &p).is_empty(),
        "a resting row paints nothing at all over the card"
    );
}

#[test]
fn test_a_hovered_row_paints_exactly_its_fill() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::HOVERED, &p);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let layers = paint_layers(&action(), bounds, &style, &p);

    assert_eq!(layers.len(), 1, "fill only — no ring without focus");
    assert_eq!(layers[0].fill, p.surface_hover);
    assert_eq!(layers[0].bounds, bounds);
}

#[test]
fn test_the_background_paints_before_the_outline() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let layers = paint_layers(&action(), bounds, &style, &p);

    assert_eq!(layers.len(), 2, "fill, then outline");
    assert_eq!(layers[0].bounds, bounds, "the fill covers the row");
    assert_ne!(
        layers[1].border.color, layers[0].fill,
        "the outline is a distinct stroke, not the fill redrawn"
    );
    assert_eq!(
        layers[1].fill.a, 0.0,
        "the outline layer strokes only — it must not lay a second fill over \
         the one the state just resolved"
    );
}

/// The geometry §3.0.2 rule 2 asks of a row: the whole `1px` outline is
/// inside the row's own perimeter, so a dense list cannot clip it.
#[test]
fn test_the_outline_is_drawn_inside_the_rows_own_box() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let bounds = Rectangle::new(Point::new(20.0, 20.0), Size::new(240.0, ROW_HEIGHT));
    let outline = focus_outline_layer(bounds, &style).expect("a focused row draws an outline");

    // iced strokes a quad inward from its bounds, so taking the row's own
    // box puts the stroke's outer edge on the row's perimeter and the
    // whole 1px inside it.
    assert_eq!(
        outline.bounds, bounds,
        "an inset outline takes the row's own quad; shrinking it would leave \
         a gap of fill at the perimeter the spec does not describe"
    );
    assert!(outline.bounds.x >= bounds.x && outline.bounds.y >= bounds.y);
    assert!(
        outline.bounds.x + outline.bounds.width <= bounds.x + bounds.width
            && outline.bounds.y + outline.bounds.height <= bounds.y + bounds.height,
        "no part of the outline may reach outside the row"
    );
    assert_eq!(outline.border.width, FOCUS_OUTLINE_WIDTH);
    assert_eq!(outline.border.width, shape::BORDER_HAIRLINE);
    assert_eq!(
        outline.border.radius.top_left, ROW_RADIUS,
        "the outline's outer edge IS the row's edge, so it rides the row's \
         own corner rather than taking a ring's grown radius"
    );
    assert_ne!(
        outline.border.width,
        shape::BORDER_FOCUS,
        "a row must never be drawn with the 2px outer-ring stroke"
    );
}

/// The reason the placement exists at all: rows are packed edge to edge,
/// and an indicator that stayed inside its own row cannot be cut by the
/// neighbour above or below it.
#[test]
fn test_a_neighbouring_row_cannot_clip_the_outline() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let row = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let next = Rectangle::new(Point::new(0.0, ROW_HEIGHT), Size::new(240.0, ROW_HEIGHT));

    let outline = focus_outline_layer(row, &style).expect("a focused row draws an outline");

    assert!(
        outline.bounds.y + outline.bounds.height <= next.y + outline.border.width,
        "the outline's lowest stroke must stay inside the row it belongs to"
    );
    assert!(
        !outline
            .bounds
            .intersects(&next.shrink(outline.border.width)),
        "the outline must not reach into the row packed below it"
    );
    assert!(
        outline.bounds.x + outline.bounds.width <= row.x + row.width,
        "and not past the right edge either"
    );
}

#[test]
fn test_an_unfocused_row_has_no_outline_layer() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::HOVERED, &p);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));

    assert!(
        focus_outline_layer(bounds, &style).is_none(),
        "hover alone is not focus — no outline may be drawn"
    );
}

#[test]
fn test_a_disabled_row_paints_no_fill_even_under_the_pointer() {
    let p = palette();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let style = resolve_row_style(&disabled_action(), RowState::DISABLED, &p);

    assert!(
        paint_layers(&disabled_action(), bounds, &style, &p).is_empty(),
        "a disabled row must not take a hovered ancestor's fill"
    );
}

#[test]
fn test_a_separator_paints_its_rule_and_nothing_else() {
    let p = palette();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, SEPARATOR_HEIGHT));
    let style = resolve_row_style(&separator(), RowState::SECTION_HEADING, &p);
    let layers = paint_layers(&separator(), bounds, &style, &p);

    assert_eq!(layers.len(), 1, "a separator paints its rule only");
    assert_eq!(layers[0].bounds.height, SEPARATOR_THICKNESS);
    assert_eq!(layers[0].fill, p.border_subtle);
}

// ---------------------------------------------------------------------------
// The `line_height` unit boundary — measured in pixels, not derived
//
// A text widget can lay out at exactly the height its caller asked for and
// still paint its glyphs outside that box. That is how the `line_height`
// unit bug hid: the row's outer height was pinned by `ROW_HEIGHT`, so no
// bounds check moved, while the paragraph inside was being asked for
// `line_height × size` pixels per line and clipping its own text away.
// These tests therefore read the framebuffer, not the layout tree.
// ---------------------------------------------------------------------------

/// The element type the pixel and layout legs build.
type PixelEl = Element<'static, (), iced::Theme, HeadlessRenderer>;

/// The y a probe slot's top edge is placed at.
///
/// Pushed well down the canvas so ink thrown *downward* out of the slot —
/// which is the direction the inflated line box threw it — is still
/// inside the frame and shows up as a displaced band rather than nothing.
const SLOT_TOP: f32 = 200.0;

/// The canvas every pixel probe renders into.
const CANVAS_HEIGHT: f32 = 800.0;

/// The intrinsic height of `child` with nothing constraining its height.
fn intrinsic_height(child: PixelEl) -> f32 {
    const PROBE_ID: &str = "line_height_probe_box";

    let root: PixelEl = iced::widget::container(child).id(PROBE_ID).into();
    let mut sim: iced_test::Simulator<'_, (), iced::Theme, HeadlessRenderer> =
        iced_test::Simulator::with_size(
            iced_test::core::Settings::default(),
            Size::new(1000.0, 4000.0),
            root,
        );
    sim.find(iced::widget::Id::from(PROBE_ID))
        .expect("probe box should be reachable")
        .bounds()
        .height
}

/// The vertical band of ink when `root` is placed in a slot of
/// `slot_height` whose top sits at [`SLOT_TOP`].
fn ink_over_slot(root: PixelEl, slot_height: f32) -> Option<(f32, f32)> {
    let viewport = Size::new(300.0, SLOT_TOP + CANVAS_HEIGHT);
    let tree: PixelEl = iced::widget::column![
        iced::widget::Space::new().height(SLOT_TOP),
        iced::widget::container(root)
            .height(slot_height)
            .align_y(iced::alignment::Vertical::Top),
    ]
    .into();

    let rgba = crate::test_helpers::screenshot(
        viewport,
        crate::test_helpers::PIXEL_SCALE,
        Color::BLACK,
        tree,
    );
    crate::test_helpers::ink_band(
        &rgba,
        crate::test_helpers::physical_width(viewport, crate::test_helpers::PIXEL_SCALE),
        crate::test_helpers::PIXEL_SCALE,
    )
}

/// Every slot's text lays out on the line grid its rung declares — the
/// pixel figure in the token, never that figure multiplied by the size.
#[test]
fn test_every_menu_slot_lays_out_on_its_declared_line_grid() {
    let cases: [(&str, PixelEl, f32, f32); 5] = [
        (
            "label",
            label_text::<(), iced::Theme, HeadlessRenderer>("Edit document".into()),
            typography::UI_COMPACT.line_height,
            typography::UI_COMPACT.size * typography::UI_COMPACT.line_height,
        ),
        (
            "trailing",
            trailing_text::<(), iced::Theme, HeadlessRenderer>("Ctrl+E".into()),
            typography::CAPTION.line_height,
            typography::CAPTION.size * typography::CAPTION.line_height,
        ),
        (
            "heading",
            heading_text::<(), iced::Theme, HeadlessRenderer>("SECTION".into()),
            typography::CAPTION.line_height,
            typography::CAPTION.size * typography::CAPTION.line_height,
        ),
        (
            "leading glyph",
            leading_glyph::<(), iced::Theme, HeadlessRenderer>("\u{2713}".into()),
            LEADING_SLOT,
            LEADING_SLOT * LEADING_SLOT,
        ),
        (
            "empty leading",
            empty_leading::<(), iced::Theme, HeadlessRenderer>(),
            LEADING_SLOT,
            LEADING_SLOT * LEADING_SLOT,
        ),
    ];

    for (name, element, declared, wrong) in cases {
        let height = intrinsic_height(element);
        assert_eq!(
            height, declared,
            "the {name} slot must lay out on its declared {declared}px line grid"
        );
        assert_ne!(
            height, wrong,
            "the {name} slot must not lay out at size × line_height"
        );
    }
}

/// Each slot paints its glyph inside the box the row actually gives it.
///
/// Per-slot rather than per-row on purpose: a row-level check can be
/// satisfied by the leading glyph while the label paints nothing, and
/// that is exactly how the original defect hid behind a row that still
/// looked populated from a distance.
///
/// Before `iced_line_height` existed every case here produced an empty
/// frame: the paragraph was clipped to the box it was handed while its
/// glyphs were being laid out `line_height × size` pixels down.
#[test]
fn test_each_menu_slot_paints_its_glyph_inside_its_own_box() {
    let action_row = action();
    let content_box = ROW_HEIGHT - 2.0 * row_padding_y(&action_row);
    let heading_row = Row::heading("SECTION");
    let heading_box = HEADING_HEIGHT - 2.0 * row_padding_y(&heading_row);

    let cases: [(&str, PixelEl, f32); 4] = [
        (
            "label",
            label_text::<(), iced::Theme, HeadlessRenderer>("Edit document".into()),
            content_box,
        ),
        (
            "trailing",
            trailing_text::<(), iced::Theme, HeadlessRenderer>("Ctrl+E".into()),
            content_box,
        ),
        (
            "heading",
            heading_text::<(), iced::Theme, HeadlessRenderer>("SECTION".into()),
            heading_box,
        ),
        (
            "leading glyph",
            leading_glyph::<(), iced::Theme, HeadlessRenderer>("\u{2713}".into()),
            LEADING_SLOT,
        ),
    ];

    for (name, element, box_height) in cases {
        let band = ink_over_slot(element, box_height).unwrap_or_else(|| {
            panic!(
                "the {name} slot painted nothing in its {box_height}px box — \
                 an empty frame means the paragraph clipped its own text away"
            )
        });
        assert!(
            band.0 >= SLOT_TOP && band.1 <= SLOT_TOP + box_height,
            "the {name} slot ink landed at {band:?}, outside its box {SLOT_TOP}..{:.1}",
            SLOT_TOP + box_height
        );
    }
}

/// A whole rest row paints inside its 32px slot — the assembled view of
/// the per-slot guarantee above, kept so a regression in the row's own
/// composition is caught and not only in a helper.
#[test]
fn test_a_rest_row_paints_its_text_inside_its_own_slot() {
    let row = Row::action("edit", "Edit document")
        .with_shortcut("Ctrl+E")
        .with_icon("\u{270e}");
    let widget: PixelEl = MenuRow::new(row, "ink_probe", |_| ())
        .palette(palette())
        .into();

    let band = ink_over_slot(widget, ROW_HEIGHT)
        .expect("a rest row must paint; an empty frame means clipped text");

    assert!(
        band.0 >= SLOT_TOP && band.1 <= SLOT_TOP + ROW_HEIGHT,
        "row ink landed at {band:?}, outside the slot {SLOT_TOP}..{:.1}",
        SLOT_TOP + ROW_HEIGHT
    );
}

/// The pixel leg itself is trustworthy: a control that is known to render
/// must produce ink, so a `None` band above means a real defect rather
/// than a harness that never painted anything.
#[test]
fn test_the_pixel_leg_renders_a_known_good_text_widget() {
    let control: PixelEl = iced::widget::text("Ag")
        .size(typography::UI_COMPACT.size)
        .line_height(typography::UI_COMPACT.iced_line_height())
        .color(Color::WHITE)
        .into();

    let band = ink_over_slot(control, ROW_HEIGHT).expect("the control must paint");
    assert!(
        band.0 >= SLOT_TOP && band.1 <= SLOT_TOP + ROW_HEIGHT,
        "control ink landed at {band:?}, outside the slot"
    );
}

#[test]
fn test_a_destructive_row_paints_the_danger_fill_and_never_the_text_red() {
    let p = palette();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_HOVERED, &p);
    let layers = paint_layers(&destructive(), bounds, &style, &p);

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].fill, p.status_danger_fill);
    assert_ne!(
        layers[0].fill, p.status_danger_text,
        "G-11 trap 1: the fill token is not the text token"
    );
}
