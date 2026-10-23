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
//! §3.1c is silent on the chevron's colour. §3.4's combobox trigger is
//! the nearest precedent and paints its glyph `text.muted` at rest — but
//! `text.muted` on `accent.fill` measures `1.8363:1`, which fails
//! outright, so the precedent cannot be lifted onto a Primary split
//! button. The rule that *does* travel is §3.1's: a button's icons are
//! painted with its [`foreground`](ButtonStyle::foreground) channel, the
//! same colour as its label. So the chevron is [`chevron_color`], which
//! is the menu half's resolved foreground — `text.strong` (`4.8020:1`)
//! on a Primary fill, `text.primary` on a Secondary one.
//!
//! # The divider is invisible on a Primary fill — and that is a spec fact
//!
//! §3.1c names the divider `1px border.strong`, and that is what
//! [`divider_color`] returns. Against `accent.fill` (`RGB(93,96,238)`)
//! `border.strong` (`RGB(120,120,120)`) measures **`1.0876:1`** — the
//! divider cannot be seen on a filled primary half. It is legible on
//! every Secondary rung (`4.5062:1` on `surface.base`, `3.6772:1` on
//! `surface.hover`).
//!
//! This is recorded rather than fixed. The spec assigns the token
//! explicitly, the task forbids changing any rendered colour, and the
//! alternative — resolving the divider adaptively the way the focus ring
//! is — would invent a rule §3.1c does not contain. It is a spec gap:
//! §3.1c needs either a per-variant divider or a gap between the halves.
//! For the same reason the divider is deliberately **not** offered to the
//! gallery as a judged contrast pair; the number is reported in the cell's
//! `resolved` note instead of being scored against a floor it was never
//! drawn to meet.
//!
//! # The divider belongs to the chevron half
//!
//! [`zone_at`] gives the divider's pixel to [`SplitZone::Menu`]. A 1px
//! aim error on a shared edge should open a menu, which is reversible,
//! rather than fire the primary action, which is not.
//!
//! Unit tests live in the sibling `split_button_tests.rs` sidecar.

use crate::state::VisualState;
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button::{self, ButtonStyle, ButtonVariant};
use iced::{Color, Point, Rectangle, Size};

/// The trailing chevron zone's width — the `28px`-wide `▾` zone of
/// §3.1c (`UIF-ATOM-001c`).
///
/// Equal to `size.control.sm`, so the zone is square at the compact
/// height and a tall narrow rectangle at the standard one. It is a
/// *fixed* width, not a proportion: the spec gives the chevron zone a
/// number and the label zone the remainder.
pub const CHEVRON_ZONE_WIDTH: f32 = size::CONTROL_SM;

/// The vertical divider between the halves — `1px border.strong`
/// (§3.1c).
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
        Point::new(x, box_.y),
        Size::new(box_.width - x, box_.height),
    )
}

/// The divider's own rectangle within a control box.
#[must_use]
pub fn divider_bounds(box_: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(divider_x(box_.width), box_.y),
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
        divider: divider_color(palette),
    }
}

/// The divider colour — `border.strong`, as §3.1c names it.
#[must_use]
pub const fn divider_color(palette: &Palette) -> Color {
    palette.border_strong
}

/// The `▾` glyph colour: the chevron half's own foreground.
///
/// See the [module docs](self#the-chevron-takes-the-halves-own-foreground)
/// for why §3.4's muted-glyph precedent cannot be used here.
#[must_use]
pub fn chevron_color(style: &SplitStyle) -> Color {
    style.menu.foreground
}

#[cfg(test)]
#[path = "split_button_tests.rs"]
mod split_button_tests;
