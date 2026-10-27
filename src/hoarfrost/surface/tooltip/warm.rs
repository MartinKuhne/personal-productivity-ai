//! The tooltip warm-handoff session (`UIF-TOK-007`, `time.delay.tooltip_warm`).
//!
//! # The behaviour the rung exists for
//!
//! §2.6's row says:
//!
//! > `time.delay.tooltip_warm` | `100` | — | Warm-handoff window: moving between
//! > adjacent icon-only controls (`Navbar`, `NavigationRail`) within `100ms` opens
//! > the next tooltip immediately (`0ms` wait)
//!
//! Read as a sentence rather than as a number, that row carries three claims:
//!
//! 1. The **first** tooltip-bearing control the user reaches pays the full
//!    [`HOVER_DELAY`](super::HOVER_DELAY) of 400 ms.
//! 2. Once a tooltip **has been shown**, the next control arrived at inside the
//!    window opens **immediately** — the wait is `0 ms`, not merely a shorter one.
//! 3. The 100 ms is therefore the **window the handoff must happen inside**, and
//!    equivalently the interval after which warmth lapses. It is *not* the wait.
//!
//! The third point is worth stating plainly because the row's shape invites the
//! opposite reading — "a warm arrival waits 100 ms" — and that reading needs a
//! *second*, unspec'd duration for the window while leaving the row's explicit
//! `0ms wait` unimplemented. The spec fixes both numbers, so this module invents
//! neither: the window is
//! [`DELAY_TOOLTIP_WARM`](crate::token::time::DELAY_TOOLTIP_WARM) and the warm
//! wait is [`INSTANT`](crate::token::time::INSTANT).
//!
//! # Why the memory has to be shared
//!
//! Warmth is a property of *the user's session*, not of a control. "The user has
//! demonstrably gone looking for a label" is a fact about the person, and it holds
//! across every icon they are about to hover next. A per-control controller cannot
//! express it: [`HoverController`](super::HoverController) is deliberately
//! per-control, which is exactly why every arrival pays 400 ms today. So the
//! memory lives in a separate value — [`WarmSession`] — that the host owns and
//! every control in the group consults.
//!
//! It lives in the **tooltip subsystem** rather than beside the widgets for the
//! same reason [`hover`](super::hover) does: it answers *when a tooltip may
//! paint*, which is the tooltip's question, not the control's. Putting it under
//! `widget/` would split one timing decision across two subsystems and let a
//! second copy appear next to the next control family that wanted it.
//!
//! # The decay rule
//!
//! Warmth is granted by **visibility** and decays purely with **time**:
//!
//! * The host calls [`WarmSession::note_visible`] on every frame a tooltip is
//!   actually painted. That is the only thing that keeps a session warm.
//! * An arrival is warm when it lands within
//!   [`handoff_window_ms`] of the last visible frame; otherwise it is cold.
//! * Nothing was visible in between ⇒ the session cooled on its own. There is no
//!   timer to cancel, no background task, and no state that can leak.
//!
//! Granting warmth on *visibility* rather than on *arrival* is what makes the
//! rule survive the case that matters: the user rests on an icon for five
//! seconds, its tooltip paints the whole time, and then steps one icon along.
//! The last visible frame is the frame they left, so the handoff is warm.
//! Granting it on arrival instead would have needed the arrival to still be
//! inside a window measured from an arrival that stopped being refreshed long
//! ago.
//!
//! Two consequences are deliberate and tested rather than incidental:
//!
//! * A **cold, still-pending** hover grants no warmth. Nothing was shown, so the
//!   user has not been told anything, and the next control must not be let off
//!   for a wait the user never benefited from.
//! * **Any** visible tooltip warms the session, including one shown by keyboard
//!   focus. Warmth records "a label was on screen", not which channel put it
//!   there. The keyboard channel itself never needs the handoff — its delay is
//!   already `0 ms` — so the only thing this widens is a pointer arriving at a
//!   neighbouring control moments after a tabbed tooltip appeared, which is
//!   precisely the handoff the rung describes.
//!
//! # The session instance *is* the adjacency scope
//!
//! §2.6 says "adjacent icon-only controls", and this layer cannot see geometry:
//! it does not know that two navbar icons touch or that a tab-strip tab is not
//! next to a toolbar button. Rather than invent an adjacency model, the scope is
//! the **session instance**. A host gives one [`WarmSession`] to one group of
//! tooltip-bearing controls — the navbar, the tab strip — and controls in
//! different groups share nothing. Warmth cannot leak from a navbar into an
//! unrelated toolbar because there is no path between them, and a host that
//! wants two bars to share hands them the same session and owns that choice
//! explicitly.
//!
//! # Time enters as a value
//!
//! This module never reads a clock, and follows the precedent
//! [`surface::menu::aim`](crate::surface::menu::aim) sets: every method that
//! cares about *when* takes a [`TimeMs`] the caller supplies, the decision is a
//! pure query against state and that instant, and determinism is proved by
//! replaying an event script twice rather than asserted. The host reads its own
//! clock once per frame and hands the same instant to `note_visible` and to
//! every `warmth_for` in that frame.
//!
//! Unit tests live in the sibling `warm_tests.rs` sidecar.

