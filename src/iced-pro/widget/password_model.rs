//! Secret-entry model for `UIF-ATOM-002c`: a password value that refuses to
//! print itself, and the reveal policy that keeps a shown secret inside the one
//! control that is allowed to show it.
//!
//! # Why a type rather than a `String`
//!
//! The single-line atom hands its value to every tier the editing engine
//! produces, and tiers get logged. A `String` in a `Debug` impl is a
//! plaintext secret in a log file, one careless `{:?}` away from a
//! disclosure report. [`SecretInput`] is the same `String` with `Debug`
//! and `Display` rewritten to print the mask instead, so the redaction
//! holds at the type rather than at every call site — and the tiers that
//! carry it ([`Live<SecretInput>`], [`Commit<SecretInput>`]) inherit it
//! for free, because [`Live`]'s `Debug` delegates to its payload.
//!
//! Getting the plaintext back is a named operation
//! ([`expose`](SecretInput::expose)), not a field access. That is the
//! whole point: the unsafe thing should look unsafe in the code review.
//!
//! # Why the mask is iced's mask
//!
//! [`masked_len`](SecretInput::masked_len) and
//! [`masked`](SecretInput::masked) route through iced's own
//! [`iced::widget::text_input::Value::secure`] rather than counting
//! characters here. Iced replaces **each grapheme** with `•` (U+2022), so
//! the mask length is a grapheme count, not a `char` count and not a byte
//! count — and a model that counted differently would report a length the
//! editor is not drawing. Deriving both from the same function is what
//! makes "what the model says is hidden" agree with "what the screen
//! hides".
//!
//! # The reveal invariant
//!
//! `revealed ⇒ focused`. A revealed secret in an unfocused control is a
//! secret left on screen after the user has gone somewhere else, so
//! [`note_focus`](PasswordInputModel::note_focus) drops the reveal on the
//! falling edge and [`toggle_revealed`](PasswordInputModel::toggle_revealed)
//! refuses outright when the field cannot hold the keyboard. The spec
//! states the reset ("automatically resets `revealed_value` to masked on
//! focus loss"); making the *rise* impossible too is what stops the
//! invariant from depending on the reset firing.
//!
//! Unit tests live in the sibling `password_model_tests.rs` sidecar.

use std::fmt;

use crate::binding::Cancel;
use crate::binding::Commit;
use crate::binding::Live;
use crate::widget::text_input::SlotContent;
use crate::widget::text_input::slot_content;

/// The bullet iced masks a grapheme with (`U+2022`).
///
/// Named because [`SecretInput`]'s redaction and the mask it mirrors must
/// be the same character, and two literals spelling the same code point is
/// how they drift apart.
pub const SECRET_MASK: char = '\u{2022}';

/// A password that will not print itself.
///
/// `Debug` and `Display` both render the mask, never the contents, so a
/// `SecretInput` reaching a logger, a `format!`, an assertion message or
/// a panic payload cannot leak. Reading the plaintext is explicit:
/// [`expose`](Self::expose) for a borrow,
/// [`into_exposed`](Self::into_exposed) to take it.
///
/// Equality compares the underlying text. That is deliberate — a password
/// field has to be able to ask "did this change?" — but it is worth
/// knowing that the answer is computed on the secret, so a
/// timing-sensitive comparison is the caller's problem, not this type's.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SecretInput(String);

impl SecretInput {
    /// Wrap a plaintext secret.
    #[must_use]
    pub fn new(plaintext: impl Into<String>) -> Self {
        Self(plaintext.into())
    }

    /// An empty secret — the state of a password field nobody has typed in.
    #[must_use]
    pub fn empty() -> Self {
        Self(String::new())
    }

    /// Read the plaintext.
    ///
    /// Named `expose`, not `get` or `as_str`, because it is the one way
    /// out of the redaction and should read that way at every call site.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Take the plaintext out of the wrapper.
    #[must_use]
    pub fn into_exposed(self) -> String {
        self.0
    }

    /// How many graphemes the secret holds, counted the way iced counts
    /// them when it masks them.
    #[must_use]
    pub fn len(&self) -> usize {
        iced::widget::text_input::Value::new(&self.0).len()
    }

    /// Whether the secret holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The masked string exactly as the editor draws it.
    ///
    /// One `•` per grapheme, so `"é"` (one grapheme, two scalars) masks
    /// to a single bullet — which is what the user sees, and therefore
    /// what any code that describes the masked form should get.
    #[must_use]
    pub fn masked(&self) -> String {
        iced::widget::text_input::Value::new(&self.0)
            .secure()
            .to_string()
    }

