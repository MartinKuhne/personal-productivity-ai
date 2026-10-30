//! Tests for the chip gallery cells — `UIF-ATOM-006`, §3.6.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Forty cells, unique ids, the fixed
//!    kind/subject/spec triple, and the order they are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What each shape records.** The radius split, the reserved remove
//!    slot, and the fill-vs-mount rule.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};

/// The expected cell count: 4 shapes × 2 rungs × 5 state columns.
const EXPECTED_CELLS: usize = 40;

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

/// Does this cell declare an audit pair on `channel`?
fn has_channel(cell: &Cell, channel: &str) -> bool {
    cell.audit.iter().any(|pair| pair.channel == channel)
}

#[test]
fn the_chip_block_is_forty_cells() {
    assert_eq!(chip_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_chip_cell_answers_to_the_chip_requirement() {
    for cell in chip_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-006", "{}", cell.id);
        assert_eq!(cell.subject, "chip", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_chip_cell_ids_are_unique() {
    let cells = chip_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate chip cell ids: {ids:?}");
}

#[test]
fn the_cells_are_emitted_shape_then_rung_then_state() {
    let cells = chip_cells();
    let shapes: Vec<&str> = cells
        .iter()
        .map(|cell| cell.id.split('/').nth(1).unwrap_or_default())
        .collect();
    let mut seen: Vec<&str> = Vec::new();
    for shape in &shapes {
        if seen.last() != Some(shape) {
            seen.push(shape);
        }
    }
    assert_eq!(
        seen,
        vec!["assist", "filter-off", "filter-on", "input"],
        "the shape block must run assist, filter-off, filter-on, input"
    );
}

#[test]
fn no_chip_colour_is_unmapped() {
    let unmapped = unmapped_colours(&chip_cells());
    assert!(unmapped.is_empty(), "unmapped chip colours: {unmapped:?}");
}

#[test]
fn every_chip_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = chip_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(
        failing.is_empty(),
        "chip pairs failing the gate: {failing:?}"
    );
}

#[test]
fn the_assist_shape_takes_the_pill_radius_and_every_other_shape_the_widget_radius() {
    for cell in chip_cells() {
        let radius = number(&cell, "radius").expect("every chip records a radius");
        if cell.id.contains("/assist/") {
            assert_eq!(
                radius,
                crate::widget::chip::RADIUS_PILL,
                "{}: assist takes radius.pill",
                cell.id
            );
        } else {
            assert_eq!(
                radius,
                crate::widget::chip::RADIUS_WIDGET,
                "{}: filter and input take radius.widget",
                cell.id
            );
        }
    }
}

#[test]
fn only_the_input_shape_reserves_a_remove_slot() {
    for cell in chip_cells() {
        let slot = text(&cell, "geometry.remove-slot").expect("every chip records its slot");
        if cell.id.contains("/input/") {
            assert_eq!(slot, "reserved", "{}", cell.id);
            assert_eq!(
                number(&cell, "geometry.remove-target"),
                Some(crate::widget::chip::REMOVE_TARGET),
                "{}: the slot is a 24px hit target",
                cell.id
            );
        } else {
            assert_eq!(slot, "none", "{}", cell.id);
            assert_eq!(
                number(&cell, "geometry.remove-target"),
                Some(0.0),
                "{}: nothing is reserved",
                cell.id
            );
        }
    }
}

#[test]
fn only_the_selected_filter_carries_a_check_glyph() {
    for cell in chip_cells() {
        let glyph = text(&cell, "glyph.kind").expect("every chip records its glyph kind");
        if cell.id.contains("/filter-on/") {
            assert_eq!(glyph, "check", "{}", cell.id);
        } else {
            assert_eq!(glyph, "none", "{}", cell.id);
        }
    }
}

#[test]
fn the_fill_is_audited_against_the_mount_only_for_the_selected_chip() {
    for cell in chip_cells() {
        let audited = has_channel(&cell, "chip.fill");
        if cell.id.contains("/filter-on/") && !cell.id.contains("disabled") {
            assert!(audited, "{}: the selected fill is a signal", cell.id);
        } else {
            assert!(
                !audited,
                "{}: an unselected or disabled fill is the surface itself",
                cell.id
            );
        }
    }
}

#[test]
fn the_two_rungs_are_the_two_heights_3_6_prices() {
    let mut compact = 0usize;
    let mut bar = 0usize;
    for cell in chip_cells() {
        let height = number(&cell, "geometry.height").expect("every chip records a height");
        if cell.id.contains("/compact-") {
            assert_eq!(height, crate::token::size::BAR_STATUS, "{}", cell.id);
            compact += 1;
        } else {
            assert_eq!(height, crate::token::size::CONTROL_SM, "{}", cell.id);
            bar += 1;
        }
    }
    assert_eq!(compact, EXPECTED_CELLS / 2);
    assert_eq!(bar, EXPECTED_CELLS / 2);
}

#[test]
fn the_focus_ring_is_audited_only_where_one_is_drawn() {
    for cell in chip_cells() {
        let width = number(&cell, "focus-ring.width").expect("every chip records a ring width");
        assert_eq!(
            has_channel(&cell, "chip.focus-ring"),
            width > 0.0,
            "{}: the ring pair follows the ring",
            cell.id
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a: Vec<String> = chip_cells().iter().map(Cell::snapshot).collect();
    let b: Vec<String> = chip_cells().iter().map(Cell::snapshot).collect();
    assert_eq!(a, b);
}
