//! The dialog footer button on the binding foundation (`UIF-ATOM-000b`,
//! §3.0.4.3, `UIF-INP-004`).
//!
//! A dialog's `Ok` is a primary call to action, which §3.0.4.3 names
//! explicitly as an `ActionPending` control: while the command it dispatched
//! is still running it must lock its bounds, occupy the spinner slot, refuse a
//! second activation, and offer `on_cancel_request()` if the host can stop
//! it. `Cancel` is the mirror control and never dispatches anything, so its
//! press settles straight onto the `Cancel` tier.
//!
//! # Why this is not just the button again
//!
//! [`ButtonController`](crate::widget::button_controller) is the same shape
//! but answers to [`ButtonModel`](crate::widget::button_model), and a dialog
//! footer is not a button model — the dialog owns the value, the verdict and
//! the dismissal cause, and the button only reports what its press settled
//! into. What the footer needs is the *tier*, carried by a settlement that
//! speaks about the dialog rather than about a label and a variant.
//!
//! The role split is the whole difference that matters for tiers:
//!
//! | Role | Press settles into | While busy |
//! |---|---|---|
//! | [`Primary`](DialogButtonRole::Primary) | `Commit` — the dialog's answer is accepted | stop request, second press suppressed |
//! | [`Secondary`](DialogButtonRole::Secondary) | `Cancel` — the dialog's answer is discarded | a stop request for whatever the primary left running |
//!
//! # The clock stays out
//!
//! `busy` arrives as an [`ActionPending`] value through
//! [`observe`](DialogButtonController::observe), exactly as the button's
//! does. Nothing here
//! reads a clock, holds a duration, or resolves a colour.
//!
//! Unit tests live in the sibling `dialog_button_tests.rs` sidecar.

use std::fmt;

use crate::binding::{
    ActionPending, Activation, ActivationGate, Cancel, CancelRequest, Commit, GestureSlot,
    GestureStart,
};
use crate::widget::button_controller::ActivationSource;

/// Which footer control a dialog tier is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DialogButtonRole {
    /// `Ok` / `Save` / `Delete` — the control that dispatches.
    Primary,
    /// `Cancel` — the control that discards.
    Secondary,
}

impl DialogButtonRole {
    /// Whether this is the dispatching control.
    #[must_use]
    pub const fn is_primary(self) -> bool {
        matches!(self, Self::Primary)
    }

    /// The manifest slug for this role.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    /// The tier a completed press on this role settles into.
    ///
    /// The secondary control's whole job is the revert, so its settled press
    /// is a `Cancel` even though nothing was ever captured to put back — the
    /// value it hands back is the dialog's `Cancelled` answer.
    #[must_use]
    pub const fn settle_tier(self) -> &'static str {
        match self {
            Self::Primary => "commit",
            Self::Secondary => "cancel",
        }
    }
}

/// What a footer-button press settled into.
///
/// Carried by the `Commit` the primary control emits and by the `Cancel`
/// either control can emit. The pre-gesture settlement the snapshot holds has
/// `dispatched: false`, so a cancel hands the host back "nothing was
/// accepted".
#[derive(Clone, Debug, PartialEq)]
pub struct DialogButtonSettlement {
    /// Which footer control this settles.
    pub role: DialogButtonRole,
    /// How the user drove it.
    pub source: ActivationSource,
    /// The pending-action projection at the moment of settling.
    pub pending: ActionPending,
    /// Whether a command reached the application.
    pub dispatched: bool,
}

impl DialogButtonSettlement {
    /// A settlement for a press that has not dispatched anything.
    #[must_use]
    pub fn undispatched(
        role: DialogButtonRole,
        source: ActivationSource,
        pending: ActionPending,
    ) -> Self {
        Self {
            role,
            source,
            pending,
            dispatched: false,
        }
    }
}

/// One tier's worth of output from a dialog footer button.
#[derive(Clone)]
pub enum DialogButtonEmission {
    /// `Commit<DialogButtonSettlement>` — the dialog was accepted.
    Committed(Commit<DialogButtonSettlement>),
    /// `Cancel<DialogButtonSettlement>` — the dialog was discarded.
    Cancelled(Cancel<DialogButtonSettlement>),
    /// `Cancel<CancelRequest>` — the user asked to stop the running action.
    CancelRequested(CancelRequest),
}

