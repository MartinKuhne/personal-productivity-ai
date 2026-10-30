//! The hyperlink resolver, geometry and focus state — the inline
//! interactive text control of §3.1d (`UIF-ATOM-001d`).
//!
//! A hyperlink is not a flat button. It has no border, its resting fill is
//! absent rather than a surface, and its primary affordance is a two-pixel
//! underline rather than a perimeter. So this module does not borrow the
//! button resolver — there is nothing in §3.1's matrix it could reuse
//! without inventing a fifth variant the spec does not list. What it does
//! reuse is every rule that is *not* about the shape: the token set, the
//! adaptive ring, the state resolver, and the focus wrapper.
//!
//! # The ring is resolved against the mount
//!
//! At rest the control paints nothing behind its label, so the ring is
//! drawn over whatever the link sits on. Resolving it against the link's
//! own (transparent) background would ask the adaptive rule to judge a
//! colour that is not there. So [`resolve_style`] takes the `mount` the
//! link is drawn on and resolves against that — the same decision the
//! segmented strip made for the same reason. The hover pill is inside the
//! ring, not under it, so the mount stays the right answer in every state.
//!
//! # The underline is the affordance, and it leaves when the control dies
//!
//! §3.1d gives the resting, hovered and pressed states an underline and
//! the disabled state none. That is the whole non-colour cue for a link:
//! a greyed label that is also un-underlined cannot be mistaken for a live
//! one. [`HyperlinkStyle::underlined`] is therefore resolved from the
//! state rather than left to the renderer, and it is `false` in exactly
//! one cell.
//!
//! # Contrast is quoted against two mounts because the control lives on two
//!
//! §3.1d prints `accent.text` at `10.66:1` on `base` and `9.49:1` on
//! `overlay`. A link is the one atom in §3.1 that is specified against
//! two different surfaces in a single cell, because it is the one atom
//! that appears both in body copy and on floating surfaces. Both figures
//! are recomputed in the sidecar from the raw sRGB rather than trusted.
//!
//! # Focus is the iced router's, through [`FocusableState`]
//!
//! [`HyperlinkState`] implements [`operation::Focusable`] by delegating
//! to the crate's existing [`FocusableState`] rather than carrying its own
//! `is_focused` bit. The router therefore drives the link exactly the way
//! it drives every other focusable wrapper, and the hover and press
//! signals that sit beside it are pointer observations, not focus ones.
//!
//! Unit tests live in the sibling `hyperlink_tests.rs` sidecar.

use crate::binding::Controlled;
use crate::focus::FocusId;
use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;
use crate::widget::focusable::FocusableState;
use crate::widget::hyperlink_model::{HyperlinkEmission, HyperlinkModel, HyperlinkTarget};

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph as _};
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree;
use iced::advanced::widget::{Id, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Size};

/// The minimum hit-target height — `min_height: 24px` (§3.1d,
/// `size.hit_target.min`).
pub const MIN_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// The focus ring's corner radius — `radius.widget` (§3.1d).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The focus ring width — `border.focus` (§3.0.2).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The underline stroke width — the `1px` underline of §3.1d.
pub const UNDERLINE_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Horizontal padding around the label.
///
/// Deliberately the smallest token in the scale: a hyperlink is inline
/// content, and padding wide enough to look like a control would break
/// the line it sits in. It exists so the underline does not touch the
/// neighbouring glyph.
pub const PADDING_X: f32 = space::XXS;

/// No ring is drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// No ring is drawn.
const NO_RING_WIDTH: f32 = 0.0;

/// A hyperlink's resolved paint for one state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperlinkStyle {
    /// The label colour.
    pub foreground: Color,
    /// The pill fill behind the label — transparent wherever §3.1d paints
    /// none.
    pub background: Color,
    /// Whether the underline is drawn.
    pub underlined: bool,
    /// The focus ring colour — transparent when no ring is drawn.
    pub focus_ring_color: Color,
    /// The focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
}

impl HyperlinkStyle {
    /// Whether nothing is painted behind the label.
    #[must_use]
    pub const fn is_transparent_background(&self) -> bool {
        self.background.a == 0.0
    }
}

