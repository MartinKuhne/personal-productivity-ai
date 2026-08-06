//! Fair Table Width Algorithm (FTWA) - pure column-width solver.
//!
//! Pure layout math with no egui dependency and no Markdown types; consumes
//! `&[f32]` measurements. Lives in `markdown::` per `src/desktop/AGENTS.md §5`
//! (pure layout math belongs in `markdown/`, not `ui/`).
//!
//! The egui-bridging adapter - `measure`, `measure_cached`, `ftwa_cached`,
//! `measure_cell`, and the per-UI caches - lives in `crate::ui::table_width`
//! and re-exports the pure API via `pub use`.
//!
//!
//! Public-API tests live in `tests/table_width_algorithm_test.rs`.
//! Internal-helper tests live in the sibling `internal_tests.rs` sidecar.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A breakpoint in a column's wrap-cost curve.
///
/// At column width `w`, the column produces `extra_lines` additional wrapped
/// lines beyond its single-line max-content layout. Breakpoints are sorted
/// by ascending width; the cost curve is a step function where `extra_lines`
/// decreases as width increases.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakpoint {
    /// Column width in pixels at which this breakpoint applies.
    pub width: f32,
    /// Number of extra wrapped lines at this width (0 = no wrapping).
    pub extra_lines: i32,
}

/// Table-width algorithm selector. Covers both the original FTWA
/// "deficit distribution" strategies (which build a minimum-cardinality
/// wrap set and shrink it) and three additional algorithms from the
/// survey in `doc/planning/table-column-width-algorithm.md` that operate
/// on all columns directly (no wrap set).
///
/// All strategies preserve the never-break-token invariant
/// (`w_j >= min_content[j]`) and the §3.6 fallback when the container is
/// narrower than the sum of min-contents. They differ in how the deficit
/// is allocated and (for the survey algorithms) in the shape of the
/// objective that picks the widths.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum DeficitStrategy {
    /// FTWA v1: shrink each wrap-set column proportionally to its slack.
    /// O(N). Simple but may produce suboptimal G1 (total wrapped lines).
    ProportionalToSlack,
    /// FTWA v2: greedy marginal-cost water-fill using per-column
    /// breakpoints. O(K log |S|) where K = breakpoints consumed. Minimizes
    /// G1 more aggressively by allocating deficit to columns with the
    /// lowest marginal cost (fewest extra lines per pixel of shrinkage).
    BreakpointWaterFill,
    /// Doc §2.10: water-filling by `max_j / w_j` ratio. Equalize the
    /// "wrap pressure" across all columns: at the optimum every column
    /// has the same `max_j / w_j` ratio, modulo clamping at `min_j`.
    /// Closed-form given the active-column set; O(N²) worst case when
    /// the active set shrinks iteratively.
    WaterFillRatio,
    /// Doc §2.13: per-column wrap-penalty minimization with a global
    /// Lagrange multiplier bisection. Minimizes `Σ extraLines_j(w_j)`
    /// subject to `Σ w_j = available`. Per-column cost is
    /// `e_j(w) + λ * w`; bisect on `λ` (sum is monotone non-increasing
    /// in λ) to hit the available-width constraint.
    LagrangePenalty,
    /// Doc §2.14: hybrid (min-content floor + per-column penalty +
    /// slack water-fill). Per-column target is the "first-wrap"
    /// boundary (the largest breakpoint width, where the column first
    /// starts to wrap, or `max_j` for single-line columns). The
    /// residual `(available − Σ target)` is distributed by water-fill
    /// proportional to headroom `max_j − target_j`.
    ///
    /// **Default strategy** — set by `default_table_width_strategy` in
    /// `config.rs` and the `from_config` fallback below. The hybrid
    /// gives the best G1/G2 trade-off on degenerate cell
    /// distributions (a few very wide cells with many breakpoints
    /// alongside many short cells with no slack); the pure
    /// breakpoint water-fill tends to over-allocate the
    /// already-wide columns when the wrap set's combined slack is
    /// only slightly more than the deficit, while the pure
    /// ratio-equalising strategies don't respect the wrap-set
    /// cardinality minimum.
    HybridMinPenaltyWaterFill,
}

impl DeficitStrategy {
    /// Parse from the config string. Unknown values fall back to
    /// [`Self::HybridMinPenaltyWaterFill`], the current default
    /// (`default_table_width_strategy` in `config.rs`).
    pub fn from_config(s: &str) -> Self {
        match s {
            "proportional" | "prop" => Self::ProportionalToSlack,
            "waterfill" | "water-fill" | "water_fill" | "wf" => Self::BreakpointWaterFill,
            "ratio" | "waterfill-ratio" | "waterfill_ratio" => Self::WaterFillRatio,
            "lagrange" | "lagrange-penalty" | "lagrange_penalty" | "lp" => Self::LagrangePenalty,
            "hybrid" | "hybrid-min-penalty" | "hybrid_min_penalty" | "hmp" => {
                Self::HybridMinPenaltyWaterFill
            }
            // Default. Matches `default_table_width_strategy` in
            // `config.rs` so an empty/unknown config string produces
            // the same strategy as a fresh install.
            _ => Self::HybridMinPenaltyWaterFill,
        }
    }

    /// Serialize to the config string form. Inverse of
    /// [`Self::from_config`]. Used by the top-panel strategy combobox
    /// when persisting a user choice back to `AppConfig::table_width_strategy`.
    pub fn to_config(self) -> &'static str {
        match self {
            Self::ProportionalToSlack => "proportional",
            Self::BreakpointWaterFill => "waterfill",
            Self::WaterFillRatio => "ratio",
            Self::LagrangePenalty => "lagrange",
            Self::HybridMinPenaltyWaterFill => "hybrid",
        }
    }
}

