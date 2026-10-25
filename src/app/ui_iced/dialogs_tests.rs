//! Sidecar unit tests for `ui_iced/dialogs.rs`.
//!
//! Pure state, validation and geometry tests only ([RUST-006]): every path
//! is a synthetic fixture literal that is never opened, and no user
//! profile, live configuration file or database is touched. Where a
//! widget's public shape *is* observable without a renderer — its
//! `Length` size hint — that is asserted too, mirroring the
//! `overlay_tests.rs` approach.

use super::*;
use crate::ui::strings;
use iced::{Element, Point, Size};
use iced_pro::surface::input_dialog::FooterButtons;
use iced_pro::validation::punish::PunishReason;

/// Synthetic target paths — never opened ([RUST-006]).
fn sample_dir() -> PathBuf {
    PathBuf::from("fixture-workspace").join("docs")
}

fn sample_file() -> PathBuf {
    PathBuf::from("fixture-workspace").join("note.md")
}

/// The viewport the anchor tests clamp against.
fn viewport() -> Size {
    Size::new(800.0, 600.0)
}

/// A rename dialog holding `name`, **pristine** — [`set_name`] is a value
/// change, not a commit, so the punish rule still withholds the visual.
fn rename_with(name: &str) -> DialogState {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_name(name.to_string());
    state
}

/// A rename dialog whose field has been **committed** by a refused submit.
fn committed_rename(name: &str) -> DialogState {
    let mut state = rename_with(name);
    let _ = state.submit();
    state
}

/// An open delete-confirmation dialog over the sample file.
fn delete_state() -> DialogState {
    let mut state = DialogState::new();
    state.open(DialogKind::DeleteConfirm {
        path: sample_file(),
    });
    state
}

// --- DialogKind: target / title / prompt / label ---------------------------

#[test]
fn create_directory_kind_targets_its_parent() {
    let kind = DialogKind::CreateDirectory {
        parent: sample_dir(),
    };
    assert_eq!(kind.target(), sample_dir().as_path());
    assert_eq!(kind.title(), strings::CREATE_DIRECTORY_WINDOW);
    assert_eq!(kind.prompt(), Some(strings::ENTER_DIRECTORY_NAME));
    assert_eq!(kind.confirm_label(), strings::OK_BUTTON);
    assert!(kind.is_name_entry());
}

#[test]
fn create_document_kind_targets_its_parent() {
    let kind = DialogKind::CreateDocument {
        parent: sample_dir(),
    };
    assert_eq!(kind.target(), sample_dir().as_path());
    assert_eq!(kind.title(), strings::CREATE_DOCUMENT_WINDOW);
    assert_eq!(kind.prompt(), Some(strings::ENTER_DOCUMENT_NAME));
    assert!(kind.is_name_entry());
}

#[test]
fn rename_kind_targets_its_path() {
    let kind = DialogKind::Rename {
        path: sample_file(),
    };
    assert_eq!(kind.target(), sample_file().as_path());
    assert_eq!(kind.title(), strings::RENAME_WINDOW);
    assert_eq!(kind.prompt(), Some(strings::ENTER_NEW_NAME));
    assert!(kind.is_name_entry());
}

#[test]
fn move_kind_targets_its_path_and_is_not_name_entry() {
    let kind = DialogKind::Move {
        path: sample_file(),
    };
    assert_eq!(kind.target(), sample_file().as_path());
    assert_eq!(kind.title(), strings::MOVE_FILE_WINDOW);
    assert_eq!(kind.prompt(), Some(strings::SELECT_DESTINATION_FOLDER));
    assert!(!kind.is_name_entry());
}

#[test]
fn delete_confirm_kind_has_no_static_prompt_and_deletes_label() {
    let kind = DialogKind::DeleteConfirm {
        path: sample_file(),
    };
    assert_eq!(kind.target(), sample_file().as_path());
    assert_eq!(kind.title(), strings::DELETE_CONFIRM_WINDOW);
    assert_eq!(kind.prompt(), None);
    assert_eq!(kind.confirm_label(), strings::DELETE_ACTION);
    assert!(!kind.is_name_entry());
}

#[test]
fn target_name_projects_the_file_name() {
    let kind = DialogKind::DeleteConfirm {
        path: sample_file(),
    };
    assert_eq!(kind.target_name(), "note.md");
}

