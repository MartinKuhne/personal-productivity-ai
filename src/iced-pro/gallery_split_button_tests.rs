//! Tests for the split-button gallery cells — `UIF-ATOM-001c`, §3.1c.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Forty cells, unique ids, the fixed
//!    kind/subject/spec triple, and the nesting order they are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What each shape records.** The two-zone split, the chevron that
//!    only the menu half paints, and the divider that is recorded but
//!    deliberately never judged.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue, judge, unmapped_colours};

/// The expected cell count: 2 variants x 2 zones x 2 heights x 5 states.
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

/// Is this cell sampling the menu (chevron) half?
fn is_menu(cell: &Cell) -> bool {
    text(cell, "zone") == Some("chevron")
}

/// The cell id for one `(variant, zone, height, state)` quadruple.
fn cell_id(variant: ButtonVariant, zone: SplitZone, height: f32, state: VisualState) -> String {
    format!(
        "split-button/{}/{}-{}-{}",
        variant_slug(variant),
        zone_slug(zone),
        height_slug(height),
        state_slug(state)
    )
}

/// Find one cell by its quadruple.
fn find(
    cells: &[Cell],
    variant: ButtonVariant,
    zone: SplitZone,
    height: f32,
    state: VisualState,
) -> Cell {
    let id = cell_id(variant, zone, height, state);
    cells
        .iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no cell with id {id}"))
        .clone()
}

