//! Tests for [`super::gallery_table`] - that the grid cells exist, that
//! they carry the channels the audit needs, and that the gallery's own
//! gates stay green over them (`UIF-COMP-007`, decision **D-8**).

use crate::composite::table_cell::{self, CellBand};
use crate::composite::table_header::{self, SortDirection};
use crate::gallery::{Cell, CellKind, ContrastRole, StyleValue};
use crate::gallery_audit::{contrast_failures, unmapped_colours};
use crate::gallery_table::{
    cell_band_cell, cell_band_cells, header_posture_cell, header_posture_cells, layout_at,
    table_cells, table_width, width_regime_cell, width_regime_cells,
};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;

fn value<'a>(cell: &'a Cell, name: &str) -> &'a StyleValue {
    let entry = cell
        .resolved
        .iter()
        .find(|item| item.channel == name)
        .unwrap_or_else(|| panic!("{} has no channel {name}", cell.id));
    &entry.value
}

fn number(cell: &Cell, name: &str) -> f32 {
    match value(cell, name) {
        StyleValue::Number(v) => *v,
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn text(cell: &Cell, name: &str) -> String {
    match value(cell, name) {
        StyleValue::Text(v) => v.clone(),
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn token(cell: &Cell, name: &str) -> String {
    match value(cell, name) {
        StyleValue::Colour { token, .. } => token.clone(),
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn cell_for<'a>(cells: &'a [Cell], id: &str) -> &'a Cell {
    cells
        .iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no cell with id {id}"))
}

fn widths(cell: &Cell) -> Vec<f32> {
    text(cell, "widths")
        .split(" / ")
        .map(|part| part.parse::<f32>().expect("a width parses"))
        .collect()
}

// ---------------------------------------------------------------------------
// 1 - the cell set
// ---------------------------------------------------------------------------

#[test]
fn the_grid_subsystem_contributes_cells() {
    let cells = table_cells();
    assert_eq!(
        cells.len(),
        cell_band_cells().len() + width_regime_cells().len() + header_posture_cells().len()
    );
    assert_eq!(cells.len(), 12, "four bands, four regimes, four headers");
}

#[test]
fn every_grid_cell_is_a_composite_tagged_to_the_table_requirement() {
    for cell in table_cells() {
        assert_eq!(cell.kind, CellKind::Composite, "{}", cell.id);
        assert_eq!(cell.spec, "UIF-COMP-007", "{}", cell.id);
    }
}

#[test]
fn cell_ids_are_unique() {
    let cells = table_cells();
    let mut seen: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    seen.sort_unstable();
    let unique = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), unique, "duplicate cell id in {seen:?}");
}

#[test]
fn every_cell_paints_a_frame() {
    for cell in table_cells() {
        assert!(!cell.frame.quads.is_empty(), "{} paints nothing", cell.id);
    }
}

// ---------------------------------------------------------------------------
// 2 - the band cells
// ---------------------------------------------------------------------------

#[test]
fn all_four_bands_are_rendered() {
    let cells = cell_band_cells();
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let cell = cell_for(&cells, &format!("table-cell/{}", band.slug()));
        assert_eq!(text(cell, "audit.band"), band.slug());
    }
}

#[test]
fn every_band_clears_the_grid_line_floor() {
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let cell = cell_band_cell(band);
        assert!(
            number(&cell, "grid_line.contrast")
                >= table_cell::MIN_GRID_LINE_CONTRAST_ON_BAND - 1e-4,
            "the grid line on {} measures {} against the floor {}",
            cell.id,
            number(&cell, "grid_line.contrast"),
            table_cell::MIN_GRID_LINE_CONTRAST_ON_BAND
        );
    }
}

#[test]
fn the_resting_bands_keep_the_subtle_line() {
    assert_eq!(
        token(&cell_band_cell(CellBand::Base), "grid_line"),
        "border.subtle"
    );
    assert_eq!(
        token(&cell_band_cell(CellBand::Raised), "grid_line"),
        "border.subtle"
    );
}

#[test]
fn the_tinted_bands_promote_the_line_to_strong() {
    assert_eq!(
        token(&cell_band_cell(CellBand::Hover), "grid_line"),
        "border.strong"
    );
    assert_eq!(
        token(&cell_band_cell(CellBand::Selected), "grid_line"),
        "border.strong"
    );
}

#[test]
fn the_promoted_ratios_are_measured_not_assumed() {
    let palette = Palette::dark();
    // Recomputed from raw sRGB here rather than copied from the spec's
    // printed column.
    assert!(
        (contrast_ratio(palette.border_strong, palette.surface_hover) - 3.6772).abs() < 1e-3,
        "border.strong on surface.hover"
    );
    assert!(
        (contrast_ratio(palette.border_strong, palette.surface_active) - 3.2377).abs() < 1e-3,
        "border.strong on surface.active"
    );
    assert_eq!(
        number(&cell_band_cell(CellBand::Hover), "grid_line.contrast"),
        contrast_ratio(palette.border_strong, palette.surface_hover)
    );
    assert_eq!(
        number(&cell_band_cell(CellBand::Selected), "grid_line.contrast"),
        contrast_ratio(palette.border_strong, palette.surface_active)
    );
}

