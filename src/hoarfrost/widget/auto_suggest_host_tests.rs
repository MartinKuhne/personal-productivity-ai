//! Tests for the `AutoSuggestBox` machine — `UIF-ATOM-002d`, §3.2.
//!
//! Nine groups, in order:
//!
//! 1. **The debounce.** That a burst of keystrokes produces exactly one
//!    provider pass, carrying the text the user stopped on.
//! 2. **Stale answers.** That a response for a generation no longer in
//!    flight is refused rather than painted.
//! 3. **The two-stage `Escape`.** The whole transition table, including
//!    the third row where the key is not consumed at all.
//! 4. **Dismissal stickiness.** That the list the user just dismissed
//!    does not come back when its answer finally arrives.
//! 5. **Roving.** Wrapping, reporting, and the text staying put.
//! 6. **The three commit sources.** Enter, click and Tab, and what each
//!    one settles.
//! 7. **No match.** That nothing opens, and that the field is still a
//!    field.
//! 8. **Focus.** What leaving and returning do to the flyout, the
//!    request and the dismissal record.
//! 9. **The frame pass and teardown.** `sync` and `reset`.

use super::*;
use crate::focus::{FocusId, FocusMember, GroupId};
use crate::widget::auto_suggest_model::DEBOUNCE_MS;

/// The control identity every host in this file is built with.
fn control() -> FocusId {
    FocusId::new("suggest-control")
}

/// A fresh host over a focus state in which the control can be focused,
/// so the flyout's trap has somewhere to come back to.
fn pair() -> (AutoSuggestHost<String>, FocusState) {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("suggest-bar"),
        vec![
            FocusMember::new("suggest-control"),
            FocusMember::new("other-control"),
        ],
    );
    (AutoSuggestHost::new(control()), focus)
}

/// The three candidates a provider answers with.
fn suggestions() -> Vec<SuggestionItem<String>> {
    vec![
        SuggestionItem::new("doc-readme".to_string(), "README.md"),
        SuggestionItem::new("doc-notes".to_string(), "notes.md"),
        SuggestionItem::new("doc-todo".to_string(), "todo.md"),
    ]
}

/// The instant every test starts from.
const T0: TimeMs = TimeMs::millis(10_000);

/// An instant `ms` after [`T0`].
fn later(ms: u64) -> TimeMs {
    TimeMs::millis(10_000 + ms)
}

/// The names of the emissions a call produced, for recording.
fn names(emissions: &[AutoSuggestEmission<String>]) -> Vec<&'static str> {
    emissions.iter().map(AutoSuggestEmission::name).collect()
}

/// The generation a `QueryRequested` carried, if one was produced.
fn requested(emissions: &[AutoSuggestEmission<String>]) -> Option<(u64, String)> {
    emissions.iter().find_map(|emission| match emission {
        AutoSuggestEmission::QueryRequested { generation, query } => {
            Some((*generation, query.clone()))
        }
        _ => None,
    })
}

/// Type one character and advance the frame clock past the window,
/// returning the request that came out.
fn type_and_wait(
    host: &mut AutoSuggestHost<String>,
    model: &mut AutoSuggestModel<String>,
    text: &str,
    at: TimeMs,
) -> Vec<AutoSuggestEmission<String>> {
    let _ = host.note_edit(model, text.to_string(), at);
    host.tick(model, later(at.as_millis() - T0.as_millis() + DEBOUNCE_MS))
}

/// Drive a host to the point where the flyout is open over three
/// suggestions.
fn opened() -> (
    AutoSuggestHost<String>,
    AutoSuggestModel<String>,
    FocusState,
) {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    // The field is focused before anything is typed, so that a later focus
    // loss is a real falling edge rather than a no-op.
    let _ = host.note_focus(&mut model, true, &mut focus);

    let emissions = type_and_wait(&mut host, &mut model, "do", T0);
    let generation = requested(&emissions)
        .expect("the wait should have dispatched a request")
        .0;
    let _ = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);
    assert!(
        host.is_open(),
        "the sample list should have opened the flyout"
    );
    (host, model, focus)
}

// ---------------------------------------------------------------------------
// 1 — The debounce
// ---------------------------------------------------------------------------

#[test]
fn a_keystroke_arms_the_window_and_reports_the_live_tier_only() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    let emissions = host.note_edit(&mut model, "r".to_string(), T0);

    assert_eq!(names(&emissions), vec!["QueryChanged"]);
    assert_eq!(model.query, "r");
    assert!(host.is_debouncing());
    assert!(!host.is_busy());
    assert_eq!(host.provider_passes(), 0);
}

#[test]
fn no_request_is_dispatched_before_the_window_closes() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    let _ = host.note_edit(&mut model, "re".to_string(), T0);
    let mid = host.tick(&mut model, later(DEBOUNCE_MS - 1));

    assert!(mid.is_empty(), "the window had not closed");
    assert_eq!(host.provider_passes(), 0);
}

#[test]
fn the_request_fires_the_moment_the_window_closes() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    let _ = host.note_edit(&mut model, "read".to_string(), T0);
    let fired = host.tick(&mut model, later(DEBOUNCE_MS));

    assert_eq!(requested(&fired), Some((1, "read".to_string())));
    assert_eq!(host.provider_passes(), 1);
    assert!(host.is_busy());
    assert!(model.busy, "the model must show the request is outstanding");
    assert!(!host.is_debouncing());
}

