//! The segmented-control resolver, geometry, and live interactive widget —
//! the joined strip of §3.1b (`UIF-ATOM-001b`).
//!
//! This module resolves paint, computes boxes, and provides the live
//! [`SegmentedButton`] widget. It is **not** a second button. Every
//! unselected segment is [`button::resolve_style`] called with
//! [`ButtonVariant::Secondary`], and every selected segment is that same
//! resolver pinned to the `Pressed` cell — so the strip cannot drift from
//! §3.1 the way a copied button would.
//!
//! # The selected segment *is* the Secondary `Pressed` cell
//!
//! §3.1b specifies the selected segment as `BG: surface.active`,
//! `FG: text.strong` (`14.29:1`), `Border: 1px accent.border`
//! (`4.79:1`). That triple is, token for token, §3.1's Secondary
//! **Pressed** cell. So this resolver does not restate it: it asks
//! `button::resolve_style(Secondary, Pressed, palette)` and takes the
//! answer. Four consequences worth naming:
//!
//! * the fill/foreground pair goes through
//!   [`assert_fill_foreground_lock`](crate::token::contrast::assert_fill_foreground_lock)
//!   for free, because the button resolver already runs it;
//! * the ring is resolved against `surface.active` by the same adaptive
//!   rule that produced §3.1's Secondary ring, so it is `accent.border`
//!   here for the same arithmetic that makes it `accent.border` there;
//! * a future change to §3.1's Secondary pressed cell moves the selected
//!   segment with it, which is the point — the two are specified as the
//!   same paint and now *are* the same paint;
//! * the disabled selected segment is the Secondary **Disabled** cell,
//!   which is the crate-wide disabled convention §3.1 and §3.3 already
//!   share.
//!
//! # The selected paint is held through hover and press
//!
//! §3.1b gives the selected segment **one** cell and says nothing about
//! what it looks like under the pointer. Three readings were possible; the
//! one taken is that the selection paint does not move.
//!
//! The reason is the ladder. `surface.active` (`RGB(42,42,46)`) is
//! already lighter than `surface.hover` (`RGB(32,32,36)`), so a selected
//! segment that "responded" to hover by dropping to the hover rung would
//! go *darker* on the interaction that is supposed to say "you can act
//! on this", and a selected-and-pressed segment would have nowhere left to
//! go that was not darker still. Holding `surface.active` keeps a selected
//! segment above every hover rung in the palette at every pointer state,
//! which is the only reading that needs no invented token.
//!
//! **The finding that comes with that decision:** a *pressed unselected*
//! segment resolves to exactly the same three colours as a *selected* one
//! — both are `surface.active` / `text.strong` / `accent.border`. The
//! reserved `✓` is the only thing that tells them apart. That is not
//! fixable inside this atom: separating them would need a fourth surface
//! rung the palette does not have, or a selected fill the spec assigns to
//! `accent.fill` elsewhere. It is recorded here, pinned by a sidecar test
//! that asserts the collision rather than hiding it, and it is a spec
//! question rather than an implementation bug.
//!
//! # The `✓` slot is reserved on every segment
//!
//! §3.1b calls the leading `✓` "a reserved `14px` leading icon".
//! Reserved is the operative word, and for a strip it means more than it
//! does for a [`chip`](crate::widget::chip): if only the *selected*
//! segment carried the glyph, then selecting a different segment would
//! shift every label to its right, and a keyboard user roving the strip
//! would watch the whole control reflow on every arrow key.
//! `UIF-PRIN-003` forbids exactly that, so [`segment_width`] charges
//! the glyph box plus its gap to **every** segment whether or not that
//! segment is selected, and [`check_slot`] hands back the box that was
//! paid for. Selection changes which segment paints `✓` in; it moves no
//! text.
//!
//! # Focus is the strip's, not the segment's
//!
//! The ring goes around the **whole strip** (§3.0.2's `2px` outer ring,
//! one per control), not around the focused segment. A per-segment ring
//! would sit on the shared border between two neighbours and read as that
//! neighbour's edge, and it would make the ring's colour depend on which
//! segment's fill it happened to overlap. [`resolve_strip_ring`] therefore
//! resolves against the **mount** the strip sits on — the surface the ring
//! is actually drawn over — and the roving index is carried separately by
//! the model, because which segment holds the keyboard is not a paint
//! question.
//!
//! Unit tests live in the sibling `segmented_tests.rs` sidecar.

