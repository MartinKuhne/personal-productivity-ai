//! The segmented-control resolver and geometry — the joined strip of
//! §3.1b (`UIF-ATOM-001b`).
//!
//! This module resolves paint and computes boxes; it does not build a
//! widget. It is also **not** a second button. Every unselected segment is
//! [`button::resolve_style`] called with [`ButtonVariant::Secondary`],
//! and every selected segment is that same resolver pinned to the
//! `Pressed` cell — so the strip cannot drift from §3.1 the way a copied
//! button would.
//!
//! # The selected segment *is* the Secondary `Pressed` cell
//!
//! §3.1b specifies the selected segment as `BG: surface.active`,
//! `FG: text.strong` (`14.29:1`), `Border: 1px accent.border`
//! (`4.79:1`). That triple is, token for token, §3.1's Secondary
//! **Pressed** cell. So this resolver does not restate it: it asks
//! `button::resolve_style(Secondary, Pressed, palette)` and takes the
//! answer. Four consequences worth naming:
//!
//! * the fill/foreground pair goes through
//!   [`assert_fill_foreground_lock`](crate::token::contrast::assert_fill_foreground_lock)
//!   for free, because the button resolver already runs it;
//! * the ring is resolved against `surface.active` by the same adaptive
//!   rule that produced §3.1's Secondary ring, so it is `accent.border`
//!   here for the same arithmetic that makes it `accent.border` there;
//! * a future change to §3.1's Secondary pressed cell moves the selected
//!   segment with it, which is the point — the two are specified as the
//!   same paint and now *are* the same paint;
//! * the disabled selected segment is the Secondary **Disabled** cell,
//!   which is the crate-wide disabled convention §3.1 and §3.3 already
//!   share.
//!
//! # The selected paint is held through hover and press
//!
//! §3.1b gives the selected segment **one** cell and says nothing about
//! what it looks like under the pointer. Three readings were possible; the
//! one taken is that the selection paint does not move.
//!
//! The reason is the ladder. `surface.active` (`RGB(42,42,46)`) is
//! already lighter than `surface.hover` (`RGB(32,32,36)`), so a selected
//! segment that "responded" to hover by dropping to the hover rung would
//! go *darker* on the interaction that is supposed to say "you can act
//! on this", and a selected-and-pressed segment would have nowhere left to
//! go that was not darker still. Holding `surface.active` keeps a selected
//! segment above every hover rung in the palette at every pointer state,
//! which is the only reading that needs no invented token.
//!
//! **The finding that comes with that decision:** a *pressed unselected*
//! segment resolves to exactly the same three colours as a *selected* one
//! — both are `surface.active` / `text.strong` / `accent.border`. The
//! reserved `✓` is the only thing that tells them apart. That is not
//! fixable inside this atom: separating them would need a fourth surface
//! rung the palette does not have, or a selected fill the spec assigns to
//! `accent.fill` elsewhere. It is recorded here, pinned by a sidecar test
//! that asserts the collision rather than hiding it, and it is a spec
//! question rather than an implementation bug.
//!
//! # The `✓` slot is reserved on every segment
//!
//! §3.1b calls the leading `✓` "a reserved `14px` leading icon".
//! Reserved is the operative word, and for a strip it means more than it
//! does for a [`chip`](crate::widget::chip): if only the *selected*
//! segment carried the glyph, then selecting a different segment would
//! shift every label to its right, and a keyboard user roving the strip
//! would watch the whole control reflow on every arrow key.
//! `UIF-PRIN-003` forbids exactly that, so [`segment_width`] charges
//! the glyph box plus its gap to **every** segment whether or not that
//! segment is selected, and [`check_slot`] hands back the box that was
//! paid for. Selection changes which segment paints `✓` in; it moves no
//! text.
//!
//! # Focus is the strip's, not the segment's
//!
//! The ring goes around the **whole strip** (§3.0.2's `2px` outer ring,
//! one per control), not around the focused segment. A per-segment ring
//! would sit on the shared border between two neighbours and read as that
//! neighbour's edge, and it would make the ring's colour depend on which
//! segment's fill it happened to overlap. [`resolve_strip_ring`] therefore
//! resolves against the **mount** the strip sits on — the surface the ring
//! is actually drawn over — and the roving index is carried separately by
//! the model, because which segment holds the keyboard is not a paint
//! question.
//!
//! Unit tests live in the sibling `segmented_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button::{self, ButtonStyle, ButtonVariant};
use crate::widget::focus;
use crate::widget::segmented_model::SegmentedModel;
use iced::{Color, Point, Rectangle, Size};

