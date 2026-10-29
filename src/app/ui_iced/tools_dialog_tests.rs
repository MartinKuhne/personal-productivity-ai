//! Sidecar unit tests for `ui_iced/tools_dialog.rs`.
//!
//! Pure mapping, state-transition and widget-construction tests
//! ([RUST-003]). Every group is a synthetic [`ToolGroupState`] literal, so
//! no registry, subprocess, network or filesystem is touched
//! ([RUST-006]); the only app-level read is one `FastMdApp::empty_state`
//! smoke test.
//!
//! The `tools_card_*` / defect #4 cases pin the card's viewport-fill
//! policy by measurement — the real mount laid out headless at
//! 1200x800, 1600x1000 and a stubby window, plus the inner list and
//! title band spanning the card — and the `modal_error` cases pin
//! `UIF-MOD-003`'s auth-failure banner and its `Recovery`.

use super::*;
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use hoarfrost::surface::modal::{TOOLS_HEIGHT_MIN, TOOLS_WIDTH, VIEWPORT_GUTTER};
use hoarfrost::widget::badge_model::BadgeKind;
use iced::{Rectangle, Size};
use iced_test::selector::Candidate;

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

/// `n` synthetic group rows, so count-driven text can be tested at any size.
fn rows_n(n: usize) -> Vec<ToolGroupRow> {
    (0..n)
        .map(|index| {
            row_from_group(
                &internal_group(match index % 3 {
                    0 => InternalToolGroup::Filesystem,
                    1 => InternalToolGroup::Web,
                    _ => InternalToolGroup::Weather,
                }),
                McpAuthFacts::default(),
                &name_len,
            )
        })
        .collect()
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
    assert!(!row.parallel_safe);
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

// --- parallel-safe chip (UI-058) ---------------------------------------

#[test]
fn row_from_group_carries_parallel_safe_through_in_both_directions() {
    let unsafe_group = ToolGroupState {
        parallel_safe: false,
        ..mcp_group("serial", ToolGroupKind::McpRemote, false)
    };
    let safe_group = ToolGroupState {
        parallel_safe: true,
        ..internal_group(InternalToolGroup::Filesystem)
    };

    let serial = row_from_group(&unsafe_group, McpAuthFacts::default(), &name_len);
    let parallel = row_from_group(&safe_group, McpAuthFacts::default(), &name_len);

    assert!(
        !serial.parallel_safe,
        "a `false` group must not gain the flag on the way to the row"
    );
    assert!(
        parallel.parallel_safe,
        "a `true` group must not lose the flag on the way to the row"
    );
}

#[test]
fn the_row_fixtures_cover_both_parallel_safe_values() {
    // The mapping tests lean on these helpers; pinning both values here
    // keeps a future edit from collapsing the pair into one case.
    assert!(!internal_group(InternalToolGroup::Web).parallel_safe);
    assert!(mcp_group("srv", ToolGroupKind::McpStdio, false).parallel_safe);
}

#[test]
fn a_parallel_safe_row_gets_the_chip_label_from_strings() {
    let row = row_from_group(
        &ToolGroupState {
            parallel_safe: true,
            ..internal_group(InternalToolGroup::Filesystem)
        },
        McpAuthFacts::default(),
        &name_len,
    );

    assert_eq!(
        parallel_chip_label(&row),
        Some(strings::TOOLS_PARALLEL_CHIP),
        "the chip text must come from `ui::strings`, never a panel literal"
    );
    assert_eq!(parallel_chip_label(&row), Some("✓ parallel"));
}

#[test]
fn a_row_that_is_not_parallel_safe_gets_no_chip_at_all() {
    let row = row_from_group(
        &internal_group(InternalToolGroup::Filesystem),
        McpAuthFacts::default(),
        &name_len,
    );

    assert_eq!(
        parallel_chip_label(&row),
        None,
        "UI-058: the chip's absence is not a negative signal, so no \
         \"not parallel\" label may appear"
    );
}

#[test]
fn the_char_count_cell_builds_with_and_without_the_chip() {
    let safe = row_from_group(
        &ToolGroupState {
            parallel_safe: true,
            ..mcp_group("srv", ToolGroupKind::McpRemote, false)
        },
        McpAuthFacts::default(),
        &name_len,
    );
    let not_safe = row_from_group(
        &internal_group(InternalToolGroup::Filesystem),
        McpAuthFacts::default(),
        &name_len,
    );

    assert!(std::mem::size_of_val(&char_count_cell(&safe)) > 0);
    assert!(std::mem::size_of_val(&char_count_cell(&not_safe)) > 0);
}

// --- error tooltip (UI-057) --------------------------------------------

#[test]
fn the_error_tooltip_body_is_the_kind_colon_message_line() {
    let group = ToolGroupState {
        last_error: Some(ToolGroupError {
            kind: ToolErrorKind::Execution,
            message: "Permission denied".to_string(),
            occurred_at_unix_ms: 1_700_000_000_000,
        }),
        ..internal_group(InternalToolGroup::Filesystem)
    };
    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);

    assert_eq!(
        error_tooltip_body(&row),
        Some("Execution: Permission denied"),
        "the tooltip body is the already-derived `Kind: message` line"
    );
    assert_eq!(error_tooltip_body(&row), row.error.as_deref());
}

