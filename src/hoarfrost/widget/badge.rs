//! The badge paint resolver and geometry — the `6px` dot, the count pill
//! and the status chip (`UIF-ATOM-008`, §3.8).
//!
//! Like [`toggle`](crate::widget::toggle), this module resolves paint; it
//! does not build a widget. [`resolve_style`] is a pure function of
//! `(model, palette)` returning a [`BadgeStyle`], and the geometry
//! functions are pure functions of the style and the measured text
//! width, so every cell of §3.8 is checkable without a window.
//!
//! # There is no state half to this resolver
//!
//! Every other resolver in this crate takes a
//! [`VisualState`](crate::state::VisualState). This one does not, and
//! the reason is §3.8 itself: the badge table has no hover column, no
//! pressed column, no focus column and no disabled column. A badge is
//! not a control. It is a mark some other thing has earned, and §3.8
//! gives it nothing to respond to.
//!
//! That absence is load-bearing rather than an omission to be filled in.
//! A badge that brightened on hover would imply it could be clicked; a
//! badge that took focus would compete with the control it is pinned to
//! for the keyboard. The resolver therefore has **no arm** for those
//! states — not a no-op arm, no arm at all, so a future edit that wants
//! a hover skin has to add the state to the resolver and to the spec
//! rather than discovering one was already there.
//!
//! # The three kinds
//!
//! | Kind | Box | Type | Fill / border |
//! |---|---|---|---|
//! | [`BadgeKind::Dot`] | `6 × 6`, `radius.pill` | none | solid mark, no stroke |
//! | [`BadgeKind::Count`] | `18px` h, `18px` min-w, `[0, 4px]` pad | `12px` SemiBold | solid `accent.fill` / `status.danger.fill`, or subtle `surface.active` + `1px border.subtle` |
//! | [`BadgeKind::Status`] | `24px` h, `[0, 8px]` pad | `12px` Medium | `surface.raised` / `1px` tone border |
//!
//! The dot carries no text and no stroke: it is the only badge whose
//! entire visual payload is one filled circle, which is what lets it be
//! pinned to a corner without resizing anything. The count pill is the
//! only badge that **inverts to a solid fill**, and the only one with a
//! ceiling — past [`COUNT_CEILING`](crate::widget::badge_model::COUNT_CEILING)
//! the number folds to `99+`. The status chip is the only badge that
//! carries a word, and §3.8 makes its leading glyph mandatory: the tone
//! alone is never allowed to carry the meaning, because tone is the one
//! property a colour-blind user may not receive.
//!
//! # A tone is three channels, not one
//!
//! §3.8 does not give a tone a single colour. It gives the **dot** one
//! rung, the **chip's perimeter** another and the **pill's solid body** a
//! third, and the rungs differ: the accent tone is `accent.border` on
//! the dot, `accent.text` on the chip and `accent.fill` in the pill.
//! [`ToneColors`] carries all three so a resolver that wants one of them
//! cannot accidentally paint another.
//!
//! The subtle count pill is the exception that proves the split: §3.8
//! gives it **no tone at all**. Every non-solid tone resolves to
//! `surface.active` behind `text.strong` with a `1px border.subtle`,
//! because a subtle pill says "a number lives here" rather than "in what
//! mood", and the section prices that as one appearance at `14.29:1`.
//!
//! # Every ratio below was recomputed
//!
//! The figures are from the raw sRGB in [`Palette::dark`] through
//! [`contrast::contrast_ratio`], not copied from §3.8's printed column.
//! Against the chip's `surface.raised` mount:
//!
//! | Pair | §3.8 prints | True |
//! |---|---|---|
//! | `status.success` / `surface.raised` | `13.52:1` | `14.2825:1` |
//! | `status.warning` / `surface.raised` | `6.10:1` | `6.4474:1` |
//! | `accent.text` / `surface.raised` | `9.96:1` | `10.0215:1` |
//! | `status.danger.text` / `surface.raised` | `6.18:1` | `6.1812:1` ✓ |
//!
//! The first three disagree (RUST-041). Two of them are traceable:
//! `13.52` is `status.success` on `surface.overlay` (`13.5237`) and
//! `6.10` is `status.warning` on `surface.overlay` (`6.1048`) — the
//! spec's numbers were measured against the wrong mount. `9.96` is
//! reproduced by no surface at all (`surface.code` comes closest at
//! `9.8543`), so it is a stale figure from an earlier token set. None
//! of the corrections breaks a floor: every true value clears what its
//! class owes, so nothing here changes to match the printed numbers and
//! the sidecar pins the true ones instead.
//!
//! # The solid count pill goes through the fill lock
//!
//! The pill's solid variant is `accent.fill` — or, for the danger tone,
//! `status.danger.fill` — behind `text.strong`, exactly the pair
//! `UIF-TOK-006` rule 4 pins, so
//! [`contrast::assert_fill_foreground_lock`] runs at construction and
//! reports `Locked` (`4.8020:1` on `accent.fill`, `5.1270:1` on
//! `status.danger.fill`). The subtle pill and the chip sit on a *surface*
//! rather than a locked fill, so the same call reports `NotSolidFill` and
//! the ordinary text contrast rule applies.
//!
//! # Perimeter classification
//!
//! [`badge_chrome_class`] sorts each resolved perimeter under
//! `UIF-TOK-006` rule 3 by token identity rather than defaulting:
//!
//! * the dot and the solid pill are [`ChromeClass::CoincidentWithFill`]
//!   — their stroke *is* their fill, so the fill carries the boundary
//!   and owes `3.00:1` against the mount. `accent.fill` on
//!   `surface.raised` is `3.8965:1` and on `surface.overlay`
//!   `3.6895:1`; `status.danger.fill` on raised is `3.6495:1`.
//! * the chip's tone border is [`ChromeClass::Judged`] at `3.0:1` —
//!   `status.success` `14.2825:1`, `accent.text` `10.0215:1`,
//!   `status.warning` `6.4474:1`, `status.danger.text` `6.1812:1`.
//! * the neutral chip wears `border.strong` (`4.2379:1`) rather than
//!   `border.subtle` **on purpose**. `border.subtle` would classify as
//!   [`ChromeClass::RestingDivider`] and drop out of measurement
//!   entirely, which would let a chip that is an *indicator* hide behind
//!   a decorative hairline's exemption. The subtle **pill** does wear
//!   `border.subtle`, and does so because §3.8 says so outright rather
//!   than leaving it to be inferred — the pill's number is carried by its
//!   `14.29:1` fill, not by its edge.
//!
//! Unit tests live in the sibling `badge_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography::{self, Weight};
use crate::widget::badge_model::{BadgeKind, BadgeModel, BadgeTone};
use iced::Color;

