//! The inline severity banner (`UIF-COMP-002`).
//!
//! An `InfoBar` is a statement the host puts inside its own layout — a
//! modal body, a pane, a settings card — that says *this is true right
//! now* and stays true until somebody says otherwise. That makes it the
//! opposite of a [`toast`](super::toast) in every way that matters: it
//! occupies layout, it owns no timer, and it cannot leave on its own.
//!
//! # The vertical rhythm cannot see what is docked at the end
//!
//! §3B.2 fixes the bar at full container width with a `36px` floor, and
//! `UIF-PRIN-003` forbids a state change from moving content. The
//! guarantee is stated as a signature rather than asserted:
//! [`bar_bounds`], [`icon_bounds`] and the text band's `y`/`height` take
//! **no** parameter for whether an action button or a `[×]` is present.
//! A bar with two trailing controls and a bar with none resolve the same
//! vertical boxes, so nothing below the bar can be pushed when a host
//! adds or removes a trailing affordance.
//!
//! The text band's *width* does depend on the trailing slots, and that is
//! honest: a shorter text band is a reflow inside a band whose position
//! did not move, not a moved band. The distinction is the one
//! `UIF-PRIN-003` actually draws.
//!
//! # The stripe is a second shape, not a border
//!
//! §3B.2 asks for a `4px` solid left accent stripe *beside* a `1px`
//! `border.strong` perimeter. They are separate layers rather than one
//! stroke with one side made thicker, because iced's border is uniform —
//! faking the stripe with a border would either fatten the whole
//! perimeter or need a second quad anyway. Painting the stripe as its own
//! layer is the honest version of what is already true, and it keeps the
//! stripe's colour free to be the severity's without dragging the
//! perimeter along.
//!
//! # Contrast is recomputed, not copied
//!
//! §2.5 prints contrast for `surface.base`, `surface.overlay` and
//! `surface.active` — and **not** for `surface.raised`, which is where
//! §3B.2 says the bar goes. So every figure below was computed from raw
//! sRGB by [`crate::token::contrast`] against the bar's own fill rather
//! than lifted from a printed column. Against `surface.raised`
//! (`RGB(18,18,20)`):
//!
//! | Foreground | Fill | Ratio |
//! | --- | --- | --- |
//! | `accent.text` (Info) | `surface.raised` | 10.0215:1 |
//! | `status.success` | `surface.raised` | 14.2825:1 |
//! | `status.warning` | `surface.raised` | 6.4474:1 |
//! | `status.danger.text` | `surface.raised` | 6.1812:1 |
//! | `text.strong` (title) | `surface.raised` | 18.7108:1 |
//! | `text.primary` (body) | `surface.raised` | 12.3755:1 |
//! | `text.muted` (dismiss) | `surface.raised` | 7.1553:1 |
//! | `border.strong` (perimeter) | `surface.raised` | 4.2379:1 |
//!
//! Every severity accent clears the `4.50:1` text floor with the worst
//! case at `6.1812:1`, and the perimeter clears the `3.00:1` chrome
//! floor. The same eight channels against `surface.overlay` are in the
//! sidecar; the lowest there is `status.danger.text` at `5.8528:1`, so
//! the bar is compliant on either fill §3B.2 permits.
//!
//! Unit tests live in the sibling `info_bar_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, GestureSlot, GestureStart};
use crate::composite::notification::{NotificationAction, NotificationId, Severity};
use crate::focus::{FocusId, FocusMember};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// The bar's minimum height — `size.control.lg` (§3B.2's `36px`).
///
/// A token read rather than a registered figure: §3B.2's `36px` *is*
/// `size.control.lg`, so no new constant needs a §-reference.
pub const MIN_HEIGHT: f32 = size::CONTROL_LG;

/// Vertical padding — `space.sm` (§3B.2's `8px 12px`, the `8px`).
pub const PADDING_Y: f32 = space::SM;

/// Horizontal padding — `space.md` (§3B.2's `8px 12px`, the `12px`).
pub const PADDING_X: f32 = space::MD;

/// The left accent stripe's width — `space.xs` (§3B.2's `4px`).
pub const STRIPE_WIDTH: f32 = space::XS;

/// The leading icon's box — `size.icon.md` (§3B.2's `16px`).
pub const ICON_BOX: f32 = size::ICON_MD;

/// Gap between the icon and the text band — `space.sm`.
pub const ICON_TEXT_GAP: f32 = space::SM;

/// Gap between a docked action button and the `[×]` — `space.sm`.
///
/// §3B.2 does not name a text-to-control gap; the bar's own `12px`
/// horizontal padding plus the `[×]`'s centred glyph cover the bare
/// case, so this constant exists only for the pair of controls that
/// genuinely sit side by side.
pub const ACTION_DISMISS_GAP: f32 = space::SM;

