//! Sidecar unit tests for `ui_iced/tools_dialog.rs` ([TASK-017b3]).
//!
//! Pure mapping, state-transition and widget-construction tests
//! ([RUST-003]). Every group is a synthetic [`ToolGroupState`] literal, so
//! no registry, subprocess, network or filesystem is touched
//! ([RUST-006]); the only app-level read is one `FastMdApp::empty_state`
//! smoke test that mirrors the existing the legacy UI fixtures.
//!
//! The `compute_dialog_size` cases mirror the legacy UI originals in
//! `src/app/ui/tools_dialog_tests.rs:127-188` so the height rule is
//! pinned on both sides of the migration.

use super::*;
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use iced::Size;

/// An internal family group with two tools.
fn internal_group(family: InternalToolGroup) -> ToolGroupState {
    ToolGroupState {
        id: ToolGroupId::Internal(family),
        display_name: family.display_name().to_string(),
        kind: ToolGroupKind::Internal,
        enabled: true,
        needs_auth: false,
        tool_names: vec!["read_note".to_string(), "write_note".to_string()],
        parallel_safe: false,
        last_error: None,
    }
}

/// An MCP group with one tool.
fn mcp_group(name: &str, kind: ToolGroupKind, needs_auth: bool) -> ToolGroupState {
    ToolGroupState {
        id: ToolGroupId::Mcp(name.to_string()),
        display_name: name.to_string(),
        kind,
        enabled: true,
        needs_auth,
        tool_names: vec![format!("{name}_tool")],
        parallel_safe: true,
        last_error: None,
    }
}

/// A deterministic per-tool schema-size lookup: the name's length.
fn name_len(name: &str) -> Option<usize> {
    Some(name.chars().count())
}

/// A lookup that reports nothing, mirroring an unknown tool.
fn unknown_size(_name: &str) -> Option<usize> {
    None
}

// --- group -> row mapping ---------------------------------------------

#[test]
fn row_carries_identity_kind_and_labels() {
    let group = internal_group(InternalToolGroup::Filesystem);
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);

    assert_eq!(row.id, ToolGroupId::Internal(InternalToolGroup::Filesystem));
    assert_eq!(row.display_name, "Filesystem");
    assert_eq!(row.kind, ToolGroupKind::Internal);
    assert_eq!(row.kind_label, strings::TOOLS_KIND_INTERNAL);
    assert!(row.enabled);
    assert_eq!(row.tools_label, "read_note, write_note");
    assert!(!row.has_error());
    assert_eq!(row.mcp_server(), None);
}

#[test]
fn kind_labels_cover_every_variant_from_strings() {
    assert_eq!(
        kind_label(ToolGroupKind::Internal),
        strings::TOOLS_KIND_INTERNAL
    );
    assert_eq!(
        kind_label(ToolGroupKind::McpStdio),
        strings::TOOLS_KIND_MCP_STDIO
    );
    assert_eq!(
        kind_label(ToolGroupKind::McpRemote),
        strings::TOOLS_KIND_MCP_REMOTE
    );
}

#[test]
fn prompt_char_count_uses_the_supplied_lookup_over_every_tool() {
    let group = internal_group(InternalToolGroup::Web);
    // "read_note"(9) + "write_note"(10)
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    assert_eq!(row.prompt_char_count, 19);
}

#[test]
fn unknown_tools_count_as_zero() {
    let group = internal_group(InternalToolGroup::Web);
    let row = row_from_group(&group, McpAuthFacts::default(), &unknown_size);
    assert_eq!(row.prompt_char_count, 0);
}

#[test]
fn a_group_with_no_tools_renders_an_empty_tools_label() {
    let group = ToolGroupState {
        tool_names: Vec::new(),
        ..mcp_group("empty-server", ToolGroupKind::McpStdio, false)
    };
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    assert_eq!(row.tools_label, "");
    assert_eq!(row.prompt_char_count, 0);
}

