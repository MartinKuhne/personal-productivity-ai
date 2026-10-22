//! Unit tests for the deterministic `(Variant × State)` gallery
//! (`UIF-VER-004`).
//!
//! Five things are asserted here, and each one is a different failure mode
//! a gallery can have:
//!
//! 1. **Completeness** — the id list is written out in full rather than
//!    counted, so a cell that silently stops being produced shows up as a
//!    missing line rather than as a number that happens to still be right.
//! 2. **Determinism** — two runs are byte-identical. This is the gate's
//!    actual subject: a snapshot that cannot be reproduced cannot be
//!    diffed, and a snapshot that cannot be diffed asserts nothing.
//! 3. **Resolved-style content** — the snapshot names the token behind
//!    every channel, so a regression reads as a named colour moving.
//! 4. **Scope honesty** — the `NotOwned` cells are an explicit list with
//!    a justification each, and they draw nothing.
//! 5. **The gates** — no unmapped colour, and every judged pair clears
//!    its threshold with an empty accepted-failure ledger. The three
//!    non-judged classes are themselves cross-checked, so an exemption
//!    cannot be used as a place to hide a real failure.

use super::*;

use crate::state::BaseState;
use crate::state::VisualState;
use crate::surface::menu::view as menu_view;
use crate::surface::zorder::ZBand;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::button;
use crate::widget::button::ButtonVariant;
use crate::widget::text_input::InputKind;

// ---------------------------------------------------------------------------
// 1 — completeness
// ---------------------------------------------------------------------------

/// The nine reachable visual states, in snapshot order.
fn expected_states() -> [&'static str; 9] {
    [
        "rest",
        "rest+focused",
        "hovered",
        "hovered+focused",
        "pressed",
        "pressed+focused",
        "invalid",
        "invalid+focused",
        "disabled",
    ]
}

/// Every cell id the gallery must produce, in the order it produces them.
///
/// Written out rather than derived. A count-only assertion passes when a
/// cell disappears and a spurious one appears; this list cannot.
fn expected_ids() -> Vec<String> {
    let mut ids = Vec::new();

    for variant in ["primary", "secondary", "ghost", "danger"] {
        for state in expected_states() {
            ids.push(format!("button/{variant}/{state}"));
        }
    }
    for kind in ["text-entry", "search", "dropdown-trigger"] {
        for state in expected_states() {
            ids.push(format!("input/{kind}/{state}"));
        }
    }
    ids.extend(
        [
            "rest",
            "hovered",
            "pressed",
            "hovered+focused",
            "submenu-parent",
            "destructive",
            "destructive-pressed",
            "destructive+focused",
            "submenu-parent+focused",
            "rest+focused",
            "disabled",
            "heading",
        ]
        .iter()
        .map(|state| format!("menu/row/{state}")),
    );
    ids.extend(
        ["clean", "dirty", "error"]
            .iter()
            .map(|name| format!("modal/{name}")),
    );
    ids.extend(
        [
            "rename-valid",
            "rename-invalid",
            "create-valid",
            "move-folder-list",
            "delete-danger",
        ]
        .iter()
        .map(|name| format!("input-dialog/{name}")),
    );
    ids.extend(
        ["idle", "dragging", "resizing", "auto-scroll-paused"]
            .iter()
            .map(|name| format!("movable/{name}")),
    );
    ids.push("zband/overlay-menu-modal".to_string());
    ids.extend(
        ["navbar", "panes", "tabs"]
            .iter()
            .map(|subject| format!("{subject}/not-owned")),
    );

    ids
}

#[test]
fn the_matrix_produces_every_expected_cell_in_the_expected_order() {
    let cells = matrix();
    let actual: Vec<String> = cells.iter().map(|cell| cell.id.clone()).collect();

    assert_eq!(actual, expected_ids(), "the cell inventory moved");
}

#[test]
fn the_matrix_has_no_duplicate_cell_ids() {
    let cells = matrix();
    let mut seen = Vec::new();
    for cell in &cells {
        assert!(
            !seen.contains(&cell.id),
            "cell id {} is produced twice",
            cell.id
        );
        seen.push(cell.id.clone());
    }
}

