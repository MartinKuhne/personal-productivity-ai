//! The `PasswordInput` host — the reveal policy and the tier translation
//! (`UIF-ATOM-002c`).
//!
//! # There is no second text field here
//!
//! A password field **is** the `002a` atom. It gets the same
//! [`TextInput`](super::text_input::TextInput) wrapper, the same
//! [`TextInputController`] editing engine
//! sitting in the widget tree, the same snapshot-at-first-edit, the same
//! coalesced `Live` tick, the same `Escape` revert, the same §3.2 cell
//! matrix and the same zero-shift trailing slot. What `002c` adds is three
//! things, and all three sit *beside* the atom rather than inside a copy
//! of it:
//!
//! 1. **Masking** — a `bool` handed to the atom, which passes it to
//!    iced's `Value::secure`.
//! 2. **A reveal toggle** — the atom's existing trailing-slot action,
//!    pointed at a different message ([`on_reveal`] rather than
//!    [`on_clear`]).
//! 3. **Redaction** — the tiers carry [`SecretInput`] instead of
//!    `String`, so the transcript a host logs cannot contain the
//!    password.
//!
//! [`on_reveal`]: crate::widget::text_input::TextInput::on_reveal
//! [`on_clear`]: crate::widget::text_input::TextInput::on_clear
//! [`TextInputController`]: super::text_input_controller::TextInputController
//!
//! # Why the host translates rather than re-implements
//!
//! The atom's engine emits [`TextInputEmission`]. The spec asks for
//! `on_change(SecretInput)` / `on_commit()` / `on_reveal_toggle(bool)`.
//! [`PasswordController::translate`] is the seam: it re-wraps each value
//! tier's `String` in a [`SecretInput`] and passes the focus edge
//! through, so the *engine* stays the single owner of the editing
//! session and the host never keeps a competing copy of "is an edit in
//! progress".
//!
//! Two tiers deliberately have no password counterpart, and both are
//! spec rather than omission:
//!
//! * [`TextInputEmission::Cleared`] never arrives, because the mirror
//!   model is built with `clearable: false` — §3.2 gives `002c` no
//!   `on_clear`, and a password field is never `Escape`-cleared.
//! * There is no `Committed` on `Enter`... except that there is: `Enter`
//!   *does* settle a password field, exactly as it settles a single-line
//!   input. What `002c` lacks is a *clear*, not a commit.
//!
//! # The reveal is host-owned because the spec says so
//!
//! `revealed_value` is a field of `PasswordInputModel`, so the model owns
//! it and enforces `revealed ⇒ focused`. The controller owns the
//! *transcript* — how many times the secret was shown, how many frames
//! it spent exposed — and nothing else, which is what keeps this file
//! from becoming a second place the reveal state lives.
//!
//! Unit tests live in the sibling `password_host_tests.rs` sidecar.

use crate::widget::password_model::PasswordEmission;
use crate::widget::password_model::PasswordInputModel;
use crate::widget::password_model::SecretInput;
use crate::widget::text_input::InputKind;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::TextInputModel;

/// Everything one host input to a password field did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PasswordOutcome {
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<PasswordEmission>,
}