/// The strip's outer corner radius — `radius.widget` (§3.1b,
/// `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The strip's perimeter border width — `border.hairline` (§3.1b).
///
/// Constant across every state: a strip that thickened its perimeter when
/// a segment was selected would move its own segments.
pub const PERIMETER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The focus ring width around the strip — `border.focus` (§3.0.2).
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The horizontal padding inside a segment — `space.md` (§3.1b,
/// `UIF-TOK-001`).
pub const SEGMENT_PADDING: f32 = space::MD;

/// The reserved leading `✓` box — `size.icon.sm` (`14px`, §3.1b).
pub const CHECK_GLYPH: f32 = size::ICON_SM;

/// Gap between the reserved `✓` box and the segment's label —
/// `space.sm` (§3.1b, `UIF-TOK-001`).
pub const CHECK_GAP: f32 = space::SM;

/// The width the reserved check slot costs a segment: the glyph box plus
/// its gap.
///
/// Charged to every segment by [`segment_width`], selected or not.
pub const CHECK_SLOT_WIDTH: f32 = CHECK_GLYPH + CHECK_GAP;

/// The compact strip height — `size.control.sm` (§3.1b).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// The standard strip height — `size.control.md` (§3.1b).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// Which of §3.1b's two segment polarities a cell belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SegmentPolarity {
    /// §3.1b's "Unselected segments resolve through `ButtonVariant::Secondary`"
    /// — and the hover/press/disabled rungs that go with it.
    #[default]
    Unselected,
    /// The selected cell: `surface.active` / `text.strong` /
    /// `accent.border`, plus the leading `✓`.
    Selected,
}

impl SegmentPolarity {
    /// The polarity of one segment of a model.
    #[must_use]
    pub fn of<Id: Clone + Ord>(model: &SegmentedModel<Id>, index: usize) -> Self {
        let Some(segment) = model.segments.get(index) else {
            return Self::Unselected;
        };
        if model.is_selected(&segment.id) {
            Self::Selected
        } else {
            Self::Unselected
        }
    }
}

/// A segment's resolved paint for one `(polarity, state)`.
///
/// A [`ButtonStyle`] rather than a new struct on purpose: the segment's
/// fill, foreground, border and ring are the *button atom's* four
/// channels, and giving the strip its own name for them would be the
/// first step toward two tables that disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentStyle {
    /// The resolved button style for this segment.
    pub paint: ButtonStyle,
    /// The leading `✓` colour — [`accent_text`](Palette::accent_text)
    /// while live, [`text_disabled`](Palette::text_disabled) when inert.
    pub check: Color,
    /// Whether the `✓` is painted in this cell.
    ///
    /// True only in the selected polarity. Kept separate from
    /// [`paint`](Self::paint) so a renderer knows whether to draw the
    /// glyph without re-deriving the polarity, and so the *reserved*
    /// slot in an unselected segment is not mistaken for a hidden one.
    pub shows_check: bool,
}

/// Resolve one segment's paint (`UIF-ATOM-001b`).
///
/// Unselected is a direct pass-through to
/// [`button::resolve_style`] with [`ButtonVariant::Secondary`]. Selected
/// is that same resolver pinned to `BaseState::Pressed`, with the
/// `focused` half carried across untouched so focus stays additive here
/// exactly as it is in §3.1.
///
/// The disabled selected segment maps to the Secondary `Disabled` cell
/// rather than holding `surface.active`: `text_disabled` over
/// `surface.active` is `4.2510:1`, which is inside the `4.0`–`6.0`
/// disabled band, but the crate's disabled convention is `surface.base`,
/// and a selected segment that stayed lit while dead would out-shine a
/// live unselected neighbour.
#[must_use]
pub fn resolve_style(
    polarity: SegmentPolarity,
    state: VisualState,
    palette: &Palette,
) -> SegmentStyle {
    let base = match polarity {
        SegmentPolarity::Unselected => state.base,
        SegmentPolarity::Selected if state.base == BaseState::Disabled => BaseState::Disabled,
        SegmentPolarity::Selected => BaseState::Pressed,
    };

    let paint = button::resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base,
            focused: state.focused,
        },
        palette,
    );

    SegmentStyle {
        paint,
        check: check_color(state, palette),
        shows_check: matches!(polarity, SegmentPolarity::Selected),
    }
}

