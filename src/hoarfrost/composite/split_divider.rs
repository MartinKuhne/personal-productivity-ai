//! The SplitDivider composite pane resize handle (`UIF-COMP-016`, `UIF-PANE-001`, spec §3B.16).
//!
//! Reusable drag-handle composite for splitting adjacent container regions:
//! workspace pane splits (§4.3.2), `SideSheet` inspector edges (§3B.12), and
//! `DataTable` column resize borders (§3B.7).
//!
//! Unit tests live in the sibling `split_divider_tests.rs` sidecar.

use iced::advanced::Clipboard;
use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Widget;
use iced::advanced::widget::tree::{self, Tree};
use iced::mouse;
use iced::{Element, Length, Rectangle, Size};

use crate::binding::pending::{ActionPending, ActivationGate};
use crate::binding::pre_rank::{CaptureId, DragCapture, DragKind};
use crate::binding::snapshot::GestureSlot;
use crate::binding::{Cancel, Commit, GestureStart, Live};
use crate::state::{self, BaseState, InteractionInput, VisualState};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;

/// The axis along which a divider separates panes (§3B.16, `UIF-COMP-016`, `UIF-PANE-001`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SplitAxis {
    /// Splits widths (vertical divider line, horizontal resize cursor: `↔`).
    #[default]
    Horizontal,
    /// Splits heights (horizontal divider line, vertical resize cursor: `↕`).
    Vertical,
}

/// A snapshot of the split ratio captured before an active drag begins (§3B.16, `UIF-COMP-016`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitSnapshot {
    /// Pre-drag split ratio in `[0.0, 1.0]`.
    pub ratio: f32,
}

/// Controlled data model for the [`SplitDivider`] composite (§3B.16, `UIF-COMP-016`, `UIF-PANE-001`).
#[derive(Clone, Debug, PartialEq)]
pub struct SplitDividerModel {
    /// The axis along which the split operates.
    pub axis: SplitAxis,
    /// Committed ratio along the split, clamped to `0.0..=1.0`.
    pub ratio: f32,
    /// Minimum allowed ratio floor (`0.0..=1.0`).
    pub min_ratio: f32,
    /// Maximum allowed ratio ceiling (`0.0..=1.0`).
    pub max_ratio: f32,
    /// Whether the divider is disabled and refuses interaction.
    pub disabled: bool,
}

impl SplitDividerModel {
    /// Create a new model with `axis` and `ratio` clamped to `[0.0, 1.0]` (`UIF-COMP-016`).
    #[must_use]
    pub fn new(axis: SplitAxis, ratio: f32) -> Self {
        let clamped = Self::clamp_ratio(ratio, 0.0, 1.0);
        Self {
            axis,
            ratio: clamped,
            min_ratio: 0.0,
            max_ratio: 1.0,
            disabled: false,
        }
    }

    /// Builder method to specify `min_ratio` floor (`UIF-COMP-016`).
    #[must_use]
    pub fn with_min_ratio(mut self, min: f32) -> Self {
        self.min_ratio = min.clamp(0.0, 1.0);
        self.ratio = self.clamped_ratio();
        self
    }

    /// Builder method to specify `max_ratio` ceiling (`UIF-COMP-016`).
    #[must_use]
    pub fn with_max_ratio(mut self, max: f32) -> Self {
        self.max_ratio = max.clamp(0.0, 1.0);
        self.ratio = self.clamped_ratio();
        self
    }

    /// Builder method to toggle `disabled` state (`UIF-COMP-016`).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Clamp a raw ratio into `[min_ratio, max_ratio]` bounded by `[0.0, 1.0]` (`UIF-COMP-016`).
    #[must_use]
    pub fn clamp_ratio(raw: f32, min_ratio: f32, max_ratio: f32) -> f32 {
        let floor = min_ratio.clamp(0.0, 1.0);
        let ceil = max_ratio.clamp(floor, 1.0);
        raw.clamp(floor, ceil)
    }

    /// The effectively clamped ratio of this model (`UIF-COMP-016`).
    #[must_use]
    pub fn clamped_ratio(&self) -> f32 {
        Self::clamp_ratio(self.ratio, self.min_ratio, self.max_ratio)
    }
}

