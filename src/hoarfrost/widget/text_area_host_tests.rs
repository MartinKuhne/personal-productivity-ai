//! Tests for the multi-line editing host (`UIF-ATOM-002e`).
//!
//! The load-bearing claims are that the host is driving the *same*
//! editing engine the single-line atom uses — so the snapshot-at-first-
//! edit rule and the exactly-once settle behave identically — and that
//! the two places the multi-line field genuinely differs from the
//! single-line one hold: `Enter` is not a settle (`Ctrl+Enter` is a
//! send that leaves the session open), and undo does not double-report.

use crate::widget::text_area_host::TEXT_AREA_KIND;
use crate::widget::text_area_host::TextAreaController;
use crate::widget::text_area_model::LineCol;
use crate::widget::text_area_model::TextAreaEmission;
use crate::widget::text_area_model::TextAreaModel;
use crate::widget::text_input::InputKind;

// ---------------------------------------------------------------------------
// Kind
// ---------------------------------------------------------------------------

#[test]
fn test_the_well_renders_as_the_text_area_kind() {
    assert_eq!(TEXT_AREA_KIND, InputKind::TextArea);
}

// ---------------------------------------------------------------------------
// Editing
// ---------------------------------------------------------------------------

#[test]
fn test_an_edit_records_history_and_emits_one_live_tick() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();

    let outcome = controller.note_edit(&mut model, "hello".to_string());
    assert_eq!(
        outcome.emissions,
        vec![TextAreaEmission::Changed(crate::binding::Live::new(
            "hello".to_string()
        ))]
    );
    assert_eq!(model.text(), "hello");
    assert!(model.is_dirty());
    assert_eq!(controller.history().undo_depth(), 1);
}

#[test]
fn test_an_edit_that_changes_nothing_says_nothing() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("same");

    let outcome = controller.note_edit(&mut model, "same".to_string());
    assert!(outcome.is_noop());
    assert_eq!(controller.history().undo_depth(), 0);
}

#[test]
fn test_a_multi_line_edit_is_carried_verbatim() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();

    controller.note_edit(&mut model, "line one\nline two\nline three".to_string());
    assert_eq!(model.line_count(), 3);
    assert!(model.text().contains('\n'));
}

#[test]
fn test_sync_does_not_overwrite_an_open_session() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("host value");
    controller.sync(&model);

    controller.note_edit(&mut model, "typed".to_string());
    let mut stale = TextAreaModel::new().with_text("host value");
    stale.set_cursor(LineCol::new(0, 0));
    controller.sync(&stale);

    assert_eq!(
        controller.value(),
        "typed",
        "the host clobbered a live session"
    );
    assert_eq!(model.text(), "typed");
}

// ---------------------------------------------------------------------------
// The Escape revert, multi-line
// ---------------------------------------------------------------------------

#[test]
fn test_escape_restores_the_value_from_before_the_first_edit() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("original\nsecond line");
    controller.sync(&model);
    controller.note_focus(&mut model, true);

    controller.note_edit(&mut model, "a".to_string());
    controller.note_edit(&mut model, "ab".to_string());
    controller.note_edit(&mut model, "abc".to_string());

    let outcome = controller.cancel(&mut model);
    assert_eq!(
        outcome.emissions,
        vec![TextAreaEmission::Cancelled(crate::binding::Cancel::new(
            "original\nsecond line".to_string()
        ))]
    );
    assert_eq!(model.text(), "original\nsecond line");
    assert!(!model.is_dirty());
    assert!(!controller.session_open());
}

#[test]
fn test_escape_with_no_session_reverts_nothing() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("untouched");
    controller.sync(&model);
    controller.note_focus(&mut model, true);

    let outcome = controller.cancel(&mut model);
    assert!(outcome.is_noop());
    assert_eq!(model.text(), "untouched");
}

#[test]
fn test_a_send_leaves_the_session_open_so_escape_still_reverts() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("seed");
    controller.sync(&model);
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "seed edited".to_string());

    let sent = controller.submit(&model);
    assert_eq!(
        sent.emissions,
        vec![TextAreaEmission::Submitted(crate::binding::Commit::new(
            "seed edited".to_string()
        ))]
    );
    assert!(
        controller.session_open(),
        "a submit settled the session; the Escape after it would have nothing to revert"
    );

    let reverted = controller.cancel(&mut model);
    assert_eq!(
        reverted.emissions,
        vec![TextAreaEmission::Cancelled(crate::binding::Cancel::new(
            "seed".to_string()
        ))]
    );
    assert_eq!(model.text(), "seed");
}

#[test]
fn test_the_snapshot_is_the_first_pre_edit_value_not_a_later_one() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("v0");
    controller.sync(&model);
    controller.note_focus(&mut model, true);

    controller.note_edit(&mut model, "v1".to_string());
    assert_eq!(controller.snapshot(), Some("v0"));
    controller.note_edit(&mut model, "v2".to_string());
    assert_eq!(
        controller.snapshot(),
        Some("v0"),
        "the snapshot moved mid-session"
    );
    controller.cancel(&mut model);
    assert_eq!(model.text(), "v0");
}

// ---------------------------------------------------------------------------
// Settle
// ---------------------------------------------------------------------------

