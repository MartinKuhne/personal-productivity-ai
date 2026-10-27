//! The NavigationRail / Collapsible NavigationView composite widget (`UIF-COMP-009`, `UIF-PRIN-005`, `UIF-PRIN-006`).
//!
//! §3B.9 specifies a vertical navigation rail supporting both compact (48px wide icon-only)
//! and expanded (220px default, resizable across `[180px, 320px]`) modes.
//!
//! - **Compact mode (48px wide):** Centered 36×36px cells with 16px icons, optional badge count indicators,
//!   and a mandatory right-aligned hover [`crate::surface::tooltip::Tooltip`] displaying the destination label using the warm
//!   handoff window (`time::DELAY_TOOLTIP_WARM`).
//! - **Expanded mode (220px default, 180–320px resizable):** Leading 16px icon, 13px Medium label
//!   (`typography::UI_COMPACT`, `text.strong` or `text.primary`), optional trailing [`BadgeModel`] pill.
//! - **Selected destination:** `surface.active` fill (14.29:1 contrast), `text.strong`, and a 2px
//!   `palette.accent.border` (4.79:1 contrast) leading indicator bar along the left edge.
//! - **Header & Toggle:** 28×28px (`size::CONTROL_SM`) toggle button at top toggling between compact and expanded.
//! - **Middle section:** Scrollable destination list.
//! - **Pinned bottom section:** Separated by a full-width `1px border.subtle` divider, holding utility
//!   destinations (e.g. Settings, Help).
//! - **Resize handle (when Expanded):** 10px grab zone on the right edge (`size::SPLITTER_GRAB`), resizes
//!   within `[180px, 320px]` tracked by [`GestureSlot<f32>`]. Emits live, commit, and cancel callbacks.
//! - **Keyboard navigation:** Single Tab stop with roving focus over enabled destinations (`Up`/`Down`,
//!   `Home`/`End`, `Enter`/`Space` activation).
//!
//! Unit tests live in the sibling `navigation_rail_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart, Live};
use crate::composite::Icon;
use crate::surface::tooltip::hover::{DismissReason, HoverController, TimeMs};
use crate::surface::tooltip::warm::WarmSession;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::badge::resolve_style as resolve_badge_style;
use crate::widget::badge_model::BadgeModel;
use crate::widget::focusable::FocusableState;
use crate::widget::range_model::RangeKey;
use crate::widget::rove::{RoveDirection, rove_index};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph as _};
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::alignment;
use iced::mouse;
use iced::{Element, Length, Pixels, Point, Rectangle, Size};

// ---------------------------------------------------------------------------
// String Constants ([RUST-021])
// ---------------------------------------------------------------------------

/// Default glyph for the navigation rail expand/collapse toggle button — `"☰"` ([RUST-021]).
pub const GLYPH_TOGGLE: &str = "☰";

/// Accessibility label for the navigation rail expand/collapse toggle button ([RUST-021]).
pub const LABEL_TOGGLE: &str = "Toggle navigation rail";

// ---------------------------------------------------------------------------
// Surface Geometry Constants & Tokens (§3B.9)
// ---------------------------------------------------------------------------

/// Compact rail width — 48px (§3B.9, `UIF-COMP-009`).
pub const COMPACT_WIDTH: f32 = 48.0;

/// Default expanded rail width — 220px (§3B.9, `UIF-COMP-009`).
pub const EXPANDED_DEFAULT_WIDTH: f32 = 220.0;

/// Minimum expanded rail width — 180px (§3B.9, `UIF-COMP-009`).
pub const EXPANDED_MIN_WIDTH: f32 = 180.0;

/// Maximum expanded rail width — 320px (§3B.9, `UIF-COMP-009`).
pub const EXPANDED_MAX_WIDTH: f32 = 320.0;

/// Row height for destinations and header — `size::CONTROL_LG` (36px, §3B.9, `UIF-COMP-009`).
pub const ROW_HEIGHT: f32 = size::CONTROL_LG;

/// Toggle button size — `size::CONTROL_SM` (28px, §3B.9, `UIF-COMP-009`).
pub const TOGGLE_SIZE: f32 = size::CONTROL_SM;

/// Standard icon slot size — `size::ICON_MD` (16px, §3B.9, `UIF-COMP-009`).
pub const ICON_SIZE: f32 = size::ICON_MD;

/// Selected destination leading indicator bar thickness — `shape::BORDER_FOCUS` (2px, §3B.9, `UIF-PRIN-006`).
pub const INDICATOR_WIDTH: f32 = shape::BORDER_FOCUS;

/// Resize grab band width — `size::SPLITTER_GRAB` (10px, §3B.9, `UIF-COMP-009`).
pub const GRAB_WIDTH: f32 = size::SPLITTER_GRAB;

