//! Tests for the non-destructive recovery path — buffer, selection and
//! scroll offset surviving a rejected edit (`UIF-PRIN-009` clause 3).

use crate::widget::text_area_model::{Growth, TextAreaModel};
use crate::widget::text_input_model::{TextInputModel, TextRange};

use super::EditSnapshot;
use super::RecoverySession;
use super::RestoredFields;
use super::ScrollOffset;
use crate::binding::snapshot::GestureStart;

/// A well tall enough that its content overflows and it can actually scroll.
fn scrollable_well() -> TextAreaModel {
    let mut model = TextAreaModel::new().with_growth(Growth::new(2, Some(3)));
    model.set_text("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten");
    model
}

// -- ScrollOffset -----------------------------------------------------------

#[test]
fn a_scroll_offset_never_goes_negative() {
    assert_eq!(ScrollOffset::new(-40.0).get(), 0.0);
    assert!(ScrollOffset::new(-40.0).is_zero());
}

#[test]
fn a_scroll_offset_passes_a_positive_value_through() {
    assert_eq!(ScrollOffset::new(48.0).get(), 48.0);
    assert!(!ScrollOffset::new(48.0).is_zero());
}

#[test]
fn the_zero_offset_constant_is_zero() {
    assert!(ScrollOffset::ZERO.is_zero());
    assert_eq!(ScrollOffset::ZERO.get(), 0.0);
}

#[test]
fn a_scroll_offset_renders_with_its_unit() {
    assert_eq!(ScrollOffset::new(12.0).to_string(), "12px");
}

// -- RestoredFields ---------------------------------------------------------

#[test]
fn the_none_constant_moved_nothing() {
    assert!(!RestoredFields::NONE.any());
    assert_eq!(RestoredFields::NONE.count(), 0);
}

#[test]
fn the_count_is_the_number_of_fields_that_moved() {
    assert_eq!(
        RestoredFields {
            buffer: true,
            selection: true,
            scroll: true,
        }
        .count(),
        3
    );
    assert_eq!(
        RestoredFields {
            buffer: true,
            selection: false,
            scroll: true,
        }
        .count(),
        2
    );
    assert_eq!(
        RestoredFields {
            buffer: false,
            selection: true,
            scroll: false,
        }
        .count(),
        1
    );
}

#[test]
fn any_is_true_whenever_the_count_is_nonzero() {
    for buffer in [false, true] {
        for selection in [false, true] {
            for scroll in [false, true] {
                let fields = RestoredFields {
                    buffer,
                    selection,
                    scroll,
                };
                assert_eq!(fields.any(), fields.count() > 0);
            }
        }
    }
}

// -- capture --------------------------------------------------------------

#[test]
fn a_hand_built_snapshot_carries_what_it_is_given() {
    let snapshot = EditSnapshot::new(
        "draft",
        Some(TextRange::new(1, 4)),
        Some(ScrollOffset::new(20.0)),
    );
    assert_eq!(snapshot.buffer(), "draft");
    assert_eq!(snapshot.selection(), Some(TextRange::new(1, 4)));
    assert_eq!(snapshot.scroll(), Some(ScrollOffset::new(20.0)));
    assert_eq!(snapshot.carried_count(), 3);
}

#[test]
fn a_single_line_capture_carries_two_of_the_three() {
    let model = TextInputModel::new("rename me").with_selection(TextRange::new(0, 6));
    let snapshot = EditSnapshot::from_text_input(&model);
    assert_eq!(snapshot.buffer(), "rename me");
    assert_eq!(snapshot.selection(), Some(TextRange::new(0, 6)));
    assert_eq!(
        snapshot.scroll(),
        None,
        "TextInputModel carries no scroll position, so claiming one would be \
         restoring something that was never there"
    );
    assert_eq!(snapshot.carried_count(), 2);
}

#[test]
fn a_multi_line_capture_carries_all_three() {
    let mut model = scrollable_well();
    let limit = model.max_scroll();
    assert!(limit > 0.0, "the fixture must actually overflow");
    model.set_scroll(limit);
    model.set_selection(Some(TextRange::new(4, 9)));
    assert_eq!(model.scroll_offset(), limit);
    assert!(model.is_dirty());

    let snapshot = EditSnapshot::from_text_area(&model);
    assert_eq!(snapshot.buffer(), model.text());
    assert_eq!(snapshot.selection(), Some(TextRange::new(4, 9)));
    assert!(
        snapshot
            .scroll()
            .is_some_and(|offset| (offset.get() - limit).abs() < f32::EPSILON),
        "the scroll offset was not carried"
    );
    assert_eq!(snapshot.carried_count(), 3);
    assert!(snapshot.was_dirty());
}

