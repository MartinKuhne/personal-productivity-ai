//! Live tooltip widget wrapping child content with delayed hover overlay and shortcut pill (`UIF-ATOM-007`).
//!
//! A tooltip wraps any interactive or static content widget and coordinates hover timing
//! through [`HoverController`]. When the hover delay elapses or keyboard focus lands, it paints
//! a floating card overlay containing a primary explanation label and an optional trailing
//! monospace shortcut pill.
//!
//! Unit tests live in the sibling `widget_tests.rs` sidecar.

use std::time::Duration;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph as _};
use iced::advanced::widget::operation::Operation;
use iced::advanced::widget::tree;
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Background, Border, Element, Event, Pixels, Point, Rectangle, Shadow, Size};

use crate::token::color::Palette;

use super::hover::{DismissReason, HoverController, TimeMs, Visibility};
use super::place::TooltipAnchor;
use super::{SHORTCUT_GAP, TooltipContent, TooltipStyle, clamp_width};

/// Measure text width using the renderer's paragraph engine, with fallback for headless renderers.
fn measure_text_width<Renderer: text_core::Renderer<Font = iced::Font>>(
    _renderer: &Renderer,
    content: &str,
    size: f32,
    font: iced::Font,
) -> f32 {
    let paragraph = <Renderer::Paragraph as text_core::Paragraph>::with_text(text_core::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: text_core::LineHeight::default(),
        font,
        align_x: text_core::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text_core::Shaping::default(),
        wrapping: text_core::Wrapping::None,
    });
    let measured = paragraph.min_bounds().width;
    if measured > 0.0 {
        measured
    } else {
        content.len() as f32 * (size * 0.6)
    }
}

/// Compute the dimensions of a tooltip card given measured text widths and style (`UIF-ATOM-007`).
///
/// A pure function: given the label width, optional shortcut width, and style, computes
/// the card size clamped to [`super::MAX_WIDTH`].
#[must_use]
pub fn card_size(label_width: f32, shortcut_width: Option<f32>, style: &TooltipStyle) -> Size {
    let content_width = match shortcut_width {
        Some(sw) if sw > 0.0 => label_width + SHORTCUT_GAP + sw,
        _ => label_width,
    };
    let width = clamp_width(content_width + style.padding_horizontal * 2.0);
    let height = style.font.line_height + style.padding_vertical * 2.0;
    Size::new(width, height)
}

/// The quad representation of the tooltip card background and border (`UIF-ATOM-007`).
#[must_use]
pub fn card_quad(bounds: Rectangle, style: &TooltipStyle) -> renderer::Quad {
    renderer::Quad {
        bounds,
        border: Border::default()
            .color(style.border_color)
            .width(style.border_width)
            .rounded(style.radius),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// The layout of the wrapped child content widget.
fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout.children().next().unwrap_or(layout)
}

/// The persistent state of a [`Tooltip`] widget stored in the widget tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipState {
    /// The hover controller managing the open delay and dismissal state.
    pub controller: HoverController,
    /// Whether the pointer was over the widget in the previous frame.
    pub hovered: bool,
}

impl Default for TooltipState {
    fn default() -> Self {
        Self::new()
    }
}

impl TooltipState {
    /// Create a new resting tooltip state with standard timing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            controller: HoverController::new(),
            hovered: false,
        }
    }

    /// Whether the tooltip is currently visible at `now`.
    #[must_use]
    pub fn is_shown(&self, now: TimeMs) -> bool {
        self.controller.tick(now).is_visible()
    }
}

/// Access the mutable [`TooltipState`] from a widget tree node.
fn tooltip_state(tree: &mut Tree) -> &mut TooltipState {
    tree.state.downcast_mut::<TooltipState>()
}

/// Access the read-only [`TooltipState`] from a widget tree node.
fn tooltip_state_ref(tree: &Tree) -> &TooltipState {
    tree.state.downcast_ref::<TooltipState>()
}

/// A container widget that equips an inner element with a floating tooltip (`UIF-ATOM-007`).
///
/// Wraps child content and coordinates hover arrival, delay countdown, and dismissal
/// on click, pointer exit, or Escape key.
pub struct Tooltip<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    tooltip: TooltipContent,
    palette: Palette,
    delay: Option<Duration>,
    anchor: Option<TooltipAnchor>,
}

impl<'a, Message, Theme, Renderer> Tooltip<'a, Message, Theme, Renderer> {
    /// Create a new tooltip wrapper over `content` displaying `tooltip` (`UIF-ATOM-007`).
    #[must_use]
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        tooltip: impl Into<TooltipContent>,
    ) -> Self {
        Self {
            content: content.into(),
            tooltip: tooltip.into(),
            palette: Palette::dark(),
            delay: None,
            anchor: None,
        }
    }

    /// Override the hover delay for this tooltip.
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Set the design token palette for this tooltip.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set an explicit anchor geometry for this tooltip.
    #[must_use]
    pub fn with_anchor(mut self, anchor: TooltipAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Read the tooltip content.
    #[must_use]
    pub fn content(&self) -> &TooltipContent {
        &self.tooltip
    }

    /// Read the configured palette.
    #[must_use]
    pub const fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Read the custom delay, if any.
    #[must_use]
    pub const fn delay(&self) -> Option<Duration> {
        self.delay
    }

    /// Read the explicit anchor, if any.
    #[must_use]
    pub const fn anchor(&self) -> Option<TooltipAnchor> {
        self.anchor
    }
}

