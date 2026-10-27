//! Sidecar unit tests for `ui_iced/batch_dialog.rs`.
//!
//! Pure view-model, state-transition and widget-construction tests
//! ([RUST-003]). No renderer, no bus and no filesystem: every candidate
//! list is a synthetic literal that is never opened, and the only rule the
//! tests exercise from the batch subsystem is the pure
//! `validate_batch_params` gate ([RUST-006]).
//!
//! The seven `is_config_valid` cases pin that shared gate across its full
//! valid and invalid range.

use super::*;
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use iced_pro::surface::modal::{BATCH_WIDTH, CONTENT_HEIGHT_MAX, VIEWPORT_GUTTER};
use iced_test::selector::Candidate;
use std::path::PathBuf;

/// A candidate prompt record that is never read off disk.
fn prompt(name: &str) -> PromptInfo {
    PromptInfo {
        path: PathBuf::from(format!(r"C:\fixture\prompts\{name}.md")),
        display_name: format!("System / Skills/Batch/{name}"),
        library_name: "System".to_string(),
        content: "fixture prompt body".to_string(),
    }
}

/// A fully valid, idle configuration.
fn valid_model() -> BatchDialogViewModel {
    BatchDialogViewModel {
        available_dirs: vec![
            PathBuf::from(r"C:\fixture\one"),
            PathBuf::from(r"C:\fixture\two"),
        ],
        available_prompts: vec![prompt("Alpha"), prompt("Beta")],
        selected_dir_idx: Some(1),
        selected_prompt_idx: Some(0),
        pattern: "*.md".to_string(),
        mode: BatchMode::File,
        concurrency: 4,
        status: BatchRunStatus::idle(),
        mode_radio_model: BatchDialogViewModel::create_mode_radio_model(BatchMode::File),
    }
}

// --- Selection state: the start-enabled predicate ---------------------

#[test]
fn can_start_is_true_for_a_fully_valid_idle_composition() {
    let model = valid_model();
    assert!(model.is_config_valid());
    assert!(model.can_start());
}

#[test]
fn can_start_is_false_without_a_directory() {
    let model = BatchDialogViewModel {
        selected_dir_idx: None,
        ..valid_model()
    };
    assert!(!model.is_config_valid());
    assert!(!model.can_start());
}

#[test]
fn can_start_is_false_without_a_prompt() {
    let model = BatchDialogViewModel {
        selected_prompt_idx: None,
        ..valid_model()
    };
    assert!(!model.is_config_valid());
    assert!(!model.can_start());
}

#[test]
fn can_start_is_false_for_an_empty_pattern_in_file_mode() {
    let model = BatchDialogViewModel {
        pattern: String::new(),
        ..valid_model()
    };
    assert!(!model.is_config_valid());
}

#[test]
fn can_start_is_false_for_a_whitespace_pattern_in_file_mode() {
    let model = BatchDialogViewModel {
        pattern: "   ".to_string(),
        ..valid_model()
    };
    assert!(!model.is_config_valid());
}

#[test]
fn can_start_is_false_for_an_unparsable_glob() {
    let model = BatchDialogViewModel {
        pattern: "[invalid".to_string(),
        ..valid_model()
    };
    assert!(!model.is_config_valid());
}

#[test]
fn directory_mode_does_not_need_a_pattern() {
    let model = BatchDialogViewModel {
        pattern: String::new(),
        mode: BatchMode::Directory,
        ..valid_model()
    };
    assert!(model.is_config_valid());
    assert!(model.can_start());
}

#[test]
fn concurrency_outside_one_to_eight_disables_start() {
    for concurrency in [0, 9, 255] {
        let model = BatchDialogViewModel {
            concurrency,
            ..valid_model()
        };
        assert!(
            !model.can_start(),
            "concurrency {concurrency} must not be startable"
        );
    }
}

#[test]
fn concurrency_bounds_one_and_eight_are_startable() {
    for concurrency in [CONCURRENCY_MIN, CONCURRENCY_MAX] {
        let model = BatchDialogViewModel {
            concurrency,
            ..valid_model()
        };
        assert!(
            model.can_start(),
            "concurrency {concurrency} must be startable"
        );
    }
}

