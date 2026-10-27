//! Tests for `widget/button_controller.rs` — the `UIF-ATOM-001a` activation engine.

use super::*;

/// A model with no action for the gate to police: a synchronous command.
fn sync() -> ButtonModel {
    ButtonModel::new("Save")
}

/// A model whose host will report back on what gets dispatched.
fn tracked() -> ButtonModel {
    ButtonModel::new("Save").cancellable()
}

/// A tracked model the host has marked in flight.
fn busy() -> ButtonModel {
    ButtonModel::new("Saving").busy().cancellable()
}

/// A busy model whose host never offered a stop path — fire and forget.
fn busy_fire_and_forget() -> ButtonModel {
    ButtonModel::new("Saving").busy()
}

// ---------------------------------------------------------------------------
// Fresh state
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_controller_has_no_gesture_nothing_in_flight_and_no_request() {
    let controller = ButtonController::new();

    assert!(!controller.gesture_open());
    assert!(!controller.is_pending());
    assert!(!controller.busy_observed());
    assert!(!controller.cancel_requested());
    assert_eq!(controller.fired(), 0);
    assert_eq!(controller.suppressed(), 0);
    assert_eq!(controller.last_begin(), None);
}

// ---------------------------------------------------------------------------
// Opening a gesture
// ---------------------------------------------------------------------------

#[test]
fn a_press_opens_a_gesture_and_records_that_it_started() {
    let mut controller = ButtonController::new();

    assert_eq!(
        controller.begin(&sync(), ActivationSource::Pointer),
        None,
        "a press on a live, idle button produces no tier — nothing has settled yet"
    );
    assert!(controller.gesture_open());
    assert_eq!(controller.last_begin(), Some(GestureStart::Started));
}

#[test]
fn a_second_press_cannot_re_snapshot_the_open_gesture() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);

    assert!(
        matches!(
            controller.begin(&busy(), ActivationSource::Pointer),
            Some(ButtonEmission::CancelRequested(_))
        ),
        "the second press lands on a busy model, so it puts a stop request up too"
    );
    assert_eq!(
        controller.last_begin(),
        Some(GestureStart::AlreadyOpen),
        "the nesting rule held: the outer snapshot still governs"
    );

    // And the revert really does go back to the *outer* value, not the inner one.
    let emission = controller
        .cancel(&busy())
        .expect("the outer gesture is still open");
    let ButtonEmission::Cancelled(cancel) = emission else {
        panic!("expected a Cancel tier, got {emission:?}");
    };
    assert!(
        !cancel.restored().pending.is_busy(),
        "the snapshot is the one taken before the *first* press; re-capturing it on the \
         second would silently shorten the revert to an intermediate the user never agreed to"
    );
}

// ---------------------------------------------------------------------------
// Committing
// ---------------------------------------------------------------------------

#[test]
fn releasing_inside_a_live_button_dispatches_and_carries_the_activation() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);

    let emission = controller
        .commit(&sync(), ActivationSource::Pointer)
        .expect("a live button commits");
    let ButtonEmission::Activated(commit) = emission else {
        panic!("expected an Activated tier, got {emission:?}");
    };

    let settlement = commit.value();
    assert_eq!(settlement.source, ActivationSource::Pointer);
    assert_eq!(settlement.activated, Some(Activation::Fired));
    assert!(settlement.dispatched());
    assert!(!settlement.pending.is_busy());
    assert_eq!(controller.fired(), 1);
    assert!(!controller.gesture_open(), "settling closes the gesture");
}

#[test]
fn a_keyboard_activation_opens_its_own_gesture_before_dispatching() {
    let mut controller = ButtonController::new();
    assert!(!controller.gesture_open());

    let emission = controller
        .commit(&sync(), ActivationSource::Keyboard)
        .expect("Enter on a live button commits");
    let ButtonEmission::Activated(commit) = emission else {
        panic!("expected an Activated tier, got {emission:?}");
    };

    assert_eq!(
        commit.value().source,
        ActivationSource::Keyboard,
        "the modality is recorded, so a host can tell a click from a keystroke"
    );
    assert!(commit.value().dispatched());
}

#[test]
fn a_commit_dispatches_only_once_per_gesture() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);
    assert!(
        controller
            .commit(&sync(), ActivationSource::Pointer)
            .is_some()
    );

    // A second commit opens a *fresh* gesture, so it fires again — which is
    // correct: two presses are two activations. What cannot happen is the same
    // gesture settling twice, because `Gesture::commit` consumes it.
    assert!(
        controller
            .commit(&sync(), ActivationSource::Pointer)
            .is_some()
    );
    assert_eq!(controller.fired(), 2);
}

// ---------------------------------------------------------------------------
// Cancelling
// ---------------------------------------------------------------------------