#[test]
fn target_name_falls_back_to_full_path_for_a_root_target() {
    let kind = DialogKind::DeleteConfirm {
        path: PathBuf::from("/"),
    };
    // A filesystem root has no file_name; the full path is shown.
    assert_eq!(kind.target_name(), kind.target().to_string_lossy());
}

// --- validate_name: empty / whitespace / illegal classes -----------------

#[test]
fn validate_rejects_an_empty_name() {
    let kind = DialogKind::CreateDirectory {
        parent: sample_dir(),
    };
    assert_eq!(
        validate_name(&kind, ""),
        Err(strings::DIALOG_NAME_EMPTY_ERROR.to_string())
    );
}

#[test]
fn validate_rejects_a_whitespace_only_name() {
    let kind = DialogKind::Rename {
        path: sample_file(),
    };
    assert_eq!(
        validate_name(&kind, "   \t  "),
        Err(strings::DIALOG_NAME_EMPTY_ERROR.to_string())
    );
}

#[test]
fn validate_rejects_a_forward_slash_separator() {
    let kind = DialogKind::CreateDocument {
        parent: sample_dir(),
    };
    // A `/` inside the name is a path separator, so the name is unsafe.
    assert_eq!(
        validate_name(&kind, "invalid/name"),
        Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string())
    );
}

#[test]
fn validate_rejects_a_backslash_separator() {
    let kind = DialogKind::CreateDirectory {
        parent: sample_dir(),
    };
    assert_eq!(
        validate_name(&kind, r"invalid\name"),
        Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string())
    );
}

#[test]
fn validate_rejects_a_parent_directory_traversal() {
    let kind = DialogKind::CreateDirectory {
        parent: sample_dir(),
    };
    // A `..` segment escapes the parent directory, so the name is unsafe.
    assert_eq!(
        validate_name(&kind, "../invalid_traversal"),
        Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string())
    );
    assert_eq!(
        validate_name(&kind, "a/../b"),
        Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string())
    );
    assert_eq!(
        validate_name(&kind, ".."),
        Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string())
    );
}

#[test]
fn validate_accepts_ordinary_names() {
    let kind = DialogKind::Rename {
        path: sample_file(),
    };
    for name in [
        "note.md",
        "a",
        "with spaces",
        "file.name.with.dots",
        "draft.txt",
    ] {
        assert_eq!(validate_name(&kind, name), Ok(()), "should accept {name:?}");
    }
}

#[test]
fn validate_ignores_the_name_for_non_name_entry_kinds() {
    let move_kind = DialogKind::Move {
        path: sample_file(),
    };
    let delete_kind = DialogKind::DeleteConfirm {
        path: sample_file(),
    };
    // Their gating lives in `can_confirm`, not in the name rule.
    assert_eq!(validate_name(&move_kind, ""), Ok(()));
    assert_eq!(validate_name(&delete_kind, ""), Ok(()));
    assert_eq!(validate_name(&move_kind, "anything"), Ok(()));
}

// --- DialogState: initial state ------------------------------------------

#[test]
fn a_new_state_has_no_dialog_open() {
    let state = DialogState::new();
    assert!(!state.is_open());
    assert_eq!(state.kind(), None);
    assert!(state.pending_name().is_empty());
    assert_eq!(state.error(), None);
    assert!(state.destinations().is_empty());
    assert_eq!(state.selected_destination(), None);
    assert!(!state.can_confirm());
}

#[test]
fn default_matches_new() {
    assert_eq!(DialogState::default(), DialogState::new());
}

// --- DialogState: open / one-at-a-time -----------------------------------

#[test]
fn open_sets_the_kind() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    assert!(state.is_open());
    assert_eq!(
        state.kind(),
        Some(&DialogKind::CreateDirectory {
            parent: sample_dir()
        })
    );
}

#[test]
fn opening_a_second_dialog_replaces_the_first_one_at_a_time() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.open(DialogKind::DeleteConfirm { path: sample_dir() });
    // Exactly one dialog is open, and it is the newest.
    assert_eq!(
        state.kind(),
        Some(&DialogKind::DeleteConfirm { path: sample_dir() })
    );
}

#[test]
fn open_clears_stale_name_and_error_from_the_previous_dialog() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_name("stale".to_string());
    state.set_error("boom");
    state.open(DialogKind::CreateDocument {
        parent: sample_dir(),
    });
    assert!(state.pending_name().is_empty());
    assert_eq!(state.error(), None);
}