#[test]
fn the_subtle_line_would_fail_the_tinted_bands_which_is_why_it_is_promoted() {
    let palette = Palette::dark();
    let floor = table_cell::MIN_GRID_LINE_CONTRAST_ON_BAND;
    for surface in [palette.surface_hover, palette.surface_active] {
        assert!(
            contrast_ratio(palette.border_subtle, surface) < floor,
            "border.subtle measures {} here and should have been promoted",
            contrast_ratio(palette.border_subtle, surface)
        );
    }
    for surface in [palette.surface_base, palette.surface_raised] {
        assert!(contrast_ratio(palette.border_subtle, surface) >= floor);
    }
}

#[test]
fn only_the_selected_band_carries_a_selection_outline() {
    let selected = cell_band_cell(CellBand::Selected);
    assert!(
        selected
            .resolved
            .iter()
            .any(|item| item.channel == "selection_outline")
    );
    assert!(
        selected
            .audit
            .iter()
            .any(|pair| pair.channel == "selection_outline")
    );

    for band in [CellBand::Base, CellBand::Raised, CellBand::Hover] {
        let cell = cell_band_cell(band);
        assert!(
            !cell
                .resolved
                .iter()
                .any(|item| item.channel == "selection_outline"),
            "{} reports an outline it does not paint",
            cell.id
        );
        assert!(
            cell.audit
                .iter()
                .all(|pair| pair.channel != "selection_outline")
        );
    }
}

#[test]
fn the_grid_line_is_reported_rather_than_audited() {
    // The hairline separator is not rule-2 chrome; see the module docs.
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let cell = cell_band_cell(band);
        assert!(
            cell.audit.iter().all(|pair| pair.channel != "grid_line"),
            "{} audits the grid line",
            cell.id
        );
        assert!(has_number(&cell, "grid_line.contrast"));
    }
}

fn has_number(cell: &Cell, name: &str) -> bool {
    cell.resolved
        .iter()
        .any(|item| item.channel == name && matches!(item.value, StyleValue::Number(_)))
}

#[test]
fn every_band_cell_audits_its_text_and_its_focus_ring() {
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let cell = cell_band_cell(band);
        let roles: Vec<&str> = cell.audit.iter().map(|pair| pair.channel).collect();
        assert!(roles.contains(&"text"), "{:?} for {}", roles, cell.id);
        assert!(roles.contains(&"focus_ring"), "{:?} for {}", roles, cell.id);
        for pair in &cell.audit {
            assert_eq!(pair.background, band.colour(&Palette::dark()));
        }
    }
}

// ---------------------------------------------------------------------------
// 3 - the width regimes
// ---------------------------------------------------------------------------

#[test]
fn all_four_regimes_are_rendered() {
    let cells = width_regime_cells();
    let labels: Vec<String> = cells
        .iter()
        .map(|cell| text(cell, "audit.regime"))
        .collect();
    assert_eq!(
        labels,
        vec!["surplus", "at-target", "deficit", "oversubscribed"]
    );
}

#[test]
fn the_surplus_regime_gives_the_elastic_column_all_it_can_take() {
    let cell = width_regime_cell("surplus", 480.0);
    assert_eq!(widths(&cell), vec![280.0, 120.0, 80.0]);
    assert_eq!(number(&cell, "total"), 480.0);
    assert_eq!(text(&cell, "scroll"), "none");
}

#[test]
fn the_at_target_regime_lands_exactly_on_the_targets() {
    let cell = width_regime_cell("at-target", 240.0);
    assert_eq!(widths(&cell), vec![40.0, 120.0, 80.0]);
    assert_eq!(number(&cell, "sum.floors"), 180.0);
    assert_eq!(text(&cell, "scroll"), "none");
}

#[test]
fn the_deficit_regime_scales_only_what_has_headroom() {
    let cell = width_regime_cell("deficit", 210.0);
    // The fixed column cannot give anything back and the proportional column has
    // no headroom above its floor, so the whole deficit comes out of the
    // content column.
    assert_eq!(widths(&cell), vec![40.0, 90.0, 80.0]);
    assert_eq!(number(&cell, "total"), 210.0);
    assert_eq!(text(&cell, "scroll"), "none");
}