use std::fmt;

use crate::binding::Controlled;
use crate::focus::FocusId;
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button::{self, ButtonStyle, ButtonVariant};
use crate::widget::focus;
use crate::widget::focusable::FocusableState;
use crate::widget::rove::RoveDirection;
use crate::widget::segmented_model::{SegmentItem, SegmentedModel};

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph as _};
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

pub use crate::bridge::widget_id;

/// The strip's outer corner radius — `radius.widget` (§3.1b,
/// `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The strip's perimeter border width — `border.hairline` (§3.1b).
///
/// Constant across every state: a strip that thickened its perimeter when
/// a segment was selected would move its own segments.
pub const PERIMETER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The focus ring width around the strip — `border.focus` (§3.0.2).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The horizontal padding inside a segment — `space.md` (§3.1b,
/// `UIF-TOK-001`).
pub const SEGMENT_PADDING: f32 = space::MD;

/// The reserved leading `✓` box — `size.icon.sm` (`14px`, §3.1b).
pub const CHECK_GLYPH: f32 = size::ICON_SM;

/// Gap between the reserved `✓` box and the segment's label —
/// `space.sm` (§3.1b, `UIF-TOK-001`).
pub const CHECK_GAP: f32 = space::SM;

/// The width the reserved check slot costs a segment: the glyph box plus
/// its gap.
///
/// Charged to every segment by [`segment_width`], selected or not.
pub const CHECK_SLOT_WIDTH: f32 = CHECK_GLYPH + CHECK_GAP;

/// The checkmark glyph rendered in the selected segment's leading slot (§3.1b, `UIF-ATOM-001b`).
pub const CHECK_MARK: &str = "✓";

/// The compact strip height — `size.control.sm` (§3.1b).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// The standard strip height — `size.control.md` (§3.1b).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// Which of §3.1b's two segment polarities a cell belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SegmentPolarity {
    /// §3.1b's "Unselected segments resolve through `ButtonVariant::Secondary`"
    /// — and the hover/press/disabled rungs that go with it.
    #[default]
    Unselected,
    /// The selected cell: `surface.active` / `text.strong` /
    /// `accent.border`, plus the leading `✓`.
    Selected,
}

impl SegmentPolarity {
    /// The polarity of one segment of a model.
    #[must_use]
    pub fn of<Id: Clone + Ord>(model: &SegmentedModel<Id>, index: usize) -> Self {
        let Some(segment) = model.segments.get(index) else {
            return Self::Unselected;
        };
        if model.is_selected(&segment.id) {
            Self::Selected
        } else {
            Self::Unselected
        }
    }
}

/// A segment's resolved paint for one `(polarity, state)`.
///
/// A [`ButtonStyle`] rather than a new struct on purpose: the segment's
/// fill, foreground, border and ring are the *button atom's* four
/// channels, and giving the strip its own name for them would be the
/// first step toward two tables that disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentStyle {
    /// The resolved button style for this segment.
    pub paint: ButtonStyle,
    /// The leading `✓` colour — [`accent_text`](Palette::accent_text)
    /// while live, [`text_disabled`](Palette::text_disabled) when inert.
    pub check: Color,
    /// Whether the `✓` is painted in this cell.
    ///
    /// True only in the selected polarity. Kept separate from
    /// [`paint`](Self::paint) so a renderer knows whether to draw the
    /// glyph without re-deriving the polarity, and so the *reserved*
    /// slot in an unselected segment is not mistaken for a hidden one.
    pub shows_check: bool,
}

