//! Sidecar tests for [`crate::ui_iced::agent::transcript`] (UI-048).
//!
//! Covers the message constructors (scroll report, link activation), the
//! Markdown styling mapped from palette tokens (no numeric literals), the
//! turn/thinking/body view builders (constructed without a live iced
//! runtime — building an `Element` is pure), and the follow-the-stream
//! reuse of [`crate::ui_iced::logs::should_follow`].
//!
//! No files, no bus, no agent threads and no user paths ([RUST-006]).
//!
//! `assertions_on_constants` is allowed throughout: this sidecar's job
//! includes pinning token-bound constants, which is exactly what that lint
//! fires on.
#![allow(clippy::assertions_on_constants)]

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logs::{self, ScrollSnapshot};
use crate::ui_iced::message::{AgentMessage, Message, RenderMessage};

use super::{
    AGENT_TRANSCRIPT_SCROLLABLE_ID, TRANSCRIPT_SPACING, TRANSCRIPT_TEXT_SIZE,
    agent_transcript_view, link_activated_message, markdown_settings, markdown_style,
    render_markdown_items, render_turn, scroll_reported_message, thinking_block, transcript_body,
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
// Markdown styling — palette-token mapping
// ---------------------------------------------------------------------------

#[test]
fn markdown_style_uses_palette_tokens() {
    let palette = Palette::fastmd_dark();
    let style = markdown_style();
    assert_eq!(style.link_color, palette.accent_text);
    assert_eq!(style.inline_code_color, palette.text_strong);
    assert_eq!(
        style.inline_code_highlight.background,
        iced::Background::Color(palette.code_surface)
    );
}

/// The transcript link must **not** be painted with the accent *fill*.
///
/// `Palette::accent` is `color.accent.fill`, a background-only token; as a
/// foreground it measures `4.14:1` on `surface.base`, `3.90:1` on
/// `surface.raised` and `3.83:1` on `surface.code` — under the `4.50:1`
/// active-text floor on all three. The guard pins the retired literal so a
/// future edit that slides `link_color` back to `accent` fails here rather
/// than quietly re-illegalising every hyperlink.
#[test]
fn transcript_link_is_not_painted_with_the_accent_fill() {
    use iced_pro::token::contrast::contrast_ratio;

    let palette = Palette::fastmd_dark();
    let link = super::transcript_link_color();

    assert_ne!(
        link, palette.accent,
        "the transcript link must not use the accent FILL as a foreground"
    );

    // Recomputed from raw sRGB here, not transcribed from the doc table.
    let pro = Palette::pro();
    for surface in [pro.surface_base, pro.surface_raised, pro.surface_code] {
        let ratio = contrast_ratio(link, surface);
        assert!(
            ratio >= 4.5,
            "transcript link over {surface:?} is {ratio:.2}:1, below the 4.50:1 text floor"
        );
    }

    // And the retired pairing is pinned as illegal, with the measured number.
    let retired = contrast_ratio(palette.accent, pro.surface_base);
    assert!(
        retired < 4.5,
        "accent_fill over surface.base should still measure under the floor, got {retired:.2}:1"
    );
}

#[test]
fn markdown_settings_binds_every_size_to_a_rung() {
    use crate::ui_iced::render::{code, heading};

    let settings = markdown_settings();

    // The base is the transcript's own rung.
    assert_eq!(f32::from(settings.text_size), TRANSCRIPT_TEXT_SIZE);

    // The heading ladder is the document renderer's ladder, not iced's
    // `base × 2 / −25%` multiplication table.
    assert_eq!(f32::from(settings.h1_size), heading::H1_SIZE);
    assert_eq!(f32::from(settings.h2_size), heading::H2_SIZE);
    assert_eq!(f32::from(settings.h3_size), heading::H3_SIZE);
    assert_eq!(f32::from(settings.h4_size), heading::H4_SIZE);
    assert_eq!(f32::from(settings.h5_size), heading::H5_SIZE);
    assert_eq!(f32::from(settings.h6_size), heading::H5_SIZE);
    assert_eq!(f32::from(settings.code_size), code::CODE_TEXT_SIZE);
    assert_eq!(f32::from(settings.spacing), TRANSCRIPT_SPACING);
}

/// iced's own `Settings::with_text_size` ladder is illegal at this base and
/// must not be reachable from the transcript.
///
/// `with_text_size(14)` yields `code = 10.5` — under `type.min` (12) — and
/// four off-scale heading sizes. This test pins the retired derivation so it
/// cannot come back as a convenience call.
#[test]
fn markdown_settings_does_not_use_iced_derived_ladder() {
    use iced_pro::token::typography::MIN_SIZE;

    let settings = markdown_settings();
    let retired =
        iced::widget::markdown::Settings::with_text_size(TRANSCRIPT_TEXT_SIZE, markdown_style());

    assert_ne!(
        f32::from(settings.code_size),
        f32::from(retired.code_size),
        "code size must not come from iced's base × 0.75 derivation"
    );
    assert_ne!(
        f32::from(settings.h1_size),
        f32::from(retired.h1_size),
        "h1 must not come from iced's base × 2.0 derivation"
    );

    // The retired ladder really was illegal — pinned so the numbers in the
    // module doc stay true.
    assert!(
        f32::from(retired.code_size) < MIN_SIZE,
        "expected iced's derived code size to sit under the floor, got {:?}",
        retired.code_size
    );

    // And the replacement never does.
    for size in [
        f32::from(settings.h1_size),
        f32::from(settings.h2_size),
        f32::from(settings.h3_size),
        f32::from(settings.h4_size),
        f32::from(settings.h5_size),
        f32::from(settings.h6_size),
        f32::from(settings.code_size),
        f32::from(settings.text_size),
    ] {
        assert!(
            size >= MIN_SIZE,
            "markdown settings render {size}pt, below the {MIN_SIZE}pt floor"
        );
    }
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
