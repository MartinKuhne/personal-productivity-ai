//! Tests for the progress gallery cells — `UIF-ATOM-009`, §3.9.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Twenty-two cells — fifteen bars, four
//!    spinners, three stop buttons — with unique ids and the fixed
//!    kind/subject/spec triple.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    passes.
//! 3. **The motion record.** The indeterminate cells carry the injected
//!    phase and no fraction; the reduced-motion cells are striped and
//!    frozen; the determinate cells carry a fraction and no motion.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};

/// The expected cell count: 15 bars + 4 spinners + 3 stop buttons.
const EXPECTED_CELLS: usize = 22;

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
    progress_cells()
        .into_iter()
        .filter(|cell| cell.id.starts_with(prefix))
        .collect()
}

#[test]
fn the_progress_block_is_twenty_two_cells() {
    assert_eq!(progress_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_progress_cell_answers_to_the_progress_requirement() {
    for cell in progress_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-009", "{}", cell.id);
        assert_eq!(cell.subject, "progress", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_progress_cell_ids_are_unique() {
    let cells = progress_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate progress cell ids: {ids:?}");
}

#[test]
fn the_block_is_fifteen_bars_four_spinners_and_three_stop_buttons() {
    assert_eq!(cells_with_prefix("progress/bar/").len(), 15);
    assert_eq!(cells_with_prefix("progress/spinner/").len(), 4);
    assert_eq!(cells_with_prefix("progress/stop/").len(), 3);
}

#[test]
fn no_progress_cell_emits_an_unmapped_colour() {
    let unmapped = unmapped_colours(&progress_cells());
    assert!(unmapped.is_empty(), "unmapped: {unmapped:?}");
}

#[test]
fn every_progress_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = progress_cells()
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

    assert!(failing.is_empty(), "failing progress pairs: {failing:?}");
}

#[test]
fn the_indeterminate_cells_carry_the_injected_phase_and_no_fraction() {
    let cells = cells_with_prefix("progress/bar/");
    let moving: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.id.ends_with("/indeterminate"))
        .collect();

    assert_eq!(moving.len(), 3);
    for cell in moving {
        assert_eq!(text(cell, "mode"), Some("indeterminate"), "{}", cell.id);
        assert_eq!(text(cell, "motion"), Some("running"), "{}", cell.id);
        assert_eq!(
            number(cell, "motion.phase"),
            Some(SAMPLE_PHASE),
            "{} must record the phase it was sampled at",
            cell.id
        );
        assert!(
            value(cell, "fraction").is_none(),
            "{} must not answer a question the model refuses",
            cell.id
        );
        assert_eq!(number(cell, "motion.segment.width-fraction"), Some(0.35));
    }
}

#[test]
fn the_reduced_motion_cells_are_striped_and_frozen() {
    let cells = cells_with_prefix("progress/bar/");
    let still: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.id.ends_with("/reduced-static"))
        .collect();

    assert_eq!(still.len(), 3);
    for cell in still {
        assert_eq!(text(cell, "motion"), Some("reduced-static"), "{}", cell.id);
        assert_eq!(text(cell, "motion.striped"), Some("true"), "{}", cell.id);
        assert_eq!(
            number(cell, "motion.segment.offset-fraction"),
            Some(0.0),
            "{} must not be caught mid-slide",
            cell.id
        );
        assert!(
            number(cell, "motion.stripe-period").is_some(),
            "{} must record the stripe period it repeats at",
            cell.id
        );
    }
}

#[test]
fn the_determinate_cells_carry_a_fraction_and_no_motion_record() {
    let cells = cells_with_prefix("progress/bar/");
    let determinate: Vec<&Cell> = cells
        .iter()
        .filter(|cell| text(cell, "mode") == Some("determinate"))
        .collect();

    assert_eq!(determinate.len(), 9);
    for cell in determinate {
        let fraction = number(cell, "fraction").unwrap_or_default();
        assert!((0.0..=1.0).contains(&fraction), "{}", cell.id);
        assert!(
            value(cell, "motion.phase").is_none(),
            "{} is not moving through a phase",
            cell.id
        );
    }
}

#[test]
fn the_determinate_fractions_are_the_three_sampled_positions() {
    let mut fractions: Vec<f32> = cells_with_prefix("progress/bar/")
        .iter()
        .filter(|cell| text(cell, "mode") == Some("determinate"))
        .filter_map(|cell| number(cell, "fraction"))
        .collect();
    fractions.sort_by(|a, b| a.total_cmp(b));
    fractions.dedup();

    assert_eq!(fractions, vec![0.25, 0.5, 1.0]);
}

#[test]
fn the_spinner_cells_carry_both_rungs_and_both_mounts() {
    let spinners = cells_with_prefix("progress/spinner/");
    let diameters: Vec<f32> = spinners
        .iter()
        .filter_map(|cell| number(cell, "diameter"))
        .collect();

    assert_eq!(spinners.len(), 4);
    assert!(diameters.contains(&14.0), "no small spinner cell");
    assert!(diameters.contains(&20.0), "no medium spinner cell");

    for cell in &spinners {
        assert_eq!(number(cell, "stroke"), Some(2.0), "{}", cell.id);
        assert!(
            cell.audit.iter().any(|pair| pair.channel == "spinner.arc"),
            "{} must audit its arc",
            cell.id
        );
    }

    let solid = spinners
        .iter()
        .find(|cell| cell.id.ends_with("/solid"))
        .expect("a solid-mount spinner cell");
    assert_eq!(text(solid, "mount"), Some("solid-fill"));
}

#[test]
fn the_stop_button_cells_record_the_colour_gap_they_filled() {
    let stops = cells_with_prefix("progress/stop/");
    assert_eq!(stops.len(), 3);

    for cell in &stops {
        assert_eq!(number(cell, "stop.size"), Some(24.0), "{}", cell.id);
        assert_eq!(text(cell, "cancellable"), Some("true"), "{}", cell.id);
        let note = cell.note.clone().unwrap_or_default();
        assert!(
            note.contains("no colour"),
            "{} must record that §3.9 left the stop button's colour open",
            cell.id
        );
    }
}

#[test]
fn two_progress_cell_calls_are_byte_identical() {
    let a: Vec<String> = progress_cells().iter().map(Cell::snapshot).collect();
    let b: Vec<String> = progress_cells().iter().map(Cell::snapshot).collect();
    assert_eq!(a, b);
}
