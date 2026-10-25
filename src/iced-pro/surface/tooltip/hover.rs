//! The tooltip hover-delay state machine — *when* a tooltip may paint.
//!
//! A tooltip is the one surface whose visibility is a function of **time**, which makes it
//! the only part of the framework that needs a clock. That is also the one thing the rest of
//! the framework refuses to own: every other surface resolves from data alone, and a
//! controller that called [`std::time::Instant::now`] internally could not be tested
//! without sleeping. So time enters here as a **value**, at exactly one boundary:
//!
//! - The **host** owns the clock. It calls [`TimeMs::now`] once per frame and passes that
//!   same value to every controller call in the frame.
//! - The **controller** is pure with respect to it: [`HoverController::tick`] answers
//!   "what should be visible at `now`?" without mutating anything, so the same inputs
//!   always produce the same output and no test ever waits.
//!
//! ## The machine
//!
//! ```text
//!        pointer_enter ────────┐            ┌──(400 ms elapsed)──► Shown
//!                             ├─► Pending ──┤
//!     keyboard_focus_enter ───┘             └──(0 ms: immediate)─► Shown
//!              ▲                            │
//!              │                           └──── exit / dismiss ───────┘
//! ```
//!
//! - [`Visibility::Hidden`] — not hovering.
//! - [`Visibility::Pending`] — arrived, that channel's delay not yet elapsed. **Nothing
//!   paints.** This state is the entire point of the machine: it is what stops a tooltip
//!   firing while the pointer is still travelling across the window.
//! - [`Visibility::Shown`] — the delay belonging to the arrival channel has elapsed while
//!   the user stayed.
//!
//! The two channels share one machine and one `Pending`/`Shown` boundary; only the number
//! being compared against changes. See "Two arrival channels" below.
//!
//! ## Why arrival carries the delay and departure carries none
//!
//! [`HoverController::pointer_exit`] deliberately takes no clock. The delay is a property
//! of *arriving* at a control, so a departure resolved at any later time has the same
//! effect; taking a timestamp there would invite the reader to look for a second, exit-side
//! delay that does not exist.
//!
//! ## Two arrival channels, two delays
//!
//! `UIF-ATOM-007` gives the pointer and the keyboard **different** delays: `400 ms` of
//! rest before a hover tooltip, and `0 ms` for a control reached by keyboard. That is not
//! one timer configured twice — it is a genuine second axis, because the two channels have
//! different false-positive rates. A pointer crosses a control on its way somewhere else
//! constantly; keyboard focus only lands where the user deliberately tabbed to, so a
//! tooltip there is already wanted and any delay is just lag.
//!
//! So the controller records **which channel armed it** ([`ArrivalSource`]) and `tick`
//! applies the delay belonging to that channel. [`HoverController::pointer_enter`] arms
//! with [`super::HOVER_DELAY`]; [`HoverController::keyboard_focus_enter`] arms with
//! [`super::KEYBOARD_FOCUS_DELAY`] and is therefore `Shown` on the very first tick.
//!
//! The channel is decided at arrival and not revisited: if the pointer wanders onto a
//! control that keyboard focus already holds, the tooltip is already showing and there is
//! nothing to re-time. That is the same idempotence rule
//! [`HoverController::pointer_enter`] already implements, applied to a second
//! channel rather than a second call.
//!
//! ## A third case: the warm handoff
//!
//! The two channels above answer *how did the user get here*. `UIF-TOK-007` adds
//! a third question that is orthogonal to both: **has the user already been
//! shown a tooltip somewhere else, moments ago?** If so, `time.delay.tooltip_warm`
//! says the wait is `0 ms` rather than 400 ms.
//!
//! That question is unanswerable inside one controller, because the fact is not
//! about this control. So the controller does not try. The host keeps a shared
//! [`WarmSession`](super::warm::WarmSession) per group of tooltip-bearing
//! controls and hands the verdict in through
//! [`HoverController::pointer_enter_warm`](HoverController::pointer_enter_warm),
//! which arms the machine with the wait that verdict carries. Everything else
//! here — the `Pending` boundary, idempotence, suppression — is unchanged,
//! because the handoff only changes *which number* the boundary is drawn
//! against.
//!
//! A cold verdict defers to this controller's own configured delay rather than
//! to the token, so the handoff can only ever *remove* a wait, never override
//! one. See [`HoverController::pointer_enter_warm`](HoverController::pointer_enter_warm).
//!
//! ## Suppression
//!
//! [`HoverController::dismiss`] (a click, an Escape) hides the tooltip **and suppresses
//! it** until the pointer leaves the control. Without that, a click that also moves the
//! cursor one pixel would re-arm the timer and the tooltip would reappear over the very
//! thing the user just acted on. See [`DismissReason`] for which exits clear suppression
//! and which do not.
//!
//! Both delay values are **specified** — `UIF-ATOM-007` fixes the hover delay at 400 ms and
//! the keyboard-focus delay at 0 ms, and both are read from
//! [`super::HOVER_DELAY`] / [`super::KEYBOARD_FOCUS_DELAY`] rather than restated here.
//! What the spec still does not cover is the absence of a re-show delay and the suppression
//! rule; those stay framework decisions recorded in the gap list in the
//! [module docs](super).
//!
//! Unit tests live in the sibling `hover_tests.rs` sidecar.

