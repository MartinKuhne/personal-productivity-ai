//! The menu-row action engine — `ActionPending` on a `MenuRow` (§3.0.4.3,
//! `UIF-ATOM-000b`).
//!
//! §3.0.4.3 names `MenuRow` as an action control, which means a row owes the
//! same four things a button owes: a locked bounding box while busy, an
//! occupied spinner slot, suppression of duplicate activations, and a
//! `on_cancel_request()` path. Until now the row model had nowhere to put any
//! of that, and the widget had no memory of having fired.
//!
//! # Why the pending state is not a field on [`Row`]
//!
//! [`MenuCascade`](super::cascade::MenuCascade) owns *cloned* menu models and
//! [`sync`](super::cascade::MenuCascade::sync) overwrites the local copy from
//! the source tree. A `busy` flag stored on a row would therefore be erased by
//! the next sync — the row would go idle the moment the application refreshed
//! the menu, which is exactly the lie §3.0.4.1 exists to prevent. The engine
//! lives *beside* the model instead, keyed by the row the host is driving, and
//! the projection arrives as a value through
//! [`observe`](RowActionEngine::observe).
//!
//! # The revert question, answered per row kind
//!
//! A menu is not a text field. `on_cancel()` means "restore the pre-gesture
//! snapshot", and for several row kinds there is no such snapshot worth
//! restoring — the gesture that dispatched the command also took the tree with
//! it. Inventing a `Cancel` arm there would advertise an undo the interaction
//! model cannot perform, so each kind is classified honestly by
//! [`row_revert`] and the classification travels with the settlement:
//!
//! | Row kind | `RevertKind` | Why |
//! |---|---|---|
//! | `Check` staying open | [`RevertKind::Value`] | The pre-toggle `checked` is retained and handed back |
//! | `Radio` staying open | [`RevertKind::Value`] | The pre-selection member and cleared set are retained |
//! | `Check` / `Radio` closing the tree | [`RevertKind::FireAndSettle`] | The card is gone; there is nothing left to put back |
//! | `Action` | [`RevertKind::FireAndSettle`] | A command dispatched and the tree closed with it |
//! | `Submenu` open | [`RevertKind::Navigation`] | Nothing changed value; closing the child is not an undo |
//! | Heading / separator / disabled | [`RevertKind::Nothing`] | The click was absorbed (`UIF-CMENU-002`) |
//!
//! The cancel that *is* always genuine is the **pre-dispatch** one: press and
//! release outside the row's bounds never dispatched anything, so the snapshot
//! it restores — "nothing happened" — is a state the user actually agreed to.
//!
//! # Why the gate is only armed for tracked actions
//!
//! Same trap the button fell into ([`ButtonController`](crate::widget::button_controller)):
//! [`ActivationGate`] latches `in_flight` when it lets an activation through
//! and clears only on an observed busy→idle transition. Arming it for a row
//! whose host never reports `busy` would latch forever, so the gate is
//! consulted only when [`row_tracks_action`] holds.
//!
//! Unit tests live in the sibling `row_action_tests.rs` sidecar.

use std::fmt;

use crate::binding::{
    ActionPending, Activation, ActivationGate, Cancel, CancelRequest, Commit, GestureSlot,
    GestureStart,
};
use crate::surface::menu::model::{ActivationEffect, Row};
use crate::widget::button_controller::ActivationSource;

/// Whether a row's activation can be undone by the interaction model itself.
///
/// This is the answer to "what would `on_cancel()` restore?", recorded at the
/// moment the gesture settles rather than assumed by the caller. A host that
/// sees [`RevertKind::FireAndSettle`] knows the `Cancel` tier it is being
/// handed reverts *nothing but the dispatch itself*, and must not render an
/// undo affordance for the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevertKind {
    /// The row changed a value the pre-gesture snapshot still holds, so a
    /// cancel is a real revert.
    Value,
    /// A command was dispatched and the gesture took the tree with it. The
    /// cancel reverts the dispatch, not the command.
    FireAndSettle,
    /// The gesture moved through the tree rather than changing anything.
    Navigation,
    /// Nothing was dispatched at all.
    Nothing,
}

impl RevertKind {
    /// Whether a cancel through this kind restores a value the user can see.
    #[must_use]
    pub const fn restores_value(self) -> bool {
        matches!(self, Self::Value)
    }

    /// The gallery/manifest slug for this kind.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::FireAndSettle => "fire-and-settle",
            Self::Navigation => "navigation",
            Self::Nothing => "nothing",
        }
    }
}

/// Classify what cancelling a row activation would restore.
///
/// Reads the effect the model reported and the row's own
/// [`ClosePolicy`](super::model::ClosePolicy) — the same two facts that decide
/// whether the tree goes with the activation, which is exactly what decides
/// whether anything is left to revert.
#[must_use]
pub fn row_revert(row: &Row, effect: &ActivationEffect) -> RevertKind {
    match effect {
        ActivationEffect::Absorbed => RevertKind::Nothing,
        ActivationEffect::OpenSubmenu { .. } => RevertKind::Navigation,
        ActivationEffect::Command => RevertKind::FireAndSettle,
        ActivationEffect::Toggled { .. } | ActivationEffect::Selected { .. } => {
            if row.closes_menu_on_activate() {
                RevertKind::FireAndSettle
            } else {
                RevertKind::Value
            }
        }
    }
}

