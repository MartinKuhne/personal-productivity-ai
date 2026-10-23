//! The slider's drag engine — the scrub contract, made structural
//! (§3.5, `UIF-ATOM-005`, §3.0.4.2).
//!
//! §3.5's whole point is the three tiers: `on_change(f64)` continuously
//! during the drag, `on_commit(f64)` **once** on release, `on_cancel()`
//! on `Escape` reverting to the pre-drag value. That is the same shape
//! [`DragGesture`](crate::surface::drag_tier::DragGesture) was built for
//! in `INC-36`, and this module is that shape re-pointed at a `f64`
//! instead of a rectangle: one [`GestureSlot`], snapshot taken at grab,
//! many live ticks, one settle.
//!
//! # Exactly-once comes from the slot, not from a counter
//!
//! [`GestureSlot::commit`] *takes* the open gesture. A second settle
//! through the same gesture has nothing left to settle and returns `None`.
//! The counters on this type are a transcript for the gallery and the log
//! — they describe what happened, they do not make it happen. A host that
//! called [`release`](crate::widget::slider_host::SliderHost::release) twice gets one `Commit` and then
//! nothing, and no branch in this file could have produced the second one.
//!
//! # Release takes no `inside` flag, on purpose
//!
//! The click-shaped controls in this crate gate on whether the pointer is
//! still over the target, because a press that slides off a button is not
//! a click. A slider is the opposite case: the pointer *leaves the track*
//! in the middle of the gesture that the track exists to perform, and the
//! value the user dragged to is still the value they chose.
//!
//! So [`release`](crate::widget::slider_host::SliderHost::release) does not ask where the pointer is. It
//! settles whatever the drag reached. Had it taken an `inside: bool` the
//! way [`ComboHost::release`](crate::widget::combo_host::ComboHost::release)
//! does, a thumb released outside the groove would leave the gesture open
//! forever — a leaked `Live`-only drag whose value is never persisted,
//! which is the exact failure the `Commit` tier exists to prevent.
//!
//! Pointer capture is the same rule seen from the other end: while the
//! gesture is open the host keeps sending it position events wherever the
//! pointer goes, and [`is_capturing`](crate::widget::slider_host::SliderHost::is_capturing) goes false at
//! the settle, so the capture ends with the gesture rather than with the
//! geometry.
//!
//! # Keyboard steps are `Commit`, not `Live`
//!
//! §3.5 lists the arrow keys, `PageUp`/`PageDown`, `Home` and `End`
//! under `on_commit`, not under `on_change`. That is the spec's placement
//! and it is the right one: a keyed step is a decided value, not a
//! preview of one, and holding `→` produces a run of commits rather than
//! a scrub with a settle at the end.
//!
//! A keyed step with a drag already open settles *through the drag's
//! slot*, so the two input paths cannot each produce a settle for one
//! gesture.
//!
//! # No clock
//!
//! A frame is a fraction in and an emission out. Nothing here reads a
//! clock, and nothing here persists — that stays the host's decision at
//! the settle, exactly as [`drag_tier`](crate::surface::drag_tier)
//! leaves it.
//!
//! Unit tests live in the sibling `slider_host_tests.rs` sidecar.

use std::fmt;

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart, Live};
use crate::widget::range_model::{RangeKey, RangeModel};

/// One piece of output the drag produced, already sorted into the tier
/// §3.5's callback belongs to.
///
/// A sum type rather than three optional callbacks so a host wiring all
/// three is forced to write an arm for each and cannot silently drop the
/// one it did not demo — the same reason
/// [`ComboEmission`](crate::widget::combo_host::ComboEmission) exists.
#[derive(Clone)]
pub enum SliderEmission {
    /// The scrub moved — `on_change(f64)`, the `Live` tier.
    Changed(Live<f64>),
    /// The value settled — `on_commit(f64)`, the `Commit` tier.
    Committed(Commit<f64>),
    /// The gesture was abandoned — `Escape`, the `Cancel` tier, carrying
    /// the pre-drag value to restore.
    Cancelled(Cancel<f64>),
}

impl SliderEmission {
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

impl PartialEq for SliderEmission {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers
    /// that deliberately do not implement `PartialEq`. Matching the tier
    /// first means a commit can never compare equal to a cancel.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Changed(a), Self::Changed(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            _ => false,
        }
    }
}

