//! Tests for `ui_iced/editor.rs` (UI-037..UI-045).
//!
//! The history stack and the action-classification rule are the test-critical
//! parts; the view functions are asserted to build for every state they can
//! be handed (front-matter round-trip, cancel discards, multibyte).

use super::*;
use crate::markdown::DocumentContent;
use crate::ui::strings;
use iced::widget::text_editor::{Edit, Motion, Position};
use iced_pro::surface::modal::{
    EDITOR_HEIGHT_MAX, EDITOR_HEIGHT_MIN, EDITOR_WIDTH_MAX, EDITOR_WIDTH_MIN,
};
use std::path::PathBuf;
use std::sync::Arc;

/// A raw file with a YAML front-matter block and a body.
const RAW_WITH_FRONT_MATTER: &str = "---\ntitle: Test\ntags: [a, b]\n---\n# Body\n\ntext\n";

/// The body `DocumentContent::parse` yields for [`RAW_WITH_FRONT_MATTER`].
const BODY_WITH_FRONT_MATTER: &str = "\n# Body\n\ntext\n";

/// A body with no front matter at all.
const RAW_NO_FRONT_MATTER: &str = "# Just markdown\n\nplain body\n";

/// A body whose fence never closes — UI-043's failure case.
const UNTERMINATED_FENCE_BODY: &str = "# Title\n\n```\nstill inside the fence\n";

/// A fresh buffer opened on a valid, front-matter-free document.
fn open_vm(raw: &str) -> EditorViewModel {
    EditorViewModel::open("C:/tmp/note.md", raw)
}

/// A typical desktop viewport the view functions are built against.
///
/// Wide and tall enough that [`EDITOR_SIZING`] lands on its *fraction* arm
/// (80%) rather than a clamp, so a view-construction test never depends on
/// the card being squeezed. The clamp arms are pinned separately, in
/// [`editor_size_resolves_per_viewport`].
const TEST_VIEWPORT: Size = Size::new(1600.0, 1000.0);

/// Every `Action` variant, with a representative payload, for exhaustive
/// classification coverage.
fn every_action() -> Vec<Action> {
    vec![
        Action::Move(Motion::Left),
        Action::Move(Motion::Right),
        Action::Move(Motion::Up),
        Action::Move(Motion::Down),
        Action::Move(Motion::WordLeft),
        Action::Move(Motion::WordRight),
        Action::Move(Motion::Home),
        Action::Move(Motion::End),
        Action::Move(Motion::PageUp),
        Action::Move(Motion::PageDown),
        Action::Move(Motion::DocumentStart),
        Action::Move(Motion::DocumentEnd),
        Action::Select(Motion::Left),
        Action::Select(Motion::Right),
        Action::Select(Motion::DocumentEnd),
        Action::SelectWord,
        Action::SelectLine,
        Action::SelectAll,
        Action::Edit(Edit::Insert('x')),
        Action::Edit(Edit::Paste(Arc::new("pasted".into()))),
        Action::Edit(Edit::Enter),
        Action::Edit(Edit::Indent),
        Action::Edit(Edit::Unindent),
        Action::Edit(Edit::Backspace),
        Action::Edit(Edit::Delete),
        Action::Click(iced::Point::new(1.0, 2.0)),
        Action::Drag(iced::Point::new(3.0, 4.0)),
        Action::Scroll { lines: 3 },
    ]
}

// =========================================================================
// EditorHistory — construction
// =========================================================================

#[test]
fn capacity_satisfies_the_ui_042_minimum() {
    // Compile-time guarantees: UI-042's floor, and the +1 that makes that
    // floor reachable (N stored snapshots only give N-1 undo steps).
    const _: () = assert!(
        MIN_UNDO_STEPS >= 100,
        "UI-042 requires at least 100 entries"
    );
    const _: () = assert!(
        UNDO_CAPACITY > MIN_UNDO_STEPS,
        "N snapshots only give N-1 undo steps, so the cap must exceed the floor"
    );

    assert_eq!(EditorHistory::new().capacity(), UNDO_CAPACITY);
    assert_eq!(EditorHistory::new().capacity(), MIN_UNDO_STEPS + 1);
}

#[test]
fn history_starts_empty() {
    let history = EditorHistory::new();

    assert!(history.is_empty());
    assert_eq!(history.len(), 0);
    assert_eq!(history.current(), None);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn default_history_matches_new() {
    let history = EditorHistory::default();

    assert!(history.is_empty());
    assert_eq!(history.capacity(), UNDO_CAPACITY);
}

#[test]
fn zero_capacity_is_raised_to_one() {
    assert_eq!(EditorHistory::with_capacity(0).capacity(), 1);
}

// =========================================================================
// EditorHistory — push / undo / redo round-trips
// =========================================================================

#[test]
fn push_makes_the_snapshot_current() {
    let mut history = EditorHistory::new();
    history.push("one");

    assert_eq!(history.len(), 1);
    assert_eq!(history.current(), Some("one"));
    assert!(!history.can_undo(), "a single state has nothing to undo");
    assert!(!history.can_redo());
}

#[test]
fn push_then_undo_returns_the_previous_state() {
    let mut history = EditorHistory::new();
    history.push("one");
    history.push("two");

    assert!(history.can_undo());
    assert_eq!(history.undo(), Some("one"));
    assert_eq!(history.current(), Some("one"));
    assert!(history.can_redo());
}

#[test]
fn undo_and_redo_round_trip_across_three_states() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");
    history.push("c");

    assert_eq!(history.undo(), Some("b"));
    assert_eq!(history.undo(), Some("a"));
    assert_eq!(
        history.undo(),
        None,
        "the oldest state cannot be undone further"
    );
    assert_eq!(history.redo(), Some("b"));
    assert_eq!(history.redo(), Some("c"));
    assert_eq!(history.redo(), None, "the newest state cannot be redone");
}

