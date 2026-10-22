//! The interaction state machine every interactive atom resolves through
//! (`UIF-ATOM-001`, `UIF-ATOM-002`, `UIF-VER-003`).
//!
//! Before an atom paints, it has to answer one question: given what the
//! pointer, the keyboard and the validator are saying right now, what state am
//! I in? [`resolve`] answers it as a pure function of [`InteractionInput`],
//! so the judgement is made once, in one place, and is checkable without a
//! window.
//!
//! This module resolves **state** only. It reads no colour, no stroke and no
//! size, and it owns no style — turning a [`VisualState`] into paint is the
//! atoms' job from `INC-07` onward. That separation is what keeps the state
//! rules independent of the palette: when the `G-7` retune moved
//! [`token::color`](crate::token::color) — `surface.active`,
//! `accent.fill`, `status.danger.fill` — nothing here had to change, because
//! this module never named those colours in the first place.
//!
//! # Why the resolved state is two parts, not one six-value enum
//!
//! `UIF-VER-004` names six snapshot states — `Rest`, `Hovered`, `Pressed`,
//! `Focused`, `Disabled`, `Invalid` — and a single mutually-exclusive enum
//! looks like the obvious encoding. It is the wrong one, because the spec
//! does **not** treat focus as mutually exclusive with the pointer states.
//! Two rows settle it:
//!
//! * **§3.1** — every button variant's *Keyboard Focused* cell reads
//!   "Rest + 2px … focus ring". The ring is spelled as an addition to a base
//!   state, not as the base state itself.
//! * **§4.9.2** — the menu row's *Keyboard Focused* row is `surface.hover`
//!   plus a `1px accent.border` inset outline: the **hover background** and
//!   a **focus outline**, at the same time.
//!
//! A flat six-value enum must choose between the hover fill and the focus ring
//! when both are present, and either choice contradicts one of those rows. So
//! the state is split in two:
//!
//! * [`BaseState`] — the mutually-exclusive visual: `Rest`, `Hovered`,
//!   `Pressed`, `Disabled`, `Invalid`.
//! * [`VisualState`] — that base plus `focused`, an orthogonal ring a
//!   renderer draws on top of whatever `base` says.
//!
//! §3.1's cell is then `VisualState { base: Rest, focused: true }` and
//! §4.9.2's is `VisualState { base: Hovered, focused: true }` — the two
//! cells the flat enum could not both express.
//!
//! The ledger (`INC-06` in `doc/planning/iced-pro-tasks.md`) writes the chain
//! as `Disabled > Invalid > Pressed > Focused > Hovered > Rest`, with
//! `Focused` slotted between `Pressed` and `Hovered`. The split removes that
//! slot rather than ordering it: focus is a ring, not a rung, so "does focus
//! beat hover?" is not a question this model has to answer.
//!
//! # Precedence
//!
//! `Disabled` > `Invalid` > `Pressed` > `Hovered` > `Rest`, with `focused`
//! resolved independently of all five.
//!
//! | Winner | Because |
//! |---|---|
//! | `Disabled` | a disabled control shows its disabled skin no matter what the pointer or the validator does |
//! | `Invalid` | validation is a property of the *data*, not of the pointer — the error must stay visible while the control is hovered, pressed or focused, or the act of clicking at it would visually clear it |
//! | `Pressed` | the pointer is committed, which outranks the mere fact of being over the control |
//! | `Hovered` | the pointer is present, which outranks nothing happening at all |
//! | `Rest` | the floor |
//!
//! # Two edge cases, decided rather than left to fall through
//!
//! **Disabled dominance is total.** When `enabled` is `false`, `focused`
//! resolves to `false` as well as `base` resolving to
//! [`BaseState::Disabled`]. A control that cannot take interaction cannot
//! hold keyboard focus either, so it must not draw a ring: a focus ring is an
//! affordance, and advertising one on a dead control advertises something that
//! cannot happen.
//!
//! **`pressed` without `hovered` is real, and resolves to `Pressed`.** It
//! arrives whenever the pointer is dragged into a control and released there,
//! and whenever an event is synthesised by a test harness or an accessibility
//! layer. The press is the committed fact; the hover flag only records whether
//! the pointer happened to be inside the bounds. Falling back to `Hovered` or
//! `Rest` would drop press feedback precisely when the user is pressing.
//!
//! # Snapshot vocabulary
//!
//! [`VisualState::snapshot_label`] names a resolved state using the six names
//! `UIF-VER-004` iterates. It is a many-to-one projection, not a bijection:
//! `Hovered + focused` and `Pressed + focused` are combinations the spec never
//! snapshots, and both still have to be reported under one of the six names.
//! See that method for the naming rule and for why `Hovered + focused` is
//! called `Focused`.
//!
//! Unit tests live in the sibling `state_tests.rs` sidecar.

