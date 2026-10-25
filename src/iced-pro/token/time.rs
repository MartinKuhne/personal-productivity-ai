//! Temporal, motion and choreography tokens (`UIF-TOK-007`).
//!
//! Every duration this framework times — a hover delay, a debounce
//! window, a flyout transition, a toast's whole life — is one of the ten
//! rungs below, and nothing outside `token/` may define one. Before this
//! module existed each consumer declared its own constant in its own file,
//! which is the outside-`token/` numeric definition `UIF-PRIN-001`
//! forbids; the rungs are now the single source those constants read
//! through.
//!
//! # A rung carries its curve
//!
//! §2.6 is a table of *pairs*, not a column of numbers: every duration
//! arrives with an easing, or with an explicit dash saying it has none.
//! A [`DurationToken`] therefore holds both, so a caller cannot take the
//! 180ms of a flyout without also taking the curve that shapes it:
//!
//! * [`Easing::Linear`], [`Easing::EaseOutCubic`] and
//!   [`Easing::EaseInOutCubic`] are the three curves the table names, and
//!   each is *implemented* — [`Easing::apply`] is a pure `f(t: f32) ->
//!   f32` mapping `0.0` to `0.0` and `1.0` to `1.0`, which is what a
//!   caller interpolating between two states actually needs.
//! * The six timer rungs carry `easing: None`, and that dash is
//!   information. A debounce window does not ease into being; treating it
//!   as an un-eased transition would be a lie about what the number is.
//!
//! The split is also load-bearing for reduced motion, because
//! [`is_transition`](DurationToken::is_transition) is exactly the set of
//! rungs §2.6 says a `reduced_motion` flag may act on.
//!
//! # Reduced motion resolves here
//!
//! §2.6 makes `time.instant` the **mandatory** fallback duration for all
//! motion transitions when the host injects `reduced_motion == true`.
//! "Mandatory" is not a suggestion a widget may weigh up per control, so
//! the rule lives in [`resolve_ms`] rather than in every widget that
//! might remember it:
//!
//! ```text
//! resolve_ms(NORMAL, false)  -> 180
//! resolve_ms(NORMAL, true)   ->   0     (time.instant)
//! resolve_ms(DEBOUNCE_LIVE, true) -> 200 (untouched — not a transition)
//! ```
//!
//! The second line is the requirement. The third is the corollary that is
//! easy to get wrong: reduced motion suppresses *animation*, not
//! *timing*. Zeroing a debounce window would fire a provider request on
//! every keystroke, and zeroing a tooltip's hover delay would make the
//! tooltip appear the instant the cursor crossed a control — a
//! "reduced motion" setting that produces *more* visual noise than the
//! setting it replaced. Only rungs with a curve are transitions, and only
//! transitions fall back.
//!
//! [`eased_progress`] is the same rule one step further out: it answers
//! "how far through this transition are we, having elapsed `n` ms?", and
//! returns `1.0` immediately for a resolved duration of zero. Without
//! it, the mandatory fallback would hand every caller a division by zero
//! at exactly the moment the requirement says the transition must be
//! over.
//!
//! # No clock is read here
//!
//! Nothing in this module reads a clock, and nothing in it may. Time
//! enters as a value — the host reads its own clock once per frame and
//! hands the instant over as a
//! [`TimeMs`](crate::surface::tooltip::hover::TimeMs) — which is what
//! keeps a headless test from having to wait for anything. A rung is a
//! *duration*, and a duration is data; an *instant* is a reading. This
//! module owns the first and never the second, so it has no import to
//! forget. The sidecar scans this file's own text for clock-reading
//! identifiers so the rule cannot be broken quietly.
//!
//! # Aliases are names, not rungs
//!
//! §2.6 gives two rungs a second dotted name in parentheses —
//! `time.menu.aim_grace` for [`DELAY_SUBMENU`] and
//! `time.tooltip.hover_delay` for [`DELAY_TOOLTIP_OPEN`]. They are
//! recorded on the rung through [`DurationToken::alias`] rather than
//! declared as eleventh and twelfth constants, because a second constant
//! for one value is a second thing that can drift from the first. The
//! canonical dotted name is on the rung as
//! [`name`](DurationToken::name), and [`DurationToken::ALL`] transcribes
//! the whole table for iteration and diagnostics.
//!
//! Durations are whole milliseconds — the unit
//! [`TimeMs`](crate::surface::tooltip::hover::TimeMs) is written in, and
//! the unit §2.6's table prints.
//!
//! Unit tests live in the sibling `time_tests.rs` sidecar.

