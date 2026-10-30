//! Headless render + state-transition tests for the composites pane (S5 + S6).
//!
//! Every section is drawn at a normal and a narrow viewport, the widget count
//! the status bar reports is checked against the builders, and every message
//! is verified to actually move the underlying hoarfrost model.

use std::collections::BTreeSet;

use crate::app::Message;
use crate::data;
use crate::pane_composites::{
    CompositesMessage, CompositesState, code_block_section, command_palette_section,
    composites_pane, diff_viewer_section, empty_state_section, form_field_section,
    grid_view_section, group1_sections, group2_sections, keybind_section, menu_section,
    navigation_rail_section, pagination_section, pips_pager_section, settings_rows_section,
    side_sheet_section, skeleton_section, stepper_section, tab_strip_section,
};
use crate::section::{PaneBody, Section};
use hoarfrost::composite::diff_viewer::DiffDisplayMode;
use hoarfrost::composite::form_field;
use hoarfrost::composite::keybind_recorder::KeyChord;
use hoarfrost::composite::side_sheet::SheetMode;
use hoarfrost::composite::stepper::{StepItem, StepStatus};
use iced::{Element, Size};

/// The right half of the demo window.
const PANE: Size = Size::new(600.0, 900.0);
/// A width narrow enough that a fixed-size label is clamped below the
/// icon-only threshold, which is what makes hoarfrost enforce its tooltip rule.
const NARROW: Size = Size::new(280.0, 700.0);

/// Lays `element` out headless at `viewport`; panics if the frame does.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

/// Apply a message and return the status-bar line it produced.
fn fired(state: &mut CompositesState, message: &CompositesMessage) -> String {
    state.apply(message);
    state.describe(message)
}

/// Every S5 section builder, in display order.
type SectionBuilder = for<'a> fn(&'a CompositesState) -> Section<'a>;

fn group1_builders() -> Vec<SectionBuilder> {
    vec![
        tab_strip_section,
        navigation_rail_section,
        stepper_section,
        pagination_section,
        pips_pager_section,
    ]
}

fn group2_builders() -> Vec<SectionBuilder> {
    vec![
        code_block_section,
        diff_viewer_section,
        empty_state_section,
        grid_view_section,
        keybind_section,
        |_: &CompositesState| skeleton_section(),
        side_sheet_section,
        command_palette_section,
        form_field_section,
        settings_rows_section,
        menu_section,
    ]
}

/// The widget count each S5 section is expected to report, in display order.
const EXPECTED_COUNTS: [usize; 5] = [1, 1, 2, 1, 1];

/// The widget count each S6 section is expected to report, in display order.
///
/// The skeleton is `0` on purpose: a loading placeholder is not interactive,
/// and counting it would inflate the status bar.
const EXPECTED_COUNTS_G2: [usize; 11] = [1, 1, 3, 1, 1, 0, 1, 1, 2, 3, 5];

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Every S5 section renders, headless, at both widths.
#[test]
fn every_group1_section_renders() {
    let state = CompositesState::new();
    for viewport in [PANE, NARROW] {
        for build in group1_builders() {
            laid_out(viewport, build(&state));
        }
    }
}

/// Every S6 section renders, headless, at both widths.
#[test]
fn every_group2_section_renders() {
    let state = CompositesState::new();
    for viewport in [PANE, NARROW] {
        for build in group2_builders() {
            laid_out(viewport, build(&state));
        }
    }
}

/// The S6 sections render after their state has been exercised, which is the
/// path a real session takes: click, rebuild, click again.
#[test]
fn group2_sections_render_after_state_changes() {
    let mut state = CompositesState::new();
    state.apply(&CompositesMessage::CodeCopied("rust".into()));
    state.apply(&CompositesMessage::DiffModeChanged(
        DiffDisplayMode::SideBySide,
    ));
    state.apply(&CompositesMessage::FormValidityToggled(false));
    state.apply(&CompositesMessage::SettingToggled(0, true));
    state.apply(&CompositesMessage::SheetModeToggled(
        SheetMode::ModelessOverlay,
    ));
    state.apply(&CompositesMessage::EmptyAction("empty-retry".into()));

    for build in group2_builders() {
        laid_out(PANE, build(&state));
    }
}

/// The whole pane renders as one column of cards at both widths.
#[test]
fn composites_pane_renders() {
    let state = CompositesState::new();
    for viewport in [PANE, NARROW] {
        laid_out(viewport, composites_pane(&state).sections);
    }
}

/// The builders listed here are the builders the pane actually uses.
#[test]
fn builder_list_matches_the_pane() {
    let state = CompositesState::new();
    assert_eq!(
        group1_builders().len(),
        group1_sections(&state).len(),
        "the pane and the test list must agree on the section inventory"
    );
}

// ---------------------------------------------------------------------------
// Widget accounting
// ---------------------------------------------------------------------------

