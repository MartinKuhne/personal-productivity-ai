//! The text field's editing engine — gesture linearity, live coalescing and
//! tiered emission (`UIF-ATOM-002a`, spec §3.0.4.2).
//!
//! [`TextInputController`] is the part of a text field that has to
//! *remember* things: whether an editing session is open, what the value
//! was before the first keystroke of that session, whether a live tick is
//! still owed to the host, and whether the field currently holds focus.
//! Everything else about an input is a pure function of the model and the
//! cursor; this is the residue.
//!
//! # What a "gesture" is for a text field
//!
//! For a button a gesture is a press. A text field has no press — a click
//! only places a caret — so the gesture has to be something else, and the
//! choice decides whether `Escape` is useful or destructive.
//!
//! **The gesture is the editing session, and its snapshot is taken at the
//! first edit after focus, not at focus itself.**
//!
//! Snapshot-at-focus-gain is the tempting rule and it is wrong in two ways.
//! It captures a value on every `Tab` through a form whether or not the
//! user ever types, so `Escape` becomes a live revert of a session nobody
//! opened. Worse, the snapshot goes stale: focus a field holding `"hello"`,
//! leave it focused while the host syncs the field to `"goodbye"`, type one
//! character, press `Escape` — and the user's `Escape` destroys a value
//! they had already been shown and never touched.
//!
//! Snapshot-at-first-edit avoids both. The retained value is always the
//! last one the user saw *before they started changing it*, which is
//! exactly what "undo my edit" means. It also falls out of
//! [`GestureSlot`]'s existing nesting rule rather than needing a new one:
//! `begin` is called on every edit and refused after the first, so the
//! first edit's value governs the eventual cancel and no extra discipline
//! is required to keep it that way.
//!
//! # The four `002a` callbacks, and the tier each one is
//!
//! | Callback | Tier | Produced by |
//! |---|---|---|
//! | `on_change(String)` | [`Live<String>`] | [`note_edit`](TextInputController::note_edit) → [`take_live`](TextInputController::take_live) |
//! | `on_commit(String)` | [`Commit<String>`] | `Enter`, or focus loss with an open session |
//! | `on_clear()` | [`Commit<String>`] of the empty value | the trailing `×`, or `Escape` on an untouched non-empty field |
//! | `on_focus_change(bool)` | not a value tier — the edge that opens and closes sessions | [`note_focus`](TextInputController::note_focus) |
//!
//! Plus `Escape` with an open session, which is [`Cancel<String>`]: the
//! spec's `on_cancel()` from §3.0.4.2, carrying the pre-gesture value the
//! host should restore.
//!
//! `on_clear()` is a **commit** rather than its own tier because that is
//! what its cost profile is: the user pressed a button once, and the host
//! is expected to re-run whatever the field drives. It is a separate
//! *arm* of the sum type so the host can tell "the user cleared this" from
//! "the user typed a selection away and pressed Enter" — two events that
//! carry the same empty string and mean different things.
//!
//! # Why the tiers are a sum type
//!
//! Same argument the button makes: a host wiring all of these should be
//! *forced* to write an arm for each. Three optional callbacks silently
//! degrade into "we remembered to wire the one we demoed", and a match
//! statement cannot silently drop a tier.
//!
//! # The live tier is coalesced, and here is exactly what that means
//!
//! [`LiveCoalescer`] keeps at most one pending tick for this control.
//! Every [`note_edit`](TextInputController::note_edit) replaces the
//! survivor with the newer value, which is lossless because `Live` carries
//! the whole string rather than a delta. Two consequences the widget
//! depends on:
//!
//! - **An unchanged value produces no tick at all.** A keystroke that did
//!   not change the value is not a change, and waking a live-validation
//!   subscriber for it is pure cost.
//! - **A settled tier supersedes a pending tick outright.** When `commit`,
//!   `revert` or `clear` runs, any live tick still waiting is dropped
//!   rather than delivered after the settled value it describes. A host
//!   that has been handed `Commit("abc")` has no use for `Live("ab")`.
//!
//! Both drops are counted ([`coalesced`](TextInputController::coalesced),
//! [`superseded`](TextInputController::superseded)) so the coalescing is
//! observable rather than asserted.
//!
//! [`Live<String>`]: crate::binding::Live
//! [`Commit<String>`]: crate::binding::Commit
//! [`Cancel<String>`]: crate::binding::Cancel
//! [`GestureSlot`]: crate::binding::GestureSlot
//! [`LiveCoalescer`]: crate::binding::LiveCoalescer
//!
//! Unit tests live in the sibling `text_input_controller_tests.rs` sidecar.

