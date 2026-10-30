//! Tests for the toggle-family gallery cells — `UIF-ATOM-003`,
//! `UIF-VER-001`, `UIF-VER-004`.
//!
//! Three groups:
//!
//! 1. **Shape of the set.** Thirty cells, unique ids, a fixed order, and
//!    the whole matrix grown by exactly that many.
//! 2. **The gates.** Every colour the cells emit maps to a token, and
//!    every judged audit pair clears the threshold its role carries.
//! 3. **What the frame actually contains.** The layer stack per control:
//!    where the thumb is, when the dot appears, why the tick does not,
//!    and that the ring grows outward rather than eating the fill.

use super::*;
use crate::gallery::{Cell, FrameQuad, StyleValue, matrix};
use crate::gallery_audit::{contrast_failures, unmapped_colours};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::{shape, size};
use crate::widget::toggle::{self, ToggleControl, TogglePolarity};
use iced::Color;

fn cells() -> Vec<Cell> {
    toggle_cells()
}

/// The quad a frame drew for a layer, if it drew one.
fn quad<'a>(cell: &'a Cell, layer: &str) -> Option<&'a FrameQuad> {
    cell.frame.quads.iter().find(|q| q.layer == layer)
}

fn resolved<'a>(cell: &'a Cell, channel: &str) -> &'a StyleEntry {
    cell.resolved
        .iter()
        .find(|e| e.channel == channel)
        .unwrap_or_else(|| panic!("{} has no `{channel}` channel", cell.id))
}

fn colour(cell: &Cell, channel: &str) -> Color {
    resolved(cell, channel)
        .value
        .colour()
        .unwrap_or_else(|| panic!("{}: `{channel}` is not a colour", cell.id))
}

fn number(cell: &Cell, channel: &str) -> f32 {
    match &resolved(cell, channel).value {
        StyleValue::Number(value) => *value,
        other => panic!("{}: `{channel}` is not a number ({other:?})", cell.id),
    }
}

fn text(cell: &Cell, channel: &str) -> String {
    match &resolved(cell, channel).value {
        StyleValue::Text(value) => value.clone(),
        other => panic!("{}: `{channel}` is not text ({other:?})", cell.id),
    }
}

// ---------------------------------------------------------------------------
// 1. Shape of the set
// ---------------------------------------------------------------------------

#[test]
fn the_set_is_thirty_cells_in_a_fixed_order() {
    let all = cells();
    assert_eq!(all.len(), 30);

    let pairs = [
        (ToggleControl::Checkbox, TogglePolarity::Off),
        (ToggleControl::Checkbox, TogglePolarity::On),
        (ToggleControl::Radio, TogglePolarity::Off),
        (ToggleControl::Radio, TogglePolarity::On),
        (ToggleControl::Switch, TogglePolarity::Off),
        (ToggleControl::Switch, TogglePolarity::On),
    ];
    let expected_ids: Vec<String> = pairs
        .iter()
        .flat_map(|(control, polarity)| {
            COLUMNS.iter().map(move |state| {
                format!(
                    "{}/{}/{}",
                    control_slug(*control),
                    polarity_slug(*polarity),
                    state_slug(*state)
                )
            })
        })
        .collect();

    let actual: Vec<String> = all.iter().map(|c| c.id.clone()).collect();
    assert_eq!(actual, expected_ids);
    assert_eq!(
        cells(),
        all,
        "the cell set must be reproducible call to call"
    );
}

#[test]
fn every_cell_is_an_atom_answering_the_toggle_requirement() {
    for cell in cells() {
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
        assert_eq!(cell.spec, "UIF-ATOM-003", "{}", cell.id);
        assert_eq!(cell.namer, Namer::Standard, "{}", cell.id);
        assert!(
            ["checkbox", "radio", "switch"].contains(&cell.subject),
            "{}: unexpected subject {}",
            cell.id,
            cell.subject
        );
    }
}

#[test]
fn the_matrix_grew_by_exactly_thirty_toggle_cells() {
    let all = matrix();
    let ids: Vec<String> = all
        .iter()
        .filter(|c| c.spec == "UIF-ATOM-003")
        .map(|c| c.id.clone())
        .collect();
    assert_eq!(ids.len(), 30);
    assert_eq!(
        ids,
        cells().iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        "the matrix must carry the toggle cells in the order the builder emits them"
    );
}

#[test]
fn the_cell_ids_do_not_collide_across_the_whole_matrix() {
    let mut seen = std::collections::HashSet::new();
    let mut collisions = Vec::new();
    for cell in matrix() {
        if !seen.insert(cell.id.clone()) {
            collisions.push(cell.id.clone());
        }
    }
    assert!(collisions.is_empty(), "duplicate cell ids: {collisions:?}");
}

