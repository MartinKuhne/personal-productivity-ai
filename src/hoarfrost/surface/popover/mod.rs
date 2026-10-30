//! The teaching popover — §3.7's interactive callout (`UIF-ATOM-007`).
//!
//! §3.7 describes two surfaces under one requirement id and they share
//! nothing but a word. The [`tooltip`](crate::surface::tooltip) half is
//! a pointer-transparent label whose lifetime is a hover timer. This half
//! is a card with buttons in it that the user is expected to reach,
//! click and dismiss. Splitting the two concerns is what makes each
//! testable:
//!
//! * [`model`] — **what** the callout says and what acting on it means.
//!   The footer roster, the initial-focus rule, the derived focus ids.
//! * [`place`] — **where** it lands. A preferred side over the existing
//!   [`anchor::solve`], with the solver's flip and clamp flags carried
//!   through rather than flattened away.
//! * [`host`] — **how** it behaves. Trap acquisition, dismissal
//!   precedence, and the `UnwindAnswer` the escape walk reads.
//! * [`PopoverStyle::resolve`] — **what it paints**. A pure resolver
//!   against the palette, in the same shape as
//!   [`TooltipStyle::resolve`](crate::surface::tooltip::TooltipStyle::resolve).
//!
//! # Dismissal precedence
//!
//! Four things can close a callout and they are not peers. The order is
//! the order the *user* reaches them, and each one wins over the ones
//! below it:
//!
//! 1. **A footer button.** The pointer is inside the card, so the
//!    Band 2 catcher never fires and the click cannot be double-handled.
//!    The primary action closes *with* the action.
//! 2. **`Escape`.** Consumed outright at `Escape Rank 3`. Nothing
//!    underneath sees it — see the section below.
//! 3. **Click-away.** The Band 2 `TransparentCatcher` route: a pointer
//!    down outside the card dismisses the callout and the event does not
//!    reach the workspace below.
//! 4. **Programmatic close.** The application's own path, which bypasses
//!    nothing because there is no guard to bypass.
//!
//! What is deliberately **not** in the list is pointer leave. §3.7 gives
//! the tooltip "dismisses immediately on pointer leave" and gives the
//! popover no such rule, and the omission is load-bearing rather than an
//! oversight: the user must be able to drive the cursor from the trigger,
//! across the [`place::POPOVER_GAP`] gap, onto a footer button. A
//! popover that dismissed on leave would close the moment the pointer
//! began the journey that dismisses nothing.
//!
//! # How `Escape` differs from the tooltip's
//!
//! The tooltip dismisses on `Escape` **and does not consume it**, which
//! is why it lives outside the registry: no [`UnwindAnswer`] arm means
//! "dismiss but hand the key on". The popover does the opposite — it
//! dismisses **and consumes**, because §3.7 places it at `Escape Rank 3`
//! and a rank is a statement about the unwind walk, not about a
//! pre-pass the host has to remember to run.
//!
//! The practical difference is what happens to a modal underneath. With
//! a tooltip open over a modal, `Escape` closes the tooltip and the modal
//! then receives the key and closes too. With a teaching popover open
//! over a modal, `Escape` closes the callout and the modal **stays
//! open**, because the key stopped at rank 3. That is the behaviour a
//! teaching callout wants: dismissing the explanation must not also
//! destroy the thing being explained.
//!
//! # Why it enters `SurfaceRegistry`
//!
//! The tooltip's three reasons for staying out are good reasons for a
//! tooltip, and all three invert here:
//!
//! | Tooltip's reason | Popover |
//! |---|---|
//! | The band model has no vocabulary for "dismiss but do not consume" | It consumes, so `UnwindAnswer::Dismissed` is exactly its answer |
//! | Registering would move `topmost()` and `keyboard_owner()` and change how existing bands route | Moving them **is** the requirement: while a callout is open, Band 2 owns the keyboard |
//! | It answers "none" to everything a band is asked | It answers every question concretely — `TransparentCatcher`, `CaptureInsideDismissOutside`, rank `3` |
//!
//! The distinguishing property is **interactivity**. Pointer-transparency
//! is what forces the tooltip out of a model built on pointer ownership;
//! click-away ownership is what forces the popover in.
//!
//! # §3.7's "Z-Band 2 / Escape Rank 3" reconciles exactly
//!
//! §3.7 says the callout "mounts in `Z-Band 2` (`Escape Rank 3`)". In
//! this crate [`ZBand::Menu`] *is* band index `2` and its
//! `escape_rank()` *is* `3`, and §5's band table already lists
//! `TeachingPopover` in that row beside the menus. So no new band is
//! invented and no rank is retuned: [`SurfaceKind::TeachingPopover`]
//! returns `ZBand::Menu` from its `band()`, and inherits the backdrop,
//! pointer rule and escape rank the table already prints for it.
//!
//! [`SurfaceKind::TeachingPopover`]: crate::surface::zorder::SurfaceKind::TeachingPopover
//!
//! # Every ratio was recomputed
//!
//! The figures come from the raw sRGB in [`Palette::dark`] through
//! [`contrast::contrast_ratio`], not from §3.7's printed column.
//!
//! | Pair | §3.7 prints | True |
//! |---|---|---|
//! | `accent.border` / `surface.overlay` | `5.94:1` | `5.9394:1` ✓ |
//!
//! The printed figure agrees, so nothing here is conformed to it. The
//! supporting pairs the card leans on are `text.strong` on
//! `surface.overlay` at `17.7168:1` and `text.primary` on
//! `surface.overlay` at `11.7180:1` — the same two the tooltip's
//! `11.72:1` text figure resolves to — and `surface.overlay` against
//! `surface.base` at `1.1230:1`, which is why the `accent.border`
//! hairline, not the fill, is what makes the card read as floating.
//!
//! [`anchor::solve`]: crate::geometry::anchor::solve
//! [`contrast::contrast_ratio`]: crate::token::contrast::contrast_ratio
//! [`ZBand::Menu`]: crate::surface::zorder::ZBand::Menu
//! [`UnwindAnswer`]: crate::surface::zorder::UnwindAnswer
//!
//! Unit tests live in the sibling `popover_tests.rs` sidecar.

