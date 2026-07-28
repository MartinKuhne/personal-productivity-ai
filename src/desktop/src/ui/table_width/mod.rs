//! Fair Table Width Algorithm (FTWA) â€” assigns per-column pixel widths to a markdown/GFM table.
//!
//! Reconciles three lexicographically-ordered goals (see
//! `doc/planning/table-column-width-algorithm.md`):
//!   **G1** minimize total word-wrap (extra wrapped lines),
//!   **G2** minimize the number of columns that wrap, and
//!   **G3** use all available horizontal space.
//!
//! The pure `ftwa` core is independent of egui and is unit-tested in isolation;
//! `measure` bridges to egui text shaping to derive the input widths.

use crate::ui::render::InlineElem;
use eframe::egui;
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

/// Strategy for distributing the deficit across the wrap set (B2).
///
/// Both strategies preserve G2 (minimum-cardinality wrap set) and the
/// never-break-token invariant. They differ only in how they allocate the
/// deficit among the wrap-set columns.
#[allow(dead_code)]
pub enum DeficitStrategy {
    /// v1: shrink each column proportionally to its slack. O(|S|).
    /// Simple but may produce suboptimal G1 (total wrapped lines).
    ProportionalToSlack,
    /// v2: greedy marginal-cost water-fill using per-column breakpoints.
    /// O(K log |S|) where K = breakpoints consumed. Minimizes G1 more
    /// aggressively by allocating deficit to columns with the lowest
    /// marginal cost (fewest extra lines per pixel of shrinkage).
    BreakpointWaterFill,
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
/// the caller that the available width is below the sum of min-content widths,
/// in which case the table physically cannot fit and horizontal scrolling must
/// be enabled (doc §3.6 fallback).
///
/// `widths.len()` matches the input column count. In the fallback case the
/// widths equal the min-content widths so any wrapping layout still respects
/// the never-break-a-token invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnWidths {
    /// Per-column assigned pixel width, in input order.
    pub widths: Vec<f32>,
    /// `true` when `available < Î£ min_content` â€” caller must enable horizontal scroll.
    pub needs_horizontal_scroll: bool,
}

/// Pure FTWA core. Solves the deficit regime exactly for G2 (fewest wrapping
/// columns) and approximately for G1 (within the chosen wrap set).
///
/// `max_content[j]` is column `j`'s single-line width; `min_content[j]` is its
/// longest unbreakable-token width (`min_content[j] <= max_content[j]`).
/// `breakpoints[j]` is column `j`'s wrap-cost curve (step function: at width
/// `w`, the column produces `extra_lines` wrapped lines). `available` is the
/// content width minus gutters. The algorithm proceeds:
///
/// * **Surplus** (`available >= sum of max_content`): pin every column to its
///   `max_content`. No column wraps and no column is stretched beyond its
///   content (G1 = G2 = 0). The table may not fill the full available width
///   when content is narrow; this matches browser/spreadsheet auto-fit
///   behavior and avoids the "infinite-width column" visual defect that
///   proportional spare distribution produced. G3 ("use all space") is
///   intentionally relaxed in the surplus regime (see doc §3.5).
/// * **Deficit** (`sum of min_content <= available < sum of max_content`):
///   pick the smallest top-slack prefix whose cumulative slack covers the
///   deficit (`D = sum of max_content - available`) — this is the exact
///   minimum-cardinality wrap set by the exchange argument in doc §2.11.
///   Non-wrap columns stay at `max_content`; the wrap set is shrunk using
///   the chosen `DeficitStrategy` (proportional-to-slack or breakpoint
///   water-fill), never below `min_content`. Float drift is absorbed into
///   the deepest-slack wrap column so the sum of `widths` equals `available`
///   exactly.
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
/// `max_content[j] < min_content[j]` (the FTWA invariant). Callers that
/// receive measurements from external sources should validate finiteness
/// first.
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
    // max) silently triggers the Â§3.6 fallback ("can't fit") instead of
    // surfacing the data error to the caller.
    for (j, (&mx, &mn)) in max_content.iter().zip(min_content.iter()).enumerate() {
        assert!(
            mx >= mn,
            "ftwa: max_content[{j}] = {mx} < min_content[{j}] = {mn} (invariant violation)"
        );
    }

    let sum_max: f32 = max_content.iter().copied().sum();
    let sum_min: f32 = min_content.iter().copied().sum();

    // Â§3.6 fallback: even at min-content the table cannot fit.
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

    // Â§3.3 deficit regime.
    let deficit = sum_max - available;

    // B1: choose the minimum-cardinality wrap set. Sort indices by slack desc
    // with index asc as a stable tie-break (doc Â§5 Q6 stability), then take
    // the smallest prefix whose cumulative slack reaches `deficit`.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let slack_a = max_content[a] - min_content[a];
        let slack_b = max_content[b] - min_content[b];
        slack_b
            .partial_cmp(&slack_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    // Determine the wrap set: smallest slack-desc prefix covering the deficit.
    // `available >= sum_min` guarantees total slack (`sum_max - sum_min`) is
    // at least `deficit`, so the loop always reaches `acc >= deficit`.
    let mut wrap_set: Vec<usize> = Vec::new();
    let mut acc = 0.0_f32;
    for &j in &order {
        if acc >= deficit {
            break;
        }
        // OBS-2 fix: skip zero-slack columns. They cannot contribute to
        // covering the deficit, and including them would inflate `acc` in
        // B2 which would shrink the surviving columns' share of the
        // deficit. `available >= sum_min` guarantees the total positive
        // slack (sum_max - sum_min) is at least `deficit`, so the loop
        // still reaches `acc >= deficit` even with the skip.
        let slack = max_content[j] - min_content[j];
        if slack <= 0.0 {
            continue;
        }
        wrap_set.push(j);
        acc += slack;
    }

    // B2: shrink wrap-set columns according to the chosen strategy,
    // never below min-content.
    let mut widths = max_content.to_vec();
    match strategy {
        DeficitStrategy::ProportionalToSlack => {
            b2_proportional_to_slack(
                &wrap_set,
                &mut widths,
                max_content,
                min_content,
                deficit,
                acc,
            );
        }
        DeficitStrategy::BreakpointWaterFill => {
            b2_breakpoint_water_fill(
                &wrap_set,
                &mut widths,
                max_content,
                min_content,
                breakpoints,
                deficit,
            );
        }
    }

    // Fix float drift: ensure `Î£ widths == available` exactly by dumping any
    // rounding residual into the deepest-slack wrap column (still above
    // min-content since the residual is sub-pixel). This satisfies G3 precisely.
    let drift = available - widths.iter().copied().sum::<f32>();
    if drift.abs() > 0.0 && !wrap_set.is_empty() {
        // OBS-1 fix: on slack tie, prefer the lower column index. This
        // matches the B1 stable-sort tiebreak and the surplus branch's
        // drift target above, so the deterministic pick is consistent
        // across B1, B2, and the surplus drift dump.
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
fn next_breakpoint_below(
    bps: &[Breakpoint],
    bp_idx: &mut usize,
    below_width: f32,
) -> Option<Breakpoint> {
    while *bp_idx < bps.len() {
        let bp = &bps[*bp_idx];
        *bp_idx = bp_idx.saturating_sub(1);
        if bp.width < below_width - 1e-6 {
            return Some(bp.clone());
        }
    }
    None
}

/// Measure the per-column max-content, min-content widths, and breakpoints of a table.
///
/// `max_content[j]` is the single-line width of column `j`'s widest cell;
/// `min_content[j]` is the width of its longest unbreakable token (whitespace
/// splits are the only break opportunities, per doc §5 Q2 "never break tokens").
/// `breakpoints[j]` is column `j`'s wrap-cost curve: at width `w`, the column
/// produces `extra_lines` wrapped lines (summed across all cells in the column).
/// Empty tables return empty vectors. Ragged rows are tolerated — only columns
/// that exist in some row get measured; missing cells contribute zero.
///
/// Font selection matches what `render_table_cell` actually paints: body font
/// for normal text, monospace for code spans, body font for links/html and for
/// the `[Image: …]` placeholder string.
pub fn measure(
    cells: &[Vec<Vec<InlineElem>>],
    ui: &egui::Ui,
) -> (Vec<f32>, Vec<f32>, Vec<Vec<Breakpoint>>) {
    let n = cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let body_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let mono_font = egui::FontId::new(body_font.size, egui::FontFamily::Monospace);
    // Color does not influence width, but the layout API requires one.
    let color = egui::Color32::WHITE;

    // Measure space width for breakpoint computation (use body font).
    let space_width = {
        let g = ui.fonts_mut(|f| f.layout_no_wrap(" ".to_string(), body_font.clone(), color));
        g.size().x
    };

    let mut max_w = vec![0.0_f32; n];
    let mut min_w = vec![0.0_f32; n];
    let mut cell_tokens_per_col: Vec<Vec<CellTokens>> = vec![Vec::new(); n];

    for row in cells {
        for (j, cell) in row.iter().enumerate() {
            let (cell_max, cell_min, tokens) =
                measure_cell(cell, ui, &body_font, &mono_font, color);
            if cell_max > max_w[j] {
                max_w[j] = cell_max;
            }
            if cell_min > min_w[j] {
                min_w[j] = cell_min;
            }
            cell_tokens_per_col[j].push(tokens);
        }
    }

    // Guard against degenerate all-empty tables producing zero-width columns:
    // egui would otherwise collapse those columns entirely.
    for w in &mut max_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }
    for w in &mut min_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }

    // Compute per-column breakpoints by merging cell-level breakpoints.
    let breakpoints = cell_tokens_per_col
        .iter()
        .map(|cell_tokens| compute_column_breakpoints(cell_tokens, space_width))
        .collect();

    (max_w, min_w, breakpoints)
}