use std::fmt;

use crate::binding::Cancel;
use crate::binding::Commit;
use crate::binding::GestureSlot;
use crate::binding::GestureStart;
use crate::binding::Live;
use crate::binding::LiveCoalescer;
use crate::widget::text_input_model::TextInputModel;

/// One tier's worth of output from the editing engine.
#[derive(Clone)]
pub enum TextInputEmission {
    /// `Live<String>` — `on_change`, one coalesced tick per delivery.
    Changed(Live<String>),
    /// `Commit<String>` — `on_commit`, settled by `Enter` or focus loss.
    Committed(Commit<String>),
    /// `Commit<String>` — `on_clear`, the empty value by way of the
    /// trailing `×` or an `Escape` that had no session to revert.
    Cleared(Commit<String>),
    /// `Cancel<String>` — `Escape` inside an open editing session,
    /// carrying the pre-gesture value to restore.
    Cancelled(Cancel<String>),
    /// `on_focus_change(bool)` — the edge that opens and closes sessions.
    ///
    /// Not a value tier: focus is not a value the host owns, it is the
    /// boundary of the window in which edits are one gesture. It shares
    /// the sum type anyway so a host cannot wire three of the four
    /// `002a` callbacks and miss the fourth.
    FocusChanged(bool),
}

impl TextInputEmission {
    /// The value this tier carries, or `None` for the focus edge.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Changed(live) => Some(live.value()),
            Self::Committed(commit) => Some(commit.value()),
            Self::Cleared(commit) => Some(commit.value()),
            Self::Cancelled(cancel) => Some(cancel.restored()),
            Self::FocusChanged(_) => None,
        }
    }

    /// The §3.0.4.2 tier name this arm belongs to.
    ///
    /// `Cleared` reports `"commit"` because that is the tier its cost
    /// profile lives in; the distinct arm is what separates it from a
    /// typed-away empty value, not a distinct tier.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Changed(_) => "live",
            Self::Committed(_) | Self::Cleared(_) => "commit",
            Self::Cancelled(_) => "cancel",
            Self::FocusChanged(_) => "focus",
        }
    }

    /// Whether this arm settles the editing session.
    #[must_use]
    pub const fn settles(&self) -> bool {
        !matches!(self, Self::Changed(_) | Self::FocusChanged(_))
    }

    /// Whether a host should mark the model it drives touched (`UIF-PRIN-009`).
    ///
    /// The bridge between the engine's tier vocabulary and the model's
    /// history, so the rule "which events commit a user to a field" lives
    /// here rather than being re-derived at every wiring site — and every
    /// wiring site getting it subtly different.
    ///
    /// * `Committed` and `Cleared` advance it: both are deliberate acts
    ///   on the value.
    /// * `FocusChanged(false)` advances it — that is a `blur`, which
    ///   §1.1 names outright.
    /// * `Changed` does not: a keystroke is not a commitment, and
    ///   advancing on every keystroke is precisely the 'punish on the
    ///   first character' behaviour the pillar forbids.
    /// * `FocusChanged(true)` does not: taking focus is the opposite of
    ///   finishing.
    /// * `Cancelled` does **not**, which is the interesting one. `Escape`
    ///   restores the pre-edit value and leaves the field where it was;
    ///   marking it touched would let a restored-but-invalid value paint
    ///   red the moment the user tried to back out of their edit.
    #[must_use]
    pub const fn advances_stance(&self) -> bool {
        match self {
            Self::Committed(_) | Self::Cleared(_) | Self::FocusChanged(false) => true,
            Self::Changed(_) | Self::Cancelled(_) | Self::FocusChanged(true) => false,
        }
    }
}