pub mod host;
pub mod model;
pub mod place;

use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::shape;
use crate::token::space;
use crate::token::typography::{self, Weight};
use iced::Color;

/// The narrowest a teaching popover may be — §3.7's `280px` lower bound.
///
/// A bare numeric literal on a visually-named `pub const`, so it is
/// registered as a Surface Geometry Constant under `UIF-PRIN-001` §1.1
/// with §3.7 as its authority.
pub const WIDTH_MIN: f32 = 280.0;

/// The widest a teaching popover may be — §3.7's `360px` upper bound.
///
/// Registered alongside [`WIDTH_MIN`]. The band between the two is the
/// host's to fill: §3.7 gives a *range* rather than a size because the
/// card is sized to its prose, and a range nobody enforces is a
/// suggestion.
pub const WIDTH_MAX: f32 = 360.0;

/// Padding inside the card — §3.7's `12px`, [`space::MD`].
pub const PADDING: f32 = space::MD;

/// The card's border width — [`shape::BORDER_HAIRLINE`].
///
/// Constant. A card that thickened its edge to be taken more seriously
/// would change its own box with its confidence.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The card's corner radius — §3.7's `radius.card`, [`shape::RADIUS_CARD`].
///
/// One rung up from the tooltip's `radius.widget`, and the reason is the
/// card's own size: a `280 × 120` block at a `4px` radius reads as a
/// large button, and the same block at `6px` reads as a card. §3.7
/// picks the card rung for a card.
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// The type rung both the title and the body draw from — `type.ui_compact`.
///
/// §3.7 asks for `13px` in both, and [`typography::UI_COMPACT`] is the
/// `13px` rung. The two take different weights from it, and both weights
/// are inside the rung's permitted set, so neither has to leave the scale
/// to get the emphasis the spec prints.
pub const TYPE_RUNG: typography::TypeRung = typography::UI_COMPACT;

/// The title's weight — §3.7's `SemiBold`.
pub const TITLE_WEIGHT: Weight = Weight::SemiBold;

/// The body's weight — §3.7's `Regular`.
pub const BODY_WEIGHT: Weight = Weight::Regular;

/// The resolved paint and box of a teaching popover card.
///
/// One style, not a matrix: a callout has no hover, pressed, disabled or
/// selected state of its own. Its *buttons* have those states, and they
/// resolve through
/// [`dialog_button`](crate::surface::dialog_button) like every other
/// footer control. The card itself is a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverStyle {
    /// Card fill — [`Palette::surface_overlay`].
    pub background: Color,
    /// Border colour — [`Palette::accent_border`].
    pub border_color: Color,
    /// Border width — [`BORDER_WIDTH`].
    pub border_width: f32,
    /// Corner radius — [`RADIUS`].
    pub radius: f32,
    /// Inner padding — [`PADDING`].
    pub padding: f32,
    /// Title colour — [`Palette::text_strong`].
    pub title_color: Color,
    /// Title size — [`TYPE_RUNG`]'s `13px`.
    pub title_size: f32,
    /// Title weight — [`TITLE_WEIGHT`].
    pub title_weight: Weight,
    /// Body colour — [`Palette::text_primary`].
    pub body_color: Color,
    /// Body size — [`TYPE_RUNG`]'s `13px`.
    pub body_size: f32,
    /// Body weight — [`BODY_WEIGHT`].
    pub body_weight: Weight,
    /// The narrowest the card may be — [`WIDTH_MIN`].
    pub width_min: f32,
    /// The widest the card may be — [`WIDTH_MAX`].
    pub width_max: f32,
}