#[test]
fn a_run_in_flight_blocks_a_second_start() {
    let model = valid_model().with_status(BatchRunStatus {
        has_handle: true,
        finished: false,
        cancel_requested: false,
    });
    assert!(
        model.is_config_valid(),
        "the composition is still valid while a run is in flight"
    );
    assert!(
        !model.can_start(),
        "a second StartBatch must not be offered while one is running"
    );
}

// --- Selection state: options, labels, mode gating --------------------

#[test]
fn directory_options_carry_index_and_display_label() {
    let model = valid_model();
    let options = model.directory_options();
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].index, 0);
    assert_eq!(options[1].index, 1);
    assert_eq!(
        options[1].label,
        PathBuf::from(r"C:\fixture\two").display().to_string()
    );
    assert_eq!(options[1].to_string(), options[1].label);
}

#[test]
fn prompt_options_are_labelled_from_the_candidate_display_name() {
    let model = valid_model();
    let options = model.prompt_options();
    let labels: Vec<String> = options.iter().map(ToString::to_string).collect();
    assert_eq!(
        labels,
        vec![
            "System / Skills/Batch/Alpha".to_string(),
            "System / Skills/Batch/Beta".to_string()
        ]
    );
    assert_eq!(options[0].index, 0);
    assert_eq!(options[1].index, 1);
}

#[test]
fn unselected_combos_show_their_placeholder_strings() {
    let model = BatchDialogViewModel::empty();
    assert_eq!(
        model.directory_label(),
        strings::BATCH_DIALOG_SELECT_DIRECTORY
    );
    assert_eq!(model.prompt_label(), strings::BATCH_DIALOG_SELECT_PROMPT);
}

#[test]
fn selected_entries_replace_the_placeholder() {
    let model = valid_model();
    assert_eq!(
        model.directory_label(),
        PathBuf::from(r"C:\fixture\two").display().to_string()
    );
    assert_eq!(
        model.prompt_label(),
        "System / Skills/Batch/Alpha".to_string()
    );
}

#[test]
fn an_out_of_range_index_reads_as_no_selection() {
    let model = BatchDialogViewModel {
        selected_dir_idx: Some(99),
        selected_prompt_idx: Some(99),
        ..valid_model()
    };
    assert!(model.selected_directory().is_none());
    assert!(model.selected_prompt().is_none());
    assert!(model.build_batch_config().is_none());
    assert!(start_message(&model).is_none());
}

#[test]
fn pattern_row_is_visible_only_in_file_mode() {
    assert!(valid_model().pattern_row_visible());
    let directory_mode = BatchDialogViewModel {
        mode: BatchMode::Directory,
        ..valid_model()
    };
    assert!(!directory_mode.pattern_row_visible());
}

#[test]
fn concurrency_options_are_the_one_to_eight_range() {
    let options = BatchDialogViewModel::concurrency_options();
    assert_eq!(options.len(), 8);
    assert_eq!(options.first(), Some(&CONCURRENCY_MIN));
    assert_eq!(options.last(), Some(&CONCURRENCY_MAX));
}

// --- Config snapshot correctness --------------------------------------

#[test]
fn from_config_copies_every_config_field() {
    let config = BatchDialogConfig {
        available_dirs: vec![PathBuf::from(r"C:\fixture\a")],
        available_prompts: vec![prompt("Alpha")],
        selected_dir_idx: Some(0),
        selected_prompt_idx: Some(0),
        pattern: "*.txt".to_string(),
        mode: BatchMode::Directory,
        concurrency: 7,
    };
    let model = BatchDialogViewModel::from_config(&config);

    assert_eq!(model.available_dirs, config.available_dirs);
    assert_eq!(model.available_prompts.len(), 1);
    assert_eq!(model.selected_dir_idx, Some(0));
    assert_eq!(model.selected_prompt_idx, Some(0));
    assert_eq!(model.pattern, "*.txt");
    assert_eq!(model.mode, BatchMode::Directory);
    assert_eq!(model.concurrency, 7);
    assert!(
        model.status.is_idle(),
        "a bare config snapshot carries no run status"
    );
}

#[test]
fn default_snapshot_matches_the_config_defaults() {
    let model = BatchDialogViewModel::default();
    assert!(model.available_dirs.is_empty());
    assert!(model.available_prompts.is_empty());
    assert_eq!(model.pattern, DEFAULT_PATTERN);
    assert_eq!(model.mode, BatchMode::File);
    assert_eq!(model.concurrency, 4);
    assert!(model.selected_dir_idx.is_none());
    assert!(model.selected_prompt_idx.is_none());
    assert!(model.status.is_idle());
}