/// The per-section counts match the fixture table, in order.
#[test]
fn section_counts_match_the_table() {
    let state = CompositesState::new();
    let counts: Vec<usize> = group1_sections(&state)
        .iter()
        .map(|section| section.widgets)
        .collect();
    assert_eq!(EXPECTED_COUNTS.to_vec(), counts);

    let counts: Vec<usize> = group2_sections(&state)
        .iter()
        .map(|section| section.widgets)
        .collect();
    assert_eq!(EXPECTED_COUNTS_G2.to_vec(), counts);
}

/// The status bar's widget count is the sum of both groups' section counts.
#[test]
fn widget_count_matches_sum_of_sections() {
    let state = CompositesState::new();
    let expected: usize =
        EXPECTED_COUNTS.iter().sum::<usize>() + EXPECTED_COUNTS_G2.iter().sum::<usize>();
    assert_eq!(25, expected);
    assert_eq!(expected, state.widget_count());
    assert_eq!(expected, composites_pane(&state).widgets);
}

/// The wizard footer really is a second interactive control, which is why its
/// section reports two widgets instead of one.
#[test]
fn wizard_footer_contributes_two_widgets() {
    let state = CompositesState::new();
    assert_eq!(2, stepper_section(&state).widgets);
}

/// A `PaneBody` totals exactly what its sections claim.
#[test]
fn pane_body_totals_follow_its_sections() {
    let state = CompositesState::new();
    let body = PaneBody::new(group1_sections(&state));
    assert_eq!(6, body.widgets);
}

// ---------------------------------------------------------------------------
// Tab strip (UIF-TAB-003)
// ---------------------------------------------------------------------------

#[test]
fn tab_selection_moves_the_active_tab() {
    let mut state = CompositesState::new();
    assert_eq!(Some(data::TAB_SELECTED.to_string()), state.tabs.active_id);

    let line = fired(
        &mut state,
        &CompositesMessage::TabSelected("tab-todo".into()),
    );
    assert_eq!(Some("tab-todo".to_string()), state.tabs.active_id);
    assert!(line.contains("todo.md"), "{line}");
    assert!(!line.contains("(unsaved)"), "{line}");
}

#[test]
fn selecting_a_dirty_tab_says_so() {
    let mut state = CompositesState::new();
    let line = fired(
        &mut state,
        &CompositesMessage::TabSelected("tab-notes".into()),
    );
    assert!(line.contains("notes.md"), "{line}");
    assert!(line.contains("(unsaved)"), "{line}");
}

#[test]
fn closing_a_dirty_tab_asks_for_confirmation_first() {
    let mut state = CompositesState::new();
    let before = state.tabs.tabs.len();

    let line = fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-notes".into()),
    );
    assert_eq!(Some("tab-notes".to_string()), state.pending_close);
    assert!(line.contains("Confirmation required"), "{line}");
    assert!(line.contains("unsaved"), "{line}");
    assert_eq!(before, state.tabs.tabs.len(), "nothing closes yet");
}

#[test]
fn confirming_a_dirty_tab_closes_it() {
    let mut state = CompositesState::new();
    fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-notes".into()),
    );

    let before = state.tabs.tabs.len();
    let line = fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-notes".into()),
    );
    assert_eq!(None, state.pending_close);
    assert_eq!(before - 1, state.tabs.tabs.len());
    assert!(
        state.tabs.tabs.iter().all(|tab| tab.id != "tab-notes"),
        "the confirmed tab should be gone"
    );
    assert!(line.contains("Closed"), "{line}");
}

#[test]
fn closing_a_clean_tab_never_asks() {
    let mut state = CompositesState::new();
    let before = state.tabs.tabs.len();

    let line = fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-report".into()),
    );
    assert_eq!(None, state.pending_close);
    assert_eq!(before - 1, state.tabs.tabs.len());
    assert!(!line.contains("Confirmation"), "{line}");
}

#[test]
fn closing_a_tab_reassigns_the_active_one() {
    let mut state = CompositesState::new();
    assert_eq!(Some("tab-report".to_string()), state.tabs.active_id);

    fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-report".into()),
    );
    let active = state.tabs.active_id.clone();
    assert_ne!(Some("tab-report".to_string()), active);
    assert!(
        state
            .tabs
            .tabs
            .iter()
            .any(|tab| Some(&tab.id) == active.as_ref()),
        "the new active tab must still be open"
    );
}

#[test]
fn a_pending_confirmation_does_not_leak_across_tabs() {
    let mut state = CompositesState::new();
    fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-notes".into()),
    );
    assert_eq!(Some("tab-notes".to_string()), state.pending_close);

    // A close on a *different* tab must not be treated as confirming the first.
    fired(
        &mut state,
        &CompositesMessage::TabCloseRequested("tab-todo".into()),
    );
    assert_eq!(Some("tab-notes".to_string()), state.pending_close);
    assert!(
        state.tabs.tabs.iter().any(|tab| tab.id == "tab-notes"),
        "the still-pending tab must survive"
    );
}

