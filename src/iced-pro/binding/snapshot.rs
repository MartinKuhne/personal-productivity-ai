//! The pre-gesture snapshot, and the linear guard that owns it (§3.0.4.2).
//!
//! `on_cancel()` is the tier that is easiest to fake and hardest to actually
//! deliver. "Revert to the state before the gesture" sounds like a callback and
//! turns out to be a **data retention** problem: something has to still hold the
//! value from before the first keystroke, for as long as the gesture is open, and
//! hand it back when the user presses Escape. If nothing holds it, the cancel is
//! a lie.
//!
//! This module makes the retention explicit and makes misuse unrepresentable:
//!
//! - [`Snapshot<T>`] is the retained value, captured at gesture start.
//! - [`Gesture<T>`] is a **linear guard** built on it. `commit` and `cancel` both
//!   consume `self`, so "commit twice" and "commit then cancel" are compile
//!   errors rather than runtime surprises or a boolean that someone forgets to set.
//! - [`GestureSlot<T>`] is the `Option<Gesture<T>>` a stateful host actually
//!   keeps, including the nesting rule.
//!
//! # Why consuming `self` instead of a flag
//!
//! The obvious implementation is a `settled: bool` checked at the top of each
//! method. It works right up until a code path sets the flag in the wrong place,
//! and then it produces exactly the double-commit the type was supposed to
//! prevent — silently, at runtime, in whichever branch nobody tested.
//!
//! Moving ownership instead of recording state means the compiler tracks the
//! discipline for free: after `gesture.commit(x)` the guard is gone, and a second
//! `gesture.commit(y)` is a use-after-move. The rule cannot drift because there is
//! no rule to remember.
//!
//! Unit tests live in the sibling `snapshot_tests.rs` sidecar.

use std::fmt;

use crate::binding::tier::Commit;
use crate::binding::tier::Live;

/// The value as it stood before the gesture began.
///
/// Held by value, not by reference, so it cannot be invalidated by the application
/// mutating the thing it was captured from. A snapshot that aliased live model data
/// would revert to whatever the model became, which is not a revert at all.
#[derive(Clone)]
pub struct Snapshot<T> {
    value: T,
}

impl<T> Snapshot<T> {
    /// Capture a copy of `value` as it stands right now.
    #[must_use]
    pub fn capture(value: &T) -> Self
    where
        T: Clone,
    {
        Self {
            value: value.clone(),
        }
    }

    /// Read the retained value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Take ownership of the retained value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Whether `current` still equals the snapshot.
    ///
    /// Lets a host skip a cancel emission when the user never actually changed
    /// anything — pressing Escape after typing nothing should not wake a
    /// persistence layer with a no-op write.
    #[must_use]
    pub fn matches(&self, current: &T) -> bool
    where
        T: PartialEq,
    {
        self.value == *current
    }
}

impl<T: fmt::Debug> fmt::Debug for Snapshot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Snapshot")
            .field("value", &self.value)
            .finish()
    }
}

/// What the application is told to restore when a gesture is cancelled.
///
/// `on_cancel()` in the spec carries no payload, but the host still needs to know
/// *what* to go back to. Carrying the restored value here means the cancel is
/// self-sufficient: the host does not have to have kept its own copy of
/// pre-gesture state somewhere else, which is the copy that gets out of sync.
#[derive(Clone)]
pub struct Cancel<T> {
    restored: T,
}

impl<T> Cancel<T> {
    /// Build a cancel instruction that restores `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self { restored: value }
    }

    /// The value the application should hold after the revert.
    #[must_use]
    pub fn restored(&self) -> &T {
        &self.restored
    }

    /// Take ownership of the restored value.
    #[must_use]
    pub fn into_restored(self) -> T {
        self.restored
    }
}

impl<T: fmt::Debug> fmt::Debug for Cancel<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cancel")
            .field("restored", &self.restored)
            .finish()
    }
}

/// Outcome of asking to open a gesture that may already be open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureStart {
    /// The slot was empty; a [`Gesture`] is now open.
    Started,
    /// A gesture was already open, so this request was refused and the **outer**
    /// snapshot still governs.
    AlreadyOpen,
}