#[test]
fn push_after_undo_truncates_the_redo_tail() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");
    history.push("c");

    assert_eq!(history.undo(), Some("b"));
    history.push("d");

    assert_eq!(history.current(), Some("d"));
    assert_eq!(history.len(), 3, "the 'c' redo entry was discarded");
    assert!(!history.can_redo());
    assert_eq!(history.undo(), Some("b"));
    assert_eq!(history.undo(), Some("a"));
}

#[test]
fn identical_consecutive_pushes_are_coalesced() {
    let mut history = EditorHistory::new();
    history.push("same");
    history.push("same");
    history.push("same");

    assert_eq!(history.len(), 1);
    assert_eq!(history.current(), Some("same"));
    assert!(!history.can_undo());
}

#[test]
fn coalescing_preserves_the_redo_tail() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");

    assert_eq!(history.undo(), Some("a"));
    history.push("a");

    assert_eq!(
        history.len(),
        2,
        "a no-op push must not destroy redo capability"
    );
    assert_eq!(history.redo(), Some("b"));
}

#[test]
fn coalescing_only_compares_against_the_current_state() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");
    history.push("a");

    assert_eq!(
        history.len(),
        3,
        "returning to an older text is still a new edit"
    );
    assert_eq!(history.undo(), Some("b"));
}

#[test]
fn clear_forgets_everything() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");
    history.clear();

    assert!(history.is_empty());
    assert_eq!(history.current(), None);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn history_is_clonable_independently() {
    let mut history = EditorHistory::new();
    history.push("a");
    history.push("b");
    let copy = history.clone();

    assert_eq!(history.undo(), Some("a"));
    assert_eq!(
        copy.current(),
        Some("b"),
        "the clone is not moved by the original"
    );
}

// =========================================================================
// EditorHistory — the >= 100-entry cap
// =========================================================================

#[test]
fn pushing_150_snapshots_stays_bounded_and_keeps_100_undo_steps() {
    let mut history = EditorHistory::new();
    for index in 0..150 {
        history.push(format!("s{index}"));
    }

    assert_eq!(history.len(), UNDO_CAPACITY, "bounded by the cap");
    assert!(history.len() >= 100, "and still >= 100 entries held");
    assert_eq!(history.current(), Some("s149"));

    let mut steps = 0;
    while history.can_undo() {
        assert!(history.undo().is_some());
        steps += 1;
    }

    assert_eq!(
        steps, MIN_UNDO_STEPS,
        "exactly the UI-042 minimum is usable"
    );
    assert_eq!(history.current(), Some("s49"), "the oldest retained state");
    assert!(!history.can_undo());
    assert!(history.can_redo());
}

#[test]
fn cap_evicts_the_oldest_snapshot_first() {
    let mut history = EditorHistory::with_capacity(3);
    history.push("a");
    history.push("b");
    history.push("c");
    history.push("d");

    assert_eq!(history.len(), 3);
    assert_eq!(history.current(), Some("d"));
    assert_eq!(history.undo(), Some("c"));
    assert_eq!(history.undo(), Some("b"));
    assert_eq!(history.undo(), None, "'a' was evicted");
}

#[test]
fn cap_holds_after_a_full_undo_then_new_edits() {
    let mut history = EditorHistory::with_capacity(3);
    for index in 0..10 {
        history.push(format!("s{index}"));
    }
    assert_eq!(history.len(), 3);

    while history.can_undo() {
        let _ = history.undo();
    }
    assert_eq!(history.current(), Some("s7"), "the oldest retained state");

    history.push("fresh");

    // The cap is a ceiling, not a floor: dropping the redo tail shortens the
    // history, it does not refill it.
    assert_eq!(history.len(), 2);
    assert!(history.len() <= 3);
    assert_eq!(history.current(), Some("fresh"));
    assert!(!history.can_redo(), "the redo tail was dropped by the push");
    assert_eq!(history.undo(), Some("s7"));
}

// =========================================================================
// Action classification
// =========================================================================