/// The easing curve a motion token is bound to (`UIF-TOK-007`).
///
/// Three curves, because §2.6 names three and no more. Each is
/// *implemented* rather than merely named: [`apply`](Easing::apply) is a
/// pure function over normalised progress, so a caller holding a rung can
/// interpolate between two states without inventing the maths itself.
///
/// The three are deliberately distinct rather than one curve with a
/// parameter. `EaseOutCubic` decelerates into its target and is what a
/// control settling under a pointer should do; `EaseInOutCubic` eases
/// away from *both* ends, which is what a scrim or a width transition
/// needs because it has no "from" the user considers resting. A single
/// parameterised curve would let a caller dial in something the spec
/// never approved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Easing {
    /// `Linear` — constant rate: `f(t) = t`.
    ///
    /// Bound to `time.instant`, where the duration is `0` and the curve
    /// is therefore never actually sampled in a rendered frame. It is
    /// still a real curve rather than a placeholder: §2.6 pairs the two
    /// explicitly, and an immediate transition *is* a linear one.
    Linear,
    /// `EaseOutCubic` — fast out, slow in: `f(t) = 1 - (1 - t)³`.
    ///
    /// Bound to `time.fast` and `time.normal`. The motion leaves
    /// immediately at full speed and decelerates into its resting place,
    /// which is the shape a pointer-driven change wants: the user has
    /// already acted, so the response should start now and settle
    /// gently.
    EaseOutCubic,
    /// `EaseInOutCubic` — slow out, slow in:
    /// `f(t) = 4t³` for `t < 0.5`, `1 - (-2t + 2)³ / 2` after.
    ///
    /// Bound to `time.slow`, the largest transition on the scale. At
    /// 240ms a hard start reads as a jump, and the rungs either side of
    /// the midpoint are continuous at `f(0.5) = 0.5`, so the curve has
    /// no kink where the two branches meet.
    EaseInOutCubic,
}

impl Easing {
    /// Every curve in the set, in the order §2.6 introduces them.
    pub const ALL: [Self; 3] = [Self::Linear, Self::EaseOutCubic, Self::EaseInOutCubic];

    /// The curve's value at normalised progress `t`, clamped to
    /// `[0.0, 1.0]`.
    ///
    /// A pure function with three guarantees the sidecar pins for every
    /// curve: `f(0.0) == 0.0`, `f(1.0) == 1.0`, and monotonic
    /// non-decreasing across the interval. Those are what make it safe to
    /// use as an interpolation weight without checking it first.
    ///
    /// The input is clamped rather than rejected because a caller
    /// computing `elapsed / duration` overshoots `1.0` as a matter of
    /// course — the frame after a transition ends still asks — and the
    /// honest answer to "how far past the end are we" is "at the end",
    /// not a value outside the interpolable range.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseOutCubic => {
                let remaining = 1.0 - t;
                1.0 - remaining * remaining * remaining
            }
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let mirrored = 2.0 - 2.0 * t;
                    1.0 - mirrored * mirrored * mirrored / 2.0
                }
            }
        }
    }

    /// The spec's own spelling of this curve.
    #[must_use]
    pub const fn token_name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::EaseOutCubic => "EaseOutCubic",
            Self::EaseInOutCubic => "EaseInOutCubic",
        }
    }
}

/// One rung of the `UIF-TOK-007` duration scale: a duration paired with
/// the curve that shapes it.
///
/// A rung is a value, not a knob. The ten constants in this module are
/// the scale, and a hand-built [`DurationToken`] outside them is the same
/// kind of spec violation a raw numeric literal in a widget is.
///
/// The pairing is the reason this is a struct rather than a `u64`. §2.6
/// never gives a duration without also giving its easing or an explicit
/// dash, and a bare number forces every caller to remember which of the
/// two cases it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DurationToken {
    /// The spec's dotted name for this rung, e.g. `time.delay.submenu`.
    ///
    /// Carried on the rung so a diagnostic, a gallery cell or a ledger
    /// report can say which token it is looking at instead of printing an
    /// anonymous number.
    pub name: &'static str,
    /// The duration in whole milliseconds.
    ///
    /// Whole milliseconds because that is the resolution
    /// [`TimeMs`](crate::surface::tooltip::hover::TimeMs) is kept in:
    /// the shortest rung on the scale is 100ms, so sub-millisecond
    /// precision would buy nothing and would make expected values in
    /// tests unreadable. `0` is a real value here, not an absence — it
    /// is `time.instant`.
    pub ms: u64,
    /// The easing curve this rung is bound to, or `None` where §2.6
    /// prints a dash.
    ///
    /// `None` means this rung is a **timer**, not a transition: there is
    /// nothing being interpolated across frames, so there is no curve to
    /// apply. It is not "linear", and the distinction is not pedantry —
    /// the reduced-motion rule acts on the `Some` set only, so calling a
    /// debounce "linear" would pull it inside a rule that must not touch
    /// it.
    pub easing: Option<Easing>,
    /// A second dotted name §2.6 uses for this rung, if it gives one.
    ///
    /// The spec names `time.menu.aim_grace` and
    /// `time.tooltip.hover_delay` in parentheses. They are recorded here
    /// rather than declared as extra constants so there is exactly one
    /// value per rung; the alias exists so a reader following the
    /// spec's alternate name can find its rung.
    pub alias: Option<&'static str>,
}