// ---------------------------------------------------------------------------
// Navigation rail (UIF-COMP-009)
// ---------------------------------------------------------------------------

#[test]
fn rail_selection_updates_the_model() {
    let mut state = CompositesState::new();
    assert_eq!(data::RAIL_SELECTED, state.rail.selected);

    let line = fired(
        &mut state,
        &CompositesMessage::RailSelected("rail-tasks".into()),
    );
    assert_eq!("rail-tasks", state.rail.selected);
    assert!(line.contains("Tasks"), "{line}");
}

#[test]
fn a_footer_destination_resolves_too() {
    let mut state = CompositesState::new();
    let line = fired(
        &mut state,
        &CompositesMessage::RailSelected("rail-help".into()),
    );
    assert_eq!("rail-help", state.rail.selected);
    assert!(line.contains("Help"), "{line}");
}

#[test]
fn an_unknown_rail_id_falls_back_to_the_id() {
    let mut state = CompositesState::new();
    let line = fired(
        &mut state,
        &CompositesMessage::RailSelected("rail-unknown".into()),
    );
    assert!(line.contains("rail-unknown"), "{line}");
}

#[test]
fn rail_expansion_toggles_both_ways() {
    let mut state = CompositesState::new();
    assert!(state.rail.expanded, "the fixture starts expanded");

    let line = fired(&mut state, &CompositesMessage::RailExpanded(false));
    assert!(!state.rail.expanded);
    assert!(line.contains("collapsed"), "{line}");

    let line = fired(&mut state, &CompositesMessage::RailExpanded(true));
    assert!(state.rail.expanded);
    assert!(line.contains("expanded"), "{line}");
}

// ---------------------------------------------------------------------------
// Stepper (UIF-COMP-011)
// ---------------------------------------------------------------------------

#[test]
fn stepper_next_and_previous_move_the_current_step() {
    let mut state = CompositesState::new();
    assert_eq!(Some(1), state.stepper.current_index());

    let line = fired(&mut state, &CompositesMessage::StepNext);
    assert_eq!(Some(2), state.stepper.current_index());
    assert!(line.contains("Layout"), "{line}");
    assert!(line.contains("3 of 4"), "{line}");

    let line = fired(&mut state, &CompositesMessage::StepPrevious);
    assert_eq!(Some(1), state.stepper.current_index());
    assert!(line.contains("Theme"), "{line}");
    assert!(line.contains("2 of 4"), "{line}");
}

#[test]
fn stepper_clamps_at_both_ends() {
    let mut state = CompositesState::new();

    state.stepper.current_step = data::WIZARD_STEPS[0].0.to_string();
    fired(&mut state, &CompositesMessage::StepPrevious);
    assert_eq!(
        Some(0),
        state.stepper.current_index(),
        "cannot go before the first"
    );

    state.stepper.current_step = data::WIZARD_STEPS[3].0.to_string();
    fired(&mut state, &CompositesMessage::StepNext);
    assert_eq!(
        Some(3),
        state.stepper.current_index(),
        "cannot go past the last"
    );
}

#[test]
fn selecting_a_step_jumps_straight_to_it() {
    let mut state = CompositesState::new();
    fired(&mut state, &CompositesMessage::StepSelected(3));
    assert_eq!(Some(3), state.stepper.current_index());
    assert_eq!("step-export", state.stepper.current_step);
}

#[test]
fn selecting_an_out_of_range_step_is_ignored() {
    let mut state = CompositesState::new();
    let before = state.stepper.current_step.clone();
    let line = fired(&mut state, &CompositesMessage::StepSelected(99));
    assert_eq!(before, state.stepper.current_step);
    assert!(line.contains("#99"), "{line}");
}

#[test]
fn step_status_is_derived_from_the_current_step() {
    let model = data::stepper_model();
    assert!(matches!(model.steps[0].status, StepStatus::Completed));
    assert!(matches!(model.steps[1].status, StepStatus::Current));
    assert!(matches!(model.steps[2].status, StepStatus::Pending));
    assert!(matches!(model.steps[3].status, StepStatus::Pending));
}

#[test]
fn a_step_item_reports_its_own_status() {
    let item = StepItem::new("s", "Label", StepStatus::Completed);
    assert_eq!(StepStatus::Completed, item.status);
}

// ---------------------------------------------------------------------------
// The status ladder (UIF-COMP-011)
// ---------------------------------------------------------------------------

/// The four wizard steps, all starting in the same state so a test can say
/// exactly which one it changed.
fn ladder_steps() -> Vec<StepItem<String>> {
    data::WIZARD_STEPS
        .iter()
        .map(|(id, label)| StepItem::new((*id).to_string(), *label, StepStatus::Pending))
        .collect()
}

/// The ladder `statuses_for` produces, as a plain vec for comparison.
fn ladder_at(current: usize, steps: &[StepItem<String>]) -> Vec<StepStatus> {
    super::statuses_for(current, steps)
}

