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

/// **Defect 03, fixed.** `RatingControl::new` minted `Id::unique()` on every
/// construction, so an id read in one frame addressed nothing in the next and
/// `focus(id)` landed nowhere. It now claims a stable default name, like the
/// pickers do.
#[test]
fn the_rating_control_keeps_one_id_across_rebuilds() {
    let mut harness = Harness::<(), _>::headless(|| {
        rating_control::<()>(RatingModel::new(5).with_value(Some(3.0))).into()
    });

    let first = harness.registered();
    let second = harness.registered();
    assert_eq!(
        first, second,
        "defect 03: two rebuilds of the same rating must register the same id"
    );

    let expected = iced::advanced::widget::Id::new(DEFAULT_RATING_CONTROL_ID);
    assert_eq!(
        first,
        vec![Some(expected.clone())],
        "defect 03: the control registers under the documented default name"
    );

    // A control built somewhere else claims the same name, so a bridge can
    // address it without holding a handle to the original.
    let mut other = Harness::<(), _>::headless(|| {
        rating_control::<()>(RatingModel::new(5).with_value(Some(3.0))).into()
    });
    assert_eq!(
        other.registered(),
        first,
        "defect 03: the default name is derived, not randomly minted per build"
    );
}

/// Regression test for defect 03 (fixed).
#[test]
fn id_names_the_rating_control_for_the_focus_router() {
    assert_eq!(
        rating_control::<()>(RatingModel::new(5)).widget_id(),
        &iced::advanced::widget::Id::new(DEFAULT_RATING_CONTROL_ID),
        "defect 03: widget_id reports the id the router sees"
    );

    let named = rating_control::<()>(RatingModel::new(5))
        .id(iced::advanced::widget::Id::new("rating.review"));
    assert_eq!(
        named.widget_id(),
        &iced::advanced::widget::Id::new("rating.review"),
        "defect 03: the router must see the name the host chose"
    );
}

// ===========================================================================
// Branch coverage added in the coverage push
//
// The cases below are grouped so that one headless renderer serves several
// claims: the widget-contract sweep, the pointer sweep, the keyboard sweep and
// the paint sweep each pay the renderer build once. Every assertion names the
// key, the gesture or the state it is about, so a failure still points at one
// behaviour.
// ===========================================================================

use crate::test_helpers::{Capture, assert_repainted};

/// A `KeyPressed` event carrying `key`.
fn key_pressed(key: Key) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        modifiers: keyboard::Modifiers::default(),
        location: keyboard::Location::Standard,
        text: None,
        repeat: false,
    })
}

/// A `KeyPressed` event for a named key.
fn named_key(key: Named) -> Event {
    key_pressed(Key::Named(key))
}

/// A `KeyPressed` event for a printable character.
fn char_key(ch: &str) -> Event {
    key_pressed(Key::Character(ch.into()))
}

/// A fresh [`Tree`] carrying the control's own state.
fn fresh_tree(ctrl: &RatingControl<'_, TestMsg>) -> Tree {
    Tree::new(ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>)
}

/// Hand the row's focusable state the keyboard focus.
fn focus_row(tree: &mut Tree) {
    let state = tree.state.downcast_mut::<RatingControlState>();
    <FocusableState as operation::Focusable>::focus(&mut state.focusable);
}

/// Whether the row currently holds the keyboard focus.
fn row_is_focused(tree: &Tree) -> bool {
    tree.state
        .downcast_ref::<RatingControlState>()
        .focusable
        .is_focused()
}

/// The hover bookkeeping the row carries between frames.
fn hover_state(tree: &Tree) -> (Option<usize>, Option<f32>) {
    let state = tree.state.downcast_ref::<RatingControlState>();
    (state.hovered_slot, state.hover_value)
}

/// Whether a gesture currently holds the row's slot.
fn gesture_is_open(tree: &Tree) -> bool {
    tree.state
        .downcast_ref::<RatingControlState>()
        .gesture
        .is_open()
}