#[test]
fn error_maps_to_the_kind_colon_message_line() {
    let group = ToolGroupState {
        last_error: Some(ToolGroupError {
            kind: ToolErrorKind::Execution,
            message: "Permission denied".to_string(),
            occurred_at_unix_ms: 1_700_000_000_000,
        }),
        ..internal_group(InternalToolGroup::Filesystem)
    };
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);

    assert!(row.has_error());
    assert_eq!(row.error.as_deref(), Some("Execution: Permission denied"));
}

#[test]
fn error_line_text_ignores_the_timestamp() {
    let a = ToolGroupError {
        kind: ToolErrorKind::Discovery,
        message: "boom".to_string(),
        occurred_at_unix_ms: 1,
    };
    let b = ToolGroupError {
        occurred_at_unix_ms: 999_999,
        ..a.clone()
    };
    assert_eq!(error_line_text(&a), error_line_text(&b));
    assert_eq!(error_line_text(&a), "Discovery: boom");
}

#[test]
fn rows_from_groups_preserves_the_registry_order() {
    let groups = vec![
        internal_group(InternalToolGroup::Filesystem),
        internal_group(InternalToolGroup::Web),
        mcp_group("alpha", ToolGroupKind::McpStdio, false),
    ];
    let rows = rows_from_groups(&groups, &HashMap::new(), &name_len);
    let names: Vec<&str> = rows.iter().map(|row| row.display_name.as_str()).collect();
    assert_eq!(names, vec!["Filesystem", "Web", "alpha"]);
}

#[test]
fn rows_from_an_empty_group_list_is_empty() {
    let rows = rows_from_groups(&[], &HashMap::new(), &name_len);
    assert!(rows.is_empty());
    let view_model = ToolsDialogViewModel::from_groups(&[], &HashMap::new(), &name_len);
    assert!(view_model.is_empty());
    assert_eq!(view_model.count(), 0);
}

// --- auth-state mapping -----------------------------------------------

#[test]
fn internal_groups_never_get_an_auth_state() {
    let group = internal_group(InternalToolGroup::Email);
    assert_eq!(
        auth_state_for(&group, McpAuthFacts::default()),
        ToolAuthState::NotApplicable
    );
    // Even a bogus `needs_auth` on an internal group stays inert.
    let flagged = ToolGroupState {
        needs_auth: true,
        ..internal_group(InternalToolGroup::Email)
    };
    assert_eq!(
        auth_state_for(&flagged, McpAuthFacts::default()),
        ToolAuthState::NotApplicable
    );
}

#[test]
fn mcp_group_without_observed_auth_is_not_required() {
    let group = mcp_group("srv", ToolGroupKind::McpRemote, false);
    assert_eq!(
        auth_state_for(&group, McpAuthFacts::default()),
        ToolAuthState::NotRequired
    );
    assert_eq!(
        auth_state_for(
            &group,
            McpAuthFacts {
                eligible: true,
                in_progress: false
            }
        ),
        ToolAuthState::NotRequired,
        "eligibility alone must not show controls the user did not earn"
    );
}

#[test]
fn observed_auth_on_an_ineligible_server_is_not_eligible() {
    let group = mcp_group("srv", ToolGroupKind::McpStdio, true);
    assert_eq!(
        auth_state_for(&group, McpAuthFacts::default()),
        ToolAuthState::NotEligible
    );
}

#[test]
fn observed_auth_on_an_eligible_idle_server_is_available() {
    let group = mcp_group("srv", ToolGroupKind::McpRemote, true);
    assert_eq!(
        auth_state_for(
            &group,
            McpAuthFacts {
                eligible: true,
                in_progress: false
            }
        ),
        ToolAuthState::Available
    );
}

#[test]
fn observed_auth_on_an_eligible_running_server_is_in_progress() {
    let group = mcp_group("srv", ToolGroupKind::McpRemote, true);
    assert_eq!(
        auth_state_for(
            &group,
            McpAuthFacts {
                eligible: true,
                in_progress: true
            }
        ),
        ToolAuthState::InProgress
    );
}

