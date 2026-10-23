//! Tests for the teaching-popover gallery cells — `UIF-ATOM-007`, §3.7.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Six cells, unique ids, the fixed
//!    kind/subject/spec triple.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What the cells record.** The width clamp's three regimes, the
//!    footer shapes, and the §3.7 contrast figures this increment
//!    recomputed from raw sRGB.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue, judge, unmapped_colours};

/// The expected cell count: 3 width regimes × 2 footer shapes.
const EXPECTED_CELLS: usize = 6;

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

/// The ratio a declared audit channel measured at.
fn measured(cell: &Cell, channel: &str) -> f32 {
    let pair = cell
        .audit
        .iter()
        .find(|pair| pair.channel == channel)
        .unwrap_or_else(|| panic!("{} declares no {channel} pair", cell.id));
    crate::token::contrast::contrast_ratio(pair.foreground, pair.background)
}

#[test]
fn the_popover_block_is_six_cells() {
    assert_eq!(popover_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_popover_cell_answers_to_the_popover_requirement() {
    for cell in popover_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-007", "{}", cell.id);
        assert_eq!(cell.subject, "popover", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Composite, "{}", cell.id);
    }
}

#[test]
fn the_popover_cell_ids_are_unique() {
    let cells = popover_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate popover cell ids: {ids:?}");
}

#[test]
fn no_popover_colour_is_unmapped() {
    let unmapped = unmapped_colours(&popover_cells());
    assert!(
        unmapped.is_empty(),
        "unmapped popover colours: {unmapped:?}"
    );
}

#[test]
fn every_popover_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = popover_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(
        failing.is_empty(),
        "popover pairs failing the gate: {failing:?}"
    );
}

#[test]
fn every_cell_carries_the_three_card_channels() {
    for cell in popover_cells() {
        for channel in [
            "popover.border-on-card",
            "popover.title-on-card",
            "popover.body-on-card",
        ] {
            assert!(
                cell.audit.iter().any(|pair| pair.channel == channel),
                "{}: missing {channel}",
                cell.id
            );
        }
    }
}

#[test]
fn the_width_band_is_enforced_at_both_ends() {
    let cells = popover_cells();
    let under = cells
        .iter()
        .find(|cell| cell.id.contains("under-min"))
        .expect("an under-min regime cell");
    let in_band = cells
        .iter()
        .find(|cell| cell.id.contains("in-band"))
        .expect("an in-band regime cell");
    let over = cells
        .iter()
        .find(|cell| cell.id.contains("over-max"))
        .expect("an over-max regime cell");

    assert_eq!(
        number(under, "card.width.resolved"),
        Some(crate::surface::popover::WIDTH_MIN),
        "a measurement below the floor is raised to it"
    );
    assert_eq!(
        number(in_band, "card.width.resolved"),
        number(in_band, "card.width.measured"),
        "a measurement inside the band passes through untouched"
    );
    assert_eq!(
        number(over, "card.width.resolved"),
        Some(crate::surface::popover::WIDTH_MAX),
        "a measurement above the ceiling is lowered to it"
    );
}

#[test]
fn the_border_channel_reproduces_the_figure_3_7_prints() {
    // §3.7's contrast column prints `accent.border` on `surface.overlay`
    // as 5.94:1. Recomputed from the raw sRGB channels this increment,
    // that is 5.9394:1 — the section rounds, and the measurement agrees.
    for cell in popover_cells() {
        let ratio = measured(&cell, "popover.border-on-card");
        assert!(
            (ratio - 5.9394).abs() < 0.001,
            "{}: border-on-card measures {ratio:.4}, expected 5.9394",
            cell.id
        );
    }
}

#[test]
fn the_text_channels_reproduce_the_recomputed_figures() {
    for cell in popover_cells() {
        let title = measured(&cell, "popover.title-on-card");
        let body = measured(&cell, "popover.body-on-card");
        assert!(
            (title - 17.7168).abs() < 0.001,
            "{}: title-on-card measures {title:.4}, expected 17.7168",
            cell.id
        );
        assert!(
            (body - 11.7180).abs() < 0.001,
            "{}: body-on-card measures {body:.4}, expected 11.7180",
            cell.id
        );
    }
}

#[test]
fn the_dismiss_label_is_recorded_on_every_cell() {
    for cell in popover_cells() {
        assert_eq!(
            text(&cell, "footer.dismiss-label"),
            Some(crate::surface::popover::DISMISS_LABEL),
            "{}",
            cell.id
        );
    }
}

#[test]
fn the_two_footer_shapes_are_both_present() {
    let cells = popover_cells();
    assert_eq!(
        cells
            .iter()
            .filter(|cell| cell.id.contains("with-action"))
            .count(),
        3
    );
    assert_eq!(
        cells
            .iter()
            .filter(|cell| cell.id.contains("dismiss-only"))
            .count(),
        3
    );
}

#[test]
fn the_card_perimeter_is_judged_not_a_resting_divider() {
    for cell in popover_cells() {
        assert_eq!(
            text(&cell, "chrome.class"),
            Some("judged"),
            "{}: an accent.border edge is indicator material",
            cell.id
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a: Vec<String> = popover_cells().iter().map(Cell::snapshot).collect();
    let b: Vec<String> = popover_cells().iter().map(Cell::snapshot).collect();
    assert_eq!(a, b);
}
