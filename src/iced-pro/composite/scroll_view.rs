//! ScrollView & annotated scrollbar contract (`UIF-COMP-004`).
//!
//! This module delivers the scrollbar as a **contract rather than a widget**: every
//! quantity Â§3B.4 names is a pure function, so the behaviours the requirement
//! actually asserts â€” that a growing track moves nothing, that a thumb never
//! collapses below a hittable length, that a drag settles exactly once â€” are
//! testable facts instead of things observed in a running app.
//!
//! # The layout-shift verdict: overlay, and the signature that proves it
//!
//! [`content_bounds`] takes the viewport, the [`LayoutMode`] and the [`Axis`] â€”
//! and **not** the track's phase. That omission is the whole verdict. Because the
//! content rectangle cannot see whether the track is at rest (`8px`) or expanded
//! under hover or drag (`10px`), the `2px` growth is paint-only and inward: no
//! hover can reflow a line of text, which is what
//! the `UIF-PRIN-003` invariant (`Layout Stability`) forbids.
//!
//! [`LayoutMode::Reserved`] is the classic variant, and it is priced honestly: a
//! host that reserves space permanently reserves it at the **widest** state, so
//! `TRACK_WIDTH_RESERVED` equals [`TRACK_WIDTH_ACTIVE`] rather than the resting
//! `8px`. Reserving the resting width and then growing on hover is the exact
//! reflow the overlay mode exists to avoid, and a reservation that cannot hold its
//! own expansion is not a reservation.
//!
//! # Auto-hide is a phase, not a timer
//!
//! Â§3B.4 does not ask the scrollbar to fade away on a countdown, and a countdown
//! would need a clock the crate does not own. [`TrackPhase`] is therefore a pure
//! function of two things the caller already knows â€” whether the view can scroll,
//! and where the pointer is ([`resolve_phase`]). A host that later wants a timed
//! fade feeds [`TimeMs`](crate::surface::tooltip::TimeMs) in from outside and maps *that* to
//! `Hidden`; nothing in this module reads a wall clock to decide what to paint.
//!
//! # The thumb is `Judged` chrome
//!
//! Under Â§2.5's four-class model the thumb is classified
//! [`ChromeClass::Judged`] â€” measured directly at the `3.00:1` floor â€” and not
//! [`ChromeClass::CoincidentWithFill`]. The distinction is real: the thumb has no
//! perimeter stroke set to its own fill token, it *is* the fill, drawn against
//! whatever surface scrolls beneath it. `CoincidentWithFill`'s higher floor was
//! derived for button fills that carry a same-token outline, and borrowing it here
//! would price a number the thumb does not actually have to meet.
//!
//! The track itself paints **no fill in any state**. Â§3B.4 styles only the thumb;
//! a track fill would be an invented surface that then has to be contrast-graded
//! against every surface it can appear over.
//!
//! # Contrast, recomputed
//!
//! The spec prints the thumb's rest/hover ratio as a range, `4.01:1`â€“`4.51:1`.
//! Recomputing from raw sRGB shows that range only spans the thumb over
//! `surface.base` and `surface.overlay`; it never prices the thumb over the two
//! surfaces a scrollbar actually passes during interaction. The full set:
//!
//! | `border.strong` over | Ratio |
//! | --- | --- |
//! | `surface.base` | 4.5854:1 |
//! | `surface.sunken` | 4.5062:1 |
//! | `surface.raised` | 4.2379:1 |
//! | `surface.code` | 4.1672:1 |
//! | `surface.overlay` | 4.0128:1 |
//! | `surface.hover` | 3.6772:1 |
//! | `surface.active` | 3.2377:1 |
//!
//! Every figure clears the `Judged` floor of `3.00:1`, including the two the
//! spec's printed range omits. The disagreement is that the printed range is
//! narrower than the contract needs: it is stated over the two calmest surfaces
//! and silently excludes the hovered and pressed ones, where the margin is
//! `0.68:1` and `0.24:1` rather than `1.0:1`+. The sidecar asserts the full
//! matrix so a future retune of `surface.active` that pushed the thumb under
//! `3.00:1` fails a test rather than a user.
//!
//! Unit tests live in the sibling `scroll_view_tests.rs` sidecar.