#[test]
fn auth_state_labels_and_control_visibility() {
    assert!(!ToolAuthState::NotApplicable.shows_controls());
    assert!(!ToolAuthState::NotRequired.shows_controls());
    assert!(!ToolAuthState::NotEligible.shows_controls());
    assert!(ToolAuthState::Available.shows_controls());
    assert!(ToolAuthState::InProgress.shows_controls());

    assert_eq!(ToolAuthState::Available.label(), strings::TOOLS_AUTH_BUTTON);
    assert_eq!(
        ToolAuthState::InProgress.label(),
        strings::TOOLS_AUTH_RUNNING
    );
    assert_eq!(ToolAuthState::NotApplicable.label(), "");
    assert!(ToolAuthState::Available.can_authenticate());
    assert!(!ToolAuthState::InProgress.can_authenticate());
}

#[test]
fn a_missing_server_entry_falls_back_to_no_facts() {
    // Config entry removed while the group still reports needs_auth.
    let group = mcp_group("gone", ToolGroupKind::McpRemote, true);
    let rows = rows_from_groups(&[group], &HashMap::new(), &name_len);
    assert_eq!(rows[0].auth_state, ToolAuthState::NotEligible);
}

#[test]
fn facts_are_looked_up_by_server_name_only() {
    let groups = vec![
        mcp_group("alpha", ToolGroupKind::McpRemote, true),
        mcp_group("beta", ToolGroupKind::McpRemote, true),
    ];
    let mut facts = HashMap::new();
    facts.insert(
        "alpha".to_string(),
        McpAuthFacts {
            eligible: true,
            in_progress: false,
        },
    );
    facts.insert(
        "beta".to_string(),
        McpAuthFacts {
            eligible: false,
            in_progress: false,
        },
    );

    let rows = rows_from_groups(&groups, &facts, &name_len);
    assert_eq!(rows[0].auth_state, ToolAuthState::Available);
    assert_eq!(rows[1].auth_state, ToolAuthState::NotEligible);
}

// --- action -> command mapping ----------------------------------------

#[test]
fn a_clean_internal_row_has_no_actions() {
    let group = internal_group(InternalToolGroup::Weather);
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    assert!(row_actions(&row).is_empty());
}

#[test]
fn an_error_row_offers_restart_with_clear_tool_group_error() {
    let group = ToolGroupState {
        last_error: Some(ToolGroupError {
            kind: ToolErrorKind::ConfigInvalid,
            message: "bad yaml".to_string(),
            occurred_at_unix_ms: 0,
        }),
        ..internal_group(InternalToolGroup::CsvDb)
    };
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    let actions = row_actions(&row);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].label, strings::TOOLS_RESTART);
    assert!(actions[0].is_enabled());
    assert_eq!(
        actions[0].press,
        Some(Message::Command(UserCommand::ClearToolGroupError(
            ToolGroupId::Internal(InternalToolGroup::CsvDb)
        )))
    );
}

#[test]
fn an_available_mcp_row_offers_authenticate_then_forget() {
    let group = mcp_group("github", ToolGroupKind::McpRemote, true);
    let row = row_from_group(
        &group,
        McpAuthFacts {
            eligible: true,
            in_progress: false,
        },
        &name_len,
    );
    let actions = row_actions(&row);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].label, strings::TOOLS_AUTH_BUTTON);
    assert!(actions[0].is_enabled());
    assert_eq!(
        actions[0].press,
        Some(Message::Command(UserCommand::StartMcpAuth(
            "github".to_string()
        )))
    );
    assert_eq!(actions[1].label, strings::TOOLS_FORGET);
    assert_eq!(
        actions[1].press,
        Some(Message::Command(UserCommand::ForgetMcpAuth(
            "github".to_string()
        )))
    );
}

#[test]
fn an_in_progress_mcp_row_shows_an_inert_authenticating_button() {
    let group = mcp_group("github", ToolGroupKind::McpRemote, true);
    let row = row_from_group(
        &group,
        McpAuthFacts {
            eligible: true,
            in_progress: true,
        },
        &name_len,
    );
    let actions = row_actions(&row);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].label, strings::TOOLS_AUTH_RUNNING);
    assert!(
        !actions[0].is_enabled(),
        "the running flow must not accept a second click"
    );
    assert_eq!(actions[1].label, strings::TOOLS_FORGET);
    assert!(actions[1].is_enabled());
}