#[test]
fn only_edit_actions_are_content_changes() {
    let edits = [
        Edit::Insert('x'),
        Edit::Paste(Arc::new("pasted".into())),
        Edit::Enter,
        Edit::Indent,
        Edit::Unindent,
        Edit::Backspace,
        Edit::Delete,
    ];

    let edit_count = edits.len();
    for edit in &edits {
        assert_eq!(
            classify_action(&Action::Edit(edit.clone())),
            ActionKind::ContentChange,
            "{edit:?} must classify as a content change"
        );
    }

    let all = every_action();
    let non_edits: Vec<Action> = all
        .iter()
        .filter(|action| !action.is_edit())
        .cloned()
        .collect();

    assert_eq!(non_edits.len(), all.len() - edit_count);
    for action in non_edits {
        assert_eq!(
            classify_action(&action),
            ActionKind::CaretOnly,
            "{action:?} must not classify as a content change"
        );
    }
}

#[test]
fn classification_agrees_with_iced_is_edit_for_every_variant() {
    for action in every_action() {
        assert_eq!(
            classify_action(&action) == ActionKind::ContentChange,
            action.is_edit(),
            "classify_action must agree with Action::is_edit() for {action:?}"
        );
    }
}

#[test]
fn caret_selection_and_viewport_actions_are_caret_only() {
    let all = every_action();
    let caret_only: Vec<Action> = all
        .iter()
        .filter(|action| classify_action(action) == ActionKind::CaretOnly)
        .cloned()
        .collect();

    assert_eq!(caret_only.len(), all.len() - 7);
    for action in caret_only {
        assert!(
            matches!(
                action,
                Action::Move(_)
                    | Action::Select(_)
                    | Action::SelectWord
                    | Action::SelectLine
                    | Action::SelectAll
                    | Action::Click(_)
                    | Action::Drag(_)
                    | Action::Scroll { .. }
            ),
            "unexpected caret-only action {action:?}"
        );
    }
}

// =========================================================================
// Status-bar geometry
// =========================================================================

#[test]
fn display_line_column_is_one_based() {
    let cursor = EditorCursor {
        position: Position { line: 0, column: 0 },
        selection: None,
    };
    assert_eq!(display_line_column(cursor), (1, 1));

    let cursor = EditorCursor {
        position: Position { line: 4, column: 7 },
        selection: Some(Position { line: 0, column: 0 }),
    };
    assert_eq!(display_line_column(cursor), (5, 8));
}

// =========================================================================
// EditorViewModel — open / front matter (UI-037, UI-044)
// =========================================================================

#[test]
fn open_shows_only_the_body_and_caches_the_front_matter() {
    let vm = open_vm(RAW_WITH_FRONT_MATTER);

    assert_eq!(vm.text(), BODY_WITH_FRONT_MATTER);
    assert!(
        !vm.text().contains("title: Test"),
        "front matter must not reach the text area (UI-037)"
    );
    assert_eq!(
        vm.front_matter(),
        Some("---\ntitle: Test\ntags: [a, b]\n---")
    );
}

#[test]
fn open_without_front_matter_keeps_the_whole_file_as_body() {
    let vm = open_vm(RAW_NO_FRONT_MATTER);

    assert_eq!(vm.text(), RAW_NO_FRONT_MATTER);
    assert_eq!(vm.front_matter(), None);
}

#[test]
fn document_rejoins_body_with_the_original_front_matter() {
    let mut vm = open_vm(RAW_WITH_FRONT_MATTER);
    let _ = vm.apply_action(Action::Edit(Edit::Insert('#')));

    let rejoined = vm.document().to_string();

    assert!(rejoined.starts_with("---\ntitle: Test\ntags: [a, b]\n---"));
    assert!(rejoined.ends_with(&vm.text()));
    assert_eq!(
        DocumentContent::parse(&rejoined).front_matter.as_deref(),
        Some("---\ntitle: Test\ntags: [a, b]\n---"),
        "the front matter survives the round-trip unchanged (UI-044)"
    );
}

#[test]
fn document_without_front_matter_is_just_the_body() {
    let vm = open_vm(RAW_NO_FRONT_MATTER);

    assert_eq!(vm.document().to_string(), vm.text());
}

#[test]
fn open_seeds_history_so_the_first_edit_is_undoable() {
    let mut vm = open_vm(RAW_NO_FRONT_MATTER);
    assert!(!vm.can_undo(), "nothing has been edited yet");

    let _ = vm.apply_action(Action::Edit(Edit::Insert('x')));

    assert!(vm.can_undo());
    assert!(vm.undo());
    assert_eq!(vm.text(), RAW_NO_FRONT_MATTER, "back to the opened state");
}

#[test]
fn open_reports_the_opened_file_path() {
    let vm = EditorViewModel::open("C:/tmp/note.md", "# hi");

    assert_eq!(vm.path(), std::path::Path::new("C:/tmp/note.md"));
}

// =========================================================================
// EditorViewModel — editing, history, validation
// =========================================================================

#[test]
fn a_content_edit_updates_the_text_and_pushes_history() {
    let mut vm = open_vm("abc");
    let before = vm.history().len();

    let kind = vm.apply_action(Action::Edit(Edit::Insert('z')));

    assert_eq!(kind, ActionKind::ContentChange);
    assert_eq!(vm.text(), "zabc");
    assert_eq!(vm.history().len(), before + 1);
}

