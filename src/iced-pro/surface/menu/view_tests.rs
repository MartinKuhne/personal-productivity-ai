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
use crate::test_helpers::{Blank, Harness};
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, composite_over, contrast_ratio,
};
use crate::token::{shape, size, space, typography};
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
    assert_eq!(style.focus_ring_width, 0.0);
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
fn test_keyboard_focused_is_the_hover_fill_plus_a_ring() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let hover = resolve_row_style(&action(), RowState::HOVERED, &p);

    // The ring is additive: the base paint is identical to Hovered's.
    assert_eq!(style.background, hover.background);
    assert_eq!(style.label, hover.label);
    assert_eq!(style.focus_ring_width, FOCUS_RING_WIDTH);
    assert_eq!(style.focus_ring_width, shape::BORDER_FOCUS);
    assert_ne!(style.focus_ring_color, Color::TRANSPARENT);
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
/// The focus ring is audited against the resolved fill (it is drawn in
/// the annulus *outside* the fill, so the fill is its adjacent colour),
/// and the three content channels against the effective background.
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

        if cell.focused {
            rows.push(AuditRow {
                cell: name,
                channel: "focus_ring",
                fg: style.focus_ring_color,
                bg: style.background,
                min: MIN_CHROME_CONTRAST,
                measured: contrast_ratio(style.focus_ring_color, style.background),
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

    // Every focused cell must contribute a ring measurement, or the
    // audit would silently skip the trap it exists to catch.
    for cell in RowState::ALL.iter().filter(|c| c.focused) {
        assert!(
            audit_rows()
                .iter()
                .any(|r| r.cell == cell.matrix_row() && r.channel == "focus_ring"),
            "{} has no focus_ring pair",
            cell.matrix_row()
        );
    }
}

#[test]
fn test_trap_1_destructive_hover_uses_the_fill_not_the_text_red() {
    let p = palette();
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_HOVERED, &p);

    assert_eq!(style.background, p.status_danger_fill);
    assert_ne!(style.background, p.status_danger);
    assert!(
        contrast_ratio(style.label, style.background) >= MIN_TEXT_CONTRAST,
        "the destructive label must clear the text floor on the fill it paints"
    );
    // The pairing G-11 records, pinned so a token move is noticed.
    assert!(
        (contrast_ratio(p.text_strong, p.status_danger) - 3.8753).abs() < 1e-3,
        "text.strong on status.danger drifted off the 3.8753 G-11 measured"
    );
    assert!(
        (contrast_ratio(p.text_strong, p.status_danger_fill) - 5.5185).abs() < 1e-3,
        "text.strong on status.danger_fill drifted off the 5.5185 D-5 target"
    );
}

#[test]
fn test_trap_2_the_ring_on_a_destructive_fill_is_never_the_literal_accent() {
    let p = palette();

    for cell in [RowState::DESTRUCTIVE_HOVERED, RowState::DESTRUCTIVE_FOCUSED] {
        let style = resolve_row_style(&destructive(), cell, &p);

        if cell.focused {
            assert_ne!(
                style.focus_ring_color,
                p.accent_primary,
                "accent.primary on the destructive fill is {} — the ring must \
                 have been resolved, not copied from the matrix",
                contrast_ratio(p.accent_primary, style.background)
            );
            assert_eq!(style.focus_ring_color, p.text_strong);
            assert!(
                contrast_ratio(style.focus_ring_color, style.background) >= MIN_CHROME_CONTRAST,
                "the destructive ring must clear the 3.0 chrome floor"
            );
        }
    }

    // The measurements G-11 records, pinned.
    assert!(
        (contrast_ratio(p.accent_primary, p.status_danger_fill) - 1.2089).abs() < 1e-3,
        "accent.primary on danger_fill drifted off 1.2089"
    );
}

#[test]
fn test_trap_2_the_destructive_pressed_fill_still_carries_every_slot() {
    let p = palette();
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_PRESSED, &p);

    // The pressed fill is translucent; the audit composites it onto the
    // card before measuring, because that is the colour that reaches the
    // screen.
    let composited = composite_over(style.background, card_surface());

    assert!(
        contrast_ratio(p.text_strong, composited) >= MIN_TEXT_CONTRAST,
        "destructive pressed label over the card measures {}",
        contrast_ratio(p.text_strong, composited)
    );

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

    // The two pairings that make the transparency load-bearing.
    assert!(
        contrast_ratio(p.text_disabled, p.surface_hover) < MIN_TEXT_CONTRAST,
        "if this ever clears 4.5 the transparency stopped being load-bearing \
         and this test should say so"
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
fn test_trap_5_the_keyboard_ring_on_a_normal_row_keeps_its_headroom() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let measured = contrast_ratio(style.focus_ring_color, style.background);

    assert_eq!(style.focus_ring_color, p.accent_primary);
    assert!(
        measured >= 3.2954 - 1e-3,
        "the keyboard ring on surface.hover has only {:.4} of headroom over \
         the 3.0 floor; it measured {}, expected 3.2954",
        measured - MIN_CHROME_CONTRAST,
        measured
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
        (delta - 0.1778).abs() < 1e-3,
        "surface.hover over surface.overlay drifted off the 1.1778 G-11 measured"
    );
    assert!(
        (contrast_ratio(p.status_danger_fill, p.surface_hover) - 2.7259).abs() < 1e-3,
        "danger_fill over surface.hover drifted off the 2.7259 G-11 measured"
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

    assert_eq!(
        harness.press(at),
        Vec::<String>::new(),
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
        Vec::<String>::new(),
        "walking the pointer off the row must cancel the activation"
    );
}

#[test]
fn test_a_press_started_outside_the_row_is_never_activated_by_a_release_inside() {
    let row = action();
    let (mut harness, _) = headless_row(row.clone());

    harness.press(outside(&row));

    assert_eq!(
        harness.release(inside(&row)),
        Vec::<String>::new(),
        "the row never saw the press, so the release inside it is not its \
         business"
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
fn test_the_background_paints_before_the_ring() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let layers = paint_layers(&action(), bounds, &style, &p);

    assert_eq!(layers.len(), 2, "fill, then ring");
    assert_eq!(layers[0].bounds, bounds, "the fill covers the row");
    assert_ne!(
        layers[1].border.color, layers[0].fill,
        "the ring is a distinct stroke, not the fill redrawn"
    );
}

#[test]
fn test_the_ring_is_drawn_outside_the_rows_own_box() {
    let p = palette();
    let style = resolve_row_style(&action(), RowState::KEYBOARD_FOCUSED, &p);
    let bounds = Rectangle::new(Point::new(20.0, 20.0), Size::new(240.0, ROW_HEIGHT));
    let ring = focus_ring_layer(bounds, &style).expect("a focused row draws a ring");

    assert!(
        ring.bounds.x < bounds.x,
        "the ring must not be clipped by the row it rings"
    );
    assert!(ring.bounds.y < bounds.y);
    assert!(ring.bounds.x + ring.bounds.width > bounds.x + bounds.width);
    assert!(ring.bounds.y + ring.bounds.height > bounds.y + bounds.height);
    assert_eq!(ring.border.width, FOCUS_RING_WIDTH);
    assert_eq!(ring.border.width, shape::BORDER_FOCUS);
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

#[test]
fn test_a_destructive_row_paints_the_danger_fill_and_never_the_text_red() {
    let p = palette();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(240.0, ROW_HEIGHT));
    let style = resolve_row_style(&destructive(), RowState::DESTRUCTIVE_HOVERED, &p);
    let layers = paint_layers(&destructive(), bounds, &style, &p);

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].fill, p.status_danger_fill);
    assert_ne!(
        layers[0].fill, p.status_danger,
        "G-11 trap 1: the fill token is not the text token"
    );
}