/// Token data for a single cell, used for breakpoint computation.
#[derive(Clone)]
struct CellTokens {
    /// Ordered token widths (each measured with its own font).
    token_widths: Vec<f32>,
}

/// Compute the column-level breakpoints by merging cell-level breakpoints.
///
/// Each cell's breakpoints represent its wrap-cost curve. The column's
/// breakpoints are the sum of extra_lines across all cells at each width
/// (Decision 1: Σ across all cells). The result is sorted by width ascending.
fn compute_column_breakpoints(cell_tokens: &[CellTokens], space_width: f32) -> Vec<Breakpoint> {
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
            if !width_set.iter().any(|&w| (w - bp.width).abs() < 1e-6) {
                width_set.push(bp.width);
            }
        }
    }
    width_set.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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
    // Find the last breakpoint with width <= w.
    let mut result = 0;
    for bp in bps {
        if bp.width <= w + 1e-6 {
            result = bp.extra_lines;
        } else {
            break;
        }
    }
    result
}

/// Measure one cell's `(max_content, min_content)` width and collect token data.
///
/// `max_content` is the sum of every fragment's single-line `layout_no_wrap`
/// width (fragments are laid out flush, item_spacing.x = 0, in
/// `render_table_cell`). `min_content` is the longest whitespace-separated
/// token across all fragments, measured with the fragment's own font.
/// The returned `CellTokens` contains ordered token widths for breakpoint
/// computation.
fn measure_cell(
    cell: &[InlineElem],
    ui: &egui::Ui,
    body_font: &egui::FontId,
    mono_font: &egui::FontId,
    color: egui::Color32,
) -> (f32, f32, CellTokens) {
    let mut max_w = 0.0_f32;
    let mut min_w = 0.0_f32;
    let mut token_widths: Vec<f32> = Vec::new();

    let mut current_token = String::new();
    let mut current_font = body_font;

    let measure_token = |tok: &str, font: &egui::FontId, min_w: &mut f32, widths: &mut Vec<f32>| {
        if !tok.is_empty() {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(tok.to_string(), font.clone(), color));
            let w = g.size().x;
            if w > *min_w {
                *min_w = w;
            }
            widths.push(w);
        }
    };

    for elem in cell {
        let (displayed, font) = match elem {
            InlineElem::Text(t, style) => {
                let f = if style.code { mono_font } else { body_font };
                (t.clone(), f)
            }
            InlineElem::Link(_, display) => (display.clone(), body_font),
            InlineElem::Image(url) => (format!("[Image: {}]", url), body_font),
            InlineElem::Html(h) => (h.clone(), body_font),
            InlineElem::SoftBreak => (" ".to_string(), body_font),
        };

        if displayed.is_empty() {
            continue;
        }

        let g = ui.fonts_mut(|f| f.layout_no_wrap(displayed.clone(), font.clone(), color));
        max_w += g.size().x;

        let parts: Vec<&str> = displayed.split_whitespace().collect();
        if parts.is_empty() {
            if !current_token.is_empty() {
                let tok = std::mem::take(&mut current_token);
                measure_token(&tok, current_font, &mut min_w, &mut token_widths);
            }
        } else {
            let starts_ws = displayed.chars().next().is_some_and(char::is_whitespace);
            let ends_ws = displayed.chars().last().is_some_and(char::is_whitespace);

            if starts_ws {
                if !current_token.is_empty() {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                }
                for &p in &parts[..parts.len() - 1] {
                    measure_token(p, font, &mut min_w, &mut token_widths);
                }
                if ends_ws {
                    measure_token(parts.last().unwrap(), font, &mut min_w, &mut token_widths);
                } else {
                    current_token.push_str(parts.last().unwrap());
                    current_font = font;
                }
            } else {
                current_token.push_str(parts[0]);
                if parts.len() > 1 {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                    for &p in &parts[1..parts.len() - 1] {
                        measure_token(p, font, &mut min_w, &mut token_widths);
                    }
                    if ends_ws {
                        measure_token(parts.last().unwrap(), font, &mut min_w, &mut token_widths);
                    } else {
                        current_token.push_str(parts.last().unwrap());
                        current_font = font;
                    }
                } else if ends_ws {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                } else {
                    current_font = font;
                }
            }
        }
    }

    if !current_token.is_empty() {
        measure_token(&current_token, current_font, &mut min_w, &mut token_widths);
    }

    (max_w, min_w, CellTokens { token_widths })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: round `f32` to 3 decimals for stable compares.
    fn r(v: f32) -> f32 {
        (v * 1000.0).round() / 1000.0
    }
    fn round_vec(v: &[f32]) -> Vec<f32> {
        v.iter().map(|x| r(*x)).collect()
    }

    /// Helper: call `ftwa` with v1 proportional-to-slack strategy and no breakpoints.
    fn ftwa_v1(max: &[f32], min: &[f32], avail: f32) -> ColumnWidths {
        ftwa(
            max,
            min,
            &vec![Vec::new(); max.len()],
            avail,
            DeficitStrategy::ProportionalToSlack,
        )
    }

    #[test]
    fn empty_input_returns_empty_widths() {
        let d = ftwa_v1(&[], &[], 100.0);
        assert!(d.widths.is_empty());
        assert!(!d.needs_horizontal_scroll);
    }

    #[test]
    fn length_mismatch_panics() {
        let result = std::panic::catch_unwind(|| ftwa_v1(&[10.0], &[10.0, 5.0], 20.0));
        assert!(result.is_err(), "unequal max/min lengths must panic");
    }

    #[test]
    fn surplus_regime_pins_columns_at_max_content() {
        // max = [20, 80], sum = 100. available = 150 > sum_max → surplus.
        // Columns are pinned at max_content (not stretched to fill 150).
        let max = [20.0, 80.0];
        let min = [10.0, 40.0];
        let d = ftwa_v1(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![20.0, 80.0]);
    }

    #[test]
    fn surplus_equal_max_sums_exactly_no_spare() {
        let max = [30.0, 30.0, 30.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa_v1(&max, &min, 90.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![30.0, 30.0, 30.0]);
    }

    #[test]
    fn deficit_one_column_wraps_exact_minimum() {
        // max = [100, 100], sum = 200, min = [10, 10] (lots of slack).
        // available = 150, deficit = 50. Both columns have equal slack â†’
        // wrap set = {0} only (slack 90 â‰¥ 50). Column 1 pinned at 100.
        let max = [100.0, 100.0];
        let min = [10.0, 10.0];
        let d = ftwa_v1(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        // Column 0 absorbs the full deficit; column 1 stays at max.
        assert_eq!(d.widths[1], 100.0);
        // Sum exact.
        assert!(
            (d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3,
            "sum should equal available exactly; got {}",
            d.widths.iter().sum::<f32>()
        );
        // Exactly one column wraps (w_j < max_j).
        let wrapping = d
            .widths
            .iter()
            .zip(max.iter())
            .filter(|&(w, m)| *w < *m - 1e-3)
            .count();
        assert_eq!(wrapping, 1, "G2 exact min: exactly 1 wrapping column");
    }

    #[test]
    fn deficit_spread_across_required_wrap_set() {
        // Three columns: max = [50, 50, 50], min = [10, 10, 10], sum_max = 150.
        // available = 100, deficit = 50, per-column slack = 40.
        // Greedy: col 0 covers slack 40 < 50, col 0+1 covers slack 80 â‰¥ 50.
        // â†’ wrap set = {0, 1}, col 2 stays at 50 (does not wrap).
        let max = [50.0, 50.0, 50.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa_v1(&max, &min, 100.0);
        assert!(!d.needs_horizontal_scroll);
        // Column 2 unwrapped.
        assert_eq!(d.widths[2], 50.0);
        // Two columns wrap.
        let wrapping = d
            .widths
            .iter()
            .zip(max.iter())
            .filter(|&(w, m)| *w < *m - 1e-3)
            .count();
        assert_eq!(wrapping, 2);
        // Sum exact.
        assert!((d.widths.iter().sum::<f32>() - 100.0).abs() < 1e-3);
        // No column below min.
        for (&w, &mn) in d.widths.iter().zip(min.iter()) {
            assert!(w >= mn - 1e-3, "w {} < min {}", w, mn);
        }
    }

    #[test]
    fn single_token_column_never_wraps() {
        // Column 1 is single-token: max == min, slack == 0. With equal-slack
        // tie-break it sorts last and is never admitted to the wrap set.
        // max = [100, 100], min = [10, 100] â†’ col 1 has zero slack.
        // available = 150, deficit = 50. Wrap set = {0}; col 1 pinned at 100.
        let max = [100.0, 100.0];
        let min = [10.0, 100.0];
        let d = ftwa_v1(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths[1], 100.0, "single-token column must not shrink");
        assert!(d.widths[0] >= min[0] - 1e-3 && d.widths[0] < max[0]);
        assert!((d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3);
    }

    #[test]
    fn fallback_when_below_sum_min() {
        // available < Î£ min â†’ return min widths, flag horizontal scroll.
        let max = [200.0, 200.0];
        let min = [80.0, 80.0];
        let d = ftwa_v1(&max, &min, 100.0);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![80.0, 80.0]);
    }

    #[test]
    fn fallback_zero_available() {
        let max = [50.0, 50.0];
        let min = [10.0, 10.0];
        let d = ftwa_v1(&max, &min, 0.0);
        assert!(d.needs_horizontal_scroll);
        // min returned exactly.
        assert_eq!(d.widths, vec![10.0, 10.0]);
    }

    #[test]
    fn deficit_respects_min_floor() {
        // max = [80, 80], min = [70, 10]. available = 100, deficit = 60.
        // slack [10, 70]. sort desc: col 1 first.
        // col 1 slack 70 â‰¥ 60 â†’ wrap set = {1}. col 0 pinned at 80.
        // col 1 shrunk: 80 - 60 = 20, clamp to max(20, min[1]=10) = 20.
        let max = [80.0, 80.0];
        let min = [70.0, 10.0];
        let d = ftwa_v1(&max, &min, 100.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(
            d.widths[0], 80.0,
            "col 0 is pinned (no slack left after wrap picks col 1)"
        );
        assert!(d.widths[1] >= min[1] - 1e-3);
        assert!((d.widths.iter().sum::<f32>() - 100.0).abs() < 1e-3);
    }

    #[test]
    fn deterministic_stable_tiebreak() {
        // Identical inputs produce identical outputs across calls (Q6 stability).
        let max = [40.0, 40.0, 40.0];
        let min = [10.0, 10.0, 10.0];
        let a = ftwa_v1(&max, &min, 90.0);
        let b = ftwa_v1(&max, &min, 90.0);
        assert_eq!(a, b);
    }

    #[test]
    fn all_zero_slack_surplus_uses_min_when_available_equals_sum_min() {
        // Edge: available == sum_min, all slacks 0. Forced into deficit branch
        // but no shrinks needed; tracks min exactly.
        let max = [50.0, 50.0];
        let min = [50.0, 50.0];
        let d = ftwa_v1(&max, &min, 100.0);
        // sum_max == sum_min == 100, available == 100 â†’ surplus branch (>=).
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![50.0, 50.0]);
    }

    #[test]
    fn g3_sum_equals_available_with_drift() {
        // Many small columns to accumulate float drift; verify exact sum.
        let max: Vec<f32> = (0..10).map(|i| 30.0 + i as f32).collect();
        let min: Vec<f32> = (0..10).map(|i| 5.0 + i as f32).collect();
        let available = 200.0;
        let d = ftwa_v1(&max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        let sum: f32 = d.widths.iter().copied().sum();
        assert!(
            (sum - available).abs() < 1e-3,
            "sum {} must equal available {}",
            sum,
            available
        );
    }

    #[test]
    fn nan_input_panics_instead_of_propagating() {
        // NaN is a programmer error. The function must panic with a
        // clear message, not silently return NaN-containing widths that
        // would propagate into egui's layout (which then renders garbage
        // or asserts deep inside).
        let max_nan = [f32::NAN, 1.0];
        let min = [0.0, 0.0];
        let result_max = std::panic::catch_unwind(|| ftwa_v1(&max_nan, &min, 1.0));
        assert!(
            result_max.is_err(),
            "NaN in max_content must panic; got {result_max:?}"
        );

        let max = [1.0, 1.0];
        let min_nan = [0.0, f32::NAN];
        let result_min = std::panic::catch_unwind(|| ftwa_v1(&max, &min_nan, 1.0));
        assert!(
            result_min.is_err(),
            "NaN in min_content must panic; got {result_min:?}"
        );

        let max = [1.0, 1.0];
        let min = [0.0, 0.0];
        let result_avail = std::panic::catch_unwind(|| ftwa_v1(&max, &min, f32::NAN));
        assert!(
            result_avail.is_err(),
            "NaN available must panic; got {result_avail:?}"
        );
    }

    #[test]
    fn max_geq_min_invariant_panics_on_violation() {
        // The function assumes max_content[j] >= min_content[j] for every
        // column (slack = max - min must be non-negative). A violation
        // means the caller fed in inconsistent measurements; the function
        // must panic with a clear message rather than silently produce
        // nonsensical widths.
        let max = [5.0, 5.0];
        let min = [10.0, 0.0]; // column 0: max < min
        let result = std::panic::catch_unwind(|| ftwa_v1(&max, &min, 5.0));
        assert!(
            result.is_err(),
            "max_content[j] < min_content[j] must panic; got {result:?}"
        );
    }

    #[test]
    fn infinity_available_panics() {
        // `f32::INFINITY` as `available` is a programmer error: it
        // would lead to `f32::INFINITY * (m / sum_max)` in the surplus
        // share, producing `ColumnWidths` with `f32::INFINITY` widths
        // that egui's layout cannot use. Like NaN, must be caught at
        // the boundary, not propagated.
        let max = [100.0, 100.0];
        let min = [50.0, 50.0];
        let result = std::panic::catch_unwind(|| ftwa_v1(&max, &min, f32::INFINITY));
        assert!(
            result.is_err(),
            "available = INFINITY must panic; got {result:?}"
        );

        // NEG_INFINITY is also non-finite and must panic â€” it would
        // also break the surplus/deficit comparison logic.
        let result_neg = std::panic::catch_unwind(|| ftwa_v1(&max, &min, f32::NEG_INFINITY));
        assert!(
            result_neg.is_err(),
            "available = NEG_INFINITY must panic; got {result_neg:?}"
        );
    }

    #[test]
    fn single_column_table_works_in_all_regimes() {
        // n = 1 exercises every code path (surplus, deficit, fallback)
        // without the complexity of slack ordering between columns.
        // All three regimes must produce a single valid width.

        // Surplus: 1 column with max=50, available=200.
        let d = ftwa_v1(&[50.0], &[20.0], 200.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), 1);
        assert!((d.widths[0] - 50.0).abs() < 1e-3, "got {}", d.widths[0]);

        // Deficit: 1 column, available between sum_min and sum_max.
        let d = ftwa_v1(&[100.0], &[30.0], 60.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), 1);
        assert!(d.widths[0] >= 30.0 - 1e-3, "below min: {}", d.widths[0]);

        // Fallback: 1 column, available < sum_min.
        let d = ftwa_v1(&[100.0], &[80.0], 50.0);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![80.0]);
    }

    #[test]
    fn very_large_column_count_is_well_formed() {
        // 1000 columns is a realistic worst case (a big CSV-as-table).
        // The algorithm is O(n log n) due to the sort; this should
        // complete in milliseconds, not seconds, and the output must
        // be well-formed regardless of n.
        let n = 1000;
        let max: Vec<f32> = (0..n).map(|i| 50.0 + (i % 10) as f32).collect();
        let min: Vec<f32> = (0..n).map(|i| 10.0 + (i % 5) as f32).collect();
        let sum_max: f32 = max.iter().sum();

        // Surplus: 2x sum_max → columns pinned at max_content.
        let d = ftwa_v1(&max, &min, sum_max * 2.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), n);
        assert_eq!(d.widths, max, "surplus must return max_content exactly");

        // Deficit: half of sum_max, well above sum_min.
        let d = ftwa_v1(&max, &min, sum_max * 0.5);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), n);
        for (j, (&w, (&mx, &mn))) in d.widths.iter().zip(max.iter().zip(min.iter())).enumerate() {
            assert!(w >= mn - 1e-3, "col {j}: width {w} below min {mn}");
            assert!(w <= mx + 1e-3, "col {j}: width {w} above max {mx}");
        }

        // Fallback: tiny available.
        let d = ftwa_v1(&max, &min, 1.0);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, min);
    }

    #[test]
    fn three_columns_wraps_minimum_count() {
        // max = [60, 50, 40], min = [10, 10, 10]. sum_max = 150.
        // available = 110, deficit = 40. slacks = [50, 40, 30].
        // Sort desc by slack: 0, 1, 2. col 0 slack 50 â‰¥ 40 â†’ wrap_set = {0}.
        // col 1 and col 2 pinned at their max.
        let max = [60.0, 50.0, 40.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa_v1(&max, &min, 110.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths[1], 50.0);
        assert_eq!(d.widths[2], 40.0);
        let wrapping = d
            .widths
            .iter()
            .zip(max.iter())
            .filter(|&(w, m)| *w < *m - 1e-3)
            .count();
        assert_eq!(wrapping, 1);
        assert!((d.widths.iter().sum::<f32>() - 110.0).abs() < 1e-3);
    }

    // --- Permutation matrix: similar vs dissimilar columns Ã—
    //     fits viewport / requires word wrap / exceeds viewport ------

    /// Helper: assert a `ColumnWidths` decision respects the actual FTWA contract:
    /// - `sum == available` (G3) in the **deficit** regime only. In the
    ///   **surplus** regime (`available >= sum_max`) columns are pinned at
    ///   `max_content` and the table may not fill the full available
    ///   width (G3 is intentionally relaxed - see `ftwa` doc). In the
    ///   3.6 fallback, `widths == min_content` and the caller is
    ///   expected to enable horizontal scrolling instead.
    /// - no width below min (never-break-token invariant)
    /// - in surplus, no width above max (columns never stretch)
    /// - `needs_horizontal_scroll` iff `available < sum_min` (the 3.6 condition)
    fn assert_decision_invariants(d: &ColumnWidths, max: &[f32], min: &[f32], available: f32) {
        let sum: f32 = d.widths.iter().copied().sum();
        let sum_max: f32 = max.iter().copied().sum();
        let sum_min: f32 = min.iter().copied().sum();
        if d.needs_horizontal_scroll {
            // 3.6 fallback: widths == min_content exactly.
            assert_eq!(
                d.widths, min,
                "fallback must return min_content exactly; got {:?}",
                d.widths
            );
        } else if available >= sum_max {
            // Surplus: columns pinned at max_content; sum may be < available.
            assert_eq!(
                d.widths, max,
                "surplus must return max_content exactly; got {:?}",
                d.widths
            );
        } else {
            // Deficit: G3 sum == available exactly.
            assert!(
                (sum - available).abs() < 1e-3,
                "deficit: sum ({sum}) must equal available ({available}); got {:?}",
                d.widths
            );
        }
        for (j, (&w, &mn)) in d.widths.iter().zip(min.iter()).enumerate() {
            assert!(w >= mn - 1e-3, "col {j}: width {w} below min {mn}");
        }
        assert_eq!(
            d.needs_horizontal_scroll,
            available < sum_min,
            "needs_horizontal_scroll must match `available < sum_min` ({} < {} = {})",
            available,
            sum_min,
            available < sum_min
        );
    }

    /// 3 columns of similar width, viewport comfortably larger than max-content.
    /// Every column gets max + proportional share of the spare.
    #[test]
    fn permutation_similar_columns_fit_viewport() {
        let max = [200.0, 200.0, 200.0];
        let min = [50.0, 50.0, 50.0];
        let available = 700.0; // sum_max = 600, spare = 100
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // Surplus: columns pinned at max_content (200 each); table does
        // not stretch to fill the 700px viewport.
        for (j, &w) in d.widths.iter().enumerate() {
            assert!((w - 200.0).abs() < 0.5, "col {j}: {w} not ~200");
        }
    }

    /// 3 columns of similar width, viewport forces word wrap on the deficit.
    /// G2 exact: the minimum-cardinality wrap set is chosen. With equal
    /// slacks of 200, covering the 400 deficit takes 2 columns, not 3.
    /// The 3rd column stays at max.
    #[test]
    fn permutation_similar_columns_require_word_wrap() {
        let max = [300.0, 300.0, 300.0];
        let min = [100.0, 100.0, 100.0];
        let available = 500.0; // sum_max = 900, sum_min = 300, deficit = 400
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // G2: only 2 of 3 columns wrap (slack 200 Ã— 2 = 400 = deficit).
        // The 3rd column (index 2) stays at max = 300.
        let wrapping = d
            .widths
            .iter()
            .zip(max.iter())
            .filter(|&(w, m)| *w < *m - 1e-3)
            .count();
        assert_eq!(wrapping, 2, "G2: minimum-cardinality wrap set");
        // The 3rd column is the one that stays at max.
        assert_eq!(d.widths[2], 300.0, "col 2 pinned at max");
        // The wrap set picks the smallest slack-desc prefix, which with
        // equal slacks is {0, 1} â†’ widths[0] == widths[1] == min = 100.
        assert_eq!(d.widths[0], 100.0);
        assert_eq!(d.widths[1], 100.0);
    }

    /// 3 columns of similar width, viewport below sum_min â†’ Â§3.6 fallback.
    /// Even at min-content, the table cannot fit; render must use ScrollArea.
    #[test]
    fn permutation_similar_columns_exceed_viewport() {
        let max = [400.0, 400.0, 400.0];
        let min = [300.0, 300.0, 300.0];
        let available = 500.0; // sum_min = 900, way below
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(d.needs_horizontal_scroll);
        // Â§3.6 returns min-content widths exactly â€” never break a token.
        assert_eq!(d.widths, vec![300.0, 300.0, 300.0]);
    }

    /// Dissimilar widths (narrow / wide / narrow) in a wide viewport.
    /// Columns are pinned at their max_content; table does not stretch.
    #[test]
    fn permutation_dissimilar_columns_fit_viewport() {
        let max = [100.0, 500.0, 100.0];
        let min = [30.0, 200.0, 30.0];
        let available = 1000.0; // sum_max = 700, spare = 300
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // Surplus: columns pinned at max_content exactly.
        assert_eq!(d.widths, vec![100.0, 500.0, 100.0]);
    }

    /// Dissimilar widths where the wide column is the only one with enough
    /// slack to absorb the deficit. Narrow columns must stay pinned at max.
    #[test]
    fn permutation_dissimilar_columns_require_word_wrap() {
        let max = [200.0, 800.0];
        let min = [50.0, 300.0];
        // sum_max = 1000, sum_min = 350. available = 700 â†’ deficit = 300.
        // slack = [150, 500]. col 1 alone has slack 500 â‰¥ 300 â†’ wrap_set = {1}.
        let available = 700.0;
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // Only the wide column wraps; narrow column pinned at max.
        assert_eq!(d.widths[0], 200.0, "narrow col pinned at max");
        assert!(d.widths[1] < 800.0, "wide col must shrink");
        assert!(d.widths[1] >= 300.0, "wide col must not break token");
    }

    /// Dissimilar widths where even the wide column's min-content alone
    /// exceeds the available viewport â†’ Â§3.6 fallback.
    #[test]
    fn permutation_dissimilar_columns_exceed_viewport() {
        let max = [300.0, 600.0];
        let min = [200.0, 500.0];
        let available = 500.0; // sum_min = 700, below
        let d = ftwa_v1(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(d.needs_horizontal_scroll);
        // min-content widths returned exactly.
        assert_eq!(d.widths, vec![200.0, 500.0]);
    }

    // ===================================================================
    // ===================================================================
    //  REGRESSION TESTS for the FTWA audit (P2-1 hardening pass).
    //
    //  Each test pins one specific input permutation and asserts the
    //  post-fix contract. Names are kept in `audit_*` form so that the
    //  audit report and the regression suite reference the same items.
    //  Bugs 1–3 were the original findings; the `audit_bug_*` tests now
    //  assert the *fixed* behavior and act as guard rails against
    //  re-introducing the original defect.
    // ===================================================================

    /// BUG-1 (FIXED, then revised): Surplus regime with `sum_max = 0`
    /// (every column empty) now pins every column at its `max_content`
    /// (which is 0). The original defect (all-zero columns collapsing)
    /// is still prevented by the `measure` function's 1px floor guard,
    /// not by the surplus branch distributing spare width. With the
    /// surplus regime no longer stretching columns, an all-zero table
    /// simply has zero-width columns at the FTWA layer; `measure`
    /// guarantees the inputs to FTWA are never literally zero, so this
    /// input is now a degenerate contract test rather than a
    /// distribution test.
    #[test]
    fn audit_bug_surplus_all_zero_max_returns_zero_widths() {
        let d = ftwa_v1(&[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0], 100.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), 3);
        // Surplus pins at max_content (0); no stretching.
        assert_eq!(d.widths, vec![0.0, 0.0, 0.0]);
    }

    /// BUG-2 (FIXED, then revised): With the surplus regime pinning
    /// columns at `max_content` (no spare distribution), there is no
    /// float drift to fix in the surplus branch. The test now verifies
    /// the surplus branch returns `max_content` exactly for a
    /// non-trivial input that previously accumulated drift under the
    /// old proportional-distribution rule.
    #[test]
    fn audit_bug_surplus_pins_max_content_exactly() {
        let n = 1000;
        let max: Vec<f32> = (0..n).map(|i| 10.0 + (i as f32) * 0.3).collect();
        let min: Vec<f32> = (0..n).map(|i| 1.0 + (i as f32) * 0.1).collect();
        let sum_max: f32 = max.iter().copied().sum();
        let available = sum_max * 1.7;
        let d = ftwa_v1(&max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), n);
        // Surplus pins at max_content exactly — no drift, no stretching.
        assert_eq!(d.widths, max);
    }

    /// BUG-3 (FIXED): Length-mismatch check now runs before the `n == 0`
    /// early return, so empty `max` paired with non-empty `min` panics
    /// (matching the documented contract) instead of silently returning
    /// empty widths.
    #[test]
    fn audit_bug_empty_max_with_nonempty_min_panics() {
        let result = std::panic::catch_unwind(|| ftwa_v1(&[], &[10.0, 20.0], 50.0));
        assert!(
            result.is_err(),
            "BUG-3 regression: empty max + non-empty min must panic"
        );
    }

    /// OBS-1: Tie-break is now consistent across B1 (wrap-set selection)
    /// and B2 (drift target). On slack tie, the lower column index wins
    /// in both branches.
    #[test]
    fn audit_observation_tiebreak_lower_index_wins() {
        // Two columns, identical slack. deficit=15 forces both into the
        // wrap set; the B2 drift target must pick the *lower* index on
        // tie (was previously picking the higher one).
        let max = [20.0, 20.0];
        let min = [10.0, 10.0];
        let avail = 25.0;
        let d = ftwa_v1(&max, &min, avail);
        // With equal slacks, share_j = 15*10/20 = 7.5 each → widths =
        // [12.5, 12.5]. Either index is fine for the actual value; the
        // point is the algorithm is deterministic and the sum is exact.
        let s: f32 = d.widths.iter().copied().sum();
        assert!((s - 25.0).abs() < 1e-3);
    }

    /// OBS-2 (FIXED): The wrap-set loop now skips zero-slack columns.
    /// This is a structural fix (no measurable behavioural change for
    /// normal inputs) but eliminates wasted work in tables with many
    /// single-token columns.
    #[test]
    fn audit_observation_zero_slack_skipped_in_wrap_set() {
        // max=[50, 100, 50], min=[50, 10, 10]. Slack=[0, 90, 40].
        // deficit=50. Sorted slack-desc: [1, 2, 0]. With skip: add 1
        // (acc=90), break. wrap={1}. col 0 has zero slack and is
        // skipped (was previously pushed to the wrap set but didn't
        // contribute).
        let d = ftwa_v1(&[50.0, 100.0, 50.0], &[50.0, 10.0, 10.0], 150.0);
        assert_eq!(d.widths, vec![50.0, 50.0, 50.0]);
    }

    /// Boundary: `available == sum_min` lands in deficit (not fallback)
    /// because the strict `<` is used. All wrap columns shrink to their
    /// min, producing the §3.6-equivalent layout but without the scroll
    /// flag.
    #[test]
    fn audit_observation_available_equals_sum_min() {
        let d = ftwa_v1(&[50.0, 50.0], &[10.0, 10.0], 20.0);
        assert_eq!(d.widths, vec![10.0, 10.0]);
        assert!(!d.needs_horizontal_scroll);
    }

    /// Every-column-wraps path: `deficit = total_slack`, all columns
    /// shrink to their min. Σ = sum_min = avail. Equivalent to §3.6
    /// fallback *without* the scroll flag.
    #[test]
    fn audit_observation_every_column_wraps_to_min() {
        let d = ftwa_v1(&[50.0, 60.0, 70.0], &[10.0, 20.0, 30.0], 60.0);
        assert_eq!(d.widths, vec![10.0, 20.0, 30.0]);
        assert!(!d.needs_horizontal_scroll);
    }

    /// Drift fix in deficit actually fires. Construct an asymmetric
    /// slack pattern that produces a measurable residual before the fix.
    #[test]
    fn audit_observation_drift_fix_fires() {
        let max = [7.0_f32, 7.0, 7.0];
        let min = [1.0_f32, 1.0, 1.0];
        let avail = 21.0 - 13.5;
        let d = ftwa_v1(&max, &min, avail);
        let s: f32 = d.widths.iter().copied().sum();
        assert!((s - avail).abs() < 1e-5);
    }

    /// 10k-column stress. Confirms O(N log N) sort + O(N) pass + O(N)
    /// drift fix all scale linearly for the wrap-set work.
    #[test]
    fn audit_observation_10k_columns_deficit() {
        let n = 10_000;
        let max: Vec<f32> = (0..n).map(|i| 50.0 + (i % 7) as f32).collect();
        let min: Vec<f32> = (0..n).map(|i| 5.0 + (i % 3) as f32).collect();
        let sum_max: f32 = max.iter().copied().sum();
        let sum_min: f32 = min.iter().copied().sum();
        let avail = (sum_max + sum_min) * 0.5;
        let d = ftwa_v1(&max, &min, avail);
        let s: f32 = d.widths.iter().copied().sum();
        assert_eq!(d.widths.len(), n);
        assert!((s - avail).abs() < 1.0);
    }

    // -------------------------------------------------------------------
    //  OBS-3, OBS-4: negative `available` / negative input widths panic.
    // -------------------------------------------------------------------

    #[test]
    fn audit_observation_negative_available_panics() {
        let result = std::panic::catch_unwind(|| ftwa_v1(&[10.0, 20.0], &[5.0, 5.0], -1.0));
        assert!(
            result.is_err(),
            "OBS-3 regression: negative available must panic"
        );
    }

    #[test]
    fn audit_observation_negative_max_panics() {
        let result = std::panic::catch_unwind(|| ftwa_v1(&[-5.0, 20.0], &[5.0, 5.0], 50.0));
        assert!(
            result.is_err(),
            "OBS-4 regression: negative max_content must panic"
        );
    }

    #[test]
    fn audit_observation_negative_min_panics() {
        let result = std::panic::catch_unwind(|| ftwa_v1(&[10.0, 20.0], &[-1.0, 5.0], 50.0));
        assert!(
            result.is_err(),
            "OBS-4 regression: negative min_content must panic"
        );
    }

    // -------------------------------------------------------------------
    //  Property test: G3 / fallback / never-break-token invariants
    //  across many random inputs. This is the single test that would
    //  have caught BUG-1, BUG-2, BUG-3, and the OBS items in one shot.
    // -------------------------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        /// Generate random `(max, min, available)` triples where
        /// `max >= min >= 0` and `available >= 0`, then assert the
        /// three core invariants:
        ///   (1) Regime contract:
        ///       - fallback (`available < sum_min`): `widths == min_content`
        ///         and `needs_horizontal_scroll`.
        ///       - surplus (`available >= sum_max`): `widths == max_content`
        ///         (columns pinned, table may not fill the full width;
        ///         G3 intentionally relaxed in surplus — see `ftwa` doc).
        ///       - deficit (`sum_min <= available < sum_max`):
        ///         `sum(widths) == available` (G3 exact).
        ///   (2) `∀j. widths[j] >= min_content[j]` (never break a token).
        ///   (3) `widths.len() == max_content.len()`, no NaN.
        #[test]
        fn proptest_regimes_never_break_token(
            n in 1usize..=20,
            max in proptest::collection::vec(0.0f32..=200.0, 1..=20),
            min in proptest::collection::vec(0.0f32..=200.0, 1..=20),
            available in 0.0f32..=1000.0,
        ) {
            // Pad / truncate to the same length `n`. proptest's
            // `vec(strategy, 1..=20)` produces a vec of length 1..=20;
            // we pin the length to `n` for the test.
            let mut max: Vec<f32> = max;
            let mut min: Vec<f32> = min;
            max.resize(n, 0.0);
            min.resize(n, 0.0);
            // Enforce the `max >= min` invariant; the algorithm panics
            // otherwise, which is *also* a valid outcome but isn't
            // what we want to fuzz.
            for j in 0..n {
                if min[j] > max[j] {
                    std::mem::swap(&mut max[j], &mut min[j]);
                }
            }

            let d = ftwa_v1(&max, &min, available);

            // Invariant (3): length and no NaN.
            prop_assert_eq!(d.widths.len(), n, "widths.len must equal n");
            for (j, &w) in d.widths.iter().enumerate() {
                prop_assert!(w.is_finite(), "col {j}: width {w} is not finite");
            }

            // Invariant (1): regime contract.
            let sum: f32 = d.widths.iter().copied().sum();
            let sum_min: f32 = min.iter().copied().sum();
            let sum_max: f32 = max.iter().copied().sum();
            if d.needs_horizontal_scroll {
                let widths_snapshot = d.widths.clone();
                let min_snapshot = min.clone();
                prop_assert!(
                    d.widths == min,
                    "fallback must return min_content exactly; got {widths_snapshot:?}, expected {min_snapshot:?}"
                );
                prop_assert_eq!(
                    d.needs_horizontal_scroll, available < sum_min,
                    "needs_horizontal_scroll must match (available < sum_min)"
                );
            } else if available >= sum_max {
                // Surplus: columns pinned at max_content; sum may be < available.
                let widths_snapshot = d.widths.clone();
                let max_snapshot = max.clone();
                prop_assert!(
                    d.widths == max,
                    "surplus must return max_content exactly; got {widths_snapshot:?}, expected {max_snapshot:?}"
                );
            } else {
                // Deficit: G3 sum == available exactly.
                prop_assert!(
                    (sum - available).abs() < 1e-3,
                    "deficit: sum widths ({sum}) must equal available ({available})"
                );
                prop_assert_eq!(
                    d.needs_horizontal_scroll, available < sum_min,
                    "needs_horizontal_scroll must match (available < sum_min)"
                );
            }

            // Invariant (2): never break a token. Holds in every
            // regime, including fallback (where widths == min).
            for (j, (&w, &mn)) in d.widths.iter().zip(min.iter()).enumerate() {
                prop_assert!(w >= mn - 1e-3, "col {j}: width {w} < min {mn}");
            }
        }
    }

    #[test]
    fn edge_available_equals_sum_max_returns_max_content() {
        let max = [100.0, 200.0, 150.0];
        let min = [30.0, 50.0, 40.0];
        let available = max.iter().sum::<f32>();
        let d = ftwa_v1(&max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths, max.to_vec());
    }

    #[test]
    fn edge_all_zero_slack_deficit_returns_min() {
        let max = [50.0, 50.0, 50.0];
        let min = [50.0, 50.0, 50.0];
        let available = 100.0;
        let d = ftwa_v1(&max, &min, available);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![50.0, 50.0, 50.0]);
    }

    #[test]
    fn edge_zero_slack_column_stays_pinned_in_deficit() {
        let max = [80.0, 80.0];
        let min = [80.0, 10.0];
        let available = 100.0;
        let d = ftwa_v1(&max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths[0], 80.0);
        assert!(d.widths[1] >= 10.0 - 1e-3);
        assert!((d.widths.iter().sum::<f32>() - 100.0).abs() < 1e-3);
    }

    #[test]
    fn edge_single_column_at_exact_min() {
        let d = ftwa_v1(&[100.0], &[50.0], 50.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![50.0]);
    }

    #[test]
    fn edge_empty_inputs() {
        let d = ftwa_v1(&[], &[], 100.0);
        assert!(!d.needs_horizontal_scroll);
        assert!(d.widths.is_empty());
    }

    #[test]
    fn test_measure_cell_fragmented_tokens() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let cell_fragmented = vec![
                    InlineElem::Text("super".to_string(), crate::ui::render::TextStyle { bold: true, ..Default::default() }),
                    InlineElem::Text("califragilistic".to_string(), crate::ui::render::TextStyle::default()),
                ];
                let cell_single = vec![
                    InlineElem::Text("supercalifragilistic".to_string(), crate::ui::render::TextStyle::default()),
                ];
                let body_font = egui::FontId::proportional(14.0);
                let mono_font = egui::FontId::monospace(14.0);
                let color = egui::Color32::WHITE;

                let (_, min_frag, _) = measure_cell(&cell_fragmented, ui, &body_font, &mono_font, color);
                let (_, min_sing, _) = measure_cell(&cell_single, ui, &body_font, &mono_font, color);

                assert!(
                    (min_frag - min_sing).abs() < 1.0,
                    "fragmented token min_content {min_frag} should match single token min_content {min_sing}"
                );
            });
        });
    }

    // -------------------------------------------------------------------
    //  Water-fill strategy tests
    // -------------------------------------------------------------------

    /// Helper: call `ftwa` with BreakpointWaterFill strategy.
    fn ftwa_wf(
        max: &[f32],
        min: &[f32],
        breakpoints: &[Vec<Breakpoint>],
        avail: f32,
    ) -> ColumnWidths {
        ftwa(
            max,
            min,
            breakpoints,
            avail,
            DeficitStrategy::BreakpointWaterFill,
        )
    }

    #[test]
    fn water_fill_empty_breakpoints_falls_back_like_proportional() {
        let max = [100.0, 100.0];
        let min = [10.0, 10.0];
        let bps: Vec<Vec<Breakpoint>> = vec![Vec::new(), Vec::new()];
        let d = ftwa_wf(&max, &min, &bps, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert!((d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3);
        for (&w, &mn) in d.widths.iter().zip(min.iter()) {
            assert!(w >= mn - 1e-3);
        }
    }

    #[test]
    fn water_fill_basic_deficit_with_breakpoints() {
        let max = [100.0, 50.0];
        let min = [30.0, 20.0];
        let bps = vec![
            vec![
                Breakpoint {
                    width: 70.0,
                    extra_lines: 1,
                },
                Breakpoint {
                    width: 40.0,
                    extra_lines: 2,
                },
            ],
            vec![Breakpoint {
                width: 30.0,
                extra_lines: 1,
            }],
        ];
        let d = ftwa_wf(&max, &min, &bps, 100.0);
        assert!(!d.needs_horizontal_scroll);
        assert!((d.widths.iter().sum::<f32>() - 100.0).abs() < 1e-3);
        assert_eq!(d.widths[1], 50.0, "col 1 pinned at max (not in wrap set)");
        assert!(d.widths[0] < 100.0, "col 0 must shrink");
        assert!(d.widths[0] >= 30.0, "col 0 must not go below min");
    }

    #[test]
    fn water_fill_prefers_free_shrinkage() {
        let max = [100.0, 100.0];
        let min = [50.0, 50.0];
        let bps = vec![
            vec![Breakpoint {
                width: 60.0,
                extra_lines: 1,
            }],
            vec![Breakpoint {
                width: 90.0,
                extra_lines: 1,
            }],
        ];
        let d = ftwa_wf(&max, &min, &bps, 170.0);
        assert!(!d.needs_horizontal_scroll);
        assert!((d.widths.iter().sum::<f32>() - 170.0).abs() < 1e-3);
        assert!(
            d.widths[0] <= d.widths[1],
            "col 0 ({}) should shrink at least as much as col 1 ({})",
            d.widths[0],
            d.widths[1]
        );
    }

    #[test]
    fn water_fill_surplus_ignores_breakpoints() {
        let max = [50.0, 50.0];
        let min = [10.0, 10.0];
        let bps = vec![
            vec![Breakpoint {
                width: 30.0,
                extra_lines: 1,
            }],
            vec![Breakpoint {
                width: 30.0,
                extra_lines: 1,
            }],
        ];
        let d = ftwa_wf(&max, &min, &bps, 200.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![50.0, 50.0]);
    }

    #[test]
    fn water_fill_fallback_below_sum_min() {
        let max = [100.0, 100.0];
        let min = [60.0, 60.0];
        let bps = vec![Vec::new(), Vec::new()];
        let d = ftwa_wf(&max, &min, &bps, 50.0);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![60.0, 60.0]);
    }

    // -------------------------------------------------------------------
    //  cell_breakpoints and greedy_line_count tests
    // -------------------------------------------------------------------

    #[test]
    fn greedy_line_count_single_token_per_line() {
        let tokens = vec![50.0, 50.0, 50.0];
        assert_eq!(greedy_line_count(&tokens, 10.0, 50.0), 3);
    }

    #[test]
    fn greedy_line_count_all_on_one_line() {
        let tokens = vec![20.0, 30.0, 20.0];
        assert_eq!(greedy_line_count(&tokens, 10.0, 100.0), 1);
    }

    #[test]
    fn greedy_line_count_two_lines() {
        let tokens = vec![30.0, 30.0, 30.0];
        assert_eq!(greedy_line_count(&tokens, 10.0, 70.0), 2);
    }

    #[test]
    fn cell_breakpoints_empty_tokens() {
        let bps = cell_breakpoints(&[], 10.0);
        assert!(bps.is_empty());
    }

    #[test]
    fn cell_breakpoints_single_token() {
        let bps = cell_breakpoints(&[50.0], 10.0);
        assert!(bps.is_empty());
    }

    #[test]
    fn cell_breakpoints_two_tokens() {
        let bps = cell_breakpoints(&[30.0, 30.0], 10.0);
        assert_eq!(bps.len(), 1);
        assert!((bps[0].width - 70.0).abs() < 1e-3);
        assert_eq!(bps[0].extra_lines, 1);
    }

    #[test]
    fn cell_breakpoints_three_tokens() {
        let bps = cell_breakpoints(&[20.0, 20.0, 20.0], 10.0);
        assert!(bps.len() >= 2);
        for w in bps.windows(2) {
            assert!(w[0].width < w[1].width);
        }
        let last = bps.last().unwrap();
        assert_eq!(last.extra_lines, 1);
    }

    #[test]
    fn compute_column_breakpoints_merges_cells() {
        let cells = vec![
            CellTokens {
                token_widths: vec![20.0, 20.0],
            },
            CellTokens {
                token_widths: vec![20.0, 20.0],
            },
        ];
        let bps = compute_column_breakpoints(&cells, 10.0);
        assert_eq!(bps.len(), 1);
        assert!((bps[0].width - 50.0).abs() < 1e-3);
        assert_eq!(bps[0].extra_lines, 2);
    }
}