#[test]
fn per_subject_cell_counts_match_the_plan() {
    let cells = matrix();
    let counts = subject_counts_for(&cells);

    let mut expected: Vec<(&str, usize)> = vec![
        ("button", 36),
        ("input", 27),
        ("menu", 12),
        ("modal", 3),
        ("input-dialog", 5),
        ("movable", 4),
        ("zband", 1),
        ("navbar", 1),
        ("panes", 1),
        ("tabs", 1),
    ];
    expected.sort();
    let mut actual = counts.clone();
    actual.sort();

    assert_eq!(actual, expected, "per-subject coverage moved");
}

/// Per-subject counts, sorted so the assertion is order independent.
fn subject_counts_for(cells: &[Cell]) -> Vec<(&'static str, usize)> {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for cell in cells {
        if let Some(entry) = counts.iter_mut().find(|(s, _)| *s == cell.subject) {
            entry.1 += 1;
        } else {
            counts.push((cell.subject, 1));
        }
    }
    counts
}

#[test]
fn every_button_variant_is_covered_across_every_reachable_state() {
    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            let id = format!("button/{}/{}", slug(variant), state_slug(state));
            assert!(
                matrix().iter().any(|cell| cell.id == id),
                "missing button cell {id}"
            );
        }
    }
}

#[test]
fn every_input_kind_is_covered_across_every_reachable_state() {
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            let id = format!("input/{}/{}", slug_kind(kind), state_slug(state));
            assert!(
                matrix().iter().any(|cell| cell.id == id),
                "missing input cell {id}"
            );
        }
    }
}

#[test]
fn the_reachable_state_set_is_nine_and_snapshot_order_agrees_with_it() {
    let reachable = reachable_visual_states();
    let ordered = snapshot_states();

    assert_eq!(reachable.len(), 9, "the reachable state set moved");
    assert_eq!(ordered.len(), 9);

    for state in &reachable {
        assert!(
            ordered.contains(state),
            "reachable state {:?}+focused={} is missing from the snapshot order",
            state.base,
            state.focused
        );
    }
    for state in &ordered {
        assert!(reachable.contains(state), "snapshot state is not reachable");
    }
}

#[test]
fn disabled_and_focused_is_not_snapshotted_because_resolve_zeroes_it() {
    let dead = VisualState {
        base: BaseState::Disabled,
        focused: true,
    };
    assert!(
        !snapshot_states().contains(&dead),
        "a dead control cannot show a ring, so the gallery must not claim one"
    );
}

// ---------------------------------------------------------------------------
// 2 — determinism
// ---------------------------------------------------------------------------

#[test]
fn two_matrix_calls_are_byte_identical() {
    let first: String = matrix().iter().map(Cell::snapshot).collect();
    let second: String = matrix().iter().map(Cell::snapshot).collect();

    assert_eq!(first, second, "the gallery is not deterministic");
}

#[test]
fn two_manifest_calls_are_byte_identical() {
    assert_eq!(manifest(), manifest(), "the manifest is not deterministic");
}

#[test]
fn a_cell_built_twice_equals_itself() {
    for state in snapshot_states() {
        assert_eq!(
            button_cell(ButtonVariant::Primary, state),
            button_cell(ButtonVariant::Primary, state)
        );
    }
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            assert_eq!(text_input_cell(kind, state), text_input_cell(kind, state));
        }
    }
    for state in menu_view::RowState::ALL {
        assert_eq!(menu_row_cell(state), menu_row_cell(state));
    }
    assert_eq!(zband_cell(), zband_cell());
}

