//! The split-button geometry and paint resolver — the two-zone control of
//! §3.1c (`UIF-ATOM-001c`).
//!
//! This module answers two questions and holds no state: *which half is
//! under this point*, and *what does each half paint*. The state machine
//! that uses them is [`SplitButtonHost`](crate::widget::split_button_host).
//!
//! # Both halves *are* buttons
//!
//! §3.1c describes the control as "combining a primary action zone" with
//! a chevron zone, and gives it a `variant: Primary | Secondary` — but it
//! prints **no state matrix of its own**. The reading taken here is the
//! compositional one: each half is resolved through
//! [`button::resolve_style`] with the control's variant, exactly as
//! §3.1b's segments are. So a Primary split button's label half is
//! §3.1's Primary cell for its state, and its chevron half is the same
//! cell for *its own* state.
//!
//! That is what makes the two zones genuinely independent rather than one
//! control with a decoration on the end: they are two resolvers, each fed
//! its own [`VisualState`], and nothing in this module lets one borrow the
//! other's rung. The sidecar pins the two negative cases that follow —
//! hovering the label leaves the chevron half at rest, and hovering the
//! chevron leaves the label half at rest.
//!
//! # The chevron takes the half's own foreground
//!
//! §3.1c names the chevron per variant: `text.primary` at rest and
//! `text.strong` on hover/open for `Secondary`, `text.strong` for
//! `Primary`. That is exactly the [`foreground`](ButtonStyle::foreground)
//! channel of the menu half's resolved [`ButtonStyle`], so
//! [`chevron_color`] reads that channel rather than restating the tokens —
//! `text.strong` (`4.8020:1`) on a Primary fill, `text.primary` at rest
//! and `text.strong` on hover for a Secondary one.
//!
//! What cannot travel here is §3.4's combobox precedent of painting the
//! glyph `text.muted` at rest: `text.muted` on `accent.fill` measures
//! `1.8363:1`, which the spec names as a failure outright.
//!
//! # The seam is per variant, because one border token cannot serve both fills
//!
//! §3.1c resolves the `1px` internal divider **per variant**, and
//! [`divider_color`] follows it:
//!
//! - `Secondary` — `border.strong`, `4.2379:1` on `surface.raised`.
//! - `Primary` — `text.strong`, `4.8020:1` on `accent.fill`.
//!
//! The reason the Primary half cannot borrow the Secondary token is arithmetic:
//! `border.strong` (`RGB(120,120,120)`) against `accent.fill`
//! (`RGB(93,96,238)`) measures **`1.0876:1`**, and `text.muted` is no
//! better at **`1.8363:1`**. Both are named as failures by the spec, which
//! routes the seam to `text.strong` under `UIF-TOK-006` rule 4 — the same
//! rule that already fixes the Primary label's foreground. The seam, the
//! chevron and the label therefore form one `4.80:1` family on a Primary
//! fill instead of a visible line on one variant and an invisible one on
//! the other.
//!
//! The divider is still **not** offered to the gallery as a judged
//! contrast pair. It is a seam, not a foreground: scoring a 1px line
//! against a text-contrast floor would be a category error even now that
//! the token is legible, so the figure is reported in the cell's
//! `resolved` note instead.
//!
//! # The divider belongs to the chevron half
//!
//! [`zone_at`] gives the divider's pixel to [`SplitZone::Menu`]. A 1px
//! aim error on a shared edge should open a menu, which is reversible,
//! rather than fire the primary action, which is not.
//!
//! Unit tests live in the sibling `split_button_tests.rs` sidecar.

use std::fmt;

use crate::binding::Controlled;
use crate::focus::{FocusId, FocusState};
use crate::state::{BaseState, VisualState};
use crate::surface::menu::cascade::MenuKey;
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button::{self, ButtonStyle, ButtonVariant};
use crate::widget::button_model::ButtonDensity;
use crate::widget::focusable::FocusableState;
use crate::widget::split_button_host::{SplitButtonEmission, SplitButtonHost, SplitButtonOutcome};
use crate::widget::split_button_model::SplitButtonModel;

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
use iced::{
    Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Shadow, Size,
};

