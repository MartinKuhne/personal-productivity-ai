//! The SideSheet / Docked Inspector Drawer composite widget (`UIF-COMP-012`, `UIF-PRIN-006`, `UIF-MOD-002`).
//!
//! §3B.12 specifies a contextual right-hand (or left-hand) inspector drawer (`DEFAULT_WIDTH = 360px`,
//! resizable across `[280px, 520px]` via a `10px` `size.splitter.grab` edge divider) with a `36px`
//! (`size.control.lg`) header (`16px SemiBold` `type.title_sm` title, `24×24px` `[Pin/Unpin]`, and
//! `24×24px` `[Close ×]`), scrollable `16px`-padded body, and optional `48px` action footer.
//!
//! The drawer operates in three distinct z-band and escape-unwinding modes:
//! - [`SheetMode::Docked`]: Z-Band 0 (Workspace), Escape Rank 0. Shares horizontal layout with the
//!   center workspace without covering content.
//! - [`SheetMode::ModelessOverlay`]: Z-Band 1 (UtilityOverlay), Escape Rank 1 when focused. Floats
//!   over the workspace edge without a scrim.
//! - [`SheetMode::ModalScrim`]: Z-Band 3 (Modal), Escape Rank 2. Slides over `surface.scrim` when the
//!   viewport width drops below `800px` ([`RESPONSIVE_BREAKPOINT`]) or when invoked as a blocking detail
//!   sheet, honoring unsaved-edit and busy protection per `UIF-MOD-002`.
//!
//! The edge divider line is drawn full-height in `1px border.subtle`, with a centered `2px × 24px`
//! (`space.2xs × size.hit_target.min`, `radius.pill`) grip pill in `border.strong` (`4.51:1` on `surface.base`)
//! at rest. On hover or an active resize drag, both the divider line and the grip pill are promoted
//! to `accent.border` (`UIF-PRIN-006`).
//!
//! Unit tests live in the sibling `side_sheet_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart, Live};
use crate::surface::zorder::ZBand;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight};
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::alignment;
use iced::mouse;
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Size, Vector};

// ---------------------------------------------------------------------------
// String Constants
// ---------------------------------------------------------------------------

/// Default title for an inspector drawer (`UIF-COMP-012`).
pub const DEFAULT_TITLE: &str = "Inspector";

/// Glyph for the pinned mode button — `"📌"` (§3B.12).
pub const GLYPH_PINNED: &str = "📌";

/// Glyph for the unpinned mode button — `"📍"` (§3B.12).
pub const GLYPH_UNPINNED: &str = "📍";

/// Glyph for the close button — `"×"` (§3B.12).
pub const GLYPH_CLOSE: &str = "×";

/// Label for the pin button — `"Pin"` (§3B.12).
pub const LABEL_PIN: &str = "Pin";

/// Label for the unpin button — `"Unpin"` (§3B.12).
pub const LABEL_UNPIN: &str = "Unpin";

/// Label for the close button — `"Close"` (§3B.12).
pub const LABEL_CLOSE: &str = "Close";

// ---------------------------------------------------------------------------
// Surface Geometry Constants & Tokens (§3B.12)
// ---------------------------------------------------------------------------

/// Default width of the side sheet drawer — 360px (§3B.12, `UIF-COMP-012`).
pub const DEFAULT_WIDTH: f32 = 360.0;

/// Minimum width of the side sheet drawer — 280px (§3B.12, `UIF-COMP-012`).
pub const MIN_WIDTH: f32 = 280.0;

/// Maximum width of the side sheet drawer — 520px (§3B.12, `UIF-COMP-012`).
pub const MAX_WIDTH: f32 = 520.0;

/// Height of the side sheet header — `size::CONTROL_LG` (36px, §3B.12, `UIF-COMP-012`).
pub const HEADER_HEIGHT: f32 = size::CONTROL_LG;

/// Height of the optional action footer — 48px (§3B.12, `UIF-COMP-012`).
pub const FOOTER_HEIGHT: f32 = 48.0;

/// Pointer grab band for resizing the drawer — `size::SPLITTER_GRAB` (10px, §3B.12, `UIF-COMP-012`).
pub const GRAB_WIDTH: f32 = size::SPLITTER_GRAB;

/// Width of the centered grip pill — `space::XXS` (2px, §3B.12, `UIF-PRIN-006`).
pub const GRIP_PILL_WIDTH: f32 = space::XXS;

