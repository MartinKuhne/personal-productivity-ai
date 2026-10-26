//! Tests for the text field's editing engine (`UIF-ATOM-002a`, §3.0.4.2).
//!
//! The claims under test are the ones a reader cannot get from looking at
//! the type: that the snapshot is taken at the *first* edit rather than at
//! focus, that a settled tier supersedes a pending live tick rather than
//! trailing it, and that a clear is not a commit of an empty string that
//! merely looks the same.

use crate::binding::Cancel;
use crate::binding::Commit;
use crate::binding::GestureStart;
use crate::binding::Live;
use crate::widget::text_input_controller::TextInputController;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::TextInputModel;

/// Drive the engine the way the widget does: observe the projection first.
fn engine_with(value: &str) -> TextInputController {
    let mut engine = TextInputController::new();
    engine.observe(&TextInputModel::new(value));
    engine
}

fn changed(value: &str) -> TextInputEmission {
    TextInputEmission::Changed(Live::new(value.to_owned()))
}

fn committed(value: &str) -> TextInputEmission {
    TextInputEmission::Committed(Commit::new(value.to_owned()))
}

fn focus_committed(value: &str) -> TextInputEmission {
    TextInputEmission::FocusCommitted(Commit::new(value.to_owned()))
}

fn cleared() -> TextInputEmission {
    TextInputEmission::Cleared(Commit::new(String::new()))
}

fn cancelled(restored: &str) -> TextInputEmission {
    TextInputEmission::Cancelled(Cancel::new(restored.to_owned()))
}

fn focused(on: bool) -> TextInputEmission {
    TextInputEmission::FocusChanged(on)
}

// -------------------------------------------------------------------
// Starting state
// -------------------------------------------------------------------

#[test]
fn a_fresh_engine_holds_nothing() {
    let engine = TextInputController::new();

    assert_eq!(engine.value(), "");
    assert!(!engine.session_open());
    assert_eq!(engine.snapshot(), None);
    assert!(!engine.live_pending());
    assert!(!engine.is_focused());
    assert_eq!(engine.coalesced(), 0);
    assert_eq!(engine.superseded(), 0);
    assert_eq!(engine.clears(), 0);
    assert_eq!(engine.last_begin(), None);
}

#[test]
fn an_engine_adopted_mid_edit_starts_from_the_value_it_was_handed() {
    let engine = TextInputController::with_value("already here");

    assert_eq!(engine.value(), "already here");
    assert!(
        !engine.session_open(),
        "adopting a value is not starting a gesture: the first `Escape` \
         after adoption must revert to the adopted value, not to nothing"
    );
}

// -------------------------------------------------------------------
// `observe`: sync while idle, freeze while editing
// -------------------------------------------------------------------

#[test]
fn observing_syncs_the_engine_while_no_session_is_open() {
    let mut engine = TextInputController::new();

    engine.observe(&TextInputModel::new("host says hello"));

    assert_eq!(engine.value(), "host says hello");
}

#[test]
fn observing_does_not_overwrite_a_value_the_user_is_editing() {
    let mut engine = engine_with("start");
    engine.note_edit("star".to_owned());

    engine.observe(&TextInputModel::new("something the host pushed mid-edit"));

    assert_eq!(
        engine.value(),
        "star",
        "overwriting mid-session would make a later `Escape` restore a \
         value the user never typed over"
    );
}

#[test]
fn observing_resumes_syncing_once_the_session_closes() {
    let mut engine = engine_with("start");
    engine.note_edit("sta".to_owned());
    engine.commit();

    engine.observe(&TextInputModel::new("host value after settle"));

    assert_eq!(engine.value(), "host value after settle");
}

// -------------------------------------------------------------------
// The snapshot is taken at the first edit
// -------------------------------------------------------------------

#[test]
fn the_first_edit_opens_a_session_snapshotting_the_pre_edit_value() {
    let mut engine = engine_with("hello");

    assert!(engine.note_edit("hell".to_owned()));
    assert!(engine.session_open());
    assert_eq!(
        engine.snapshot(),
        Some("hello"),
        "the snapshot must be the value from *before* the edit, not after"
    );
    assert_eq!(engine.last_begin(), Some(GestureStart::Started));
}

