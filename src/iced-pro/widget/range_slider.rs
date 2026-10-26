//! Paint resolver, geometry, and live interactive widget for `RangeSlider` (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).
//!
//! A dual-thumb continuous and stepped interval control conforming to §3.5:
//! - 4.0px pill rail with `surface.active` unselected background and `border.subtle` border
//! - Selected active rail segment in `accent.fill` (or `border.strong` when disabled)
//! - 16.0px thumbs with 24.0px minimum hit targets
//! - Tabular-numeral readout pill (`12px`, `FontFamily::Mono`) rendered 4px above
//!   any thumb that is `Hovered`, `Pressed`, or `Focused` without shifting outer layout (`UIF-PRIN-003`)
//! - Roving/tab keyboard focus traversing `Lower -> Upper -> next` control
//! - Three-tier scrub contract: `Live` on movement, `Commit` on release/keys, and `Cancel` on Escape
//!
//! Unit tests live in the sibling `range_slider_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, GestureSlot, Live};
use crate::focus::FocusId;
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography;
use crate::widget::focus::{self, FocusPlacement};
use crate::widget::focusable::FocusableState;
use crate::widget::range_model::RangeKey;
use crate::widget::range_slider_model::{RangeSliderModel, RangeThumb};

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight};
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree;
use iced::advanced::widget::{Id as WidgetId, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{
    Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Shadow, Size,
};

/// Rail height in logical pixels — §3.5's `size.slider.track_height` (`4px`, `UIF-ATOM-005b`).
pub const TRACK_HEIGHT: f32 = size::SLIDER_TRACK_HEIGHT;

/// Rail corner radius — `radius.pill` (`999px`, `UIF-ATOM-005b`).
pub const TRACK_RADIUS: f32 = shape::RADIUS_PILL;

/// Thumb diameter in logical pixels — §3.5's `size.slider.thumb` (`16px`, `UIF-ATOM-005b`).
pub const THUMB_SIZE: f32 = size::SLIDER_THUMB;

/// Thumb pointer hit target size — `size.hit_target_min` (`24px`, `UIF-ATOM-005b`).
pub const THUMB_HIT_SIZE: f32 = size::HIT_TARGET_MIN;

/// Thumb border ring width — `shape::BORDER_FOCUS` (`2px`, `UIF-ATOM-005b`).
pub const THUMB_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// Rail hairline border width — `shape::BORDER_HAIRLINE` (`1px`, `UIF-ATOM-005b`).
pub const RAIL_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Vertical gap between thumb and readout pill — `space::XS` (`4px`, `UIF-ATOM-005b`).
pub const READOUT_GAP: f32 = space::XS;

/// Corner radius of the readout pill — `shape::RADIUS_WIDGET` (`4px`, `UIF-ATOM-005b`).
pub const READOUT_RADIUS: f32 = shape::RADIUS_WIDGET;

/// Hairline border width of the readout pill — `shape::BORDER_HAIRLINE` (`1px`, `UIF-ATOM-005b`).
pub const READOUT_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Typography size for readout pill digits — `12px` caption (`UIF-ATOM-005b`, `UIF-TOK-004`).
pub const READOUT_FONT_SIZE: f32 = typography::CAPTION.size;

/// Horizontal padding inside the readout pill — `space::SM` (`8px`, `UIF-ATOM-005b`).
pub const READOUT_PADDING_X: f32 = space::SM;

/// Vertical padding inside the readout pill — `space::XXS` (`2px`, `UIF-ATOM-005b`).
pub const READOUT_PADDING_Y: f32 = space::XXS;

/// Focus ring width — `shape::BORDER_FOCUS` (`2px`, `UIF-ATOM-005b`).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// Resolved paint colors for a `RangeSlider` (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeSliderStyle {
    /// Unselected rail groove fill (`surface.active`, `UIF-ATOM-005b`).
    pub rail_unselected: Color,
    /// Unselected rail border stroke (`border.subtle`, `UIF-ATOM-005b`).
    pub rail_border: Color,
    /// Selected interval fill between thumbs (`accent.fill` or `border.strong`, `UIF-ATOM-005b`).
    pub rail_selected: Color,
    /// Thumb face in resting state (`text.strong`, `UIF-ATOM-005b`).
    pub thumb_resting_fill: Color,
    /// Thumb ring border in resting state (`accent.border`, `UIF-ATOM-005b`).
    pub thumb_resting_border: Color,
    /// Thumb face in hovered/pressed state (`accent.fill`, `UIF-ATOM-005b`).
    pub thumb_hovered_fill: Color,
    /// Thumb ring border in hovered/pressed state (`text.strong`, `UIF-ATOM-005b`).
    pub thumb_hovered_border: Color,
    /// Thumb face in disabled state (`surface.raised`, `UIF-ATOM-005b`).
    pub thumb_disabled_fill: Color,
    /// Thumb ring border in disabled state (`text.disabled`, `UIF-ATOM-005b`).
    pub thumb_disabled_border: Color,
    /// Value readout pill background (`surface.overlay`, `UIF-ATOM-005b`).
    pub readout_bg: Color,
    /// Value readout pill border (`border.strong`, `UIF-ATOM-005b`).
    pub readout_border: Color,
    /// Value readout pill text color (`text.primary`, `UIF-ATOM-005b`).
    pub readout_text: Color,
    /// Keyboard focus ring indicator color (`UIF-ATOM-005b`).
    pub focus_ring_color: Color,
    /// Keyboard focus ring indicator width (`UIF-ATOM-005b`).
    pub focus_ring_width: f32,
}