/// Resolve the paint for one state against the surface the link is drawn
/// on (`UIF-ATOM-001d`).
///
/// `Invalid` shares the `Rest` cell: §3.1d gives a link no error skin, and
/// a link has no value to be invalid in the first place.
#[must_use]
pub fn resolve_style(state: VisualState, mount: Color, palette: &Palette) -> HyperlinkStyle {
    let (foreground, background, underlined) = match state.base {
        BaseState::Rest => (palette.accent_text, Color::TRANSPARENT, true),
        BaseState::Hovered => (palette.text_strong, palette.surface_hover, true),
        BaseState::Pressed => (palette.accent_text, palette.surface_active, true),
        BaseState::Disabled => (palette.text_disabled, Color::TRANSPARENT, false),
        // §3.1d gives a link no error skin, and a link has no value that
        // could fail validation; `Invalid` reads as `Rest`.
        BaseState::Invalid => (palette.accent_text, Color::TRANSPARENT, true),
    };

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            crate::widget::focus::resolve_ring_color_for_background(mount, palette),
            FOCUS_RING_WIDTH,
        )
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    HyperlinkStyle {
        foreground,
        background,
        underlined,
        focus_ring_color,
        focus_ring_width,
    }
}

/// The size a link needs for a label of `label_width`.
///
/// The height is the hit target, not the text: §3.1d asks for `24px`
/// minimum so a link in a line of body copy is still clickable, and a
/// taller mount does not shrink it.
#[must_use]
pub fn control_size(label_width: f32) -> Size {
    Size::new(label_width.max(0.0) + PADDING_X * 2.0, MIN_HEIGHT)
}

/// The label rectangle inside a link's hit box.
#[must_use]
pub fn label_bounds(control: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(control.x + PADDING_X, control.y),
        Size::new((control.width - PADDING_X * 2.0).max(0.0), control.height),
    )
}

/// The underline's rectangle.
///
/// It spans the **label**, not the hit box, so the rule never runs out
/// past the last glyph into the padding — the padding exists precisely to
/// keep it away from the neighbouring character.
///
/// The exact baseline is a renderer concern that depends on font metrics
/// the atom does not have; what the atom fixes is the horizontal extent
/// and the stroke width, which is what `UIF-PRIN-003` can actually check.
#[must_use]
pub fn underline_bounds(control: Rectangle) -> Rectangle {
    let label = label_bounds(control);
    Rectangle::new(
        Point::new(label.x, control.y + control.height - UNDERLINE_WIDTH),
        Size::new(label.width, UNDERLINE_WIDTH),
    )
}

/// The per-control state a hyperlink carries: the focus wrapper's bit,
/// plus the two pointer observations that drive its paint.
///
/// The focus half is [`FocusableState`], delegated rather than
/// reimplemented, so `operation::Focusable` behaves here exactly as it
/// does on every other wrapper in the crate. Hover and press are plain
/// pointer facts and are *not* allowed to imply focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HyperlinkState {
    focusable: FocusableState,
    hovered: bool,
    pressed: bool,
}

impl HyperlinkState {
    /// A link that holds neither focus nor the pointer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focusable: FocusableState::new(),
            hovered: false,
            pressed: false,
        }
    }

    /// Whether this link currently holds keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focusable.is_focused()
    }

    /// The focus wrapper half, read-only.
    #[must_use]
    pub const fn focusable(&self) -> &FocusableState {
        &self.focusable
    }

    /// Whether the pointer is inside the hit box.
    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a button is held on the link.
    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Move the pointer in or out.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Press or release the pointer on the link.
    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    /// Resolve what this link should paint, given whether it is live.
    ///
    /// Goes through the crate-wide [`resolve`] so the precedence — a dead
    /// link shows the disabled skin and holds no ring, whatever the
    /// pointer or the router were doing — is the one rule every atom
    /// shares rather than a local copy of it.
    #[must_use]
    pub fn visual_state(&self, enabled: bool) -> VisualState {
        resolve(InteractionInput {
            enabled,
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.focusable.is_focused(),
            invalid: false,
        })
    }

    /// Drop the pointer signals without touching focus.
    pub fn release_pointer(&mut self) {
        self.hovered = false;
        self.pressed = false;
    }
}

impl operation::Focusable for HyperlinkState {
    fn is_focused(&self) -> bool {
        self.focusable.is_focused()
    }

    fn focus(&mut self) {
        self.focusable.focus();
    }

    fn unfocus(&mut self) {
        self.focusable.unfocus();
    }
}

/// The pill layer painted behind a hyperlink in hovered and pressed states (`UIF-ATOM-001d`).
#[must_use]
pub fn pill_layer(bounds: Rectangle, style: &HyperlinkStyle) -> Option<Layer> {
    if style.is_transparent_background() {
        return None;
    }
    Some(Layer {
        bounds,
        fill: style.background,
        border: Border::default().rounded(RADIUS),
    })
}