/// The presence dot's edge length — §3.8's `6px`, [`size::INDICATOR_DOT`].
///
/// The dot is square in its own box and circular in its radius, so this
/// is both its width and its height. It is not derived from the type
/// rung: a mark sized to a glyph would change size when the label did,
/// and a marker that resizes with the text beside it stops reading as a
/// marker.
pub const DOT_SIZE: f32 = size::INDICATOR_DOT;

/// The count pill's height — §3.8's `18px`.
pub const COUNT_HEIGHT: f32 = size::BADGE_COUNT_HEIGHT;

/// The count pill's minimum width — §3.8's `18px`.
///
/// Equal to [`COUNT_HEIGHT`] because the pill must stay circular at the
/// single-digit end: a `1` in a box `10 × 18` is a lozenge, and a
/// lozenge does not read as a count.
pub const COUNT_MIN_WIDTH: f32 = size::BADGE_COUNT_HEIGHT;

/// The count pill's horizontal padding — §3.8's `[0, 4px]`.
///
/// The vertical half of the spec's `[0, 4px]` is zero because the pill
/// has no vertical padding to give: the `18px` height *is* the box and
/// the `12px` label is centred in it. Only the horizontal half exists.
pub const COUNT_H_PADDING: f32 = space::XS;

/// The status chip's height — §3.8's `24px`.
pub const CHIP_HEIGHT: f32 = size::BADGE_CHIP_HEIGHT;

/// The status chip's horizontal padding — §3.8's `[0, 8px]`.
pub const CHIP_H_PADDING: f32 = space::SM;

/// The status chip's border width — §3.8's `1px` status border.
///
/// Constant, and constant at the hairline: a chip that thickened its
/// border for a louder tone would change the chip's size with its
/// meaning, which is the same layout-instability objection
/// `UIF-PRIN-003` raises against hover growth.
pub const CHIP_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Gap between a chip's mandatory glyph and its label — `space.xs`.
///
/// The glyph and the word are one statement, so they sit at the tightest
/// text-adjacent rung rather than at a grouping gap.
pub const GLYPH_LABEL_GAP: f32 = space::XS;

