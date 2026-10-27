//! The virtualised row viewport — the recycling primitive under every
//! long list in the framework (`UIF-COMP-006`, decision **D-8**).
//!
//! # What is proved here, and what is merely asserted elsewhere
//!
//! A virtualised list is a claim: *only the rows you can see are built*.
//! Stated in a widget that claim is invisible, because the widget that
//! realises the rows is the same code that draws them, and a demo with a
//! hundred rows never tests it. Stated as a pure function from
//! `(total, row height, viewport, offset)` to a row range, the claim
//! becomes arithmetic that a test can drive to a hundred thousand rows in
//! a millisecond and check exactly. That is why this module is a
//! resolver and not a widget, and it is the same shape
//! [`scroll_view`] took: the contract is
//! the deliverable.
//!
//! Three things are checked in the sidecar rather than observed:
//!
//! * **Boundedness.** The realised count never exceeds
//!   [`VirtualMetrics::realized_count_bound`], a closed-form ceiling that
//!   contains no term for the total.
//! * **Independence from total.** A list of `100 000` rows and a list of
//!   `1 000 000` rows, same viewport, same offset, realise *the same
//!   number of rows*. This is the property that makes the primitive worth
//!   having; a list that realises `total / 100` is not virtualised no
//!   matter what it is called.
//! * **No accumulation.** Every position is `index × height`, computed
//!   fresh. Nothing in this module adds a height to a running total.
//!
//! # Fixed row height, and the API that refuses to pretend otherwise
//!
//! §3B.6 prints two row heights and no third: `24px` compact, `28px`
//! standard. This module treats that as a closed set ([`RowRung`]) and
//! **does not support variable heights at all** — not as an
//! unimplemented option but as a type that cannot express one.
//! [`VirtualMetrics`] carries a single `row_height`, and there is no
//! call anywhere that takes a per-row height, a height callback, or a
//! measured-content input.
//!
//! The reason is the one this crate has already been burned by. A
//! variable-height list must know where row *n* starts, and the only
//! cheap way to know is a prefix sum of the heights before it. Done in
//! `f32` by accumulation, that sum drifts: [`ACCUMULATION_ILLUSTRATION`]
//! shows a `0.1`-unit step losing its last bit inside a thousand
//! additions, which is a list where row 900 no longer starts where the
//! scrollbar says it does. [`RangeModel`](crate::widget::range_model)
//! exists in this crate precisely because the same trap was already
//! stepped in once with slider steps — it re-derives the grid index from
//! the base rather than accumulating, and this module applies the same
//! rule by having nothing to accumulate.
//!
//! A variable-height list *can* be built without drift — an integer-unit
//! offset index, or a balanced tree carrying exact prefix sums — but that
//! is a different, heavier primitive, and nothing in §3B prints a row
//! that needs it. Choosing the cheaper thing and saying so is better than
//! offering a `Fn(usize) -> f32` that quietly reintroduces the drift the
//! crate already paid to learn about.
//!
//! The consequence for the migration decision **D-8** creates is stated
//! plainly in [`crate::composite::tree_view`]: `fastmd`'s current tree
//! rows are `15px` tall, below the `24px` hit-target floor
//! (`UIF-VER-002`), and cannot be handed to this primitive without the
//! rows getting taller. That is recorded as a gap rather than solved by
//! inventing a third rung.
//!
//! # The thumb is priced against the whole list, not the realised slice
//!
//! A virtualised list's scrollbar is a lie if it is computed from the
//! rows that exist this frame: a `40`-row window into a `100 000`-row
//! document would draw a thumb nearly as tall as its track. Every thumb
//! quantity here goes through [`scroll_view`] with
//! [`VirtualMetrics::content_len`] — the *logical* height of all
//! `total` rows — so the thumb shrinks as the document grows even though
//! the realised rows do not change.
//!
//! [`scroll_view`]: crate::composite::scroll_view
//!
//! Unit tests live in the sibling `virtual_list_tests.rs` sidecar.