#[test]
fn the_ladder_marks_every_step_by_its_position_relative_to_the_cursor() {
    let steps = ladder_steps();
    assert_eq!(
        ladder_at(2, &steps),
        vec![
            StepStatus::Completed,
            StepStatus::Completed,
            StepStatus::Current,
            StepStatus::Pending
        ]
    );
    assert_eq!(
        ladder_at(0, &steps),
        vec![
            StepStatus::Current,
            StepStatus::Pending,
            StepStatus::Pending,
            StepStatus::Pending
        ]
    );
    assert_eq!(
        ladder_at(3, &steps),
        vec![
            StepStatus::Completed,
            StepStatus::Completed,
            StepStatus::Completed,
            StepStatus::Current
        ]
    );
}

#[test]
fn the_ladder_never_laundered_an_error_step_into_success() {
    let mut steps = ladder_steps();
    steps[0].status = StepStatus::Error;

    // Ahead of the cursor the failure is neither Completed nor Pending —
    // nothing in a navigation step earned the right to call it either.
    assert_eq!(
        ladder_at(2, &steps),
        vec![
            StepStatus::Error,
            StepStatus::Completed,
            StepStatus::Current,
            StepStatus::Pending
        ]
    );
    // Behind the cursor, the same.
    assert_eq!(
        ladder_at(3, &steps)[0],
        StepStatus::Error,
        "moving further from a failed step cleared it"
    );
}

#[test]
fn re_entering_a_failed_step_is_what_clears_its_error() {
    let mut steps = ladder_steps();
    steps[1].status = StepStatus::Error;

    // Under the cursor the error is released: the user is back on the
    // step that owns it and gets to fix it.
    assert_eq!(ladder_at(1, &steps)[1], StepStatus::Current);
    // Anywhere else it stays.
    assert_eq!(ladder_at(0, &steps)[1], StepStatus::Error);
    assert_eq!(ladder_at(3, &steps)[1], StepStatus::Error);
}

#[test]
fn every_error_in_the_ladder_is_sticky_on_its_own_terms() {
    let mut steps = ladder_steps();
    steps[0].status = StepStatus::Error;
    steps[3].status = StepStatus::Error;

    assert_eq!(
        ladder_at(2, &steps),
        vec![
            StepStatus::Error,
            StepStatus::Completed,
            StepStatus::Current,
            StepStatus::Error
        ]
    );
    // Moving onto one of them clears only that one.
    assert_eq!(
        ladder_at(3, &steps),
        vec![
            StepStatus::Error,
            StepStatus::Completed,
            StepStatus::Completed,
            StepStatus::Current
        ]
    );
}

#[test]
fn the_ladder_is_a_pure_function_of_the_steps_it_is_given() {
    let steps = ladder_steps();
    assert_eq!(ladder_at(1, &steps), ladder_at(1, &steps));
    // And it never mutates what it was handed.
    assert!(
        steps.iter().all(|step| step.status == StepStatus::Pending),
        "statuses_for wrote through its input"
    );
}

#[test]
fn landed_step_clamps_to_the_ends_of_the_sequence() {
    let mut state = CompositesState::new();
    state.stepper.current_step = data::WIZARD_STEPS[1].0.to_string();

    assert_eq!(Some(2), super::landed_step(&state.stepper, 1));
    assert_eq!(Some(0), super::landed_step(&state.stepper, -1));
    assert_eq!(
        Some(3),
        super::landed_step(&state.stepper, 99),
        "a long jump clamps to the last step"
    );
    assert_eq!(
        Some(0),
        super::landed_step(&state.stepper, -99),
        "a long retreat clamps to the first step"
    );
    assert_eq!(Some(1), super::landed_step(&state.stepper, 0));
}

#[test]
fn landed_step_reports_none_when_there_is_nowhere_to_land() {
    let empty = hoarfrost::composite::stepper::StepperModel::new(vec![], "gone".to_string(), true);
    assert_eq!(None, super::landed_step(&empty, 1));

    let mut orphan = CompositesState::new();
    orphan.stepper.current_step = "not-a-step".to_string();
    assert_eq!(None, super::landed_step(&orphan.stepper, 1));
}

#[test]
fn walking_the_wizard_forward_rebuilds_the_whole_ladder_each_time() {
    let mut state = CompositesState::new();
    state.stepper.current_step = data::WIZARD_STEPS[0].0.to_string();
    state.stepper.steps[0].status = StepStatus::Current;

    for target in 1..data::WIZARD_STEPS.len() {
        fired(&mut state, &CompositesMessage::StepNext);
        assert_eq!(Some(target), state.stepper.current_index());
        assert_eq!(
            ladder_at(target, &state.stepper.steps),
            state
                .stepper
                .steps
                .iter()
                .map(|step| step.status)
                .collect::<Vec<_>>(),
            "the ladder at step {target} disagrees with the model"
        );
        assert!(
            matches!(state.stepper.steps[target].status, StepStatus::Current),
            "step {target} was entered but never painted Current"
        );
        assert!(
            state.stepper.steps[..target]
                .iter()
                .all(|step| step.status == StepStatus::Completed),
            "a step behind the cursor at {target} is not Completed"
        );
        assert!(
            state.stepper.steps[target + 1..]
                .iter()
                .all(|step| step.status == StepStatus::Pending),
            "a step ahead of the cursor at {target} is not Pending"
        );
    }
}