#[test]
fn an_error_and_auth_row_stacks_restart_before_the_auth_actions() {
    let group = ToolGroupState {
        last_error: Some(ToolGroupError {
            kind: ToolErrorKind::Authentication,
            message: "401".to_string(),
            occurred_at_unix_ms: 0,
        }),
        ..mcp_group("github", ToolGroupKind::McpRemote, true)
    };
    let row = row_from_group(
        &group,
        McpAuthFacts {
            eligible: true,
            in_progress: false,
        },
        &name_len,
    );
    let labels: Vec<&str> = row_actions(&row)
        .iter()
        .map(|action| action.label)
        .collect();
    assert_eq!(
        labels,
        vec![
            strings::TOOLS_RESTART,
            strings::TOOLS_AUTH_BUTTON,
            strings::TOOLS_FORGET
        ]
    );
}

#[test]
fn command_helpers_carry_their_own_target() {
    let id = ToolGroupId::Mcp("srv".to_string());
    assert_eq!(
        clear_error_command(&id),
        Message::Command(UserCommand::ClearToolGroupError(id.clone()))
    );
    assert_eq!(
        set_enabled_command(&id, false),
        Message::Command(UserCommand::SetToolGroupEnabled {
            id: id.clone(),
            enabled: false
        })
    );
    assert_eq!(
        start_auth_command("srv"),
        Message::Command(UserCommand::StartMcpAuth("srv".to_string()))
    );
    assert_eq!(
        forget_auth_command("srv"),
        Message::Command(UserCommand::ForgetMcpAuth("srv".to_string()))
    );
}

#[test]
fn every_row_action_rides_the_command_channel() {
    // [FMD-068]: the tools dialog has no private intent vocabulary —
    // each action is already a self-contained UserCommand.
    let group = ToolGroupState {
        last_error: Some(ToolGroupError {
            kind: ToolErrorKind::Execution,
            message: "boom".to_string(),
            occurred_at_unix_ms: 0,
        }),
        ..mcp_group("github", ToolGroupKind::McpRemote, true)
    };
    let row = row_from_group(
        &group,
        McpAuthFacts {
            eligible: true,
            in_progress: false,
        },
        &name_len,
    );
    for action in row_actions(&row) {
        assert!(
            matches!(action.press, Some(Message::Command(_))),
            "{:?} must ride the command channel",
            action.label
        );
    }
}

#[test]
fn the_enable_checkbox_emits_set_tool_group_enabled() {
    let group = internal_group(InternalToolGroup::Browser);
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    assert_eq!(
        set_enabled_command(&row.id, !row.enabled),
        Message::Command(UserCommand::SetToolGroupEnabled {
            id: ToolGroupId::Internal(InternalToolGroup::Browser),
            enabled: false
        })
    );
}

// --- error rendering tokens --------------------------------------------

#[test]
fn the_error_marker_uses_the_warning_token_and_the_text_the_error_token() {
    // the legacy UI painted the marker with `palette.warning`
    // (src/app/ui/tools_dialog.rs:255); the inline text uses the
    // stronger `error` token because iced has no tooltip to hide it in.
    assert_eq!(error_marker_color(), Palette::fastmd_dark().warning);
    assert_eq!(error_text_color(), Palette::fastmd_dark().error);
    assert_ne!(
        error_marker_color(),
        error_text_color(),
        "the marker and the message must stay visually distinct"
    );
}

#[test]
fn row_and_muted_tokens_come_from_the_shared_palette() {
    assert_eq!(row_label_color(), Palette::fastmd_dark().text_strong);
    assert_eq!(muted_color(), Palette::fastmd_dark().text_muted);
}

// --- view model --------------------------------------------------------