/// The in-flight resize gesture for [`SplitDivider`] (`UIF-COMP-016`, `UIF-PANE-001`).
#[derive(Clone, Debug, Default)]
pub struct SplitDividerResize {
    slot: GestureSlot<SplitSnapshot>,
    gate: ActivationGate,
    axis: SplitAxis,
    start_ratio: f32,
    current_ratio: f32,
    last_cursor_pos: f32,
    min_ratio: f32,
    max_ratio: f32,
    total_length: f32,
}

impl SplitDividerResize {
    /// Create a new idle resize gesture for `axis` (`UIF-COMP-016`).
    #[must_use]
    pub fn new(axis: SplitAxis, ratio: f32, min_ratio: f32, max_ratio: f32) -> Self {
        let clamped = SplitDividerModel::clamp_ratio(ratio, min_ratio, max_ratio);
        Self {
            slot: GestureSlot::new(),
            gate: ActivationGate::new(),
            axis,
            start_ratio: clamped,
            current_ratio: clamped,
            last_cursor_pos: 0.0,
            min_ratio,
            max_ratio,
            total_length: 0.0,
        }
    }

    /// Whether an active drag gesture is currently open (`UIF-COMP-016`).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.slot.is_open()
    }

    /// The axis governing this resize gesture (`UIF-COMP-016`).
    #[must_use]
    pub fn axis(&self) -> SplitAxis {
        self.axis
    }

    /// Pre-drag split ratio captured when the gesture started (`UIF-COMP-016`).
    #[must_use]
    pub fn start_ratio(&self) -> f32 {
        self.start_ratio
    }

    /// Current live split ratio tracked by the gesture (`UIF-COMP-016`).
    #[must_use]
    pub fn current_ratio(&self) -> f32 {
        self.current_ratio
    }

    /// Read the internal activation gate (`UIF-COMP-016`).
    #[must_use]
    pub fn gate(&self) -> &ActivationGate {
        &self.gate
    }

    /// Build a Pre-Rank drag capture registration for this gesture (`UIF-COMP-016`, `UIF-WIN-003`).
    #[must_use]
    pub fn capture(&self, id: impl Into<CaptureId>) -> DragCapture {
        DragCapture::new(id, DragKind::SplitDivider)
    }

    /// Begin a resize gesture, capturing `start_ratio` and `cursor_pos` (`UIF-COMP-016`).
    pub fn begin(
        &mut self,
        ratio: f32,
        cursor_pos: f32,
        total_length: f32,
        min_ratio: f32,
        max_ratio: f32,
    ) -> GestureStart {
        let pending = ActionPending::idle();
        let act = self.gate.try_activate(&pending);
        if act.is_suppressed() {
            return GestureStart::AlreadyOpen;
        }

        let clamped = SplitDividerModel::clamp_ratio(ratio, min_ratio, max_ratio);
        let snapshot = SplitSnapshot { ratio: clamped };
        let start = self.slot.begin(&snapshot);
        if start == GestureStart::Started {
            self.start_ratio = clamped;
            self.current_ratio = clamped;
            self.last_cursor_pos = cursor_pos;
            self.total_length = total_length;
            self.min_ratio = min_ratio;
            self.max_ratio = max_ratio;
        } else {
            self.gate.settle();
        }
        start
    }

    /// Update gesture with new cursor coordinate along the split axis (`UIF-COMP-016`).
    ///
    /// Clamps from the last clamped position within `[min_ratio, max_ratio]`,
    /// never accumulating overshoot delta.
    pub fn update(&mut self, cursor_pos: f32) -> Option<Live<f32>> {
        if !self.is_active() {
            return None;
        }

        let delta_px = cursor_pos - self.last_cursor_pos;
        self.last_cursor_pos = cursor_pos;

        let delta_ratio = if self.total_length > 0.0 {
            delta_px / self.total_length
        } else {
            0.0
        };

        let new_ratio = SplitDividerModel::clamp_ratio(
            self.current_ratio + delta_ratio,
            self.min_ratio,
            self.max_ratio,
        );

        if (new_ratio - self.current_ratio).abs() > f32::EPSILON {
            self.current_ratio = new_ratio;
            Some(Live::new(new_ratio))
        } else {
            None
        }
    }

    /// Settle the resize gesture on pointer release (`UIF-COMP-016`).
    pub fn commit(&mut self) -> Option<Commit<f32>> {
        if !self.is_active() {
            return None;
        }
        let val = self.current_ratio;
        let _ = self.slot.commit(SplitSnapshot { ratio: val });
        self.gate.settle();
        Some(Commit::new(val))
    }

    /// Cancel the resize gesture, restoring the pre-drag snapshot (`UIF-COMP-016`).
    pub fn cancel(&mut self) -> Option<Cancel<SplitSnapshot>> {
        if !self.is_active() {
            return None;
        }
        let res = self.slot.cancel();
        self.current_ratio = self.start_ratio;
        self.gate.settle();
        res
    }

    /// Abort the gesture without emitting callbacks (`UIF-COMP-016`).
    pub fn abort(&mut self) {
        self.slot.abort();
        self.gate.settle();
    }
}