impl PartialEq for TextInputEmission {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers
    /// that deliberately do not implement `PartialEq` — a commit and a
    /// cancel of the same value are *different events*, and comparing them
    /// structurally at the wrapper level would invite exactly that
    /// confusion. The tiers are matched first, so a `Committed` can never
    /// equal a `Cancelled`.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Changed(a), Self::Changed(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cleared(a), Self::Cleared(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::FocusChanged(a), Self::FocusChanged(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for TextInputEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed(live) => f.debug_tuple("Changed").field(live.value()).finish(),
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Cleared(commit) => f.debug_tuple("Cleared").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
            Self::FocusChanged(focused) => f.debug_tuple("FocusChanged").field(focused).finish(),
        }
    }
}

/// The per-control memory a text field needs: the open editing session,
/// the pending live tick, and the focus edge.
///
/// Lives in the widget tree under the [`Ephemeral`] tag, so a rebuild
/// drops it along with every other piece of interaction state. That
/// matters more here than it does for a button: a snapshot retained past
/// the life of the control it describes would revert a *new* field to a
/// *dead* field's value, which is a worse lie than a stale hover.
///
/// [`Ephemeral`]: crate::binding::Ephemeral
#[derive(Debug, Default)]
pub struct TextInputController {
    /// The open editing session, holding the pre-first-edit value.
    gesture: GestureSlot<String>,
    /// At most one live tick, newest wins.
    live: LiveCoalescer<String>,
    /// The newest value this engine has seen.
    current: String,
    /// The value as it stood when the current session opened.
    ///
    /// Mirrors what [`GestureSlot`] holds, because the slot deliberately
    /// exposes no read-only view of its payload: settling is the only way
    /// to get at it, and an engine that could not say "this is what
    /// `Escape` would restore" without consuming the session could not
    /// answer the one question that decides whether to offer `Escape` at
    /// all.
    snapshot: Option<String>,
    /// The focus state last reported through [`note_focus`](Self::note_focus).
    focused: bool,
    /// How many pending live ticks a settled tier discarded.
    superseded: usize,
    /// How many clears this control has emitted.
    clears: usize,
    /// The outcome of the most recent attempt to open a session.
    last_begin: Option<GestureStart>,
}

impl TextInputController {
    /// A controller with no session open, nothing pending, no focus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A controller whose current value is already `value`.
    ///
    /// For a host that adopts the engine mid-edit and wants the first
    /// `Escape` to revert to the value it is presenting rather than to an
    /// empty string.
    #[must_use]
    pub fn with_value(value: impl Into<String>) -> Self {
        Self {
            current: value.into(),
            ..Self::default()
        }
    }

    /// Record the host's projection. Call once per frame, before any
    /// event handling.
    ///
    /// Syncs the engine's `current` from the model **only while no
    /// session is open**. Once the user has started editing, the engine's
    /// value is the truth of what is on screen; overwriting it from the
    /// model mid-session would make a later `Escape` restore a value the
    /// user never typed over, and would report the wrong thing on `Enter`.
    pub fn observe(&mut self, model: &TextInputModel) {
        self.sync_value(&model.value);
    }

    /// Record the host's value directly, without a [`TextInputModel`].
    ///
    /// The same guarded write [`observe`](Self::observe) performs — the
    /// engine's `current` follows the host **only while no session is
    /// open** — exposed so a control that owns a different model can
    /// drive the engine without minting a `TextInputModel` per frame to
    /// carry a string it already has. `observe` is this method with a
    /// projection in front of it, which keeps the one rule about when the
    /// host may overwrite the engine in one place rather than two.
    pub fn sync_value(&mut self, value: &str) {
        if !self.gesture.is_open() {
            self.current = value.to_string();
        }
    }

