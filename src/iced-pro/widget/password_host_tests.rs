//! Tests for the password host — the reveal transcript and the tier
//! translation (`UIF-ATOM-002c`).
//!
//! The load-bearing claims are that the mirror the password field renders
//! through cannot give the atom a clear affordance, that every tier
//! leaving the translation carries a redacting `SecretInput` rather than
//! the `String` the editor produced, and that a refused reveal says
//! nothing at all.

use crate::widget::password_host::PASSWORD_KIND;
use crate::widget::password_host::PasswordController;
use crate::widget::password_host::mirror;
use crate::widget::password_model::PasswordEmission;
use crate::widget::password_model::PasswordInputModel;
use crate::widget::password_model::SecretInput;
use crate::widget::text_input::InputKind;
use crate::widget::text_input_controller::TextInputEmission;

const PLAINTEXT: &str = "s3cr3t-passphrase";

// ---------------------------------------------------------------------------
// The mirror
// ---------------------------------------------------------------------------

#[test]
fn test_the_mirror_is_a_password_kind_field() {
    assert_eq!(PASSWORD_KIND, InputKind::Password);
}

#[test]
fn test_the_mirror_can_never_be_cleared() {
    let model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let mirrored = mirror(&model);

    assert!(
        !mirrored.clearable,
        "a password field was handed a clear affordance it must not have"
    );
    assert!(
        !mirrored.clear_affordance(InputKind::Password),
        "the atom still found a clear affordance on the password mirror"
    );
}

#[test]
fn test_the_mirror_carries_the_plaintext_because_the_editor_has_to_mask_it() {
    let model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    assert_eq!(mirror(&model).value, PLAINTEXT);
}

#[test]
fn test_the_mirror_carries_placeholder_and_flags() {
    let model = PasswordInputModel::new()
        .with_placeholder("Work password")
        .with_value(SecretInput::new(PLAINTEXT))
        .with_disabled(true)
        .with_read_only(true);
    let mirrored = mirror(&model);

    assert_eq!(mirrored.placeholder, "Work password");
    assert!(mirrored.disabled);
    assert!(mirrored.read_only);
}

#[test]
fn test_the_mirror_carries_the_validation_words() {
    let mut model = PasswordInputModel::new();
    model.set_validation(Some("at least 12 characters".to_string()));
    assert_eq!(
        mirror(&model).validation.as_deref(),
        Some("at least 12 characters")
    );
}

#[test]
fn test_the_mirror_of_an_empty_field_is_an_empty_field() {
    let mirrored = mirror(&PasswordInputModel::new());
    assert_eq!(mirrored.value, "");
    assert!(!mirrored.disabled);
    assert!(!mirrored.read_only);
    assert!(!mirrored.clearable);
}

#[test]
fn test_the_mirror_is_a_projection_not_a_second_owner() {
    let mut model = PasswordInputModel::new().with_value(SecretInput::new("one"));
    let first = mirror(&model);
    model.set_value(SecretInput::new("two"));
    let second = mirror(&model);

    assert_eq!(first.value, "one");
    assert_eq!(second.value, "two");
}

// ---------------------------------------------------------------------------
// The translation
// ---------------------------------------------------------------------------

#[test]
fn test_a_changed_tier_comes_back_wrapped_and_redacted() {
    let emission = PasswordController::translate(TextInputEmission::Changed(
        crate::binding::Live::new(PLAINTEXT.to_string()),
    ))
    .expect("a Changed tier must translate");

    match &emission {
        PasswordEmission::Changed(live) => assert_eq!(live.value().expose(), PLAINTEXT),
        other => panic!("translated a Changed into {other:?}"),
    }

    assert!(
        !format!("{emission:?}").contains(PLAINTEXT),
        "the translated tier leaked the plaintext through Debug"
    );
}

#[test]
fn test_a_committed_tier_comes_back_wrapped_and_redacted() {
    let emission = PasswordController::translate(TextInputEmission::Committed(
        crate::binding::Commit::new(PLAINTEXT.to_string()),
    ))
    .expect("a Committed tier must translate");

    assert!(matches!(emission, PasswordEmission::Committed(_)));
    assert!(!format!("{emission:?}").contains(PLAINTEXT));
}

