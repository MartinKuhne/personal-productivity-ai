//! Tests for the combobox host — `UIF-ATOM-004`, §3.4.
//!
//! Five groups, in order:
//!
//! 1. **Opening.** What each key does from a closed control, and the two
//!    conditions under which the flyout cannot open at all.
//! 2. **Traversing.** Movement inside the open list, including the
//!    Home/End arithmetic and the wrap that makes `Up`-from-closed land
//!    on the last row.
//! 3. **Settling.** Commit, cancel, and the snapshot discipline that
//!    makes the cancel restore what the user actually saw.
//! 4. **The filter.** The live tier, the no-match invariant, and the
//!    working copy that keeps a slow round-trip from yanking the list.
//! 5. **Pointer and teardown.** Click semantics and what a reset does and
//!    deliberately does not do.

use super::*;
use crate::focus::{FocusId, FocusMember, GroupId};
use crate::widget::combo_model::SelectOption;

/// The control identity every host in this file is built with.
fn control() -> FocusId {
    FocusId::new("combo-control")
}

/// The three-option list every test is driven against.
fn options() -> Vec<SelectOption<String>> {
    vec![
        SelectOption::new("wrap".to_string(), "Word Wrap"),
        SelectOption::new("tab".to_string(), "Tab Size"),
        SelectOption::new("font".to_string(), "Font Ligatures"),
    ]
}

/// A closed, non-editable select with nothing picked.
fn model() -> ComboBoxModel<String> {
    ComboBoxModel::new(options(), "Choose")
}

/// A fresh host over a focus state in which the control can be focused,
/// so the trap has somewhere to come back to.
fn pair() -> (ComboHost<String>, FocusState) {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("combo-bar"),
        vec![
            FocusMember::new("combo-control"),
            FocusMember::new("other-control"),
        ],
    );
    (ComboHost::new(control()), focus)
}

/// The committed id carried by an outcome, if it committed.
fn committed<Id>(outcome: &ComboOutcome<Id>) -> Option<&Id> {
    outcome
        .emissions
        .iter()
        .find_map(|emission| match emission {
            ComboEmission::Committed(commit) => Some(commit.value()),
            _ => None,
        })
}

/// The snapshot restored by an outcome, if it cancelled.
fn restored<Id>(outcome: &ComboOutcome<Id>) -> Option<&ComboSnapshot<Id>> {
    outcome
        .emissions
        .iter()
        .find_map(|emission| match emission {
            ComboEmission::Cancelled(cancel) => Some(cancel.restored()),
            _ => None,
        })
}

/// The live filter an outcome reported, if it reported one.
fn live<Id>(outcome: &ComboOutcome<Id>) -> Option<&str> {
    outcome
        .emissions
        .iter()
        .find_map(|emission| match emission {
            ComboEmission::FilterChanged(tick) => Some(tick.value().as_str()),
            _ => None,
        })
}

/// Open the flyout with `Down`, asserting it opened.
fn open(host: &mut ComboHost<String>, model: &ComboBoxModel<String>, focus: &mut FocusState) {
    let outcome = host.key(ComboKey::Down, model, focus);
    assert!(host.is_open(), "Down on a closed control should open");
    assert!(
        outcome
            .emissions
            .contains(&ComboEmission::OpenChanged(true)),
        "opening should report the toggle"
    );
}

// ---------------------------------------------------------------------------
// 1 — opening
// ---------------------------------------------------------------------------

#[test]
fn down_on_a_closed_control_opens_the_flyout() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    assert_eq!(
        host.cascade().root_id().map(|id| id.as_str()),
        Some(model.menu().id().as_str()),
        "the flyout's own card is what opened"
    );
    assert_eq!(host.cascade().focused_row(), Some(0));
}

#[test]
fn up_on_a_closed_control_opens_and_lands_on_the_last_row() {
    let (mut host, mut focus) = pair();
    let model = model();
    let outcome = host.key(ComboKey::Up, &model, &mut focus);
    assert!(host.is_open());
    assert_eq!(
        host.cascade().focused_row(),
        Some(2),
        "Up entering from above lands on the last row"
    );
    assert!(
        outcome
            .emissions
            .contains(&ComboEmission::OpenChanged(true))
    );
}