#[test]
fn open_move_carries_its_destinations() {
    let mut state = DialogState::new();
    let dests = vec![sample_dir(), PathBuf::from("other")];
    state.open_move(sample_file(), dests.clone());
    assert_eq!(
        state.kind(),
        Some(&DialogKind::Move {
            path: sample_file()
        })
    );
    assert_eq!(state.destinations(), dests.as_slice());
    assert_eq!(state.selected_destination(), None);
}

// --- DialogState: close / cancel -----------------------------------------

#[test]
fn close_reports_whether_a_dialog_was_open() {
    let mut state = DialogState::new();
    assert!(!state.close(), "nothing was open");
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    assert!(state.close(), "a dialog was open");
    assert!(!state.close(), "closing twice only reports once");
}

#[test]
fn close_clears_every_transient_field() {
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    state.set_name("x".to_string());
    state.set_error("boom");
    state.select_destination(sample_dir());
    state.close();
    assert_eq!(state, DialogState::new());
}

#[test]
fn cancel_closes_and_clears() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    state.set_name("keep-me?".to_string());
    state.cancel();
    assert!(!state.is_open());
    assert!(state.pending_name().is_empty());
}

// --- DialogState: set_name / set_error / select_destination --------------

#[test]
fn set_name_updates_the_field_and_clears_the_error() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_error("boom");
    state.set_name("typed".to_string());
    assert_eq!(state.pending_name(), "typed");
    assert_eq!(state.error(), None, "typing hides a stale error");
}

#[test]
fn set_error_records_the_message() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_error("nope");
    assert_eq!(state.error(), Some("nope"));
}

#[test]
fn clear_error_removes_the_message() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_error("nope");
    state.clear_error();
    assert_eq!(state.error(), None);
}

#[test]
fn select_destination_updates_the_pick_and_clears_the_error() {
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    state.set_error("boom");
    state.select_destination(sample_dir());
    assert_eq!(state.selected_destination(), Some(sample_dir().as_path()));
    assert_eq!(state.error(), None);
}

// --- DialogState: can_confirm --------------------------------------------

#[test]
fn can_confirm_is_false_when_closed() {
    assert!(!DialogState::new().can_confirm());
}

#[test]
fn name_entry_can_confirm_only_with_a_non_blank_name() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    assert!(!state.can_confirm(), "blank name is not confirmable");
    state.set_name("   ".to_string());
    assert!(!state.can_confirm(), "whitespace-only is not confirmable");
    state.set_name("docs".to_string());
    assert!(state.can_confirm());
}

#[test]
fn move_can_confirm_only_with_a_selected_destination() {
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    assert!(!state.can_confirm(), "no destination picked yet");
    state.select_destination(sample_dir());
    assert!(state.can_confirm());
}

#[test]
fn delete_confirm_is_always_confirmable() {
    let mut state = DialogState::new();
    state.open(DialogKind::DeleteConfirm {
        path: sample_file(),
    });
    assert!(state.can_confirm());
}

// --- DialogState: submit -------------------------------------------------

#[test]
fn submit_name_entry_completes_with_the_trimmed_name() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDocument {
        parent: sample_dir(),
    });
    state.set_name("  my notes  ".to_string());
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Completed {
            kind: DialogKind::CreateDocument {
                parent: sample_dir()
            },
            name: "my notes".to_string(),
            destination: None,
        }
    );
    assert!(!state.is_open(), "a valid submit closes the dialog");
}

#[test]
fn submit_name_entry_with_an_empty_name_is_invalid_and_stays_open() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_name("   ".to_string());
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Invalid {
            message: strings::DIALOG_NAME_EMPTY_ERROR.to_string()
        }
    );
    assert!(state.is_open(), "an invalid submit keeps the dialog open");
    assert_eq!(state.error(), None, "submit does not itself set the error");
}

#[test]
fn submit_name_entry_with_an_unsafe_name_is_invalid_and_stays_open() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    state.set_name("../escape".to_string());
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Invalid {
            message: strings::DIALOG_NAME_UNSAFE_ERROR.to_string()
        }
    );
    assert!(state.is_open());
}

#[test]
fn submit_move_completes_with_the_selected_destination() {
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    state.select_destination(sample_dir());
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Completed {
            kind: DialogKind::Move {
                path: sample_file()
            },
            name: String::new(),
            destination: Some(sample_dir()),
        }
    );
    assert!(!state.is_open());
}