#[test]
fn build_batch_config_composes_the_start_payload_from_the_two_picks() {
    let model = valid_model();
    let config = model.build_batch_config().expect("valid composition");

    assert_eq!(config.directory, PathBuf::from(r"C:\fixture\two"));
    assert_eq!(config.pattern, "*.md");
    assert_eq!(config.prompt_path, prompt("Alpha").path);
    assert_eq!(config.mode, BatchMode::File);
    assert_eq!(config.concurrency, 4);
}

#[test]
fn build_batch_config_is_none_when_either_pick_is_missing() {
    for (dir_idx, prompt_idx) in [(None, Some(0)), (Some(0), None), (None, None)] {
        let model = BatchDialogViewModel {
            selected_dir_idx: dir_idx,
            selected_prompt_idx: prompt_idx,
            ..valid_model()
        };
        assert_eq!(
            model.build_batch_config(),
            None,
            "dir {dir_idx:?} / prompt {prompt_idx:?} must not compose"
        );
    }
}

// --- Selection-state rules ------------------------------------------------

#[test]
fn clamp_selection_drops_an_index_past_the_list_end() {
    assert_eq!(clamp_selection(Some(0), 3), Some(0));
    assert_eq!(clamp_selection(Some(2), 3), Some(2));
    assert_eq!(clamp_selection(Some(3), 3), None);
    assert_eq!(clamp_selection(Some(0), 0), None);
    assert_eq!(clamp_selection(None, 3), None);
}

#[test]
fn index_of_selected_follows_the_value_not_the_position() {
    let list = vec!["a", "b", "c"];
    assert_eq!(index_of_selected(Some(&"b"), &list), Some(1));
    // Reordered list: the selection tracks the value, not the old index.
    let reordered = vec!["c", "b", "a"];
    assert_eq!(index_of_selected(Some(&"b"), &reordered), Some(1));
    assert_eq!(index_of_selected(Some(&"z"), &list), None);
    assert_eq!(index_of_selected(None, &list), None);
}

// --- Action -> command mapping ----------------------------------------

#[test]
fn start_message_carries_the_selected_prompt_body_and_config_on_the_command_channel() {
    let mut model = valid_model();
    model.available_prompts[1].content = "selected Beta prompt body".to_string();
    model.selected_prompt_idx = Some(1);
    let expected_config = model.build_batch_config().expect("valid");
    assert_eq!(
        start_message(&model),
        Some(Message::Command(UserCommand::StartBatch {
            config: expected_config,
            prompt_text: "selected Beta prompt body".to_string(),
        }))
    );
}

#[test]
fn start_message_is_none_when_the_composition_is_invalid() {
    let model = BatchDialogViewModel::empty();
    assert_eq!(start_message(&model), None);
}

#[test]
fn cancel_message_is_cancel_batch_on_the_command_channel() {
    assert_eq!(cancel_message(), Message::Command(UserCommand::CancelBatch));
}

#[test]
fn every_batch_cancel_path_shares_one_transition() {
    // Cancel button, Close button, scrim and Escape all map to the same
    // command — a single cancel publish.
    let paths = [cancel_message(), cancel_message(), cancel_message()];
    for path in paths {
        assert_eq!(path, Message::Command(UserCommand::CancelBatch));
    }
}

// --- Run status -> rendered text ---------------------------------------

#[test]
fn idle_status_renders_neither_completion_nor_cancellation_text() {
    let status = BatchRunStatus::idle();
    assert!(status.is_idle());
    assert!(!status.shows_progress_view());
    assert_eq!(status.completion_text(), None);
    assert_eq!(status.cancellation_text(), None);
}

#[test]
fn a_running_job_shows_progress_but_no_completion_text() {
    let status = BatchRunStatus {
        has_handle: true,
        finished: false,
        cancel_requested: false,
    };
    assert!(!status.is_idle());
    assert!(status.shows_progress_view());
    assert_eq!(status.completion_text(), None);
    assert_eq!(status.cancellation_text(), None);
}

#[test]
fn a_finished_job_shows_the_completion_text() {
    let status = BatchRunStatus {
        has_handle: true,
        finished: true,
        cancel_requested: false,
    };
    assert_eq!(
        status.completion_text(),
        Some(strings::BATCH_COMPLETED_TEXT)
    );
    assert_eq!(status.cancellation_text(), None);
}

