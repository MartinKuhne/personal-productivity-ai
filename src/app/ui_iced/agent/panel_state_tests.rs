//! Sidecar tests for [`crate::ui_iced::agent::panel_state`] (UI-048).
//!
//! Covers the turn→render-model mapping (user vs assistant, Markdown
//! passthrough, the `**User:** ` marker convention), the UI-048
//! thinking-delimiter gate (and its agreement with the center panel's copy),
//! the waiting-for-first-response decision, and the chat-model option
//! builder.
//!
//! Fixtures build plain strings and [`AppConfig`]/[`LlmConfig`] values
//! directly — no files, no bus, no agent threads and no user paths
//! ([RUST-006]).

use std::collections::HashMap;

use crate::config::{AppConfig, LlmConfig};
use crate::ui::strings;
use iced_pro::widget::text_input_model::TextInputModel;

use super::{
    AgentPanelViewModel, TranscriptTurn, TurnRole, chat_model_options, split_user_turn,
    thinking_section_visible, transcript_is_empty, turns_from_content, waiting_for_first_response,
};

/// A model config with the given cost and use cases.
fn llm(cost: i32, use_cases: &[&str]) -> LlmConfig {
    LlmConfig {
        model: "test-model".to_string(),
        api_url: "https://example.test".to_string(),
        api_key: "k".to_string(),
        cost: Some(cost),
        use_case: use_cases.iter().map(|s| s.to_string()).collect(),
    }
}

/// An AppConfig whose `models` map is exactly `models`.
fn config_with(models: &[(&str, LlmConfig)]) -> AppConfig {
    let mut map = HashMap::new();
    for (name, cfg) in models {
        map.insert((*name).to_string(), cfg.clone());
    }
    AppConfig {
        models: map,
        ..AppConfig::default()
    }
}

// ---------------------------------------------------------------------------
// split_user_turn
// ---------------------------------------------------------------------------

#[test]
fn split_user_turn_recognises_marker_at_start() {
    let (user, rest) = split_user_turn("**User:** do the thing\n\nAssistant reply").unwrap();
    assert_eq!(user, "do the thing");
    assert_eq!(rest, "Assistant reply");
}

#[test]
fn split_user_turn_with_no_trailing_blank_line() {
    let (user, rest) = split_user_turn("**User:** just a prompt").unwrap();
    assert_eq!(user, "just a prompt");
    assert_eq!(rest, "");
}

#[test]
fn split_user_turn_returns_none_without_marker() {
    assert!(split_user_turn("plain assistant prose").is_none());
}

#[test]
fn split_user_turn_ignores_mid_line_marker() {
    // The marker is not at a paragraph start, so it is not a turn boundary.
    assert!(split_user_turn("see the **User:** field here").is_none());
}

#[test]
fn split_user_turn_trims_the_prompt() {
    let (user, _rest) = split_user_turn("**User:**   padded   \n\nx").unwrap();
    assert_eq!(user, "padded");
}

// ---------------------------------------------------------------------------
// turns_from_content — the turn→render-model mapping
// ---------------------------------------------------------------------------

#[test]
fn turns_from_empty_content_is_empty() {
    assert!(turns_from_content("").is_empty());
    assert!(turns_from_content("   \n\n  ").is_empty());
}

#[test]
fn turns_from_pure_assistant_content_passthrough() {
    let turns = turns_from_content("Hello **world**\n\n- a list");
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].role(), TurnRole::Assistant);
    assert_eq!(turns[0].text(), "Hello **world**\n\n- a list");
    // The assistant turn carries parsed items for the view.
    match &turns[0] {
        TranscriptTurn::Assistant { items, markdown } => {
            assert_eq!(markdown, "Hello **world**\n\n- a list");
            assert!(!items.is_empty(), "assistant turn must own parsed items");
        }
        TranscriptTurn::User { .. } => panic!("expected assistant turn"),
    }
}

#[test]
fn turns_from_continuation_splits_user_and_assistant() {
    let content = format!("\n\n{}do it\n\nDone.\n", strings::AGENT_USER_TURN_MARKER);
    let turns = turns_from_content(&content);
    assert_eq!(turns.len(), 2);
    assert!(turns[0].is_user());
    assert_eq!(turns[0].text(), "do it");
    assert_eq!(turns[1].role(), TurnRole::Assistant);
    assert_eq!(turns[1].text(), "Done.\n");
}

