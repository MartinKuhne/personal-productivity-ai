//! Tests for `ActionPending` and the activation gate (§3.0.4.3).
//!
//! The gate's reason for existing is a race, so most of this file is about the
//! window between firing a command and the application's `busy` projection
//! arriving. A gate that only read the projection would pass a naive test and
//! still run the command twice.

use crate::binding::pending::ActionPending;
use crate::binding::pending::Activation;
use crate::binding::pending::ActivationGate;
use crate::binding::pending::CancelRefusal;
use crate::binding::pending::SuppressedReason;

// ---------------------------------------------------------------------------
// ActionPending: the value
// ---------------------------------------------------------------------------

#[test]
fn idle_is_not_busy_and_asks_for_nothing() {
    let pending = ActionPending::idle();
    assert!(!pending.is_busy());
    assert_eq!(pending.progress(), None);
    assert!(!pending.is_cancellable());
    assert!(!pending.blocks_activation());
    assert!(!pending.locks_bounds());
    assert!(!pending.reserves_spinner());
}

#[test]
fn the_default_is_idle() {
    assert_eq!(ActionPending::default(), ActionPending::idle());
}

#[test]
fn an_indeterminate_action_triggers_every_busy_consequence() {
    let pending = ActionPending::indeterminate();
    assert!(pending.is_busy());
    assert_eq!(pending.progress(), None);
    assert!(pending.blocks_activation());
    assert!(pending.locks_bounds(), "UIF-PRIN-003 box lock");
    assert!(
        pending.reserves_spinner(),
        "the inline spinner slot is held"
    );
}

#[test]
fn a_determinate_action_keeps_its_fraction() {
    let pending = ActionPending::determinate(0.25);
    assert!(pending.is_busy());
    assert_eq!(pending.progress(), Some(0.25));
}

#[test]
fn a_fraction_above_one_is_clamped_rather_than_trusted() {
    assert_eq!(ActionPending::determinate(1.7).progress(), Some(1.0));
    assert_eq!(ActionPending::determinate(9_000.0).progress(), Some(1.0));
}

#[test]
fn a_negative_fraction_is_clamped_to_zero_rather_than_painting_in_reverse() {
    assert_eq!(ActionPending::determinate(-0.5).progress(), Some(0.0));
}

#[test]
fn the_endpoints_of_the_range_survive_exactly() {
    assert_eq!(ActionPending::determinate(0.0).progress(), Some(0.0));
    assert_eq!(ActionPending::determinate(1.0).progress(), Some(1.0));
}

#[test]
fn a_nan_fraction_becomes_no_progress_not_a_nan_bar() {
    // `f32::clamp` passes NaN through untouched, so this is the case a naive
    // implementation gets wrong while still looking clamped.
    let pending = ActionPending::determinate(f32::NAN);
    assert!(pending.is_busy(), "the action is still running");
    assert_eq!(pending.progress(), None, "NaN must not reach a width");
}

#[test]
fn an_infinite_fraction_becomes_no_progress() {
    assert_eq!(ActionPending::determinate(f32::INFINITY).progress(), None);
    assert_eq!(
        ActionPending::determinate(f32::NEG_INFINITY).progress(),
        None,
        "-inf is not 0% either; it is unknown"
    );
}

#[test]
fn cancellability_is_opt_in_and_orthogonal_to_progress() {
    let pending = ActionPending::determinate(0.5).with_cancellable(true);
    assert!(pending.is_cancellable());
    assert_eq!(pending.progress(), Some(0.5));
    let forced_off = ActionPending::indeterminate().with_cancellable(false);
    assert!(!forced_off.is_cancellable());
}

// ---------------------------------------------------------------------------
// Cancel requests
// ---------------------------------------------------------------------------

#[test]
fn cancelling_a_running_cancellable_action_is_granted() {
    let pending = ActionPending::indeterminate().with_cancellable(true);
    assert!(pending.try_cancel().is_ok());
}

#[test]
fn refusing_to_cancel_nothing_is_a_different_answer_from_refusing_a_locked_action() {
    assert_eq!(
        ActionPending::idle().try_cancel(),
        Err(CancelRefusal::NotBusy),
        "nothing was running"
    );
    assert_eq!(
        ActionPending::indeterminate().try_cancel(),
        Err(CancelRefusal::NotCancellable),
        "something was running but no stop path was ever offered"
    );
}

#[test]
fn a_cancel_refusal_is_never_mistakable_for_a_grant() {
    let refused = ActionPending::idle().try_cancel();
    assert!(refused.is_err());
    assert!(matches!(refused, Err(CancelRefusal::NotBusy)));
}

// ---------------------------------------------------------------------------
// Activation: the shape of an answer
// ---------------------------------------------------------------------------

#[test]
fn a_fired_activation_has_no_suppression_reason() {
    let activation = Activation::Fired;
    assert!(activation.is_fired());
    assert!(!activation.is_suppressed());
    assert_eq!(activation.reason(), None);
}

#[test]
fn a_suppressed_activation_names_why() {
    let busy = Activation::Suppressed(SuppressedReason::Busy);
    assert!(!busy.is_fired());
    assert!(busy.is_suppressed());
    assert_eq!(busy.reason(), Some(SuppressedReason::Busy));

    let stale = Activation::Suppressed(SuppressedReason::AwaitingProjection);
    assert_eq!(stale.reason(), Some(SuppressedReason::AwaitingProjection));
}

#[test]
fn the_two_suppression_reasons_are_distinguishable() {
    assert_ne!(
        Activation::Suppressed(SuppressedReason::Busy),
        Activation::Suppressed(SuppressedReason::AwaitingProjection),
        "the app must be able to tell 'already running' from 'not caught up yet'"
    );
}