#[test]
fn test_a_cancelled_tier_carries_the_restored_secret_redacted() {
    let emission = PasswordController::translate(TextInputEmission::Cancelled(
        crate::binding::Cancel::new(PLAINTEXT.to_string()),
    ))
    .expect("a Cancelled tier must translate");

    match &emission {
        PasswordEmission::Cancelled(cancel) => assert_eq!(cancel.restored().expose(), PLAINTEXT),
        other => panic!("translated a Cancelled into {other:?}"),
    }

    assert!(!format!("{emission:?}").contains(PLAINTEXT));
}

#[test]
fn test_the_focus_edge_passes_straight_through() {
    let emission =
        PasswordController::translate(TextInputEmission::FocusChanged(true)).expect("passes");
    assert_eq!(emission, PasswordEmission::FocusChanged(true));
}

#[test]
fn test_a_cleared_tier_translates_to_nothing_rather_than_an_empty_secret() {
    let translated = PasswordController::translate(TextInputEmission::Cleared(
        crate::binding::Commit::new(String::new()),
    ));
    assert!(
        translated.is_none(),
        "a clear produced a password emission; the mirror would look like an empty password"
    );
}

#[test]
fn test_translation_preserves_tier_identity() {
    let live = PasswordController::translate(TextInputEmission::Changed(
        crate::binding::Live::new("x".to_string()),
    ))
    .expect("live");
    let commit = PasswordController::translate(TextInputEmission::Committed(
        crate::binding::Commit::new("x".to_string()),
    ))
    .expect("commit");

    assert_eq!(live.tier(), "live");
    assert_eq!(commit.tier(), "commit");
    assert_ne!(live, commit);
}

// ---------------------------------------------------------------------------
// The reveal transcript
// ---------------------------------------------------------------------------

#[test]
fn test_a_refused_reveal_emits_nothing() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));

    let outcome = controller.toggle_reveal(&mut model);
    assert!(outcome.is_noop());
    assert_eq!(controller.refusals(), 1);
    assert_eq!(controller.reveals(), 0);
}

#[test]
fn test_an_accepted_reveal_emits_the_new_state() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let _ = model.note_focus(true);

    let outcome = controller.toggle_reveal(&mut model);
    assert_eq!(outcome.emissions, vec![PasswordEmission::Revealed(true)]);
    assert_eq!(controller.reveals(), 1);
    assert_eq!(controller.refusals(), 0);
}

#[test]
fn test_toggling_back_emits_revealed_false() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let _ = model.note_focus(true);
    controller.toggle_reveal(&mut model);

    let outcome = controller.toggle_reveal(&mut model);
    assert_eq!(outcome.emissions, vec![PasswordEmission::Revealed(false)]);
    assert_eq!(controller.reveals(), 2);
}

#[test]
fn test_a_reveal_after_focus_loss_is_refused_not_silently_ignored() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let _ = model.note_focus(true);
    controller.toggle_reveal(&mut model);
    let _ = model.note_focus(false);

    let outcome = controller.toggle_reveal(&mut model);
    assert!(outcome.is_noop());
    assert_eq!(controller.refusals(), 1);
    assert!(!model.is_revealed());
}

#[test]
fn test_frame_accounting_separates_masked_from_exposed() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));

    assert!(!controller.observe(&model));
    assert!(!controller.observe(&model));

    let _ = model.note_focus(true);

    controller.toggle_reveal(&mut model);
    assert!(controller.observe(&model));

    assert_eq!(controller.masked_frames(), 2);
    assert_eq!(controller.exposed_frames(), 1);
}

#[test]
fn test_losing_focus_ends_the_exposure_streak_on_the_next_frame() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let _ = model.note_focus(true);
    controller.toggle_reveal(&mut model);
    controller.observe(&model);

    let _ = model.note_focus(false);

    assert!(!controller.observe(&model));
    assert_eq!(controller.exposed_frames(), 1);
    assert_eq!(controller.masked_frames(), 1);
}

#[test]
fn test_reset_clears_the_whole_transcript() {
    let mut controller = PasswordController::new();
    let mut model = PasswordInputModel::new().with_value(SecretInput::new(PLAINTEXT));
    let _ = model.note_focus(true);
    controller.toggle_reveal(&mut model);
    controller.observe(&model);
    controller.toggle_reveal(&mut model);

    controller.reset();
    assert_eq!(controller.reveals(), 0);
    assert_eq!(controller.refusals(), 0);
    assert_eq!(controller.masked_frames(), 0);
    assert_eq!(controller.exposed_frames(), 0);
}