/// Marginal cost entry for the water-fill min-heap.
/// Lower `cost` = cheaper to shrink this column to its next breakpoint.
#[derive(Clone, Copy, PartialEq)]
struct ShrinkStep {
    /// Marginal cost: incremental extra lines / pixels of shrinkage gained.
    /// NaN-safe: callers guarantee finite values.
    cost: f32,
    /// Column index in the wrap set.
    col: usize,
    /// Width to shrink to (the breakpoint width).
    target_width: f32,
    /// Incremental extra lines paid when crossing this breakpoint.
    delta_lines: i32,
}

impl Eq for ShrinkStep {}

impl Ord for ShrinkStep {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost
            .partial_cmp(&other.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(self.col.cmp(&other.col))
    }
}

impl PartialOrd for ShrinkStep {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Outcome of an FTWA computation: per-column pixel widths plus a flag telling
/// the caller that the available width is below the sum of min-content widths
/// (or that the deficit cannot be absorbed by the wrap set without exceeding
/// `available` after the B2 distribution), in which case the table physically
/// cannot fit and horizontal scrolling must be enabled (doc §3.6 fallback).
///
/// `widths.len()` matches the input column count. In the fallback case the
/// widths equal the min-content widths so any wrapping layout still respects
/// the never-break-a-token invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnWidths {
    /// Per-column assigned pixel width, in input order.
    pub widths: Vec<f32>,
    /// `true` when the assigned widths do not fit within `available` — the
    /// caller must enable horizontal scroll. Set when `available < Σ
    /// min_content` (the standard §3.6 fallback) or when the B2 distribution
    /// fully clamps the wrap set to `min_content` and the post-drift `Σ
    /// widths` still exceeds `available` (the wrap-set-saturation case).
    pub needs_horizontal_scroll: bool,
}