#[test]
fn a_second_edit_cannot_move_the_snapshot() {
    let mut engine = engine_with("hello");
    engine.note_edit("hell".to_owned());
    engine.note_edit("he".to_owned());
    engine.note_edit("h".to_owned());

    assert_eq!(
        engine.snapshot(),
        Some("hello"),
        "outermost wins: three edits in, `Escape` still restores what the \
         user last saw before they started"
    );
    assert_eq!(engine.last_begin(), Some(GestureStart::AlreadyOpen));
    assert_eq!(engine.value(), "h");
}

#[test]
fn an_edit_that_changes_nothing_records_nothing() {
    let mut engine = engine_with("same");

    assert!(
        !engine.note_edit("same".to_owned()),
        "a keystroke that did not change the value is not a change"
    );
    assert!(
        !engine.session_open(),
        "a no-op edit must not open a session, or `Escape` would appear to \
         revert to the value the field already had"
    );
    assert!(!engine.live_pending());
}

#[test]
fn editing_from_empty_opens_a_session_snapshotting_empty() {
    let mut engine = engine_with("");

    assert!(engine.note_edit("f".to_owned()));
    assert_eq!(engine.snapshot(), Some(""));
}

// -------------------------------------------------------------------
// The live tier is coalesced
// -------------------------------------------------------------------

#[test]
fn one_edit_queues_exactly_one_tick_and_take_empties_it() {
    let mut engine = engine_with("");
    engine.note_edit("a".to_owned());

    assert!(engine.live_pending());
    assert_eq!(engine.take_live(), Some("a".to_owned()));
    assert!(!engine.live_pending());
    assert_eq!(engine.take_live(), None);
}

#[test]
fn three_undelivered_edits_become_the_newest_one() {
    let mut engine = engine_with("");
    engine.note_edit("a".to_owned());
    engine.note_edit("ab".to_owned());
    engine.note_edit("abc".to_owned());

    assert_eq!(engine.take_live(), Some("abc".to_owned()));
    assert_eq!(
        engine.coalesced(),
        2,
        "two pending values were displaced; a counter that never moves means \
         the coalescer never ran"
    );
    assert_eq!(engine.take_live(), None);
}

#[test]
fn delivering_every_pass_never_counts_a_coalescing() {
    let mut engine = engine_with("");

    for value in ["a", "ab", "abc"] {
        engine.note_edit(value.to_owned());
        assert_eq!(engine.take_live(), Some(value.to_owned()));
    }

    assert_eq!(
        engine.coalesced(),
        0,
        "nothing is ever displaced when every tick is taken before the \
         next edit arrives"
    );
}

// -------------------------------------------------------------------
// Settling
// -------------------------------------------------------------------

#[test]
fn commit_settles_the_pending_value_and_closes_the_session() {
    let mut engine = engine_with("hello");
    engine.note_edit("help".to_owned());

    let emission = engine.commit();

    assert_eq!(emission, Some(committed("help")));
    assert!(!engine.session_open());
    assert_eq!(engine.snapshot(), None);
}

#[test]
fn commit_supersedes_a_pending_live_tick() {
    let mut engine = engine_with("hello");
    engine.note_edit("help".to_owned());

    engine.commit();

    assert_eq!(
        engine.superseded(),
        1,
        "the host has been handed `Commit(\"help\")`; it has no use for a \
         `Live(\"help\")` arriving afterwards"
    );
    assert_eq!(engine.take_live(), None);
}

#[test]
fn a_commit_with_no_session_still_settles_the_current_value() {
    let mut engine = engine_with("untouched");

    let emission = engine.commit();

    assert_eq!(
        emission,
        Some(committed("untouched")),
        "`Enter` means commit what is in the field, whether or not this \
         session changed it"
    );
    assert!(!engine.session_open());
    assert_eq!(
        engine.superseded(),
        0,
        "there was nothing pending, so nothing was superseded"
    );
}

#[test]
fn a_settled_session_cannot_also_be_cancelled() {
    let mut engine = engine_with("a");
    engine.note_edit("ab".to_owned());
    engine.commit();

    assert_eq!(
        engine.revert(),
        None,
        "the gesture is spent: the two settling paths are mutually \
         exclusive, so a commit cannot be walked back with a cancel"
    );
    assert_eq!(engine.value(), "ab", "the committed value stands");
}