use crate::composite::scroll_view;
use crate::token::size;
use iced::Rectangle;
use std::ops::Range;

/// Narrows an `f64` to `f32`, saturating at the finite extremes.
///
/// `f32: TryFrom<f64>` does not exist because the conversion is lossy, and
/// it *is* lossy here — but the loss is a scroll extent rounding by a
/// fraction of a pixel at the far end of a million-row list, never a row
/// position near the top. Saturating rather than panicking keeps a hostile
/// `total_rows` from taking the process down.
fn saturate_f32(value: f64) -> f32 {
    if !value.is_finite() {
        return f32::MAX;
    }
    if value >= f64::from(f32::MAX) {
        f32::MAX
    } else if value <= f64::from(f32::MIN) {
        f32::MIN
    } else {
        value as f32
    }
}

/// Narrows a non-negative `f64` to a `usize`, clamping to `0..=cap`.
///
/// A non-finite or negative input reads as `0`: there is no row that is
/// "minus three", and pretending otherwise would index outside the list.
fn clamp_index(value: f64, cap: usize) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    if value >= cap as f64 {
        cap
    } else {
        value as usize
    }
}

/// Relative tolerance for deciding that a quotient names a whole row.
///
/// `f64::EPSILON` cannot be used for this. It is `2.22e-16`, which is
/// far smaller than the spacing between representable values once the
/// magnitude passes about `32` - so `48.0 - f64::EPSILON` *is* `48.0`,
/// and a boundary test built on it silently never fires. The tolerance
/// has to scale with the quantity being tested, which is what
/// [`names_whole_rows`] does.
const QUOTIENT_TOLERANCE: f64 = 1e-9;

/// Whether `quotient` is close enough to a whole number to be read as one.
fn names_whole_rows(quotient: f64) -> bool {
    if !quotient.is_finite() {
        return false;
    }
    let rounded = quotient.round();
    (quotient - rounded).abs() <= quotient.abs().max(1.0) * QUOTIENT_TOLERANCE
}

/// The compact row rung — §3B.6's `24px`, which is also
/// `size.hit_target.min`.
///
/// The two figures being the same number is not a coincidence the spec
/// could have got wrong: a row is the smallest interactive thing in a
/// list, so its floor *is* the hit-target floor.
pub const COMPACT_ROW_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// The standard row rung — §3B.6's `28px`, which is `size.control.sm`.
pub const STANDARD_ROW_HEIGHT: f32 = size::CONTROL_SM;

/// The rows realised beyond each edge of the viewport by default.
///
/// **An invented value.** §3B.6 says only "rendering only visible rows"
/// and prints no margin. The margin exists so a scroll of a few pixels
/// does not have to build a row on the frame it becomes visible: rows
/// are built while they are still off-screen and reused as they enter.
/// Three is enough to cover a fast wheel notch at either rung without
/// turning the margin into a second viewport.
///
/// A behavioural count rather than a visual measurement, so it carries
/// no token — nothing paints "three rows".
pub const DEFAULT_OVERSCAN_ROWS: usize = 3;

/// A worked illustration of why this module refuses variable heights.
///
/// `0.1` added to itself a thousand times in `f32`, against the product
/// `1000 × 0.1`. Exposed as a constant so the drift is a named fact in
/// the crate rather than a claim in a comment, and asserted in the
/// sidecar.
///
/// The point is not that `0.1` is evil; it is that *any* height derived
/// from measured content — a wrapped label, a two-line tile in a
/// one-line list — lands on a value like this, and a prefix sum over a
/// long list walks that error into every row below it.
pub const ACCUMULATION_ILLUSTRATION: f32 = {
    let mut sum = 0.0_f32;
    let mut index = 0;
    while index < 1000 {
        sum += 0.1;
        index += 1;
    }
    sum
};

/// The exact value [`ACCUMULATION_ILLUSTRATION`] was trying to reach.
pub const ACCUMULATION_TARGET: f32 = 100.0;