impl DialogButtonEmission {
    /// The settlement carried by the committed / cancelled tiers.
    #[must_use]
    pub fn settlement(&self) -> Option<&DialogButtonSettlement> {
        match self {
            Self::Committed(commit) => Some(commit.value()),
            Self::Cancelled(cancel) => Some(cancel.restored()),
            Self::CancelRequested(_) => None,
        }
    }

    /// The tier name this emission is, for a manifest line.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Committed(_) => "commit",
            Self::Cancelled(_) => "cancel",
            Self::CancelRequested(_) => "cancel-request",
        }
    }
}

impl PartialEq for DialogButtonEmission {
    /// Compares the payload each tier carries; see the identical reasoning on
    /// [`RowEmission`](crate::surface::menu::row_action::RowEmission).
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::CancelRequested(a), Self::CancelRequested(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for DialogButtonEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
            Self::CancelRequested(request) => {
                f.debug_tuple("CancelRequested").field(request).finish()
            }
        }
    }
}

/// The footer button's memory: the open press, the activation gate, and the
/// stop request currently on the table.
///
/// Held by the dialog host, not by the widget, because the host is what owns
/// the value being accepted and the dismissal that follows.
#[derive(Debug)]
pub struct DialogButtonController {
    role: DialogButtonRole,
    gesture: GestureSlot<DialogButtonSettlement>,
    gate: ActivationGate,
    pending: ActionPending,
    cancel_requested: bool,
    dispatched: usize,
    last_begin: Option<GestureStart>,
}

impl Clone for DialogButtonController {
    fn clone(&self) -> Self {
        Self {
            role: self.role,
            gesture: self.gesture.clone(),
            gate: self.gate,
            pending: self.pending,
            cancel_requested: self.cancel_requested,
            dispatched: self.dispatched,
            last_begin: self.last_begin,
        }
    }
}

impl Default for DialogButtonController {
    fn default() -> Self {
        Self::new(DialogButtonRole::Primary)
    }
}

impl DialogButtonController {
    /// A controller for one footer role, with nothing in flight.
    #[must_use]
    pub fn new(role: DialogButtonRole) -> Self {
        Self {
            role,
            gesture: GestureSlot::new(),
            gate: ActivationGate::new(),
            pending: ActionPending::idle(),
            cancel_requested: false,
            dispatched: 0,
            last_begin: None,
        }
    }

    /// The footer role this controller drives.
    #[must_use]
    pub const fn role(&self) -> DialogButtonRole {
        self.role
    }

    /// Record the host's current projection, once per frame, before any
    /// press is handled.
    pub fn observe(&mut self, pending: ActionPending) {
        self.pending = pending;
        self.gate.observe(&pending);
        if self.tracks() && !pending.is_busy() && self.gate.busy_observed() {
            self.gate.settle();
            self.cancel_requested = false;
        }
    }