/// The type rung both text-bearing badges draw from — `type.caption`.
///
/// §3.8 asks for `12px` in both the pill and the chip, and
/// [`typography::CAPTION`] is the `12px` rung. The two badges take
/// different weights from it, and both weights are inside the rung's
/// permitted set, so neither badge has to leave the scale to get the
/// emphasis the spec prints.
pub const TYPE_RUNG: typography::TypeRung = typography::CAPTION;

/// The count pill's weight — §3.8's `SemiBold`.
pub const COUNT_WEIGHT: Weight = Weight::SemiBold;

/// The status chip's weight — §3.8's `Medium`.
pub const CHIP_WEIGHT: Weight = Weight::Medium;

/// A badge's fully resolved paint and box for one model.
///
/// Every field is a value a renderer consumes as-is: no token lookup, no
/// conditional, no further resolution. The style deliberately does not
/// carry the model — the model selected the values and is not itself
/// painted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeStyle {
    /// The badge's fill.
    ///
    /// The solid count pill's is `accent.fill`; the dot's is its mark
    /// colour; the chip's is `surface.raised`.
    pub fill: Color,
    /// The perimeter stroke colour.
    ///
    /// Coincident with [`fill`](Self::fill) wherever no stroke is drawn.
    pub border: Color,
    /// The perimeter stroke width — `0.0` on the dot and the solid pill.
    pub border_width: f32,
    /// The label colour.
    pub text: Color,
    /// The mandatory leading glyph colour.
    ///
    /// A separate channel from [`text`](Self::text) even though both are
    /// the tone's colour today, because the glyph is painted as its own
    /// run at a fixed offset and a renderer must be able to place it
    /// without re-deriving its colour.
    pub glyph: Color,
    /// The badge's height.
    pub height: f32,
    /// The badge's minimum width — the floor the pill holds so a single
    /// digit stays circular.
    pub min_width: f32,
    /// The badge's horizontal padding.
    pub h_padding: f32,
    /// The corner radius — [`shape::RADIUS_PILL`] on all three kinds.
    pub radius: f32,
    /// The label's font size.
    pub font_size: f32,
    /// The label's weight.
    pub weight: Weight,
    /// Whether the leading glyph is painted.
    ///
    /// True only for the status chip. The pill and the dot have no glyph
    /// to paint, and the flag exists so a renderer does not have to
    /// match on the model to know whether to reserve the glyph box.
    pub shows_glyph: bool,
}