#[test]
fn turns_interleave_leading_assistant_then_user_then_assistant() {
    let content = format!(
        "intro line\n\n{}question\n\nanswer",
        strings::AGENT_USER_TURN_MARKER
    );
    let turns = turns_from_content(&content);
    assert_eq!(
        turns.iter().map(|t| t.role()).collect::<Vec<_>>(),
        vec![TurnRole::Assistant, TurnRole::User, TurnRole::Assistant]
    );
    // The leading assistant span keeps the injected separator's newlines —
    // harmless paragraph whitespace for the Markdown renderer.
    assert_eq!(turns[0].text(), "intro line\n\n");
    assert_eq!(turns[1].text(), "question");
    assert_eq!(turns[2].text(), "answer");
}

#[test]
fn turns_drop_blank_assistant_spans() {
    // Two adjacent markers with only whitespace between them produce no
    // empty assistant turn in between.
    let content = format!(
        "{}one\n\n\n\n{}two\n\nend",
        strings::AGENT_USER_TURN_MARKER,
        strings::AGENT_USER_TURN_MARKER
    );
    let turns = turns_from_content(&content);
    // User(one), User(two), Assistant(end) — no blank assistant between.
    assert_eq!(
        turns.iter().map(|t| t.role()).collect::<Vec<_>>(),
        vec![TurnRole::User, TurnRole::User, TurnRole::Assistant]
    );
}

#[test]
fn turns_keep_truncation_marker_in_assistant_span() {
    // The bus-lag truncation marker is ordinary text in the buffer and must
    // ride along inside an assistant span (the bus-lag loss notice).
    let marker = crate::orchestrator::LAG_TRUNCATION_MARKER;
    let turns = turns_from_content(&format!("before\n\n{marker}\n\nafter"));
    assert_eq!(turns.len(), 1);
    assert!(turns[0].text().contains(marker));
}

// ---------------------------------------------------------------------------
// TranscriptTurn helpers
// ---------------------------------------------------------------------------

#[test]
fn transcript_turn_assistant_parses_items() {
    let turn = TranscriptTurn::assistant("**bold** and `code`");
    assert_eq!(turn.role(), TurnRole::Assistant);
    assert!(!turn.is_user());
    match &turn {
        TranscriptTurn::Assistant { items, markdown } => {
            assert_eq!(markdown, "**bold** and `code`");
            assert!(!items.is_empty());
        }
        TranscriptTurn::User { .. } => panic!("expected assistant turn"),
    }
}

#[test]
fn transcript_turn_user_helpers() {
    let turn = TranscriptTurn::User {
        text: "hi".to_string(),
    };
    assert!(turn.is_user());
    assert_eq!(turn.role(), TurnRole::User);
    assert_eq!(turn.text(), "hi");
}

// ---------------------------------------------------------------------------
// UI-048 thinking gate
// ---------------------------------------------------------------------------

#[test]
fn thinking_gate_empty_vs_nonempty() {
    assert!(!thinking_section_visible(""));
    assert!(thinking_section_visible("some reasoning"));
}

#[test]
fn thinking_gate_agrees_with_center_panel_copy() {
    // The agent-panel gate and the center-panel gate must be identical so the
    // two render paths cannot drift (RUST-041).
    for probe in ["", "x", "reasoning\nmulti-line", "   "] {
        assert_eq!(
            thinking_section_visible(probe),
            crate::ui_iced::panels::center::thinking_section_visible(probe),
            "gate disagreement on {probe:?}"
        );
    }
}

#[test]
fn transcript_is_empty_only_when_no_turns_and_no_thinking() {
    assert!(transcript_is_empty(&[], ""));
    assert!(!transcript_is_empty(&[], "thinking"));
    assert!(!transcript_is_empty(
        &[TranscriptTurn::User { text: "q".into() }],
        ""
    ));
}

// ---------------------------------------------------------------------------
// waiting_for_first_response
// ---------------------------------------------------------------------------

#[test]
fn waiting_when_running_with_no_turns() {
    assert!(waiting_for_first_response(&[], true));
}

#[test]
fn waiting_when_running_with_trailing_user_turn() {
    let turns = vec![
        TranscriptTurn::assistant("prior answer"),
        TranscriptTurn::User {
            text: "new question".into(),
        },
    ];
    assert!(waiting_for_first_response(&turns, true));
}

#[test]
fn not_waiting_when_assistant_output_present() {
    let turns = vec![
        TranscriptTurn::User { text: "q".into() },
        TranscriptTurn::assistant("a"),
    ];
    assert!(!waiting_for_first_response(&turns, true));
}

#[test]
fn never_waiting_when_idle() {
    assert!(!waiting_for_first_response(&[], false));
    assert!(!waiting_for_first_response(
        &[TranscriptTurn::User { text: "q".into() }],
        false
    ));
}

// ---------------------------------------------------------------------------
// chat_model_options
// ---------------------------------------------------------------------------

#[test]
fn chat_model_options_empty_config_is_empty() {
    assert!(chat_model_options(&config_with(&[])).is_empty());
}