/// Resolve one segment's paint (`UIF-ATOM-001b`).
///
/// Unselected is a direct pass-through to
/// [`button::resolve_style`] with [`ButtonVariant::Secondary`]. Selected
/// is that same resolver pinned to `BaseState::Pressed`, with the
/// `focused` half carried across untouched so focus stays additive here
/// exactly as it is in §3.1.
///
/// The disabled selected segment maps to the Secondary `Disabled` cell
/// rather than holding `surface.active`: `text_disabled` over
/// `surface.active` is `4.2510:1`, which is inside the `4.0`–`6.0`
/// disabled band, but the crate's disabled convention is `surface.base`,
/// and a selected segment that stayed lit while dead would out-shine a
/// live unselected neighbour.
#[must_use]
pub fn resolve_style(
    polarity: SegmentPolarity,
    state: VisualState,
    palette: &Palette,
) -> SegmentStyle {
    let base = match polarity {
        SegmentPolarity::Unselected => state.base,
        SegmentPolarity::Selected if state.base == BaseState::Disabled => BaseState::Disabled,
        SegmentPolarity::Selected => BaseState::Pressed,
    };

    let paint = button::resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base,
            focused: state.focused,
        },
        palette,
    );

    SegmentStyle {
        paint,
        check: check_color(state, palette),
        shows_check: matches!(polarity, SegmentPolarity::Selected),
    }
}

/// The leading `✓` colour for one state.
///
/// `accent.text` is the token §3.1b names, and it is the *only* cue that
/// separates a selected segment from a pressed unselected one (see the
/// [module docs](self#the-selected-paint-is-held-through-hover-and-press)),
/// so it is never dropped while the control is live. A disabled strip
/// takes `text.disabled`, which keeps the mark present and stops it
/// claiming attention it cannot act on.
#[must_use]
pub const fn check_color(state: VisualState, palette: &Palette) -> Color {
    match state.base {
        BaseState::Disabled => palette.text_disabled,
        _ => palette.accent_text,
    }
}

/// The control-level focus ring for the whole strip.
///
/// Resolved against `mount` — the surface the strip is drawn on, and
/// therefore the surface the ring is drawn over — through the same
/// adaptive rule every atom in the crate uses (`UIF-TOK-006` rule 2,
/// decision **D-6**). Returns `None` when the state carries no ring.
#[must_use]
pub fn resolve_strip_ring(
    state: VisualState,
    mount: Color,
    palette: &Palette,
) -> Option<(Color, f32)> {
    if !state.ring_visible() {
        return None;
    }
    Some((
        focus::resolve_ring_color_for_background(mount, palette),
        FOCUS_RING_WIDTH,
    ))
}

/// The width one segment needs for its label.
///
/// The reserved check slot is charged in **unconditionally**, which is
/// the whole of the zero-shift guarantee for this control: the width a
/// segment gets does not depend on whether it is selected, so moving the
/// selection moves no label (`UIF-PRIN-003`).
#[must_use]
pub fn segment_width(label_width: f32) -> f32 {
    label_width.max(0.0) + CHECK_SLOT_WIDTH + SEGMENT_PADDING * 2.0
}

/// The width a whole strip needs for its labels, in display order.
#[must_use]
pub fn strip_width(label_widths: &[f32]) -> f32 {
    label_widths.iter().map(|width| segment_width(*width)).sum()
}

/// The rectangle of segment `index` inside a strip occupying `strip`.
///
/// Segments are laid out left to right at their own widths, flush to the
/// strip's leading edge, so the box this returns is the box
/// [`strip_width`] paid for.
#[must_use]
pub fn segment_bounds(strip: Rectangle, index: usize, label_widths: &[f32]) -> Rectangle {
    let offset: f32 = label_widths
        .iter()
        .take(index)
        .map(|width| segment_width(*width))
        .sum();
    let width = label_widths
        .get(index)
        .map(|width| segment_width(*width))
        .unwrap_or(0.0);

    Rectangle::new(
        Point::new(strip.x + offset, strip.y),
        Size::new(width, strip.height),
    )
}