pub use crate::bridge::widget_id;

/// The trailing chevron zone's width — the `28px`-wide `▾` zone of
/// §3.1c (`UIF-ATOM-001c`).
///
/// Equal to `size.control.sm`, so the zone is square at the compact
/// height and a tall narrow rectangle at the standard one. It is a
/// *fixed* width, not a proportion: the spec gives the chevron zone a
/// number and the label zone the remainder.
pub const CHEVRON_ZONE_WIDTH: f32 = size::CONTROL_SM;

/// The vertical divider between the halves — `1px`, the token being
/// per-variant (see [`divider_color`], §3.1c).
pub const DIVIDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Horizontal padding inside the primary action zone — the
/// `padding: [0, 12px]` of §3.1c (`UIF-TOK-001`).
pub const PRIMARY_PADDING: f32 = space::MD;

/// The control's outer corner radius — `radius.widget` (§3.0.2).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The control perimeter width.
pub const PERIMETER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The focus ring width — `border.focus` (§3.0.2).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The compact control height — `size.control.sm` (§3.1c).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// The standard control height — `size.control.md` (§3.1c).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// Which half of a split control a point fell in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SplitZone {
    /// The primary action zone — the label half.
    #[default]
    Primary,
    /// The trailing `▾` menu zone.
    Menu,
    /// Outside the control entirely.
    ///
    /// A distinct answer rather than a clamp, because a press that started
    /// outside and dragged in must not be read as a press *on* the half
    /// it ended over, and a release outside must cancel rather than fire.
    Outside,
}

impl SplitZone {
    /// Whether the zone is one of the two live halves.
    #[must_use]
    pub const fn is_hit(self) -> bool {
        !matches!(self, Self::Outside)
    }
}

/// The x at which the chevron zone begins — and therefore where the
/// divider is drawn.
#[must_use]
pub fn divider_x(width: f32) -> f32 {
    (width - CHEVRON_ZONE_WIDTH).max(0.0)
}

/// Which half `x` falls in, for a control `width` wide.
///
/// The divider's pixel goes to [`SplitZone::Menu`] — see the
/// [module docs](self#the-divider-belongs-to-the-chevron-half).
#[must_use]
pub fn zone_at(x: f32, width: f32) -> SplitZone {
    if x < 0.0 || x >= width {
        return SplitZone::Outside;
    }
    if x < divider_x(width) {
        SplitZone::Primary
    } else {
        SplitZone::Menu
    }
}

/// The primary action zone's rectangle within a control box.
#[must_use]
pub fn primary_zone(box_: Rectangle) -> Rectangle {
    Rectangle::new(
        box_.position(),
        Size::new(divider_x(box_.width), box_.height),
    )
}

/// The trailing chevron zone's rectangle within a control box.
///
/// Starts at the divider, so the divider is inside this half's left edge
/// rather than a third box between them.
#[must_use]
pub fn menu_zone(box_: Rectangle) -> Rectangle {
    let x = divider_x(box_.width);
    Rectangle::new(
        Point::new(box_.x + x, box_.y),
        Size::new(box_.width - x, box_.height),
    )
}

/// The divider's own rectangle within a control box.
#[must_use]
pub fn divider_bounds(box_: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(box_.x + divider_x(box_.width), box_.y),
        Size::new(DIVIDER_WIDTH, box_.height),
    )
}

/// The width the control needs for a label of `label_width`.
///
/// The chevron zone is charged in full and does not depend on the label,
/// so widening the label never moves the chevron's *right* edge relative
/// to the control — only the divider slides, which is what the fixed
/// chevron zone means.
#[must_use]
pub fn control_width(label_width: f32) -> f32 {
    label_width.max(0.0) + PRIMARY_PADDING * 2.0 + CHEVRON_ZONE_WIDTH
}