#[test]
fn a_burst_inside_the_window_produces_exactly_one_pass_carrying_the_final_text() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    // Five keystrokes, each 20ms apart — every one inside the window of
    // the one before it.
    for (index, text) in ["d", "do", "doc", "doc-", "doc-r"].iter().enumerate() {
        let at = later(index as u64 * 20);
        let emissions = host.note_edit(&mut model, text.to_string(), at);
        assert_eq!(names(&emissions), vec!["QueryChanged"]);
        // The frame pass that would fire a request has not arrived yet.
        assert!(host.tick(&mut model, at).is_empty());
    }

    let fired = host.tick(&mut model, later(4 * 20 + DEBOUNCE_MS));

    assert_eq!(
        host.provider_passes(),
        1,
        "a five-keystroke burst must dispatch exactly one provider pass"
    );
    assert_eq!(
        requested(&fired),
        Some((1, "doc-r".to_string())),
        "the pass must carry the text the user stopped on"
    );
}

#[test]
fn each_keystroke_restarts_the_window_rather_than_extending_a_fixed_one() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    let _ = host.note_edit(&mut model, "a".to_string(), T0);
    // One ms before the first window closes, a new keystroke lands.
    let _ = host.note_edit(&mut model, "ab".to_string(), later(DEBOUNCE_MS - 1));

    assert!(
        host.tick(&mut model, later(DEBOUNCE_MS)).is_empty(),
        "the window must have restarted at the second keystroke"
    );
    let fired = host.tick(&mut model, later(DEBOUNCE_MS - 1 + DEBOUNCE_MS));
    assert_eq!(requested(&fired), Some((1, "ab".to_string())));
}

#[test]
fn a_second_burst_after_a_first_gets_its_own_generation() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new();

    let first = type_and_wait(&mut host, &mut model, "ab", T0);
    let second = type_and_wait(&mut host, &mut model, "abc", later(DEBOUNCE_MS + 500));

    assert_eq!(requested(&first).map(|g| g.0), Some(1));
    assert_eq!(requested(&second).map(|g| g.0), Some(2));
    assert_eq!(host.provider_passes(), 2);
}

#[test]
fn an_edit_that_changes_nothing_arms_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new().with_query("same");
    host.sync(&mut model, &mut focus);

    let emissions = host.note_edit(&mut model, "same".to_string(), T0);

    assert!(emissions.is_empty());
    assert!(!host.is_debouncing());
}

#[test]
fn a_disabled_control_arms_nothing_and_drops_any_arming_it_held() {
    let (mut host, _focus) = pair();
    let mut model = AutoSuggestModel::new().disabled();

    assert!(host.note_edit(&mut model, "x".to_string(), T0).is_empty());
    assert!(!host.is_debouncing());

    let mut live = AutoSuggestModel::new();
    let _ = host.note_edit(&mut live, "x".to_string(), T0);
    assert!(host.is_debouncing());
    live.disabled = true;
    assert!(host.tick(&mut live, later(DEBOUNCE_MS)).is_empty());
    assert!(
        !host.is_debouncing(),
        "a disabled control must not hold a request"
    );
}

// ---------------------------------------------------------------------------
// 2 — Stale answers
// ---------------------------------------------------------------------------

#[test]
fn an_answer_for_the_generation_in_flight_is_applied() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "do", T0);
    let generation = requested(&fired).unwrap().0;

    let applied = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);

    assert_eq!(model.suggestions.len(), 3);
    assert!(!model.busy);
    assert!(host.is_open());
    assert!(applied.contains(&AutoSuggestEmission::OpenChanged(true)));
}

#[test]
fn an_answer_for_a_superseded_generation_is_dropped_and_counted() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let first = type_and_wait(&mut host, &mut model, "ab", T0);
    let stale_generation = requested(&first).unwrap().0;
    let second = type_and_wait(&mut host, &mut model, "abc", later(DEBOUNCE_MS + 100));
    let live_generation = requested(&second).unwrap().0;

    let dropped = host.apply_suggestions(&mut model, stale_generation, suggestions(), &mut focus);

    assert!(dropped.is_empty(), "a stale answer must report nothing");
    assert!(
        model.suggestions.is_empty(),
        "a stale answer must paint nothing"
    );
    assert_eq!(host.stale_dropped(), 1);
    assert_ne!(stale_generation, live_generation);

    // The live one still lands afterwards.
    let _ = host.apply_suggestions(&mut model, live_generation, suggestions(), &mut focus);
    assert_eq!(model.suggestions.len(), 3);
    assert_eq!(
        host.stale_dropped(),
        1,
        "the live answer must not be counted stale"
    );
}

#[test]
fn out_of_order_answers_leave_the_list_matching_the_text_not_the_first_arrival() {
    // The race the generation exists to settle: ask for "ab", then for
    // "abc"; the provider answers "abc" first and "ab" second.
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let for_ab = type_and_wait(&mut host, &mut model, "ab", T0);
    let ab = requested(&for_ab).unwrap().0;
    let for_abc = type_and_wait(&mut host, &mut model, "abc", later(DEBOUNCE_MS + 50));
    let abc = requested(&for_abc).unwrap().0;

    let fresh = vec![SuggestionItem::new("abc-one".to_string(), "abc-one.md")];
    let _ = host.apply_suggestions(&mut model, abc, fresh, &mut focus);
    let _ = host.apply_suggestions(&mut model, ab, suggestions(), &mut focus);

    assert_eq!(
        model.suggestions.len(),
        1,
        "the late answer for `ab` must not replace the list for `abc`"
    );
    assert_eq!(model.suggestions[0].id, "abc-one");
    assert_eq!(host.stale_dropped(), 1);
}

#[test]
fn an_answer_with_no_request_in_flight_is_refused() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let emissions = host.apply_suggestions(&mut model, 1, suggestions(), &mut focus);

    assert!(emissions.is_empty());
    assert!(model.suggestions.is_empty());
    assert_eq!(host.stale_dropped(), 1);
}

#[test]
fn an_answer_arriving_after_the_query_was_cleared_is_refused() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "do", T0);
    let generation = requested(&fired).unwrap().0;

    // The user clears the field before the provider answers.
    let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert_eq!(model.query, "");

    let late = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);

    assert!(late.is_empty());
    assert!(model.suggestions.is_empty());
    assert!(!host.is_open());
    assert_eq!(host.stale_dropped(), 1);
}

