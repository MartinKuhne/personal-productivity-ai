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

use super::{
    BAND_PADDING, BAND_SPACING, BottomPanelViewModel, COMMAND_INPUT_ID, CONTEXT_SPACING,
    FILE_CONTEXT_BASE_SIZE, FILE_CONTEXT_FALLBACK_SIZE, FILE_CONTEXT_MIN_SIZE, PROMPT_TEXT_SIZE,
    STOP_BUTTON_ID, STOP_BUTTON_TEXT_SIZE, compute_file_context, compute_prompt_prefix,
    format_models_list, parse_command_intent, stop_button_color, stop_button_variant, stop_enabled,
    submit_commands,
};
use hoarfrost::binding::{Cancel, Commit, Live};
use hoarfrost::state::{BaseState, VisualState};
use hoarfrost::token::{size, space, typography};
use hoarfrost::widget::button::{self, ButtonVariant};
use hoarfrost::widget::text_input as pro_text_input;
use hoarfrost::widget::text_input_controller::TextInputEmission;
use hoarfrost::widget::text_input_model::TextInputModel;

use crate::bus::events::user_command::UserCommand;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};

/// A file path that never touches the filesystem — the band only formats it.
fn note_path() -> PathBuf {
    PathBuf::from("workspace/notes/note.md")
}

/// The command band's one field model, including its placeholder and disabled
/// clear affordance.
fn command_model(value: &str) -> TextInputModel {
    TextInputModel::new(value)
        .with_placeholder(strings::COMMAND_INPUT_HINT)
        .with_clearable(false)
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

    assert_eq!(view_model.command_input.value, "");
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
    assert_eq!(view_model.command_input.value, "draft prompt");
}