#[test]
fn revert_restores_the_snapshot_and_reports_what_it_restored() {
    let mut engine = engine_with("hello");
    engine.note_edit("hello world".to_owned());

    let emission = engine.revert();

    assert_eq!(emission, Some(cancelled("hello")));
    assert_eq!(engine.value(), "hello");
    assert!(!engine.session_open());
    assert_eq!(engine.snapshot(), None);
}

#[test]
fn revert_supersedes_a_pending_live_tick() {
    let mut engine = engine_with("hello");
    engine.note_edit("hell".to_owned());

    engine.revert();

    assert_eq!(engine.superseded(), 1);
    assert_eq!(engine.take_live(), None);
}

#[test]
fn revert_with_no_session_is_not_an_event() {
    let mut engine = engine_with("hello");

    assert_eq!(
        engine.revert(),
        None,
        "with nothing open there is nothing to revert; the caller decides \
         whether that is a clear instead"
    );
    assert_eq!(engine.value(), "hello");
}

// -------------------------------------------------------------------
// Clearing
// -------------------------------------------------------------------

#[test]
fn clear_empties_the_field_and_counts_itself() {
    let mut engine = engine_with("query");

    let emission = engine.clear();

    assert_eq!(emission, Some(cleared()));
    assert_eq!(engine.value(), "");
    assert_eq!(engine.clears(), 1);
    assert!(!engine.session_open());
}

#[test]
fn clear_closes_the_session_rather_than_committing_it() {
    let mut engine = engine_with("query");
    engine.note_edit("que".to_owned());

    engine.clear();

    assert!(
        !engine.session_open(),
        "leaving a session open after a clear would hand a later `Escape` a \
         snapshot of what the user just threw away"
    );
    assert_eq!(engine.snapshot(), None);
    assert_eq!(
        engine.superseded(),
        1,
        "the pending `Live(\"que\")` is dropped by the clear, not delivered"
    );
}

#[test]
fn clearing_an_empty_field_is_not_an_event() {
    let mut engine = engine_with("");

    assert_eq!(engine.clear(), None);
    assert_eq!(engine.clears(), 0);
}

#[test]
fn a_clear_is_distinguishable_from_a_typed_away_empty_value() {
    let mut cleared_engine = engine_with("x");
    let clear_event = cleared_engine.clear();

    let mut typed_engine = engine_with("x");
    typed_engine.note_edit(String::new());
    let typed_event = typed_engine.commit();

    assert_eq!(clear_event.as_ref().and_then(|e| e.value()), Some(""));
    assert_eq!(typed_event.as_ref().and_then(|e| e.value()), Some(""));
    assert_ne!(
        clear_event, typed_event,
        "two events carry the same empty string and mean different things; \
         the sum type is what keeps them apart"
    );
}

// -------------------------------------------------------------------
// The focus edge
// -------------------------------------------------------------------

#[test]
fn focus_gain_reports_the_edge_and_takes_no_snapshot() {
    let mut engine = engine_with("hello");

    let emissions = engine.note_focus(true);

    assert_eq!(emissions, vec![focused(true)]);
    assert!(
        !engine.session_open(),
        "focus-gain is not a gesture: taking a snapshot here would make \
         every `Tab` through a form a revertable session"
    );
    assert_eq!(engine.snapshot(), None);
}

#[test]
fn focus_loss_settles_an_open_session_before_reporting_the_edge() {
    let mut engine = engine_with("hello");
    engine.note_focus(true);
    engine.note_edit("hell".to_owned());

    let emissions = engine.note_focus(false);

    assert_eq!(
        emissions,
        vec![focus_committed("hell"), focused(false)],
        "the host must see the settled value before it sees that the field \
         is gone"
    );
    assert!(!engine.session_open());
    assert_eq!(engine.superseded(), 1);
}

#[test]
fn focus_loss_with_no_open_session_commits_nothing() {
    let mut engine = engine_with("hello");
    engine.note_focus(true);

    let emissions = engine.note_focus(false);

    assert_eq!(emissions, vec![focused(false)]);
    assert_eq!(
        engine.superseded(),
        0,
        "the user focused the field and left it alone; a persistence layer \
         must not be woken with a no-op write"
    );
}

#[test]
fn a_focus_state_that_did_not_change_emits_nothing() {
    let mut engine = TextInputController::new();

    assert!(engine.note_focus(false).is_empty());
    engine.note_focus(true);
    assert!(engine.note_focus(true).is_empty());
}