#[test]
fn an_answer_arriving_after_focus_was_lost_is_refused() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "do", T0);
    let generation = requested(&fired).unwrap().0;

    let _ = host.note_focus(&mut model, false, &mut focus);
    let late = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);

    assert!(late.is_empty());
    assert!(model.suggestions.is_empty());
    assert!(!model.busy);
    assert_eq!(host.stale_dropped(), 1);
}

// ---------------------------------------------------------------------------
// 3 — The two-stage Escape
// ---------------------------------------------------------------------------

#[test]
fn escape_with_the_flyout_open_closes_only_the_flyout() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.escape(&mut model, &mut focus);

    assert_eq!(names(&emissions), vec!["Dismissed", "OpenChanged"]);
    assert!(!host.is_open());
    assert_eq!(model.query, "do", "the text the user typed must survive");
    assert_eq!(model.highlighted_index, None);
    assert_eq!(
        model.suggestions.len(),
        3,
        "the list itself is not thrown away"
    );
}

#[test]
fn the_second_escape_clears_the_query() {
    let (mut host, mut model, mut focus) = opened();

    let _ = host.escape(&mut model, &mut focus);
    let cleared = host.escape(&mut model, &mut focus);

    assert_eq!(names(&cleared), vec!["QueryCleared"]);
    assert_eq!(model.query, "");
    assert!(model.suggestions.is_empty());
    assert_eq!(model.highlighted_index, None);
    assert!(!model.busy);
}

#[test]
fn the_third_escape_on_an_empty_closed_field_emits_nothing() {
    let (mut host, mut model, mut focus) = opened();

    let _ = host.escape(&mut model, &mut focus);
    let _ = host.escape(&mut model, &mut focus);
    let third = host.escape(&mut model, &mut focus);

    assert!(
        third.is_empty(),
        "with nothing to dismiss and nothing to clear the key belongs to the next rank"
    );
}

#[test]
fn escape_clears_rather_than_reverting_to_the_pre_edit_value() {
    // The recorded deviation from the plain field's Escape. The field
    // started empty, the user typed `report`, and Escape must empty it —
    // not restore some earlier snapshot.
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new().with_query("report");
    let _ = host.sync(&mut model, &mut focus);

    let _ = host.note_edit(&mut model, "repo".to_string(), T0);
    let cleared = host.escape(&mut model, &mut focus);

    assert_eq!(names(&cleared), vec!["QueryCleared"]);
    assert_eq!(model.query, "", "Escape must clear, not put `report` back");
}

#[test]
fn escape_from_a_flyout_never_clears_in_the_same_press() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.escape(&mut model, &mut focus);

    assert!(
        !emissions
            .iter()
            .any(|e| matches!(e, AutoSuggestEmission::QueryCleared(_))),
        "stage one must not also perform stage two"
    );
    assert_eq!(model.query, "do");
}

#[test]
fn escape_on_a_disabled_control_emits_nothing() {
    let (mut host, mut model, mut focus) = opened();
    model.disabled = true;

    assert!(host.escape(&mut model, &mut focus).is_empty());
}

// ---------------------------------------------------------------------------
// 4 — Dismissal stickiness
// ---------------------------------------------------------------------------

#[test]
fn a_dismissed_query_does_not_reopen_when_its_answer_arrives() {
    let (mut host, mut model, mut focus) = opened();

    // Typing on while the list is showing puts a *new* request in flight.
    let _ = host.note_edit(&mut model, "doc".to_string(), later(500));
    let fired = host.tick(&mut model, later(500 + DEBOUNCE_MS));
    let generation = requested(&fired).unwrap().0;

    // The user dismisses the list before that answer lands.
    let dismissed = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert_eq!(names(&dismissed), vec!["Dismissed", "OpenChanged"]);
    assert_eq!(host.dismissed_query(), Some("doc"));
    assert!(!host.is_open());

    // The answer for exactly the dismissed text arrives.
    let late = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);

    assert!(!host.is_open(), "the dismissed list must not come back");
    assert_eq!(model.suggestions.len(), 3, "the answer is still applied");
    assert!(!late.contains(&AutoSuggestEmission::OpenChanged(true)));
}

#[test]
fn a_dismissed_query_does_not_reopen_on_down_or_up() {
    let (mut host, mut model, mut focus) = opened();

    let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert!(
        host.key(&mut model, AutoSuggestKey::Down, &mut focus)
            .is_empty()
    );
    assert!(
        host.key(&mut model, AutoSuggestKey::Up, &mut focus)
            .is_empty()
    );
    assert!(!host.is_open());
}

#[test]
fn typing_a_different_query_re_arms_a_dismissed_box() {
    let (mut host, mut model, mut focus) = opened();

    let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert!(host.is_dismissed("do"));

    let _ = host.note_edit(&mut model, "doc".to_string(), later(500));
    assert!(!host.is_dismissed("doc"));

    let fired = host.tick(&mut model, later(500 + DEBOUNCE_MS));
    let generation = requested(&fired).unwrap().0;
    let _ = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);

    assert!(host.is_open(), "a new query is a new request for the list");
}

#[test]
fn a_commit_drops_the_dismissal_record() {
    let (mut host, mut model, mut focus) = opened();
    let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert_eq!(host.dismissed_query(), Some("do"));

    // A new query re-opens the box, and the old record is still held.
    let _ = host.note_edit(&mut model, "doc".to_string(), later(500));
    let fired = host.tick(&mut model, later(500 + DEBOUNCE_MS));
    let generation = requested(&fired).unwrap().0;
    let _ = host.apply_suggestions(&mut model, generation, suggestions(), &mut focus);
    assert!(host.is_open());
    assert_eq!(host.dismissed_query(), Some("do"));

    // Committing a candidate spends it.
    let _ = host.click(&mut model, 0, &mut focus);

    assert_eq!(host.dismissed_query(), None);
}

