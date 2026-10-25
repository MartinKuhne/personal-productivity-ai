//! Sidecar tests for [`crate::ui_iced::panels::bottom`] —
//! UI-001's bottom command band.
//!
//! Covers the reused domain helpers reached through this layer, the extracted
//! submit-intent solver (idle vs running, blank input, whitespace trimming),
//! the Stop-button gate, the `FastMdApp` snapshot and the view builder.
//!
//! Fixtures use [`test_app`], which builds `FastMdApp::empty_state` over a
//! default `AppConfig` — no files, no bus traffic and no user paths
//! ([RUST-006]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::LlmConfig;
use crate::ui::strings;
use crate::ui_iced::test_helpers::test_app;

use iced::Background;
use iced_pro::state::{BaseState, VisualState};
use iced_pro::token::{shape, size, space, typography};
use iced_pro::widget::button::{self, ButtonVariant};
use iced_pro::widget::text_input as pro_text_input;

use super::{
    BAND_PADDING, BAND_SPACING, BottomPanelViewModel, COMMAND_INPUT_ID, CONTEXT_SPACING,
    FILE_CONTEXT_BASE_SIZE, FILE_CONTEXT_FALLBACK_SIZE, FILE_CONTEXT_MIN_SIZE, PROMPT_TEXT_SIZE,
    STOP_BUTTON_ID, STOP_BUTTON_TEXT_SIZE, command_field_style, compute_file_context,
    compute_prompt_prefix, format_models_list, parse_command_intent, stop_button_color,
    stop_button_variant, stop_enabled, submit_commands,
};

use crate::bus::events::user_command::UserCommand;
use crate::ui::theme::Palette;

/// A file path that never touches the filesystem — the band only formats it.
fn note_path() -> PathBuf {
    PathBuf::from("workspace/notes/note.md")
}

/// Open `path` as a tab and select it — the state the band reads.
fn open_and_select(app: &mut crate::ui::app::FastMdApp, path: &Path) {
    app.orchestrator.tabs.tabs.push(path.to_path_buf());
    app.selection_mut().selected_file = Some(path.to_path_buf());
}

// ---------------------------------------------------------------------------
// Reused domain-side helpers ([RUST-051]) — reachable and unchanged
// ---------------------------------------------------------------------------

#[test]
fn parse_command_intent_maps_every_known_prefix() {
    assert_eq!(
        parse_command_intent("/models"),
        Some(UserCommand::ShowModels),
        "exact command"
    );
    assert_eq!(
        parse_command_intent("/models extra"),
        Some(UserCommand::ShowModels),
        "prefix match"
    );
    assert_eq!(
        parse_command_intent("/model gpt-x"),
        Some(UserCommand::ShowDeprecatedModelMessage),
        "deprecated switch"
    );
    assert_eq!(
        parse_command_intent("summarise this"),
        Some(UserCommand::RunAgent("summarise this".to_string())),
        "anything else is a prompt"
    );
    assert_eq!(parse_command_intent(""), None, "blank is not a command");
    assert_eq!(
        parse_command_intent("   "),
        None,
        "whitespace is not a command"
    );
}

#[test]
fn prompt_prefix_falls_back_to_the_raw_directory() {
    assert_eq!(compute_prompt_prefix(None, &[]), ">");
    assert_eq!(compute_prompt_prefix(Some(Path::new("")), &[]), ">");
    let prefix = compute_prompt_prefix(Some(Path::new("some/dir")), &[]);
    assert!(
        prefix.ends_with(" >"),
        "a directory prefix always ends the prompt: {prefix}"
    );
    assert!(prefix.contains("some"));
}

#[test]
fn file_context_is_none_without_an_active_file() {
    assert_eq!(compute_file_context(None, ">"), None);
    assert_eq!(compute_file_context(Some(Path::new("")), ">"), None);
}