/// Where the label sits inside the primary zone.
#[must_use]
pub fn label_bounds(primary: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(primary.x + PRIMARY_PADDING, primary.y),
        Size::new(
            (primary.width - PRIMARY_PADDING * 2.0).max(0.0),
            primary.height,
        ),
    )
}

/// The `▾` glyph box, centred in the chevron zone.
#[must_use]
pub fn chevron_bounds(menu: Rectangle) -> Rectangle {
    let side = size::ICON_SM;
    Rectangle::new(
        Point::new(
            menu.x + (menu.width - side) / 2.0,
            menu.y + (menu.height - side) / 2.0,
        ),
        Size::new(side, side),
    )
}

/// Both halves' resolved paint for one frame.
///
/// Two [`ButtonStyle`]s rather than a merged struct because the halves are
/// independently hoverable: a single style could not say "label hovered,
/// chevron at rest", which is the most common state this control is in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitStyle {
    /// The primary action half.
    pub primary: ButtonStyle,
    /// The chevron half.
    pub menu: ButtonStyle,
    /// The vertical divider colour.
    pub divider: Color,
}

/// Resolve both halves against the control's variant (`UIF-ATOM-001c`).
///
/// Each half goes through [`button::resolve_style`] with its own state, so
/// the `UIF-TOK-006` rule 4 solid-fill foreground lock is asserted for
/// both by the button resolver itself — which matters most for a Primary
/// split button, whose `accent.fill` + `text.strong` pair is exactly the
/// combination rule 4 names.
///
/// The two states are *not* reconciled. Nothing here copies one half's rung
/// onto the other, which is the whole guarantee that the halves have
/// independent hover targets.
#[must_use]
pub fn resolve_style(
    variant: ButtonVariant,
    primary_state: VisualState,
    menu_state: VisualState,
    palette: &Palette,
) -> SplitStyle {
    SplitStyle {
        primary: button::resolve_style(variant, primary_state, palette),
        menu: button::resolve_style(variant, menu_state, palette),
        divider: divider_color(variant, palette),
    }
}

/// The divider colour — resolved **per variant**, as §3.1c prescribes
/// (`UIF-ATOM-001c`).
///
/// `Primary` takes `text.strong` (`4.8020:1` on `accent.fill`);
/// `Secondary` takes `border.strong` (`4.2379:1` on `surface.raised`).
/// See the
/// [module docs](self#the-seam-is-per-variant-because-one-border-token-cannot-serve-both-fills)
/// for why the Primary half may not borrow the Secondary token.
///
/// `Ghost` and `Danger` keep `border.strong`: §3.1c names a seam token for
/// `Secondary` and `Primary` only, so those two variants fall back to the
/// strongest resting border rather than an invented rule. That fallback is
/// unverified against a Danger fill — `border.strong` on
/// `status.danger.fill` is not a pair the spec prices — and is reported as
/// such rather than claimed as correct.
#[must_use]
pub const fn divider_color(variant: ButtonVariant, palette: &Palette) -> Color {
    match variant {
        ButtonVariant::Primary => palette.text_strong,
        ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Danger => {
            palette.border_strong
        }
    }
}

/// The `▾` glyph colour: the chevron half's own foreground.
///
/// See the [module docs](self#the-chevron-takes-the-halves-own-foreground)
/// for why §3.4's muted-glyph precedent cannot be used here.
#[must_use]
pub fn chevron_color(style: &SplitStyle) -> Color {
    style.menu.foreground
}

/// A stroke width that draws no stroke at all.
const NO_STROKE_WIDTH: f32 = 0.0;

/// Snap painted quads to the device pixel grid.
const SNAP_TO_PIXEL_GRID: bool = true;