#[test]
fn a_clean_row_has_no_tooltip_body() {
    let row = row_from_group(
        &internal_group(InternalToolGroup::Web),
        McpAuthFacts::default(),
        &name_len,
    );
    assert_eq!(error_tooltip_body(&row), None);
}

#[test]
fn the_actions_cell_builds_with_and_without_an_error_marker() {
    let errored = row_from_group(
        &ToolGroupState {
            last_error: Some(ToolGroupError {
                kind: ToolErrorKind::Authentication,
                message: "401".to_string(),
                occurred_at_unix_ms: 0,
            }),
            ..mcp_group("github", ToolGroupKind::McpRemote, true)
        },
        McpAuthFacts::default(),
        &name_len,
    );
    let clean = row_from_group(
        &internal_group(InternalToolGroup::Weather),
        McpAuthFacts::default(),
        &name_len,
    );

    assert!(std::mem::size_of_val(&actions_cell(&errored)) > 0);
    assert!(std::mem::size_of_val(&actions_cell(&clean)) > 0);
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
    // The tools dialog has no private intent vocabulary —
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
    // The marker uses `palette.warning` and the tooltip body uses the
    // stronger `error` token. iced 0.14 ships
    // `iced::widget::tooltip::Tooltip`, so the body is revealed on hover
    // rather than shown inline.
    assert_eq!(error_marker_color(), Palette::fastmd_dark().warning);
    assert_eq!(error_text_color(), Palette::fastmd_dark().error);
    assert_ne!(
        error_marker_color(),
        error_text_color(),
        "the marker and the message must stay visually distinct"
    );
}

/// **This pin moved, and here is why.** It used to assert that the marker
/// read `Palette::success` and `Palette::surface_variant` directly through
/// two card-local helpers. Those helpers are retired: the marker is now a
/// `UIF-ATOM-008` status badge, so its text and fill come out of
/// `badge::resolve_tone` / `badge::resolve_style` and a card-local colour
/// getter would be a second source of truth for one pill's paint.
///
/// What the pin still guards is unchanged and restated against the real
/// source: the marker reads as *success*, and it must never read as an
/// error.
#[test]
fn the_parallel_marker_tokens_come_from_the_badge_tone_resolver() {
    let palette = Palette::pro();
    let model = BadgeModel::status(StatusIcon::Success, strings::TOOLS_PARALLEL_CHIP)
        .with_tone(BadgeTone::Success);
    let style = badge::resolve_style(&model, &palette);
    let tone = badge::resolve_tone(BadgeTone::Success, &palette);

    assert_eq!(style.text, tone.text);
    assert_ne!(
        style.text,
        badge::resolve_tone(BadgeTone::Danger, &palette).text,
        "the parallel-safe marker must not read as an error badge"
    );

    let danger = badge::resolve_tone(BadgeTone::Danger, &palette);
    assert_ne!(
        style.fill, danger.dot_fill,
        "the marker's surface must not read as an error badge either"
    );
    assert_ne!(style.fill, danger.solid_fill, "and not a solid error pill");
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

/// Defect #5 — the summary line read `"10 Group"`.
///
/// It was built from [`strings::TOOLS_GROUP_COLUMN`], which is the table's
/// *column header*, not a pluralisable noun. The count now rides
/// [`strings::tools_group_count`], which pluralises.
#[test]
fn summary_pluralises_the_group_count() {
    assert_eq!(
        ToolsDialogViewModel::with_rows(rows_n(1)).summary(),
        "1 group",
        "one group must read as a single group, not `1 Group`"
    );
    assert_eq!(
        ToolsDialogViewModel::with_rows(rows_n(10)).summary(),
        "10 groups",
        "defect #5: ten groups must read `10 groups`, not `10 Group`"
    );
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
    assert!(std::mem::size_of_val(&tools_dialog_card(&populated, ROOMY)) > 0);
    assert!(std::mem::size_of_val(&tools_table(populated.rows())) > 0);

    let empty = ToolsDialogViewModel::empty();
    assert!(std::mem::size_of_val(&tools_dialog_card(&empty, ROOMY)) > 0);
    // The header still renders with zero rows, so no extra widget is needed.
    assert!(std::mem::size_of_val(&tools_table(empty.rows())) > 0);
}

// --- §4.6.2 sizing / defect #4: 80% of the viewport -----------------------
//
// The card used to be a 720px block with a content-fit height, which left
// most of a 1200px window empty — defect #4, "the Tools dialog doesn't
// use the real estate". The owner's decision is *"#4 use 80% of the
// viewport"*, so the card is now [`VIEWPORT_FILL`] of the window on both
// axes. Every measured case below lays the **real mount** out headless
// through [`tools_modal`] — the same element `view::overlay_element`
// stacks — and asserts the card's own bounds, so the policy is pinned by
// geometry rather than by reading a number back out of the policy object.

/// A roomy viewport, so the unclamped branch is what is being measured.
const ROOMY: Size = Size::new(1920.0, 1080.0);

/// Lays the real modal mount out headless in a window of `viewport` and
/// returns the measured bounds of the widget registered under `id`.
fn mounted_bounds(
    id: &'static str,
    view_model: &ToolsDialogViewModel,
    viewport: Size,
) -> Rectangle {
    let mut ui = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        tools_modal(view_model, viewport),
    );
    let bounds = ui
        .find(iced::widget::Id::from(id))
        .unwrap_or_else(|err| panic!("widget {id:?} should be reachable: {err:?}"))
        .bounds();
    drop(ui);
    bounds
}

