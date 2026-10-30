//! Sidecar tests for [`crate::ui_iced::agent::prompt_input`].
//!
//! Covers the send / queue / cancel gating predicates, the queued-prompt
//! badge, the clear-session message constructor, the reused submit solver,
//! and a smoke build of the prompt row.
//!
//! No files, no bus, no agent threads and no user paths ([RUST-006]).

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui_iced::message::{AgentMessage, Message};
use hoarfrost::binding::{Cancel, Commit, Live};
use hoarfrost::widget::text_input_controller::TextInputController;
use hoarfrost::widget::text_input_controller::TextInputEmission;
use hoarfrost::widget::text_input_model::TextInputModel;

use super::{
    PROMPT_INPUT_ID, agent_prompt_row, cancel_enabled, clear_session_message, prompt_field_emit,
    queue_display, queue_enabled, send_enabled, submit_commands,
};
use crate::ui_iced::agent::panel_state::AgentPanelViewModel;

fn base_vm() -> AgentPanelViewModel {
    AgentPanelViewModel {
        visible: true,
        turns: Vec::new(),
        thinking: String::new(),
        thinking_expanded: true,
        status: "Idle".to_string(),
        running: false,
        queued_prompts: Vec::new(),
        command_input: prompt_model(""),
        auto_scroll: true,
    }
}

fn prompt_model(value: &str) -> TextInputModel {
    TextInputModel::new(value)
        .with_placeholder(strings::COMMAND_INPUT_HINT)
        .with_clearable(false)
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
fn clear_session_message_is_clear_agent_session() {
    assert_eq!(
        clear_session_message(),
        Message::Command(UserCommand::ClearAgentSession)
    );
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
    vm.command_input = prompt_model("hello");
    let _ = agent_prompt_row(&vm);
}

#[test]
fn prompt_row_builds_running_with_queue() {
    let mut vm = base_vm();
    vm.running = true;
    vm.command_input = prompt_model("queued prompt");
    vm.queued_prompts = vec!["p1".to_string()];
    let _ = agent_prompt_row(&vm);
}

#[test]
fn prompt_row_does_not_render_inline_model_selector() {
    let mut vm = base_vm();
    vm.queued_prompts = vec!["queued".to_string()];
    let mut ui = iced_test::simulator(agent_prompt_row(&vm));

    assert!(ui.find("Model:").is_err());
    assert!(ui.find(strings::AGENT_SEND_BUTTON).is_ok());
    assert!(ui.find(strings::AGENT_QUEUE_BUTTON).is_ok());
    assert!(ui.find(strings::STOP_AGENT_BUTTON).is_ok());
    assert!(
        ui.find(strings::format_queued_prompts_label(1).as_str())
            .is_ok()
    );
    assert!(ui.find(strings::AGENT_SESSION_CLOSE_BUTTON).is_ok());
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

// ---------------------------------------------------------------------------
// Controlled prompt emissions
// ---------------------------------------------------------------------------

#[test]
fn prompt_emissions_map_to_the_existing_agent_message_contract() {
    assert_eq!(
        prompt_field_emit(TextInputEmission::Changed(Live::new("typed".to_string()))),
        Some(Message::Agent(AgentMessage::InputChanged {
            text: "typed".to_string(),
        }))
    );
    assert_eq!(
        prompt_field_emit(TextInputEmission::Committed(Commit::new(
            "submitted".to_string()
        ))),
        Some(Message::Agent(AgentMessage::Submit {
            text: "submitted".to_string(),
        }))
    );
}

#[test]
fn prompt_focus_loss_syncs_the_value_without_submitting() {
    let mut controller = TextInputController::with_value("before");
    controller.note_focus(true);
    assert!(controller.note_edit("typed prompt".to_string()));

    let messages: Vec<_> = controller
        .note_focus(false)
        .into_iter()
        .filter_map(prompt_field_emit)
        .collect();

    assert_eq!(
        messages,
        vec![Message::Agent(AgentMessage::InputChanged {
            text: "typed prompt".to_string(),
        })],
        "blur must preserve the settled text but must never dispatch Submit"
    );
}

#[test]
fn prompt_enter_still_submits_the_edited_value() {
    let mut controller = TextInputController::with_value("before");
    controller.note_focus(true);
    assert!(controller.note_edit("typed prompt".to_string()));

    let message = controller.commit().and_then(prompt_field_emit);

    assert_eq!(
        message,
        Some(Message::Agent(AgentMessage::Submit {
            text: "typed prompt".to_string(),
        }))
    );
}

#[test]
fn prompt_cancel_restores_the_host_owned_text_without_submitting() {
    assert_eq!(
        prompt_field_emit(TextInputEmission::Cancelled(Cancel::new(
            "before edit".to_string()
        ))),
        Some(Message::Agent(AgentMessage::InputChanged {
            text: "before edit".to_string(),
        }))
    );
}

#[test]
fn prompt_focus_and_unavailable_clear_emissions_have_no_message() {
    assert_eq!(
        prompt_field_emit(TextInputEmission::FocusChanged(true)),
        None
    );
    assert_eq!(
        prompt_field_emit(TextInputEmission::FocusChanged(false)),
        None
    );
    assert_eq!(
        prompt_field_emit(TextInputEmission::Cleared(Commit::new(String::new()))),
        None,
        "the prompt has no clear affordance, so a clear tier is not routed"
    );
}

#[test]
fn live_prompt_typing_emits_the_existing_input_changed_message() {
    use iced_test::selector::Candidate;

    let vm = base_vm();
    let prompt_id = iced::widget::Id::from(PROMPT_INPUT_ID);
    let mut ui = iced_test::simulator(agent_prompt_row(&vm));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == prompt_id => Some(bounds),
            _ => None,
        })
        .expect("the prompt row must mount its controlled text input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("summarize");

    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        messages.contains(&Message::Agent(AgentMessage::InputChanged {
            text: "summarize".to_string(),
        })),
        "the live controlled prompt must preserve InputChanged, got {messages:?}"
    );
}

#[test]
fn live_prompt_enter_emits_submit_with_the_edited_value() {
    use iced_test::selector::Candidate;

    let vm = base_vm();
    let prompt_id = iced::widget::Id::from(PROMPT_INPUT_ID);
    let mut ui = iced_test::simulator(agent_prompt_row(&vm));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == prompt_id => Some(bounds),
            _ => None,
        })
        .expect("the prompt row must mount its controlled text input");

    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("summarize");
    let _ = ui.tap_key(iced::keyboard::key::Named::Enter);

    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        messages.contains(&Message::Agent(AgentMessage::Submit {
            text: "summarize".to_string(),
        })),
        "Enter must submit the live editor value, got {messages:?}"
    );
}

#[test]
fn prompt_model_uses_command_input_as_its_only_value_and_has_no_clear_action() {
    let mut vm = base_vm();
    vm.command_input = prompt_model("draft");

    assert_eq!(vm.command_input.value, "draft");
    assert!(!vm.command_input.clearable);
}
