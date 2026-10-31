//! Unit tests for the live [`Tooltip`] widget (`UIF-ATOM-007`).

use std::time::Duration;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::{Clipboard, Shell};
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Color, Element, Event, Length, Point, Rectangle, Size};
use iced_test::renderer::Renderer as HeadlessRenderer;

use crate::surface::tooltip::hover::TimeMs;
use crate::surface::tooltip::{MAX_WIDTH, TooltipContent, TooltipStyle};
use crate::test_helpers::headless_renderer;
use crate::token::color::Palette;
use crate::token::shape;

use super::*;

type TestTooltip<'a> = Tooltip<'a, (), iced::Theme, HeadlessRenderer>;

fn test_tree<'a>(widget: &'a TestTooltip<'a>) -> Tree {
    Tree::new(widget as &dyn Widget<(), iced::Theme, HeadlessRenderer>)
}

/// A dummy inert element for testing widget wrappers.
struct DummyWidget {
    size: Size,
}

impl DummyWidget {
    fn new(width: f32, height: f32) -> Self {
        Self {
            size: Size::new(width, height),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for DummyWidget
where
    Renderer: text_core::Renderer<Font = iced::Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.size.width),
            Length::Fixed(self.size.height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

impl<'a, Message, Theme, Renderer> From<DummyWidget> for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + text_core::Renderer<Font = iced::Font>,
    Theme: 'a,
    Message: 'a,
{
    fn from(widget: DummyWidget) -> Self {
        Element::new(widget)
    }
}

// ---------------------------------------------------------------------------
// TooltipContent construction tests
// ---------------------------------------------------------------------------

#[test]
fn test_tooltip_content_new() {
    let content = TooltipContent::new("Save file");
    assert_eq!(content.label, "Save file");
    assert_eq!(content.shortcut, None);
}

#[test]
fn test_tooltip_content_with_shortcut() {
    let content = TooltipContent::with_shortcut("Save file", "Ctrl+S");
    assert_eq!(content.label, "Save file");
    assert_eq!(content.shortcut, Some("Ctrl+S".to_string()));
}

#[test]
fn test_tooltip_content_from_str() {
    let content: TooltipContent = "Edit".into();
    assert_eq!(content.label, "Edit");
    assert_eq!(content.shortcut, None);
}

// ---------------------------------------------------------------------------
// Pure helper geometry tests: card_size and card_quad
// ---------------------------------------------------------------------------

#[test]
fn test_card_size_arithmetic_without_shortcut() {
    let style = TooltipStyle::resolve(&Palette::dark());
    let size = card_size(100.0, None, &style);
    assert_eq!(size.width, 100.0 + style.padding_horizontal * 2.0);
    assert_eq!(
        size.height,
        style.font.line_height + style.padding_vertical * 2.0
    );
}

#[test]
fn test_card_size_arithmetic_with_shortcut() {
    let style = TooltipStyle::resolve(&Palette::dark());
    let size = card_size(100.0, Some(40.0), &style);
    let expected_w = 100.0 + SHORTCUT_GAP + 40.0 + style.padding_horizontal * 2.0;
    assert_eq!(size.width, expected_w);
    assert_eq!(
        size.height,
        style.font.line_height + style.padding_vertical * 2.0
    );
}

#[test]
fn test_card_size_clamps_at_max_width() {
    let style = TooltipStyle::resolve(&Palette::dark());
    let size = card_size(500.0, Some(100.0), &style);
    assert_eq!(size.width, MAX_WIDTH);
}

#[test]
fn test_card_quad_creation() {
    let style = TooltipStyle::resolve(&Palette::dark());
    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(150.0, 24.0));
    let quad = card_quad(bounds, &style);
    assert_eq!(quad.bounds, bounds);
    assert_eq!(quad.border.color, style.border_color);
    assert_eq!(quad.border.width, shape::BORDER_HAIRLINE);
    assert_eq!(quad.border.radius, shape::RADIUS_WIDGET.into());
    assert!(quad.snap);
}

// ---------------------------------------------------------------------------
// Tooltip builder & accessor tests
// ---------------------------------------------------------------------------

#[test]
fn test_tooltip_builder_and_accessors() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let tt: TestTooltip<'_> = tooltip(dummy, TooltipContent::with_shortcut("Action", "Cmd+K"))
        .with_delay(Duration::from_millis(250))
        .with_palette(Palette::dark());

    assert_eq!(tt.content().label, "Action");
    assert_eq!(tt.content().shortcut.as_deref(), Some("Cmd+K"));
    assert_eq!(tt.delay(), Some(Duration::from_millis(250)));
    assert_eq!(*tt.palette(), Palette::dark());
    assert_eq!(tt.anchor(), None);
}

// ---------------------------------------------------------------------------
// Widget lifecycle & layout tests
// ---------------------------------------------------------------------------

#[test]
fn test_widget_lifecycle_and_layout() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let mut tt: TestTooltip<'_> = Tooltip::new(dummy, "Help");

    let tree = test_tree(&tt);
    assert_eq!(tree.children.len(), 1);

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(!state.hovered);
    assert!(!state.is_shown(TimeMs::millis(0)));

    let renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let mut tree_clone = test_tree(&tt);
    let node = tt.layout(&mut tree_clone, &renderer, &limits);
    assert_eq!(node.size(), Size::new(80.0, 32.0));
}

// ---------------------------------------------------------------------------
// Widget event handling & dismissal tests
// ---------------------------------------------------------------------------