#[test]
fn the_gallery_reads_no_wall_clock_or_random_source() {
    // The determinism contract is structural: the same call, twice, with
    // nothing between the calls, produces the same bytes. A clock or a
    // random source anywhere in the build path would show up here, and
    // `two_matrix_calls_are_byte_identical` is the cheap form of it.
    let a = manifest();
    let b = manifest();
    let c = manifest();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

// ---------------------------------------------------------------------------
// 3 — resolved-style content
// ---------------------------------------------------------------------------

/// The snapshot line for one resolved channel of one cell.
fn resolved_line(cell: &Cell, channel: &str) -> String {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .unwrap_or_else(|| panic!("cell {} has no channel {channel}", cell.id))
        .value
        .display()
}

#[test]
fn every_resolved_colour_channel_names_a_real_token() {
    for cell in matrix() {
        for entry in &cell.resolved {
            if let StyleValue::Colour { token, .. } = &entry.value {
                assert!(
                    !token.starts_with("UNMAPPED"),
                    "{} resolved.{} is {}",
                    cell.id,
                    entry.channel,
                    token
                );
            }
        }
    }
}

#[test]
fn the_snapshot_names_the_token_behind_a_primary_resting_fill() {
    let cell = button_cell(
        ButtonVariant::Primary,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    let palette = Palette::dark();

    assert_eq!(
        resolved_line(&cell, "background"),
        format!("accent.primary ({})", rgb8(palette.accent_primary)),
        "the primary resting fill must be named by its token, not by a literal"
    );
    assert!(cell.snapshot().contains("button/primary/rest"));
}

#[test]
fn a_ghost_at_rest_records_a_transparent_fill_rather_than_an_invented_one() {
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let ghost = button_cell(ButtonVariant::Ghost, rest);
    let primary = button_cell(ButtonVariant::Primary, rest);

    let ghost_fill = ghost
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "background")
        .expect("the Ghost cell records its background layer");
    assert_eq!(
        ghost_fill.fill, "transparent",
        "a Ghost at rest must record the spec's own word for its fill, not a colour"
    );
    assert_eq!(ghost_fill.border, "transparent");
    assert_eq!(resolved_line(&ghost, "transparent-background"), "true");

    let primary_fill = primary
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "background")
        .expect("the Primary cell must still paint its fill");
    assert_ne!(
        primary_fill.fill, "transparent",
        "the control cell must be distinguishable from the Ghost in the manifest"
    );
}

#[test]
fn the_focus_ring_is_a_quad_outside_the_background_layer() {
    let focused = VisualState {
        base: BaseState::Rest,
        focused: true,
    };
    let cell = button_cell(ButtonVariant::Secondary, focused);
    let layers: Vec<&str> = cell
        .frame
        .quads
        .iter()
        .map(|quad| quad.layer.as_str())
        .collect();

    assert_eq!(layers, vec!["background", "focus-ring"]);
    assert!(
        cell.resolved
            .iter()
            .any(|entry| entry.channel == "focus-ring.width"
                && matches!(entry.value, StyleValue::Number(w) if w > 0.0))
    );
}

#[test]
fn an_unfocused_cell_records_no_ring_layer() {
    let cell = button_cell(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    assert!(
        cell.frame
            .quads
            .iter()
            .all(|quad| quad.layer != "focus-ring")
    );
}

#[test]
fn the_input_zero_shift_rectangles_are_identical_across_every_state_of_a_kind() {
    // `UIF-INP-003`: the text area and the reserved trailing slot are a
    // function of (bounds, kind) alone. If an error state moved either
    // one, this is the assertion that catches it.
    for kind in INPUT_KINDS {
        let reference = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
        );
        let text_area = resolved_line(&reference, "geometry.text-area");
        let slot = resolved_line(&reference, "geometry.trailing-slot");

        for state in snapshot_states() {
            let cell = text_input_cell(kind, state);
            assert_eq!(
                resolved_line(&cell, "geometry.text-area"),
                text_area,
                "{} moved the text area",
                cell.id
            );
            assert_eq!(
                resolved_line(&cell, "geometry.trailing-slot"),
                slot,
                "{} moved the trailing slot",
                cell.id
            );
        }
    }
}

#[test]
fn the_invalid_input_is_the_only_one_flagged_as_an_error_border() {
    for kind in INPUT_KINDS {
        let invalid = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Invalid,
                focused: false,
            },
        );
        let rest = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
        );

        assert_eq!(resolved_line(&invalid, "error-border"), "true");
        assert_eq!(resolved_line(&rest, "error-border"), "false");
    }
}

#[test]
fn the_invalid_and_focused_input_keeps_the_error_and_relocates_focus_to_a_ring() {
    let cell = text_input_cell(
        InputKind::TextEntry,
        VisualState {
            base: BaseState::Invalid,
            focused: true,
        },
    );

    assert_eq!(resolved_line(&cell, "error-border"), "true");
    assert!(
        cell.frame
            .quads
            .iter()
            .any(|quad| quad.layer == "focus-ring"),
        "focus must stay visible while the error holds the perimeter"
    );
    assert_eq!(
        resolved_line(&cell, "geometry.text-area"),
        resolved_line(
            &text_input_cell(
                InputKind::TextEntry,
                VisualState {
                    base: BaseState::Rest,
                    focused: false,
                }
            ),
            "geometry.text-area"
        ),
        "the ring is drawn outside the control, so the text area cannot shift"
    );
}