#[test]
fn walking_off_either_end_leaves_the_ladder_intact() {
    let mut state = CompositesState::new();

    for _ in 0..6 {
        fired(&mut state, &CompositesMessage::StepPrevious);
    }
    assert_eq!(Some(0), state.stepper.current_index());
    assert_eq!(
        ladder_at(0, &state.stepper.steps),
        state
            .stepper
            .steps
            .iter()
            .map(|step| step.status)
            .collect::<Vec<_>>(),
        "clamping at the first step corrupted the ladder"
    );

    for _ in 0..8 {
        fired(&mut state, &CompositesMessage::StepNext);
    }
    let last = data::WIZARD_STEPS.len() - 1;
    assert_eq!(Some(last), state.stepper.current_index());
    assert_eq!(
        ladder_at(last, &state.stepper.steps),
        state
            .stepper
            .steps
            .iter()
            .map(|step| step.status)
            .collect::<Vec<_>>(),
        "clamping at the last step corrupted the ladder"
    );
}

#[test]
fn a_direct_selection_recomputes_the_ladder_too() {
    let mut state = CompositesState::new();
    state.stepper.steps[0].status = StepStatus::Error;

    fired(&mut state, &CompositesMessage::StepSelected(2));
    assert_eq!(
        ladder_at(2, &state.stepper.steps),
        state
            .stepper
            .steps
            .iter()
            .map(|step| step.status)
            .collect::<Vec<_>>(),
        "selecting a step left the ladder behind"
    );
    assert_eq!(state.stepper.steps[0].status, StepStatus::Error);
}

#[test]
fn selecting_an_unknown_step_leaves_the_ladder_untouched() {
    let mut state = CompositesState::new();
    let before = state
        .stepper
        .steps
        .iter()
        .map(|step| step.status)
        .collect::<Vec<_>>();

    fired(&mut state, &CompositesMessage::StepSelected(42));
    let after = state
        .stepper
        .steps
        .iter()
        .map(|step| step.status)
        .collect::<Vec<_>>();
    assert_eq!(before, after);
}

#[test]
fn a_completed_ladder_makes_linear_navigation_selectable() {
    // The functional half of the bug: `can_select` in linear mode only
    // lets a step through if the ladder actually marked it Completed.
    let mut state = CompositesState::new();
    state.stepper.current_step = data::WIZARD_STEPS[0].0.to_string();
    fired(&mut state, &CompositesMessage::StepNext);
    fired(&mut state, &CompositesMessage::StepNext);

    let first = state.stepper.steps[0].id.clone();
    let second = state.stepper.steps[1].id.clone();
    assert!(
        state.stepper.can_select(&first),
        "a step the ladder marked Completed is still not selectable in linear mode"
    );
    assert!(
        state.stepper.can_select(&second),
        "a step the ladder marked Completed is still not selectable in linear mode"
    );
}

#[test]
fn an_unreached_step_stays_unselectable_in_linear_mode() {
    let mut state = CompositesState::new();
    state.stepper.current_step = data::WIZARD_STEPS[0].0.to_string();
    fired(&mut state, &CompositesMessage::StepSelected(0));

    let two_ahead = state.stepper.steps[2].id.clone();
    assert!(
        !state.stepper.can_select(&two_ahead),
        "a Pending step two ahead of the cursor must not be selectable in linear mode"
    );

    let next = state.stepper.steps[1].id.clone();
    assert!(
        state.stepper.can_select(&next),
        "the immediate next step is the one thing linear mode must always allow"
    );
}

// ---------------------------------------------------------------------------
// Pagination bar (UIF-COMP-012)
// ---------------------------------------------------------------------------

#[test]
fn pagination_page_change_updates_the_model() {
    let mut state = CompositesState::new();
    assert_eq!(data::PAGINATION.0, state.pagination.page);

    let line = fired(&mut state, &CompositesMessage::PageChanged(7));
    assert_eq!(7, state.pagination.page);
    assert!(line.contains("Page 7"), "{line}");
    assert!(line.contains("per page"), "{line}");
}

#[test]
fn pagination_clamps_to_the_last_page() {
    let mut state = CompositesState::new();
    let last = state.pagination.total_pages();
    assert_eq!(20, last, "480 items at 25 per page is 20 pages");

    fired(&mut state, &CompositesMessage::PageChanged(9_999));
    assert_eq!(last, state.pagination.page);

    fired(&mut state, &CompositesMessage::PageChanged(0));
    assert_eq!(1, state.pagination.page, "pages are 1-based");
}