#[test]
fn file_context_keeps_the_base_size_when_the_directory_is_not_longer() {
    let (name, size) = compute_file_context(Some(Path::new("a.md")), "Long Directory >")
        .expect("a file name is present");
    assert_eq!(name, "a.md");
    assert_eq!(
        size, FILE_CONTEXT_BASE_SIZE,
        "file shorter than the directory keeps base size"
    );
    assert_eq!(size, 12.0);
}

/// The floor is now `typography::MIN_SIZE` (`UIF-TOK-004`), not the 8 pt
/// the helper used to carry.
///
/// **This pin moved on purpose.** It asserted `8.0`, a size the type scale
/// forbids anywhere in the interface. Raising the floor to the rule's own
/// value makes it equal [`FILE_CONTEXT_BASE_SIZE`], so the ratio branch
/// collapses onto the base and the label no longer *modulates* with the
/// directory/file length ratio — see [`FILE_CONTEXT_MIN_SIZE`] for why
/// restoring a gradient is a separate behaviour decision. The label is
/// still de-emphasised: `CAPTION` (12) against the prompt's `BODY` (14).
#[test]
fn file_context_never_renders_below_the_typography_floor() {
    let cases = [
        ("a-very-long-file-name.md", "Notes >"),
        (
            "an-absurdly-long-file-name-that-should-shrink-further.md",
            "N >",
        ),
        ("x.md", "Notes >"),
    ];
    for (file, prefix) in cases {
        let (_, size) =
            compute_file_context(Some(Path::new(file)), prefix).expect("a file name is present");
        assert!(
            size >= typography::MIN_SIZE,
            "`{file}` against `{prefix}` rendered at {size}, below MIN_SIZE {}",
            typography::MIN_SIZE
        );
        assert_eq!(size, FILE_CONTEXT_MIN_SIZE);
        assert_ne!(
            size, 8.0,
            "the retired 8 pt floor must not come back for `{file}`"
        );
    }
    assert_eq!(FILE_CONTEXT_MIN_SIZE, typography::MIN_SIZE);
    assert_eq!(FILE_CONTEXT_MIN_SIZE, 12.0);
    assert_eq!(FILE_CONTEXT_BASE_SIZE, typography::CAPTION.size);
}

#[test]
fn format_models_list_reports_an_empty_catalogue() {
    let output = format_models_list(&HashMap::new());
    assert!(output.starts_with(strings::MODELS_LIST_HEADER));
    assert!(output.contains(strings::MODELS_LIST_NO_ADDITIONAL));
}

#[test]
fn format_models_list_lists_name_cost_and_use_case() {
    let mut models = HashMap::new();
    models.insert(
        "alpha".to_string(),
        LlmConfig {
            model: "alpha-model".to_string(),
            api_url: "http://localhost:1".to_string(),
            api_key: "key".to_string(),
            cost: Some(7),
            use_case: vec!["chat".to_string(), "vision".to_string()],
        },
    );

    let output = format_models_list(&models);
    assert!(output.starts_with(strings::MODELS_LIST_HEADER));
    assert!(output.contains("- alpha [cost: 7, use_case: chat, vision]"));
    assert!(
        !output.contains(strings::MODELS_LIST_NO_ADDITIONAL),
        "a populated catalogue is not 'no additional models'"
    );
}

// ---------------------------------------------------------------------------
// Submit intent (idle vs running branch resolution)
// ---------------------------------------------------------------------------