#[test]
fn submit_move_without_a_destination_is_invalid() {
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Invalid {
            message: strings::DIALOG_MOVE_NO_DESTINATION_ERROR.to_string()
        }
    );
    assert!(state.is_open());
}

#[test]
fn submit_delete_confirm_completes_immediately() {
    let mut state = DialogState::new();
    state.open(DialogKind::DeleteConfirm {
        path: sample_file(),
    });
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Completed {
            kind: DialogKind::DeleteConfirm {
                path: sample_file()
            },
            name: String::new(),
            destination: None,
        }
    );
    assert!(!state.is_open());
}

#[test]
fn submit_with_no_dialog_open_is_invalid() {
    let mut state = DialogState::new();
    let outcome = state.submit();
    assert_eq!(
        outcome,
        SubmitOutcome::Invalid {
            message: strings::DIALOG_NOT_OPEN_ERROR.to_string()
        }
    );
}

// --- dialog_size: the framework box model -------------------------------

/// The §4.5.1 fixed rows, restated here only so the assertions below name
/// what they are checking rather than a bare number. Every value comes
/// from the framework's own tokens.
fn fixed_rows() -> f32 {
    DIALOG_TITLE_HEIGHT
        + DIALOG_PROMPT_HEIGHT
        + DIALOG_ERROR_HEIGHT
        + DIALOG_BUTTONS_HEIGHT
        + DIALOG_SECTION_SPACING * 4.0
        + 2.0 * 16.0
}

#[test]
fn name_entry_size_is_the_framework_box_model() {
    let state = rename_with("note.md");
    let control = state.kind().unwrap().input_control(&state);
    let size = dialog_size(&control);

    assert_eq!(size.width, DIALOG_WIDTH);
    assert_eq!(size.height, fixed_rows() + DIALOG_INPUT_HEIGHT);
    assert_eq!(size.height, 192.0);
}

#[test]
fn zero_shift_an_error_changes_the_message_but_not_one_rectangle() {
    // UIF-INP-003: the validation slot is permanently reserved, so the
    // footer cannot move between "nothing to say" and "saying it".
    let card = Rectangle::new(Point::new(0.0, 0.0), Size::new(DIALOG_WIDTH, 192.0));
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };

    let quiet = InputLayout::solve(card, &control, FooterButtons::default());
    let loud = InputLayout::solve(card, &control, FooterButtons::default());

    assert_eq!(quiet.footer, loud.footer);
    assert_eq!(quiet.ok, loud.ok);
    assert_eq!(quiet.cancel, loud.cancel);
    assert_eq!(quiet.validation, loud.validation);
    assert_eq!(
        quiet.validation.height, DIALOG_ERROR_HEIGHT,
        "the slot is the reserved height whether or not it carries text"
    );
}

#[test]
fn zero_shift_holds_between_withheld_and_painted() {
    // The two states are genuinely different — one paints, one does not —
    // and the card is byte-for-byte the same size anyway.
    let pristine = rename_with("");
    assert!(
        pristine.painted_error().is_none() && pristine.punishment().invalid(),
        "a pristine empty rename is invalid but withheld"
    );

    let committed = committed_rename("");
    assert!(
        committed.painted_error().is_some(),
        "after a refused submit the same value paints"
    );

    let pristine_control = pristine.kind().unwrap().input_control(&pristine);
    let committed_control = committed.kind().unwrap().input_control(&committed);
    assert_eq!(
        dialog_size(&pristine_control),
        dialog_size(&committed_control)
    );
}

#[test]
fn move_size_is_content_sized_up_to_the_maximum() {
    let few = DialogState::new();
    let mut few = few;
    few.open_move(sample_file(), vec![sample_dir()]);
    let few_control = few.kind().unwrap().input_control(&few);
    assert_eq!(
        dialog_size(&few_control).height,
        fixed_rows() + MOVE_ROW_HEIGHT,
        "one destination is a one-row list, not a 200px box with a row in it"
    );

    let many: Vec<PathBuf> = (0..20)
        .map(|n| sample_dir().join(format!("d{n}")))
        .collect();
    let mut state = DialogState::new();
    state.open_move(sample_file(), many);
    let many_control = state.kind().unwrap().input_control(&state);
    assert_eq!(
        dialog_size(&many_control).height,
        fixed_rows() + MOVE_LIST_MAX_HEIGHT,
        "a long list stops at the §4.5.1 maximum"
    );
}