#[test]
fn a_caret_action_never_touches_history() {
    let mut vm = open_vm("one\ntwo\nthree");
    let before = vm.history().len();

    let caret_actions = [
        Action::Move(Motion::Down),
        Action::Select(Motion::Right),
        Action::SelectAll,
        Action::SelectWord,
        Action::SelectLine,
        Action::Click(iced::Point::new(4.0, 4.0)),
        Action::Drag(iced::Point::new(9.0, 4.0)),
        Action::Scroll { lines: 2 },
    ];
    let caret_count = caret_actions.len();

    for action in caret_actions {
        let label = format!("{action:?}");
        assert_eq!(vm.apply_action(action), ActionKind::CaretOnly);
        assert_eq!(vm.history().len(), before, "{label} must not push");
    }
    assert_eq!(caret_count, 8);
}

#[test]
fn undo_restores_the_previous_text() {
    let mut vm = open_vm("start");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('a')));
    let _ = vm.apply_action(Action::Edit(Edit::Insert('b')));
    assert_eq!(vm.text(), "abstart");

    assert!(vm.undo());
    assert_eq!(vm.text(), "astart");
    assert!(vm.undo());
    assert_eq!(vm.text(), "start");
    assert!(!vm.undo(), "the seeded baseline is the oldest state");
}

#[test]
fn redo_reapplies_an_undone_edit() {
    let mut vm = open_vm("start");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('a')));
    assert!(vm.undo());
    assert!(!vm.is_dirty());

    assert!(vm.redo());
    assert_eq!(vm.text(), "astart");
    assert!(vm.is_dirty());
    assert!(!vm.redo());
}

#[test]
fn dirty_flag_tracks_the_opened_state() {
    let mut vm = open_vm(RAW_NO_FRONT_MATTER);
    assert!(!vm.is_dirty());

    let _ = vm.apply_action(Action::Edit(Edit::Insert('x')));
    assert!(vm.is_dirty());

    assert!(vm.undo());
    assert!(
        !vm.is_dirty(),
        "undoing back to the opened text clears dirty"
    );
}

#[test]
fn paste_is_recorded_as_one_history_entry() {
    let mut vm = open_vm("body");
    let before = vm.history().len();

    let _ = vm.apply_action(Action::Edit(Edit::Paste(Arc::new("pasted\n".into()))));

    assert_eq!(vm.text(), "pasted\nbody");
    assert_eq!(vm.history().len(), before + 1);
}

#[test]
fn validation_blocks_save_on_an_unterminated_fence() {
    let mut vm = open_vm("# Title\n\nok\n");
    assert!(vm.save_enabled());

    let _ = vm.apply_action(Action::Edit(Edit::Paste(Arc::new(
        "```\nforgotten\n".into(),
    ))));

    assert!(!vm.save_enabled(), "UI-043 aborts the save");
    assert!(vm.can_undo(), "the bad edit is still undoable");

    assert!(vm.undo());
    assert!(
        vm.save_enabled(),
        "undoing the bad edit re-enables the save"
    );
}

#[test]
fn validation_error_carries_a_location() {
    let vm = open_vm(UNTERMINATED_FENCE_BODY);

    let error = vm.validation_error().expect("the fence is unterminated");
    assert_eq!(error.kind, MarkdownErrorKind::UnterminatedCodeFence);
    assert_eq!(error.line, 3);
    assert_eq!(error.column, 1);
}

#[test]
fn validation_message_is_built_from_the_strings_module() {
    let vm = open_vm(UNTERMINATED_FENCE_BODY);

    let message = vm.validation_message().expect("error present");
    let expected =
        strings::build_editor_validation_error(strings::EDITOR_ERR_UNTERMINATED_FENCE, 3, 1);

    assert_eq!(message, expected);
    assert!(message.contains("line 3"));
}

#[test]
fn valid_body_has_no_validation_message() {
    let vm = open_vm(valid_body());

    assert_eq!(vm.validation_error(), None);
    assert_eq!(vm.validation_message(), None);
    assert!(vm.save_enabled());
}

/// A body exercising the GFM extensions UI-043 names.
fn valid_body() -> &'static str {
    "# Title\n\n~~struck~~ and | a |\n| - |\n| 1 |\n\n- [x] task\n"
}

#[test]
fn status_text_reports_the_current_line_and_column() {
    let mut vm = open_vm("one\ntwo\nthree");
    assert_eq!(vm.line(), 1);
    assert_eq!(vm.column(), 1);
    assert_eq!(vm.status_text(), strings::build_editor_status_text(1, 1));

    let _ = vm.apply_action(Action::Move(Motion::Down));
    let _ = vm.apply_action(Action::Move(Motion::End));

    assert_eq!(vm.line(), 2);
    assert_eq!(vm.column(), 4);
    assert_eq!(vm.status_text(), "Line: 2 | Col: 4");
}

#[test]
fn undo_resets_the_caret_to_the_document_start() {
    // Pins the documented limitation: snapshots carry text only, so the
    // caret cannot be restored — it lands back at the document start.
    let mut vm = open_vm("one\ntwo\nthree");
    let _ = vm.apply_action(Action::Move(Motion::Down));
    assert_eq!(vm.line(), 2);

    let _ = vm.apply_action(Action::Edit(Edit::Insert('x')));
    assert!(vm.undo());

    assert_eq!((vm.line(), vm.column()), (1, 1));
}