#[test]
fn every_rendered_cell_records_a_finite_frame_viewport() {
    for cell in matrix() {
        if cell.kind == CellKind::NotOwned {
            continue;
        }
        for value in cell.frame.viewport {
            assert!(
                value.is_finite(),
                "{} recorded a non-finite viewport",
                cell.id
            );
        }
        assert!(
            cell.frame.viewport[2] > 0.0 && cell.frame.viewport[3] > 0.0,
            "{} recorded an empty viewport",
            cell.id
        );
    }
}

#[test]
fn every_emitted_quad_is_finite_and_non_negative() {
    for cell in matrix() {
        for quad in &cell.frame.quads {
            for value in quad.bounds {
                assert!(
                    value.is_finite(),
                    "{} {} bounds are not finite",
                    cell.id,
                    quad.layer
                );
            }
            assert!(
                quad.border_width >= 0.0 && quad.radius >= 0.0,
                "{} {} has a negative stroke or radius",
                cell.id,
                quad.layer
            );
        }
    }
}

#[test]
fn the_modal_cells_differ_only_in_the_error_channels_they_add() {
    let cells = modal_cells();
    let clean = cells.iter().find(|c| c.id == "modal/clean").unwrap();
    let errored = cells.iter().find(|c| c.id == "modal/error").unwrap();

    assert!(!clean.resolved.iter().any(|e| e.channel == "error.code"));
    assert!(errored.resolved.iter().any(|e| e.channel == "error.code"));
    assert!(
        errored
            .resolved
            .iter()
            .any(|e| e.channel == "error.recovery")
    );
    assert_eq!(
        errored
            .resolved
            .iter()
            .filter(|e| e.channel.starts_with("frame."))
            .count(),
        clean
            .resolved
            .iter()
            .filter(|e| e.channel.starts_with("frame."))
            .count(),
        "an error must not change the card chrome"
    );
}

#[test]
fn the_input_dialog_validation_slot_is_recorded_whether_or_not_it_is_showing() {
    let cells = input_dialog_cells();
    let valid = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-valid")
        .unwrap();
    let invalid = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-invalid")
        .unwrap();

    assert_eq!(
        resolved_line(valid, "geometry.validation-slot"),
        resolved_line(invalid, "geometry.validation-slot"),
        "the slot occupies the same rectangle full or empty"
    );
    assert_eq!(
        resolved_line(valid, "geometry.footer"),
        resolved_line(invalid, "geometry.footer"),
        "the footer cannot shift when an error appears"
    );
    assert_eq!(resolved_line(valid, "ok-enabled"), "true");
    assert_eq!(resolved_line(invalid, "ok-enabled"), "false");
}

#[test]
fn the_delete_confirmation_carries_the_danger_intent_and_the_button_follows_it() {
    let cells = input_dialog_cells();
    let delete = cells
        .iter()
        .find(|c| c.id == "input-dialog/delete-danger")
        .unwrap();

    assert_eq!(resolved_line(delete, "intent"), "Danger");
    let primary = delete
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "footer-primary")
        .expect("the delete card must paint a primary footer button");

    // `UIF-INP-004` fixes the *intent*, and the intent selects the
    // variant. The crate's `Danger` button rests on a neutral surface and
    // carries the error red as its border — the red fill only arrives on
    // hover and press — so the proof that the intent drove the choice is
    // that the footer resolved to the Danger resting style rather than to
    // the `Primary` one a non-destructive card would have used.
    let palette = Palette::dark();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let danger = button::resolve_style(ButtonVariant::Danger, rest, &palette);
    let standard = button::resolve_style(ButtonVariant::Primary, rest, &palette);

    assert_eq!(
        primary.fill,
        token_name(danger.background),
        "the Danger intent must render the primary with the Danger variant's fill"
    );
    assert_eq!(primary.border, token_name(danger.border_color));
    assert_ne!(
        primary.fill,
        token_name(standard.background),
        "a destructive primary must not be the ordinary primary"
    );
    assert_eq!(
        token_name(danger.border_color),
        token_name(palette.status_danger),
        "the destructive signal is the error red on the border"
    );
}

