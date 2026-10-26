//! The tooltip subsystem — the framework-owned hover and keyboard-focus tooltip
//! (`UIF-ATOM-007`, `UIF-ATOM-001`, `UIF-NAV-003`, `UIF-TAB-003`).
//!
//! A tooltip looks like the smallest surface in the framework and is one of its most
//! entangled: it is the only surface whose visibility is a function of **time**, the only
//! one that must paint above everything it is not related to, and the only one whose
//! trigger is a control that may not exist as a surface at all. Splitting those three
//! concerns is what makes it testable without a window:
//!
//! * [`hover`] — **when** it paints. A hover-delay state machine that takes the clock as
//!   an argument, so no test sleeps and nothing in the paint path reads a wall clock.
//! * [`warm`] — **when the second one paints**. The cross-control memory that lets a
//!   tooltip the user has just seen hand off to the next one at `0 ms` instead of
//!   charging it the full 400 ms again (`time.delay.tooltip_warm`).
//! * [`place`] — **where** it paints. An anchor type over the existing
//!   [`anchor::solve`], including the case where the horizontal and vertical anchors come
//!   from different rects.
//! * [`TooltipStyle::resolve`] - **what** it paints. A pure resolver against the palette,
//!   in the same shape as [`widget::button::resolve_style`].
//!
//! ## Why the framework owns this at all
//!
//! `UIF-NAV-003` and `UIF-TAB-003` both say "show a tooltip with the full label", and
//! both the navbar and the tab strip are application-owned (decision **D-2**). If the
//! framework did not own the tooltip, each would invent its own delay, its own colour and
//! its own placement, and the app would drift into two tooltips that look and behave
//! differently. What the framework owns is the *decision*; the app owns the trigger and
//! the text.
//!
//! ## Z-order: a layer, not a band
//!
//! A tooltip deliberately does **not** register a [`zorder::ZBand`](crate::surface::zorder::ZBand). Three reasons, and the first
//! one is decisive:
//!
//! 1. **The band model has no vocabulary for what a tooltip does with Escape.** A band
//!    answers [`UnwindAnswer`] — it either unwinds or declines — and both branches mean
//!    the key was *consumed*. A tooltip must dismiss itself on Escape and still let the
//!    keyboard owner underneath have the key. Expressing that would mean changing what
//!    [`UnwindAnswer`] means for every existing host.
//! 2. **Registering a band would change how existing bands answer.** Adding a variant moves
//!    [`zorder::SurfaceRegistry::topmost`](crate::surface::zorder::SurfaceRegistry::topmost), `keyboard_owner` and `highest_open_band`, which
//!    changes routing decisions for menus and modals that are already correct. This
//!    increment is not allowed to change how existing surfaces behave.
//! 3. **A tooltip answers "none" to everything a band is asked.** No backdrop, no
//!    pointer grab, no key disposition, no unwind. It is a presentation layer, not a
//!    participant.
//!
//! So the tooltip is a **presentation-only layer above every band, outside
//! [`zorder::SurfaceRegistry`](crate::surface::zorder::SurfaceRegistry)**. The host dismisses it as a **pre-pass** before driving
//! [`zorder::SurfaceRegistry::escape`](crate::surface::zorder::SurfaceRegistry::escape), so the owner underneath sees Escape exactly as it did
//! before the tooltip existed. See [`DismissReason::Escape`].
//!
//! ## What the spec fixes, and what is still a framework decision
//!
//! `UIF-ATOM-007` was rewritten on 2026-10-22 and now pins most of what an earlier
//! revision left open. The table below separates the values the spec **supplies** from the
//! ones the framework still chooses, so nothing here reads as if the spec is silent when it
//! is not.
//!
//! | Item | Authority | Value |
//! |---|---|---|
//! | Hover delay | **§3.7 specifies** | **400 ms** — [`HOVER_DELAY`] |
//! | Keyboard-focus delay | **§3.7 specifies** | **0 ms**, opens immediately — [`KEYBOARD_FOCUS_DELAY`] |
//! | Corner radius | **§3.7 specifies** | **`radius.widget` (4px)** — [`shape::RADIUS_WIDGET`] |
//! | Max width | **§3.7 specifies** | **280px** — [`MAX_WIDTH`] |
//! | Padding | **§3.7 specifies** | **`4px 8px`** = `space.xs` vertical, `space.sm` horizontal |
//! | Fill / border / text | **§3.7 specifies** | `surface.overlay`, `1px border.strong`, `text.primary` at `type.caption` — unchanged from INC-31, and the spec's own figures (`4.01:1` border, `11.72:1` text) match what this crate recomputed from raw sRGB |
//! | Offset from the trigger | **§3.7 specifies** | `4px` (`space.xs`), which is the anchor solver's own trigger gap — [`TOOLTIP_GAP`] |
//! | Dismissal set | **§3.7 specifies** | pointer leave, any key press, `Escape` — all present in [`DismissReason`] |
//! | Re-show / re-arm delay | **framework** | **not implemented.** Would require inventing *two* more numbers the spec does not give. One delay, plus [`DismissReason::clears_suppression`], covers the behaviour the requirements describe. |
//! | Placement beyond the navbar | **framework** | [`TOOLTIP_ALIGNMENT`] plus the solver's flip and clamp. §3.7 defers flipping to `UIF-CMENU-001` and says nothing about a tab, dialog-button or menu-row tooltip; reusing the menu's solver means one placement model with one set of provenance flags rather than three ad-hoc ones. |
//! | Relationship to z-bands | **framework** | A presentation layer above all bands, outside the registry. See "Z-order" above. |
//! | Text **colour** | **framework** (spec names only the type rung) | [`Palette::text_primary`] on [`Palette::surface_overlay`]. §3.7 now *does* name `text.primary`, so this is no longer a guess — the row above records it as specified. |
//! | A `token::time` category | **framework** | **exists as of `UIF-TOK-007`, and all three tooltip rungs are now consumed.** [`HOVER_DELAY`] reads through [`time.tooltip.hover_delay`](crate::token::time::DELAY_TOOLTIP_OPEN) and [`KEYBOARD_FOCUS_DELAY`] through [`time.keyboard.focus`](crate::token::time::INSTANT); the literals live only under `token/`. `time.delay.tooltip_warm` is consumed by [`warm::WarmSession`] — the cross-control memory that makes the 100 ms handoff a *behaviour* rather than a declaration, so a control arrived at within the window after a visible tooltip waits `0 ms`. |
//! | Whether the overlay surface reads as elevated | **framework finding; spec is silent** | Measured, not assumed: `surface.overlay` against `surface.base` is **1.1230:1**, far under the 3:1 chrome floor, so the fill carries no layer separation. The `border.strong` hairline is what makes the card read as floating. A spec that assumes the overlay is visually distinct from the base is wrong about its own palette. |
//! | `TeachingPopover` | **§3.7 specifies, not built** | The interactive callout half of §3.7 (280–360px, `accent.border`, `radius.card`, title/body/footer, Z-Band 2) is **out of scope for this increment and unbuilt**. `UIF-ATOM-007` is therefore `Partial`, not `Implemented`. |
//!
//! Three of INC-31's invented values were wrong against the new revision and have been
//! conformed: the hover delay (500 → **400 ms**), the corner radius (`RADIUS_CARD` 6px →
//! **`RADIUS_WIDGET` 4px**) and the max width ("host's problem" → **280px, owned here**).
//! Keyboard-focus opening at 0 ms did not exist at all and is now implemented as a second
//! arrival channel in [`hover`].
//!
//! [`anchor::solve`]: crate::geometry::anchor::solve
//! [`shape::RADIUS_WIDGET`]: crate::token::shape::RADIUS_WIDGET
//! [`UnwindAnswer`]: crate::surface::zorder::UnwindAnswer
//! [`widget::button::resolve_style`]: crate::widget::button::resolve_style
//! [`zorder::ZBand`](crate::surface::zorder::ZBand): crate::surface::zorder::ZBand
//!
//! Unit tests live in the sibling `tooltip_tests.rs` sidecar.