/// The two row rungs §3B.6 permits.
///
/// An enum rather than a free `f32` on purpose: the closed set *is* the
/// API's honesty. A caller cannot pass a height the spec does not print,
/// and cannot pass a per-row height at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RowRung {
    /// `24px` — the compact rung, and the hit-target floor.
    #[default]
    Compact,
    /// `28px` — the standard rung.
    Standard,
}

impl RowRung {
    /// The rung's height.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Compact => COMPACT_ROW_HEIGHT,
            Self::Standard => STANDARD_ROW_HEIGHT,
        }
    }

    /// Whether this rung satisfies the `UIF-VER-002` hit-target floor.
    #[must_use]
    pub const fn meets_hit_target_floor(self) -> bool {
        match self {
            Self::Compact => COMPACT_ROW_HEIGHT >= size::HIT_TARGET_MIN,
            Self::Standard => STANDARD_ROW_HEIGHT >= size::HIT_TARGET_MIN,
        }
    }
}

/// The four quantities a virtualised viewport is a function of.
///
/// Constructed once per frame from what the host already knows. Every
/// method takes only an offset and returns only indices and rectangles, so
/// the whole mapping is checkable with no renderer, no widget tree and no
/// clock.
///
/// # Sanitisation
///
/// A non-finite or negative input collapses rather than propagating:
/// `viewport_len` falls to `0.0` (nothing is visible), `row_height`
/// falls to the compact rung (a zero-height row would make every offset
/// map to the same row and the list would stop scrolling). The rule is
/// the crate's own — [`RangeModel::clamp`](crate::widget::range_model)
/// documents that `f32::clamp` passes `NaN` straight through, so
/// finiteness is tested *before* any narrowing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualMetrics {
    /// Every row the list has, realised or not.
    total_rows: usize,
    /// The height of one row. Identical for all of them.
    row_height: f32,
    /// The viewport's length along the scrolled axis.
    viewport_len: f32,
    /// Rows realised beyond each edge of the visible range.
    overscan: usize,
}

impl VirtualMetrics {
    /// Build the metrics for one frame.
    ///
    /// A [`RowRung`] is the normal way to supply the height; the
    /// free-height constructor is [`with_row_height`](Self::with_row_height),
    /// which exists so a host that has already resolved a rung to a
    /// number need not re-name it.
    #[must_use]
    pub fn new(total_rows: usize, rung: RowRung, viewport_len: f32) -> Self {
        Self::with_row_height(
            total_rows,
            rung.height(),
            viewport_len,
            DEFAULT_OVERSCAN_ROWS,
        )
    }

    /// The full constructor, with the overscan margin stated.
    #[must_use]
    pub fn with_row_height(
        total_rows: usize,
        row_height: f32,
        viewport_len: f32,
        overscan: usize,
    ) -> Self {
        Self {
            total_rows,
            row_height: if row_height.is_finite() && row_height > 0.0 {
                row_height
            } else {
                COMPACT_ROW_HEIGHT
            },
            viewport_len: if viewport_len.is_finite() && viewport_len > 0.0 {
                viewport_len
            } else {
                0.0
            },
            overscan,
        }
    }

    /// The row height in force.
    #[must_use]
    pub const fn row_height(&self) -> f32 {
        self.row_height
    }

    /// The viewport length in force.
    #[must_use]
    pub const fn viewport_len(&self) -> f32 {
        self.viewport_len
    }

    /// The overscan margin in force.
    #[must_use]
    pub const fn overscan(&self) -> usize {
        self.overscan
    }

    /// Every row the list has.
    #[must_use]
    pub const fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// The **logical** height of the whole list.
    ///
    /// Computed in `f64` and narrowed once. At `1 000 000 × 24` the
    /// product is `24 000 000`, past the `2^24` at which `f32` stops
    /// representing every integer, so doing the multiply in `f32` would
    /// round the document's own length. The narrowing is lossy at the far
    /// end and deliberately so: the value is a scroll extent, not an
    /// index, and one rounded pixel at the bottom of a million-row list
    /// is a smaller error than a drifted row near the top.
    #[must_use]
    pub fn content_len(&self) -> f32 {
        let total = self.total_rows as f64;
        saturate_f32(total * self.row_height as f64)
    }

