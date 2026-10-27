//! Column model and pure width resolution for the structured grid
//! (`UIF-COMP-007`).
//!
//! The spec fixes what a table *looks like* at a given width — a 32px sticky
//! header, 32px body rows, `10px` resize grabs — but says nothing about how
//! the columns are supposed to *share* the width they are given. That gap is
//! where every table implementation quietly grows a second, non-pure layout
//! engine, so this module answers the question once and in isolation:
//! [`resolve_widths`] is a function of `(&[ColumnSpec], available)` and
//! nothing else. No clock, no measured text, no OS, no mutable state.
//!
//! # The three policies
//!
//! [`WidthPolicy`] is deliberately three cases, because those are the three
//! things a column can *want*:
//!
//! * [`WidthPolicy::Fixed`] — an exact width. Rigid: floor, target and
//!   ceiling collapse onto the same number, so the column neither gives nor
//!   takes in any regime.
//! * [`WidthPolicy::Content`] — "as wide as my content, no wider". The
//!   host supplies the measured preferred width; the column may shrink to
//!   its minimum but **never grows past its content**, which is what stops
//!   a short column from being stretched into a gulf.
//! * [`WidthPolicy::Proportional`] — "share what is left". Sits at its
//!   minimum until there is surplus, then absorbs surplus by weight.
//!
//! # The three regimes
//!
//! Given floors `F = Σ floor`, targets `T = Σ target` and `available`:
//!
//! * `available >= T` — **surplus**. Everyone is paid their target, then
//!   the remainder is water-filled across the elastic columns by weight,
//!   capped at each ceiling.
//! * `F <= available < T` — **deficit**. Every column is scaled linearly
//!   between its floor and its target by `scale = 1 - deficit / headroom`,
//!   a closed form with no iteration at all.
//! * `available < F` — **over-subscribed**. Nothing can be paid. Every
//!   column is pinned at its floor and the layout reports
//!   [`ColumnLayout::needs_horizontal_scroll`].
//!
//! # Termination
//!
//! The deficit regime is closed-form, so it cannot loop. The surplus regime
//! iterates, and it terminates because each pass either empties the elastic
//! pool or caps at least one column and removes it from the pool — so at
//! most one pass per column, `n + 1` in total. The loop counter is explicit
//! rather than trusted, and the residual that survives is pushed into the
//! column with the most remaining headroom so sub-pixel float drift cannot
//! accumulate across columns the way [`crate::composite::tree_row`]'s
//! indent policy had to avoid it by construction.
//!
//! This mirrors the shape of `fastmd`'s Fair Table Width Algorithm
//! (`src/app/markdown/table_width/`), which has the same three regimes,
//! but stays **measurement-free**: the resolver never asks how wide a
//! string is. The host passes `preferred` and `min` in, which is also what
//! lets the same resolver drive a column of icons, of buttons, or of text.
//!
//! Unit tests live in the sibling `table_column_tests.rs` sidecar.

/// A column's stable identity.
///
/// A `u16` rather than a `usize` because the spec's `ColId` is a small,
/// dense, host-assigned key — the same shape as the tree's `Id` — and
/// keeping it narrow makes it cheap to carry inside a `(RowId, ColId)`
/// pair in a `BTreeSet`.
pub type ColId = u16;

/// Sub-pixel tolerance. Anything smaller than this cannot be drawn, so two
/// widths that differ by less are the same width.
pub(crate) const EPS: f32 = 1e-3;

/// A weight below this cannot carry surplus.
///
/// A zero-weight [`WidthPolicy::Proportional`] column is deliberately
/// inert: it holds its minimum and takes nothing. Dividing by a weight of
/// zero would produce `NaN` and propagate it into every column's width, so
/// the guard is what makes a degenerate weight safe rather than fatal.
pub(crate) const WEIGHT_EPS: f32 = 1e-6;

/// The width a column would like, must have, and may take.
///
/// Derived from a [`ColumnSpec`] rather than stored, so the three numbers
/// cannot disagree with the policy they came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnBounds {
    /// The hard minimum — the width below which the column cannot render.
    pub floor: f32,
    /// The width the column asks for when there is room.
    pub target: f32,
    /// The widest the column may ever be drawn.
    pub ceiling: f32,
    /// The share of surplus this column absorbs. Zero for a column that
    /// never grows.
    pub weight: f32,
}