use crate::binding::{self, Cancel, Commit, GestureSlot, Live};
use crate::token::color::Palette;
use crate::token::contrast::{ChromeClass, contrast_ratio};
use crate::token::{shape, size, space, typography};
use iced::{Color, Rectangle};

/// Track width at rest â€” `space.sm` (Â§3B.4's `8px`).
pub const TRACK_WIDTH_REST: f32 = space::SM;

/// Track width under hover or drag â€” `size.splitter.grab` (Â§3B.4's `10px`).
pub const TRACK_WIDTH_ACTIVE: f32 = size::SPLITTER_GRAB;

/// A reserved gutter's width.
///
/// Priced at the widest state on purpose: a gutter that reserves the resting
/// `8px` and then grows to `10px` on hover reflows exactly the content a
/// reservation is supposed to protect. See the module docs.
pub const TRACK_WIDTH_RESERVED: f32 = TRACK_WIDTH_ACTIVE;

/// The thumb's minimum length â€” `size.hit_target.min` (Â§3B.4's `24px`).
pub const THUMB_MIN_LENGTH: f32 = size::HIT_TARGET_MIN;

/// The tick annotation's width â€” `space.xxs` (Â§3B.4's `2px`).
pub const TICK_WIDTH: f32 = space::XXS;

/// The thumb's corner radius â€” `radius.pill` (Â§3B.4).
pub const THUMB_RADIUS: f32 = shape::RADIUS_PILL;

/// The keyboard's single-line scroll step â€” one body line, `type.body.line_height`.
///
/// **An invented value.** Â§3B.4 lists the keys but prints no distance. A line
/// step is chosen rather than a fixed pixel count so the scroll moves by the thing
/// the user is reading, and the body rung is the crate's own reading measure.
pub const ARROW_STEP: f32 = typography::BODY.line_height;

/// The line of context kept visible when paging, so a `PageDown` never leaves the
/// reader with no anchor to the previous page.
///
/// **An invented value.** Â§3B.4 prints no page step.
pub const PAGE_KEEP: f32 = space::MD;

/// How close to the bottom counts as "at bottom" for stream auto-scroll
/// (`at_bottom_within_2px`, `UIF-MOV-004`) â€” `space.xxs` (Â§3B.4).
pub const AT_BOTTOM_TOLERANCE: f32 = space::XXS;

/// How far from the end of content triggers lazy pagination â€” Â§3B.4's `64px`.
///
/// A behavioural distance rather than a visual one, so it is not a Surface
/// Geometry Constant: nothing in the crate paints `64px`.
pub const REACH_END_DISTANCE: f32 = 64.0;

/// The chrome class the thumb is graded under. See the module docs for why.
pub const THUMB_CHROME_CLASS: ChromeClass = ChromeClass::Judged;

/// Which axis a scroll view scrolls along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Vertical scrolling; the track sits on the trailing vertical edge.
    Vertical,
    /// Horizontal scrolling; the track sits on the bottom horizontal edge.
    Horizontal,
}

impl Axis {
    /// The other axis.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }
}

/// How the track's width is paid for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// The track floats over the content; content keeps the whole viewport.
    #[default]
    Overlay,
    /// A gutter is carved out of the viewport permanently, at the widest price.
    Reserved,
}

/// The scrollbar's current visual phase.
///
/// A pure function of scrollability and pointer position â€” see
/// [`resolve_phase`]. There is no timed member, because a timed member would need
/// a clock this crate does not own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackPhase {
    /// Nothing to scroll, or the host has faded the bar out. Paints nothing.
    Hidden,
    /// Scrollable, pointer elsewhere.
    Rest,
    /// Pointer over the track or thumb.
    Hovered,
    /// The thumb is under an in-flight drag.
    Dragged,
}

/// Whether the view can scroll along an axis at all.
#[must_use]
pub fn is_scrollable(content_len: f32, viewport_len: f32) -> bool {
    content_len > viewport_len
}

/// Resolve the phase from scrollability, pointer position and drag state.
///
/// Drag outranks hover, and hover outranks rest. A view that cannot scroll is
/// `Hidden` even with the pointer over where its track would be: a bar you cannot
/// move is worse than no bar, because it invites an interaction that does nothing.
#[must_use]
pub fn resolve_phase(scrollable: bool, pointer_over: bool, dragging: bool) -> TrackPhase {
    if !scrollable {
        TrackPhase::Hidden
    } else if dragging {
        TrackPhase::Dragged
    } else if pointer_over {
        TrackPhase::Hovered
    } else {
        TrackPhase::Rest
    }
}

