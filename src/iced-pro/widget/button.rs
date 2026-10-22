//! The button atom — four variants across the resolved interaction state
//! matrix (`UIF-ATOM-001`).
//!
//! This module resolves paint; it does not build a widget.
//! [`resolve_style`] is a pure function of `(variant, state, palette)`
//! returning a [`ButtonStyle`] value, which is what makes every cell of
//! §3.1 checkable without a window, a renderer or a GPU. The iced
//! wrapper that turns a [`ButtonStyle`] into an `Element` is `INC-07b`
//! and is deliberately absent here.
//!
//! # The matrix
//!
//! §3.1 gives four variants across five painted cells. [`resolve_style`]
//! *is* that table, with every colour read from a [`Palette`] token and no
//! colour literal of its own beyond the pressed-fill opacity the spec
//! itself states ([`PRESSED_FILL_ALPHA`]).
//!
//! | Variant | Rest | Hovered | Pressed | Disabled |
//! |---|---|---|---|---|
//! | Primary | `accent.primary` / `text.strong` / `accent.primary` | `accent.primary` / `text.strong` / `text.strong` | `accent.primary` @ 85% / `text.strong` / `accent.primary` | `surface.raised` / `text.disabled` / `border.subtle` |
//! | Secondary | `surface.raised` / `text.primary` / `border.strong` | `surface.hover` / `text.strong` / `border.strong` | `surface.active` / `text.strong` / `accent.primary` | `surface.base` / `text.disabled` / `border.subtle` |
//! | Ghost | *transparent* / `text.primary` / *transparent* | `surface.hover` / `text.strong` / `border.subtle` | `surface.active` / `text.strong` / `border.strong` | *transparent* / `text.disabled` / *transparent* |
//! | Danger | `surface.raised` / `status.danger` / `status.danger` | `status.danger` / `text.strong` / `text.strong` | `status.danger` @ 85% / `text.strong` / `status.danger` | `surface.base` / `text.disabled` / `border.subtle` |
//!
//! # Focus is added, never substituted
//!
//! Every *Keyboard Focused* cell of §3.1 reads "Rest **+** 2px … focus
//! ring", and [`crate::state`] models focus as an orthogonal ring for
//! exactly that reason. The resolver follows the same shape: the base
//! state fixes background, foreground and border, and `focused` only
//! sets the ring. `Hovered + focused` therefore keeps the hover fill
//! *and* gains a ring rather than being pulled back to `Rest` — which is
//! what the spec's own wording demands, and what a flat six-value state
//! enum could not express without contradicting §4.9.2.
//!
//! The ring colour is variant-specific: Primary wears a `text.strong`
//! ring, the other three an `accent.primary` one.
//!
//! # The pressed fill is translucent, and that changes what is measured
//!
//! Primary and Danger specify their pressed fill at 85% opacity.
//! [`PRESSED_FILL_ALPHA`] is the only non-token magnitude in this file
//! and it is a number the spec itself prints; it is applied by replacing
//! the token's alpha while keeping its channels.
//!
//! Because that fill is translucent, the surface a pressed label actually
//! sits on is the fill composited over whatever the button is mounted to,
//! not the token itself.
//! [`contrast_ratio`](crate::token::contrast::contrast_ratio) deliberately
//! ignores alpha, so measuring the raw token reports the *opaque* pairing
//! — and for Danger the two answers straddle the readability line. The
//! sidecar audit measures the composited colour, per the rule
//! [`contrast`](crate::token::contrast) states: a translucent token
//! measured before it is laid down describes a colour that never appears.
//!
//! # `Invalid` has no cell, and that is recorded rather than glossed
//!
//! [`BaseState::Invalid`] exists in the state machine, but §3.1 gives a
//! button no invalid skin: validation is a §3.2 input concern and no
//! button in the spec validates anything. The resolver paints `Invalid`
//! exactly as `Rest` rather than inventing a cell the spec does not have.
//! That is a recorded gap in the spec's coverage of the atom, not a
//! silent fall-through — the sidecar pins it so a future invalid-button
//! requirement has to be added deliberately rather than discovered by
//! accident when a control starts painting the wrong thing.
//!
//! Unit tests live in the sibling `button_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::{color::Palette, shape, size, space};
use iced::Color;

/// The compact button height — `size.control.sm`, for the navbar and
/// dense toolbars (§3.1, `UIF-TOK-002`).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// The standard button height — `size.control.md`, for dialog CTA buttons
/// and forms (§3.1, `UIF-TOK-002`).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// Horizontal padding inside a button — `space.md` (§3.1, `UIF-TOK-001`).
pub const HORIZONTAL_PADDING: f32 = space::MD;

/// Gap between a leading icon and its label — `space.sm` (§3.1,
/// `UIF-TOK-001`).
pub const ICON_LABEL_GAP: f32 = space::SM;