/// Resolve `RangeSlider` paint tokens for a given palette and disabled state (§3.5, `UIF-ATOM-005b`).
#[must_use]
pub fn resolve_style(palette: &Palette, disabled: bool) -> RangeSliderStyle {
    let rail_selected = if disabled {
        palette.border_strong
    } else {
        palette.accent_fill
    };

    RangeSliderStyle {
        rail_unselected: palette.surface_active,
        rail_border: palette.border_subtle,
        rail_selected,
        thumb_resting_fill: palette.text_strong,
        thumb_resting_border: palette.accent_border,
        thumb_hovered_fill: palette.accent_fill,
        thumb_hovered_border: palette.text_strong,
        thumb_disabled_fill: palette.surface_raised,
        thumb_disabled_border: palette.text_disabled,
        readout_bg: palette.surface_overlay,
        readout_border: palette.border_strong,
        readout_text: palette.text_primary,
        focus_ring_color: palette.accent_border,
        focus_ring_width: FOCUS_RING_WIDTH,
    }
}

/// Determine thumb `(fill, border)` colors for a given visual state (§3.5, `UIF-ATOM-005b`).
#[must_use]
pub fn thumb_paint(style: &RangeSliderStyle, state: VisualState) -> (Color, Color) {
    match state.base {
        BaseState::Disabled => (style.thumb_disabled_fill, style.thumb_disabled_border),
        BaseState::Hovered | BaseState::Pressed => {
            (style.thumb_hovered_fill, style.thumb_hovered_border)
        }
        BaseState::Rest | BaseState::Invalid => {
            (style.thumb_resting_fill, style.thumb_resting_border)
        }
    }
}

// =============================================================================
// Geometry calculations (§3.5, `UIF-ATOM-005b`)
// =============================================================================

/// Compute the rail track rectangle, vertically centered within bounds (`UIF-ATOM-005b`).
#[must_use]
pub fn track_rect(bounds: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(bounds.x, bounds.y + (bounds.height - TRACK_HEIGHT) / 2.0),
        Size::new(bounds.width, TRACK_HEIGHT),
    )
}

/// Travel distance between thumb centers across the track (`UIF-ATOM-005b`).
#[must_use]
pub fn thumb_travel(track: Rectangle) -> f32 {
    (track.width - THUMB_SIZE).max(0.0)
}

/// Compute center x position of a thumb at a normalized fraction (`UIF-ATOM-005b`).
#[must_use]
pub fn thumb_center_x(track: Rectangle, fraction: f32) -> f32 {
    let t = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    track.x + THUMB_SIZE / 2.0 + t * thumb_travel(track)
}

/// Compute normalized track fraction for an x-coordinate (`UIF-ATOM-005b`).
#[must_use]
pub fn fraction_at_x(track: Rectangle, x: f32) -> f32 {
    if x.is_nan() {
        return 0.0;
    }
    let span = thumb_travel(track);
    if span <= 0.0 {
        return 0.0;
    }
    ((x - track.x - THUMB_SIZE / 2.0) / span).clamp(0.0, 1.0)
}

