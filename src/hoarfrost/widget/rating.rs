//! The RatingControl widget (§3.13, `UIF-ATOM-013`).
//!
//! A rating control displays a row of star glyphs allowing a user to rate
//! an item with either whole-step or half-step increments. It supports live
//! hover previewing, clear-on-reclick, keyboard navigation, and three-tier
//! interaction callbacks ([`Live`], [`Commit`], [`Cancel`]).
//!
//! Unit tests live in the sibling `rating_tests.rs` sidecar.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text as text_core;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::event::Event;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::widget;
use iced::{Background, Border, Color, Element, Length, Pixels, Point, Rectangle, Shadow, Size};

use crate::binding::snapshot::GestureSlot;
use crate::binding::{Cancel, Commit, Live};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::focus::{self, FocusPlacement};
use crate::widget::focusable::FocusableState;

/// Filled star glyph (`UIF-ATOM-013`, §3.13).
pub const STAR_FILLED: &str = "★";

/// Empty star glyph (`UIF-ATOM-013`, §3.13).
pub const STAR_EMPTY: &str = "☆";

/// Default maximum rating value.
pub const DEFAULT_MAX_RATING: u8 = 5;

/// Rating increment step size (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum RatingStep {
    /// Half-star stepping (0.5 increments).
    Half,
    /// Whole-star stepping (1.0 increments).
    #[default]
    Whole,
}

impl RatingStep {
    /// Numeric increment step for this stepping mode (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub const fn increment(self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::Whole => 1.0,
        }
    }
}

/// Declarative data model driving a [`RatingControl`] (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Debug, PartialEq)]
pub struct RatingModel {
    /// Current settled rating value.
    pub value: Option<f32>,
    /// Optional externally controlled preview rating.
    pub hover_preview: Option<f32>,
    /// Maximum rating (number of stars in the row).
    pub max: u8,
    /// Stepping granularity.
    pub step: RatingStep,
    /// Whether clicking the active rating or pressing Delete clears to `None`.
    pub allow_clear: bool,
    /// Non-interactive display mode.
    pub read_only: bool,
    /// Disabled state.
    pub disabled: bool,
}

impl Default for RatingModel {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RATING)
    }
}

impl RatingModel {
    /// Construct a rating model with `max` stars and default Whole stepping.
    #[must_use]
    pub fn new(max: u8) -> Self {
        Self {
            value: None,
            hover_preview: None,
            max,
            step: RatingStep::Whole,
            allow_clear: false,
            read_only: false,
            disabled: false,
        }
    }

    /// Set the current value (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_value(mut self, value: impl Into<Option<f32>>) -> Self {
        self.value = value.into();
        self
    }

    /// Set an explicit hover preview (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_hover_preview(mut self, hover_preview: impl Into<Option<f32>>) -> Self {
        self.hover_preview = hover_preview.into();
        self
    }

    /// Set the stepping granularity (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_step(mut self, step: RatingStep) -> Self {
        self.step = step;
        self
    }

    /// Configure whether re-clicking the current value clears it to `None` (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_allow_clear(mut self, allow_clear: bool) -> Self {
        self.allow_clear = allow_clear;
        self
    }

    /// Set read-only mode (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set disabled state (`UIF-ATOM-013`).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Return the active display value: preview if set, otherwise settled value (`UIF-ATOM-013`).
    #[must_use]
    pub fn effective_value(&self) -> Option<f32> {
        self.hover_preview.or(self.value)
    }

    /// Total layout width of the star row in logical pixels (`UIF-ATOM-013`).
    #[must_use]
    pub fn total_width(&self) -> f32 {
        total_rating_width(self.max)
    }
}

/// Compute total layout width of a rating row with `max` slots (`UIF-ATOM-013`).
#[must_use]
pub fn total_rating_width(max: u8) -> f32 {
    if max == 0 {
        0.0
    } else {
        max as f32 * size::HIT_TARGET_MIN + (max.saturating_sub(1)) as f32 * space::XS
    }
}