struct TestClipboard;
impl Clipboard for TestClipboard {
    fn read(&self, _kind: iced::advanced::clipboard::Kind) -> Option<String> {
        None
    }
    fn write(&mut self, _kind: iced::advanced::clipboard::Kind, _content: String) {}
}

#[test]
fn test_hover_enter_and_exit_events() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let mut tt: TestTooltip<'_> = Tooltip::new(dummy, "Tip");
    let mut tree = test_tree(&tt);

    let renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let node = tt.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let mut clipboard = TestClipboard;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

    // 1. Move cursor inside widget bounds (x: 10, y: 10)
    let cursor_over = mouse::Cursor::Available(Point::new(10.0, 10.0));
    let move_event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(10.0, 10.0),
    });

    tt.update(
        &mut tree,
        &move_event,
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(state.hovered, "cursor should be over widget");
    assert!(state.controller.is_active(), "controller should be armed");

    // 2. Move cursor outside widget bounds (x: 200, y: 200)
    let cursor_out = mouse::Cursor::Available(Point::new(200.0, 200.0));
    let move_out_event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(200.0, 200.0),
    });

    tt.update(
        &mut tree,
        &move_out_event,
        layout,
        cursor_out,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(!state.hovered, "cursor should no longer be over widget");
    assert!(
        !state.controller.is_active(),
        "controller should have exited"
    );
}

#[test]
fn test_escape_dismisses_without_consuming_event() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let mut tt: TestTooltip<'_> = Tooltip::new(dummy, "Tip");
    let mut tree = test_tree(&tt);

    let renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let node = tt.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let mut clipboard = TestClipboard;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

    // Arm hover
    let cursor_over = mouse::Cursor::Available(Point::new(10.0, 10.0));
    let move_event = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(10.0, 10.0),
    });
    tt.update(
        &mut tree,
        &move_event,
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(state.controller.is_active());

    // Press Escape
    let escape_event = Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(Named::Escape),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        repeat: false,
        text: None,
        modified_key: Key::Named(Named::Escape),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
    });

    tt.update(
        &mut tree,
        &escape_event,
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(
        state.controller.is_suppressed(),
        "Escape must suppress the tooltip"
    );
    assert!(
        !shell.is_event_captured(),
        "Escape MUST NOT be consumed by tooltip"
    );
}

#[test]
fn test_keyboard_press_dismisses_tooltip() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let mut tt: TestTooltip<'_> = Tooltip::new(dummy, "Tip");
    let mut tree = test_tree(&tt);

    let renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let node = tt.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let mut clipboard = TestClipboard;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

    // Arm hover
    let cursor_over = mouse::Cursor::Available(Point::new(10.0, 10.0));
    tt.update(
        &mut tree,
        &Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(10.0, 10.0),
        }),
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    // Any key press (e.g. 'a')
    let key_event = Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Character("a".into()),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        repeat: false,
        text: Some("a".into()),
        modified_key: Key::Character("a".into()),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyA),
    });

    tt.update(
        &mut tree,
        &key_event,
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(
        state.controller.is_suppressed(),
        "Any key press must dismiss/suppress tooltip"
    );
}

#[test]
fn test_mouse_click_dismisses_tooltip() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let mut tt: TestTooltip<'_> = Tooltip::new(dummy, "Tip");
    let mut tree = test_tree(&tt);

    let renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let node = tt.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let mut clipboard = TestClipboard;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

    let cursor_over = mouse::Cursor::Available(Point::new(10.0, 10.0));
    tt.update(
        &mut tree,
        &Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(10.0, 10.0),
        }),
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    // Click mouse
    let click_event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    tt.update(
        &mut tree,
        &click_event,
        layout,
        cursor_over,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    let state = tree.state.downcast_ref::<TooltipState>();
    assert!(
        state.controller.is_suppressed(),
        "Click must suppress tooltip"
    );
}

// ---------------------------------------------------------------------------
// Headless draw tests
// ---------------------------------------------------------------------------

#[test]
fn test_headless_draw_when_hidden_and_when_shown() {
    let dummy = DummyWidget::new(80.0, 32.0);
    let tt: TestTooltip<'_> = Tooltip::new(dummy, TooltipContent::with_shortcut("Open", "Ctrl+O"))
        .with_delay(Duration::ZERO); // 0ms delay so it shows immediately

    let mut tree = test_tree(&tt);
    let mut renderer = headless_renderer();
    let limits = layout::Limits::NONE;
    let mut tt_clone: TestTooltip<'_> = Tooltip::new(
        DummyWidget::new(80.0, 32.0),
        TooltipContent::with_shortcut("Open", "Ctrl+O"),
    )
    .with_delay(Duration::ZERO);
    let node = tt_clone.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));
    let cursor = mouse::Cursor::Available(Point::new(10.0, 10.0));

    // At rest: controller is not armed, nothing extra is painted
    tt.draw(
        &tree,
        &mut renderer,
        &iced::Theme::Dark,
        &renderer::Style {
            text_color: Color::WHITE,
        },
        layout,
        cursor,
        &viewport,
    );

    // Now simulate hover arrival
    {
        let state = tree.state.downcast_mut::<TooltipState>();
        state.controller.pointer_enter(TimeMs::millis(0));
    }

    // Now drawing should execute the Shown branch including overlay quad and text
    tt.draw(
        &tree,
        &mut renderer,
        &iced::Theme::Dark,
        &renderer::Style {
            text_color: Color::WHITE,
        },
        layout,
        cursor,
        &viewport,
    );
}