/// Hairline divider stroke — `shape::BORDER_HAIRLINE` (1px, §3B.9, `UIF-PRIN-006`).
pub const DIVIDER_STROKE: f32 = shape::BORDER_HAIRLINE;

// ---------------------------------------------------------------------------
// Data Model
// ---------------------------------------------------------------------------

/// A destination entry in a [`NavigationRail`] (§3B.9, `UIF-COMP-009`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavDestination<DestId> {
    /// Unique identifier for this destination.
    pub id: DestId,
    /// Human-readable label for this destination.
    pub label: String,
    /// Icon glyph or symbol for this destination.
    pub icon: Icon,
    /// Optional badge notification pill or count marker.
    pub badge: Option<BadgeModel>,
    /// Whether this destination is disabled and skipped during roving navigation.
    pub disabled: bool,
}

impl<DestId> NavDestination<DestId> {
    /// Construct a new destination with default enabled state and no badge.
    #[must_use]
    pub fn new(id: DestId, label: impl Into<String>, icon: impl Into<Icon>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: icon.into(),
            badge: None,
            disabled: false,
        }
    }

    /// Attach an optional badge model to this destination.
    #[must_use]
    pub fn with_badge(mut self, badge: impl Into<Option<BadgeModel>>) -> Self {
        self.badge = badge.into();
        self
    }

    /// Set whether this destination is disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The controlled state model for a [`NavigationRail`] (§3B.9, `UIF-COMP-009`).
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationRailModel<DestId> {
    /// Destinations pinned to the top scrollable list.
    pub top_items: Vec<NavDestination<DestId>>,
    /// Utility destinations pinned to the bottom (e.g. Settings, Help).
    pub bottom_items: Vec<NavDestination<DestId>>,
    /// Currently selected destination identifier.
    pub selected: DestId,
    /// Whether the navigation rail is in expanded (vs compact) mode.
    pub expanded: bool,
    /// Width of the rail when expanded, clamped to `[180px, 320px]`.
    pub expanded_width: f32,
}

impl<DestId: Clone + PartialEq> NavigationRailModel<DestId> {
    /// Construct a new navigation rail model.
    #[must_use]
    pub fn new(
        top_items: Vec<NavDestination<DestId>>,
        bottom_items: Vec<NavDestination<DestId>>,
        selected: DestId,
    ) -> Self {
        Self {
            top_items,
            bottom_items,
            selected,
            expanded: true,
            expanded_width: EXPANDED_DEFAULT_WIDTH,
        }
    }

    /// Set whether the rail is expanded.
    #[must_use]
    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set the expanded width, clamped to `[EXPANDED_MIN_WIDTH, EXPANDED_MAX_WIDTH]`.
    #[must_use]
    pub fn with_expanded_width(mut self, width: f32) -> Self {
        self.expanded_width = Self::clamp_expanded_width(width);
        self
    }

    /// Clamp a requested expanded width to the specification range `[180px, 320px]` (§3B.9).
    #[must_use]
    pub fn clamp_expanded_width(w: f32) -> f32 {
        w.clamp(EXPANDED_MIN_WIDTH, EXPANDED_MAX_WIDTH)
    }

    /// Current layout width based on `expanded` flag and `expanded_width`.
    #[must_use]
    pub fn current_width(&self) -> f32 {
        if self.expanded {
            Self::clamp_expanded_width(self.expanded_width)
        } else {
            COMPACT_WIDTH
        }
    }

    /// Iterator over all destination items (top followed by bottom).
    pub fn all_items(&self) -> impl Iterator<Item = &NavDestination<DestId>> {
        self.top_items.iter().chain(self.bottom_items.iter())
    }

    /// Returns the identifiers of all enabled destinations.
    #[must_use]
    pub fn enabled_destinations(&self) -> Vec<DestId> {
        self.all_items()
            .filter(|item| !item.disabled)
            .map(|item| item.id.clone())
            .collect()
    }

    /// Rove navigation focus over enabled destinations according to `key` (`UIF-COMP-009`).
    #[must_use]
    pub fn rove(&self, key: RangeKey) -> Option<DestId> {
        let items: Vec<&NavDestination<DestId>> = self.all_items().collect();
        let enabled_indices: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.disabled)
            .map(|(i, _)| i)
            .collect();

        if enabled_indices.is_empty() {
            return None;
        }