#[test]
fn a_clean_well_is_captured_as_clean() {
    let model = TextAreaModel::new();
    let snapshot = EditSnapshot::from_text_area(&model);
    assert!(!snapshot.was_dirty());
}

// -- restore: single line ---------------------------------------------------

#[test]
fn a_restore_into_a_single_line_field_returns_buffer_and_selection() {
    let mut model = TextInputModel::new("original").with_selection(TextRange::new(0, 8));
    let snapshot = EditSnapshot::from_text_input(&model);

    model.value = "typo'd value".to_string();
    model.selection = Some(TextRange::caret(12));

    let restored = snapshot.restore_text_input(&mut model);
    assert_eq!(model.value, "original");
    assert_eq!(model.selection, Some(TextRange::new(0, 8)));
    assert_eq!(restored.count(), 2);
    assert!(restored.buffer);
    assert!(restored.selection);
}

#[test]
fn restoring_into_an_empty_never_edited_field_is_safe() {
    let snapshot = EditSnapshot::from_text_input(&TextInputModel::empty());
    let mut model = TextInputModel::empty();
    let restored = snapshot.restore_text_input(&mut model);
    assert_eq!(restored, RestoredFields::NONE);
    assert_eq!(model.value, "");
    assert_eq!(model.selection, None);
}

#[test]
fn a_selection_pointing_past_the_restored_value_is_clamped_not_panicking() {
    let mut model =
        TextInputModel::new("a very long filename indeed").with_selection(TextRange::new(5, 25));
    let snapshot = EditSnapshot::from_text_input(&model);

    model.value = "long".to_string();
    let restored = snapshot.restore_text_input(&mut model);

    assert_eq!(model.value, "a very long filename indeed");
    let selection = model.selection.expect("a selection was carried");
    assert!(selection.end <= model.value.len());
    assert!(selection.start <= selection.end);
    assert!(restored.buffer);
}

#[test]
fn a_stale_selection_surviving_a_short_restore_cannot_split_a_character() {
    let mut model = TextInputModel::new("ünïcode name");
    let snapshot = EditSnapshot::from_text_input(&model);
    model.value = "ab".to_string();
    let restored = snapshot.restore_text_input(&mut model);
    assert!(restored.buffer);
    let selection = model.selection.unwrap_or(TextRange::caret(0));
    assert!(selection.text_of(&model.value).is_some());
}

// -- restore: multi line ---------------------------------------------------

#[test]
fn a_restore_into_a_well_returns_all_three_fields() {
    let mut model = scrollable_well();
    let limit = model.max_scroll();
    model.set_scroll(limit);
    model.set_selection(Some(TextRange::new(3, 7)));
    let snapshot = EditSnapshot::from_text_area(&model);

    model.set_text("everything changed now\nand grew\nmore");
    model.set_scroll(0.0);
    model.set_selection(None);

    let restored = snapshot.restore_text_area(&mut model);
    assert_eq!(model.text(), snapshot.buffer());
    assert_eq!(model.selection(), Some(TextRange::new(3, 7)));
    assert_eq!(
        model.scroll_offset(),
        snapshot.scroll().expect("carried").get()
    );
    assert_eq!(restored.count(), 3, "{restored}");
    assert!(restored.buffer && restored.selection && restored.scroll);
}

#[test]
fn restoring_into_an_empty_never_edited_well_is_safe() {
    let snapshot = EditSnapshot::from_text_area(&TextAreaModel::new());
    let mut model = TextAreaModel::new();
    let restored = snapshot.restore_text_area(&mut model);
    assert_eq!(restored, RestoredFields::NONE);
    assert_eq!(model.text(), "");
    assert_eq!(model.selection(), None);
    assert_eq!(model.scroll_offset(), 0.0);
    assert!(!model.is_dirty());
}

#[test]
fn a_restore_does_not_make_a_clean_well_dirty() {
    let mut model = TextAreaModel::new();
    model.set_text("loaded from the host");
    model.mark_clean();
    assert!(!model.is_dirty());

    let snapshot = EditSnapshot::from_text_area(&model);
    model.set_text("user typed this");
    assert!(model.is_dirty());

    let restored = snapshot.restore_text_area(&mut model);
    assert!(restored.buffer);
    assert_eq!(model.text(), "loaded from the host");
    assert!(
        !model.is_dirty(),
        "reverting a clean field must not claim unsaved changes"
    );
}

#[test]
fn a_restore_keeps_a_dirty_well_dirty() {
    let mut model = TextAreaModel::new();
    model.set_text("an unsaved draft");
    assert!(model.is_dirty());

    let snapshot = EditSnapshot::from_text_area(&model);
    model.set_text("an unsaved draft edited further");
    let restored = snapshot.restore_text_area(&mut model);
    assert_eq!(model.text(), "an unsaved draft");
    assert!(restored.buffer);
    assert!(
        model.is_dirty(),
        "the well was dirty before the session and must still be dirty after \
         reverting into the middle of that draft"
    );
}