/// The measured card box, as mounted and centred.
fn mounted_card_bounds(view_model: &ToolsDialogViewModel, viewport: Size) -> Rectangle {
    mounted_bounds(TOOLS_CARD_ID, view_model, viewport)
}

/// The measured scrollable group list inside the mounted card.
fn mounted_list_bounds(view_model: &ToolsDialogViewModel, viewport: Size) -> Rectangle {
    mounted_bounds(TOOLS_LIST_ID, view_model, viewport)
}

/// A view model over `n` synthetic groups.
fn model_with_rows(n: usize) -> ToolsDialogViewModel {
    ToolsDialogViewModel::with_rows(rows_n(n))
}

/// The width a card of `card_width` leaves its content, once the frame's
/// padding is spent.
fn inner_width(card_width: f32) -> f32 {
    card_width - 2.0 * modal_frame().content_padding
}

/// The ratio is a named policy constant driving **both** axes, not a
/// literal sprinkled through the card.
#[test]
fn the_fill_ratio_is_a_named_constant_on_both_axes() {
    assert_eq!(
        VIEWPORT_FILL, 0.8,
        "defect #4: the card fills 80% of the viewport, and the ratio is named, not inlined"
    );
    assert_eq!(
        TOOLS_SIZING.width.desired,
        Desired::Viewport(VIEWPORT_FILL),
        "the width must be viewport-driven, not the retired fixed 720"
    );
    assert_eq!(
        TOOLS_SIZING.height.desired,
        Desired::Viewport(VIEWPORT_FILL),
        "the height must be viewport-driven, not content-fit"
    );
}

/// The width is 80% of the window until the framework's gutter — applied
/// last, and the one thing that outranks the floor — has to take over.
#[test]
fn card_width_is_eighty_percent_until_the_gutter_clamps() {
    assert_eq!(tools_card_width(1200.0), 960.0);
    assert_eq!(tools_card_width(1600.0), 1280.0);
    assert_eq!(tools_card_width(1920.0), 1536.0);
    assert_ne!(
        tools_card_width(1920.0),
        TOOLS_WIDTH,
        "defect #4: the retired 720px block must not come back"
    );
    // Below 160px wide, 80% is wider than `viewport - VIEWPORT_GUTTER`.
    assert_eq!(tools_card_width(120.0), 120.0 - VIEWPORT_GUTTER);
}

/// A 1200x800 window gets a 960x640 card, centred.
#[test]
fn a_1200x800_viewport_measures_a_960x640_card() {
    let viewport = Size::new(1200.0, 800.0);
    let bounds = mounted_card_bounds(&model_with_rows(9), viewport);

    assert_eq!(
        bounds.size(),
        Size::new(960.0, 640.0),
        "defect #4: 80% of a 1200x800 viewport is 960x640"
    );
    assert_eq!(bounds.x, 120.0, "the card must stay centred horizontally");
    assert_eq!(bounds.y, 80.0, "the card must stay centred vertically");
}

/// A 1600x1000 window gets a 1280x800 card, centred.
#[test]
fn a_1600x1000_viewport_measures_a_1280x800_card() {
    let viewport = Size::new(1600.0, 1000.0);
    let bounds = mounted_card_bounds(&model_with_rows(9), viewport);

    assert_eq!(
        bounds.size(),
        Size::new(1280.0, 800.0),
        "defect #4: 80% of a 1600x1000 viewport is 1280x800"
    );
    assert_eq!(bounds.x, 160.0, "the card must stay centred horizontally");
    assert_eq!(bounds.y, 100.0, "the card must stay centred vertically");
}

/// The list spans the card's inner width instead of sitting inside it as
/// the old 720px block — the second half of "use the real estate".
#[test]
fn the_group_list_spans_the_card_instead_of_a_720_block_inside_it() {
    let viewport = Size::new(1200.0, 800.0);
    let view_model = model_with_rows(9);
    let card = mounted_card_bounds(&view_model, viewport);
    let list = mounted_list_bounds(&view_model, viewport);

    assert_eq!(
        list.width,
        inner_width(card.width),
        "defect #4: the list must span the card's inner width, not a stale fixed block \
         (list={:.1}, inner={:.1})",
        list.width,
        inner_width(card.width)
    );
    assert!(
        list.width > inner_width(TOOLS_WIDTH),
        "defect #4: the list must be wider than the retired 720px card ever gave it \
         (list={:.1}, retired inner={:.1})",
        list.width,
        inner_width(TOOLS_WIDTH)
    );
}