/// The trailing chevron glyph (`▾`) painted in the menu zone (§3.1c, `UIF-ATOM-001c`).
pub const CHEVRON_GLYPH: &str = "▾";

/// The control height for a given [`ButtonDensity`] (§3.1c, `UIF-ATOM-001c`).
#[must_use]
pub fn height_for_density(density: ButtonDensity) -> f32 {
    match density {
        ButtonDensity::Compact => HEIGHT_COMPACT,
        ButtonDensity::Standard => HEIGHT_STANDARD,
    }
}

/// The persistent interaction state for a [`SplitButton`] widget (§3.1c, `UIF-ATOM-001c`).
///
/// Holds the [`SplitButtonHost`] state machine, the focus router's [`FocusableState`],
/// and the [`FocusState`] required for menu cascade trapping and keyboard navigation.
#[derive(Debug)]
pub struct SplitButtonState {
    /// The host state machine driving the cascade and primary activation controller.
    pub host: SplitButtonHost,
    /// The widget's keyboard focus state managed by iced's operation router.
    pub focusable: FocusableState,
    /// The menu cascade focus routing state.
    pub focus: FocusState,
}

impl SplitButtonState {
    /// Create a new resting split button state for the given `control` focus id (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn new(control: FocusId) -> Self {
        let mut focus = FocusState::new();
        focus.register_group(
            crate::focus::GroupId::new(format!("split-button:{}", control.as_str())),
            vec![crate::focus::FocusMember::new(control.as_str())],
        );
        Self {
            host: SplitButtonHost::new(control),
            focusable: FocusableState::new(),
            focus,
        }
    }
}

impl Default for SplitButtonState {
    fn default() -> Self {
        Self::new(FocusId::new("split-button"))
    }
}

/// One painted quad layer of a [`SplitButton`] in draw order (§3.1c, `UIF-ATOM-001c`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The rectangle the layer covers.
    pub bounds: Rectangle,
    /// What is painted inside [`bounds`](Self::bounds).
    pub fill: Color,
    /// The stroke drawn on the inside of [`bounds`](Self::bounds).
    pub border: Border,
}

impl Layer {
    /// The [`renderer::Quad`] this layer becomes.
    #[must_use]
    pub fn quad(&self) -> renderer::Quad {
        renderer::Quad {
            bounds: self.bounds,
            border: self.border,
            shadow: Shadow::default(),
            snap: SNAP_TO_PIXEL_GRID,
        }
    }
}

/// The primary action half's background layer with left rounded corners (§3.1c, `UIF-ATOM-001c`).
#[must_use]
pub fn primary_background_layer(bounds: Rectangle, style: &SplitStyle) -> Layer {
    Layer {
        bounds: primary_zone(bounds),
        fill: style.primary.background,
        border: Border::default()
            .color(style.primary.border_color)
            .width(style.primary.border_width)
            .rounded(iced::border::Radius {
                top_left: RADIUS,
                top_right: 0.0,
                bottom_right: 0.0,
                bottom_left: RADIUS,
            }),
    }
}

/// The menu half's background layer with right rounded corners (§3.1c, `UIF-ATOM-001c`).
#[must_use]
pub fn menu_background_layer(bounds: Rectangle, style: &SplitStyle) -> Layer {
    Layer {
        bounds: menu_zone(bounds),
        fill: style.menu.background,
        border: Border::default()
            .color(style.menu.border_color)
            .width(style.menu.border_width)
            .rounded(iced::border::Radius {
                top_left: 0.0,
                top_right: RADIUS,
                bottom_right: RADIUS,
                bottom_left: 0.0,
            }),
    }
}

/// The vertical divider line layer between the two halves (§3.1c, `UIF-ATOM-001c`).
#[must_use]
pub fn divider_layer(bounds: Rectangle, style: &SplitStyle) -> Layer {
    Layer {
        bounds: divider_bounds(bounds),
        fill: style.divider,
        border: Border::default(),
    }
}