/// The reserved leading `✓` box for a segment.
///
/// The box is the same rectangle whether or not the `✓` is painted in
/// it — that is what "reserved" buys — and it sits at the segment's
/// leading padding, vertically centred.
#[must_use]
pub fn check_slot(segment: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            segment.x + SEGMENT_PADDING,
            segment.y + (segment.height - CHECK_GLYPH) / 2.0,
        ),
        Size::new(CHECK_GLYPH, CHECK_GLYPH),
    )
}

/// The label rectangle for a segment: everything past the reserved slot
/// and its gap, inside the trailing padding.
#[must_use]
pub fn label_bounds(segment: Rectangle) -> Rectangle {
    let x = segment.x + SEGMENT_PADDING + CHECK_SLOT_WIDTH;
    Rectangle::new(
        Point::new(x, segment.y),
        Size::new(
            (segment.width - SEGMENT_PADDING * 2.0 - CHECK_SLOT_WIDTH).max(0.0),
            segment.height,
        ),
    )
}

/// Classify the strip's perimeter under `UIF-TOK-006` rule 3.
///
/// The perimeter is `border.strong` in every state, which against the
/// dark mounts measures `4.2379:1` on `surface.raised` and `4.5062:1`
/// on `surface.base` — comfortably [`Judged`](ChromeClass::Judged). A
/// perimeter that ever resolved to `border.subtle` would be a resting
/// divider and exempt, which is why the classification is by token
/// identity rather than a constant.
#[must_use]
pub fn perimeter_class(style: &SegmentStyle, palette: &Palette) -> ChromeClass {
    if same_channels(style.paint.border_color, style.paint.background) {
        ChromeClass::CoincidentWithFill
    } else if same_channels(style.paint.border_color, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Whether two colours carry identical channels.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

/// Compute the asymmetric corner radii for segment `index` in a strip of `count` segments (§3.1b, `UIF-ATOM-001b`).
#[must_use]
pub fn segment_radius(index: usize, count: usize) -> iced::border::Radius {
    if count <= 1 {
        iced::border::Radius {
            top_left: RADIUS,
            top_right: RADIUS,
            bottom_right: RADIUS,
            bottom_left: RADIUS,
        }
    } else if index == 0 {
        iced::border::Radius {
            top_left: RADIUS,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: RADIUS,
        }
    } else if index == count - 1 {
        iced::border::Radius {
            top_left: 0.0,
            top_right: RADIUS,
            bottom_right: RADIUS,
            bottom_left: 0.0,
        }
    } else {
        iced::border::Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }
    }
}

/// Measure the intrinsic label widths for a slice of segments using the text renderer (§3.1b, `UIF-ATOM-001b`).
fn measure_label_widths<Renderer: text_core::Renderer, Id>(
    segments: &[SegmentItem<Id>],
    renderer: &Renderer,
) -> Vec<f32> {
    segments
        .iter()
        .map(|segment| {
            let label = &segment.label;
            let measured = {
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
            if measured > 0.0 {
                measured
            } else {
                label.len() as f32 * 8.0
            }
        })
        .collect()
}

/// The internal state of a [`SegmentedButton`] stored in the iced widget tree (§3.1b, `UIF-ATOM-001b`).
#[derive(Clone, Debug, Default)]
pub struct SegmentedState {
    /// The index of the currently hovered segment, if any.
    pub hovered: Option<usize>,
    /// The index of the currently pressed segment, if any.
    pub pressed: Option<usize>,
    /// The iced keyboard focusable state.
    pub focusable: FocusableState,
    /// The index of the currently keyboard-focused segment within the strip, if any.
    pub focused_segment: Option<usize>,
}

impl SegmentedState {
    /// Create a new resting [`SegmentedState`] (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// A live interactive segmented control widget (§3.1b, `UIF-ATOM-001b`).
pub struct SegmentedButton<'a, Message, Id = String>
where
    Id: Clone + Ord,
{
    focus_id: FocusId,
    widget_id: WidgetId,
    model: Controlled<'a, SegmentedModel<Id>>,
    palette: Palette,
    emit: Box<dyn Fn(std::collections::BTreeSet<Id>) -> Option<Message> + 'a>,
}

impl<'a, Message, Id> SegmentedButton<'a, Message, Id>
where
    Id: Clone + Ord,
{
    /// Build a controlled [`SegmentedButton`] over a host-owned [`SegmentedModel`] (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, SegmentedModel<Id>>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(std::collections::BTreeSet<Id>) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = crate::bridge::widget_id(&focus_id);

        Self {
            focus_id,
            widget_id,
            model,
            palette: Palette::dark(),
            emit: Box::new(emit),
        }
    }

    /// Set the palette used to resolve colours (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Builder alias for [`palette`](Self::palette) (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn with_palette(self, palette: Palette) -> Self {
        self.palette(palette)
    }

    /// The focus id registered for this control (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced widget id corresponding to the focus id (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn widget_id(&self) -> &WidgetId {
        &self.widget_id
    }

    /// The model driving this segmented button (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn model(&self) -> &SegmentedModel<Id> {
        self.model.get()
    }

    /// The palette used by this segmented button (§3.1b, `UIF-ATOM-001b`).
    #[must_use]
    pub fn palette_ref(&self) -> &Palette {
        &self.palette
    }
}