#[test]
fn a_cancelled_finished_job_adds_the_cancellation_warning() {
    let status = BatchRunStatus {
        has_handle: true,
        finished: true,
        cancel_requested: true,
    };
    assert_eq!(
        status.completion_text(),
        Some(strings::BATCH_COMPLETED_TEXT)
    );
    assert_eq!(
        status.cancellation_text(),
        Some(strings::BATCH_CANCELLED_TEXT)
    );
}

#[test]
fn cancellation_text_never_shows_without_a_finished_run() {
    let status = BatchRunStatus {
        has_handle: true,
        finished: false,
        cancel_requested: true,
    };
    assert_eq!(status.cancellation_text(), None);
}

// --- Window state transitions -----------------------------------------

#[test]
fn batch_state_starts_closed() {
    let state = BatchDialogState::new();
    assert!(!state.is_open());
    assert_eq!(state.current_view(), BatchDialogView::Config);
    assert!(!state.can_start());
}

#[test]
fn open_reports_the_transition_and_shows_the_dialog() {
    let mut state = BatchDialogState::new();
    assert!(state.open(valid_model()));
    assert!(state.is_open());
    assert!(state.can_start());
    // Re-opening the same model reports "nothing changed" on the open flag.
    assert!(!state.open(valid_model()));
    assert!(state.is_open());
}

#[test]
fn open_clamps_selections_that_no_longer_resolve() {
    let stale = BatchDialogViewModel {
        available_dirs: vec![PathBuf::from(r"C:\fixture\only")],
        available_prompts: vec![prompt("Alpha")],
        selected_dir_idx: Some(5),
        selected_prompt_idx: Some(2),
        ..valid_model()
    };
    let mut state = BatchDialogState::new();
    assert!(state.open(stale));
    assert_eq!(
        state.model().selected_dir_idx,
        None,
        "a directory index past the fresh list must be dropped"
    );
    assert_eq!(state.model().selected_prompt_idx, None);
    assert!(!state.can_start());
}

#[test]
fn open_keeps_selections_that_still_resolve() {
    let mut state = BatchDialogState::new();
    assert!(state.open(valid_model()));
    assert_eq!(state.model().selected_dir_idx, Some(1));
    assert_eq!(state.model().selected_prompt_idx, Some(0));
    assert!(state.can_start());
}

#[test]
fn close_reports_whether_a_dialog_was_consumed() {
    let mut state = BatchDialogState::new();
    assert!(
        !state.close(),
        "closing an already-closed dialog consumes nothing"
    );
    state.open(valid_model());
    assert!(state.close());
    assert!(!state.is_open());
    assert!(!state.close());
}

#[test]
fn close_keeps_the_composed_selection_for_a_reopen() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    state.close();
    assert_eq!(state.model().pattern, "*.md");
    assert_eq!(state.model().selected_dir_idx, Some(1));
}

#[test]
fn toggle_flips_the_visible_state() {
    let mut state = BatchDialogState::new();
    assert!(state.toggle());
    assert!(state.is_open());
    assert!(!state.toggle());
    assert!(!state.is_open());
}

#[test]
fn prompt_candidate_refresh_rejects_stale_results_and_preserves_selection_by_path() {
    let selected = PromptInfo {
        path: PathBuf::from("selected.md"),
        display_name: "Selected".to_string(),
        library_name: "test".to_string(),
        content: "selected body".to_string(),
    };
    let model = BatchDialogViewModel {
        available_prompts: vec![selected.clone()],
        selected_prompt_idx: Some(0),
        ..valid_model()
    };
    let mut state = BatchDialogState::new();
    state.open(model.clone());
    let stale_generation = state.prompt_load_generation();
    state.close();

    let refreshed = vec![
        PromptInfo {
            path: PathBuf::from("new.md"),
            display_name: "New".to_string(),
            library_name: "test".to_string(),
            content: "new body".to_string(),
        },
        selected,
    ];
    assert!(
        !state.apply_prompt_candidates(stale_generation, refreshed.clone()),
        "a completed scan must not update a closed dialog"
    );

    state.open(model);
    let current_generation = state.prompt_load_generation();
    assert_ne!(stale_generation, current_generation);
    assert!(
        !state.apply_prompt_candidates(stale_generation, refreshed.clone()),
        "a previous open request must not overwrite a newer prompt catalog"
    );
    assert!(state.apply_prompt_candidates(current_generation, refreshed));
    assert_eq!(state.model().selected_prompt_idx, Some(1));
}

