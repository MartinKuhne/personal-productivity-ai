//! Whether the user has committed to a field yet (`UIF-PRIN-009`).
//!
//! §1.1's 'Punish Late' clause turns on exactly one question: has this
//! field been *committed to*? Not "has it been changed", not "is it
//! dirty" — whether the user has finished with it enough that telling them
//! they were wrong is now fair rather than premature.
//!
//! That question was previously unaskable. A host could tell you a field
//! was invalid; it could not tell you whether the user had even left the
//! field yet, so the framework had no way to hold the red border back and
//! no way to tell a withheld error from an absent one.
//!
//! # Why this is a value and not a `bool`
//!
//! It is a `bool`'s worth of information, but naming the two states is the
//! point. `pristine: false` reads as "not pristine", which invites a
//! third state nobody modelled — *unknown*, *reset*, *host-seeded*.
//! [`EditStance::Pristine`] and [`EditStance::Touched`] are the only two
//! states the pillar describes, and a match over them is exhaustive in a
//! way an `if` over a flag is not.
//!
//! # What does *not* advance the stance
//!
//! A value change. The transition is tied to the *commit* boundary —
//! `on_commit`, `blur`, submit — because that is what the spec names. A
//! host that sets `value` programmatically (filling a rename field with
//! the current filename, a template populating a form) has told the
//! framework nothing about the user's history, and must not accidentally
//! declare that the user has finished. This is why the models expose
//! `note_commit` / `note_blur` / `note_submit` rather than deriving the
//! stance from "did the value change": the only honest source for a
//! history fact is the event that makes it history.
//!
//! Unit tests live in the sibling `stance_tests.rs` sidecar.

use std::fmt;

/// Where a validation-bearing field stands in its own commit history.
///
/// The distinction is one-way in normal use: a field becomes
/// [`Touched`](Self::Touched) the first time the user commits to it or
/// leaves it, and stays touched until something explicitly
/// [`reset`](Self::reset)s it. Nothing the user types can put it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditStance {
    /// The user has not committed to this field yet.
    ///
    /// An `Invalid` fact may be *known* about the field — the host may
    /// have validated it and found it wanting — but §1.1 forbids painting
    /// it. See [`punish`](super::punish::punish) for the decision that
    /// follows from this.
    Pristine,
    /// The user has committed to this field, or left it, at least once.
    ///
    /// From here the field is fair game: an invalid value paints
    /// immediately, and clears immediately when it becomes valid.
    Touched,
}

impl Default for EditStance {
    /// A field starts pristine.
    ///
    /// Not `Touched`: defaulting to touched would punish every freshly
    /// opened dialog before the user has typed a character, which is the
    /// exact behaviour this pillar prohibits.
    fn default() -> Self {
        Self::Pristine
    }
}

impl EditStance {
    /// Every stance, in lifecycle order.
    pub const ALL: [Self; 2] = [Self::Pristine, Self::Touched];

    /// Whether the user has not yet committed to the field.
    #[must_use]
    pub const fn is_pristine(self) -> bool {
        matches!(self, Self::Pristine)
    }

    /// Whether the user has committed to, or left, the field.
    #[must_use]
    pub const fn is_touched(self) -> bool {
        matches!(self, Self::Touched)
    }

    /// The stance a commit-shaped event produces.
    ///
    /// `on_commit`, `blur` and submit all land here, so the three
    /// transitions the spec lists share one implementation rather than
    /// three places that each have to remember to write `Touched`.
    #[must_use]
    pub const fn after_commit(self) -> Self {
        Self::Touched
    }

    /// The stance a form reset returns the field to.
    ///
    /// Reset is the only legitimate way back to
    /// [`Pristine`](Self::Pristine). It is a *new* form, not the old one
    /// with the memory wiped by something that looked like user input.
    #[must_use]
    pub const fn reset(self) -> Self {
        Self::Pristine
    }
}

impl fmt::Display for EditStance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pristine => f.write_str("Pristine"),
            Self::Touched => f.write_str("Touched"),
        }
    }
}

#[cfg(test)]
#[path = "stance_tests.rs"]
mod stance_tests;
