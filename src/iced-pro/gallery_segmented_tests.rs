//! Tests for the segmented-strip gallery cells — `UIF-ATOM-001b`, §3.1b.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Forty cells, unique ids, the fixed
//!    kind/subject/spec triple, and the nesting order they are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What each shape records.** The selected/unselected paint, the
//!    reserved check slot, the control-level ring, and what is
//!    deliberately left unaudited.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};
use crate::widget::button::{self, ButtonVariant};

/// The expected cell count: 2 subjects x 2 polarities x 2 heights x 5
/// state columns.
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

/// The colour a resolved channel carries, if it is a colour entry.
fn colour(cell: &Cell, channel: &str) -> Option<Color> {
    value(cell, channel).and_then(StyleValue::colour)
}

/// Does this cell declare an audit pair on `channel`?
fn has_channel(cell: &Cell, channel: &str) -> bool {
    cell.audit.iter().any(|pair| pair.channel == channel)
}

/// Is this cell sampling a selected segment?
fn is_on(cell: &Cell) -> bool {
    text(cell, "polarity") == Some("on")
}

/// The disabled column.
const DISABLED: VisualState = VisualState {
    base: BaseState::Disabled,
    focused: false,
};

/// Find one cell by its `(subject, polarity, height, state)` quadruple.
fn find(
    cells: &[Cell],
    subject: StripSubject,
    polarity: SegmentPolarity,
    height: f32,
    state: VisualState,
) -> Cell {
    let id = format!(
        "{}/{}/{}-{}",
        subject.slug(),
        polarity_slug(polarity),
        height_slug(height),
        state_slug(state)
    );
    cells
        .iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no cell with id {id}"))
        .clone()
}

#[test]
fn the_segmented_block_is_forty_cells() {
    assert_eq!(segmented_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_cell_answers_to_the_segmented_requirement() {
    for cell in segmented_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-001b", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
        let expected = match text(&cell, "subject") {
            Some("toggle") => "toggle-button",
            Some("strip") => "segmented-button",
            other => panic!("{}: unknown subject {other:?}", cell.id),
        };
        assert_eq!(cell.subject, expected, "{}", cell.id);
    }
}

#[test]
fn the_cell_ids_are_unique() {
    let cells = segmented_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate segmented cell ids: {ids:?}");
}

#[test]
fn each_subject_prints_its_off_block_then_its_on_block() {
    let cells = segmented_cells();
    let mut runs: Vec<&str> = Vec::new();
    for cell in &cells {
        let polarity = text(cell, "polarity").unwrap_or_default();
        if runs.last() != Some(&polarity) {
            runs.push(polarity);
        }
    }
    assert_eq!(
        runs,
        vec!["off", "on", "off", "on"],
        "toggle off, toggle on, strip off, strip on"
    );
}

#[test]
fn no_colour_is_unmapped() {
    let unmapped = unmapped_colours(&segmented_cells());
    assert!(
        unmapped.is_empty(),
        "unmapped segmented colours: {unmapped:?}"
    );
}

#[test]
fn every_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = segmented_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(failing.is_empty(), "pairs failing the gate: {failing:?}");
}

#[test]
fn every_cell_audits_its_segment_border_and_label() {
    for cell in segmented_cells() {
        assert!(
            has_channel(&cell, "segment.border"),
            "{}: no segment.border audit",
            cell.id
        );
        assert!(
            has_channel(&cell, "segment.text"),
            "{}: no segment.text audit",
            cell.id
        );
    }
}

#[test]
fn a_selected_segment_paints_the_secondary_pressed_cell() {
    let palette = Palette::dark();
    let expected = button::resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Pressed,
            focused: false,
        },
        &palette,
    );
    for cell in segmented_cells()
        .iter()
        .filter(|cell| is_on(cell) && !cell.id.contains("disabled"))
    {
        assert_eq!(
            colour(cell, "fill"),
            Some(expected.background),
            "{}: a selected segment must be the Secondary Pressed fill",
            cell.id
        );
        assert_eq!(
            colour(cell, "text"),
            Some(expected.foreground),
            "{}: a selected label must come from the same cell",
            cell.id
        );
    }
}

#[test]
fn an_unselected_segment_passes_the_widget_style_straight_through() {
    let palette = Palette::dark();
    let cells = segmented_cells();
    for subject in [StripSubject::Toggle, StripSubject::Strip] {
        for height in HEIGHTS {
            for state in COLUMNS {
                let cell = find(&cells, subject, SegmentPolarity::Unselected, height, state);
                let expected = button::resolve_style(ButtonVariant::Secondary, state, &palette);
                assert_eq!(
                    colour(&cell, "fill"),
                    Some(expected.background),
                    "{}: an unselected segment must be the widget's own cell",
                    cell.id
                );
                assert_eq!(
                    colour(&cell, "text"),
                    Some(expected.foreground),
                    "{}: an unselected label must be the widget's own cell",
                    cell.id
                );
            }
        }
    }
}