/// Corner radius of every button — `radius.widget` (§3.1, `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// Resting border width of every button — `border.hairline` (§3.1,
/// `UIF-TOK-003`).
///
/// The width is constant across the base states on purpose: a button that
/// thickened its border on hover would move its own label, which
/// `UIF-PRIN-003` forbids.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Focus ring width — `border.focus` (§3.1, `UIF-TOK-003`).
///
/// Twice [`BORDER_WIDTH`], so a focused control can never read as one
/// that merely has a heavier border.
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// Minimum width of an icon-only button — `size.control.sm` (§3.1).
///
/// Stricter than `size.hit_target.min` (`24`), which is the general
/// floor for any interactive control. Ledger gap **G-2** settles the
/// pair: both hold, and this atom rule is the one a navbar-class control
/// must satisfy.
pub const ICON_ONLY_MIN_WIDTH: f32 = size::CONTROL_SM;

/// Minimum height of an icon-only button — `size.control.sm` (§3.1).
pub const ICON_ONLY_MIN_HEIGHT: f32 = size::CONTROL_SM;

/// Alpha a Primary or Danger pressed fill is painted at — the `85%`
/// opacity §3.1 states for those two cells.
///
/// This is the one non-token magnitude in the widget layer, and it is a
/// number the spec itself prints rather than a magic constant: §3.1
/// writes the pressed fill as "`accent.primary` (85% opacity)" and
/// "`status.danger` (85% opacity)". It is named rather than written
/// inline at the two sites because `RUST-021` forbids repeating a bare
/// literal, and because a pressed fill that drifted to a different
/// opacity in one variant and not the other would be a silent spec break
/// no test would catch.
pub const PRESSED_FILL_ALPHA: f32 = 0.85;

/// The width of a focus ring that is not being drawn.
///
/// Not a visual magnitude but the *absence* of one, which is why it is
/// allowed outside the token modules — and named, so `resolve_style`
/// reads as "no ring" rather than "zero point zero".
const NO_RING_WIDTH: f32 = 0.0;

/// The colour of a focus ring that is not being drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// The `transparent` §3.1 writes for the Ghost variant's resting and
/// disabled fill and border.
///
/// A separate name from [`NO_RING_COLOR`] even though both are
/// `Color::TRANSPARENT`: one is "no ring is drawn", the other is "no
/// fill is painted". Collapsing them into a single constant would make
/// `ghost_paint` read as though it were switching a ring off.
const TRANSPARENT: Color = Color::TRANSPARENT;

/// The four button variants of §3.1 (`UIF-ATOM-001`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// The primary call to action — filled with
    /// [`accent_primary`](Palette::accent_primary) and carrying the
    /// `UIF-TOK-006` Selection Foreground Lock.
    Primary,
    /// A bordered, surface-filled control for secondary actions.
    Secondary,
    /// A quiet control with no fill at rest, for toolbar and inline
    /// actions.
    Ghost,
    /// A destructive action, resting quiet and escalating to a
    /// [`status_danger`](Palette::status_danger) fill on hover.
    Danger,
}

/// A button's fully resolved paint for one `(variant, state)` pair.
///
/// Every field is a value the renderer consumes as-is: no token lookup,
/// no conditional, no further resolution. A background of
/// [`Color::TRANSPARENT`] means *nothing is painted behind the label* —
/// the label then sits on whatever the button is mounted to, which is why
/// [`is_transparent_background`](Self::is_transparent_background) exists
/// rather than a boolean variant of the style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonStyle {
    /// The fill behind the label.
    pub background: Color,
    /// The label and icon colour.
    pub foreground: Color,
    /// The perimeter stroke colour.
    pub border_color: Color,
    /// The perimeter stroke width, in logical pixels.
    pub border_width: f32,
    /// The keyboard focus ring colour — fully transparent when no ring
    /// is drawn.
    pub focus_ring_color: Color,
    /// The keyboard focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
}

impl ButtonStyle {
    /// Whether nothing is painted behind the label.
    ///
    /// True for the Ghost variant's `Rest` and `Disabled` cells, where
    /// §3.1 specifies `transparent`. A renderer must still lay out the
    /// same box — the fill is absent, not the geometry
    /// (`UIF-PRIN-003`) — and anything measuring the label's contrast
    /// has to ask the *container* what is behind it, because this style
    /// does not know.
    #[must_use]
    pub const fn is_transparent_background(&self) -> bool {
        self.background.a == 0.0
    }
}