/// The mutually-exclusive visual half of a control's state.
///
/// Exactly one of these is painted at a time. Keyboard focus is deliberately
/// absent from this enum: it is a ring drawn *over* one of these rather than a
/// competitor with them, which is why it lives on [`VisualState`] instead. See
/// the [module docs](self) for the two spec rows that force the split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BaseState {
    /// The untouched control: resting fill, resting border, no attention.
    Rest,
    /// The pointer is over the control and nothing outranks that.
    Hovered,
    /// The pointer is committed — button down, or a synthesised press.
    Pressed,
    /// The control cannot be interacted with.
    Disabled,
    /// The control's value failed validation.
    Invalid,
}

/// The raw observations a widget makes about itself, before any judgement.
///
/// Every field is a plain boolean because every field comes from somewhere
/// different — the widget's own `enabled` flag, iced's cursor events, the
/// keyboard-focus router, a validator — and this type deliberately records them
/// without reconciling them. [`resolve`] does the reconciling, which keeps the
/// observations honest and the precedence rules in one place.
///
/// The derived [`Default`] is *all false*, which resolves to
/// [`BaseState::Disabled`]. That is the fail-safe reading: a control that has
/// not been told it is enabled shows the disabled skin rather than pretending
/// to be live.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InteractionInput {
    /// Whether the control accepts interaction at all. `false` dominates
    /// every other field, including [`focused`](Self::focused).
    pub enabled: bool,
    /// The pointer is inside the control's hit bounds.
    pub hovered: bool,
    /// A button is held down on the control. May arrive without
    /// [`hovered`](Self::hovered) when the pointer was dragged in.
    pub pressed: bool,
    /// The control holds keyboard focus. Orthogonal to the pointer signals:
    /// it survives alongside `Hovered` and `Pressed`.
    pub focused: bool,
    /// The control's value failed validation.
    pub invalid: bool,
}

/// The resolved state a renderer consumes: a [`BaseState`] plus an orthogonal
/// focus ring.
///
/// `focused` is a separate field rather than a `BaseState` variant so that a
/// renderer can draw the hover fill *and* the focus ring together, which is
/// exactly what §4.9.2's *Keyboard Focused* menu row specifies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VisualState {
    /// The mutually-exclusive visual to paint.
    pub base: BaseState,
    /// Whether the keyboard focus ring is drawn on top of [`Self::base`].
    ///
    /// Orthogonal by construction: `Rest` + `true` is §3.1's *Keyboard
    /// Focused* button, `Hovered` + `true` is §4.9.2's *Keyboard Focused*
    /// menu row. Always `false` when `base` is [`BaseState::Disabled`] —
    /// [`resolve`] zeroes it, because a dead control must not advertise a
    /// focus affordance.
    pub focused: bool,
}

/// `Rest` — the spec's snapshot name for an untouched control
/// (`UIF-VER-004`).
pub const SNAPSHOT_REST: &str = "Rest";

/// `Hovered` — the spec's snapshot name for the pointer-over state
/// (`UIF-VER-004`).
pub const SNAPSHOT_HOVERED: &str = "Hovered";

/// `Pressed` — the spec's snapshot name for the committed-pointer state
/// (`UIF-VER-004`).
pub const SNAPSHOT_PRESSED: &str = "Pressed";

/// `Focused` — the spec's snapshot name for the keyboard-focus state
/// (`UIF-VER-004`).
pub const SNAPSHOT_FOCUSED: &str = "Focused";

/// `Disabled` — the spec's snapshot name for the inert state
/// (`UIF-VER-004`).
pub const SNAPSHOT_DISABLED: &str = "Disabled";

/// `Invalid` — the spec's snapshot name for the validation-error state
/// (`UIF-VER-004`).
pub const SNAPSHOT_INVALID: &str = "Invalid";

/// The six snapshot names `UIF-VER-004` iterates `(Variant × State)` over,
/// in the order the spec lists them.
///
/// Constants rather than inline literals because [`VisualState::snapshot_label`]
/// and the verification gate both name them: a snapshot name allowed to drift
/// between the producer and the checker is a gate that quietly stops gating.
pub const SNAPSHOT_STATE_NAMES: [&str; 6] = [
    SNAPSHOT_REST,
    SNAPSHOT_HOVERED,
    SNAPSHOT_PRESSED,
    SNAPSHOT_FOCUSED,
    SNAPSHOT_DISABLED,
    SNAPSHOT_INVALID,
];

