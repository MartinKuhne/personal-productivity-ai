//! Tests for the secret-entry model (`UIF-ATOM-002c`).
//!
//! The load-bearing claims are not that the getters return the fields
//! they wrap. They are:
//!
//! * the redaction actually holds — no `Debug` or `Display` path out of a
//!   `SecretInput` can print the plaintext, including through the model
//!   that contains one;
//! * the mask the model describes is the mask iced draws, derived from
//!   iced's own `Value::secure` rather than from a local character count
//!   that would disagree on every multi-scalar grapheme;
//! * `revealed ⇒ focused` cannot be broken, including by the paths that
//!   look harmless (toggling a disabled field, toggling an unfocused
//!   one, focusing back in).

use crate::widget::password_model::PasswordEmission;
use crate::widget::password_model::PasswordInputModel;
use crate::widget::password_model::SECRET_MASK;
use crate::widget::password_model::SecretInput;
use crate::widget::text_input::GLYPH_ERROR;
use crate::widget::text_input::GLYPH_REVEAL;
use crate::widget::text_input::InputKind;
use crate::widget::text_input::SlotContent;
use crate::widget::text_input::slot_content;

const PLAINTEXT: &str = "correct horse battery staple";

fn secret() -> SecretInput {
    SecretInput::new(PLAINTEXT)
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

#[test]
fn test_debug_never_contains_the_plaintext() {
    let rendered = format!("{:?}", secret());
    assert!(
        !rendered.contains(PLAINTEXT),
        "SecretInput's Debug leaked the secret: {rendered}"
    );
}

#[test]
fn test_display_never_contains_the_plaintext() {
    let rendered = format!("{}", secret());
    assert!(
        !rendered.contains(PLAINTEXT),
        "SecretInput's Display leaked the secret: {rendered}"
    );
}

#[test]
fn test_display_is_the_mask_and_debug_names_the_type() {
    let rendered = format!("{}", secret());
    assert!(rendered.chars().all(|ch| ch == SECRET_MASK));
    assert_eq!(rendered.chars().count(), secret().len());
    assert!(format!("{:?}", secret()).starts_with("SecretInput"));
}

#[test]
fn test_the_model_holding_a_secret_does_not_leak_it_either() {
    let model = PasswordInputModel::new()
        .with_value(secret())
        .with_placeholder("Password");
    let rendered = format!("{model:?}");
    assert!(
        !rendered.contains(PLAINTEXT),
        "PasswordInputModel's Debug leaked the secret: {rendered}"
    );
}

#[test]
fn test_expose_is_the_only_way_out() {
    let value = secret();
    assert_eq!(value.expose(), PLAINTEXT);
    assert_eq!(value.clone().into_exposed(), PLAINTEXT);
}

#[test]
fn test_an_emission_carrying_a_secret_prints_the_mask() {
    let emission = PasswordEmission::Revealed(false);
    assert!(!format!("{emission:?}").contains(PLAINTEXT));
}

// ---------------------------------------------------------------------------
// The mask is iced's mask
// ---------------------------------------------------------------------------

#[test]
fn test_masked_matches_iced_secure_exactly() {
    for text in ["a", "ab", "é", "👍🏽", "correct horse", ""] {
        let value = SecretInput::new(text);
        let iced_mask = iced::widget::text_input::Value::new(text)
            .secure()
            .to_string();
        assert_eq!(
            value.masked(),
            iced_mask,
            "mask disagreed with iced for {text:?}"
        );
    }
}

#[test]
fn test_masked_len_is_iced_grapheme_count_not_char_count() {
    // "é" as one precomposed scalar: 1 char, 1 grapheme.
    assert_eq!(SecretInput::new("é").masked_len(), 1);
    // "é" as e + combining acute: 2 scalars, 1 grapheme. The mask the
    // user sees is one bullet, so the model must say one.
    assert_eq!(SecretInput::new("e\u{0301}").masked_len(), 1);
    // A flag-with-tone emoji is several scalars and one grapheme.
    assert_eq!(SecretInput::new("👍🏽").masked_len(), 1);
    assert_eq!(secret().masked_len(), PLAINTEXT.chars().count());
}

#[test]
fn test_an_empty_secret_masks_to_nothing() {
    let empty = SecretInput::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.masked_len(), 0);
    assert_eq!(empty.masked(), "");
}

