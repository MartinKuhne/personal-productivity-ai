//! Sidecar tests for [`crate::ui_iced::agent`] composition (UI-048).
//!
//! Verifies the top-level `agent_panel` builder assembles without a live
//! iced runtime across the idle ⇄ running and empty ⇄ populated transitions,
//! and that the header close intent matches the shared `ClearAgentSession`
//! command.
//!
//! No files, no bus, no agent threads and no user paths ([RUST-006]).

use crate::bus::events::user_command::UserCommand;
use crate::ui_iced::message::Message;
use crate::ui_iced::panels::top::ChatModelEntry;
use hoarfrost::widget::text_input_model::TextInputModel;

use super::{AgentPanelViewModel, TranscriptTurn, agent_panel, close_message};

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
fn close_message_is_clear_agent_session() {
    assert_eq!(
        close_message(),
        Message::Command(UserCommand::ClearAgentSession)
    );
}

#[test]
fn agent_panel_builds_empty() {
    let vm = base_vm();
    let _ = agent_panel(&vm);
}

#[test]
fn agent_panel_builds_with_conversation() {
    let mut vm = base_vm();
    vm.turns.push(TranscriptTurn::User {
        text: "what is this?".into(),
    });
    vm.turns.push(TranscriptTurn::assistant(
        "It is **a document**.\n\n```rust\nfn main() {}\n```",
    ));
    vm.thinking = "considering the request".to_string();
    vm.status = "Streaming".to_string();
    vm.running = true;
    vm.chat_models = vec![
        ChatModelEntry {
            name: "a".into(),
            cost: 1,
        },
        ChatModelEntry {
            name: "b".into(),
            cost: 2,
        },
    ];
    vm.current_chat_model = Some("a".into());
    vm.command_input = TextInputModel::new("next").with_clearable(false);
    vm.queued_prompts = vec!["q1".into()];
    let _ = agent_panel(&vm);
}

#[test]
fn agent_panel_builds_collapsed_thinking() {
    let mut vm = base_vm();
    vm.thinking = "hidden reasoning".to_string();
    vm.thinking_expanded = false;
    let _ = agent_panel(&vm);
}

#[test]
fn re_exports_are_reachable() {
    // Compile-time reachability of the module's public surface.
    let _ = super::thinking_section_visible("x");
    let _ = super::turns_from_content("hi");
    let _ = super::send_enabled("x", false);
    let _ = super::queue_display(&[]);
    let _ = super::AGENT_TRANSCRIPT_SCROLLABLE_ID;
    let _: super::TurnRole = super::TurnRole::User;
    let _: super::TranscriptBlock = super::TranscriptBlock::Content {
        text: "x".to_string(),
    };
    let _: super::AgentPanelState = super::AgentPanelState::new();
}