#[test]
fn releasing_outside_restores_the_pre_gesture_settlement() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);

    let emission = controller
        .cancel(&sync())
        .expect("an open gesture owes a Cancel tier");
    let ButtonEmission::Cancelled(cancel) = emission else {
        panic!("expected a Cancelled tier, got {emission:?}");
    };

    let restored = cancel.restored();
    assert_eq!(restored.source, ActivationSource::Pointer);
    assert_eq!(
        restored.activated, None,
        "nothing was dispatched, so there is nothing for the host to undo"
    );
    assert!(!restored.dispatched());
    assert_eq!(controller.fired(), 0);
    assert!(!controller.gesture_open());
}

#[test]
fn cancelling_with_no_gesture_open_and_nothing_running_emits_nothing() {
    let mut controller = ButtonController::new();
    assert_eq!(controller.cancel(&sync()), None);
}

#[test]
fn escape_with_an_action_in_flight_and_no_gesture_becomes_a_stop_request() {
    let mut controller = ButtonController::new();
    // The action was dispatched and the gesture settled; now the user wants out.
    controller.observe(&busy());

    let emission = controller
        .cancel(&busy())
        .expect("Escape while busy asks to stop");
    assert!(
        matches!(emission, ButtonEmission::CancelRequested(_)),
        "with no snapshot to revert to, the honest tier is the request, not a fabricated Cancel"
    );
}

// ---------------------------------------------------------------------------
// Busy suppression
// ---------------------------------------------------------------------------

#[test]
fn pressing_a_busy_cancellable_button_asks_to_stop_instead_of_dispatching() {
    let mut controller = ButtonController::new();

    let emission = controller
        .begin(&busy(), ActivationSource::Pointer)
        .expect("a busy cancellable button surfaces a request");
    assert!(matches!(emission, ButtonEmission::CancelRequested(_)));
    assert_eq!(controller.fired(), 0);
    assert!(controller.cancel_requested());
}

#[test]
fn pressing_a_busy_uncancellable_button_offers_no_stop_affordance_at_all() {
    let mut controller = ButtonController::new();

    assert_eq!(
        controller.begin(&busy_fire_and_forget(), ActivationSource::Pointer),
        None,
        "the spec makes the request conditional on the action supporting cancellation, so \
         a fire-and-forget button shows a dead end rather than a dead affordance"
    );
    assert!(!controller.cancel_requested());
}

#[test]
fn a_stop_request_is_surfaced_once_per_busy_period() {
    let mut controller = ButtonController::new();

    assert!(
        controller
            .begin(&busy(), ActivationSource::Pointer)
            .is_some()
    );
    assert_eq!(
        controller.begin(&busy(), ActivationSource::Pointer),
        None,
        "the second press must not stack a second request onto the one already on the table"
    );
    assert!(controller.cancel_requested());
}

#[test]
fn releasing_inside_a_busy_button_is_a_second_request_not_a_second_action() {
    let mut controller = ButtonController::new();
    controller.begin(&busy_fire_and_forget(), ActivationSource::Pointer);

    assert_eq!(
        controller.commit(&busy_fire_and_forget(), ActivationSource::Pointer),
        None,
        "the gate withheld it and there is no stop path to ask for"
    );
    assert_eq!(controller.fired(), 0);
    assert_eq!(
        controller.suppressed(),
        1,
        "but the withholding is recorded — a suppressed activation is not the same as one \
         that never arrived"
    );
}

#[test]
fn the_double_click_window_stays_shut_until_the_host_says_busy() {
    let mut controller = ButtonController::new();

    // First press fires.
    assert!(
        controller
            .commit(&tracked(), ActivationSource::Pointer)
            .is_some()
    );
    assert_eq!(controller.fired(), 1);

    // The host has not caught up yet: still idle. The second press must not land.
    controller.observe(&tracked());
    assert!(
        controller.is_pending(),
        "`in_flight` holds the window open even though nothing busy has been seen"
    );
    assert_eq!(
        controller.commit(&tracked(), ActivationSource::Pointer),
        None,
        "the second click of a double-click is swallowed"
    );
    assert_eq!(controller.fired(), 1);
    assert_eq!(controller.suppressed(), 1);
    assert!(
        !controller.busy_observed(),
        "the suppression came from `in_flight`, not from a busy projection — which is \
         exactly the `AwaitingProjection` case §3.0.4.3 is about"
    );
}

#[test]
fn the_gate_opens_again_once_the_host_has_been_seen_busy_and_come_back_idle() {
    let mut controller = ButtonController::new();

    controller.commit(&tracked(), ActivationSource::Pointer);
    controller.observe(&busy());
    assert!(controller.busy_observed());
    assert!(controller.is_pending());

    controller.observe(&tracked());
    assert!(
        !controller.is_pending(),
        "the round trip completed: the gate saw the action running and has now seen it finish"
    );

    assert!(
        controller
            .commit(&tracked(), ActivationSource::Pointer)
            .is_some(),
        "and the button works again"
    );
    assert_eq!(controller.fired(), 2);
}