        match key {
            RangeKey::Home => {
                let first_idx = *enabled_indices.first()?;
                Some(items[first_idx].id.clone())
            }
            RangeKey::End => {
                let last_idx = *enabled_indices.last()?;
                Some(items[last_idx].id.clone())
            }
            RangeKey::Increase | RangeKey::PageDown => {
                let from = items.iter().position(|item| item.id == self.selected);
                let next_idx = rove_index(&enabled_indices, from, RoveDirection::Forward)?;
                Some(items[next_idx].id.clone())
            }
            RangeKey::Decrease | RangeKey::PageUp => {
                let from = items.iter().position(|item| item.id == self.selected);
                let next_idx = rove_index(&enabled_indices, from, RoveDirection::Backward)?;
                Some(items[next_idx].id.clone())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Resize Gesture Engine
// ---------------------------------------------------------------------------

/// The in-flight resize gesture for [`NavigationRail`] (`UIF-COMP-009`, `UIF-PRIN-006`).
///
/// Uses [`GestureSlot<f32>`] to track edge drag linearly. Captures width
/// at press; live drag ticks emit [`Live<f32>`]; release emits [`Commit<f32>`];
/// Escape reverts to pre-drag snapshot via [`Cancel<f32>`].
#[derive(Clone, Debug)]
pub struct NavRailResize {
    slot: GestureSlot<f32>,
    start_width: f32,
    start_x: f32,
    current: f32,
}

impl Default for NavRailResize {
    fn default() -> Self {
        Self::new()
    }
}

impl NavRailResize {
    /// Create a new idle resize gesture.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slot: GestureSlot::new(),
            start_width: EXPANDED_DEFAULT_WIDTH,
            start_x: 0.0,
            current: EXPANDED_DEFAULT_WIDTH,
        }
    }

    /// Whether an active drag gesture is currently open.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.slot.is_open()
    }

    /// Pre-drag width captured when the gesture started.
    #[must_use]
    pub fn start_width(&self) -> f32 {
        self.start_width
    }

    /// Current live width tracked by the gesture.
    #[must_use]
    pub fn current_width(&self) -> f32 {
        self.current
    }

    /// Begin a resize gesture, capturing `start_width` and `start_x`.
    pub fn begin(&mut self, start_width: f32, start_x: f32) -> GestureStart {
        let clamped = NavigationRailModel::<()>::clamp_expanded_width(start_width);
        let start = self.slot.begin(&clamped);
        if start == GestureStart::Started {
            self.start_width = clamped;
            self.start_x = start_x;
            self.current = clamped;
        }
        start
    }

    /// Update gesture with new cursor X position.
    pub fn update(&mut self, current_x: f32) -> Option<Live<f32>> {
        let dx = current_x - self.start_x;
        let raw_width = self.start_width + dx;
        let clamped = NavigationRailModel::<()>::clamp_expanded_width(raw_width);
        self.current = clamped;
        self.slot.live(clamped)
    }

    /// Commit the settled width, consuming the linear gesture guard.
    pub fn commit(&mut self) -> Option<Commit<f32>> {
        self.slot.commit(self.current)
    }

    /// Cancel the drag, reverting to the pre-drag snapshot.
    pub fn cancel(&mut self) -> Option<Cancel<f32>> {
        let outcome = self.slot.cancel()?;
        self.current = *outcome.restored();
        Some(outcome)
    }
}

// ---------------------------------------------------------------------------
// Widget State
// ---------------------------------------------------------------------------

/// Internal interaction state for [`NavigationRail`] (`UIF-COMP-009`).
#[derive(Debug)]
pub struct NavigationRailState<DestId> {
    /// In-flight edge resize gesture.
    pub resize: NavRailResize,
    /// Whether pointer is hovering over the resize grab band.
    pub is_hovered_grab: bool,
    /// Whether pointer is hovering over the toggle button.
    pub is_hovered_toggle: bool,
    /// Currently hovered destination item identifier, if any.
    pub hovered_dest: Option<DestId>,
    /// Currently keyboard-focused destination item identifier, if any.
    pub focused_dest: Option<DestId>,
    /// Single tab-stop focusable state.
    pub focusable: FocusableState,
    /// Tooltip hover controller for compact mode.
    pub hover_controller: HoverController,
    /// Shared warm session for adjacent tooltip handoffs (`time.delay.tooltip_warm`).
    pub warm_session: WarmSession,
    /// Scroll offset for middle section.
    pub scroll_offset: f32,
}

impl<DestId> Default for NavigationRailState<DestId> {
    fn default() -> Self {
        Self {
            resize: NavRailResize::new(),
            is_hovered_grab: false,
            is_hovered_toggle: false,
            hovered_dest: None,
            focused_dest: None,
            focusable: FocusableState::new(),
            hover_controller: HoverController::new(),
            warm_session: WarmSession::default(),
            scroll_offset: 0.0,
        }
    }
}

impl<DestId> NavigationRailState<DestId> {
    /// Construct a new idle state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this widget currently holds keyboard focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focusable.is_focused()
    }
}