/// The title band rides the card's trailing edge as well: the `×` sits at
/// the card's inner right, and that edge is further right than the retired
/// 720px block ever reached — otherwise the band would still be parked in
/// the middle of the card's new real estate.
#[test]
fn the_title_band_rides_the_cards_trailing_edge() {
    let viewport = Size::new(1200.0, 800.0);
    let view_model = model_with_rows(9);
    let card = mounted_card_bounds(&view_model, viewport);
    let close = mounted_bounds(TOOLS_CLOSE_BUTTON_ID, &view_model, viewport);
    let inner_right = card.x + card.width - modal_frame().content_padding;
    // Where the `×` would have landed on the retired 720px card, centred
    // in this same window.
    let retired_inner_right =
        (viewport.width - TOOLS_WIDTH) / 2.0 + TOOLS_WIDTH - modal_frame().content_padding;

    assert_eq!(
        close.x + close.width,
        inner_right,
        "defect #4: the `×` must sit at the card's inner right edge (close={:.1}, \
         inner right={:.1})",
        close.x + close.width,
        inner_right
    );
    assert!(
        close.x + close.width > retired_inner_right,
        "defect #4: the `×` must ride the new card edge, not the retired 720px one \
         (close right={:.1}, retired inner right={:.1})",
        close.x + close.width,
        retired_inner_right
    );
}

/// A short list expands into the vertical slack the card gained instead of
/// collapsing back to the height its two rows need.
#[test]
fn the_group_list_expands_into_the_vertical_slack_it_was_given() {
    let viewport = Size::new(1200.0, 800.0);
    let view_model = model_with_rows(2);
    let list = mounted_list_bounds(&view_model, viewport);
    let content = tools_content_height(2, false);

    assert!(
        list.height > content,
        "defect #4: with two groups the list must take the space the card gained rather \
         than shrink to its content (list={:.1}, content={:.1})",
        list.height,
        content
    );
}

/// 80% of a tiny viewport must not overflow or clip: the gutter clamp is
/// applied after the floor, so the window always wins.
#[test]
fn eighty_percent_of_a_tiny_viewport_still_fits_the_window() {
    let viewport = Size::new(200.0, 120.0);
    let bounds = mounted_card_bounds(&ToolsDialogViewModel::empty(), viewport);

    assert!(
        bounds.width <= viewport.width - VIEWPORT_GUTTER,
        "the gutter must keep the card inside a tiny window (width={:.1})",
        bounds.width
    );
    assert!(
        bounds.height <= viewport.height - VIEWPORT_GUTTER,
        "the height floor must never push the card past the window edge (height={:.1})",
        bounds.height
    );
    assert!(
        bounds.x >= 0.0 && bounds.y >= 0.0,
        "the card must not be placed off-screen ({:.1}, {:.1})",
        bounds.x,
        bounds.y
    );
    assert!(
        bounds.x + bounds.width <= viewport.width && bounds.y + bounds.height <= viewport.height,
        "the card must not extend past the window ({:.1}x{:.1} at {:.1},{:.1})",
        bounds.width,
        bounds.height,
        bounds.x,
        bounds.y
    );
}

/// Seven groups on a 1080p screen: the content only wants 301px, but the
/// card is 80% of the screen (864px) and the list takes the slack.
///
/// This is where the old "few rows sit on the floor" pin moved. The card
/// no longer shrinks to its content: defect #4 made the viewport the
/// authority, so the same 864px box is handed to two groups and to
/// twenty.
#[test]
fn few_rows_still_get_the_whole_viewport_fill() {
    let viewport = Size::new(1920.0, 1080.0);
    let content = tools_content_height(7, false);
    assert_eq!(content, 301.0, "24 + 20 + 1 + 32*8");

    let height = tools_card_height(viewport);
    assert_eq!(height, 1080.0 * VIEWPORT_FILL);
    assert!(
        height > content,
        "the card must be taller than its content so the list owns the slack \
         (card={height}, content={content})"
    );
}

/// Thirty groups on a 1080p screen: the content wants 1037px and the
/// **80%-of-viewport cap** holds the card at 864, so the list scrolls.
///
/// The old pin said 804 fits inside a 918 cap (0.85 of the viewport).
/// The framework caps at 0.80, so the card is 40px shorter and the
/// remaining rows scroll. The cap moved from the app's 0.85 to the
/// spec's 0.80.
#[test]
fn many_rows_are_capped_at_eighty_percent_of_the_viewport() {
    let viewport = Size::new(1920.0, 1080.0);
    assert_eq!(tools_content_height(30, false), 1037.0);

    let height = tools_card_height(viewport);
    assert_eq!(height, 1080.0 * 0.8);
    assert!(height < 918.0, "the retired 0.85 cap must not return");
}

/// A small screen with a hundred groups: the 320 floor and the 80% fill
/// agree at 320, and the gutter leaves room for it.
#[test]
fn a_small_screen_still_holds_the_floor() {
    let viewport = Size::new(800.0, 400.0);
    let height = tools_card_height(viewport);
    assert_eq!(height, TOOLS_HEIGHT_MIN);
    assert!(height < viewport.height - VIEWPORT_GUTTER);
}

/// A viewport shorter than the floor: the **gutter beats the floor**, the
/// resolution order the framework documents.
///
/// The old rule clamped to `MIN_HEIGHT` and would have handed back 200 on
/// a 100px screen — a card taller than the window. The framework takes
/// `viewport - VIEWPORT_GUTTER` = 68 instead, so the card always fits.
#[test]
fn the_gutter_beats_the_floor_on_a_stubby_viewport() {
    let viewport = Size::new(800.0, 100.0);
    let height = tools_card_height(viewport);
    assert_eq!(height, 100.0 - VIEWPORT_GUTTER);
    assert!(
        height < TOOLS_HEIGHT_MIN,
        "the floor must yield to the gutter rather than overflow the screen"
    );
}