/// Compute the interactive grab rect (`SPLITTER_GRAB` = 10px) centered on divider (`UIF-COMP-016`).
#[must_use]
pub fn compute_grab_rect(bounds: Rectangle, axis: SplitAxis) -> Rectangle {
    match axis {
        SplitAxis::Horizontal => {
            let width = size::SPLITTER_GRAB;
            let x = bounds.x + bounds.width / 2.0 - width / 2.0;
            Rectangle {
                x,
                y: bounds.y,
                width,
                height: bounds.height,
            }
        }
        SplitAxis::Vertical => {
            let height = size::SPLITTER_GRAB;
            let y = bounds.y + bounds.height / 2.0 - height / 2.0;
            Rectangle {
                x: bounds.x,
                y,
                width: bounds.width,
                height,
            }
        }
    }
}

/// Compute the resting stroke rect and centered grip pill rect (`UIF-COMP-016`, `UIF-PANE-001`).
#[must_use]
pub fn split_divider_geometry(bounds: Rectangle, axis: SplitAxis) -> (Rectangle, Rectangle) {
    match axis {
        SplitAxis::Horizontal => {
            let line_x = bounds.x + (bounds.width - 1.0) / 2.0;
            let stroke_rect = Rectangle {
                x: line_x,
                y: bounds.y,
                width: 1.0,
                height: bounds.height,
            };

            let pill_width = space::XXS;
            let pill_height = size::HIT_TARGET_MIN.min(bounds.height);
            let pill_x = line_x - (pill_width - 1.0) / 2.0;
            let pill_y = bounds.y + (bounds.height - pill_height) / 2.0;
            let pill_rect = Rectangle {
                x: pill_x,
                y: pill_y,
                width: pill_width,
                height: pill_height,
            };

            (stroke_rect, pill_rect)
        }
        SplitAxis::Vertical => {
            let line_y = bounds.y + (bounds.height - 1.0) / 2.0;
            let stroke_rect = Rectangle {
                x: bounds.x,
                y: line_y,
                width: bounds.width,
                height: 1.0,
            };

            let pill_width = size::HIT_TARGET_MIN.min(bounds.width);
            let pill_height = space::XXS;
            let pill_x = bounds.x + (bounds.width - pill_width) / 2.0;
            let pill_y = line_y - (pill_height - 1.0) / 2.0;
            let pill_rect = Rectangle {
                x: pill_x,
                y: pill_y,
                width: pill_width,
                height: pill_height,
            };

            (stroke_rect, pill_rect)
        }
    }
}