pub mod hover;
pub mod place;
pub mod warm;
pub mod widget;

use crate::token::{color::Palette, contrast, shape, space, time, typography};
use iced::Color;

pub use hover::{ArrivalSource, DismissReason, Dismissal, HoverController, TimeMs, Visibility};
pub use place::{TOOLTIP_ALIGNMENT, TOOLTIP_GAP, TooltipAnchor};
pub use warm::{ArrivalWarmth, WarmSession, cold_wait_ms, handoff_window_ms, warm_wait_ms};
pub use widget::{Tooltip, TooltipState, tooltip};

/// Content of a tooltip card: a primary text label and an optional trailing shortcut pill (`UIF-ATOM-007`, §3.7).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TooltipContent {
    /// The primary explanation label.
    pub label: String,
    /// Optional trailing shortcut text rendered in `FontFamily::Mono`.
    pub shortcut: Option<String>,
}

impl TooltipContent {
    /// Create a tooltip content with only a primary label (`UIF-ATOM-007`).
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
        }
    }

    /// Create a tooltip content with a primary label and trailing shortcut pill (`UIF-ATOM-007`).
    #[must_use]
    pub fn with_shortcut(label: impl Into<String>, shortcut: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: Some(shortcut.into()),
        }
    }
}

impl<T: Into<String>> From<T> for TooltipContent {
    fn from(label: T) -> Self {
        Self::new(label)
    }
}

