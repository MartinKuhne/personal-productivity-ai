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

/// Outcome of an FTWA computation: per-column pixel widths plus a flag telling
/// the caller that the available width is below the sum of min-content widths,
/// in which case the table physically cannot fit and horizontal scrolling must
/// be enabled (doc Â§3.6 fallback).
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
/// longest unbreakable-token width (`min_content[j] â‰¤ max_content[j]`).
/// `available` is the content width minus gutters. The algorithm proceeds:
///
/// * **Surplus** (`available â‰¥ Î£ max_content`): pin every column to `max_content`
///   and distribute the spare proportionally to `max_content` (doc Â§3.5/Â§3.2).
///   No column wraps. G1 = G2 = 0.
/// * **Deficit** (`Î£ min_content â‰¤ available < Î£ max_content`): pick the
///   smallest top-slack prefix whose cumulative slack covers the deficit
///   (`D = Î£ max_content âˆ’ available`) â€” this is the exact minimum-cardinality
///   wrap set by the exchange argument in doc Â§2.11. Non-wrap columns stay at
///   `max_content`; the wrap set is shrunk proportionally to slack (v1
///   simplification of the doc Â§3.3 B2 breakpoint water-fill), never below
///   `min_content`. Float drift is absorbed into the deepest-slack wrap column
///   so `Î£ widths == available` exactly.
/// * **Fallback** (`available < Î£ min_content`): return `min_content` and set
///   `needs_horizontal_scroll = true` (doc Â§3.6). The strongest invariant
///   (tokens never break) holds by construction.
///
/// Returns `widths.len() == max_content.len()`. Empty input â†’ empty output,
/// no scroll needed.
///
/// # Panics
///
/// Panics if `available` is not finite (NaN or Â±âˆž), if any element of
/// `max_content` or `min_content` is not finite, or if any `max_content[j]
/// < min_content[j]` (the FTWA invariant). Callers that receive
/// measurements from external sources should validate finiteness first.
///
/// # Examples
///
/// (This function is not part of the public API; the `ui::table_width`
/// module is intentionally private. See the unit tests in this file
/// for usage examples covering all three regimes.)
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

    // Â§3.2 surplus regime: give every column its max-content plus a fair
    // share of the spare, proportional to max-content (doc Â§3.5 decision Q7).
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
        wrap_set.push(j);
        acc += max_content[j] - min_content[j];
    }

    // B2: shrink wrap-set columns proportional to each column's share of the
    // captured slack `acc`, never below min-content. v1 simplification of the
    // doc Â§3.3 B2 breakpoint water-fill (marginal-cost minimization is future
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

    // Fix float drift: ensure `Î£ widths == available` exactly by dumping any
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
/// splits are the only break opportunities, per doc Â§5 Q2 "never break tokens").
/// Empty tables return empty vectors. Ragged rows are tolerated â€” only columns
/// that exist in some row get measured; missing cells contribute zero.
///
/// Font selection matches what `render_table_cell` actually paints: body font
/// for normal text, monospace for code spans, body font for links/html and for
/// the `[Image: â€¦]` placeholder string.
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
                // render_table_cell prints "[Image: {url}]" â€” measure that.
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
    // egui 0.35: `FontsView::layout_no_wrap` requires `&mut self`, so the
    // call site has to use `fonts_mut` instead of `fonts`. The shape of
    // the closure argument is otherwise identical.
    let g = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), color));
    *max_w += g.size().x;
    for tok in text.split_whitespace() {
        let g = ui.fonts_mut(|f| f.layout_no_wrap(tok.to_string(), font.clone(), color));
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
        // max = [20, 80], sum = 100. available = 150 â†’ spare 50, split 1:4.
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
        // available = 150, deficit = 50. Both columns have equal slack â†’
        // wrap set = {0} only (slack 90 â‰¥ 50). Column 1 pinned at 100.
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
        // Greedy: col 0 covers slack 40 < 50, col 0+1 covers slack 80 â‰¥ 50.
        // â†’ wrap set = {0, 1}, col 2 stays at 50 (does not wrap).
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
        // max = [100, 100], min = [10, 100] â†’ col 1 has zero slack.
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
        // available < Î£ min â†’ return min widths, flag horizontal scroll.
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
        // col 1 slack 70 â‰¥ 60 â†’ wrap set = {1}. col 0 pinned at 80.
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
    fn nan_input_panics_instead_of_propagating() {
        // NaN is a programmer error. The function must panic with a
        // clear message, not silently return NaN-containing widths that
        // would propagate into egui's layout (which then renders garbage
        // or asserts deep inside).
        let max_nan = [f32::NAN, 1.0];
        let min = [0.0, 0.0];
        let result_max = std::panic::catch_unwind(|| ftwa(&max_nan, &min, 1.0));
        assert!(
            result_max.is_err(),
            "NaN in max_content must panic; got {result_max:?}"
        );

        let max = [1.0, 1.0];
        let min_nan = [0.0, f32::NAN];
        let result_min = std::panic::catch_unwind(|| ftwa(&max, &min_nan, 1.0));
        assert!(
            result_min.is_err(),
            "NaN in min_content must panic; got {result_min:?}"
        );

        let max = [1.0, 1.0];
        let min = [0.0, 0.0];
        let result_avail = std::panic::catch_unwind(|| ftwa(&max, &min, f32::NAN));
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
        let result = std::panic::catch_unwind(|| ftwa(&max, &min, 5.0));
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
        let result = std::panic::catch_unwind(|| ftwa(&max, &min, f32::INFINITY));
        assert!(
            result.is_err(),
            "available = INFINITY must panic; got {result:?}"
        );

        // NEG_INFINITY is also non-finite and must panic â€” it would
        // also break the surplus/deficit comparison logic.
        let result_neg = std::panic::catch_unwind(|| ftwa(&max, &min, f32::NEG_INFINITY));
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
        let d = ftwa(&[50.0], &[20.0], 200.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), 1);
        assert!((d.widths[0] - 200.0).abs() < 1e-3, "got {}", d.widths[0]);

        // Deficit: 1 column, available between sum_min and sum_max.
        let d = ftwa(&[100.0], &[30.0], 60.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), 1);
        assert!(d.widths[0] >= 30.0 - 1e-3, "below min: {}", d.widths[0]);

        // Fallback: 1 column, available < sum_min.
        let d = ftwa(&[100.0], &[80.0], 50.0);
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

        // Surplus: 2x sum_max.
        let d = ftwa(&max, &min, sum_max * 2.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), n);
        let sum: f32 = d.widths.iter().sum();
        assert!((sum - sum_max * 2.0).abs() < 1.0, "Î£ must equal available");

        // Deficit: half of sum_max, well above sum_min.
        let d = ftwa(&max, &min, sum_max * 0.5);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(d.widths.len(), n);
        for (j, (&w, (&mx, &mn))) in d.widths.iter().zip(max.iter().zip(min.iter())).enumerate() {
            assert!(w >= mn - 1e-3, "col {j}: width {w} below min {mn}");
            assert!(w <= mx + 1e-3, "col {j}: width {w} above max {mx}");
        }

        // Fallback: tiny available.
        let d = ftwa(&max, &min, 1.0);
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

    // --- Permutation matrix: similar vs dissimilar columns Ã—
    //     fits viewport / requires word wrap / exceeds viewport ------

    /// Helper: assert a `ColumnWidths` decision respects the actual FTWA contract:
    /// - `sum == available` (G3), **except in the Â§3.6 fallback** where
    ///   `widths == min_content` and the caller is expected to enable
    ///   horizontal scrolling instead.
    /// - no width below min (never-break-token invariant)
    /// - `needs_horizontal_scroll` iff `available < sum_min` (the Â§3.6 condition)
    ///
    /// Note: the surplus regime *intentionally* grows columns beyond their
    /// max-content (spare is distributed proportionally), so no upper bound
    /// is asserted here â€” only the lower bound and the sum/scroll invariants.
    fn assert_decision_invariants(d: &ColumnWidths, _max: &[f32], min: &[f32], available: f32) {
        let sum: f32 = d.widths.iter().copied().sum();
        if !d.needs_horizontal_scroll {
            assert!(
                (sum - available).abs() < 1e-3,
                "Î£ widths ({sum}) must equal available ({available}); got {:?}",
                d.widths
            );
        }
        for (j, (&w, &mn)) in d.widths.iter().zip(min.iter()).enumerate() {
            assert!(w >= mn - 1e-3, "col {j}: width {w} below min {mn}");
        }
        let sum_min: f32 = min.iter().copied().sum();
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
        let d = ftwa(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // Surplus: spare split 1:1:1 â†’ 33.33 each, plus max 200 â†’ 233.33 each.
        for (j, &w) in d.widths.iter().enumerate() {
            assert!((w - 233.333).abs() < 0.5, "col {j}: {w} not ~233.33");
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
        let d = ftwa(&max, &min, available);
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
        let d = ftwa(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(d.needs_horizontal_scroll);
        // Â§3.6 returns min-content widths exactly â€” never break a token.
        assert_eq!(d.widths, vec![300.0, 300.0, 300.0]);
    }

    /// Dissimilar widths (narrow / wide / narrow) in a wide viewport.
    /// Wide column gets most of the spare, narrow columns get a fair share.
    #[test]
    fn permutation_dissimilar_columns_fit_viewport() {
        let max = [100.0, 500.0, 100.0];
        let min = [30.0, 200.0, 30.0];
        let available = 1000.0; // sum_max = 700, spare = 300
        let d = ftwa(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(!d.needs_horizontal_scroll);
        // Spare split proportional to max: col 0/2 get 100/700*300 = 42.86,
        // col 1 gets 500/700*300 = 214.29.
        assert!((d.widths[0] - 142.86).abs() < 0.5, "narrow col 0");
        assert!((d.widths[1] - 714.29).abs() < 0.5, "wide col 1");
        assert!((d.widths[2] - 142.86).abs() < 0.5, "narrow col 2");
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
        let d = ftwa(&max, &min, available);
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
        let d = ftwa(&max, &min, available);
        assert_decision_invariants(&d, &max, &min, available);
        assert!(d.needs_horizontal_scroll);
        // min-content widths returned exactly.
        assert_eq!(d.widths, vec![200.0, 500.0]);
    }
}