#[test]
fn a_scroll_restore_reports_what_the_model_actually_holds() {
    // Snapshot taken while the well was tall and scrolled; the well is then
    // shrunk so the carried offset cannot land where it started. The
    // report must describe the clamped result, not the carried value.
    let mut model = scrollable_well();
    let limit = model.max_scroll();
    model.set_scroll(limit);
    let snapshot = EditSnapshot::from_text_area(&model);

    model.set_scroll(0.0);
    model.set_text("one\ntwo");
    let restored = snapshot.restore_text_area(&mut model);

    assert_eq!(model.text(), snapshot.buffer());
    assert_eq!(model.scroll_offset(), limit);
    assert!(restored.scroll);
}

// -- the session guard ------------------------------------------------------

#[test]
fn a_session_with_nothing_open_restores_nothing() {
    let mut session = RecoverySession::new();
    assert!(!session.is_open());
    assert!(session.cancel().is_none());

    let mut model = TextInputModel::new("untouched");
    assert!(session.cancel_into_text_input(&mut model).is_none());
    assert_eq!(model.value, "untouched");
}

#[test]
fn a_full_round_trip_through_the_existing_guard_returns_all_three() {
    let mut model = scrollable_well();
    let limit = model.max_scroll();
    model.set_scroll(limit);
    model.set_selection(Some(TextRange::new(2, 6)));

    let mut session = RecoverySession::new();
    assert_eq!(session.begin_from_text_area(&model), GestureStart::Started);
    assert!(session.is_open());

    model.set_text("changed\nand changed again\nplus more lines");
    model.set_scroll(0.0);
    model.set_selection(None);

    let restored = session
        .cancel_into_text_area(&mut model)
        .expect("a session was open");
    assert_eq!(restored.count(), 3);
    assert_eq!(model.selection(), Some(TextRange::new(2, 6)));
    assert_eq!(model.scroll_offset(), limit);
    assert!(!session.is_open());
}

#[test]
fn the_outer_snapshot_governs_a_nested_begin() {
    let mut session = RecoverySession::new();
    let first = EditSnapshot::new(
        "first",
        Some(TextRange::caret(2)),
        Some(ScrollOffset::new(30.0)),
    );
    let second = EditSnapshot::new("second", None, None);

    assert_eq!(session.begin(&first), GestureStart::Started);
    assert_eq!(
        session.begin(&second),
        GestureStart::AlreadyOpen,
        "a nested edit must not silently shorten the revert"
    );

    let cancel = session.cancel().expect("the outer session is open");
    assert_eq!(cancel.restored().buffer(), "first");
    assert_eq!(cancel.restored().selection(), Some(TextRange::caret(2)));
    assert_eq!(cancel.restored().scroll(), Some(ScrollOffset::new(30.0)));
}

#[test]
fn settling_closes_the_session_without_restoring_anything() {
    let mut model = TextInputModel::new("committed value");
    let mut session = RecoverySession::new();
    assert_eq!(session.begin_from_text_input(&model), GestureStart::Started);

    model.value = "accepted".to_string();
    assert!(session.settle());
    assert!(!session.is_open());
    assert_eq!(model.value, "accepted");
    assert!(!session.settle(), "nothing was open to settle");
}

#[test]
fn a_settled_session_cannot_be_cancelled_into_the_model() {
    let mut model = TextInputModel::new("before");
    let mut session = RecoverySession::new();
    assert_eq!(session.begin_from_text_input(&model), GestureStart::Started);
    assert!(session.settle());

    model.value = "after".to_string();
    assert!(session.cancel_into_text_input(&mut model).is_none());
    assert_eq!(model.value, "after");
}

#[test]
fn two_identical_restores_agree() {
    let snapshot = EditSnapshot::new(
        "same buffer",
        Some(TextRange::new(0, 4)),
        Some(ScrollOffset::new(10.0)),
    );
    let mut a = TextInputModel::new("different");
    let mut b = TextInputModel::new("different");
    let first = snapshot.restore_text_input(&mut a);
    let second = snapshot.restore_text_input(&mut b);
    assert_eq!(first, second);
    assert_eq!(a, b);
}

#[test]
fn a_snapshot_held_by_value_does_not_follow_the_model() {
    let mut model = TextInputModel::new("at capture time");
    let snapshot = EditSnapshot::from_text_input(&model);
    model.value = "mutated afterwards".to_string();
    assert_eq!(
        snapshot.buffer(),
        "at capture time",
        "the snapshot aliased live model data, so reverting would restore \
         whatever the model had become"
    );
}