// ---------------------------------------------------------------------------
// 5 — Roving
// ---------------------------------------------------------------------------

#[test]
fn down_and_up_rove_the_highlight_and_report_each_step() {
    let (mut host, mut model, mut focus) = opened();
    assert_eq!(model.highlighted_index, Some(0));

    let down = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert_eq!(names(&down), vec!["HighlightChanged"]);
    assert_eq!(model.highlighted_index, Some(1));

    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert_eq!(model.highlighted_index, Some(2));

    let up = host.key(&mut model, AutoSuggestKey::Up, &mut focus);
    assert_eq!(model.highlighted_index, Some(1));
    assert_eq!(names(&up), vec!["HighlightChanged"]);
}

#[test]
fn the_rover_wraps_at_both_ends() {
    let (mut host, mut model, mut focus) = opened();

    // Down from the last row wraps to the first.
    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert_eq!(model.highlighted_index, Some(2));
    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert_eq!(
        model.highlighted_index,
        Some(0),
        "Down must wrap past the end"
    );

    // Up from the first row wraps to the last.
    let _ = host.key(&mut model, AutoSuggestKey::Up, &mut focus);
    assert_eq!(
        model.highlighted_index,
        Some(2),
        "Up must wrap past the top"
    );
}

#[test]
fn roving_leaves_the_typed_text_and_the_editing_session_alone() {
    // The spec's "without losing the input caret". Nothing in the roving
    // path settles the query, clears it, or issues an iced focus
    // operation, so the text the user typed is exactly where they left it.
    let (mut host, mut model, mut focus) = opened();

    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);

    assert_eq!(model.query, "do");
    assert!(host.is_open());
    assert!(!host.is_debouncing());
    assert_eq!(
        host.provider_passes(),
        1,
        "roving must not ask the provider again"
    );
}

#[test]
fn roving_reports_nothing_when_the_highlight_cannot_move() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();
    // Closed flyout, no suggestions: Down cannot rove anything.
    let emissions = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert!(emissions.is_empty());
}

// ---------------------------------------------------------------------------
// 6 — The three commit sources
// ---------------------------------------------------------------------------

#[test]
fn enter_on_a_highlighted_row_commits_with_the_enter_source() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);

    let commit = emissions
        .iter()
        .find_map(|e| match e {
            AutoSuggestEmission::Committed(commit) => Some(commit.clone()),
            _ => None,
        })
        .expect("Enter must commit the highlighted row");
    assert_eq!(commit.id, "doc-readme");
    assert_eq!(commit.source, CommitSource::Enter);
    assert_eq!(commit.label, "README.md");
}

#[test]
fn tab_on_a_highlighted_row_commits_with_the_tab_source() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.key(&mut model, AutoSuggestKey::Tab, &mut focus);

    let commit = emissions
        .iter()
        .find_map(|e| match e {
            AutoSuggestEmission::Committed(commit) => Some(commit.clone()),
            _ => None,
        })
        .expect("Tab must commit the highlighted row");
    assert_eq!(commit.source, CommitSource::Tab);
    assert_eq!(commit.id, "doc-readme");
}

#[test]
fn a_click_commits_with_the_click_source() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.click(&mut model, 1, &mut focus);

    let commit = emissions
        .iter()
        .find_map(|e| match e {
            AutoSuggestEmission::Committed(commit) => Some(commit.clone()),
            _ => None,
        })
        .expect("a click must commit");
    assert_eq!(commit.source, CommitSource::Click);
    assert_eq!(commit.id, "doc-notes");
}

#[test]
fn a_commit_settles_the_field_to_the_picked_label() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);

    assert_eq!(model.query, "README.md");
    assert!(
        emissions.contains(&AutoSuggestEmission::QueryCommitted(Commit::new(
            "README.md".to_string()
        ))),
        "the query's own commit tier must report the settled text"
    );
}

#[test]
fn a_commit_closes_the_flyout_and_drops_everything_in_flight() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);

    assert!(emissions.contains(&AutoSuggestEmission::OpenChanged(false)));
    assert!(!host.is_open());
    assert!(!host.is_busy());
    assert!(!host.is_debouncing());
    assert_eq!(host.in_flight_query(), None);
    assert!(!model.busy);
    assert_eq!(model.highlighted_index, None);
}

#[test]
fn a_commit_emits_the_suggestion_before_the_query_settle_and_the_close() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);

    assert_eq!(
        names(&emissions),
        vec!["Committed", "QueryCommitted", "OpenChanged"],
        "the intent comes first, then the settled text, then the state change"
    );
}

#[test]
fn a_click_on_a_row_that_no_longer_exists_commits_nothing() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.click(&mut model, 99, &mut focus);

    assert!(emissions.is_empty());
    assert!(host.is_open(), "a refused click must not close anything");
    assert_eq!(model.query, "do");
}

#[test]
fn a_click_with_no_flyout_open_commits_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();
    assert!(host.click(&mut model, 0, &mut focus).is_empty());
}

#[test]
fn enter_with_no_flyout_settles_the_query_without_a_candidate_commit() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let _ = host.note_edit(&mut model, "solo".to_string(), T0);
    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);

    assert_eq!(names(&emissions), vec!["QueryCommitted"]);
    assert_eq!(model.query, "solo");
    assert!(
        !emissions
            .iter()
            .any(|e| matches!(e, AutoSuggestEmission::Committed(_))),
        "no candidate was chosen, so no candidate commit may fire"
    );
}

#[test]
fn tab_with_no_flyout_open_emits_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();
    let _ = host.note_edit(&mut model, "solo".to_string(), T0);

    assert!(
        host.key(&mut model, AutoSuggestKey::Tab, &mut focus)
            .is_empty()
    );
}