// ---------------------------------------------------------------------------
// ActivationGate: the round-trip window
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_gate_is_idle() {
    let gate = ActivationGate::new();
    assert!(!gate.is_pending());
    assert!(!gate.busy_observed());
    assert_eq!(gate.fired(), 0);
    assert_eq!(gate.suppressed(), 0);
}

#[test]
fn the_default_gate_behaves_like_a_fresh_one() {
    let mut gate = ActivationGate::default();
    assert!(gate.try_activate(&ActionPending::idle()).is_fired());
}

#[test]
fn activating_an_idle_control_fires_once() {
    let mut gate = ActivationGate::new();
    assert_eq!(gate.try_activate(&ActionPending::idle()), Activation::Fired);
    assert_eq!(gate.fired(), 1);
    assert!(gate.is_pending(), "the gate remembers it fired");
}

#[test]
fn a_second_click_inside_the_round_trip_window_is_suppressed() {
    // The whole point: the projection still says idle because the app has not
    // redrawn yet, and the gate must not believe it.
    let mut gate = ActivationGate::new();
    let stale = ActionPending::idle();
    assert_eq!(gate.try_activate(&stale), Activation::Fired);
    assert_eq!(
        gate.try_activate(&stale),
        Activation::Suppressed(SuppressedReason::AwaitingProjection)
    );
    assert_eq!(gate.fired(), 1, "the command ran exactly once");
    assert_eq!(gate.suppressed(), 1);
}

#[test]
fn the_window_stays_closed_against_many_rapid_clicks() {
    let mut gate = ActivationGate::new();
    let stale = ActionPending::idle();
    gate.try_activate(&stale);
    for _ in 0..20 {
        assert!(gate.try_activate(&stale).is_suppressed());
    }
    assert_eq!(gate.fired(), 1);
    assert_eq!(gate.suppressed(), 20);
}

#[test]
fn an_arriving_busy_projection_is_observed_as_busy() {
    let mut gate = ActivationGate::new();
    gate.observe(&ActionPending::idle());
    assert!(!gate.busy_observed());
    gate.observe(&ActionPending::indeterminate());
    assert!(gate.busy_observed(), "the app caught up");
}

#[test]
fn a_visible_busy_projection_suppresses_rather_than_relying_on_memory() {
    let mut gate = ActivationGate::new();
    assert_eq!(
        gate.try_activate(&ActionPending::indeterminate()),
        Activation::Suppressed(SuppressedReason::Busy)
    );
    assert!(gate.busy_observed());
    assert_eq!(gate.fired(), 0);
}

#[test]
fn the_projection_takes_precedence_over_the_gate_own_memory() {
    // Fire on an idle projection, then a *busy* projection arrives. A later click
    // must report `Busy` (what the app actually says), not `AwaitingProjection`.
    let mut gate = ActivationGate::new();
    gate.try_activate(&ActionPending::idle());
    gate.observe(&ActionPending::indeterminate());
    assert_eq!(
        gate.try_activate(&ActionPending::indeterminate()),
        Activation::Suppressed(SuppressedReason::Busy)
    );
}

#[test]
fn settling_clears_both_latches_and_reports_it_had_been_pending() {
    let mut gate = ActivationGate::new();
    gate.try_activate(&ActionPending::idle());
    gate.observe(&ActionPending::indeterminate());
    assert!(gate.settle());
    assert!(!gate.is_pending());
    assert!(!gate.busy_observed());
}

#[test]
fn settling_an_idle_gate_reports_that_nothing_was_outstanding() {
    let mut gate = ActivationGate::new();
    assert!(!gate.settle(), "a stray settle is visible, not absorbed");
}

#[test]
fn a_control_can_fire_again_once_its_action_has_settled() {
    let mut gate = ActivationGate::new();
    gate.try_activate(&ActionPending::idle());
    gate.observe(&ActionPending::indeterminate());
    gate.settle();
    assert_eq!(gate.try_activate(&ActionPending::idle()), Activation::Fired);
    assert_eq!(gate.fired(), 2);
}

#[test]
fn a_full_session_keeps_the_fire_and_suppression_counts_separate() {
    let mut gate = ActivationGate::new();
    // Save clicked, held busy through three further clicks, then released.
    assert!(gate.try_activate(&ActionPending::idle()).is_fired());
    gate.observe(&ActionPending::determinate(0.4).with_cancellable(true));
    for _ in 0..3 {
        assert!(
            gate.try_activate(&ActionPending::determinate(0.4).with_cancellable(true))
                .is_suppressed()
        );
    }
    gate.settle();
    assert_eq!(gate.fired(), 1, "one command dispatched");
    assert_eq!(gate.suppressed(), 3, "three clicks swallowed");
    assert!(!gate.is_pending());
}

#[test]
fn the_gate_can_be_driven_without_ever_settingtling_and_stays_consistent() {
    // A never-settling action must not let a second command through either.
    let mut gate = ActivationGate::new();
    gate.try_activate(&ActionPending::idle());
    for _ in 0..50 {
        gate.observe(&ActionPending::indeterminate());
        assert!(
            gate.try_activate(&ActionPending::indeterminate())
                .is_suppressed()
        );
    }
    assert_eq!(gate.fired(), 1);
}

#[test]
fn an_activation_renders_readably_for_logs() {
    assert_eq!(Activation::Fired.to_string(), "fired");
    assert_eq!(
        Activation::Suppressed(SuppressedReason::Busy).to_string(),
        "suppressed: busy"
    );
    assert!(
        Activation::Suppressed(SuppressedReason::AwaitingProjection)
            .to_string()
            .contains("awaiting")
    );
}