impl ColumnBounds {
    /// Whether this column can absorb any surplus at all.
    #[must_use]
    pub fn is_elastic(&self) -> bool {
        self.weight > WEIGHT_EPS && self.ceiling > self.target + EPS
    }

    /// How much more width this column could take above `current`.
    #[must_use]
    pub fn headroom_above(&self, current: f32) -> f32 {
        (self.ceiling - current).max(0.0)
    }

    /// How much width this column could give below `current` without
    /// breaking its minimum.
    #[must_use]
    pub fn giveback_below(&self, current: f32) -> f32 {
        (current - self.floor).max(0.0)
    }
}

/// Horizontal alignment of a cell's content inside its column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellAlign {
    /// Flush to the reading edge — the default for prose-like cells.
    #[default]
    Start,
    /// Centred in the column.
    Center,
    /// Flush to the trailing edge — what numeric columns want, so the
    /// digits line up across rows.
    End,
}

impl CellAlign {
    /// The gallery slug for this alignment.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }

    /// Place `content_width` inside a box of `box_width` starting at
    /// `box_x`, clamped so content never starts before the box.
    ///
    /// Content wider than the box is *not* an error: it starts at the box
    /// and is clipped by the caller. Truncating toward the alignment would
    /// hide the beginning of a long string, which is the wrong end to
    /// lose.
    #[must_use]
    pub fn place(self, box_x: f32, box_width: f32, content_width: f32) -> f32 {
        let spare = box_width - content_width;
        if spare <= 0.0 {
            return box_x;
        }
        match self {
            Self::Start => box_x,
            Self::Center => box_x + spare / 2.0,
            Self::End => box_x + spare,
        }
    }
}

/// How a column's width is decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WidthPolicy {
    /// An exact width. Rigid in every regime.
    Fixed(f32),
    /// "As wide as my content, never wider." The payload is the host's
    /// measured preferred width.
    Content(f32),
    /// "Share what is left, by this weight."
    Proportional(f32),
}

impl WidthPolicy {
    /// The surplus-carrying weight this policy implies.
    #[must_use]
    pub fn weight(self) -> f32 {
        match self {
            Self::Fixed(_) | Self::Content(_) => 0.0,
            Self::Proportional(w) => sane(w),
        }
    }

    /// The gallery slug for this policy's class.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Fixed(_) => "fixed",
            Self::Content(_) => "content",
            Self::Proportional(_) => "proportional",
        }
    }
}

/// One declared column.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnSpec {
    /// The column's stable identity.
    pub id: ColId,
    /// The header title.
    pub title: &'static str,
    /// How the width is decided.
    pub policy: WidthPolicy,
    /// The hard minimum width.
    pub min_width: f32,
    /// The widest the column may be drawn, `None` for unbounded.
    pub max_width: Option<f32>,
    /// How the cell content aligns.
    pub align: CellAlign,
    /// Whether the header offers a sort affordance.
    pub sortable: bool,
    /// Whether the divider on this column's trailing edge may be dragged.
    pub resizable: bool,
}

impl ColumnSpec {
    /// A rigid column of exactly `width`.
    ///
    /// Not resizable: "fixed" and "the user may drag this edge" are a
    /// contradiction, and a host that wants both has asked for a
    /// [`WidthPolicy::Content`] column with a minimum.
    #[must_use]
    pub fn fixed(id: ColId, title: &'static str, width: f32) -> Self {
        Self {
            id,
            title,
            policy: WidthPolicy::Fixed(width),
            min_width: 0.0,
            max_width: None,
            align: CellAlign::Start,
            sortable: false,
            resizable: false,
        }
    }

    /// A column that wants its content's measured width and never more.
    #[must_use]
    pub fn content(id: ColId, title: &'static str, preferred: f32) -> Self {
        Self {
            id,
            title,
            policy: WidthPolicy::Content(preferred),
            min_width: 0.0,
            max_width: None,
            align: CellAlign::Start,
            sortable: false,
            resizable: true,
        }
    }

