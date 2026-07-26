//! Fair Table Width Algorithm (FTWA) — assigns per-column pixel widths to a markdown/GFM table.
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
    /// `true` when `available < Σ min_content` — caller must enable horizontal scroll.
    pub needs_horizontal_scroll: bool,
}

/// Pure FTWA core. Solves the deficit regime exactly for G2 (fewest wrapping
/// columns) and approximately for G1 (within the chosen wrap set).
///
/// `max_content[j]` is column `j`'s single-line width; `min_content[j]` is its
/// longest unbreakable-token width (`min_content[j] ≤ max_content[j]`).
/// `available` is the content width minus gutters. The algorithm proceeds:
///
/// * **Surplus** (`available ≥ Σ max_content`): pin every column to `max_content`
///   and distribute the spare proportionally to `max_content` (doc §3.5/§3.2).
///   No column wraps. G1 = G2 = 0.
/// * **Deficit** (`Σ min_content ≤ available < Σ max_content`): pick the
///   smallest top-slack prefix whose cumulative slack covers the deficit
///   (`D = Σ max_content − available`) — this is the exact minimum-cardinality
///   wrap set by the exchange argument in doc §2.11. Non-wrap columns stay at
///   `max_content`; the wrap set is shrunk proportionally to slack (v1
///   simplification of the doc §3.3 B2 breakpoint water-fill), never below
///   `min_content`. Float drift is absorbed into the deepest-slack wrap column
///   so `Σ widths == available` exactly.
/// * **Fallback** (`available < Σ min_content`): return `min_content` and set
///   `needs_horizontal_scroll = true` (doc §3.6). The strongest invariant
///   (tokens never break) holds by construction.
///
/// Returns `widths.len() == max_content.len()`. Empty input → empty output,
/// no scroll needed.
pub fn ftwa(max_content: &[f32], min_content: &[f32], available: f32) -> ColumnWidths {
    let n = max_content.len();
    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }
    assert_eq!(
        n,
        min_content.len(),
        "ftwa: max_content and min_content must have equal length"
    );

    let sum_max: f32 = max_content.iter().copied().sum();
    let sum_min: f32 = min_content.iter().copied().sum();

    // §3.6 fallback: even at min-content the table cannot fit.
    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    // §3.2 surplus regime: give every column its max-content plus a fair
    // share of the spare, proportional to max-content (doc §3.5 decision Q7).
    if available >= sum_max {
        let spare = available - sum_max;
        let widths = max_content
            .iter()
            .map(|&m| {
                let share = if sum_max > 0.0 {
                    spare * (m / sum_max)
                } else {
                    0.0
                };
                m + share
            })
            .collect();
        return ColumnWidths {
            widths,
            needs_horizontal_scroll: false,
        };
    }

    // §3.3 deficit regime.
    let deficit = sum_max - available;

    // B1: choose the minimum-cardinality wrap set. Sort indices by slack desc
    // with index asc as a stable tie-break (doc §5 Q6 stability), then take
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
        wrap_set.push(j);
        acc += max_content[j] - min_content[j];
    }

    // B2: shrink wrap-set columns proportional to each column's share of the
    // captured slack `acc`, never below min-content. v1 simplification of the
    // doc §3.3 B2 breakpoint water-fill (marginal-cost minimization is future
    // work); remains G2-exact and never breaks a token.
    let mut widths = max_content.to_vec();
    for &j in &wrap_set {
        let slack_j = max_content[j] - min_content[j];
        let share = if acc > 0.0 {
            deficit * (slack_j / acc)
        } else {
            0.0
        };
        widths[j] = (max_content[j] - share).max(min_content[j]);
    }

    // Fix float drift: ensure `Σ widths == available` exactly by dumping any
    // rounding residual into the deepest-slack wrap column (still above
    // min-content since the residual is sub-pixel). This satisfies G3 precisely.
    let drift = available - widths.iter().copied().sum::<f32>();
    if drift.abs() > 0.0 && !wrap_set.is_empty() {
        let target = *wrap_set
            .iter()
            .max_by(|&&a, &&b| {
                let sa = max_content[a] - min_content[a];
                let sb = max_content[b] - min_content[b];
                sa.partial_cmp(&sb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.cmp(&b))
            })
            .unwrap();
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

/// Measure the per-column max-content and min-content widths of a table.
///
/// `max_content[j]` is the single-line width of column `j`'s widest cell;
/// `min_content[j]` is the width of its longest unbreakable token (whitespace
/// splits are the only break opportunities, per doc §5 Q2 "never break tokens").
/// Empty tables return empty vectors. Ragged rows are tolerated — only columns
/// that exist in some row get measured; missing cells contribute zero.
///
/// Font selection matches what `render_table_cell` actually paints: body font
/// for normal text, monospace for code spans, body font for links/html and for
/// the `[Image: …]` placeholder string.
pub fn measure(cells: &[Vec<Vec<InlineElem>>], ui: &egui::Ui) -> (Vec<f32>, Vec<f32>) {
    let n = cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return (Vec::new(), Vec::new());
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

    let mut max_w = vec![0.0_f32; n];
    let mut min_w = vec![0.0_f32; n];

    for row in cells {
        for (j, cell) in row.iter().enumerate() {
            let (cell_max, cell_min) = measure_cell(cell, ui, &body_font, &mono_font, color);
            if cell_max > max_w[j] {
                max_w[j] = cell_max;
            }
            if cell_min > min_w[j] {
                min_w[j] = cell_min;
            }
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

    (max_w, min_w)
}

/// Measure one cell's `(max_content, min_content)` width.
///
/// `max_content` is the sum of every fragment's single-line `layout_no_wrap`
/// width (fragments are laid out flush, item_spacing.x = 0, in
/// `render_table_cell`). `min_content` is the longest whitespace-separated
/// token across all fragments, measured with the fragment's own font.
fn measure_cell(
    cell: &[InlineElem],
    ui: &egui::Ui,
    body_font: &egui::FontId,
    mono_font: &egui::FontId,
    color: egui::Color32,
) -> (f32, f32) {
    let mut max_w = 0.0_f32;
    let mut min_w = 0.0_f32;

    for elem in cell {
        match elem {
            InlineElem::Text(t, style) => {
                let font = if style.code { mono_font } else { body_font };
                accumulate(t, font, ui, color, &mut max_w, &mut min_w);
            }
            InlineElem::Link(_, display) => {
                accumulate(display, body_font, ui, color, &mut max_w, &mut min_w);
            }
            InlineElem::Image(url) => {
                // render_table_cell prints "[Image: {url}]" — measure that.
                let displayed = format!("[Image: {}]", url);
                accumulate(&displayed, body_font, ui, color, &mut max_w, &mut min_w);
            }
            InlineElem::Html(h) => {
                accumulate(h, body_font, ui, color, &mut max_w, &mut min_w);
            }
            InlineElem::SoftBreak => {
                // SoftBreak is rendered as a single space; never a wrap point
                // of its own but contributes one space-width to max-content.
                accumulate(" ", body_font, ui, color, &mut max_w, &mut min_w);
            }
        }
    }

    (max_w, min_w)
}

/// Add `text`'s contribution to `max_w` (full single-line width) and update
/// `min_w` with the longest whitespace-separated token width.
fn accumulate(
    text: &str,
    font: &egui::FontId,
    ui: &egui::Ui,
    color: egui::Color32,
    max_w: &mut f32,
    min_w: &mut f32,
) {
    if text.is_empty() {
        return;
    }
    let g = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), color));
    *max_w += g.size().x;
    for tok in text.split_whitespace() {
        let g = ui.fonts(|f| f.layout_no_wrap(tok.to_string(), font.clone(), color));
        let w = g.size().x;
        if w > *min_w {
            *min_w = w;
        }
    }
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

    #[test]
    fn empty_input_returns_empty_widths() {
        let d = ftwa(&[], &[], 100.0);
        assert!(d.widths.is_empty());
        assert!(!d.needs_horizontal_scroll);
    }

    #[test]
    fn length_mismatch_panics() {
        let result = std::panic::catch_unwind(|| ftwa(&[10.0], &[10.0, 5.0], 20.0));
        assert!(result.is_err(), "unequal max/min lengths must panic");
    }

    #[test]
    fn surplus_regime_distributes_proportional_to_max() {
        // max = [20, 80], sum = 100. available = 150 → spare 50, split 1:4.
        let max = [20.0, 80.0];
        let min = [10.0, 40.0];
        let d = ftwa(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![30.0, 120.0]);
        assert!((d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3);
    }

    #[test]
    fn surplus_equal_max_sums_exactly_no_spare() {
        let max = [30.0, 30.0, 30.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa(&max, &min, 90.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![30.0, 30.0, 30.0]);
    }

    #[test]
    fn deficit_one_column_wraps_exact_minimum() {
        // max = [100, 100], sum = 200, min = [10, 10] (lots of slack).
        // available = 150, deficit = 50. Both columns have equal slack →
        // wrap set = {0} only (slack 90 ≥ 50). Column 1 pinned at 100.
        let max = [100.0, 100.0];
        let min = [10.0, 10.0];
        let d = ftwa(&max, &min, 150.0);
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
        // Greedy: col 0 covers slack 40 < 50, col 0+1 covers slack 80 ≥ 50.
        // → wrap set = {0, 1}, col 2 stays at 50 (does not wrap).
        let max = [50.0, 50.0, 50.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa(&max, &min, 100.0);
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
        // max = [100, 100], min = [10, 100] → col 1 has zero slack.
        // available = 150, deficit = 50. Wrap set = {0}; col 1 pinned at 100.
        let max = [100.0, 100.0];
        let min = [10.0, 100.0];
        let d = ftwa(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths[1], 100.0, "single-token column must not shrink");
        assert!(d.widths[0] >= min[0] - 1e-3 && d.widths[0] < max[0]);
        assert!((d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3);
    }

    #[test]
    fn fallback_when_below_sum_min() {
        // available < Σ min → return min widths, flag horizontal scroll.
        let max = [200.0, 200.0];
        let min = [80.0, 80.0];
        let d = ftwa(&max, &min, 100.0);
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![80.0, 80.0]);
    }

    #[test]
    fn fallback_zero_available() {
        let max = [50.0, 50.0];
        let min = [10.0, 10.0];
        let d = ftwa(&max, &min, 0.0);
        assert!(d.needs_horizontal_scroll);
        // min returned exactly.
        assert_eq!(d.widths, vec![10.0, 10.0]);
    }

    #[test]
    fn deficit_respects_min_floor() {
        // max = [80, 80], min = [70, 10]. available = 100, deficit = 60.
        // slack [10, 70]. sort desc: col 1 first.
        // col 1 slack 70 ≥ 60 → wrap set = {1}. col 0 pinned at 80.
        // col 1 shrunk: 80 - 60 = 20, clamp to max(20, min[1]=10) = 20.
        let max = [80.0, 80.0];
        let min = [70.0, 10.0];
        let d = ftwa(&max, &min, 100.0);
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
        let a = ftwa(&max, &min, 90.0);
        let b = ftwa(&max, &min, 90.0);
        assert_eq!(a, b);
    }

    #[test]
    fn all_zero_slack_surplus_uses_min_when_available_equals_sum_min() {
        // Edge: available == sum_min, all slacks 0. Forced into deficit branch
        // but no shrinks needed; tracks min exactly.
        let max = [50.0, 50.0];
        let min = [50.0, 50.0];
        let d = ftwa(&max, &min, 100.0);
        // sum_max == sum_min == 100, available == 100 → surplus branch (>=).
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![50.0, 50.0]);
    }

    #[test]
    fn g3_sum_equals_available_with_drift() {
        // Many small columns to accumulate float drift; verify exact sum.
        let max: Vec<f32> = (0..10).map(|i| 30.0 + i as f32).collect();
        let min: Vec<f32> = (0..10).map(|i| 5.0 + i as f32).collect();
        let available = 200.0;
        let d = ftwa(&max, &min, available);
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
    fn three_columns_wraps_minimum_count() {
        // max = [60, 50, 40], min = [10, 10, 10]. sum_max = 150.
        // available = 110, deficit = 40. slacks = [50, 40, 30].
        // Sort desc by slack: 0, 1, 2. col 0 slack 50 ≥ 40 → wrap_set = {0}.
        // col 1 and col 2 pinned at their max.
        let max = [60.0, 50.0, 40.0];
        let min = [10.0, 10.0, 10.0];
        let d = ftwa(&max, &min, 110.0);
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
}