use std::time::{Duration, Instant};

use super::warm::{ArrivalWarmth, warm_wait_ms};

/// Process-wide origin for [`TimeMs::now`], fixed the first instant one is taken.
static CLOCK_ORIGIN: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// A moment on the host's monotonic clock, in whole milliseconds.
///
/// A newtype rather than a bare `u64` so a `TimeMs` cannot be confused with the many
/// unrelated millisecond quantities in a UI frame (durations, timeouts, deltas), and so
/// the arithmetic that is easy to get wrong — subtraction across a clock that went
/// backwards — lives in one place with a defined answer.
///
/// Whole milliseconds because the shortest delay this subsystem times is a hover timeout
/// measured in hundreds of them: sub-millisecond resolution would buy nothing and would
/// make expected values in tests unreadable. A `0 ms` keyboard-focus delay is still
/// expressible exactly, which is the only precision that actually matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimeMs(pub u64);

impl TimeMs {
    /// Reads the monotonic clock, milliseconds since this process first asked.
    ///
    /// **The only place in the crate a clock is read.** Call it **once per frame** and pass
    /// the result to every controller call in that frame: two calls that saw two different
    /// `now`s inside one frame would make the tooltip's state depend on the order the
    /// widgets happened to be visited in.
    ///
    /// Relative rather than wall-clock because a tooltip must not care whether the user
    /// changed their system clock mid-hover; only the distance between two instants matters.
    #[must_use]
    pub fn now() -> Self {
        Self(CLOCK_ORIGIN.elapsed().as_millis() as u64)
    }

    /// Wraps a raw millisecond count, for tests and for hosts with their own clock.
    #[must_use]
    pub const fn millis(ms: u64) -> Self {
        Self(ms)
    }

    /// This instant as a millisecond count.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    /// Milliseconds elapsed from `self` to `now`, saturating at zero.
    ///
    /// Saturating rather than wrapping because a monotonic clock should never go backwards —
    /// but if a host hands over two readings that arrived out of order, the honest answer is
    /// "no time has passed", not a panic in the middle of a frame and not the enormous
    /// number `u64::sub` would produce.
    #[must_use]
    pub const fn elapsed_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// Whether a tooltip paints, and whether it is waiting for its delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Not hovering the control. Nothing paints.
    Hidden,
    /// Hovering, delay not yet elapsed. **Nothing paints** — this is the state that keeps a
    /// tooltip from firing while the pointer is still in motion across the window.
    Pending,
    /// The delay elapsed while the pointer stayed. Paint.
    Shown,
}

impl Visibility {
    /// Whether anything should be painted in this state.
    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Shown)
    }
}

/// Why a visible tooltip was dismissed.
///
/// The taxonomy matters because the exits are **not** equivalent: leaving the control
/// forgets the dismissal, acting on it does not. [`HoverController::is_suppressed`]
/// distinguishes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DismissReason {
    /// The pointer left the control. The common case, and the only one that clears
    /// suppression *by itself* — re-entering shows the tooltip again after the delay.
    PointerExit,
    /// The user pressed the mouse while the tooltip showed. Suppresses until the pointer
    /// exits: a click that nudges the cursor must not re-arm the tooltip over the thing the
    /// user just clicked.
    MousePress,
    /// The anchor control disappeared from under the tooltip — the tab closed, the navbar
    /// entry was removed. Treated as an exit, because there is nothing left to hover.
    AnchorGone,
    /// Keyboard focus moved elsewhere while the tooltip showed. Suppresses until the pointer
    /// exits, so the tooltip does not follow the focus back onto the control the user just
    /// left.
    FocusMoved,
    /// Escape was pressed. Suppresses until the pointer exits.
    ///
    /// Note what this does **not** mean: the tooltip does not *own* Escape. The dismissal
    /// is a pre-pass the host runs **before**
    /// [`zorder::SurfaceRegistry::escape`](crate::surface::zorder::SurfaceRegistry::escape),
    /// so the keyboard owner underneath still sees the key. See the [module
    /// docs](super) for why a tooltip is not a
    /// [`zorder::ZBand`](crate::surface::zorder::ZBand).
    Escape,
    /// The window lost the pointer outright (cursor left the window, window lost focus).
    /// Treated as an exit.
    LeftWindow,
}