#[test]
fn every_commit_source_is_recorded_as_a_commit_tier() {
    for source in [CommitSource::Enter, CommitSource::Click, CommitSource::Tab] {
        let commit = AutoSuggestCommit::new("x".to_string(), source, "X");
        let emission = AutoSuggestEmission::Committed(commit);
        assert_eq!(emission.tier(), "Commit");
        assert!(emission.settles());
    }
}

// ---------------------------------------------------------------------------
// 7 — No match
// ---------------------------------------------------------------------------

#[test]
fn a_query_with_no_suggestions_never_opens_a_flyout() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "zzz", T0);
    let generation = requested(&fired).unwrap().0;

    let applied = host.apply_suggestions(&mut model, generation, Vec::new(), &mut focus);

    assert!(!host.is_open());
    assert!(
        applied.is_empty(),
        "nothing appeared, so nothing is reported"
    );
    assert_eq!(model.highlighted_index, None);
}

#[test]
fn a_no_match_query_leaves_the_field_fully_usable() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "zzz", T0);
    let generation = requested(&fired).unwrap().0;
    let _ = host.apply_suggestions(&mut model, generation, Vec::new(), &mut focus);

    // Typing still reports, and Escape still clears.
    let typed = host.note_edit(&mut model, "zzzz".to_string(), later(500));
    assert_eq!(names(&typed), vec!["QueryChanged"]);

    let cleared = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
    assert_eq!(names(&cleared), vec!["QueryCleared"]);
    assert_eq!(model.query, "");
}

#[test]
fn down_on_a_no_match_query_opens_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let fired = type_and_wait(&mut host, &mut model, "zzz", T0);
    let generation = requested(&fired).unwrap().0;
    let _ = host.apply_suggestions(&mut model, generation, Vec::new(), &mut focus);

    assert!(
        host.key(&mut model, AutoSuggestKey::Down, &mut focus)
            .is_empty()
    );
    assert!(!host.is_open());
}

#[test]
fn a_flyout_whose_next_answer_is_empty_closes_and_reports() {
    let (mut host, mut model, mut focus) = opened();

    // Ask again, and this time the provider has nothing to give.
    let fired = type_and_wait(&mut host, &mut model, "doc-x", later(1_000));
    let generation = requested(&fired).unwrap().0;
    let applied = host.apply_suggestions(&mut model, generation, Vec::new(), &mut focus);

    assert!(!host.is_open());
    assert!(applied.contains(&AutoSuggestEmission::OpenChanged(false)));
    assert!(model.suggestions.is_empty());
    assert_eq!(model.highlighted_index, None);
}

// ---------------------------------------------------------------------------
// 8 — Focus
// ---------------------------------------------------------------------------

#[test]
fn losing_focus_closes_the_flyout_and_settles_the_query() {
    let (mut host, mut model, mut focus) = opened();

    let emissions = host.note_focus(&mut model, false, &mut focus);

    assert!(emissions.contains(&AutoSuggestEmission::OpenChanged(false)));
    assert!(emissions.contains(&AutoSuggestEmission::FocusChanged(false)));
    assert!(
        emissions.contains(&AutoSuggestEmission::QueryCommitted(Commit::new(
            "do".to_string()
        )))
    );
    assert!(!host.is_open());
    assert!(!host.is_busy());
}

#[test]
fn losing_focus_drops_the_dismissal_record() {
    let (mut host, mut model, mut focus) = opened();
    let _ = host.escape(&mut model, &mut focus);
    assert!(host.dismissed_query().is_some());

    let _ = host.note_focus(&mut model, false, &mut focus);

    assert_eq!(host.dismissed_query(), None);
    assert!(!host.is_debouncing());
    assert_eq!(host.in_flight_query(), None);
}

#[test]
fn gaining_focus_reports_the_edge_and_opens_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    let emissions = host.note_focus(&mut model, true, &mut focus);

    assert_eq!(names(&emissions), vec!["FocusChanged"]);
    assert!(!host.is_open());
}

#[test]
fn a_focus_edge_that_is_not_an_edge_emits_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    assert!(host.note_focus(&mut model, false, &mut focus).is_empty());
}

// ---------------------------------------------------------------------------
// 9 — The frame pass and teardown
// ---------------------------------------------------------------------------

#[test]
fn sync_never_opens_a_flyout_on_its_own() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new().with_suggestions(suggestions());

    let emissions = host.sync(&mut model, &mut focus);

    assert!(emissions.is_empty());
    assert!(
        !host.is_open(),
        "a maintenance pass must not invent an appearance"
    );
}

#[test]
fn sync_re_resolves_an_open_card_against_the_current_model() {
    let (mut host, mut model, mut focus) = opened();

    model.suggestions = vec![SuggestionItem::new("only".to_string(), "only.md")];
    let emissions = host.sync(&mut model, &mut focus);

    assert!(host.is_open());
    assert_eq!(model.highlighted_index, Some(0));
    assert!(emissions.is_empty() || emissions == vec![AutoSuggestEmission::HighlightChanged(0)]);
}

#[test]
fn sync_closes_a_card_the_model_has_made_unopenable() {
    let (mut host, mut model, mut focus) = opened();

    model.suggestions.clear();
    let emissions = host.sync(&mut model, &mut focus);

    assert!(!host.is_open());
    assert!(emissions.contains(&AutoSuggestEmission::OpenChanged(false)));
}

#[test]
fn sync_closes_a_card_at_a_query_that_was_dismissed() {
    let (mut host, mut model, mut focus) = opened();
    let _ = host.escape(&mut model, &mut focus);
    assert!(!host.is_open());

    // Reopening by hand is not possible from sync; assert it stays shut
    // while the dismissal holds.
    let emissions = host.sync(&mut model, &mut focus);
    assert!(emissions.is_empty());
    assert!(!host.is_open());
}