/// Does a row with this projection need its activations gated?
///
/// The mirror of [`ButtonModel::tracks_action`](crate::widget::button_model::ButtonModel::tracks_action):
/// work is underway, or the host said it can settle what gets dispatched. A
/// projection with neither is a synchronous command — the gate stays shut so
/// the row fires every press instead of latching on an action nobody will
/// ever report back on.
#[must_use]
pub fn row_tracks_action(pending: &ActionPending) -> bool {
    pending.is_busy() || pending.is_cancellable()
}

/// What a menu-row gesture settled into — the value a `Commit` carries and a
/// `Cancel` hands back.
///
/// The pre-gesture settlement the snapshot holds has `dispatched: false` and
/// `effect: None`, because nothing had been dispatched when the gesture
/// opened. That is what makes the pre-dispatch cancel honest: the host is
/// handed back the exact state before the press, and it says so.
#[derive(Clone, Debug, PartialEq)]
pub struct RowSettlement {
    /// How the user drove the row.
    pub source: ActivationSource,
    /// The row's model key, so a host can address the settlement without
    /// holding the index that produced it.
    pub row_key: String,
    /// What the model changed, once it has. `None` before dispatch.
    pub effect: Option<ActivationEffect>,
    /// What a cancel through this settlement would restore.
    pub revert: RevertKind,
    /// The pending-action projection at the moment of settling.
    pub pending: ActionPending,
    /// Whether a command reached the application.
    pub dispatched: bool,
}

impl RowSettlement {
    /// A settlement for a gesture that has not dispatched anything yet —
    /// what the snapshot holds at gesture start.
    #[must_use]
    pub fn undispatched(row: &Row, source: ActivationSource, pending: ActionPending) -> Self {
        Self {
            source,
            row_key: row.key().to_owned(),
            effect: None,
            revert: RevertKind::Nothing,
            pending,
            dispatched: false,
        }
    }
}

/// One tier's worth of output from the row action engine.
///
/// A sum type rather than three callbacks for the reason the button established
/// ([`ButtonEmission`](crate::widget::button_controller::ButtonEmission)): a
/// host wiring all three is *forced* to write an arm for each, where three
/// optional callbacks degrade into whatever was remembered to be wired.
#[derive(Clone)]
pub enum RowEmission {
    /// `Commit<RowSettlement>` — the activation was dispatched.
    Committed(Commit<RowSettlement>),
    /// `Cancel<RowSettlement>` — the gesture was abandoned, carrying what to
    /// go back to.
    Cancelled(Cancel<RowSettlement>),
    /// `Cancel<CancelRequest>` — the user asked to stop a running action
    /// (`on_cancel_request`), granted only when the host offered a stop path.
    CancelRequested(CancelRequest),
}

