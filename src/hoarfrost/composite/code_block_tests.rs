use super::*;
use crate::binding::Commit;
use crate::test_helpers::Candidate;
use crate::test_helpers::{
    Capture, Harness, KIND_CONTAINER, RecordCandidates, assert_repainted, centre, headless_renderer,
};
use crate::token::{shape, size, space};
use iced::Point;
use iced::advanced::widget::operation::black_box;
use iced::advanced::widget::tree::State as TreeState;

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

// ---------------------------------------------------------------------------
// Helpers for the widget-level contract
// ---------------------------------------------------------------------------

/// Marker stashed in a child tree to see whether `diff` kept it or rebuilt it.
const SLOT_MARKER: u32 = 0xC0DE;

/// Commit the block's own source text, so a test can compare what the button
/// emitted against what the model holds.
fn commit_code(commit: Commit<String>) -> String {
    commit.into_value()
}

/// A block tall enough that no other box can be mistaken for the 24px button.
fn sample_model() -> CodeBlockModel {
    CodeBlockModel::new("fn main() {\n    let answer = 42;\n    println!(\"{answer}\");\n}")
}

/// The box the copy button published into the operation stream.
///
/// Recovered from the tree rather than hard-coded, so the hit test keeps
/// working when the header padding or the viewport changes.
fn copy_button_box<Message: Send + 'static>(
    harness: &mut Harness<'_, Message, iced::Renderer>,
) -> Rectangle {
    let boxes: Vec<Rectangle> = harness
        .candidates()
        .into_iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .filter(|bounds| bounds.width == COPY_BUTTON_SIZE && bounds.height == COPY_BUTTON_SIZE)
        .collect();

    assert_eq!(
        boxes.len(),
        1,
        "the copy button must publish exactly one {COPY_BUTTON_SIZE}px box, found {boxes:?}"
    );
    boxes[0]
}

/// Every box a laid-out widget publishes through the operation stream.
fn published_boxes<Message, Theme, Renderer: renderer::Renderer>(
    widget: &mut dyn Widget<Message, Theme, Renderer>,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
) -> Vec<Candidate> {
    let mut probe = RecordCandidates::default();
    {
        let probe: &mut dyn Operation<Vec<Candidate>> = &mut probe;
        let mut erased = black_box(probe);
        widget.operate(tree, layout, renderer, &mut erased);
    }
    probe.candidates()
}

// ---------------------------------------------------------------------------
// The widget's own contract with the host
// ---------------------------------------------------------------------------

#[test]
fn test_the_widget_asks_to_fill_the_width_and_shrink_to_its_content() {
    let widget = code_block::<()>(sample_model());
    let requested = widget.size();

    assert_eq!(
        requested.width,
        Length::Fill,
        "a code block spans the width it is given (UIF-PRIN-004)"
    );
    assert_eq!(
        requested.height,
        Length::Shrink,
        "a code block is as tall as its lines and must not stretch to fill"
    );
}

#[test]
fn test_the_widget_hands_the_host_exactly_one_child_slot() {
    let widget = code_block::<()>(sample_model());
    let slots = widget.children();

    assert_eq!(
        slots.len(),
        1,
        "the composite wraps a single content element, so it must publish a single child tree"
    );
}

#[test]
fn test_diff_rebuilds_a_missing_child_slot_and_keeps_an_unchanged_one() {
    let widget = code_block::<()>(sample_model());

    let mut tree = Tree::empty();
    assert!(
        tree.children.is_empty(),
        "an empty tree carries no child slot to diff"
    );

    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against a tree with no children must create the content slot"
    );

    tree.children[0].state = TreeState::new(SLOT_MARKER);
    widget.diff(&mut tree);

    assert_eq!(
        tree.children.len(),
        1,
        "diffing an unchanged content element must neither add nor drop a slot"
    );
    assert_eq!(
        tree.children[0].state.downcast_ref::<u32>(),
        &SLOT_MARKER,
        "the content widget did not change, so its child state must survive the diff"
    );
}

// ---------------------------------------------------------------------------
// The copy button as a real control
// ---------------------------------------------------------------------------

#[test]
fn test_a_copy_button_with_a_callback_commits_the_code_on_release() {
    let model = sample_model().with_language("rust");
    let expected = model.code.clone();

    let mut harness =
        Harness::headless(move || code_block(model.clone()).on_copy(commit_code).into());

    let button = copy_button_box(&mut harness);
    let on_button = centre(button);

    assert!(
        harness.hover(on_button).is_empty(),
        "moving the pointer over the copy button must not copy anything"
    );
    assert!(
        harness.press(on_button).is_empty(),
        "the copy is committed on release, not on press"
    );

    let emitted = harness.release(on_button);
    assert_eq!(
        emitted,
        vec![expected],
        "releasing over the copy button commits the block's own source text"
    );

    harness.render();
}

