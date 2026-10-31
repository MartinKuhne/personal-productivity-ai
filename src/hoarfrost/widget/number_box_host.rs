//! The `NumberBox` host — the stepper's tiers and the entry's settle
//! (§3.5, `UIF-ATOM-005`, §3.0.4.2).
//!
//! Same shape as [`SliderHost`](crate::widget::slider_host): one
//! [`GestureSlot`], the snapshot taken before the first move, many
//! `Live` frames, one `Commit`. What differs is where the frames come
//! from. A slider's are position events; a number box's are stepper
//! repeats, typed text and keyed steps.
//!
//! # The hold's clock is the caller's
//!
//! A press-and-hold on a stepper repeats, so the obvious implementation
//! asks a timer how long the button has been down. That implementation
//! cannot be snapshotted, replayed in a test, or made to produce the
//! same transcript twice — the same reason
//! [`progress`](crate::widget::progress) takes its animation phase as
//! an argument and [`drag_tier`](crate::surface::drag_tier) refuses to
//! read a clock.
//!
//! So [`hold_stepper`](crate::widget::number_box_host::NumberBoxHost::hold_stepper) takes **both** numbers:
//! `elapsed_ms`, the time since the press as the host measured it, and
//! `repeat_interval_ms`, the cadence the host chose. This crate never
//! reads a clock and never picks a cadence for anybody. It answers one
//! question — *how many steps has this hold reached, and what value is
//! it at now* — and the answer is a pure function of the two values it
//! was handed.
//!
//! The first step is taken by [`press_stepper`](crate::widget::number_box_host::NumberBoxHost::press_stepper),
//! not by the hold, because a press that showed nothing until the repeat
//! interval elapsed would read as a dropped click. The hold adds the
//! steps after that, and a hold reported at an elapsed time that has not
//! crossed the next interval emits nothing rather than a second tick for
//! the same instant.
//!
//! # Exactly-once, same source as the slider's
//!
//! [`GestureSlot::commit`] takes the open gesture, so a second settle
//! through the same hold has nothing to settle and returns `None`. The
//! counters describe the transcript; they are not what enforces it.
//!
//! # Typed text that does not parse commits nothing
//!
//! §3.5 gives the model a `validation` field but no callback for a
//! rejected entry, and there is nothing sensible to send: a `Commit` of
//! the old value would be indistinguishable from one that changed
//! something, and a `Commit` of a guessed value would be the control
//! deciding what the user meant. So an unparseable entry produces no
//! emission at all, and the host's own validation words — which the
//! model already carries — are what tells the user why.
//!
//! Unit tests live in the sibling `number_box_host_tests.rs` sidecar.

use std::fmt;

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart, Live};
use crate::widget::number_box::StepDirection;
use crate::widget::range_model::{RangeKey, RangeModel};

/// One piece of output the number box produced, sorted into the tier
/// §3.5's callback belongs to.
#[derive(Clone)]
pub enum NumberBoxEmission {
    /// The value moved during a hold — `on_change(f64)`, the `Live` tier.
    Changed(Live<f64>),
    /// The value settled — `on_commit(f64)`, the `Commit` tier.
    Committed(Commit<f64>),
    /// The gesture was abandoned — `Escape`, the `Cancel` tier.
    Cancelled(Cancel<f64>),
}

impl NumberBoxEmission {
    /// Whether this emission changes the control's value rather than only
    /// previewing where it stands.
    #[must_use]
    pub fn is_value_change(&self) -> bool {
        matches!(self, Self::Committed(_) | Self::Cancelled(_))
    }

    /// The tier's name, for a manifest line.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Changed(_) => "live",
            Self::Committed(_) => "commit",
            Self::Cancelled(_) => "cancel",
        }
    }
}

impl PartialEq for NumberBoxEmission {
    /// Compares the payload each tier carries, matching the tier first so
    /// a commit can never compare equal to a cancel.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Changed(a), Self::Changed(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            _ => false,
        }
    }
}

impl fmt::Debug for NumberBoxEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed(live) => f.debug_tuple("Changed").field(live.value()).finish(),
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
        }
    }
}

/// Everything one host input did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NumberBoxOutcome {
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<NumberBoxEmission>,
}