/// Ordered draw layer quads for [`SplitDivider`] (`UIF-COMP-016`, `UIF-PANE-001`, `UIF-PRIN-006`).
#[must_use]
pub fn split_divider_layers(
    bounds: Rectangle,
    axis: SplitAxis,
    visual: VisualState,
    palette: &Palette,
) -> Vec<(&'static str, Layer)> {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Vec::new();
    }

    let is_disabled = visual.base == BaseState::Disabled;
    let is_promoted =
        !is_disabled && (visual.base == BaseState::Hovered || visual.base == BaseState::Pressed);

    let stroke_color = if is_promoted {
        palette.accent_border
    } else {
        palette.border_subtle
    };

    let pill_color = if is_disabled {
        palette.border_subtle
    } else if is_promoted {
        palette.accent_border
    } else {
        palette.border_strong
    };

    let (stroke_rect, pill_rect) = split_divider_geometry(bounds, axis);

    vec![
        (
            "divider-stroke",
            Layer {
                bounds: stroke_rect,
                fill: stroke_color,
                border: iced::Border::default(),
            },
        ),
        (
            "grip-pill",
            Layer {
                bounds: pill_rect,
                fill: pill_color,
                border: iced::Border {
                    radius: shape::RADIUS_PILL.into(),
                    ..iced::Border::default()
                },
            },
        ),
    ]
}

/// Internal interaction state for the [`SplitDivider`] widget (`UIF-COMP-016`).
#[derive(Debug, Default)]
pub struct SplitDividerState {
    /// Active or idle resize gesture.
    pub resize: SplitDividerResize,
    /// Whether the pointer currently hovers the grab zone.
    pub is_hovered: bool,
    /// Keyboard focus status.
    pub is_focused: bool,
}

/// The reusable SplitDivider composite widget (`UIF-COMP-016`, `UIF-PANE-001`, §3B.16).
pub struct SplitDivider<'a, Message> {
    model: SplitDividerModel,
    total_length: Option<f32>,
    on_resize_change: Option<Box<dyn Fn(Live<f32>) -> Message + 'a>>,
    on_resize_commit: Option<Box<dyn Fn(Commit<f32>) -> Message + 'a>>,
    on_resize_cancel: Option<Box<dyn Fn(Cancel<SplitSnapshot>) -> Message + 'a>>,
}

impl<'a, Message> SplitDivider<'a, Message> {
    /// Create a new `SplitDivider` over `model` (`UIF-COMP-016`, `UIF-PANE-001`).
    #[must_use]
    pub fn new(model: SplitDividerModel) -> Self {
        Self {
            model,
            total_length: None,
            on_resize_change: None,
            on_resize_commit: None,
            on_resize_cancel: None,
        }
    }

    /// Set total length of the parent container along the split axis (`UIF-COMP-016`).
    #[must_use]
    pub fn with_total_length(mut self, length: f32) -> Self {
        self.total_length = Some(length);
        self
    }

    /// Callback emitted live as ratio changes during an active drag (`UIF-COMP-016`).
    #[must_use]
    pub fn on_resize_change<F>(mut self, f: F) -> Self
    where
        F: Fn(Live<f32>) -> Message + 'a,
    {
        self.on_resize_change = Some(Box::new(f));
        self
    }

    /// Callback emitted once on pointer release inside the grab zone (`UIF-COMP-016`).
    #[must_use]
    pub fn on_resize_commit<F>(mut self, f: F) -> Self
    where
        F: Fn(Commit<f32>) -> Message + 'a,
    {
        self.on_resize_commit = Some(Box::new(f));
        self
    }

    /// Callback emitted on pointer release outside grab zone or on Escape (`UIF-COMP-016`).
    #[must_use]
    pub fn on_resize_cancel<F>(mut self, f: F) -> Self
    where
        F: Fn(Cancel<SplitSnapshot>) -> Message + 'a,
    {
        self.on_resize_cancel = Some(Box::new(f));
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for SplitDivider<'a, Message>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SplitDividerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SplitDividerState::default())
    }

