//! Tests for the dialog footer button tiers (`UIF-ATOM-000b`, `UIF-INP-004`).

use crate::binding::{ActionPending, GestureStart};
use crate::surface::dialog_button::{
    DialogButtonController, DialogButtonEmission, DialogButtonRole,
};
use crate::widget::button_controller::ActivationSource;

const POINTER: ActivationSource = ActivationSource::Pointer;
const KEYBOARD: ActivationSource = ActivationSource::Keyboard;

fn primary() -> DialogButtonController {
    DialogButtonController::new(DialogButtonRole::Primary)
}

fn secondary() -> DialogButtonController {
    DialogButtonController::new(DialogButtonRole::Secondary)
}

fn busy() -> ActionPending {
    ActionPending::indeterminate().with_cancellable(true)
}

// -- role ------------------------------------------------------------------

#[test]
fn only_the_primary_role_dispatches() {
    assert!(DialogButtonRole::Primary.is_primary());
    assert!(!DialogButtonRole::Secondary.is_primary());
    assert_eq!(DialogButtonRole::Primary.slug(), "primary");
    assert_eq!(DialogButtonRole::Secondary.slug(), "secondary");
}

#[test]
fn the_two_roles_settle_on_opposite_tiers() {
    assert_eq!(DialogButtonRole::Primary.settle_tier(), "commit");
    assert_eq!(DialogButtonRole::Secondary.settle_tier(), "cancel");
}

#[test]
fn a_controller_defaults_to_the_primary_call_to_action() {
    assert_eq!(
        DialogButtonController::default().role(),
        DialogButtonRole::Primary
    );
}

// -- primary: the accept tier ----------------------------------------------

#[test]
fn a_primary_press_commits_and_marks_the_settlement_dispatched() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    c.begin(POINTER);

    let emission = c.commit(POINTER).expect("a primary press dispatches");
    assert!(matches!(emission, DialogButtonEmission::Committed(_)));
    assert_eq!(emission.tier(), "commit");
    let settlement = emission.settlement().expect("a commit carries one");
    assert!(settlement.dispatched);
    assert_eq!(settlement.role, DialogButtonRole::Primary);
    assert_eq!(c.fired(), 1);
    assert!(!c.gesture_open());
}

#[test]
fn a_keyboard_primary_press_opens_its_own_gesture_first() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    assert!(!c.gesture_open());
    let emission = c.commit(KEYBOARD).expect("Enter dispatches");
    assert!(matches!(emission, DialogButtonEmission::Committed(_)));
    assert_eq!(emission.settlement().expect("one").source, KEYBOARD);
    assert_eq!(c.last_begin(), Some(GestureStart::Started));
}

#[test]
fn a_primary_commit_consumes_the_gesture_so_a_second_emits_nothing() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    c.begin(POINTER);
    assert!(c.commit(POINTER).is_some());
    // The slot is spent; a second settle through the same press cannot exist.
    assert!(c.gesture.cancel().is_none());
    assert_eq!(c.fired(), 1);
}

// -- secondary: the discard tier -------------------------------------------

#[test]
fn a_secondary_press_settles_on_the_cancel_tier_and_never_dispatches() {
    let mut c = secondary();
    c.observe(ActionPending::idle());
    c.begin(POINTER);

    let emission = c.commit(POINTER).expect("Cancel always settles");
    assert!(matches!(emission, DialogButtonEmission::Cancelled(_)));
    assert_eq!(emission.tier(), "cancel");
    let settlement = emission.settlement().expect("a cancel carries one");
    assert!(!settlement.dispatched);
    assert_eq!(settlement.role, DialogButtonRole::Secondary);
    assert_eq!(c.fired(), 0, "discarding cannot dispatch");
}

#[test]
fn the_secondary_control_never_gates_a_press() {
    let mut c = secondary();
    c.observe(busy());
    c.begin(POINTER);
    let emission = c.commit(POINTER).expect("Cancel still settles while busy");
    assert!(matches!(emission, DialogButtonEmission::Cancelled(_)));
    assert_eq!(
        c.suppressed(),
        0,
        "the gate is never consulted for a control that cannot dispatch"
    );
}

// -- busy behaviour --------------------------------------------------------

#[test]
fn a_press_on_a_busy_primary_surfaces_a_stop_request() {
    let mut c = primary();
    c.observe(busy());
    let emission = c.begin(POINTER).expect("busy + cancellable requests");
    assert!(matches!(emission, DialogButtonEmission::CancelRequested(_)));
    assert_eq!(emission.tier(), "cancel-request");
    assert!(c.cancel_requested());
    assert_eq!(c.fired(), 0);
}

#[test]
fn only_one_stop_request_is_surfaced_per_busy_period() {
    let mut c = primary();
    c.observe(busy());
    assert!(c.begin(POINTER).is_some());
    assert!(c.begin(POINTER).is_none());
    assert!(c.commit(POINTER).is_none(), "and the release adds nothing");
    assert_eq!(c.fired(), 0);
}

#[test]
fn a_busy_primary_with_no_stop_path_emits_nothing() {
    let mut c = primary();
    c.observe(ActionPending::indeterminate());
    assert!(c.begin(POINTER).is_none());
    assert!(!c.cancel_requested());
}