#[test]
fn chat_model_options_filters_to_chat_use_case() {
    let config = config_with(&[
        ("chatA", llm(10, &["chat"])),
        ("visionOnly", llm(5, &["vision"])),
        ("chatB", llm(20, &["chat", "vision"])),
    ]);
    let options = chat_model_options(&config);
    let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["chatA", "chatB"]);
}

#[test]
fn chat_model_options_falls_back_to_all_when_no_chat_use_case() {
    let config = config_with(&[
        ("vision", llm(5, &["vision"])),
        ("embed", llm(1, &["embeddings"])),
    ]);
    let options = chat_model_options(&config);
    let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["embed", "vision"]);
}

#[test]
fn chat_model_options_sorted_by_name() {
    let config = config_with(&[
        ("zeta", llm(1, &["chat"])),
        ("alpha", llm(2, &["chat"])),
        ("mid", llm(3, &["chat"])),
    ]);
    let options = chat_model_options(&config);
    let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mid", "zeta"]);
}

// ---------------------------------------------------------------------------
// AgentPanelViewModel helpers (constructed directly, no FastMdApp)
// ---------------------------------------------------------------------------

fn base_vm() -> AgentPanelViewModel {
    AgentPanelViewModel {
        visible: true,
        turns: Vec::new(),
        thinking: String::new(),
        thinking_expanded: true,
        status: "Idle".to_string(),
        running: false,
        queued_prompts: Vec::new(),
        chat_models: Vec::new(),
        current_chat_model: None,
        command_input: TextInputModel::empty().with_clearable(false),
        auto_scroll: true,
    }
}

#[test]
fn vm_show_thinking_tracks_thinking_field() {
    let mut vm = base_vm();
    assert!(!vm.show_thinking());
    vm.thinking = "reasoning".to_string();
    assert!(vm.show_thinking());
}

#[test]
fn vm_is_empty_tracks_turns_and_thinking() {
    let mut vm = base_vm();
    assert!(vm.is_empty());
    vm.turns.push(TranscriptTurn::User { text: "q".into() });
    assert!(!vm.is_empty());
}

#[test]
fn vm_waiting_for_response_delegates_to_helper() {
    let mut vm = base_vm();
    vm.running = true;
    assert!(vm.waiting_for_response());
    vm.turns.push(TranscriptTurn::assistant("answer"));
    assert!(!vm.waiting_for_response());
}

#[test]
fn vm_with_thinking_expanded_overrides_field() {
    let vm = base_vm().with_thinking_expanded(false);
    assert!(!vm.thinking_expanded);
    let vm = vm.with_thinking_expanded(true);
    assert!(vm.thinking_expanded);
}

// ---------------------------------------------------------------------------
// from_app — visibility gate
// ---------------------------------------------------------------------------

/// Regression guard: while the agent session is off screen the transcript
/// must not be re-parsed through `markdown::parse` and the thinking buffer
/// must not be cloned — doing either during a workspace scan is pure
/// UI-thread waste.
#[test]
fn from_app_skips_the_transcript_parse_while_not_visible() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.agent_panel_state.show_results = false;
    app.orchestrator.agent_transcript.content =
        "**User:** question\n\nSome long assistant answer with `code`.".to_string();
    app.orchestrator.agent_transcript.thinking = "deep thought".to_string();

    let vm = AgentPanelViewModel::from_app(&app);

    assert!(!vm.visible);
    assert!(
        vm.turns.is_empty(),
        "an off-screen session must not re-parse the transcript"
    );
    assert!(
        vm.thinking.is_empty(),
        "an off-screen session must not clone the thinking buffer"
    );
}

/// Starting a session must produce the full transcript again — the gate may not
/// strand the panel on an empty snapshot.
#[test]
fn from_app_parses_the_transcript_once_the_session_is_visible() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.agent_panel_state.show_results = false;
    app.orchestrator.agent_transcript.content =
        "**User:** question\n\nAn assistant answer.".to_string();
    app.orchestrator.agent_transcript.thinking = "deep thought".to_string();

    assert!(AgentPanelViewModel::from_app(&app).turns.is_empty());

    app.orchestrator.agent_panel_state.show_results = true;
    let vm = AgentPanelViewModel::from_app(&app);

    assert!(vm.visible);
    assert_eq!(vm.turns.len(), 2, "a visible session parses both turns");
    assert_eq!(vm.thinking, "deep thought");
}

#[test]
fn from_app_snapshots_command_input_into_the_prompt_model() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.agent_panel_state.command_input = "draft prompt".to_string();

    let vm = AgentPanelViewModel::from_app(&app);

    assert_eq!(vm.command_input.value, "draft prompt");
    assert_eq!(vm.command_input.placeholder, strings::COMMAND_INPUT_HINT);
    assert!(!vm.command_input.clearable);
}