impl DismissReason {
    /// Whether this exit **clears** suppression, letting a re-enter show the tooltip again.
    ///
    /// The rule: an exit that means "the pointer is gone from the control" clears it,
    /// because the next arrival is a genuinely new arrival. An exit caused by *acting* on
    /// the control does not — the user's intent was to use the control, not to be reminded
    /// of it again.
    #[must_use]
    pub const fn clears_suppression(self) -> bool {
        matches!(
            self,
            Self::PointerExit | Self::AnchorGone | Self::LeftWindow
        )
    }
}

/// What a dismissal did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dismissal {
    /// Why.
    pub reason: DismissReason,
    /// Whether there was a live hover to dismiss - armed and not already suppressed.
    ///
    /// Note this reports **armed**, not *visible*. Whether the armed hover had already
    /// crossed the delay depends on the clock, and [`HoverController::dismiss`]
    /// deliberately takes none (the delay is a property of arrival, not of departure). A
    /// caller that needs the visible answer asks [`tick`](HoverController::tick) with its
    /// own `now`.
    pub was_active: bool,
    /// Whether the tooltip consumed the input that dismissed it.
    ///
    /// **Always `false`.** A tooltip is a presentation layer with no input surface; it must
    /// never swallow a key or a click on the way out. The field exists so the host can
    /// check it rather than have to remember the rule — and so a future change that made a
    /// tooltip consume input would have to change this type, not just a behaviour.
    pub consumed_input: bool,
}

/// Which channel brought the user onto the control, and therefore which delay applies.
///
/// `UIF-ATOM-007` gives the two channels different delays, so the controller has to know
/// which one armed it. The distinction is not cosmetic: applying the pointer's 400 ms to a
/// keyboard arrival would make a tabbed-to control sit mute for almost half a second, and
/// applying the keyboard's 0 ms to a pointer arrival would set a tooltip off every time
/// the cursor crossed a button on its way somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrivalSource {
    /// The pointer arrived; [`super::HOVER_DELAY`] applies.
    Pointer,
    /// Keyboard focus landed; [`super::KEYBOARD_FOCUS_DELAY`] applies.
    Keyboard,
}

impl ArrivalSource {
    /// The delay this channel waits before painting.
    #[must_use]
    pub const fn delay_ms(self) -> u64 {
        match self {
            Self::Pointer => super::HOVER_DELAY,
            Self::Keyboard => super::KEYBOARD_FOCUS_DELAY,
        }
    }
}

/// A live arrival: when it happened, through which channel, and how long it
/// actually waits.
///
/// One value rather than two parallel fields so "armed" and "how it was armed" cannot drift
/// apart — there is no such thing as an arrival instant with no channel, or a channel with
/// no instant.
///
/// The wait rides on the arrival rather than being read off the controller at
/// `tick` time because the delay is a property of *arriving*, which is the same
/// argument the module docs give for `pointer_exit` taking no clock. It is also
/// what lets one controller take a **different** wait per arrival: a cold
/// pointer arrival carries [`HOVER_DELAY`](super::HOVER_DELAY) while a warm
/// handoff from a [`WarmSession`](super::warm::WarmSession) carries `0 ms`,
/// against the same configured controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Arm {
    at: TimeMs,
    by: ArrivalSource,
    wait: Duration,
}

/// Decides *when* a tooltip may paint, from arrival events and a clock the caller supplies.
///
/// One controller per tooltip-bearing control. See the [module docs](self) for the state
/// machine and the clock discipline.
///
/// ```text
/// hidden ──pointer_enter(t0)──────────► pending ──tick(t0+400)──► shown
/// hidden ──keyboard_focus_enter(t0)──► shown on the first tick (0 ms delay)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct HoverController {
    /// The live arrival, or `None` when neither the pointer nor the keyboard is on the
    /// control. Carries the channel so `tick` can pick the right delay.
    armed: Option<Arm>,
    /// Set by a dismissal that outlives the current hover (a click, Escape) and cleared by
    /// an exit. Blocks re-showing while the cursor stays put.
    suppressed: bool,
    /// The delay a pointer arrival waits.
    delay: Duration,
    /// The delay a keyboard-focus arrival waits.
    keyboard_delay: Duration,
}