    fn size(&self) -> Size<Length> {
        match self.model.axis {
            SplitAxis::Horizontal => Size::new(Length::Fixed(size::SPLITTER_GRAB), Length::Fill),
            SplitAxis::Vertical => Size::new(Length::Fill, Length::Fixed(size::SPLITTER_GRAB)),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(
            match self.model.axis {
                SplitAxis::Horizontal => Length::Fixed(size::SPLITTER_GRAB),
                SplitAxis::Vertical => Length::Fill,
            },
            match self.model.axis {
                SplitAxis::Horizontal => Length::Fill,
                SplitAxis::Vertical => Length::Fixed(size::SPLITTER_GRAB),
            },
            Size::ZERO,
        );
        layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let grab_rect = compute_grab_rect(bounds, self.model.axis);
        let state = tree.state.downcast_mut::<SplitDividerState>();

        let is_hovered = cursor.is_over(grab_rect);
        if !state.resize.is_active() {
            state.is_hovered = is_hovered;
        }

        if self.model.disabled {
            return;
        }

        let total_len = self.total_length.unwrap_or(match self.model.axis {
            SplitAxis::Horizontal => {
                if viewport.width > 0.0 {
                    viewport.width
                } else {
                    0.0
                }
            }
            SplitAxis::Vertical => {
                if viewport.height > 0.0 {
                    viewport.height
                } else {
                    0.0
                }
            }
        });

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(grab_rect)
                    && let Some(pos) = cursor.position()
                {
                    let cursor_pos = match self.model.axis {
                        SplitAxis::Horizontal => pos.x,
                        SplitAxis::Vertical => pos.y,
                    };
                    let _ = state.resize.begin(
                        self.model.ratio,
                        cursor_pos,
                        total_len,
                        self.model.min_ratio,
                        self.model.max_ratio,
                    );
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.resize.is_active() {
                    let cursor_pos = match self.model.axis {
                        SplitAxis::Horizontal => position.x,
                        SplitAxis::Vertical => position.y,
                    };
                    if let Some(live) = state.resize.update(cursor_pos)
                        && let Some(on_change) = &self.on_resize_change
                    {
                        shell.publish((on_change)(live));
                    }
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.resize.is_active() {
                    if cursor.is_over(grab_rect) {
                        if let Some(commit) = state.resize.commit()
                            && let Some(on_commit) = &self.on_resize_commit
                        {
                            shell.publish((on_commit)(commit));
                        }
                    } else if let Some(cancel) = state.resize.cancel()
                        && let Some(on_cancel) = &self.on_resize_cancel
                    {
                        shell.publish((on_cancel)(cancel));
                    }
                    state.is_hovered = cursor.is_over(grab_rect);
                    shell.capture_event();
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                if *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                    && state.resize.is_active() =>
            {
                if let Some(cancel) = state.resize.cancel()
                    && let Some(on_cancel) = &self.on_resize_cancel
                {
                    shell.publish((on_cancel)(cancel));
                }
                state.is_hovered = cursor.is_over(grab_rect);
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.model.disabled {
            return mouse::Interaction::None;
        }

        let bounds = layout.bounds();
        let grab_rect = compute_grab_rect(bounds, self.model.axis);
        let state = tree.state.downcast_ref::<SplitDividerState>();

        if state.resize.is_active() || cursor.is_over(grab_rect) {
            match self.model.axis {
                SplitAxis::Horizontal => mouse::Interaction::ResizingHorizontally,
                SplitAxis::Vertical => mouse::Interaction::ResizingVertically,
            }
        } else {
            mouse::Interaction::None
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return;
        }

        let state = tree.state.downcast_ref::<SplitDividerState>();
        let is_active_drag = state.resize.is_active();
        let is_hovered = state.is_hovered;

        let input = InteractionInput {
            enabled: !self.model.disabled,
            hovered: is_hovered,
            pressed: is_active_drag,
            focused: state.is_focused,
            invalid: false,
        };
        let visual = state::resolve(input);
        let palette = Palette::dark();
        let layers = split_divider_layers(bounds, self.model.axis, visual, &palette);

        for (_name, layer) in layers {
            renderer.fill_quad(layer.quad(), layer.fill);
        }
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer: 'a + renderer::Renderer> From<SplitDivider<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(divider: SplitDivider<'a, Message>) -> Self {
        Element::new(divider)
    }
}

/// Helper function to create a new `SplitDivider` element (`UIF-COMP-016`, `UIF-PANE-001`).
#[must_use]
pub fn split_divider<'a, Message: 'a>(model: SplitDividerModel) -> SplitDivider<'a, Message> {
    SplitDivider::new(model)
}

#[cfg(test)]
#[path = "split_divider_tests.rs"]
mod split_divider_tests;