#[test]
fn a_second_primary_press_inside_the_round_trip_window_is_suppressed() {
    let mut c = primary();
    c.observe(ActionPending::idle().with_cancellable(true));
    assert!(matches!(
        c.commit(POINTER),
        Some(DialogButtonEmission::Committed(_))
    ));

    // The projection has not caught up: it still says idle.
    c.observe(ActionPending::idle().with_cancellable(true));
    assert!(
        c.commit(POINTER).is_none(),
        "the double-click window must not accept twice"
    );
    assert_eq!(c.fired(), 1);
    assert_eq!(c.suppressed(), 1);
}

#[test]
fn the_gate_settles_when_a_tracked_action_goes_busy_then_idle() {
    let mut c = primary();
    c.observe(ActionPending::idle().with_cancellable(true));
    c.commit(POINTER).expect("first accepts");

    c.observe(busy());
    assert!(c.busy_observed());
    assert!(c.is_pending());

    c.observe(ActionPending::idle().with_cancellable(true));
    assert!(!c.is_pending());
    assert!(
        matches!(c.commit(POINTER), Some(DialogButtonEmission::Committed(_))),
        "the dialog accepts again once the action finished"
    );
    assert_eq!(c.fired(), 2);
}

#[test]
fn an_untracked_projection_fires_every_press() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    for _ in 0..3 {
        assert!(matches!(
            c.commit(POINTER),
            Some(DialogButtonEmission::Committed(_))
        ));
    }
    assert_eq!(c.fired(), 3);
    assert_eq!(c.suppressed(), 0);
}

// -- cancel / nesting ------------------------------------------------------

#[test]
fn releasing_outside_restores_an_undispatched_settlement() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    c.begin(POINTER);
    assert!(c.gesture_open(), "the press opened the gesture");
    let emission = c.cancel().expect("cancels");
    let restored = match &emission {
        DialogButtonEmission::Cancelled(cancel) => cancel.restored(),
        other => panic!("expected a cancel, got {other:?}"),
    };
    assert!(!restored.dispatched);
    assert_eq!(restored.role, DialogButtonRole::Primary);
    assert_eq!(c.fired(), 0);
}

#[test]
fn a_second_begin_while_open_is_refused_and_the_outer_snapshot_governs() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    c.begin(POINTER);
    assert!(c.gesture_open(), "the first press opened the gesture");
    c.begin(POINTER);
    assert_eq!(c.last_begin(), Some(GestureStart::AlreadyOpen));
    let emission = c.cancel().expect("outer cancels");
    assert!(
        !emission.settlement().expect("outer snapshot").dispatched,
        "the inner begin must not have re-captured the snapshot mid-flight"
    );
}

#[test]
fn cancel_with_no_open_press_degrades_to_a_stop_request() {
    let mut c = primary();
    c.observe(busy());
    let emission = c
        .cancel()
        .expect("a busy cancellable control can be stopped");
    assert!(matches!(emission, DialogButtonEmission::CancelRequested(_)));
}

#[test]
fn cancel_with_no_open_press_and_nothing_running_emits_nothing() {
    let mut c = primary();
    c.observe(ActionPending::idle());
    assert!(c.cancel().is_none());
}

// -- reset / equality / clone ----------------------------------------------

#[test]
fn reset_drops_the_press_and_keeps_the_counters() {
    let mut c = primary();
    c.observe(ActionPending::idle().with_cancellable(true));
    c.commit(POINTER).expect("accepts");
    c.begin(POINTER);
    let fired = c.fired();

    c.reset();
    assert!(!c.gesture_open());
    assert!(!c.cancel_requested());
    assert_eq!(c.last_begin(), None);
    assert!(
        !c.is_pending(),
        "reset releases the activation gate, so a reused control can fire again"
    );
    assert!(!c.busy_observed());
    assert_eq!(c.fired(), fired);
}

#[test]
fn controllers_compare_on_shape_and_role() {
    let mut a = primary();
    let mut b = primary();
    assert_eq!(a, b);

    a.observe(ActionPending::idle());
    b.observe(ActionPending::idle());
    a.begin(POINTER);
    assert_ne!(a, b, "gesture-open state is part of the shape");

    let c = secondary();
    assert_ne!(primary(), c, "role is part of the shape");
}

#[test]
fn a_clone_carries_the_open_press_independently() {
    let mut a = primary();
    a.observe(ActionPending::idle());
    a.begin(POINTER);
    let mut b = a.clone();
    assert_eq!(a, b);

    b.commit(POINTER).expect("the clone settles its own press");
    assert_ne!(a, b, "the original's press is untouched by the clone");
    assert!(a.gesture_open());
    assert!(!b.gesture_open());
}

#[test]
fn a_committed_never_equals_a_cancelled() {
    let mut a = primary();
    a.observe(ActionPending::idle());
    a.begin(POINTER);
    let committed = a.commit(POINTER).expect("accepts");

    let mut b = primary();
    b.observe(ActionPending::idle());
    b.begin(POINTER);
    let cancelled = b.cancel().expect("cancels");

    assert_ne!(committed, cancelled);
}