/// [`update_rating`] with the box the test chooses instead of the default row.
fn update_rating_in(
    ctrl: &mut RatingControl<'_, TestMsg>,
    tree: &mut Tree,
    event: &Event,
    cursor: Cursor,
    renderer: &iced::Renderer,
    shell: &mut Shell<'_, TestMsg>,
    bounds: Rectangle,
) {
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

/// Deliver one pointer move to `at` over the row.
fn move_pointer(
    ctrl: &mut RatingControl<'_, TestMsg>,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    at: Point,
    messages: &mut Vec<TestMsg>,
) {
    let mut shell = Shell::new(messages);
    update_rating(
        ctrl,
        tree,
        &crate::test_helpers::moved(at),
        Cursor::Available(at),
        renderer,
        &mut shell,
    );
}

/// Deliver one keyboard event to a row that is not under the pointer.
fn press_key(
    ctrl: &mut RatingControl<'_, TestMsg>,
    tree: &mut Tree,
    event: &Event,
    renderer: &iced::Renderer,
    messages: &mut Vec<TestMsg>,
) {
    let mut shell = Shell::new(messages);
    update_rating(ctrl, tree, event, Cursor::Unavailable, renderer, &mut shell);
}

#[test]
fn test_value_at_position_rejects_an_empty_row_and_a_box_wider_than_the_stars() {
    let wide = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));

    assert_eq!(
        value_at_position(wide, Point::new(10.0, 10.0), 0, RatingStep::Whole),
        None,
        "a row with no stars has no value to point at"
    );
    assert_eq!(
        value_at_position(wide, Point::new(10.0, 10.0), 0, RatingStep::Half),
        None,
        "the stepping mode cannot rescue a row with no stars"
    );

    // One star occupies x 0..24 of a 136-wide box; everything past its gap
    // belongs to no star at all, even though the point is inside the box.
    assert_eq!(
        value_at_position(wide, Point::new(100.0, 10.0), 1, RatingStep::Whole),
        None,
        "a box wider than its stars must not invent a value past the last one"
    );
    assert_eq!(
        value_at_position(wide, Point::new(100.0, 10.0), 1, RatingStep::Half),
        None,
        "half stepping must not invent a value past the last star either"
    );

    // The far edge of the last star still belongs to the last star.
    assert_eq!(
        value_at_position(wide, Point::new(135.0, 10.0), 5, RatingStep::Whole),
        Some((4, 5.0)),
        "the far edge of the last star is the last star's own value"
    );

    // A point inside the box's width but below its height is on no star.
    assert_eq!(
        value_at_position(wide, Point::new(10.0, 30.0), 5, RatingStep::Whole),
        None,
        "a point below the row is not on a star"
    );
}

#[test]
fn test_value_at_position_clamps_a_half_step_to_a_single_star_row() {
    let one = Rectangle::new(Point::ORIGIN, Size::new(24.0, 24.0));

    assert_eq!(
        value_at_position(one, Point::new(6.0, 12.0), 1, RatingStep::Half),
        Some((0, 0.5)),
        "the left half of the only star is half a star"
    );
    assert_eq!(
        value_at_position(one, Point::new(20.0, 12.0), 1, RatingStep::Half),
        Some((0, 1.0)),
        "the right half of the only star is the whole row"
    );
    assert_eq!(
        value_at_position(one, Point::new(20.0, 12.0), 1, RatingStep::Whole),
        Some((0, 1.0)),
        "whole stepping over the only star cannot exceed the row"
    );
}

#[test]
fn test_the_rating_control_exposes_its_model_its_palette_and_its_identity() {
    let model = RatingModel::new(3)
        .with_value(Some(2.0))
        .with_step(RatingStep::Half);
    let ctrl = rating_control::<()>(model.clone());

    assert_eq!(
        ctrl.model(),
        &model,
        "model() hands back the model the row was built from"
    );
    assert_eq!(
        ctrl.palette,
        Palette::dark(),
        "a row that was given no palette paints with the dark one"
    );
    assert_eq!(
        ctrl.size().width,
        Length::Fixed(80.0),
        "a three-star row is three hit targets plus two gaps wide"
    );

    let mut tint = Palette::dark();
    tint.status_warning = Color::from_rgb(0.0, 1.0, 0.0);
    let painted = rating_control::<()>(RatingModel::new(5)).palette(tint);
    assert_eq!(
        painted.palette.status_warning,
        Color::from_rgb(0.0, 1.0, 0.0),
        "palette() replaces the tokens the row paints with"
    );

    let named = rating_control::<()>(RatingModel::new(5))
        .id(iced::advanced::widget::Id::new("rating.feedback"));
    assert_eq!(
        named.widget_id(),
        &iced::advanced::widget::Id::new("rating.feedback"),
        "widget_id() reports the name the host chose for the router"
    );
}

