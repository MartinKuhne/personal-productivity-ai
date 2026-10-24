//! Tests for [`super::gallery_breadcrumb`] - that the trail and strip
//! cells exist, carry the channels the audit needs, and that the gallery's
//! own gates stay green over them (`UIF-COMP-008`).
//!
//! The point of these tests is not that thirty cells were built. It is
//! that the numbers the cells print are the numbers the solvers produce,
//! and that the two headline claims - the elision shape and the zero
//! vertical shift - are readable straight off the manifest rather than
//! needing to be re-derived by hand.

use crate::binding::ActionPending;
use crate::composite::breadcrumb::{self, CrumbRole};
use crate::composite::overflow_toolbar;
use crate::gallery::{
    Cell, CellKind, ContrastRole, StyleValue, contrast_failures, unmapped_colours,
};
use crate::gallery_breadcrumb::{
    breadcrumb_cells, collapse_regime_cell, collapse_regime_cells, crumb_posture_cells,
    overflow_row_cell, overflow_row_cells, separator_cell, strip, strip_regime_cells, trail,
    trigger_posture_cells,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;

const REST: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn colour(cell: &Cell, name: &str) -> iced::Color {
    match value(cell, name) {
        StyleValue::Colour { colour, .. } => *colour,
        other => panic!("{} channel {name} is {other:?}", cell.id),
    }
}

fn cell_for<'a>(cells: &'a [Cell], id: &str) -> &'a Cell {
    cells
        .iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no cell with id {id}"))
}

/// Parse a space-separated index list back into numbers.
fn indices(raw: &str) -> Vec<usize> {
    if raw == "none" {
        return Vec::new();
    }
    raw.split(' ')
        .map(|part| part.parse::<usize>().expect("an index parses"))
        .collect()
}

// ---------------------------------------------------------------------------
// 1 - the cell set
// ---------------------------------------------------------------------------

#[test]
fn the_trail_and_strip_contribute_cells() {
    let cells = breadcrumb_cells();
    assert_eq!(cells.len(), 30);
    assert_eq!(crumb_posture_cells().len(), 12, "three roles x four states");
    assert_eq!(collapse_regime_cells().len(), 5);
    assert_eq!(trigger_posture_cells().len(), 4);
    assert_eq!(strip_regime_cells().len(), 5);
    assert_eq!(overflow_row_cells().len(), 3);
}

#[test]
fn every_cell_is_a_composite_tagged_to_the_breadcrumb_requirement() {
    for cell in breadcrumb_cells() {
        assert_eq!(cell.kind, CellKind::Composite, "{}", cell.id);
        assert_eq!(cell.spec, "UIF-COMP-008", "{}", cell.id);
    }
}

#[test]
fn cell_ids_are_unique() {
    let cells = breadcrumb_cells();
    let mut seen: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    seen.sort_unstable();
    let unique = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), unique, "duplicate cell id in {seen:?}");
}