    /// The furthest the list can scroll.
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        scroll_view::max_offset(self.content_len(), self.viewport_len)
    }

    /// Whether the list scrolls at all.
    #[must_use]
    pub fn is_scrollable(&self) -> bool {
        scroll_view::is_scrollable(self.content_len(), self.viewport_len)
    }

    /// The first row intersecting the viewport at `offset`.
    #[must_use]
    pub fn first_visible(&self, offset: f32) -> usize {
        if self.total_rows == 0 || self.viewport_len <= 0.0 {
            return 0;
        }
        let offset = self.clamped(offset);
        let quotient = f64::from(offset) / f64::from(self.row_height);
        let index = if names_whole_rows(quotient) {
            quotient.round()
        } else {
            quotient.floor()
        };
        clamp_index(index, self.total_rows.saturating_sub(1))
    }

    /// The row range intersecting the viewport at `offset`, exclusive at
    /// the end.
    ///
    /// The end is derived from the *bottom edge* rather than by counting
    /// rows from the start, so a viewport that ends exactly on a row
    /// boundary does not realise the row it merely touches.
    #[must_use]
    pub fn visible_range(&self, offset: f32) -> Range<usize> {
        if self.total_rows == 0 || self.viewport_len <= 0.0 {
            return 0..0;
        }
        let offset = self.clamped(offset);
        let bottom = f64::from(offset) + f64::from(self.viewport_len);
        let quotient = bottom / f64::from(self.row_height);
        // The exclusive end is the first row whose top sits at or past the
        // viewport's bottom edge. A quotient that names a whole row *is*
        // that boundary, so it is rounded to the row it names rather than
        // rounded up past it.
        let end = if names_whole_rows(quotient) {
            quotient.round()
        } else {
            quotient.ceil()
        };
        let first = self.first_visible(offset);
        let end = clamp_index(end, self.total_rows);
        first..end.max(first)
    }

    /// The rows that must be realised: the visible range plus the
    /// overscan margin on both sides, clamped to the list.
    #[must_use]
    pub fn realized_range(&self, offset: f32) -> Range<usize> {
        let visible = self.visible_range(offset);
        if visible.is_empty() {
            return visible;
        }
        let first = visible.start.saturating_sub(self.overscan);
        let last = visible
            .end
            .saturating_add(self.overscan)
            .min(self.total_rows);
        first..last.max(first)
    }

    /// How many rows are realised at `offset`.
    #[must_use]
    pub fn realized_count(&self, offset: f32) -> usize {
        let range = self.realized_range(offset);
        range.end - range.start
    }

    /// Whether `index` is realised at `offset`.
    #[must_use]
    pub fn is_realized(&self, index: usize, offset: f32) -> bool {
        self.realized_range(offset).contains(&index)
    }

    /// The closed-form ceiling on [`realized_count`](Self::realized_count).
    ///
    /// `ceil(viewport / height) + 1 + 2 × overscan`, with **no term for
    /// `total_rows`**. The `+1` covers the partial row at the bottom of
    /// the viewport when the offset is not a whole multiple of the
    /// height. This is the bound the sidecar checks the realised count
    /// against at every offset, and the reason the independence-from-
    /// total proof is not a spot check.
    #[must_use]
    pub fn realized_count_bound(&self) -> usize {
        let whole = if self.viewport_len <= 0.0 || self.row_height <= 0.0 {
            0
        } else {
            (f64::from(self.viewport_len) / f64::from(self.row_height)).ceil() as usize
        };
        whole + 1 + self.overscan.saturating_mul(2)
    }

    /// The top of row `index`, in content coordinates.
    ///
    /// A single multiply. There is no accumulator in this module to
    /// drift, and the sidecar asserts this equals the product for every
    /// index in a hundred-thousand-row list.
    #[must_use]
    pub fn row_top(&self, index: usize) -> f32 {
        saturate_f32(index as f64 * self.row_height as f64)
    }

    /// The bottom edge of row `index`.
    #[must_use]
    pub fn row_bottom(&self, index: usize) -> f32 {
        saturate_f32(index as f64 * self.row_height as f64 + self.row_height as f64)
    }

    /// Row `index` laid out inside a viewport.
    ///
    /// `viewport` is the rectangle the list occupies; the row spans its
    /// full width and sits at `viewport.y + row_top − offset`, so the
    /// same call produces the on-screen box for whatever offset the list
    /// is scrolled to.
    #[must_use]
    pub fn row_rect(&self, index: usize, viewport: Rectangle, offset: f32) -> Rectangle {
        Rectangle {
            x: viewport.x,
            y: viewport.y + self.row_top(index) - self.clamped(offset),
            width: viewport.width.max(0.0),
            height: self.row_height,
        }
    }

    /// Which row covers `position` in content coordinates.
    ///
    /// `None` past the end of the list, and for a negative position:
    /// there is no row above the first one, and inventing one would let
    /// a scroll overshoot address a row that does not exist.
    #[must_use]
    pub fn row_at(&self, position: f32) -> Option<usize> {
        if self.total_rows == 0 || !position.is_finite() || position < 0.0 {
            return None;
        }
        if position >= self.content_len() {
            return None;
        }
        Some((f64::from(position) / f64::from(self.row_height)).floor() as usize)
    }

    /// The smallest offset change that brings row `index` fully into the
    /// viewport, or `None` when it is already visible or does not exist.
    ///
    /// "Smallest" is the whole point: a tree that recentres the focused
    /// row on every arrow key makes the list jump and the user loses
    /// their place. This moves only when the row is actually outside, and
    /// then only by the distance needed to clear the edge.
    ///
    /// A row taller than the viewport pins its top to the viewport's
    /// top, which is the only reading that shows any of it.
    #[must_use]
    pub fn offset_to_reveal(&self, index: usize, current: f32) -> Option<f32> {
        if index >= self.total_rows || self.viewport_len <= 0.0 {
            return None;
        }
        let offset = self.clamped(current);
        let top = self.row_top(index);
        let bottom = self.row_bottom(index);

        let target = if top < offset {
            top
        } else if bottom > offset + self.viewport_len {
            bottom - self.viewport_len
        } else {
            return None;
        };

        let target = target.clamp(0.0, self.max_offset());
        (target != offset).then_some(target)
    }

    /// The thumb length for this list's *total* content.
    ///
    /// Delegated to [`scroll_view::thumb_length`] with
    /// [`content_len`](Self::content_len), which is the guarantee that
    /// the thumb shrinks as the document grows rather than tracking the
    /// handful of rows that happen to be realised.
    #[must_use]
    pub fn thumb_length(&self) -> f32 {
        scroll_view::thumb_length(self.content_len(), self.viewport_len)
    }

    /// The thumb's leading position along the track at `offset`.
    #[must_use]
    pub fn thumb_offset(&self, offset: f32) -> f32 {
        scroll_view::thumb_offset(
            self.clamped(offset),
            self.content_len(),
            self.viewport_len,
            self.thumb_length(),
        )
    }

    /// The thumb's rectangle inside a track laid along `axis`.
    #[must_use]
    pub fn thumb_rect(&self, track: Rectangle, offset: f32, axis: scroll_view::Axis) -> Rectangle {
        scroll_view::thumb_rect(
            track,
            self.clamped(offset),
            self.content_len(),
            self.viewport_len,
            axis,
        )
    }

    /// Clamp an offset into this list's scrollable range.
    fn clamped(&self, offset: f32) -> f32 {
        if !offset.is_finite() {
            return 0.0;
        }
        offset.clamp(0.0, self.max_offset())
    }
}

#[cfg(test)]
#[path = "virtual_list_tests.rs"]
mod virtual_list_tests;