/// The gap between a primary tooltip label and an optional trailing shortcut pill (`UIF-ATOM-007`).
///
/// **8px** ([`space::SM`]). Bound to a design token per `UIF-PRIN-001`.
pub const SHORTCUT_GAP: f32 = space::SM;

/// How long the pointer must rest before a tooltip appears, in milliseconds.
///
/// **400 ms**, read through
/// [`time.tooltip.hover_delay`](crate::token::time::DELAY_TOOLTIP_OPEN)
/// — the rung `UIF-TOK-007` §2.6 defines for exactly this wait, and
/// which the spec also calls `time.tooltip.hover_delay`. `UIF-ATOM-007`
/// states the same figure: "Opens after `400ms` hover delay or `0ms`
/// keyboard focus delay". Short enough that a deliberate hover is
/// answered promptly, long enough that the pointer crossing a window on
/// its way somewhere else does not set off a chain of tooltips.
///
/// This is a **thin re-export of the token, not a second definition.**
/// The literal lives only in `token/time.rs`; retuning the rung retunes
/// the tooltip with it, which is what keeps the two from drifting into
/// two different hover delays. The name stays here because "hover delay"
/// is the tooltip subsystem's own vocabulary and every call site reads
/// better for it than `token::time::DELAY_TOOLTIP_OPEN` would.
///
/// It is `u64` milliseconds rather than a [`std::time::Duration`] so it
/// can be a `const`; [`HoverController::with_delay`] takes the
/// [`std::time::Duration`] form.
///
/// The earlier revision of this module used 500 ms, the converged value
/// across Win32's `TDT_INITIAL`, GTK's `gtk-tooltip-timeout` and
/// Material Design. That was a guess made when the spec carried no
/// number; §3.7 now carries one and this is it.
///
/// This is one of **three** timing values in the subsystem — see
/// [`KEYBOARD_FOCUS_DELAY`] for the keyboard channel, and
/// [`warm::cold_wait_ms`] for the same figure as the cold arm of the
/// warm-handoff decision. What is still absent is a *re-show* delay, which the
/// spec does not give and this crate does not invent.
pub const HOVER_DELAY: u64 = time::DELAY_TOOLTIP_OPEN.ms;

/// How long a control reached by keyboard waits before its tooltip appears, in milliseconds.
///
/// **0 ms**, read through [`time.keyboard.focus`](crate::token::time::INSTANT)
/// — `UIF-ATOM-007`'s "`0ms` keyboard focus delay", i.e. it opens
/// immediately. The reason the two channels differ is that their
/// false-positive rates differ. A pointer crosses controls constantly on
/// its way somewhere else, so a hover tooltip needs a rest period to
/// avoid firing on transit. Keyboard focus only lands where the user
/// deliberately tabbed to, so the tooltip is already wanted and any
/// delay is pure lag.
///
/// Sharing the `time.instant` rung rather than carrying a private `0` is
/// deliberate: §2.6 puts the `0ms` keyboard exception on the
/// `time.delay.tooltip_open` row, and `time.instant` is the rung that
/// means "no wait at all". A second literal would be a second thing to
/// retune.
///
/// This is **behaviour, not just a constant**: [`HoverController::keyboard_focus_enter`]
/// arms the controller through [`ArrivalSource::Keyboard`], which makes
/// [`HoverController::tick`] `Shown` on the very first tick after focus arrives.
pub const KEYBOARD_FOCUS_DELAY: u64 = time::INSTANT.ms;

/// The widest a tooltip card may get, in logical pixels (`UIF-ATOM-007`).
///
/// **280px**, as §3.7 specifies. The resolver takes a card size from the host because it
/// does not measure text, but the *cap* is the framework's: a host that let a label run to
/// 600px would break the spec just as surely as one that hardcoded a colour, and a cap
/// nobody enforces is a suggestion. [`clamp_width`] applies it, and the gallery snapshots
/// the clamped result.
///
/// A bare numeric literal on a visually-named `pub const`, so it is registered as a
/// Surface Geometry Constant under `UIF-PRIN-001` §1.1 with §3.7 as its authority.
pub const MAX_WIDTH: f32 = 280.0;

