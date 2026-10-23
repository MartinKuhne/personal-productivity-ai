//! Tests for the combobox gallery cells — `UIF-ATOM-004`, §3.4.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Twenty-nine cells, unique ids, the
//!    fixed subject/spec triple, and the order they are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What the cells record.** The open/closed chevron, the six state
//!    columns, and the tier cells reporting the tier the host really used.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue, judge, unmapped_colours};

/// The expected cell count: 2 rungs x 6 states x 2 open + 5 tiers.
const EXPECTED_CELLS: usize = 29;

/// The trigger cells, i.e. everything that is not a tier cell.
fn trigger_cells() -> Vec<Cell> {
    combo_cells()
        .into_iter()
        .filter(|cell| cell.subject == "combo")
        .collect()
}

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

/// The colour a resolved channel carries, if it is a colour entry.
fn colour(cell: &Cell, channel: &str) -> Option<Color> {
    value(cell, channel).and_then(StyleValue::colour)
}

/// Find the trigger cell for one `(rung, state, open)` triple.
fn find(rung: ComboRung, state: VisualState, open: bool) -> Cell {
    let id = format!(
        "combo/{}-{}-{}",
        rung.slug(),
        state_slug(state),
        if open { "open" } else { "closed" }
    );
    combo_cells()
        .into_iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no cell with id {id}"))
}

/// The tier cell for one tier.
fn tier_cell(tier: ComboTier) -> Cell {
    combo_cells()
        .into_iter()
        .find(|cell| cell.id == format!("combo-tier/{}", tier.slug()))
        .unwrap_or_else(|| panic!("no tier cell for {:?}", tier))
}

// ---------------------------------------------------------------------------
// 1 — the shape of the block
// ---------------------------------------------------------------------------

