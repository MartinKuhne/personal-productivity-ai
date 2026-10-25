//! Tests for the pristine/touched stance and its wiring into the three
//! validation-bearing models (`UIF-PRIN-009`).

use crate::widget::password_model::PasswordInputModel;
use crate::widget::text_area_model::TextAreaModel;
use crate::widget::text_input_model::{TextInputModel, TextRange};

use super::EditStance;

// -- the value type -----------------------------------------------------------

#[test]
fn a_field_starts_pristine() {
    assert_eq!(EditStance::default(), EditStance::Pristine);
    assert!(EditStance::default().is_pristine());
    assert!(!EditStance::default().is_touched());
}

#[test]
fn both_stances_are_listed_in_all() {
    assert_eq!(EditStance::ALL, [EditStance::Pristine, EditStance::Touched]);
}

#[test]
fn is_pristine_and_is_touched_are_exact_complements() {
    for stance in EditStance::ALL {
        assert_ne!(
            stance.is_pristine(),
            stance.is_touched(),
            "{stance} is neither or both"
        );
    }
}

#[test]
fn a_commit_lands_on_touched_from_either_side() {
    for stance in EditStance::ALL {
        assert_eq!(stance.after_commit(), EditStance::Touched);
    }
}

#[test]
fn a_reset_lands_on_pristine_from_either_side() {
    for stance in EditStance::ALL {
        assert_eq!(stance.reset(), EditStance::Pristine);
    }
}

#[test]
fn the_stances_render_as_their_own_names() {
    assert_eq!(EditStance::Pristine.to_string(), "Pristine");
    assert_eq!(EditStance::Touched.to_string(), "Touched");
}

// -- TextInputModel -----------------------------------------------------------

#[test]
fn a_new_text_field_is_pristine() {
    assert!(TextInputModel::empty().is_pristine());
    assert!(TextInputModel::new("seeded").is_pristine());
    assert_eq!(TextInputModel::empty().stance(), EditStance::Pristine);
}

#[test]
fn commit_blur_and_submit_each_transition_the_field() {
    for transition in ["commit", "blur", "submit"] {
        let mut model = TextInputModel::new("value");
        match transition {
            "commit" => model.note_commit(),
            "blur" => model.note_blur(),
            "submit" => model.note_submit(),
            _ => unreachable!(),
        }
        assert!(!model.is_pristine(), "{transition} left the field pristine");
        assert_eq!(model.stance(), EditStance::Touched, "{transition} missed");
    }
}

#[test]
fn the_transition_is_idempotent() {
    let mut model = TextInputModel::new("value");
    model.note_blur();
    model.note_commit();
    model.note_submit();
    assert_eq!(model.stance(), EditStance::Touched);
}

#[test]
fn a_programmatic_value_change_does_not_touch_the_field() {
    let mut model = TextInputModel::empty();
    model.value = "filled in by the host".to_string();
    assert!(
        model.is_pristine(),
        "a value write declared the user finished with a field they never focused"
    );
}

#[test]
fn a_host_reporting_invalidity_does_not_touch_the_field() {
    let model = TextInputModel::empty().with_validation("A name is required.");
    assert!(
        model.is_pristine(),
        "the validator arriving must not be mistaken for the user committing"
    );
    assert!(model.is_invalid());
}

#[test]
fn a_selection_request_does_not_touch_the_field() {
    let model = TextInputModel::new("rename me").with_selection(TextRange::new(0, 6));
    assert!(model.is_pristine());
}

#[test]
fn reset_returns_a_touched_field_to_pristine() {
    let mut model = TextInputModel::new("value");
    model.note_commit();
    model.reset_stance();
    assert!(model.is_pristine());
}

#[test]
fn a_model_can_be_constructed_already_touched() {
    let model = TextInputModel::new("value").with_stance(EditStance::Touched);
    assert!(model.is_touched());
}

#[test]
fn history_is_part_of_the_models_identity() {
    let pristine = TextInputModel::new("same");
    let mut touched = TextInputModel::new("same");
    touched.note_blur();
    assert_ne!(
        pristine, touched,
        "two fields with different histories compared equal, so a caller could \
         not tell a withheld error from an absent one"
    );
}

// -- TextAreaModel -----------------------------------------------------------

#[test]
fn a_new_well_is_pristine_and_clean() {
    let model = TextAreaModel::new();
    assert!(model.is_pristine());
    assert!(!model.is_dirty());
}

#[test]
fn a_wells_dirty_flag_and_its_stance_are_different_facts() {
    let mut model = TextAreaModel::new();
    model.set_text("typed but not committed");
    assert!(model.is_dirty(), "set_text should mark the buffer dirty");
    assert!(
        model.is_pristine(),
        "editing is not committing: a dirty-but-uncommitted well must still \
         have its error withheld"
    );

    model.note_commit();
    assert!(model.is_touched());
    assert!(model.is_dirty());
}

#[test]
fn the_well_transitions_on_every_commit_shaped_event() {
    for transition in ["commit", "blur", "submit"] {
        let mut model = TextAreaModel::new().with_text("v");
        match transition {
            "commit" => model.note_commit(),
            "blur" => model.note_blur(),
            "submit" => model.note_submit(),
            _ => unreachable!(),
        }
        assert_eq!(model.stance(), EditStance::Touched, "{transition} missed");
    }
}

#[test]
fn the_well_resets_to_pristine() {
    let mut model = TextAreaModel::new().with_text("v");
    model.note_blur();
    model.reset_stance();
    assert!(model.is_pristine());
}

#[test]
fn a_well_can_be_constructed_already_touched() {
    let model = TextAreaModel::new()
        .with_text("v")
        .with_stance(EditStance::Touched);
    assert!(model.is_touched());
}

// -- PasswordInputModel ------------------------------------------------------

#[test]
fn a_new_secret_field_is_pristine() {
    let model = PasswordInputModel::new();
    assert!(model.is_pristine());
    assert_eq!(model.stance(), EditStance::Pristine);
}

#[test]
fn the_secret_field_transitions_on_every_commit_shaped_event() {
    for transition in ["commit", "blur", "submit"] {
        let mut model = PasswordInputModel::new();
        match transition {
            "commit" => model.note_commit(),
            "blur" => model.note_blur(),
            "submit" => model.note_submit(),
            _ => unreachable!(),
        }
        assert_eq!(model.stance(), EditStance::Touched, "{transition} missed");
    }
}

#[test]
fn the_secret_field_carries_its_stance_into_the_mirror() {
    use crate::widget::password_host::mirror;

    let mut model = PasswordInputModel::new();
    assert!(mirror(&model).is_pristine());

    model.note_blur();
    assert!(
        mirror(&model).is_touched(),
        "the mirror is rebuilt every frame, so a stance it did not carry from \
         its source would reset the password field to pristine no matter what \
         the user did"
    );
}

#[test]
fn the_secret_field_resets_to_pristine() {
    let mut model = PasswordInputModel::new();
    model.note_submit();
    model.reset_stance();
    assert!(model.is_pristine());
}