#[test]
fn a_session_survives_a_redundant_focus_pass_and_can_be_reverted_later() {
    let mut engine = engine_with("hello");
    engine.note_focus(true);
    engine.note_edit("help".to_owned());
    engine.note_focus(true);

    assert_eq!(engine.snapshot(), Some("hello"));
    assert_eq!(engine.revert(), Some(cancelled("hello")));
}

// -------------------------------------------------------------------
// Teardown
// -------------------------------------------------------------------

#[test]
fn reset_drops_the_session_and_the_pending_tick_without_emitting() {
    let mut engine = engine_with("hello");
    engine.note_focus(true);
    engine.note_edit("hello world".to_owned());

    engine.reset();

    assert!(!engine.session_open());
    assert_eq!(engine.snapshot(), None);
    assert!(!engine.live_pending());
    assert!(!engine.is_focused());
    assert_eq!(engine.last_begin(), None);
    assert_eq!(
        engine.value(),
        "hello world",
        "reset forgets the session, not the value: the value belongs to the \
         model, and the next `observe` will resync it"
    );
}

#[test]
fn reset_keeps_the_history_counters() {
    let mut engine = engine_with("a");
    engine.note_edit("ab".to_owned());
    engine.note_edit("abc".to_owned());
    engine.commit();
    engine.clear();
    let before = (engine.coalesced(), engine.superseded(), engine.clears());

    engine.reset();

    assert_eq!(
        (engine.coalesced(), engine.superseded(), engine.clears()),
        before,
        "how many ticks were displaced is history, not live state"
    );
}

#[test]
fn resetting_an_idle_engine_is_well_defined() {
    let mut engine = TextInputController::new();
    engine.reset();

    assert_eq!(engine, TextInputController::new());
}

// -------------------------------------------------------------------
// The emission sum type
// -------------------------------------------------------------------

#[test]
fn every_tier_names_its_tier_and_carries_its_value() {
    assert_eq!(changed("a").tier(), "live");
    assert_eq!(committed("b").tier(), "commit");
    assert_eq!(focus_committed("b").tier(), "commit");
    assert_eq!(cleared().tier(), "commit");
    assert_eq!(cancelled("c").tier(), "cancel");
    assert_eq!(focused(true).tier(), "focus");

    assert_eq!(changed("a").value(), Some("a"));
    assert_eq!(committed("b").value(), Some("b"));
    assert_eq!(focus_committed("b").value(), Some("b"));
    assert_eq!(cleared().value(), Some(""));
    assert_eq!(cancelled("c").value(), Some("c"));
    assert_eq!(focused(true).value(), None);
}

#[test]
fn only_the_settling_arms_close_a_session() {
    assert!(!changed("a").settles());
    assert!(!focused(true).settles());
    assert!(committed("b").settles());
    assert!(focus_committed("b").settles());
    assert!(cleared().settles());
    assert!(cancelled("c").settles());
}

#[test]
fn tiers_of_different_kinds_never_compare_equal() {
    assert_ne!(changed("x"), committed("x"));
    assert_ne!(committed("x"), focus_committed("x"));
    assert_ne!(focus_committed("x"), focus_committed("y"));
    assert_ne!(committed("x"), cancelled("x"));
    assert_ne!(committed("x"), cleared());
    assert_ne!(cancelled("x"), cleared());
}

#[test]
fn same_tier_same_payload_compares_equal() {
    assert_eq!(committed("x"), committed("x"));
    assert_ne!(committed("x"), committed("y"));
    assert_eq!(focus_committed("x"), focus_committed("x"));
    assert_eq!(focused(true), focused(true));
    assert_ne!(focused(true), focused(false));
    assert_eq!(changed("x"), changed("x"));
    assert_eq!(cleared(), cleared());
}

#[test]
fn a_tier_debug_prints_its_payload_not_its_wrapper() {
    assert_eq!(format!("{:?}", committed("v")), "Committed(\"v\")");
    assert_eq!(
        format!("{:?}", focus_committed("v")),
        "FocusCommitted(\"v\")"
    );
    assert_eq!(format!("{:?}", cleared()), "Cleared(\"\")");
    assert_eq!(format!("{:?}", cancelled("v")), "Cancelled(\"v\")");
    assert_eq!(format!("{:?}", focused(true)), "FocusChanged(true)");
}

// -------------------------------------------------------------------
// Controller equality
// -------------------------------------------------------------------

