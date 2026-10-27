//! The modal's pending-action half — `Controlled` frame, `Ephemeral` action
//! (§3.0.4.1, §3.0.4.3, `UIF-MOD-002`).
//!
//! §3.0.4.1 splits a control by *owner*, and a modal is the clearest case in
//! the crate of two halves that must not be confused:
//!
//! * **Controlled** — whether the modal is open, whether it is dirty, what
//!   error it carries. The application owns all of it and the frame reads it
//!   as [`Controlled`](crate::binding::Controlled).
//! * **Ephemeral** — what the frame knows about the action the user has under
//!   way inside the card. That is framework-side interaction state: it has to
//!   be readable in the same frame it was written, and the application must
//!   not be able to reach in and change it.
//!
//! [`ModalAction`] is the ephemeral half. It is deliberately *not* the
//! `Controlled` half: the dirty guard (`UIF-MOD-002`) is the application's
//! answer to "may I close?", while `blocks_dismiss` is the framework's answer
//! to "is something still running that closing would strand?".
//!
//! # Why a running action refuses the unwind rather than being cancelled by it
//!
//! Closing a modal while its primary command is mid-flight leaves the
//! application holding a half-finished operation with no card to report it
//! in. So `Escape` against a busy modal declines, and the host surfaces the
//! stop request instead — the user gets a way to stop the work, not a way to
//! walk away from it. This is a *new* state: a modal that never reports
//! pending behaves exactly as it did before, so the dirty-guard contract is
//! untouched.
//!
//! # The clock stays out
//!
//! `busy` and `progress` arrive as [`ActionPending`] values through
//! [`observe`](ModalAction::observe). Nothing here reads a clock or holds a
//! duration.
//!
//! Unit tests live in the sibling `modal_tier_tests.rs` sidecar.

use crate::binding::{ActionPending, Activation, ActivationGate, CancelRequest};

/// The framework-side half of a modal: what is running, and whether a stop
/// request has been put up for it.
#[derive(Clone, Debug, Default)]
pub struct ModalAction {
    pending: ActionPending,
    gate: ActivationGate,
    cancel_requested: bool,
    dispatched: usize,
}

impl PartialEq for ModalAction {
    /// Compares the observable shape.
    ///
    /// Hand-written because [`ActivationGate`] deliberately has no structural
    /// equality — it is a latch, not a value — so the comparison goes through
    /// the counters the gate exposes.
    fn eq(&self, other: &Self) -> bool {
        self.pending == other.pending
            && self.gate.suppressed() == other.gate.suppressed()
            && self.dispatched == other.dispatched
            && self.gate.is_pending() == other.gate.is_pending()
            && self.gate.busy_observed() == other.gate.busy_observed()
            && self.cancel_requested == other.cancel_requested
    }
}

impl ModalAction {
    /// An idle action: nothing running, nothing requested.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the host's current projection, once per frame.
    ///
    /// Latches `busy_observed` through the gate and settles once a *tracked*
    /// action has been seen busy and has now come back idle.
    pub fn observe(&mut self, pending: ActionPending) {
        self.pending = pending;
        self.gate.observe(&pending);
        if self.tracks() && !pending.is_busy() && self.gate.busy_observed() {
            self.gate.settle();
            self.cancel_requested = false;
        }
    }

    /// The projection this action is running against.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.pending
    }

    /// Whether an action is currently running inside the card.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.pending.is_busy()
    }

    /// Whether the gate is consulted at all for this modal.
    ///
    /// The same rule as every other action control: gate only what the host
    /// will report back on, or the latch never clears.
    #[must_use]
    pub fn tracks(&self) -> bool {
        self.pending.is_busy() || self.pending.is_cancellable()
    }

    /// Ask whether the primary call to action may dispatch.
    ///
    /// An untracked modal fires every press; a tracked one goes through the
    /// gate, which is what stops the double-activation window.
    pub fn try_activate(&mut self) -> Activation {
        let activation = if self.tracks() {
            self.gate.try_activate(&self.pending)
        } else {
            Activation::Fired
        };
        if activation.is_fired() {
            self.dispatched += 1;
        }
        activation
    }

    /// Surface a stop request for the running action, once per busy period.
    ///
    /// `None` when nothing is running, or when something is running that the
    /// host never offered a stop path for.
    pub fn request_cancel(&mut self) -> Option<CancelRequest> {
        if self.cancel_requested {
            return None;
        }
        self.pending.try_cancel().ok().inspect(|_| {
            self.cancel_requested = true;
        })
    }

    /// Whether a dismiss must be refused because work is still running.
    ///
    /// Distinct from the dirty guard: `UIF-MOD-002` refuses because the user
    /// would discard *unsaved edits*; this refuses because the application is
    /// mid-operation and closing would strand it.
    #[must_use]
    pub fn blocks_dismiss(&self) -> bool {
        self.pending.is_busy()
    }

    /// Tell the action the running work has finished.
    #[must_use]
    pub fn settle(&mut self) -> bool {
        let was_pending = self.gate.is_pending();
        self.gate.settle();
        self.cancel_requested = false;
        was_pending
    }

    /// Whether the gate currently holds this modal.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.gate.is_pending()
    }

    /// Whether a `busy` projection has been seen since the last settle.
    #[must_use]
    pub fn busy_observed(&self) -> bool {
        self.gate.busy_observed()
    }

    /// How many activations were let through.
    #[must_use]
    pub fn fired(&self) -> usize {
        self.dispatched
    }

    /// How many activations the gate withheld.
    #[must_use]
    pub fn suppressed(&self) -> usize {
        self.gate.suppressed()
    }

    /// Whether a stop request is currently on the table.
    #[must_use]
    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    /// Drop the stop-request state and the gate latches without emitting a
    /// tier — the teardown path. Counters are kept.
    pub fn reset(&mut self) {
        self.gate.settle();
        self.cancel_requested = false;
    }
}

/// The manifest slug for what a modal's action half is doing right now.
///
/// A single word per state so a gallery cell and a log line read the same way.
#[must_use]
pub fn modal_action_state(action: &ModalAction) -> &'static str {
    if action.is_busy() && action.cancel_requested() {
        "cancel-requested"
    } else if action.is_busy() {
        "pending"
    } else if action.fired() > 0 {
        "settled"
    } else {
        "idle"
    }
}

#[cfg(test)]
#[path = "modal_tier_tests.rs"]
mod modal_tier_tests;