#[test]
fn the_rename_dialog_rejects_an_unchanged_value_and_accepts_a_changed_one() {
    let cells = input_dialog_cells();
    let unchanged = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-invalid")
        .unwrap();
    let changed = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-valid")
        .unwrap();

    assert_eq!(resolved_line(unchanged, "ok-enabled"), "false");
    assert_eq!(resolved_line(changed, "ok-enabled"), "true");
}

#[test]
fn the_movable_cells_report_the_gesture_they_were_driven_with() {
    let cells = movable_cells();
    let idle = cells.iter().find(|c| c.id == "movable/idle").unwrap();
    let dragging = cells.iter().find(|c| c.id == "movable/dragging").unwrap();
    let resizing = cells.iter().find(|c| c.id == "movable/resizing").unwrap();
    let paused = cells
        .iter()
        .find(|c| c.id == "movable/auto-scroll-paused")
        .unwrap();

    assert_eq!(resolved_line(idle, "dragging"), "false");
    assert_eq!(resolved_line(dragging, "dragging"), "true");
    assert_eq!(resolved_line(resizing, "resizing"), "Some(BottomRight)");
    assert_eq!(resolved_line(paused, "follow"), "Paused");
    assert_eq!(resolved_line(idle, "follow"), "Following");

    assert_ne!(
        resolved_line(idle, "geometry.card"),
        resolved_line(dragging, "geometry.card"),
        "the drag must actually have moved the card"
    );
    assert_ne!(
        resolved_line(idle, "geometry.card"),
        resolved_line(resizing, "geometry.card"),
        "the resize must actually have changed the card"
    );
}