#[test]
fn test_the_widget_contract_reports_its_layout_size_cursor_and_default_state() {
    let renderer = crate::test_helpers::headless_renderer();
    let limits = layout::Limits::NONE;

    for (max, expected_width) in [(0_u8, 0.0_f32), (1, 24.0), (3, 80.0), (5, 136.0)] {
        let mut ctrl = rating_control::<TestMsg>(RatingModel::new(max));
        let mut tree = fresh_tree(&ctrl);
        let widget = &mut ctrl as &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
        let node = widget.layout(&mut tree, &renderer, &limits);

        assert_eq!(
            node.size().width,
            expected_width,
            "a {max}-star row lays itself out {expected_width} wide"
        );
        assert_eq!(
            node.size().height,
            24.0,
            "a {max}-star row is exactly one hit target tall"
        );
    }

    // The cursor the row asks for, in every combination of state and position.
    let ctrl = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(2.0)));
    let tree = fresh_tree(&ctrl);
    let node = layout::Node::new(Size::new(136.0, 24.0));
    let layout = Layout::with_offset(iced::Vector::ZERO, &node);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(400.0, 300.0));
    let widget = &ctrl as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>;

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            Cursor::Available(Point::new(10.0, 10.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "a pointer over the row asks for the pointer cursor"
    );
    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            Cursor::Available(Point::new(300.0, 200.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "a pointer off the row asks for nothing"
    );
    assert_eq!(
        widget.mouse_interaction(&tree, layout, Cursor::Unavailable, &viewport, &renderer),
        mouse::Interaction::None,
        "an unavailable cursor asks for nothing"
    );

    let read_only = rating_control::<TestMsg>(RatingModel::new(5).with_read_only(true));
    let read_only_tree = fresh_tree(&read_only);
    assert_eq!(
        (&read_only as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>).mouse_interaction(
            &read_only_tree,
            layout,
            Cursor::Available(Point::new(10.0, 10.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "a read-only row must not offer the pointer cursor"
    );

    let disabled = rating_control::<TestMsg>(RatingModel::new(5).with_disabled(true));
    let disabled_tree = fresh_tree(&disabled);
    assert_eq!(
        (&disabled as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>).mouse_interaction(
            &disabled_tree,
            layout,
            Cursor::Available(Point::new(10.0, 10.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "a disabled row must not offer the pointer cursor"
    );

    // The state a freshly built row starts from.
    let state = tree.state.downcast_ref::<RatingControlState>();
    assert!(
        !state.focusable.is_focused()
            && state.hovered_slot.is_none()
            && state.hover_value.is_none(),
        "a freshly built row is idle: unfocused and with nothing hovered"
    );
    assert!(
        !state.gesture.is_open(),
        "a freshly built row holds no open gesture"
    );

    let blank = RatingControlState::default();
    assert!(
        !blank.focusable.is_focused()
            && blank.hovered_slot.is_none()
            && blank.hover_value.is_none()
            && !blank.gesture.is_open(),
        "the default row state matches the state a fresh tree is given"
    );
}

#[test]
fn test_the_pointer_preview_lifecycle_across_moves_and_leaves() {
    let renderer = crate::test_helpers::headless_renderer();
    let mut ctrl = rating_control::<TestMsg>(RatingModel::new(5))
        .on_hover_preview(|live| TestMsg::Preview(*live.value()));
    let mut tree = fresh_tree(&ctrl);
    let mut messages: Vec<TestMsg> = Vec::new();

    move_pointer(
        &mut ctrl,
        &mut tree,
        &renderer,
        Point::new(60.0, 10.0),
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Preview(Some(3.0))],
        "the first move onto a star publishes that star as the preview"
    );
    assert_eq!(
        hover_state(&tree),
        (Some(2), Some(3.0)),
        "the row remembers which star sits under the pointer"
    );
    assert!(
        gesture_is_open(&tree),
        "previewing a star opens the gesture slot over it"
    );

    move_pointer(
        &mut ctrl,
        &mut tree,
        &renderer,
        Point::new(60.0, 10.0),
        &mut messages,
    );
    assert_eq!(
        messages.len(),
        1,
        "re-entering the star that is already previewed must not publish twice"
    );

    move_pointer(
        &mut ctrl,
        &mut tree,
        &renderer,
        Point::new(6.0, 10.0),
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Preview(Some(3.0)), TestMsg::Preview(Some(1.0))],
        "moving to another star re-publishes over the gesture that is already open"
    );
    assert_eq!(
        hover_state(&tree),
        (Some(0), Some(1.0)),
        "and the row moves its hover to the new star"
    );

    move_pointer(
        &mut ctrl,
        &mut tree,
        &renderer,
        Point::new(200.0, 10.0),
        &mut messages,
    );
    assert_eq!(
        messages.last(),
        Some(&TestMsg::Preview(None)),
        "leaving the row publishes an empty preview"
    );
    assert_eq!(
        hover_state(&tree),
        (None, None),
        "leaving the row drops the hover bookkeeping"
    );

    move_pointer(
        &mut ctrl,
        &mut tree,
        &renderer,
        Point::new(200.0, 10.0),
        &mut messages,
    );
    assert_eq!(
        messages.len(),
        3,
        "a second leave with nothing hovered must stay silent"
    );

    {
        let mut shell = Shell::new(&mut messages);
        update_rating(
            &mut ctrl,
            &mut tree,
            &Event::Mouse(mouse::Event::CursorLeft),
            Cursor::Unavailable,
            &renderer,
            &mut shell,
        );
    }
    assert_eq!(
        messages.len(),
        3,
        "a CursorLeft with nothing hovered must stay silent too"
    );

    // A row with nobody listening still tracks the hover, just says nothing.
    let mut quiet = rating_control::<TestMsg>(RatingModel::new(5));
    let mut quiet_tree = fresh_tree(&quiet);
    let mut quiet_messages: Vec<TestMsg> = Vec::new();
    {
        let mut shell = Shell::new(&mut quiet_messages);
        update_rating(
            &mut quiet,
            &mut quiet_tree,
            &crate::test_helpers::moved(Point::new(60.0, 10.0)),
            Cursor::Available(Point::new(60.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    assert!(
        quiet_messages.is_empty(),
        "a row with no preview callback publishes nothing"
    );
    assert_eq!(
        hover_state(&quiet_tree),
        (Some(2), Some(3.0)),
        "but the hover is recorded all the same"
    );
    assert!(
        gesture_is_open(&quiet_tree),
        "and the gesture slot opens even with nobody listening"
    );
}

#[test]
fn test_pointer_events_that_miss_the_stars_are_ignored() {
    let renderer = crate::test_helpers::headless_renderer();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));
    let mut ctrl = rating_control::<TestMsg>(RatingModel::new(0))
        .on_hover_preview(|live| TestMsg::Preview(*live.value()))
        .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut tree = fresh_tree(&ctrl);
    let mut messages: Vec<TestMsg> = Vec::new();
    let inside = Point::new(60.0, 10.0);

    // The box is wider than the row: a point inside it belongs to no star.
    {
        let mut shell = Shell::new(&mut messages);
        update_rating_in(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::moved(inside),
            Cursor::Available(inside),
            &renderer,
            &mut shell,
            bounds,
        );
    }
    assert!(
        messages.is_empty(),
        "a move over a row with no stars publishes no preview"
    );
    assert_eq!(
        hover_state(&tree),
        (None, None),
        "and records no hover to paint later"
    );

    {
        let mut shell = Shell::new(&mut messages);
        update_rating_in(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::pressed(),
            Cursor::Available(inside),
            &renderer,
            &mut shell,
            bounds,
        );
    }
    assert!(
        messages.is_empty(),
        "a press over a row with no stars commits nothing"
    );
    assert!(
        !gesture_is_open(&tree),
        "and opens no gesture to settle later"
    );

    {
        let mut shell = Shell::new(&mut messages);
        update_rating_in(
            &mut ctrl,
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Cursor::Available(inside),
            &renderer,
            &mut shell,
            bounds,
        );
    }
    assert!(
        messages.is_empty(),
        "a released button is left for somebody else to handle"
    );
}

#[test]
fn test_a_press_without_a_commit_callback_still_settles_the_gesture() {
    let renderer = crate::test_helpers::headless_renderer();
    let mut ctrl = rating_control::<TestMsg>(
        RatingModel::new(5)
            .with_value(Some(2.0))
            .with_allow_clear(true),
    );
    let mut tree = fresh_tree(&ctrl);
    let mut messages: Vec<TestMsg> = Vec::new();

    {
        let mut shell = Shell::new(&mut messages);
        update_rating(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::pressed(),
            Cursor::Available(Point::new(90.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    assert!(
        messages.is_empty(),
        "a row with no commit callback publishes nothing on a press"
    );
    assert!(
        !gesture_is_open(&tree),
        "the press still settles and closes the gesture it opened"
    );

    {
        let mut shell = Shell::new(&mut messages);
        update_rating(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::pressed(),
            Cursor::Available(Point::new(60.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    assert!(
        messages.is_empty(),
        "the clear-on-reclick path publishes nothing either"
    );

    {
        let mut shell = Shell::new(&mut messages);
        update_rating(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::pressed(),
            Cursor::Available(Point::new(300.0, 200.0)),
            &renderer,
            &mut shell,
        );
    }
    assert!(
        messages.is_empty(),
        "a press outside the row's box is not the row's business"
    );
    assert!(!gesture_is_open(&tree), "and leaves no gesture behind");
}

#[test]
fn test_escape_restores_the_snapshot_or_falls_back_to_the_model() {
    let renderer = crate::test_helpers::headless_renderer();

    // -- Escape over an open gesture reverts to the value captured when it opened.
    let mut ctrl = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(2.0)))
        .on_hover_preview(|live| TestMsg::Preview(*live.value()))
        .on_cancel(|cancel| TestMsg::Cancel(*cancel.restored()));
    let mut tree = fresh_tree(&ctrl);
    focus_row(&mut tree);
    let mut messages: Vec<TestMsg> = Vec::new();

    {
        let mut shell = Shell::new(&mut messages);
        update_rating(
            &mut ctrl,
            &mut tree,
            &crate::test_helpers::moved(Point::new(90.0, 10.0)),
            Cursor::Available(Point::new(90.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    assert_eq!(
        hover_state(&tree),
        (Some(3), Some(4.0)),
        "precondition: the fourth star is being previewed"
    );
    assert!(
        gesture_is_open(&tree),
        "precondition: previewing it opened the gesture slot"
    );

    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::Escape),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Preview(Some(4.0)), TestMsg::Cancel(Some(2.0))],
        "Escape must revert to the value the gesture started from, not the preview"
    );
    assert_eq!(
        hover_state(&tree),
        (None, None),
        "Escape drops the hover bookkeeping along with the gesture"
    );
    assert!(
        !gesture_is_open(&tree),
        "Escape closes the gesture slot it cancelled"
    );

    // -- Escape with no gesture open falls back to the model's own value.
    let mut fallback = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(1.5)))
        .on_cancel(|cancel| TestMsg::Cancel(*cancel.restored()));
    let mut fallback_tree = fresh_tree(&fallback);
    focus_row(&mut fallback_tree);
    let mut fallback_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut fallback,
        &mut fallback_tree,
        &named_key(Named::Escape),
        &renderer,
        &mut fallback_messages,
    );
    assert_eq!(
        fallback_messages,
        vec![TestMsg::Cancel(Some(1.5))],
        "Escape with no gesture open still reports the model's own value as restored"
    );

    // -- ... and with no callback at all it says nothing.
    let mut silent = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(3.5)));
    let mut silent_tree = fresh_tree(&silent);
    focus_row(&mut silent_tree);
    let mut silent_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut silent,
        &mut silent_tree,
        &named_key(Named::Escape),
        &renderer,
        &mut silent_messages,
    );
    assert!(
        silent_messages.is_empty(),
        "a row with no cancel callback publishes nothing on Escape"
    );
    assert!(
        !gesture_is_open(&silent_tree),
        "Escape cannot close a gesture that was never open"
    );
}

#[test]
fn test_the_arrow_keys_step_and_clamp_by_the_configured_step() {
    let renderer = crate::test_helpers::headless_renderer();

    // -- one step above the floor, clearing allowed: the arrow clears the row.
    let mut ctrl = rating_control::<TestMsg>(
        RatingModel::new(5)
            .with_value(Some(1.0))
            .with_allow_clear(true),
    )
    .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut tree = fresh_tree(&ctrl);
    focus_row(&mut tree);
    let mut messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::ArrowLeft),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(None)],
        "stepping below the first star clears the row when clearing is allowed"
    );

    // -- the same step with clearing refused lands on the first star.
    let mut floor = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(1.0)))
        .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut floor_tree = fresh_tree(&floor);
    focus_row(&mut floor_tree);
    let mut floor_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut floor,
        &mut floor_tree,
        &named_key(Named::ArrowDown),
        &renderer,
        &mut floor_messages,
    );
    assert_eq!(
        floor_messages,
        vec![TestMsg::Commit(Some(1.0))],
        "with clearing refused the floor is the first star, not nothing"
    );

    // -- stepping down from an empty half-step row lands on the first half.
    let mut empty = rating_control::<TestMsg>(RatingModel::new(5).with_step(RatingStep::Half))
        .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut empty_tree = fresh_tree(&empty);
    focus_row(&mut empty_tree);
    let mut empty_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut empty,
        &mut empty_tree,
        &named_key(Named::ArrowDown),
        &renderer,
        &mut empty_messages,
    );
    assert_eq!(
        empty_messages,
        vec![TestMsg::Commit(Some(0.5))],
        "stepping down from an empty half-step row lands on the first half star"
    );

    // -- stepping up clamps at the top of the row for both up arrows.
    let mut top = rating_control::<TestMsg>(RatingModel::new(3).with_value(Some(3.0)))
        .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut top_tree = fresh_tree(&top);
    focus_row(&mut top_tree);
    let mut top_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut top,
        &mut top_tree,
        &named_key(Named::ArrowRight),
        &renderer,
        &mut top_messages,
    );
    assert_eq!(
        top_messages,
        vec![TestMsg::Commit(Some(3.0))],
        "stepping past the last star clamps to the last star"
    );
    top_messages.clear();
    press_key(
        &mut top,
        &mut top_tree,
        &named_key(Named::ArrowUp),
        &renderer,
        &mut top_messages,
    );
    assert_eq!(
        top_messages,
        vec![TestMsg::Commit(Some(3.0))],
        "the clamp holds for the second up-arrow key as well"
    );

    // -- half stepping moves by half a star in both directions.
    let mut half = rating_control::<TestMsg>(
        RatingModel::new(5)
            .with_value(Some(2.0))
            .with_step(RatingStep::Half),
    )
    .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut half_tree = fresh_tree(&half);
    focus_row(&mut half_tree);
    let mut half_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut half,
        &mut half_tree,
        &named_key(Named::ArrowLeft),
        &renderer,
        &mut half_messages,
    );
    assert_eq!(
        half_messages,
        vec![TestMsg::Commit(Some(1.5))],
        "half stepping down moves by half a star"
    );
    half_messages.clear();
    press_key(
        &mut half,
        &mut half_tree,
        &named_key(Named::ArrowRight),
        &renderer,
        &mut half_messages,
    );
    assert_eq!(
        half_messages,
        vec![TestMsg::Commit(Some(2.5))],
        "and half stepping back up moves by half a star"
    );
}