impl Default for HoverController {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverController {
    /// A controller with the standard [`super::HOVER_DELAY`] and
    /// [`super::KEYBOARD_FOCUS_DELAY`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_delays(
            Duration::from_millis(super::HOVER_DELAY),
            Duration::from_millis(super::KEYBOARD_FOCUS_DELAY),
        )
    }

    /// A controller with the standard keyboard delay and an explicit hover delay.
    ///
    /// Public so a host with a different policy — and a test that wants to cross the
    /// boundary without waiting — can set one. A zero delay makes the tooltip appear on the
    /// first tick, which is a legitimate configuration for a user who has turned hover
    /// delays off; it is not a mistake to support.
    #[must_use]
    pub fn with_delay(delay: Duration) -> Self {
        Self::with_delays(delay, Duration::from_millis(super::KEYBOARD_FOCUS_DELAY))
    }

    /// A controller with both delays set explicitly.
    ///
    /// The two are independent because they answer two different requirements; a host that
    /// only ever changes the hover delay should use [`with_delay`](Self::with_delay) so the
    /// keyboard side keeps the specified `0 ms`.
    #[must_use]
    pub const fn with_delays(hover: Duration, keyboard_focus: Duration) -> Self {
        Self {
            armed: None,
            suppressed: false,
            delay: hover,
            keyboard_delay: keyboard_focus,
        }
    }

    /// Arm the controller from `now` through `by`, waiting `wait`, unless
    /// something is already armed.
    fn arm(&mut self, now: TimeMs, by: ArrivalSource, wait: Duration) -> bool {
        if self.armed.is_some() {
            return false;
        }
        self.armed = Some(Arm { at: now, by, wait });
        self.suppressed = false;
        true
    }

    /// The pointer arrived at the control at `now`.
    ///
    /// **Idempotent while already armed.** A host that calls this every frame the cursor is
    /// over the control — which is the natural way to write one — must not keep resetting
    /// the timer, or the tooltip never appears. Only a real arrival (from `Hidden`, or from
    /// a suppressed state the pointer has since left) re-arms.
    ///
    /// Returns whether this call re-armed the delay.
    pub fn pointer_enter(&mut self, now: TimeMs) -> bool {
        self.arm(now, ArrivalSource::Pointer, self.delay)
    }

    /// The pointer arrived at the control at `now`, carrying a wait already
    /// decided by a shared [`WarmSession`](super::warm::WarmSession)
    /// (`UIF-TOK-007`).
    ///
    /// This is the warm-handoff entry point: the controller does not own the
    /// cross-control memory and cannot answer "has the user just seen a tooltip
    /// elsewhere?" on its own, so the host asks its session and hands the verdict
    /// over here. A [`Warm`](super::warm::ArrivalWarmth::Warm) arrival is armed
    /// with `0 ms` and is `Shown` on the first tick.
    ///
    /// A **cold** arrival is armed with this controller's own configured
    /// [`delay`](Self::delay), not with the token's 400 ms. That is deliberate
    /// and it is an accessibility property: a host that raised the hover delay —
    /// a user who set it to two seconds because tooltips otherwise fire under a
    /// travelling cursor — must not find the handoff path quietly *shortening*
    /// it. The handoff only ever removes a wait the user has already earned; it
    /// never overrides a delay the host was configured with.
    ///
    /// Same idempotence rule as [`pointer_enter`](Self::pointer_enter): an
    /// already-armed controller is left alone, so a host that reports the
    /// pointer every frame cannot re-time an arrival in flight — and cannot turn
    /// a cold wait into a warm one by arriving again.
    ///
    /// Returns whether this call armed the controller.
    pub fn pointer_enter_warm(&mut self, now: TimeMs, warmth: ArrivalWarmth) -> bool {
        let wait = match warmth {
            ArrivalWarmth::Warm => Duration::from_millis(warm_wait_ms()),
            ArrivalWarmth::Cold => self.delay,
        };
        self.arm(now, ArrivalSource::Pointer, wait)
    }

    /// Keyboard focus landed on the control at `now`.
    ///
    /// Arms the controller through [`ArrivalSource::Keyboard`], so `tick` applies
    /// [`super::KEYBOARD_FOCUS_DELAY`] — `0 ms` as `UIF-ATOM-007` specifies — and the
    /// tooltip is `Shown` on the very first tick after this call. A control reached by
    /// `Tab` must not have to wait half a second to explain itself.
    ///
    /// Same idempotence rule as [`pointer_enter`](Self::pointer_enter): if the pointer
    /// already armed the controller, this does not re-arm or switch the channel, because
    /// the tooltip is already on its way and re-timing it would only delay what the user
    /// can already see coming.
    ///
    /// Returns whether this call armed the controller.
    pub fn keyboard_focus_enter(&mut self, now: TimeMs) -> bool {
        self.arm(now, ArrivalSource::Keyboard, self.keyboard_delay)
    }

    /// The pointer left the control.
    ///
    /// Takes no clock on purpose — see the [module docs](self). Clears suppression, so a
    /// later re-enter shows the tooltip again after the delay.
    pub fn pointer_exit(&mut self) {
        self.armed = None;
        self.suppressed = false;
    }

    /// Keyboard focus left the control.
    ///
    /// Identical to [`pointer_exit`](Self::pointer_exit) because the machine holds no
    /// channel-specific state beyond the arrival itself: once the arrival is gone, which
    /// channel it came through no longer means anything. Named separately so a host can
    /// report what actually happened rather than route a keyboard event through a
    /// pointer-named method.
    pub fn keyboard_focus_exit(&mut self) {
        self.pointer_exit();
    }

    /// The tooltip was dismissed for `reason`.
    ///
    /// Hides the tooltip and, unless the reason is itself an exit
    /// ([`DismissReason::clears_suppression`]), suppresses it until the pointer leaves.
    /// Reports what it did through [`Dismissal`]; `consumed_input` is always `false`.
    #[must_use]
    pub fn dismiss(&mut self, reason: DismissReason) -> Dismissal {
        let was_active = self.is_active();
        if reason.clears_suppression() {
            self.armed = None;
            self.suppressed = false;
        } else {
            self.suppressed = true;
        }
        Dismissal {
            reason,
            was_active,
            consumed_input: false,
        }
    }

    /// Whether the tooltip is currently shown or armed to show.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.armed.is_some() && !self.suppressed
    }

    /// Whether a shown tooltip is being held back by a dismissal.
    #[must_use]
    pub fn is_suppressed(&self) -> bool {
        self.suppressed
    }

    /// The arrival instant, if a channel is on the control.
    #[must_use]
    pub fn entered_at(&self) -> Option<TimeMs> {
        self.armed.map(|arm| arm.at)
    }

    /// The channel that armed the controller, if any.
    #[must_use]
    pub fn arrival_source(&self) -> Option<ArrivalSource> {
        self.armed.map(|arm| arm.by)
    }

    /// Whether the current arrival came from the keyboard.
    #[must_use]
    pub fn is_keyboard_armed(&self) -> bool {
        matches!(self.arrival_source(), Some(ArrivalSource::Keyboard))
    }

    /// The configured pointer delay.
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// The configured keyboard-focus delay.
    #[must_use]
    pub const fn keyboard_delay(&self) -> Duration {
        self.keyboard_delay
    }

    /// The delay that actually applies to the current arrival.
    ///
    /// The wait the live arrival carries — the pointer delay for an ordinary
    /// pointer arrival, `0 ms` for a warm handoff, the keyboard delay for a
    /// keyboard arrival. When nothing is armed it reports the configured pointer
    /// delay: a controller at rest has no live wait, and reporting the
    /// pointer's keeps this accessor total rather than optional.
    #[must_use]
    pub fn active_delay(&self) -> Duration {
        match self.armed {
            Some(arm) => arm.wait,
            None => self.delay,
        }
    }

    /// What should be visible at `now`.
    ///
    /// **Pure**: answers without mutating, so a host may call it as often as it likes and a
    /// test may probe the same controller at many instants. The controller does not need to
    /// remember that it became visible — `Shown` is recomputed from the arrival instant
    /// every time, against the wait that arrival carried.
    #[must_use]
    pub fn tick(&self, now: TimeMs) -> Visibility {
        let Some(armed) = self.armed else {
            return Visibility::Hidden;
        };
        if self.suppressed {
            return Visibility::Hidden;
        }
        if now.elapsed_since(armed.at) >= armed.wait.as_millis() as u64 {
            Visibility::Shown
        } else {
            Visibility::Pending
        }
    }
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod hover_tests;
