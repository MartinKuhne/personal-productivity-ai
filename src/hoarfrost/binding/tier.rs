//! The three callback tiers of §3.0.4.2, as three unrelated types.
//!
//! A widget that only ever emits "something happened" forces its host to guess
//! *which* question it is being asked: should I re-validate this, or should I
//! write it to disk? The spec answers by splitting emissions into tiers with
//! different costs and different guarantees, and the split is only real if the
//! host can tell the tiers apart without re-deriving them from context.
//!
//! | Tier | Spec | Fires | Cost of getting it wrong |
//! |---|---|---|---|
//! | [`Live<T>`] | `on_change(T)` | every keystroke, scrub pixel, hover change | expensive work on the frame path → dropped frames |
//! | [`Commit<T>`] | `on_commit(T)` | exactly once, when the interaction settles | persistence skipped, or run a hundred times |
//! | [`Cancel<T>`](crate::binding::snapshot::Cancel) | `on_cancel()` | Escape, or release outside the bounds | the user's undo is a lie |
//!
//! # Why the tiers share no trait
//!
//! The tempting unification is a `trait Payload<T>` implemented by all three so
//! generic dispatch can treat them alike. That would undo the entire section: once
//! the tiers are interchangeable at the type level, a caller can pass a `Live`
//! where a `Commit` is required and nothing complains — which is precisely the bug
//! the tiers exist to make impossible. So `Live<T>` and `Commit<T>` are separate
//! types with no common ancestor beyond what `Clone` and `Debug` give by accident.
//!
//! The cost of that choice is that generic "forward whichever tier arrived" code
//! cannot be written. That is the intended trade: the tier is a decision, and a
//! decision belongs at a call site a human wrote, not inside a generic.
//!
//! # `Live` carries the whole value, and that is what makes coalescing safe
//!
//! A delta-based live stream cannot be dropped: losing one delta corrupts every
//! value after it. `Live<T>` instead carries the **complete** current value, so
//! any number of intermediate ticks can be discarded and the survivor is still
//! correct. [`LiveCoalescer`] exists to exploit exactly that — it keeps at most
//! one pending tick per control per frame and drops the ones it replaces, lossless
//! by construction rather than by luck.
//!
//! Unit tests live in the sibling `tier_tests.rs` sidecar.

use std::fmt;

/// A frame-immediate `on_change(T)` emission — §3.0.4.2 tier 1.
///
/// Emitted on every keystroke, thumb-scrub pixel and hover change. The contract
/// that matters is the one in the spec's own words: it "MUST never block or
/// trigger synchronous disk/network I/O".
///
/// ## Can this type enforce that? No — and the honest answer is worth stating
///
/// `Live<T>` guarantees what *travels with* the value: that it is complete, that
/// it is tagged as live, and that producing one requires no clock, no allocation
/// policy and no I/O of its own. It cannot guarantee anything about the
/// **subscriber**, because a callback the host registers is the host's code.
///
/// What the type system does contribute is the *separation*: because `Live` and
/// `Commit` are distinct types, the host can wire the expensive persistence path
/// to `Commit` only, and a `Live` emission cannot reach it by accident. The
/// blocking risk is then confined to subscribers the host itself chose to attach
/// to the cheap tier. That is containment, not prevention, and pretending
/// otherwise would oversell what a value type can do.
#[derive(Clone)]
pub struct Live<T> {
    value: T,
}

impl<T> Live<T> {
    /// Wrap a complete current value as a live tick.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// The complete value at the moment of the tick.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Take ownership of the value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for Live<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Live").field("value", &self.value).finish()
    }
}

/// A settled `on_commit(T)` emission — §3.0.4.2 tier 2.
///
/// Emitted **exactly once** when an interaction settles: `Enter`, focus loss,
/// pointer release at the end of a drag, or dropdown option selection. This is
/// the tier that is allowed to be expensive, because it fires once per intent
/// rather than once per pixel.
///
/// The "exactly once" guarantee is not a property of this struct standing alone —
/// it is a property of how it is produced. See
/// [`Gesture`](crate::binding::snapshot::Gesture), whose `commit` consumes
/// `self` so that a second commit from one gesture does not compile.
#[derive(Clone)]
pub struct Commit<T> {
    value: T,
}

impl<T> Commit<T> {
    /// Wrap the settled value.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// The settled value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Take ownership of the value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for Commit<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commit")
            .field("value", &self.value)
            .finish()
    }
}

/// Keeps at most one pending [`Live`] tick per control, dropping what it replaces.
///
/// A scrub across a slider at 144 Hz over a 200 px track is ~29 live ticks for one
/// user intent. The subscriber needs the value at the moment it next runs, not a
/// replay of the path. Because `Live` carries the whole value rather than a delta,
/// dropping every intermediate is safe: the survivor is exactly correct.
///
/// This is a *coalescer*, not a queue. There is no way to drain history, because
/// history is not what the tier promises.
#[derive(Debug)]
pub struct LiveCoalescer<T> {
    pending: Option<T>,
    /// How many pending values were displaced by a newer one.
    ///
    /// Recorded rather than silently dropped so the coalescing is observable: a
    /// coalescer whose counter never moves is a coalescer that never ran, and a
    /// test that cannot tell the difference is not proving anything.
    coalesced: usize,
}

impl<T> LiveCoalescer<T> {
    /// An empty coalescer with nothing pending.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: None,
            coalesced: 0,
        }
    }

    /// Record the newest value, discarding any older pending one.
    ///
    /// Returns `true` when an older value was displaced — i.e. when coalescing
    /// actually happened on this call.
    pub fn push(&mut self, value: T) -> bool {
        let displaced = self.pending.replace(value).is_some();
        if displaced {
            self.coalesced += 1;
        }
        displaced
    }

    /// Whether a tick is waiting to be taken.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// How many values have been displaced since construction.
    #[must_use]
    pub fn coalesced(&self) -> usize {
        self.coalesced
    }

    /// Drain the newest pending value, leaving the slot empty.
    pub fn take(&mut self) -> Option<T> {
        self.pending.take()
    }
}

/// An empty coalescer. Hand-written rather than derived so it does not demand
/// `T: Default` — the slot is `None`, which needs nothing of the payload.
impl<T> Default for LiveCoalescer<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tier_tests.rs"]
mod tier_tests;