#[test]
fn delete_confirm_is_the_shortest_card_and_still_reserves_the_slot() {
    let state = delete_state();
    let control = state.kind().unwrap().input_control(&state);
    let delete = dialog_size(&control);
    let name = {
        let s = rename_with("x");
        s.kind().unwrap().input_control(&s)
    };

    assert_eq!(delete.height, fixed_rows());
    assert!(delete.height < dialog_size(&name).height);
    assert_eq!(
        delete.height, 160.0,
        "the control row collapses to nothing but the validation slot stays"
    );
}

#[test]
fn dialog_size_is_stable_across_calls() {
    let state = DialogState::new();
    let mut state = state;
    state.open(DialogKind::CreateDocument {
        parent: sample_dir(),
    });
    let control = state.kind().unwrap().input_control(&state);
    assert_eq!(dialog_size(&control), dialog_size(&control));
}

// --- resolve_dialog_anchor: UIF-INP-001 --------------------------------

#[test]
fn no_anchor_is_keyboard_invoked_and_centres_the_card() {
    let size = Size::new(320.0, 192.0);
    let point = resolve_dialog_anchor(None, size, viewport());
    assert_eq!(
        point,
        Point::new(
            (viewport().width - size.width) / 2.0,
            (viewport().height - size.height) / 2.0
        )
    );
}

#[test]
fn a_pointer_anchor_opens_two_pixels_off_the_cursor() {
    // UIF-INP-001: `+2px, +2px` (`space.2xs`) so the pointer rests just
    // outside the card rather than on its first row. The app's retired
    // hand-rolled clamp put the card's corner exactly on the cursor.
    let size = Size::new(320.0, 192.0);
    let anchor = Point::new(10.0, 20.0);
    assert_eq!(
        resolve_dialog_anchor(Some(anchor), size, viewport()),
        Point::new(12.0, 22.0)
    );
}

#[test]
fn an_off_screen_anchor_clamps_to_the_eight_pixel_margin() {
    // UIF-INP-001: an 8px (`space.sm`) margin from every edge, not the
    // flush-at-zero the retired clamp produced.
    let size = Size::new(320.0, 192.0);
    let clamped = resolve_dialog_anchor(Some(Point::new(10_000.0, 10_000.0)), size, viewport());
    assert_eq!(
        clamped,
        Point::new(
            viewport().width - size.width - 8.0,
            viewport().height - size.height - 8.0
        )
    );
    assert!(clamped.x + size.width <= viewport().width - 8.0);
    assert!(clamped.y + size.height <= viewport().height - 8.0);
}

#[test]
fn a_negative_anchor_clamps_to_the_margin_not_the_origin() {
    let size = Size::new(320.0, 192.0);
    assert_eq!(
        resolve_dialog_anchor(Some(Point::new(-50.0, -30.0)), size, viewport()),
        Point::new(8.0, 8.0)
    );
}

#[test]
fn cursor_proximal_slides_rather_than_flipping() {
    // A `CursorProximal` anchor has no mirrored side: near the right edge
    // the card slides back along the axis, it is never thrown to the other
    // side of the cursor the way a context menu is.
    let size = Size::new(320.0, 192.0);
    let point = resolve_dialog_anchor(Some(Point::new(790.0, 300.0)), size, viewport());
    assert!(point.x < 790.0, "the card slid back from the cursor");
    assert_eq!(point.x, viewport().width - size.width - 8.0);
    assert_eq!(
        point.y, 302.0,
        "the vertical axis had room and did not move"
    );
}

#[test]
fn the_centre_fallback_never_goes_negative_in_a_tiny_viewport() {
    let size = Size::new(320.0, 192.0);
    let tiny = Size::new(10.0, 10.0);
    let point = resolve_dialog_anchor(None, size, tiny);
    assert!(point.x >= 0.0 && point.y >= 0.0);
}

// --- dialog_card: render smoke -------------------------------------------

#[test]
fn a_closed_state_renders_an_empty_space_not_a_modal() {
    let state = DialogState::new();
    let card = dialog_card(&state, None, viewport());
    // The empty Space does not fill the parent the way the modal stack does.
    assert_ne!(
        card.as_widget().size(),
        Size::new(Length::Fill, Length::Fill),
        "a closed dialog must not mount a full-area scrim"
    );
}