#[test]
fn controllers_equal_their_observable_shape() {
    let mut a = engine_with("v");
    let mut b = engine_with("v");

    assert_eq!(a, b);

    a.note_edit("va".to_owned());
    assert_ne!(a, b);

    b.note_edit("vb".to_owned());
    assert_ne!(a, b, "different values, different shape");
}

#[test]
fn equality_tracks_whether_a_session_is_open_not_just_the_current_value() {
    let mut a = engine_with("hello");
    a.note_edit("hell".to_owned());

    let mut b = engine_with("hello");
    b.note_edit("hell".to_owned());

    assert_eq!(a, b, "same value, same session state, same counters");

    let idle = engine_with("hell");
    assert_ne!(
        a, idle,
        "both hold \"hell\", but one is mid-gesture and the other is idle; \
         a test that could not tell those apart would not be proving the \
         session is state at all"
    );
}

// -- advances_stance: the tier-to-history bridge (UIF-PRIN-009) -------------

#[test]
fn a_committed_or_cleared_value_advances_the_stance() {
    assert!(TextInputEmission::Committed(Commit::new("v".to_owned())).advances_stance());
    assert!(focus_committed("v").advances_stance());
    assert!(TextInputEmission::Cleared(Commit::new(String::new())).advances_stance());
}

#[test]
fn focus_loss_advances_the_stance_and_focus_gain_does_not() {
    assert!(TextInputEmission::FocusChanged(false).advances_stance());
    assert!(!TextInputEmission::FocusChanged(true).advances_stance());
}

#[test]
fn a_live_tick_never_advances_the_stance() {
    for value in ["", "one keystroke", "the whole buffer"] {
        assert!(
            !TextInputEmission::Changed(Live::new(value.to_owned())).advances_stance(),
            "a keystroke declared the user finished with the field"
        );
    }
}

#[test]
fn a_cancel_never_advances_the_stance() {
    assert!(
        !TextInputEmission::Cancelled(Cancel::new("restored".to_owned())).advances_stance(),
        "backing out of an edit must not count as committing to it"
    );
}

#[test]
fn every_tier_is_accounted_for_in_the_stance_rule() {
    let tiers = [
        TextInputEmission::Changed(Live::new("a".to_owned())),
        TextInputEmission::Committed(Commit::new("a".to_owned())),
        focus_committed("a"),
        TextInputEmission::Cleared(Commit::new(String::new())),
        TextInputEmission::Cancelled(Cancel::new("a".to_owned())),
        TextInputEmission::FocusChanged(true),
        TextInputEmission::FocusChanged(false),
    ];
    let advancing: Vec<&str> = tiers
        .iter()
        .filter(|emission| emission.advances_stance())
        .map(TextInputEmission::tier)
        .collect();
    assert_eq!(advancing, vec!["commit", "commit", "commit", "focus"]);
}

#[test]
fn a_host_wired_by_advances_stance_gets_the_pillar_right() {
    let mut model = TextInputModel::empty();
    let mut engine = TextInputController::new();

    engine
        .note_edit("dup".to_owned())
        .then(|| model.value = engine.take_live().expect("a live tick"));
    model = model.with_validation("That name is taken.");
    assert!(
        model.is_pristine(),
        "typing must not commit the user to the field"
    );
    assert!(
        !model.punishment().painted(),
        "a field the user is still typing in must not go red"
    );

    let emission = engine.commit().expect("Enter settles");
    if emission.advances_stance() {
        model.note_commit();
    }
    assert!(
        model.punishment().painted(),
        "after a commit the invalidity shows"
    );
}

#[test]
fn escaping_a_bad_edit_leaves_the_field_pristine_and_unpunished() {
    let mut model = TextInputModel::new("kept");
    let mut engine = TextInputController::with_value("kept");

    engine.note_edit("kept but wrong".to_owned());
    model.value = engine.take_live().expect("a live tick");
    model.validation = Some("nope".to_owned());
    assert!(!model.punishment().painted());

    let emission = engine.revert().expect("Escape reverts the session");
    assert!(
        !emission.advances_stance(),
        "Escape must not declare the user finished, or the restored value \
         would go red the moment they tried to back out"
    );
    model.value = emission.value().expect("a restored value").to_owned();
    model.validation = None;
    assert!(model.is_pristine());
    assert!(!model.punishment().painted());
}