#[test]
fn test_the_jump_and_clear_keys_respect_allow_clear() {
    let renderer = crate::test_helpers::headless_renderer();

    // -- Home and End on a row that may clear.
    let mut ctrl = rating_control::<TestMsg>(
        RatingModel::new(5)
            .with_value(Some(3.0))
            .with_allow_clear(true),
    )
    .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut tree = fresh_tree(&ctrl);
    focus_row(&mut tree);
    let mut messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::Home),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(None)],
        "Home clears a row that is allowed to be empty"
    );
    messages.clear();
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::End),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(Some(5.0))],
        "End jumps to the last star of the row"
    );
    messages.clear();
    press_key(
        &mut ctrl,
        &mut tree,
        &char_key("0"),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(None)],
        "typing zero clears a row that is allowed to be empty"
    );
    messages.clear();
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::Delete),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(None)],
        "Delete clears a row that is allowed to be empty"
    );
    messages.clear();
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::Backspace),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(None)],
        "Backspace clears a row that is allowed to be empty"
    );

    // -- Home on a row that may not clear lands on the first star.
    let mut strict = rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(3.0)))
        .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut strict_tree = fresh_tree(&strict);
    focus_row(&mut strict_tree);
    let mut strict_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut strict,
        &mut strict_tree,
        &named_key(Named::Home),
        &renderer,
        &mut strict_messages,
    );
    assert_eq!(
        strict_messages,
        vec![TestMsg::Commit(Some(1.0))],
        "Home on a row that may not clear lands on the first star"
    );
    strict_messages.clear();
    press_key(
        &mut strict,
        &mut strict_tree,
        &named_key(Named::Delete),
        &renderer,
        &mut strict_messages,
    );
    press_key(
        &mut strict,
        &mut strict_tree,
        &named_key(Named::Backspace),
        &renderer,
        &mut strict_messages,
    );
    press_key(
        &mut strict,
        &mut strict_tree,
        &char_key("0"),
        &renderer,
        &mut strict_messages,
    );
    assert!(
        strict_messages.is_empty(),
        "Delete, Backspace and zero must not clear a row that may not be cleared"
    );

    // -- Half stepping moves the jump keys by half a star.
    let mut half = rating_control::<TestMsg>(
        RatingModel::new(3)
            .with_value(Some(2.0))
            .with_step(RatingStep::Half),
    )
    .on_commit(|commit| TestMsg::Commit(*commit.value()));
    let mut half_tree = fresh_tree(&half);
    focus_row(&mut half_tree);
    let mut half_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut half,
        &mut half_tree,
        &named_key(Named::Home),
        &renderer,
        &mut half_messages,
    );
    assert_eq!(
        half_messages,
        vec![TestMsg::Commit(Some(0.5))],
        "Home on a half-step row lands on the first half star"
    );
    half_messages.clear();
    press_key(
        &mut half,
        &mut half_tree,
        &named_key(Named::End),
        &renderer,
        &mut half_messages,
    );
    assert_eq!(
        half_messages,
        vec![TestMsg::Commit(Some(3.0))],
        "End on a half-step row still lands on the last whole star"
    );
}