/// The outer focus ring layer surrounding the split button, if focused (§3.1c, `UIF-ATOM-001c`).
#[must_use]
pub fn focus_ring_layer(bounds: Rectangle, style: &SplitStyle) -> Option<Layer> {
    if style.primary.focus_ring_width == NO_STROKE_WIDTH {
        return None;
    }

    Some(Layer {
        bounds: bounds.expand(style.primary.focus_ring_width),
        fill: Color::TRANSPARENT,
        border: Border::default()
            .color(style.primary.focus_ring_color)
            .width(style.primary.focus_ring_width)
            .rounded(RADIUS + style.primary.focus_ring_width),
    })
}

/// A live, interactive split button widget combining a primary action button with
/// an independent trailing chevron menu trigger (§3.1c, `UIF-ATOM-001c`).
pub struct SplitButton<'a, Message, ActionId = String> {
    focus_id: FocusId,
    widget_id: Id,
    model: Controlled<'a, SplitButtonModel<ActionId>>,
    palette: Palette,
    emit: Box<dyn Fn(SplitButtonEmission<ActionId>) -> Option<Message> + 'a>,
}

impl<'a, Message, ActionId> SplitButton<'a, Message, ActionId> {
    /// Build a **controlled** split button over a host-owned [`SplitButtonModel`]
    /// (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, SplitButtonModel<ActionId>>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(SplitButtonEmission<ActionId>) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);

        Self {
            focus_id,
            widget_id,
            model,
            palette: Palette::dark(),
            emit: Box::new(emit),
        }
    }

    /// Set the palette used to resolve colours (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Builder alias for [`palette`](Self::palette) (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn with_palette(self, palette: Palette) -> Self {
        self.palette(palette)
    }

    /// The focus id registered for this control (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced widget id corresponding to the focus id (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// The model driving this split button (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn model(&self) -> &SplitButtonModel<ActionId> {
        self.model.get()
    }

    /// The palette used by this split button (§3.1c, `UIF-ATOM-001c`).
    #[must_use]
    pub fn palette_ref(&self) -> &Palette {
        &self.palette
    }
}