/// Height of the centered grip pill — `size::HIT_TARGET_MIN` (24px, §3B.12, `UIF-PRIN-006`).
pub const GRIP_PILL_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// Responsive breakpoint below which the drawer promotes to ModalScrim — 800px (§3B.12, `UIF-COMP-012`).
pub const RESPONSIVE_BREAKPOINT: f32 = 800.0;

/// Corner radius of the grip pill — `shape::RADIUS_PILL` (999px, §3B.12, `UIF-PRIN-006`).
pub const GRIP_PILL_RADIUS: f32 = shape::RADIUS_PILL;

/// Stroke width of the edge divider line — `shape::BORDER_HAIRLINE` (1px, §3B.12, `UIF-PRIN-006`).
pub const DIVIDER_STROKE: f32 = shape::BORDER_HAIRLINE;

/// Size of the header action buttons ([Pin/Unpin] and [Close ×]) — `size::HIT_TARGET_MIN` (24px, §3B.12, `UIF-COMP-012`).
pub const HEADER_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Padding around the scrollable body content — `space::LG` (16px, §3B.12, `UIF-COMP-012`).
pub const BODY_PADDING: f32 = space::LG;

// ---------------------------------------------------------------------------
// Data Model Enums and Structs
// ---------------------------------------------------------------------------

/// The display and layering mode of a [`SideSheet`] (§3B.12, `UIF-COMP-012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SheetMode {
    /// Band 0 Workspace docked panel sharing horizontal layout without covering content.
    #[default]
    Docked,
    /// Band 1 Modeless overlay floating over workspace edge without a scrim.
    ModelessOverlay,
    /// Band 3 Modal sheet sliding over dimmed scrim, trapping focus.
    ModalScrim,
}

impl SheetMode {
    /// Toggle between Docked (pinned) and ModelessOverlay (unpinned).
    /// If ModalScrim, toggles to Docked.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Docked => Self::ModelessOverlay,
            Self::ModelessOverlay => Self::Docked,
            Self::ModalScrim => Self::Docked,
        }
    }
}

/// The placement side of a [`SideSheet`] (§3B.12, `UIF-COMP-012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SheetPlacement {
    /// Drawer is anchored to the left edge of the workspace.
    Left,
    /// Drawer is anchored to the right edge of the workspace.
    #[default]
    Right,
}

/// Controlled data model for [`SideSheet`] (§3B.12, `UIF-COMP-012`, `UIF-MOD-002`).
#[derive(Debug, Clone, PartialEq)]
pub struct SideSheetModel {
    /// Header title string.
    pub title: String,
    /// Drawer display mode.
    pub mode: SheetMode,
    /// Drawer placement side (Left or Right).
    pub placement: SheetPlacement,
    /// Drawer width in logical pixels.
    pub width: f32,
    /// Whether the drawer is currently open.
    pub open: bool,
    /// Whether the drawer holds unsaved edits (`UIF-MOD-002`).
    pub dirty: bool,
    /// Whether the drawer is actively processing a background task (`UIF-MOD-002`).
    pub busy: bool,
}

impl Default for SideSheetModel {
    fn default() -> Self {
        Self::new(DEFAULT_TITLE)
    }
}

