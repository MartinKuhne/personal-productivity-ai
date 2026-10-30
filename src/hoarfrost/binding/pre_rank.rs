//! The Pre-Rank pointer-capture ledger — who holds the pointer, and why
//! `Escape` reaches them before any surface does (§3.0.4.2, `UIF-WIN-003`
//! step 1).
//!
//! # The gap this closes
//!
//! Drag state in this crate is **per-control**. [`SliderHost`](crate::widget::slider_host::SliderHost)
//! holds its own [`GestureSlot`](crate::binding::GestureSlot), [`ThumbDrag`](crate::composite::scroll_view::ThumbDrag)
//! holds its own, [`DragGesture`](crate::surface::drag_tier::DragGesture) holds
//! its own. That is the right ownership — a slider has no business knowing about
//! a splitter — but it leaves one question with nobody to answer it:
//!
//! > *the user is mid-drag and presses `Escape`. Which control is that for?*
//!
//! [`SurfaceRegistry::escape`](crate::surface::zorder::SurfaceRegistry::escape)
//! cannot answer it. It walks *surfaces*, and a drag is not a surface — it is a
//! gesture inside a control that may sit anywhere, including in a band the walk
//! reaches last. So without the rule §3.0.4.2 states, `Escape` goes to whatever
//! holds keyboard focus, the modal dismisses, and the slider the user is still
//! holding keeps tracking a pointer nobody is watching.
//!
//! The ledger is the missing cross-control record. One place answers "is a
//! pointer-captured drag in flight, and whose", so the escape path can ask it
//! **before** it asks any surface.
//!
//! # The rule, verbatim
//!
//! §3.0.4.2, on `on_cancel(Cancel<T>)`:
//!
//! > Pressing `Escape` while any pointer-captured drag (`Slider`,
//! > `RangeSlider`, `SplitDivider`, `DataTable` column resize, `SideSheet`
//! > resize, `MovableOverlay` drag/resize) is active cancels that drag first
//! > (`Pre-Rank`), even if keyboard focus resides elsewhere.
//!
//! Two words in that sentence do the work. **First** is the precedence: the drag
//! cancel runs ahead of the whole rank walk, and the key is spent there — the
//! walk does not also run for the same keypress. **Elsewhere** is the point of
//! the whole rule: the cancel is *not* addressed to the focused control, so
//! focus is irrelevant to whether it fires.
//!
//! # Why the ledger holds no focus
//!
//! It would be easy to give the coordinator a "focused control" field so it can
//! decide whether the drag and the focus collide. That would be a mistake: the
//! rule is unconditional, so any focus information in the decision is
//! information that could be *wrong* and change the outcome. The ledger
//! therefore records the capture and nothing else. Focus-independence is not a
//! branch here; it is the absence of a branch.
//!
//! # Why this lives in `binding/`
//!
//! It is the gesture foundation, not a surface. The ledger holds no surface, no
//! focus, no geometry, no iced and no clock — it is the same company as
//! [`GestureSlot`](crate::binding::GestureSlot), which it complements: the slot
//! answers "what does *this* control revert to", the ledger answers "is *any*
//! control mid-drag". The composition with
//! [`SurfaceRegistry`](crate::surface::zorder::SurfaceRegistry) lives on the
//! registry's side of the house, in
//! [`escape_with_pre_rank`](crate::surface::zorder::escape_with_pre_rank),
//! because `binding/` is vocabulary and must not reach up into the band model.
//!
//! # Exclusivity: one slot, incumbent wins
//!
//! A single pointer can physically capture one drag at a time, so the ledger has
//! one slot. A second [`begin`](PreRankCoordinator::begin) while it is held is
//! **refused**, not silently overwritten, for the same reason
//! [`GestureSlot::begin`](crate::binding::GestureSlot::begin) refuses a nested
//! gesture: overwriting discards the incumbent's registration, and from then on
//! the incumbent's drag is invisible to `Escape` — the user would press the key
//! and nothing would revert. Refusing keeps the incumbent cancellable and hands
//! the caller [`CaptureStart::AlreadyHeld`] with the incumbent's identity, so a
//! host that forgot to release on pointer-up gets told rather than quietly
//! shadowed.
//!
//! The asymmetry with a stack is deliberate. A stack would imply several drags
//! can be legitimately in flight and that `Escape` should unwind them one at a
//! time; neither is true. One pointer, one drag, one slot.
//!
//! # Cancel, never commit
//!
//! The ledger has no commit path. There is no method that takes a `Commit`, no
//! closure it may call that could settle a gesture, and no way to ask it to
//! "finish" a capture. The only settling operation is
//! [`pre_rank_cancel`](PreRankCoordinator::pre_rank_cancel), which accepts a
//! *cancel* closure and nothing else. What the closure does to the control is
//! the control's own [`GestureSlot::cancel`](crate::binding::GestureSlot::cancel) — a consuming call that restores
//! the snapshot taken when the drag began. Settling a drag as a `Commit` from
//! the escape path is therefore not forbidden by convention; there is no route
//! to it.
//!
//! Unit tests live in the sibling `pre_rank_tests.rs` sidecar.