// ---------------------------------------------------------------------------
// 2. The gates
// ---------------------------------------------------------------------------

#[test]
fn no_toggle_cell_emits_an_unmapped_colour() {
    let failures = unmapped_colours(&cells());
    assert!(
        failures.is_empty(),
        "UIF-VER-001: the toggle cells produced colours no token accounts for: {failures:?}"
    );
}

#[test]
fn every_toggle_audit_pair_clears_its_gate() {
    let failures = contrast_failures(&cells());
    assert!(
        failures.is_empty(),
        "a toggle pairing failed the rule its role carries: {failures:?}"
    );
}

#[test]
fn the_resolved_colours_are_the_resolvers_own_and_nothing_else() {
    let palette = Palette::dark();
    for control in CONTROLS {
        for polarity in POLARITIES {
            for state in COLUMNS {
                let cell = toggle_cell(control, polarity, state);
                let style = toggle::resolve_style(polarity, state, &palette);

                assert_eq!(
                    colour(&cell, "indicator.fill"),
                    style.indicator_fill,
                    "{}",
                    cell.id
                );
                assert_eq!(
                    colour(&cell, "indicator.border.color"),
                    style.indicator_border,
                    "{}",
                    cell.id
                );
                assert_eq!(colour(&cell, "mark"), style.mark, "{}", cell.id);
                assert_eq!(colour(&cell, "label"), style.label, "{}", cell.id);
                assert_eq!(
                    colour(&cell, "focus-ring.color"),
                    style.focus_ring_color,
                    "{}",
                    cell.id
                );
                assert_eq!(
                    number(&cell, "focus-ring.width"),
                    style.focus_ring_width,
                    "{}",
                    cell.id
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. What the frame contains
// ---------------------------------------------------------------------------

#[test]
fn the_checkbox_never_draws_its_mark_as_a_quad() {
    // The tick is a font glyph. A rectangle in the manifest claiming to be
    // one would be a lie about what the renderer was told to draw.
    for cell in cells() {
        if cell.subject != "checkbox" {
            continue;
        }
        assert!(
            quad(&cell, "mark").is_none(),
            "{}: the checkbox mark is a glyph and must not be recorded as a quad",
            cell.id
        );
        assert_eq!(text(&cell, "mark.rendered"), "font-glyph", "{}", cell.id);
    }
}

#[test]
fn the_radio_draws_its_dot_only_when_selected() {
    for cell in cells() {
        if cell.subject != "radio" {
            continue;
        }
        let is_on = cell.id.contains("/on/");
        assert_eq!(
            quad(&cell, "mark").is_some(),
            is_on,
            "{}: the inner dot tracks the polarity",
            cell.id
        );
        if let Some(dot) = quad(&cell, "mark") {
            assert_eq!(dot.bounds[2], toggle::radio_dot_size());
            assert_eq!(dot.bounds[3], toggle::radio_dot_size());
            assert_eq!(dot.bounds[2], size::CHECKBOX_BOX / 2.0);
        }
    }
}

#[test]
fn the_switch_draws_a_thumb_in_both_polarities_and_it_moves() {
    let off = toggle_cell(
        ToggleControl::Switch,
        TogglePolarity::Off,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    let on = toggle_cell(
        ToggleControl::Switch,
        TogglePolarity::On,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );

    let off_thumb = quad(&off, "mark").expect("the off thumb is painted");
    let on_thumb = quad(&on, "mark").expect("the on thumb is painted");
    let off_track = quad(&off, "indicator").expect("the track is painted");

    assert_eq!(off_thumb.bounds[2], size::SWITCH_THUMB);
    assert_eq!(on_thumb.bounds[2], size::SWITCH_THUMB);
    assert!(
        on_thumb.bounds[0] > off_thumb.bounds[0],
        "the thumb must move along the track: off {} on {}",
        off_thumb.bounds[0],
        on_thumb.bounds[0]
    );
    assert_eq!(
        on_thumb.bounds[0] - off_thumb.bounds[0],
        toggle::thumb_travel()
    );
    assert_eq!(
        off_thumb.bounds[1] - off_track.bounds[1],
        size::SWITCH_THUMB_INSET,
        "the thumb is vertically inset inside the track"
    );
}

#[test]
fn the_indicator_layer_is_present_in_every_cell_at_the_controls_size() {
    for cell in cells() {
        let indicator = quad(&cell, "indicator").expect("every cell paints its indicator");
        let control = match cell.subject {
            "checkbox" => ToggleControl::Checkbox,
            "radio" => ToggleControl::Radio,
            _ => ToggleControl::Switch,
        };
        let (w, h) = toggle::indicator_size(control);
        assert_eq!(indicator.bounds[2], w, "{}", cell.id);
        assert_eq!(indicator.bounds[3], h, "{}", cell.id);
        assert_eq!(
            indicator.radius,
            toggle::indicator_radius(control),
            "{}",
            cell.id
        );
    }
}

#[test]
fn the_focused_column_is_rest_plus_ring_and_the_disabled_column_has_none() {
    for control in CONTROLS {
        for polarity in POLARITIES {
            let focused = toggle_cell(
                control,
                polarity,
                VisualState {
                    base: BaseState::Rest,
                    focused: true,
                },
            );
            let rest = toggle_cell(
                control,
                polarity,
                VisualState {
                    base: BaseState::Rest,
                    focused: false,
                },
            );
            let disabled = toggle_cell(
                control,
                polarity,
                VisualState {
                    base: BaseState::Disabled,
                    focused: false,
                },
            );

            assert!(quad(&focused, "focus-ring").is_some(), "{}", focused.id);
            assert!(quad(&rest, "focus-ring").is_none(), "{}", rest.id);
            assert!(
                quad(&disabled, "focus-ring").is_none(),
                "{}: a dead control draws no focus affordance",
                disabled.id
            );

            // The ring is *outside* the indicator: the box is grown and
            // the grown quad is stroked, so the indicator is untouched and
            // the ring is larger on every side by the ring width.
            let ring = quad(&focused, "focus-ring").expect("ring present");
            let indicator = quad(&focused, "indicator").expect("indicator present");
            assert_eq!(ring.border_width, shape::BORDER_FOCUS);
            assert!(ring.bounds[2] > indicator.bounds[2]);
            assert!(ring.bounds[3] > indicator.bounds[3]);
            assert_eq!(ring.fill, "transparent");
        }
    }
}

#[test]
fn the_checked_polarity_rings_in_white_and_the_off_polarity_in_the_accent() {
    let p = Palette::dark();
    let on = toggle_cell(
        ToggleControl::Checkbox,
        TogglePolarity::On,
        VisualState {
            base: BaseState::Rest,
            focused: true,
        },
    );
    let off = toggle_cell(
        ToggleControl::Checkbox,
        TogglePolarity::Off,
        VisualState {
            base: BaseState::Rest,
            focused: true,
        },
    );

    assert_eq!(colour(&on, "focus-ring.color"), p.text_strong);
    assert_eq!(colour(&off, "focus-ring.color"), p.accent_border);
}

#[test]
fn the_chrome_class_the_cell_reports_matches_the_derived_class() {
    let p = Palette::dark();
    let mut seen_classes = Vec::new();
    for cell in cells() {
        let class = toggle::indicator_chrome_class(
            colour(&cell, "indicator.border.color"),
            colour(&cell, "indicator.fill"),
            &p,
        );
        let reported = text(&cell, "chrome.class");
        assert_eq!(reported, chrome_class_label(class), "{}", cell.id);
        if !seen_classes.contains(&reported) {
            seen_classes.push(reported);
        }
    }

    // All three reachable classes show up across the thirty cells; if one
    // ever silently disappeared the matrix stopped covering a case §3.3
    // prints.
    assert!(seen_classes.contains(&"judged".to_string()));
    assert!(seen_classes.contains(&"coincident-with-fill".to_string()));
    assert!(seen_classes.contains(&"resting-divider".to_string()));
    assert!(
        !seen_classes.contains(&"unpainted-background".to_string()),
        "no toggle cell paints a transparent indicator"
    );
}

#[test]
fn the_hit_geometry_extends_across_the_label_and_floors_at_the_minimum() {
    for cell in cells() {
        let hit_w = number(&cell, "geometry.hit.width");
        let bare_w = number(&cell, "geometry.hit.width-no-label");
        let hit_h = number(&cell, "geometry.hit.height");
        let gap = number(&cell, "geometry.label-gap");

        assert!(hit_w >= size::HIT_TARGET_MIN, "{}", cell.id);
        assert!(hit_h >= size::HIT_TARGET_MIN, "{}", cell.id);

        // The target is the indicator plus the gap plus the whole label, and
        // the bare indicator on its own is floored at the same minimum. So
        // the label always buys at least its own width — which is the whole
        // point of the extension for a 16px box.
        let indicator_w = number(&cell, "geometry.indicator.width");
        assert_eq!(
            hit_w,
            indicator_w + gap + SAMPLE_LABEL_WIDTH,
            "{}: hit = indicator + gap + label",
            cell.id
        );
        assert_eq!(
            bare_w,
            indicator_w.max(size::HIT_TARGET_MIN),
            "{}: the bare indicator is floored, not the row",
            cell.id
        );
        assert!(
            hit_w >= bare_w + SAMPLE_LABEL_WIDTH,
            "{}: the label must widen the target by at least its own width",
            cell.id
        );
    }
}