    /// A column that absorbs surplus by `weight`.
    #[must_use]
    pub fn proportional(id: ColId, title: &'static str, weight: f32) -> Self {
        Self {
            id,
            title,
            policy: WidthPolicy::Proportional(weight),
            min_width: 0.0,
            max_width: None,
            align: CellAlign::Start,
            sortable: false,
            resizable: true,
        }
    }

    /// Set the hard minimum width.
    #[must_use]
    pub fn with_min(mut self, width: f32) -> Self {
        self.min_width = sane(width);
        self
    }

    /// Set the widest this column may be drawn.
    #[must_use]
    pub fn with_max(mut self, width: f32) -> Self {
        self.max_width = Some(sane(width));
        self
    }

    /// Set the content alignment.
    #[must_use]
    pub fn with_align(mut self, align: CellAlign) -> Self {
        self.align = align;
        self
    }

    /// Allow or deny the sort affordance.
    #[must_use]
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Allow or deny dragging this column's trailing divider.
    #[must_use]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Resolve this spec into the three numbers the solver works with.
    ///
    /// Every input is sanitised on the way through: a `NaN` or infinite
    /// width becomes `0.0` rather than poisoning the whole layout, and a
    /// `max_width` below the minimum is dropped rather than inverted,
    /// because a ceiling under a floor has no reading and silently
    /// swapping them would let a column shrink past what its host asked
    /// for.
    #[must_use]
    pub fn bounds(&self) -> ColumnBounds {
        let floor = sane(self.min_width);
        let ceiling = match self.max_width {
            Some(max) if max >= floor => sane(max),
            _ => f32::INFINITY,
        };
        match self.policy {
            WidthPolicy::Fixed(width) => {
                let width = clamp(sane(width), floor, ceiling);
                ColumnBounds {
                    floor: width,
                    target: width,
                    ceiling: width,
                    weight: 0.0,
                }
            }
            WidthPolicy::Content(preferred) => {
                let target = clamp(sane(preferred), floor, ceiling);
                ColumnBounds {
                    floor,
                    target,
                    ceiling: target,
                    weight: 0.0,
                }
            }
            WidthPolicy::Proportional(weight) => ColumnBounds {
                floor,
                target: floor,
                ceiling,
                weight: sane(weight),
            },
        }
    }
}

/// Which regime the solver landed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthRegime {
    /// `available >= Σ targets` — every column got what it asked for and
    /// surplus was distributed.
    Surplus,
    /// `Σ floors <= available < Σ targets` — every column was scaled
    /// between its floor and its target.
    Deficit,
    /// `available < Σ floors` — the columns cannot all be paid.
    Oversubscribed,
}

impl WidthRegime {
    /// The gallery slug for this regime.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Surplus => "surplus",
            Self::Deficit => "deficit",
            Self::Oversubscribed => "oversubscribed",
        }
    }
}

/// The resolved width of every column in a table.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnLayout {
    ids: Vec<ColId>,
    widths: Vec<f32>,
    offsets: Vec<f32>,
    total_width: f32,
    sum_minimums: f32,
    available: f32,
    slack: f32,
    regime: WidthRegime,
}

impl Default for ColumnLayout {
    fn default() -> Self {
        Self::empty()
    }
}