#[test]
fn changing_the_page_size_recomputes_the_page_count() {
    let mut state = CompositesState::new();
    let line = fired(&mut state, &CompositesMessage::PageSizeChanged(50));
    assert_eq!(50, state.pagination.page_size);
    assert_eq!(10, state.pagination.total_pages());
    assert!(line.contains("50"), "{line}");
    assert!(line.contains("10 pages"), "{line}");
}

#[test]
fn shrinking_the_page_count_pulls_the_current_page_back() {
    let mut state = CompositesState::new();
    fired(&mut state, &CompositesMessage::PageChanged(19));
    assert_eq!(19, state.pagination.page);

    // 480 items at 100 per page is only 5 pages.
    fired(&mut state, &CompositesMessage::PageSizeChanged(100));
    assert_eq!(
        5, state.pagination.page,
        "the page must not dangle past the end"
    );
}

// ---------------------------------------------------------------------------
// Pips pager (UIF-COMP-013)
// ---------------------------------------------------------------------------

#[test]
fn pips_pager_moves_between_pips() {
    let mut state = CompositesState::new();
    assert_eq!(data::PIPS.0, state.pips.page);

    let line = fired(&mut state, &CompositesMessage::PipsPageChanged(5));
    assert_eq!(5, state.pips.page);
    assert!(line.contains("Pips page 5 of 7"), "{line}");
}

#[test]
fn pips_pager_clamps_to_its_ends() {
    let mut state = CompositesState::new();

    fired(&mut state, &CompositesMessage::PipsPageChanged(99));
    assert_eq!(7, state.pips.page);

    fired(&mut state, &CompositesMessage::PipsPageChanged(0));
    assert_eq!(1, state.pips.page);
}

// ---------------------------------------------------------------------------
// S6 — code block, diff, grid, keybind, sheet, palette, form, settings, menu
// ---------------------------------------------------------------------------

#[test]
fn copying_the_code_block_sets_the_copied_state() {
    let mut state = CompositesState::new();
    assert!(!state.code.is_copied());

    let line = fired(&mut state, &CompositesMessage::CodeCopied("rust".into()));
    assert!(state.code.is_copied());
    assert!(line.contains("rust"), "{line}");
}

#[test]
fn accepting_and_rejecting_a_hunk_both_remove_it() {
    let mut state = CompositesState::new();
    assert_eq!(2, state.diff.hunks.len());

    let line = fired(
        &mut state,
        &CompositesMessage::DiffHunkAccepted("hunk-1".into()),
    );
    assert_eq!(1, state.diff.hunks.len());
    assert!(line.contains("1 remain"), "{line}");

    let line = fired(
        &mut state,
        &CompositesMessage::DiffHunkRejected("hunk-2".into()),
    );
    assert!(state.diff.hunks.is_empty());
    assert!(line.contains("0 remain"), "{line}");
}

#[test]
fn diff_mode_switches_between_unified_and_side_by_side() {
    let mut state = CompositesState::new();
    assert_eq!(DiffDisplayMode::Unified, state.diff.mode);

    let line = fired(
        &mut state,
        &CompositesMessage::DiffModeChanged(DiffDisplayMode::SideBySide),
    );
    assert_eq!(DiffDisplayMode::SideBySide, state.diff.mode);
    assert!(line.contains("side by side"), "{line}");
}

#[test]
fn grid_selection_replaces_the_selected_set() {
    let mut state = CompositesState::new();
    let picked: BTreeSet<String> = ["tile-1".to_string(), "tile-4".to_string()]
        .into_iter()
        .collect();

    let line = fired(&mut state, &CompositesMessage::GridSelectionChanged(picked));
    assert_eq!(2, state.grid.selected_ids.len());
    assert!(state.grid.selected_ids.contains("tile-4"));
    assert!(line.contains("2 tile(s) selected"), "{line}");
}

#[test]
fn a_keybind_commit_lands_and_stops_listening() {
    let mut state = CompositesState::new();
    state.keybind.listening = true;
    let chord = KeyChord::new(true, true, false, false, "K");

    let line = fired(
        &mut state,
        &CompositesMessage::KeybindCommitted(Some(chord)),
    );
    assert_eq!(
        Some("Ctrl + Alt + K".to_string()),
        state.keybind.chord.map(|c| c.format())
    );
    assert!(!state.keybind.listening, "committing must end the capture");
    assert!(line.contains("Ctrl + Alt + K"), "{line}");
}

#[test]
fn clearing_a_keybind_is_reported_as_cleared() {
    let mut state = CompositesState::new();
    let line = fired(&mut state, &CompositesMessage::KeybindCommitted(None));
    assert_eq!(None, state.keybind.chord);
    assert!(line.contains("cleared"), "{line}");
}

#[test]
fn cancelling_a_keybind_capture_keeps_the_old_chord() {
    let mut state = CompositesState::new();
    let before = state.keybind.chord.clone();
    state.keybind.listening = true;

    let line = fired(&mut state, &CompositesMessage::KeybindCancelled);
    assert_eq!(
        before, state.keybind.chord,
        "cancel must not lose the old chord"
    );
    assert!(!state.keybind.listening);
    assert!(line.contains("cancelled"), "{line}");
}