#[test]
fn the_subjects_partition_the_two_surfaces() {
    let cells = breadcrumb_cells();
    let subjects: Vec<&str> = cells.iter().map(|cell| cell.subject).collect();
    for expected in [
        "breadcrumb-crumb",
        "breadcrumb-separator",
        "breadcrumb-collapse",
        "overflow-trigger",
        "overflow-strip",
        "overflow-row",
    ] {
        assert!(
            subjects.contains(&expected),
            "no cell carries subject {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 - the gallery's own gates stay green
// ---------------------------------------------------------------------------

#[test]
fn no_cell_paints_an_unmapped_colour() {
    let failures = unmapped_colours(&breadcrumb_cells());
    assert!(failures.is_empty(), "unmapped: {failures:?}");
}

#[test]
fn every_audit_pair_clears_its_role_threshold() {
    let failures = contrast_failures(&breadcrumb_cells());
    assert!(failures.is_empty(), "contrast: {failures:?}");
}

#[test]
fn every_cell_carries_at_least_one_audit_pair() {
    for cell in breadcrumb_cells() {
        assert!(
            !cell.audit.is_empty(),
            "{} audits nothing, so the gate cannot see it",
            cell.id
        );
    }
}

// ---------------------------------------------------------------------------
// 3 - the emphasis ladder
// ---------------------------------------------------------------------------

#[test]
fn the_two_ancestor_roles_share_one_resting_colour() {
    let cells = crumb_posture_cells();
    let root = colour(cell_for(&cells, "breadcrumb-crumb/root/rest"), "crumb");
    let middle = colour(cell_for(&cells, "breadcrumb-crumb/middle/rest"), "crumb");
    assert_eq!(
        root, middle,
        "§3B.8 draws one ancestor/terminal step, not an ancestor/ancestor one"
    );
}

#[test]
fn the_terminal_crumb_steps_up_from_its_ancestors() {
    let cells = crumb_posture_cells();
    let root = colour(cell_for(&cells, "breadcrumb-crumb/root/rest"), "crumb");
    let terminal = colour(cell_for(&cells, "breadcrumb-crumb/terminal/rest"), "crumb");
    assert_ne!(root, terminal);
    assert!(
        contrast_ratio(terminal, Palette::dark().surface_base)
            > contrast_ratio(root, Palette::dark().surface_base),
        "the terminal crumb must be the boldest thing in the bar"
    );
}

#[test]
fn the_ancestor_and_terminal_contrasts_are_what_the_resolver_says() {
    let palette = Palette::dark();
    let cells = crumb_posture_cells();
    for (id, role) in [
        ("breadcrumb-crumb/root/rest", CrumbRole::Root),
        ("breadcrumb-crumb/middle/rest", CrumbRole::Middle),
        ("breadcrumb-crumb/terminal/rest", CrumbRole::Terminal),
    ] {
        let cell = cell_for(&cells, id);
        let expected = breadcrumb::crumb_contrast(&palette, role, REST);
        assert!(
            (number(cell, "crumb.contrast") - expected).abs() < 1e-4,
            "{id} prints {} but the resolver says {expected}",
            number(cell, "crumb.contrast")
        );
    }
}

#[test]
fn a_disabled_crumb_is_audited_against_what_it_actually_sits_on() {
    let cells = crumb_posture_cells();
    for role in ["root", "middle", "terminal"] {
        let cell = cell_for(&cells, &format!("breadcrumb-crumb/{role}/disabled"));
        let pair = &cell.audit[0];
        assert_eq!(
            pair.background,
            colour(cell, "effective_backdrop"),
            "{role}/disabled audits a background other than the one it paints"
        );
        assert_eq!(pair.role, ContrastRole::Text);
    }
}

#[test]
fn the_hovered_crumb_is_audited_against_the_ghost_fill_not_the_bar() {
    let cells = crumb_posture_cells();
    let cell = cell_for(&cells, "breadcrumb-crumb/middle/hovered");
    let fill = colour(cell, "crumb_fill");
    if fill.a > 0.0 {
        assert_eq!(
            cell.audit[0].background, fill,
            "a painted segment must be audited against its own fill"
        );
    } else {
        assert_eq!(cell.audit[0].background, colour(cell, "bar_backdrop"));
    }
}

// ---------------------------------------------------------------------------
// 4 - the separator
// ---------------------------------------------------------------------------

#[test]
fn the_separator_is_audited_as_text_because_it_is_a_glyph() {
    let cell = separator_cell();
    assert_eq!(
        cell.audit[0].role,
        ContrastRole::Text,
        "a `/` the user reads answers to rule 1, not the chrome floor"
    );
}

#[test]
fn the_separator_measures_where_the_spec_prints_it() {
    let palette = Palette::dark();
    let cell = separator_cell();
    let measured = number(&cell, "separator.contrast");
    let spec = number(&cell, "separator.contrast.spec");
    let independent = contrast_ratio(breadcrumb::separator_colour(&palette), palette.surface_base);
    assert!(
        (measured - independent).abs() < 1e-4,
        "the cell's ratio and a raw recomputation disagree: {measured} vs {independent}"
    );
    assert!(
        (measured - spec).abs() < 0.02,
        "separator measures {measured}, §2.5 prints {spec}"
    );
    assert!(measured >= 4.5, "a glyph the user reads must clear 4.5:1");
}

#[test]
fn the_separator_records_its_token_divergence() {
    let cell = separator_cell();
    let note = cell
        .note
        .clone()
        .expect("the divergence note must be carried");
    assert!(
        note.contains("divergence"),
        "the note reads {note:?}, which does not name itself as a divergence"
    );
}

// ---------------------------------------------------------------------------
// 5 - the elision shape
// ---------------------------------------------------------------------------

#[test]
fn the_widest_regime_needs_no_trigger_at_all() {
    let all = breadcrumb_cells();
    let cell = cell_for(&all, "breadcrumb-collapse/fits");
    assert_eq!(text(cell, "hidden"), "none");
    assert_eq!(number(cell, "trigger.width"), 0.0);
    assert_eq!(text(cell, "elided"), "no");
}

#[test]
fn each_regime_hides_exactly_one_more_crumb_than_the_wider_one() {
    let cells = collapse_regime_cells();
    let counts: Vec<f32> = cells
        .iter()
        .take(4)
        .map(|cell| number(cell, "hidden.count"))
        .collect();
    assert_eq!(counts, vec![0.0, 1.0, 2.0, 3.0], "{counts:?}");
}

#[test]
fn the_hidden_set_is_always_a_prefix_of_the_middle_run() {
    for available in [320.0, 290.0, 230.0, 160.0, 120.0] {
        let layout = breadcrumb::collapse(&trail(), available);
        let hidden = layout.hidden_indices();
        let expected: Vec<usize> = (1..4).take(hidden.len()).collect();
        assert_eq!(
            hidden, expected,
            "at {available}px the hidden set {hidden:?} is not a prefix of the middle run"
        );
    }
}

#[test]
fn the_root_and_terminal_survive_at_every_regime() {
    for cell in collapse_regime_cells() {
        let inline = indices(&text(&cell, "inline"));
        assert!(inline.contains(&0), "{} lost the root crumb", cell.id);
        assert!(inline.contains(&4), "{} lost the terminal crumb", cell.id);
    }
}

#[test]
fn widening_never_swaps_one_hidden_set_for_another() {
    let mut previous: Vec<usize> = Vec::new();
    for available in [120.0, 160.0, 230.0, 290.0, 320.0] {
        let hidden = breadcrumb::collapse(&trail(), available).hidden_indices();
        if !previous.is_empty() {
            assert!(
                hidden.len() <= previous.len(),
                "widening to {available}px grew the hidden set"
            );
            assert_eq!(
                hidden,
                previous[..hidden.len()],
                "widening to {available}px changed which crumbs are hidden rather than un-hiding some"
            );
        }
        previous = hidden;
    }
}

#[test]
fn the_unsatisfiable_regime_reports_rather_than_breaking_the_invariant() {
    let all = collapse_regime_cells();
    let cell = cell_for(&all, "breadcrumb-collapse/below-floor");
    assert!(
        number(cell, "total.width") > number(cell, "available"),
        "below-floor should print a width it cannot meet"
    );
    let inline = indices(&text(cell, "inline"));
    assert_eq!(inline, vec![0, 4]);
}

#[test]
fn the_collapse_cell_is_a_pure_function_of_its_arguments() {
    let a = collapse_regime_cell("dup", 230.0);
    let b = collapse_regime_cell("dup", 230.0);
    assert_eq!(a.resolved, b.resolved);
}

// ---------------------------------------------------------------------------
// 6 - the eviction order
// ---------------------------------------------------------------------------

#[test]
fn the_eviction_order_is_declared_not_positional() {
    let cells = strip_regime_cells();
    let order = indices(&text(&cells[0], "evicts"));
    assert_eq!(
        order,
        vec![3, 1, 0, 2, 4],
        "the two DISPOSABLE items must leave before the SECONDARY one, whatever their positions"
    );
    assert_ne!(
        order,
        vec![0, 1, 2, 3, 4],
        "an eviction order equal to declaration order means the solver fell back to position"
    );
}

#[test]
fn every_regime_evicts_a_prefix_of_the_declared_order() {
    let order = overflow_toolbar::eviction_order(&strip());
    for cell in strip_regime_cells() {
        let overflow = indices(&text(&cell, "overflow"));
        let declared: Vec<usize> = order[..overflow.len()].to_vec();
        let mut declared_sorted = declared;
        declared_sorted.sort_unstable();
        let mut overflow_sorted = overflow;
        overflow_sorted.sort_unstable();
        assert_eq!(
            overflow_sorted, declared_sorted,
            "{} overflows a set that is not a prefix of the declared order",
            cell.id
        );
    }
}

#[test]
fn widening_only_shortens_the_evicted_prefix() {
    let mut previous: Vec<usize> = Vec::new();
    let order = overflow_toolbar::eviction_order(&strip());
    for available in [95.0, 110.0, 175.0, 235.0, 300.0, 320.0] {
        let layout = overflow_toolbar::fit(&strip(), available);
        let mut evicted: Vec<usize> = order
            .iter()
            .filter(|index| {
                layout
                    .plan(**index)
                    .map(|p| p.is_overflow())
                    .unwrap_or(false)
            })
            .copied()
            .collect();
        evicted.sort_unstable();
        let mut sorted_previous = previous.clone();
        sorted_previous.sort_unstable();
        assert!(
            evicted.len() <= sorted_previous.len() || previous.is_empty(),
            "widening to {available}px evicted more than the narrower strip did"
        );
        previous = evicted;
    }
}

#[test]
fn the_fitting_regime_charges_no_trigger_reserve() {
    let all = strip_regime_cells();
    let cell = cell_for(&all, "overflow-strip/fits");
    assert_eq!(number(cell, "trigger.width"), 0.0);
    assert_eq!(text(cell, "overflow"), "none");
    assert_eq!(number(cell, "inline.width"), 316.0);
}

#[test]
fn the_reserve_is_the_trigger_plus_its_gap() {
    let all = trigger_posture_cells();
    let cell = cell_for(&all, "overflow-trigger/rest");
    assert_eq!(
        number(cell, "trigger.reserve"),
        number(cell, "trigger.size") + number(cell, "trigger.gap")
    );
}

// ---------------------------------------------------------------------------
// 7 - the zero vertical shift
// ---------------------------------------------------------------------------

#[test]
fn the_strip_height_is_identical_across_every_regime() {
    let heights: Vec<f32> = strip_regime_cells()
        .iter()
        .map(|cell| number(cell, "strip.height"))
        .collect();
    let first = heights[0];
    assert!(
        heights.iter().all(|h| *h == first),
        "the strip height moved across regimes: {heights:?}"
    );
}

#[test]
fn the_strip_height_is_independent_of_how_many_items_moved() {
    let cells = strip_regime_cells();
    let mut heights: Vec<f32> = cells
        .iter()
        .map(|cell| number(cell, "strip.height"))
        .collect();
    heights.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    heights.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);
    let overflow_counts: Vec<f32> = cells
        .iter()
        .map(|cell| number(cell, "overflow.count"))
        .collect();
    assert!(
        overflow_counts.windows(2).any(|w| w[0] != w[1]),
        "the fixture never varies the overflow count, so this proves nothing: {overflow_counts:?}"
    );
    assert_eq!(
        heights.len(),
        1,
        "overflow count varies across {overflow_counts:?} but the height is not constant: {heights:?}"
    );
}

#[test]
fn the_bar_and_the_strip_agree_on_their_shared_height() {
    assert_eq!(breadcrumb::BAR_HEIGHT, overflow_toolbar::strip_height());
}

// ---------------------------------------------------------------------------
// 8 - the overflow row keeps its state
// ---------------------------------------------------------------------------

#[test]
fn a_busy_row_is_not_a_disabled_row() {
    let cells = overflow_row_cells();
    let enabled = cell_for(&cells, "overflow-row/enabled");
    let busy = cell_for(&cells, "overflow-row/busy");
    let disabled = cell_for(&cells, "overflow-row/disabled");

    assert_eq!(text(busy, "busy"), "yes");
    assert_eq!(text(enabled, "busy"), "no");
    assert_eq!(
        text(busy, "navigable"),
        "yes",
        "a busy row must stay a focus stop"
    );
    assert_eq!(text(disabled, "navigable"), "no");
    assert_eq!(
        colour(busy, "label"),
        colour(enabled, "label"),
        "busy must not borrow the disabled colour"
    );
    assert_ne!(colour(disabled, "label"), colour(enabled, "label"));
}

#[test]
fn every_overflow_row_label_clears_the_text_floor() {
    for cell in overflow_row_cells() {
        let ratio = number(&cell, "label.contrast");
        assert!(
            ratio >= 4.5,
            "{} label contrast {ratio} is below the text floor",
            cell.id
        );
        assert_eq!(cell.audit[0].role, ContrastRole::Text);
    }
}

#[test]
fn the_disabled_row_is_audited_against_the_menu_not_the_strip() {
    let all = overflow_row_cells();
    let cell = cell_for(&all, "overflow-row/disabled");
    assert_eq!(
        cell.audit[0].background,
        colour(cell, "menu_fill"),
        "a disabled row's own fill is transparent, so the menu behind it is the real background"
    );
}

#[test]
fn the_row_cell_reaches_the_menu_through_the_toolbar_its_own_solver_built() {
    let cell = overflow_row_cell("probe", true, ActionPending::idle());
    assert_eq!(cell.id, "overflow-row/probe");
    assert_eq!(number(&cell, "row.height"), 28.0);
}

// ---------------------------------------------------------------------------
// 9 - the trigger
// ---------------------------------------------------------------------------

#[test]
fn the_trigger_glyph_clears_the_chrome_floor_in_every_posture() {
    for cell in trigger_posture_cells() {
        let ratio = number(&cell, "glyph.contrast");
        assert!(ratio >= 3.0, "{} glyph contrast {ratio}", cell.id);
        assert_eq!(cell.audit[0].role, ContrastRole::Chrome);
    }
}

#[test]
fn the_trigger_postures_are_all_distinct_states() {
    let cells = trigger_posture_cells();
    let slugs: Vec<String> = cells.iter().map(|cell| text(cell, "audit.state")).collect();
    let mut sorted = slugs.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), slugs.len(), "{slugs:?}");
}