    /// Record an edit. Returns whether it changed anything.
    ///
    /// Opens the editing session **before** applying the new value, so the
    /// snapshot is the pre-edit string — this is the whole
    /// snapshot-at-first-edit decision, executed. A value identical to the
    /// one already held records nothing and queues no tick.
    pub fn note_edit(&mut self, value: String) -> bool {
        if value == self.current {
            return false;
        }

        self.open_session();
        self.current = value.clone();
        self.live.push(value);
        true
    }

    /// Open the editing session, recording what the value was beforehand.
    ///
    /// Both the first edit of a session and a keyboard settle come
    /// through here, so the snapshot is captured the same way whichever
    /// path opened the session. `GestureSlot`'s refusal keeps the
    /// **first** one authoritative: a second call is a no-op, so the
    /// value an eventual `Escape` restores is the one the user last saw
    /// before they started changing it, never one they typed partway
    /// through.
    fn open_session(&mut self) {
        if self.gesture.is_open() {
            // Record the refusal rather than swallowing it: `last_begin`
            // is the only place the nesting rule becomes observable, and a
            // rule nobody can see is a rule nobody can prove held.
            self.last_begin = Some(GestureStart::AlreadyOpen);
            return;
        }

        self.last_begin = Some(self.gesture.begin(&self.current));
        self.snapshot = Some(self.current.clone());
    }

    /// Take the pending live tick, leaving the slot empty.
    ///
    /// This is the delivery point. The widget calls it once per pass, so
    /// however many edits landed since the last call, exactly one tick —
    /// the newest — comes out.
    pub fn take_live(&mut self) -> Option<String> {
        self.live.take()
    }

    /// `Enter`: settle the value now.
    ///
    /// A keyboard activation has no press behind it, so a session is
    /// opened first when none is open; the snapshot is still captured
    /// before anything settles, which is the only ordering the tier
    /// requires. The pending live tick is discarded rather than delivered,
    /// because the commit carries the same value and says more.
    pub fn commit(&mut self) -> Option<TextInputEmission> {
        self.open_session();
        self.discard_live();
        self.snapshot = None;

        self.gesture
            .commit(self.current.clone())
            .map(TextInputEmission::Committed)
    }

    /// `Escape`: revert to the pre-gesture snapshot.
    ///
    /// Only meaningful with an open session. With none open there is
    /// nothing to revert — the caller decides whether that is instead a
    /// [`clear`](Self::clear) (the spec's "Escape when non-empty") or
    /// nothing at all, because that depends on the input kind and the
    /// model's `clearable` flag, neither of which this engine knows.
    pub fn revert(&mut self) -> Option<TextInputEmission> {
        self.discard_live();
        let cancel = self.gesture.cancel()?;
        self.snapshot = None;
        self.current = cancel.restored().clone();
        Some(TextInputEmission::Cancelled(cancel))
    }

    /// The trailing `×`, or `Escape` on an untouched non-empty field.
    ///
    /// Closes the session rather than committing it. Leaving a session open
    /// after a clear would hand a later `Escape` a snapshot of the value
    /// the user has just deliberately thrown away, and "undo" would put
    /// back something the user asked twice to remove.
    ///
    /// Emits nothing when the field is already empty: clearing nothing is
    /// not an event.
    pub fn clear(&mut self) -> Option<TextInputEmission> {
        if self.current.is_empty() {
            return None;
        }

        self.discard_live();
        self.gesture.abort();
        self.snapshot = None;
        self.current.clear();
        self.clears += 1;
        Some(TextInputEmission::Cleared(Commit::new(String::new())))
    }