/// Compute the thumb face rectangle at a given track fraction (`UIF-ATOM-005b`).
#[must_use]
pub fn thumb_rect(bounds: Rectangle, fraction: f32) -> Rectangle {
    let track = track_rect(bounds);
    let cx = thumb_center_x(track, fraction);
    Rectangle::new(
        Point::new(
            cx - THUMB_SIZE / 2.0,
            track.y + (TRACK_HEIGHT - THUMB_SIZE) / 2.0,
        ),
        Size::new(THUMB_SIZE, THUMB_SIZE),
    )
}

/// Compute the 24px minimum hit target rectangle around a thumb (`UIF-ATOM-005b`).
#[must_use]
pub fn hit_rect(bounds: Rectangle, fraction: f32) -> Rectangle {
    let track = track_rect(bounds);
    let cx = thumb_center_x(track, fraction);
    Rectangle::new(
        Point::new(
            cx - THUMB_HIT_SIZE / 2.0,
            track.y + (TRACK_HEIGHT - THUMB_HIT_SIZE) / 2.0,
        ),
        Size::new(THUMB_HIT_SIZE, THUMB_HIT_SIZE),
    )
}

/// Compute the active selected rail segment between lower and upper thumbs (`UIF-ATOM-005b`).
#[must_use]
pub fn active_segment_rect(
    bounds: Rectangle,
    lower_fraction: f32,
    upper_fraction: f32,
) -> Rectangle {
    let track = track_rect(bounds);
    let lx = thumb_center_x(track, lower_fraction);
    let ux = thumb_center_x(track, upper_fraction);
    let left = lx.min(ux);
    let width = (ux - lx).abs();
    Rectangle::new(Point::new(left, track.y), Size::new(width, TRACK_HEIGHT))
}

/// Compute the readout pill rectangle positioned 4px above the thumb (`UIF-ATOM-005b`).
#[must_use]
pub fn readout_rect(thumb_box: Rectangle, text_size: Size) -> Rectangle {
    let pill_w = (text_size.width + READOUT_PADDING_X * 2.0).max(size::HIT_TARGET_MIN);
    let pill_h = (text_size.height + READOUT_PADDING_Y * 2.0).max(typography::CAPTION.line_height);
    let pill_x = thumb_box.x + (thumb_box.width - pill_w) / 2.0;
    let pill_y = thumb_box.y - READOUT_GAP - pill_h;
    Rectangle::new(Point::new(pill_x, pill_y), Size::new(pill_w, pill_h))
}

// =============================================================================
// Ephemeral State & Widget
// =============================================================================

/// Ephemeral widget state retained in iced's widget tree (`UIF-ATOM-005b`).
#[derive(Clone, Debug, Default)]
pub struct RangeSliderState {
    /// Thumb currently under the pointer (`UIF-ATOM-005b`).
    pub hovered_thumb: Option<RangeThumb>,
    /// Thumb pressed by the mouse button (`UIF-ATOM-005b`).
    pub pressed_thumb: Option<RangeThumb>,
    /// Thumb currently captured in an active pointer drag (`UIF-ATOM-005b`).
    pub captured_thumb: Option<RangeThumb>,
    /// Linear gesture slot owning the scrub snapshot (`UIF-ATOM-005b`).
    pub gesture: GestureSlot<(f64, f64)>,
    /// Focus router state for lower thumb (`UIF-ATOM-005b`).
    pub lower_focusable: FocusableState,
    /// Focus router state for upper thumb (`UIF-ATOM-005b`).
    pub upper_focusable: FocusableState,
}

impl RangeSliderState {
    /// Create a new resting `RangeSliderState` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The thumb currently holding keyboard focus, if either (`UIF-ATOM-005b`).
    #[must_use]
    pub fn focused_thumb(&self) -> Option<RangeThumb> {
        if self.lower_focusable.is_focused() {
            Some(RangeThumb::Lower)
        } else if self.upper_focusable.is_focused() {
            Some(RangeThumb::Upper)
        } else {
            None
        }
    }

    /// Whether a thumb drag gesture is currently capturing the pointer (`UIF-ATOM-005b`).
    #[must_use]
    pub fn is_capturing(&self) -> bool {
        self.captured_thumb.is_some()
    }
}

/// Closure type emitting a message during continuous thumb scrub (`Live<(f64, f64)>`) (`UIF-ATOM-005b`).
pub type RangeChange<'a, Message> = Box<dyn Fn(Live<(f64, f64)>) -> Option<Message> + 'a>;