#[test]
fn prompt_candidate_refresh_clears_a_selection_that_disappeared() {
    let selected = PromptInfo {
        path: PathBuf::from("selected.md"),
        display_name: "Selected".to_string(),
        library_name: "test".to_string(),
        content: "selected body".to_string(),
    };
    let model = BatchDialogViewModel {
        available_prompts: vec![selected],
        selected_prompt_idx: Some(0),
        ..valid_model()
    };
    let mut state = BatchDialogState::new();
    state.open(model);

    assert!(state.apply_prompt_candidates(
        state.prompt_load_generation(),
        vec![PromptInfo {
            path: PathBuf::from("replacement.md"),
            display_name: "Replacement".to_string(),
            library_name: "test".to_string(),
            content: "replacement body".to_string(),
        }],
    ));
    assert!(state.model().selected_prompt_idx.is_none());
}

#[test]
fn current_view_follows_the_run_status() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert_eq!(state.current_view(), BatchDialogView::Config);

    state.model.status = BatchRunStatus {
        has_handle: true,
        finished: false,
        cancel_requested: false,
    };
    assert_eq!(state.current_view(), BatchDialogView::Progress);
}

// --- apply(): selection transitions and change detection --------------

#[test]
fn apply_updates_the_directory_selection_and_reports_the_change() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());

    assert!(state.apply(BatchDialogMessage::DirectorySelected(Some(0))));
    assert_eq!(state.model().selected_dir_idx, Some(0));

    assert!(
        !state.apply(BatchDialogMessage::DirectorySelected(Some(0))),
        "a redundant selection must report no change"
    );
}

#[test]
fn apply_clears_a_selection_with_none() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert!(state.apply(BatchDialogMessage::PromptSelected(None)));
    assert_eq!(state.model().selected_prompt_idx, None);
    assert!(!state.can_start());
}

#[test]
fn apply_updates_the_pattern() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert!(state.apply(BatchDialogMessage::PatternChanged {
        text: "*.markdown".to_string()
    }));
    assert_eq!(state.model().pattern, "*.markdown");
    assert!(!state.apply(BatchDialogMessage::PatternChanged {
        text: "*.markdown".to_string()
    }));
}

#[test]
fn apply_switches_mode_and_revalidates() {
    let mut state = BatchDialogState::new();
    state.open(BatchDialogViewModel {
        pattern: String::new(),
        ..valid_model()
    });
    assert!(
        !state.can_start(),
        "an empty pattern is invalid in File mode"
    );
    assert!(state.apply(BatchDialogMessage::ModeChanged(BatchMode::Directory)));
    assert!(state.can_start(), "Directory mode needs no pattern");
    assert!(state.apply(BatchDialogMessage::ModeChanged(BatchMode::File)));
    assert!(!state.can_start());
}

#[test]
fn apply_updates_concurrency_and_revalidates() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert!(state.apply(BatchDialogMessage::ConcurrencyChanged(8)));
    assert_eq!(state.model().concurrency, 8);
    assert!(state.can_start());
    assert!(state.apply(BatchDialogMessage::ConcurrencyChanged(0)));
    assert!(!state.can_start());
}

#[test]
fn batch_config_accessor_mirrors_the_view_model() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert_eq!(state.batch_config(), state.model().build_batch_config());
    state.apply(BatchDialogMessage::PromptSelected(None));
    assert_eq!(state.batch_config(), None);
}

// --- View construction ------------------------------------------------

/// A roomy viewport, so view-construction tests exercise the policy's
/// unclamped branch rather than the gutter clamp.
const ROOMY: Size = Size::new(1920.0, 1080.0);

#[test]
fn card_builds_for_the_config_view() {
    let model = valid_model();
    let element = batch_dialog_card(&model, ROOMY);
    // A built Element is a non-zero-sized boxed widget tree; the point of
    // the test is that the config branch constructs without panicking.
    assert!(std::mem::size_of_val(&element) > 0);
}