#[test]
fn a_model_that_tracks_nothing_never_latches_the_gate() {
    let mut controller = ButtonController::new();

    for _ in 0..5 {
        assert!(
            controller
                .commit(&sync(), ActivationSource::Pointer)
                .is_some()
        );
    }
    assert_eq!(
        controller.fired(),
        5,
        "a synchronous command fires every time — arming the gate for it would have left \
         the button dead after one press"
    );
    assert!(!controller.is_pending());
}

#[test]
fn observing_a_non_tracking_model_never_settles_a_latched_gate() {
    let mut controller = ButtonController::new();
    controller.commit(&tracked(), ActivationSource::Pointer);
    assert!(controller.is_pending());

    controller.observe(&sync());
    assert!(
        controller.is_pending(),
        "a model that never claimed to track an action cannot be the thing that settles one"
    );
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_drops_the_gesture_and_the_request_but_keeps_the_history() {
    let mut controller = ButtonController::new();
    controller.begin(&busy(), ActivationSource::Pointer);
    assert!(controller.gesture_open());
    assert!(controller.cancel_requested());

    controller.reset();

    assert!(!controller.gesture_open());
    assert!(!controller.cancel_requested());
    assert_eq!(controller.last_begin(), None);
    assert_eq!(
        controller.fired(),
        0,
        "counters are history, not live state — resetting the live half must not rewrite them"
    );
}

#[test]
fn a_reset_controller_can_run_a_whole_gesture_again() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);
    controller.reset();

    assert!(
        controller
            .commit(&sync(), ActivationSource::Pointer)
            .is_some()
    );
    assert_eq!(controller.fired(), 1);
}

// ---------------------------------------------------------------------------
// The emission sum type
// ---------------------------------------------------------------------------

#[test]
fn only_the_two_settling_tiers_carry_a_settlement() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);
    let activated = controller
        .commit(&sync(), ActivationSource::Pointer)
        .unwrap();
    assert!(activated.settlement().is_some());

    controller.begin(&sync(), ActivationSource::Pointer);
    let cancelled = controller.cancel(&sync()).unwrap();
    assert!(cancelled.settlement().is_some());

    controller.observe(&busy());
    let requested = controller.cancel(&busy()).unwrap();
    assert_eq!(
        requested.settlement(),
        None,
        "a stop request carries no value to revert — it is a different tier, and the type says so"
    );
}

#[test]
fn tiers_of_different_kinds_never_compare_equal() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);
    let activated = controller
        .commit(&sync(), ActivationSource::Pointer)
        .unwrap();

    controller.begin(&sync(), ActivationSource::Pointer);
    let cancelled = controller.cancel(&sync()).unwrap();

    assert_ne!(
        activated, cancelled,
        "a commit and a cancel of an identical settlement are different events and must \
         never be mistaken for each other"
    );
    assert_eq!(
        activated,
        activated.clone(),
        "and the same tier with the same payload does compare equal"
    );
}

#[test]
fn an_emission_renders_its_tier_name_and_payload() {
    let mut controller = ButtonController::new();
    controller.begin(&sync(), ActivationSource::Pointer);
    let activated = controller
        .commit(&sync(), ActivationSource::Pointer)
        .unwrap();

    let text = format!("{activated:?}");
    assert!(
        text.starts_with("Activated"),
        "the tier is the first thing a log line shows: {text}"
    );
    assert!(
        text.contains("Fired"),
        "and the payload is inside it: {text}"
    );
}

// ---------------------------------------------------------------------------
// Controller equality
// ---------------------------------------------------------------------------

#[test]
fn controller_equality_compares_shape_not_the_retained_snapshot() {
    let mut a = ButtonController::new();
    let mut b = ButtonController::new();
    assert_eq!(a, b);

    a.begin(&sync(), ActivationSource::Pointer);
    b.begin(&busy(), ActivationSource::Pointer);
    assert_ne!(
        a, b,
        "one has a stop request outstanding and the other does not — that is a real difference"
    );

    let mut c = ButtonController::new();
    c.begin(&sync(), ActivationSource::Pointer);
    assert_eq!(
        a, c,
        "same gate, same request state, same gesture-open state — the *value* inside the \
         snapshot is a copy of the model and is not interaction state"
    );
}

#[test]
fn a_settled_controller_differs_from_a_fresh_one() {
    let mut controller = ButtonController::new();
    let fresh = ButtonController::new();

    controller.commit(&sync(), ActivationSource::Pointer);
    assert_ne!(
        controller, fresh,
        "having fired is a difference a test should be able to see"
    );
}