impl VisualState {
    /// Whether the keyboard focus ring is drawn.
    ///
    /// Equal to [`Self::focused`]: [`resolve`] already zeroes that for a
    /// disabled control, so a renderer never has to re-check `base` before
    /// asking. Renderers ask this instead of reading the field so the rule has
    /// one named owner — if a future rule ever suppresses the ring for some
    /// other reason, [`resolve`] and this method change and no call site has
    /// to be hunted down.
    #[must_use]
    pub const fn ring_visible(&self) -> bool {
        self.focused
    }

    /// Name this state using the six snapshot names `UIF-VER-004` iterates.
    ///
    /// The rule, in priority order — the name tracks the most specific thing
    /// the spec actually takes a snapshot of:
    ///
    /// 1. `Disabled` and `Invalid` own their names absolutely. Nothing a
    ///    pointer or a ring does on top of them changes what the state *is*.
    /// 2. `Pressed` owns its name whether or not the ring is drawn. The
    ///    ledger's chain ranks `Pressed` above `Focused`, and a committed
    ///    press repaints the whole control.
    /// 3. A visible ring owns the name `Focused`, over `Rest` or `Hovered`.
    ///    This is the spec's own usage: §3.1 calls `Rest` + ring *Keyboard
    ///    Focused* and §4.9.2 calls `surface.hover` + ring *Keyboard
    ///    Focused* — one name covering both, because the name is about the
    ///    ring.
    /// 4. Otherwise the base names itself.
    ///
    /// **`Hovered + focused` is outside the spec's snapshot matrix.** No row
    /// of §3.1 or §4.9.2 is titled "Hovered *and* focused", so
    /// `UIF-VER-004` never snapshots the pair under a name of its own. It is
    /// reported as `Focused` rather than `Hovered` because §4.9.2's *Keyboard
    /// Focused* row already *is* the hover fill with a ring on it — naming
    /// the pair `Hovered` would rename a row the spec has already named, and
    /// would make the gate look for a snapshot that no longer describes the
    /// visual. Nothing is lost by the projection: [`base`](Self::base) and
    /// [`focused`](Self::focused) are still on the struct, and a renderer
    /// reads those rather than going through a name.
    ///
    /// `Pressed + focused` is off-matrix in the same way and lands on
    /// `Pressed` by rule 2.
    #[must_use]
    pub const fn snapshot_label(&self) -> &'static str {
        match self.base {
            BaseState::Disabled => SNAPSHOT_DISABLED,
            BaseState::Invalid => SNAPSHOT_INVALID,
            BaseState::Pressed => SNAPSHOT_PRESSED,
            BaseState::Hovered => {
                if self.focused {
                    SNAPSHOT_FOCUSED
                } else {
                    SNAPSHOT_HOVERED
                }
            }
            BaseState::Rest => {
                if self.focused {
                    SNAPSHOT_FOCUSED
                } else {
                    SNAPSHOT_REST
                }
            }
        }
    }
}

/// Resolve the raw observations into the state a widget paints.
///
/// A pure function: no hidden state, no memory of the previous call, no
/// implicit transition. The same [`InteractionInput`] always yields the same
/// [`VisualState`], which is what makes the whole machine verifiable from the
/// 32-row table in the sidecar.
///
/// Precedence for [`BaseState`] is `Disabled` > `Invalid` > `Pressed` >
/// `Hovered` > `Rest`; [`focused`](VisualState::focused) is resolved
/// independently, except that a disabled control cannot hold focus and so gets
/// no ring. See the [module docs](self) for why each rule is ordered the way
/// it is.
#[must_use]
pub const fn resolve(input: InteractionInput) -> VisualState {
    if !input.enabled {
        // Disabled dominance is total: the skin is the disabled one and the
        // ring is suppressed, whatever the pointer, the keyboard or the
        // validator were doing.
        return VisualState {
            base: BaseState::Disabled,
            focused: false,
        };
    }

    let base = if input.invalid {
        // The error belongs to the data, so it survives the pointer.
        BaseState::Invalid
    } else if input.pressed {
        // Committed beats merely-over — including when `hovered` never
        // arrived, which is a real input, not a missing one.
        BaseState::Pressed
    } else if input.hovered {
        BaseState::Hovered
    } else {
        BaseState::Rest
    };

    VisualState {
        base,
        focused: input.focused,
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
