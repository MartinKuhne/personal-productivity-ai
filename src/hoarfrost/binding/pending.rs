//! `ActionPending` — what a control knows while its command is still running (§3.0.4.3).
//!
//! The spec's list of consequences for `busy == true` is unusually concrete: lock
//! the outer bounding box (`UIF-PRIN-003`), render an inline `14px`
//! indeterminate spinner, suppress duplicate `on_activate` emissions, and
//! optionally expose `on_cancel_request()`. Three of those four are *decisions*,
//! and until now each consuming widget would have re-derived them from a bare
//! `busy: bool` and could have disagreed with its neighbours about what `busy`
//! meant.
//!
//! This module names the value and answers the decisions in one place.
//!
//! # Scope note: this is not the spinner
//!
//! `reserves_spinner` answers *whether* the inline spinner slot is occupied. It
//! does not paint one, and the `size.spinner.sm` / `size.spinner.md` tokens are
//! deliberately **not** added here — they belong with `UIF-ATOM-009`, the
//! progress and spinner requirement that actually consumes them. Adding tokens with
//! no renderer would put numbers in the token layer that nothing in §2 has a
//! provenance for yet.
//!
//! # The race this exists to win
//!
//! The suppression requirement is not "don't fire while `busy` is true". A widget
//! reading only the incoming projection would fail in the window between firing
//! the command and the application's next frame arriving with `busy == true`. The
//! user double-clickes inside that window; the projection still says idle; the
//! command runs twice.
//!
//! [`ActivationGate`] closes it by remembering that *it* fired, independently of
//! what the projection has caught up to saying. See
//! [`SuppressedReason::AwaitingProjection`].
//!
//! Unit tests live in the sibling `pending_tests.rs` sidecar.

use std::fmt;

/// How far a running action has got, as reported by the application.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionPending {
    busy: bool,
    progress: Option<f32>,
    cancellable: bool,
}

impl ActionPending {
    /// Nothing is running; the control behaves normally.
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            busy: false,
            progress: None,
            cancellable: false,
        }
    }

    /// An action is running and its extent is unknown — the indeterminate case.
    #[must_use]
    pub const fn indeterminate() -> Self {
        Self {
            busy: true,
            progress: None,
            cancellable: false,
        }
    }

    /// An action is running with a known fraction complete.
    ///
    /// The fraction is clamped to `0.0..=1.0`, and a non-finite input is recorded
    /// as *no* progress rather than a garbage bar. A control must be able to take
    /// a hostile or buggy upstream number without painting a negative-width fill.
    #[must_use]
    pub fn determinate(progress: f32) -> Self {
        // `f32::clamp` passes NaN straight through, so the finiteness check has
        // to come first — otherwise the documented guarantee is the one thing the
        // code does not deliver.
        let progress = if progress.is_finite() {
            Some(progress.clamp(0.0, 1.0))
        } else {
            None
        };
        Self {
            busy: true,
            progress,
            cancellable: false,
        }
    }

    /// Set whether the user may ask for this action to be stopped.
    #[must_use]
    pub const fn with_cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    /// Whether an action is currently running.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    /// The known fraction complete, if any.
    #[must_use]
    pub const fn progress(&self) -> Option<f32> {
        self.progress
    }

    /// Whether a cancel request is offered for this action.
    #[must_use]
    pub const fn is_cancellable(&self) -> bool {
        self.cancellable
    }

    /// §3.0.4.3 — a busy control does not accept a fresh activation.
    #[must_use]
    pub const fn blocks_activation(&self) -> bool {
        self.busy
    }

    /// §3.0.4.3 / `UIF-PRIN-003` — a busy control holds its outer bounds.
    ///
    /// The label may change to "Saving…" without the control resizing, because a
    /// neighbour that moves when this one changes is a layout jump.
    #[must_use]
    pub const fn locks_bounds(&self) -> bool {
        self.busy
    }

    /// §3.0.4.3 — the inline spinner slot is occupied.
    #[must_use]
    pub const fn reserves_spinner(&self) -> bool {
        self.busy
    }

    /// Ask the running action to stop.
    ///
    /// Refused when nothing is running, and separately when something is running
    /// that the application never offered a stop for. The two refusals are
    /// different user-visible outcomes — one is a no-op, the other is a control
    /// that should not have had a stop affordance at all — so they are not
    /// collapsed into a single `false`.
    ///
    /// No `#[must_use]`: `Result` already carries it, and stacking the two only
    /// produces a `double_must_use` warning.
    pub fn try_cancel(&self) -> Result<CancelRequest, CancelRefusal> {
        if !self.busy {
            return Err(CancelRefusal::NotBusy);
        }
        if !self.cancellable {
            return Err(CancelRefusal::NotCancellable);
        }
        Ok(CancelRequest(()))
    }
}