#[test]
fn the_split_button_block_is_forty_cells() {
    assert_eq!(split_button_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_cell_answers_to_the_split_button_requirement() {
    for cell in split_button_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-001c", "{}", cell.id);
        assert_eq!(cell.subject, "split-button", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_cell_ids_are_unique() {
    let cells = split_button_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(
        ids.len(),
        unique,
        "duplicate split-button cell ids: {ids:?}"
    );
}

#[test]
fn every_variant_prints_each_zone_once_per_column() {
    let cells = split_button_cells();
    for variant in VARIANTS {
        for height in HEIGHTS {
            for state in COLUMNS {
                let label = find(&cells, variant, SplitZone::Primary, height, state);
                let menu = find(&cells, variant, SplitZone::Menu, height, state);
                assert_ne!(label.id, menu.id, "the two halves need their own cells");
            }
        }
    }
}

#[test]
fn no_colour_is_unmapped() {
    let unmapped = unmapped_colours(&split_button_cells());
    assert!(
        unmapped.is_empty(),
        "unmapped split-button colours: {unmapped:?}"
    );
}

#[test]
fn every_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = split_button_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(failing.is_empty(), "pairs failing the gate: {failing:?}");
}

#[test]
fn every_cell_audits_its_own_half_against_its_own_fill() {
    for cell in split_button_cells() {
        assert!(
            has_channel(&cell, "half.text"),
            "{}: every half must audit its own ink",
            cell.id
        );
        assert!(
            has_channel(&cell, "half.border"),
            "{}: every half must audit its own perimeter",
            cell.id
        );
    }
}

#[test]
fn only_the_menu_half_paints_and_judges_a_chevron() {
    for cell in split_button_cells() {
        if is_menu(&cell) {
            assert!(
                has_channel(&cell, "chevron"),
                "{}: the menu half must judge the glyph it paints",
                cell.id
            );
        } else {
            assert!(
                !has_channel(&cell, "chevron"),
                "{}: the label half paints no glyph and must not judge one",
                cell.id
            );
        }
    }
}

#[test]
fn the_chevron_never_takes_the_spec_quoted_text_muted() {
    // §3.4 gives the combobox chevron `text.muted`. On a Primary split
    // button's `accent.fill` that token measures 1.84:1 — unusable — so
    // the chevron takes the menu half's own resolved ink instead. The
    // substitution is deliberate and is recorded in the module doc; this
    // pins that it actually happened.
    let palette = Palette::dark();
    let cells = split_button_cells();
    let mut checked = 0usize;
    for cell in cells.iter().filter(|cell| is_menu(cell)) {
        let ink = colour(cell, "chevron.color").expect("every menu half records the chevron ink");
        assert_ne!(
            ink, palette.text_muted,
            "{}: the chevron must not take text.muted",
            cell.id
        );
        checked += 1;
    }
    assert_eq!(checked, 20, "every menu half must be checked");
}

#[test]
fn the_divider_is_recorded_on_every_cell_and_never_judged() {
    for cell in split_button_cells() {
        assert!(
            cell.resolved
                .iter()
                .any(|entry| entry.channel == "divider.color"),
            "{}: the divider colour must be carried",
            cell.id
        );
        assert!(
            number(&cell, "divider.contrast-on-fill").is_some(),
            "{}: the divider's contrast against the fill must be carried",
            cell.id
        );
        assert!(
            !has_channel(&cell, "divider"),
            "{}: the divider must NOT be in the judged set — the spec gives \
             it one token for both variants and border.strong on accent.fill \
             is invisible by construction",
            cell.id
        );
    }
}

#[test]
fn the_divider_on_a_primary_fill_is_recorded_as_the_invisible_figure_it_is() {
    let cells = split_button_cells();
    let cell = find(
        &cells,
        ButtonVariant::Primary,
        SplitZone::Primary,
        HEIGHTS[0],
        COLUMNS[0],
    );
    let ratio = number(&cell, "divider.contrast-on-fill").expect("a divider ratio");
    assert!(
        ratio < 1.2,
        "border.strong on accent.fill must be reported as roughly \
         indistinguishable, got {ratio:.4}"
    );
    let palette = Palette::dark();
    assert_eq!(
        colour(&cell, "divider.color"),
        Some(palette.border_strong),
        "the divider must be the spec's own token"
    );
}

#[test]
fn the_divider_on_a_secondary_fill_is_legible() {
    let cells = split_button_cells();
    let cell = find(
        &cells,
        ButtonVariant::Secondary,
        SplitZone::Primary,
        HEIGHTS[0],
        COLUMNS[0],
    );
    let ratio = number(&cell, "divider.contrast-on-fill").expect("a divider ratio");
    assert!(
        ratio > 3.0,
        "the same stroke reads clearly on the Secondary fill, which is why \
         one token for both variants is a spec gap, got {ratio:.4}"
    );
}

#[test]
fn the_two_halves_tile_the_control() {
    for cell in split_button_cells() {
        let total = number(&cell, "control.width").expect("every cell records the control width");
        let primary = number(&cell, "primary.zone.width").expect("a primary zone width");
        let menu = number(&cell, "menu.zone.width").expect("a menu zone width");
        assert!(
            (total - (primary + menu)).abs() < 1e-3,
            "{}: the two halves must tile the control",
            cell.id
        );
        assert!(
            (menu - split_button::CHEVRON_ZONE_WIDTH).abs() < 1e-3,
            "{}: the menu half is exactly the chevron zone",
            cell.id
        );
    }
}

#[test]
fn the_sampled_zone_matches_the_zone_named_by_the_cell() {
    let cells = split_button_cells();
    for variant in VARIANTS {
        for height in HEIGHTS {
            for state in COLUMNS {
                let label = find(&cells, variant, SplitZone::Primary, height, state);
                let menu = find(&cells, variant, SplitZone::Menu, height, state);
                assert_eq!(
                    number(&label, "sampled.zone.width"),
                    number(&label, "primary.zone.width"),
                    "{}: the label cell samples the primary zone",
                    label.id
                );
                assert_eq!(
                    number(&menu, "sampled.zone.width"),
                    number(&menu, "menu.zone.width"),
                    "{}: the menu cell samples the menu zone",
                    menu.id
                );
            }
        }
    }
}

#[test]
fn the_resting_state_of_the_other_half_is_pinned_by_the_resolver() {
    // Each cell paints the sampled half in the column's state and the
    // other half at Rest, so the hover-independence the host pins in
    // tests is visible in the frame. The label half of a hovered cell must
    // therefore differ from the menu half of a hovered cell.
    let cells = split_button_cells();
    let hovered_label = find(
        &cells,
        ButtonVariant::Secondary,
        SplitZone::Primary,
        HEIGHTS[0],
        COLUMNS[1],
    );
    let resting_label = find(
        &cells,
        ButtonVariant::Secondary,
        SplitZone::Primary,
        HEIGHTS[0],
        COLUMNS[0],
    );
    assert_ne!(
        colour(&hovered_label, "half.fill"),
        colour(&resting_label, "half.fill"),
        "a hovered label half must not look like a resting one"
    );
}

#[test]
fn the_focus_ring_is_control_level_and_only_on_a_focused_column() {
    for cell in split_button_cells() {
        let width = number(&cell, "focus-ring.width").unwrap_or(0.0);
        if cell.id.contains("+focused") {
            assert!(width > 0.0, "{}: a focused control carries a ring", cell.id);
        } else {
            assert_eq!(
                width, 0.0,
                "{}: an unfocused control carries no ring",
                cell.id
            );
        }
    }
}

#[test]
fn every_cell_carries_the_spec_geometry_constants() {
    for cell in split_button_cells() {
        assert_eq!(
            number(&cell, "divider.width"),
            Some(split_button::DIVIDER_WIDTH),
            "{}: 1px divider",
            cell.id
        );
        assert_eq!(
            number(&cell, "primary.padding-x"),
            Some(split_button::PRIMARY_PADDING),
            "{}: the primary zone's horizontal padding",
            cell.id
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a = split_button_cells();
    let b = split_button_cells();
    assert_eq!(
        a.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>(),
        b.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>()
    );
}