impl ColumnLayout {
    /// A layout with no columns.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            ids: Vec::new(),
            widths: Vec::new(),
            offsets: Vec::new(),
            total_width: 0.0,
            sum_minimums: 0.0,
            available: 0.0,
            slack: 0.0,
            regime: WidthRegime::Surplus,
        }
    }

    /// The column ids, in declaration order.
    #[must_use]
    pub fn ids(&self) -> &[ColId] {
        &self.ids
    }

    /// Each column's resolved width, in declaration order.
    #[must_use]
    pub fn widths(&self) -> &[f32] {
        &self.widths
    }

    /// Each column's left edge, in declaration order.
    #[must_use]
    pub fn offsets(&self) -> &[f32] {
        &self.offsets
    }

    /// The width the table actually occupies.
    #[must_use]
    pub fn total_width(&self) -> f32 {
        self.total_width
    }

    /// `Σ floor` — the width below which the table cannot be drawn.
    #[must_use]
    pub fn sum_minimums(&self) -> f32 {
        self.sum_minimums
    }

    /// The width the resolver was asked to fit into.
    #[must_use]
    pub fn available(&self) -> f32 {
        self.available
    }

    /// Width that no column could take — the table is narrower than the
    /// viewport because nothing was elastic enough to absorb the surplus.
    #[must_use]
    pub fn slack(&self) -> f32 {
        self.slack
    }

    /// The regime the solver landed in.
    #[must_use]
    pub const fn regime(&self) -> WidthRegime {
        self.regime
    }

    /// Whether the columns' minimums exceed the viewport.
    #[must_use]
    pub fn oversubscribed(&self) -> bool {
        self.regime == WidthRegime::Oversubscribed
    }

    /// Whether the table must be scrolled sideways to be read whole.
    ///
    /// Distinct from [`Self::oversubscribed`] rather than an alias: today
    /// the two coincide, but a future non-scrolling fallback (ellipsis the
    /// columns instead) would break the tie, and collapsing them now would
    /// hide that decision from the caller.
    #[must_use]
    pub fn needs_horizontal_scroll(&self) -> bool {
        self.total_width > self.available + EPS
    }

    /// The number of columns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.widths.len()
    }

    /// Whether the layout has no columns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    /// The resolved width of the column at `index`, `0.0` when out of
    /// range.
    #[must_use]
    pub fn width_at(&self, index: usize) -> f32 {
        self.widths.get(index).copied().unwrap_or(0.0)
    }

    /// The left edge of the column at `index`, `0.0` when out of range.
    #[must_use]
    pub fn x_at(&self, index: usize) -> f32 {
        self.offsets.get(index).copied().unwrap_or(0.0)
    }

    /// The position of `id` in the layout.
    #[must_use]
    pub fn index_of(&self, id: ColId) -> Option<usize> {
        self.ids.iter().position(|&candidate| candidate == id)
    }

    /// The resolved width of the column `id`, `0.0` when absent.
    #[must_use]
    pub fn width_of(&self, id: ColId) -> f32 {
        self.index_of(id).map_or(0.0, |index| self.widths[index])
    }

    /// The trailing edge of the divider between column `index` and the
    /// next column.
    #[must_use]
    pub fn divider_x(&self, index: usize) -> f32 {
        self.x_at(index) + self.width_at(index)
    }
}