/// The painted track width for a phase and a mode.
///
/// In [`LayoutMode::Overlay`] a `Hidden` phase paints nothing, so the width is
/// `0.0`. In [`LayoutMode::Reserved`] the width is constant across every phase â€”
/// that constancy is the mode's entire promise.
#[must_use]
pub fn track_width(phase: TrackPhase, mode: LayoutMode) -> f32 {
    match mode {
        LayoutMode::Reserved => TRACK_WIDTH_RESERVED,
        LayoutMode::Overlay => match phase {
            TrackPhase::Hidden => 0.0,
            TrackPhase::Rest => TRACK_WIDTH_REST,
            TrackPhase::Hovered | TrackPhase::Dragged => TRACK_WIDTH_ACTIVE,
        },
    }
}

/// The content rectangle inside a viewport.
///
/// **No phase parameter.** The content box cannot see whether the track is at
/// `8px` or `10px`, so a hover cannot reflow it. This signature *is* the
/// layout-shift verdict.
#[must_use]
pub fn content_bounds(viewport: Rectangle, mode: LayoutMode, axis: Axis) -> Rectangle {
    let gutter = match mode {
        LayoutMode::Overlay => 0.0,
        LayoutMode::Reserved => TRACK_WIDTH_RESERVED,
    };
    match axis {
        Axis::Vertical => Rectangle {
            x: viewport.x,
            y: viewport.y,
            width: (viewport.width - gutter).max(0.0),
            height: viewport.height.max(0.0),
        },
        Axis::Horizontal => Rectangle {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width.max(0.0),
            height: (viewport.height - gutter).max(0.0),
        },
    }
}

/// The track rectangle, laid against the viewport's trailing edge on `axis`.
///
/// The width comes straight from [`track_width`] for the phase and mode, so the
/// painted bar and the reserved gutter are the same number the layout used.
#[must_use]
pub fn track_bounds(
    viewport: Rectangle,
    phase: TrackPhase,
    mode: LayoutMode,
    axis: Axis,
) -> Rectangle {
    let width = track_width(phase, mode);
    match axis {
        Axis::Vertical => Rectangle {
            x: viewport.x + viewport.width - width,
            y: viewport.y,
            width,
            height: viewport.height.max(0.0),
        },
        Axis::Horizontal => Rectangle {
            x: viewport.x,
            y: viewport.y + viewport.height - width,
            width: viewport.width.max(0.0),
            height: width,
        },
    }
}

/// The furthest a view can scroll along an axis.
#[must_use]
pub fn max_offset(content_len: f32, viewport_len: f32) -> f32 {
    (content_len - viewport_len).max(0.0)
}

/// Clamp an offset into the scrollable range.
#[must_use]
pub fn clamp_offset(offset: f32, content_len: f32, viewport_len: f32) -> f32 {
    offset.clamp(0.0, max_offset(content_len, viewport_len))
}

/// The thumb's length along the axis.
///
/// The proportional length is `viewportÂ² / content` â€” the standard expression,
/// because a thumb should occupy the same fraction of the track that the viewport
/// occupies of the content. It is then floored at [`THUMB_MIN_LENGTH`] so an
/// extreme ratio cannot produce a thumb too small to hit, and capped at the track
/// length so a scrollable-by-one-pixel view does not overflow.
///
/// The floor is the interesting case: at `content = 10 000`, `viewport = 100`
/// the proportional length is `1.0px`, which the floor lifts to `24px`. That
/// makes the thumb *less* accurate at extreme ratios, and the trade is deliberate
/// â€” a `1px` target is not a target, and Â§3B.4 names the floor rather than the
/// accuracy.
#[must_use]
pub fn thumb_length(content_len: f32, viewport_len: f32) -> f32 {
    if viewport_len <= 0.0 {
        return THUMB_MIN_LENGTH;
    }
    let proportional = if content_len > 0.0 {
        (viewport_len * viewport_len) / content_len
    } else {
        viewport_len
    };
    proportional
        .max(THUMB_MIN_LENGTH)
        .min(viewport_len.max(THUMB_MIN_LENGTH))
}

