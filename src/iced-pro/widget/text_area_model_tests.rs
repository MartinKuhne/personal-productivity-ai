//! Tests for the multi-line model (`UIF-ATOM-002e`).
//!
//! The load-bearing claims are that the growth solver is arithmetic on
//! the tokens rather than on the number §3.2 printed (and that the
//! disagreement is therefore visible), that `LineCol` survives being
//! handed a buffer it was not computed against, and that undo and redo
//! are exact inverses across any number of steps.

use crate::token::space;
use crate::token::typography;
use crate::widget::text_area_model::EditHistory;
use crate::widget::text_area_model::Growth;
use crate::widget::text_area_model::LineCol;
use crate::widget::text_area_model::ParseDiagnostic;
use crate::widget::text_area_model::TextAreaEmission;
use crate::widget::text_area_model::TextAreaModel;

const LINE_HEIGHT: f32 = typography::BODY.line_height;
const PADDING: f32 = space::SM * 2.0;

// ---------------------------------------------------------------------------
// LineCol
// ---------------------------------------------------------------------------

#[test]
fn test_the_origin_is_the_first_position_of_the_first_line() {
    assert!(LineCol::origin().is_origin());
    assert!(!LineCol::new(0, 1).is_origin());
    assert!(!LineCol::new(1, 0).is_origin());
}

#[test]
fn test_linecol_resolves_to_the_right_byte_offset() {
    let text = "one\ntwo\nthree";
    assert_eq!(LineCol::new(0, 0).offset_in(text), 0);
    assert_eq!(LineCol::new(0, 3).offset_in(text), 3);
    assert_eq!(LineCol::new(1, 0).offset_in(text), 4);
    assert_eq!(LineCol::new(1, 3).offset_in(text), 7);
    assert_eq!(LineCol::new(2, 2).offset_in(text), 10);
}

#[test]
fn test_offset_resolves_back_to_the_same_linecol() {
    let text = "one\ntwo\nthree";
    for offset in 0..=text.len() {
        let at = LineCol::from_offset(text, offset);
        assert_eq!(at.offset_in(text), offset, "round trip broke at {offset}");
    }
}

#[test]
fn test_columns_count_characters_not_bytes() {
    let text = "héllo\nwörld";
    // "é" is two bytes and one character; column 2 is after it.
    let at = LineCol::new(0, 2).offset_in(text);
    assert_eq!(&text[at..at + 1], "l");
    assert_eq!(LineCol::from_offset(text, at), LineCol::new(0, 2));
}

#[test]
fn test_a_line_past_the_end_of_the_buffer_resolves_to_its_end() {
    let text = "short";
    assert_eq!(LineCol::new(9, 0).offset_in(text), text.len());
    assert_eq!(LineCol::new(9, 40).offset_in(text), text.len());
}

#[test]
fn test_a_column_past_the_end_of_a_line_clamps_to_that_line_not_the_buffer() {
    let text = "ab\ncdef";
    // Line 0 has two characters; overshooting must land at the end of
    // line 0, not run on into line 1.
    assert_eq!(LineCol::new(0, 99).offset_in(text), 2);
}

#[test]
fn test_clamping_a_stale_coordinate_never_panics() {
    let stale = LineCol::new(40, 40);
    let clamped = stale.clamped_to("one\ntwo");
    assert_eq!(clamped, LineCol::from_offset("one\ntwo", 7));
}

// ---------------------------------------------------------------------------
// The growth solver
// ---------------------------------------------------------------------------

#[test]
fn test_a_row_is_one_line_of_body_type() {
    assert_eq!(Growth::line_height(), LINE_HEIGHT);
    assert_eq!(Growth::text_height(3), 3.0 * LINE_HEIGHT);
}

#[test]
fn test_three_rows_is_seventy_six_not_the_seventy_two_the_spec_printed() {
    // §3.2 says "min_height: 72px / 3 lines of type.body" and, in the
    // same sentence, "padding space.sm = 8px vertical". The tokens say
    // 3 x 20 + 2 x 8 = 76. The solver follows the tokens.
    assert_eq!(
        Growth::new(3, None).min_height(),
        3.0 * LINE_HEIGHT + PADDING
    );
    assert_eq!(Growth::new(3, None).min_height(), 76.0);
}

#[test]
fn test_the_band_is_normalised_so_the_floor_is_never_above_the_ceiling() {
    let inverted = Growth::new(5, Some(2));
    assert_eq!(inverted.max_rows, Some(5));
    assert!(inverted.min_height() <= inverted.max_height());
}