impl DurationToken {
    /// Every rung on the scale, in §2.6's table order.
    ///
    /// Transcribed from the table so an invariant checked over `ALL` is
    /// checked over the whole spec section rather than a hand-picked
    /// subset, and so an insertion that misses the list shows up as a
    /// length failure.
    pub const ALL: [Self; 10] = [
        INSTANT,
        FAST,
        NORMAL,
        SLOW,
        DEBOUNCE_LIVE,
        DELAY_SUBMENU,
        DELAY_TOOLTIP_OPEN,
        DELAY_TOOLTIP_WARM,
        HOLD_COPIED,
        TOAST_AUTO_DISMISS,
    ];

    /// Whether this rung is a motion transition rather than a timer —
    /// `true` exactly where the rung carries an easing.
    ///
    /// This is the predicate the reduced-motion rule is scoped to, and
    /// it is derived from the curve rather than asserted separately so
    /// the two cannot disagree. A rung with a curve has something being
    /// animated; a rung without one is waiting for something.
    #[must_use]
    pub const fn is_transition(&self) -> bool {
        self.easing.is_some()
    }

    /// The curve this rung is shaped by, defaulting to
    /// [`Easing::Linear`] for a timer rung.
    ///
    /// The default exists for callers that want *a* curve unconditionally
    /// and do not care that a timer rung has no transition to shape. A
    /// caller that must distinguish the two reads
    /// [`easing`](Self::easing) or [`is_transition`](Self::is_transition)
    /// instead.
    #[must_use]
    pub const fn easing_or_linear(&self) -> Easing {
        match self.easing {
            Some(easing) => easing,
            None => Easing::Linear,
        }
    }

    /// The spec's alternate dotted name for this rung, if it has one.
    #[must_use]
    pub const fn alias(&self) -> Option<&'static str> {
        self.alias
    }
}

/// Resolve an intended duration against an injected `reduced_motion`
/// flag (`UIF-TOK-007`).
///
/// **The mandatory fallback, in one place.** §2.6 makes `time.instant`
/// the required duration for every motion transition when the host
/// reports `reduced_motion == true`, and a rule stated as "every widget
/// must remember to apply this" is a rule that will be forgotten. A
/// caller passes the rung it intended and the flag it was given, and gets
/// back the duration it is actually allowed to animate over.
///
/// * A transition rung under `reduced_motion` resolves to
///   [`INSTANT`] — `0ms`, no animation.
/// * A transition rung without the flag resolves to its own duration.
/// * A timer rung resolves to its own duration **either way**. Reduced
///   motion suppresses animation, not timing: a debounce, a hover delay,
///   a copied-state hold and a toast countdown are not motion, and
///   zeroing them would make the accessibility setting produce *more*
///   churn — a provider request per keystroke, a tooltip firing on
///   cursor transit — rather than less.
///
/// The flag is injected, never read: this function takes the host's
/// answer to the OS setting as an argument and queries nothing, which is
/// the same discipline `widget/progress.rs` already follows.
#[must_use]
pub const fn resolve_ms(intended: DurationToken, reduced_motion: bool) -> u64 {
    if reduced_motion && intended.is_transition() {
        INSTANT.ms
    } else {
        intended.ms
    }
}

/// How far through `intended` a caller is after `elapsed_ms`, already
/// shaped by the rung's easing and already reduced-motion-resolved.
///
/// The answer is the eased fraction in `[0.0, 1.0]`: multiply a
/// from/to pair by it and the pair lands exactly on `to` when the
/// transition's resolved duration has elapsed.
///
/// A resolved duration of `0` — which is what every transition rung
/// becomes under `reduced_motion` — returns `1.0` immediately rather
/// than dividing by zero. That is the requirement read correctly: under
/// reduced motion a transition is not slow, it is *finished*, and the
/// caller should be showing its end state on the frame the transition
/// was requested.
///
/// `elapsed_ms` is whatever the caller has measured against the instant
/// the transition started; it is a value, not a clock read, and this
/// function reads none.
#[must_use]
pub fn eased_progress(intended: DurationToken, elapsed_ms: u64, reduced_motion: bool) -> f32 {
    let total_ms = resolve_ms(intended, reduced_motion);
    if total_ms == 0 {
        return 1.0;
    }
    let fraction = (elapsed_ms as f64 / total_ms as f64) as f32;
    intended.easing_or_linear().apply(fraction)
}