#[test]
fn enter_on_a_closed_control_opens_rather_than_committing() {
    let (mut host, mut focus) = pair();
    let model = model();
    let outcome = host.key(ComboKey::Enter, &model, &mut focus);
    assert!(host.is_open());
    assert_eq!(committed(&outcome), None);
}

#[test]
fn space_on_a_closed_control_opens_rather_than_committing() {
    let (mut host, mut focus) = pair();
    let model = model();
    let outcome = host.key(ComboKey::Space, &model, &mut focus);
    assert!(host.is_open());
    assert_eq!(committed(&outcome), None);
}

#[test]
fn a_disabled_control_answers_no_key_at_all() {
    let model = model().disabled();
    for key in [
        ComboKey::Down,
        ComboKey::Up,
        ComboKey::Enter,
        ComboKey::Space,
        ComboKey::Home,
        ComboKey::End,
        ComboKey::Escape,
    ] {
        let (mut host, mut focus) = pair();
        let outcome = host.key(key, &model, &mut focus);
        assert!(
            outcome.is_noop() && !host.is_open(),
            "{key:?} should be refused by a disabled control"
        );
    }
}

#[test]
fn an_empty_list_cannot_open_a_flyout() {
    let (mut host, mut focus) = pair();
    let empty = ComboBoxModel::new(Vec::new(), "Nothing");
    let outcome = host.key(ComboKey::Down, &empty, &mut focus);
    assert!(!host.is_open());
    assert!(outcome.emissions.is_empty());
}

#[test]
fn a_list_of_only_disabled_options_cannot_open_a_flyout() {
    let (mut host, mut focus) = pair();
    let dead = ComboBoxModel::new(
        vec![
            SelectOption::new("a".to_string(), "Alpha").disabled(),
            SelectOption::new("b".to_string(), "Beta").disabled(),
        ],
        "Nothing pickable",
    );
    let outcome = host.key(ComboKey::Down, &dead, &mut focus);
    assert!(!host.is_open(), "no focusable row means no flyout");
    assert!(outcome.emissions.is_empty());
}

#[test]
fn the_control_is_focused_before_the_trap_is_pushed_so_escape_returns_there() {
    let (mut host, mut focus) = pair();
    let model = model();
    focus.focus(&control());
    open(&mut host, &model, &mut focus);
    assert_ne!(
        focus.current(),
        Some(&control()),
        "the flyout has taken focus into a row"
    );
    let _ = host.escape(&mut focus);
    assert_eq!(
        focus.current(),
        Some(&control()),
        "the trap must hand focus back to the trigger"
    );
}

// ---------------------------------------------------------------------------
// 2 — traversing
// ---------------------------------------------------------------------------

#[test]
fn down_and_up_move_within_the_open_list() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(1));
    let _ = host.key(ComboKey::Up, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(0));
}

#[test]
fn traversal_wraps_at_both_ends() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Up, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(2), "wraps up to the end");
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    assert_eq!(
        host.cascade().focused_row(),
        Some(0),
        "wraps down to the top"
    );
}

#[test]
fn home_jumps_to_the_first_row_from_anywhere() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::End, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(2));
    let _ = host.key(ComboKey::Home, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(0));
}

#[test]
fn end_jumps_to_the_last_row_from_anywhere() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::End, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(2));
}

#[test]
fn home_and_end_on_the_row_they_would_land_on_are_noops() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(0));
    let outcome = host.key(ComboKey::Home, &model, &mut focus);
    assert!(outcome.is_noop(), "already home: nothing moved");
    assert_eq!(host.cascade().focused_row(), Some(0));
}

#[test]
fn home_and_end_on_a_closed_control_do_nothing() {
    for key in [ComboKey::Home, ComboKey::End] {
        let (mut host, mut focus) = pair();
        let model = model();
        let outcome = host.key(key, &model, &mut focus);
        assert!(outcome.is_noop(), "{key:?} needs an open list");
        assert!(!host.is_open());
    }
}

