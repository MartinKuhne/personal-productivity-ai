//! Tests for the pre-gesture snapshot and the linear gesture guard (§3.0.4.2).
//!
//! The three failure modes this file exists to catch are the ones that make
//! `on_cancel()` a lie: a snapshot that aliases live data, a nested gesture that
//! silently shortens the outer revert, and a slot that settles twice.

use crate::binding::snapshot::Gesture;
use crate::binding::snapshot::GestureSlot;
use crate::binding::snapshot::GestureStart;
use crate::binding::snapshot::Snapshot;

fn text(value: &str) -> String {
    value.to_string()
}

// ---------------------------------------------------------------------------
// Snapshot: retained by value
// ---------------------------------------------------------------------------

#[test]
fn a_snapshot_keeps_the_value_it_captured_even_after_the_source_changes() {
    let mut live = text("original");
    let snapshot = Snapshot::capture(&live);
    live.push_str(" mutated");
    assert_eq!(
        snapshot.value().as_str(),
        "original",
        "a snapshot must not alias"
    );
}

#[test]
fn a_snapshot_can_be_taken_into_ownership() {
    let snapshot = Snapshot::capture(&vec![1, 2, 3]);
    assert_eq!(snapshot.into_value(), vec![1, 2, 3]);
}

#[test]
fn matches_reports_whether_a_value_still_equals_the_snapshot() {
    let snapshot = Snapshot::capture(&text("hello"));
    assert!(snapshot.matches(&text("hello")));
    assert!(!snapshot.matches(&text("hellp")));
}

#[test]
fn a_snapshot_of_an_empty_value_is_still_a_usable_revert_target() {
    let snapshot = Snapshot::capture(&String::new());
    assert_eq!(snapshot.value().as_str(), "");
    assert!(snapshot.matches(&String::new()));
    assert!(!snapshot.matches(&text("typed")));
}

#[test]
fn a_snapshot_renders_its_value_in_debug() {
    assert!(format!("{:?}", Snapshot::capture(&text("visible"))).contains("visible"));
}

// ---------------------------------------------------------------------------
// Gesture: linear by construction
// ---------------------------------------------------------------------------

#[test]
fn a_guard_exposes_the_value_it_will_revert_to() {
    let gesture = Gesture::new(Snapshot::capture(&text("before")));
    assert_eq!(gesture.snapshot().as_str(), "before");
}

#[test]
fn a_guard_emits_any_number_of_live_ticks_without_being_consumed() {
    let gesture = Gesture::new(Snapshot::capture(&text("start")));
    for index in 0..50 {
        let live = gesture.live(text(&format!("tick{index}")));
        assert_eq!(live.value().as_str(), format!("tick{index}"));
    }
    // Still here, still able to settle — observing never settles anything.
    assert_eq!(gesture.commit(text("end")).into_value(), "end");
}

#[test]
fn committing_hands_back_the_settled_value_not_the_snapshot() {
    let gesture = Gesture::new(Snapshot::capture(&text("old")));
    let commit = gesture.commit(text("new"));
    assert_eq!(commit.into_value(), "new");
}

#[test]
fn cancelling_hands_back_the_snapshot_rather_than_the_last_live_value() {
    let gesture = Gesture::new(Snapshot::capture(&text("old")));
    let _ = gesture.live(text("half-typed"));
    let cancel = gesture.cancel();
    assert_eq!(
        cancel.restored().as_str(),
        "old",
        "cancel reverts, it does not keep"
    );
    assert_eq!(cancel.into_restored(), "old");
}

#[test]
fn a_guard_renders_its_snapshot_in_debug() {
    let gesture = Gesture::new(Snapshot::capture(&text("visible")));
    assert!(format!("{:?}", gesture).contains("visible"));
}

// ---------------------------------------------------------------------------
// GestureSlot: the nesting and settling discipline
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_slot_is_closed_and_owes_nothing() {
    let mut slot = GestureSlot::<String>::new();
    assert!(!slot.is_open());
    assert!(slot.live(text("x")).is_none());
    assert!(slot.commit(text("x")).is_none());
    assert!(slot.cancel().is_none());
}

#[test]
fn the_default_slot_behaves_like_a_fresh_one() {
    let mut slot = GestureSlot::<String>::default();
    assert!(!slot.is_open());
    assert_eq!(slot.begin(&text("v")), GestureStart::Started);
}

#[test]
fn beginning_a_gesture_opens_the_slot() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("hello")), GestureStart::Started);
    assert!(slot.is_open());
}

#[test]
fn a_nested_begin_is_refused_and_the_outer_snapshot_still_governs() {
    // The nesting rule: re-capturing on the inner `begin` would silently shorten
    // the outer revert to an intermediate the user never agreed to.
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("original")), GestureStart::Started);
    assert_eq!(slot.begin(&text("intermediate")), GestureStart::AlreadyOpen);
    let cancel = slot.cancel().expect("the outer gesture is still open");
    assert_eq!(cancel.restored().as_str(), "original");
}