#[test]
fn test_losing_focus_settles_the_buffer_once() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "draft".to_string());

    let settled = controller.note_focus(&mut model, false);
    assert!(
        settled
            .emissions
            .contains(&TextAreaEmission::Committed(crate::binding::Commit::new(
                "draft".to_string()
            )))
    );
    assert!(
        settled
            .emissions
            .contains(&TextAreaEmission::FocusChanged(false))
    );

    assert!(
        !controller.session_open(),
        "the settle left the editing session open"
    );
}

#[test]
fn test_losing_focus_on_a_field_that_was_never_edited_commits_nothing() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("never edited");
    controller.sync(&model);
    controller.note_focus(&mut model, true);

    let settled = controller.note_focus(&mut model, false);
    assert_eq!(
        settled.emissions,
        vec![TextAreaEmission::FocusChanged(false)],
        "focusing a field and leaving it alone woke the host with a write"
    );
}

#[test]
fn test_an_explicit_settle_closes_the_session_it_settled() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "draft".to_string());

    let settled = controller.commit();
    assert_eq!(
        settled.emissions,
        vec![TextAreaEmission::Committed(crate::binding::Commit::new(
            "draft".to_string()
        ))]
    );
    assert!(!controller.session_open());
}

#[test]
fn test_a_blur_after_a_settle_reports_no_edge_and_nothing_else() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "draft".to_string());
    controller.note_focus(&mut model, false);

    let again = controller.note_focus(&mut model, false);
    assert!(again.is_noop(), "a second falling edge was reported");
}

// ---------------------------------------------------------------------------
// Undo and redo
// ---------------------------------------------------------------------------

#[test]
fn test_undo_restores_the_previous_buffer_and_reports_once() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_edit(&mut model, "one".to_string());
    controller.note_edit(&mut model, "two".to_string());

    let outcome = controller.undo(&mut model);
    assert_eq!(
        outcome.emissions,
        vec![TextAreaEmission::Undone("one".to_string())]
    );
    assert_eq!(model.text(), "one");
    assert_eq!(controller.undos(), 1);
}

#[test]
fn test_undo_does_not_also_emit_a_live_tick() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_edit(&mut model, "one".to_string());

    let outcome = controller.undo(&mut model);
    assert_eq!(
        outcome.emissions.len(),
        1,
        "undo reported the same change twice"
    );
    assert!(
        !outcome
            .emissions
            .contains(&TextAreaEmission::Changed(crate::binding::Live::new(
                "one".to_string()
            )))
    );
}

#[test]
fn test_redo_steps_forward_again() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_edit(&mut model, "one".to_string());
    controller.note_edit(&mut model, "two".to_string());
    controller.undo(&mut model);

    let outcome = controller.redo(&mut model);
    assert_eq!(
        outcome.emissions,
        vec![TextAreaEmission::Redone("two".to_string())]
    );
    assert_eq!(model.text(), "two");
    assert_eq!(controller.redos(), 1);
}

#[test]
fn test_undo_with_an_empty_ledger_says_nothing() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("nothing to undo");
    assert!(controller.undo(&mut model).is_noop());
    assert!(controller.redo(&mut model).is_noop());
    assert_eq!(model.text(), "nothing to undo");
}

#[test]
fn test_the_engine_knows_about_the_undone_value() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("start");
    controller.sync(&model);
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "changed".to_string());
    controller.undo(&mut model);

    assert_eq!(controller.value(), "start");
    let settled = controller.commit();
    assert_eq!(
        settled.emissions,
        vec![TextAreaEmission::Committed(crate::binding::Commit::new(
            "start".to_string()
        ))]
    );
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

#[test]
fn test_a_cursor_move_reports_only_when_it_moves() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("a\nb\nc");

    assert!(
        controller
            .note_cursor(&mut model, LineCol::origin())
            .is_noop()
    );

    let outcome = controller.note_cursor(&mut model, LineCol::new(1, 1));
    assert_eq!(
        outcome.emissions,
        vec![TextAreaEmission::CursorChanged(LineCol::new(1, 1))]
    );
    assert_eq!(model.cursor(), LineCol::new(1, 1));
}

#[test]
fn test_a_cursor_move_follows_the_scroll_only_when_it_has_to() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new()
        .with_growth(crate::widget::text_area_model::Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");

    controller.note_cursor(&mut model, LineCol::new(0, 0));
    let resting = model.scroll_offset();
    assert_eq!(resting, 0.0);

    controller.note_cursor(&mut model, LineCol::new(3, 0));
    assert!(
        model.scroll_offset() > resting,
        "the caret moved off-screen and the well did not follow"
    );
    assert!(model.scroll_offset() <= model.max_scroll());
}

#[test]
fn test_a_stale_cursor_is_clamped_instead_of_panicking() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new().with_text("short");
    let outcome = controller.note_cursor(&mut model, LineCol::new(99, 99));
    assert!(!outcome.is_noop());
    assert_eq!(model.cursor(), LineCol::new(0, 5));
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn test_reset_forgets_session_history_and_transcript() {
    let mut controller = TextAreaController::new();
    let mut model = TextAreaModel::new();
    controller.note_focus(&mut model, true);
    controller.note_edit(&mut model, "a".to_string());
    let _ = controller.submit(&model);
    controller.undo(&mut model);
    controller.redo(&mut model);

    controller.reset();
    assert_eq!(controller.submits(), 0);
    assert_eq!(controller.undos(), 0);
    assert_eq!(controller.redos(), 0);
    assert!(!controller.session_open());
    assert!(!controller.history().can_undo());
    assert!(!controller.history().can_redo());
}