#[test]
fn traversal_skips_a_disabled_row() {
    let (mut host, mut focus) = pair();
    let model = ComboBoxModel::new(
        vec![
            SelectOption::new("a".to_string(), "Alpha"),
            SelectOption::new("b".to_string(), "Beta").disabled(),
            SelectOption::new("c".to_string(), "Gamma"),
        ],
        "Choose",
    );
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    assert_eq!(
        host.cascade().focused_row(),
        Some(2),
        "the disabled middle row is not in the roster at all"
    );
}

// ---------------------------------------------------------------------------
// 3 — settling
// ---------------------------------------------------------------------------

#[test]
fn enter_on_an_open_list_commits_the_focused_option_and_closes() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    let outcome = host.key(ComboKey::Enter, &model, &mut focus);
    assert_eq!(committed(&outcome), Some(&"tab".to_string()));
    assert!(!host.is_open());
    assert!(
        outcome
            .emissions
            .contains(&ComboEmission::OpenChanged(false))
    );
}

#[test]
fn space_commits_what_enter_commits() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let outcome = host.key(ComboKey::Space, &model, &mut focus);
    assert_eq!(committed(&outcome), Some(&"wrap".to_string()));
    assert!(!host.is_open());
}

#[test]
fn the_commit_carries_the_applications_own_id_not_a_row_index() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::End, &model, &mut focus);
    let outcome = host.key(ComboKey::Enter, &model, &mut focus);
    assert_eq!(
        committed(&outcome),
        Some(&"font".to_string()),
        "the id is the application's, mapped through the row's key"
    );
}

#[test]
fn escape_restores_the_selection_and_the_filter_from_open_time() {
    let (mut host, mut focus) = pair();
    let model = model()
        .editable()
        .with_selected("tab".to_string())
        .with_filter("wo".to_string());
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    let outcome = host.key(ComboKey::Escape, &model, &mut focus);
    let restored = restored(&outcome).expect("Escape should cancel");
    assert_eq!(restored.selected, Some("tab".to_string()));
    assert_eq!(restored.filter, Some("wo".to_string()));
    assert!(!host.is_open());
}

#[test]
fn the_snapshot_holds_what_the_user_saw_not_what_the_host_last_pushed() {
    // The pre-open snapshot is captured before the cascade opens, so a
    // gesture that changes the selection mid-flight still reverts to the
    // value that was on screen when the flyout appeared.
    let (mut host, mut focus) = pair();
    let before = model().with_selected("wrap".to_string());
    open(&mut host, &before, &mut focus);
    let _ = host.key(ComboKey::Down, &before, &mut focus);
    let outcome = host.key(ComboKey::Escape, &before, &mut focus);
    assert_eq!(
        restored(&outcome).map(|snapshot| snapshot.selected.clone()),
        Some(Some("wrap".to_string()))
    );
}

#[test]
fn escape_on_a_closed_control_emits_nothing() {
    let (mut host, mut focus) = pair();
    let model = model();
    let outcome = host.key(ComboKey::Escape, &model, &mut focus);
    assert!(outcome.is_noop());
    assert_eq!(restored(&outcome), None);
}

#[test]
fn settling_clears_the_held_snapshot_so_a_second_escape_cannot_replay_it() {
    let (mut host, mut focus) = pair();
    let model = model();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Enter, &model, &mut focus);
    assert!(!host.gesture_open());
    let second = host.key(ComboKey::Escape, &model, &mut focus);
    assert_eq!(restored(&second), None);
}

#[test]
fn closing_by_the_trigger_is_not_a_cancellation() {
    let (mut host, mut focus) = pair();
    let model = model().with_selected("tab".to_string());
    open(&mut host, &model, &mut focus);
    let outcome = host.key(ComboKey::Down, &model, &mut focus);
    assert_eq!(restored(&outcome), None, "Down is not Escape");
    let closed = host.close(&mut focus);
    assert!(!host.is_open());
    assert_eq!(restored(&closed), None, "closing gives nothing back");
    assert!(
        closed
            .emissions
            .contains(&ComboEmission::OpenChanged(false))
    );
}

// ---------------------------------------------------------------------------
// 4 — the filter
// ---------------------------------------------------------------------------