/// A 4K viewport with five hundred groups caps at 80% of the screen
/// (1728px), **not** at the retired 1200px hard cap.
///
/// The old `MAX_HEIGHT_HARD_CAP = 1200` pin is the one that moved most.
/// The framework has no absolute height ceiling for this modal: §4.6.2
/// caps it against the viewport, and a 2160px screen has the room. The
/// card is therefore taller than the old cap allowed, and still bounded —
/// by the screen rather than by a number the app invented.
#[test]
fn a_giant_viewport_caps_at_the_viewport_not_a_hard_cap() {
    let viewport = Size::new(3840.0, 2160.0);
    let height = tools_card_height(viewport);
    assert_eq!(height, 2160.0 * VIEWPORT_FILL);
    assert_ne!(height, 1200.0, "the retired hard cap must not return");
}

/// The error banner's band is charged into the content height, so a
/// failing authentication cannot push the list out of a card sized for
/// the clean state.
#[test]
fn the_error_banner_band_is_reserved_in_the_height() {
    let clean = tools_content_height(7, false);
    let failing = tools_content_height(7, true);
    assert!(
        failing > clean,
        "showing an error must add height, not steal it from the list"
    );
    assert_eq!(failing - clean, 20.0 + 8.0, "BODY line + space.SM");
}