/// Build a tooltip wrapper over `content` displaying `tooltip` (`UIF-ATOM-007`).
pub fn tooltip<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    tooltip: impl Into<TooltipContent>,
) -> Tooltip<'a, Message, Theme, Renderer> {
    Tooltip::new(content, tooltip)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Tooltip<'_, Message, Theme, Renderer>
where
    Renderer: text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TooltipState>()
    }

    fn state(&self) -> tree::State {
        let controller = match self.delay {
            Some(delay) => HoverController::with_delay(delay),
            None => HoverController::new(),
        };
        tree::State::new(TooltipState {
            controller,
            hovered: false,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<iced::Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let content_node =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        let size = content_node.size();
        layout::Node::with_children(size, vec![content_node])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                content_layout(layout),
                renderer,
                operation,
            );
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout(layout),
            cursor,
            viewport,
            renderer,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Forward event down to the wrapped content first.
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content_layout(layout),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tooltip_state(tree);
        let bounds = layout.bounds();
        let is_over = cursor.is_over(bounds);
        let was_over = state.hovered;
        state.hovered = is_over;

        let now = TimeMs::now();

        // Manage hover arrival and departure.
        if is_over {
            state.controller.pointer_enter(now);
        } else if was_over || state.controller.entered_at().is_some() {
            state.controller.pointer_exit();
        }

        // Handle dismissal events.
        match event {
            // Keyboard Escape: dismisses without consuming Escape.
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => {
                let _ = state.controller.dismiss(DismissReason::Escape);
            }
            // Any other keyboard press: dismisses without consuming the key.
            Event::Keyboard(keyboard::Event::KeyPressed { .. }) => {
                let _ = state.controller.dismiss(DismissReason::Escape);
            }
            // Mouse press on the control dismisses with MousePress.
            Event::Mouse(mouse::Event::ButtonPressed(_)) if is_over => {
                let _ = state.controller.dismiss(DismissReason::MousePress);
            }
            // Pointer left the window.
            Event::Mouse(mouse::Event::CursorLeft) => {
                let _ = state.controller.dismiss(DismissReason::LeftWindow);
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        // 1. Draw inner content.
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            content_layout(layout),
            cursor,
            viewport,
        );

        // 2. Check if the tooltip card is shown.
        let state = tooltip_state_ref(tree);
        let now = TimeMs::now();
        if state.controller.tick(now) != Visibility::Shown {
            return;
        }

        let tooltip_style = TooltipStyle::resolve(&self.palette);
        let label_font = iced::Font {
            weight: tooltip_style.font.weight.iced(),
            ..renderer.default_font()
        };
        let label_size = tooltip_style.font.size;
        let label_width = measure_text_width(renderer, &self.tooltip.label, label_size, label_font);

        let shortcut_width = self.tooltip.shortcut.as_ref().map(|s| {
            let shortcut_font = tooltip_style
                .font
                .font(tooltip_style.shortcut_font, tooltip_style.font.weight);
            measure_text_width(renderer, s, label_size, shortcut_font)
        });

        let card_dimensions = card_size(label_width, shortcut_width, &tooltip_style);
        let anchor = self
            .anchor
            .unwrap_or(TooltipAnchor::Control(layout.bounds()));
        let placement = anchor.resolve(card_dimensions, *viewport);
        let card_rect = placement.rect();

        // 3. Draw tooltip overlay background quad.
        let quad = card_quad(card_rect, &tooltip_style);
        renderer.fill_quad(quad, Background::Color(tooltip_style.background));

        // 4. Draw primary label.
        let inner_x = card_rect.x + tooltip_style.padding_horizontal;
        let inner_y = card_rect.y + tooltip_style.padding_vertical;
        let inner_h = tooltip_style.font.line_height;

        let max_label_w = match shortcut_width {
            Some(sw) if sw > 0.0 => {
                (card_rect.width - tooltip_style.padding_horizontal * 2.0 - SHORTCUT_GAP - sw)
                    .max(0.0)
            }
            _ => (card_rect.width - tooltip_style.padding_horizontal * 2.0).max(0.0),
        };

        renderer.fill_text(
            text_core::Text {
                content: self.tooltip.label.clone(),
                bounds: Size::new(max_label_w, inner_h),
                size: Pixels(tooltip_style.font.size),
                line_height: LineHeight::Absolute(Pixels(inner_h)),
                font: label_font,
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(inner_x, inner_y),
            tooltip_style.foreground,
            *viewport,
        );

        // 5. Draw trailing shortcut pill in FontFamily::Mono if present.
        if let Some(shortcut) = &self.tooltip.shortcut {
            let shortcut_font = tooltip_style
                .font
                .font(tooltip_style.shortcut_font, tooltip_style.font.weight);
            let shortcut_x = inner_x + label_width.min(max_label_w) + SHORTCUT_GAP;
            let max_shortcut_w =
                (card_rect.x + card_rect.width - tooltip_style.padding_horizontal - shortcut_x)
                    .max(0.0);

            if max_shortcut_w > 0.0 {
                renderer.fill_text(
                    text_core::Text {
                        content: shortcut.clone(),
                        bounds: Size::new(max_shortcut_w, inner_h),
                        size: Pixels(tooltip_style.font.size),
                        line_height: LineHeight::Absolute(Pixels(inner_h)),
                        font: shortcut_font,
                        align_x: text_core::Alignment::Left,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    Point::new(shortcut_x, inner_y),
                    tooltip_style.shortcut_foreground,
                    *viewport,
                );
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Tooltip<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + text_core::Renderer<Font = iced::Font>,
{
    fn from(tooltip: Tooltip<'a, Message, Theme, Renderer>) -> Self {
        Element::new(tooltip)
    }
}

#[cfg(test)]
#[path = "widget_tests.rs"]
mod widget_tests;
