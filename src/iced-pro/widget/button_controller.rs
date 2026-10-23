//! The Button activation engine — gesture linearity, busy suppression and
//! tiered emission (`UIF-ATOM-001a`, spec §3.0.4.3).
//!
//! [`ButtonController`] is the part of a button that has to *remember* things:
//! whether a gesture is open, whether this control already fired an action the
//! host has not finished with, and whether a stop request has already been put up.
//! Everything else about a button is a pure function of the model and the current
//! [`VisualState`]; this is the residue.
//!
//! # The three tiers a button can emit
//!
//! | User action | Emitted | Host callback |
//! |---|---|---|
//! | release inside / `Enter` / `Space` | `Commit<ButtonSettlement>` | `on_activation()` |
//! | release outside / `Escape` | `Cancel<ButtonSettlement>` | `on_cancel()` |
//! | press or `Escape` while busy | `Cancel<CancelRequest>` | `on_cancel_request()` |
//!
//! They are a sum type ([`ButtonEmission`]) rather than three closures because a
//! host wiring all three should be *forced* to write an arm for each. Three
//! optional callbacks silently degrade into "we remembered to wire the one we
//! demoed"; a match statement cannot silently drop a tier.
//!
//! # What the snapshot holds
//!
//! Opening a gesture captures a [`ButtonSettlement`] from the model as it stands
//! *before* the press: the modality, whether anything had been dispatched, and
//! the [`ActionPending`] projection at that instant. A cancel hands that exact
//! value back through [`Cancel::restored`], so "revert to before the gesture" is
//! a value the button actually retained rather than a promise the host keeps
//! somewhere else.
//!
//! # Why the gate is only armed for tracked actions
//!
//! [`ActivationGate`] latches `in_flight` when it lets an activation through, and
//! only clears on an explicit settle. Arming it for a button whose host never
//! reports `busy` would therefore latch forever — the button would fire exactly
//! once and then suppress everything. So the gate is consulted only when
//! [`ButtonModel::tracks_action`] holds: the host has said work is underway, or
//! said it can settle what gets dispatched. Both mean "this Commit starts
//! something you will hear back about".
//!
//! [`VisualState`]: crate::state::VisualState

use std::fmt;

use crate::binding::{
    ActionPending, Activation, ActivationGate, Cancel, CancelRequest, Commit, GestureSlot,
    GestureStart,
};
use crate::widget::button_model::ButtonModel;

/// How the user drove the control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationSource {
    /// A pointer press, and whatever settled it.
    Pointer,
    /// `Enter` / `Space` on the focused control.
    Keyboard,
}

/// What a button gesture settled into — the spec's `ButtonSettlement`.
///
/// This is both the value carried by a `Commit` and the value the gesture
/// snapshot retains for a `Cancel`. A `Cancel` hands back the *pre-gesture*
/// settlement, in which `activated` is `None` because nothing had been
/// dispatched yet — which is exactly what the host needs to know in order to
/// not revert something that never happened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonSettlement {
    /// The modality of the gesture this settles.
    pub source: ActivationSource,
    /// The activation this settlement carries: `Some(Fired)` when a command
    /// reached the application, `None` when nothing did.
    pub activated: Option<Activation>,
    /// The pending-action projection observed when the gesture settled.
    pub pending: ActionPending,
}

impl ButtonSettlement {
    /// A settlement for a gesture that has not dispatched anything yet.
    ///
    /// This is what the snapshot holds at gesture start.
    #[must_use]
    pub fn undispatched(source: ActivationSource, pending: ActionPending) -> Self {
        Self {
            source,
            activated: None,
            pending,
        }
    }

    /// Whether a command reached the application through this settlement.
    #[must_use]
    pub fn dispatched(&self) -> bool {
        self.activated
            .is_some_and(|activation| activation.is_fired())
    }
}

/// One tier's worth of output from the activation engine.
#[derive(Clone)]
pub enum ButtonEmission {
    /// `Commit<ButtonSettlement>` — a command was dispatched (`on_activation`).
    Activated(Commit<ButtonSettlement>),
    /// `Cancel<ButtonSettlement>` — the gesture was abandoned before dispatching
    /// (`on_cancel`), carrying what to go back to.
    Cancelled(Cancel<ButtonSettlement>),
    /// `Cancel<CancelRequest>` — the user asked to stop a running action
    /// (`on_cancel_request`). Granted only when the host offered a stop path.
    CancelRequested(CancelRequest),
}