/// Compute bounding box for star slot `slot_index` (`UIF-ATOM-013`).
#[must_use]
pub fn star_slot_bounds(bounds: Rectangle, slot_index: usize) -> Rectangle {
    let x = bounds.x + slot_index as f32 * (size::HIT_TARGET_MIN + space::XS);
    Rectangle {
        x,
        y: bounds.y,
        width: size::HIT_TARGET_MIN,
        height: size::HIT_TARGET_MIN,
    }
}

/// Calculate the slot index and rating value at pointer position `pos` (`UIF-ATOM-013`).
#[must_use]
pub fn value_at_position(
    bounds: Rectangle,
    pos: Point,
    max: u8,
    step: RatingStep,
) -> Option<(usize, f32)> {
    if !bounds.contains(pos) || max == 0 {
        return None;
    }

    for i in 0..max as usize {
        let slot = star_slot_bounds(bounds, i);
        let slot_hit_width = if i + 1 < max as usize {
            size::HIT_TARGET_MIN + space::XS
        } else {
            size::HIT_TARGET_MIN
        };
        let slot_extended = Rectangle {
            x: slot.x,
            y: slot.y,
            width: slot_hit_width,
            height: slot.height,
        };

        if slot_extended.contains(pos) {
            let val = match step {
                RatingStep::Whole => (i + 1) as f32,
                RatingStep::Half => {
                    let rel_x = (pos.x - slot.x) / slot.width;
                    if rel_x < 0.5 {
                        i as f32 + 0.5
                    } else {
                        (i + 1) as f32
                    }
                }
            };
            return Some((i, val.min(max as f32)));
        }
    }

    None
}

/// Callback invoked during live hover preview updates (`UIF-ATOM-013`, §3.13).
pub type OnHoverPreview<'a, Message> = Box<dyn Fn(Live<Option<f32>>) -> Message + 'a>;

/// Callback invoked when a rating value is settled and committed (`UIF-ATOM-013`, §3.13).
pub type OnCommit<'a, Message> = Box<dyn Fn(Commit<Option<f32>>) -> Message + 'a>;

/// Callback invoked when an active interaction is cancelled (`UIF-ATOM-013`, §3.13).
pub type OnCancel<'a, Message> = Box<dyn Fn(Cancel<Option<f32>>) -> Message + 'a>;

/// The interactive RatingControl widget (`UIF-ATOM-013`, §3.13).
pub struct RatingControl<'a, Message> {
    model: RatingModel,
    palette: Palette,
    widget_id: widget::Id,
    on_hover_preview: Option<OnHoverPreview<'a, Message>>,
    on_commit: Option<OnCommit<'a, Message>>,
    on_cancel: Option<OnCancel<'a, Message>>,
}

impl<'a, Message> RatingControl<'a, Message> {
    /// Construct a new rating control with the given model (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn new(model: RatingModel) -> Self {
        Self {
            model,
            palette: Palette::dark(),
            widget_id: widget::Id::unique(),
            on_hover_preview: None,
            on_commit: None,
            on_cancel: None,
        }
    }

    /// Attach a tier-1 live hover preview callback (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn on_hover_preview(
        mut self,
        callback: impl Fn(Live<Option<f32>>) -> Message + 'a,
    ) -> Self {
        self.on_hover_preview = Some(Box::new(callback));
        self
    }

    /// Attach a tier-2 settled commit callback (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn on_commit(mut self, callback: impl Fn(Commit<Option<f32>>) -> Message + 'a) -> Self {
        self.on_commit = Some(Box::new(callback));
        self
    }

    /// Attach an interaction cancellation callback (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn on_cancel(mut self, callback: impl Fn(Cancel<Option<f32>>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    /// Supply custom semantic palette tokens.
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set an explicit widget ID for keyboard focus targeting.
    #[must_use]
    pub fn id(mut self, id: widget::Id) -> Self {
        self.widget_id = id;
        self
    }

    /// Layout size of the rating control in logical pixels (`UIF-ATOM-013`).
    #[must_use]
    pub fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.model.total_width()),
            Length::Fixed(size::HIT_TARGET_MIN),
        )
    }

    /// Reference to the underlying data model.
    #[must_use]
    pub fn model(&self) -> &RatingModel {
        &self.model
    }
}