impl Default for ActionPending {
    fn default() -> Self {
        Self::idle()
    }
}

/// A granted request for the application to stop a running action.
///
/// A token rather than a bare signal so that granting is a value the caller can
/// route, log and assert on — and so a refusal cannot be mistaken for a request
/// that was simply ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelRequest(());

/// Why a cancel request was not granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelRefusal {
    /// No action is running, so there is nothing to stop.
    NotBusy,
    /// An action is running, but the application never exposed a stop path for it.
    NotCancellable,
}

/// Why a fresh activation did not reach the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuppressedReason {
    /// The application's projection says an action is already running.
    Busy,
    /// This gate fired the action and the application's `busy` projection has not
    /// arrived yet — the double-click window §3.0.4.3 is about.
    AwaitingProjection,
}

/// The result of asking a control to activate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// The activation was let through and the command was dispatched.
    Fired,
    /// The activation was withheld for a named reason.
    Suppressed(SuppressedReason),
}

impl Activation {
    /// Whether the command ran.
    #[must_use]
    pub const fn is_fired(&self) -> bool {
        matches!(self, Self::Fired)
    }

    /// Whether the command was withheld.
    #[must_use]
    pub const fn is_suppressed(&self) -> bool {
        !self.is_fired()
    }

    /// The suppression reason, if this was suppressed.
    #[must_use]
    pub const fn reason(&self) -> Option<SuppressedReason> {
        match self {
            Self::Fired => None,
            Self::Suppressed(reason) => Some(*reason),
        }
    }
}

/// Decides whether an activation may fire, remembering what it already let through.
///
/// The gate holds two independent pieces of knowledge:
///
/// - `in_flight` — *this gate* emitted an activation and has not been told the
///   action finished. This is what closes the round-trip window.
/// - `busy_observed` — the application's projection has actually been seen to
///   report busy. This is what proves the two halves met, rather than the gate
///   suppressing on a coincidence.
///
/// Counters are kept alongside so a test can assert the *shape* of a session
/// (one fire, N suppressions) instead of only the answer to the last call.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActivationGate {
    in_flight: bool,
    busy_observed: bool,
    fired: usize,
    suppressed: usize,
}

impl ActivationGate {
    /// A gate with nothing in flight.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            in_flight: false,
            busy_observed: false,
            fired: 0,
            suppressed: 0,
        }
    }

    /// Whether the gate considers an activation outstanding.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.in_flight || self.busy_observed
    }

    /// Whether a `busy` projection has been observed since the last settle.
    #[must_use]
    pub const fn busy_observed(&self) -> bool {
        self.busy_observed
    }

    /// How many activations have been let through.
    #[must_use]
    pub const fn fired(&self) -> usize {
        self.fired
    }

    /// How many activations have been withheld.
    #[must_use]
    pub const fn suppressed(&self) -> usize {
        self.suppressed
    }

    /// Record what the application's current projection says.
    ///
    /// Called every frame with the incoming [`ActionPending`], whether or not the
    /// user is activating. Latching `busy_observed` here is what lets a later
    /// settle distinguish "the app caught up" from "the app never moved".
    pub fn observe(&mut self, pending: &ActionPending) {
        if pending.is_busy() {
            self.busy_observed = true;
        }
    }

    /// Ask whether an activation may fire right now.
    ///
    /// Checks the projection first, because a projection the gate can see is
    /// stronger evidence than the gate's own memory. Falls back to `in_flight`
    /// for the window where the projection has not yet caught up.
    pub fn try_activate(&mut self, pending: &ActionPending) -> Activation {
        if pending.blocks_activation() {
            self.suppressed += 1;
            self.busy_observed = true;
            return Activation::Suppressed(SuppressedReason::Busy);
        }
        if self.in_flight {
            self.suppressed += 1;
            return Activation::Suppressed(SuppressedReason::AwaitingProjection);
        }
        self.fired += 1;
        self.in_flight = true;
        Activation::Fired
    }

    /// Tell the gate the action has finished, clearing both latches.
    ///
    /// Returns whether anything had actually been outstanding, so a stray settle
    /// on an already-idle gate is visible rather than silently absorbed.
    pub fn settle(&mut self) -> bool {
        let was_pending = self.is_pending();
        self.in_flight = false;
        self.busy_observed = false;
        was_pending
    }
}

impl fmt::Display for Activation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fired => write!(f, "fired"),
            Self::Suppressed(SuppressedReason::Busy) => write!(f, "suppressed: busy"),
            Self::Suppressed(SuppressedReason::AwaitingProjection) => {
                write!(f, "suppressed: awaiting projection")
            }
        }
    }
}

#[cfg(test)]
#[path = "pending_tests.rs"]
mod pending_tests;