/// The perimeter border weight — `shape.border_hairline` (§3B.2's `1px`).
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The bar's corner radius — `radius.widget` (§3B.2's `4px`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// A trailing `[×]`'s hit box — `size.hit_target.min` (`24px`).
///
/// §3B.2 does not size the dismiss affordance, but §6's gate 2 does:
/// every interactive element measures at least `24×24`. Taking the
/// token rather than inventing a smaller quiet box keeps the bar inside
/// the gate without a registered exception.
pub const DISMISS_TARGET: f32 = size::HIT_TARGET_MIN;

/// The title's rung — `type.ui_compact`.
pub const TITLE_RUNG: &typography::TypeRung = &typography::UI_COMPACT;

/// The body's rung — `type.caption`.
pub const BODY_RUNG: &typography::TypeRung = &typography::CAPTION;

/// Which of §3B.2's two permitted bar fills a bar uses.
///
/// §3B.2 writes "`BG: surface.raised` (or `surface.overlay`)" without
/// saying which is which, so the choice is the host's and is modelled
/// rather than resolved behind its back. `Raised` is the default because
/// it is the surface the bar is *docked into* — a modal body and a pane
/// are both raised surfaces — while `overlay` is for a bar floating over
/// content it does not own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BarSurface {
    /// `surface.raised` — docked inside a pane or modal body.
    #[default]
    Raised,
    /// `surface.overlay` — floating above content the bar does not own.
    Overlay,
}

impl BarSurface {
    /// The fill this surface resolves to.
    #[must_use]
    pub fn fill(self, palette: &Palette) -> Color {
        match self {
            Self::Raised => palette.surface_raised,
            Self::Overlay => palette.surface_overlay,
        }
    }

    /// The lowercase slug used in cell ids and manifests.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Raised => "raised",
            Self::Overlay => "overlay",
        }
    }
}

/// The bar box for a container.
///
/// Full container width, and at least [`MIN_HEIGHT`] tall. The trailing
/// slots are **not** parameters: the box is the same whether or not
/// anything is docked at the end, which is the zero-shift guarantee for
/// everything below the bar.
#[must_use]
pub fn bar_bounds(container: Rectangle) -> Rectangle {
    Rectangle {
        x: container.x,
        y: container.y,
        width: container.width.max(0.0),
        height: container.height.max(MIN_HEIGHT),
    }
}

/// The `4px` accent stripe: the bar's full height at its left edge.
///
/// The stripe spans the whole height rather than sitting inside the
/// padding, so a taller bar shows more of it instead of leaving the
/// corners unpainted — a stripe that stops short reads as a stray mark
/// rather than as the severity's edge.
#[must_use]
pub fn stripe_bounds(bar: Rectangle) -> Rectangle {
    Rectangle {
        x: bar.x,
        y: bar.y,
        width: STRIPE_WIDTH.min(bar.width.max(0.0)),
        height: bar.height,
    }
}

/// The bar's content box: inside the padding, past the stripe.
#[must_use]
pub fn content_bounds(bar: Rectangle) -> Rectangle {
    let x = bar.x + STRIPE_WIDTH + PADDING_X;
    let right = bar.x + bar.width - PADDING_X;
    Rectangle {
        x,
        y: bar.y + PADDING_Y,
        width: (right - x).max(0.0),
        height: (bar.height - PADDING_Y * 2.0).max(0.0),
    }
}

/// The leading icon's box, vertically centred at the content's leading
/// edge.
///
/// Vertically centred in the **bar** rather than in the content box so a
/// two-line body and a one-line title share one icon position instead of
/// the icon drifting with the text's line count.
#[must_use]
pub fn icon_bounds(bar: Rectangle) -> Rectangle {
    Rectangle {
        x: bar.x + STRIPE_WIDTH + PADDING_X,
        y: bar.y + (bar.height - ICON_BOX) / 2.0,
        width: ICON_BOX,
        height: ICON_BOX,
    }
}

/// The text band: after the icon, before the trailing slots.
#[must_use]
pub fn text_bounds(bar: Rectangle, trailing_width: f32) -> Rectangle {
    let content = content_bounds(bar);
    let x = content.x + ICON_BOX + ICON_TEXT_GAP;
    let right = content.x + content.width - trailing_width.max(0.0);
    Rectangle {
        x,
        y: content.y,
        width: (right - x).max(0.0),
        height: content.height,
    }
}

/// The trailing `[×]`'s hit box, vertically centred in the bar.
#[must_use]
pub fn dismiss_bounds(bar: Rectangle) -> Rectangle {
    Rectangle {
        x: bar.x + bar.width - PADDING_X - DISMISS_TARGET,
        y: bar.y + (bar.height - DISMISS_TARGET) / 2.0,
        width: DISMISS_TARGET,
        height: DISMISS_TARGET,
    }
}