/// The thumb's leading offset along the track for a scroll offset.
///
/// At the floor length the mapping is compressed into the track's remaining
/// travel, so the thumb still starts at the top at offset `0` and still reaches
/// the far end at `max_offset`.
#[must_use]
pub fn thumb_offset(offset: f32, content_len: f32, viewport_len: f32, thumb_len: f32) -> f32 {
    let max = max_offset(content_len, viewport_len);
    if max <= 0.0 {
        return 0.0;
    }
    let travel = (viewport_len - thumb_len).max(0.0);
    (offset / max) * travel
}

/// The thumb's rectangle inside a track.
///
/// The axis is passed rather than inferred from the track's shape: a square-ish
/// track at a narrow size would otherwise flip which edge the thumb travels along,
/// and a scrollbar that changes axis because of a resize is a worse failure than
/// one that requires its caller to say what it is.
#[must_use]
pub fn thumb_rect(
    track: Rectangle,
    offset: f32,
    content_len: f32,
    viewport_len: f32,
    axis: Axis,
) -> Rectangle {
    let thumb_len = thumb_length(content_len, viewport_len);
    let lead = thumb_offset(offset, content_len, viewport_len, thumb_len);
    match axis {
        Axis::Vertical => Rectangle {
            x: track.x,
            y: track.y + lead,
            width: track.width,
            height: thumb_len.min(track.height.max(0.0)),
        },
        Axis::Horizontal => Rectangle {
            x: track.x + lead,
            y: track.y,
            width: thumb_len.min(track.width.max(0.0)),
            height: track.height,
        },
    }
}

/// The thumb's fill for a phase.
///
/// `border.strong` at rest and hover, `accent.border` while dragged (Â§3B.4). The
/// two are identical between rest and hover because that is what the spec prints;
/// the hover affordance is the track widening, not the thumb changing colour.
#[must_use]
pub fn thumb_color(phase: TrackPhase, palette: &Palette) -> Color {
    match phase {
        TrackPhase::Hidden => Color::TRANSPARENT,
        TrackPhase::Rest | TrackPhase::Hovered => palette.border_strong,
        TrackPhase::Dragged => palette.accent_border,
    }
}

/// Whether a thumb colour clears the [`THUMB_CHROME_CLASS`] floor over a surface.
#[must_use]
pub fn thumb_clears_chrome_floor(thumb: Color, surface: Color) -> bool {
    contrast_ratio(thumb, surface) >= chrome_floor()
}

/// The contrast the thumb must reach, taken from its chrome class rather than
/// hard-coded.
#[must_use]
pub fn chrome_floor() -> f32 {
    THUMB_CHROME_CLASS
        .minimum_contrast()
        .unwrap_or(f32::INFINITY)
}

/// Whether the view is at bottom within [`AT_BOTTOM_TOLERANCE`] (`UIF-MOV-004`).
#[must_use]
pub fn at_bottom_within_2px(offset: f32, content_len: f32, viewport_len: f32) -> bool {
    max_offset(content_len, viewport_len) - offset <= AT_BOTTOM_TOLERANCE
}

/// Whether the offset has entered the final [`REACH_END_DISTANCE`] of content.
#[must_use]
pub fn in_reach_end(offset: f32, content_len: f32, viewport_len: f32) -> bool {
    let max = max_offset(content_len, viewport_len);
    max > 0.0 && max - offset <= REACH_END_DISTANCE
}

/// The distance a page scroll travels: the viewport less the context that stays
/// visible, never less than one line.
#[must_use]
pub fn page_step(viewport_len: f32) -> f32 {
    (viewport_len - PAGE_KEEP).max(ARROW_STEP)
}

/// A keyboard scroll request (Â§3B.4's `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollKey {
    /// `Up` â€” one line toward the start.
    Up,
    /// `Down` â€” one line toward the end.
    Down,
    /// `PageUp` â€” one page toward the start.
    PageUp,
    /// `PageDown` â€” one page toward the end.
    PageDown,
    /// `Home` â€” the start of content.
    Home,
    /// `End` â€” the end of content.
    End,
}