/// Closure type emitting a message once on interaction commit (`Commit<(f64, f64)>`) (`UIF-ATOM-005b`).
pub type RangeCommit<'a, Message> = Box<dyn Fn(Commit<(f64, f64)>) -> Option<Message> + 'a>;

/// Closure type emitting a message on scrub cancellation (`Cancel<(f64, f64)>`) (`UIF-ATOM-005b`).
pub type RangeCancel<'a, Message> = Box<dyn Fn(Cancel<(f64, f64)>) -> Option<Message> + 'a>;

/// Live interactive `RangeSlider` widget for continuous/stepped intervals (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).
pub struct RangeSlider<'a, Message> {
    focus_id: FocusId,
    lower_widget_id: WidgetId,
    upper_widget_id: WidgetId,
    model: RangeSliderModel,
    palette: Palette,
    on_change: Option<RangeChange<'a, Message>>,
    on_commit: Option<RangeCommit<'a, Message>>,
    on_cancel: Option<RangeCancel<'a, Message>>,
}

impl<'a, Message> RangeSlider<'a, Message> {
    /// Create a new `RangeSlider` backed by `model` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn new(model: RangeSliderModel) -> Self {
        let focus_id = FocusId::new("range_slider");
        let lower_widget_id = crate::bridge::widget_id(&FocusId::new("range_slider.lower"));
        let upper_widget_id = crate::bridge::widget_id(&FocusId::new("range_slider.upper"));
        Self {
            focus_id,
            lower_widget_id,
            upper_widget_id,
            model,
            palette: Palette::dark(),
            on_change: None,
            on_commit: None,
            on_cancel: None,
        }
    }

    /// Assign custom focus id for keyboard focus routing (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_focus_id(mut self, focus_id: impl Into<FocusId>) -> Self {
        let focus_id = focus_id.into();
        self.lower_widget_id =
            crate::bridge::widget_id(&FocusId::new(format!("{}.lower", focus_id.as_str())));
        self.upper_widget_id =
            crate::bridge::widget_id(&FocusId::new(format!("{}.upper", focus_id.as_str())));
        self.focus_id = focus_id;
        self
    }

    /// Set theme palette for token resolution (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Callback emitted continuously during thumb scrub (`Live<(f64, f64)>`) (`UIF-ATOM-005b`).
    #[must_use]
    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: Fn(Live<(f64, f64)>) -> Option<Message> + 'a,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    /// Callback emitted once when interaction settles (`Commit<(f64, f64)>`) (`UIF-ATOM-005b`).
    #[must_use]
    pub fn on_commit<F>(mut self, on_commit: F) -> Self
    where
        F: Fn(Commit<(f64, f64)>) -> Option<Message> + 'a,
    {
        self.on_commit = Some(Box::new(on_commit));
        self
    }

    /// Callback emitted when scrub is cancelled with Escape (`Cancel<(f64, f64)>`) (`UIF-ATOM-005b`).
    #[must_use]
    pub fn on_cancel<F>(mut self, on_cancel: F) -> Self
    where
        F: Fn(Cancel<(f64, f64)>) -> Option<Message> + 'a,
    {
        self.on_cancel = Some(Box::new(on_cancel));
        self
    }

    /// Access the underlying `RangeSliderModel` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn model(&self) -> &RangeSliderModel {
        &self.model
    }

    /// Access the palette (`UIF-ATOM-005b`).
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Access the registered focus identifier (`UIF-ATOM-005b`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }
}

