//! The hyperlink resolver, geometry and focus state — the inline
//! interactive text control of §3.1d (`UIF-ATOM-001d`).
//!
//! A hyperlink is not a flat button. It has no border, its resting fill is
//! absent rather than a surface, and its primary affordance is a two-pixel
//! underline rather than a perimeter. So this module does not borrow the
//! button resolver — there is nothing in §3.1's matrix it could reuse
//! without inventing a fifth variant the spec does not list. What it does
//! reuse is every rule that is *not* about the shape: the token set, the
//! adaptive ring, the state resolver, and the focus wrapper.
//!
//! # The ring is resolved against the mount
//!
//! At rest the control paints nothing behind its label, so the ring is
//! drawn over whatever the link sits on. Resolving it against the link's
//! own (transparent) background would ask the adaptive rule to judge a
//! colour that is not there. So [`resolve_style`] takes the `mount` the
//! link is drawn on and resolves against that — the same decision the
//! segmented strip made for the same reason. The hover pill is inside the
//! ring, not under it, so the mount stays the right answer in every state.
//!
//! # The underline is the affordance, and it leaves when the control dies
//!
//! §3.1d gives the resting, hovered and pressed states an underline and
//! the disabled state none. That is the whole non-colour cue for a link:
//! a greyed label that is also un-underlined cannot be mistaken for a live
//! one. [`HyperlinkStyle::underlined`] is therefore resolved from the
//! state rather than left to the renderer, and it is `false` in exactly
//! one cell.
//!
//! # Contrast is quoted against two mounts because the control lives on two
//!
//! §3.1d prints `accent.text` at `10.66:1` on `base` and `9.49:1` on
//! `overlay`. A link is the one atom in §3.1 that is specified against
//! two different surfaces in a single cell, because it is the one atom
//! that appears both in body copy and on floating surfaces. Both figures
//! are recomputed in the sidecar from the raw sRGB rather than trusted.
//!
//! # Focus is the iced router's, through [`FocusableState`]
//!
//! [`HyperlinkState`] implements [`operation::Focusable`] by delegating
//! to the crate's existing [`FocusableState`] rather than carrying its own
//! `is_focused` bit. The router therefore drives the link exactly the way
//! it drives every other focusable wrapper, and the hover and press
//! signals that sit beside it are pointer observations, not focus ones.
//!
//! Unit tests live in the sibling `hyperlink_tests.rs` sidecar.

use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::focusable::FocusableState;
use iced::advanced::widget::operation;
use iced::{Color, Point, Rectangle, Size};

/// The minimum hit-target height — `min_height: 24px` (§3.1d,
/// `size.hit_target.min`).
pub const MIN_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// The focus ring's corner radius — `radius.widget` (§3.1d).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The focus ring width — `border.focus` (§3.0.2).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The underline stroke width — the `1px` underline of §3.1d.
pub const UNDERLINE_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Horizontal padding around the label.
///
/// Deliberately the smallest token in the scale: a hyperlink is inline
/// content, and padding wide enough to look like a control would break
/// the line it sits in. It exists so the underline does not touch the
/// neighbouring glyph.
pub const PADDING_X: f32 = space::XXS;

/// No ring is drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// No ring is drawn.
const NO_RING_WIDTH: f32 = 0.0;

/// A hyperlink's resolved paint for one state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperlinkStyle {
    /// The label colour.
    pub foreground: Color,
    /// The pill fill behind the label — transparent wherever §3.1d paints
    /// none.
    pub background: Color,
    /// Whether the underline is drawn.
    pub underlined: bool,
    /// The focus ring colour — transparent when no ring is drawn.
    pub focus_ring_color: Color,
    /// The focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
}

impl HyperlinkStyle {
    /// Whether nothing is painted behind the label.
    #[must_use]
    pub const fn is_transparent_background(&self) -> bool {
        self.background.a == 0.0
    }
}