#[test]
fn test_an_uncapped_band_has_no_ceiling() {
    assert_eq!(Growth::new(2, None).max_height(), f32::INFINITY);
    assert!(Growth::new(2, None).can_grow(10_000));
}

#[test]
fn test_height_grows_with_the_content_and_stops_at_the_ceiling() {
    let band = Growth::new(2, Some(4));
    assert_eq!(band.height_for_lines(1), band.min_height());
    assert_eq!(band.height_for_lines(2), band.min_height());
    assert_eq!(band.height_for_lines(3), Growth::height_at(3));
    assert_eq!(band.height_for_lines(4), band.max_height());
    assert_eq!(band.height_for_lines(9), band.max_height());
}

#[test]
fn test_an_empty_buffer_still_holds_one_line() {
    let band = Growth::new(1, None);
    assert_eq!(band.height_for_lines(0), Growth::height_at(1));
}

#[test]
fn test_clamp_height_applies_the_ceiling_as_much_as_the_floor() {
    // §3.2e's closed form clamps the row count between both rungs, so
    // the height-space equivalent has to clamp both ways. A host that
    // hands back a rectangle taller than `max_rows` must not get to
    // keep it, or the cap is a suggestion.
    let band = Growth::new(2, Some(4));
    assert_eq!(band.clamp_height(1.0), band.min_height(), "floor");
    assert_eq!(
        band.clamp_height(Growth::height_at(3)),
        Growth::height_at(3)
    );
    assert_eq!(band.clamp_height(10_000.0), band.max_height(), "ceiling");
    // An uncapped band clamps upward only.
    let open = Growth::new(3, None);
    assert_eq!(open.clamp_height(1.0), open.min_height());
    assert_eq!(open.clamp_height(5_000.0), 5_000.0, "no ceiling to stop it");
    // A band built outside `new` can be inverted. `max` then `min`
    // resolves to the ceiling instead of panicking the way `f32::clamp`
    // does when handed min > max.
    let inverted = Growth {
        min_rows: 6,
        max_rows: Some(2),
    };
    assert!(inverted.min_height() > inverted.max_height());
    assert_eq!(inverted.clamp_height(100.0), inverted.max_height());
    assert_eq!(inverted.clamp_height(1.0), inverted.max_height());
}

#[test]
fn test_rows_fitting_counts_whole_rows_only() {
    assert_eq!(Growth::rows_fitting(Growth::height_at(3)), 3);
    assert_eq!(Growth::rows_fitting(Growth::height_at(3) - 1.0), 2);
    assert_eq!(Growth::rows_fitting(0.0), 0);
}

#[test]
fn test_the_solver_is_built_from_tokens_not_restated_numbers() {
    // Every height in this file must be reachable from the tokens, so a
    // change to space.sm or BODY.line_height moves all of them together
    // rather than leaving one constant behind.
    assert_eq!(Growth::height_at(1), LINE_HEIGHT + 2.0 * space::SM);
    assert_eq!(Growth::height_at(4), 4.0 * LINE_HEIGHT + 2.0 * space::SM);
    assert_eq!(PADDING, 2.0 * space::SM);
    assert_eq!(LINE_HEIGHT, typography::BODY.line_height);
}

// ---------------------------------------------------------------------------
// EditHistory
// ---------------------------------------------------------------------------

#[test]
fn test_a_fresh_ledger_can_undo_nothing() {
    let history = EditHistory::new();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.undo_depth(), 0);
}

#[test]
fn test_undo_and_redo_are_exact_inverses() {
    let mut history = EditHistory::new();
    // Each record is the value that stood *before* the edit that follows,
    // so the stack reads bottom-up as the timeline the user walked.
    history.record("");
    history.record("a");
    history.record("ab");

    let back1 = history.undo("abc").expect("undo 1");
    assert_eq!(back1, "ab");
    let back2 = history.undo(&back1).expect("undo 2");
    assert_eq!(back2, "a");
    let back3 = history.undo(&back2).expect("undo 3");
    assert_eq!(back3, "");

    let fwd1 = history.redo(&back3).expect("redo 1");
    assert_eq!(fwd1, "a");
    let fwd2 = history.redo(&fwd1).expect("redo 2");
    assert_eq!(fwd2, "ab");
    let fwd3 = history.redo(&fwd2).expect("redo 3");
    assert_eq!(fwd3, "abc");
}