#[test]
fn idle_submit_publishes_the_parsed_command_then_clears() {
    assert_eq!(
        submit_commands(false, "summarise this"),
        vec![
            UserCommand::RunAgent("summarise this".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
    assert_eq!(
        submit_commands(false, "/models"),
        vec![UserCommand::ShowModels, UserCommand::ClearCommandInput]
    );
    assert_eq!(
        submit_commands(false, "/model gpt-x"),
        vec![
            UserCommand::ShowDeprecatedModelMessage,
            UserCommand::ClearCommandInput,
        ]
    );
}

#[test]
fn idle_submit_of_a_blank_prompt_publishes_nothing() {
    assert!(submit_commands(false, "").is_empty());
    assert!(submit_commands(false, "   ").is_empty());
    assert!(
        submit_commands(false, "\n").is_empty(),
        "a lone newline trims to nothing"
    );
}

#[test]
fn running_submit_queues_the_prompt_then_clears() {
    assert_eq!(
        submit_commands(true, "next instruction"),
        vec![
            UserCommand::QueueAgentPrompt("next instruction".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
}

#[test]
fn running_submit_of_a_blank_prompt_publishes_nothing_at_all() {
    // The `!prompt.is_empty()` guard: no queue message *and* no clear.
    assert!(submit_commands(true, "").is_empty());
    assert!(submit_commands(true, "   ").is_empty());
}

#[test]
fn submit_trims_only_trailing_whitespace() {
    // `command_input.trim_end()` — leading indentation the user
    // typed is preserved verbatim, only the trailing newline/spaces go.
    assert_eq!(
        submit_commands(true, "  keep me\n"),
        vec![
            UserCommand::QueueAgentPrompt("  keep me".to_string()),
            UserCommand::ClearCommandInput,
        ],
        "trim_end strips the trailing newline the user typed"
    );
    assert_eq!(
        submit_commands(false, "hello  world"),
        vec![
            UserCommand::RunAgent("hello  world".to_string()),
            UserCommand::ClearCommandInput,
        ],
        "inner spacing is the parser's business, not the band's"
    );
}

#[test]
fn running_submit_does_not_parse_slash_commands() {
    // While the agent runs, everything is queued verbatim — `/models` is not
    // routed through the parser in that branch.
    assert_eq!(
        submit_commands(true, "/models"),
        vec![
            UserCommand::QueueAgentPrompt("/models".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
}

#[test]
fn stop_is_enabled_only_while_the_agent_runs() {
    assert!(stop_enabled(true));
    assert!(!stop_enabled(false));
}

/// The always-allocated Stop label must read as *inactive* while idle — a
/// red, live-looking Stop over a turn that never ran is the UI-001 defect.
#[test]
fn stop_button_label_drops_to_muted_while_idle() {
    let palette = Palette::fastmd_dark();
    assert_eq!(stop_button_color(true), palette.error);
    assert_eq!(stop_button_color(false), palette.text_muted);
}

// ---------------------------------------------------------------------------
// View-model snapshot
// ---------------------------------------------------------------------------

#[test]
fn from_app_snapshots_a_bare_band() {
    let app = test_app();
    let view_model = BottomPanelViewModel::from_app(&app);

    assert_eq!(view_model.command_input, "");
    assert!(!view_model.agent_running);
    assert_eq!(view_model.prompt_prefix, ">");
    assert_eq!(view_model.file_context, None);
}

#[test]
fn from_app_carries_the_command_input_text() {
    let mut app = test_app();
    app.orchestrator
        .agent_panel_state
        .command_input
        .push_str("draft prompt");

    let view_model = BottomPanelViewModel::from_app(&app);
    assert_eq!(view_model.command_input, "draft prompt");
}

#[test]
fn from_app_carries_the_agent_running_flag() {
    let mut app = test_app();
    app.agent_mut().set_running(true);
    assert!(BottomPanelViewModel::from_app(&app).agent_running);

    app.agent_mut().set_running(false);
    assert!(!BottomPanelViewModel::from_app(&app).agent_running);
}

#[test]
fn from_app_reports_no_file_context_until_a_tab_is_open() {
    let mut app = test_app();
    // Selected in the tree but never opened as a tab: `agent_context` withholds
    // the file, so the band shows no file line.
    app.selection_mut().selected_file = Some(note_path());
    assert_eq!(BottomPanelViewModel::from_app(&app).file_context, None);
}

#[test]
fn from_app_shows_the_active_file_once_its_tab_is_open() {
    let mut app = test_app();
    open_and_select(&mut app, &note_path());

    let view_model = BottomPanelViewModel::from_app(&app);
    let (name, size) = view_model
        .file_context
        .expect("an open tab gives the band a file context");
    assert_eq!(name, "note.md");
    assert_eq!(
        size,
        typography::MIN_SIZE,
        "the band's file line sits on the typography floor, its lowest legal size"
    );
    assert_ne!(size, 8.0, "the retired 8 pt floor must not come back");
}

#[test]
fn from_app_prefix_matches_the_prompt_directory() {
    let mut app = test_app();
    app.selection_mut().selected_dir = Some(PathBuf::from("workspace/notes"));

    let view_model = BottomPanelViewModel::from_app(&app);
    assert_eq!(
        view_model.prompt_prefix,
        compute_prompt_prefix(Some(Path::new("workspace/notes")), app.content_libraries()),
        "the displayed prefix must be the same directory the prompt runs against"
    );
    assert!(view_model.prompt_prefix.ends_with(" >"));
}

#[test]
fn view_model_submit_commands_uses_its_own_running_flag() {
    let mut app = test_app();
    app.orchestrator.agent_panel_state.command_input = "go".to_string();

    let idle = BottomPanelViewModel::from_app(&app);
    assert_eq!(
        idle.submit_commands(),
        vec![
            UserCommand::RunAgent("go".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );

    app.agent_mut().set_running(true);
    let running = BottomPanelViewModel::from_app(&app);
    assert_eq!(
        running.submit_commands(),
        vec![
            UserCommand::QueueAgentPrompt("go".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
}

// ---------------------------------------------------------------------------
// Framework widget resolution (INT-7a)
// ---------------------------------------------------------------------------

/// Both states of the Stop control are pinned: `Danger` while a turn is
/// cancellable, and explicitly **not** `Danger` when idle, so the band can
/// never paint a red CTA over nothing to stop.
#[test]
fn stop_button_is_danger_only_while_work_is_running() {
    assert_eq!(stop_button_variant(true), ButtonVariant::Danger);
    assert_eq!(stop_button_variant(false), ButtonVariant::Secondary);
    assert_ne!(
        stop_button_variant(false),
        ButtonVariant::Danger,
        "idle must not read as a live destructive action"
    );
    // The variant and the enabled flag track the same fact: exactly the
    // running state decides both.
    for running in [true, false] {
        assert_eq!(
            stop_enabled(running),
            stop_button_variant(running) == ButtonVariant::Danger,
            "enabled and Danger must agree for running = {running}"
        );
    }
}

/// The running label colour is exactly the `Danger` rest cell's resolved
/// foreground, so the label obeys the framework's contrast lock rather
/// than a locally chosen red.
#[test]
fn stop_button_label_colour_matches_the_danger_rest_lock() {
    let pro = Palette::pro();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let danger = button::resolve_style(ButtonVariant::Danger, rest, &pro);
    assert_eq!(stop_button_color(true), danger.foreground);
    assert_eq!(stop_button_color(false), pro.text_muted);
    assert_ne!(
        stop_button_color(false),
        danger.foreground,
        "the idle label must sit below the danger lock"
    );
}

/// The command field's paint resolves through the framework text-input
/// atom for every iced status, including the focus and disabled cells.
#[test]
fn command_field_paint_resolves_through_the_input_atom() {
    use iced::widget::text_input::Status;

    let pro = Palette::pro();
    let resolve = |base: BaseState, focused: bool| {
        pro_text_input::resolve_input_style(
            pro_text_input::InputKind::TextEntry,
            VisualState { base, focused },
            &pro,
        )
    };

    let active = command_field_style(Status::Active);
    let rest = resolve(BaseState::Rest, false);
    assert_eq!(active.background, Background::Color(rest.background));
    assert_eq!(active.border.color, rest.border_color);
    assert_eq!(active.border.width, rest.border_width);
    assert_eq!(active.value, rest.text);
    assert_eq!(active.placeholder, rest.placeholder);
    assert_eq!(active.selection, rest.selection_background);
    assert_eq!(active.border.radius, shape::RADIUS_WIDGET.into());

    let hover = command_field_style(Status::Hovered);
    let hovered = resolve(BaseState::Hovered, false);
    assert_eq!(hover.border.color, hovered.border_color);
    assert_ne!(
        hover.border.color, active.border.color,
        "hover must be distinguishable from rest through the atom"
    );

    let focused = command_field_style(Status::Focused { is_hovered: false });
    let focused_resolved = resolve(BaseState::Rest, true);
    assert_eq!(focused.border.color, focused_resolved.border_color);
    assert_eq!(focused.border.width, focused_resolved.border_width);

    let disabled = command_field_style(Status::Disabled);
    let disabled_resolved = resolve(BaseState::Disabled, false);
    assert_eq!(disabled.border.color, disabled_resolved.border_color);
    assert_eq!(disabled.value, disabled_resolved.text);
    assert_ne!(
        disabled.value, active.value,
        "a disabled field must not read as live"
    );
}

/// The band's scalars resolve from tokens, and the two interactive
/// controls share one density rung (§1 Row Density Homogeneity).
#[test]
fn band_scalars_resolve_from_tokens_and_share_one_density_rung() {
    assert_eq!(BAND_SPACING, space::SM);
    assert_eq!(CONTEXT_SPACING, space::XXS);
    assert_eq!(BAND_PADDING, space::XS);
    assert_ne!(
        BAND_PADDING, 6.0,
        "6 was off the base-4 grid; it must not return"
    );
    assert_eq!(PROMPT_TEXT_SIZE, typography::BODY.size);
    assert_eq!(STOP_BUTTON_TEXT_SIZE, typography::BODY.size);
    assert_eq!(FILE_CONTEXT_FALLBACK_SIZE, typography::MIN_SIZE);

    // Field: BODY line height + 2 x XS padding lands on CONTROL_SM, the
    // same rung the Stop button takes via `.compact()`.
    let field_height = typography::BODY.line_height + 2.0 * space::XS;
    assert_eq!(field_height, size::CONTROL_SM);
    assert_eq!(
        field_height,
        button::HEIGHT_COMPACT,
        "both controls in the band must share one ControlDensity"
    );
}

/// Both stateful controls carry an app-chosen stable `Id` rather than
/// `Id::unique()`, so the reducer can address them and they keep their
/// state across tree rebuilds ([AGENTS §5]).
#[test]
fn band_controls_have_stable_ids() {
    assert_eq!(COMMAND_INPUT_ID, "bottom_command_input");
    assert_eq!(STOP_BUTTON_ID, "bottom_stop_button");
    assert_ne!(COMMAND_INPUT_ID, STOP_BUTTON_ID, "one id, one live widget");
}

// ---------------------------------------------------------------------------
// View builder
// ---------------------------------------------------------------------------

#[test]
fn bottom_panel_builds_idle_and_running_snapshots() {
    let idle = BottomPanelViewModel {
        command_input: String::new(),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let running = BottomPanelViewModel {
        command_input: "queued".to_string(),
        agent_running: true,
        prompt_prefix: "notes >".to_string(),
        file_context: Some(("note.md".to_string(), 10.0)),
    };

    // Both build without panicking. iced 0.14 dropped `Widget::type_name`, so
    // widget-tree identity cannot be asserted in a unit test; the shape
    // guarantee comes from the builder always allocating every item
    // (`on_press_maybe` / empty-string label) rather than from a branch.
    let _idle_element = super::bottom_panel(&idle);
    let _running_element = super::bottom_panel(&running);
}