use std::fmt;

/// The class of pointer-captured drag holding the capture.
///
/// The spec names six classes in §3.0.4.2 and introduces them with "any
/// pointer-captured drag", so the list is illustrative rather than closed.
/// [`Scrollbar`](Self::Scrollbar) is added because this crate's scroll-view
/// thumb drag is a pointer capture in exactly the same sense — the offset it
/// reverts to is a [`Snapshot`](crate::binding::snapshot::Snapshot) like any
/// other — and because "focus in a text field while a scrollbar is mid-drag"
/// is the canonical case the rule exists for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragKind {
    /// `Slider` thumb scrub (§3.0.4.2).
    Slider,
    /// `RangeSlider` thumb scrub (§3.0.4.2).
    RangeSlider,
    /// `SplitDivider` pane resize (§3.0.4.2).
    SplitDivider,
    /// `DataTable` column resize (§3.0.4.2).
    ColumnResize,
    /// `SideSheet` resize (§3.0.4.2).
    SideSheetResize,
    /// `MovableOverlay` drag (§3.0.4.2).
    OverlayMove,
    /// `MovableOverlay` resize (§3.0.4.2).
    OverlayResize,
    /// Scroll-view thumb drag — the same class of capture, unnamed by the spec.
    Scrollbar,
}

impl DragKind {
    /// Every class, in the order the spec names them.
    pub const ALL: [Self; 8] = [
        Self::Slider,
        Self::RangeSlider,
        Self::SplitDivider,
        Self::ColumnResize,
        Self::SideSheetResize,
        Self::OverlayMove,
        Self::OverlayResize,
        Self::Scrollbar,
    ];

    /// The class name, for a report or log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Slider => "Slider",
            Self::RangeSlider => "RangeSlider",
            Self::SplitDivider => "SplitDivider",
            Self::ColumnResize => "ColumnResize",
            Self::SideSheetResize => "SideSheetResize",
            Self::OverlayMove => "OverlayMove",
            Self::OverlayResize => "OverlayResize",
            Self::Scrollbar => "Scrollbar",
        }
    }
}

/// Which control holds the capture.
///
/// A [`DragKind`] alone cannot route a cancel: a window has many sliders, and
/// the one that must revert is the one the pointer grabbed. The id is the
/// host's name for that control, and — as with
/// [`SurfaceId`](crate::surface::zorder::SurfaceId) — it is a name the host
/// can speak before the control is mounted and after it is gone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CaptureId(String);

impl CaptureId {
    /// Create an id from any string-like value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Read the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CaptureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for CaptureId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for CaptureId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A registered pointer capture: which control holds it, and what it is dragging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragCapture {
    id: CaptureId,
    kind: DragKind,
}

impl DragCapture {
    /// Register a capture for `id` over a drag of `kind`.
    #[must_use]
    pub fn new(id: impl Into<CaptureId>, kind: DragKind) -> Self {
        Self {
            id: id.into(),
            kind,
        }
    }

    /// The capturing control.
    #[must_use]
    pub fn id(&self) -> &CaptureId {
        &self.id
    }

    /// What class of drag it is.
    #[must_use]
    pub fn kind(&self) -> DragKind {
        self.kind
    }
}

/// What the ledger answered when a control asked to take the capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureStart {
    /// The slot was free; this capture now holds it.
    Started,
    /// Another capture holds the slot, so this request was refused and the
    /// **incumbent** keeps governing.
    AlreadyHeld {
        /// The capture that refused the request.
        held: DragCapture,
    },
}

impl CaptureStart {
    /// Whether the caller got the capture.
    #[must_use]
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Started)
    }

    /// The incumbent that refused, if any.
    #[must_use]
    pub fn held_by(&self) -> Option<&DragCapture> {
        match self {
            Self::Started => None,
            Self::AlreadyHeld { held } => Some(held),
        }
    }
}

/// What the Pre-Rank step did with one `Escape`.
///
/// The distinction the caller needs is between "the key is spent, do not walk
/// the ranks" and "nothing was being dragged, walk the ranks".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreRankOutcome {
    /// A pointer-captured drag was in flight and the cancel closure settled it
    /// with the control's `Cancel` tier. **The `Escape` is consumed here** —
    /// the rank walk must not run for the same keypress, or one keypress would
    /// both revert a drag and dismiss a surface.
    Cancelled {
        /// The capture that was cancelled.
        capture: DragCapture,
    },
    /// A capture was registered, but the control reported nothing to cancel —
    /// its own [`GestureSlot`](crate::binding::GestureSlot) was already empty. The registration is
    /// provably dead, so the slot has been cleared and the caller should
    /// proceed with the rank walk rather than swallow the key on a ghost.
    Stale {
        /// The registration that turned out to be dead.
        capture: DragCapture,
    },
    /// No capture was held. Proceed with the rank walk exactly as if the
    /// Pre-Rank step had not existed.
    NotCaptured,
}

impl PreRankOutcome {
    /// Whether the drag cancel consumed the key.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    /// Whether the caller should go on to the rank walk.
    #[must_use]
    pub fn proceed_with_rank_walk(&self) -> bool {
        !self.is_cancelled()
    }