impl PopoverStyle {
    /// Resolve the popover's paint and box against `palette`
    /// (`UIF-ATOM-007`).
    ///
    /// A pure function: the same palette always produces the same style,
    /// with no memory of the previous call and no state to reset.
    ///
    /// The `(background, foreground)` pair goes through
    /// [`contrast::assert_fill_foreground_lock`] for both text channels,
    /// mirroring
    /// [`TooltipStyle::resolve`](crate::surface::tooltip::TooltipStyle::resolve).
    /// `surface.overlay` is a surface rather than one of the four locked
    /// solid fills, so the lock reports
    /// [`FillLockRole::Unlocked`](contrast::FillLockRole::Unlocked) and
    /// places no demand — but it is still checked, because the value of
    /// the shield is that a later edit swapping in a locked fill would
    /// fail at construction rather than ship an unreadable card.
    #[must_use]
    pub fn resolve(palette: &Palette) -> Self {
        let background = palette.surface_overlay;

        contrast::assert_fill_foreground_lock(background, palette.text_strong, palette);
        contrast::assert_fill_foreground_lock(background, palette.text_primary, palette);

        assert!(
            TYPE_RUNG.allows(TITLE_WEIGHT),
            "type.ui_compact does not permit the weight §3.7 asks the title to wear",
        );
        assert!(
            TYPE_RUNG.allows(BODY_WEIGHT),
            "type.ui_compact does not permit the weight §3.7 asks the body to wear",
        );

        Self {
            background,
            border_color: palette.accent_border,
            border_width: BORDER_WIDTH,
            radius: RADIUS,
            padding: PADDING,
            title_color: palette.text_strong,
            title_size: TYPE_RUNG.size,
            title_weight: TITLE_WEIGHT,
            body_color: palette.text_primary,
            body_size: TYPE_RUNG.size,
            body_weight: BODY_WEIGHT,
            width_min: WIDTH_MIN,
            width_max: WIDTH_MAX,
        }
    }
}

/// Clamp a measured card width into §3.7's `280..360` band.
///
/// Pure, and total over the awkward inputs. A non-finite measurement
/// cannot be compared against the band meaningfully, so it lands on
/// [`WIDTH_MIN`] rather than being propagated — a card of `NaN` width
/// paints nothing at all, whereas one at the minimum still shows the
/// prose. A narrower-than-minimum measurement is raised to the floor:
/// the band is a floor as well as a ceiling, and a `180px` callout with
/// three lines of wrapped body is unreadable in a way a `280px` one is
/// not.
#[must_use]
pub fn clamp_width(measured: f32) -> f32 {
    if !measured.is_finite() {
        return WIDTH_MIN;
    }
    measured.clamp(WIDTH_MIN, WIDTH_MAX)
}

/// Classify the card's perimeter under `UIF-TOK-006` rule 3
/// (`UIF-ATOM-007`).
///
/// The card's `accent.border` edge is [`ChromeClass::Judged`] against
/// `3.0:1` and clears it at `5.9394:1` over `surface.overlay`. Neither
/// of the two exempt classes is reachable: the perimeter is not
/// coincident with the fill, and it is not `border.subtle`. A card that
/// resolved to a resting divider here would be a card whose edge had
/// quietly become decoration.
#[must_use]
pub fn card_chrome_class(style: &PopoverStyle, palette: &Palette) -> ChromeClass {
    if same_channels(style.border_color, style.background) {
        ChromeClass::CoincidentWithFill
    } else if same_channels(style.border_color, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Whether two colours are the same channel-for-channel.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

pub use host::{DismissCause, IgnoredReason, OpenRefusal, PopoverEvent, PopoverHost};
pub use model::{DISMISS_LABEL, PopoverModel, dismiss_focus_id, group_for, primary_focus_id};
pub use place::{POPOVER_GAP, PreferredSide};

#[cfg(test)]
#[path = "popover_tests.rs"]
mod popover_tests;