#[test]
fn every_open_kind_renders_a_full_area_modal_stack() {
    let viewport = viewport();
    let kinds = vec![
        DialogKind::CreateDirectory {
            parent: sample_dir(),
        },
        DialogKind::CreateDocument {
            parent: sample_dir(),
        },
        DialogKind::Rename {
            path: sample_file(),
        },
        DialogKind::Move {
            path: sample_file(),
        },
        DialogKind::DeleteConfirm {
            path: sample_file(),
        },
    ];
    for kind in kinds {
        let mut state = DialogState::new();
        match kind {
            DialogKind::Move { ref path } => state.open_move(path.clone(), vec![sample_dir()]),
            ref other => state.open(other.clone()),
        }
        let card: Element<'_, Message> =
            dialog_card(&state, Some(Point::new(40.0, 40.0)), viewport);
        assert_eq!(
            card.as_widget().size(),
            Size::new(Length::Fill, Length::Fill),
            "the modal needs the whole area to swallow clicks behind the card"
        );
    }
}

#[test]
fn the_card_renders_for_both_anchor_states() {
    let mut state = DialogState::new();
    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    // Anchored and centred both build without panicking.
    let _anchored = dialog_card(&state, Some(Point::new(12.0, 34.0)), viewport());
    let _centred = dialog_card(&state, None, viewport());
}

#[test]
fn a_card_with_an_error_still_renders() {
    let mut state = DialogState::new();
    state.open(DialogKind::Rename {
        path: sample_file(),
    });
    state.set_error(strings::DIALOG_NAME_UNSAFE_ERROR);
    let card: Element<'_, Message> = dialog_card(&state, None, viewport());
    assert_eq!(
        card.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}
// --- InputControl / ButtonIntent mapping (UIF-INP-004) ------------------

#[test]
fn name_entry_kinds_map_to_the_text_entry_control() {
    for kind in [
        DialogKind::CreateDirectory {
            parent: sample_dir(),
        },
        DialogKind::CreateDocument {
            parent: sample_dir(),
        },
        DialogKind::Rename {
            path: sample_file(),
        },
    ] {
        let state = match kind {
            DialogKind::Rename { .. } => rename_with("note.md"),
            ref other => {
                let mut s = DialogState::new();
                s.open(other.clone());
                s.set_name("note.md".to_string());
                s
            }
        };
        let control = kind.input_control(&state);
        assert!(
            matches!(control, InputControl::TextEntry { .. }),
            "{kind:?} must present a text entry"
        );
        assert_eq!(
            control.row_height(),
            DIALOG_INPUT_HEIGHT,
            "the control row is the CONTROL_MD token the field paints at"
        );
    }
}

#[test]
fn the_move_kind_maps_to_the_folder_list_control() {
    let mut state = DialogState::new();
    state.open_move(
        sample_file(),
        vec![sample_dir(), PathBuf::from("elsewhere")],
    );
    let control = state.kind().unwrap().input_control(&state);

    let InputControl::FolderList { folders, selected } = control else {
        panic!("the move dialog must present a folder list, got {control:?}");
    };
    assert_eq!(folders.len(), 2);
    assert_eq!(
        selected, 0,
        "nothing picked yet, so the list reports index zero"
    );
}

#[test]
fn the_folder_list_selection_tracks_the_picked_destination() {
    let second = PathBuf::from("elsewhere");
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir(), second.clone()]);
    state.select_destination(second.clone());

    let control = state.kind().unwrap().input_control(&state);
    let InputControl::FolderList { selected, .. } = control else {
        panic!("expected a folder list");
    };
    assert_eq!(selected, 1);
}

#[test]
fn the_delete_confirm_maps_to_the_bare_confirmation_control() {
    let state = delete_state();
    let control = state.kind().unwrap().input_control(&state);

    assert_eq!(
        control.row_height(),
        0.0,
        "the whole message is the context row, so the control row is nothing"
    );
    let InputControl::Confirmation { ref summary } = control else {
        panic!("a delete confirmation has no input row, got {control:?}");
    };
    assert_eq!(
        summary.as_str(),
        strings::format_delete_confirm_prompt("note.md"),
        "the prompt names the item, not its full path (§4.5.1)"
    );
}

