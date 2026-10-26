use super::*;
use crate::binding::Commit;
use crate::token::{shape, size};

#[test]
fn test_code_block_model_defaults_and_builder() {
    let model = CodeBlockModel::new("fn main() {\n    println!(\"Hello!\");\n}");
    assert_eq!(model.code, "fn main() {\n    println!(\"Hello!\");\n}");
    assert_eq!(model.language, None);
    assert!(model.show_line_numbers);
    assert_eq!(model.copied_hold_remaining_ms, 0);
    assert!(!model.is_copied());

    let rust_model = model.clone().with_language("rust").with_line_numbers(false);
    assert_eq!(rust_model.language.as_deref(), Some("rust"));
    assert!(!rust_model.show_line_numbers);
}

#[test]
fn test_line_count_calculations() {
    let empty = CodeBlockModel::new("");
    assert_eq!(empty.line_count(), 1);

    let single = CodeBlockModel::new("let x = 42;");
    assert_eq!(single.line_count(), 1);

    let multi = CodeBlockModel::new("line 1\nline 2\nline 3");
    assert_eq!(multi.line_count(), 3);

    let trailing = CodeBlockModel::new("line 1\nline 2\n");
    assert_eq!(trailing.line_count(), 2);
}

#[test]
fn test_mark_copied_and_tick_copied_hold_duration() {
    let mut model = CodeBlockModel::new("let a = 1;");
    assert!(!model.is_copied());
    assert_eq!(model.copied_hold_remaining_ms, 0);

    model.mark_copied();
    assert!(model.is_copied());
    assert_eq!(model.copied_hold_remaining_ms, 1500);
    assert_eq!(model.copied_hold_remaining_ms, HOLD_COPIED_MS);

    // Tick by 500ms
    model.tick_copied(500);
    assert!(model.is_copied());
    assert_eq!(model.copied_hold_remaining_ms, 1000);

    // Tick by 999ms
    model.tick_copied(999);
    assert!(model.is_copied());
    assert_eq!(model.copied_hold_remaining_ms, 1);

    // Tick by 1ms -> reaches 0
    model.tick_copied(1);
    assert!(!model.is_copied());
    assert_eq!(model.copied_hold_remaining_ms, 0);

    // Further ticks saturate at 0
    model.tick_copied(100);
    assert_eq!(model.copied_hold_remaining_ms, 0);
}

#[test]
fn test_geometry_constants_conform_to_spec() {
    assert_eq!(HEADER_HEIGHT, size::CONTROL_SM);
    assert_eq!(HEADER_HEIGHT, 28.0);

    assert_eq!(COPY_BUTTON_SIZE, size::HIT_TARGET_MIN);
    assert_eq!(COPY_BUTTON_SIZE, 24.0);

    assert_eq!(GUTTER_WIDTH, 40.0);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(RADIUS, 4.0);

    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(BORDER_WIDTH, 1.0);

    assert_eq!(HOLD_COPIED_MS, 1500);
    assert_eq!(GLYPH_COPY, "📋");
    assert_eq!(GLYPH_COPIED, "✓");
}

#[test]
fn test_build_content_with_and_without_language() {
    let model_no_lang = CodeBlockModel::new("let x = 1;");
    let _elem1: Element<'_, ()> = build_code_block_content(&model_no_lang, None);

    let model_with_lang = CodeBlockModel::new("let x = 1;").with_language("rust");
    let _elem2: Element<'_, ()> = build_code_block_content(&model_with_lang, None);
}

#[test]
fn test_build_content_with_and_without_line_numbers() {
    let model_gutter = CodeBlockModel::new("let x = 1;\nlet y = 2;");
    let _elem1: Element<'_, ()> = build_code_block_content(&model_gutter, None);

    let model_no_gutter = model_gutter.with_line_numbers(false);
    let _elem2: Element<'_, ()> = build_code_block_content(&model_no_gutter, None);
}

#[test]
fn test_widget_creation_and_callbacks() {
    let model = CodeBlockModel::new("const PI: f64 = 3.14159;").with_language("rust");
    let mut copied_text: Option<String> = None;

    let widget = code_block(model.clone()).on_copy(|commit: Commit<String>| commit.into_value());

    assert_eq!(widget.model().code, "const PI: f64 = 3.14159;");
    assert_eq!(widget.model().language.as_deref(), Some("rust"));

    if let Some(on_copy) = &widget.on_copy {
        let msg = (on_copy)(Commit::new(model.code.clone()));
        copied_text = Some(msg);
    }
    assert_eq!(copied_text.as_deref(), Some("const PI: f64 = 3.14159;"));
}

#[test]
fn test_from_code_block_into_element() {
    let model = CodeBlockModel::new("echo hello");
    let widget = code_block(model);
    let _element: Element<'_, ()> = widget.into();
}

#[test]
fn test_copy_button_slot_and_0px_shift_during_copied_hold() {
    // Both resting and copied states use fixed 24x24 hit target and slot dimensions
    let mut model = CodeBlockModel::new("code");
    assert!(!model.is_copied());

    let resting = build_code_block_content::<()>(&model, None);
    assert_eq!(resting.as_widget().size().width, Length::Fill);

    model.mark_copied();
    assert!(model.is_copied());
    let copied = build_code_block_content::<()>(&model, None);
    assert_eq!(copied.as_widget().size().width, Length::Fill);
}