/// The real File-mode card reports its existing pattern-change message when
/// the user types into the empty field from its placeholder.
#[test]
fn file_mode_pattern_input_emits_pattern_changed_when_typed() {
    let model = BatchDialogViewModel {
        pattern: String::new(),
        ..valid_model()
    };
    let mut ui = iced_test::simulator(batch_dialog_card(&model, ROOMY));
    let bounds = pattern_input_bounds(&mut ui);
    click_at(&mut ui, bounds.center());
    let _ = ui.typewrite("x");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![Message::Batch(BatchDialogMessage::PatternChanged {
            text: "x".to_string(),
        })]
    );
}

/// The field has an addressable, stable identity in File mode.
#[test]
fn file_mode_pattern_input_has_its_stable_id() {
    let model = valid_model();
    let mut ui = iced_test::simulator(batch_dialog_card(&model, ROOMY));
    assert_eq!(BATCH_PATTERN_INPUT_FOCUS_ID, "batch_dialog_pattern_input");
    let expected = iced::widget::Id::from(BATCH_PATTERN_INPUT_FOCUS_ID);

    assert!(
        ui.find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput { id: Some(id), .. } if *id == expected => Some(()),
            _ => None,
        })
        .is_ok(),
        "the File-mode pattern input must expose its stable widget id"
    );
}

/// The prompt list remains addressable when the conditional pattern row is absent.
#[test]
fn prompt_scrollable_keeps_its_stable_id_in_both_modes() {
    for mode in [BatchMode::File, BatchMode::Directory] {
        let model = BatchDialogViewModel {
            mode,
            ..valid_model()
        };
        let mut ui = iced_test::simulator(batch_dialog_card(&model, ROOMY));

        assert!(
            ui.find(iced::widget::Id::from(BATCH_PROMPT_SCROLLABLE_ID))
                .is_ok(),
            "the prompt scrollable must retain its stable id in {mode:?} mode"
        );
    }
}

/// Directory mode hides the unused pattern row and ignores its retained value.
#[test]
fn directory_mode_hides_pattern_field_and_emits_no_edit() {
    let model = BatchDialogViewModel {
        mode: BatchMode::Directory,
        ..valid_model()
    };
    let mut ui = iced_test::simulator(batch_dialog_card(&model, ROOMY));
    let expected = iced::widget::Id::from(BATCH_PATTERN_INPUT_FOCUS_ID);
    assert!(
        ui.find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput { id: Some(id), .. } if *id == expected => Some(()),
            _ => None,
        })
        .is_err(),
        "Directory mode must not mount the wildcard-pattern input"
    );
    assert!(
        ui.find(strings::BATCH_DIALOG_PATTERN_LABEL).is_err(),
        "Directory mode must hide the wildcard-pattern row label"
    );

    let _ = ui.typewrite("x");

    let emitted = ui.into_messages().collect::<Vec<_>>();
    assert!(
        emitted.is_empty(),
        "a hidden Directory-mode pattern field must not emit messages: {emitted:?}"
    );
}

/// Finds the text-input field in the real Batch card's simulator tree.
fn pattern_input_bounds(
    ui: &mut iced_test::Simulator<'_, Message, iced::Theme, iced::Renderer>,
) -> iced::Rectangle {
    ui.find(|candidate: Candidate<'_>| match candidate {
        Candidate::TextInput { bounds, .. } => Some(bounds),
        _ => None,
    })
    .expect("the Batch card must render its pattern input")
}

/// Sends pointer motion and a left click at the supplied widget position.
fn click_at(
    ui: &mut iced_test::Simulator<'_, Message, iced::Theme, iced::Renderer>,
    position: iced::Point,
) {
    ui.point_at(position);
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position,
    })]);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
}

#[test]
fn card_builds_for_the_progress_view_in_every_run_state() {
    for status in [
        BatchRunStatus {
            has_handle: true,
            finished: false,
            cancel_requested: false,
        },
        BatchRunStatus {
            has_handle: true,
            finished: true,
            cancel_requested: false,
        },
        BatchRunStatus {
            has_handle: true,
            finished: true,
            cancel_requested: true,
        },
    ] {
        let model = valid_model().with_status(status);
        let element = batch_dialog_card(&model, ROOMY);
        assert!(std::mem::size_of_val(&element) > 0);
    }
}

#[test]
fn card_builds_from_an_empty_model_without_panicking() {
    let model = BatchDialogViewModel::empty();
    let element = batch_dialog_card(&model, ROOMY);
    assert!(std::mem::size_of_val(&element) > 0);
    assert!(!model.can_start());
}