#[test]
fn typing_in_the_editable_variant_reports_a_live_tick() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    let outcome = host.on_filter_typed(&model, "wo".to_string(), &mut focus);
    assert_eq!(live(&outcome), Some("wo"));
    assert_eq!(host.filter(), Some("wo"));
}

#[test]
fn typing_on_a_non_editable_control_reports_nothing() {
    let (mut host, mut focus) = pair();
    let model = model();
    let outcome = host.on_filter_typed(&model, "wo".to_string(), &mut focus);
    assert!(outcome.is_noop());
    assert_eq!(host.filter(), None);
}

#[test]
fn a_read_only_control_reports_no_filter_at_all() {
    let (mut host, mut focus) = pair();
    let model = model().editable().read_only();
    let outcome = host.on_filter_typed(&model, "wo".to_string(), &mut focus);
    assert!(outcome.is_noop());
}

#[test]
fn a_filter_matching_nothing_still_reports_the_keystroke() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    let outcome = host.on_filter_typed(&model, "zzz".to_string(), &mut focus);
    assert_eq!(
        live(&outcome),
        Some("zzz"),
        "the user's text is live whether or not a list appeared"
    );
}

#[test]
fn a_filter_matching_nothing_cannot_open_the_flyout() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    let _ = host.on_filter_typed(&model, "zzz".to_string(), &mut focus);
    let outcome = host.key(ComboKey::Down, &model, &mut focus);
    assert!(
        !host.is_open(),
        "the engine refuses a card with no focusable row"
    );
    assert!(outcome.emissions.is_empty());
}

#[test]
fn the_open_flyout_narrows_as_the_filter_narrows() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    open(&mut host, &model, &mut focus);
    assert_eq!(host.cascade().active_menu().map(|menu| menu.len()), Some(3));
    let _ = host.on_filter_typed(&model, "Tab".to_string(), &mut focus);
    assert_eq!(
        host.cascade().active_menu().map(|menu| menu.len()),
        Some(1),
        "the open card is rebuilt under the host's working filter"
    );
}

#[test]
fn the_working_filter_survives_a_model_that_has_not_caught_up() {
    // The application has not yet applied the keystroke to its own model.
    // Syncing against that stale model must still show what the user typed.
    let (mut host, mut focus) = pair();
    let stale = model().editable();
    let _ = host.on_filter_typed(&stale, "Tab".to_string(), &mut focus);
    open(&mut host, &stale, &mut focus);
    let _ = host.sync(&stale, &mut focus);
    assert_eq!(
        host.cascade().active_menu().map(|menu| menu.len()),
        Some(1),
        "sync uses the host's filter, not the model's"
    );
}

#[test]
fn typing_that_makes_the_open_list_unopenable_closes_it_and_says_so() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    open(&mut host, &model, &mut focus);
    let outcome = host.on_filter_typed(&model, "zzz".to_string(), &mut focus);
    assert_eq!(
        live(&outcome),
        Some("zzz"),
        "the keystroke is still reported"
    );
    assert!(!host.is_open());
    assert!(
        outcome
            .emissions
            .contains(&ComboEmission::OpenChanged(false))
    );
}

#[test]
fn a_sync_against_a_model_that_no_longer_matches_the_filter_closes_the_flyout() {
    let (mut host, mut focus) = pair();
    let model = model().editable().with_filter("Si".to_string());
    open(&mut host, &model, &mut focus);
    assert_eq!(host.cascade().active_menu().map(|menu| menu.len()), Some(1));

    let rebuilt = ComboBoxModel::new(
        vec![SelectOption::new("wrap".to_string(), "Word Wrap")],
        "Choose",
    )
    .editable()
    .with_filter("Si".to_string());
    let outcome = host.sync(&rebuilt, &mut focus);
    assert!(!host.is_open());
    assert!(
        outcome
            .emissions
            .contains(&ComboEmission::OpenChanged(false))
    );
}

#[test]
fn sync_on_a_closed_flyout_is_a_noop() {
    let (mut host, mut focus) = pair();
    let model = model();
    assert!(host.sync(&model, &mut focus).is_noop());
}

