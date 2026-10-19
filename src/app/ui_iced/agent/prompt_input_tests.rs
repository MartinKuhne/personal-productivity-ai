//! Sidecar tests for [`crate::ui_iced::agent::prompt_input`] ([TASK-018],
//! UI-001 / UI-064 / UI-065).
//!
//! Covers the send / queue / cancel gating predicates, the queued-prompt
//! badge, the model-pick and clear-session message constructors, the
//! current-model option lookup, the reused submit solver, and a smoke build
//! of the prompt row.
//!
//! No files, no bus, no agent threads and no user paths ([RUST-006]).

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui_iced::message::{AgentMessage, Message};
use crate::ui_iced::panels::top::ChatModelEntry;

use super::{
    agent_prompt_row, cancel_enabled, clear_session_message, current_model_option, queue_display,
    queue_enabled, select_model_message, send_enabled, submit_commands,
};
use crate::ui_iced::agent::panel_state::AgentPanelViewModel;

fn entry(name: &str, cost: i32) -> ChatModelEntry {
    ChatModelEntry {
        name: name.to_string(),
        cost,
    }
}

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
        command_input: String::new(),
        auto_scroll: true,
    }
}

// ---------------------------------------------------------------------------
// Send / queue / cancel gating
// ---------------------------------------------------------------------------

#[test]
fn send_requires_nonempty_prompt_and_idle() {
    assert!(!send_enabled("", false));
    assert!(!send_enabled("   ", false));
    assert!(send_enabled("do it", false));
    assert!(
        !send_enabled("do it", true),
        "running blocks Send (use Queue)"
    );
}

#[test]
fn queue_requires_nonempty_prompt_and_running() {
    assert!(!queue_enabled("", true));
    assert!(!queue_enabled("  ", true));
    assert!(queue_enabled("do it", true));
    assert!(!queue_enabled("do it", false), "idle does not queue");
}

#[test]
fn cancel_only_while_running() {
    assert!(cancel_enabled(true));
    assert!(!cancel_enabled(false));
}

/// The agent window's always-allocated Stop label must read as inactive
/// while idle, matching the bottom band (`stop_button_color`).
#[test]
fn cancel_button_label_drops_to_muted_while_idle() {
    let palette = crate::ui::theme::Palette::fastmd_dark();
    assert_eq!(
        crate::ui_iced::panels::bottom::stop_button_color(true),
        palette.error
    );
    assert_eq!(
        crate::ui_iced::panels::bottom::stop_button_color(false),
        palette.text_muted
    );
}

#[test]
fn send_and_queue_are_mutually_exclusive_for_a_prompt() {
    for running in [false, true] {
        let prompt = "hello";
        assert_ne!(
            send_enabled(prompt, running),
            queue_enabled(prompt, running),
            "exactly one of Send/Queue fires for a non-empty prompt"
        );
    }
}

// ---------------------------------------------------------------------------
// Queue badge
// ---------------------------------------------------------------------------

#[test]
fn queue_display_none_when_empty() {
    assert_eq!(queue_display(&[]), None);
}

#[test]
fn queue_display_counts_parked_prompts() {
    let queued = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    assert_eq!(
        queue_display(&queued),
        Some(strings::format_queued_prompts_label(3))
    );
    assert_eq!(queue_display(&queued).unwrap(), "Queued: 3");
}

// ---------------------------------------------------------------------------
// Message constructors
// ---------------------------------------------------------------------------

#[test]
fn select_model_message_carries_name() {
    assert_eq!(
        select_model_message("gpt-4o"),
        Message::Command(UserCommand::SelectChatModel("gpt-4o".to_string()))
    );
}

#[test]
fn clear_session_message_is_clear_agent_session() {
    assert_eq!(
        clear_session_message(),
        Message::Command(UserCommand::ClearAgentSession)
    );
}

// ---------------------------------------------------------------------------
// Current-model lookup
// ---------------------------------------------------------------------------

#[test]
fn current_model_option_matches_by_name() {
    let options = vec![entry("a", 1), entry("b", 2)];
    let found = current_model_option(&options, Some("b")).unwrap();
    assert_eq!(found.name, "b");
}

#[test]
fn current_model_option_none_when_absent() {
    let options = vec![entry("a", 1)];
    assert!(current_model_option(&options, Some("missing")).is_none());
}

#[test]
fn current_model_option_none_when_no_current() {
    let options = vec![entry("a", 1)];
    assert!(current_model_option(&options, None).is_none());
}

// ---------------------------------------------------------------------------
// Reused submit solver (the reducer's decision, exercised here)
// ---------------------------------------------------------------------------

#[test]
fn submit_idle_runs_agent_then_clears() {
    let commands = submit_commands(false, "summarize this");
    assert_eq!(
        commands,
        vec![
            UserCommand::RunAgent("summarize this".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
}

#[test]
fn submit_running_queues_then_clears() {
    let commands = submit_commands(true, "next thing");
    assert_eq!(
        commands,
        vec![
            UserCommand::QueueAgentPrompt("next thing".to_string()),
            UserCommand::ClearCommandInput,
        ]
    );
}

#[test]
fn submit_blank_produces_nothing() {
    assert!(submit_commands(false, "   ").is_empty());
    assert!(submit_commands(true, "").is_empty());
}

// ---------------------------------------------------------------------------
// Prompt row smoke build
// ---------------------------------------------------------------------------

#[test]
fn prompt_row_builds_idle() {
    let mut vm = base_vm();
    vm.command_input = "hello".to_string();
    vm.chat_models = vec![entry("a", 1), entry("b", 2)];
    vm.current_chat_model = Some("a".to_string());
    let _ = agent_prompt_row(&vm);
}

#[test]
fn prompt_row_builds_running_with_queue() {
    let mut vm = base_vm();
    vm.running = true;
    vm.command_input = "queued prompt".to_string();
    vm.queued_prompts = vec!["p1".to_string()];
    let _ = agent_prompt_row(&vm);
}

#[test]
fn prompt_row_builds_with_no_models() {
    let vm = base_vm();
    let _ = agent_prompt_row(&vm);
}

#[test]
fn submit_message_from_row_is_agent_submit() {
    // The Send/Enter path emits AgentMessage::Submit; the reducer turns it
    // into the commands asserted above. Confirm the message shape here.
    let message = Message::Agent(AgentMessage::Submit {
        text: "x".to_string(),
    });
    match message {
        Message::Agent(AgentMessage::Submit { text }) => assert_eq!(text, "x"),
        other => panic!("expected Submit, got {other:?}"),
    }
}
