//! Tests for [`super::gallery_tree`] - that the tree cells exist, that
//! they carry the channels the audit needs, and that the gallery's own
//! gates stay green over them (`UIF-COMP-006`, decision **D-8**).

use crate::composite::tree_row::{INDENT_STEP, MIN_INDENT_STEP, MIN_LABEL_WIDTH};
use crate::composite::virtual_list::{COMPACT_ROW_HEIGHT, RowRung, STANDARD_ROW_HEIGHT};
use crate::gallery::{Cell, CellKind, ContrastRole, StyleValue};
use crate::gallery_audit::{contrast_failures, unmapped_colours};
use crate::gallery_tree::{
    Posture, narrow_row, sample_viewport, style_for, tree_cells, tree_row_cell, tree_row_cells,
    virtual_list_cell, virtual_list_cells, wide_row,
};
use crate::state::BaseState;
use crate::token::color::Palette;
use iced::Color;

fn number(cell: &Cell, name: &str) -> f32 {
    let found = cell
        .resolved
        .iter()
        .find(|item| item.channel == name)
        .unwrap_or_else(|| panic!("{} has no channel {name}", cell.id));
    match &found.value {
        StyleValue::Number(value) => *value,
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn token(cell: &Cell, name: &str) -> String {
    let found = cell
        .resolved
        .iter()
        .find(|item| item.channel == name)
        .unwrap_or_else(|| panic!("{} has no channel {name}", cell.id));
    match &found.value {
        StyleValue::Colour { token, .. } => token.clone(),
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn has_channel(cell: &Cell, name: &str) -> bool {
    cell.resolved.iter().any(|item| item.channel == name)
}

fn posture(state: &'static str, depth: usize, width: f32) -> Posture {
    Posture {
        state,
        depth,
        selected: false,
        warning: false,
        disabled: false,
        base: BaseState::Rest,
        width,
        rung: RowRung::Standard,
    }
}

fn selected_posture(state: &'static str, depth: usize, warning: bool) -> Posture {
    Posture {
        state,
        depth,
        selected: true,
        warning,
        disabled: false,
        base: BaseState::Rest,
        width: wide_row(),
        rung: RowRung::Standard,
    }
}

// ---------------------------------------------------------------------------
// 1 - the cell set
// ---------------------------------------------------------------------------

#[test]
fn the_tree_subsystem_contributes_cells() {
    let cells = tree_cells();
    assert_eq!(
        cells.len(),
        tree_row_cells().len() + virtual_list_cells().len()
    );
    assert!(cells.len() >= 16, "got {}", cells.len());
    for cell in &cells {
        assert_eq!(cell.kind, CellKind::Composite);
        assert_eq!(cell.spec, "UIF-COMP-006");
        assert_eq!(cell.namer, crate::gallery::Namer::Standard);
        assert!(cell.note.is_none());
    }
}

#[test]
fn every_cell_id_is_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for cell in tree_cells() {
        assert!(
            seen.insert(cell.id.clone()),
            "duplicate cell id {}",
            cell.id
        );
    }
}

#[test]
fn every_cell_painted_a_frame() {
    for cell in tree_cells() {
        assert!(!cell.frame.quads.is_empty(), "{} painted nothing", cell.id);
    }
}

// ---------------------------------------------------------------------------
// 2 - the gallery's own gates stay green
// ---------------------------------------------------------------------------

#[test]
fn no_tree_cell_uses_an_unmapped_colour() {
    let offenders = unmapped_colours(&tree_cells());
    assert!(offenders.is_empty(), "{offenders:?}");
}

#[test]
fn no_tree_pair_fails_its_contrast_role() {
    let offenders = contrast_failures(&tree_cells());
    assert!(
        offenders.is_empty(),
        "the D-6 promotion is supposed to make this list empty: {offenders:?}"
    );
}

#[test]
fn the_audit_pairs_cover_every_painted_mark() {
    for cell in tree_row_cells() {
        let channels: Vec<&str> = cell.audit.iter().map(|pair| pair.channel).collect();
        assert!(channels.contains(&"label"), "{}: {channels:?}", cell.id);
        assert!(channels.contains(&"chevron"), "{}: {channels:?}", cell.id);
        assert!(channels.contains(&"guide"), "{}: {channels:?}", cell.id);
        let label = cell
            .audit
            .iter()
            .find(|pair| pair.channel == "label")
            .unwrap();
        assert_eq!(label.role, ContrastRole::Text);
        for pair in cell.audit.iter().filter(|p| p.channel != "label") {
            assert_eq!(pair.role, ContrastRole::Chrome, "{}", cell.id);
        }
    }
}

#[test]
fn the_selection_bar_is_audited_only_where_it_is_painted() {
    let cells = tree_row_cells();
    let painted = cells
        .iter()
        .filter(|cell| has_channel(cell, "selection_bar"))
        .count();
    let audited = cells
        .iter()
        .filter(|cell| {
            cell.audit
                .iter()
                .any(|pair| pair.channel == "selection_bar")
        })
        .count();
    assert!(painted > 0, "at least one selected posture exists");
    assert_eq!(audited, painted);
}

// ---------------------------------------------------------------------------
// 3 - the indent policy is visible in the numbers
// ---------------------------------------------------------------------------

#[test]
fn the_label_width_saturates_at_its_floor_and_never_goes_below() {
    // The indent does consume row width - that is what indent *is*. What
    // the policy guarantees is that the consumption stops at the floor:
    // past the collapse point the label stops shrinking, so a 20-deep
    // tree and a 200-deep one render the same label box.
    let shallow = tree_row_cell(&posture("shallow", 1, narrow_row()));
    let deep = tree_row_cell(&posture("deep", 20, narrow_row()));
    let deeper = tree_row_cell(&posture("deeper", 200, narrow_row()));

    let shallow_label = number(&shallow, "geometry.label.width");
    let deep_label = number(&deep, "geometry.label.width");
    let deeper_label = number(&deeper, "geometry.label.width");

    assert!(
        deep_label >= MIN_LABEL_WIDTH - 1e-3,
        "the floor must hold, got {deep_label} against {MIN_LABEL_WIDTH}"
    );
    assert!(
        (deep_label - deeper_label).abs() < 1e-3,
        "the collapsed tier must not erode the label further: {deep_label} vs {deeper_label}"
    );
    assert!(
        shallow_label > deep_label,
        "the shallow row should still have room the deep row lost"
    );
}

#[test]
fn a_deep_narrow_row_reports_its_collapsed_tier() {
    let cell = tree_row_cell(&posture("collapsed", 20, narrow_row()));
    let guides = number(&cell, "geometry.indent.guides");
    assert!(guides > 0.0 && guides < 20.0, "guides drawn: {guides}");
    assert!(
        (number(&cell, "geometry.indent.step") - MIN_INDENT_STEP).abs() < 1e-3,
        "the collapsed tier draws at the floor step, got {}",
        number(&cell, "geometry.indent.step")
    );
}

#[test]
fn a_wide_row_draws_every_guide_at_the_nominal_step() {
    let cell = tree_row_cell(&posture("wide", 4, wide_row()));
    assert_eq!(number(&cell, "geometry.indent.guides"), 4.0);
    assert_eq!(number(&cell, "geometry.indent.step"), INDENT_STEP);
}

// ---------------------------------------------------------------------------
// 4 - the D-6 promotion is visible in the colours
// ---------------------------------------------------------------------------

#[test]
fn the_selection_row_promotes_its_marks_off_the_fill() {
    let selected = tree_row_cell(&selected_posture("sel", 2, true));
    let mut resting = posture("rest", 2, wide_row());
    resting.warning = true;
    let resting = tree_row_cell(&resting);
    assert_eq!(
        token(&selected, "guide"),
        token_of(&Palette::dark().text_strong),
        "border.strong cannot be seen on accent.fill"
    );
    assert_eq!(
        token(&selected, "chevron"),
        token_of(&Palette::dark().text_strong)
    );
    assert_eq!(
        token(&selected, "warning"),
        token_of(&Palette::dark().text_strong)
    );
    assert_eq!(
        token(&resting, "guide"),
        token_of(&Palette::dark().border_strong)
    );
    assert_eq!(
        token(&resting, "chevron"),
        token_of(&Palette::dark().text_muted)
    );
    assert_eq!(
        token(&resting, "warning"),
        token_of(&Palette::dark().status_warning)
    );
}

#[test]
fn the_selected_band_is_the_fill_itself() {
    let palette = Palette::dark();
    let style = style_for(&selected_posture("sel", 2, true), &palette);
    assert_eq!(style.band, palette.accent_fill);
    assert_eq!(style.background, palette.accent_fill);
}

#[test]
fn a_disabled_row_shows_no_warning_and_no_bar() {
    let cell = tree_row_cell(&Posture {
        state: "disabled",
        depth: 2,
        selected: true,
        warning: true,
        disabled: true,
        base: BaseState::Hovered,
        width: wide_row(),
        rung: RowRung::Standard,
    });
    assert!(!has_channel(&cell, "warning"));
    assert!(!has_channel(&cell, "selection_bar"));
    assert_eq!(
        token(&cell, "label"),
        token_of(&Palette::dark().text_disabled)
    );
}

fn token_of(colour: &Color) -> String {
    crate::gallery::Namer::Standard.name(*colour)
}

// ---------------------------------------------------------------------------
// 5 - the recycling bound is visible in the numbers
// ---------------------------------------------------------------------------

#[test]
fn the_realised_bound_does_not_move_with_the_total() {
    let small = virtual_list_cell(100_000, RowRung::Compact, "100k");
    let huge = virtual_list_cell(1_000_000, RowRung::Compact, "1m");
    assert_eq!(
        number(&small, "geometry.realized.bound"),
        number(&huge, "geometry.realized.bound")
    );
    assert_eq!(
        number(&small, "geometry.realized.count"),
        number(&huge, "geometry.realized.count")
    );
    assert_ne!(
        number(&small, "geometry.content.len"),
        number(&huge, "geometry.content.len"),
        "the document sizes differ even though the realised work does not"
    );
}

#[test]
fn the_thumb_shrinks_as_the_document_grows() {
    let small = virtual_list_cell(1_000, RowRung::Compact, "1k");
    let huge = virtual_list_cell(1_000_000, RowRung::Compact, "1m");
    assert!(
        number(&huge, "geometry.thumb.length") <= number(&small, "geometry.thumb.length"),
        "a bigger document must not get a bigger thumb"
    );
}

#[test]
fn the_viewport_and_rung_are_recorded() {
    let compact = virtual_list_cell(500, RowRung::Compact, "c");
    let standard = virtual_list_cell(500, RowRung::Standard, "s");
    assert_eq!(number(&compact, "geometry.viewport"), sample_viewport());
    assert_eq!(number(&compact, "geometry.row.height"), COMPACT_ROW_HEIGHT);
    assert_eq!(
        number(&standard, "geometry.row.height"),
        STANDARD_ROW_HEIGHT
    );
}