/// Construct a rating control widget from a [`RatingModel`] (`UIF-ATOM-013`, §3.13).
#[must_use]
pub fn rating_control<'a, Message>(model: RatingModel) -> RatingControl<'a, Message> {
    RatingControl::new(model)
}

/// Internal widget state for [`RatingControl`] held across frames in [`Tree`].
#[derive(Debug, Default)]
pub struct RatingControlState {
    focusable: FocusableState,
    hovered_slot: Option<usize>,
    hover_value: Option<f32>,
    gesture: GestureSlot<Option<f32>>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for RatingControl<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<RatingControlState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(RatingControlState::default())
    }

    fn size(&self) -> Size<Length> {
        RatingControl::size(self)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let w = self.model.total_width();
        let h = size::HIT_TARGET_MIN;
        let size = limits.resolve(Length::Fixed(w), Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<RatingControlState>();
        if !self.model.disabled && !self.model.read_only {
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
        if self.model.disabled || self.model.read_only {
            return mouse::Interaction::None;
        }

        if cursor.is_over(layout.bounds()) {
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
        if self.model.disabled || self.model.read_only {
            return;
        }

        let state = tree.state.downcast_mut::<RatingControlState>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    if let Some((slot_idx, val)) =
                        value_at_position(bounds, pos, self.model.max, self.model.step)
                    {
                        state.hovered_slot = Some(slot_idx);
                        if state.hover_value != Some(val) {
                            state.hover_value = Some(val);
                            if !state.gesture.is_open() {
                                let _ = state.gesture.begin(&self.model.value);
                            }
                            if let Some(on_preview) = &self.on_hover_preview {
                                shell.publish(on_preview(Live::new(Some(val))));
                            }
                        }
                    }
                } else if state.hovered_slot.is_some() || state.hover_value.is_some() {
                    state.hovered_slot = None;
                    state.hover_value = None;
                    if let Some(on_preview) = &self.on_hover_preview {
                        shell.publish(on_preview(Live::new(None)));
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                if state.hovered_slot.is_some() || state.hover_value.is_some() {
                    state.hovered_slot = None;
                    state.hover_value = None;
                    if let Some(on_preview) = &self.on_hover_preview {
                        shell.publish(on_preview(Live::new(None)));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(bounds)
                    && let Some((_, val)) =
                        value_at_position(bounds, pos, self.model.max, self.model.step)
                {
                    let commit_val = if self.model.allow_clear && self.model.value == Some(val) {
                        None
                    } else {
                        Some(val)
                    };

                    if !state.gesture.is_open() {
                        let _ = state.gesture.begin(&self.model.value);
                    }
                    let commit = state
                        .gesture
                        .commit(commit_val)
                        .unwrap_or_else(|| Commit::new(commit_val));

                    if let Some(on_commit) = &self.on_commit {
                        shell.publish(on_commit(commit));
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() =>
            {
                match key {
                    Key::Named(Named::Escape) => {
                        let cancel = state
                            .gesture
                            .cancel()
                            .unwrap_or_else(|| Cancel::new(self.model.value));
                        state.hover_value = None;
                        state.hovered_slot = None;
                        if let Some(on_cancel) = &self.on_cancel {
                            shell.publish(on_cancel(cancel));
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::ArrowLeft | Named::ArrowDown) => {
                        let step = self.model.step.increment();
                        let current = self.model.value.unwrap_or(0.0);
                        let next = current - step;
                        let commit_val = if next <= 0.0 {
                            if self.model.allow_clear {
                                None
                            } else {
                                Some(step)
                            }
                        } else {
                            Some(next)
                        };

                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::ArrowRight | Named::ArrowUp) => {
                        let step = self.model.step.increment();
                        let current = self.model.value.unwrap_or(0.0);
                        let next = (current + step).min(self.model.max as f32);
                        let commit_val = Some(next);

                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::Home) => {
                        let commit_val = if self.model.allow_clear {
                            None
                        } else {
                            Some(self.model.step.increment())
                        };

                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::End) => {
                        let commit_val = Some(self.model.max as f32);

                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    Key::Named(Named::Delete | Named::Backspace) if self.model.allow_clear => {
                        let commit_val = None;
                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    Key::Character(c) if *c == "0" && self.model.allow_clear => {
                        let commit_val = None;
                        if !state.gesture.is_open() {
                            let _ = state.gesture.begin(&self.model.value);
                        }
                        let commit = state
                            .gesture
                            .commit(commit_val)
                            .unwrap_or_else(|| Commit::new(commit_val));

                        if let Some(on_commit) = &self.on_commit {
                            shell.publish(on_commit(commit));
                        }
                        shell.capture_event();
                    }
                    _ => {}
                }
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
        let state = tree.state.downcast_ref::<RatingControlState>();
        let bounds = layout.bounds();

        // 1. Focus ring when focused
        if state.focusable.is_focused() {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(bounds),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(shape::RADIUS_WIDGET)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // 2. Active rating value for rendering
        let effective_val = self
            .model
            .hover_preview
            .or(state.hover_value)
            .or(self.model.value)
            .unwrap_or(0.0);

        let filled_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.status_warning
        };
        let empty_color = if self.model.disabled {
            self.palette.border_subtle
        } else {
            self.palette.border_strong
        };

        let half_slot_width = size::HIT_TARGET_MIN / 2.0;

        // 3. Render each star slot
        for i in 0..self.model.max as usize {
            let slot = star_slot_bounds(bounds, i);

            // Hover outline around hovered star slot
            if state.hovered_slot == Some(i) && !self.model.disabled && !self.model.read_only {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: slot,
                        border: Border::default()
                            .color(self.palette.text_strong)
                            .width(shape::BORDER_HAIRLINE)
                            .rounded(shape::RADIUS_WIDGET),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(Color::TRANSPARENT),
                );
            }

            let threshold_full = (i + 1) as f32;
            let threshold_half = i as f32 + 0.5;

            if effective_val >= threshold_full {
                // Fully filled star
                renderer.fill_text(
                    text_core::Text {
                        content: STAR_FILLED.to_owned(),
                        bounds: slot.size(),
                        size: Pixels(size::ICON_LG),
                        line_height: text_core::LineHeight::Absolute(Pixels(slot.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    slot.position(),
                    filled_color,
                    *viewport,
                );
            } else if effective_val >= threshold_half {
                // Half-filled star
                let left_half = Rectangle {
                    x: slot.x,
                    y: slot.y,
                    width: half_slot_width,
                    height: slot.height,
                };
                let right_half = Rectangle {
                    x: slot.x + half_slot_width,
                    y: slot.y,
                    width: half_slot_width,
                    height: slot.height,
                };

                // Draw filled star clipped to left half
                renderer.start_layer(left_half);
                renderer.fill_text(
                    text_core::Text {
                        content: STAR_FILLED.to_owned(),
                        bounds: slot.size(),
                        size: Pixels(size::ICON_LG),
                        line_height: text_core::LineHeight::Absolute(Pixels(slot.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    slot.position(),
                    filled_color,
                    *viewport,
                );
                renderer.end_layer();

                // Draw empty star clipped to right half
                renderer.start_layer(right_half);
                renderer.fill_text(
                    text_core::Text {
                        content: STAR_EMPTY.to_owned(),
                        bounds: slot.size(),
                        size: Pixels(size::ICON_LG),
                        line_height: text_core::LineHeight::Absolute(Pixels(slot.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    slot.position(),
                    empty_color,
                    *viewport,
                );
                renderer.end_layer();
            } else {
                // Fully empty star
                renderer.fill_text(
                    text_core::Text {
                        content: STAR_EMPTY.to_owned(),
                        bounds: slot.size(),
                        size: Pixels(size::ICON_LG),
                        line_height: text_core::LineHeight::Absolute(Pixels(slot.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    slot.position(),
                    empty_color,
                    *viewport,
                );
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<RatingControl<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(rating: RatingControl<'a, Message>) -> Self {
        Element::new(rating)
    }
}

#[cfg(test)]
#[path = "rating_tests.rs"]
mod rating_tests;