// ---------------------------------------------------------------------------
// NavigationRail Widget
// ---------------------------------------------------------------------------

/// The live NavigationRail / Collapsible NavigationView widget (`UIF-COMP-009`, `UIF-PRIN-005`, `UIF-PRIN-006`).
pub struct NavigationRail<'a, Message, DestId, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: NavigationRailModel<DestId>,
    on_select: Option<Box<dyn Fn(Commit<DestId>) -> Message + 'a>>,
    on_expand_toggle: Option<Box<dyn Fn(Commit<bool>) -> Message + 'a>>,
    on_resize_change: Option<Box<dyn Fn(Live<f32>) -> Message + 'a>>,
    on_resize_commit: Option<Box<dyn Fn(Commit<f32>) -> Message + 'a>>,
    on_resize_cancel: Option<Box<dyn Fn(Cancel<f32>) -> Message + 'a>>,
    palette: Option<Palette>,
    _marker: std::marker::PhantomData<(&'a (), Theme, Renderer)>,
}

impl<'a, Message, DestId: 'a + Clone + PartialEq, Theme, Renderer>
    NavigationRail<'a, Message, DestId, Theme, Renderer>
{
    /// Construct a new [`NavigationRail`] widget around `model`.
    #[must_use]
    pub fn new(model: NavigationRailModel<DestId>) -> Self {
        Self {
            model,
            on_select: None,
            on_expand_toggle: None,
            on_resize_change: None,
            on_resize_commit: None,
            on_resize_cancel: None,
            palette: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Attach destination selection callback.
    #[must_use]
    pub fn on_select(mut self, handler: impl Fn(Commit<DestId>) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Attach compact/expanded mode toggle callback.
    #[must_use]
    pub fn on_expand_toggle(mut self, handler: impl Fn(Commit<bool>) -> Message + 'a) -> Self {
        self.on_expand_toggle = Some(Box::new(handler));
        self
    }

    /// Attach live edge resize movement callback.
    #[must_use]
    pub fn on_resize_change(mut self, handler: impl Fn(Live<f32>) -> Message + 'a) -> Self {
        self.on_resize_change = Some(Box::new(handler));
        self
    }

    /// Attach edge resize commit callback.
    #[must_use]
    pub fn on_resize_commit(mut self, handler: impl Fn(Commit<f32>) -> Message + 'a) -> Self {
        self.on_resize_commit = Some(Box::new(handler));
        self
    }

    /// Attach edge resize cancel callback.
    #[must_use]
    pub fn on_resize_cancel(mut self, handler: impl Fn(Cancel<f32>) -> Message + 'a) -> Self {
        self.on_resize_cancel = Some(Box::new(handler));
        self
    }

    /// Set an explicit palette override.
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Access the underlying model.
    #[must_use]
    pub fn model(&self) -> &NavigationRailModel<DestId> {
        &self.model
    }

    /// Current layout width.
    #[must_use]
    pub fn current_width(&self) -> f32 {
        self.model.current_width()
    }

    /// Whether the rail is currently in expanded mode.
    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.model.expanded
    }

    /// Compute toggle button rect given widget layout bounds.
    #[must_use]
    pub fn toggle_rect(&self, bounds: Rectangle) -> Rectangle {
        let toggle_x = bounds.x + (COMPACT_WIDTH - TOGGLE_SIZE) / 2.0;
        let toggle_y = bounds.y + (ROW_HEIGHT - TOGGLE_SIZE) / 2.0;
        Rectangle {
            x: toggle_x,
            y: toggle_y,
            width: TOGGLE_SIZE,
            height: TOGGLE_SIZE,
        }
    }

    /// Compute destination hit test and destination reference if cursor is over a cell.
    #[must_use]
    pub fn hit_destination(
        &self,
        bounds: Rectangle,
        pos: Point,
        scroll_offset: f32,
    ) -> Option<(usize, &NavDestination<DestId>)> {
        let bottom_count = self.model.bottom_items.len();
        let bottom_height = bottom_count as f32 * ROW_HEIGHT;
        let bottom_y = bounds.y + bounds.height - bottom_height;
        let divider_y = bottom_y - DIVIDER_STROKE;

        // 1. Check bottom pinned items
        if pos.y >= bottom_y && pos.y < bounds.y + bounds.height {
            let idx = ((pos.y - bottom_y) / ROW_HEIGHT).floor() as usize;
            if idx < bottom_count {
                return Some((
                    idx + self.model.top_items.len(),
                    &self.model.bottom_items[idx],
                ));
            }
        }

        // 2. Check middle scrollable items
        let middle_top = bounds.y + ROW_HEIGHT;
        let middle_bottom = divider_y;
        if pos.y >= middle_top && pos.y < middle_bottom {
            let effective_y = pos.y - middle_top + scroll_offset;
            if effective_y >= 0.0 {
                let idx = (effective_y / ROW_HEIGHT).floor() as usize;
                if idx < self.model.top_items.len() {
                    return Some((idx, &self.model.top_items[idx]));
                }
            }
        }

        None
    }
}

/// Fallback text measurement helper.
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
        line_height: LineHeight::default(),
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

impl<Message, Theme, Renderer, DestId: 'static + Clone + PartialEq> Widget<Message, Theme, Renderer>
    for NavigationRail<'_, Message, DestId, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<NavigationRailState<DestId>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(NavigationRailState::<DestId>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.model.current_width()), Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let width = self.model.current_width();
        let height = limits.max().height;
        layout::Node::new(Size::new(width, height))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<NavigationRailState<DestId>>();
        operation.focusable(None, layout.bounds(), &mut state.focusable);
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let current_w = self.model.current_width();
        let grab_rect = Rectangle {
            x: bounds.x + current_w - GRAB_WIDTH / 2.0,
            y: bounds.y,
            width: GRAB_WIDTH,
            height: bounds.height,
        };
        let toggle_rect = self.toggle_rect(bounds);
        let now = TimeMs::now();

        let state = tree.state.downcast_mut::<NavigationRailState<DestId>>();

        // Middle scrolling bounds computation
        let bottom_height = self.model.bottom_items.len() as f32 * ROW_HEIGHT;
        let divider_y = bounds.y + bounds.height - bottom_height - DIVIDER_STROKE;
        let middle_top = bounds.y + ROW_HEIGHT;
        let middle_height = (divider_y - middle_top).max(0.0);
        let top_content_height = self.model.top_items.len() as f32 * ROW_HEIGHT;
        let max_scroll = (top_content_height - middle_height).max(0.0);

        // Hover tracking
        if !state.resize.is_active() {
            state.is_hovered_grab = self.model.expanded && cursor.is_over(grab_rect);
            state.is_hovered_toggle = cursor.is_over(toggle_rect);

            if let Some(pos) = cursor.position() {
                if let Some((_, dest)) = self.hit_destination(bounds, pos, state.scroll_offset) {
                    if state.hovered_dest.as_ref() != Some(&dest.id) {
                        state.hovered_dest = Some(dest.id.clone());
                        let warmth = state.warm_session.warmth_for(now);
                        state.hover_controller.pointer_enter_warm(now, warmth);
                    }
                } else if state.hovered_dest.is_some() {
                    state.hovered_dest = None;
                    state.hover_controller.pointer_exit();
                }
            } else if state.hovered_dest.is_some() {
                state.hovered_dest = None;
                state.hover_controller.pointer_exit();
            }

            if !self.model.expanded
                && state.hovered_dest.is_some()
                && state.hover_controller.tick(now).is_visible()
            {
                state.warm_session.note_visible(now);
            }
        }

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if self.model.expanded && cursor.is_over(grab_rect) {
                    if let Some(pos) = cursor.position() {
                        let _ = state.resize.begin(self.model.expanded_width, pos.x);
                        shell.capture_event();
                    }
                } else if cursor.is_over(toggle_rect) {
                    if let Some(on_toggle) = &self.on_expand_toggle {
                        shell.publish((on_toggle)(Commit::new(!self.model.expanded)));
                        shell.capture_event();
                    }
                } else if let Some(pos) = cursor.position()
                    && let Some((_, dest)) = self.hit_destination(bounds, pos, state.scroll_offset)
                    && !dest.disabled
                {
                    operation::Focusable::focus(&mut state.focusable);
                    state.focused_dest = Some(dest.id.clone());
                    if let Some(on_select) = &self.on_select {
                        shell.publish((on_select)(Commit::new(dest.id.clone())));
                    }
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.resize.is_active() {
                    if let Some(live) = state.resize.update(position.x)
                        && let Some(on_resize) = &self.on_resize_change
                    {
                        shell.publish((on_resize)(live));
                    }
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.resize.is_active() {
                    if let Some(commit) = state.resize.commit()
                        && let Some(on_commit) = &self.on_resize_commit
                    {
                        shell.publish((on_commit)(commit));
                    }
                    state.is_hovered_grab = self.model.expanded && cursor.is_over(grab_rect);
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if let Some(pos) = cursor.position()
                    && pos.y >= middle_top
                    && pos.y < divider_y
                {
                    let delta_y = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y * 20.0,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    state.scroll_offset = (state.scroll_offset - delta_y).clamp(0.0, max_scroll);
                    shell.capture_event();
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                // 1. Escape cancels in-flight resize
                if *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    if state.resize.is_active() {
                        if let Some(cancel) = state.resize.cancel()
                            && let Some(on_cancel) = &self.on_resize_cancel
                        {
                            shell.publish((on_cancel)(cancel));
                        }
                        state.is_hovered_grab = self.model.expanded && cursor.is_over(grab_rect);
                        shell.capture_event();
                    } else {
                        let _ = state.hover_controller.dismiss(DismissReason::Escape);
                    }
                } else if state.focusable.is_focused() {
                    // 2. Keyboard roving focus if rail is focused
                    let range_key = match key {
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                            Some(RangeKey::Increase)
                        }
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                            Some(RangeKey::Decrease)
                        }
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                            Some(RangeKey::Home)
                        }
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::End) => {
                            Some(RangeKey::End)
                        }
                        _ => None,
                    };

                    if let Some(rk) = range_key {
                        if let Some(next_id) = self.model.rove(rk) {
                            state.focused_dest = Some(next_id);
                            shell.capture_event();
                        }
                    } else if matches!(
                        key,
                        iced::keyboard::Key::Named(
                            iced::keyboard::key::Named::Enter | iced::keyboard::key::Named::Space
                        )
                    ) {
                        let target = state
                            .focused_dest
                            .clone()
                            .unwrap_or_else(|| self.model.selected.clone());
                        if let Some(on_select) = &self.on_select {
                            shell.publish((on_select)(Commit::new(target)));
                        }
                        shell.capture_event();
                    }
                }
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
        let state = tree.state.downcast_ref::<NavigationRailState<DestId>>();
        let bounds = layout.bounds();

        if self.model.expanded {
            let grab_x = bounds.x + bounds.width - GRAB_WIDTH / 2.0;
            let grab_rect = Rectangle {
                x: grab_x,
                y: bounds.y,
                width: GRAB_WIDTH,
                height: bounds.height,
            };
            if state.resize.is_active() || cursor.is_over(grab_rect) {
                return mouse::Interaction::ResizingHorizontally;
            }
        }

        let toggle_rect = self.toggle_rect(bounds);
        if cursor.is_over(toggle_rect) {
            return mouse::Interaction::Pointer;
        }

        if let Some(pos) = cursor.position()
            && let Some((_, dest)) = self.hit_destination(bounds, pos, state.scroll_offset)
            && !dest.disabled
        {
            return mouse::Interaction::Pointer;
        }

        mouse::Interaction::Idle
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let palette = self.palette.unwrap_or_else(Palette::dark);
        let width = bounds.width;
        let now = TimeMs::now();

        let state = tree.state.downcast_ref::<NavigationRailState<DestId>>();

        // 1. Background fill: palette.surface_raised
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.surface_raised),
        );

        // 2. Right border hairline: 1px border.subtle (promoted to accent.border on resize or hover per UIF-PRIN-006)
        let border_color = if state.resize.is_active() || state.is_hovered_grab {
            palette.accent_border
        } else {
            palette.border_subtle
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + width - DIVIDER_STROKE,
                    y: bounds.y,
                    width: DIVIDER_STROKE,
                    height: bounds.height,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(border_color),
        );

        // 3. Header toggle button: 28x28px inside 36px row
        let toggle_rect = self.toggle_rect(bounds);
        if state.is_hovered_toggle || cursor.is_over(toggle_rect) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: toggle_rect,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.surface_hover),
            );
        }
        renderer.fill_text(
            text_core::Text {
                content: GLYPH_TOGGLE.to_string(),
                bounds: toggle_rect.size(),
                size: Pixels(ICON_SIZE),
                line_height: LineHeight::Absolute(Pixels(TOGGLE_SIZE)),
                font: typography::CAPTION
                    .font(typography::FAMILY_SANS, typography::Weight::Regular),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            toggle_rect.position(),
            palette.text_primary,
            *viewport,
        );

        // Bottom items calculation
        let bottom_count = self.model.bottom_items.len();
        let bottom_height = bottom_count as f32 * ROW_HEIGHT;
        let bottom_y = bounds.y + bounds.height - bottom_height;
        let divider_y = bottom_y - DIVIDER_STROKE;

        // Middle clipping area
        let middle_top = bounds.y + ROW_HEIGHT;
        let middle_height = (divider_y - middle_top).max(0.0);
        let middle_rect = Rectangle {
            x: bounds.x,
            y: middle_top,
            width,
            height: middle_height,
        };

        // Helper closure to render one destination row
        let render_item = |renderer: &mut Renderer,
                           item: &NavDestination<DestId>,
                           item_rect: Rectangle,
                           clip_rect: Rectangle| {
            let is_selected = item.id == self.model.selected;
            let is_hovered = state.hovered_dest.as_ref() == Some(&item.id);
            let is_focused = state.focusable.is_focused()
                && (state.focused_dest.as_ref() == Some(&item.id)
                    || (state.focused_dest.is_none() && is_selected));

            // Background
            if is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: item_rect,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    iced::Background::Color(palette.surface_active),
                );
            } else if is_hovered && !item.disabled {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: item_rect,
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    iced::Background::Color(palette.surface_hover),
                );
            }

            // Focus ring (UIF-PRIN-006)
            if is_focused {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: item_rect,
                        border: iced::Border {
                            color: palette.accent_border,
                            width: shape::BORDER_FOCUS,
                            radius: shape::RADIUS_WIDGET.into(),
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    iced::Background::Color(iced::Color::TRANSPARENT),
                );
            }

            // Leading indicator bar (2px accent.border on left edge for selected item)
            if is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: item_rect.x,
                            y: item_rect.y,
                            width: INDICATOR_WIDTH,
                            height: ROW_HEIGHT,
                        },
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    iced::Background::Color(palette.accent_border),
                );
            }

            // Foreground text/icon color
            let fg_color = if item.disabled {
                palette.text_disabled
            } else if is_selected {
                palette.text_strong
            } else {
                palette.text_primary
            };

            if self.model.expanded {
                // Expanded layout: 16px icon, 13px label, optional trailing badge
                let icon_x = item_rect.x + space::MD;
                let icon_y = item_rect.y + (ROW_HEIGHT - ICON_SIZE) / 2.0;
                renderer.fill_text(
                    text_core::Text {
                        content: item.icon.0.clone(),
                        bounds: Size::new(ICON_SIZE, ICON_SIZE),
                        size: Pixels(ICON_SIZE),
                        line_height: LineHeight::Absolute(Pixels(ICON_SIZE)),
                        font: typography::CAPTION
                            .font(typography::FAMILY_SANS, typography::Weight::Regular),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    Point::new(icon_x, icon_y),
                    fg_color,
                    clip_rect,
                );

                // Label (13px Medium UI_COMPACT)
                let label_x = icon_x + ICON_SIZE + space::SM;
                let label_w = (item_rect.width - (label_x - item_rect.x) - space::MD).max(0.0);
                renderer.fill_text(
                    text_core::Text {
                        content: item.label.clone(),
                        bounds: Size::new(label_w, ROW_HEIGHT),
                        size: Pixels(typography::UI_COMPACT.size),
                        line_height: LineHeight::Absolute(Pixels(ROW_HEIGHT)),
                        font: typography::UI_COMPACT
                            .font(typography::FAMILY_SANS, typography::Weight::Medium),
                        align_x: text_core::Alignment::Left,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    Point::new(label_x, item_rect.y),
                    fg_color,
                    clip_rect,
                );

                // Trailing Badge
                if let Some(badge) = &item.badge
                    && !badge.is_empty()
                {
                    let b_style = resolve_badge_style(badge, &palette);
                    let badge_text = badge.text().unwrap_or_default();
                    let font = typography::CAPTION
                        .font(typography::FAMILY_SANS, typography::Weight::Medium);
                    let text_w =
                        measure_text_width(renderer, &badge_text, typography::CAPTION.size, font);
                    let pill_w = (text_w + space::SM * 2.0).max(b_style.height);
                    let badge_x = item_rect.x + item_rect.width - space::MD - pill_w;
                    let badge_y = item_rect.y + (ROW_HEIGHT - b_style.height) / 2.0;

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: badge_x,
                                y: badge_y,
                                width: pill_w,
                                height: b_style.height,
                            },
                            border: iced::Border {
                                color: b_style.border,
                                width: shape::BORDER_HAIRLINE,
                                radius: b_style.radius.into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: true,
                        },
                        iced::Background::Color(b_style.fill),
                    );

                    renderer.fill_text(
                        text_core::Text {
                            content: badge_text,
                            bounds: Size::new(pill_w, b_style.height),
                            size: Pixels(typography::CAPTION.size),
                            line_height: LineHeight::Absolute(Pixels(b_style.height)),
                            font,
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        Point::new(badge_x, badge_y),
                        b_style.text,
                        clip_rect,
                    );
                }
            } else {
                // Compact layout: Centered 16px icon inside 48px cell, optional count/dot badge
                let icon_x = item_rect.x + (COMPACT_WIDTH - ICON_SIZE) / 2.0;
                let icon_y = item_rect.y + (ROW_HEIGHT - ICON_SIZE) / 2.0;
                renderer.fill_text(
                    text_core::Text {
                        content: item.icon.0.clone(),
                        bounds: Size::new(ICON_SIZE, ICON_SIZE),
                        size: Pixels(ICON_SIZE),
                        line_height: LineHeight::Absolute(Pixels(ICON_SIZE)),
                        font: typography::CAPTION
                            .font(typography::FAMILY_SANS, typography::Weight::Regular),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    Point::new(icon_x, icon_y),
                    fg_color,
                    clip_rect,
                );

                // Compact Badge dot / count marker
                if let Some(badge) = &item.badge
                    && !badge.is_empty()
                {
                    let b_style = resolve_badge_style(badge, &palette);
                    let dot_size = size::INDICATOR_DOT;
                    let dot_x = item_rect.x + COMPACT_WIDTH - dot_size - space::XS;
                    let dot_y = item_rect.y + space::XS;
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: dot_x,
                                y: dot_y,
                                width: dot_size,
                                height: dot_size,
                            },
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            snap: true,
                        },
                        iced::Background::Color(b_style.fill),
                    );
                }
            }
        };

        // 4. Render middle items
        for (i, item) in self.model.top_items.iter().enumerate() {
            let item_y = middle_top + i as f32 * ROW_HEIGHT - state.scroll_offset;
            if item_y + ROW_HEIGHT >= middle_top && item_y <= divider_y {
                let item_rect = Rectangle {
                    x: bounds.x,
                    y: item_y,
                    width,
                    height: ROW_HEIGHT,
                };
                render_item(renderer, item, item_rect, middle_rect);
            }
        }

        // 5. Divider above pinned bottom items
        if bottom_count > 0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y: divider_y,
                        width,
                        height: DIVIDER_STROKE,
                    },
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.border_subtle),
            );

            // 6. Render bottom items
            for (i, item) in self.model.bottom_items.iter().enumerate() {
                let item_y = bottom_y + i as f32 * ROW_HEIGHT;
                let item_rect = Rectangle {
                    x: bounds.x,
                    y: item_y,
                    width,
                    height: ROW_HEIGHT,
                };
                render_item(renderer, item, item_rect, *viewport);
            }
        }

        // 7. Compact Hover Tooltip (mandatory right-aligned tooltip on time.delay.tooltip_warm handoff)
        if !self.model.expanded
            && let Some(hovered_id) = &state.hovered_dest
            && state.hover_controller.tick(now).is_visible()
            && let Some(item) = self.model.all_items().find(|item| &item.id == hovered_id)
        {
            let font =
                typography::CAPTION.font(typography::FAMILY_SANS, typography::Weight::Regular);
            let text_w = measure_text_width(renderer, &item.label, typography::CAPTION.size, font);
            let card_w = text_w + space::SM * 2.0;
            let card_h = typography::CAPTION.line_height + space::XS * 2.0;

            // Anchor to hovered destination cell
            let cell_y = if let Some(idx) = self
                .model
                .top_items
                .iter()
                .position(|i| &i.id == hovered_id)
            {
                middle_top + idx as f32 * ROW_HEIGHT - state.scroll_offset
            } else if let Some(idx) = self
                .model
                .bottom_items
                .iter()
                .position(|i| &i.id == hovered_id)
            {
                bottom_y + idx as f32 * ROW_HEIGHT
            } else {
                middle_top
            };

            let card_x = bounds.x + COMPACT_WIDTH + space::XS;
            let card_y = cell_y + (ROW_HEIGHT - card_h) / 2.0;

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: card_x,
                        y: card_y,
                        width: card_w,
                        height: card_h,
                    },
                    border: iced::Border {
                        color: palette.border_strong,
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.surface_overlay),
            );

            renderer.fill_text(
                text_core::Text {
                    content: item.label.clone(),
                    bounds: Size::new(text_w, card_h),
                    size: Pixels(typography::CAPTION.size),
                    line_height: LineHeight::Absolute(Pixels(card_h)),
                    font,
                    align_x: text_core::Alignment::Left,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                Point::new(card_x + space::SM, card_y),
                palette.text_primary,
                *viewport,
            );
        }
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer: 'a, DestId: 'static + Clone + PartialEq>
    From<NavigationRail<'a, Message, DestId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(rail: NavigationRail<'a, Message, DestId, Theme, Renderer>) -> Self {
        Element::new(rail)
    }
}

/// Helper to construct a [`NavigationRail`] widget from a declarative model (`UIF-COMP-009`).
#[must_use]
pub fn navigation_rail<'a, Message, DestId: 'a + Clone + PartialEq>(
    model: NavigationRailModel<DestId>,
) -> NavigationRail<'a, Message, DestId> {
    NavigationRail::new(model)
}

#[cfg(test)]
#[path = "navigation_rail_tests.rs"]
mod navigation_rail_tests;