#[test]
fn test_a_new_edit_destroys_the_redo_stack() {
    let mut history = EditHistory::new();
    history.record("a");
    let _ = history.undo("ab");
    assert!(history.can_redo());

    history.record("zz");
    assert!(!history.can_redo());
}

#[test]
fn test_the_ledger_evicts_its_oldest_step_at_the_limit() {
    let mut history = EditHistory::with_limit(3);
    for index in 0..5 {
        history.record(&format!("v{index}"));
    }
    assert_eq!(history.undo_depth(), 3);
    assert_eq!(
        history.undo("v5").expect("still undoable"),
        "v4",
        "the newest recorded step must survive; the eviction drops the oldest"
    );
    assert_eq!(history.undo_depth(), 2);
}

#[test]
fn test_a_limit_of_one_still_holds_one_step() {
    let mut history = EditHistory::with_limit(0);
    history.record("a");
    assert_eq!(history.undo("b"), Some("a".to_string()));
    assert_eq!(history.undo("a"), None);
}

#[test]
fn test_clearing_loses_both_stacks() {
    let mut history = EditHistory::new();
    history.record("a");
    let _ = history.undo("b");
    history.clear();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

// ---------------------------------------------------------------------------
// TextAreaModel
// ---------------------------------------------------------------------------

#[test]
fn test_an_empty_well_is_one_line_tall_at_its_floor() {
    let model = TextAreaModel::new();
    assert_eq!(model.line_count(), 1);
    assert_eq!(model.height(), Growth::new(3, None).min_height());
    assert!(!model.is_scrollable());
    assert_eq!(model.max_scroll(), 0.0);
}

#[test]
fn test_the_well_grows_until_it_hits_its_ceiling() {
    let model = TextAreaModel::new().with_growth(Growth::new(2, Some(3)));
    assert_eq!(model.height(), Growth::height_at(2));

    let grown = model.clone().with_text("a\nb\nc");
    assert_eq!(grown.line_count(), 3);
    assert_eq!(grown.height(), Growth::height_at(3));

    let past = model.with_text("a\nb\nc\nd");
    assert_eq!(past.line_count(), 4);
    assert_eq!(past.height(), Growth::height_at(3));
    assert!(past.is_scrollable());
}

#[test]
fn test_max_scroll_is_the_surplus_over_the_visible_text_height() {
    let model = TextAreaModel::new()
        .with_growth(Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");
    assert_eq!(model.content_height(), 4.0 * LINE_HEIGHT);
    assert_eq!(model.visible_text_height(), 2.0 * LINE_HEIGHT);
    assert_eq!(model.max_scroll(), 2.0 * LINE_HEIGHT);
}

#[test]
fn test_scroll_is_clamped_to_what_the_content_allows() {
    let mut model = TextAreaModel::new()
        .with_growth(Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");
    assert_eq!(model.set_scroll(9_000.0), model.max_scroll());
    assert_eq!(model.set_scroll(-50.0), 0.0);
}

#[test]
fn test_scrolling_to_a_visible_line_moves_nothing() {
    let mut model = TextAreaModel::new()
        .with_growth(Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");
    model.set_scroll(LINE_HEIGHT);
    let before = model.scroll_offset();
    assert_eq!(model.scroll_to_show(LineCol::new(1, 0)), before);
    assert_eq!(model.scroll_to_show(LineCol::new(2, 0)), before);
}

#[test]
fn test_scrolling_to_a_line_above_the_top_pulls_it_to_the_first_pixel() {
    let mut model = TextAreaModel::new()
        .with_growth(Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");
    model.set_scroll(2.0 * LINE_HEIGHT);
    assert_eq!(model.scroll_to_show(LineCol::new(0, 0)), 0.0);
}

#[test]
fn test_scrolling_to_a_line_below_the_fold_puts_its_last_pixel_on_it() {
    let mut model = TextAreaModel::new()
        .with_growth(Growth::new(1, Some(2)))
        .with_text("a\nb\nc\nd");
    let applied = model.scroll_to_show(LineCol::new(3, 0));
    assert_eq!(applied, 2.0 * LINE_HEIGHT);
    assert_eq!(applied, model.max_scroll());
}

#[test]
fn test_setting_text_marks_the_buffer_dirty_and_mark_clean_clears_it() {
    let mut model = TextAreaModel::new();
    assert!(!model.is_dirty());
    model.set_text("typed");
    assert!(model.is_dirty());
    model.mark_clean();
    assert!(!model.is_dirty());
}

#[test]
fn test_the_cursor_is_clamped_to_the_buffer_it_was_given() {
    let mut model = TextAreaModel::new().with_text("ab\ncd");
    model.set_cursor(LineCol::new(40, 40));
    assert_eq!(model.cursor(), LineCol::new(1, 2));
}

#[test]
fn test_a_diagnostic_carries_a_place_a_caret_can_be_moved_to() {
    let mut model = TextAreaModel::new().with_text("ok\nbad token here");
    let at = LineCol::new(1, 4);
    model.set_validation(Some(ParseDiagnostic::new(at, "unexpected token")));

    let diagnostic = model.validation().expect("a diagnostic");
    assert_eq!(diagnostic.at, at);
    assert_eq!(diagnostic.message, "unexpected token");
    assert!(model.is_invalid());
    assert_eq!(
        diagnostic.at.offset_in(model.text()),
        "ok\n".len() + 4,
        "the diagnostic's place must resolve into the buffer it describes"
    );
}

#[test]
fn test_clearing_the_diagnostic_clears_invalidity() {
    let mut model = TextAreaModel::new();
    model.set_validation(Some(ParseDiagnostic::new(LineCol::origin(), "x")));
    assert!(model.is_invalid());
    model.set_validation(None);
    assert!(!model.is_invalid());
}

// ---------------------------------------------------------------------------
// Emission shape
// ---------------------------------------------------------------------------

#[test]
fn test_only_commit_and_cancel_settle() {
    assert!(
        TextAreaEmission::Committed(crate::binding::Commit::new("x".to_string())).settles(),
        "focus loss must settle"
    );
    assert!(
        TextAreaEmission::Cancelled(crate::binding::Cancel::new("x".to_string())).settles(),
        "Escape must settle"
    );
    assert!(
        !TextAreaEmission::Submitted(crate::binding::Commit::new("x".to_string())).settles(),
        "a submit must leave the user in the field with the session open"
    );
    assert!(!TextAreaEmission::CursorChanged(LineCol::origin()).settles());
    assert!(!TextAreaEmission::FocusChanged(true).settles());
}

#[test]
fn test_submit_and_commit_are_never_equal() {
    let submitted = TextAreaEmission::Submitted(crate::binding::Commit::new("x".to_string()));
    let committed = TextAreaEmission::Committed(crate::binding::Commit::new("x".to_string()));
    assert_ne!(submitted, committed);
    assert_eq!(submitted.tier(), "commit");
    assert_eq!(committed.tier(), "commit");
}

#[test]
fn test_every_value_bearing_arm_reports_its_value() {
    assert_eq!(
        TextAreaEmission::Changed(crate::binding::Live::new("a".to_string())).value(),
        Some("a")
    );
    assert_eq!(
        TextAreaEmission::Submitted(crate::binding::Commit::new("b".to_string())).value(),
        Some("b")
    );
    assert_eq!(
        TextAreaEmission::Cancelled(crate::binding::Cancel::new("c".to_string())).value(),
        Some("c")
    );
    assert_eq!(TextAreaEmission::Undone("d".to_string()).value(), Some("d"));
    assert_eq!(TextAreaEmission::Redone("e".to_string()).value(), Some("e"));
    assert_eq!(
        TextAreaEmission::CursorChanged(LineCol::new(1, 1)).value(),
        None
    );
    assert_eq!(TextAreaEmission::FocusChanged(false).value(), None);
}

#[test]
fn test_history_arms_report_their_own_tier() {
    assert_eq!(TextAreaEmission::Undone("a".to_string()).tier(), "history");
    assert_eq!(TextAreaEmission::Redone("a".to_string()).tier(), "history");
    assert_eq!(
        TextAreaEmission::CursorChanged(LineCol::origin()).tier(),
        "cursor"
    );
}

// ---------------------------------------------------------------------------
// Font family tests (`UIF-ATOM-002e`)
// ---------------------------------------------------------------------------

#[test]
fn test_default_font_family_is_sans() {
    let model = TextAreaModel::new();
    assert_eq!(model.font_family(), typography::FontFamily::Sans);
}

#[test]
fn test_with_font_family_sets_family() {
    let model = TextAreaModel::new().with_font_family(typography::FontFamily::Mono);
    assert_eq!(model.font_family(), typography::FontFamily::Mono);

    let back_to_sans = model.with_font_family(typography::FontFamily::Sans);
    assert_eq!(back_to_sans.font_family(), typography::FontFamily::Sans);
}

#[test]
fn test_mono_builder_sets_monospace_family() {
    let model = TextAreaModel::new().mono();
    assert_eq!(model.font_family(), typography::FontFamily::Mono);
}