#[test]
fn the_combo_block_is_twenty_nine_cells() {
    assert_eq!(combo_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_cell_answers_to_the_combobox_requirement() {
    for cell in combo_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-004", "{}", cell.id);
        assert!(
            cell.subject == "combo" || cell.subject == "combo-tier",
            "{} has an unexpected subject {}",
            cell.id,
            cell.subject
        );
    }
}

#[test]
fn the_cell_ids_are_unique() {
    let cells = combo_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate combo cell ids: {ids:?}");
}

#[test]
fn the_trigger_cells_come_before_the_tier_cells() {
    let cells = combo_cells();
    let first_tier = cells
        .iter()
        .position(|cell| cell.subject == "combo-tier")
        .expect("a tier cell exists");
    let last_trigger = cells
        .iter()
        .rposition(|cell| cell.subject == "combo")
        .expect("a trigger cell exists");
    assert!(last_trigger < first_tier, "atoms first, then tiers");
}

#[test]
fn the_trigger_block_is_two_rungs_x_six_states_x_two_open() {
    assert_eq!(trigger_cells().len(), 24);
    for rung in combo_box::RUNGS {
        for state in COLUMNS {
            for open in [false, true] {
                let cell = find(rung, state, open);
                assert_eq!(text(&cell, "rung"), Some(rung.slug()));
                assert_eq!(
                    text(&cell, "flyout"),
                    if open { Some("open") } else { Some("closed") }
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — the gates
// ---------------------------------------------------------------------------

#[test]
fn no_colour_is_unmapped() {
    let unmapped = unmapped_colours(&combo_cells());
    assert!(unmapped.is_empty(), "unmapped combo colours: {unmapped:?}");
}

#[test]
fn every_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = combo_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(
        failing.is_empty(),
        "failing combo contrast pairs: {failing:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — what the cells record
// ---------------------------------------------------------------------------

#[test]
fn the_sixth_column_is_the_validation_state_3_4_gives_the_control() {
    assert_eq!(COLUMNS.len(), 6);
    assert_eq!(COLUMNS[4].base, BaseState::Invalid);
    assert_eq!(COLUMNS[5].base, BaseState::Disabled);
}

#[test]
fn opening_the_flyout_makes_the_chevron_strong_from_rest() {
    let palette = Palette::dark();
    let closed = find(ComboRung::Standard, COLUMNS[0], false);
    let open = find(ComboRung::Standard, COLUMNS[0], true);
    assert_eq!(colour(&closed, "chevron.color"), Some(palette.text_muted));
    assert_eq!(colour(&open, "chevron.color"), Some(palette.text_strong));
}

#[test]
fn the_open_axis_changes_only_the_chevron_not_the_trigger_fill() {
    let closed = find(ComboRung::Standard, COLUMNS[0], false);
    let open = find(ComboRung::Standard, COLUMNS[0], true);
    assert_eq!(
        colour(&closed, "trigger.fill"),
        colour(&open, "trigger.fill")
    );
    assert_eq!(
        colour(&closed, "trigger.border.color"),
        colour(&open, "trigger.border.color")
    );
    assert_ne!(
        colour(&closed, "chevron.color"),
        colour(&open, "chevron.color")
    );
}

#[test]
fn the_invalid_column_paints_the_danger_chevron() {
    let cell = find(ComboRung::Standard, COLUMNS[4], false);
    assert_eq!(
        colour(&cell, "chevron.color"),
        Some(Palette::dark().status_danger_text)
    );
}

#[test]
fn the_disabled_column_paints_a_dead_chevron_on_both_open_states() {
    for open in [false, true] {
        let cell = find(ComboRung::Compact, COLUMNS[5], open);
        assert_eq!(
            colour(&cell, "chevron.color"),
            Some(Palette::dark().text_disabled)
        );
    }
}

#[test]
fn the_rungs_change_the_height_and_nothing_else_about_the_chevron() {
    for state in COLUMNS {
        let std = find(ComboRung::Standard, state, false);
        let cmp = find(ComboRung::Compact, state, false);
        assert_eq!(
            number(&std, "trigger.height"),
            Some(combo_box::HEIGHT_STANDARD)
        );
        assert_eq!(
            number(&cmp, "trigger.height"),
            Some(combo_box::HEIGHT_COMPACT)
        );
        assert_eq!(colour(&std, "chevron.color"), colour(&cmp, "chevron.color"));
    }
}

#[test]
fn every_trigger_cell_declares_the_chevron_it_paints() {
    for cell in trigger_cells() {
        assert!(
            cell.audit.iter().any(|pair| pair.channel == "chevron"),
            "{} does not audit its chevron",
            cell.id
        );
    }
}

#[test]
fn the_flyout_geometry_is_recorded_on_every_trigger_cell() {
    for cell in trigger_cells() {
        assert_eq!(
            number(&cell, "flyout.max-height"),
            Some(combo_model::FLYOUT_MAX_HEIGHT)
        );
        assert_eq!(number(&cell, "flyout.gap"), Some(combo_model::FLYOUT_GAP));
        assert_eq!(
            number(&cell, "trailing.slot"),
            Some(combo_box::TRAILING_SLOT)
        );
        assert_eq!(number(&cell, "chevron.size"), Some(combo_box::CHEVRON_SIZE));
    }
}

#[test]
fn the_tier_cells_record_the_tier_the_host_actually_used() {
    let expected = [
        (ComboTier::Open, "OpenChanged"),
        (ComboTier::FilterLive, "FilterChanged"),
        (ComboTier::Commit, "Committed+OpenChanged"),
        (ComboTier::Cancel, "Cancelled+OpenChanged"),
    ];
    for (tier, emission) in expected {
        let cell = tier_cell(tier);
        assert_eq!(
            text(&cell, "emissions"),
            Some(emission),
            "{:?} should have produced exactly {emission}",
            tier
        );
    }
}

#[test]
fn the_commit_and_cancel_tiers_are_the_only_value_changes() {
    for tier in [
        ComboTier::Open,
        ComboTier::FilterLive,
        ComboTier::Commit,
        ComboTier::Cancel,
        ComboTier::NoMatch,
    ] {
        let cell = tier_cell(tier);
        let expected = if tier.is_value_change() {
            "true"
        } else {
            "false"
        };
        assert_eq!(text(&cell, "value-changed"), Some(expected), "{tier:?}");
    }
}

#[test]
fn the_no_match_tier_records_that_the_flyout_stayed_shut() {
    let cell = tier_cell(ComboTier::NoMatch);
    assert_eq!(
        text(&cell, "flyout"),
        Some("closed"),
        "a filter matching nothing cannot open a flyout"
    );
    assert_eq!(
        text(&cell, "emissions"),
        Some("FilterChanged"),
        "the keystroke is reported even though no list appeared for it"
    );
}

#[test]
fn the_open_and_commit_tiers_end_with_the_flyout_open_then_closed() {
    assert_eq!(text(&tier_cell(ComboTier::Open), "flyout"), Some("open"));
    assert_eq!(
        text(&tier_cell(ComboTier::Commit), "flyout"),
        Some("closed")
    );
    assert_eq!(
        text(&tier_cell(ComboTier::Cancel), "flyout"),
        Some("closed")
    );
}

#[test]
fn the_tier_cells_are_composites_because_they_are_driven_through_the_host() {
    for tier in [
        ComboTier::Open,
        ComboTier::FilterLive,
        ComboTier::Commit,
        ComboTier::Cancel,
        ComboTier::NoMatch,
    ] {
        assert_eq!(tier_cell(tier).kind, CellKind::Composite, "{tier:?}");
    }
}

// ---------------------------------------------------------------------------
// 4 — byte stability
// ---------------------------------------------------------------------------

#[test]
fn a_cell_built_twice_equals_itself() {
    for rung in combo_box::RUNGS {
        for state in COLUMNS {
            for open in [false, true] {
                assert_eq!(combo_cell(rung, state, open), combo_cell(rung, state, open));
            }
        }
    }
    for tier in [
        ComboTier::Open,
        ComboTier::FilterLive,
        ComboTier::Commit,
        ComboTier::Cancel,
        ComboTier::NoMatch,
    ] {
        assert_eq!(combo_tier_cell(tier), combo_tier_cell(tier));
    }
}