/// Send emissions to the host callback and publish resulting messages to the shell.
fn route_emissions<Message, ActionId>(
    emit: &(dyn Fn(SplitButtonEmission<ActionId>) -> Option<Message> + '_),
    emissions: Vec<SplitButtonEmission<ActionId>>,
    shell: &mut Shell<'_, Message>,
) {
    for emission in emissions {
        if let Some(message) = emit(emission) {
            shell.publish(message);
        }
    }
}

impl<'a, Message, Theme, Renderer, ActionId> Widget<Message, Theme, Renderer>
    for SplitButton<'a, Message, ActionId>
where
    Message: 'a + Clone,
    Renderer: renderer::Renderer + text_core::Renderer,
    ActionId: Clone + fmt::Display + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SplitButtonState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SplitButtonState::new(self.focus_id.clone()))
    }

    fn size(&self) -> Size<Length> {
        let height = height_for_density(self.model.get().density());
        Size::new(Length::Shrink, Length::Fixed(height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let label = self.model.get().label();
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
        let height = height_for_density(self.model.get().density());
        let intrinsic = Size::new(control_width(label_width), height);
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
        let state = tree.state.downcast_mut::<SplitButtonState>();
        if !self.model.get().disabled_flag() {
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
        if !self.model.get().disabled_flag() && cursor.is_over(layout.bounds()) {
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

        let state = tree.state.downcast_mut::<SplitButtonState>();
        let model = self.model.get();

        state.host.observe(model);

        if state.focus.group_of(&self.focus_id).is_none() {
            state.focus.register_group(
                crate::focus::GroupId::new(format!("split-button:{}", self.focus_id.as_str())),
                vec![crate::focus::FocusMember::new(self.focus_id.as_str())],
            );
        }

        let bounds = layout.bounds();
        let zone = if let Some(pos) = cursor.position_over(bounds) {
            zone_at(pos.x - bounds.x, bounds.width)
        } else {
            SplitZone::Outside
        };

        state.host.hover(zone);

        if model.disabled_flag() {
            state.host.reset();
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if zone.is_hit() {
                    let outcome = state.host.press(zone, model);
                    route_emissions(&self.emit, outcome.emissions, shell);
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.host.pressed_zone().is_some() {
                    let outcome = state.host.release(zone, model, &mut state.focus);
                    route_emissions(&self.emit, outcome.emissions, shell);
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() =>
            {
                let outcome = match key {
                    Key::Named(Named::Enter) if state.host.is_menu_open() => {
                        state.host.menu_key(MenuKey::Enter, model, &mut state.focus)
                    }
                    Key::Named(Named::Space) if state.host.is_menu_open() => {
                        state.host.menu_key(MenuKey::Space, model, &mut state.focus)
                    }
                    Key::Named(Named::Enter) | Key::Named(Named::Space) => {
                        state.host.activate_primary(model)
                    }
                    Key::Named(Named::ArrowDown) if state.host.is_menu_open() => {
                        state.host.menu_key(MenuKey::Down, model, &mut state.focus)
                    }
                    Key::Named(Named::ArrowDown) => state.host.open_menu(model, &mut state.focus),
                    Key::Named(Named::ArrowUp) if state.host.is_menu_open() => {
                        state.host.menu_key(MenuKey::Up, model, &mut state.focus)
                    }
                    Key::Named(Named::Escape) => state.host.escape(model, &mut state.focus),
                    _ => SplitButtonOutcome::noop(),
                };
                if !outcome.is_noop() {
                    route_emissions(&self.emit, outcome.emissions, shell);
                    shell.capture_event();
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
        let state = tree.state.downcast_ref::<SplitButtonState>();
        let model = self.model.get();
        let bounds = layout.bounds();
        let focused = state.focusable.is_focused();

        let (primary_state, menu_state) = if model.disabled_flag() {
            (
                VisualState {
                    base: BaseState::Disabled,
                    focused: false,
                },
                VisualState {
                    base: BaseState::Disabled,
                    focused: false,
                },
            )
        } else {
            (
                state.host.half_state(SplitZone::Primary, focused),
                state.host.half_state(SplitZone::Menu, focused),
            )
        };

        let style = resolve_style(model.variant(), primary_state, menu_state, &self.palette);

        for layer in [
            Some(primary_background_layer(bounds, &style)),
            Some(menu_background_layer(bounds, &style)),
            Some(divider_layer(bounds, &style)),
            focus_ring_layer(bounds, &style),
        ]
        .into_iter()
        .flatten()
        {
            renderer.fill_quad(layer.quad(), Background::Color(layer.fill));
        }

        let label_rect = label_bounds(primary_zone(bounds));
        renderer.fill_text(
            text_core::Text {
                content: model.label().to_owned(),
                bounds: label_rect.size(),
                size: renderer.default_size(),
                line_height: LineHeight::Absolute(Pixels(label_rect.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            label_rect.center(),
            style.primary.foreground,
            *viewport,
        );

        let chev_rect = chevron_bounds(menu_zone(bounds));
        renderer.fill_text(
            text_core::Text {
                content: CHEVRON_GLYPH.to_owned(),
                bounds: chev_rect.size(),
                size: renderer.default_size(),
                line_height: LineHeight::Absolute(Pixels(chev_rect.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            chev_rect.center(),
            chevron_color(&style),
            *viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer, ActionId> From<SplitButton<'a, Message, ActionId>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer,
    ActionId: Clone + fmt::Display + 'a,
{
    fn from(button: SplitButton<'a, Message, ActionId>) -> Self {
        Element::new(button)
    }
}

#[cfg(test)]
#[path = "split_button_tests.rs"]
mod split_button_tests;