#[test]
fn a_refused_nested_begin_leaves_the_slot_open_rather_than_closing_it() {
    let mut slot = GestureSlot::<u32>::new();
    assert_eq!(slot.begin(&10), GestureStart::Started);
    assert_eq!(slot.begin(&20), GestureStart::AlreadyOpen);
    assert!(slot.is_open());
    assert_eq!(slot.commit(30).map(|commit| commit.into_value()), Some(30));
}

#[test]
fn a_gesture_round_trips_live_ticks_then_cancels_to_the_snapshot() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("hello")), GestureStart::Started);
    assert!(slot.live(text("h")).is_some());
    assert!(slot.live(text("he")).is_some());
    assert!(slot.live(text("hello world")).is_some());
    let cancel = slot.cancel().expect("open");
    assert_eq!(cancel.restored().as_str(), "hello");
    assert!(!slot.is_open(), "cancelling closes the slot");
}

#[test]
fn a_gesture_round_trips_live_ticks_then_commits_the_last_one() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("hello")), GestureStart::Started);
    assert!(slot.live(text("hell")).is_some());
    assert!(slot.live(text("help")).is_some());
    let commit = slot.commit(text("help")).expect("open");
    assert_eq!(commit.into_value(), "help");
    assert!(!slot.is_open());
}

#[test]
fn settling_twice_through_the_slot_cannot_re_emit() {
    // The guard itself makes a double commit a compile error; this covers the
    // storage layer, where the second call must find nothing to settle.
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("a")), GestureStart::Started);
    assert!(slot.commit(text("b")).is_some());
    assert!(
        slot.commit(text("c")).is_none(),
        "the slot is already settled"
    );
    assert!(slot.cancel().is_none(), "cancel after commit emits nothing");
}

#[test]
fn cancelling_then_committing_through_the_slot_cannot_re_emit() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("a")), GestureStart::Started);
    assert!(slot.cancel().is_some());
    assert!(slot.commit(text("b")).is_none());
    assert!(slot.cancel().is_none());
}

#[test]
fn aborting_drops_the_gesture_without_owing_either_tier() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("a")), GestureStart::Started);
    slot.abort();
    assert!(!slot.is_open());
    assert!(slot.cancel().is_none(), "no revert is owed after an abort");
    assert!(
        slot.commit(text("b")).is_none(),
        "no commit is owed after an abort"
    );
}

#[test]
fn aborting_a_closed_slot_is_harmless() {
    let mut slot = GestureSlot::<String>::new();
    slot.abort();
    slot.abort();
    assert!(!slot.is_open());
}

#[test]
fn a_slot_can_be_reused_for_a_fresh_gesture_after_settling() {
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("first")), GestureStart::Started);
    slot.commit(text("first edited"));
    assert_eq!(slot.begin(&text("second")), GestureStart::Started);
    let cancel = slot.cancel().expect("the new gesture is open");
    assert_eq!(
        cancel.restored().as_str(),
        "second",
        "the new snapshot governs"
    );
}

#[test]
fn a_stale_snapshot_cannot_be_reused_after_its_gesture_settled() {
    // The stale-snapshot failure: holding on to the captured value and trying to
    // revert with it later must not be possible through the slot, because the
    // slot no longer has a gesture to revert.
    let mut slot = GestureSlot::<String>::new();
    assert_eq!(slot.begin(&text("stale")), GestureStart::Started);
    slot.commit(text("settled"));
    assert!(
        slot.cancel().is_none(),
        "a settled gesture has no revert left to hand out"
    );
}

#[test]
fn a_cancel_reports_a_no_op_revert_as_matching_the_current_value() {
    // Escape with nothing typed should not wake a persistence layer.
    let mut slot = GestureSlot::<String>::new();
    let current = text("unchanged");
    assert_eq!(slot.begin(&current), GestureStart::Started);
    let cancel = slot.cancel().expect("open");
    let snapshot = Snapshot::capture(cancel.restored());
    assert!(
        snapshot.matches(&current),
        "the revert target equals what the app already holds"
    );
}

#[test]
fn a_cancel_after_editing_does_not_match_the_current_value() {
    // The counterpart: a real revert must be distinguishable from a no-op, or
    // the skip-the-write optimisation would swallow genuine changes too.
    let mut slot = GestureSlot::<String>::new();
    let current = text("kept");
    assert_eq!(slot.begin(&current), GestureStart::Started);
    assert!(slot.live(text("edited")).is_some());
    let cancel = slot.cancel().expect("open");
    assert_ne!(cancel.restored().as_str(), "edited");
    assert!(!Snapshot::capture(cancel.restored()).matches(&text("edited")));
}