/// Vertical padding inside the tooltip card — `space.xs` (`4px`), per §3.7's `4px 8px`.
pub const PADDING_VERTICAL: f32 = space::XS;

/// Horizontal padding inside the tooltip card — `space.sm` (`8px`), per §3.7's `4px 8px`.
pub const PADDING_HORIZONTAL: f32 = space::SM;

/// Cap a measured card width at [`MAX_WIDTH`], leaving anything narrower alone.
///
/// Pure, and total over the awkward inputs. A non-finite width cannot be compared against
/// the cap meaningfully, so it is reported as the cap rather than propagated — a tooltip
/// of `NaN` width paints nothing at all, whereas one at 280px still shows the label. A
/// negative measurement is not a width, and floors to `0` rather than being honoured.
#[must_use]
pub fn clamp_width(measured: f32) -> f32 {
    if !measured.is_finite() {
        return MAX_WIDTH;
    }
    measured.clamp(0.0, MAX_WIDTH)
}

/// The resolved paint of a tooltip card.
///
/// One style, not a matrix: a tooltip has no hover, active, pressed, disabled or selected
/// state — it is not interactive, and by the time the pointer could act on it the tooltip
/// is gone. That makes this the simplest style resolution in the framework, which is the
/// honest shape for it rather than a lazily-shared one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipStyle {
    /// Card fill — [`Palette::surface_overlay`].
    pub background: Color,
    /// Label colour — [`Palette::text_primary`].
    pub foreground: Color,
    /// Border colour — [`Palette::border_strong`].
    pub border_color: Color,
    /// Border width — [`shape::BORDER_HAIRLINE`].
    pub border_width: f32,
    /// Corner radius — [`shape::RADIUS_WIDGET`], the 4px control rung §3.7 names.
    pub radius: f32,
    /// The type rung — [`typography::CAPTION`].
    pub font: typography::TypeRung,
    /// The widest the card may get — [`MAX_WIDTH`].
    pub max_width: f32,
    /// Vertical padding — [`PADDING_VERTICAL`] (`space.xs`).
    pub padding_vertical: f32,
    /// Horizontal padding — [`PADDING_HORIZONTAL`] (`space.sm`).
    pub padding_horizontal: f32,
    /// Shortcut pill text colour — [`Palette::text_muted`] (`UIF-ATOM-007`).
    pub shortcut_foreground: Color,
    /// Shortcut pill font family — [`typography::FontFamily::Mono`] (`UIF-ATOM-007`).
    pub shortcut_font: typography::FontFamily,
}

impl TooltipStyle {
    /// Resolve the tooltip's paint against `palette` (`UIF-ATOM-007`).
    ///
    /// A pure function: the same palette always produces the same style, with no memory of
    /// the previous call and no state to reset.
    ///
    /// The `(background, foreground)` pair goes through
    /// [`contrast::assert_fill_foreground_lock`] before anything else touches it, mirroring
    /// [`widget::button::resolve_style`](crate::widget::button::resolve_style). For
    /// [`Palette::surface_overlay`] the role is
    /// [`FillLockRole::Unlocked`](contrast::FillLockRole::Unlocked) — a surface, not one of
    /// the four locked solid fills — so the lock places no demand here. It is still checked,
    /// because the value of the shield is that a later edit swapping in a locked fill would
    /// fail at construction rather than ship an unreadable card.
    #[must_use]
    pub fn resolve(palette: &Palette) -> Self {
        let background = palette.surface_overlay;
        let foreground = palette.text_primary;

        contrast::assert_fill_foreground_lock(background, foreground, palette);
        contrast::assert_contrast(
            palette.text_muted,
            background,
            contrast::MIN_TEXT_CONTRAST,
            "tooltip shortcut pill",
        );

        Self {
            background,
            foreground,
            border_color: palette.border_strong,
            border_width: shape::BORDER_HAIRLINE,
            radius: shape::RADIUS_WIDGET,
            font: typography::CAPTION,
            max_width: MAX_WIDTH,
            padding_vertical: PADDING_VERTICAL,
            padding_horizontal: PADDING_HORIZONTAL,
            shortcut_foreground: palette.text_muted,
            shortcut_font: typography::FontFamily::Mono,
        }
    }
}

#[cfg(test)]
#[path = "tooltip_tests.rs"]
mod tooltip_tests;