impl SideSheetModel {
    /// Construct a new model with default geometry and state.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            mode: SheetMode::Docked,
            placement: SheetPlacement::Right,
            width: DEFAULT_WIDTH,
            open: true,
            dirty: false,
            busy: false,
        }
    }

    /// Set the sheet display mode.
    #[must_use]
    pub fn with_mode(mut self, mode: SheetMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the sheet placement (Left or Right).
    #[must_use]
    pub fn with_placement(mut self, placement: SheetPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Set the drawer width, clamped to `[MIN_WIDTH, MAX_WIDTH]`.
    #[must_use]
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = Self::clamp_width(width);
        self
    }

    /// Set whether the sheet is open.
    #[must_use]
    pub fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set whether the sheet has unsaved edits (`UIF-MOD-002`).
    #[must_use]
    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Set whether the sheet is busy processing a background task (`UIF-MOD-002`).
    #[must_use]
    pub fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Clamp a requested drawer width to the specification range `[MIN_WIDTH, MAX_WIDTH]` (§3B.12).
    #[must_use]
    pub fn clamp_width(w: f32) -> f32 {
        w.clamp(MIN_WIDTH, MAX_WIDTH)
    }

    /// Resolve the effective display mode taking the responsive viewport breakpoint into account (§3B.12).
    ///
    /// If `viewport_width < RESPONSIVE_BREAKPOINT` (800px), promotes automatically to [`SheetMode::ModalScrim`].
    #[must_use]
    pub fn effective_mode(&self, viewport_width: f32) -> SheetMode {
        if viewport_width < RESPONSIVE_BREAKPOINT {
            SheetMode::ModalScrim
        } else {
            self.mode
        }
    }

    /// Whether the sheet can be dismissed immediately without prompt or refusal (`UIF-MOD-002`).
    ///
    /// Returns false if `busy` is active, or if `dirty` unsaved edits exist (requiring confirmation).
    #[must_use]
    pub fn can_close(&self) -> bool {
        !self.busy && !self.dirty
    }

    /// The Z-Band where this drawer registers in [`crate::surface::zorder::SurfaceRegistry`] (§3B.12).
    #[must_use]
    pub fn z_band(&self, viewport_width: f32) -> ZBand {
        match self.effective_mode(viewport_width) {
            SheetMode::Docked => ZBand::Workspace,
            SheetMode::ModelessOverlay => ZBand::UtilityOverlay,
            SheetMode::ModalScrim => ZBand::Modal,
        }
    }

    /// The Escape unwinding priority rank (`UIF-WIN-003`, §3B.12).
    ///
    /// - Docked (Z-Band 0) -> Rank 0
    /// - ModelessOverlay (Z-Band 1) -> Rank 1
    /// - ModalScrim (Z-Band 3) -> Rank 2
    #[must_use]
    pub fn escape_rank(&self, viewport_width: f32) -> u8 {
        match self.effective_mode(viewport_width) {
            SheetMode::Docked => 0,
            SheetMode::ModelessOverlay => 1,
            SheetMode::ModalScrim => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Resize Gesture Engine
// ---------------------------------------------------------------------------

/// The in-flight resize gesture for [`SideSheet`] (`UIF-COMP-012`, `UIF-PRIN-006`).
///
/// Uses [`GestureSlot<f32>`] to track edge drag linearly. Captures the width
/// at press; live drag ticks emit [`Live<f32>`]; release emits [`Commit<f32>`];
/// Escape reverts to the pre-drag snapshot via [`Cancel<f32>`].
#[derive(Clone, Debug)]
pub struct SideSheetResize {
    slot: GestureSlot<f32>,
    /// Drawer placement side (governs drag direction).
    pub placement: SheetPlacement,
    start_width: f32,
    start_x: f32,
    current: f32,
}

impl Default for SideSheetResize {
    fn default() -> Self {
        Self::new(SheetPlacement::Right)
    }
}

impl SideSheetResize {
    /// Create a new idle resize gesture for `placement`.
    #[must_use]
    pub fn new(placement: SheetPlacement) -> Self {
        Self {
            slot: GestureSlot::new(),
            placement,
            start_width: DEFAULT_WIDTH,
            start_x: 0.0,
            current: DEFAULT_WIDTH,
        }
    }

    /// Whether an active drag gesture is currently open.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.slot.is_open()
    }

    /// The placement side governing this resize gesture.
    #[must_use]
    pub fn placement(&self) -> SheetPlacement {
        self.placement
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
        let clamped = SideSheetModel::clamp_width(start_width);
        let start = self.slot.begin(&clamped);
        if start == GestureStart::Started {
            self.start_width = clamped;
            self.start_x = start_x;
            self.current = clamped;
        }
        start
    }

    /// Update gesture with new cursor X position, computing delta according to placement.
    pub fn update(&mut self, current_x: f32) -> Option<Live<f32>> {
        let dx = current_x - self.start_x;
        let raw_width = match self.placement {
            SheetPlacement::Right => self.start_width - dx,
            SheetPlacement::Left => self.start_width + dx,
        };
        let clamped = SideSheetModel::clamp_width(raw_width);
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

/// Internal interaction state for [`SideSheet`] (`UIF-COMP-012`).
#[derive(Debug, Default)]
pub struct SideSheetState {
    /// In-flight edge resize gesture.
    pub resize: SideSheetResize,
    /// Whether the pointer is hovering over the resize grab band.
    pub is_hovered_grab: bool,
    /// Whether the pointer is hovering over the [Pin/Unpin] button.
    pub is_hovered_pin: bool,
    /// Whether the pointer is hovering over the [Close ×] button.
    pub is_hovered_close: bool,
}

impl SideSheetState {
    /// Construct a new idle interaction state for `placement`.
    #[must_use]
    pub fn new(placement: SheetPlacement) -> Self {
        Self {
            resize: SideSheetResize::new(placement),
            is_hovered_grab: false,
            is_hovered_pin: false,
            is_hovered_close: false,
        }
    }
}

// ---------------------------------------------------------------------------
// SideSheet Widget
// ---------------------------------------------------------------------------

/// The live SideSheet / Docked Inspector Drawer widget (`UIF-COMP-012`, `UIF-PRIN-006`, `UIF-MOD-002`).
pub struct SideSheet<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: SideSheetModel,
    content: Element<'a, Message, Theme, Renderer>,
    footer: Option<Element<'a, Message, Theme, Renderer>>,
    on_resize_change: Option<Box<dyn Fn(Live<f32>) -> Message + 'a>>,
    on_resize_commit: Option<Box<dyn Fn(Commit<f32>) -> Message + 'a>>,
    on_resize_cancel: Option<Box<dyn Fn(Cancel<f32>) -> Message + 'a>>,
    on_mode_toggle: Option<Box<dyn Fn(Commit<SheetMode>) -> Message + 'a>>,
    on_close_request: Option<Box<dyn Fn(Cancel<()>) -> Message + 'a>>,
}

impl<'a, Message, Theme, Renderer> SideSheet<'a, Message, Theme, Renderer> {
    /// Construct a new [`SideSheet`] widget around `model` and `content`.
    #[must_use]
    pub fn new(
        model: SideSheetModel,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            model,
            content: content.into(),
            footer: None,
            on_resize_change: None,
            on_resize_commit: None,
            on_resize_cancel: None,
            on_mode_toggle: None,
            on_close_request: None,
        }
    }

    /// Attach an optional action footer to the bottom of the drawer.
    #[must_use]
    pub fn footer(mut self, footer: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// Set callback emitted on live edge resize motion (`UIF-COMP-012`).
    #[must_use]
    pub fn on_resize_change(mut self, f: impl Fn(Live<f32>) -> Message + 'a) -> Self {
        self.on_resize_change = Some(Box::new(f));
        self
    }

    /// Set callback emitted on resize drag release commitment (`UIF-COMP-012`).
    #[must_use]
    pub fn on_resize_commit(mut self, f: impl Fn(Commit<f32>) -> Message + 'a) -> Self {
        self.on_resize_commit = Some(Box::new(f));
        self
    }

    /// Set callback emitted when Escape cancels a resize drag (`UIF-COMP-012`).
    #[must_use]
    pub fn on_resize_cancel(mut self, f: impl Fn(Cancel<f32>) -> Message + 'a) -> Self {
        self.on_resize_cancel = Some(Box::new(f));
        self
    }

    /// Set callback emitted when the [Pin/Unpin] mode toggle button is clicked.
    #[must_use]
    pub fn on_mode_toggle(mut self, f: impl Fn(Commit<SheetMode>) -> Message + 'a) -> Self {
        self.on_mode_toggle = Some(Box::new(f));
        self
    }

    /// Set callback emitted when dismissal is requested via [Close ×] or Escape.
    #[must_use]
    pub fn on_close_request(mut self, f: impl Fn(Cancel<()>) -> Message + 'a) -> Self {
        self.on_close_request = Some(Box::new(f));
        self
    }

    /// Read the model driving this sheet.
    #[must_use]
    pub fn model(&self) -> &SideSheetModel {
        &self.model
    }

    /// The current resolved width of the sheet.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.model.width
    }

    /// Whether the sheet is currently open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.model.open
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SideSheet<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SideSheetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SideSheetState::new(self.model.placement))
    }

    fn children(&self) -> Vec<Tree> {
        if let Some(footer) = &self.footer {
            vec![Tree::new(&self.content), Tree::new(footer)]
        } else {
            vec![Tree::new(&self.content)]
        }
    }

    fn diff(&self, tree: &mut Tree) {
        if let Some(footer) = &self.footer {
            tree.diff_children(&[&self.content, footer]);
        } else {
            tree.diff_children(std::slice::from_ref(&self.content));
        }
    }

    fn size(&self) -> Size<Length> {
        if self.model.open {
            Size::new(Length::Fixed(self.model.width), Length::Fill)
        } else {
            Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        if !self.model.open {
            return layout::Node::new(Size::ZERO);
        }

        let clamped_w = SideSheetModel::clamp_width(self.model.width);
        let total_height = limits.max().height;

        let footer_h = if self.footer.is_some() {
            FOOTER_HEIGHT
        } else {
            0.0
        };
        let body_h = (total_height - HEADER_HEIGHT - footer_h).max(0.0);

        // Content layout inside padded body:
        let content_w = (clamped_w - BODY_PADDING * 2.0).max(0.0);
        let content_limits = layout::Limits::new(Size::ZERO, Size::new(content_w, body_h));
        let content_node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &content_limits)
            .move_to(Point::new(BODY_PADDING, HEADER_HEIGHT + BODY_PADDING));

        let mut children = vec![content_node];

        if let Some(footer) = &mut self.footer {
            let footer_w = (clamped_w - space::MD * 2.0).max(0.0);
            let footer_limits = layout::Limits::new(Size::ZERO, Size::new(footer_w, FOOTER_HEIGHT));
            let footer_node =
                footer
                    .as_widget_mut()
                    .layout(&mut tree.children[1], renderer, &footer_limits);
            let footer_y =
                total_height - FOOTER_HEIGHT + (FOOTER_HEIGHT - footer_node.size().height) / 2.0;
            let footer_node = footer_node.move_to(Point::new(space::MD, footer_y));
            children.push(footer_node);
        }

        layout::Node::with_children(Size::new(clamped_w, total_height), children)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if !self.model.open {
            return;
        }

        let bounds = layout.bounds();
        let half_grab = GRAB_WIDTH / 2.0;
        let grab_x = match self.model.placement {
            SheetPlacement::Right => bounds.x - half_grab,
            SheetPlacement::Left => bounds.x + bounds.width - half_grab,
        };
        let grab_rect = Rectangle {
            x: grab_x,
            y: bounds.y,
            width: GRAB_WIDTH,
            height: bounds.height,
        };

        let pin_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE * 2.0 - space::XS;
        let pin_y = bounds.y + (HEADER_HEIGHT - HEADER_BUTTON_SIZE) / 2.0;
        let pin_rect = Rectangle {
            x: pin_x,
            y: pin_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        let close_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE;
        let close_y = pin_y;
        let close_rect = Rectangle {
            x: close_x,
            y: close_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        let state = tree.state.downcast_mut::<SideSheetState>();
        state.resize.placement = self.model.placement;

        // Hover tracking
        if !state.resize.is_active() {
            state.is_hovered_grab = cursor.is_over(grab_rect);
            state.is_hovered_pin = cursor.is_over(pin_rect);
            state.is_hovered_close = cursor.is_over(close_rect);
        }

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(grab_rect) {
                    if let Some(pos) = cursor.position() {
                        let _ = state.resize.begin(self.model.width, pos.x);
                        shell.capture_event();
                        return;
                    }
                } else if cursor.is_over(pin_rect) {
                    if let Some(on_toggle) = &self.on_mode_toggle {
                        shell.publish((on_toggle)(Commit::new(self.model.mode.toggle())));
                        shell.capture_event();
                        return;
                    }
                } else if cursor.is_over(close_rect)
                    && !self.model.busy
                    && let Some(on_close) = &self.on_close_request
                {
                    shell.publish((on_close)(Cancel::new(())));
                    shell.capture_event();
                    return;
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.resize.is_active() {
                    if let Some(live) = state.resize.update(position.x)
                        && let Some(on_resize_change) = &self.on_resize_change
                    {
                        shell.publish((on_resize_change)(live));
                    }
                    shell.capture_event();
                    return;
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.resize.is_active() {
                    if let Some(commit) = state.resize.commit()
                        && let Some(on_resize_commit) = &self.on_resize_commit
                    {
                        shell.publish((on_resize_commit)(commit));
                    }
                    state.is_hovered_grab = cursor.is_over(grab_rect);
                    shell.capture_event();
                    return;
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                if *key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) =>
            {
                if state.resize.is_active() {
                    if let Some(cancel) = state.resize.cancel()
                        && let Some(on_resize_cancel) = &self.on_resize_cancel
                    {
                        shell.publish((on_resize_cancel)(cancel));
                    }
                    state.is_hovered_grab = cursor.is_over(grab_rect);
                    shell.capture_event();
                    return;
                } else if self.model.can_close()
                    && let Some(on_close) = &self.on_close_request
                {
                    shell.publish((on_close)(Cancel::new(())));
                    shell.capture_event();
                    return;
                }
            }
            _ => {}
        }

        // Forward to content children
        let mut children_layout = layout.children();
        if let Some(content_layout) = children_layout.next() {
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                content_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        if let (Some(footer), Some(footer_layout)) = (&mut self.footer, children_layout.next()) {
            footer.as_widget_mut().update(
                &mut tree.children[1],
                event,
                footer_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if !self.model.open {
            return mouse::Interaction::None;
        }

        let bounds = layout.bounds();
        let half_grab = GRAB_WIDTH / 2.0;
        let grab_x = match self.model.placement {
            SheetPlacement::Right => bounds.x - half_grab,
            SheetPlacement::Left => bounds.x + bounds.width - half_grab,
        };
        let grab_rect = Rectangle {
            x: grab_x,
            y: bounds.y,
            width: GRAB_WIDTH,
            height: bounds.height,
        };

        let state = tree.state.downcast_ref::<SideSheetState>();
        if state.resize.is_active() || cursor.is_over(grab_rect) {
            return mouse::Interaction::ResizingHorizontally;
        }

        let pin_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE * 2.0 - space::XS;
        let pin_y = bounds.y + (HEADER_HEIGHT - HEADER_BUTTON_SIZE) / 2.0;
        let pin_rect = Rectangle {
            x: pin_x,
            y: pin_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        let close_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE;
        let close_y = pin_y;
        let close_rect = Rectangle {
            x: close_x,
            y: close_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        if cursor.is_over(pin_rect) || (cursor.is_over(close_rect) && !self.model.busy) {
            return mouse::Interaction::Pointer;
        }

        let mut children_layout = layout.children();
        if let Some(content_layout) = children_layout.next() {
            let interaction = self.content.as_widget().mouse_interaction(
                &tree.children[0],
                content_layout,
                cursor,
                viewport,
                renderer,
            );
            if interaction != mouse::Interaction::None {
                return interaction;
            }
        }

        if let (Some(footer), Some(footer_layout)) = (&self.footer, children_layout.next()) {
            let interaction = footer.as_widget().mouse_interaction(
                &tree.children[1],
                footer_layout,
                cursor,
                viewport,
                renderer,
            );
            if interaction != mouse::Interaction::None {
                return interaction;
            }
        }

        mouse::Interaction::None
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
        if !self.model.open {
            return;
        }

        let bounds = layout.bounds();
        let palette = Palette::dark();

        // 1. Drawer background: surface.raised with 1px border.subtle
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border {
                    color: palette.border_subtle,
                    width: shape::BORDER_HAIRLINE,
                    radius: 0.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.surface_raised),
        );

        // 2. Header bottom border (1px border.subtle)
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: bounds.y + HEADER_HEIGHT - shape::BORDER_HAIRLINE,
                    width: bounds.width,
                    height: shape::BORDER_HAIRLINE,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.border_subtle),
        );

        // 3. Header title
        let title_width =
            (bounds.width - (space::MD * 2.0) - (HEADER_BUTTON_SIZE * 2.0) - space::XS).max(0.0);
        let title_rect = Rectangle {
            x: bounds.x + space::MD,
            y: bounds.y,
            width: title_width,
            height: HEADER_HEIGHT,
        };
        renderer.fill_text(
            text_core::Text {
                content: self.model.title.clone(),
                bounds: title_rect.size(),
                size: Pixels(typography::TITLE_SM.size),
                line_height: LineHeight::Absolute(Pixels(HEADER_HEIGHT)),
                font: iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..renderer.default_font()
                },
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(title_rect.x, title_rect.center_y()),
            palette.text_strong,
            *viewport,
        );

        // 4. Header buttons: [Pin/Unpin] & [Close ×]
        let pin_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE * 2.0 - space::XS;
        let pin_y = bounds.y + (HEADER_HEIGHT - HEADER_BUTTON_SIZE) / 2.0;
        let pin_rect = Rectangle {
            x: pin_x,
            y: pin_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        let close_x = bounds.x + bounds.width - space::MD - HEADER_BUTTON_SIZE;
        let close_y = pin_y;
        let close_rect = Rectangle {
            x: close_x,
            y: close_y,
            width: HEADER_BUTTON_SIZE,
            height: HEADER_BUTTON_SIZE,
        };

        let state = tree.state.downcast_ref::<SideSheetState>();

        // Pin button hover background
        if state.is_hovered_pin {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: pin_rect,
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.surface_active),
            );
        }

        // Close button hover background (if not busy)
        if state.is_hovered_close && !self.model.busy {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: close_rect,
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.surface_active),
            );
        }

        // Pin glyph
        let pin_glyph = match self.model.mode {
            SheetMode::Docked => GLYPH_PINNED,
            _ => GLYPH_UNPINNED,
        };
        renderer.fill_text(
            text_core::Text {
                content: pin_glyph.to_owned(),
                bounds: pin_rect.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: LineHeight::Absolute(Pixels(HEADER_BUTTON_SIZE)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            pin_rect.center(),
            palette.text_strong,
            *viewport,
        );

        // Close glyph
        let close_color = if self.model.busy {
            palette.text_disabled
        } else {
            palette.text_strong
        };
        renderer.fill_text(
            text_core::Text {
                content: GLYPH_CLOSE.to_owned(),
                bounds: close_rect.size(),
                size: Pixels(typography::TITLE_SM.size),
                line_height: LineHeight::Absolute(Pixels(HEADER_BUTTON_SIZE)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            close_rect.center(),
            close_color,
            *viewport,
        );

        // 5. Children drawing
        let mut children_layout = layout.children();
        if let Some(content_layout) = children_layout.next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                content_layout,
                cursor,
                viewport,
            );
        }

        if let (Some(footer), Some(footer_layout)) = (&self.footer, children_layout.next()) {
            // Footer top border (1px border.subtle)
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y: bounds.y + bounds.height - FOOTER_HEIGHT,
                        width: bounds.width,
                        height: shape::BORDER_HAIRLINE,
                    },
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(palette.border_subtle),
            );

            footer.as_widget().draw(
                &tree.children[1],
                renderer,
                theme,
                style,
                footer_layout,
                cursor,
                viewport,
            );
        }

        // 6. Draw edge divider line & centered grip pill
        let is_active_drag = state.resize.is_active();
        let is_hovered = state.is_hovered_grab;
        let is_promoted = is_active_drag || is_hovered;

        let divider_color = if is_promoted {
            palette.accent_border
        } else {
            palette.border_subtle
        };

        let pill_color = if is_promoted {
            palette.accent_border
        } else {
            palette.border_strong
        };

        let divider_x = match self.model.placement {
            SheetPlacement::Right => bounds.x,
            SheetPlacement::Left => bounds.x + bounds.width - DIVIDER_STROKE,
        };

        // Full-height edge divider line (1px)
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: divider_x,
                    y: bounds.y,
                    width: DIVIDER_STROKE,
                    height: bounds.height,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(divider_color),
        );

        // Centered grip pill (2px × 24px)
        let pill_x = divider_x - (GRIP_PILL_WIDTH - DIVIDER_STROKE) / 2.0;
        let pill_y = bounds.y + (bounds.height - GRIP_PILL_HEIGHT) / 2.0;

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: pill_x,
                    y: pill_y,
                    width: GRIP_PILL_WIDTH,
                    height: GRIP_PILL_HEIGHT,
                },
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: GRIP_PILL_RADIUS.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(pill_color),
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if !self.model.open {
            return;
        }
        let mut children_layout = layout.children();
        if let Some(content_layout) = children_layout.next() {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                content_layout,
                renderer,
                operation,
            );
        }
        if let (Some(footer), Some(footer_layout)) = (&mut self.footer, children_layout.next()) {
            footer.as_widget_mut().operate(
                &mut tree.children[1],
                footer_layout,
                renderer,
                operation,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        if !self.model.open {
            return None;
        }
        let mut children_layout = layout.children();
        let content_layout = children_layout.next()?;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<
    'a,
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
> From<SideSheet<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(sheet: SideSheet<'a, Message, Theme, Renderer>) -> Self {
        Element::new(sheet)
    }
}

/// Helper function to construct a [`SideSheet`] widget (`UIF-COMP-012`).
#[must_use]
pub fn side_sheet<
    'a,
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
>(
    model: SideSheetModel,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> SideSheet<'a, Message, Theme, Renderer> {
    SideSheet::new(model, content)
}

#[cfg(test)]
#[path = "side_sheet_tests.rs"]
mod side_sheet_tests;