#[test]
fn multibyte_body_survives_open_and_edit() {
    let mut vm = open_vm("héllo wörld\n");

    assert_eq!(vm.text(), "héllo wörld\n");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('é')));
    assert_eq!(vm.text(), "éhéllo wörld\n");
    assert!(vm.undo());
    assert_eq!(vm.text(), "héllo wörld\n");
}

// =========================================================================
// apply_editor_message — the pure reducer entry point
// =========================================================================

#[test]
fn reducer_applies_action_messages() {
    let mut vm = open_vm("body");

    let changed = apply_editor_message(
        &mut vm,
        &EditorMessage::Action(Action::Edit(Edit::Insert('q'))),
    );

    assert!(changed);
    assert_eq!(vm.text(), "qbody");
}

#[test]
fn reducer_undoes_and_redoes_for_the_matching_path() {
    let mut vm = EditorViewModel::open("/tmp/a.md", "body");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('z')));

    assert!(apply_editor_message(
        &mut vm,
        &EditorMessage::Undo {
            path: PathBuf::from("/tmp/a.md")
        }
    ));
    assert_eq!(vm.text(), "body");

    assert!(apply_editor_message(
        &mut vm,
        &EditorMessage::Redo {
            path: PathBuf::from("/tmp/a.md")
        }
    ));
    assert_eq!(vm.text(), "zbody");
}

#[test]
fn reducer_ignores_undo_and_redo_for_another_tab() {
    let mut vm = EditorViewModel::open("/tmp/a.md", "body");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('z')));

    assert!(!apply_editor_message(
        &mut vm,
        &EditorMessage::Undo {
            path: PathBuf::from("/tmp/other.md")
        }
    ));
    assert_eq!(
        vm.text(),
        "zbody",
        "another tab's undo must not touch this buffer"
    );

    assert!(!apply_editor_message(
        &mut vm,
        &EditorMessage::Redo {
            path: PathBuf::from("/tmp/other.md")
        }
    ));
    assert_eq!(vm.text(), "zbody");
}

#[test]
fn reducer_never_mutates_the_buffer_for_save_and_cancel() {
    let mut vm = open_vm("body");
    let _ = vm.apply_action(Action::Edit(Edit::Insert('z')));
    let before = vm.text();
    let path = vm.path().to_path_buf();

    assert!(
        !apply_editor_message(&mut vm, &EditorMessage::SaveRequested { path }),
        "the write is the reducer's job, not the buffer's (UI-044)"
    );
    assert_eq!(vm.text(), before);

    assert!(
        !apply_editor_message(&mut vm, &EditorMessage::CancelRequested),
        "cancel is an overlay transition, not a buffer mutation (UI-045)"
    );
    assert_eq!(vm.text(), before);
}

#[test]
fn reducer_reports_no_change_when_history_is_exhausted() {
    let mut vm = open_vm("body");

    assert!(
        !apply_editor_message(
            &mut vm,
            &EditorMessage::Undo {
                path: PathBuf::from("C:/tmp/note.md")
            }
        ),
        "nothing to undo at the baseline"
    );
    assert!(
        !apply_editor_message(
            &mut vm,
            &EditorMessage::Redo {
                path: PathBuf::from("C:/tmp/note.md")
            }
        ),
        "nothing to redo at the newest state"
    );
}

// =========================================================================
// View construction
// =========================================================================

#[test]
fn editor_card_builds_for_a_valid_buffer() {
    let vm = open_vm("# Title\n\nbody\n");

    let element: Element<'_, Message> = editor_card(&vm, TEST_VIEWPORT);

    let _ = element;
}

#[test]
fn editor_card_builds_for_an_invalid_buffer() {
    let vm = open_vm(UNTERMINATED_FENCE_BODY);

    let element: Element<'_, Message> = editor_card(&vm, TEST_VIEWPORT);

    assert!(!vm.save_enabled());
    let _ = element;
}

#[test]
fn editor_modal_builds_over_a_scrim() {
    let vm = open_vm("# Title\n\nbody\n");

    let element: Element<'_, Message> = editor_modal(&vm, TEST_VIEWPORT);

    let _ = element;
}

#[test]
fn editor_card_builds_for_an_empty_buffer() {
    let vm = open_vm("");

    assert_eq!(vm.text(), "");
    assert!(vm.save_enabled(), "an empty body is valid markdown");

    let element: Element<'_, Message> = editor_card(&vm, TEST_VIEWPORT);
    let _ = element;
}