impl PasswordOutcome {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions`.
    #[must_use]
    pub fn emissions(emissions: Vec<PasswordEmission>) -> Self {
        Self { emissions }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
    }
}

/// The host-side memory of a password field: the reveal transcript and
/// the tier translation.
///
/// Deliberately **not** a second editing engine. The session, the
/// snapshot and the coalescer belong to the
/// [`TextInputController`](crate::widget::text_input_controller::TextInputController)
/// the atom runs in the widget tree; a second one here would be a second
/// answer to "is the user mid-edit", and the two would disagree.
#[derive(Clone, Debug, Default)]
pub struct PasswordController {
    /// How many times the secret has been shown or hidden, successfully.
    reveals: usize,
    /// How many times the toggle was pressed and refused.
    refusals: usize,
    /// Frames observed with the secret masked.
    masked_frames: usize,
    /// Frames observed with the secret exposed.
    exposed_frames: usize,
}

impl PasswordController {
    /// A controller that has seen nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what this frame shows, and return whether the secret is
    /// currently exposed.
    ///
    /// Call once per frame with the model being rendered. The only thing
    /// the controller learns from the frame is how long a secret was left
    /// visible, which is the one part of a password control's behaviour
    /// a host might later want reported: "exposed for 400 frames" is a
    /// finding; "was exposed at some point" is not.
    pub fn observe(&mut self, model: &PasswordInputModel) -> bool {
        if model.is_revealed() {
            self.exposed_frames += 1;
        } else {
            self.masked_frames += 1;
        }
        model.is_revealed()
    }

    /// Ask for the reveal toggle.
    ///
    /// Delegates the decision entirely to the model, which owns the
    /// `revealed ⇒ focused` rule, and turns the answer into
    /// `on_reveal_toggle(bool)`. A refused toggle emits nothing at all:
    /// the spec's callback reports a *change*, and reporting
    /// `Revealed(false)` after a press that changed nothing would tell
    /// the host it had hidden a secret it was never showing.
    pub fn toggle_reveal(&mut self, model: &mut PasswordInputModel) -> PasswordOutcome {
        if !model.toggle_revealed() {
            self.refusals += 1;
            return PasswordOutcome::noop();
        }

        self.reveals += 1;
        PasswordOutcome::emissions(vec![PasswordEmission::Revealed(model.is_revealed())])
    }

    /// Translate one tier the atom's engine produced.
    ///
    /// `Changed`, `Committed` and `FocusCommitted` have their `String`
    /// re-wrapped as a [`SecretInput`] — the redaction is applied at the
    /// boundary where the plaintext leaves the editor, which is the last
    /// point at which it can be applied before a host sees it. `Cancelled`
    /// carries the restore target, which is a secret too, and is wrapped
    /// the same way. `Cleared` cannot occur on a password mirror and is
    /// mapped to `None` rather than to a fabricated arm, so a host that
    /// ever does see one knows the mirror was built wrong instead of
    /// quietly receiving an empty password.
    #[must_use]
    pub fn translate(emission: TextInputEmission) -> Option<PasswordEmission> {
        match emission {
            TextInputEmission::Changed(live) => Some(PasswordEmission::Changed(
                crate::binding::Live::new(SecretInput::new(live.into_value())),
            )),
            TextInputEmission::Committed(commit) | TextInputEmission::FocusCommitted(commit) => {
                Some(PasswordEmission::Committed(crate::binding::Commit::new(
                    SecretInput::new(commit.into_value()),
                )))
            }
            TextInputEmission::Cancelled(cancel) => Some(PasswordEmission::Cancelled(
                crate::binding::Cancel::new(SecretInput::new(cancel.into_restored())),
            )),
            TextInputEmission::FocusChanged(focused) => {
                Some(PasswordEmission::FocusChanged(focused))
            }
            TextInputEmission::Cleared(_) => None,
        }
    }

    /// How many successful reveal toggles have happened.
    #[must_use]
    pub const fn reveals(&self) -> usize {
        self.reveals
    }

    /// How many presses were refused because the field could not reveal.
    #[must_use]
    pub const fn refusals(&self) -> usize {
        self.refusals
    }

    /// Frames observed masked.
    #[must_use]
    pub const fn masked_frames(&self) -> usize {
        self.masked_frames
    }

    /// Frames observed exposed.
    #[must_use]
    pub const fn exposed_frames(&self) -> usize {
        self.exposed_frames
    }

    /// Forget the transcript.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// The [`TextInputModel`] a password field renders through.
///
/// This function is the reuse seam: it says, in one place, that a
/// password field is the `002a` atom with three fixed differences —
///
/// * `clearable: false`, because §3.2 gives `002c` no `on_clear` and an
///   `Escape` that emptied a half-typed password would be the worst
///   possible default;
/// * the value carried across as plaintext, because the *editor* has to
///   hold the real string in order to mask it — masking is a rendering
///   property, not a storage one, and a model that stored bullets could
///   never be revealed;
/// * `InputKind::Password`, which is what gives the field its reveal
///   slot instead of a clear button.
///
/// The returned model is a projection, not a second source of truth: it
/// is rebuilt from `model` every frame and never mutated in place.
#[must_use]
pub fn mirror(model: &PasswordInputModel) -> TextInputModel {
    let mut mirrored = TextInputModel::new(model.value().expose())
        .with_placeholder(model.placeholder().to_string())
        .with_clearable(false)
        .with_stance(model.stance());

    if model.is_read_only() {
        mirrored = mirrored.read_only();
    }
    if model.is_disabled() {
        mirrored = mirrored.disabled();
    }
    if let Some(message) = model.validation() {
        mirrored = mirrored.with_validation(message);
    }

    mirrored
}

/// The kind the mirror renders as.
///
/// Named so the call site reads as the decision rather than as a magic
/// variant.
pub const PASSWORD_KIND: InputKind = InputKind::Password;

#[cfg(test)]
#[path = "password_host_tests.rs"]
mod password_host_tests;