    /// The projection this controller is running against.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.pending
    }

    /// Whether this button's presses should go through the gate.
    ///
    /// The same rule the button and the menu row use: gate only what the host
    /// will report back on, or the latch never clears.
    #[must_use]
    pub fn tracks(&self) -> bool {
        self.pending.is_busy() || self.pending.is_cancellable()
    }

    /// A press began.
    ///
    /// Opens the gesture so a drag-out has something to cancel, and surfaces
    /// a stop request when the control is already busy.
    pub fn begin(&mut self, source: ActivationSource) -> Option<DialogButtonEmission> {
        self.last_begin = Some(self.gesture.begin(&DialogButtonSettlement::undispatched(
            self.role,
            source,
            self.pending,
        )));
        if self.tracks() && self.pending.is_busy() {
            return self.request_cancel();
        }
        None
    }

    /// Whether a press on this control may dispatch, according to the gate.
    ///
    /// The [`Secondary`](DialogButtonRole::Secondary) control always answers
    /// `true`: discarding cannot double-dispatch, and gating `Cancel` would
    /// only risk wedging the one control the user needs to get out.
    fn dispatch_allowed(&mut self) -> bool {
        if !self.role.is_primary() {
            return true;
        }
        let activation = if self.tracks() {
            self.gate.try_activate(&self.pending)
        } else {
            Activation::Fired
        };
        if activation.is_fired() {
            self.dispatched += 1;
            return true;
        }
        false
    }

    /// Ask whether the dialog's primary call to action may dispatch,
    /// without settling a press gesture.
    ///
    /// The primitive a dialog host calls when it owns the press itself and
    /// needs to know whether there is something worth dispatching *before*
    /// it runs its accept path.
    pub fn try_dispatch(&mut self) -> bool {
        self.dispatch_allowed()
    }

    /// The press completed inside the control.
    ///
    /// A [`Secondary`](DialogButtonRole::Secondary) press settles as a
    /// `Cancel` and never touches the gate — discarding cannot
    /// double-dispatch. A [`Primary`](DialogButtonRole::Primary) press goes
    /// through the gate and either commits or is suppressed into a stop
    /// request.
    pub fn commit(&mut self, source: ActivationSource) -> Option<DialogButtonEmission> {
        if !self.gesture.is_open() {
            self.last_begin = Some(self.gesture.begin(&DialogButtonSettlement::undispatched(
                self.role,
                source,
                self.pending,
            )));
        }

        if !self.dispatch_allowed() {
            self.gesture.abort();
            return self.request_cancel();
        }

        let dispatched = self.role.is_primary();
        let settled = self.gesture.commit(DialogButtonSettlement {
            role: self.role,
            source,
            pending: self.pending,
            dispatched,
        });
        match (self.role, settled) {
            (DialogButtonRole::Primary, Some(commit)) => {
                Some(DialogButtonEmission::Committed(commit))
            }
            (_, Some(commit)) => Some(DialogButtonEmission::Cancelled(Cancel::new(
                commit.into_value(),
            ))),
            (_, None) => None,
        }
    }

    /// The press was abandoned — released outside the control, or `Escape`.
    pub fn cancel(&mut self) -> Option<DialogButtonEmission> {
        if let Some(cancel) = self.gesture.cancel() {
            return Some(DialogButtonEmission::Cancelled(cancel));
        }
        self.request_cancel()
    }

    /// Surface a stop request for the running action, once per busy period.
    ///
    /// `None` when nothing is running, or when something is running that
    /// the host never offered a stop path for — the spec makes the request
    /// conditional on the action supporting cancellation, so a
    /// fire-and-forget control shows no dead stop affordance.
    pub fn take_cancel_request(&mut self) -> Option<CancelRequest> {
        if self.cancel_requested {
            return None;
        }
        self.pending.try_cancel().ok().inspect(|_| {
            self.cancel_requested = true;
        })
    }

    /// The stop request wrapped in its emission tier.
    fn request_cancel(&mut self) -> Option<DialogButtonEmission> {
        self.take_cancel_request()
            .map(DialogButtonEmission::CancelRequested)
    }

    /// Whether a press is currently open on this control.
    #[must_use]
    pub fn gesture_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// The outcome of the most recent attempt to open a gesture.
    #[must_use]
    pub fn last_begin(&self) -> Option<GestureStart> {
        self.last_begin
    }

    /// Whether the gate currently holds this control.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.gate.is_pending()
    }

    /// Whether a `busy` projection has been seen since the last settle.
    #[must_use]
    pub fn busy_observed(&self) -> bool {
        self.gate.busy_observed()
    }

    /// How many activations this control has let through.
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

    /// Drop the open press and stop-request state without emitting a tier,
    /// and release the activation gate.
    ///
    /// The gate release is the point of the call: a control that kept an
    /// in-flight latch would never fire again for whoever reuses it, which
    /// is the same reason [`ModalAction::reset`](crate::surface::modal_tier::ModalAction::reset)
    /// settles its own. Counters are kept: they are history, not live
    /// state.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.gate.settle();
        self.cancel_requested = false;
        self.last_begin = None;
    }
}

impl PartialEq for DialogButtonController {
    /// Compares the observable shape, not the retained settlement — the same
    /// discipline as the row and button controllers.
    fn eq(&self, other: &Self) -> bool {
        self.role == other.role
            && self.gate.suppressed() == other.gate.suppressed()
            && self.dispatched == other.dispatched
            && self.gate.is_pending() == other.gate.is_pending()
            && self.gate.busy_observed() == other.gate.busy_observed()
            && self.cancel_requested == other.cancel_requested
            && self.gesture.is_open() == other.gesture.is_open()
            && self.last_begin == other.last_begin
    }
}

#[cfg(test)]
#[path = "dialog_button_tests.rs"]
mod dialog_button_tests;
