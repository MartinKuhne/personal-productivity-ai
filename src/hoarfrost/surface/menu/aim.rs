//! The diagonal submenu aiming corridor (`UIF-PRIN-008`, `UIF-FMENU-002`).
//!
//! # The behaviour, and why it needs a timer at all
//!
//! A submenu opens beside its parent card. To reach it, the pointer must
//! travel *diagonally* — and on the way there it necessarily crosses the
//! rows sitting between the trigger and the child card. Without a grace
//! window, crossing those rows would retire the branch the user is
//! aiming at before the pointer arrives, and the menu would visibly
//! flicker shut under a hand that was doing exactly the right thing.
//!
//! §1.1 fixes the tolerance at `150ms` (`time.delay.submenu`), and
//! §4.9 adds the second half: hovering the open branch's **own trigger
//! row** preserves the branch outright, with no flicker.
//!
//! # Time enters as a value
//!
//! This module never reads a clock. Every method that cares about *when*
//! takes a [`TimeMs`] the caller supplies, which is what makes the
//! whole corridor testable without sleeping, and what lets a host drive
//! the entire menu from one timestamp per frame.
//!
//! The rule the tests pin is the strong one: **the same sequence of
//! events at the same timestamps produces the same decisions**, with no
//! hidden state anywhere. [`AimCorridor::decision`] is a pure query of
//! the corridor's state against a supplied `now`; nothing advances
//! itself.
//!
//! # The corridor is not an animation
//!
//! Reduced-motion does **not** shorten the grace window, and this is
//! deliberate rather than an oversight. `resolve_ms` exists to collapse
//! transitions for users who do not want things moving; the aiming
//! corridor is not a transition but an *input tolerance* — a window in
//! which the framework declines to read a pointer crossing as a
//! decision. Collapsing it to `0ms` would make menus *harder* to use
//! for exactly the users the setting is meant to help. The full
//! [`DELAY_SUBMENU`] therefore applies regardless.
//!
//! # Who owns what
//!
//! The corridor owns **aiming**: which row's branch is open, where the
//! pointer is, and whether a sibling switch is currently permitted. It
//! deliberately does not own dismissal — [`Collapse`](AimDecision::Collapse)
//! is *reported*, never applied, because closing a menu is the cascade's
//! job and the two must not fight over one surface.
//!
//! Nor is the corridor keyed by menu identity. It tracks one parent
//! card's trigger row by index, and
//! [`MenuCascade`](crate::surface::menu::cascade::MenuCascade) pairs it
//! with the card that owns it. That keeps this type free of `MenuId` and
//! cloneable cheaply, at the cost of one rule the host must keep: one
//! corridor per parent card, reset when that card closes.
//!
//! Unit tests live in the sibling `aim_tests.rs` sidecar.

use crate::surface::tooltip::TimeMs;
use crate::token::time::DELAY_SUBMENU;

/// The corridor's grace window, read from the token rather than restated.
///
/// `UIF-PRIN-001` forbids a duration defined outside `token/`, and the
/// whole point of `time.delay.submenu` existing is that this is the one
/// place the 150ms is consumed.
#[must_use]
pub fn grace_ms() -> u64 {
    DELAY_SUBMENU.ms
}

/// Where the pointer is, as the host reports it (`UIF-PRIN-008`).
///
/// The host reports only facts it actually knows — which row the pointer
/// is over, or that it left — and the corridor does the comparison
/// against its own trigger. A host that had to decide for itself whether
/// a row *was* the trigger would be re-implementing the rule, in the
/// wrong place, with incomplete information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AimTarget {
    /// A row on the parent card, by index.
    ParentRow {
        /// The row the pointer is over.
        row: usize,
    },
    /// Inside the open child branch's own card.
    ChildBranch,
    /// Outside the parent card and outside its open branch.
    Outside,
}

impl AimTarget {
    /// The row this target names, if it names one.
    #[must_use]
    pub const fn row(self) -> Option<usize> {
        match self {
            Self::ParentRow { row } => Some(row),
            Self::ChildBranch | Self::Outside => None,
        }
    }
}

/// What the corridor concludes about the open branch right now
/// (`UIF-PRIN-008`).
///
/// Four live answers plus `Idle`, and they are mutually exclusive by
/// construction: a host can act on one decision per query without
/// reconciling a set of flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AimDecision {
    /// No branch is open in this corridor, so the corridor has nothing
    /// to protect. Normal hover-to-open rules apply.
    Idle,
    /// Keep the branch open. Either the pointer is on the branch's own
    /// trigger row (§4.9's no-flicker clause) or inside the branch
    /// itself.
    Preserve,
    /// The pointer is on a sibling and the corridor is still open, so
    /// the switch is **suppressed** for `remaining_ms` longer.
    Hold {
        /// The sibling the pointer is on.
        row: usize,
        /// Milliseconds left before a switch becomes permitted.
        remaining_ms: u64,
    },
    /// The sibling switch is permitted: the corridor elapsed, or the
    /// pointer moved to a row the corridor has no reason to protect.
    Switch {
        /// The sibling to switch to.
        row: usize,
    },
    /// The pointer is outside both the parent card and its open branch.
    ///
    /// Reported, not applied — see the module docs on who owns
    /// dismissal.
    Collapse,
}

impl AimDecision {
    /// Whether the currently open branch survives this decision.
    #[must_use]
    pub const fn keeps_branch(self) -> bool {
        matches!(self, Self::Preserve | Self::Hold { .. })
    }