#[test]
fn sync_picks_up_an_externally_changed_query() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    model.query = "external".to_string();
    let _ = host.sync(&mut model, &mut focus);

    // The editing engine now agrees with the model, so a later commit
    // settles to the externally-set text rather than a stale one.
    let emissions = host.key(&mut model, AutoSuggestKey::Enter, &mut focus);
    assert_eq!(
        emissions,
        vec![AutoSuggestEmission::QueryCommitted(Commit::new(
            "external".to_string()
        ))]
    );
}

#[test]
fn reset_drops_timing_and_editing_state_but_leaves_an_open_flyout_alone() {
    let (mut host, model, focus) = opened();

    host.reset();

    assert!(!host.is_debouncing());
    assert_eq!(host.in_flight_query(), None);
    assert_eq!(host.dismissed_query(), None);
    assert_eq!(
        model.query, "do",
        "a reset must not rewrite the model's text"
    );
    assert!(
        focus.current().is_some(),
        "a reset must not silently move the keyboard"
    );
    assert!(
        host.is_open(),
        "closing a flyout is a reportable event; a silent reset must not drop that report"
    );
}

// ---------------------------------------------------------------------------
// 10 — Tier vocabulary
// ---------------------------------------------------------------------------

#[test]
fn every_emission_arm_names_a_tier_and_only_the_settling_ones_settle() {
    let all: Vec<AutoSuggestEmission<String>> = vec![
        AutoSuggestEmission::QueryChanged(Live::new("q".to_string())),
        AutoSuggestEmission::QueryRequested {
            generation: 1,
            query: "q".to_string(),
        },
        AutoSuggestEmission::HighlightChanged(0),
        AutoSuggestEmission::Committed(AutoSuggestCommit::new(
            "id".to_string(),
            CommitSource::Enter,
            "L",
        )),
        AutoSuggestEmission::QueryCommitted(Commit::new("q".to_string())),
        AutoSuggestEmission::QueryCleared(Commit::new(String::new())),
        AutoSuggestEmission::Dismissed,
        AutoSuggestEmission::OpenChanged(true),
        AutoSuggestEmission::FocusChanged(true),
    ];

    let settling: Vec<&str> = all
        .iter()
        .filter(|e| e.settles())
        .map(|e| e.name())
        .collect();
    assert_eq!(
        settling,
        vec!["Committed", "QueryCommitted", "QueryCleared"]
    );

    let tiers: Vec<&str> = all.iter().map(|e| e.tier()).collect();
    assert_eq!(
        tiers,
        vec![
            "Live", "Live", "Live", "Commit", "Commit", "Commit", "Dismiss", "Toggle", "Focus"
        ]
    );
}

#[test]
fn a_live_tier_never_equals_a_commit_tier_carrying_the_same_string() {
    let live: AutoSuggestEmission<String> =
        AutoSuggestEmission::QueryChanged(Live::new("same".to_string()));
    let commit: AutoSuggestEmission<String> =
        AutoSuggestEmission::QueryCommitted(Commit::new("same".to_string()));

    assert_ne!(live, commit);
}

// ---------------------------------------------------------------------------
// 10 — The machine's own read-out
//
// The identity, the counters and the in-flight text are the only way a host
// can see what the machine did on its behalf: how many provider passes it
// spent, which text is still waiting for an answer, which query the user
// dismissed, and which row the keyboard is on. Each claim below pins one of
// those read-outs against a transition the user can actually cause.
// ---------------------------------------------------------------------------

/// A `QueryChanged` over `text`, spelled out so the equality claims compare
/// like with like instead of leaving the candidate id type unresolved.
fn changed(text: &str) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::QueryChanged(Live::new(text.to_string()))
}

/// A `QueryRequested` for `generation` over `query`.
fn asked(generation: u64, query: &str) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::QueryRequested {
        generation,
        query: query.to_string(),
    }
}

/// A `HighlightChanged` for `row`.
fn roved(row: usize) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::HighlightChanged(row)
}

/// A `Committed` for candidate `id` under `source`.
fn picked(id: &str, source: CommitSource) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::Committed(AutoSuggestCommit::new(id.to_string(), source, "L"))
}

/// A `QueryCommitted` carrying `text`.
fn settled(text: &str) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::QueryCommitted(Commit::new(text.to_string()))
}

/// A `QueryCleared` carrying `text`.
fn cleared(text: &str) -> AutoSuggestEmission<String> {
    AutoSuggestEmission::QueryCleared(Commit::new(text.to_string()))
}

#[test]
fn a_host_can_be_built_at_its_own_debounce_window() {
    assert_eq!(
        AutoSuggestHost::<String>::new(control()).debounce_ms(),
        DEBOUNCE_MS,
        "a host built with new() trades at the documented window"
    );

    let mut host: AutoSuggestHost<String> = AutoSuggestHost::with_debounce(control(), 25);
    assert_eq!(
        host.debounce_ms(),
        25,
        "with_debounce() replaces the window the host trades at"
    );

    let mut model = AutoSuggestModel::new();
    let _ = host.note_edit(&mut model, "do".to_string(), T0);
    assert!(host.is_debouncing(), "the keystroke armed the window");

    let early = host.tick(&mut model, later(24));
    assert!(
        early.is_empty(),
        "one millisecond short of its own window the host must still be silent"
    );
    assert_eq!(host.provider_passes(), 0, "and must not have spent a pass");

    let fired = host.tick(&mut model, later(25));
    assert_eq!(
        requested(&fired),
        Some((1, "do".to_string())),
        "the request fires on the millisecond the window closes"
    );
    assert_eq!(
        host.provider_passes(),
        1,
        "and that millisecond is what spends the pass"
    );
}