#[test]
fn a_selected_segment_carries_the_tick_and_an_unselected_one_does_not() {
    for cell in segmented_cells() {
        if is_on(&cell) {
            assert_eq!(
                text(&cell, "check.shown"),
                Some("true"),
                "{}: a selected segment must show the tick",
                cell.id
            );
            assert!(
                has_channel(&cell, "segment.check"),
                "{}: a shown tick must be audited",
                cell.id
            );
        } else {
            assert_eq!(
                text(&cell, "check.shown"),
                Some("false"),
                "{}: an unselected segment must not show a tick",
                cell.id
            );
            assert!(
                !has_channel(&cell, "segment.check"),
                "{}: an unpainted tick must not be audited",
                cell.id
            );
        }
    }
}

#[test]
fn a_disabled_selected_segment_keeps_its_tick_in_disabled_ink() {
    let cells = segmented_cells();
    let palette = Palette::dark();
    for subject in [StripSubject::Toggle, StripSubject::Strip] {
        for height in HEIGHTS {
            let cell = find(&cells, subject, SegmentPolarity::Selected, height, DISABLED);
            assert_eq!(
                colour(&cell, "check.color"),
                Some(palette.text_disabled),
                "{}: the tick must be drawn, in the disabled ink",
                cell.id
            );
            assert_eq!(
                text(&cell, "check.shown"),
                Some("true"),
                "{}: a disabled selected segment must not lose its tick",
                cell.id
            );
        }
    }
}

#[test]
fn every_segment_charges_the_check_slot() {
    for cell in segmented_cells() {
        assert_eq!(
            number(&cell, "segment.check-slot"),
            Some(segmented::CHECK_SLOT_WIDTH),
            "{}: the check slot must be the reserved width",
            cell.id
        );
    }
}

#[test]
fn selection_does_not_resize_the_segment() {
    let cells = segmented_cells();
    for subject in [StripSubject::Toggle, StripSubject::Strip] {
        for height in HEIGHTS {
            for state in COLUMNS {
                let off = find(&cells, subject, SegmentPolarity::Unselected, height, state);
                let on = find(&cells, subject, SegmentPolarity::Selected, height, state);
                assert_eq!(
                    number(&off, "segment.width"),
                    number(&on, "segment.width"),
                    "selection must not resize the segment"
                );
                assert_eq!(
                    number(&off, "strip.width"),
                    number(&on, "strip.width"),
                    "selection must not resize the strip"
                );
            }
        }
    }
}

#[test]
fn the_strip_geometry_is_the_spec_rung_and_perimeter() {
    for cell in segmented_cells() {
        assert_eq!(
            number(&cell, "strip.radius"),
            Some(segmented::RADIUS),
            "{}: 4px outer radius",
            cell.id
        );
        assert_eq!(
            number(&cell, "strip.perimeter-width"),
            Some(segmented::PERIMETER_WIDTH),
            "{}: 1px border.strong perimeter",
            cell.id
        );
    }
}

#[test]
fn the_focus_ring_is_present_only_on_a_focused_column() {
    for cell in segmented_cells() {
        let width = number(&cell, "strip.focus-ring.width").unwrap_or(0.0);
        if cell.id.contains("+focused") {
            assert!(
                width > 0.0,
                "{}: a focused strip must carry a ring",
                cell.id
            );
            assert!(
                has_channel(&cell, "strip.focus-ring"),
                "{}: a painted ring must be audited",
                cell.id
            );
        } else {
            assert_eq!(
                width, 0.0,
                "{}: an unfocused strip must carry no ring",
                cell.id
            );
            assert!(
                !has_channel(&cell, "strip.focus-ring"),
                "{}: an unpainted ring must not be audited",
                cell.id
            );
        }
    }
}

#[test]
fn the_focus_ring_is_audited_against_the_mount_not_the_segment() {
    let palette = Palette::dark();
    for cell in segmented_cells() {
        let Some(pair) = cell
            .audit
            .iter()
            .find(|pair| pair.channel == "strip.focus-ring")
        else {
            continue;
        };
        assert_eq!(
            pair.background, palette.surface_raised,
            "{}: the ring sits on the mount, not on the segment",
            cell.id
        );
    }
}

#[test]
fn the_segment_fill_is_never_audited_against_the_mount() {
    for cell in segmented_cells() {
        assert!(
            !has_channel(&cell, "fill"),
            "{}: no surface rung clears 3.0 against the mount, so the fill \
             is not indicator material and must not be judged",
            cell.id
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a = segmented_cells();
    let b = segmented_cells();
    assert_eq!(
        a.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>(),
        b.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>()
    );
}