    /// Record the focus state and emit the edges.
    ///
    /// Returns zero, one or two emissions in order:
    ///
    /// - rising edge → `FocusChanged(true)`. **No snapshot is taken.**
    ///   That is the decision the module doc argues for: focus-gain is not
    ///   a gesture.
    /// - falling edge → `Committed(v)` if a session is open, then
    ///   `FocusChanged(false)`. The commit comes first because the host
    ///   should see the settled value before it sees that the field is
    ///   gone.
    /// - no edge → nothing.
    ///
    /// A falling edge with no open session commits nothing: the user
    /// focused the field and left it alone, and waking a persistence layer
    /// with a no-op write is exactly what
    /// [`Snapshot::matches`](crate::binding::Snapshot::matches) exists to
    /// avoid.
    pub fn note_focus(&mut self, focused: bool) -> Vec<TextInputEmission> {
        if focused == self.focused {
            return Vec::new();
        }

        self.focused = focused;

        if focused {
            return vec![TextInputEmission::FocusChanged(true)];
        }

        let mut emissions = Vec::new();
        if self.gesture.is_open() {
            // Same discipline as every other settling path: the live tick
            // the session was carrying is superseded by the commit, and
            // the snapshot is spent the moment the session closes.
            self.discard_live();
            self.snapshot = None;

            if let Some(commit) = self.gesture.commit(self.current.clone()) {
                emissions.push(TextInputEmission::Committed(commit));
            }
        }
        emissions.push(TextInputEmission::FocusChanged(false));
        emissions
    }

    /// Drop the open session and any pending tick without emitting a tier.
    ///
    /// For the path where the control stops being able to act — disabled
    /// mid-edit, or torn down. Neither a commit nor a revert is owed to
    /// anybody, and carrying a session into a control that can no longer
    /// settle it would hand the next focus a snapshot predating everything
    /// since.
    ///
    /// Counters are deliberately **kept**: how many clears fired and how
    /// many ticks were superseded is history, not live state.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.discard_live();
        self.snapshot = None;
        self.focused = false;
        self.last_begin = None;
    }

    /// Discard a pending live tick as superseded, counting the drop.
    fn discard_live(&mut self) {
        if self.live.take().is_some() {
            self.superseded += 1;
        }
    }

    /// The value this engine currently holds.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.current
    }

    /// Whether an editing session is open.
    #[must_use]
    pub fn session_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// The value an `Escape` would restore right now, if a session is open.
    #[must_use]
    pub fn snapshot(&self) -> Option<&str> {
        if self.gesture.is_open() {
            self.snapshot.as_deref()
        } else {
            None
        }
    }

    /// The focus state last reported through [`note_focus`](Self::note_focus).
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether a live tick is waiting to be taken.
    #[must_use]
    pub fn live_pending(&self) -> bool {
        self.live.is_pending()
    }

    /// How many pending live values a newer one displaced.
    #[must_use]
    pub fn coalesced(&self) -> usize {
        self.live.coalesced()
    }

    /// How many pending live ticks a settled tier discarded.
    #[must_use]
    pub fn superseded(&self) -> usize {
        self.superseded
    }

    /// How many clears this control has emitted.
    #[must_use]
    pub fn clears(&self) -> usize {
        self.clears
    }

    /// The outcome of the most recent attempt to open a session.
    ///
    /// [`GestureStart::AlreadyOpen`] means the nesting rule held: a second
    /// edit arrived while a session was already open, the slot refused it,
    /// and the **first** edit's snapshot still governs the eventual cancel
    /// (§3.0.4.2).
    #[must_use]
    pub fn last_begin(&self) -> Option<GestureStart> {
        self.last_begin
    }
}

impl PartialEq for TextInputController {
    /// Compares the controller's observable shape, not its retained
    /// payload.
    ///
    /// Two controllers are equal when they hold the same value, the same
    /// focus state, the same counters and the same session-open state. The
    /// snapshot *value* inside an open session is deliberately excluded:
    /// it is a copy of the model, and comparing it would make controller
    /// equality depend on a string the user has not finished typing.
    fn eq(&self, other: &Self) -> bool {
        self.current == other.current
            && self.focused == other.focused
            && self.superseded == other.superseded
            && self.clears == other.clears
            && self.coalesced() == other.coalesced()
            && self.gesture.is_open() == other.gesture.is_open()
            && self.last_begin == other.last_begin
    }
}

#[cfg(test)]
#[path = "text_input_controller_tests.rs"]
mod text_input_controller_tests;