/// Resolve the paint and box for one badge model (`UIF-ATOM-008`).
///
/// A pure function: the same two inputs always produce the same
/// [`BadgeStyle`]. The kind selects the box and the tone selects the
/// colours; nothing else participates, because §3.8 gives the badge no
/// state to resolve against.
///
/// The `(fill, text)` pair goes through
/// [`contrast::assert_fill_foreground_lock`] before the style is handed
/// back — the `UIF-TOK-006` rule 4 solid-fill foreground lock, checked
/// at the moment the style is built rather than trusted.
#[must_use]
pub fn resolve_style(model: &BadgeModel, palette: &Palette) -> BadgeStyle {
    let tone = resolve_tone(model.tone(), palette);

    let mut style = match model.kind() {
        BadgeKind::Dot => BadgeStyle {
            fill: tone.dot_fill,
            border: tone.dot_fill,
            border_width: NO_BORDER_WIDTH,
            text: tone.text,
            glyph: tone.dot_fill,
            height: DOT_SIZE,
            min_width: DOT_SIZE,
            h_padding: NO_PADDING,
            radius: shape::RADIUS_PILL,
            font_size: TYPE_RUNG.size,
            weight: COUNT_WEIGHT,
            shows_glyph: false,
        },
        BadgeKind::Count(_) => {
            if model.tone().paints_solid() {
                BadgeStyle {
                    fill: tone.solid_fill,
                    border: tone.solid_fill,
                    border_width: NO_BORDER_WIDTH,
                    text: palette.text_strong,
                    glyph: tone.solid_fill,
                    height: COUNT_HEIGHT,
                    min_width: COUNT_MIN_WIDTH,
                    h_padding: COUNT_H_PADDING,
                    radius: shape::RADIUS_PILL,
                    font_size: TYPE_RUNG.size,
                    weight: COUNT_WEIGHT,
                    shows_glyph: false,
                }
            } else {
                // §3.8's subtle count branch is **tone-independent**:
                // `surface.active` behind `text.strong` with a `1px
                // border.subtle`, for every non-solid tone. The tone
                // still selects the solid branch's fill and the chip's
                // perimeter, but a subtle pill says "a number lives
                // here", not "in what mood", so the section gives it one
                // appearance and the `14.29:1` it prints.
                BadgeStyle {
                    fill: palette.surface_active,
                    border: palette.border_subtle,
                    border_width: CHIP_BORDER_WIDTH,
                    text: palette.text_strong,
                    glyph: palette.border_subtle,
                    height: COUNT_HEIGHT,
                    min_width: COUNT_MIN_WIDTH,
                    h_padding: COUNT_H_PADDING,
                    radius: shape::RADIUS_PILL,
                    font_size: TYPE_RUNG.size,
                    weight: COUNT_WEIGHT,
                    shows_glyph: false,
                }
            }
        }
        BadgeKind::Status { .. } => BadgeStyle {
            fill: palette.surface_raised,
            border: tone.border,
            border_width: CHIP_BORDER_WIDTH,
            text: tone.text,
            glyph: tone.text,
            height: CHIP_HEIGHT,
            min_width: NO_MIN_WIDTH,
            h_padding: CHIP_H_PADDING,
            radius: shape::RADIUS_PILL,
            font_size: TYPE_RUNG.size,
            weight: CHIP_WEIGHT,
            shows_glyph: true,
        },
    };

    // A zero count paints nothing: no fill, no stroke, no text. The
    // host still receives a well-formed style so the geometry half stays
    // testable, but nothing in it is visible.
    if model.is_empty() {
        style.fill = Color::TRANSPARENT;
        style.border = Color::TRANSPARENT;
        style.border_width = NO_BORDER_WIDTH;
        style.text = Color::TRANSPARENT;
        style.glyph = Color::TRANSPARENT;
    }

    // The rung's own permission check runs here rather than at the call
    // site: §3.8 asks for SemiBold and Medium at 12px, and if the rung
    // ever stops permitting them this is where it fails.
    assert!(
        TYPE_RUNG.allows(style.weight),
        "type.caption does not permit the weight §3.8 asks a badge to wear"
    );

    // The `UIF-TOK-006` rule 4 lock runs against what is *actually
    // painted*, after the zero-count override rather than before it.
    // The dot is handed an unpainted foreground because that is the
    // truth of it: a `6px` mark carries no text, so rule 4 has nothing
    // to protect and reports `ForegroundNotPainted` — governed, and
    // compliant. Handing the lock the dot's `text` channel instead would
    // have it adjudicate a colour no renderer ever puts on the dot.
    let painted_foreground = if matches!(model.kind(), BadgeKind::Dot) {
        Color::TRANSPARENT
    } else {
        style.text
    };
    contrast::assert_fill_foreground_lock(style.fill, painted_foreground, palette);

    style
}

/// The width a pill needs for `text_width`, given the style's padding.
///
/// The pill's width is not known ahead of the value it carries, which is
/// the one respect in which [`BadgeKind::Count`] differs from the other
/// two kinds. The floor is [`BadgeStyle::min_width`], so a single digit
/// stays circular and a folded `99+` widens the pill rather than
/// overflowing it.
#[must_use]
pub fn pill_width(style: &BadgeStyle, text_width: f32) -> f32 {
    let content = text_width + style.h_padding * 2.0;
    if content < style.min_width {
        style.min_width
    } else {
        content
    }
}

/// The width a status chip needs for its glyph and label.
///
/// The mandatory glyph is always charged to the width when
/// [`BadgeStyle::shows_glyph`] is set, so a chip never has to reflow
/// when a label changes: the glyph box is part of the chip's structure,
/// not an optional decoration that can arrive later.
#[must_use]
pub fn chip_width(style: &BadgeStyle, glyph_width: f32, text_width: f32) -> f32 {
    let content = if style.shows_glyph {
        glyph_width + GLYPH_LABEL_GAP + text_width
    } else {
        text_width
    };
    let total = content + style.h_padding * 2.0;
    if total < style.min_width {
        style.min_width
    } else {
        total
    }
}