#[test]
fn command_field_model_tracks_app_state_without_becoming_clearable() {
    let mut app = test_app();
    app.orchestrator.agent_panel_state.command_input = "first draft".to_string();

    let first = BottomPanelViewModel::from_app(&app);
    assert_eq!(first.command_input.value, "first draft");
    assert_eq!(first.command_input.placeholder, strings::COMMAND_INPUT_HINT);
    assert!(!first.command_input.clearable);

    app.orchestrator.agent_panel_state.command_input = "authoritative update".to_string();
    let updated = BottomPanelViewModel::from_app(&app);
    assert_eq!(updated.command_input.value, "authoritative update");
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

#[test]
fn rendered_command_input_and_submit_command_share_one_value() {
    let displayed_value = "displayed value";
    let view_model = BottomPanelViewModel {
        command_input: command_model(displayed_value),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let mut ui = iced_test::simulator(super::bottom_panel(&view_model));

    assert!(
        ui.find(displayed_value).is_ok(),
        "the command input must render its view-model value"
    );
    assert_eq!(
        view_model.submit_commands(),
        vec![
            UserCommand::RunAgent(displayed_value.to_string()),
            UserCommand::ClearCommandInput,
        ],
        "submitting must use the same value shown in the command input"
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

    // The framework field's `.compact()` height remains the old explicit
    // BODY line box + 2 x XS padding, and matches the Stop button's rung.
    let field_height = typography::BODY.line_height + 2.0 * space::XS;
    assert_eq!(field_height, size::CONTROL_SM);
    assert_eq!(pro_text_input::HEIGHT_COMPACT, field_height);
    assert_eq!(
        pro_text_input::HEIGHT_COMPACT,
        button::HEIGHT_COMPACT,
        "both controls in the band must share one ControlDensity"
    );
}

#[test]
fn command_field_emissions_keep_enter_blur_escape_and_clear_routes_distinct() {
    assert_eq!(
        super::command_field_emit(TextInputEmission::Changed(Live::new("typed".to_string()))),
        Some(Message::Agent(AgentMessage::InputChanged {
            text: "typed".to_string(),
        }))
    );
    assert_eq!(
        super::command_field_emit(TextInputEmission::Committed(Commit::new(
            "submitted".to_string(),
        ))),
        Some(Message::Agent(AgentMessage::Submit {
            text: "submitted".to_string(),
        }))
    );
    assert_eq!(
        super::command_field_emit(TextInputEmission::FocusCommitted(Commit::new(
            "blurred".to_string(),
        ))),
        Some(Message::Agent(AgentMessage::InputChanged {
            text: "blurred".to_string(),
        }))
    );
    assert_eq!(
        super::command_field_emit(TextInputEmission::Cancelled(Cancel::new(
            "before edit".to_string(),
        ))),
        Some(Message::Agent(AgentMessage::InputChanged {
            text: "before edit".to_string(),
        }))
    );
    assert_eq!(
        super::command_field_emit(TextInputEmission::FocusChanged(false)),
        None
    );
    assert_eq!(
        super::command_field_emit(TextInputEmission::Cleared(Commit::new(String::new()))),
        None
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
        command_input: command_model(""),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let running = BottomPanelViewModel {
        command_input: command_model("queued"),
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

// ---------------------------------------------------------------------------
// Live controlled command-field interactions
// ---------------------------------------------------------------------------

#[test]
fn live_command_field_typing_emits_input_changed() {
    use iced_test::selector::Candidate;

    let view_model = BottomPanelViewModel {
        command_input: command_model(""),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let input_id = iced::widget::Id::from(COMMAND_INPUT_ID);
    let mut ui = iced_test::simulator(super::bottom_panel(&view_model));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == input_id => Some(bounds),
            _ => None,
        })
        .expect("the bottom band must mount its stable command input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("x");

    let messages: Vec<_> = ui.into_messages().collect();
    assert!(
        messages.contains(&Message::Agent(AgentMessage::InputChanged {
            text: "x".to_string(),
        })),
        "typing must continue to emit InputChanged, got {messages:?}"
    );
}

#[test]
fn live_command_field_enter_submits_the_edited_value() {
    use iced_test::selector::Candidate;

    let view_model = BottomPanelViewModel {
        command_input: command_model(""),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let input_id = iced::widget::Id::from(COMMAND_INPUT_ID);
    let mut ui = iced_test::simulator(super::bottom_panel(&view_model));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == input_id => Some(bounds),
            _ => None,
        })
        .expect("the bottom band must mount its stable command input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("submitted value");
    let _ = ui.tap_key(iced::keyboard::key::Named::Enter);

    let messages: Vec<_> = ui.into_messages().collect();
    assert!(
        messages.contains(&Message::Agent(AgentMessage::Submit {
            text: "submitted value".to_string(),
        })),
        "Enter must submit the live editor value, got {messages:?}"
    );
}

#[test]
fn live_command_field_escape_restores_the_pre_edit_text() {
    use iced_test::selector::Candidate;

    let view_model = BottomPanelViewModel {
        command_input: command_model("original"),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let input_id = iced::widget::Id::from(COMMAND_INPUT_ID);
    let mut ui = iced_test::simulator(super::bottom_panel(&view_model));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == input_id => Some(bounds),
            _ => None,
        })
        .expect("the bottom band must mount its stable command input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite(" edited");
    let _ = ui.tap_key(iced::keyboard::key::Named::Escape);

    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        messages.contains(&Message::Agent(AgentMessage::InputChanged {
            text: "original".to_string(),
        })),
        "Escape must synchronize the pre-edit text, got {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Message::Agent(AgentMessage::Submit { .. }))),
        "Escape must never submit, got {messages:?}"
    );
}

#[test]
fn live_command_field_blur_syncs_without_submitting() {
    use iced_test::selector::Candidate;

    let view_model = BottomPanelViewModel {
        command_input: command_model(""),
        agent_running: false,
        prompt_prefix: ">".to_string(),
        file_context: None,
    };
    let input_id = iced::widget::Id::from(COMMAND_INPUT_ID);
    let mut ui = iced_test::simulator(super::bottom_panel(&view_model));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == input_id => Some(bounds),
            _ => None,
        })
        .expect("the bottom band must mount its stable command input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("draft");
    let _ = ui.click(">");

    let messages: Vec<Message> = ui.into_messages().collect();
    let synced_values: Vec<_> = messages
        .iter()
        .filter_map(|message| match message {
            Message::Agent(AgentMessage::InputChanged { text }) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        synced_values.len() >= 2,
        "focus loss must synchronize after the keystroke stream, got {synced_values:?}"
    );
    assert_eq!(
        &synced_values[synced_values.len() - 2..],
        ["draft", "draft"],
        "the last keystroke and focus-loss commit must both carry the settled value"
    );
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Message::Agent(AgentMessage::Submit { .. }))),
        "blur must never submit, got {messages:?}"
    );
}