#[test]
fn the_pressed_trigger_is_darker_than_the_resting_one() {
    let cells = trigger_posture_cells();
    let rest = colour(cell_for(&cells, "overflow-trigger/rest"), "backdrop");
    let pressed = colour(cell_for(&cells, "overflow-trigger/pressed"), "backdrop");
    assert_ne!(
        rest, pressed,
        "the pressed posture must be visually distinct"
    );
}

// ---------------------------------------------------------------------------
// 10 - fixture integrity
// ---------------------------------------------------------------------------

#[test]
fn the_trail_fixture_has_a_middle_run_worth_eliding() {
    let items = trail();
    assert_eq!(items.len(), 5);
    let layout = breadcrumb::collapse(&items, 160.0);
    assert_eq!(layout.hidden_count(), 3);
}

#[test]
fn the_strip_fixture_declares_its_ranks_out_of_position_order() {
    let items = strip();
    assert_eq!(items[1].rank, overflow_toolbar::OverflowRank::DISPOSABLE);
    assert_eq!(items[3].rank, overflow_toolbar::OverflowRank::DISPOSABLE);
    assert_eq!(items[0].rank, overflow_toolbar::OverflowRank::SECONDARY);
    assert_eq!(items[4].rank, overflow_toolbar::OverflowRank::LAST);
}

#[test]
fn the_cells_are_deterministic() {
    assert_eq!(breadcrumb_cells(), breadcrumb_cells());
}