/// Resolve the paint for one `(variant, state)` pair against a palette
/// (`UIF-ATOM-001`).
///
/// A pure function: the same three inputs always produce the same
/// [`ButtonStyle`], with no memory of the previous call. The base half of
/// `state` selects the background, foreground and border from §3.1; the
/// `focused` half adds the ring on top without touching them. See the
/// [module docs](self) for the matrix and for why focus is additive.
#[must_use]
pub const fn resolve_style(
    variant: ButtonVariant,
    state: VisualState,
    palette: &Palette,
) -> ButtonStyle {
    let base = base_paint(variant, state.base, palette);

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (focus_ring_color(variant, palette), FOCUS_RING_WIDTH)
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    ButtonStyle {
        background: base.background,
        foreground: base.foreground,
        border_color: base.border_color,
        border_width: BORDER_WIDTH,
        focus_ring_color,
        focus_ring_width,
    }
}

/// The three paint channels a base state fixes, before the ring is added.
struct BasePaint {
    background: Color,
    foreground: Color,
    border_color: Color,
}

/// Select the base-state triple for a variant.
///
/// Split per variant rather than matched as a `(variant, state)` cross
/// product so each variant's column of §3.1 reads top to bottom in one
/// place (`RUST-023`).
const fn base_paint(variant: ButtonVariant, base: BaseState, palette: &Palette) -> BasePaint {
    match variant {
        ButtonVariant::Primary => primary_paint(base, palette),
        ButtonVariant::Secondary => secondary_paint(base, palette),
        ButtonVariant::Ghost => ghost_paint(base, palette),
        ButtonVariant::Danger => danger_paint(base, palette),
    }
}

/// §3.1 — Primary.
const fn primary_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        // `Invalid` shares the `Rest` cell: §3.1 gives a button no
        // invalid skin. See the module doc.
        BaseState::Rest | BaseState::Invalid => BasePaint {
            background: palette.accent_primary,
            foreground: palette.text_strong,
            border_color: palette.accent_primary,
        },
        BaseState::Hovered => BasePaint {
            background: palette.accent_primary,
            foreground: palette.text_strong,
            border_color: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            background: with_pressed_alpha(palette.accent_primary),
            foreground: palette.text_strong,
            border_color: palette.accent_primary,
        },
        BaseState::Disabled => BasePaint {
            background: palette.surface_raised,
            foreground: palette.text_disabled,
            border_color: palette.border_subtle,
        },
    }
}

/// §3.1 — Secondary.
const fn secondary_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            background: palette.surface_raised,
            foreground: palette.text_primary,
            border_color: palette.border_strong,
        },
        BaseState::Hovered => BasePaint {
            background: palette.surface_hover,
            foreground: palette.text_strong,
            border_color: palette.border_strong,
        },
        BaseState::Pressed => BasePaint {
            background: palette.surface_active,
            foreground: palette.text_strong,
            border_color: palette.accent_primary,
        },
        BaseState::Disabled => BasePaint {
            background: palette.surface_base,
            foreground: palette.text_disabled,
            border_color: palette.border_subtle,
        },
    }
}

/// §3.1 — Ghost / Quiet.
const fn ghost_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            background: TRANSPARENT,
            foreground: palette.text_primary,
            border_color: TRANSPARENT,
        },
        BaseState::Hovered => BasePaint {
            background: palette.surface_hover,
            foreground: palette.text_strong,
            border_color: palette.border_subtle,
        },
        BaseState::Pressed => BasePaint {
            background: palette.surface_active,
            foreground: palette.text_strong,
            border_color: palette.border_strong,
        },
        BaseState::Disabled => BasePaint {
            background: TRANSPARENT,
            foreground: palette.text_disabled,
            border_color: TRANSPARENT,
        },
    }
}

/// §3.1 — Danger.
const fn danger_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            background: palette.surface_raised,
            foreground: palette.status_danger,
            border_color: palette.status_danger,
        },
        BaseState::Hovered => BasePaint {
            background: palette.status_danger,
            foreground: palette.text_strong,
            border_color: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            background: with_pressed_alpha(palette.status_danger),
            foreground: palette.text_strong,
            border_color: palette.status_danger,
        },
        BaseState::Disabled => BasePaint {
            background: palette.surface_base,
            foreground: palette.text_disabled,
            border_color: palette.border_subtle,
        },
    }
}

/// The colour of a variant's keyboard focus ring (§3.1).
///
/// Primary's ring is `text.strong` because its fill is already the
/// accent — an accent ring on an accent fill would be invisible. The
/// other three take the accent ring the spec gives them.
const fn focus_ring_color(variant: ButtonVariant, palette: &Palette) -> Color {
    match variant {
        ButtonVariant::Primary => palette.text_strong,
        ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Danger => {
            palette.accent_primary
        }
    }
}

/// A token at [`PRESSED_FILL_ALPHA`]: same channels, pressed alpha.
const fn with_pressed_alpha(fill: Color) -> Color {
    Color {
        r: fill.r,
        g: fill.g,
        b: fill.b,
        a: PRESSED_FILL_ALPHA,
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod button_tests;