/// Resolve a keyboard scroll to a new offset, or `None` when nothing would move.
///
/// Suppressing the no-op is part of the contract: a host that persists every
/// offset it is handed would write a scroll event the user did not perform.
#[must_use]
pub fn scroll_to_key(
    offset: f32,
    key: ScrollKey,
    content_len: f32,
    viewport_len: f32,
) -> Option<f32> {
    let current = clamp_offset(offset, content_len, viewport_len);
    let next = match key {
        ScrollKey::Up => current - ARROW_STEP,
        ScrollKey::Down => current + ARROW_STEP,
        ScrollKey::PageUp => current - page_step(viewport_len),
        ScrollKey::PageDown => current + page_step(viewport_len),
        ScrollKey::Home => 0.0,
        ScrollKey::End => max_offset(content_len, viewport_len),
    };
    let next = clamp_offset(next, content_len, viewport_len);
    (next != current).then_some(next)
}

/// What a tick annotation marks (Â§3B.4: search hits, headings, diagnostic errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickKind {
    /// A search hit.
    SearchHit,
    /// A document heading.
    Heading,
    /// A diagnostic error.
    Error,
}

/// A tick annotation positioned in content space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollTick {
    /// The position along the scrolled axis, in content coordinates.
    pub position: f32,
    /// What it marks.
    pub kind: TickKind,
}

/// The tick's rectangle on a track.
///
/// Ticks are positioned against the **track**, not the content, and are clamped
/// inside it, so a tick near the end of a long document still lands on the bar
/// rather than off its bottom edge.
#[must_use]
pub fn tick_rect(track: Rectangle, tick: &ScrollTick, content_len: f32, axis: Axis) -> Rectangle {
    let track_len = match axis {
        Axis::Vertical => track.height,
        Axis::Horizontal => track.width,
    };
    let lead = if content_len > 0.0 {
        (tick.position / content_len) * track_len.max(0.0)
    } else {
        0.0
    };
    let lead = lead.clamp(0.0, (track_len - TICK_WIDTH).max(0.0));
    match axis {
        Axis::Vertical => Rectangle {
            x: track.x,
            y: track.y + lead,
            width: track.width,
            height: TICK_WIDTH,
        },
        Axis::Horizontal => Rectangle {
            x: track.x + lead,
            y: track.y,
            width: TICK_WIDTH,
            height: track.height,
        },
    }
}

/// An in-flight thumb drag.
///
/// The same transcript shape [`GestureSlot`] defines, over the scroll offset:
/// `begin` captures the offset the user was looking at, each pointer move emits a
/// [`Live`] that the host may coalesce, `release` commits once, and `cancel`
/// restores the captured offset. A drag that was never begun commits nothing, so a
/// stray release cannot scroll the view.
#[derive(Clone, Debug, Default)]
pub struct ThumbDrag {
    offset: f32,
    slot: GestureSlot<f32>,
}

impl ThumbDrag {
    /// A drag at rest at `offset`.
    #[must_use]
    pub fn new(offset: f32) -> Self {
        Self {
            offset,
            slot: GestureSlot::new(),
        }
    }

    /// The current offset.
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Whether a drag is in flight.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.slot.is_open()
    }

    /// Begin the drag against the current offset.
    #[must_use]
    pub fn begin(&mut self) -> binding::GestureStart {
        self.slot.begin(&self.offset)
    }

    /// Move the thumb: emit a live tick through the open drag.
    ///
    /// The value is clamped before it is emitted, so a host that writes the live
    /// value straight into its scroll position cannot be pushed past the end of
    /// the content mid-drag.
    #[must_use]
    pub fn drag(&mut self, offset: f32, content_len: f32, viewport_len: f32) -> Option<Live<f32>> {
        let next = clamp_offset(offset, content_len, viewport_len);
        let live = self.slot.live(next)?;
        self.offset = next;
        Some(live)
    }

    /// Settle the drag: commit once.
    pub fn release(&mut self) -> Option<Commit<f32>> {
        self.slot.commit(self.offset)
    }

    /// Escape the drag: restore the offset captured when it began.
    pub fn cancel(&mut self) -> Option<Cancel<f32>> {
        let outcome = self.slot.cancel()?;
        self.offset = *outcome.restored();
        Some(outcome)
    }
}

#[cfg(test)]
#[path = "scroll_view_tests.rs"]
mod scroll_view_tests;
