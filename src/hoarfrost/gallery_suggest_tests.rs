//! Tests for the autosuggest gallery cells — `UIF-ATOM-002d`, §3.2.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** The field grid and the eleven tier
//!    cells, unique ids, the fixed subject/spec pair, and the order they
//!    are emitted in.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **The field borrows its chrome.** Every field cell paints exactly
//!    what the `UIF-ATOM-002` `Search` cell paints for the same
//!    `(state, rung)`, which is what keeps this increment from putting a
//!    second definition of a text field into the matrix.
//! 4. **What the tier cells record.** Each tier reports the emissions the
//!    host really produced, including the two failure shapes.
//! 5. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue};
use crate::gallery_audit::{judge, unmapped_colours};

/// The number of tiers the block drives.
const TIERS: [SuggestTier; 11] = [
    SuggestTier::DebounceLive,
    SuggestTier::Request,
    SuggestTier::Highlight,
    SuggestTier::CommitEnter,
    SuggestTier::CommitClick,
    SuggestTier::CommitTab,
    SuggestTier::Dismiss,
    SuggestTier::Clear,
    SuggestTier::NoMatch,
    SuggestTier::StaleAnswer,
    SuggestTier::FocusLoss,
];

/// The expected cell count: states x rungs x open/closed, plus the tiers.
fn expected_cells() -> usize {
    snapshot_states().len() * RUNGS.len() * 2 + TIERS.len()
}

/// The field cells, i.e. everything that is not a tier cell.
fn field_cells() -> Vec<Cell> {
    suggest_cells()
        .into_iter()
        .filter(|cell| cell.subject == "suggest")
        .collect()
}

/// The tier cells.
fn tier_cells() -> Vec<Cell> {
    suggest_cells()
        .into_iter()
        .filter(|cell| cell.subject == "suggest-tier")
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

/// The field cell for one `(state, rung, open)` triple.
fn field(state: VisualState, rung: Rung, open: bool) -> Cell {
    let id = format!(
        "suggest/{}-{}{}",
        state_slug(state),
        if open { "open" } else { "closed" },
        rung.slug()
    );
    suggest_cells()
        .into_iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no field cell with id {id}"))
}

/// The tier cell for one tier.
fn tier(tier: SuggestTier) -> Cell {
    suggest_cells()
        .into_iter()
        .find(|cell| cell.id == format!("suggest-tier/{}", tier.slug()))
        .unwrap_or_else(|| panic!("no tier cell for {:?}", tier))
}

// ---------------------------------------------------------------------------
// 1 — the shape of the block
// ---------------------------------------------------------------------------

#[test]
fn the_block_is_the_field_grid_plus_eleven_tier_cells() {
    let cells = suggest_cells();
    assert_eq!(cells.len(), expected_cells());
    assert_eq!(
        field_cells().len(),
        snapshot_states().len() * RUNGS.len() * 2
    );
    assert_eq!(tier_cells().len(), TIERS.len());
}

#[test]
fn every_cell_id_is_unique() {
    let cells = suggest_cells();
    let mut seen: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "duplicate suggest cell ids: {seen:?}");
}

#[test]
fn every_cell_carries_the_002d_spec_id() {
    assert!(
        suggest_cells()
            .iter()
            .all(|cell| cell.spec == "UIF-ATOM-002d"),
        "every suggest cell must answer to UIF-ATOM-002d"
    );
}

#[test]
fn the_field_cells_come_before_the_tier_cells() {
    let subjects: Vec<&str> = suggest_cells().iter().map(|cell| cell.subject).collect();
    let first_tier = subjects
        .iter()
        .position(|subject| *subject == "suggest-tier")
        .expect("the tier cells must be present");
    assert!(
        subjects[..first_tier]
            .iter()
            .all(|subject| *subject == "suggest"),
        "the grid must be emitted before the tiers"
    );
}