#[test]
fn editor_footer_puts_status_left_of_right_aligned_cancel_then_save_actions() {
    use iced_test::selector::Candidate;

    type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

    fn text_bounds(ui: &mut Sim<'_>, label: &str) -> iced::Rectangle {
        ui.find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text {
                content, bounds, ..
            } if content == label => Some(bounds),
            _ => None,
        })
        .unwrap_or_else(|error| panic!("footer text {label:?} should be visible: {error:?}"))
    }

    let vm = open_vm("# Title\n\nbody\n");
    let width = 800.0;
    let mut ui = Sim::with_size(
        iced_test::core::Settings::default(),
        Size::new(width, 80.0),
        editor_footer(&vm),
    );

    let status = text_bounds(&mut ui, &vm.status_text());
    let cancel = text_bounds(&mut ui, strings::CANCEL_BUTTON);
    let save = text_bounds(&mut ui, strings::SAVE_BUTTON);

    assert!(status.x < cancel.x, "status should be left of the actions");
    assert!(
        cancel.x < save.x,
        "Cancel should precede the primary Save action"
    );
    assert!(
        save.x > width * 0.75,
        "the action group should be pinned to the right edge, got Save at x={}",
        save.x
    );
}

#[test]
fn validation_reason_covers_every_error_kind() {
    assert_eq!(
        validation_reason(MarkdownErrorKind::UnterminatedCodeFence),
        strings::EDITOR_ERR_UNTERMINATED_FENCE
    );
}

// =========================================================================
// UIF-MOD-002 - the guard rule, as a truth table
// =========================================================================

/// Every gesture x dirty-state combination, pinned in one place.
///
/// The guard is a pure function precisely so this table can exist: no
/// widget tree, no runtime, no file. If a future change makes `Escape`
/// discard a dirty buffer, this row fails rather than a user finding out.
#[test]
fn guard_leave_truth_table() {
    use LeaveDecision::{AskBeforeDiscarding, Proceed};

    let rows: Vec<(bool, LeaveGesture, LeaveDecision, &str)> = vec![
        (
            false,
            LeaveGesture::Escape,
            Proceed,
            "clean + Escape closes: nothing is at stake",
        ),
        (
            false,
            LeaveGesture::Scrim,
            Proceed,
            "clean + scrim closes: nothing is at stake",
        ),
        (
            false,
            LeaveGesture::Cancel,
            Proceed,
            "clean + Cancel closes: nothing is at stake",
        ),
        (
            true,
            LeaveGesture::Escape,
            AskBeforeDiscarding,
            "dirty + Escape must NEVER discard silently",
        ),
        (
            true,
            LeaveGesture::Scrim,
            AskBeforeDiscarding,
            "dirty + scrim must NEVER discard silently",
        ),
        (
            true,
            LeaveGesture::Cancel,
            Proceed,
            "dirty + Cancel closes: that button IS the explicit discard",
        ),
    ];

    for (dirty, gesture, expected, why) in rows {
        assert_eq!(
            guard_leave(dirty, gesture),
            expected,
            "dirty={dirty} {:?}: {why}",
            gesture
        );
    }
}

/// The app's gesture vocabulary maps onto the framework's `DismissCause`
/// without inventing a third word for the same thing.
#[test]
fn leave_gesture_maps_to_the_framework_dismiss_cause() {
    use iced_pro::surface::modal::DismissCause;

    assert_eq!(LeaveGesture::Escape.dismiss_cause(), DismissCause::Escape);
    assert_eq!(LeaveGesture::Scrim.dismiss_cause(), DismissCause::Scrim);
    // `Cancel` is "the caller decided", which is the cause the framework's
    // ModalHost already exempts from UIF-MOD-002.
    assert_eq!(
        LeaveGesture::Cancel.dismiss_cause(),
        DismissCause::Programmatic
    );
}

/// The guard never mutates anything: same inputs, same answer, forever.
#[test]
fn guard_leave_is_stable_across_repeated_calls() {
    for _ in 0..3 {
        assert_eq!(
            guard_leave(true, LeaveGesture::Escape),
            LeaveDecision::AskBeforeDiscarding
        );
        assert_eq!(
            guard_leave(false, LeaveGesture::Escape),
            LeaveDecision::Proceed
        );
    }
}

// =========================================================================
// 4.6.2 - resolved card size, per viewport
// =========================================================================

/// The card is viewport-responsive, and the clamps are evidenced rather
/// than asserted at one lucky window size.
///
/// Resolution order is the framework's: want 80% of the viewport, floor
/// at the policy minimum, cap at the policy maximum, then let the shared
/// `VIEWPORT_GUTTER` (`space::XXL` = 32px) win over the floor when the
/// two disagree.
#[test]
fn editor_size_resolves_per_viewport() {
    let rows: Vec<(f32, f32, f32, f32, &str)> = vec![
        // viewport w, h,   card w,   card h, which arm produced it
        (800.0, 600.0, 640.0, 480.0, "80% of both axes"),
        (1024.0, 768.0, 819.2, 614.4, "80% of both axes"),
        (
            1280.0,
            800.0,
            960.0,
            640.0,
            "width capped at 960, height 80%",
        ),
        (1920.0, 1080.0, 960.0, 760.0, "both axes capped"),
        (3840.0, 2160.0, 960.0, 760.0, "both axes capped"),
        (600.0, 420.0, 520.0, 380.0, "both axes floored"),
        (500.0, 400.0, 468.0, 368.0, "gutter beats the floor"),
        (
            320.0,
            240.0,
            288.0,
            208.0,
            "gutter only; the floor cannot fit",
        ),
    ];

    for (vw, vh, want_w, want_h, arm) in rows {
        let size = editor_size(Size::new(vw, vh));
        assert_eq!(
            size,
            Size::new(want_w, want_h),
            "viewport {vw}x{vh} should resolve {want_w}x{want_h} ({arm})"
        );
    }
}

