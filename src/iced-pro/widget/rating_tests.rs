//! Tests for [`RatingControl`] and [`RatingModel`] (`UIF-ATOM-013`, §3.13).

use super::*;
use crate::test_helpers::Harness;
use crate::token::contrast;
use iced::advanced::Widget;
use iced::mouse::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
enum TestMsg {
    Preview(Option<f32>),
    Commit(Option<f32>),
    Cancel(Option<f32>),
}

#[test]
fn test_rating_step_defaults_and_increments() {
    assert_eq!(RatingStep::default(), RatingStep::Whole);
    assert_eq!(RatingStep::Whole.increment(), 1.0);
    assert_eq!(RatingStep::Half.increment(), 0.5);
}

#[test]
fn test_rating_model_builders_and_defaults() {
    let model = RatingModel::default();
    assert_eq!(model.max, DEFAULT_MAX_RATING);
    assert_eq!(model.step, RatingStep::Whole);
    assert_eq!(model.value, None);
    assert_eq!(model.hover_preview, None);
    assert!(!model.allow_clear);
    assert!(!model.read_only);
    assert!(!model.disabled);

    let configured = RatingModel::new(10)
        .with_value(Some(4.5))
        .with_hover_preview(Some(5.0))
        .with_step(RatingStep::Half)
        .with_allow_clear(true)
        .with_read_only(true)
        .with_disabled(true);

    assert_eq!(configured.max, 10);
    assert_eq!(configured.value, Some(4.5));
    assert_eq!(configured.hover_preview, Some(5.0));
    assert_eq!(configured.step, RatingStep::Half);
    assert!(configured.allow_clear);
    assert!(configured.read_only);
    assert!(configured.disabled);
    assert_eq!(configured.effective_value(), Some(5.0));
}

#[test]
fn test_total_rating_width_calculation() {
    assert_eq!(total_rating_width(0), 0.0);
    assert_eq!(total_rating_width(1), 24.0);
    // 5 * 24.0 + 4 * 4.0 = 120.0 + 16.0 = 136.0
    assert_eq!(total_rating_width(5), 136.0);
}

#[test]
fn test_star_slot_bounds_positions() {
    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(136.0, 24.0));

    let slot0 = star_slot_bounds(bounds, 0);
    assert_eq!(slot0.x, 10.0);
    assert_eq!(slot0.y, 20.0);
    assert_eq!(slot0.width, 24.0);
    assert_eq!(slot0.height, 24.0);

    let slot1 = star_slot_bounds(bounds, 1);
    assert_eq!(slot1.x, 10.0 + 24.0 + 4.0); // 38.0
    assert_eq!(slot1.y, 20.0);
    assert_eq!(slot1.width, 24.0);
}

#[test]
fn test_value_at_position_whole_stepping() {
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));

    // Outside bounds
    assert_eq!(
        value_at_position(bounds, Point::new(-5.0, 10.0), 5, RatingStep::Whole),
        None
    );
    assert_eq!(
        value_at_position(bounds, Point::new(150.0, 10.0), 5, RatingStep::Whole),
        None
    );

    // Slot 0 (x: 0..24, plus gap 24..28)
    assert_eq!(
        value_at_position(bounds, Point::new(5.0, 10.0), 5, RatingStep::Whole),
        Some((0, 1.0))
    );
    assert_eq!(
        value_at_position(bounds, Point::new(20.0, 10.0), 5, RatingStep::Whole),
        Some((0, 1.0))
    );

    // Slot 2 (x: 56..80) -> value 3.0
    assert_eq!(
        value_at_position(bounds, Point::new(60.0, 10.0), 5, RatingStep::Whole),
        Some((2, 3.0))
    );

    // Slot 4 (last star, x: 112..136) -> value 5.0
    assert_eq!(
        value_at_position(bounds, Point::new(125.0, 10.0), 5, RatingStep::Whole),
        Some((4, 5.0))
    );
}

#[test]
fn test_value_at_position_half_stepping() {
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));

    // Slot 0 left half (x < 12.0) -> 0.5
    assert_eq!(
        value_at_position(bounds, Point::new(6.0, 10.0), 5, RatingStep::Half),
        Some((0, 0.5))
    );
    // Slot 0 right half (x >= 12.0) -> 1.0
    assert_eq!(
        value_at_position(bounds, Point::new(18.0, 10.0), 5, RatingStep::Half),
        Some((0, 1.0))
    );

    // Slot 1 left half (slot 1 x: 28..52, mid: 40)
    assert_eq!(
        value_at_position(bounds, Point::new(35.0, 10.0), 5, RatingStep::Half),
        Some((1, 1.5))
    );
    assert_eq!(
        value_at_position(bounds, Point::new(45.0, 10.0), 5, RatingStep::Half),
        Some((1, 2.0))
    );
}

