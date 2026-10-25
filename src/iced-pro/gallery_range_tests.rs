//! Tests for the range-control gallery cells — `UIF-ATOM-005`, §3.5.
//!
//! Five groups, in order:
//!
//! 1. **The shape of the block.** Thirty-one cells with unique ids and
//!    the fixed kind/subject/spec triple, split across the four families.
//! 2. **The gates.** No emitted colour is unmapped and every audit pair
//!    passes its floor.
//! 3. **The thumb's carrier.** Each slider cell names its audit channel
//!    after the channel that actually separates the thumb from the filled
//!    rail, so no state is quietly measured through an exempt pair.
//! 4. **The tier transcripts.** The drag tier and the step tier record
//!    the sequences they are claiming — one commit on release, a restore
//!    on cancel, a settle for a pointer that left the track, and no Live
//!    tier on a keyboard step.
//! 5. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};

/// The expected cell count: 12 slider + 4 slider-tier + 12 number box +
/// 7 number-box-tier.
const EXPECTED_CELLS: usize = 35;

/// Look up a resolved channel on a cell.
fn value<'a>(cell: &'a Cell, channel: &str) -> Option<&'a StyleValue> {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .map(|entry| &entry.value)
}

/// The text a resolved channel carries, if it is a text entry.
fn text<'a>(cell: &'a Cell, channel: &str) -> Option<&'a str> {
    match value(cell, channel) {
        Some(StyleValue::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The number a resolved channel carries, if it is a numeric entry.
fn number(cell: &Cell, channel: &str) -> Option<f32> {
    match value(cell, channel) {
        Some(StyleValue::Number(n)) => Some(*n),
        _ => None,
    }
}

/// The cells whose id starts with one prefix.
fn cells_with_prefix(prefix: &str) -> Vec<Cell> {
    range_cells()
        .into_iter()
        .filter(|cell| cell.id.starts_with(prefix))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. The shape of the block
// ---------------------------------------------------------------------------

#[test]
fn the_range_block_is_thirty_five_cells() {
    assert_eq!(range_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_range_cell_answers_to_the_range_requirement() {
    for cell in range_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-005", "{}", cell.id);
        assert!(
            matches!(
                cell.subject,
                "slider" | "slider-tier" | "number-box" | "number-box-tier"
            ),
            "{}: unexpected subject {}",
            cell.id,
            cell.subject
        );
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_range_cell_ids_are_unique() {
    let cells = range_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate range cell ids: {ids:?}");
}

#[test]
fn the_block_is_twelve_sliders_four_tiers_twelve_boxes_and_seven_box_tiers() {
    assert_eq!(cells_with_prefix("slider/").len(), 12);
    assert_eq!(cells_with_prefix("slider-tier/").len(), 4);
    assert_eq!(cells_with_prefix("number-box/").len(), 12);
    assert_eq!(cells_with_prefix("number-box-tier/").len(), 7);
}

#[test]
fn every_state_appears_in_every_slider_and_box_configuration() {
    for family in ["plain", "ticks"] {
        for state in CellState::ALL {
            let id = format!("slider/{family}-{}", state.slug());
            assert!(
                range_cells().iter().any(|cell| cell.id == id),
                "missing {id}"
            );
        }
    }
    for shape in ["stacked", "side-by-side"] {
        for state in CellState::ALL {
            let id = format!("number-box/{shape}-{}", state.slug());
            assert!(
                range_cells().iter().any(|cell| cell.id == id),
                "missing {id}"
            );
        }
    }
}

#[test]
fn the_cell_state_slugs_are_the_snapshot_names_the_matrix_iterates() {
    let slugs: Vec<&str> = CellState::ALL.iter().map(|s| s.slug()).collect();
    assert_eq!(
        slugs,
        vec![
            "rest",
            "hovered",
            "pressed",
            "rest+focused",
            "invalid",
            "disabled"
        ]
    );
}

#[test]
fn only_the_rest_focused_state_carries_a_visual_focus_flag() {
    for state in CellState::ALL {
        assert_eq!(
            state.visual().focused,
            state == CellState::RestFocused,
            "{state:?}: focus flag must track the cell"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. The gates
// ---------------------------------------------------------------------------

#[test]
fn no_range_cell_emits_an_unmapped_colour() {
    let cells = range_cells();
    assert_eq!(unmapped_colours(&cells), Vec::<String>::new());
}

#[test]
fn every_range_audit_pair_clears_its_floor() {
    for cell in range_cells() {
        for pair in &cell.audit {
            assert!(
                judge(pair),
                "{}: {} failed at {:.4}",
                cell.id,
                pair.channel,
                crate::gallery_audit::measure(pair).0
            );
        }
    }
}

#[test]
fn the_fill_on_unfill_ratio_is_recorded_but_never_audited() {
    // The sub-floor interior measurement is in the resolved channels so
    // it is in the artifact, and is absent from the audit list so it is
    // not claimed as a perimeter. Both halves of that decision are pinned.
    let cells = range_cells();
    for cell in cells.iter().filter(|c| c.id.starts_with("slider/")) {
        let recorded = number(cell, "fill-on-unfill.ratio").unwrap_or_default();
        assert!(
            (recorded - 2.9768).abs() < 0.01,
            "{}: fill-on-unfill recorded {recorded}",
            cell.id
        );
        for pair in &cell.audit {
            assert!(
                !pair.channel.contains("fill-on-unfill"),
                "{}: the interior pair must not be audited",
                cell.id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. The thumb's carrier
// ---------------------------------------------------------------------------

#[test]
fn each_slider_cell_names_the_channel_actually_carrying_its_thumb() {
    let palette = Palette::dark();
    for ticks in [false, true] {
        for state in CellState::ALL {
            let cell = slider_cell(ticks, state);
            let style = slider::resolve_style(state.visual(), palette.surface_overlay, &palette);
            let channel = match slider::thumb_carrier(&style, style.rail_filled) {
                slider::ThumbCarrier::Face | slider::ThumbCarrier::Both => {
                    "slider.thumb.face-on-fill"
                }
                slider::ThumbCarrier::Ring => "slider.thumb.ring-on-fill",
                slider::ThumbCarrier::Neither => {
                    panic!("{state:?}: the thumb is invisible against its own rail")
                }
            };
            assert!(
                cell.audit
                    .iter()
                    .any(|pair| pair.channel == channel && pair.background == style.rail_filled),
                "{state:?}: expected the audit to run through {channel}"
            );
        }
    }
}

#[test]
fn the_resting_thumb_is_carried_by_its_face_and_the_hovered_one_by_its_ring() {
    // The two states the union rule was written for, pinned at the cell
    // level rather than only in the resolver's own sidecar.
    let palette = Palette::dark();
    let rest = slider::resolve_style(CellState::Rest.visual(), palette.surface_overlay, &palette);
    let hover = slider::resolve_style(
        CellState::Hovered.visual(),
        palette.surface_overlay,
        &palette,
    );
    assert_eq!(
        slider::thumb_carrier(&rest, rest.rail_filled),
        slider::ThumbCarrier::Face
    );
    assert_eq!(
        slider::thumb_carrier(&hover, hover.rail_filled),
        slider::ThumbCarrier::Ring
    );
}

#[test]
fn the_disabled_thumb_is_carried_by_its_raised_face() {
    let palette = Palette::dark();
    let off = slider::resolve_style(
        CellState::Disabled.visual(),
        palette.surface_overlay,
        &palette,
    );
    assert_eq!(
        off.rail_filled, palette.border_strong,
        "the rail lost its accent"
    );
    assert_eq!(
        slider::thumb_carrier(&off, off.rail_filled),
        slider::ThumbCarrier::Face
    );
}

#[test]
fn only_the_ticks_cells_draw_ticks() {
    for state in CellState::ALL {
        assert_eq!(
            number(&slider_cell(false, state), "ticks.count"),
            Some(0.0),
            "{state:?}: a plain slider has no graduations"
        );
        assert_eq!(
            number(&slider_cell(true, state), "ticks.count"),
            Some(11.0),
            "{state:?}: an eleven-graduation slider"
        );
    }
}

#[test]
fn the_focus_ring_channels_appear_only_on_the_focused_cell() {
    for state in CellState::ALL {
        let cell = slider_cell(false, state);
        if state == CellState::RestFocused {
            assert!(
                value(&cell, "focus.ring").is_some(),
                "{state:?}: a focused cell records its ring"
            );
            assert!(
                cell.audit
                    .iter()
                    .any(|pair| pair.channel == "slider.focus-ring-on-mount"),
                "{state:?}: and audits it against the mount"
            );
        } else {
            assert!(
                value(&cell, "focus.ring").is_none(),
                "{state:?}: no ring, no channel"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. The tier transcripts
// ---------------------------------------------------------------------------

#[test]
fn the_live_cell_is_all_live_and_still_open() {
    let cell = slider_live_cell();
    assert_eq!(
        number(&cell, "transcript.live"),
        Some(3.0),
        "press plus two drags"
    );
    assert_eq!(number(&cell, "transcript.commit"), Some(0.0));
    assert_eq!(text(&cell, "gesture-open"), Some("true"));
    assert_eq!(text(&cell, "settled"), Some("false"));
    assert_eq!(number(&cell, "captured"), Some(45.0), "the pre-drag value");
}

#[test]
fn the_committed_cell_settles_exactly_once() {
    let cell = slider_committed_cell();
    assert_eq!(number(&cell, "transcript.commit"), Some(1.0));
    assert_eq!(number(&cell, "commits"), Some(1.0));
    assert_eq!(number(&cell, "transcript.cancel"), Some(0.0));
    assert_eq!(text(&cell, "settled"), Some("true"));
    assert_eq!(text(&cell, "gesture-open"), Some("false"));
    assert_eq!(
        number(&cell, "committed"),
        number(&cell, "value-before-release"),
        "the commit carries the value the drag reached"
    );
}

#[test]
fn the_cancelled_cell_restores_the_pre_press_value_not_the_dragged_one() {
    let cell = slider_cancelled_cell();
    assert_eq!(number(&cell, "transcript.cancel"), Some(1.0));
    assert_eq!(number(&cell, "transcript.commit"), Some(0.0));
    assert_eq!(number(&cell, "restored"), Some(45.0), "back to the press");
    assert_eq!(number(&cell, "dragged-to"), Some(50.0), "not this");
    assert_ne!(
        number(&cell, "restored"),
        number(&cell, "dragged-to"),
        "a cancel is a revert, not a step back"
    );
}

#[test]
fn the_release_outside_cell_still_settles_and_leaks_nothing() {
    let cell = slider_release_outside_cell();
    assert_eq!(
        number(&cell, "transcript.commit"),
        Some(1.0),
        "a release outside the track still settles"
    );
    assert_eq!(number(&cell, "clamped-fraction"), Some(1.0));
    assert_eq!(number(&cell, "value"), Some(100.0));
    assert_eq!(text(&cell, "capturing-before-release"), Some("true"));
    assert_eq!(text(&cell, "capturing"), Some("false"), "capture released");
    assert_eq!(
        text(&cell, "gesture-open"),
        Some("false"),
        "and no open gesture survives"
    );
    assert!(
        number(&cell, "pointer.x-outside-track").unwrap_or_default() > 216.0,
        "the pointer really was past the rail"
    );
}

#[test]
fn every_keyboard_step_cell_is_commits_only_and_never_live() {
    let cells = number_box_key_cells();
    assert_eq!(cells.len(), 6);
    for cell in &cells {
        assert_eq!(
            number(cell, "transcript.live"),
            Some(0.0),
            "{}: a keyboard step is never a Live frame",
            cell.id
        );
        assert_eq!(
            number(cell, "transcript.commit"),
            Some(2.0),
            "{}: two presses, two commits",
            cell.id
        );
    }
}

#[test]
fn the_keyboard_step_cells_reach_the_values_their_keys_name() {
    let cells = number_box_key_cells();
    let find = |slug: &str| -> String {
        cells
            .iter()
            .find(|c| c.id == format!("number-box-tier/{slug}"))
            .and_then(|c| text(c, "value-after"))
            .unwrap_or_default()
            .to_string()
    };
    // From 45 with a 5 step on 0..=100, twice.
    assert_eq!(find("step-up"), "55");
    assert_eq!(find("step-down"), "35");
    assert_eq!(find("page-up"), "100", "clamped at the top");
    assert_eq!(find("page-down"), "0", "clamped at the bottom");
    assert_eq!(find("home"), "0");
    assert_eq!(find("end"), "100");
}

#[test]
fn the_held_cell_is_live_while_held_and_commits_once_on_release() {
    let cell = number_box_held_cell();
    assert_eq!(number(&cell, "transcript.live"), Some(5.0));
    assert_eq!(number(&cell, "transcript.commit"), Some(1.0));
    assert_eq!(number(&cell, "steps"), Some(0.0), "the hold was released");
    assert_eq!(text(&cell, "settled"), Some("true"));
    assert_eq!(number(&cell, "repeat.interval-ms"), Some(100.0));
}

#[test]
fn every_tier_cell_carries_a_note_explaining_what_it_proves() {
    for cell in range_cells() {
        if cell.id.contains("-tier/") {
            assert!(
                cell.note.is_some(),
                "{}: a tier cell has to say what its transcript shows",
                cell.id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Byte stability
// ---------------------------------------------------------------------------

#[test]
fn building_the_block_twice_produces_the_same_cells() {
    assert_eq!(range_cells(), range_cells());
}

#[test]
fn the_number_box_cells_carry_both_shapes_glyphs() {
    let stacked = number_box_cell(StepperShape::Stacked, CellState::Rest);
    let abreast = number_box_cell(StepperShape::SideBySide, CellState::Rest);
    assert_eq!(text(&stacked, "glyph.decrement"), Some("▼"));
    assert_eq!(text(&stacked, "glyph.increment"), Some("▲"));
    assert_eq!(text(&abreast, "glyph.decrement"), Some("−"));
    assert_eq!(text(&abreast, "glyph.increment"), Some("+"));
    assert_eq!(
        number(&abreast, "stepper.width"),
        Some(48.0),
        "the side-by-side block is the wider one"
    );
    assert_eq!(number(&stacked, "stepper.width"), Some(28.0));
}