/// Whatever the viewport, the card never eats the gutter and never goes
/// negative - the two properties the app relies on when it hands the
/// resolved length straight to `Length::Fixed`.
#[test]
fn editor_size_always_leaves_the_gutter_and_stays_non_negative() {
    for w in [0.0, 1.0, 64.0, 300.0, 1024.0, 4096.0] {
        for h in [0.0, 1.0, 64.0, 300.0, 768.0, 4096.0] {
            let size = editor_size(Size::new(w, h));
            assert!(
                size.width <= (w - space::XXL).max(0.0) + f32::EPSILON,
                "width {w} must leave the {}px gutter, got {}",
                space::XXL,
                size.width
            );
            assert!(
                size.height <= (h - space::XXL).max(0.0) + f32::EPSILON,
                "height {h} must leave the {}px gutter, got {}",
                space::XXL,
                size.height
            );
            assert!(size.width >= 0.0 && size.height >= 0.0);
        }
    }
}

/// The card never exceeds the policy's own ceiling on a huge screen, and
/// never drops below it while the gutter can still fit it.
#[test]
fn editor_size_respects_the_policy_bounds() {
    let size = editor_size(Size::new(4096.0, 4096.0));
    assert_eq!(size.width, EDITOR_WIDTH_MAX);
    assert_eq!(size.height, EDITOR_HEIGHT_MAX);

    // A viewport big enough that the floors are reachable without the
    // gutter interfering.
    let size = editor_size(Size::new(900.0, 520.0));
    assert!(
        size.width >= EDITOR_WIDTH_MIN || size.width >= 900.0 - space::XXL,
        "width {} should reach the floor or the gutter",
        size.width
    );
    assert!(size.height >= EDITOR_HEIGHT_MIN);
}

// =========================================================================
// UIF-MOD-003 - the structured error slot
// =========================================================================

/// A clean, valid buffer has nothing to show.
#[test]
fn a_clean_valid_buffer_has_no_modal_error() {
    let vm = open_vm("# Title\n\nbody\n");
    assert_eq!(vm.modal_error(), None);
}

/// A validation failure reaches the banner as a *structured* error: a
/// grouping code, the offending line and column, and the control that
/// must be fixed - not just a string.
#[test]
fn a_validation_failure_carries_its_location_and_a_focus_recovery() {
    let vm = open_vm(UNTERMINATED_FENCE_BODY);
    let error = vm
        .modal_error()
        .expect("an invalid buffer must show an error");

    assert_eq!(error.code(), EDITOR_VALIDATION_ERROR_CODE);
    assert_eq!(
        error.location(),
        Some(&ErrorLocation::Position { line: 3, column: 1 }),
        "the parser's line/column must reach the banner, not be flattened away"
    );
    assert_eq!(
        error.recovery(),
        &Recovery::FocusControl(iced_pro::focus::FocusId::new(EDITOR_TEXT_ID)),
        "the recovery must name the buffer the user has to fix"
    );
    assert!(
        error.message().contains("line 3"),
        "the message must say where, got {:?}",
        error.message()
    );
}

/// A save that was refused outranks a validation failure: the user has
/// already tried to commit and been turned down, which is the more recent
/// fact about what went wrong.
#[test]
fn a_save_failure_outranks_a_validation_failure() {
    let mut vm = open_vm(UNTERMINATED_FENCE_BODY);
    assert_eq!(
        vm.modal_error().map(|e| e.code().to_string()),
        Some(EDITOR_VALIDATION_ERROR_CODE.to_string())
    );

    vm.set_save_error(save_queue_error());
    assert_eq!(
        vm.modal_error().map(|e| e.code().to_string()),
        Some(EDITOR_SAVE_ERROR_CODE.to_string()),
        "the refused save must be what the banner shows"
    );
    assert_eq!(
        vm.modal_error().map(|e| e.recovery().clone()),
        Some(Recovery::Retry),
        "and it must offer a retry, not a focus jump"
    );
}

/// The banner changes by exactly two operations, mirroring
/// `ModalHost::set_error` / `clear_error`.
#[test]
fn save_error_set_clear_and_note_round_trip() {
    let mut vm = open_vm("body");
    assert!(vm.save_error().is_none());

    // `note_save_failed` reports whether the banner *changed*.
    assert!(
        vm.note_save_failed(),
        "showing a banner where none was is a change"
    );
    assert!(
        !vm.note_save_failed(),
        "showing the same banner twice is not"
    );

    let removed = vm.clear_save_error();
    assert!(removed.is_some(), "clearing must return what it removed");
    assert_eq!(
        removed.map(|e| e.code().to_string()),
        Some(EDITOR_SAVE_ERROR_CODE.to_string())
    );
    assert!(vm.save_error().is_none());
    assert!(
        vm.clear_save_error().is_none(),
        "clearing an empty slot returns nothing"
    );
}