/// Resolve every column's width for a given viewport.
///
/// The whole of `UIF-COMP-007`'s layout story runs through this one
/// function, and it is total: an empty column list, a zero or negative
/// viewport, a `NaN` preferred width and an over-subscribed set of
/// minimums all return a usable layout rather than failing.
///
/// See the [module docs](self) for the regimes and the termination
/// argument.
#[must_use]
pub fn resolve_widths(specs: &[ColumnSpec], available: f32) -> ColumnLayout {
    let available = sane(available);
    let bounds: Vec<ColumnBounds> = specs.iter().map(ColumnSpec::bounds).collect();
    let sum_minimums: f32 = bounds.iter().map(|b| b.floor).sum();
    let sum_targets: f32 = bounds.iter().map(|b| b.target).sum();

    let mut widths = vec![0.0_f32; bounds.len()];
    let regime = if available < sum_minimums {
        for (slot, bound) in widths.iter_mut().zip(&bounds) {
            *slot = bound.floor;
        }
        WidthRegime::Oversubscribed
    } else if available < sum_targets {
        let deficit = sum_targets - available;
        let headroom = (sum_targets - sum_minimums).max(0.0);
        let scale = if headroom > EPS {
            (1.0 - deficit / headroom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for (slot, bound) in widths.iter_mut().zip(&bounds) {
            *slot = bound.floor + scale * (bound.target - bound.floor);
        }
        absorb(
            &mut widths,
            &bounds,
            available,
            |bound| bound.target,
            |bound| bound.target > bound.floor + EPS,
        );
        WidthRegime::Deficit
    } else {
        for (slot, bound) in widths.iter_mut().zip(&bounds) {
            *slot = bound.target;
        }
        distribute_surplus(&mut widths, &bounds, available);
        WidthRegime::Surplus
    };

    let total_width = sum(&widths);
    let ids: Vec<ColId> = specs.iter().map(|spec| spec.id).collect();
    let offsets = prefix_sums(&widths);
    let slack = (available - total_width).max(0.0);

    ColumnLayout {
        ids,
        widths,
        offsets,
        total_width,
        sum_minimums,
        available,
        slack,
        regime,
    }
}

/// Water-fill the surplus across the elastic columns by weight.
///
/// Bounded by construction: a pass that caps no column consumes all the
/// slack and ends the loop, and a pass that caps a column shrinks the pool.
/// The explicit guard makes the bound visible rather than argued.
fn distribute_surplus(widths: &mut [f32], bounds: &[ColumnBounds], available: f32) {
    let mut pool: Vec<usize> = (0..widths.len())
        .filter(|&index| bounds[index].is_elastic())
        .collect();
    let mut slack = available - sum(widths);
    let mut passes = 0usize;

    while slack > EPS && !pool.is_empty() && passes <= widths.len() {
        passes += 1;
        let total_weight: f32 = pool.iter().map(|&index| bounds[index].weight).sum();
        if total_weight <= WEIGHT_EPS {
            break;
        }
        let mut next = Vec::with_capacity(pool.len());
        let mut consumed = 0.0_f32;
        for &index in &pool {
            let share = slack * bounds[index].weight / total_weight;
            let headroom = bounds[index].headroom_above(widths[index]);
            let take = share.min(headroom);
            widths[index] += take;
            consumed += take;
            if headroom - take > EPS {
                next.push(index);
            }
        }
        slack -= consumed;
        pool = next;
    }

    absorb(
        widths,
        bounds,
        available,
        |bound| bound.ceiling,
        ColumnBounds::is_elastic,
    );
}

/// Push the difference between `Σ widths` and `want` into the column with
/// the most room, where "room" is measured up to `limit` and only columns
/// `eligible` to grow are considered.
///
/// Without this, `Σ widths` can sit a few ten-thousandths away from the
/// target and the drift shows up as the last column failing to meet the
/// table's edge. The absorbed amount is capped by the chosen column's own
/// room, so this can never push a column past the limit the regime set
/// for it — which is why `limit` and `eligible` are passed in rather than
/// assumed: the surplus regime grows columns toward their **ceiling**, the
/// deficit regime toward their **target**, and conflating the two would let
/// a deficit layout inflate a column above what it asked for.
fn absorb(
    widths: &mut [f32],
    bounds: &[ColumnBounds],
    want: f32,
    limit: impl Fn(&ColumnBounds) -> f32,
    eligible: impl Fn(&ColumnBounds) -> bool,
) {
    if widths.is_empty() {
        return;
    }
    let residual = want - sum(widths);
    if residual.abs() <= EPS {
        return;
    }

    let mut best: Option<usize> = None;
    let mut best_room = 0.0_f32;
    for (index, &width) in widths.iter().enumerate() {
        let room = if residual > 0.0 {
            if eligible(&bounds[index]) {
                (limit(&bounds[index]) - width).max(0.0)
            } else {
                0.0
            }
        } else {
            bounds[index].giveback_below(width)
        };
        if room > best_room {
            best_room = room;
            best = Some(index);
        }
    }

    if let Some(index) = best {
        let take = residual.abs().min(best_room);
        widths[index] += if residual > 0.0 { take } else { -take };
    }
}

/// Running left-edge for each column.
fn prefix_sums(widths: &[f32]) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(widths.len());
    let mut cursor = 0.0_f32;
    for &width in widths {
        offsets.push(cursor);
        cursor += width;
    }
    offsets
}

/// Sum a slice of widths without letting a `NaN` escape the total.
fn sum(values: &[f32]) -> f32 {
    values.iter().map(|&value| sane(value)).sum()
}

/// Clamp a width to a finite, non-negative number.
fn sane(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Clamp into `[lo, hi]` where `hi` may be infinite.
fn clamp(value: f32, lo: f32, hi: f32) -> f32 {
    value.max(lo).min(hi)
}

#[cfg(test)]
#[path = "table_column_tests.rs"]
mod tests;