/// The action button's box, docked to the left of the `[×]`.
///
/// `dismiss_present` shifts the action left rather than shrinking it:
/// two controls that overlapped because a `[×]` appeared would be worse
/// than a text band that gave up some width.
#[must_use]
pub fn action_bounds(bar: Rectangle, action_width: f32, dismiss_present: bool) -> Rectangle {
    let right = bar.x + bar.width
        - PADDING_X
        - if dismiss_present {
            DISMISS_TARGET + ACTION_DISMISS_GAP
        } else {
            0.0
        };
    let width = action_width.max(0.0);
    Rectangle {
        x: (right - width).max(bar.x),
        y: bar.y + (bar.height - size::CONTROL_SM) / 2.0,
        width,
        height: size::CONTROL_SM.min(bar.height.max(0.0)),
    }
}

/// How much trailing room the docked controls reserve.
#[must_use]
pub fn trailing_width(has_action: bool, action_width: f32, has_dismiss: bool) -> f32 {
    let dismiss = if has_dismiss { DISMISS_TARGET } else { 0.0 };
    let action = if has_action {
        action_width.max(0.0) + ACTION_DISMISS_GAP
    } else {
        0.0
    };
    // The gap beside a lone `[×]` belongs to the padding, not to a second
    // control, so it is not double-counted when no action is docked.
    (dismiss + action).max(0.0)
}

/// Everything docked inside one bar, resolved once.
///
/// A struct rather than six calls so a caller cannot mix a `dismiss`
/// rectangle computed with `has_dismiss = true` against a text band
/// computed with it `false`. The slots are consistent by construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarSlots {
    /// The bar box.
    pub bar: Rectangle,
    /// The `4px` severity stripe.
    pub stripe: Rectangle,
    /// The `16px` leading icon.
    pub icon: Rectangle,
    /// The title/body band.
    pub text: Rectangle,
    /// The action button, when one is docked.
    pub action: Option<Rectangle>,
    /// The `[×]`, when one is offered.
    pub dismiss: Option<Rectangle>,
}

/// Resolve every slot in one bar.
#[must_use]
pub fn slots(
    container: Rectangle,
    action: Option<&NotificationAction<impl Sized>>,
    action_width: f32,
    has_dismiss: bool,
) -> BarSlots {
    let bar = bar_bounds(container);
    let trailing = trailing_width(action.is_some(), action_width, has_dismiss);
    BarSlots {
        bar,
        stripe: stripe_bounds(bar),
        icon: icon_bounds(bar),
        text: text_bounds(bar, trailing),
        action: action.map(|_| action_bounds(bar, action_width, has_dismiss)),
        dismiss: has_dismiss.then(|| dismiss_bounds(bar)),
    }
}

/// A bar's fully resolved paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfoBarStyle {
    /// The bar fill.
    pub background: Color,
    /// The perimeter stroke.
    pub border_colour: Color,
    /// The perimeter weight.
    pub border_width: f32,
    /// The left accent stripe.
    pub stripe_colour: Color,
    /// The leading icon.
    pub icon_colour: Color,
    /// The title text.
    pub title_colour: Color,
    /// The body text.
    pub body_colour: Color,
    /// The `[×]` glyph.
    pub dismiss_colour: Color,
}

/// Resolve a bar's paint from its severity and fill.
///
/// The severity colours the stripe and the icon and **nothing else**. The
/// title stays `text.strong` and the body stays `text.primary` at every
/// severity, because tinting body copy red or amber makes it read as
/// decoration rather than as something to read, and because a severity
/// carried by two small marks plus the glyph is already carried twice
/// (`UIF-PRIN-005`).
#[must_use]
pub fn resolve_style(severity: Severity, surface: BarSurface, palette: &Palette) -> InfoBarStyle {
    let accent = severity.accent(palette);
    InfoBarStyle {
        background: surface.fill(palette),
        border_colour: palette.border_strong,
        border_width: BORDER_WIDTH,
        stripe_colour: accent,
        icon_colour: accent,
        title_colour: palette.text_strong,
        body_colour: palette.text_primary,
        dismiss_colour: palette.text_muted,
    }
}

/// The bar's fill-and-perimeter layer.
#[must_use]
pub fn background_layer(bar: Rectangle, style: &InfoBarStyle) -> Layer {
    Layer {
        bounds: bar,
        fill: style.background,
        border: iced::Border {
            color: style.border_colour,
            width: style.border_width,
            radius: RADIUS.into(),
        },
    }
}

/// The severity stripe's layer.
///
/// Stroked with nothing: the stripe *is* a fill. Giving it a border too
/// would draw a second perimeter inside the bar's own.
#[must_use]
pub fn stripe_layer(stripe: Rectangle, style: &InfoBarStyle) -> Layer {
    Layer {
        bounds: stripe,
        fill: style.stripe_colour,
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0_f32.into(),
        },
    }
}