#[test]
fn every_movable_cell_paints_the_full_card_anatomy() {
    for cell in movable_cells() {
        let layers: Vec<&str> = cell
            .frame
            .quads
            .iter()
            .map(|quad| quad.layer.as_str())
            .collect();
        assert_eq!(
            layers,
            vec![
                "card",
                "title-bar",
                "filter-bar",
                "content-viewport",
                "footer"
            ],
            "{} did not paint the whole card",
            cell.id
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — scope honesty (D-2)
// ---------------------------------------------------------------------------

#[test]
fn the_not_owned_cells_are_the_three_named_surfaces_and_nothing_else() {
    let cells: Vec<Cell> = matrix()
        .into_iter()
        .filter(|cell| cell.kind == CellKind::NotOwned)
        .collect();

    let ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["navbar/not-owned", "panes/not-owned", "tabs/not-owned"]
    );
}

#[test]
fn every_not_owned_cell_carries_a_justification_and_draws_nothing() {
    for cell in not_owned_cells() {
        let note = cell
            .note
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no justification note", cell.id));
        assert!(
            note.contains("D-2"),
            "{}'s note must cite D-2, got: {note}",
            cell.id
        );
        assert!(
            note.len() > 80,
            "{}'s note is too thin to be a justification",
            cell.id
        );
        assert!(
            cell.frame.quads.is_empty(),
            "{} must not paint a frame it does not own",
            cell.id
        );
        assert!(cell.resolved.is_empty());
        assert!(cell.audit.is_empty());
    }
}

#[test]
fn every_rendered_cell_resolves_channels_and_audit_pairs() {
    for cell in matrix() {
        if cell.kind == CellKind::NotOwned {
            continue;
        }
        assert!(!cell.resolved.is_empty(), "{} resolved nothing", cell.id);
        assert!(!cell.audit.is_empty(), "{} audited nothing", cell.id);
    }
}

/// A cell may legitimately draw zero quads, and that is not the same thing
/// as being `NotOwned`.
///
/// A menu row at Rest paints no background at all — the resolver returns
/// `None` because there is no highlight to draw — and a Ghost button draws
/// a transparent one. Both are the resolvers' own answers, recorded as
/// given. What the gallery must never do is turn a surface it does not own
/// into a cell that looks rendered, which is what the `NotOwned` kind is
/// for and what the test above pins.
#[test]
fn a_resting_cell_that_paints_nothing_is_still_a_rendered_cell() {
    let cells = matrix();
    let empty: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.kind != CellKind::NotOwned && cell.frame.quads.is_empty())
        .collect();

    for cell in &empty {
        assert!(
            !cell.resolved.is_empty(),
            "{} drew nothing and resolved nothing, which is indistinguishable from NotOwned",
            cell.id
        );
    }

    assert!(
        empty.iter().any(|cell| cell.id == "menu/row/rest"),
        "the menu Rest row is expected to paint no background layer"
    );
}

// ---------------------------------------------------------------------------
// 5 — the z-band stack (G-13)
// ---------------------------------------------------------------------------

#[test]
fn the_zband_cell_paints_overlay_then_menu_then_scrim_then_modal() {
    let cell = zband_cell();
    let layers: Vec<&str> = cell
        .frame
        .quads
        .iter()
        .map(|quad| quad.layer.as_str())
        .collect();

    assert_eq!(
        layers,
        vec![
            "band1-overlay-card",
            "band2-menu-card",
            "band3-scrim",
            "band3-modal-card"
        ]
    );
}

#[test]
fn the_band_three_scrim_is_painted_over_the_band_two_menu_card() {
    let cell = zband_cell();
    let index = |wanted: &str| {
        cell.frame
            .quads
            .iter()
            .position(|quad| quad.layer == wanted)
            .unwrap_or_else(|| panic!("no {} quad", wanted))
    };

    assert!(
        index("band3-scrim") > index("band2-menu-card"),
        "G-13: the Modal band dims the menu as well as Bands 0 and 1"
    );
    assert!(index("band3-scrim") > index("band1-overlay-card"));
}

#[test]
fn escape_order_is_not_band_order() {
    assert_eq!(ZBand::Menu.index(), 2);
    assert_eq!(ZBand::Modal.index(), 3);
    assert!(
        ZBand::Menu.escape_rank() > ZBand::Modal.escape_rank(),
        "G-13: Menu unwinds before Modal even though it paints below it"
    );
    assert_eq!(ZBand::UtilityOverlay.escape_rank(), 1);
    assert_eq!(ZBand::Workspace.escape_rank(), 0);
}

#[test]
fn the_zband_escape_ranks_are_read_from_a_real_registry() {
    let cell = zband_cell();
    assert_eq!(resolved_line(&cell, "highest-open-band"), "Some(Modal)");
    assert_eq!(resolved_line(&cell, "topmost"), "zband/modal");
    assert_eq!(resolved_line(&cell, "escape-rank.menu"), "3");
    assert_eq!(resolved_line(&cell, "escape-rank.modal"), "2");
}

#[test]
fn the_zband_cell_states_the_finding_in_its_note() {
    let cell = zband_cell();
    let note = cell.note.expect("the z-band cell must state G-13");
    assert!(note.contains("G-13"));
    assert!(note.contains("paint order"));
    assert!(note.contains("escape order"));
}

// ---------------------------------------------------------------------------
// 6 — the gates
// ---------------------------------------------------------------------------

#[test]
fn no_cell_emits_a_colour_the_token_set_does_not_account_for() {
    let unmapped = unmapped_colours(&matrix());
    assert!(
        unmapped.is_empty(),
        "unmapped colours:\n{}",
        unmapped.join("\n")
    );
}

#[test]
fn the_contrast_ledger_is_empty() {
    let failures = contrast_failures(&matrix());
    assert!(
        failures.is_empty(),
        "accepted contrast failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_judged_pair_clears_its_role_threshold() {
    let mut judged = 0usize;
    for cell in matrix() {
        for pair in &cell.audit {
            let (ratio, _, class) = measure(pair);
            if class != AuditClass::Judged {
                continue;
            }
            judged += 1;
            assert!(
                ratio >= threshold(pair.role),
                "{} {} {} on {} measures {ratio} against a floor of {}",
                cell.id,
                pair.channel,
                token_name(pair.foreground),
                token_name(pair.background),
                threshold(pair.role)
            );
        }
    }
    assert!(judged > 100, "the audit judged only {judged} pairs");
}

#[test]
fn the_role_predicates_agree_with_the_measured_verdicts() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (ratio, painted, class) = measure(pair);
            if class != AuditClass::Judged {
                continue;
            }
            let predicate = match pair.role {
                ContrastRole::Text => contrast::meets_text_contrast(pair.foreground, painted),
                ContrastRole::Chrome => contrast::meets_chrome_contrast(pair.foreground, painted),
            };
            assert_eq!(
                predicate,
                ratio >= threshold(pair.role),
                "{} {} : the role predicate and the threshold disagree",
                cell.id,
                pair.channel
            );
        }
    }
}