#[test]
fn a_model_filter_narrows_the_flyout_before_the_host_takes_ownership() {
    let (mut host, mut focus) = pair();
    let model = model().editable().with_filter("Si".to_string());
    open(&mut host, &model, &mut focus);
    assert_eq!(host.cascade().active_menu().map(|menu| menu.len()), Some(1));
    assert_eq!(
        host.filter(),
        None,
        "the model is still deciding; the host has not taken the filter over"
    );
}

#[test]
fn settling_clears_the_working_filter() {
    let (mut host, mut focus) = pair();
    let model = model().editable();
    open(&mut host, &model, &mut focus);
    let _ = host.on_filter_typed(&model, "Tab".to_string(), &mut focus);
    let _ = host.key(ComboKey::Enter, &model, &mut focus);
    assert_eq!(host.filter(), None);
}

// ---------------------------------------------------------------------------
// 5 — read-only, pointer and teardown
// ---------------------------------------------------------------------------

#[test]
fn a_read_only_control_opens_browses_and_commits_nothing() {
    let (mut host, mut focus) = pair();
    let model = model().read_only();
    open(&mut host, &model, &mut focus);
    let _ = host.key(ComboKey::Down, &model, &mut focus);
    assert_eq!(host.cascade().focused_row(), Some(1), "still browsable");
    let outcome = host.key(ComboKey::Enter, &model, &mut focus);
    assert_eq!(committed(&outcome), None, "read-only never commits");
    assert!(!host.is_open(), "the flyout still closes");
}

#[test]
fn a_read_only_control_still_shows_which_row_is_current() {
    let model = model().read_only().with_selected("tab".to_string());
    let menu = model.menu();
    let checked: Vec<&str> = menu
        .rows()
        .iter()
        .filter(|row| row.is_checked())
        .map(|row| row.key())
        .collect();
    assert_eq!(checked, vec!["tab"]);
}

#[test]
fn a_press_does_not_open_the_flyout() {
    let (mut host, _focus) = pair();
    let model = model();
    host.press(&model);
    assert!(!host.is_open(), "a click is press-and-release, not press");
    assert!(host.pressed());
}

#[test]
fn releasing_inside_the_trigger_toggles_the_flyout() {
    let (mut host, mut focus) = pair();
    let model = model();
    host.press(&model);
    let first = host.release(true, &model, &mut focus);
    assert!(host.is_open());
    assert!(first.emissions.contains(&ComboEmission::OpenChanged(true)));

    host.press(&model);
    let second = host.release(true, &model, &mut focus);
    assert!(!host.is_open(), "a second click shuts it");
    assert!(
        second
            .emissions
            .contains(&ComboEmission::OpenChanged(false))
    );
}

#[test]
fn releasing_outside_the_trigger_opens_nothing() {
    let (mut host, mut focus) = pair();
    let model = model();
    host.press(&model);
    let outcome = host.release(false, &model, &mut focus);
    assert!(!host.is_open());
    assert!(outcome.is_noop());
    assert!(!host.pressed(), "the gesture is spent");
}

#[test]
fn a_release_without_a_press_does_nothing() {
    let (mut host, mut focus) = pair();
    let model = model();
    assert!(host.release(true, &model, &mut focus).is_noop());
}

#[test]
fn a_disabled_control_takes_no_press() {
    let (mut host, _focus) = pair();
    let model = model().disabled();
    host.press(&model);
    assert!(!host.pressed());
}

#[test]
fn hover_only_paints_and_never_opens() {
    let (mut host, focus) = pair();
    host.hover(true);
    assert!(host.hovered());
    assert!(!host.is_open());
    host.hover(false);
    assert!(!host.hovered());
    assert!(!focus.is_focused(&control()));
}

#[test]
fn reset_drops_pointer_state_and_snapshot_but_leaves_the_flyout_alone() {
    let (mut host, mut focus) = pair();
    let model = model();
    host.hover(true);
    open(&mut host, &model, &mut focus);
    host.reset();
    assert!(!host.hovered());
    assert!(!host.pressed());
    assert!(!host.gesture_open());
    assert_eq!(host.filter(), None);
    assert!(
        host.is_open(),
        "tearing a flyout down silently would drop the visible event"
    );
}

#[test]
fn the_host_holds_its_control_identity() {
    let (host, _focus) = pair();
    assert_eq!(host.control(), &control());
}