    /// §3.2's `masked_len`: the width of the mask, in graphemes.
    #[must_use]
    pub fn masked_len(&self) -> usize {
        self.len()
    }
}

impl Default for SecretInput {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretInput")
            .field("masked", &self.masked())
            .field("len", &self.masked_len())
            .finish()
    }
}

impl fmt::Display for SecretInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.masked())
    }
}

/// The state of a password field (`UIF-ATOM-002c`).
///
/// §3.2 prints `PasswordInputModel { masked_len, revealed_value,
/// validation, disabled }`. That shape cannot be a *controlled* model:
/// both of its value-shaped fields are derived, and a model that stores
/// only derivatives has nothing left to edit — the host would have to keep
/// the secret somewhere else anyway. So this model owns the secret
/// ([`value`](Self::value)) and derives the two spec fields from it,
/// which is the same relationship the spec is describing, pointed the
/// right way round.
///
/// Fields are private rather than `pub` as the other models in this
/// directory make theirs, for one reason: a public field is a field that
/// can be written from anywhere, and `revealed` is a security boundary.
/// Every change to it goes through a method that enforces the
/// `revealed ⇒ focused` rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordInputModel {
    /// The secret. Printed as its mask by [`SecretInput`]'s `Debug`.
    value: SecretInput,
    placeholder: String,
    validation: Option<String>,
    revealed: bool,
    focused: bool,
    disabled: bool,
    read_only: bool,
}

impl PasswordInputModel {
    /// A blank, enabled, unfocused, masked field.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: SecretInput::empty(),
            placeholder: String::new(),
            validation: None,
            revealed: false,
            focused: false,
            disabled: false,
            read_only: false,
        }
    }

    /// Set the placeholder shown while the field is empty.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Seed the secret.
    #[must_use]
    pub fn with_value(mut self, value: SecretInput) -> Self {
        self.value = value;
        self
    }

    /// Report the field disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Report the field read-only.
    #[must_use]
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    // -- reads ---------------------------------------------------------

    /// The secret itself.
    #[must_use]
    pub fn value(&self) -> &SecretInput {
        &self.value
    }

    /// §3.2's `masked_len`.
    #[must_use]
    pub fn masked_len(&self) -> usize {
        self.value.masked_len()
    }

    /// §3.2's `revealed_value`: the plaintext while the field is showing
    /// it, `None` while it is masked.
    ///
    /// `None` while masked is what makes the field honest — there is no
    /// revealed value to report, rather than a revealed value that
    /// happens not to be drawn.
    #[must_use]
    pub fn revealed_value(&self) -> Option<&str> {
        if self.revealed {
            Some(self.value.expose())
        } else {
            None
        }
    }

    /// §3.2's `validation`.
    #[must_use]
    pub fn validation(&self) -> Option<&str> {
        self.validation.as_deref()
    }

    /// The hint shown while the field is empty.
    #[must_use]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Whether a validation verdict is outstanding.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.validation.is_some()
    }

    /// Whether the field is showing its secret.
    #[must_use]
    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    /// Whether the field holds the keyboard.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether the host has taken the field away.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Whether the field can be looked at but not changed.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// What the reserved trailing slot carries.
    ///
    /// Delegates to the atom's own rule so a password field's slot is
    /// decided by the same function that decides a search field's, and
    /// the "reveal outranks the error glyph" exception lives in one place
    /// ([`slot_content`]) rather than being restated here.
    #[must_use]
    pub fn slot_for(&self) -> SlotContent {
        slot_content(
            crate::widget::text_input::InputKind::Password,
            !self.value.is_empty(),
            self.is_invalid(),
        )
    }

    // -- the reveal policy ---------------------------------------------

    /// Whether the reveal toggle is able to act.
    ///
    /// A field must hold the keyboard to show its secret: a disabled or
    /// read-only field cannot be focused, and a field the user is not in
    /// has no business displaying a password. This is the invariant
    /// `revealed ⇒ focused` read from the other end.
    #[must_use]
    pub fn can_reveal(&self) -> bool {
        self.focused && !self.disabled && !self.read_only
    }

    /// Flip the reveal state. Returns whether anything changed.
    ///
    /// Refused — returning `false` and changing nothing — when
    /// [`can_reveal`](Self::can_reveal) is false. The toggle is inert
    /// rather than misleading: pressing it on a field that cannot show
    /// its secret does not open a session, does not emit a tier, and does
    /// not put a password on screen behind an unfocused control.
    #[must_use]
    pub fn toggle_revealed(&mut self) -> bool {
        if !self.can_reveal() {
            return false;
        }

        self.revealed = !self.revealed;
        true
    }

    /// Record a focus edge. Returns whether the reveal state changed.
    ///
    /// Losing focus always drops the reveal — §3.2's "automatically
    /// resets `revealed_value` to masked on focus loss". Gaining focus
    /// never reveals anything by itself; showing a secret is something
    /// the user asks for.
    #[must_use]
    pub fn note_focus(&mut self, focused: bool) -> bool {
        self.focused = focused;

        if !focused && self.revealed {
            self.revealed = false;
            return true;
        }

        false
    }

    /// Record an edit.
    pub fn set_value(&mut self, value: SecretInput) {
        self.value = value;
    }

    /// Report or clear a validation verdict.
    pub fn set_validation(&mut self, validation: Option<String>) {
        self.validation = validation;
    }
}