use crate::surface::tooltip::TimeMs;
use crate::token::time;

/// The window a warm handoff must happen inside, in milliseconds.
///
/// Read from [`time::DELAY_TOOLTIP_WARM`] rather than restated, so this is the
/// crate's consumption point for that rung: retuning the rung retunes the
/// handoff, and a widget cannot quietly carry its own idea of how long the user
/// has been "still in the zone".
///
/// The same number is the decay interval — the gap after the last visible
/// tooltip beyond which the next arrival is cold again. That is not a second
/// value wearing the first's clothes; §2.6's row makes the window and the lapse
/// the same thing, because "within 100ms" is the whole rule.
#[must_use]
pub const fn handoff_window_ms() -> u64 {
    time::DELAY_TOOLTIP_WARM.ms
}

/// What a **cold** arrival waits, in milliseconds.
///
/// The full [`HOVER_DELAY`](super::HOVER_DELAY), which is itself a read-through
/// of [`time::DELAY_TOOLTIP_OPEN`]. Named here so the decision table has both
/// of its answers in one place and a caller never has to remember which of the
/// two rungs a first hover pays.
#[must_use]
pub const fn cold_wait_ms() -> u64 {
    super::HOVER_DELAY
}

/// What a **warm** arrival waits, in milliseconds.
///
/// `0`, read through [`time::INSTANT`]. §2.6's row is explicit — the next
/// tooltip opens "immediately (`0ms` wait)" — and `time.instant` is the rung
/// that means "no wait at all", the same one
/// [`KEYBOARD_FOCUS_DELAY`](super::KEYBOARD_FOCUS_DELAY) reads through for the
/// keyboard channel.
#[must_use]
pub const fn warm_wait_ms() -> u64 {
    time::INSTANT.ms
}

/// What one arrival inherits from the shared session (`UIF-TOK-007`).
///
/// Two arms, because the spec gives two answers and no third. A `Warm` arrival
/// is not "a shorter cold wait" — it is a different rule, and modelling it as
/// its own arm is what stops a caller from tuning the 400 ms down by
/// arithmetic and calling that a handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArrivalWarmth {
    /// No tooltip has been shown, or the window lapsed: pay
    /// [`cold_wait_ms`].
    Cold,
    /// A tooltip was visible inside the window: pay [`warm_wait_ms`].
    Warm,
}

impl ArrivalWarmth {
    /// The wait this verdict carries, in milliseconds.
    #[must_use]
    pub const fn wait_ms(self) -> u64 {
        match self {
            Self::Cold => cold_wait_ms(),
            Self::Warm => warm_wait_ms(),
        }
    }

    /// Whether this arrival was handed off rather than charged the full delay.
    #[must_use]
    pub const fn is_warm(self) -> bool {
        matches!(self, Self::Warm)
    }
}

