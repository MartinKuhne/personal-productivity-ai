//! The movable overlay's drag tiers — the spec's named `on_drag_settle` case
//! (§3.0.4.2, `UIF-MOV-001`).
//!
//! §3.0.4.2 calls out the overlay drag by name as the case where `Live` and
//! `Commit` must not be confused: every frame of the drag is a `Live` tick the
//! host may preview cheaply, and the release is **one** `Commit` — the
//! `on_drag_settle` — that is allowed to persist. Getting that wrong in either
//! direction is what the two tiers exist to make impossible: a commit per
//! frame puts storage on the frame path, and no commit at all means the card
//! the user positioned is never the card that gets saved.
//!
//! # Where the exactly-once guarantee comes from
//!
//! Not from a counter. [`GestureSlot::commit`] *takes* the open gesture, so a
//! second settle through the same gesture has nothing to settle and returns
//! `None`. The counter in this type is a transcript for the gallery and the
//! log, not the mechanism.
//!
//! # What `Cancel` restores
//!
//! The snapshot is captured at the moment the drag begins, before the first
//! delta is applied, so `Escape` mid-drag hands back the exact rectangle the
//! card had when the user grabbed it — not the last clamped frame, and not the
//! persisted value, which may be older. The host applies it back through its
//! own clamped geometry setter, so a restore cannot create a state the clamp
//! forbids.
//!
//! # The clock stays out
//!
//! A drag frame is a value in, a value out. Nothing here reads a clock, and
//! nothing here touches storage — persisting is the host's decision at the
//! settle, never per frame.
//!
//! Unit tests live in the sibling `drag_tier_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart, Live};
use crate::surface::movable::MovableGeometry;

/// One drag gesture's tier transcript.
///
/// Wraps the linear [`GestureSlot`] over the card's geometry and records how
/// many frames of each tier were produced, so a test or a gallery cell can
/// assert the *shape* of a drag — many live ticks, one settle — rather than
/// only the answer to the last call.
#[derive(Debug)]
pub struct DragGesture {
    slot: GestureSlot<MovableGeometry>,
    /// The geometry the open gesture captured, kept so the host can read the
    /// revert target without settling the gesture.
    captured: Option<MovableGeometry>,
    live: usize,
    committed: bool,
    cancelled: bool,
}

impl Clone for DragGesture {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
            captured: self.captured,
            live: self.live,
            committed: self.committed,
            cancelled: self.cancelled,
        }
    }
}

impl Default for DragGesture {
    fn default() -> Self {
        Self::new()
    }
}

impl DragGesture {
    /// A gesture that has not begun.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slot: GestureSlot::new(),
            captured: None,
            live: 0,
            committed: false,
            cancelled: false,
        }
    }

    /// Whether a drag currently holds the gesture.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.slot.is_open()
    }

    /// The card geometry a cancel of the open drag will restore.
    ///
    /// `None` when no drag is open. This is the same value `cancel` hands back,
    /// readable without settling anything.
    #[must_use]
    pub fn snapshot(&self) -> Option<MovableGeometry> {
        if self.is_open() { self.captured } else { None }
    }

    /// Begin a drag over the card's current geometry.
    ///
    /// A second begin while one is open is refused and the **outer** snapshot
    /// keeps governing, so a nested grab cannot silently shorten the revert to
    /// an intermediate frame the user never agreed to.
    pub fn begin(&mut self, geometry: MovableGeometry) -> GestureStart {
        let start = self.slot.begin(&geometry);
        if start == GestureStart::Started {
            self.captured = Some(geometry);
        }
        start
    }

    /// Emit one `Live` frame for the geometry the card has reached.
    ///
    /// Usable any number of times. A closed gesture emits nothing rather than
    /// inventing a tick it never saw a drag over.
    pub fn live(&mut self, geometry: MovableGeometry) -> Option<Live<MovableGeometry>> {
        let tick = self.slot.live(geometry);
        if tick.is_some() {
            self.live += 1;
        }
        tick
    }

    /// Settle the drag — the spec's `on_drag_settle` — exactly once.
    ///
    /// Returns `None` when no drag is open, which is how a second release is
    /// made structurally impossible rather than merely unlikely.
    pub fn commit(&mut self, geometry: MovableGeometry) -> Option<Commit<MovableGeometry>> {
        let commit = self.slot.commit(geometry);
        if commit.is_some() {
            self.committed = true;
            self.captured = None;
        }
        commit
    }

    /// Abandon the drag, handing back the pre-drag geometry to restore.
    pub fn cancel(&mut self) -> Option<Cancel<MovableGeometry>> {
        let cancel = self.slot.cancel();
        if cancel.is_some() {
            self.cancelled = true;
            self.captured = None;
        }
        cancel
    }

    /// Drop the gesture without emitting any tier — the teardown path.
    pub fn abort(&mut self) {
        self.slot.abort();
        self.captured = None;
    }

    /// How many `Live` frames this drag produced.
    #[must_use]
    pub fn live_ticks(&self) -> usize {
        self.live
    }

    /// Whether this drag settled with a `Commit`.
    #[must_use]
    pub fn committed(&self) -> bool {
        self.committed
    }

    /// Whether this drag was abandoned with a `Cancel`.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the drag reached a settling tier at all.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.committed || self.cancelled
    }

    /// The tier name this gesture last produced, for a manifest line.
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
}

impl PartialEq for DragGesture {
    /// Compares the transcript and whether a gesture is open.
    ///
    /// The captured geometry is excluded: it is a copy of the card's
    /// geometry, and comparing it would make gesture equality depend on where
    /// the card happened to be rather than on what the gesture has done.
    fn eq(&self, other: &Self) -> bool {
        self.slot.is_open() == other.slot.is_open()
            && self.live == other.live
            && self.committed == other.committed
            && self.cancelled == other.cancelled
    }
}

#[cfg(test)]
#[path = "drag_tier_tests.rs"]
mod drag_tier_tests;