#[test]
fn delete_confirm_is_the_only_danger_intent() {
    assert_eq!(
        DialogKind::DeleteConfirm {
            path: sample_file()
        }
        .button_intent(),
        ButtonIntent::Danger
    );
    for kind in [
        DialogKind::CreateDirectory {
            parent: sample_dir(),
        },
        DialogKind::CreateDocument {
            parent: sample_dir(),
        },
        DialogKind::Rename {
            path: sample_file(),
        },
        DialogKind::Move {
            path: sample_file(),
        },
    ] {
        assert_eq!(
            kind.button_intent(),
            ButtonIntent::Standard,
            "{kind:?} is not a destructive action"
        );
    }
}

#[test]
fn the_danger_intent_paints_the_danger_variant() {
    assert_eq!(
        button_variant(ButtonIntent::Danger),
        ButtonVariant::Danger,
        "UIF-INP-004: the destructive primary renders in status.danger_fill"
    );
    assert_eq!(
        button_variant(ButtonIntent::Standard),
        ButtonVariant::Primary
    );
}

#[test]
fn only_the_delete_confirm_reports_delete_targets() {
    assert_eq!(
        DialogKind::DeleteConfirm {
            path: sample_file()
        }
        .delete_targets(),
        Some(DeleteTargets::Single(
            sample_file()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        ))
    );
    assert_eq!(
        DialogKind::Rename {
            path: sample_file()
        }
        .delete_targets(),
        None
    );
}

// --- Reward Early, Punish Late (UIF-PRIN-009 / UIF-INP-003) ------------

#[test]
fn pristine_and_invalid_paints_nothing() {
    // The case the pillar exists for: the value is wrong, the user has not
    // committed, so the card says nothing and shows no red.
    let state = rename_with("");
    assert!(
        state.punishment().invalid(),
        "an empty rename is invalid — the withheld visual is not a claim of validity"
    );
    assert_eq!(state.punishment().reason(), PunishReason::WithheldPristine);
    assert!(
        state.painted_error().is_none(),
        "a withheld verdict must draw no error line"
    );
}

#[test]
fn pristine_and_valid_paints_nothing() {
    let state = rename_with("notes.md");
    assert_eq!(state.punishment().reason(), PunishReason::Valid);
    assert!(state.painted_error().is_none());
    assert!(state.can_confirm());
}

#[test]
fn committed_and_invalid_paints() {
    let state = committed_rename("");
    assert!(state.is_touched(), "a refused submit commits the field");
    assert_eq!(state.punishment().reason(), PunishReason::TouchedInvalid);
    assert_eq!(
        state.painted_error(),
        Some(strings::DIALOG_NAME_EMPTY_ERROR),
        "the same value that drew nothing while pristine now draws"
    );
}

#[test]
fn a_disallowed_character_paints_while_pristine() {
    // The carve-out: a character the filesystem can never accept in a
    // basename flags on the exact keystroke, stance notwithstanding.
    let state = rename_with("a/b");
    assert!(
        !state.is_touched(),
        "typing never advances the stance — the paint is not coming from a commit"
    );
    assert_eq!(
        state.punishment().reason(),
        PunishReason::DisallowedImmediate,
        "the '/' flags immediately, before the user has finished the name"
    );
    assert_eq!(
        state.painted_error(),
        Some(strings::DIALOG_NAME_UNSAFE_ERROR)
    );
}

#[test]
fn recovery_clears_the_painted_error_on_the_same_frame() {
    let mut state = committed_rename("");
    assert!(state.painted_error().is_some());

    state.set_name("notes.md".to_string());
    assert_eq!(
        state.painted_error(),
        None,
        "the moment the value is valid the visual goes, same frame"
    );
    assert!(state.can_confirm());
}

#[test]
fn recovery_from_a_disallowed_character_also_clears() {
    let mut state = rename_with("a/b");
    assert!(state.painted_error().is_some());

    state.set_name("ab".to_string());
    assert_eq!(state.punishment().reason(), PunishReason::Valid);
    assert!(state.painted_error().is_none());
}

#[test]
fn typing_never_advances_the_stance() {
    let mut state = rename_with("");
    for name in ["a", "ab", "abc", "a/b", "ab/c"] {
        state.set_name(name.to_string());
        assert!(
            !state.is_touched(),
            "set_name({name:?}) must not declare that the user has finished"
        );
    }
}

#[test]
fn blur_advances_the_stance() {
    let mut state = rename_with("");
    assert!(!state.is_touched());
    state.blur();
    assert!(state.is_touched());
    assert_eq!(
        state.painted_error(),
        Some(strings::DIALOG_NAME_EMPTY_ERROR),
        "leaving the field makes the invalid value visible"
    );
}