impl ButtonEmission {
    /// The settlement carried by the `Activated` / `Cancelled` tiers.
    #[must_use]
    pub fn settlement(&self) -> Option<&ButtonSettlement> {
        match self {
            Self::Activated(commit) => Some(commit.value()),
            Self::Cancelled(cancel) => Some(cancel.restored()),
            Self::CancelRequested(_) => None,
        }
    }
}

impl PartialEq for ButtonEmission {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers that
    /// deliberately do not implement `PartialEq` — a commit and a cancel of the
    /// same value are *different events*, and comparing them structurally at the
    /// wrapper level would invite exactly that confusion. Here the tiers are
    /// matched first, so an `Activated` can never equal a `Cancelled`.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Activated(a), Self::Activated(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::CancelRequested(a), Self::CancelRequested(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for ButtonEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Activated(commit) => f.debug_tuple("Activated").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
            Self::CancelRequested(request) => {
                f.debug_tuple("CancelRequested").field(request).finish()
            }
        }
    }
}

/// The per-control memory a button needs: open gesture, activation gate, and the
/// stop request currently on the table.
///
/// Lives in the widget tree under the [`Ephemeral`] tag, so a rebuild drops it
/// along with every other piece of interaction state — a stale `in_flight` latch
/// cannot survive the control being rebuilt, which is the failure mode a
/// long-lived global gate would have.
///
/// [`Ephemeral`]: crate::binding::Ephemeral
#[derive(Debug, Default)]
pub struct ButtonController {
    /// The open gesture, holding the pre-gesture settlement.
    gesture: GestureSlot<ButtonSettlement>,
    /// The busy / double-activation gate.
    gate: ActivationGate,
    /// Whether a stop request has been surfaced for the current busy period.
    cancel_requested: bool,
    /// How many activations this control has actually let through.
    ///
    /// Tracked separately from the gate's own counter because a model that
    /// [`tracks_action`](ButtonModel::tracks_action) never consults the gate at
    /// all — so the gate fired nothing while the button dispatched everything.
    /// Conflating the two would report a working synchronous button as one that
    /// never fired.
    dispatched: usize,
    /// The outcome of the most recent attempt to open a gesture.
    last_begin: Option<GestureStart>,
}

impl ButtonController {
    /// A controller with no gesture open and nothing in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the host's current projection. Call once per frame, before any
    /// gesture handling.
    ///
    /// Latches `busy_observed` through the gate, and settles the gate once a
    /// *tracked* action has been seen busy and has now come back idle. The
    /// `busy_observed` precondition is what keeps the double-click window shut:
    /// a press that fired, followed by an idle frame the host has not caught up
    /// with, must not clear `in_flight` — otherwise the second click lands.
    pub fn observe(&mut self, model: &ButtonModel) {
        let pending = model.pending();
        self.gate.observe(&pending);
        if model.tracks_action() && !pending.is_busy() && self.gate.busy_observed() {
            self.gate.settle();
            self.cancel_requested = false;
        }
    }

    /// A press began. Opens the gesture so a drag-out has something to cancel,
    /// and surfaces a stop request if the control is already busy.
    pub fn begin(
        &mut self,
        model: &ButtonModel,
        source: ActivationSource,
    ) -> Option<ButtonEmission> {
        self.last_begin = Some(
            self.gesture
                .begin(&ButtonSettlement::undispatched(source, model.pending())),
        );
        if model.tracks_action() && model.busy {
            return self.request_cancel(model);
        }
        None
    }