/// `size_for` is the viewport fill applied to the window it is handed —
/// since defect #4 the row count no longer moves the card.
#[test]
fn view_model_size_for_is_the_viewport_fill_not_the_row_count() {
    let one = ToolsDialogViewModel::with_rows(vec![row_from_group(
        &internal_group(InternalToolGroup::Trello),
        McpAuthFacts::default(),
        &name_len,
    )]);
    let many = model_with_rows(40);
    let viewport = Size::new(1200.0, 800.0);

    assert_eq!(one.size_for(viewport), tools_card_size(viewport));
    assert_eq!(
        one.size_for(viewport),
        many.size_for(viewport),
        "defect #4: the window sizes the card, so a longer list cannot shrink it"
    );
    assert_eq!(one.size_for(viewport), Size::new(960.0, 640.0));
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
    // `empty_state` is an in-memory app fixture; no user paths are read
    // ([RUST-006]).
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
// --- The `✓ parallel` marker is a framework status badge -----------------
//
// `widget::chip` (`UIF-ATOM-006`) was the first candidate for this marker
// and is the wrong component: its three variants (`Assist`, `Filter`,
// `Input`) all describe *what pressing the chip does*, and its selected
// polarity resolves to `accent.fill` — there is no success colour anywhere
// in `ChipStyle`. A `✓` that reports "this group may run concurrently" is
// a judgement, and §3.8's status chip is the framework's component for a
// judgement. These cases pin that the marker really is resolved as
// `UIF-ATOM-008` and not painted from the retired ad-hoc constants.

/// The marker resolves as a `Status` badge on the `Success` tone.
#[test]
fn the_parallel_marker_is_a_success_tone_status_badge() {
    let model = BadgeModel::status(StatusIcon::Success, strings::TOOLS_PARALLEL_CHIP)
        .with_tone(BadgeTone::Success);

    assert!(
        matches!(model.kind(), BadgeKind::Status { .. }),
        "the marker must be a judgement (Status), not a count or a dot"
    );
    assert_eq!(model.tone(), BadgeTone::Success);
}

/// Every box measurement comes from the badge resolver, and none of them is
/// the retired `CHIP_PADDING = 2` / `CHIP_RADIUS = 3` / `TOOLS_CHIP_SIZE = 12`.
#[test]
fn the_marker_box_is_resolved_not_ad_hoc() {
    let palette = Palette::pro();
    let model = BadgeModel::status(StatusIcon::Success, strings::TOOLS_PARALLEL_CHIP)
        .with_tone(BadgeTone::Success);
    let style = badge::resolve_style(&model, &palette);

    assert_eq!(style.height, badge::CHIP_HEIGHT);
    assert_eq!(style.h_padding, space::SM);
    assert_eq!(style.radius, shape::RADIUS_PILL);
    assert_eq!(style.border_width, badge::CHIP_BORDER_WIDTH);
    assert!(style.shows_glyph, "a status badge always carries its glyph");

    // The retired ad-hoc figures, asserted absent rather than merely unused.
    const RETIRED_CHIP_PADDING: f32 = 2.0;
    const RETIRED_CHIP_RADIUS: f32 = 3.0;
    const RETIRED_CHIP_SIZE: f32 = 12.0;
    assert_ne!(style.h_padding, RETIRED_CHIP_PADDING);
    assert_ne!(style.radius, RETIRED_CHIP_RADIUS);
    assert_ne!(style.height, RETIRED_CHIP_SIZE);
}

/// The marker's colour is the `Success` tone's own text colour, resolved
/// through the tone table — not the app palette's `success` token reached
/// for directly, and not the chip resolver's `accent.fill`.
#[test]
fn the_marker_colour_is_the_success_tone_not_an_accent() {
    let palette = Palette::pro();
    let model = BadgeModel::status(StatusIcon::Success, strings::TOOLS_PARALLEL_CHIP)
        .with_tone(BadgeTone::Success);
    let style = badge::resolve_style(&model, &palette);

    let tone = badge::resolve_tone(BadgeTone::Success, &palette);
    assert_eq!(style.text, tone.text);
    assert_eq!(style.glyph, tone.text);
    assert_ne!(
        style.fill, palette.accent_fill,
        "the marker must not read as a selected chip"
    );
}

/// The success text on the badge's own fill clears the text-contrast bar,
/// recomputed from the raw sRGB values rather than trusted from the
/// resolver's own lock.
#[test]
fn the_marker_text_clears_the_contrast_bar_against_its_fill() {
    let palette = Palette::pro();
    let model = BadgeModel::status(StatusIcon::Success, strings::TOOLS_PARALLEL_CHIP)
        .with_tone(BadgeTone::Success);
    let style = badge::resolve_style(&model, &palette);

    let ratio = hoarfrost::token::contrast::contrast_ratio(style.text, style.fill);
    assert!(
        hoarfrost::token::contrast::meets_text_contrast(style.text, style.fill),
        "success text on the badge fill is {ratio:.4}:1, below the 4.5:1 text bar"
    );
}

/// A row that is not parallel-safe gets no marker at all — the absence is
/// not a negative signal, so no second badge appears in its place.
#[test]
fn a_non_parallel_row_still_gets_no_marker() {
    let mut row = row_from_group(
        &internal_group(InternalToolGroup::Trello),
        McpAuthFacts::default(),
        &name_len,
    );
    row.parallel_safe = false;
    assert_eq!(parallel_chip_label(&row), None);

    row.parallel_safe = true;
    assert_eq!(
        parallel_chip_label(&row),
        Some(strings::TOOLS_PARALLEL_CHIP),
        "the pinned label is still the model-level rule"
    );
}

// --- UIF-MOD-003: the auth failure the dialog actually has -------------

/// An MCP group whose recorded error is an authentication failure raises a
/// structured banner with a live `Recovery::Retry` (`UIF-MOD-003`).
#[test]
fn a_failed_authentication_raises_a_modal_error_with_retry() {
    let view_model = ToolsDialogViewModel::with_rows(vec![auth_failing_row("srv")]);

    let error = view_model
        .modal_error()
        .expect("an authentication failure must surface a banner");
    assert_eq!(error.recovery(), &Recovery::Retry);
    assert!(view_model.shows_error());
}

/// The banner carries the `NFR-004` error code and names the server the
/// flow was for, in both the message and the location.
#[test]
fn the_banner_names_the_code_and_the_server() {
    let view_model = ToolsDialogViewModel::with_rows(vec![auth_failing_row("linear")]);
    let error = view_model.modal_error().unwrap();

    assert_eq!(error.code(), TOOLS_AUTH_ERROR_CODE);
    assert!(
        error.message().contains("linear"),
        "the message must name the server; got {:?}",
        error.message()
    );
    assert_eq!(
        error.location(),
        Some(&ErrorLocation::Region("Kind: linear".to_string()))
    );
}

/// The `Retry` affordance is genuinely wired: it re-issues
/// `StartMcpAuth` for the same server, the same command the row's own
/// Authenticate button carries.
#[test]
fn the_retry_reissues_the_authentication_that_failed() {
    let view_model = ToolsDialogViewModel::with_rows(vec![auth_failing_row("srv")]);
    assert_eq!(view_model.retry_message(), Some(start_auth_command("srv")));
}

/// An execution failure is a different fault and does **not** raise the
/// card-level auth banner — it stays where [UI-057] puts it, in the row's
/// `⚠` tooltip.
#[test]
fn an_execution_error_is_not_surfaced_as_an_auth_failure() {
    let mut row = row_from_group(
        &mcp_group("srv", ToolGroupKind::McpRemote, true),
        McpAuthFacts::default(),
        &name_len,
    );
    row.error = Some("Execution: tool blew up".to_string());
    row.error_kind = Some(ToolErrorKind::Execution);

    let view_model = ToolsDialogViewModel::with_rows(vec![row]);
    assert!(
        view_model.modal_error().is_none(),
        "an execution failure must not claim the auth banner"
    );
    assert!(!view_model.shows_error());
    // …and the row-level tooltip still carries it.
    assert_eq!(
        error_tooltip_body(view_model.rows().first().unwrap()),
        Some("Execution: tool blew up")
    );
}

/// A clean dialog has no banner, so the reserved band costs nothing.
#[test]
fn a_clean_dialog_carries_no_banner() {
    let view_model = ToolsDialogViewModel::with_rows(vec![row_from_group(
        &internal_group(InternalToolGroup::Trello),
        McpAuthFacts::default(),
        &name_len,
    )]);
    assert!(view_model.modal_error().is_none());
    assert_eq!(view_model.retry_message(), None);
    assert!(!view_model.shows_error());
}

/// The error **kind** survives the row mapping, so the banner never has to
/// parse the rendered `Kind: message` line back apart to know what failed.
#[test]
fn the_error_kind_is_carried_through_the_row_mapping() {
    let group = mcp_group("srv", ToolGroupKind::McpRemote, true);
    let mut group = group;
    group.last_error = Some(ToolGroupError {
        kind: ToolErrorKind::Authentication,
        message: "401 unauthorized".to_string(),
        occurred_at_unix_ms: 1_700_000_000_000,
    });

    let row = row_from_group(&group, McpAuthFacts::default(), &name_len);
    assert_eq!(row.error_kind, Some(ToolErrorKind::Authentication));
    assert_eq!(
        row.error.as_deref(),
        Some("Authentication: 401 unauthorized")
    );
}

// --- Button variants: what each action costs the user -------------------

/// `Forget` is the one `Danger` control on the card: it discards a stored
/// credential the user cannot re-derive without running the flow again.
#[test]
fn forget_is_the_only_danger_action() {
    let row = auth_failing_row("srv");
    let actions = row_actions(&row);

    let forget = actions
        .iter()
        .find(|a| a.label == strings::TOOLS_FORGET)
        .expect("an eligible MCP row must offer Forget");
    assert!(forget.is_enabled());
    assert_eq!(
        action_variant(forget),
        ButtonVariant::Danger,
        "discarding a credential is the destructive act"
    );
}

/// `Restart` and `Authenticate` are `Secondary`: both just try again, and
/// neither takes anything away.
#[test]
fn retrying_actions_are_secondary_not_danger() {
    let row = auth_failing_row("srv");
    for label in [strings::TOOLS_RESTART, strings::TOOLS_AUTH_BUTTON] {
        let action = row_actions(&row)
            .into_iter()
            .find(|a| a.label == label)
            .unwrap_or_else(|| panic!("{label} must be offered on a failing auth row"));
        assert_eq!(
            action_variant(&action),
            ButtonVariant::Secondary,
            "{label} only re-runs work and must not wear the destructive skin"
        );
    }
}

/// An inert `Authenticating…` action stays visible and reports itself
/// disabled rather than vanishing mid-flow (AGENTS §6).
#[test]
fn an_in_flight_auth_action_stays_visible_and_inert() {
    let mut row = auth_failing_row("srv");
    row.auth_state = ToolAuthState::InProgress;
    let actions = row_actions(&row);

    let running = actions
        .iter()
        .find(|a| a.label == strings::TOOLS_AUTH_RUNNING)
        .expect("an in-progress row must show the running label");
    assert!(!running.is_enabled());
    assert_eq!(action_focus_id(running.label), "tools.action.running");
}

/// Each action label maps to a distinct stable focus id, so no two live
/// buttons can share one (AGENTS §5).
#[test]
fn every_action_label_maps_to_its_own_focus_id() {
    let ids = [
        action_focus_id(strings::TOOLS_FORGET),
        action_focus_id(strings::TOOLS_RESTART),
        action_focus_id(strings::TOOLS_AUTH_BUTTON),
        action_focus_id(strings::TOOLS_AUTH_RUNNING),
    ];
    let mut unique = ids.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "focus ids must not collide: {ids:?}"
    );
}

