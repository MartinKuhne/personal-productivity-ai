//! The single authority on when an invalid field is allowed to *look*
//! invalid (`UIF-PRIN-009`).
//!
//! One pillar, three behaviours, one function. A renderer that wants to
//! know whether to draw the red border calls [`punish`] instead of
//! re-deriving the rule from `is_invalid()` and a guess about the field's
//! history — and a guess about history is exactly what the ledger found
//! missing.
//!
//! # The truth table
//!
//! | `stance`   | `invalid` | `disallowed` | `painted` | `invalid()` | [`PunishReason`]     |
//! |------------|-----------|--------------|-----------|-------------|----------------------|
//! | `Pristine` | `false`   | `false`      | no        | `false`     | `Valid`              |
//! | `Pristine` | `false`   | `true`       | **yes**   | `true`      | `DisallowedImmediate` |
//! | `Pristine` | `true`    | `false`      | **no**    | `true`      | `WithheldPristine`   |
//! | `Pristine` | `true`    | `true`      | **yes**   | `true`      | `DisallowedImmediate` |
//! | `Touched`  | `false`   | `false`      | no        | `false`     | `Valid`              |
//! | `Touched`  | `false`   | `true`       | **yes**   | `true`      | `DisallowedImmediate` |
//! | `Touched`  | `true`    | `false`      | yes       | `true`      | `TouchedInvalid`     |
//! | `Touched`  | `true`    | `true`       | yes       | `true`      | `DisallowedImmediate` |
//!
//! Three things fall out of that table and each is a clause of the pillar:
//!
//! * **Punish Late** — row 3. The field *is* invalid and the framework
//!   knows it, and nothing is painted. Withholding the visual is not
//!   forgetting the fact: `invalid()` still returns `true`, so a submit
//!   is still blocked while the border stays off.
//! * **The carve-out** — rows 2 and 4. A disallowed character paints
//!   regardless of stance, and outranks everything else in the table.
//! * **Reward Early** — `painted` is a pure function of this frame's
//!   inputs, so the frame the value becomes valid the paint stops. There
//!   is no latch, no decay and no hysteresis to clear, which is why the
//!   recovery is exact to the frame rather than eventual.
//!
//! # Why `invalid` is an output and not just an input
//!
//! A disallowed character *is* an invalidity, whether or not the host has
//! noticed. So the function returns `invalid = invalid || disallowed`
//! rather than echoing the input: a pristine field containing a `/` blocks
//! submit from the instant the slash lands, even if the host's own
//! validator has not run yet. The visual and the gate cannot come apart,
//! because both read the same returned value.
//!
//! Unit tests live in the sibling `punish_tests.rs` sidecar.

use crate::validation::stance::EditStance;

use std::fmt;

/// Why a punishment decision came out the way it did.
///
/// Carried on every [`Punishment`] rather than being recoverable from the
/// booleans, because two different rows of the truth table can share a
/// `painted` value. `WithheldPristine` and `Valid` both paint nothing, and
/// they are completely different situations — one is a field waiting for
/// its first commit, the other is a field with nothing wrong with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PunishReason {
    /// Nothing is wrong: the value is valid and nothing is refused.
    Valid,
    /// Invalid, but the user has not committed to the field yet, so §1.1
    /// withholds the visual.
    WithheldPristine,
    /// A character the field can never accept is present, so the visual
    /// fires now regardless of stance.
    DisallowedImmediate,
    /// The user committed to the field and the value is invalid.
    TouchedInvalid,
}

impl PunishReason {
    /// Every reason, in the order the decision tree reaches them.
    pub const ALL: [Self; 4] = [
        Self::Valid,
        Self::WithheldPristine,
        Self::DisallowedImmediate,
        Self::TouchedInvalid,
    ];

    /// Whether this reason paints the invalid visual.
    #[must_use]
    pub const fn paints(self) -> bool {
        matches!(self, Self::DisallowedImmediate | Self::TouchedInvalid)
    }
}

impl fmt::Display for PunishReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => f.write_str("Valid"),
            Self::WithheldPristine => f.write_str("WithheldPristine"),
            Self::DisallowedImmediate => f.write_str("DisallowedImmediate"),
            Self::TouchedInvalid => f.write_str("TouchedInvalid"),
        }
    }
}

/// What this frame shows, and what the application must still know.
///
/// Both halves are always present. There is no construction of this type
/// in which the visual and the fact disagree, because the only way to
/// make one is through [`punish`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Punishment {
    invalid: bool,
    painted: bool,
    reason: PunishReason,
}

impl Punishment {
    /// The underlying fact: the value fails, and a submit must be blocked.
    ///
    /// `true` even when nothing is painted. A pristine field holding a
    /// duplicate name is invalid; the pillar says do not *show* it, not
    /// pretend it is fine and let `Ok` through.
    #[must_use]
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Whether the invalid visual — red border, error caption — is drawn.
    #[must_use]
    pub const fn painted(&self) -> bool {
        self.painted
    }

    /// Which row of the truth table produced this.
    #[must_use]
    pub const fn reason(&self) -> PunishReason {
        self.reason
    }

    /// Whether an `Ok` / submit action is blocked.
    ///
    /// Spelled out separately from [`invalid`](Self::invalid) because the
    /// question a footer asks is "may I enable the button", and a name
    /// that answers the question is harder to mis-wire than a bare
    /// boolean read backwards.
    #[must_use]
    pub const fn blocks_submit(&self) -> bool {
        self.invalid
    }

    /// Whether the error caption band carries an error rather than helper
    /// text.
    #[must_use]
    pub const fn shows_error_caption(&self) -> bool {
        self.painted
    }

    /// Whether the control's perimeter carries the invalid border.
    #[must_use]
    pub const fn shows_error_border(&self) -> bool {
        self.painted
    }

    /// Whether an invalid state is being deliberately held back.
    ///
    /// The one case where `invalid() && !painted()`. Worth its own name
    /// because it is the case a naive renderer gets wrong in both
    /// directions: painting it violates the pillar, and treating it as
    /// valid breaks the submit gate.
    #[must_use]
    pub const fn withheld(&self) -> bool {
        matches!(self.reason, PunishReason::WithheldPristine)
    }
}

/// Decide what a validation-bearing control shows this frame.
///
/// The whole of 'Reward Early, Punish Late' in one pure function: no
/// clock, no geometry, no memory of the previous call. The same three
/// inputs always give the same answer, which is what makes the eight-row
/// table in the sidecar an exhaustive proof rather than a sample.
///
/// * `stance` — whether the user has committed to the field yet.
/// * `invalid` — the host's own verdict, as carried by the model.
/// * `disallowed` — whether the value contains a character the field's
///   [`DisallowedClass`](super::disallowed::DisallowedClass) refuses.
#[must_use]
pub const fn punish(stance: EditStance, invalid: bool, disallowed: bool) -> Punishment {
    // The carve-out is tested first because it outranks the pristine
    // shield: §1.1 exempts it from 'Punish Late' explicitly, so no
    // stance can hold it back.
    if disallowed {
        return Punishment {
            invalid: true,
            painted: true,
            reason: PunishReason::DisallowedImmediate,
        };
    }

    if !invalid {
        return Punishment {
            invalid: false,
            painted: false,
            reason: PunishReason::Valid,
        };
    }

    if stance.is_touched() {
        Punishment {
            invalid: true,
            painted: true,
            reason: PunishReason::TouchedInvalid,
        }
    } else {
        Punishment {
            invalid: true,
            painted: false,
            reason: PunishReason::WithheldPristine,
        }
    }
}

#[cfg(test)]
#[path = "punish_tests.rs"]
mod punish_tests;