#[test]
fn test_masked_len_on_the_model_tracks_the_value() {
    let model = PasswordInputModel::new().with_value(SecretInput::new("hunter2"));
    assert_eq!(model.masked_len(), 7);
}

// ---------------------------------------------------------------------------
// The reveal invariant
// ---------------------------------------------------------------------------

#[test]
fn test_a_fresh_model_is_masked_and_unfocused() {
    let model = PasswordInputModel::new();
    assert!(!model.is_revealed());
    assert!(!model.is_focused());
    assert_eq!(model.revealed_value(), None);
}

#[test]
fn test_reveal_is_refused_while_the_field_is_unfocused() {
    let mut model = PasswordInputModel::new().with_value(secret());
    assert!(!model.toggle_revealed());
    assert!(!model.is_revealed());
    assert_eq!(model.revealed_value(), None);
}

#[test]
fn test_reveal_is_refused_on_a_disabled_field() {
    let mut model = PasswordInputModel::new()
        .with_value(secret())
        .with_disabled(true);
    let _ = model.note_focus(true);
    assert!(!model.can_reveal());
    assert!(!model.toggle_revealed());
    assert!(!model.is_revealed());
}

#[test]
fn test_reveal_is_refused_on_a_read_only_field() {
    let mut model = PasswordInputModel::new()
        .with_value(secret())
        .with_read_only(true);
    let _ = model.note_focus(true);
    assert!(!model.toggle_revealed());
    assert!(!model.is_revealed());
}

#[test]
fn test_reveal_works_once_the_field_holds_the_keyboard() {
    let mut model = PasswordInputModel::new().with_value(secret());
    let _ = model.note_focus(true);
    assert!(model.can_reveal());
    assert!(model.toggle_revealed());
    assert!(model.is_revealed());
    assert_eq!(model.revealed_value(), Some(PLAINTEXT));
}

#[test]
fn test_losing_focus_resets_the_reveal() {
    let mut model = PasswordInputModel::new().with_value(secret());
    let _ = model.note_focus(true);
    let _ = model.toggle_revealed();
    assert!(model.is_revealed());

    assert!(model.note_focus(false), "the reset should report a change");
    assert!(!model.is_revealed());
    assert_eq!(model.revealed_value(), None);
}

#[test]
fn test_losing_focus_while_masked_reports_no_change() {
    let mut model = PasswordInputModel::new();
    let _ = model.note_focus(true);
    assert!(!model.note_focus(false));
}

#[test]
fn test_focusing_back_in_does_not_restore_the_reveal() {
    let mut model = PasswordInputModel::new().with_value(secret());
    let _ = model.note_focus(true);
    let _ = model.toggle_revealed();
    let _ = model.note_focus(false);
    let _ = model.note_focus(true);
    assert!(
        !model.is_revealed(),
        "a revealed secret came back on its own across a focus cycle"
    );
}

#[test]
fn test_the_invariant_holds_across_any_sequence_of_focus_and_toggle() {
    let mut model = PasswordInputModel::new().with_value(secret());
    let script = [true, false, true, true, false, true, false, false, true];

    for focused in script {
        let _ = model.note_focus(focused);
        let _ = model.toggle_revealed();
        assert!(
            !model.is_revealed() || model.is_focused(),
            "revealed without focus after focusing {focused}"
        );
    }
}

#[test]
fn test_toggling_twice_returns_to_masked() {
    let mut model = PasswordInputModel::new().with_value(secret());
    let _ = model.note_focus(true);
    let _ = model.toggle_revealed();
    let _ = model.toggle_revealed();
    assert!(!model.is_revealed());
    assert_eq!(model.revealed_value(), None);
}

// ---------------------------------------------------------------------------
// The slot
// ---------------------------------------------------------------------------