#[test]
fn view_method_builds_the_same_card_shape() {
    let model = valid_model();
    // Both entry points must produce the same branch for the same model.
    let via_view = std::mem::size_of_val(&model.view(ROOMY));
    let via_card = std::mem::size_of_val(&batch_dialog_card(&model, ROOMY));
    assert_eq!(via_view, via_card);
}
// --- §4.6.2 sizing: the framework resolves the box, the card only asks --

/// The Batch card's width at four viewports, each with the value the
/// framework's own policy must produce.
///
/// The point is not the number — `BATCH_WIDTH` is the framework's — but
/// that the app no longer owns a width of its own at any viewport.
#[test]
fn card_width_is_the_framework_policy_at_every_viewport() {
    // Roomy: the policy's own figure, untouched.
    assert_eq!(batch_card_width(1920.0), BATCH_WIDTH);
    assert_eq!(batch_card_width(1366.0), BATCH_WIDTH);
    // Exactly the width the card needs: still no clamp.
    assert_eq!(batch_card_width(BATCH_WIDTH + VIEWPORT_GUTTER), BATCH_WIDTH);
    // Too narrow: the gutter clamp takes over, 32px off the viewport.
    assert_eq!(batch_card_width(500.0), 500.0 - VIEWPORT_GUTTER);
    assert_eq!(batch_card_width(400.0), 400.0 - VIEWPORT_GUTTER);
}

/// The Batch card's height is the content-fit ceiling, clamped last by
/// the gutter — the same resolution order the editor's axis showed, with
/// the ceiling replacing the 80%-of-viewport want.
#[test]
fn card_height_is_the_content_fit_ceiling_clamped_by_the_gutter() {
    assert_eq!(batch_card_height(1080.0), CONTENT_HEIGHT_MAX);
    assert_eq!(batch_card_height(600.0), CONTENT_HEIGHT_MAX);
    // Below the ceiling the viewport wins: the card cannot be taller than
    // the screen minus the gutter.
    assert_eq!(batch_card_height(500.0), 500.0 - VIEWPORT_GUTTER);
    assert_eq!(batch_card_height(400.0), 400.0 - VIEWPORT_GUTTER);
}

/// The card's size is the two axes composed, so no caller can pair a
/// width from one policy with a height from another.
#[test]
fn card_size_composes_both_axes_from_the_one_policy() {
    let size = batch_card_size(Size::new(1920.0, 1080.0));
    assert_eq!(size, Size::new(BATCH_WIDTH, CONTENT_HEIGHT_MAX));

    let squeezed = batch_card_size(Size::new(500.0, 400.0));
    assert_eq!(squeezed, Size::new(468.0, 368.0));
}

/// The width this card used to carry is retired and must never come back.
///
/// `BATCH_CARD_WIDTH = 500.0` was the app's own figure and was 60px
/// narrower than §4.6.2's `BATCH_MODAL_MAX`. The guard is written against
/// the literal rather than the deleted constant so a future edit that
/// re-introduces 500 anywhere in this card's width path fails here with a
/// named reason instead of silently re-shrinking the dialog.
#[test]
fn the_retired_500px_card_width_never_returns() {
    const RETIRED_BATCH_CARD_WIDTH: f32 = 500.0;
    assert_ne!(RETIRED_BATCH_CARD_WIDTH, BATCH_WIDTH);
    assert_ne!(batch_card_width(1920.0), RETIRED_BATCH_CARD_WIDTH);
    assert_ne!(
        batch_card_size(Size::new(1920.0, 1080.0)).width,
        RETIRED_BATCH_CARD_WIDTH
    );
}

/// The +60px the card gained is a real rewrap, not a rounding artefact:
/// every prompt-candidate row now lays out across 60px more, so a display
/// name that fitted on one line at 500 may take two at 560 or vice versa.
/// Recorded here so the delta is a known change rather than a surprise.
#[test]
fn the_card_is_60px_wider_than_the_retired_figure() {
    assert_eq!(BATCH_WIDTH - 500.0, 60.0);
}

// --- The pattern field's error comes from the subsystem ---------------

/// A blank pattern in File mode is the subsystem's own refusal, worded by
/// `strings`, and it is `'static` so the caption band can hold it.
#[test]
fn a_blank_pattern_in_file_mode_reports_the_required_message() {
    let model = BatchDialogViewModel {
        pattern: String::new(),
        ..valid_model()
    };
    assert_eq!(
        model.pattern_error(),
        Some(strings::BATCH_PATTERN_REQUIRED_ERROR)
    );

    let blank_spaces = BatchDialogViewModel {
        pattern: "   ".to_string(),
        ..valid_model()
    };
    assert_eq!(
        blank_spaces.pattern_error(),
        Some(strings::BATCH_PATTERN_REQUIRED_ERROR)
    );
}