#[test]
fn summary_counts_the_groups() {
    let view_model = ToolsDialogViewModel::with_rows(vec![
        row_from_group(
            &internal_group(InternalToolGroup::Filesystem),
            McpAuthFacts::default(),
            &name_len,
        ),
        row_from_group(
            &mcp_group("srv", ToolGroupKind::McpStdio, false),
            McpAuthFacts::default(),
            &name_len,
        ),
    ]);
    assert_eq!(view_model.count(), 2);
    assert!(!view_model.is_empty());
    assert!(view_model.summary().starts_with('2'));
}

#[test]
fn an_empty_view_model_summarises_with_the_empty_state_line() {
    let view_model = ToolsDialogViewModel::empty();
    assert_eq!(view_model.summary(), strings::TOOLS_EMPTY_STATE);
}

#[test]
fn card_and_table_build_for_a_populated_and_an_empty_model() {
    let populated = ToolsDialogViewModel::with_rows(vec![
        row_from_group(
            &internal_group(InternalToolGroup::Filesystem),
            McpAuthFacts::default(),
            &name_len,
        ),
        row_from_group(
            &mcp_group("srv", ToolGroupKind::McpRemote, true),
            McpAuthFacts {
                eligible: true,
                in_progress: false,
            },
            &name_len,
        ),
    ]);
    assert!(std::mem::size_of_val(&tools_dialog_card(&populated)) > 0);
    assert!(std::mem::size_of_val(&tools_table(populated.rows())) > 0);

    let empty = ToolsDialogViewModel::empty();
    assert!(std::mem::size_of_val(&tools_dialog_card(&empty)) > 0);
    // The header still renders with zero rows, so no extra widget is needed.
    assert!(std::mem::size_of_val(&tools_table(empty.rows())) > 0);
}

// --- height rule (mirrors the legacy UI compute_dialog_size tests) ----------

#[test]
fn dialog_size_fits_few_rows_on_a_large_screen() {
    let viewport = Size::new(1920.0, 1080.0);
    // 7 groups: 56 + 20 + 7*24 + 8 = 252.
    let (default_size, min_size, max_height) = compute_dialog_size(viewport, 7);
    assert!(
        default_size.height >= 250.0,
        "default height must be large enough to fit 7 rows; got {}",
        default_size.height
    );
    assert!(
        (max_height - 918.0).abs() < 1.0,
        "max_height should be 918 on a 1080-tall screen; got {max_height}"
    );
    assert!(min_size.height <= default_size.height);
}

#[test]
fn dialog_size_fits_many_rows_when_the_screen_has_room() {
    let viewport = Size::new(1920.0, 1080.0);
    // 30 rows: 56 + 20 + 30*24 + 8 = 804, inside the 918 cap.
    let (default_size, _min_size, max_height) = compute_dialog_size(viewport, 30);
    assert!(
        default_size.height >= 804.0,
        "default height must fit all 30 rows when the screen has room; got {}",
        default_size.height
    );
    assert!(max_height >= default_size.height);
}

#[test]
fn dialog_size_caps_on_a_small_screen() {
    let viewport = Size::new(800.0, 400.0);
    let (_default_size, _min_size, max_height) = compute_dialog_size(viewport, 100);
    // 400 * 0.85 = 340, so 100 rows cannot overflow the screen.
    assert!(max_height < 400.0, "max_height must be < screen height");
    assert!(max_height > MIN_HEIGHT, "max_height must stay usable");
}

#[test]
fn dialog_size_never_goes_below_the_minimum() {
    let viewport = Size::new(800.0, 100.0);
    let (default_size, min_size, max_height) = compute_dialog_size(viewport, 0);
    assert!(default_size.height >= MIN_HEIGHT);
    assert!(min_size.height <= default_size.height);
    assert!(max_height >= MIN_HEIGHT);
}

#[test]
fn the_hard_cap_bounds_a_giant_viewport() {
    let viewport = Size::new(3840.0, 2160.0);
    let (_default, _min, max_height) = compute_dialog_size(viewport, 500);
    assert!(
        (max_height - MAX_HEIGHT_HARD_CAP).abs() < 1.0,
        "a 4K viewport must still cap at the hard limit; got {max_height}"
    );
}