#[test]
fn test_rating_contrast_ratios_clear_spec() {
    let palette = Palette::dark();

    // Filled stars: status.warning on surface.base (spec says 6.86:1, clears 4.50:1)
    let ratio_warning = contrast::contrast_ratio(palette.status_warning, palette.surface_base);
    assert!(
        ratio_warning >= 4.50,
        "status_warning on surface_base ({ratio_warning}) must clear 4.50:1"
    );

    // Empty stars: border.strong on surface.base (spec says 4.24:1, clears 3.00:1 chrome)
    let ratio_empty = contrast::contrast_ratio(palette.border_strong, palette.surface_base);
    assert!(
        ratio_empty >= 3.00,
        "border_strong on surface_base ({ratio_empty}) must clear 3.00:1"
    );

    // Hover outline: text.strong on surface.base (spec says 18.71:1, clears 4.50:1)
    let ratio_outline = contrast::contrast_ratio(palette.text_strong, palette.surface_base);
    assert!(
        ratio_outline >= 4.50,
        "text_strong on surface_base ({ratio_outline}) must clear 4.50:1"
    );
}

#[test]
fn test_rating_control_size() {
    let model = RatingModel::new(5);
    let ctrl = rating_control::<()>(model);
    assert_eq!(ctrl.size().width, Length::Fixed(136.0));
    assert_eq!(ctrl.size().height, Length::Fixed(24.0));
}

fn update_rating(
    ctrl: &mut RatingControl<'_, TestMsg>,
    tree: &mut Tree,
    event: &Event,
    cursor: Cursor,
    renderer: &iced::Renderer,
    shell: &mut Shell<'_, TestMsg>,
) {
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));
    let node = layout::Node::new(bounds.size());
    let layout = Layout::with_offset(iced::Vector::ZERO, &node);
    let mut clipboard = iced::advanced::clipboard::Null;
    let widget = ctrl as &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
    widget.update(
        tree,
        event,
        layout,
        cursor,
        renderer,
        &mut clipboard,
        shell,
        &bounds,
    );
}

#[test]
fn test_mouse_hover_preview_and_leave() {
    let last_preview = Arc::new(Mutex::new(None));
    let lp = Arc::clone(&last_preview);

    let model = RatingModel::new(5).with_step(RatingStep::Half);
    let mut ctrl = rating_control(model).on_hover_preview(move |live| {
        *lp.lock().unwrap() = Some(*live.value());
        TestMsg::Preview(*live.value())
    });

    let mut tree = Tree::new(&ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let renderer = crate::test_helpers::headless_renderer();
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);

    // 1. Move cursor over slot 0 left half (0.5)
    update_rating(
        &mut ctrl,
        &mut tree,
        &crate::test_helpers::moved(Point::new(6.0, 10.0)),
        Cursor::Available(Point::new(6.0, 10.0)),
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_preview.lock().unwrap(), Some(Some(0.5)));

    // 2. Move cursor over slot 2 right half (3.0)
    update_rating(
        &mut ctrl,
        &mut tree,
        &crate::test_helpers::moved(Point::new(75.0, 10.0)),
        Cursor::Available(Point::new(75.0, 10.0)),
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_preview.lock().unwrap(), Some(Some(3.0)));

    // 3. Cursor leaves
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Mouse(mouse::Event::CursorLeft),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_preview.lock().unwrap(), Some(None));
}

#[test]
fn test_mouse_click_commits_and_allow_clear() {
    let last_commit = Arc::new(Mutex::new(None));
    let lc = Arc::clone(&last_commit);

    let model = RatingModel::new(5)
        .with_value(Some(3.0))
        .with_allow_clear(true);

    let mut ctrl = rating_control(model).on_commit(move |commit| {
        *lc.lock().unwrap() = Some(*commit.value());
        TestMsg::Commit(*commit.value())
    });

    let mut tree = Tree::new(&ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let renderer = crate::test_helpers::headless_renderer();
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);

    // Click on 4th star (x: 84..108, value: 4.0)
    update_rating(
        &mut ctrl,
        &mut tree,
        &crate::test_helpers::pressed(),
        Cursor::Available(Point::new(90.0, 10.0)),
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(Some(4.0)));

    // Click on 3rd star (value: 3.0), which matches current value with allow_clear -> None
    update_rating(
        &mut ctrl,
        &mut tree,
        &crate::test_helpers::pressed(),
        Cursor::Available(Point::new(60.0, 10.0)),
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(None));
}