impl RowEmission {
    /// The settlement carried by the committed / cancelled tiers.
    #[must_use]
    pub fn settlement(&self) -> Option<&RowSettlement> {
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

impl PartialEq for RowEmission {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers that
    /// deliberately do not implement `PartialEq` — a commit and a cancel of
    /// the same value are *different events*. Matching the tiers first means a
    /// `Committed` can never compare equal to a `Cancelled`.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::CancelRequested(a), Self::CancelRequested(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for RowEmission {
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

/// The per-row memory a menu needs: the open gesture, the activation gate, and
/// the stop request currently on the table.
///
/// Held beside the model, not inside it — see the
/// [module docs](self#why-the-pending-state-is-not-a-field-on-row). A rebuild
/// drops it along with every other piece of interaction state, so a stale
/// `in_flight` latch cannot survive the card being rebuilt.
#[derive(Debug, Default)]
pub struct RowActionEngine {
    /// The open gesture, holding the pre-gesture settlement.
    gesture: GestureSlot<RowSettlement>,
    /// The busy / double-activation gate.
    gate: ActivationGate,
    /// The most recent projection the engine was told about.
    pending: ActionPending,
    /// Whether a stop request has been surfaced for the current busy period.
    cancel_requested: bool,
    /// How many activations this row has actually let through.
    dispatched: usize,
    /// The outcome of the most recent attempt to open a gesture.
    last_begin: Option<GestureStart>,
}

impl RowActionEngine {
    /// An engine with no gesture open and nothing in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the host's current projection. Call once per frame, before any
    /// gesture handling.
    ///
    /// Latches `busy_observed` through the gate and settles once a *tracked*
    /// action has been seen busy and has now come back idle. The
    /// `busy_observed` precondition is what keeps the double-click window
    /// shut: a press that fired, followed by an idle frame the host has not
    /// caught up with, must not clear `in_flight`.
    pub fn observe(&mut self, pending: ActionPending) {
        self.pending = pending;
        self.gate.observe(&pending);
        if row_tracks_action(&pending) && !pending.is_busy() && self.gate.busy_observed() {
            self.gate.settle();
            self.cancel_requested = false;
        }
    }

    /// The projection this engine is currently running against.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.pending
    }

    /// A press began. Opens the gesture so a drag-out has something to
    /// cancel, and surfaces a stop request if the row is already busy.
    pub fn begin(&mut self, row: &Row, source: ActivationSource) -> Option<RowEmission> {
        self.last_begin = Some(self.gesture.begin(&RowSettlement::undispatched(
            row,
            source,
            self.pending,
        )));
        if row_tracks_action(&self.pending) && self.pending.is_busy() {
            return self.request_cancel();
        }
        None
    }

    /// The gesture completed *inside* the row: try to dispatch.
    ///
    /// Returns [`RowEmission::Committed`] when the gate let the command
    /// through, or a stop request when it suppressed one — a release inside a
    /// busy row is not a second action, it is the user pressing again because
    /// nothing happened, which reads as a request to stop.
    ///
    /// A keyboard activation has no press behind it, so a gesture is opened
    /// here first; the snapshot is still captured before the dispatch, which
    /// is the only ordering the tier requires.
    pub fn commit(
        &mut self,
        row: &Row,
        effect: &ActivationEffect,
        source: ActivationSource,
    ) -> Option<RowEmission> {
        if !self.gesture.is_open() {
            self.last_begin = Some(self.gesture.begin(&RowSettlement::undispatched(
                row,
                source,
                self.pending,
            )));
        }

        let activation = if row_tracks_action(&self.pending) {
            self.gate.try_activate(&self.pending)
        } else {
            Activation::Fired
        };

        if activation.is_suppressed() {
            // Nothing was dispatched, so nothing is owed a revert tier. Drop
            // the gesture and let the stop request carry the intent.
            self.gesture.abort();
            return self.request_cancel();
        }

        let settlement = RowSettlement {
            source,
            row_key: row.key().to_owned(),
            effect: Some(effect.clone()),
            revert: row_revert(row, effect),
            pending: self.pending,
            dispatched: true,
        };
        let emission = self.gesture.commit(settlement).map(RowEmission::Committed);
        if emission.is_some() {
            self.dispatched += 1;
        }
        emission
    }

    /// The gesture was abandoned — released outside, or `Escape`.
    ///
    /// With a gesture open, hands back the pre-gesture settlement. With none
    /// open (the action is already running and the user pressed `Escape`) this
    /// degrades to a stop request rather than inventing a revert for a value
    /// that was never captured mid-flight.
    pub fn cancel(&mut self) -> Option<RowEmission> {
        if let Some(cancel) = self.gesture.cancel() {
            return Some(RowEmission::Cancelled(cancel));
        }
        self.request_cancel()
    }

    /// Surface a stop request, once per busy period.
    ///
    /// Refused silently when nothing is running, or when something is running
    /// that the host never offered a stop path for — the spec makes the
    /// request conditional on the action supporting cancellation, so a
    /// fire-and-forget row shows no dead stop affordance.
    fn request_cancel(&mut self) -> Option<RowEmission> {
        if self.cancel_requested {
            return None;
        }
        self.pending.try_cancel().ok().map(|request| {
            self.cancel_requested = true;
            RowEmission::CancelRequested(request)
        })
    }

    /// Whether a gesture is currently open on this row.
    #[must_use]
    pub fn gesture_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// The outcome of the most recent attempt to open a gesture.
    ///
    /// [`GestureStart::AlreadyOpen`] means the nesting rule held: a second
    /// press arrived while a gesture was still open, the slot refused it, and
    /// the **outer** snapshot still governs the eventual cancel.
    #[must_use]
    pub fn last_begin(&self) -> Option<GestureStart> {
        self.last_begin
    }

    /// Whether the gate currently holds this row.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.gate.is_pending()
    }

    /// Whether a `busy` projection has been seen since the last settle.
    #[must_use]
    pub fn busy_observed(&self) -> bool {
        self.gate.busy_observed()
    }

    /// How many activations this row has let through.
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

    /// Drop any open gesture and stop-request state without emitting a tier,
    /// and release the activation gate.
    ///
    /// For teardown, and for the path where the row stops being able to act
    /// mid-gesture. The gate release belongs here for the same reason
    /// [`ModalAction::reset`](crate::surface::modal_tier::ModalAction::reset)
    /// settles its own: a latch left in flight would leave the next holder
    /// of this engine unable to fire at all. Gate counters are deliberately
    /// **kept**: how many activations fired and how many were withheld is
    /// history, not live state.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.gate.settle();
        self.cancel_requested = false;
        self.last_begin = None;
    }
}

impl PartialEq for RowActionEngine {
    /// Compares the engine's observable shape, not its retained payload.
    ///
    /// The settlement inside an open gesture is excluded on purpose: it carries
    /// the row key and the pending projection, and comparing it would make
    /// engine equality depend on label text rather than interaction state.
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
#[path = "row_action_tests.rs"]
mod row_action_tests;