/// Pure FTWA core. Solves the deficit regime for G2 (minimum-cardinality
/// wrap set) plus G1 (minimize total wrapped lines within that set).
///
/// `max_content[j]` is column `j`'s single-line width; `min_content[j]` is its
/// longest unbreakable-token width (`min_content[j] <= max_content[j]`).
/// `breakpoints[j]` is column `j`'s wrap-cost curve (step function: at width
/// `w`, the column produces `extra_lines` wrapped lines). `available` is the
/// content width minus gutters. The algorithm proceeds:
///
/// * **Surplus** (`available >= sum of max_content`): pin every column to its
///   `max_content`. No column wraps and no column is stretched beyond its
///   content (G1 = 0, G2 = 0). The table may not fill the full available
///   width when content is narrow; this matches browser/spreadsheet auto-fit
///   behavior and avoids the "infinite-width column" visual defect that
///   proportional spare distribution produced. G3 ("use all space") is
///   intentionally relaxed in the surplus regime (see doc §3.5).
/// * **Deficit** (`sum of min_content <= available < sum of max_content`):
///   build a **minimum-cardinality wrap set** by sorting positive-slack
///   columns by slack descending and greedily adding them until their
///   combined slack covers the deficit. The deficit
///   (`D = sum of max_content - available`) is then distributed across
///   the wrap set per the chosen `DeficitStrategy` (proportional-to-slack
///   or breakpoint water-fill), never below `min_content`. Columns
///   outside the wrap set keep their `max_content` and do not wrap
///   (G2 § "only wrap what must wrap"). Float drift is absorbed into
///   the deepest-slack wrap column so the sum of `widths` equals
///   `available` exactly.
/// * **Fallback** (`available < sum of min_content`): return `min_content`
///   and set `needs_horizontal_scroll = true` (doc §3.6). The strongest
///   invariant (tokens never break) holds by construction.
///
/// Returns `widths.len() == max_content.len()`. Empty input returns empty
/// output, no scroll needed.
///
/// # Panics
///
/// Panics if `available` is not finite (NaN or +/-infinity), if any element
/// of `max_content` or `min_content` is not finite, or if any
/// `max_content[j] + MAX_MIN_DRIFT_TOLERANCE < min_content[j]` — i.e. the
/// `max >= min` invariant is violated by more than the sub-pixel drift
/// tolerance (see the assertion below for the rationale and the constant
/// for the value). Callers that receive measurements from external sources
/// should validate finiteness first.
///
/// # Examples
///
/// (This function is not part of the public API; the `ui::table_width`
/// module is intentionally private. See the unit tests in this file
/// for usage examples covering all three regimes.)
pub fn ftwa(
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    available: f32,
    strategy: DeficitStrategy,
) -> ColumnWidths {
    // Length check is the very first thing we do (BUG-3 fix). A length
    // mismatch with an empty `max_content` would otherwise silently
    // early-return (skipping the assertion entirely) and produce an empty
    // result for inputs that don't agree, which is a hard-to-debug error
    // mode. Putting the assertion first makes the contract uniform: any
    // mismatched-length input panics, regardless of which side is empty.
    let n = max_content.len();
    assert_eq!(
        n,
        min_content.len(),
        "ftwa: max_content and min_content must have equal length"
    );
    assert_eq!(
        n,
        breakpoints.len(),
        "ftwa: max_content and breakpoints must have equal length"
    );

    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }

    // NaN / infinity in input is a programmer error. Without this guard,
    // NaN propagates silently through every arithmetic comparison (they
    // all return false against NaN), causing the function to fall through
    // to the deficit branch with NaN sum_max / deficit, eventually
    // returning NaN-containing widths that crash egui's layout downstream.
    assert!(
        available.is_finite(),
        "ftwa: available must be finite, got {available}"
    );
    assert!(
        max_content.iter().all(|x| x.is_finite()),
        "ftwa: max_content must be finite, got {max_content:?}"
    );
    assert!(
        min_content.iter().all(|x| x.is_finite()),
        "ftwa: min_content must be finite, got {min_content:?}"
    );

    // Sign checks (OBS-3, OBS-4). The `is_finite` guard above must run
    // first; a NaN would otherwise slip past `>= 0.0` (because every
    // comparison with NaN returns false) and corrupt the surplus/deficit
    // classification. Pixel widths from `measure` are always non-negative;
    // a negative input signals a corrupt upstream measurement and must
    // fail loudly rather than produce a silent wrap or fallback.
    assert!(
        available >= 0.0,
        "ftwa: available must be non-negative, got {available}"
    );
    assert!(
        max_content.iter().all(|&x| x >= 0.0),
        "ftwa: max_content must be non-negative, got {max_content:?}"
    );
    assert!(
        min_content.iter().all(|&x| x >= 0.0),
        "ftwa: min_content must be non-negative, got {min_content:?}"
    );

    // max_content[j] >= min_content[j] is an invariant (max-content is
    // always at least the width of the longest unbreakable token).
    // Without this check, a corrupted measurement (e.g. min longer than
    // max) silently triggers the §3.6 fallback ("can't fit") instead of
    // surfacing the data error to the caller.
    //
    // Sub-pixel drift tolerance. egui's `layout_no_wrap` can return
    // `layout_no_wrap("A" + "B")` slightly different from
    // `layout_no_wrap("A") + layout_no_wrap("B")` (kerning and
    // sub-pixel rounding in the font shaper). The upstream
    // `measure_cell` builds `min_content` from the merged string when
    // no whitespace separates consecutive `InlineElem`s, while
    // `max_content` is the sum of per-fragment widths — the two can
    // differ by ~0.0625 px (= 1/16 px) on real fonts even though the
    // logical invariant holds. Real measurement errors (e.g. swap of
    // max and min) produce gaps orders of magnitude larger (>1 px);
    // 1 logical pixel is the smallest visually distinguishable unit at
    // 1x DPI, so anything above the tolerance is treated as a
    // programmer error and panics. A sub-tolerance violation is
    // absorbed by snapping `min` down to `max` so the deficit branch's
    // `slack = max_content[j] - min_content[j]` and
    // `widths[j].max(min_content[j])` floor stay well-defined.
    const MAX_MIN_DRIFT_TOLERANCE: f32 = 1.0;
    let mut min_content: Vec<f32> = min_content.to_vec();
    for j in 0..n {
        let mx = max_content[j];
        let mn = min_content[j];
        if mn > mx {
            let drift = mn - mx;
            assert!(
                drift <= MAX_MIN_DRIFT_TOLERANCE,
                "ftwa: max_content[{j}] = {mx} < min_content[{j}] = {mn} \
                 (invariant violation, drift {drift} > tolerance {MAX_MIN_DRIFT_TOLERANCE})"
            );
            // Snap `min` down to `max`: absorbs the sub-pixel drift so
            // the deficit branch sees a non-negative slack and the
            // never-break-token floor equals the rendered cell width.
            min_content[j] = mx;
        }
    }

    let sum_max: f32 = max_content.iter().copied().sum();
    let sum_min: f32 = min_content.iter().copied().sum();

    // §3.6 fallback: even at min-content the table cannot fit.
    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    // §3.2 surplus regime: every column fits at its max-content width.
    // Columns are pinned at `max_content` (not stretched). Stretching
    // columns beyond their content to fill the available width produced
    // the "infinite-width column" visual defect (e.g. a narrow Cost
    // column rendered hundreds of pixels wide with empty trailing
    // whitespace). Browser/spreadsheet auto-fit tables behave the same
    // way: a table whose content is narrower than the viewport simply
    // does not use the full width. G3 ("use all space") is intentionally
    // relaxed in the surplus regime in favor of not distorting column
    // widths (see doc §3.5).
    if available >= sum_max {
        return ColumnWidths {
            widths: max_content.to_vec(),
            needs_horizontal_scroll: false,
        };
    }

    // §2.10 / §2.13 / §2.14 dispatch. The three survey algorithms do not
    // build a minimum-cardinality wrap set — they operate on all columns
    // directly and are responsible for their own surplus / fallback /
    // overflow handling. Hand off before the wrap-set construction below
    // so the FTWA-specific G2 logic doesn't run for them.
    match strategy {
        DeficitStrategy::WaterFillRatio => {
            return solve_waterfill_ratio(
                max_content,
                &min_content,
                breakpoints,
                available,
                sum_max,
                sum_min,
            );
        }
        DeficitStrategy::LagrangePenalty => {
            return solve_lagrange_penalty(
                max_content,
                &min_content,
                breakpoints,
                available,
                sum_max,
                sum_min,
            );
        }
        DeficitStrategy::HybridMinPenaltyWaterFill => {
            return solve_hybrid(
                max_content,
                &min_content,
                breakpoints,
                available,
                sum_max,
                sum_min,
            );
        }
        DeficitStrategy::ProportionalToSlack | DeficitStrategy::BreakpointWaterFill => {}
    }

    // §3.3 deficit regime.
    let deficit = sum_max - available;

    // §3.3 deficit regime. **G2 is back**: the wrap set is the
    // *minimum-cardinality* subset of positive-slack columns that can
    // collectively absorb the deficit. A column only enters the wrap
    // set if its slack is actually needed — i.e. adding it is what
    // makes the running slack-sum reach the deficit. This restores
    // the "only wrap what must wrap" property the surplus branch has:
    // a short cell with positive slack (e.g. `XPS 15 9570`) stays
    // single-line at its max-content width when a long cell
    // (e.g. a multi-sentence Summary) can absorb the deficit alone.
    //
    // The wrap set is built greedily: sort positive-slack columns by
    // slack descending (ties broken by ascending index, OBS-1), then
    // push columns one at a time until the running sum meets the
    // deficit. `available >= sum_min` guarantees the loop terminates
    // (the worst case consumes every positive-slack column, which
    // always sums to `sum_max - sum_min >= deficit`). Zero-slack
    // columns are excluded by construction (they have nothing to
    // give); the never-break-token invariant is preserved because B2
    // clamps every shrunk width to `min_content`.
    let mut positives: Vec<usize> = (0..n)
        .filter(|&j| max_content[j] - min_content[j] > 0.0)
        .collect();
    positives.sort_by(|&a, &b| {
        let slack_a = max_content[a] - min_content[a];
        let slack_b = max_content[b] - min_content[b];
        slack_b
            .partial_cmp(&slack_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let mut wrap_set: Vec<usize> = Vec::with_capacity(positives.len());
    let mut acc_slack = 0.0_f32;
    for j in positives {
        wrap_set.push(j);
        acc_slack += max_content[j] - min_content[j];
        if acc_slack >= deficit {
            break;
        }
    }
    // `acc_slack` is the total slack accumulated by the greedy loop.
    // Normally `acc_slack >= deficit` (the loop terminates as soon as
    // the running sum covers the deficit, building the
    // *minimum-cardinality* wrap set). It can be `< deficit` only when
    // the loop exhausts `positives` before the deficit is covered — in
    // that case the wrap set is the full set of positive-slack columns
    // and B2 will end up clamping every wrap-set column to
    // `min_content`. The drift fix below cannot absorb a *negative*
    // drift (the `(widths[target] + drift).max(min_content[target])`
    // clamp prevents widths from going below min-content), so the
    // post-drift `Σ widths` may still exceed `available`. The trailing
    // overflow guard handles that case by escalating to the §3.6
    // horizontal-scroll fallback rather than letting the renderer
    // overflow the viewport.
    let total_slack: f32 = acc_slack;

    // B2: shrink wrap-set columns according to the chosen strategy,
    // never below min-content. The three survey algorithms
    // (WaterFillRatio, LagrangePenalty, HybridMinPenaltyWaterFill)
    // never reach this point — they were dispatched earlier in `ftwa`,
    // before the wrap-set construction. The wildcard arm here exists
    // only to keep the match exhaustive; it should be unreachable in
    // practice.
    let mut widths = max_content.to_vec();
    match strategy {
        DeficitStrategy::ProportionalToSlack => {
            b2_proportional_to_slack(
                &wrap_set,
                &mut widths,
                max_content,
                &min_content,
                deficit,
                total_slack,
            );
        }
        DeficitStrategy::BreakpointWaterFill => {
            b2_breakpoint_water_fill(
                &wrap_set,
                &mut widths,
                max_content,
                &min_content,
                breakpoints,
                deficit,
            );
        }
        DeficitStrategy::WaterFillRatio
        | DeficitStrategy::LagrangePenalty
        | DeficitStrategy::HybridMinPenaltyWaterFill => {
            unreachable!(
                "survey algorithms are dispatched before wrap-set construction in ftwa; \
                 strategy {:?} should not reach the B2 branch",
                strategy
            );
        }
    }

    // Fix float drift: ensure `Σ widths == available` exactly by dumping any
    // rounding residual into the deepest-slack wrap column (still above
    // min-content since the residual is sub-pixel). This satisfies G3 precisely.
    let drift = available - widths.iter().copied().sum::<f32>();
    if drift.abs() > 0.0 && !wrap_set.is_empty() {
        // OBS-1: on slack tie, prefer the lower column index. Matches
        // the wrap-set sort order above and the surplus branch's
        // drift target, so the deterministic pick is consistent
        // across the deficit and surplus drift dumps.
        let target = *wrap_set
            .iter()
            .max_by(|&&a, &&b| {
                let sa = max_content[a] - min_content[a];
                let sb = max_content[b] - min_content[b];
                sa.partial_cmp(&sb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(&a)) // lower index = "greater" → max_by picks it on tie
            })
            .expect("wrap_set is non-empty (checked above)");
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    // Overflow guard: when the wrap set is fully clamped to `min_content`
    // (every B2 share exceeded its column's slack), the drift fix above
    // can only widen the target — it cannot shrink it below
    // `min_content`. The result is `Σ widths > available` and the
    // renderer would lay out cells wider than the viewport, clipping
    // the rightmost column silently. Escalate to the §3.6 horizontal-
    // scroll fallback in that case so the `ScrollArea` is applied and
    // the user can reach every column by scrolling.
    if widths.iter().copied().sum::<f32>() > available {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

/// B2 proportional-to-slack: shrink each wrap-set column proportionally to
/// its share of the captured slack `acc`. O(|S|).
fn b2_proportional_to_slack(
    wrap_set: &[usize],
    widths: &mut [f32],
    max_content: &[f32],
    min_content: &[f32],
    deficit: f32,
    acc: f32,
) {
    for &j in wrap_set {
        let slack_j = max_content[j] - min_content[j];
        let share = if acc > 0.0 {
            deficit * (slack_j / acc)
        } else {
            0.0
        };
        widths[j] = (max_content[j] - share).max(min_content[j]);
    }
}

/// B2 breakpoint water-fill: greedily shrink wrap-set columns by marginal
/// cost (incremental extra lines per pixel of shrinkage) using a min-heap.
/// O(K log |S|) where K = total breakpoints consumed.
///
/// Each column's breakpoints define a step function of extra_lines vs. width.
/// The algorithm starts all columns at max_content and shrinks them one
/// breakpoint at a time, always picking the cheapest next step (fewest
/// additional lines per pixel). This minimizes G1 (total wrapped lines)
/// while covering the deficit.
fn b2_breakpoint_water_fill(
    wrap_set: &[usize],
    widths: &mut [f32],
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    deficit: f32,
) {
    if wrap_set.is_empty() || deficit <= 0.0 {
        return;
    }

    // Track each wrap-set column's current state.
    let mut current_extra: Vec<i32> = vec![0; wrap_set.len()];
    // Index into the breakpoints list for each wrap-set column (descending).
    // We process breakpoints from highest width to lowest.
    let mut bp_idx: Vec<usize> = Vec::with_capacity(wrap_set.len());
    for &j in wrap_set {
        bp_idx.push(breakpoints[j].len().saturating_sub(1));
    }

    // Min-heap by marginal cost (use Reverse for min ordering).
    let mut heap: BinaryHeap<Reverse<ShrinkStep>> = BinaryHeap::new();

    // Seed the heap with the first (highest-width) breakpoint for each column.
    for (si, &j) in wrap_set.iter().enumerate() {
        if let Some(bp) = next_breakpoint_below(&breakpoints[j], &mut bp_idx[si], max_content[j]) {
            let delta_w = max_content[j] - bp.width;
            let delta_l = bp.extra_lines - current_extra[si];
            if delta_w > 0.0 {
                let cost = if delta_l > 0 {
                    delta_l as f32 / delta_w
                } else {
                    0.0
                };
                heap.push(Reverse(ShrinkStep {
                    cost,
                    col: si,
                    target_width: bp.width,
                    delta_lines: delta_l,
                }));
            }
        }
    }

    let mut remaining = deficit;

    while remaining > 0.0 {
        let Some(Reverse(step)) = heap.pop() else {
            break;
        };
        let si = step.col;
        let j = wrap_set[si];
        let current_width = widths[j];

        // Skip stale entries (column already shrunk past this breakpoint).
        if current_width <= step.target_width + 1e-6 {
            continue;
        }

        let delta_w = current_width - step.target_width;
        let can_shrink = delta_w.min(remaining);

        widths[j] = current_width - can_shrink;
        remaining -= can_shrink;

        // If we fully crossed this breakpoint, update extra_lines and push next.
        if can_shrink >= delta_w - 1e-6 {
            current_extra[si] += step.delta_lines;
            // Push the next breakpoint for this column.
            if let Some(bp) =
                next_breakpoint_below(&breakpoints[j], &mut bp_idx[si], step.target_width)
            {
                let next_delta_w = step.target_width - bp.width;
                let next_delta_l = bp.extra_lines - current_extra[si];
                if next_delta_w > 0.0 {
                    let cost = if next_delta_l > 0 {
                        next_delta_l as f32 / next_delta_w
                    } else {
                        0.0
                    };
                    heap.push(Reverse(ShrinkStep {
                        cost,
                        col: si,
                        target_width: bp.width,
                        delta_lines: next_delta_l,
                    }));
                }
            }
        }
    }

    // Clamp to min_content (should already be respected, but safety net).
    for &j in wrap_set {
        if widths[j] < min_content[j] {
            widths[j] = min_content[j];
        }
    }
}

/// Find the next breakpoint strictly below `below_width` by scanning
/// `bp_idx` downward. Returns the breakpoint and advances `bp_idx`.
/// Uses `bps.len()` as a sentinel for "exhausted" (no more breakpoints).
fn next_breakpoint_below(
    bps: &[Breakpoint],
    bp_idx: &mut usize,
    below_width: f32,
) -> Option<Breakpoint> {
    loop {
        if *bp_idx >= bps.len() {
            return None;
        }
        let bp = bps[*bp_idx].clone();
        *bp_idx = (*bp_idx).checked_sub(1).unwrap_or(bps.len());
        if bp.width < below_width - 1e-6 {
            return Some(bp);
        }
    }
}

/// Token data for a single cell, used for breakpoint computation.
#[derive(Clone)]
pub struct CellTokens {
    /// Ordered token widths (each measured with its own font).
    pub token_widths: Vec<f32>,
}

/// Compute the column-level breakpoints by merging cell-level breakpoints.
///
/// Each cell's breakpoints represent its wrap-cost curve. The column's
/// breakpoints are the sum of extra_lines across all cells at each width
/// (Decision 1: Σ across all cells). The result is sorted by width ascending.
pub fn compute_column_breakpoints(cell_tokens: &[CellTokens], space_width: f32) -> Vec<Breakpoint> {
    if cell_tokens.is_empty() {
        return Vec::new();
    }

    // Compute breakpoints for each cell.
    let mut all_cell_bps: Vec<Vec<Breakpoint>> = Vec::with_capacity(cell_tokens.len());
    for ct in cell_tokens {
        all_cell_bps.push(cell_breakpoints(&ct.token_widths, space_width));
    }

    // Merge: collect all distinct widths across all cells, then sum extra_lines.
    let mut width_set: Vec<f32> = Vec::new();
    for bps in &all_cell_bps {
        for bp in bps {
            width_set.push(bp.width);
        }
    }
    width_set.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    width_set.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    let mut merged = Vec::with_capacity(width_set.len());
    for w in width_set {
        let total_lines: i32 = all_cell_bps
            .iter()
            .map(|bps| extra_lines_at_width(bps, w))
            .sum();
        if total_lines > 0 {
            merged.push(Breakpoint {
                width: w,
                extra_lines: total_lines,
            });
        }
    }

    merged
}

/// Compute breakpoints for a single cell from its token widths.
///
/// Uses O(k³) approach: for each of O(k²) candidate line widths, simulate
/// greedy left-to-right packing to get line count. With k ≤ 15 tokens per
/// cell, this is fast enough.
fn cell_breakpoints(token_widths: &[f32], space_width: f32) -> Vec<Breakpoint> {
    if token_widths.is_empty() {
        return Vec::new();
    }

    let k = token_widths.len();

    // Generate all candidate widths: sum of tokens i..j + spaces between them.
    let mut candidates: Vec<f32> = Vec::with_capacity(k * k);
    for i in 0..k {
        let mut line_w = token_widths[i];
        candidates.push(line_w);
        for &tw in &token_widths[(i + 1)..] {
            line_w += space_width + tw;
            candidates.push(line_w);
        }
    }

    // Deduplicate and sort descending.
    candidates.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    // For each candidate width, simulate greedy pack to get line count.
    // Record the width at which extra_lines *becomes* > 0 (the transition point).
    let mut breakpoints = Vec::new();
    let mut prev_extra = 0;
    for (idx, &w) in candidates.iter().enumerate() {
        let lines = greedy_line_count(token_widths, space_width, w);
        let extra = lines - 1;
        if extra > prev_extra {
            // Transition detected: extra_lines increased from prev_extra to extra.
            // The breakpoint is at the previous width (where extra_lines was lower).
            let bp_width = if idx == 0 { w } else { candidates[idx - 1] };
            breakpoints.push(Breakpoint {
                width: bp_width,
                extra_lines: extra,
            });
            prev_extra = extra;
        }
    }

    // Sort by width ascending.
    breakpoints.sort_by(|a, b| {
        a.width
            .partial_cmp(&b.width)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    breakpoints
}

/// Simulate greedy left-to-right line packing and return the line count.
fn greedy_line_count(token_widths: &[f32], space_width: f32, col_width: f32) -> i32 {
    if token_widths.is_empty() {
        return 1;
    }
    let mut lines = 1;
    let mut line_w = 0.0;
    for (i, &tw) in token_widths.iter().enumerate() {
        let token_w = if i == 0 { tw } else { space_width + tw };
        if line_w + token_w > col_width + 1e-6 {
            // Start a new line.
            lines += 1;
            line_w = tw; // First token on new line has no leading space.
        } else {
            line_w += token_w;
        }
    }
    lines
}

/// Look up the extra_lines for a given width in a breakpoint list.
///
/// The breakpoint list is sorted by width ascending. For a given width `w`,
/// the extra_lines is the extra_lines of the breakpoint with the largest
/// width <= w. If w is larger than all breakpoints, extra_lines = 0.
fn extra_lines_at_width(bps: &[Breakpoint], w: f32) -> i32 {
    let idx = bps.partition_point(|bp| bp.width <= w + 1e-6);
    if idx == 0 {
        0
    } else {
        bps[idx - 1].extra_lines
    }
}

// ---------------------------------------------------------------------------
// Survey algorithms (doc §2.10, §2.13, §2.14)
//
// These are dispatched from [`ftwa`] before the FTWA wrap-set construction.
// They each handle their own surplus / fallback / overflow / drift so the
// FTWA-specific G2 minimum-cardinality wrap-set logic doesn't run for them.
// ---------------------------------------------------------------------------

/// Doc §2.10 — water-fill by `max_j / w_j` ratio.
///
/// Equalize the "wrap pressure" `max_j / w_j` across all columns. The
/// optimum is closed-form given the active-column set: `w_j = max_j / r`
/// where `r = Σ max_j / available`; columns for which this would push
/// `w_j` below `min_j` are clamped to `min_j` and removed from the
/// active set, after which `r` is recomputed. O(N²) worst case (one
/// pass per clamped column) but converges in 1 pass when no column
/// needs clamping.
///
/// `_breakpoints` is accepted for interface symmetry with the other
/// survey solvers but unused: the ratio objective is purely geometric
/// and doesn't need per-column wrap information.
///
/// # Panics
///
/// Panics on the same input contracts as [`ftwa`]: mismatched lengths,
/// non-finite values, negative values, or `max_content[j] < min_content[j]`.
/// All those checks are re-asserted in-line because this function is
/// called from the dispatch in `ftwa` *after* the shared checks, but it
/// is also a public entry point for tests and external callers.
#[allow(clippy::too_many_arguments)]
pub fn solve_waterfill_ratio(
    max_content: &[f32],
    min_content: &[f32],
    _breakpoints: &[Vec<Breakpoint>],
    available: f32,
    sum_max: f32,
    sum_min: f32,
) -> ColumnWidths {
    let n = max_content.len();
    assert_eq!(
        n,
        min_content.len(),
        "solve_waterfill_ratio: length mismatch"
    );
    assert!(
        available.is_finite() && available >= 0.0,
        "solve_waterfill_ratio: available must be finite and non-negative, got {available}"
    );

    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }

    // Surplus: every column fits at max-content. Pin to max (no stretch —
    // see the rationale in `ftwa`'s surplus branch).
    if available >= sum_max {
        return ColumnWidths {
            widths: max_content.to_vec(),
            needs_horizontal_scroll: false,
        };
    }

    // §3.6 fallback: even at min-content the table cannot fit.
    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    // Deficit regime. Iterative clamping: solve the equal-ratio linear
    // system, then clamp any column whose target would be below min_j.
    let mut widths = vec![0.0_f32; n];
    let mut active = vec![true; n];
    let mut active_max_sum = sum_max;
    let mut remaining = available;

    loop {
        let active_count = active.iter().filter(|&&a| a).count();
        if active_count == 0 {
            break;
        }
        // `active_max_sum / remaining` is the target ratio `r`. Every
        // active column's target width is `max_j / r`; if that is below
        // `min_j` the column is clamped to `min_j` and removed from
        // the active set (its slack is fully consumed).
        let ratio = active_max_sum / remaining;
        let mut new_clamped: Vec<usize> = Vec::new();
        for j in 0..n {
            if !active[j] {
                continue;
            }
            let target = max_content[j] / ratio;
            if target < min_content[j] {
                widths[j] = min_content[j];
                active[j] = false;
                new_clamped.push(j);
            } else {
                widths[j] = target;
            }
        }
        if new_clamped.is_empty() {
            break;
        }
        for &j in &new_clamped {
            remaining -= min_content[j];
            active_max_sum -= max_content[j];
        }
    }

    // Float drift: the closed-form equal-ratio solution can leave a
    // sub-pixel residual because of float arithmetic. Dump it into the
    // column with the largest `max_j / min_j` ratio (most "room");
    // tie-break by lower column index for determinism.
    let sum: f32 = widths.iter().sum();
    let drift = available - sum;
    if drift.abs() > 0.0 {
        let target = (0..n)
            .filter(|&j| active[j])
            .max_by(|&a, &b| {
                let ra = if min_content[a] > 0.0 {
                    max_content[a] / min_content[a]
                } else {
                    f32::INFINITY
                };
                let rb = if min_content[b] > 0.0 {
                    max_content[b] / min_content[b]
                } else {
                    f32::INFINITY
                };
                ra.partial_cmp(&rb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(&a))
            })
            .unwrap_or(0);
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    // Overflow guard: if the final sum still exceeds `available` (can
    // only happen if a clamping round removed a column from active
    // incorrectly, or if the drift target was clamped at min and
    // `drift > 0`), escalate to the §3.6 fallback.
    if widths.iter().sum::<f32>() > available + 1e-3 {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

/// Doc §2.13 — per-column wrap-penalty minimization with global
/// Lagrange-multiplier bisection.
///
/// Minimize `Σ extraLines_j(w_j)` subject to `Σ w_j = available`. The
/// Lagrangian is `Σ (e_j + λ · w_j)`; for each λ every column picks its
/// optimal width independently (a small finite-set search over the
/// breakpoint boundaries). `Σ w_j(λ)` is monotone non-increasing in λ
/// because widening is more expensive at higher λ, so binary search on
/// λ converges to the right value in O(log(λ_max / ε)) iterations
/// (≤ 100 in practice). Tie-breaking on equal costs prefers the wider
/// width so the final layout is deterministic.
///
/// Uses the **wide-side** natural convention for breakpoints: at a
/// candidate width `w` the wrap cost is the value of the narrowest
/// breakpoint with `bp.width > w` (or 0 if no such breakpoint exists).
/// This treats the breakpoint as the "upper boundary" of a wrap-cost
/// interval: `[bps[k].width, bps[k+1].width)` has value
/// `bps[k+1].extra_lines`, and `w >= bps[K-1].width` has value 0. (The
/// existing `extra_lines_at_width` uses a different — narrow-side —
/// convention; the Lagrangian needs the wide-side value so the
/// minimization is monotone in w.)
///
/// # Panics
///
/// Same input contracts as [`ftwa`] (re-asserted in-line for direct callers).
#[allow(clippy::too_many_arguments)]
pub fn solve_lagrange_penalty(
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    available: f32,
    sum_max: f32,
    sum_min: f32,
) -> ColumnWidths {
    let n = max_content.len();
    assert_eq!(
        n,
        min_content.len(),
        "solve_lagrange_penalty: length mismatch"
    );
    assert_eq!(
        n,
        breakpoints.len(),
        "solve_lagrange_penalty: breakpoints length mismatch"
    );
    assert!(
        available.is_finite() && available >= 0.0,
        "solve_lagrange_penalty: available must be finite and non-negative, got {available}"
    );

    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }

    // Surplus / fallback short-circuits.
    if available >= sum_max {
        return ColumnWidths {
            widths: max_content.to_vec(),
            needs_horizontal_scroll: false,
        };
    }
    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    // Deficit regime. Bisect on λ in [0, λ_max].
    //   λ=0: every column picks its widest candidate (sum = sum_max).
    //   λ=λ_max: every column picks its narrowest candidate (sum = sum_min).
    //
    // Per-column candidate set: max_j, each breakpoint width, and
    // min_j. The cost at each candidate w is `wide_side_e(w) + λ·w`
    // where `wide_side_e` returns the wrap cost of the wide-side
    // interval (the value of the narrowest breakpoint with `bp.width > w`,
    // or 0 if w is above all breakpoints).
    //
    // 100 bisection iterations is overkill: log2(1e6 / 1e-3) ≈ 40 is
    // enough to get 1e-3 pixel accuracy on λ. 100 is cheap and gives
    // a generous safety margin against any near-degenerate cases.
    const LAMBDA_MAX: f32 = 1.0e6;
    let mut lo = 0.0_f32;
    let mut hi = LAMBDA_MAX;
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        let sum = lagrange_sum(mid, max_content, min_content, breakpoints);
        if sum > available {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    // Use `hi` (under-shoots or matches). The float drift is dumped
    // into the column with the most headroom above min.
    let mut widths = lagrange_widths(hi, max_content, min_content, breakpoints);
    let sum: f32 = widths.iter().sum();
    let drift = available - sum;
    if drift.abs() > 0.0 {
        let target = (0..n)
            .max_by(|&a, &b| {
                let sa = widths[a] - min_content[a];
                let sb = widths[b] - min_content[b];
                sa.partial_cmp(&sb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(&a))
            })
            .unwrap_or(0);
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    if widths.iter().sum::<f32>() > available + 1e-3 {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

/// Σ w_j(λ) across all columns. Used by the bisection in
/// [`solve_lagrange_penalty`].
fn lagrange_sum(lambda: f32, max: &[f32], min: &[f32], breakpoints: &[Vec<Breakpoint>]) -> f32 {
    let mut sum = 0.0;
    for j in 0..max.len() {
        sum += lagrange_optimal_width(lambda, j, max, min, breakpoints);
    }
    sum
}

/// Σ w_j(λ) → Vec<f32>. Used to build the final widths.
fn lagrange_widths(
    lambda: f32,
    max: &[f32],
    min: &[f32],
    breakpoints: &[Vec<Breakpoint>],
) -> Vec<f32> {
    (0..max.len())
        .map(|j| lagrange_optimal_width(lambda, j, max, min, breakpoints))
        .collect()
}

/// Per-column optimal width for the Lagrangian `e(w) + λ·w` at the
/// given λ. Searches the finite set of candidate widths
/// { max_j, each breakpoint width, min_j } for the minimum cost.
///
/// **Wide-side natural convention** for wrap cost: at candidate w the
/// cost is the value of the narrowest breakpoint with `bp.width > w`
/// (or 0 if no such breakpoint exists, i.e. w is above all
/// breakpoints). The candidate enumeration visits breakpoints in
/// descending width order and uses `bps[k+1].extra_lines` (or 0 for
/// the last bp) as the wide-side value at `bps[k].width`.
fn lagrange_optimal_width(
    lambda: f32,
    j: usize,
    max: &[f32],
    min: &[f32],
    breakpoints: &[Vec<Breakpoint>],
) -> f32 {
    let max_j = max[j];
    let min_j = min[j];
    let bps = &breakpoints[j];

    // max_j candidate: wide-side e = 0 (above all breakpoints).
    let mut best_w = max_j;
    let mut best_cost = lambda * max_j; // = 0 + λ·max_j

    // Breakpoints in descending width order. The wide-side e at
    // `bps[k].width` is `bps[k+1].extra_lines` (or 0 when k+1 doesn't
    // exist, i.e. the highest breakpoint's wide side is the no-wrap
    // region above it).
    let k_len = bps.len();
    for (idx, bp) in bps.iter().enumerate().rev() {
        let wide_e = if idx + 1 < k_len {
            bps[idx + 1].extra_lines
        } else {
            0
        };
        let cost = wide_e as f32 + lambda * bp.width;
        // Tie-break: prefer the wider width (less wrap) so the
        // bisection is deterministic when two candidates have the
        // same cost.
        if cost < best_cost || (cost == best_cost && bp.width > best_w) {
            best_cost = cost;
            best_w = bp.width;
        }
    }

    // min_j candidate: wide-side e = bps[0].extra_lines (narrowest
    // breakpoint's value, which is the wrap cost at the very narrow
    // end of [min_j, bps[0].width]). For columns with no breakpoints
    // there is no wrap to be had at min_j either, so e = 0.
    let e_at_min = bps.first().map(|bp| bp.extra_lines).unwrap_or(0);
    let min_cost = e_at_min as f32 + lambda * min_j;
    if min_cost < best_cost || (min_cost == best_cost && min_j > best_w) {
        best_w = min_j;
    }

    best_w
}

/// Doc §2.14 — hybrid (min-content floor + per-column target + residual
/// water-fill).
///
/// The composition that most production "auto-fit" implementations
/// converge to:
///
/// 1. Every column starts at `min_j` (the never-break-token floor).
/// 2. Per-column "target" is the **first-wrap boundary** — the largest
///    breakpoint width (`bps.last().width`) for columns that wrap, or
///    `max_j` for columns that don't. This is the widest width at which
///    the column first starts wrapping; using it as a target means the
///    column is on the edge of its 0-wrap region, leaving the most
///    headroom for the table-level water-fill step.
/// 3. The residual `R = available − Σ target_j` is distributed by
///    water-fill proportional to each column's headroom
///    `max_j − target_j`. Positive residual widens columns (up to
///    `max_j`); negative residual narrows them (down to `min_j`).
///
/// The wrap information in `breakpoints` is consulted only for the
/// per-column target step (step 2); the water-fill (step 3) is
/// geometric. As a result this solver is meaningfully distinct from
/// the FTWA strategies (which build a wrap set) and from
/// [`solve_lagrange_penalty`] (which uses breakpoints for the full
/// per-column minimization).
///
/// # Panics
///
/// Same input contracts as [`ftwa`] (re-asserted in-line for direct callers).
#[allow(clippy::too_many_arguments)]
pub fn solve_hybrid(
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    available: f32,
    sum_max: f32,
    sum_min: f32,
) -> ColumnWidths {
    let n = max_content.len();
    assert_eq!(n, min_content.len(), "solve_hybrid: length mismatch");
    assert_eq!(
        n,
        breakpoints.len(),
        "solve_hybrid: breakpoints length mismatch"
    );
    assert!(
        available.is_finite() && available >= 0.0,
        "solve_hybrid: available must be finite and non-negative, got {available}"
    );

    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }

    if available >= sum_max {
        return ColumnWidths {
            widths: max_content.to_vec(),
            needs_horizontal_scroll: false,
        };
    }
    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    // Step 2: per-column "first-wrap boundary" target.
    let mut targets = vec![0.0_f32; n];
    for j in 0..n {
        // For a column with breakpoints, the largest breakpoint width
        // is the threshold where the column first starts wrapping.
        // Picking it as the target leaves the column with 1 wrap (or
        // `bps.last().extra_lines` wraps, which is the smallest wrap
        // count in the column) and gives the residual water-fill the
        // maximum headroom to work with. For a column with no
        // breakpoints, the column never wraps → target = max_j.
        targets[j] = breakpoints[j]
            .last()
            .map(|bp| bp.width)
            .unwrap_or(max_content[j]);
        // Clamp into [min_j, max_j] just in case the breakpoints list
        // contains a value outside that range (defensive: should never
        // happen given the breakpoint construction in `cell_breakpoints`).
        targets[j] = targets[j].clamp(min_content[j], max_content[j]);
    }

    // Step 3: residual = available - Σ target.
    let sum_targets: f32 = targets.iter().sum();
    let residual = available - sum_targets;

    // Step 4: water-fill the residual. Sign determines direction.
    let mut widths = targets.clone();
    if residual.abs() > 0.0 {
        if residual > 0.0 {
            // Distribute surplus proportional to headroom (max_j - target_j).
            let headroom: Vec<f32> = (0..n)
                .map(|j| (max_content[j] - targets[j]).max(0.0))
                .collect();
            let total_headroom: f32 = headroom.iter().sum();
            if total_headroom > 0.0 {
                for j in 0..n {
                    let add = residual * headroom[j] / total_headroom;
                    widths[j] = (widths[j] + add).min(max_content[j]);
                }
            }
        } else {
            // Distribute deficit proportional to (target_j - min_j).
            let above_min: Vec<f32> = (0..n)
                .map(|j| (targets[j] - min_content[j]).max(0.0))
                .collect();
            let total_above: f32 = above_min.iter().sum();
            if total_above > 0.0 {
                let take = -residual; // positive
                for j in 0..n {
                    let sub = take * above_min[j] / total_above;
                    widths[j] = (widths[j] - sub).max(min_content[j]);
                }
            }
        }
    }

    // Float drift: dump residual into the column with the most
    // headroom above min. Sub-pixel, so a clamp to min is safe.
    let sum: f32 = widths.iter().sum();
    let drift = available - sum;
    if drift.abs() > 0.0 {
        let target = (0..n)
            .max_by(|&a, &b| {
                let sa = widths[a] - min_content[a];
                let sb = widths[b] - min_content[b];
                sa.partial_cmp(&sb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(&a))
            })
            .unwrap_or(0);
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    if widths.iter().sum::<f32>() > available + 1e-3 {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

// ---------------------------------------------------------------------------
// Internal-helper tests live in the sibling `internal_tests.rs` sidecar.
// Public-API tests live in `tests/table_width_algorithm_test.rs`.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "internal_tests.rs"]
mod tests;