impl NumberBoxOutcome {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions`.
    #[must_use]
    pub fn emissions(emissions: Vec<NumberBoxEmission>) -> Self {
        Self { emissions }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
    }
}

/// The per-control memory a number box needs: the open gesture, the
/// button being held, how many repeats that hold has reached, and the
/// transcript.
#[derive(Debug)]
pub struct NumberBoxHost {
    /// The pre-gesture snapshot, held for the cancel tier.
    gesture: GestureSlot<f64>,
    /// The direction currently being held, if a stepper button is down.
    held: Option<StepDirection>,
    /// The value the open gesture has reached.
    current: Option<f64>,
    /// The value the open gesture captured, kept so the revert target is
    /// readable without settling — the same reason
    /// [`DragGesture::captured`](crate::surface::drag_tier::DragGesture)
    /// exists.
    captured_value: Option<f64>,
    /// How many steps the open gesture has already been credited with.
    ///
    /// The press itself is step one. Keeping the count rather than a
    /// timestamp is what lets the hold be a pure function of the
    /// elapsed time handed to it: the same `elapsed_ms` reported twice
    /// credits the same number of steps and emits nothing the second
    /// time.
    steps: i64,
    /// Whether the pointer is over the control.
    hovered: bool,
    /// Whether a stepper press is currently down.
    pressed: bool,
    live: usize,
    committed: bool,
    cancelled: bool,
}

impl Clone for NumberBoxHost {
    fn clone(&self) -> Self {
        Self {
            gesture: self.gesture.clone(),
            held: self.held,
            current: self.current,
            captured_value: self.captured_value,
            steps: self.steps,
            hovered: self.hovered,
            pressed: self.pressed,
            live: self.live,
            committed: self.committed,
            cancelled: self.cancelled,
        }
    }
}

impl Default for NumberBoxHost {
    fn default() -> Self {
        Self::new()
    }
}

impl NumberBoxHost {
    /// A host with no gesture open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gesture: GestureSlot::new(),
            held: None,
            current: None,
            captured_value: None,
            steps: 0,
            hovered: false,
            pressed: false,
            live: 0,
            committed: false,
            cancelled: false,
        }
    }

    /// Whether a hold or an edit holds the gesture.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// The direction currently being held, if any.
    #[must_use]
    pub const fn held_direction(&self) -> Option<StepDirection> {
        self.held
    }

    /// The value the open gesture has reached.
    #[must_use]
    pub const fn current(&self) -> Option<f64> {
        self.current
    }

    /// How many steps the open gesture has been credited with.
    #[must_use]
    pub const fn steps(&self) -> i64 {
        self.steps
    }

    /// The value a cancel of the open gesture would restore.
    ///
    /// `None` when no gesture is open. This is the same value `cancel`
    /// hands back, readable without settling.
    #[must_use]
    pub fn snapshot(&self) -> Option<f64> {
        if self.is_open() {
            self.captured_value
        } else {
            None
        }
    }

    /// Whether the pointer is over the control.
    #[must_use]
    pub const fn hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a stepper press is down.
    #[must_use]
    pub const fn pressed(&self) -> bool {
        self.pressed
    }

    /// How many `Live` frames the last hold produced.
    #[must_use]
    pub const fn live_ticks(&self) -> usize {
        self.live
    }

    /// Whether the last gesture settled with a `Commit`.
    #[must_use]
    pub const fn committed(&self) -> bool {
        self.committed
    }

    /// Whether the last gesture was abandoned with a `Cancel`.
    #[must_use]
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the last gesture reached a settling tier at all.
    #[must_use]
    pub const fn settled(&self) -> bool {
        self.committed || self.cancelled
    }

    /// The tier this host last produced.
    #[must_use]
    pub fn last_tier(&self) -> &'static str {
        if self.cancelled {
            "cancel"
        } else if self.committed {
            "commit"
        } else if self.live > 0 {
            "live"
        } else {
            "none"
        }
    }

    // -- the stepper ----------------------------------------------------------

    /// The pointer moved onto (`true`) or off (`false`) the control.
    ///
    /// Inert. It does not end a hold either: a held button whose repeat
    /// stopped because the pointer drifted a pixel would be a hold that
    /// stops for reasons the user cannot see.
    pub fn hover(&mut self, over: bool) {
        self.hovered = over;
    }

    /// A stepper button went down (`UIF-ATOM-005`).
    ///
    /// Takes the first step immediately and opens the gesture over the
    /// value from before it, so a `Cancel` restores what the field showed
    /// before the click rather than the value the click produced.
    pub fn press_stepper(
        &mut self,
        model: &RangeModel,
        direction: StepDirection,
    ) -> NumberBoxOutcome {
        if model.disabled_flag() || model.effective_step().is_none() {
            // A stepper with no step has nothing to step by. Refusing before
            // the snapshot is taken is what keeps `release_stepper` from
            // settling a value that never moved.
            return NumberBoxOutcome::noop();
        }
        let before = model.clamped_value();
        if self.gesture.begin(&before) == GestureStart::Started {
            self.captured_value = Some(before);
        }
        self.held = Some(direction);
        self.pressed = true;
        self.steps = 1;
        self.live = 0;
        self.committed = false;
        self.cancelled = false;
        self.push_live(model.step_by(before, direction.delta()))
    }

    /// The held button is still down `elapsed_ms` after the press, at a
    /// cadence of `repeat_interval_ms` per repeat.
    ///
    /// Both numbers come from the caller; see the
    /// [module docs](self#the-holds-clock-is-the-callers) for why that
    /// is the whole design. Emits one `Live` frame carrying the value the
    /// hold has reached, or nothing when the elapsed time has not crossed
    /// the next repeat — a hold reported twice at the same point in its
    /// cadence must not tick twice.
    ///
    /// A `repeat_interval_ms` of zero emits nothing rather than dividing
    /// by it: a cadence of zero is not an infinitely fast hold, it is no
    /// cadence at all.
    pub fn hold_stepper(
        &mut self,
        model: &RangeModel,
        direction: StepDirection,
        elapsed_ms: u64,
        repeat_interval_ms: u64,
    ) -> NumberBoxOutcome {
        if model.disabled_flag() || repeat_interval_ms == 0 {
            return NumberBoxOutcome::noop();
        }
        if self.held != Some(direction) {
            return NumberBoxOutcome::noop();
        }
        let crossed = (elapsed_ms / repeat_interval_ms) as i64;
        let target = crossed.saturating_add(1);
        if target <= self.steps {
            return NumberBoxOutcome::noop();
        }
        let extra = target - self.steps;
        self.steps = target;
        let from = self.current.unwrap_or_else(|| model.clamped_value());
        self.push_live(model.step_by(from, extra.saturating_mul(direction.delta())))
    }

    /// The held button came up — the spec's `stepper click release`, the
    /// settled `on_commit`, exactly once.
    pub fn release_stepper(&mut self, model: &RangeModel) -> NumberBoxOutcome {
        if self.held.is_none() {
            return NumberBoxOutcome::noop();
        }
        let value = self.current.unwrap_or_else(|| model.clamped_value());
        self.clear_input();
        let Some(commit) = self.gesture.commit(value) else {
            return NumberBoxOutcome::noop();
        };
        self.committed = true;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Committed(commit)])
    }

    // -- the entry ------------------------------------------------------------

    /// A keyboard step on the focused field (`UIF-ATOM-005`).
    ///
    /// A `Commit`, because §3.5 lists the keys under `on_commit`. A
    /// model with no effective step produces nothing for the arrow and
    /// page keys; `Home` and `End` still work, because they name the
    /// ends rather than a distance.
    pub fn key(&mut self, model: &RangeModel, key: RangeKey) -> NumberBoxOutcome {
        if model.disabled_flag() {
            return NumberBoxOutcome::noop();
        }
        let Some(next) = model.apply_key(key) else {
            return NumberBoxOutcome::noop();
        };
        self.clear_input();
        let commit = match self.gesture.commit(next) {
            Some(commit) => commit,
            None => Commit::new(next),
        };
        self.committed = true;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Committed(commit)])
    }

    /// `Enter` with `text` in the field — parse it and settle.
    ///
    /// Unparseable text emits nothing at all; see the
    /// [module docs](self#typed-text-that-does-not-parse-commits-nothing).
    pub fn commit_text(&mut self, model: &RangeModel, text: &str) -> NumberBoxOutcome {
        if model.disabled_flag() {
            return NumberBoxOutcome::noop();
        }
        let Some(next) = model.parse(text) else {
            return NumberBoxOutcome::noop();
        };
        self.clear_input();
        let commit = match self.gesture.commit(next) {
            Some(commit) => commit,
            None => Commit::new(next),
        };
        self.committed = true;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Committed(commit)])
    }

    /// Focus left the field — the spec's `focus loss` settle.
    ///
    /// Settles whatever the hold or the last value reached. Nothing open
    /// means nothing reported.
    pub fn blur(&mut self, model: &RangeModel) -> NumberBoxOutcome {
        if !self.gesture.is_open() {
            self.held = None;
            self.pressed = false;
            return NumberBoxOutcome::noop();
        }
        let value = self.current.unwrap_or_else(|| model.clamped_value());
        self.clear_input();
        let Some(commit) = self.gesture.commit(value) else {
            return NumberBoxOutcome::noop();
        };
        self.committed = true;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Committed(commit)])
    }

    /// `Escape` — abandon the gesture and hand back the pre-gesture value.
    pub fn cancel(&mut self) -> NumberBoxOutcome {
        self.clear_input();
        let Some(cancel) = self.gesture.cancel() else {
            return NumberBoxOutcome::noop();
        };
        self.cancelled = true;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Cancelled(cancel)])
    }

    /// Drop every bit of held state without emitting — the teardown path.
    ///
    /// The counters go with the state they describe. A teardown that left
    /// `live` or `committed` standing would keep reporting a gesture that
    /// no longer exists, and [`last_tier`](Self::last_tier) would answer
    /// for a transcript this host has been told to forget.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.hovered = false;
        self.clear_input();
        self.live = 0;
        self.committed = false;
        self.cancelled = false;
    }

    /// Emit one `Live` frame for a value the hold has reached.
    fn push_live(&mut self, value: Option<f64>) -> NumberBoxOutcome {
        let Some(value) = value else {
            return NumberBoxOutcome::noop();
        };
        self.current = Some(value);
        let Some(tick) = self.gesture.live(value) else {
            return NumberBoxOutcome::noop();
        };
        self.live += 1;
        NumberBoxOutcome::emissions(vec![NumberBoxEmission::Changed(tick)])
    }

    /// Release every held-input flag and the retained snapshot.
    fn clear_input(&mut self) {
        self.held = None;
        self.pressed = false;
        self.current = None;
        self.steps = 0;
        self.captured_value = None;
    }
}

#[cfg(test)]
#[path = "number_box_host_tests.rs"]
mod number_box_host_tests;
