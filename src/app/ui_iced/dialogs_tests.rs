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

// --- dialog_size: deterministic per kind ---------------------------------

#[test]
fn name_entry_size_is_wider_than_tall_and_uses_the_input_height() {
    let kind = DialogKind::Rename {
        path: sample_file(),
    };
    let size = dialog_size(&kind);
    assert_eq!(size.width, DIALOG_WIDTH);
    let expected = MODAL_PADDING * 2.0
        + DIALOG_TITLE_HEIGHT
        + DIALOG_PROMPT_HEIGHT
        + DIALOG_BUTTONS_HEIGHT
        + DIALOG_INPUT_HEIGHT
        + DIALOG_ERROR_HEIGHT
        + DIALOG_SECTION_SPACING * 4.0;
    assert_eq!(size.height, expected);
}

#[test]
fn move_size_uses_the_list_max_height() {
    let kind = DialogKind::Move {
        path: sample_file(),
    };
    let size = dialog_size(&kind);
    assert!(
        size.height
            > dialog_size(&DialogKind::Rename {
                path: sample_file()
            })
            .height,
        "the move list makes the card taller than a name-entry card"
    );
    assert!(size.height >= MOVE_LIST_MAX_HEIGHT);
}

#[test]
fn delete_confirm_is_the_shortest_card() {
    let delete = dialog_size(&DialogKind::DeleteConfirm {
        path: sample_file(),
    });
    let name = dialog_size(&DialogKind::CreateDirectory {
        parent: sample_dir(),
    });
    let move_size = dialog_size(&DialogKind::Move {
        path: sample_file(),
    });
    assert!(delete.height < name.height);
    assert!(delete.height < move_size.height);
}

#[test]
fn dialog_size_is_stable_across_calls() {
    let kind = DialogKind::CreateDocument {
        parent: sample_dir(),
    };
    assert_eq!(dialog_size(&kind), dialog_size(&kind));
}

// --- resolve_dialog_anchor: fallback / clamp -----------------------------

#[test]
fn no_anchor_centres_the_card() {
    let size = dialog_size(&DialogKind::Rename {
        path: sample_file(),
    });
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
fn an_in_bounds_anchor_is_left_untouched() {
    let size = dialog_size(&DialogKind::Rename {
        path: sample_file(),
    });
    let anchor = Point::new(10.0, 20.0);
    assert_eq!(
        resolve_dialog_anchor(Some(anchor), size, viewport()),
        anchor
    );
}

#[test]
fn an_off_screen_anchor_is_clamped_back_onto_the_viewport() {
    let size = dialog_size(&DialogKind::Rename {
        path: sample_file(),
    });
    let clamped = resolve_dialog_anchor(Some(Point::new(10_000.0, 10_000.0)), size, viewport());
    assert_eq!(
        clamped,
        Point::new(
            viewport().width - size.width,
            viewport().height - size.height
        )
    );
    // The card's bottom-right corner lands exactly on the viewport edge.
    assert!(clamped.x + size.width <= viewport().width);
    assert!(clamped.y + size.height <= viewport().height);
}

#[test]
fn a_negative_anchor_clamps_to_the_origin() {
    let size = dialog_size(&DialogKind::Rename {
        path: sample_file(),
    });
    assert_eq!(
        resolve_dialog_anchor(Some(Point::new(-50.0, -30.0)), size, viewport()),
        Point::new(0.0, 0.0)
    );
}

#[test]
fn the_centre_fallback_never_goes_negative_in_a_tiny_viewport() {
    let size = dialog_size(&DialogKind::Move {
        path: sample_file(),
    });
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