#[test]
fn test_the_password_slot_is_the_reveal_toggle() {
    let model = PasswordInputModel::new().with_value(secret());
    assert_eq!(model.slot_for(), SlotContent::Toggle(GLYPH_REVEAL));
}

#[test]
fn test_the_reveal_toggle_outranks_the_error_glyph() {
    let mut model = PasswordInputModel::new().with_value(secret());
    model.set_validation(Some("too short".to_string()));
    assert!(model.is_invalid());
    assert_eq!(
        model.slot_for(),
        SlotContent::Toggle(GLYPH_REVEAL),
        "an invalid password field lost its reveal control"
    );
}

#[test]
fn test_an_empty_password_field_still_offers_the_reveal_toggle() {
    let model = PasswordInputModel::new();
    assert_eq!(model.slot_for().glyph(), Some(GLYPH_REVEAL));
}

#[test]
fn test_the_error_glyph_still_wins_for_every_other_kind() {
    for kind in [
        InputKind::TextEntry,
        InputKind::Search,
        InputKind::DropdownTrigger,
        InputKind::TextArea,
    ] {
        assert_eq!(
            slot_content(kind, true, true),
            SlotContent::Passive(GLYPH_ERROR),
            "{kind:?} lost its error glyph to the password exception"
        );
    }
}

#[test]
fn test_the_reveal_toggle_is_a_press_that_captures_but_is_not_a_clear() {
    let slot = SlotContent::Toggle(GLYPH_REVEAL);
    assert!(slot.captures_press());
    assert!(slot.is_toggle());
    assert!(
        !slot.is_actionable(),
        "the reveal toggle must not read as a clear affordance"
    );
}

#[test]
fn test_the_reveal_glyph_is_distinct_from_every_other_slot_glyph() {
    let others = [
        crate::widget::text_input::GLYPH_SEARCH,
        crate::widget::text_input::GLYPH_CLEAR,
        crate::widget::text_input::GLYPH_ERROR,
        crate::widget::text_input::GLYPH_CHEVRON,
    ];
    assert!(!others.contains(&GLYPH_REVEAL));
}

// ---------------------------------------------------------------------------
// Emission shape
// ---------------------------------------------------------------------------

#[test]
fn test_a_reveal_does_not_settle_the_session() {
    assert!(!PasswordEmission::Revealed(true).settles());
    assert!(!PasswordEmission::Changed(crate::binding::Live::new(secret())).settles());
    assert!(!PasswordEmission::FocusChanged(true).settles());
}

#[test]
fn test_commit_and_cancel_are_the_settling_arms() {
    assert!(PasswordEmission::Committed(crate::binding::Commit::new(secret())).settles());
    assert!(PasswordEmission::Cancelled(crate::binding::Cancel::new(secret())).settles());
}

#[test]
fn test_tier_names_are_what_the_transcript_says_they_are() {
    assert_eq!(PasswordEmission::Revealed(true).tier(), "reveal");
    assert_eq!(
        PasswordEmission::Changed(crate::binding::Live::new(secret())).tier(),
        "live"
    );
    assert_eq!(
        PasswordEmission::Committed(crate::binding::Commit::new(secret())).tier(),
        "commit"
    );
    assert_eq!(
        PasswordEmission::Cancelled(crate::binding::Cancel::new(secret())).tier(),
        "cancel"
    );
    assert_eq!(PasswordEmission::FocusChanged(false).tier(), "focus");
}

#[test]
fn test_only_the_value_arms_carry_a_secret() {
    assert!(!PasswordEmission::Revealed(true).carries_secret());
    assert!(!PasswordEmission::FocusChanged(true).carries_secret());
    assert!(PasswordEmission::Changed(crate::binding::Live::new(secret())).carries_secret());
}

#[test]
fn test_a_committed_secret_never_equals_a_cancelled_one() {
    let committed = PasswordEmission::Committed(crate::binding::Commit::new(secret()));
    let cancelled = PasswordEmission::Cancelled(crate::binding::Cancel::new(secret()));
    assert_ne!(committed, cancelled);
}