// --- Absorption -------------------------------------------------------

/// `Absorbed` is the no-op the card-boundary `mouse_area` needs.
#[test]
fn absorbed_changes_nothing() {
    let mut state = ToolsDialogState::new();
    assert!(state.open());
    assert!(
        !state.apply(ToolsDialogMessage::Absorbed),
        "Absorbed must report no state change"
    );
    assert!(state.is_open());
}

/// Absorbing while closed stays closed.
#[test]
fn absorbed_cannot_open_a_closed_dialog() {
    let mut state = ToolsDialogState::new();
    assert!(!state.apply(ToolsDialogMessage::Absorbed));
    assert!(!state.is_open());
}

// --- Defect #7: the title band's `×` dismiss control -------------------

/// A content box the size of the title band's `×` glyph: narrower and
/// shorter than §3.1's icon-only floor of `CONTROL_SM` (28px) on both
/// axes.
fn close_glyph_content() -> Size {
    Size::new(8.0, 16.0)
}

/// The `×` is icon-only, so the tooltip rule engages — and the button must
/// satisfy it. Without the tooltip, `Button::outer_size`'s `debug_assert!`
/// fires the moment the Tools card is laid out, i.e. the app crashes on
/// open. Asserted through the non-panicking predicate so the defect is
/// pinned without a window and without a crash handler.
#[test]
fn the_title_close_button_is_icon_only_and_carries_its_tooltip() {
    let button = close_button();
    let glyph = close_glyph_content();

    assert!(
        glyph.width <= hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH
            && glyph.height <= hoarfrost::widget::button::ICON_ONLY_MIN_HEIGHT,
        "the glyph content box must sit at or below the icon-only floor, otherwise \
         this test pins nothing"
    );
    assert!(
        button.is_icon_only(glyph),
        "defect #7: the `×` content must measure icon-only, otherwise the tooltip \
         rule is never exercised"
    );
    assert_eq!(
        button.tooltip_violation(glyph),
        None,
        "defect #7: an icon-only close with no tooltip trips `Button::outer_size`'s \
         debug_assert and crashes the app when the Tools card opens"
    );
    assert_eq!(
        button.tooltip_text(),
        Some(strings::TOOLS_CLOSE_TOOLTIP),
        "the tooltip must come from the shared string table, not an inline literal"
    );
    assert_eq!(
        button.variant(),
        ButtonVariant::Ghost,
        "the trailing title-band action wears the ghost skin, like every other \
         dismiss in this app"
    );
    assert_eq!(button.focus_id().as_str(), TOOLS_CLOSE_BUTTON_ID);
}