/// The underline layer drawn under the label (`UIF-ATOM-001d`).
#[must_use]
pub fn underline_layer(bounds: Rectangle, style: &HyperlinkStyle) -> Option<Layer> {
    if !style.underlined {
        return None;
    }
    Some(Layer {
        bounds: underline_bounds(bounds),
        fill: style.foreground,
        border: Border::default(),
    })
}

/// The focus ring layer drawn outside the hit box (`UIF-ATOM-001d`).
#[must_use]
pub fn focus_ring_layer(bounds: Rectangle, style: &HyperlinkStyle) -> Option<Layer> {
    if style.focus_ring_width <= 0.0 {
        return None;
    }
    Some(Layer {
        bounds: bounds.expand(style.focus_ring_width),
        fill: Color::TRANSPARENT,
        border: Border::default()
            .color(style.focus_ring_color)
            .width(style.focus_ring_width)
            .rounded(RADIUS + style.focus_ring_width),
    })
}

/// The live, interactive hyperlink widget (`UIF-ATOM-001d`, §3.1d).
///
/// An inline or standalone interactive text control displaying a label,
/// underline affordance, and hover/press pill background. It routes mouse and
/// keyboard events to either a standalone closure (`on_press`) or a
/// [`Controlled`] binding over [`HyperlinkModel`] (`emit`).
pub struct Hyperlink<'a, Message> {
    label: String,
    uri: String,
    external: bool,
    focus_id: FocusId,
    widget_id: Id,
    enabled: bool,
    palette: Palette,
    mount: Color,
    model: Option<Controlled<'a, HyperlinkModel>>,
    emit: Option<Box<dyn Fn(HyperlinkEmission) -> Option<Message> + 'a>>,
    on_press: Option<Box<dyn Fn(HyperlinkTarget) -> Option<Message> + 'a>>,
}

impl<'a, Message> Hyperlink<'a, Message> {
    /// Build a **controlled** hyperlink over a host-owned [`HyperlinkModel`] (`UIF-ATOM-001d`).
    ///
    /// The model owns the link's label, URI, externality, and disabled state.
    /// User activations and hover events are dispatched through `emit`.
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, HyperlinkModel>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(HyperlinkEmission) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = crate::bridge::widget_id(&focus_id);
        let current = model.get();
        let label = current.label.clone();
        let uri = current.uri.clone();
        let external = current.external;
        let enabled = current.enabled();
        Self {
            label,
            uri,
            external,
            focus_id,
            widget_id,
            enabled,
            palette: Palette::dark(),
            mount: Palette::dark().surface_base,
            model: Some(model),
            emit: Some(Box::new(emit)),
            on_press: None,
        }
    }

    /// Build a **standalone** hyperlink publishing to an `on_press` callback (`UIF-ATOM-001d`).
    #[must_use]
    pub fn standalone(
        label: impl Into<String>,
        uri: impl Into<String>,
        focus_id: impl Into<FocusId>,
        on_press: impl Fn(HyperlinkTarget) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = crate::bridge::widget_id(&focus_id);
        Self {
            label: label.into(),
            uri: uri.into(),
            external: false,
            focus_id,
            widget_id,
            enabled: true,
            palette: Palette::dark(),
            mount: Palette::dark().surface_base,
            model: None,
            emit: None,
            on_press: Some(Box::new(on_press)),
        }
    }

    /// Set the mount background color this link sits on (`UIF-ATOM-001d`).
    #[must_use]
    pub fn mount(mut self, mount: Color) -> Self {
        self.mount = mount;
        self
    }

    /// Set the color palette for styling (`UIF-ATOM-001d`).
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set whether this link opens externally (`UIF-ATOM-001d`).
    #[must_use]
    pub fn external(mut self, external: bool) -> Self {
        self.external = external;
        self
    }

    /// Set whether this link is enabled (`UIF-ATOM-001d`).
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The visible label of this link (`UIF-ATOM-001d`).
    #[must_use]
    pub fn label(&self) -> &str {
        if let Some(model) = &self.model {
            &model.get().label
        } else {
            &self.label
        }
    }

    /// The target URI of this link (`UIF-ATOM-001d`).
    #[must_use]
    pub fn uri(&self) -> &str {
        if let Some(model) = &self.model {
            &model.get().uri
        } else {
            &self.uri
        }
    }

    /// Whether opening this link leaves the application (`UIF-ATOM-001d`).
    #[must_use]
    pub fn is_external(&self) -> bool {
        if let Some(model) = &self.model {
            model.get().external
        } else {
            self.external
        }
    }

    /// Whether this link accepts interaction (`UIF-ATOM-001d`).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        if let Some(model) = &self.model {
            model.get().enabled()
        } else {
            self.enabled
        }
    }

    /// The framework-level identity this link answers to (`UIF-ATOM-001d`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`Id`] this link registers with the focus router (`UIF-ATOM-001d`).
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// Helper to activate the link (`UIF-ATOM-001d`).
    fn activate(&self, shell: &mut Shell<'_, Message>) {
        if !self.is_enabled() {
            return;
        }
        if let (Some(model), Some(emit)) = (&self.model, &self.emit)
            && let Some(commit) = model.get().activate()
            && let Some(message) = emit(HyperlinkEmission::Opened(commit))
        {
            shell.publish(message);
        } else if let Some(on_press) = &self.on_press {
            let target = HyperlinkTarget {
                uri: self.uri().to_owned(),
                external: self.is_external(),
            };
            if let Some(message) = on_press(target) {
                shell.publish(message);
            }
        }
    }
}