#[test]
fn the_host_names_its_control_and_mints_one_card_identity_per_control() {
    let host = AutoSuggestHost::<String>::new(control());

    assert_eq!(
        host.control(),
        &control(),
        "control() reports the focusable the host was built for"
    );
    assert_eq!(
        host.menu_id(),
        &menu_id_for(&control()),
        "the card identity is derived from the control, never from the query"
    );

    let rebuilt = AutoSuggestHost::<String>::new(control());
    assert_eq!(
        rebuilt.menu_id(),
        host.menu_id(),
        "a rebuilt box claims the card its control already had"
    );

    let neighbour = AutoSuggestHost::<String>::new(FocusId::new("other-control"));
    assert_ne!(
        neighbour.menu_id(),
        host.menu_id(),
        "two boxes on one screen must not share one card"
    );
}

#[test]
fn a_fresh_host_reports_no_activity_at_all() {
    let (host, _focus) = pair();

    assert_eq!(host.generation(), 0, "no request has been numbered yet");
    assert_eq!(host.in_flight_query(), None, "nothing is with the provider");
    assert_eq!(host.dismissed_query(), None, "nothing has been dismissed");
    assert_eq!(host.stale_dropped(), 0, "nothing has been refused");
    assert_eq!(host.provider_passes(), 0, "no pass has been spent");
    assert!(!host.is_open(), "the flyout starts shut");
    assert!(!host.is_busy(), "the field starts idle");
    assert!(!host.is_debouncing(), "no window is armed");
    assert_eq!(host.highlighted(), None, "no row is highlighted");
    assert!(
        !host.is_dismissed("") && !host.is_dismissed("do"),
        "before the first dismissal no text at all is under one"
    );
}

#[test]
fn a_host_with_nothing_to_do_reports_nothing() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();

    assert!(
        host.tick(&mut model, later(DEBOUNCE_MS * 4)).is_empty(),
        "a frame with no window armed dispatches no request"
    );
    assert!(
        host.sync(&mut model, &mut focus).is_empty(),
        "the frame pass opens nothing while no card is open"
    );
    assert!(
        host.escape(&mut model, &mut focus).is_empty(),
        "Escape on an empty, shut field is not this control's key"
    );
    assert_eq!(
        host.provider_passes(),
        0,
        "none of the idle calls may spend a provider pass"
    );
    assert_eq!(
        host.generation(),
        0,
        "or number a request that was never made"
    );
}

#[test]
fn the_counters_track_every_pass_the_host_spends() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();
    let _ = host.note_focus(&mut model, true, &mut focus);

    let _ = host.note_edit(&mut model, "do".to_string(), T0);
    assert_eq!(
        host.generation(),
        0,
        "arming a window numbers nothing until it closes"
    );
    assert!(!host.is_busy(), "and puts nothing in flight");

    let _ = host.tick(&mut model, later(DEBOUNCE_MS));
    assert_eq!(host.generation(), 1, "the first request is number one");
    assert_eq!(host.provider_passes(), 1, "and it is one pass");
    assert!(host.is_busy(), "the field is waiting on the provider");
    assert_eq!(
        host.in_flight_query(),
        Some("do"),
        "and the text it waits on is readable"
    );

    let refused = host.apply_suggestions(&mut model, 7, suggestions(), &mut focus);
    assert!(
        refused.is_empty(),
        "an answer for a generation never asked for reports nothing"
    );
    assert_eq!(host.stale_dropped(), 1, "and is counted as a refusal");
    assert!(
        host.is_busy(),
        "a refusal leaves the real request still in flight"
    );
    assert_eq!(host.in_flight_query(), Some("do"));

    let applied = host.apply_suggestions(&mut model, 1, suggestions(), &mut focus);
    assert_eq!(names(&applied), vec!["OpenChanged", "HighlightChanged"]);
    assert!(!host.is_busy(), "the answer that was asked for clears busy");
    assert_eq!(
        host.in_flight_query(),
        None,
        "and empties the in-flight text"
    );
    assert_eq!(
        host.stale_dropped(),
        1,
        "a refusal already counted stays counted"
    );
    assert_eq!(
        host.provider_passes(),
        1,
        "answering a request is not the same as making one"
    );

    let _ = host.note_edit(&mut model, "doc".to_string(), later(DEBOUNCE_MS + 500));
    let _ = host.tick(&mut model, later(DEBOUNCE_MS * 2 + 500));
    assert_eq!(host.generation(), 2, "the next burst gets the next number");
    assert_eq!(host.provider_passes(), 2, "and spends a pass of its own");
    assert_eq!(
        host.in_flight_query(),
        Some("doc"),
        "with the text the user stopped on"
    );
}

#[test]
fn losing_focus_empties_the_request_in_flight_and_the_window() {
    let (mut host, mut focus) = pair();
    let mut model = AutoSuggestModel::new();
    let _ = host.note_focus(&mut model, true, &mut focus);
    let _ = host.note_edit(&mut model, "do".to_string(), T0);
    let _ = host.tick(&mut model, later(DEBOUNCE_MS));
    assert!(host.is_busy(), "precondition: a request is outstanding");
    assert_eq!(host.in_flight_query(), Some("do"));

    let out = host.note_focus(&mut model, false, &mut focus);

    assert!(
        names(&out).contains(&"QueryCommitted"),
        "leaving the field settles the text the user typed, saw {:?}",
        names(&out)
    );
    assert!(
        !host.is_busy(),
        "and the field stops waiting on a provider nobody is looking at"
    );
    assert_eq!(
        host.in_flight_query(),
        None,
        "the unanswered request is dropped rather than left to land later"
    );
    assert!(
        !host.is_debouncing(),
        "along with any window that was still armed"
    );
}

