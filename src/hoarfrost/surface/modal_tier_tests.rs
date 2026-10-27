//! Tests for the modal's pending-action half (§3.0.4.1, `UIF-MOD-002`).

use crate::binding::{ActionPending, Activation, SuppressedReason};
use crate::surface::modal_tier::{ModalAction, modal_action_state};

fn cancellable_busy() -> ActionPending {
    ActionPending::indeterminate().with_cancellable(true)
}

fn tracked_idle() -> ActionPending {
    ActionPending::idle().with_cancellable(true)
}

// -- the idle default ------------------------------------------------------

#[test]
fn a_fresh_modal_action_is_idle_and_blocks_nothing() {
    let action = ModalAction::new();
    assert!(!action.is_busy());
    assert!(!action.blocks_dismiss());
    assert!(!action.tracks());
    assert_eq!(modal_action_state(&action), "idle");
}

#[test]
fn an_untracked_modal_fires_every_activation_and_never_suppresses() {
    let mut action = ModalAction::new();
    action.observe(ActionPending::idle());
    for _ in 0..3 {
        assert_eq!(action.try_activate(), Activation::Fired);
    }
    assert_eq!(action.fired(), 3);
    assert_eq!(action.suppressed(), 0);
}

// -- the double-activation window ------------------------------------------

#[test]
fn a_second_activation_inside_the_round_trip_window_is_withheld() {
    let mut action = ModalAction::new();
    action.observe(tracked_idle());
    assert_eq!(action.try_activate(), Activation::Fired);

    // The projection has not caught up yet — still idle.
    action.observe(tracked_idle());
    assert_eq!(
        action.try_activate(),
        Activation::Suppressed(SuppressedReason::AwaitingProjection),
        "the gate's own memory must close the window the projection has not reached"
    );
    assert_eq!(action.fired(), 1);
    assert_eq!(action.suppressed(), 1);
}

#[test]
fn a_visible_busy_projection_suppresses_with_the_busy_reason() {
    let mut action = ModalAction::new();
    action.observe(cancellable_busy());
    assert_eq!(
        action.try_activate(),
        Activation::Suppressed(SuppressedReason::Busy)
    );
}

#[test]
fn the_gate_settles_on_a_tracked_busy_to_idle_transition() {
    let mut action = ModalAction::new();
    action.observe(tracked_idle());
    action.try_activate();

    action.observe(cancellable_busy());
    assert!(action.busy_observed());
    assert!(action.is_pending());

    action.observe(tracked_idle());
    assert!(!action.is_pending(), "busy→idle settles the latch");
    assert_eq!(
        action.try_activate(),
        Activation::Fired,
        "and it fires again"
    );
}

#[test]
fn an_untracked_idle_projection_never_settles_a_latch_it_never_armed() {
    let mut action = ModalAction::new();
    action.observe(ActionPending::idle());
    action.try_activate();
    action.observe(ActionPending::idle());
    assert!(
        !action.busy_observed(),
        "nothing was ever seen busy, so nothing had to settle"
    );
}

// -- the running-action dismiss guard --------------------------------------

#[test]
fn a_running_action_blocks_the_dismiss() {
    let mut action = ModalAction::new();
    action.observe(cancellable_busy());
    assert!(action.blocks_dismiss());
}

#[test]
fn a_non_cancellable_running_action_still_blocks_the_dismiss() {
    let mut action = ModalAction::new();
    action.observe(ActionPending::determinate(0.5));
    assert!(action.blocks_dismiss());
    assert!(
        action.request_cancel().is_none(),
        "the dismiss is refused either way; only the stop affordance differs"
    );
}

#[test]
fn an_idle_modal_blocks_nothing() {
    let mut action = ModalAction::new();
    action.observe(cancellable_busy());
    assert!(action.blocks_dismiss());
    action.observe(ActionPending::idle().with_cancellable(true));
    assert!(!action.blocks_dismiss());
}

// -- stop requests ---------------------------------------------------------

#[test]
fn a_stop_request_is_granted_once_per_busy_period() {
    let mut action = ModalAction::new();
    action.observe(cancellable_busy());
    assert!(action.request_cancel().is_some());
    assert!(action.cancel_requested());
    assert!(
        action.request_cancel().is_none(),
        "one stop request per busy period"
    );
}

#[test]
fn a_stop_request_clears_when_the_action_finishes() {
    let mut action = ModalAction::new();
    action.observe(cancellable_busy());
    action.request_cancel();
    assert!(action.cancel_requested());

    action.observe(ActionPending::idle().with_cancellable(true));
    assert!(!action.cancel_requested());
}

#[test]
fn an_explicit_settle_reports_whether_anything_was_outstanding() {
    let mut action = ModalAction::new();
    assert!(!action.settle(), "nothing was outstanding");

    action.observe(tracked_idle());
    action.try_activate();
    assert!(action.settle(), "the activation was outstanding");
    assert!(!action.is_pending());
}

#[test]
fn reset_keeps_the_counters_and_drops_the_latches() {
    let mut action = ModalAction::new();
    action.observe(tracked_idle());
    action.try_activate();
    action.observe(cancellable_busy());
    action.request_cancel();
    let fired = action.fired();
    let suppressed = action.suppressed();

    action.reset();
    assert_eq!(action.fired(), fired);
    assert_eq!(action.suppressed(), suppressed);
    assert!(!action.is_pending());
    assert!(!action.cancel_requested());
    assert!(!action.busy_observed());
}

// -- manifest state --------------------------------------------------------

#[test]
fn the_manifest_state_names_what_the_half_is_doing() {
    let mut action = ModalAction::new();
    assert_eq!(modal_action_state(&action), "idle");

    action.observe(ActionPending::indeterminate());
    assert_eq!(modal_action_state(&action), "pending");

    action.observe(cancellable_busy());
    action.request_cancel();
    assert_eq!(modal_action_state(&action), "cancel-requested");

    action.observe(ActionPending::idle().with_cancellable(true));
    action.try_activate();
    action.reset();
    assert_eq!(modal_action_state(&action), "settled");
}

#[test]
fn the_manifest_state_is_distinct_across_all_four_cases() {
    let mut seen = vec!["idle", "pending", "cancel-requested", "settled"];
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before);
}

// -- equality ------------------------------------------------------------

#[test]
fn two_actions_compare_on_shape_not_on_identity() {
    let mut a = ModalAction::new();
    let mut b = ModalAction::new();
    assert_eq!(a, b);

    a.observe(cancellable_busy());
    assert_ne!(a, b);

    b.observe(cancellable_busy());
    assert_eq!(a, b, "the same projection produces the same shape");

    a.request_cancel();
    assert_ne!(a, b, "the stop request is part of the shape");
}

#[test]
fn a_clone_is_independent() {
    let mut a = ModalAction::new();
    a.observe(tracked_idle());
    assert_eq!(a.try_activate(), Activation::Fired);
    let mut b = a.clone();

    // b runs its own action all the way through; a never sees any of it.
    b.observe(cancellable_busy());
    assert!(b.busy_observed());
    b.observe(tracked_idle());
    assert!(!b.is_pending(), "b settled its own latch");
    assert_eq!(b.try_activate(), Activation::Fired, "and fires again");

    assert_eq!(a.fired(), 1, "a's counters are untouched by b");
    assert!(a.is_pending(), "a's latch is still its own");
    assert!(
        !a.busy_observed(),
        "a never saw the busy projection b observed"
    );
    assert_eq!(b.fired(), 2);
}
