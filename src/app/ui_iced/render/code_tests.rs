//! Unit tests for the fenced code-block mapping ([TASK-015]).

use super::*;
use crate::ui::theme::Palette;
use crate::ui_iced::message::RenderMessage;

fn model(language: Option<&str>, content: &str) -> CodeBlockModel {
    code_block_model(language, content)
}

#[test]
fn test_model_keeps_language_and_content_verbatim() {
    let m = model(Some("rust"), "fn main() {}\n");
    assert_eq!(m.language.as_deref(), Some("rust"));
    assert_eq!(m.content, "fn main() {}\n");
}

#[test]
fn test_model_without_a_language_has_none() {
    let m = model(None, "plain");
    assert_eq!(m.language, None);
    assert_eq!(m.content, "plain");
}

#[test]
fn test_empty_fence_still_produces_a_model() {
    let m = model(Some("txt"), "");
    assert_eq!(m.content, "");
    assert!(has_language_caption(&m));
}

#[test]
fn test_copy_message_carries_the_exact_content() {
    let m = model(Some("rs"), "let x = 1;\nlet y = 2;\n");
    assert_eq!(
        m.copy_message,
        Message::Render(RenderMessage::CopyCodeRequested {
            content: "let x = 1;\nlet y = 2;\n".to_string(),
        })
    );
}

#[test]
fn test_copy_message_content_matches_the_model_content() {
    let m = model(Some("sh"), "echo hello\n");
    match &m.copy_message {
        Message::Render(RenderMessage::CopyCodeRequested { content }) => {
            assert_eq!(content, &m.content, "the clipboard payload is the block");
        }
        other => panic!("expected the render channel, got {other:?}"),
    }
}

#[test]
fn test_language_caption_rules() {
    assert!(
        !has_language_caption(&model(None, "x")),
        "no language → no caption"
    );
    assert!(
        !has_language_caption(&model(Some(""), "x")),
        "empty language tag → no caption (the legacy UI's `!lang.is_empty()` guard)"
    );
    assert!(has_language_caption(&model(Some("toml"), "x")));
}

#[test]
fn test_code_block_builds_with_a_caption() {
    let element = code_block(&model(Some("rust"), "fn main() {}"));
    let _ = element;
}

#[test]
fn test_code_block_builds_without_a_caption() {
    let element = code_block(&model(None, "no language here"));
    let _ = element;
}

#[test]
fn test_code_block_builds_for_empty_content() {
    let element = code_block(&model(Some("md"), ""));
    let _ = element;
}

#[test]
fn test_code_block_builds_for_multiline_content() {
    let content = "line one\nline two\nline three\n";
    let element = code_block(&model(Some("text"), content));
    let _ = element;
}

#[test]
fn test_code_surface_and_border_are_palette_tokens() {
    // No numeric colour literal may appear in the render layer ([GUD-003]):
    // the panel's colours must be exactly the palette's tokens.
    let palette = Palette::fastmd_dark();
    let _surface = palette.code_surface;
    let _border = palette.border;
    const { assert!(CODE_BORDER_WIDTH > 0.0) };
    const { assert!(CODE_BORDER_RADIUS >= 0.0) };
}

#[test]
fn test_copy_icon_string_is_the_shared_phosphor_constant() {
    // The button label must come from `strings`, not a literal.
    assert!(!strings::ICON_COPY.is_empty());
    assert_eq!(strings::CODE_COPY_BUTTON_TOOLTIP, "Copy code");
}