/// The tab stops a bar contributes.
///
/// The bar itself is **not** a focus stop — it is a statement, not a
/// control, and a tab stop that does nothing is a stop that wastes a
/// keystroke. Only the action button and the `[×]` are members, and only
/// when they are present.
#[must_use]
pub fn focus_members(id: &NotificationId, has_action: bool, has_dismiss: bool) -> Vec<FocusMember> {
    let mut members = Vec::new();
    if has_action {
        members.push(FocusMember::new(action_focus_id(id)));
    }
    if has_dismiss {
        members.push(FocusMember::new(dismiss_focus_id(id)));
    }
    members
}

/// The action button's focus id for a notification.
#[must_use]
pub fn action_focus_id(id: &NotificationId) -> FocusId {
    FocusId::new(format!("{}:action", id.as_str()))
}

/// The `[×]`'s focus id for a notification.
#[must_use]
pub fn dismiss_focus_id(id: &NotificationId) -> FocusId {
    FocusId::new(format!("{}:dismiss", id.as_str()))
}

/// Which part of a bar the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarPart {
    /// The action button.
    Action,
    /// The `[×]`.
    Dismiss,
}

/// What releasing a bar's trailing control settled.
///
/// Carries the part alongside the commit because the two answer different
/// questions: the commit says the bar is gone, the part says *which
/// control* the user pressed to make it go — and a host that has to fire
/// `on_action` versus `on_dismiss` cannot tell them apart from a bare
/// `true`.
///
/// Not `PartialEq`: [`Commit`] deliberately carries no equality, because
/// two commits of the same value from different gestures are *not* the
/// same event, and deriving equality here would say they were.
#[derive(Clone, Debug)]
pub struct BarRelease {
    /// The control the press was held on.
    pub part: BarPart,
    /// The settled dismissal flag.
    pub commit: Commit<bool>,
}

/// The live-and-committed dismissal state of one docked bar.
///
/// The same [`GestureSlot`] transcript the rest of the crate produces:
/// a press captures what the user was shown, a release settles it, a
/// cancel reverts. A bar has no continuous value, so there is no `Live`
/// tier — the press is already visible as the button's own fill.
#[derive(Clone, Debug, Default)]
pub struct BarDismissal {
    dismissed: bool,
    slot: GestureSlot<bool>,
    pressed: Option<BarPart>,
}

impl BarDismissal {
    /// A bar at a settled visibility.
    #[must_use]
    pub fn new(dismissed: bool) -> Self {
        Self {
            dismissed,
            slot: GestureSlot::new(),
            pressed: None,
        }
    }

    /// Whether the bar is currently dismissed.
    #[must_use]
    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }

    /// The part currently under a press, if any.
    #[must_use]
    pub fn pressed_part(&self) -> Option<BarPart> {
        self.pressed
    }

    /// Open the gesture slot against the value the user is shown.
    ///
    /// A second press while one is open is refused rather than layered:
    /// the first capture keeps governing, per [`GestureSlot`]'s rule.
    /// The refused part is *not* remembered, so a release settles the
    /// control the user actually pressed first.
    #[must_use]
    pub fn press(&mut self, part: BarPart) -> GestureStart {
        let start = self.slot.begin(&self.dismissed);
        if start == GestureStart::Started {
            self.pressed = Some(part);
        }
        start
    }

    /// Release: dismiss the bar and settle the slot, exactly once.
    ///
    /// `None` with no state change when nothing was pressed, so a stray
    /// release cannot dismiss a bar.
    #[must_use]
    pub fn release(&mut self) -> Option<BarRelease> {
        let part = self.pressed.take()?;
        let commit = self.slot.commit(true)?;
        self.dismissed = true;
        Some(BarRelease { part, commit })
    }

    /// Escape the pending press: revert to the captured value.
    #[must_use]
    pub fn cancel(&mut self) -> Option<Cancel<bool>> {
        self.pressed.take()?;
        let outcome = self.slot.cancel()?;
        self.dismissed = *outcome.restored();
        Some(outcome)
    }

    /// Dismiss or restore the bar directly, reporting the commit.
    ///
    /// For keyboard activation, where there is a decision but no press
    /// to release. A slot left open by a stuck press is settled through
    /// the same path so the transcript cannot dangle.
    #[must_use]
    pub fn set_dismissed(&mut self, dismissed: bool) -> Commit<bool> {
        self.dismissed = dismissed;
        self.pressed = None;
        match self.slot.commit(dismissed) {
            Some(commit) => commit,
            None => Commit::new(dismissed),
        }
    }
}

#[cfg(test)]
#[path = "info_bar_tests.rs"]
mod info_bar_tests;