const SNAP_TO_PIXEL_GRID: bool = true;

impl<Message, Id: Clone + Ord + fmt::Debug> fmt::Debug for SegmentedButton<'_, Message, Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SegmentedButton")
            .field("focus_id", &self.focus_id)
            .field("widget_id", &self.widget_id)
            .field("model", self.model.get())
            .field("palette", &self.palette)
            .finish()
    }
}

impl<'a, Message, Theme, Renderer, Id> Widget<Message, Theme, Renderer>
    for SegmentedButton<'a, Message, Id>
where
    Message: 'a + Clone,
    Renderer: renderer::Renderer + text_core::Renderer,
    Id: Clone + Ord + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SegmentedState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SegmentedState::new())
    }

    fn size(&self) -> Size<Length> {
        let height = self.model.get().density.height();
        Size::new(Length::Shrink, Length::Fixed(height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let model = self.model.get();
        let label_widths = measure_label_widths(&model.segments, renderer);
        let intrinsic_width = strip_width(&label_widths);
        let height = model.density.height();
        let intrinsic = Size::new(intrinsic_width, height);
        let size = limits.resolve(Length::Shrink, Length::Fixed(height), intrinsic);
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<SegmentedState>();
        if !self.model.get().disabled {
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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let model = self.model.get();
        if model.disabled {
            return mouse::Interaction::None;
        }
        let bounds = layout.bounds();
        if let Some(pos) = cursor.position_over(bounds) {
            let label_widths = measure_label_widths(&model.segments, renderer);
            for (i, _) in model.segments.iter().enumerate() {
                let seg_bounds = segment_bounds(bounds, i, &label_widths);
                if seg_bounds.contains(pos) && model.segment_enabled(i) {
                    return mouse::Interaction::Pointer;
                }
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
        renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<SegmentedState>();
        let model = self.model.get();

        if model.disabled {
            state.hovered = None;
            state.pressed = None;
            return;
        }

        let bounds = layout.bounds();
        let label_widths = measure_label_widths(&model.segments, renderer);

        let hovered_segment = if let Some(pos) = cursor.position_over(bounds) {
            (0..model.segments.len())
                .find(|&i| segment_bounds(bounds, i, &label_widths).contains(pos))
        } else {
            None
        };

        state.hovered = hovered_segment;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(i) = hovered_segment
                    && model.segment_enabled(i)
                {
                    state.pressed = Some(i);
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(pressed_idx) = state.pressed.take() {
                    if hovered_segment == Some(pressed_idx) && model.segment_enabled(pressed_idx) {
                        state.focused_segment = Some(pressed_idx);
                        if let Some(commit) = model.toggle_index(pressed_idx)
                            && let Some(msg) = (self.emit)(commit.into_value())
                        {
                            shell.publish(msg);
                        }
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() =>
            {
                match key {
                    Key::Named(Named::ArrowLeft) | Key::Named(Named::ArrowUp) => {
                        let next = model.rove(state.focused_segment, RoveDirection::Backward);
                        if next.is_some() {
                            state.focused_segment = next;
                            shell.capture_event();
                        }
                    }
                    Key::Named(Named::ArrowRight) | Key::Named(Named::ArrowDown) => {
                        let next = model.rove(state.focused_segment, RoveDirection::Forward);
                        if next.is_some() {
                            state.focused_segment = next;
                            shell.capture_event();
                        }
                    }
                    Key::Named(Named::Space) | Key::Named(Named::Enter) => {
                        let target = state
                            .focused_segment
                            .filter(|&i| model.segment_enabled(i))
                            .or_else(|| {
                                model
                                    .segments
                                    .iter()
                                    .position(|s| model.is_selected(&s.id) && !s.disabled)
                            })
                            .or_else(|| model.enabled_indices().first().copied());

                        if let Some(idx) = target {
                            state.focused_segment = Some(idx);
                            if let Some(commit) = model.toggle_index(idx)
                                && let Some(msg) = (self.emit)(commit.into_value())
                            {
                                shell.publish(msg);
                            }
                            shell.capture_event();
                        }
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
        let state = tree.state.downcast_ref::<SegmentedState>();
        let model = self.model.get();
        let bounds = layout.bounds();
        let count = model.segments.len();
        if count == 0 {
            return;
        }

        let label_widths = measure_label_widths(&model.segments, renderer);

        for (i, segment) in model.segments.iter().enumerate() {
            let seg_bounds = segment_bounds(bounds, i, &label_widths);
            let polarity = SegmentPolarity::of(model, i);

            let base = if !model.segment_enabled(i) {
                BaseState::Disabled
            } else if state.pressed == Some(i) {
                BaseState::Pressed
            } else if state.hovered == Some(i) {
                BaseState::Hovered
            } else {
                BaseState::Rest
            };

            let seg_state = VisualState {
                base,
                focused: false,
            };

            let seg_style = resolve_style(polarity, seg_state, &self.palette);
            let radius = segment_radius(i, count);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: seg_bounds,
                    border: Border::default()
                        .color(seg_style.paint.border_color)
                        .width(seg_style.paint.border_width)
                        .rounded(radius),
                    shadow: Shadow::default(),
                    snap: SNAP_TO_PIXEL_GRID,
                },
                Background::Color(seg_style.paint.background),
            );

            if seg_style.shows_check {
                let slot = check_slot(seg_bounds);
                renderer.fill_text(
                    text_core::Text {
                        content: CHECK_MARK.to_owned(),
                        bounds: slot.size(),
                        size: Pixels(CHECK_GLYPH),
                        line_height: LineHeight::Absolute(Pixels(slot.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    slot.center(),
                    seg_style.check,
                    *viewport,
                );
            }

            let lbl_bounds = label_bounds(seg_bounds);
            renderer.fill_text(
                text_core::Text {
                    content: segment.label.clone(),
                    bounds: lbl_bounds.size(),
                    size: renderer.default_size(),
                    line_height: LineHeight::Absolute(Pixels(lbl_bounds.height)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Left,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                Point::new(lbl_bounds.x, lbl_bounds.center_y()),
                seg_style.paint.foreground,
                *viewport,
            );
        }

        if state.focusable.is_focused()
            && let Some((ring_color, ring_width)) = resolve_strip_ring(
                VisualState {
                    base: BaseState::Rest,
                    focused: true,
                },
                self.palette.surface_base,
                &self.palette,
            )
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: bounds.expand(ring_width),
                    border: Border::default()
                        .color(ring_color)
                        .width(ring_width)
                        .rounded(RADIUS + ring_width),
                    shadow: Shadow::default(),
                    snap: SNAP_TO_PIXEL_GRID,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }
    }
}

impl<'a, Message, Theme, Renderer, Id> From<SegmentedButton<'a, Message, Id>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer,
    Id: Clone + Ord + 'a,
{
    fn from(button: SegmentedButton<'a, Message, Id>) -> Self {
        Element::new(button)
    }
}

#[cfg(test)]
#[path = "segmented_tests.rs"]
mod segmented_tests;