#[test]
fn the_field_grid_samples_every_state_at_both_rungs_open_and_closed() {
    for state in snapshot_states() {
        for rung in RUNGS {
            for open in [false, true] {
                let cell = field(state, rung, open);
                assert_eq!(cell.kind, CellKind::Atom);
                assert_eq!(
                    text(&cell, "flyout"),
                    Some(if open { "open" } else { "closed" })
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
    let unmapped = unmapped_colours(&suggest_cells());
    assert!(
        unmapped.is_empty(),
        "unmapped suggest colours: {unmapped:?}"
    );
}

#[test]
fn every_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = suggest_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(
        failing.is_empty(),
        "failing suggest contrast pairs: {failing:?}"
    );
}

// ---------------------------------------------------------------------------
// 3 — the field borrows its chrome
// ---------------------------------------------------------------------------

#[test]
fn the_field_paints_exactly_what_the_search_input_paints() {
    for state in snapshot_states() {
        for rung in RUNGS {
            let suggest = field(state, rung, false);
            let search = crate::gallery_input::text_input_cell_at(InputKind::Search, state, rung);
            assert_eq!(
                suggest.frame.quads,
                search.frame.quads,
                "the suggest field must not repaint the search input at {}",
                state_slug(state)
            );
        }
    }
}

#[test]
fn opening_the_flyout_changes_no_painted_pixel_of_the_field() {
    for state in snapshot_states() {
        for rung in RUNGS {
            let closed = field(state, rung, false);
            let open = field(state, rung, true);
            assert_eq!(
                closed.frame.quads, open.frame.quads,
                "the flyout is a separate surface; the field must not shift"
            );
        }
    }
}

#[test]
fn the_flyout_geometry_the_cells_record_is_the_models_own() {
    let cell = field(snapshot_states()[0], Rung::Standard, false);
    assert_eq!(
        number(&cell, "flyout.max-height"),
        Some(auto_suggest_model::FLYOUT_MAX_HEIGHT)
    );
    assert_eq!(
        number(&cell, "flyout.row-height"),
        Some(auto_suggest_model::FLYOUT_ROW_HEIGHT)
    );
    assert_eq!(
        number(&cell, "flyout.gap"),
        Some(auto_suggest_model::FLYOUT_GAP)
    );
    assert_eq!(
        number(&cell, "flyout.radius"),
        Some(auto_suggest_model::FLYOUT_RADIUS)
    );
    assert_eq!(number(&cell, "debounce.window"), Some(DEBOUNCE_MS as f32));
}

#[test]
fn the_row_height_is_the_same_rung_the_spec_prints_as_32px() {
    assert_eq!(
        auto_suggest_model::FLYOUT_ROW_HEIGHT,
        Rung::Standard.height()
    );
}

// ---------------------------------------------------------------------------
// 4 — what the tier cells record
// ---------------------------------------------------------------------------

#[test]
fn a_keystroke_inside_the_window_reports_live_and_asks_nothing() {
    let cell = tier(SuggestTier::DebounceLive);
    assert_eq!(text(&cell, "emissions"), Some("QueryChanged"));
    assert_eq!(text(&cell, "tier"), Some("Live"));
    assert_eq!(text(&cell, "flyout"), Some("closed"));
    assert_eq!(text(&cell, "busy"), Some("false"));
    assert_eq!(text(&cell, "query"), Some("do"));
}

#[test]
fn the_window_closing_is_its_own_cell_and_marks_the_control_busy() {
    let cell = tier(SuggestTier::Request);
    assert_eq!(text(&cell, "emissions"), Some("QueryRequested"));
    assert_eq!(text(&cell, "busy"), Some("true"));
    assert_eq!(text(&cell, "flyout"), Some("closed"));
}

#[test]
fn the_highlight_tier_reports_only_the_move() {
    let cell = tier(SuggestTier::Highlight);
    assert_eq!(text(&cell, "emissions"), Some("HighlightChanged"));
    assert_eq!(text(&cell, "flyout"), Some("open"));
}

#[test]
fn each_commit_source_settles_the_candidate_and_closes_the_flyout() {
    for source in [
        SuggestTier::CommitEnter,
        SuggestTier::CommitClick,
        SuggestTier::CommitTab,
    ] {
        let cell = tier(source);
        assert_eq!(
            text(&cell, "emissions"),
            Some("Committed+QueryCommitted+OpenChanged"),
            "{:?} must settle the candidate and close the flyout",
            source
        );
        assert_eq!(text(&cell, "tier"), Some("Commit"));
        assert_eq!(text(&cell, "settles"), Some("true"));
        assert_eq!(text(&cell, "flyout"), Some("closed"));
        assert_eq!(text(&cell, "busy"), Some("false"));
    }
}

#[test]
fn the_commit_settles_the_field_to_the_picked_label() {
    let cell = tier(SuggestTier::CommitClick);
    assert_eq!(text(&cell, "query"), Some("notes.md"));
}

#[test]
fn dismiss_closes_the_flyout_and_keeps_the_text() {
    let cell = tier(SuggestTier::Dismiss);
    assert_eq!(text(&cell, "emissions"), Some("Dismissed+OpenChanged"));
    assert_eq!(text(&cell, "tier"), Some("Dismiss"));
    assert_eq!(text(&cell, "settles"), Some("false"));
    assert_eq!(text(&cell, "query"), Some("do"));
}

#[test]
fn the_second_escape_clears_on_the_commit_tier() {
    let cell = tier(SuggestTier::Clear);
    assert_eq!(text(&cell, "emissions"), Some("QueryCleared"));
    assert_eq!(text(&cell, "tier"), Some("Commit"));
    assert_eq!(text(&cell, "query"), Some(""));
}

#[test]
fn a_query_with_no_answer_opens_nothing_and_emits_nothing() {
    let cell = tier(SuggestTier::NoMatch);
    assert_eq!(text(&cell, "emissions"), Some(""));
    assert_eq!(text(&cell, "flyout"), Some("closed"));
    assert_eq!(text(&cell, "busy"), Some("false"));
    assert_eq!(number(&cell, "stale.dropped"), Some(0.0));
}

#[test]
fn a_stale_answer_is_recorded_as_dropped_rather_than_painted() {
    let cell = tier(SuggestTier::StaleAnswer);
    assert_eq!(text(&cell, "emissions"), Some(""));
    assert_eq!(number(&cell, "stale.dropped"), Some(1.0));
    assert_eq!(
        text(&cell, "busy"),
        Some("true"),
        "the live request is still out"
    );
    assert_eq!(text(&cell, "flyout"), Some("open"));
}

#[test]
fn focus_loss_settles_the_query_and_reports_the_edge() {
    let cell = tier(SuggestTier::FocusLoss);
    assert_eq!(
        text(&cell, "emissions"),
        Some("OpenChanged+QueryCommitted+FocusChanged")
    );
    assert_eq!(text(&cell, "tier"), Some("Commit"));
    assert_eq!(text(&cell, "flyout"), Some("closed"));
    assert_eq!(text(&cell, "busy"), Some("false"));
}

#[test]
fn every_tier_names_a_tier_that_is_a_real_binding_layer() {
    for tier in TIERS {
        assert!(
            ["Live", "Commit", "Dismiss", "none"].contains(&tier.tier_name()),
            "{tier:?} names an unknown tier"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — byte stability
// ---------------------------------------------------------------------------

#[test]
fn two_calls_produce_identical_snapshots() {
    let first: String = suggest_cells().iter().map(Cell::snapshot).collect();
    let second: String = suggest_cells().iter().map(Cell::snapshot).collect();
    assert_eq!(first, second);
}
