//! Integration tests for the `markdown::table_width` algorithm
//! (FTWA — fair table-width algorithm).
//!
//! These exercise the **public** API only: `ftwa`,
//! `DeficitStrategy`, `ColumnWidths`. Placing them in `tests/`
//! (rather than a sibling sidecar) pins the algorithm to its
//! public surface — if a future refactor narrows or renames an
//! item, these tests catch it.
//!
//! Tests for the internal helpers (`cell_breakpoints`,
//! `greedy_line_count`, `next_breakpoint_below`,
//! `extra_lines_at_width`, `ftwa_wf`) live in the sibling
//! `internal_tests.rs` sidecar — they need access to private
//! items, which a top-level integration test cannot provide.
//!
//! Companion to the higher-level UI rendering tests in
//! `tests/table_layout_test.rs` and
//! `tests/table_visual_layout_test.rs` (which drive the full
//! FTWA → render pipeline through egui).

// Public API of the algorithm under test.
use fastmd::markdown::table_width::{ColumnWidths, DeficitStrategy, ftwa};

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
fn deficit_picks_minimum_cardinality_wrap_set() {
    // max = [100, 100], min = [10, 10], sum_max = 200, sum_min = 20.
    // available = 150, deficit = 50, per-column slack = 90.
    // With G2 (minimum-cardinality wrap set), one column's slack
    // (90) already covers the deficit (50), so the wrap set is a
    // single column. Tie-break picks the lower index, so col 0
    // absorbs all 50 and shrinks from 100 to 50; col 1 stays at
    // max 100. widths = [50, 100]. Only one column wraps.
    let max = [100.0, 100.0];
    let min = [10.0, 10.0];
    let d = ftwa_v1(&max, &min, 150.0);
    assert!(!d.needs_horizontal_scroll);
    // Sum exact.
    assert!(
        (d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3,
        "sum should equal available exactly; got {}",
        d.widths.iter().sum::<f32>()
    );
    // G2: the lower-index column is the wrap set (absorbs 50),
    // the higher-index column is pinned at max.
    assert!(
        (d.widths[0] - 50.0).abs() < 1e-3,
        "col 0 absorbs all deficit: got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 100.0).abs() < 1e-3,
        "col 1 pinned at max (G2): got {}",
        d.widths[1]
    );
    // No column below min.
    for (&w, &mn) in d.widths.iter().zip(min.iter()) {
        assert!(w >= mn - 1e-3, "w {} < min {}", w, mn);
    }
}

#[test]
fn deficit_two_columns_cover_third_pinned_at_max() {
    // Three columns: max = [50, 50, 50], min = [10, 10, 10], sum_max = 150.
    // available = 100, deficit = 50, per-column slack = 40.
    // With G2, one column's slack (40) does not cover the deficit
    // (50), so the greedy selection adds a second column (total
    // 80 >= 50). The third column's slack is unnecessary, so it
    // is pinned at max. Proportional distribution across {0, 1}:
    // each absorbs `50 * (40/80) = 25`, widths = [25, 25, 50].
    let max = [50.0, 50.0, 50.0];
    let min = [10.0, 10.0, 10.0];
    let d = ftwa_v1(&max, &min, 100.0);
    assert!(!d.needs_horizontal_scroll);
    // Two columns wrap, one pinned at max (G2 minimum-cardinality).
    assert!(
        (d.widths[0] - 25.0).abs() < 1e-3,
        "col 0: got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 25.0).abs() < 1e-3,
        "col 1: got {}",
        d.widths[1]
    );
    assert!(
        (d.widths[2] - 50.0).abs() < 1e-3,
        "col 2 pinned at max (G2): got {}",
        d.widths[2]
    );
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
    let d = ftwa_v1(&max, &min, 150.0);
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
    // slack [10, 70]. With G2, the higher-slack column (col 1,
    // slack 70) alone covers the deficit (60), so the wrap set is
    // {1}. col 0 stays at max 80 (its slack of 10 is not needed).
    // col 1 absorbs all 60 and shrinks from 80 to 20 (above min 10).
    // widths = [80, 20].
    let max = [80.0, 80.0];
    let min = [70.0, 10.0];
    let d = ftwa_v1(&max, &min, 100.0);
    assert!(!d.needs_horizontal_scroll);
    // G2: col 0 pinned at max; col 1 absorbs the deficit.
    assert!(
        (d.widths[0] - 80.0).abs() < 1e-3,
        "col 0 pinned at max (G2): got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 20.0).abs() < 1e-3,
        "col 1 absorbs deficit: got {}",
        d.widths[1]
    );
    // Both columns stay above their min.
    assert!(d.widths[0] >= min[0] - 1e-3);
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

/// Sub-pixel drift from egui's font shaping (kerning, sub-pixel
/// rounding) can make `layout_no_wrap("A" + "B")` differ from
/// `layout_no_wrap("A") + layout_no_wrap("B")` by a fraction of a
/// pixel even though the rendered cell *is* the sum of fragments.
/// The upstream `measure_cell` accumulates a token across
/// `InlineElem` boundaries when no whitespace separates them; the
/// `max` for that column is the sum of per-fragment widths, the
/// `min` is the layout of the merged string. The two can differ
/// by ~0.0625 px (= 1/16 px) on a real font. The function must
/// tolerate this sub-pixel drift instead of panicking.
///
/// Regression for the production panic
/// `max_content[0] = 322.6875 < min_content[0] = 322.75`.
#[test]
fn max_geq_min_subpixel_drift_does_not_panic() {
    let max = [322.6875_f32, 100.0];
    let min = [322.75_f32, 50.0]; // col 0: min > max by 0.0625 (sub-pixel)
    let d = ftwa_v1(&max, &min, 400.0);
    assert!(!d.needs_horizontal_scroll);
    // The drift was absorbed: col 0 ends up at its max content
    // width (no negative slack to give).
    assert_eq!(d.widths.len(), 2);
    // Sum equals available (G3 in deficit).
    assert!((d.widths.iter().sum::<f32>() - 400.0).abs() < 1e-3);
    // No column ever below max_content (deficit only shrinks, never
    // grows a column past max).
    for (j, (&w, &mx)) in d.widths.iter().zip(max.iter()).enumerate() {
        assert!(w <= mx + 1e-3, "col {j}: width {w} above max {mx}");
    }
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

    // NEG_INFINITY is also non-finite and must panic — it would
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
fn single_high_slack_column_covers_deficit() {
    // max = [60, 50, 40], min = [10, 10, 10]. sum_max = 150.
    // available = 110, deficit = 40. slacks = [50, 40, 30].
    // With G2, the highest-slack column (col 0, slack 50) alone
    // covers the deficit (40), so the wrap set is {0}. Cols 1
    // and 2 stay at max (50 and 40). col 0 absorbs all 40 and
    // shrinks from 60 to 20 (above min 10). widths = [20, 50, 40].
    let max = [60.0, 50.0, 40.0];
    let min = [10.0, 10.0, 10.0];
    let d = ftwa_v1(&max, &min, 110.0);
    assert!(!d.needs_horizontal_scroll);
    // G2: col 0 absorbs all deficit; cols 1, 2 pinned at max.
    assert!(
        (d.widths[0] - 20.0).abs() < 1e-3,
        "col 0: got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 50.0).abs() < 1e-3,
        "col 1 pinned at max (G2): got {}",
        d.widths[1]
    );
    assert!(
        (d.widths[2] - 40.0).abs() < 1e-3,
        "col 2 pinned at max (G2): got {}",
        d.widths[2]
    );
    // Sum exact.
    assert!((d.widths.iter().sum::<f32>() - 110.0).abs() < 1e-3);
    // No column below min.
    for (&w, &mn) in d.widths.iter().zip(min.iter()) {
        assert!(w >= mn - 1e-3, "w {} < min {}", w, mn);
    }
}

// --- Permutation matrix: similar vs dissimilar columns —"
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
/// With G2 (minimum-cardinality wrap set), each column's slack
/// (200) is less than the deficit (400), so the greedy selection
/// picks two columns: their combined slack (400) covers the deficit.
/// The third column is pinned at max (its slack is unnecessary).
/// Proportional distribution across the wrap set: each absorbs
/// `400 * (200 / 400) = 200`, so widths = [100, 100, 300].
#[test]
fn permutation_similar_columns_require_word_wrap() {
    let max = [300.0, 300.0, 300.0];
    let min = [100.0, 100.0, 100.0];
    let available = 500.0; // sum_max = 900, sum_min = 300, deficit = 400
    let d = ftwa_v1(&max, &min, available);
    assert_decision_invariants(&d, &max, &min, available);
    assert!(!d.needs_horizontal_scroll);
    // G2: two columns wrap, one pinned at max.
    assert!(
        (d.widths[0] - 100.0).abs() < 1e-3,
        "col 0: got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 100.0).abs() < 1e-3,
        "col 1: got {}",
        d.widths[1]
    );
    assert!(
        (d.widths[2] - 300.0).abs() < 1e-3,
        "col 2 pinned at max (G2): got {}",
        d.widths[2]
    );
    // Sum exact.
    assert!((d.widths.iter().sum::<f32>() - 500.0).abs() < 1e-3);
    // No column below min.
    for (&w, &mn) in d.widths.iter().zip(min.iter()) {
        assert!(w >= mn - 1e-3, "w {} < min {}", w, mn);
    }
}

/// 3 columns of similar width, viewport below sum_min → §3.6 fallback.
/// Even at min-content, the table cannot fit; render must use ScrollArea.
#[test]
fn permutation_similar_columns_exceed_viewport() {
    let max = [400.0, 400.0, 400.0];
    let min = [300.0, 300.0, 300.0];
    let available = 500.0; // sum_min = 900, way below
    let d = ftwa_v1(&max, &min, available);
    assert_decision_invariants(&d, &max, &min, available);
    assert!(d.needs_horizontal_scroll);
    // §3.6 returns min-content widths exactly — never break a token.
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

/// Dissimilar widths where the wide column's slack alone covers
/// the deficit. G2: only the wide column enters the wrap set;
/// the narrow column is pinned at its max_content.
#[test]
fn permutation_dissimilar_columns_require_word_wrap() {
    // With G2 (minimum-cardinality wrap set), the wide column's
    // slack (500) already covers the deficit (300), so the wrap
    // set is {1}. The narrow column is pinned at max 200 (its
    // slack of 150 is not needed). col 1 absorbs all 300 and
    // shrinks from 800 to 500 (above min 300). widths = [200, 500].
    let max = [200.0, 800.0];
    let min = [50.0, 300.0];
    let available = 700.0;
    let d = ftwa_v1(&max, &min, available);
    assert_decision_invariants(&d, &max, &min, available);
    assert!(!d.needs_horizontal_scroll);
    // G2: narrow col pinned at max; wide col absorbs the deficit.
    assert!(
        (d.widths[0] - 200.0).abs() < 1e-3,
        "narrow col pinned at max (G2): got {}",
        d.widths[0]
    );
    assert!(
        (d.widths[1] - 500.0).abs() < 1e-3,
        "wide col absorbs deficit: got {}",
        d.widths[1]
    );
    // No column below min.
    assert!(d.widths[0] >= 50.0, "narrow col must not break token");
    assert!(d.widths[1] >= 300.0, "wide col must not break token");
    // Sum exact.
    assert!((d.widths.iter().sum::<f32>() - 700.0).abs() < 1e-3);
}
/// Dissimilar widths where even the wide column's min-content alone
/// exceeds the available viewport → §3.6 fallback.
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
//  Bugs 1â€"3 were the original findings; the `audit_bug_*` tests now
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
    // Surplus pins at max_content exactly â€" no drift, no stretching.
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

/// OBS-2: The wrap-set construction excludes zero-slack columns
/// (they have nothing to give toward the deficit). This is
/// enforced both by the G2 minimum-cardinality selection (col 0
/// cannot be selected because its slack is 0) and by the surplus
/// branch (zero-slack columns are pinned at max either way).
#[test]
fn audit_observation_zero_slack_skipped_in_wrap_set() {
    // max=[50, 100, 50], min=[50, 10, 10]. Slack=[0, 90, 40].
    // deficit=50. With G2, the highest-slack column (col 1,
    // slack 90) alone covers the deficit (50), so the wrap set
    // is {1}. Col 0 stays at max 50 (zero slack, not selectable).
    // Col 2 stays at max 50 (its slack of 40 is not needed).
    // col 1 absorbs all 50 and shrinks from 100 to 50.
    // widths = [50, 50, 50].
    let d = ftwa_v1(&[50.0, 100.0, 50.0], &[50.0, 10.0, 10.0], 150.0);
    // col 0 unchanged (zero slack, not in wrap set).
    assert!((d.widths[0] - 50.0).abs() < 1e-3);
    // G2: col 1 absorbs all deficit, col 2 pinned at max.
    assert!(
        (d.widths[1] - 50.0).abs() < 1e-3,
        "col 1: got {}",
        d.widths[1]
    );
    assert!(
        (d.widths[2] - 50.0).abs() < 1e-3,
        "col 2 pinned at max (G2): got {}",
        d.widths[2]
    );
    // Sum exact.
    assert!((d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3);
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

/// Regression test for the 7-column laptop-spec bug caught by
/// `test_render_table_laptop_spec_short_cells_unwrap_long_wraps`.
/// One column has enough slack to absorb the entire deficit, so
/// G2 (minimum-cardinality wrap set) puts only that column in
/// the wrap set; every other column stays at `max_content` and
/// does not wrap. Without G2 (the "every positive-slack column
/// participates" behaviour that was in effect before the fix),
/// short cells like "XPS 15 9570" got shrunk below their
/// max_content width and were force-wrapped to two lines.
#[test]
fn audit_g2_one_big_slack_column_absorbs_entire_deficit() {
    // Six short/medium cells with small but positive slack, plus
    // one long "Summary" cell with slack large enough to cover
    // the entire deficit on its own.
    let max = [
        22.875_f32, 72.25, 176.5, 259.71875, 157.4375, 75.65625, 820.25,
    ];
    let min = [
        22.875_f32, 29.3125, 90.71875, 73.1875, 52.40625, 32.53125, 72.375,
    ];
    // 1000 px viewport minus 6 gutters of 10 px = 940, minus the
    // 16 px outer frame margin used by CentralPanel = 924.
    let avail = 924.0_f32;
    let d = ftwa_v1(&max, &min, avail);
    assert!(!d.needs_horizontal_scroll);
    // Sum exact (G3).
    assert!((d.widths.iter().sum::<f32>() - avail).abs() < 1e-3);
    // G2: the only column in the wrap set is the Summary column
    // (index 6, slack 747.88 >= deficit 660.69). Every other
    // column stays at max_content.
    for (j, (w, &m)) in d.widths.iter().zip(max.iter()).enumerate().take(6) {
        assert!(
            (w - m).abs() < 1e-3,
            "col {j} must be pinned at max {} (G2); got {}",
            m,
            w
        );
    }
    // Summary absorbs the full deficit and shrinks accordingly.
    let deficit = max.iter().sum::<f32>() - avail;
    assert!(
        (d.widths[6] - (max[6] - deficit)).abs() < 1e-3,
        "Summary col must absorb all deficit; got {} expected {}",
        d.widths[6],
        max[6] - deficit
    );
}

// -------------------------------------------------------------------
//  OBS-3, OBS-4: negative `available` / negative input widths panic.
//  Consolidated from 3 separate audit_observation_negative_*_panics
//  tests — they all assert the same panic-on-bad-input contract,
//  just with the bad value in a different position. The
//  `std::panic::catch_unwind` + `assert!(result.is_err())` body
//  was identical.
// -------------------------------------------------------------------

/// Pin the contract: feeding a negative `available`, a negative
/// `max_content[j]`, or a negative `min_content[j]` is a programmer
/// error and must panic. Each case below corresponds to a historical
/// audit item (OBS-3, OBS-4). Keeping them as named cases in one
/// test preserves the diagnostic on failure.
#[test]
fn audit_observation_negative_inputs_panic() {
    type NegativeInputCase<'a> = (&'a str, &'a [f32], &'a [f32], f32, &'a str);
    let cases: &[NegativeInputCase<'_>] = &[
        (
            "negative available",
            &[10.0, 20.0],
            &[5.0, 5.0],
            -1.0,
            "OBS-3 regression: negative available must panic",
        ),
        (
            "negative max_content",
            &[-5.0, 20.0],
            &[5.0, 5.0],
            50.0,
            "OBS-4 regression: negative max_content must panic",
        ),
        (
            "negative min_content",
            &[10.0, 20.0],
            &[-1.0, 5.0],
            50.0,
            "OBS-4 regression: negative min_content must panic",
        ),
    ];
    for (label, max, min, avail, msg) in cases {
        let result = std::panic::catch_unwind(|| ftwa_v1(max, min, *avail));
        assert!(result.is_err(), "{msg} (case: {label}, avail={avail})");
    }
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
    ///         G3 intentionally relaxed in surplus â€" see `ftwa` doc).
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