/// Classify a resolved badge perimeter under `UIF-TOK-006` rule 3
/// (`UIF-ATOM-008`).
///
/// Three of the four [`ChromeClass`] arms are reachable and each is
/// decided by the *token identity* of the perimeter, not by which badge
/// it belongs to:
///
/// * a perimeter set to the exact same colour as the fill it surrounds
///   is [`ChromeClass::CoincidentWithFill`] — the dot and the solid
///   pill. They draw no boundary of their own, so the fill carries the
///   separation and owes `3.00:1` against the mount.
/// * `border.subtle` is [`ChromeClass::RestingDivider`] and exempt.
///   **No badge resolves to it today** — the neutral chip deliberately
///   wears `border.strong` so it stays measured — but the arm is here
///   because the classifier is shared and a caller handed a subtle
///   border must get the honest class rather than a `Judged` one that
///   would fail a `1.2691:1` hairline.
/// * everything else is [`ChromeClass::Judged`] against `3.0:1`.
///
/// [`ChromeClass::UnpaintedBackground`] is deliberately **not**
/// produced: no badge rests unpainted except the zero count, and that
/// case is not a badge at all — see [`BadgeModel::is_empty`].
#[must_use]
pub fn badge_chrome_class(style: &BadgeStyle, palette: &Palette) -> ChromeClass {
    if style.border_width == NO_BORDER_WIDTH {
        ChromeClass::CoincidentWithFill
    } else if same_channels(style.border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// The palette entries one tone resolves to.
///
/// The single place a semantic tone becomes colour. Keeping it here means
/// the mapping is auditable as one table and the model stays free of
/// paint, which is what lets a test assert "`Warning` means
/// `status.warning`" without rendering anything.
#[must_use]
pub fn resolve_tone(tone: BadgeTone, palette: &Palette) -> ToneColors {
    match tone {
        // §3.8 names four dot colours and five tones, so `Neutral` has no
        // dot the section can point at. `text.muted` is the resting mark
        // for a badge that reports nothing worth colouring, and the chip
        // wears `border.strong` rather than `border.subtle` **on
        // purpose**: a subtle hairline classifies as a decorative
        // divider and drops out of rule 3's measurement entirely, which
        // would let an *indicator* hide behind a decoration's exemption.
        BadgeTone::Neutral => ToneColors {
            text: palette.text_muted,
            border: palette.border_strong,
            dot_fill: palette.text_muted,
            solid_fill: palette.border_strong,
        },
        // The accent tone splits three ways, and §3.8 gives each channel a
        // different rung: the dot is `accent.border`, the chip's
        // perimeter is `accent.text`, and only the count pill's solid
        // body is `accent.fill`. Reading one colour for all three would
        // put the `2.5719:1` app colour behind the word.
        BadgeTone::Accent => ToneColors {
            text: palette.accent_text,
            border: palette.accent_text,
            dot_fill: palette.accent_border,
            solid_fill: palette.accent_fill,
        },
        BadgeTone::Success => ToneColors {
            text: palette.status_success,
            border: palette.status_success,
            dot_fill: palette.status_success,
            solid_fill: palette.status_success,
        },
        BadgeTone::Warning => ToneColors {
            text: palette.status_warning,
            border: palette.status_warning,
            dot_fill: palette.status_warning,
            solid_fill: palette.status_warning,
        },
        // §3.8's danger dot is `status.danger.text` and its chip border
        // is `status.danger.text` — the *text* rung, not the fill. The
        // fill appears only as the count pill's solid body, where rule 4
        // locks `text.strong` over it. Using the fill for the dot or the
        // perimeter would drop the mark to `3.6495:1` where the section
        // asks for `6.18:1`.
        BadgeTone::Danger => ToneColors {
            text: palette.status_danger_text,
            border: palette.status_danger_text,
            dot_fill: palette.status_danger_text,
            solid_fill: palette.status_danger_fill,
        },
    }
}

/// The three colour channels one tone supplies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneColors {
    /// The word colour.
    pub text: Color,
    /// The perimeter stroke colour.
    pub border: Color,
    /// The dot's fill.
    pub dot_fill: Color,
    /// The count pill's solid fill.
    pub solid_fill: Color,
}

/// The width of a perimeter that is not drawn.
///
/// Not a visual magnitude but the *absence* of one, which is why it is
/// allowed outside the token modules — and named, so the resolver reads
/// as "no stroke" rather than "zero point zero".
const NO_BORDER_WIDTH: f32 = 0.0;

/// The padding of a badge that has none.
const NO_PADDING: f32 = 0.0;

/// The minimum width of a badge that imposes no floor.
const NO_MIN_WIDTH: f32 = 0.0;

/// Whether two colours are the same channel-for-channel.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod badge_tests;