#[test]
fn the_oversubscribed_regime_holds_every_floor_and_asks_for_a_scrollbar() {
    let cell = width_regime_cell("oversubscribed", 120.0);
    assert_eq!(widths(&cell), vec![40.0, 60.0, 80.0]);
    assert_eq!(number(&cell, "available"), 120.0);
    assert_eq!(number(&cell, "sum.floors"), 180.0);
    assert_eq!(text(&cell, "scroll"), "horizontal");
    assert!(
        number(&cell, "total") > number(&cell, "available"),
        "the resolver must overflow rather than violate a minimum"
    );
}

#[test]
fn no_regime_violates_a_floor() {
    for available in [1.0, 60.0, 120.0, 180.0, 210.0, 240.0, 480.0, 4000.0] {
        let layout = layout_at(available);
        for index in 0..3 {
            assert!(
                layout.width_at(index) >= floor_at(index) - 1e-3,
                "column {index} went below its floor at width {available}: {:?}",
                layout.widths()
            );
        }
    }
}

fn floor_at(index: usize) -> f32 {
    match index {
        0 => 40.0,
        1 => 60.0,
        _ => 80.0,
    }
}

#[test]
fn the_resolved_widths_always_sum_to_at_least_the_available_width() {
    for available in [1.0, 120.0, 210.0, 480.0] {
        let layout = layout_at(available);
        assert!(
            layout.total_width() >= available - 1e-3,
            "at {available} the table only filled to {}",
            layout.total_width()
        );
    }
}

// ---------------------------------------------------------------------------
// 4 - the header postures
// ---------------------------------------------------------------------------

#[test]
fn all_four_header_postures_are_rendered() {
    let cells = header_posture_cells();
    assert_eq!(cells.len(), 4);
    for cell in &cells {
        assert_eq!(cell.subject, "table-header");
    }
}

#[test]
fn no_sort_target_overlaps_its_resize_handle() {
    let cells = header_posture_cells();
    for cell in &cells {
        if text(cell, "resize.handle") == "present" {
            assert!(
                number(cell, "resize.gap") <= 0.0,
                "{}: a positive gap means the target reaches into the handle band",
                cell.id
            );
        }
    }
}

#[test]
fn the_last_column_has_no_handle_of_its_own() {
    let cells = header_posture_cells();
    let cell = cell_for(&cells, "table-header/last-column");
    assert_eq!(text(cell, "resize.handle"), "none");
    // It still loses its leading pixels: they belong to column 1's handle.
    assert!(
        (number(cell, "sort_target.width")
            - (number(cell, "cell.width") - table_header::RESIZE_REACH))
            .abs()
            < 1e-3,
        "the last column keeps everything except its neighbour's grab band"
    );
}

#[test]
fn a_column_with_live_handles_on_both_sides_is_inset_twice() {
    let cell = header_posture_cell("both", 1, None);
    let inset = number(&cell, "cell.width") - number(&cell, "sort_target.width");
    assert!(
        (inset - 2.0 * table_header::RESIZE_REACH).abs() < 1e-3,
        "expected two insets of {:?}, got {inset}",
        table_header::RESIZE_REACH
    );
}

#[test]
fn the_header_audits_its_title_and_its_indicator() {
    for cell in header_posture_cells() {
        let channels: Vec<&str> = cell.audit.iter().map(|pair| pair.channel).collect();
        assert!(channels.contains(&"title"), "{channels:?}");
        assert!(channels.contains(&"indicator"), "{channels:?}");
        for pair in &cell.audit {
            assert_eq!(pair.background, table_header::band_colour(&Palette::dark()));
        }
        assert_eq!(
            cell.audit
                .iter()
                .find(|pair| pair.channel == "title")
                .map(|pair| pair.role),
            Some(ContrastRole::Text)
        );
    }
}

#[test]
fn the_indicator_contrast_is_measured_against_the_band_it_paints_on() {
    let palette = Palette::dark();
    let cell = header_posture_cell("x", 0, Some(SortDirection::Descending));
    assert_eq!(
        number(&cell, "indicator.contrast"),
        contrast_ratio(
            table_header::indicator_colour(&palette),
            table_header::band_colour(&palette)
        )
    );
}

// ---------------------------------------------------------------------------
// 5 - the gallery's own gates
// ---------------------------------------------------------------------------

#[test]
fn every_colour_the_grid_emits_maps_to_a_token() {
    let cells = table_cells();
    let unmapped = unmapped_colours(&cells);
    assert!(unmapped.is_empty(), "unmapped: {unmapped:?}");
}

#[test]
fn every_audited_pair_in_the_grid_passes_its_rule() {
    let cells = table_cells();
    let failures = contrast_failures(&cells);
    assert!(failures.is_empty(), "contrast failures: {failures:?}");
}

#[test]
fn the_grid_is_built_on_the_shared_table_width() {
    assert!(table_width() > 0.0);
    let layout = layout_at(table_width());
    assert_eq!(layout.len(), 3);
}
