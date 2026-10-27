//! Tests for the badge gallery cells — `UIF-ATOM-008`, §3.8.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Fifty cells, unique ids, the fixed
//!    kind/subject/spec triple, and the order they are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What each kind records.** The zero-count cells carry no audit
//!    pairs; the solid pills say so; the chips carry their glyph.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};

/// The expected cell count: 5 dots + 25 pills + 20 chips.
const EXPECTED_CELLS: usize = 50;

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

#[test]
fn the_badge_block_is_fifty_cells() {
    assert_eq!(badge_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_badge_cell_answers_to_the_badge_requirement() {
    for cell in badge_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-008", "{}", cell.id);
        assert_eq!(cell.subject, "badge", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_badge_cell_ids_are_unique() {
    let cells = badge_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate badge cell ids: {ids:?}");
}

#[test]
fn the_cells_are_emitted_dot_then_pill_then_chip() {
    let cells = badge_cells();
    let kinds: Vec<&str> = cells
        .iter()
        .map(|cell| text(cell, "kind").unwrap_or("?"))
        .collect();

    let dots = kinds.iter().filter(|k| **k == "dot").count();
    let counts = kinds.iter().filter(|k| **k == "count").count();
    let chips = kinds.iter().filter(|k| **k == "chip").count();

    assert_eq!((dots, counts, chips), (5, 25, 20));
    assert_eq!(kinds[0], "dot");
    assert_eq!(kinds[5], "count");
    assert_eq!(kinds[30], "chip");
}

#[test]
fn no_badge_cell_emits_an_unmapped_colour() {
    let unmapped = unmapped_colours(&badge_cells());
    assert!(unmapped.is_empty(), "unmapped: {unmapped:?}");
}

#[test]
fn every_badge_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = badge_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| {
            format!(
                "{} {} on {}",
                cell.id,
                pair.channel,
                cell.namer.name(pair.background)
            )
        })
        .collect();

    assert!(failing.is_empty(), "failing badge pairs: {failing:?}");
}

#[test]
fn the_zero_count_cells_record_that_nothing_is_painted() {
    let cells = badge_cells();
    let zeros: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.id.contains("/count/0/"))
        .collect();

    assert_eq!(zeros.len(), 5);
    for cell in zeros {
        assert_eq!(text(cell, "painted"), Some("false"), "{}", cell.id);
        assert!(
            cell.audit.is_empty(),
            "{} paints nothing, so it must offer the audit nothing to measure",
            cell.id
        );
    }
}

#[test]
fn the_non_empty_badge_cells_are_all_marked_painted() {
    let cells = badge_cells();
    for cell in &cells {
        if cell.id.contains("/count/0/") {
            continue;
        }
        assert_eq!(text(cell, "painted"), Some("true"), "{}", cell.id);
        assert!(!cell.audit.is_empty(), "{}", cell.id);
    }
}

#[test]
fn only_the_accent_and_danger_pills_are_recorded_as_solid() {
    let cells = badge_cells();
    for cell in &cells {
        if !cell.id.contains("/count/") || cell.id.contains("/count/0/") {
            continue;
        }
        let expected = if cell.id.contains("/accent") || cell.id.contains("/danger") {
            "true"
        } else {
            "false"
        };
        assert_eq!(text(cell, "fill.solid"), Some(expected), "{}", cell.id);
    }
}

#[test]
fn every_chip_cell_carries_its_mandatory_glyph() {
    let chips: Vec<Cell> = badge_cells()
        .into_iter()
        .filter(|cell| cell.id.contains("/chip/"))
        .collect();

    assert_eq!(chips.len(), 20);
    for cell in &chips {
        let glyph = text(cell, "glyph.char").unwrap_or_default();
        let ch = glyph.chars().next().unwrap_or(' ');
        assert!(
            ['✓', '⚠', '✕', '●'].contains(&ch),
            "{} carries the glyph {glyph:?}, which is not one §3.8 prints",
            cell.id
        );
        assert_eq!(text(cell, "label"), Some("Status"), "{}", cell.id);
        assert!(
            cell.audit.iter().any(|pair| pair.channel == "badge.glyph"),
            "{} must audit its glyph",
            cell.id
        );
    }
}

#[test]
fn the_box_heights_are_the_three_numbers_the_spec_prints() {
    let cells = badge_cells();
    for cell in &cells {
        let height = number(cell, "geometry.height").unwrap_or_default();
        let kind = text(cell, "kind").unwrap_or_default();
        let expected = match kind {
            "dot" => 6.0,
            "count" => 18.0,
            "chip" => 24.0,
            other => panic!("{} has kind {other:?}", cell.id),
        };
        assert_eq!(height, expected, "{}", cell.id);
    }
}

#[test]
fn two_badge_cell_calls_are_byte_identical() {
    let a: Vec<String> = badge_cells().iter().map(Cell::snapshot).collect();
    let b: Vec<String> = badge_cells().iter().map(Cell::snapshot).collect();
    assert_eq!(a, b);
}
