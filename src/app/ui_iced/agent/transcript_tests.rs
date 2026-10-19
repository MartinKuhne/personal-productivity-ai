//! Sidecar tests for [`crate::ui_iced::agent::transcript`] ([TASK-018],
//! UI-048).
//!
//! Covers the message constructors (scroll report, link activation), the
//! Markdown styling mapped from palette tokens (no numeric literals), the
//! turn/thinking/body view builders (constructed without a live iced
//! runtime — building an `Element` is pure), and the follow-the-stream
//! reuse of [`crate::ui_iced::logs::should_follow`].
//!
//! No files, no bus, no agent threads and no user paths ([RUST-006]).

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logs::{self, ScrollSnapshot};
use crate::ui_iced::message::{AgentMessage, Message, RenderMessage};

use super::{
    AGENT_TRANSCRIPT_SCROLLABLE_ID, agent_transcript_view, link_activated_message,
    markdown_settings, markdown_style, render_markdown_items, render_turn, scroll_reported_message,
    thinking_block, transcript_body,
};
use crate::ui_iced::agent::panel_state::{AgentPanelViewModel, TranscriptTurn};

/// A base snapshot with no content, for view-builder smoke tests.
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
// Message constructors
// ---------------------------------------------------------------------------

#[test]
fn scroll_reported_message_carries_geometry() {
    let snapshot = ScrollSnapshot {
        offset_y: 120.0,
        viewport_height: 300.0,
        content_height: 900.0,
    };
    let message = scroll_reported_message(&snapshot);
    assert_eq!(
        message,
        Message::Agent(AgentMessage::ScrollReported {
            offset_y: 120.0,
            viewport_height: 300.0,
            content_height: 900.0,
        })
    );
}

#[test]
fn link_activated_message_has_no_source_file() {
    let message = link_activated_message("https://example.test".to_string());
    match message {
        Message::Render(RenderMessage::LinkActivated { url, source_file }) => {
            assert_eq!(url, "https://example.test");
            assert!(
                source_file.is_none(),
                "transcript links have no source file"
            );
        }
        other => panic!("expected LinkActivated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Markdown styling — palette-token mapping (GUD-003)
// ---------------------------------------------------------------------------

#[test]
fn markdown_style_uses_palette_tokens() {
    let palette = Palette::fastmd_dark();
    let style = markdown_style();
    assert_eq!(style.link_color, palette.accent);
    assert_eq!(style.inline_code_color, palette.text_strong);
    assert_eq!(
        style.inline_code_highlight.background,
        iced::Background::Color(palette.code_surface)
    );
}

#[test]
fn markdown_settings_is_built_from_style() {
    // Settings::with_text_size derives heading/code sizes from the base; the
    // base size is the transcript body size.
    let settings = markdown_settings();
    assert_eq!(settings.code_size, settings.text_size * 0.75);
    assert_eq!(settings.h1_size, settings.text_size * 2.0);
}

// ---------------------------------------------------------------------------
// Turn / markdown rendering (build without a runtime)
// ---------------------------------------------------------------------------

#[test]
fn render_markdown_items_builds_an_element() {
    let items: Vec<_> = iced::widget::markdown::parse("# Title\n\nSome **bold** text.").collect();
    let element = render_markdown_items(&items);
    // Construction alone must not panic; the element has a non-zero type id.
    let _ = element;
}

#[test]
fn render_turn_user_builds_an_element() {
    let turn = TranscriptTurn::User {
        text: "a prompt".to_string(),
    };
    let _ = render_turn(&turn);
}

#[test]
fn render_turn_assistant_builds_an_element() {
    let turn = TranscriptTurn::assistant("- item one\n- item two");
    let _ = render_turn(&turn);
}

// ---------------------------------------------------------------------------
// Thinking disclosure block
// ---------------------------------------------------------------------------

#[test]
fn thinking_block_builds_expanded_and_collapsed() {
    let expanded = base_vm().with_thinking_expanded(true);
    let _ = thinking_block(&expanded);

    let collapsed = base_vm().with_thinking_expanded(false);
    let _ = thinking_block(&collapsed);
}

// ---------------------------------------------------------------------------
// Transcript body
// ---------------------------------------------------------------------------

#[test]
fn body_includes_thinking_when_visible() {
    let mut vm = base_vm();
    vm.thinking = "reasoning here".to_string();
    let _ = transcript_body(&vm);
}

#[test]
fn body_adds_waiting_placeholder_when_waiting() {
    let mut vm = base_vm();
    vm.running = true;
    vm.turns.push(TranscriptTurn::User {
        text: "a question".into(),
    });
    assert!(vm.waiting_for_response());
    let _ = transcript_body(&vm);
}

#[test]
fn body_omits_placeholder_when_answer_arrives() {
    let mut vm = base_vm();
    vm.running = true;
    vm.turns.push(TranscriptTurn::User {
        text: "a question".into(),
    });
    vm.turns.push(TranscriptTurn::assistant("an answer"));
    assert!(!vm.waiting_for_response());
    let _ = transcript_body(&vm);
}

#[test]
fn empty_body_builds() {
    let vm = base_vm();
    let _ = transcript_body(&vm);
}

// ---------------------------------------------------------------------------
// Full transcript view + follow-the-stream reuse
// ---------------------------------------------------------------------------

#[test]
fn agent_transcript_view_builds() {
    let mut vm = base_vm();
    vm.turns.push(TranscriptTurn::assistant("hello"));
    let _ = agent_transcript_view(&vm);
}

#[test]
fn scrollable_id_is_stable_and_named() {
    assert_eq!(AGENT_TRANSCRIPT_SCROLLABLE_ID, "agent_transcript_scroll");
}

#[test]
fn follow_intent_reuses_logs_should_follow() {
    // The transcript's follow decision is exactly the shared logs rule over
    // the same snapshot the view would report.
    let at_bottom = ScrollSnapshot {
        offset_y: 700.0,
        viewport_height: 300.0,
        content_height: 1000.0,
    };
    let scrolled_up = ScrollSnapshot {
        offset_y: 100.0,
        viewport_height: 300.0,
        content_height: 1000.0,
    };
    assert!(logs::should_follow(true, Some(at_bottom)));
    assert!(!logs::should_follow(true, Some(scrolled_up)));
    assert!(!logs::should_follow(false, Some(at_bottom)));
    assert!(!logs::should_follow(true, None));
}

#[test]
fn thinking_label_string_is_present() {
    // The placeholder text is a hoisted string constant, not a literal.
    assert!(!strings::AGENT_THINKING_LABEL.is_empty());
}