#[test]
fn the_dismissal_record_covers_exactly_the_text_that_was_dismissed() {
    let (mut host, mut model, mut focus) = opened();
    assert_eq!(
        host.dismissed_query(),
        None,
        "precondition: nothing has been dismissed"
    );
    assert!(!host.is_dismissed("do"));

    let out = host.escape(&mut model, &mut focus);

    assert_eq!(names(&out), vec!["Dismissed", "OpenChanged"]);
    assert_eq!(
        host.dismissed_query(),
        Some("do"),
        "the record names the query the user dismissed"
    );
    assert!(
        host.is_dismissed("do"),
        "that exact text is under the dismissal"
    );
    assert!(!host.is_dismissed("d"), "a prefix of it is not");
    assert!(
        !host.is_dismissed("dox"),
        "nor is a longer text that merely starts with it"
    );
    assert!(!host.is_dismissed(""), "and neither is the empty query");
}

#[test]
fn highlighted_reports_the_row_the_keyboard_is_on() {
    let (mut host, mut model, mut focus) = opened();
    assert_eq!(
        host.highlighted(),
        Some(0),
        "opening puts the keyboard on the first row"
    );
    assert_eq!(
        host.highlighted(),
        model.highlighted_index,
        "the read-out and the model's projection agree"
    );

    let _ = host.key(&mut model, AutoSuggestKey::Down, &mut focus);
    assert_eq!(host.highlighted(), Some(1), "and follows the rove");

    let _ = host.escape(&mut model, &mut focus);
    assert_eq!(
        host.highlighted(),
        None,
        "closing the card drops the highlight"
    );
    assert_eq!(
        model.highlighted_index, None,
        "and the model's projection goes with it"
    );
}

#[test]
fn reset_forgets_the_work_in_hand_and_not_the_history() {
    let (mut host, mut model, mut focus) = opened();
    let _ = host.apply_suggestions(&mut model, 42, Vec::new(), &mut focus);
    assert_eq!(host.stale_dropped(), 1, "precondition: one refusal counted");

    let passes = host.provider_passes();
    let generation = host.generation();
    host.reset();

    assert_eq!(
        host.provider_passes(),
        passes,
        "a reset cannot un-spend a pass that already happened"
    );
    assert_eq!(
        host.generation(),
        generation,
        "a reset cannot un-number a request that already went out"
    );
    assert_eq!(
        host.stale_dropped(),
        1,
        "a reset cannot un-count a refusal it did not make"
    );
    assert_eq!(
        host.in_flight_query(),
        None,
        "the request in flight is dropped"
    );
    assert_eq!(
        host.dismissed_query(),
        None,
        "and the dismissal record with it"
    );
    assert!(!host.is_debouncing(), "and the armed window");
}

#[test]
fn every_emission_arm_equals_only_itself_and_its_own_payload() {
    let all: Vec<AutoSuggestEmission<String>> = vec![
        changed("q"),
        asked(1, "q"),
        roved(0),
        picked("id", CommitSource::Enter),
        settled("q"),
        cleared(""),
        AutoSuggestEmission::Dismissed,
        AutoSuggestEmission::OpenChanged(true),
        AutoSuggestEmission::FocusChanged(true),
    ];

    for (this_index, this) in all.iter().enumerate() {
        for (that_index, that) in all.iter().enumerate() {
            if this_index == that_index {
                assert_eq!(
                    this,
                    that,
                    "a {} must equal another of its own kind",
                    this.name()
                );
            } else {
                assert_ne!(this, that, "a {} must never equal {that:?}", this.name());
            }
        }
    }
}

#[test]
fn two_emissions_of_the_same_arm_differ_only_by_what_they_carry() {
    assert_eq!(
        changed("a"),
        changed("a"),
        "two live ticks of the same text are the same event"
    );
    assert_ne!(
        changed("a"),
        changed("b"),
        "a live tick of other text is a different event"
    );

    assert_eq!(
        asked(1, "do"),
        asked(1, "do"),
        "the same request described twice is the same request"
    );
    assert_ne!(
        asked(1, "do"),
        asked(2, "do"),
        "a later generation over the same text is a different request"
    );
    assert_ne!(
        asked(1, "do"),
        asked(1, "dox"),
        "and so is the same generation over other text"
    );

    assert_eq!(roved(1), roved(1), "the same row twice is the same rove");
    assert_ne!(roved(1), roved(2), "another row is another rove");

    assert_eq!(
        picked("a", CommitSource::Enter),
        picked("a", CommitSource::Enter),
        "the same candidate and source twice is the same commit"
    );
    assert_ne!(
        picked("a", CommitSource::Enter),
        picked("b", CommitSource::Enter),
        "another candidate is another commit"
    );
    assert_ne!(
        picked("a", CommitSource::Enter),
        picked("a", CommitSource::Tab),
        "another source is another commit"
    );
    assert_ne!(
        picked("a", CommitSource::Enter),
        picked("a", CommitSource::Click),
        "and a third source a third commit"
    );

    assert_eq!(settled("q"), settled("q"), "the same settle twice matches");
    assert_ne!(
        settled("q"),
        settled("r"),
        "a settle of other text does not"
    );
    assert_eq!(
        cleared(""),
        cleared(""),
        "two clears of the empty string match"
    );
    assert_ne!(
        cleared(""),
        cleared("q"),
        "a clear that carried other text is another event"
    );

    let open: AutoSuggestEmission<String> = AutoSuggestEmission::OpenChanged(true);
    let shut: AutoSuggestEmission<String> = AutoSuggestEmission::OpenChanged(false);
    assert_eq!(open, open, "the same flyout edge twice matches");
    assert_ne!(open, shut, "appearing is not the same event as going away");

    let gained: AutoSuggestEmission<String> = AutoSuggestEmission::FocusChanged(true);
    let lost: AutoSuggestEmission<String> = AutoSuggestEmission::FocusChanged(false);
    assert_eq!(gained, gained, "the same focus edge twice matches");
    assert_ne!(lost, gained, "losing focus is not gaining it");

    let dismissed: AutoSuggestEmission<String> = AutoSuggestEmission::Dismissed;
    assert_eq!(dismissed, dismissed, "a dismissal always equals another");
}