    /// The gesture completed *inside* the control: try to dispatch.
    ///
    /// Returns [`ButtonEmission::Activated`] when the gate let the command
    /// through, or a stop request when it suppressed one — a release inside a
    /// busy button is not a second action, it is the user pressing again because
    /// nothing happened, which is a request to stop.
    ///
    /// A keyboard activation has no press behind it, so a gesture is opened here
    /// first; the snapshot is still captured before the dispatch, which is the
    /// only ordering the tier requires.
    pub fn commit(
        &mut self,
        model: &ButtonModel,
        source: ActivationSource,
    ) -> Option<ButtonEmission> {
        if !self.gesture.is_open() {
            self.last_begin = Some(
                self.gesture
                    .begin(&ButtonSettlement::undispatched(source, model.pending())),
            );
        }

        let pending = model.pending();
        let activation = if model.tracks_action() {
            self.gate.try_activate(&pending)
        } else {
            Activation::Fired
        };

        if activation.is_suppressed() {
            // Nothing was dispatched, so nothing is owed a revert tier. Drop the
            // gesture and let the stop request carry the user's intent.
            self.gesture.abort();
            return self.request_cancel(model);
        }

        let settlement = ButtonSettlement {
            source,
            activated: Some(activation),
            pending,
        };
        let emission = self
            .gesture
            .commit(settlement)
            .map(ButtonEmission::Activated);
        if emission.is_some() {
            self.dispatched += 1;
        }
        emission
    }

    /// The gesture was abandoned — released outside, or `Escape`.
    ///
    /// With a gesture open, hands back the pre-gesture settlement. With none open
    /// (the action is already running and the user pressed `Escape`), this
    /// degrades to a stop request instead of inventing a revert for a value that
    /// was never captured mid-flight.
    pub fn cancel(&mut self, model: &ButtonModel) -> Option<ButtonEmission> {
        if let Some(cancel) = self.gesture.cancel() {
            return Some(ButtonEmission::Cancelled(cancel));
        }
        self.request_cancel(model)
    }

    /// Surface a stop request, once per busy period.
    ///
    /// Refused silently when the model is not busy (nothing to stop) or is busy
    /// but never offered a stop path — the spec makes the request conditional on
    /// the action supporting cancellation, so a fire-and-forget button shows no
    /// stop affordance rather than a dead one.
    fn request_cancel(&mut self, model: &ButtonModel) -> Option<ButtonEmission> {
        if self.cancel_requested {
            return None;
        }
        model.pending().try_cancel().ok().map(|request| {
            self.cancel_requested = true;
            ButtonEmission::CancelRequested(request)
        })
    }

    /// Whether a gesture is currently open on this control.
    #[must_use]
    pub fn gesture_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// The outcome of the most recent attempt to open a gesture.
    ///
    /// [`GestureStart::AlreadyOpen`] means the nesting rule held: a second press
    /// arrived while a gesture was still open, the slot refused it, and the
    /// **outer** snapshot still governs the eventual cancel (§3.0.4.2).
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
    ///
    /// This is the control's own count, not the gate's: a synchronous model
    /// never consults the gate, so the gate's counter would read zero for a
    /// button that fired every single time.
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

    /// Drop any open gesture and stop-request state without emitting a tier.
    ///
    /// For the path where the control stops being able to act mid-gesture — a
    /// host disabled it, or it is being torn down. Neither a commit nor a revert
    /// is owed to anybody, and leaving the slot open would hand a re-enabled
    /// control a gesture whose snapshot predates everything since.
    ///
    /// Gate counters are deliberately **kept**: how many activations fired and
    /// how many were withheld is history, not live state.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.cancel_requested = false;
        self.last_begin = None;
    }
}

impl PartialEq for ButtonController {
    /// Compares the controller's observable shape, not its retained payload.
    ///
    /// Two controllers are equal when they have the same gate counters, the same
    /// stop-request state and the same gesture-open state. The snapshot *value*
    /// inside an open gesture is deliberately excluded: it is a copy of the
    /// model, and comparing it would make controller equality depend on label
    /// text, which is not interaction state.
    fn eq(&self, other: &Self) -> bool {
        self.gate.suppressed() == other.gate.suppressed()
            && self.dispatched == other.dispatched
            && self.gate.is_pending() == other.gate.is_pending()
            && self.gate.busy_observed() == other.gate.busy_observed()
            && self.cancel_requested == other.cancel_requested
            && self.gesture.is_open() == other.gesture.is_open()
            && self.last_begin == other.last_begin
    }
}

#[cfg(test)]
#[path = "button_controller_tests.rs"]
mod button_controller_tests;