/// `time.instant` — `0ms`, `Linear`.
///
/// §2.6 binds this to three things at once: the immediate press and
/// focus-ring response, the `Expander` body mount/unmount at `0px`
/// per-frame layout shift, and — the load-bearing one — the **mandatory
/// fallback duration for all motion transitions when the injected
/// `reduced_motion` flag is `true`**.
///
/// It is also the value the keyboard tooltip channel uses:
/// `KEYBOARD_FOCUS_DELAY` is `0ms` because a control reached by `Tab`
/// must not wait half a second to explain itself. That constant now
/// reads through this one.
pub const INSTANT: DurationToken = DurationToken {
    name: "time.instant",
    ms: 0,
    easing: Some(Easing::Linear),
    alias: None,
};

/// `time.fast` — `100ms`, `EaseOutCubic`.
///
/// §2.6 binds it to micro-interactions: hover and press fill
/// transitions, `ToggleSwitch` thumb travel, and the `Checkbox` glyph
/// appearing. Short enough to read as instant response, long enough that
/// a fill change is seen as a change rather than a flicker.
///
/// **No consumer exists in this crate yet** — nothing here animates a
/// fill across frames. The rung is declared anyway, on the
/// [`size::SPLITTER_GRAB`](super::size::SPLITTER_GRAB) precedent: §2.6
/// defines it, and the alternative is a widget inventing its own 100ms
/// for the same interaction, which puts a hole through
/// **UIF-PRIN-001**.
pub const FAST: DurationToken = DurationToken {
    name: "time.fast",
    ms: 100,
    easing: Some(Easing::EaseOutCubic),
    alias: None,
};

/// `time.normal` — `180ms`, `EaseOutCubic`.
///
/// §2.6 binds it to the open and close transitions of the flyout
/// family: `ComboBox`, `DatePicker`, `TimePicker`, `ColorPicker` and
/// `SideSheet`. The default for "a thing appeared and it should be seen
/// to arrive".
///
/// **No consumer exists in this crate yet** — see [`FAST`].
pub const NORMAL: DurationToken = DurationToken {
    name: "time.normal",
    ms: 180,
    easing: Some(Easing::EaseOutCubic),
    alias: None,
};

/// `time.slow` — `240ms`, `EaseInOutCubic`.
///
/// §2.6 binds it to the modal scrim fade and the `NavigationRail`
/// compact-to-expanded width transition — the two transitions large
/// enough that a hard start or stop would read as a jump. It is the only
/// rung on the scale on `EaseInOutCubic`, and the only one easing away
/// from a resting state as well as into one.
///
/// **No consumer exists in this crate yet** — see [`FAST`].
pub const SLOW: DurationToken = DurationToken {
    name: "time.slow",
    ms: 240,
    easing: Some(Easing::EaseInOutCubic),
    alias: None,
};

/// `time.debounce.live` — `200ms`, no easing.
///
/// §2.6 binds it to the trailing-edge input debounce that every
/// keystroke restarts, before an async `AutoSuggestBox` or
/// `CommandPalette` query is dispatched. The section names the existing
/// constant outright — "`DEBOUNCE_MS = 200`" — so this rung and
/// [`auto_suggest_model::DEBOUNCE_MS`](crate::widget::auto_suggest_model::DEBOUNCE_MS)
/// are the same number by requirement, not by coincidence, and the
/// sidecar pins the equality so neither can drift.
///
/// Not a transition: it has no curve, and `reduced_motion` must not
/// shorten it. A zero debounce window is one provider call per keystroke.
pub const DEBOUNCE_LIVE: DurationToken = DurationToken {
    name: "time.debounce.live",
    ms: 200,
    easing: None,
    alias: None,
};