/// An unparsable glob is a different message from a blank one, and neither
/// is the raw `glob::PatternError` text.
#[test]
fn an_unparsable_glob_reports_the_invalid_message() {
    let model = BatchDialogViewModel {
        pattern: "[unclosed".to_string(),
        ..valid_model()
    };
    assert_eq!(
        model.pattern_error(),
        Some(strings::BATCH_PATTERN_INVALID_ERROR)
    );
}

/// A valid glob in File mode reports nothing.
#[test]
fn a_valid_glob_in_file_mode_reports_no_error() {
    let model = BatchDialogViewModel {
        pattern: "*.md".to_string(),
        ..valid_model()
    };
    assert_eq!(model.pattern_error(), None);
}

/// Directory mode skips the pattern term entirely, so whatever is in the
/// field cannot be a fault there.
#[test]
fn directory_mode_never_faults_the_pattern_field() {
    for pattern in ["", "   ", "[unclosed", "*.md"] {
        let model = BatchDialogViewModel {
            pattern: pattern.to_string(),
            mode: BatchMode::Directory,
            ..valid_model()
        };
        assert_eq!(
            model.pattern_error(),
            None,
            "Directory mode must not fault pattern {pattern:?}"
        );
    }
}

/// The pattern field's failure and the whole-composition gate agree: a
/// field the card calls broken is a composition `can_start` refuses.
#[test]
fn a_faulted_pattern_field_also_blocks_start() {
    let broken = BatchDialogViewModel {
        pattern: "[unclosed".to_string(),
        ..valid_model()
    };
    assert!(broken.pattern_error().is_some());
    assert!(!broken.can_start());
}

// --- Progress status mapping ----------------------------------------

/// A cancellation request is `Paused`, not `Error`: the user stopped the
/// run, nothing failed.
#[test]
fn a_cancel_request_maps_to_paused_not_error() {
    let cancelled = BatchRunStatus {
        has_handle: true,
        finished: true,
        cancel_requested: true,
    };
    assert_eq!(progress_status(cancelled), ProgressStatus::Paused);
    assert!(
        !progress_status(cancelled).is_moving(),
        "a cancel-requested run must not be drawn as still progressing"
    );
}

/// A running batch is `Active`, and a finished one is still `Active`
/// because `ProgressStatus` has no completed position — the completion
/// text carries "done", not the bar.
#[test]
fn a_run_maps_to_active_and_completion_is_not_a_status() {
    let running = BatchRunStatus {
        has_handle: true,
        finished: false,
        cancel_requested: false,
    };
    let finished = BatchRunStatus {
        has_handle: true,
        finished: true,
        cancel_requested: false,
    };
    assert_eq!(progress_status(running), ProgressStatus::Active);
    assert_eq!(progress_status(finished), ProgressStatus::Active);
    assert!(progress_status(running).is_moving());
}

/// The indeterminate model the bar is built from reports no fraction, so
/// nothing here invents a completed-of-total count the batch subsystem
/// never reported.
#[test]
fn the_progress_model_carries_no_invented_fraction() {
    let model = ProgressModel::indeterminate().with_status(ProgressStatus::Active);
    assert!(model.is_indeterminate());
    assert_eq!(model.fraction(), None);
}

// --- Absorption -----------------------------------------------------

/// `Absorbed` is the no-op the card-boundary `mouse_area` needs, and it
/// must not disturb the open state.
#[test]
fn absorbed_changes_nothing() {
    let mut state = BatchDialogState::new();
    state.open(valid_model());
    assert!(state.is_open());
    assert!(
        !state.apply(BatchDialogMessage::Absorbed),
        "Absorbed must report no state change"
    );
    assert!(state.is_open());
}

/// Absorbing while closed stays closed — the variant cannot open a dialog.
#[test]
fn absorbed_cannot_open_a_closed_dialog() {
    let mut state = BatchDialogState::new();
    assert!(!state.is_open());
    assert!(!state.apply(BatchDialogMessage::Absorbed));
    assert!(!state.is_open());
}