/// The cross-control memory that makes a warm handoff possible
/// (`UIF-TOK-007`, `time.delay.tooltip_warm`).
///
/// One field. It is not a cache, a registry or a timer — it is the timestamp of
/// the last frame on which a tooltip was actually painted, and nothing more.
/// Every question about warmth is answered against that instant and a `TimeMs`
/// the caller supplies, so the type holds no clock, no closures and no
/// per-control identity.
///
/// The host owns one per group of tooltip-bearing controls and drives it from
/// the same per-frame instant it gives every [`HoverController`](super::HoverController):
///
/// ```text
/// frame(t):
///   for control in group:
///       if controller.tick(t).is_visible():
///           session.note_visible(t)
///   on arrival at the next control:
///       controller.pointer_enter_warm(t, session.warmth_for(t))
/// ```
///
/// See the [module docs](self) for why the session instance is the adjacency
/// scope, and for what does and does not refresh warmth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WarmSession {
    /// The most recent instant at which a tooltip was visible, if one ever was.
    ///
    /// `None` means the session has no memory at all, which is indistinguishable
    /// from cold but worth keeping distinct: it says the user has never been
    /// shown a tooltip in this group, rather than that one lapsed.
    last_visible_at: Option<TimeMs>,
}

impl WarmSession {
    /// A session with no memory — the next arrival is cold.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_visible_at: None,
        }
    }

    /// Record that a tooltip was visible at `at`.
    ///
    /// **Monotonic**: the stored instant only ever moves forward. A host that
    /// reports frames out of order — a re-ordered event queue, two surfaces
    /// driven from different readings of the clock — cannot rewind warmth by
    /// handing over an older instant, which would make the session cool earlier
    /// than the user's actual experience.
    ///
    /// Calling it every frame a tooltip paints is the intended usage, not a
    /// special case: each call simply pushes the last-visible instant to the
    /// current frame, which is what keeps a long hover warm.
    pub fn note_visible(&mut self, at: TimeMs) {
        self.last_visible_at = Some(match self.last_visible_at {
            Some(previous) if previous >= at => previous,
            _ => at,
        });
    }

    /// The verdict for an arrival at `arrival` (`UIF-TOK-007`).
    ///
    /// **A pure query**: no memory of previous calls to this method, and no
    /// mutation. An arrival neither grants nor consumes warmth — only
    /// [`note_visible`](Self::note_visible) grants it, and nothing consumes it —
    /// so asking twice is free and a host may probe a session for several
    /// candidate controls in one frame without side effects.
    ///
    /// The boundary is **inclusive**: a gap of exactly
    /// [`handoff_window_ms`] is still inside the window, because §2.6 says
    /// "within `100ms`". One millisecond past it is cold.
    #[must_use]
    pub fn warmth_for(&self, arrival: TimeMs) -> ArrivalWarmth {
        let Some(last_visible) = self.last_visible_at else {
            return ArrivalWarmth::Cold;
        };
        if arrival.elapsed_since(last_visible) <= handoff_window_ms() {
            ArrivalWarmth::Warm
        } else {
            ArrivalWarmth::Cold
        }
    }

    /// Whether an arrival *now* would be warm.
    ///
    /// Shorthand for the query at the current instant, for a host that only
    /// needs the boolean.
    #[must_use]
    pub fn is_warm(&self, now: TimeMs) -> bool {
        self.warmth_for(now).is_warm()
    }

    /// Milliseconds of warmth left at `now`, or `0` for a cold session.
    ///
    /// A host that has nothing painted and wants to know how long it may stay
    /// idle before the session cools can sleep for exactly this long. `0` means
    /// there is nothing to wait for: either the session was never warm, or it
    /// has already lapsed.
    #[must_use]
    pub fn warmth_remaining(&self, now: TimeMs) -> u64 {
        match self.last_visible_at {
            None => 0,
            Some(last_visible) => {
                handoff_window_ms().saturating_sub(now.elapsed_since(last_visible))
            }
        }
    }

    /// The last instant at which a tooltip was visible, if ever.
    #[must_use]
    pub const fn last_visible(&self) -> Option<TimeMs> {
        self.last_visible_at
    }

    /// Whether the session has ever held a visible tooltip.
    ///
    /// Distinct from [`is_warm`](Self::is_warm): `false` here means the user
    /// has never been shown anything in this group, where a session can have
    /// memory and still be cold.
    #[must_use]
    pub const fn has_memory(&self) -> bool {
        self.last_visible_at.is_some()
    }

    /// Forget everything.
    ///
    /// The host calls this when the whole group loses the user's attention in a
    /// way that is not a hover exit — the window lost focus, the bar was torn
    /// down — so a stale timestamp cannot hand off warmth to a session the
    /// user left a long time ago.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
#[path = "warm_tests.rs"]
mod warm_tests;