/// `set_save_error` returns what it superseded, so a caller can tell a
/// fresh failure from a replacement.
#[test]
fn set_save_error_returns_the_superseded_failure() {
    let mut vm = open_vm("body");
    assert_eq!(vm.set_save_error(save_queue_error()), None);
    assert_eq!(
        vm.set_save_error(save_queue_error())
            .map(|e| e.code().to_string()),
        Some(EDITOR_SAVE_ERROR_CODE.to_string())
    );
}

/// Each recovery affordance gets a label that says what pressing it does.
#[test]
fn recovery_labels_name_what_the_button_does() {
    let retry = ModalError::new("X", "x").with_recovery(Recovery::Retry);
    assert_eq!(recovery_label(&retry), strings::EDITOR_RETRY_LABEL);

    let focus_line = ModalError::new("X", "x")
        .at(ErrorLocation::Position { line: 7, column: 2 })
        .with_recovery(Recovery::FocusControl(iced_pro::focus::FocusId::new(
            EDITOR_TEXT_ID,
        )));
    assert_eq!(recovery_label(&focus_line), "Fix line 7");

    let focus_region = ModalError::new("X", "x")
        .at(ErrorLocation::Region("front matter".to_string()))
        .with_recovery(Recovery::FocusControl(iced_pro::focus::FocusId::new(
            EDITOR_TEXT_ID,
        )));
    assert_eq!(recovery_label(&focus_region), strings::EDITOR_FIX_LABEL);

    let ack = ModalError::new("X", "x").with_recovery(Recovery::Acknowledge);
    assert_eq!(recovery_label(&ack), strings::CLOSE_BUTTON);
}

/// The save-refusal error is the shape the banner and the retry both rely on.
#[test]
fn save_queue_error_is_structured_and_retryable() {
    let error = save_queue_error();
    assert_eq!(error.code(), EDITOR_SAVE_ERROR_CODE);
    assert_eq!(error.message(), strings::EDITOR_SAVE_QUEUE_ERROR);
    assert_eq!(error.recovery(), &Recovery::Retry);
    assert_eq!(error.location(), None);
}

/// The new editor messages never touch the buffer - they are dismiss and
/// recovery traffic, not edits.
#[test]
fn dismiss_and_recovery_messages_never_mutate_the_buffer() {
    let mut vm = open_vm("body");
    let before = vm.text();

    let messages = vec![
        EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        },
        EditorMessage::Absorbed,
        EditorMessage::RecoveryRequested {
            path: PathBuf::from("C:/tmp/note.md"),
            recovery: Recovery::Retry,
        },
        EditorMessage::CancelRequested,
    ];

    for message in messages {
        assert!(
            !apply_editor_message(&mut vm, &message),
            "{message:?} must report that it changed nothing"
        );
        assert_eq!(vm.text(), before, "{message:?} must not touch the text");
    }
}

// =========================================================================
// UI-046 - the inverted editor palette's contrast bounds
// =========================================================================

/// The two colours the framework does NOT carry are still held to the
/// framework's own contrast rules, measured from raw sRGB rather than
/// eyeballed.
///
/// `iced_pro::token::color::EditorPalette` models `bg` / `fg` / `err`
/// only (`UIF-TOK-005`), so the placeholder and selection colours the
/// app had to add are pinned here against
/// [`iced_pro::token::contrast`] - the same bar that moved the error
/// colour off pure red.
#[test]
fn inverted_editor_palette_meets_its_contrast_bounds() {
    use iced_pro::token::contrast::{meets_chrome_contrast, meets_text_contrast};

    let colors = EditorColors::inverted();

    assert!(
        meets_text_contrast(colors.text, colors.background),
        "black buffer text on the white canvas must clear 4.5:1"
    );
    assert!(
        meets_text_contrast(colors.error, colors.background),
        "the error red must clear 4.5:1 on the white canvas"
    );
    assert!(
        meets_text_contrast(colors.placeholder, colors.background),
        "the placeholder must clear 4.5:1 on the white canvas"
    );
    // The selection is a fill, so it is held to the 3.0 chrome bar...
    assert!(
        meets_chrome_contrast(colors.selection, colors.background),
        "the selection highlight must be visible against the canvas (>= 3.0)"
    );
    // ...and to the text bar for the black buffer text sitting ON it, so a
    // selection never turns the text unreadable.
    assert!(
        meets_text_contrast(colors.text, colors.selection),
        "black buffer text on the selection must still clear 4.5:1"
    );
}

/// The app's inverted editor palette is the framework token, not a
/// parallel set of numbers that can drift.
#[test]
fn inverted_editor_palette_delegates_to_the_framework_token() {
    use iced_pro::token::color::EditorPalette;

    let app = EditorColors::inverted();
    let pro = EditorPalette::inverted();

    assert_eq!(app.background, pro.background);
    assert_eq!(app.text, pro.text);
    assert_eq!(app.error, pro.error);
}
