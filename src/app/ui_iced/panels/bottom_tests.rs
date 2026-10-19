//! Sidecar tests for [`crate::ui_iced::panels::bottom`] ([TASK-016],
//! UI-001's bottom command band).
//!
//! Covers the reused the legacy UI helpers reached through this layer, the extracted
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
    BottomPanelViewModel, compute_file_context, compute_prompt_prefix, format_models_list,
    parse_command_intent, stop_enabled, submit_commands,
};

use crate::bus::events::user_command::UserCommand;

/// A file path that never touches the filesystem — the band only formats it.
fn note_path() -> PathBuf {
    PathBuf::from("workspace/notes/note.md")
}

/// Open `path` as a tab and select it, the state the legacy UI band read.
fn open_and_select(app: &mut crate::ui::app::FastMdApp, path: &Path) {
    app.orchestrator.tabs.tabs.push(path.to_path_buf());
    app.selection_mut().selected_file = Some(path.to_path_buf());
}

// ---------------------------------------------------------------------------
// Reused legacy-side helpers ([RUST-051]) — reachable and unchanged
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
        size, 12.0,
        "file shorter than the directory keeps base size"
    );
}

#[test]
fn file_context_shrinks_a_long_file_name_but_never_below_the_floor() {
    let (_, size) = compute_file_context(Some(Path::new("a-very-long-file-name.md")), "Notes >")
        .expect("a file name is present");
    assert!(
        (size - 8.0).abs() < f32::EPSILON,
        "the 8 pt floor must hold: {size}"
    );
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
// Submit intent (extracted from the legacy UI band's inline branch)
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
    // the legacy UI's `if !prompt.is_empty()` guard: no queue message *and* no clear.
    assert!(submit_commands(true, "").is_empty());
    assert!(submit_commands(true, "   ").is_empty());
}

#[test]
fn submit_trims_only_trailing_whitespace_like_the_legacy_band() {
    // the legacy UI called `command_input.trim_end()` — leading indentation the user
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
    // While the agent runs, everything is queued verbatim — the legacy UI band
    // never routed `/models` through the parser in that branch.
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
    assert!(
        (8.0..=12.0).contains(&size),
        "sized within the helper's band"
    );
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