/// Helper constructor to create a `RangeSlider` from a model (`UIF-ATOM-005b`).
#[must_use]
pub fn range_slider<'a, Message>(model: RangeSliderModel) -> RangeSlider<'a, Message> {
    RangeSlider::new(model)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for RangeSlider<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<RangeSliderState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(RangeSliderState::new())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(THUMB_HIT_SIZE))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(
            Length::Fill,
            Length::Fixed(THUMB_HIT_SIZE),
            Size::new(100.0, THUMB_HIT_SIZE),
        );
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<RangeSliderState>();
        let bounds = layout.bounds();
        let lower_fraction = self.model.lower_fraction();
        let upper_fraction = self.model.upper_fraction();

        if !self.model.disabled {
            let lower_box = hit_rect(bounds, lower_fraction);
            let upper_box = hit_rect(bounds, upper_fraction);
            operation.focusable(
                Some(&self.lower_widget_id),
                lower_box,
                &mut state.lower_focusable,
            );
            operation.focusable(
                Some(&self.upper_widget_id),
                upper_box,
                &mut state.upper_focusable,
            );
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.lower_focusable);
            <FocusableState as operation::Focusable>::unfocus(&mut state.upper_focusable);
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

        let state = tree.state.downcast_ref::<RangeSliderState>();
        if state.is_capturing() {
            return mouse::Interaction::Grabbing;
        }

        if let Some(pos) = cursor.position_over(layout.bounds()) {
            let lower_hit = hit_rect(layout.bounds(), self.model.lower_fraction());
            let upper_hit = hit_rect(layout.bounds(), self.model.upper_fraction());
            if lower_hit.contains(pos) || upper_hit.contains(pos) {
                return mouse::Interaction::Pointer;
            }
        }

        mouse::Interaction::None
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

        let state = tree.state.downcast_mut::<RangeSliderState>();
        let bounds = layout.bounds();
        let track = track_rect(bounds);

        if self.model.disabled {
            state.hovered_thumb = None;
            state.pressed_thumb = None;
            state.captured_thumb = None;
            state.gesture.abort();
            return;
        }

        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let lower_hit = hit_rect(bounds, self.model.lower_fraction());
                let upper_hit = hit_rect(bounds, self.model.upper_fraction());

                // Track hover state when not capturing
                if !state.is_capturing() {
                    let in_lower = lower_hit.contains(*position);
                    let in_upper = upper_hit.contains(*position);
                    if in_lower && in_upper {
                        let click_f = fraction_at_x(track, position.x) as f64;
                        state.hovered_thumb = Some(self.model.disambiguate_thumb(click_f));
                    } else if in_lower {
                        state.hovered_thumb = Some(RangeThumb::Lower);
                    } else if in_upper {
                        state.hovered_thumb = Some(RangeThumb::Upper);
                    } else {
                        state.hovered_thumb = None;
                    }
                }

                // If pointer is captured in an active drag: move the captured thumb
                if let Some(captured) = state.captured_thumb {
                    let click_f = fraction_at_x(track, position.x);
                    let target_val = self.model.value_at_fraction(click_f);

                    let (new_l, new_u) = match captured {
                        RangeThumb::Lower => {
                            let clamped_l = self.model.clamp_lower(target_val);
                            (clamped_l, self.model.clamped_upper())
                        }
                        RangeThumb::Upper => {
                            let clamped_u = self.model.clamp_upper(target_val);
                            (self.model.clamped_lower(), clamped_u)
                        }
                    };

                    if let Some(live) = state.gesture.live((new_l, new_u))
                        && let Some(on_change) = &self.on_change
                        && let Some(msg) = on_change(live)
                    {
                        shell.publish(msg);
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    let click_f = fraction_at_x(track, pos.x) as f64;
                    let thumb = self.model.disambiguate_thumb(click_f);

                    state.pressed_thumb = Some(thumb);
                    state.captured_thumb = Some(thumb);

                    // Begin gesture linear slot
                    let current_pair = (self.model.clamped_lower(), self.model.clamped_upper());
                    let _ = state.gesture.begin(&current_pair);

                    // Focus the targeted thumb
                    match thumb {
                        RangeThumb::Lower => {
                            <FocusableState as operation::Focusable>::focus(
                                &mut state.lower_focusable,
                            );
                            <FocusableState as operation::Focusable>::unfocus(
                                &mut state.upper_focusable,
                            );
                        }
                        RangeThumb::Upper => {
                            <FocusableState as operation::Focusable>::unfocus(
                                &mut state.lower_focusable,
                            );
                            <FocusableState as operation::Focusable>::focus(
                                &mut state.upper_focusable,
                            );
                        }
                    }

                    // Immediately move targeted thumb to click location
                    let target_val = self.model.value_at_fraction(click_f as f32);
                    let (new_l, new_u) = match thumb {
                        RangeThumb::Lower => (
                            self.model.clamp_lower(target_val),
                            self.model.clamped_upper(),
                        ),
                        RangeThumb::Upper => (
                            self.model.clamped_lower(),
                            self.model.clamp_upper(target_val),
                        ),
                    };

                    if let Some(live) = state.gesture.live((new_l, new_u))
                        && let Some(on_change) = &self.on_change
                        && let Some(msg) = on_change(live)
                    {
                        shell.publish(msg);
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(captured) = state.captured_thumb.take() {
                    state.pressed_thumb = None;

                    // Compute final settled value
                    let final_pair = if let Some(cursor_pos) = cursor.position() {
                        let click_f = fraction_at_x(track, cursor_pos.x);
                        let target_val = self.model.value_at_fraction(click_f);
                        match captured {
                            RangeThumb::Lower => (
                                self.model.clamp_lower(target_val),
                                self.model.clamped_upper(),
                            ),
                            RangeThumb::Upper => (
                                self.model.clamped_lower(),
                                self.model.clamp_upper(target_val),
                            ),
                        }
                    } else {
                        (self.model.clamped_lower(), self.model.clamped_upper())
                    };

                    if let Some(commit) = state.gesture.commit(final_pair)
                        && let Some(on_commit) = &self.on_commit
                        && let Some(msg) = on_commit(commit)
                    {
                        shell.publish(msg);
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                // Escape during active drag cancels gesture and reverts to pre-drag value
                if *key == Key::Named(Named::Escape) && state.gesture.is_open() {
                    state.captured_thumb = None;
                    state.pressed_thumb = None;
                    if let Some(cancel) = state.gesture.cancel()
                        && let Some(on_cancel) = &self.on_cancel
                        && let Some(msg) = on_cancel(cancel)
                    {
                        shell.publish(msg);
                    }
                    shell.capture_event();
                    return;
                }

                // Tab roving navigation between Lower and Upper thumbs
                if *key == Key::Named(Named::Tab) {
                    if modifiers.shift() && state.upper_focusable.is_focused() {
                        <FocusableState as operation::Focusable>::unfocus(
                            &mut state.upper_focusable,
                        );
                        <FocusableState as operation::Focusable>::focus(&mut state.lower_focusable);
                        shell.capture_event();
                        return;
                    } else if !modifiers.shift() && state.lower_focusable.is_focused() {
                        <FocusableState as operation::Focusable>::unfocus(
                            &mut state.lower_focusable,
                        );
                        <FocusableState as operation::Focusable>::focus(&mut state.upper_focusable);
                        shell.capture_event();
                        return;
                    }
                }

                // Keyboard stepping on focused thumb
                if let Some(thumb) = state.focused_thumb() {
                    let range_key = match key {
                        Key::Named(Named::ArrowLeft | Named::ArrowDown) => Some(RangeKey::Decrease),
                        Key::Named(Named::ArrowRight | Named::ArrowUp) => Some(RangeKey::Increase),
                        Key::Named(Named::PageDown) => Some(RangeKey::PageDown),
                        Key::Named(Named::PageUp) => Some(RangeKey::PageUp),
                        Key::Named(Named::Home) => Some(RangeKey::Home),
                        Key::Named(Named::End) => Some(RangeKey::End),
                        _ => None,
                    };

                    if let Some(rk) = range_key {
                        let new_range = self.model.step_thumb(thumb, rk);
                        let commit = Commit::new(new_range);
                        if let Some(on_commit) = &self.on_commit
                            && let Some(msg) = on_commit(commit)
                        {
                            shell.publish(msg);
                        }
                        shell.capture_event();
                    }
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
        let state = tree.state.downcast_ref::<RangeSliderState>();
        let bounds = layout.bounds();
        let track = track_rect(bounds);
        let style = resolve_style(&self.palette, self.model.disabled);

        let lower_fraction = self.model.lower_fraction();
        let upper_fraction = self.model.upper_fraction();

        // 1. Draw unselected rail background
        renderer.fill_quad(
            renderer::Quad {
                bounds: track,
                border: Border::default()
                    .color(style.rail_border)
                    .width(RAIL_BORDER_WIDTH)
                    .rounded(TRACK_RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(style.rail_unselected),
        );

        // 2. Draw selected rail segment between thumbs
        let active_seg = active_segment_rect(bounds, lower_fraction, upper_fraction);
        if active_seg.width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: active_seg,
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(style.rail_selected),
            );
        }

        // 3. Draw thumbs
        let lower_box = thumb_rect(bounds, lower_fraction);
        let upper_box = thumb_rect(bounds, upper_fraction);

        let lower_visual = VisualState {
            base: if self.model.disabled {
                BaseState::Disabled
            } else if state.pressed_thumb == Some(RangeThumb::Lower) {
                BaseState::Pressed
            } else if state.hovered_thumb == Some(RangeThumb::Lower) {
                BaseState::Hovered
            } else {
                BaseState::Rest
            },
            focused: state.lower_focusable.is_focused(),
        };

        let upper_visual = VisualState {
            base: if self.model.disabled {
                BaseState::Disabled
            } else if state.pressed_thumb == Some(RangeThumb::Upper) {
                BaseState::Pressed
            } else if state.hovered_thumb == Some(RangeThumb::Upper) {
                BaseState::Hovered
            } else {
                BaseState::Rest
            },
            focused: state.upper_focusable.is_focused(),
        };

        // Draw Lower Thumb
        let (lower_fill, lower_border) = thumb_paint(&style, lower_visual);
        renderer.fill_quad(
            renderer::Quad {
                bounds: lower_box,
                border: Border::default()
                    .color(lower_border)
                    .width(THUMB_RING_WIDTH)
                    .rounded(TRACK_RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(lower_fill),
        );

        if lower_visual.focused {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(lower_box),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(TRACK_RADIUS)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // Draw Upper Thumb
        let (upper_fill, upper_border) = thumb_paint(&style, upper_visual);
        renderer.fill_quad(
            renderer::Quad {
                bounds: upper_box,
                border: Border::default()
                    .color(upper_border)
                    .width(THUMB_RING_WIDTH)
                    .rounded(TRACK_RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(upper_fill),
        );

        if upper_visual.focused {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(upper_box),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(TRACK_RADIUS)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // 4. Draw Readout Pills when Hovered, Pressed, or Focused
        let show_lower_readout = lower_visual.base == BaseState::Hovered
            || lower_visual.base == BaseState::Pressed
            || lower_visual.focused;

        if show_lower_readout {
            let text = self.model.display_lower();
            draw_readout_pill(renderer, lower_box, &text, &style, viewport);
        }

        let show_upper_readout = upper_visual.base == BaseState::Hovered
            || upper_visual.base == BaseState::Pressed
            || upper_visual.focused;

        if show_upper_readout {
            let text = self.model.display_upper();
            draw_readout_pill(renderer, upper_box, &text, &style, viewport);
        }
    }
}

/// Helper function to draw a 12px Mono caption readout pill 4px above a thumb (`UIF-ATOM-005b`).
fn draw_readout_pill<Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>>(
    renderer: &mut Renderer,
    thumb_box: Rectangle,
    text: &str,
    style: &RangeSliderStyle,
    viewport: &Rectangle,
) {
    let text_w = text.len() as f32 * (READOUT_FONT_SIZE * 0.6);
    let pill_w = (text_w + READOUT_PADDING_X * 2.0).max(size::HIT_TARGET_MIN);
    let pill_h = typography::CAPTION.line_height + READOUT_PADDING_Y * 2.0;
    let pill_box = readout_rect(thumb_box, Size::new(pill_w, pill_h));

    // Fill quad background
    renderer.fill_quad(
        renderer::Quad {
            bounds: pill_box,
            border: Border::default()
                .color(style.readout_border)
                .width(READOUT_BORDER_WIDTH)
                .rounded(READOUT_RADIUS),
            shadow: Shadow::default(),
            snap: true,
        },
        Background::Color(style.readout_bg),
    );

    // Fill mono text
    renderer.fill_text(
        text_core::Text {
            content: text.to_owned(),
            bounds: pill_box.size(),
            size: Pixels(READOUT_FONT_SIZE),
            line_height: LineHeight::Absolute(Pixels(pill_box.height)),
            font: typography::FAMILY_MONO.regular(),
            align_x: text_core::Alignment::Center,
            align_y: alignment::Vertical::Center,
            shaping: text_core::Shaping::default(),
            wrapping: text_core::Wrapping::None,
        },
        pill_box.position(),
        style.readout_text,
        *viewport,
    );
}

impl<'a, Message, Theme, Renderer> From<RangeSlider<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(slider: RangeSlider<'a, Message>) -> Self {
        Element::new(slider)
    }
}

#[cfg(test)]
#[path = "range_slider_tests.rs"]
mod range_slider_tests;