/// `time.delay.submenu` — `150ms`, no easing.
///
/// §2.6 binds it to the diagonal pointer safe-corridor grace timer that
/// must elapse before an open submenu branch is switched
/// (`UIF-PRIN-008`, `UIF-FMENU-002`), and gives it the alias
/// `time.menu.aim_grace` — a name that says what the wait is *for*:
/// letting the user aim diagonally at a submenu without the branch
/// collapsing under the crossing pointer.
///
/// Consumed. [`grace_ms`](crate::surface::menu::aim::grace_ms) is the
/// only place in the crate that reads this rung, and
/// [`MenuCascade::hover_submenu_row`](crate::surface::menu::cascade::MenuCascade::hover_submenu_row)
/// drives the window it opens. The corridor never reads a clock: the
/// caller supplies the instant as a `TimeMs` value, which is what
/// makes the whole grace period testable without sleeping and what keeps
/// the decisions reproducible.
///
/// Reduced motion deliberately does **not** shorten this wait. The rung
/// is an input tolerance rather than a transition, and collapsing it to
/// `0ms` through [`resolve_ms`] would make menus harder to use for
/// exactly the users that setting exists to help.
pub const DELAY_SUBMENU: DurationToken = DurationToken {
    name: "time.delay.submenu",
    ms: 150,
    easing: None,
    alias: Some("time.menu.aim_grace"),
};

/// `time.delay.tooltip_open` — `400ms`, no easing.
///
/// §2.6 binds it to the pointer hover delay before a `Tooltip` opens,
/// and gives it the alias `time.tooltip.hover_delay`. The same row
/// carries the keyboard exception: `0ms` on keyboard focus arrival,
/// which is [`KEYBOARD_FOCUS_DELAY`](crate::surface::tooltip::KEYBOARD_FOCUS_DELAY)
/// reading through [`INSTANT`].
///
/// This is the one rung that already had a consumer at the right value:
/// [`HOVER_DELAY`](crate::surface::tooltip::HOVER_DELAY) was `400` and
/// now reads through this rung rather than restating it.
pub const DELAY_TOOLTIP_OPEN: DurationToken = DurationToken {
    name: "time.delay.tooltip_open",
    ms: 400,
    easing: None,
    alias: Some("time.tooltip.hover_delay"),
};

/// `time.delay.tooltip_warm` — `100ms`, no easing.
///
/// §2.6 binds it to the warm-handoff window: moving between adjacent
/// icon-only controls (`Navbar`, `NavigationRail`) within 100ms opens
/// the next tooltip immediately, with no wait. The first hover pays the
/// full [`DELAY_TOOLTIP_OPEN`]; a second one inside this window pays
/// nothing, because the user has demonstrably gone looking.
///
/// **No consumer exists in this crate, and neither does the behaviour.**
/// [`HoverController`](crate::surface::tooltip::hover::HoverController)
/// is per-control and holds no cross-control memory, so every arrival
/// pays the full delay today. Declaring the rung is the token half of a
/// requirement whose behavioural half is still open.
pub const DELAY_TOOLTIP_WARM: DurationToken = DurationToken {
    name: "time.delay.tooltip_warm",
    ms: 100,
    easing: None,
    alias: None,
};

/// `time.hold.copied` — `1500ms`, no easing.
///
/// §2.6 binds it to how long a `CodeBlock` `[Copy]` button holds the
/// `✓` (`status.success`) confirmation before returning to `Rest`.
/// Long enough to be read as a deliberate acknowledgement rather than a
/// flicker, short enough that the control is not stuck showing a past
/// action.
///
/// **No consumer exists in this crate, and neither does the widget.**
/// `CodeBlock` is a Phase C composite deferred on 2026-10-23
/// (`UIF-COMP-015`), so nothing here holds a confirmation tick. The
/// rung is declared so the composite reads a token instead of inventing
/// a duration when it arrives.
pub const HOLD_COPIED: DurationToken = DurationToken {
    name: "time.hold.copied",
    ms: 1_500,
    easing: None,
    alias: None,
};

/// `time.toast.auto_dismiss` — `5000ms`, no easing.
///
/// §2.6 binds it to the default auto-dismiss countdown for `Info` and
/// `Success` toasts, with the two conditions the section attaches: the
/// countdown is **paused and banked** while the toast is hovered or
/// focused, and is forced to `None` for `Warning` and `Danger`, which
/// must stay until acknowledged.
///
/// The banking half is already real —
/// [`ToastTimer`](crate::composite::toast::ToastTimer) banks its
/// remaining duration against an injected instant rather than a fixed
/// deadline — and the default itself lives as
/// [`DEFAULT_AUTO_DISMISS_MS`](crate::composite::notification::DEFAULT_AUTO_DISMISS_MS),
/// which now reads through this rung.
pub const TOAST_AUTO_DISMISS: DurationToken = DurationToken {
    name: "time.toast.auto_dismiss",
    ms: 5_000,
    easing: None,
    alias: None,
};

#[cfg(test)]
#[path = "time_tests.rs"]
mod time_tests;