#[test]
fn view_model_size_for_uses_its_own_row_count() {
    let view_model = ToolsDialogViewModel::with_rows(vec![row_from_group(
        &internal_group(InternalToolGroup::Trello),
        McpAuthFacts::default(),
        &name_len,
    )]);
    let viewport = Size::new(1920.0, 1080.0);
    assert_eq!(
        view_model.size_for(viewport),
        compute_dialog_size(viewport, 1)
    );
}

// --- window state transitions ------------------------------------------

#[test]
fn tools_state_starts_closed() {
    let state = ToolsDialogState::new();
    assert!(!state.is_open());
}

#[test]
fn open_reports_the_transition() {
    let mut state = ToolsDialogState::new();
    assert!(state.open());
    assert!(state.is_open());
    assert!(
        !state.open(),
        "a redundant open must report no visible change"
    );
}

#[test]
fn close_reports_whether_a_dialog_was_consumed() {
    let mut state = ToolsDialogState::new();
    assert!(!state.close(), "closing a closed dialog consumes nothing");
    state.open();
    assert!(state.close());
    assert!(!state.is_open());
    assert!(!state.close());
}

#[test]
fn toggle_flips_the_visible_state() {
    let mut state = ToolsDialogState::new();
    assert!(state.toggle());
    assert!(state.is_open());
    assert!(!state.toggle());
    assert!(!state.is_open());
}

#[test]
fn apply_close_requested_closes_and_reports_the_change() {
    let mut state = ToolsDialogState::new();
    state.open();
    assert!(state.apply(ToolsDialogMessage::CloseRequested));
    assert!(!state.is_open());
    assert!(
        !state.apply(ToolsDialogMessage::CloseRequested),
        "already closed: nothing consumed"
    );
}

#[test]
fn dismiss_message_is_the_tools_channel_not_a_command() {
    // The dismiss has no UserCommand equivalent (see the module header),
    // so it must not be confused with the command channel.
    assert_eq!(
        dismiss_message(),
        Message::Tools(ToolsDialogMessage::CloseRequested)
    );
    assert!(!matches!(dismiss_message(), Message::Command(_)));
}

// --- app snapshot smoke test --------------------------------------------

#[test]
fn from_app_snapshots_the_registered_internal_groups() {
    // `empty_state` is the in-memory app fixture the legacy UI dialog tests
    // already used; no user paths are read ([RUST-006]).
    let app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let view_model = ToolsDialogViewModel::from_app(&app);

    assert!(
        !view_model.is_empty(),
        "the internal tool families are registered on every startup"
    );
    assert!(
        view_model
            .rows()
            .iter()
            .any(|row| row.display_name == "Filesystem"),
        "the Filesystem family must appear"
    );
    assert!(
        view_model
            .rows()
            .iter()
            .all(|row| row.auth_state == ToolAuthState::NotApplicable),
        "internal groups never authenticate"
    );
}

#[test]
fn from_app_marks_a_stdio_server_ineligible_for_oauth() {
    let mut app_config = crate::config::AppConfig::default();
    app_config.mcp_servers.insert(
        "local-stdio".into(),
        crate::config::McpServerEntry {
            enabled: true,
            config: crate::config::McpServerConfig::Stdio {
                command: "echo".into(),
                args: vec![],
                env: Default::default(),
            },
        },
    );
    let app = crate::ui::FastMdApp::empty_state(app_config);
    let view_model = ToolsDialogViewModel::from_app(&app);

    let row = view_model
        .rows()
        .iter()
        .find(|row| row.display_name == "local-stdio")
        .expect("the configured MCP server must get a row");
    assert_eq!(row.kind, ToolGroupKind::McpStdio);
    assert_eq!(row.kind_label, strings::TOOLS_KIND_MCP_STDIO);
    assert!(!row.auth_state.shows_controls());
    assert!(row_actions(row).is_empty());
}