impl Default for PasswordInputModel {
    fn default() -> Self {
        Self::new()
    }
}

/// One tier's worth of output from a password field.
///
/// Mirrors [`TextInputEmission`](super::text_input_controller::TextInputEmission)
/// with two differences that are both spec, not taste:
///
/// * **No `Cleared` arm.** §3.2 gives `002c` `on_change`, `on_commit`
///   and `on_reveal_toggle` and no `on_clear`. A password field has no
///   clear affordance and — see [`PasswordController`](super::password_host::PasswordController) —
///   is never `Escape`-cleared either.
/// * **Every value arm carries a [`SecretInput`], not a `String`.** The
///   tiers are the leak path; wrapping them is what makes "log the
///   emission" safe.
#[derive(Clone)]
pub enum PasswordEmission {
    /// `Live<SecretInput>` — `on_change`, one coalesced tick per delivery.
    Changed(Live<SecretInput>),
    /// `Commit<SecretInput>` — `on_commit`, settled by focus loss.
    Committed(Commit<SecretInput>),
    /// `Cancel<SecretInput>` — `Escape` inside an open session, carrying
    /// the pre-gesture secret to restore.
    Cancelled(Cancel<SecretInput>),
    /// `on_reveal_toggle(bool)` — the reveal state that just took effect.
    Revealed(bool),
    /// `on_focus_change(bool)` — the edge that opens and closes sessions.
    FocusChanged(bool),
}

impl PasswordEmission {
    /// The §3.0.4.2 tier name this arm belongs to.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Changed(_) => "live",
            Self::Committed(_) => "commit",
            Self::Cancelled(_) => "cancel",
            Self::Revealed(_) => "reveal",
            Self::FocusChanged(_) => "focus",
        }
    }

    /// Whether this arm settles the editing session.
    ///
    /// A reveal does **not**: showing a password is not an edit, and a
    /// session settled by a click on the eye would commit half-typed text
    /// the user never meant to finish.
    #[must_use]
    pub const fn settles(&self) -> bool {
        matches!(self, Self::Committed(_) | Self::Cancelled(_))
    }

    /// Whether this arm carries the secret.
    #[must_use]
    pub const fn carries_secret(&self) -> bool {
        matches!(
            self,
            Self::Changed(_) | Self::Committed(_) | Self::Cancelled(_)
        )
    }
}

impl PartialEq for PasswordEmission {
    /// Compares the payload each tier carries, tier-matched first.
    ///
    /// Same reasoning as
    /// [`TextInputEmission`](super::text_input_controller::TextInputEmission)'s
    /// hand-written equality:
    /// the linear wrappers deliberately do not implement `PartialEq`, and
    /// a committed secret and a cancelled secret are different events no
    /// matter how alike their contents.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Changed(a), Self::Changed(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::Revealed(a), Self::Revealed(b)) => a == b,
            (Self::FocusChanged(a), Self::FocusChanged(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for PasswordEmission {
    /// Prints the tier and the **masked** payload.
    ///
    /// This is the mitigation for the leak the single-line atom has by
    /// construction: every keystroke becomes a tier, tiers get logged,
    /// and here the thing that gets logged is `••••` and a length rather
    /// than the password.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed(live) => f.debug_tuple("Changed").field(live.value()).finish(),
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
            Self::Revealed(revealed) => f.debug_tuple("Revealed").field(revealed).finish(),
            Self::FocusChanged(focused) => f.debug_tuple("FocusChanged").field(focused).finish(),
        }
    }
}

#[cfg(test)]
#[path = "password_model_tests.rs"]
mod password_model_tests;