#[test]
fn test_keys_are_ignored_until_the_row_holds_focus() {
    let renderer = crate::test_helpers::headless_renderer();
    let mut ctrl = rating_control::<TestMsg>(
        RatingModel::new(5)
            .with_value(Some(2.0))
            .with_allow_clear(true),
    )
    .on_commit(|commit| TestMsg::Commit(*commit.value()))
    .on_cancel(|cancel| TestMsg::Cancel(*cancel.restored()));
    let mut tree = fresh_tree(&ctrl);
    let mut messages: Vec<TestMsg> = Vec::new();

    assert!(
        !row_is_focused(&tree),
        "precondition: a freshly built tree holds no focus"
    );
    for key in [
        named_key(Named::ArrowRight),
        named_key(Named::Home),
        named_key(Named::End),
        named_key(Named::Escape),
        char_key("0"),
    ] {
        press_key(&mut ctrl, &mut tree, &key, &renderer, &mut messages);
    }
    assert!(
        messages.is_empty(),
        "an unfocused row must not answer the keyboard, saw {messages:?}"
    );
    assert!(
        !gesture_is_open(&tree),
        "and must not open a gesture from keys it never handled"
    );

    focus_row(&mut tree);
    assert!(
        row_is_focused(&tree),
        "the row reports the focus it was handed"
    );
    press_key(
        &mut ctrl,
        &mut tree,
        &named_key(Named::ArrowRight),
        &renderer,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Commit(Some(3.0))],
        "once focused, the row answers the arrow keys"
    );

    // Keys the row does not claim are left for somebody else.
    messages.clear();
    for key in [
        named_key(Named::Tab),
        named_key(Named::PageUp),
        named_key(Named::PageDown),
        char_key("x"),
        char_key("5"),
    ] {
        press_key(&mut ctrl, &mut tree, &key, &renderer, &mut messages);
    }
    assert!(
        messages.is_empty(),
        "keys the row does not handle must produce nothing, saw {messages:?}"
    );

    // A read-only row ignores the keyboard and the pointer alike.
    let mut inert = rating_control::<TestMsg>(RatingModel::new(5).with_read_only(true))
        .on_commit(|commit| TestMsg::Commit(*commit.value()))
        .on_hover_preview(|live| TestMsg::Preview(*live.value()));
    let mut inert_tree = fresh_tree(&inert);
    focus_row(&mut inert_tree);
    let mut inert_messages: Vec<TestMsg> = Vec::new();
    press_key(
        &mut inert,
        &mut inert_tree,
        &named_key(Named::ArrowRight),
        &renderer,
        &mut inert_messages,
    );
    {
        let mut shell = Shell::new(&mut inert_messages);
        update_rating(
            &mut inert,
            &mut inert_tree,
            &crate::test_helpers::pressed(),
            Cursor::Available(Point::new(60.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    {
        let mut shell = Shell::new(&mut inert_messages);
        update_rating(
            &mut inert,
            &mut inert_tree,
            &crate::test_helpers::moved(Point::new(60.0, 10.0)),
            Cursor::Available(Point::new(60.0, 10.0)),
            &renderer,
            &mut shell,
        );
    }
    assert!(
        inert_messages.is_empty(),
        "a read-only row must not answer the pointer or the keyboard, saw {inert_messages:?}"
    );
    assert_eq!(
        hover_state(&inert_tree),
        (None, None),
        "a read-only row must not record a hover"
    );
}

#[test]
fn test_a_row_that_stops_being_interactive_withdraws_its_focus_target() {
    let id = iced::advanced::widget::Id::new("rating.gate");
    let mut harness = Harness::<TestMsg, _>::headless({
        let id = id.clone();
        move || {
            rating_control::<TestMsg>(RatingModel::new(5).with_value(Some(3.0)))
                .id(id.clone())
                .into()
        }
    });

    assert_eq!(
        harness.registered(),
        vec![Some(id.clone())],
        "an interactive row offers exactly one named focus target"
    );
    harness.focus(&id);
    assert!(
        harness.is_focused(&id),
        "an interactive row accepts the focus the router offers"
    );
    harness.hover(Point::new(68.0, 12.0));
    harness.render();

    harness.set_view({
        let id = id.clone();
        move || {
            rating_control::<TestMsg>(
                RatingModel::new(5)
                    .with_value(Some(3.0))
                    .with_read_only(true),
            )
            .id(id.clone())
            .into()
        }
    });

    assert_eq!(
        harness.registered(),
        Vec::new(),
        "a row that becomes read-only must stop offering a focus target"
    );
    assert_eq!(
        harness.focused(),
        None,
        "the router must not report focus on a row that withdrew itself"
    );
    assert_eq!(
        harness.pointer(
            &[crate::test_helpers::moved(Point::new(12.0, 12.0))],
            Cursor::Available(Point::new(12.0, 12.0)),
        ),
        Vec::new(),
        "a row that became read-only must stop answering the pointer"
    );
    // The hover recorded while the row was interactive must not draw an
    // outline on the row that has since gone quiet.
    harness.render();

    harness.set_view({
        let id = id.clone();
        move || {
            rating_control::<TestMsg>(
                RatingModel::new(5)
                    .with_value(Some(3.0))
                    .with_disabled(true),
            )
            .id(id.clone())
            .into()
        }
    });

    assert_eq!(
        harness.registered(),
        Vec::new(),
        "a row that becomes disabled must stop offering a focus target too"
    );
    assert_eq!(
        harness.focused(),
        None,
        "a disabled row cannot be what the router is pointing at"
    );
    harness.render();
}

#[test]
fn test_the_row_paints_with_and_without_the_focus_ring() {
    let id = iced::advanced::widget::Id::new("rating.ring");
    let mut harness = Harness::<TestMsg, _>::headless({
        let id = id.clone();
        move || {
            rating_control::<TestMsg>(
                RatingModel::new(5)
                    .with_value(Some(2.5))
                    .with_step(RatingStep::Half),
            )
            .id(id.clone())
            .into()
        }
    });

    assert!(
        !harness.is_focused(&id),
        "precondition: nothing holds the focus yet"
    );
    harness.render();

    harness.focus(&id);
    assert!(
        harness.is_focused(&id),
        "the row accepts the focus the router hands it"
    );
    harness.render();

    harness.unfocus();
    assert!(
        !harness.is_focused(&id),
        "the row gives the focus back when the router takes it away"
    );
    harness.render();
}

#[test]
fn test_the_hover_outline_marks_the_star_under_the_pointer() {
    let mut capture = Capture::new();
    let canvas = crate::test_helpers::VIEWPORT;
    let row = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));
    let model = RatingModel::new(5).with_value(Some(2.0));

    let before = capture.frame(canvas, rating_control::<TestMsg>(model.clone()).into());

    // Park the pointer over the third star: the update pass records the hover
    // and the very next draw outlines it.
    let at = Point::new(60.0, 10.0);
    let after = capture.frame_after_at(
        canvas,
        rating_control::<TestMsg>(model).into(),
        &[crate::test_helpers::moved(at)],
        Cursor::Available(at),
    );

    assert_repainted(
        &before,
        &after,
        star_slot_bounds(row, 2),
        "hovering the third star outlines it",
    );
    assert_eq!(
        before.differing_pixels(&after, star_slot_bounds(row, 0)),
        0,
        "outlining the hovered star must not disturb the stars it is not on"
    );
}

#[test]
fn test_a_row_with_no_stars_and_a_row_that_ignores_the_pointer_paint_nothing() {
    let mut capture = Capture::new();
    let canvas = crate::test_helpers::VIEWPORT;
    let row = Rectangle::new(Point::ORIGIN, Size::new(136.0, 24.0));

    let empty = capture.frame(
        canvas,
        rating_control::<TestMsg>(RatingModel::new(0)).into(),
    );
    assert_eq!(
        empty.ink_count(row),
        0,
        "a row with no stars has nothing to paint"
    );
    assert!(
        empty.is_uniform(row, crate::test_helpers::SURFACE_TOLERANCE),
        "and the space it was meant to occupy stays untouched"
    );

    // A read-only row under the pointer never records the hover, so it never
    // outlines anything: the frame with the pointer over it is the plain one.
    let at = Point::new(60.0, 10.0);
    let before = capture.frame(
        canvas,
        rating_control::<TestMsg>(RatingModel::new(5).with_read_only(true)).into(),
    );
    let after = capture.frame_after_at(
        canvas,
        rating_control::<TestMsg>(RatingModel::new(5).with_read_only(true)).into(),
        &[crate::test_helpers::moved(at)],
        Cursor::Available(at),
    );
    assert_eq!(
        before.differing_pixels(&after, row),
        0,
        "a read-only row must not repaint under the pointer"
    );
}