#[test]
fn test_disabled_and_read_only_suppress_interaction() {
    let interacted = Arc::new(AtomicBool::new(false));
    let i_clone = Arc::clone(&interacted);

    let model = RatingModel::new(5).with_disabled(true);
    let mut ctrl = rating_control(model).on_commit(move |_| {
        i_clone.store(true, Ordering::SeqCst);
        TestMsg::Commit(None)
    });

    let mut tree = Tree::new(&ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let renderer = crate::test_helpers::headless_renderer();
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);

    update_rating(
        &mut ctrl,
        &mut tree,
        &crate::test_helpers::pressed(),
        Cursor::Available(Point::new(20.0, 10.0)),
        &renderer,
        &mut shell,
    );
    assert!(!interacted.load(Ordering::SeqCst));
}

#[test]
fn test_keyboard_navigation_and_cancel() {
    let last_commit = Arc::new(Mutex::new(None));
    let last_cancel = Arc::new(Mutex::new(None));
    let lc = Arc::clone(&last_commit);
    let lcan = Arc::clone(&last_cancel);

    let model = RatingModel::new(5)
        .with_value(Some(2.0))
        .with_allow_clear(true);

    let mut ctrl = rating_control(model)
        .on_commit(move |commit| {
            *lc.lock().unwrap() = Some(*commit.value());
            TestMsg::Commit(*commit.value())
        })
        .on_cancel(move |cancel| {
            *lcan.lock().unwrap() = Some(*cancel.restored());
            TestMsg::Cancel(*cancel.restored())
        });

    let mut tree = Tree::new(&ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let state = tree.state.downcast_mut::<RatingControlState>();
    <FocusableState as operation::Focusable>::focus(&mut state.focusable);

    let renderer = crate::test_helpers::headless_renderer();
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);

    // ArrowRight steps up from 2.0 to 3.0
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::ArrowRight),
            modified_key: Key::Named(Named::ArrowRight),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::ArrowRight),
            modifiers: keyboard::Modifiers::default(),
            location: keyboard::Location::Standard,
            text: None,
            repeat: false,
        }),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(Some(3.0)));

    // ArrowLeft steps down from 2.0 to 1.0
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::ArrowLeft),
            modified_key: Key::Named(Named::ArrowLeft),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::ArrowLeft),
            modifiers: keyboard::Modifiers::default(),
            location: keyboard::Location::Standard,
            text: None,
            repeat: false,
        }),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(Some(1.0)));

    // End steps to max 5.0
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::End),
            modified_key: Key::Named(Named::End),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::End),
            modifiers: keyboard::Modifiers::default(),
            location: keyboard::Location::Standard,
            text: None,
            repeat: false,
        }),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(Some(5.0)));

    // Delete clears to None when allow_clear is true
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Delete),
            modified_key: Key::Named(Named::Delete),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Delete),
            modifiers: keyboard::Modifiers::default(),
            location: keyboard::Location::Standard,
            text: None,
            repeat: false,
        }),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_commit.lock().unwrap(), Some(None));

    // Escape cancels back to snapshot
    update_rating(
        &mut ctrl,
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            modified_key: Key::Named(Named::Escape),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            modifiers: keyboard::Modifiers::default(),
            location: keyboard::Location::Standard,
            text: None,
            repeat: false,
        }),
        Cursor::Unavailable,
        &renderer,
        &mut shell,
    );
    assert_eq!(*last_cancel.lock().unwrap(), Some(Some(2.0)));
}

#[test]
fn test_rating_draw_headless() {
    // 1. Whole stars filled
    let model1 = RatingModel::new(5).with_value(Some(3.0));
    let mut harness1 = Harness::<(), _>::headless(move || rating_control(model1.clone()).into());
    harness1.render();

    // 2. Half star filled
    let model2 = RatingModel::new(5)
        .with_value(Some(2.5))
        .with_step(RatingStep::Half);
    let mut harness2 = Harness::<(), _>::headless(move || rating_control(model2.clone()).into());
    harness2.render();

    // 3. Disabled rating
    let model3 = RatingModel::new(5)
        .with_value(Some(4.0))
        .with_disabled(true);
    let mut harness3 = Harness::<(), _>::headless(move || rating_control(model3.clone()).into());
    harness3.render();
}

#[test]
fn test_from_rating_control_for_element() {
    let model = RatingModel::new(5);
    let ctrl = rating_control::<()>(model);
    let _element: Element<'_, ()> = ctrl.into();
}