#[test]
fn sheet_mode_and_close_update_the_model() {
    let mut state = CompositesState::new();
    assert!(state.sheet.open);

    let line = fired(
        &mut state,
        &CompositesMessage::SheetModeToggled(SheetMode::ModelessOverlay),
    );
    assert_eq!(SheetMode::ModelessOverlay, state.sheet.mode);
    assert!(line.contains("overlay"), "{line}");

    let line = fired(&mut state, &CompositesMessage::SheetClosed);
    assert!(!state.sheet.open);
    assert!(line.contains("closed"), "{line}");
}

#[test]
fn palette_query_and_execute_update_the_model() {
    let mut state = CompositesState::new();
    assert!(state.palette.open);

    let line = fired(
        &mut state,
        &CompositesMessage::PaletteQueryChanged("save".into()),
    );
    assert_eq!("save", state.palette.query);
    assert!(line.contains("save"), "{line}");

    let line = fired(
        &mut state,
        &CompositesMessage::PaletteExecuted("cmd-save".into()),
    );
    assert!(!state.palette.open, "executing closes the palette");
    assert!(line.contains("cmd-save"), "{line}");
}

#[test]
fn dismissing_the_palette_closes_it_without_executing() {
    let mut state = CompositesState::new();
    let line = fired(&mut state, &CompositesMessage::PaletteDismissed);
    assert!(!state.palette.open);
    assert!(line.contains("dismissed"), "{line}");
}

#[test]
fn form_validity_drives_which_caption_band_shows() {
    let mut state = CompositesState::new();
    let palette = hoarfrost::token::color::Palette::dark();

    let helper = form_field::resolve_caption(true, Some(data::FORM_HELPER), None, &palette);
    assert_eq!(Some(data::FORM_HELPER), helper.text);
    assert!(!helper.kind.is_error());

    let error = form_field::resolve_caption(false, None, Some(data::FORM_ERROR), &palette);
    assert_eq!(Some(data::FORM_ERROR), error.text);
    assert!(error.kind.is_error());

    let line = fired(&mut state, &CompositesMessage::FormValidityToggled(false));
    assert!(!state.form_valid);
    assert!(line.contains("invalid"), "{line}");
}

#[test]
fn settings_toggles_address_their_own_row() {
    let mut state = CompositesState::new();
    assert_eq!(vec![false, false, false], state.settings);

    let line = fired(&mut state, &CompositesMessage::SettingToggled(1, true));
    assert_eq!(vec![false, true, false], state.settings);
    assert!(line.contains("Auto-save"), "{line}");
}

#[test]
fn an_out_of_range_setting_toggle_is_ignored() {
    let mut state = CompositesState::new();
    let before = state.settings.clone();
    fired(&mut state, &CompositesMessage::SettingToggled(99, true));
    assert_eq!(before, state.settings);
}

#[test]
fn activating_a_menu_row_toggles_its_check_state() {
    let mut state = CompositesState::new();
    let index = state
        .menu
        .rows()
        .iter()
        .position(|row| row.key() == "m-wrap")
        .expect("the checkable row must exist");
    assert!(state.menu.rows()[index].is_checked());

    fired(
        &mut state,
        &CompositesMessage::MenuRowActivated("m-wrap".into()),
    );
    assert!(
        !state.menu.rows()[index].is_checked(),
        "checkable rows toggle"
    );
}

#[test]
fn activating_a_menu_radio_moves_the_selection_within_its_group() {
    let mut state = CompositesState::new();
    let group = hoarfrost::surface::menu::model::RadioGroupId::from(data::MENU_RADIO_GROUP);

    let before = state
        .menu
        .radio_selection(&group)
        .expect("the fixture selects a theme");
    assert_eq!("m-dark", state.menu.rows()[before].key());

    fired(
        &mut state,
        &CompositesMessage::MenuRowActivated("m-light".into()),
    );
    let after = state
        .menu
        .radio_selection(&group)
        .expect("the group still has a selection");
    assert_ne!(before, after, "the selection must move");
    assert_eq!("m-light", state.menu.rows()[after].key());
}
#[test]
fn activating_a_separator_is_absorbed_not_toggled() {
    let mut state = CompositesState::new();
    let before = state.menu.rows().to_vec();
    fired(
        &mut state,
        &CompositesMessage::MenuRowActivated("m-sep-1".into()),
    );
    assert_eq!(before, state.menu.rows(), "a separator changes nothing");
}

#[test]
fn an_unknown_menu_key_changes_nothing() {
    let mut state = CompositesState::new();
    let before = state.menu.rows().to_vec();
    fired(
        &mut state,
        &CompositesMessage::MenuRowActivated("nope".into()),
    );
    assert_eq!(before, state.menu.rows());
}