/// A live gesture, holding the snapshot its `cancel` will restore.
///
/// Linear by construction: `commit` and `cancel` take `self` by value, so each
/// can be called at most once per gesture and the compiler enforces it.
#[derive(Clone)]
pub struct Gesture<T> {
    snapshot: Snapshot<T>,
}

impl<T> Gesture<T> {
    /// Open a gesture over a previously captured snapshot.
    #[must_use]
    pub fn new(snapshot: Snapshot<T>) -> Self {
        Self { snapshot }
    }

    /// The pre-gesture value this gesture will revert to.
    #[must_use]
    pub fn snapshot(&self) -> &T {
        self.snapshot.value()
    }

    /// Emit a live tick for an in-progress value.
    ///
    /// Usable any number of times — the guard is not consumed by *observing*,
    /// only by settling.
    #[must_use]
    pub fn live(&self, value: T) -> Live<T> {
        Live::new(value)
    }

    /// Settle the gesture with `value`, emitting `on_commit` exactly once.
    ///
    /// Consumes the guard. A second commit from the same gesture cannot be
    /// written.
    #[must_use]
    pub fn commit(self, value: T) -> Commit<T> {
        Commit::new(value)
    }

    /// Abandon the gesture, emitting `on_cancel` with the snapshot to restore.
    ///
    /// Consumes the guard, so a cancel followed by a commit cannot be written
    /// either — the two settling paths are mutually exclusive at compile time.
    #[must_use]
    pub fn cancel(self) -> Cancel<T> {
        Cancel::new(self.snapshot.into_value())
    }
}

impl<T: fmt::Debug> fmt::Debug for Gesture<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gesture")
            .field("snapshot", &self.snapshot)
            .finish()
    }
}

/// The `Option<Gesture<T>>` a stateful host actually keeps across frames.
///
/// [`Gesture`] alone cannot be stored and settled later without the host
/// hand-rolling the `Option` and the "only one at a time" rule. This wraps both,
/// so the nesting discipline lives in one place instead of at every call site.
///
/// # The nesting rule: outermost wins
///
/// A second `begin` while a gesture is open is **refused**, not layered. The
/// outer snapshot keeps governing, because the outer gesture's cancel must revert
/// to what the user saw before *that* gesture started. Re-capturing the snapshot
/// on the inner `begin` would silently shorten the outer revert to an
/// intermediate value the user never agreed to.
#[derive(Debug)]
pub struct GestureSlot<T> {
    open: Option<Gesture<T>>,
}

/// A closed slot. Hand-written rather than derived so it does not demand
/// `T: Default` — "no gesture open" is `None`, which needs nothing of the payload.
impl<T> Default for GestureSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GestureSlot<T> {
    /// An empty slot with no gesture open.
    #[must_use]
    pub fn new() -> Self {
        Self { open: None }
    }

    /// Whether a gesture currently holds the slot.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Try to open a gesture, snapshotting `current`.
    ///
    /// On [`GestureStart::AlreadyOpen`] the existing gesture is left untouched and
    /// `current` is discarded — the outer snapshot still governs.
    #[must_use]
    pub fn begin(&mut self, current: &T) -> GestureStart
    where
        T: Clone,
    {
        if self.open.is_some() {
            return GestureStart::AlreadyOpen;
        }
        self.open = Some(Gesture::new(Snapshot::capture(current)));
        GestureStart::Started
    }

    /// Emit a live tick through the open gesture, if there is one.
    ///
    /// A closed slot emits nothing rather than inventing a tick from a value it
    /// never saw a gesture over.
    #[must_use]
    pub fn live(&self, value: T) -> Option<Live<T>> {
        self.open.as_ref().map(|gesture| gesture.live(value))
    }

    /// Settle the open gesture, or emit nothing if none is open.
    pub fn commit(&mut self, value: T) -> Option<Commit<T>> {
        self.open.take().map(|gesture| gesture.commit(value))
    }

    /// Cancel the open gesture, or emit nothing if none is open.
    pub fn cancel(&mut self) -> Option<Cancel<T>> {
        self.open.take().map(|gesture| gesture.cancel())
    }

    /// Drop the open gesture without emitting any tier at all.
    ///
    /// For the teardown path — the widget is going away mid-gesture and neither a
    /// commit nor a revert is owed to anybody.
    pub fn abort(&mut self) {
        self.open = None;
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;