impl fmt::Debug for SliderEmission {
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
pub struct SliderOutcome {
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<SliderEmission>,
}

impl SliderOutcome {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions`.
    #[must_use]
    pub fn emissions(emissions: Vec<SliderEmission>) -> Self {
        Self { emissions }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
    }
}

/// The per-control memory a slider needs: the open gesture, the value the
/// drag has reached, the pointer flags, and the transcript.
#[derive(Debug)]
pub struct SliderHost {
    /// The pre-drag snapshot, held for the cancel tier.
    gesture: GestureSlot<f64>,
    /// Whether a scrub is in progress.
    dragging: bool,
    /// Whether the host should keep routing position events here.
    ///
    /// Distinct from [`dragging`](Self::dragging) only in what it tells
    /// the caller: `dragging` is this type's own state, `captured` is the
    /// instruction to the host that the pointer's location is no longer
    /// its own business until the settle. They are set together and
    /// cleared together, and keeping both named is what makes the capture
    /// rule visible at the boundary rather than implicit in a bool.
    captured: bool,
    /// Whether the pointer is over the control.
    hovered: bool,
    /// Whether a press is currently down.
    pressed: bool,
    /// The value the open drag has reached.
    current: Option<f64>,
    /// The value the open gesture captured, kept so the revert target is
    /// readable without settling anything — the same reason
    /// [`DragGesture::captured`](crate::surface::drag_tier::DragGesture)
    /// exists. [`GestureSlot`] hands its snapshot back only through
    /// `cancel`, and a gallery cell needs to read it while the drag is
    /// still open.
    captured_value: Option<f64>,
    live: usize,
    committed: bool,
    cancelled: bool,
}

impl Clone for SliderHost {
    fn clone(&self) -> Self {
        Self {
            gesture: self.gesture.clone(),
            dragging: self.dragging,
            captured: self.captured,
            hovered: self.hovered,
            pressed: self.pressed,
            current: self.current,
            captured_value: self.captured_value,
            live: self.live,
            committed: self.committed,
            cancelled: self.cancelled,
        }
    }
}

impl Default for SliderHost {
    fn default() -> Self {
        Self::new()
    }
}

impl SliderHost {
    /// A host with no gesture open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gesture: GestureSlot::new(),
            dragging: false,
            captured: false,
            hovered: false,
            pressed: false,
            current: None,
            captured_value: None,
            live: 0,
            committed: false,
            cancelled: false,
        }
    }

    /// Whether a scrub holds the gesture.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// Whether the host should keep sending position events wherever the
    /// pointer goes.
    #[must_use]
    pub const fn is_capturing(&self) -> bool {
        self.captured
    }

    /// Whether the pointer is over the control.
    #[must_use]
    pub const fn hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a press is down.
    #[must_use]
    pub const fn pressed(&self) -> bool {
        self.pressed
    }

    /// The value a cancel of the open drag would restore.
    ///
    /// `None` when no drag is open. This is the same value `cancel` hands
    /// back, readable without settling the gesture — which is what lets a
    /// gallery cell show the revert target while the scrub is still
    /// running.
    #[must_use]
    pub fn snapshot(&self) -> Option<f64> {
        if self.is_open() {
            self.captured_value
        } else {
            None
        }
    }

    /// The value the open drag has reached, if one is open.
    #[must_use]
    pub const fn current(&self) -> Option<f64> {
        self.current
    }

    /// How many `Live` frames the last drag produced.
    #[must_use]
    pub const fn live_ticks(&self) -> usize {
        self.live
    }

    /// Whether the last drag settled with a `Commit`.
    #[must_use]
    pub const fn committed(&self) -> bool {
        self.committed
    }

    /// Whether the last drag was abandoned with a `Cancel`.
    #[must_use]
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the last drag reached a settling tier at all.
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

    // -- pointer ---------------------------------------------------------

    /// The pointer moved onto (`true`) or off (`false`) the control.
    ///
    /// Inert: hover paints, it does not emit. And it does not end a
    /// capture either — the whole point of the capture is that leaving
    /// the control mid-drag changes nothing.
    pub fn hover(&mut self, over: bool) {
        self.hovered = over;
    }

    /// A press started on the track or the thumb at `fraction` of the
    /// travel (`UIF-ATOM-005`).
    ///
    /// The snapshot is captured **before** the value is moved, so
    /// `Escape` hands back what the user saw when they grabbed the
    /// control rather than the first frame of their own drag. A press
    /// that arrives while a drag is already open is refused by the slot's
    /// outermost-wins rule and the outer snapshot keeps governing.
    ///
    /// Positioning is absolute: the value goes to where the pointer is,
    /// not to the thumb's previous position plus a delta. That is what
    /// makes a click anywhere on the groove a jump to that point, which
    /// is what a groove is for.
    pub fn press(&mut self, model: &RangeModel, fraction: f32) -> SliderOutcome {
        if model.disabled_flag() {
            return SliderOutcome::noop();
        }
        let before = model.clamped_value();
        if self.gesture.begin(&before) == GestureStart::Started {
            self.captured_value = Some(before);
        }
        self.dragging = true;
        self.captured = true;
        self.pressed = true;
        self.live = 0;
        self.committed = false;
        self.cancelled = false;
        self.push_live(model.value_at_fraction(fraction))
    }

    /// The pointer moved to `fraction` mid-drag.
    ///
    /// One `Live` frame per call. A call with no drag open emits nothing
    /// rather than inventing a tick it never saw a press over — the same
    /// rule [`GestureSlot::live`] enforces.
    pub fn drag(&mut self, model: &RangeModel, fraction: f32) -> SliderOutcome {
        if !self.dragging || model.disabled_flag() {
            return SliderOutcome::noop();
        }
        self.push_live(model.value_at_fraction(fraction))
    }

    /// The press ended — the spec's settled `on_commit`, exactly once.
    ///
    /// Note the absence of an `inside` parameter; see the
    /// [module docs](self#release-takes-no-inside-flag-on-purpose) for
    /// why that absence is the guarantee.
    pub fn release(&mut self, model: &RangeModel) -> SliderOutcome {
        if !self.dragging {
            return SliderOutcome::noop();
        }
        let value = self.take_current(model);
        let Some(commit) = self.gesture.commit(value) else {
            return SliderOutcome::noop();
        };
        self.committed = true;
        SliderOutcome::emissions(vec![SliderEmission::Committed(commit)])
    }

    /// `Escape` during an active drag — abandon and hand back the
    /// pre-drag value.
    ///
    /// With no gesture held there is nothing to abandon, so `Escape` on a
    /// resting slider emits nothing rather than cancelling a gesture that
    /// never started.
    pub fn cancel(&mut self) -> SliderOutcome {
        self.clear_pointer();
        self.current = None;
        self.captured_value = None;
        let Some(cancel) = self.gesture.cancel() else {
            return SliderOutcome::noop();
        };
        self.cancelled = true;
        SliderOutcome::emissions(vec![SliderEmission::Cancelled(cancel)])
    }

    /// Focus left the control — the spec's `focus loss` settle.
    ///
    /// Settles whatever the drag or the last value reached. A control
    /// that had nothing open reports nothing: tabbing through a slider
    /// the user never touched must not wake a persistence layer.
    pub fn blur(&mut self, model: &RangeModel) -> SliderOutcome {
        if !self.gesture.is_open() {
            self.clear_pointer();
            return SliderOutcome::noop();
        }
        let value = self.take_current(model);
        let Some(commit) = self.gesture.commit(value) else {
            return SliderOutcome::noop();
        };
        self.committed = true;
        SliderOutcome::emissions(vec![SliderEmission::Committed(commit)])
    }

    // -- keyboard --------------------------------------------------------

    /// One keyboard key (`UIF-ATOM-005`).
    ///
    /// Every keyed move is a `Commit`, because §3.5 lists the keys under
    /// `on_commit`. A model with no effective step produces nothing:
    /// §3.5 defines each key as a multiple of `step` and there is no
    /// multiple of nothing — see
    /// [`RangeModel::step_by`](crate::widget::range_model::RangeModel::step_by).
    ///
    /// A key arriving while a drag is open settles *through the drag's
    /// slot*, so the pointer path and the keyboard path can never each
    /// settle the same gesture.
    pub fn key(&mut self, model: &RangeModel, key: RangeKey) -> SliderOutcome {
        if model.disabled_flag() {
            return SliderOutcome::noop();
        }
        let Some(next) = model.apply_key(key) else {
            return SliderOutcome::noop();
        };
        let commit = match self.gesture.commit(next) {
            Some(commit) => commit,
            None => Commit::new(next),
        };
        self.clear_pointer();
        self.current = None;
        self.captured_value = None;
        self.committed = true;
        SliderOutcome::emissions(vec![SliderEmission::Committed(commit)])
    }

    /// Drop pointer state and any held snapshot without emitting.
    ///
    /// The teardown path: the control is going away mid-gesture and
    /// neither a commit nor a revert is owed to anybody.
    pub fn reset(&mut self) {
        self.gesture.abort();
        self.clear_pointer();
        self.hovered = false;
        self.current = None;
        self.captured_value = None;
        self.live = 0;
        self.committed = false;
        self.cancelled = false;
    }

    /// Emit one `Live` frame for a value the drag has reached.
    fn push_live(&mut self, value: f64) -> SliderOutcome {
        self.current = Some(value);
        let Some(tick) = self.gesture.live(value) else {
            return SliderOutcome::noop();
        };
        self.live += 1;
        SliderOutcome::emissions(vec![SliderEmission::Changed(tick)])
    }

    /// Read the settled value and release the pointer, leaving the
    /// snapshot in place for the caller to settle through.
    fn take_current(&mut self, model: &RangeModel) -> f64 {
        let value = self.current.unwrap_or_else(|| model.clamped_value());
        self.clear_pointer();
        self.current = None;
        self.captured_value = None;
        value
    }

    /// Release every pointer-side flag.
    fn clear_pointer(&mut self) {
        self.dragging = false;
        self.captured = false;
        self.pressed = false;
    }
}

#[cfg(test)]
#[path = "slider_host_tests.rs"]
mod slider_host_tests;