#[test]
fn an_empty_state_action_is_recorded() {
    let mut state = CompositesState::new();
    assert_eq!(None, state.last_empty_action);

    let line = fired(
        &mut state,
        &CompositesMessage::EmptyAction("empty-retry".into()),
    );
    assert_eq!(Some("empty-retry".to_string()), state.last_empty_action);
    assert!(line.contains("empty-retry"), "{line}");
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

#[test]
fn a_note_passes_straight_through() {
    let mut state = CompositesState::new();
    let line = fired(&mut state, &CompositesMessage::Note("hello".into()));
    assert_eq!("hello", line);
}

// ---------------------------------------------------------------------------
// Descriptions
// ---------------------------------------------------------------------------

/// Every message produces a non-empty, single-line, reasonably short status line.
#[test]
fn every_message_describes_itself() {
    let messages = [
        CompositesMessage::TabSelected("tab-notes".into()),
        CompositesMessage::TabCloseRequested("tab-notes".into()),
        CompositesMessage::RailSelected("rail-calendar".into()),
        CompositesMessage::RailExpanded(true),
        CompositesMessage::StepSelected(1),
        CompositesMessage::StepNext,
        CompositesMessage::StepPrevious,
        CompositesMessage::PageChanged(2),
        CompositesMessage::PageSizeChanged(50),
        CompositesMessage::PipsPageChanged(3),
        CompositesMessage::Note("something else".into()),
    ];
    for message in &messages {
        let mut state = CompositesState::new();
        let line = fired(&mut state, message);
        assert!(!line.trim().is_empty(), "{message:?} describes nothing");
        assert!(!line.contains('\n'), "{message:?} is not single-line");
        assert!(
            line.chars().count() <= 96,
            "{message:?} description is too long: {line}"
        );
    }
}

/// Describing the same message twice gives the same answer — the status line is a
/// function of state, not a side effect that drains.
#[test]
fn describe_is_repeatable() {
    let mut state = CompositesState::new();
    let message = CompositesMessage::StepNext;
    state.apply(&message);
    assert_eq!(state.describe(&message), state.describe(&message));
}

// ---------------------------------------------------------------------------
// Fixture sanity
// ---------------------------------------------------------------------------

/// The pane depends on the fixtures being shaped the way its builders assume.
#[test]
fn fixtures_are_shaped_as_the_pane_assumes() {
    let tabs = data::tab_strip_model();
    assert_eq!(data::TABS.len(), tabs.tabs.len());
    assert_eq!(Some(data::TAB_SELECTED.to_string()), tabs.active_id);
    assert_eq!(
        data::TABS_DIRTY.to_vec(),
        tabs.tabs
            .iter()
            .filter(|tab| tab.is_dirty)
            .map(|tab| tab.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        data::TABS_PINNED.to_vec(),
        tabs.tabs
            .iter()
            .filter(|tab| tab.is_pinned)
            .map(|tab| tab.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        tabs.tabs.iter().all(|tab| tab.tooltip.is_some()),
        "every tab should carry a tooltip"
    );

    let rail = data::rail_model();
    assert_eq!(data::RAIL_TOP.len(), rail.top_items.len());
    assert_eq!(data::RAIL_BOTTOM.len(), rail.bottom_items.len());
    assert_eq!(data::RAIL_SELECTED, rail.selected);
    assert!(rail.expanded);
    assert!(
        rail.top_items.iter().all(|d| !d.icon.0.is_empty()),
        "every rail destination needs a glyph"
    );
    assert!(
        rail.bottom_items.iter().all(|d| !d.icon.0.is_empty()),
        "every footer destination needs a glyph"
    );

    let stepper = data::stepper_model();
    assert_eq!(data::WIZARD_STEPS.len(), stepper.steps.len());
    assert_eq!(data::WIZARD_CURRENT, stepper.current_step);
    assert!(stepper.linear);

    let pagination = data::pagination_model();
    assert_eq!(data::PAGINATION.0, pagination.page);
    assert_eq!(data::PAGINATION.1, pagination.page_size);
    assert_eq!(data::PAGINATION.2, pagination.total_items);
    assert_eq!(
        data::PAGE_SIZE_OPTIONS.to_vec(),
        pagination.page_size_options
    );

    let pips = data::pips_model();
    assert_eq!(data::PIPS.0, pips.page);
    assert_eq!(data::PIPS.1, pips.total_pages);
}

/// `default()` and `new()` build the same demo.
#[test]
fn default_state_matches_new() {
    let a = CompositesState::new();
    let b = CompositesState::default();
    assert_eq!(a.widget_count(), b.widget_count());
    assert_eq!(a.tabs.tabs.len(), b.tabs.tabs.len());
    assert_eq!(a.rail.selected, b.rail.selected);
    assert_eq!(a.stepper.current_step, b.stepper.current_step);
    assert_eq!(a.pagination.page, b.pagination.page);
    assert_eq!(a.pips.page, b.pips.page);
    assert_eq!(a.pending_close, b.pending_close);
}
