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

#[cfg(test)]
#[path = "split_button_tests.rs"]
mod split_button_tests;