    /// The capture this outcome is about, when there was one.
    #[must_use]
    pub fn capture(&self) -> Option<&DragCapture> {
        match self {
            Self::Cancelled { capture } | Self::Stale { capture } => Some(capture),
            Self::NotCaptured => None,
        }
    }
}

/// The single-slot record of which control currently holds the pointer.
///
/// A plain value the host owns and passes in explicitly, exactly like the
/// [`GestureSlot`](crate::binding::GestureSlot)s it sits beside — no thread-locals, no registry of
/// registries, no global that a widget reaches for behind the host's back. A
/// host that wants the Pre-Rank rule holds one of these and feeds it to
/// [`escape_with_pre_rank`](crate::surface::zorder::escape_with_pre_rank).
///
/// # Who writes it
///
/// The *host*, at the two moments it already handles for its own drag:
///
/// * when a control's `press` opens a gesture and takes the pointer —
///   [`begin`](Self::begin);
/// * when that control settles or the pointer comes up —
///   [`release`](Self::release).
///
/// The ledger never opens or closes a capture by itself, because it cannot see
/// the pointer. It only remembers what it is told, which is what keeps it
/// honest: if the host never registers a drag, the ledger never claims one.
#[derive(Clone, Debug, Default)]
pub struct PreRankCoordinator {
    held: Option<DragCapture>,
}

impl PreRankCoordinator {
    /// A ledger with nothing captured.
    #[must_use]
    pub fn new() -> Self {
        Self { held: None }
    }

    /// Whether a pointer-captured drag currently holds the slot.
    #[must_use]
    pub fn is_captured(&self) -> bool {
        self.held.is_some()
    }

    /// The capture holding the slot, if any.
    #[must_use]
    pub fn capture(&self) -> Option<&DragCapture> {
        self.held.as_ref()
    }

    /// Take the capture for `capture`.
    ///
    /// A second `begin` while the slot is held is **refused** and the
    /// incumbent keeps governing; see the
    /// [exclusivity rationale](self#exclusivity-one-slot-incumbent-wins). The
    /// refused request is not lost silently — it comes back as
    /// [`CaptureStart::AlreadyHeld`] so the host can see it forgot to
    /// [`release`](Self::release).
    #[must_use]
    pub fn begin(&mut self, capture: DragCapture) -> CaptureStart {
        if let Some(held) = &self.held {
            return CaptureStart::AlreadyHeld { held: held.clone() };
        }
        self.held = Some(capture);
        CaptureStart::Started
    }

    /// Give up the capture for `id` — the pointer-up path.
    ///
    /// Clears the slot only when `id` is the incumbent, and reports whether it
    /// did. A release from a control that is *not* the holder cannot clear
    /// somebody else's capture, which is the mis-wiring this guards against:
    /// two controls settling in the wrong order would otherwise leave the real
    /// drag uncaptured and `Escape` back on the rank walk mid-drag.
    #[must_use]
    pub fn release(&mut self, id: &CaptureId) -> bool {
        if self.held.as_ref().is_some_and(|held| held.id() == id) {
            self.held = None;
            true
        } else {
            false
        }
    }

    /// Drop whatever the slot holds without cancelling it — the teardown path,
    /// mirroring [`GestureSlot::abort`](crate::binding::GestureSlot::abort).
    ///
    /// For when the whole host is going away mid-drag and nobody is owed a
    /// revert. Prefer [`release`](Self::release) on a normal pointer-up.
    #[must_use]
    pub fn clear(&mut self) -> Option<DragCapture> {
        self.held.take()
    }

    /// Run the Pre-Rank step for one `Escape` (§3.0.4.2, `UIF-WIN-003` step 1).
    ///
    /// If a capture is held, `cancel` is called with it and must settle the
    /// control's drag through that control's own cancel path — the one that
    /// restores the snapshot taken when the drag began. Returning `true` means
    /// a `Cancel` tier was produced and the key is spent; `false` means the
    /// control had nothing open, the registration was stale, and the caller
    /// should carry on with the rank walk.
    ///
    /// The capture is taken out of the slot **before** `cancel` runs, and the
    /// closure receives only `&DragCapture` — never the ledger — so a cancel
    /// cannot re-enter, re-take, or extend the capture it is settling.
    ///
    /// With no capture held the closure is never called and the answer is
    /// [`PreRankOutcome::NotCaptured`], which is how "no drag" leaves the
    /// existing rank walk untouched.
    #[must_use]
    pub fn pre_rank_cancel<F>(&mut self, mut cancel: F) -> PreRankOutcome
    where
        F: FnMut(&DragCapture) -> bool,
    {
        let Some(capture) = self.held.take() else {
            return PreRankOutcome::NotCaptured;
        };
        if cancel(&capture) {
            PreRankOutcome::Cancelled { capture }
        } else {
            PreRankOutcome::Stale { capture }
        }
    }
}

#[cfg(test)]
#[path = "pre_rank_tests.rs"]
mod pre_rank_tests;