/// The read-write [`HyperlinkState`] from a widget tree node.
fn hyperlink_state(tree: &mut Tree) -> &mut HyperlinkState {
    tree.state.downcast_mut::<HyperlinkState>()
}

/// The read-only [`HyperlinkState`] from a widget tree node.
fn hyperlink_state_ref(tree: &Tree) -> &HyperlinkState {
    tree.state.downcast_ref::<HyperlinkState>()
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Hyperlink<'_, Message>
where
    Renderer: text_core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HyperlinkState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HyperlinkState::new())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(MIN_HEIGHT))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let label = self.label();
        let measured_width = {
            let paragraph =
                <Renderer::Paragraph as text_core::Paragraph>::with_text(text_core::Text {
                    content: label,
                    bounds: Size::INFINITE,
                    size: renderer.default_size(),
                    line_height: text_core::LineHeight::default(),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Left,
                    align_y: alignment::Vertical::Top,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                });
            paragraph.min_bounds().width
        };
        let label_width = if measured_width > 0.0 {
            measured_width
        } else {
            label.len() as f32 * 8.0
        };
        let intrinsic = control_size(label_width);
        let size = limits.resolve(Length::Shrink, Length::Fixed(MIN_HEIGHT), intrinsic);
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = hyperlink_state(tree);

        if self.is_enabled() {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.is_enabled() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let is_over = cursor.is_over(layout.bounds());
        let state = hyperlink_state(tree);
        let was_hovered = state.hovered;

        state.hovered = is_over;

        if !self.is_enabled() {
            state.pressed = false;
            if was_hovered
                && let Some(emit) = &self.emit
                && let Some(message) = emit(HyperlinkModel::hover_cleared())
            {
                shell.publish(message);
            }
            return;
        }

        // Emit hover transitions for controlled hyperlinks
        if !was_hovered && is_over {
            if let (Some(model), Some(emit)) = (&self.model, &self.emit)
                && let Some(emission) = model.get().hover_entered()
                && let Some(message) = emit(emission)
            {
                shell.publish(message);
            }
        } else if was_hovered
            && !is_over
            && let Some(emit) = &self.emit
            && let Some(message) = emit(HyperlinkModel::hover_cleared())
        {
            shell.publish(message);
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_over => {
                state.pressed = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.pressed => {
                state.pressed = false;
                if is_over {
                    self.activate(shell);
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Enter | Named::Space),
                ..
            }) if state.is_focused() => {
                self.activate(shell);
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = hyperlink_state_ref(tree);
        let visual = state.visual_state(self.is_enabled());
        let style = resolve_style(visual, self.mount, &self.palette);
        let bounds = layout.bounds();

        for layer in [
            pill_layer(bounds, &style),
            focus_ring_layer(bounds, &style),
            underline_layer(bounds, &style),
        ]
        .into_iter()
        .flatten()
        {
            renderer.fill_quad(layer.quad(), Background::Color(layer.fill));
        }

        let text_bounds = label_bounds(bounds);
        renderer.fill_text(
            text_core::Text {
                content: self.label().to_owned(),
                bounds: text_bounds.size(),
                size: renderer.default_size(),
                line_height: LineHeight::Absolute(Pixels(text_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            text_bounds.center(),
            style.foreground,
            *viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Hyperlink<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + text_core::Renderer,
{
    fn from(hyperlink: Hyperlink<'a, Message>) -> Self {
        Element::new(hyperlink)
    }
}

#[cfg(test)]
#[path = "hyperlink_tests.rs"]
mod hyperlink_tests;