#[test]
fn the_unpainted_background_class_only_covers_truly_unpainted_pairs() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::UnpaintedBackground {
                assert_eq!(
                    pair.background.a, 0.0,
                    "{} {} is classed unpainted but its background has alpha {}",
                    cell.id, pair.channel, pair.background.a
                );
            }
        }
    }
}

#[test]
fn the_coincident_class_only_covers_strokes_that_share_their_fill() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::CoincidentWithFill {
                assert!(
                    same_channels(pair.foreground, pair.background),
                    "{} {} is classed coincident but its channels differ",
                    cell.id,
                    pair.channel
                );
            }
        }
    }
}

#[test]
fn the_resting_divider_class_only_covers_border_subtle() {
    let palette = Palette::dark();
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::RestingDivider {
                assert!(
                    same_channels(pair.foreground, palette.border_subtle),
                    "{} {} is classed a resting divider but is not border.subtle",
                    cell.id,
                    pair.channel
                );
            }
        }
    }
}

#[test]
fn a_focus_ring_is_exempt_only_where_nothing_is_painted_behind_it() {
    // A focus ring is active chrome, so the only exemption it may ever
    // take is the one that says the measurement cannot be made: nothing
    // opaque was painted behind it. A Ghost at Rest is exactly that case
    // — its ring belongs to whatever the button is mounted to, and the
    // gallery cannot see the container. What must never happen is a ring
    // being waved through as a resting divider or a coincident stroke,
    // because those exemptions describe a stroke that deliberately does
    // not signal.
    let mut judged = 0usize;
    let mut exempt = 0usize;

    for cell in matrix() {
        for pair in &cell.audit {
            if pair.channel != "ring" {
                continue;
            }
            assert_eq!(pair.role, ContrastRole::Chrome, "{} ring role", cell.id);

            let (_, _, class) = measure(pair);
            match class {
                AuditClass::Judged => judged += 1,
                AuditClass::UnpaintedBackground => {
                    assert_eq!(
                        pair.background.a, 0.0,
                        "{}'s ring was exempted as unpainted but its background has alpha {}",
                        cell.id, pair.background.a
                    );
                    exempt += 1;
                }
                other => panic!("{}'s focus ring was exempted as {:?}", cell.id, other),
            }
        }
    }

    assert!(judged > 0, "no focus ring was judged at all");
    assert!(
        exempt > 0,
        "the Ghost ring-over-nothing case is not covered"
    );
}

#[test]
fn every_ring_on_an_opaque_background_is_judged_against_the_chrome_floor() {
    for cell in matrix() {
        for pair in &cell.audit {
            if pair.channel != "ring" || pair.background.a < 1.0 {
                continue;
            }
            let (ratio, _, class) = measure(pair);
            assert_eq!(class, AuditClass::Judged, "{} ring class", cell.id);
            assert!(
                ratio >= contrast::MIN_CHROME_CONTRAST,
                "{} ring {} on {} measures {ratio}",
                cell.id,
                token_name(pair.foreground),
                token_name(pair.background)
            );
        }
    }
}

#[test]
fn the_manifest_carries_its_own_pass_conditions() {
    let text = manifest();
    assert!(text.contains("gate: token mapping (UIF-VER-001)"));
    assert!(text.contains("gate: contrast (UIF-TOK-006)"));
    assert!(text.contains("gate: scope (D-2)"));
    assert!(text.contains("PASS - every emitted colour names a Palette token"));
    assert!(text.contains("PASS - every judged pair clears its role threshold"));
    assert!(text.contains("3 surfaces emitted as NotOwned rather than omitted"));
    assert!(!text.contains("FAIL"));
}

#[test]
fn the_manifest_records_every_cell_snapshot() {
    let text = manifest();
    for cell in matrix() {
        assert!(
            text.contains(&format!("cell: {}\n", cell.id)),
            "the manifest omits {}",
            cell.id
        );
    }
}