/// Resolve the paint for one state against the surface the link is drawn
/// on (`UIF-ATOM-001d`).
///
/// `Invalid` shares the `Rest` cell: §3.1d gives a link no error skin, and
/// a link has no value to be invalid in the first place.
#[must_use]
pub fn resolve_style(state: VisualState, mount: Color, palette: &Palette) -> HyperlinkStyle {
    let (foreground, background, underlined) = match state.base {
        BaseState::Rest => (palette.accent_text, Color::TRANSPARENT, true),
        BaseState::Hovered => (palette.text_strong, palette.surface_hover, true),
        BaseState::Pressed => (palette.accent_text, palette.surface_active, true),
        BaseState::Disabled => (palette.text_disabled, Color::TRANSPARENT, false),
        // §3.1d gives a link no error skin, and a link has no value that
        // could fail validation; `Invalid` reads as `Rest`.
        BaseState::Invalid => (palette.accent_text, Color::TRANSPARENT, true),
    };

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            crate::widget::focus::resolve_ring_color_for_background(mount, palette),
            FOCUS_RING_WIDTH,
        )
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    HyperlinkStyle {
        foreground,
        background,
        underlined,
        focus_ring_color,
        focus_ring_width,
    }
}

/// The size a link needs for a label of `label_width`.
///
/// The height is the hit target, not the text: §3.1d asks for `24px`
/// minimum so a link in a line of body copy is still clickable, and a
/// taller mount does not shrink it.
#[must_use]
pub fn control_size(label_width: f32) -> Size {
    Size::new(label_width.max(0.0) + PADDING_X * 2.0, MIN_HEIGHT)
}

/// The label rectangle inside a link's hit box.
#[must_use]
pub fn label_bounds(control: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(control.x + PADDING_X, control.y),
        Size::new((control.width - PADDING_X * 2.0).max(0.0), control.height),
    )
}

/// The underline's rectangle.
///
/// It spans the **label**, not the hit box, so the rule never runs out
/// past the last glyph into the padding — the padding exists precisely to
/// keep it away from the neighbouring character.
///
/// The exact baseline is a renderer concern that depends on font metrics
/// the atom does not have; what the atom fixes is the horizontal extent
/// and the stroke width, which is what `UIF-PRIN-003` can actually check.
#[must_use]
pub fn underline_bounds(control: Rectangle) -> Rectangle {
    let label = label_bounds(control);
    Rectangle::new(
        Point::new(label.x, control.y + control.height - UNDERLINE_WIDTH),
        Size::new(label.width, UNDERLINE_WIDTH),
    )
}

/// The per-control state a hyperlink carries: the focus wrapper's bit,
/// plus the two pointer observations that drive its paint.
///
/// The focus half is [`FocusableState`], delegated rather than
/// reimplemented, so `operation::Focusable` behaves here exactly as it
/// does on every other wrapper in the crate. Hover and press are plain
/// pointer facts and are *not* allowed to imply focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HyperlinkState {
    focusable: FocusableState,
    hovered: bool,
    pressed: bool,
}

impl HyperlinkState {
    /// A link that holds neither focus nor the pointer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focusable: FocusableState::new(),
            hovered: false,
            pressed: false,
        }
    }

    /// Whether this link currently holds keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focusable.is_focused()
    }

    /// The focus wrapper half, read-only.
    #[must_use]
    pub const fn focusable(&self) -> &FocusableState {
        &self.focusable
    }

    /// Whether the pointer is inside the hit box.
    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a button is held on the link.
    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Move the pointer in or out.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Press or release the pointer on the link.
    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    /// Resolve what this link should paint, given whether it is live.
    ///
    /// Goes through the crate-wide [`resolve`] so the precedence — a dead
    /// link shows the disabled skin and holds no ring, whatever the
    /// pointer or the router were doing — is the one rule every atom
    /// shares rather than a local copy of it.
    #[must_use]
    pub fn visual_state(&self, enabled: bool) -> VisualState {
        resolve(InteractionInput {
            enabled,
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.focusable.is_focused(),
            invalid: false,
        })
    }

    /// Drop the pointer signals without touching focus.
    pub fn release_pointer(&mut self) {
        self.hovered = false;
        self.pressed = false;
    }
}

impl operation::Focusable for HyperlinkState {
    fn is_focused(&self) -> bool {
        self.focusable.is_focused()
    }

    fn focus(&mut self) {
        self.focusable.focus();
    }

    fn unfocus(&mut self) {
        self.focusable.unfocus();
    }
}

#[cfg(test)]
#[path = "hyperlink_tests.rs"]
mod hyperlink_tests;