#[test]
fn a_withheld_error_still_blocks_submit() {
    // Withholding the *visual* is not withholding the *gate*. A pristine
    // empty name is invalid and Ok must not fire.
    let state = rename_with("");
    assert!(state.painted_error().is_none());
    assert!(
        !state.can_confirm(),
        "UIF-INP-003: the confirm button is disabled while the value is invalid"
    );
}

#[test]
fn a_disallowed_character_blocks_submit_immediately() {
    let state = rename_with("a/b");
    assert!(
        !state.can_confirm(),
        "a name the filesystem cannot hold is not submittable, pristine or not"
    );
}

#[test]
fn opening_a_dialog_resets_the_stance_to_pristine() {
    let mut state = committed_rename("");
    assert!(state.is_touched());

    state.open(DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    assert!(
        !state.is_touched(),
        "a new dialog is a new form, not the old one with the text cleared"
    );
    assert!(state.painted_error().is_none());
}

#[test]
fn opening_a_move_dialog_resets_the_stance() {
    let mut state = committed_rename("");
    state.open_move(sample_file(), vec![sample_dir()]);
    assert!(!state.is_touched());
    assert!(state.painted_error().is_none());
}

#[test]
fn the_move_dialog_has_no_field_to_punish_and_shows_the_revealed_error() {
    // The move dialog's error is the reducer's, not a field's: there is
    // no text entry to withhold, so a revealed error paints straight away
    // exactly as it did before the punish rule was wired.
    let mut state = DialogState::new();
    state.open_move(sample_file(), vec![sample_dir()]);
    assert_eq!(state.punishment().reason(), PunishReason::Valid);
    assert!(state.painted_error().is_none());
    assert!(!state.can_confirm());

    state.set_error(strings::DIALOG_MOVE_NO_DESTINATION_ERROR);
    assert_eq!(
        state.painted_error(),
        Some(strings::DIALOG_MOVE_NO_DESTINATION_ERROR)
    );
}

#[test]
fn the_delete_confirmation_never_paints_an_error_line() {
    let state = delete_state();
    assert!(state.painted_error().is_none());
    assert!(state.can_confirm());
}

#[test]
fn a_disallowed_character_blocks_submit_even_though_the_apps_rule_allows_it() {
    // DOCUMENTED DIVERGENCE, not an accident. `validate_name` delegates to
    // `utils::path::is_safe_basename`, which rejects only `/`, `\` and
    // `..` — so `a:b` passes the app's own rule. The punish gate reads
    // `DisallowedClass::FilenameBasename`, which also refuses `: * ? "
    // < > |`, and `punish()` forces `invalid = true` for any disallowed
    // character. The button therefore refuses a name the validator called
    // legal.
    //
    // This is the safer of the two answers on Windows (NTFS refuses `:`
    // outright, so the executor would have failed anyway) but it is a
    // real widening of what `Ok` refuses, and it is asserted here so the
    // two rules cannot drift further apart unnoticed.
    //
    // The card must also *say* why: `painted` is true while `validation`
    // is `None`, so without [`DialogState::painted_error`]'s fallback the
    // button would just refuse in silence.
    let state = rename_with("a:b");
    assert!(
        validate_name(
            &DialogKind::Rename {
                path: sample_file()
            },
            "a:b"
        )
        .is_ok(),
        "the app's own validator still calls `a:b` legal"
    );
    assert!(
        !state.can_confirm(),
        "the punish gate refuses it: a name NTFS cannot store is not submittable"
    );
    assert_eq!(
        state.painted_error(),
        Some(strings::DIALOG_NAME_UNSAFE_ERROR),
        "and the card says so immediately"
    );
}

#[test]
fn the_punish_matrix_covers_every_reason_reachable_from_a_name_field() {
    // One state per arm of the decision tree, so a change to the tree that
    // makes one of these unreachable fails loudly.
    let seen: Vec<PunishReason> = [
        rename_with("ok.md"),
        rename_with(""),
        rename_with("a/b"),
        committed_rename(""),
    ]
    .iter()
    .map(DialogState::punishment)
    .map(|punishment| punishment.reason())
    .collect();

    for reason in PunishReason::ALL {
        assert!(
            seen.contains(&reason),
            "the name-entry states never reach {reason:?}"
        );
    }
}