/// The leading `✓` colour for one state.
///
/// `accent.text` is the token §3.1b names, and it is the *only* cue that
/// separates a selected segment from a pressed unselected one (see the
/// [module docs](self#the-selected-paint-is-held-through-hover-and-press)),
/// so it is never dropped while the control is live. A disabled strip
/// takes `text.disabled`, which keeps the mark present and stops it
/// claiming attention it cannot act on.
#[must_use]
pub const fn check_color(state: VisualState, palette: &Palette) -> Color {
    match state.base {
        BaseState::Disabled => palette.text_disabled,
        _ => palette.accent_text,
    }
}

/// The control-level focus ring for the whole strip.
///
/// Resolved against `mount` — the surface the strip is drawn on, and
/// therefore the surface the ring is drawn over — through the same
/// adaptive rule every atom in the crate uses (`UIF-TOK-006` rule 2,
/// decision **D-6**). Returns `None` when the state carries no ring.
#[must_use]
pub fn resolve_strip_ring(
    state: VisualState,
    mount: Color,
    palette: &Palette,
) -> Option<(Color, f32)> {
    if !state.ring_visible() {
        return None;
    }
    Some((
        focus::resolve_ring_color_for_background(mount, palette),
        FOCUS_RING_WIDTH,
    ))
}

/// The width one segment needs for its label.
///
/// The reserved check slot is charged in **unconditionally**, which is
/// the whole of the zero-shift guarantee for this control: the width a
/// segment gets does not depend on whether it is selected, so moving the
/// selection moves no label (`UIF-PRIN-003`).
#[must_use]
pub fn segment_width(label_width: f32) -> f32 {
    label_width.max(0.0) + CHECK_SLOT_WIDTH + SEGMENT_PADDING * 2.0
}

/// The width a whole strip needs for its labels, in display order.
#[must_use]
pub fn strip_width(label_widths: &[f32]) -> f32 {
    label_widths.iter().map(|width| segment_width(*width)).sum()
}

/// The rectangle of segment `index` inside a strip occupying `strip`.
///
/// Segments are laid out left to right at their own widths, flush to the
/// strip's leading edge, so the box this returns is the box
/// [`strip_width`] paid for.
#[must_use]
pub fn segment_bounds(strip: Rectangle, index: usize, label_widths: &[f32]) -> Rectangle {
    let offset: f32 = label_widths
        .iter()
        .take(index)
        .map(|width| segment_width(*width))
        .sum();
    let width = label_widths
        .get(index)
        .map(|width| segment_width(*width))
        .unwrap_or(0.0);

    Rectangle::new(
        Point::new(strip.x + offset, strip.y),
        Size::new(width, strip.height),
    )
}

/// The reserved leading `✓` box for a segment.
///
/// The box is the same rectangle whether or not the `✓` is painted in
/// it — that is what "reserved" buys — and it sits at the segment's
/// leading padding, vertically centred.
#[must_use]
pub fn check_slot(segment: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            segment.x + SEGMENT_PADDING,
            segment.y + (segment.height - CHECK_GLYPH) / 2.0,
        ),
        Size::new(CHECK_GLYPH, CHECK_GLYPH),
    )
}

/// The label rectangle for a segment: everything past the reserved slot
/// and its gap, inside the trailing padding.
#[must_use]
pub fn label_bounds(segment: Rectangle) -> Rectangle {
    let x = segment.x + SEGMENT_PADDING + CHECK_SLOT_WIDTH;
    Rectangle::new(
        Point::new(x, segment.y),
        Size::new(
            (segment.width - SEGMENT_PADDING * 2.0 - CHECK_SLOT_WIDTH).max(0.0),
            segment.height,
        ),
    )
}

/// Classify the strip's perimeter under `UIF-TOK-006` rule 3.
///
/// The perimeter is `border.strong` in every state, which against the
/// dark mounts measures `4.2379:1` on `surface.raised` and `4.5062:1`
/// on `surface.base` — comfortably [`Judged`](ChromeClass::Judged). A
/// perimeter that ever resolved to `border.subtle` would be a resting
/// divider and exempt, which is why the classification is by token
/// identity rather than a constant.
#[must_use]
pub fn perimeter_class(style: &SegmentStyle, palette: &Palette) -> ChromeClass {
    if same_channels(style.paint.border_color, style.paint.background) {
        ChromeClass::CoincidentWithFill
    } else if same_channels(style.paint.border_color, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Whether two colours carry identical channels.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

#[cfg(test)]
#[path = "segmented_tests.rs"]
mod segmented_tests;