    /// Whether the branch should give way to a different row.
    #[must_use]
    pub const fn switches(self) -> bool {
        matches!(self, Self::Switch { .. })
    }
}

/// The aiming corridor's state (`UIF-PRIN-008`, `UIF-FMENU-002`).
///
/// Four fields, no clock, no closures. The corridor starts empty, which
/// reads as [`AimDecision::Idle`] — nothing to protect — so a host can
/// construct it and start feeding events without an init dance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AimCorridor {
    /// The row whose child branch is currently open.
    open_trigger: Option<usize>,
    /// When that branch was opened.
    opened_at: TimeMs,
    /// Where the pointer is.
    target: Option<AimTarget>,
    /// When the pointer arrived at that target.
    target_at: TimeMs,
}

impl AimCorridor {
    /// A corridor with nothing open and no pointer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open_trigger: None,
            opened_at: TimeMs(0),
            target: None,
            target_at: TimeMs(0),
        }
    }

    // -- events ------------------------------------------------------------

    /// Record that `trigger`'s child branch is now open, as of `at`.
    ///
    /// This is the corridor's only source of truth about openness, and
    /// the caller is the one that opened the branch. Setting
    /// `opened_at` here is what starts the grace window for the branch:
    /// a switch attempted immediately after opening is suppressed,
    /// because that is precisely the moment the user is still travelling
    /// diagonally into it.
    pub fn branch_opened(&mut self, trigger: usize, at: TimeMs) {
        self.open_trigger = Some(trigger);
        self.opened_at = at;
    }

    /// Record that the open branch has closed.
    ///
    /// Leaves the pointer target alone: the pointer has not moved by
    /// being told something, and resetting it here would invent an event
    /// the host never reported.
    pub fn branch_closed(&mut self) {
        self.open_trigger = None;
    }

    /// Report where the pointer is, as of `at`.
    ///
    /// The arrival time is recorded **only when the target changes**.
    /// A host that re-reports the same target every frame must not push
    /// the grace window forward forever, or the corridor would never
    /// elapse and a sibling switch could never fire.
    pub fn pointer_enter(&mut self, target: AimTarget, at: TimeMs) {
        if self.target != Some(target) {
            self.target = Some(target);
            self.target_at = at;
        }
    }

    /// Report the pointer leaving the parent card and its open branch.
    pub fn pointer_out(&mut self, at: TimeMs) {
        self.pointer_enter(AimTarget::Outside, at);
    }

    /// Forget everything.
    ///
    /// A host calls this when the parent card closes, so a stale trigger
    /// index cannot survive into the next menu that happens to reuse the
    /// same row numbers.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // -- queries -----------------------------------------------------------

    /// The row whose branch is open, if any.
    #[must_use]
    pub const fn open_trigger(&self) -> Option<usize> {
        self.open_trigger
    }

    /// Where the pointer is, as last reported.
    #[must_use]
    pub const fn target(&self) -> Option<AimTarget> {
        self.target
    }

    /// Whether the corridor is actively suppressing a sibling switch.
    ///
    /// `true` only while a `Hold` is outstanding — the pointer is on a
    /// sibling and the grace window has not elapsed. `Preserve` is not
    /// "aiming": nothing is being held off, the branch is simply where
    /// the pointer wants it.
    #[must_use]
    pub fn is_aiming(&self, now: TimeMs) -> bool {
        matches!(self.decision(now), AimDecision::Hold { .. })
    }

    /// Milliseconds of corridor left before a sibling switch is allowed.
    ///
    /// `0` whenever there is nothing to wait for — no open branch, the
    /// pointer on the trigger, inside the branch, or outside it. A host
    /// scheduling a wake-up to re-evaluate can sleep for exactly this
    /// long and no less.
    #[must_use]
    pub fn corridor_remaining(&self, now: TimeMs) -> u64 {
        match self.decision(now) {
            AimDecision::Hold { remaining_ms, .. } => remaining_ms,
            _ => 0,
        }
    }

    /// The corridor's verdict at time `now`.
    ///
    /// Pure: it reads state and the supplied time and changes nothing.
    /// A host calls it as often as it likes and commits separately with
    /// [`branch_opened`](Self::branch_opened) /
    /// [`branch_closed`](Self::branch_closed).
    #[must_use]
    pub fn decision(&self, now: TimeMs) -> AimDecision {
        let Some(trigger) = self.open_trigger else {
            return AimDecision::Idle;
        };
        match self.target {
            None | Some(AimTarget::Outside) => AimDecision::Collapse,
            // §4.9: the open branch's own trigger row preserves it.
            Some(AimTarget::ParentRow { row }) if row == trigger => AimDecision::Preserve,
            // Crossing into the branch itself is the goal of the whole
            // manoeuvre, so it preserves too.
            Some(AimTarget::ChildBranch) => AimDecision::Preserve,
            Some(AimTarget::ParentRow { row }) => {
                let elapsed = self.elapsed(now);
                let grace = grace_ms();
                if elapsed < grace {
                    AimDecision::Hold {
                        row,
                        remaining_ms: grace.saturating_sub(elapsed),
                    }
                } else {
                    AimDecision::Switch { row }
                }
            }
        }
    }

    /// Elapsed time since the pointer reached its current target,
    /// clamped so a clock that went backwards cannot produce a
    /// negative wait.
    fn elapsed(&self, now: TimeMs) -> u64 {
        now.elapsed_since(self.target_at)
    }
}

#[cfg(test)]
#[path = "aim_tests.rs"]
mod aim_tests;
