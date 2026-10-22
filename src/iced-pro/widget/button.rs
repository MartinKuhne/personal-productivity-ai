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
//! *is* that table, and every colour in it is a [`Palette`] token read by
//! name. There is no colour literal in this file at all: the two pressed
//! fills the spec once wrote as "the rest fill at 85% opacity" are real
//! tokens now — [`accent_fill_active`](Palette::accent_fill_active) and
//! [`status_danger_fill_active`](Palette::status_danger_fill_active) —
//! so the opacity synthesis this file used to perform has been deleted
//! rather than left behind.
//!
//! | Variant | Rest | Hovered | Pressed | Disabled |
//! |---|---|---|---|---|
//! | Primary | `accent.fill` / `text.strong` / `accent.fill` | `accent.fill` / `text.strong` / `text.strong` | `accent.fill_active` / `text.strong` / `accent.fill_active` | `surface.raised` / `text.disabled` / `border.subtle` |
//! | Secondary | `surface.raised` / `text.primary` / `border.strong` | `surface.hover` / `text.strong` / `border.strong` | `surface.active` / `text.strong` / `accent.border` | `surface.base` / `text.disabled` / `border.subtle` |
//! | Ghost | *transparent* / `text.primary` / *transparent* | `surface.hover` / `text.strong` / `border.subtle` | `surface.active` / `text.strong` / `border.strong` | *transparent* / `text.disabled` / *transparent* |
//! | Danger | `surface.raised` / `status.danger.text` / `status.danger.text` | `status.danger.fill` / `text.strong` / `text.strong` | `status.danger.fill_active` / `text.strong` / `status.danger.fill_active` | `surface.base` / `text.disabled` / `border.subtle` |
//!
//! Primary is the one variant whose hover does **not** repaint the fill.
//! It stays `accent.fill` and the state is carried entirely by the border
//! going to `text.strong` (`4.8020:1` against that fill). Adding a hover
//! fill here would be inventing a cell the spec does not have, and there is
//! nowhere darker to go without spending the pressed token early.
//!
//! The Danger row is not what §3.1 prints. The spec paints that variant's
//! hover and pressed fill with `status.danger`, the same red it uses for
//! error *text*, and white body text on that red measures `3.0271:1` —
//! under the 4.5:1 rule, with no value of the token able to serve both
//! roles. Decision **D-5** splits the roles across
//! [`status_danger_text`](Palette::status_danger_text) and
//! [`status_danger_fill`](Palette::status_danger_fill); this row uses
//! each for the job it can actually do. The resting foreground and border
//! stay `status.danger.text`, which is the text/icon role and clears the
//! raised card at `6.1812:1`, while the dedicated fills carry white to
//! `5.1270:1` on hover and `5.7543:1` when pressed.
//!
//! # The ring is adaptive, not a per-variant constant
//!
//! §3.1 gives each variant one fixed ring colour — Primary `text.strong`,
//! the other three `accent.border` — but it only ever snapshots
//! `Rest + ring`. Focus is additive here, so the ring also lands on the
//! hover and press fills, and `accent.border` indigo on a red fill
//! measures `1.7188:1`: an invisible focus indicator at the moment a
//! keyboard user needs one most (gap **G-9**).
//!
//! The fix is not a longer `match`. The right ring depends on the
//! *resolved fill*, not on the variant, so the same variant needs
//! different answers in different states. [`resolve_style`] therefore
//! asks [`focus::resolve_ring_color_for_background`] for the ring
//! against the background it just resolved: `accent.border` while that
//! clears the `3.0:1` chrome floor, `text.strong` when it cannot. On
//! every dark surface the accent clears, so the ring is exactly the one
//! §3.1 asks for; only the saturated fills displace it.
//!
//! That is what makes §3.1's Danger focused cell *conditional* — and it
//! is conditional here only because the rule is evaluated per state:
//!
//! ```text
//!   Danger Rest      bg surface.raised            accent.border 6.2727 PASS -> accent.border
//!   Danger Hovered   bg status.danger.fill        accent.border 1.7188 FAIL -> text.strong 5.1270
//!   Danger Pressed   bg status.danger.fill_active accent.border 1.9291 FAIL -> text.strong 5.7543
//! ```
//!
//! No line of this file says "Danger wears a white ring when hovered". The
//! variant picks a *fill*, the shared resolver picks the ring that fill
//! allows, and the conditionality is the arithmetic. Primary is the same
//! rule with a constant answer: `accent.border` cannot clear the floor
//! against `accent.fill` (`1.6098:1`) or `accent.fill_active`
//! (`1.8961:1`), so its ring is `text.strong` in every base — which is
//! precisely why §3.1 hands Primary a white ring in the first place.
//!
//! A transparent background is not a fill and is not measured as one.
//! A Ghost button's ring sits on whatever the button is mounted to, so
//! it short-circuits to the spec's `accent.border`, which clears the
//! chrome floor against every surface in the ladder — rather than being
//! resolved against `transparent`, which would measure a colour that is
//! never painted.
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
//! # The pressed fills are tokens, not synthesised opacity
//!
//! Before the 2026-10-22 revision of §3.1 there was no token for a
//! pressed CTA, so this file made one by hand: paint the rest fill at 85%
//! opacity. That magnitude lived in `PRESSED_FILL_ALPHA`, and it was
//! the widget layer's only non-token colour value. The revision supplies
//! `accent.fill_active` and `status.danger.fill_active` outright, so the
//! synthesis is deleted rather than kept as a fallback — a constant with
//! no caller is a constant that comes back.
//!
//! Deleting it also removes a measurement hazard. A translucent fill's
//! contrast is not a property of the fill: what a pressed label actually
//! sits on is the fill composited over whatever the button is mounted to,
//! and [`contrast_ratio`](crate::token::contrast::contrast_ratio) is
//! deliberately alpha-blind, so the naive reading and the painted one
//! were different numbers and only one of them described the screen. An
//! opaque token closes that question: the pressed label is `5.6559:1` on
//! `accent.fill_active` and `5.7543:1` on `status.danger_fill_active`
//! wherever the button happens to live.
//!
//! Every fill this resolver produces is therefore either fully opaque or
//! the Ghost variant's explicit `transparent`, and the sidecar pins that
//! so an opacity hack cannot quietly return.
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
use crate::token::{color::Palette, contrast, shape, size, space};
use crate::widget::focus;
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
    /// [`accent_fill`](Palette::accent_fill) and carrying the
    /// `UIF-TOK-006` Selection Foreground Lock.
    Primary,
    /// A bordered, surface-filled control for secondary actions.
    Secondary,
    /// A quiet control with no fill at rest, for toolbar and inline
    /// actions.
    Ghost,
    /// A destructive action, resting quiet in
    /// [`status_danger_text`](Palette::status_danger_text) text and escalating to
    /// a filled [`status_danger_fill`](Palette::status_danger_fill) on
    /// hover — the background red, because the label is painted *on* the
    /// danger here rather than written in it (decision **D-5**).
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
///
/// The ring is not chosen by variant. It is resolved against the
/// background this function just resolved, through
/// [`focus::resolve_ring_color_for_background`], so a focused control
/// always wears a ring that can actually be seen against whatever it is
/// sitting on (`UIF-TOK-006` rule 2, decision **D-6**).
///
/// And the `(background, foreground)` pair it is about to hand back goes
/// through [`contrast::assert_fill_foreground_lock`] before anything else
/// touches it — the `UIF-TOK-006` rule 4 solid-fill foreground lock,
/// checked at the moment the style is built rather than after. Primary's
/// two fills and Danger's two fills are exactly the four tokens that rule
/// names, and every one of their cells carries
/// [`text_strong`](Palette::text_strong); the shield is what makes that
/// a checked fact at construction instead of a property of the table that
/// a later edit could quietly break.
#[must_use]
pub fn resolve_style(variant: ButtonVariant, state: VisualState, palette: &Palette) -> ButtonStyle {
    let base = base_paint(variant, state.base, palette);

    contrast::assert_fill_foreground_lock(base.background, base.foreground, palette);

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            focus::resolve_ring_color_for_background(base.background, palette),
            FOCUS_RING_WIDTH,
        )
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
            background: palette.accent_fill,
            foreground: palette.text_strong,
            border_color: palette.accent_fill,
        },
        BaseState::Hovered => BasePaint {
            background: palette.accent_fill,
            foreground: palette.text_strong,
            border_color: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            background: palette.accent_fill_active,
            foreground: palette.text_strong,
            border_color: palette.accent_fill_active,
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
            border_color: palette.accent_border,
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
///
/// The fills are the two background reds, never the
/// [`status_danger_text`](Palette::status_danger_text) §3.1 names here.
/// That token is the error *text* red, and white body text on it measures
/// `3.0271:1` — the two roles need opposite luminance, so decision
/// **D-5** split them (gap **G-8**). The resting foreground and border
/// keep `status.danger.text`, which is the text/icon role and clears at
/// `6.1812:1` on `surface.raised`, while hover and press take
/// [`status_danger_fill`](Palette::status_danger_fill) (`5.1270:1` under
/// `text.strong`) and
/// [`status_danger_fill_active`](Palette::status_danger_fill_active)
/// (`5.7543:1`) — the pressed one darker, the same way `accent.fill`
/// steps down to `accent.fill_active`.
///
/// The pressed border tracks the pressed fill rather than staying
/// `status.danger.text`, exactly as it tracked the fill before the
/// split: §3.1 gives that cell a border of the fill's own colour at full
/// opacity, so it is a geometry holder, not a state signal. Left on
/// `status.danger.text` it would sit `1.9009:1` from the fill it
/// outlines — a border that reads as a mistake rather than as an edge.
const fn danger_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            background: palette.surface_raised,
            foreground: palette.status_danger_text,
            border_color: palette.status_danger_text,
        },
        BaseState::Hovered => BasePaint {
            background: palette.status_danger_fill,
            foreground: palette.text_strong,
            border_color: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            background: palette.status_danger_fill_active,
            foreground: palette.text_strong,
            border_color: palette.status_danger_fill_active,
        },
        BaseState::Disabled => BasePaint {
            background: palette.surface_base,
            foreground: palette.text_disabled,
            border_color: palette.border_subtle,
        },
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod button_tests;