#[test]
fn test_a_copy_button_without_a_callback_keeps_its_slot_and_commits_nothing() {
    let mut harness = Harness::headless(|| {
        let block: Element<'_, ()> = code_block(sample_model()).into();
        block
    });

    let button = copy_button_box(&mut harness);
    let on_button = centre(button);

    harness.hover(on_button);
    assert!(
        harness.press(on_button).is_empty(),
        "a block built without `on_copy` has no message to emit on press"
    );
    assert!(
        harness.release(on_button).is_empty(),
        "releasing an inert copy button must not fabricate a message"
    );

    harness.render();
}

#[test]
fn test_a_release_outside_the_copy_button_aborts_the_click_without_wedging_it() {
    let model = sample_model();
    let expected = model.code.clone();

    let mut harness =
        Harness::headless(move || code_block(model.clone()).on_copy(commit_code).into());

    let button = copy_button_box(&mut harness);
    let on_button = centre(button);
    let beside = Point::new(button.x - 30.0, button.y + button.height / 2.0);

    harness.press(on_button);
    assert!(
        harness.release(beside).is_empty(),
        "a release outside the button must abort the click, not commit the copy"
    );

    harness.press(on_button);
    assert_eq!(
        harness.release(on_button),
        vec![expected],
        "an aborted click must leave the button ready to commit the next one"
    );
}

#[test]
fn test_the_copy_slot_stays_put_across_badge_and_copied_states() {
    let base = sample_model();

    let mut harness = Harness::headless({
        let model = base.clone();
        move || code_block(model.clone()).on_copy(commit_code).into()
    });
    let resting = copy_button_box(&mut harness);
    harness.render();

    let badged = base.clone().with_language("rust");
    harness.set_view(move || code_block(badged.clone()).on_copy(commit_code).into());
    let with_badge = copy_button_box(&mut harness);
    harness.render();

    let mut copied_model = base.clone();
    copied_model.mark_copied();
    harness.set_view(move || {
        let block: Element<'_, String> =
            code_block(copied_model.clone()).on_copy(commit_code).into();
        block
    });
    let copied = copy_button_box(&mut harness);
    harness.render();

    assert_eq!(
        resting, with_badge,
        "the language badge must not shift the fixed copy slot (UIF-PRIN-003)"
    );
    assert_eq!(
        resting, copied,
        "the copied glyph must swap in place with a 0px shift (UIF-PRIN-003)"
    );
}

#[test]
fn test_the_copied_state_replaces_the_glyph_in_place() {
    let mut capture = Capture::new();
    let viewport = Size::new(240.0, 120.0);

    let resting_model = CodeBlockModel::new("let x = 42;").with_language("rust");
    let resting = capture.frame(
        viewport,
        code_block(resting_model.clone())
            .on_copy(commit_code)
            .into(),
    );

    let mut copied_model = resting_model.clone();
    copied_model.mark_copied();
    let copied = capture.frame(
        viewport,
        code_block(copied_model).on_copy(commit_code).into(),
    );

    // The header pins the slot to the top-right corner: 6px of right padding
    // outside a 24px button centred in the 28px bar.
    let slot = Rectangle::new(
        Point::new(
            viewport.width - space::XS - COPY_BUTTON_SIZE,
            (HEADER_HEIGHT - COPY_BUTTON_SIZE) / 2.0,
        ),
        Size::new(COPY_BUTTON_SIZE, COPY_BUTTON_SIZE),
    );

    assert!(
        resting.has_ink(slot),
        "the resting block must paint the clipboard glyph inside its slot"
    );
    assert!(
        copied.has_ink(slot),
        "the copied block must paint the checkmark inside the same slot"
    );
    assert_repainted(
        &resting,
        &copied,
        slot,
        "the copy button swapping the clipboard glyph for the checkmark",
    );
    assert_eq!(
        resting.physical_size(),
        copied.physical_size(),
        "confirming a copy must not resize the block"
    );
}

// ---------------------------------------------------------------------------
// Cursor and overlay delegation
// ---------------------------------------------------------------------------

#[test]
fn test_the_cursor_turns_into_a_pointer_only_over_the_copy_button() {
    let model = sample_model().with_language("rust");
    let twin: Element<'_, String> = code_block(model.clone()).on_copy(commit_code).into();
    let mut tree = Tree::new(&twin);

    let mut widget = code_block(model).on_copy(commit_code);
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    let widget_dyn: &mut dyn Widget<String, iced::Theme, iced::Renderer> = &mut widget;
    let button = published_boxes(widget_dyn, &mut tree, layout, &renderer)
        .into_iter()
        .find(|candidate| {
            candidate.kind == KIND_CONTAINER
                && candidate.bounds.width == COPY_BUTTON_SIZE
                && candidate.bounds.height == COPY_BUTTON_SIZE
        })
        .expect("the laid-out block publishes the copy button's box")
        .bounds;

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(button)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "the copy button is clickable, so it must ask for the pointer cursor"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(Point::new(4.0, node.size().height - 4.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "the code body is not interactive, so it must not ask for a pointer"
    );

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "a code block has no popover to hand the host"
    );
}