/// The `×` is mounted in the real card's title band and actually dismisses
/// the dialog — same message the scrim click and Escape emit, so no second
/// dismiss vocabulary exists.
#[test]
fn the_title_close_control_is_mounted_and_dismisses_the_dialog() {
    let view_model = ToolsDialogViewModel::empty();
    let mut ui = iced_test::simulator(tools_dialog_card(&view_model, ROOMY));
    let bounds = focusable_bounds(&mut ui, TOOLS_CLOSE_BUTTON_ID)
        .expect("defect #7: the title band must mount the `×` dismiss control");
    assert!(bounds.width > 0.0 && bounds.height > 0.0);

    click_at(&mut ui, bounds.center());

    let emitted = ui.into_messages().collect::<Vec<_>>();
    assert!(
        emitted.contains(&dismiss_message()),
        "the `×` must emit the same dismiss Escape uses; got {emitted:?}"
    );
}

// --- Defect #8: the footer `Close` button ------------------------------

/// The footer control is a labelled button that is always live.
#[test]
fn the_footer_close_button_is_enabled_and_secondary() {
    let button = footer_close_button();

    assert!(
        button.is_enabled(),
        "defect #8: the footer Close must accept a press on every frame — a disabled \
         dismiss would trap the user in a modal whose only other exit is the scrim"
    );
    assert_eq!(
        button.variant(),
        ButtonVariant::Secondary,
        "leaving the card destroys nothing, so it must not wear the danger skin"
    );
    assert_eq!(button.focus_id().as_str(), TOOLS_FOOTER_CLOSE_BUTTON_ID);

    // The `footer_button` min-width box keeps the label clear of the
    // icon-only floor, so this control needs no tooltip to be legal.
    let labelled = Size::new(
        hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH,
        typography::BODY.line_height,
    );
    assert!(
        !button.is_icon_only(labelled),
        "the footer label must never measure icon-only"
    );
    assert_eq!(button.tooltip_violation(labelled), None);
}

/// The footer renders the shared `Close` label — read off the laid-out tree
/// rather than a re-typed literal, so a change to `strings::CLOSE_BUTTON`
/// moves the card and the assertion together.
#[test]
fn the_footer_close_button_renders_the_shared_close_label() {
    let view_model = ToolsDialogViewModel::empty();
    let mut ui = iced_test::simulator(tools_dialog_card(&view_model, ROOMY));
    assert!(
        ui.find(strings::CLOSE_BUTTON).is_ok(),
        "defect #8: the footer must render `strings::CLOSE_BUTTON`"
    );
}

/// The footer button is mounted in the real card and dismisses it.
#[test]
fn the_footer_close_control_is_mounted_and_dismisses_the_dialog() {
    let view_model = ToolsDialogViewModel::empty();
    let mut ui = iced_test::simulator(tools_dialog_card(&view_model, ROOMY));
    let bounds = focusable_bounds(&mut ui, TOOLS_FOOTER_CLOSE_BUTTON_ID)
        .expect("defect #8: the footer must mount its Close button");

    click_at(&mut ui, bounds.center());

    let emitted = ui.into_messages().collect::<Vec<_>>();
    assert!(
        emitted.contains(&dismiss_message()),
        "the footer Close must emit the same dismiss Escape uses; got {emitted:?}"
    );
}

/// The two dismiss controls are distinct live widgets (AGENTS §5: one id,
/// one live widget), so neither can shadow the other's focus.
#[test]
fn the_two_close_controls_do_not_share_an_id() {
    assert_ne!(TOOLS_CLOSE_BUTTON_ID, TOOLS_FOOTER_CLOSE_BUTTON_ID);
    assert_ne!(
        close_button().widget_id(),
        footer_close_button().widget_id()
    );
}

// --- fixtures ---------------------------------------------------------

/// The layout bounds of the focusable control registered under `focus_id`.
///
/// Hoarfrost registers a button's iced `Id` from its focus id (see
/// `hoarfrost::bridge::widget_id`), and a *disabled* button deliberately
/// does not register at all — so "found here" also means "live here".
fn focusable_bounds(
    ui: &mut iced_test::Simulator<'_, Message, iced::Theme, iced::Renderer>,
    focus_id: &'static str,
) -> Option<iced::Rectangle> {
    let expected = iced::widget::Id::from(focus_id);
    ui.find(move |candidate: Candidate<'_>| match candidate {
        Candidate::Focusable {
            id: Some(id),
            bounds,
            ..
        } if *id == expected => Some(bounds),
        _ => None,
    })
    .ok()
}

/// Points the cursor at `position` and left-clicks it.
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

/// An MCP row whose last recorded error is a failed authentication.
fn auth_failing_row(server: &str) -> ToolGroupRow {
    let mut group = mcp_group(server, ToolGroupKind::McpRemote, true);
    group.last_error = Some(ToolGroupError {
        kind: ToolErrorKind::Authentication,
        message: "401 unauthorized".to_string(),
        occurred_at_unix_ms: 1_700_000_000_000,
    });
    row_from_group(
        &group,
        McpAuthFacts {
            eligible: true,
            in_progress: false,
        },
        &name_len,
    )
}

/// The variant the renderer would give this action — the same rule
/// `action_button` applies, exposed so the test can assert it without
/// building a widget tree.
fn action_variant(action: &ToolAction) -> ButtonVariant {
    if action.label == strings::TOOLS_FORGET {
        ButtonVariant::Danger
    } else {
        ButtonVariant::Secondary
    }
}
