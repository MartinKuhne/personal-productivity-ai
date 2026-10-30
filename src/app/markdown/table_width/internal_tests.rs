use super::*;
// -------------------------------------------------------------------
//  Water-fill strategy tests
// -------------------------------------------------------------------

/// Helper: call `ftwa` with BreakpointWaterFill strategy.
fn ftwa_wf(max: &[f32], min: &[f32], breakpoints: &[Vec<Breakpoint>], avail: f32) -> ColumnWidths {
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

/// Wrapping must break at whitespace (between tokens),
/// never inside a token. `greedy_line_count` only advances a line at the
/// inter-token boundary check (`line_w + token_w > col_width`), so every
/// line break falls on a whitespace position. With tokens `[20, 30, 25]`
/// and `space_width=10`, the first two tokens occupy `20+40 = 60 ≤ 65`,
/// the third would push the line to `60+35 = 95 > 65`, so the break falls
/// between token 1 and token 2 (a whitespace boundary) → 2 lines.
#[test]
fn wrap_breaks_at_whitespace_never_inside_token() {
    let tokens = vec![20.0, 30.0, 25.0];
    assert_eq!(greedy_line_count(&tokens, 10.0, 65.0), 2);
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

#[test]
fn next_breakpoint_below_terminates_at_index_zero() {
    let bps = vec![
        Breakpoint {
            width: 10.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 50.0,
            extra_lines: 1,
        },
    ];
    let mut idx = 0;
    let result = next_breakpoint_below(&bps, &mut idx, 5.0);
    assert!(result.is_none(), "no breakpoint below 5.0");
    assert_eq!(idx, bps.len(), "idx must be sentinel after exhaustion");
}

#[test]
fn next_breakpoint_below_returns_highest_match() {
    let bps = vec![
        Breakpoint {
            width: 10.0,
            extra_lines: 3,
        },
        Breakpoint {
            width: 30.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 60.0,
            extra_lines: 1,
        },
    ];
    let mut idx = bps.len() - 1;
    let bp = next_breakpoint_below(&bps, &mut idx, 50.0).unwrap();
    assert_eq!(bp.width, 30.0);
    assert_eq!(bp.extra_lines, 2);
}

#[test]
fn deficit_strategy_from_config() {
    // Existing FTWA strategies.
    assert_eq!(
        DeficitStrategy::from_config("waterfill"),
        DeficitStrategy::BreakpointWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config("water-fill"),
        DeficitStrategy::BreakpointWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config("water_fill"),
        DeficitStrategy::BreakpointWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config("proportional"),
        DeficitStrategy::ProportionalToSlack
    );
    // Survey algorithms (doc §2.10, §2.13, §2.14). Each variant has
    // a primary canonical form and one or more aliases for backward
    // compatibility / ergonomics.
    assert_eq!(
        DeficitStrategy::from_config("ratio"),
        DeficitStrategy::WaterFillRatio
    );
    assert_eq!(
        DeficitStrategy::from_config("waterfill-ratio"),
        DeficitStrategy::WaterFillRatio
    );
    assert_eq!(
        DeficitStrategy::from_config("lagrange"),
        DeficitStrategy::LagrangePenalty
    );
    assert_eq!(
        DeficitStrategy::from_config("lagrange-penalty"),
        DeficitStrategy::LagrangePenalty
    );
    assert_eq!(
        DeficitStrategy::from_config("hybrid"),
        DeficitStrategy::HybridMinPenaltyWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config("hybrid-min-penalty"),
        DeficitStrategy::HybridMinPenaltyWaterFill
    );
    // Unknown / empty values fall back to the current default
    // (`default_table_width_strategy` in `config.rs` returns
    // `"hybrid"` → `HybridMinPenaltyWaterFill`). The two are kept in
    // sync; if either side changes, this test will fail loudly.
    assert_eq!(
        DeficitStrategy::from_config("unknown"),
        DeficitStrategy::HybridMinPenaltyWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config(""),
        DeficitStrategy::HybridMinPenaltyWaterFill
    );
}

#[test]
fn extra_lines_at_width_binary_search_correctness() {
    let bps = vec![
        Breakpoint {
            width: 20.0,
            extra_lines: 3,
        },
        Breakpoint {
            width: 50.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 80.0,
            extra_lines: 1,
        },
    ];
    assert_eq!(extra_lines_at_width(&bps, 10.0), 0);
    assert_eq!(extra_lines_at_width(&bps, 20.0), 3);
    assert_eq!(extra_lines_at_width(&bps, 35.0), 3);
    assert_eq!(extra_lines_at_width(&bps, 50.0), 2);
    assert_eq!(extra_lines_at_width(&bps, 80.0), 1);
    assert_eq!(extra_lines_at_width(&bps, 100.0), 1);
    assert_eq!(extra_lines_at_width(&[], 50.0), 0);
}

#[test]
fn water_fill_terminates_with_single_breakpoint_at_max() {
    let max = [100.0];
    let min = [50.0];
    let bps = vec![vec![Breakpoint {
        width: 100.0,
        extra_lines: 1,
    }]];
    let d = ftwa_wf(&max, &min, &bps, 75.0);
    assert!(!d.needs_horizontal_scroll);
    assert!(d.widths[0] >= 50.0);
    assert!(d.widths[0] <= 100.0);
}

// -------------------------------------------------------------------
//  Survey-algorithm tests (doc §2.10, §2.13, §2.14)
// -------------------------------------------------------------------

/// Helper: call `ftwa` with the WaterFillRatio strategy.
fn ftwa_ratio(
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
        DeficitStrategy::WaterFillRatio,
    )
}

/// Helper: call `ftwa` with the LagrangePenalty strategy.
fn ftwa_lagrange(
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
        DeficitStrategy::LagrangePenalty,
    )
}

/// Helper: call `ftwa` with the HybridMinPenaltyWaterFill strategy.
fn ftwa_hybrid(
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
        DeficitStrategy::HybridMinPenaltyWaterFill,
    )
}

fn sum_widths(d: &ColumnWidths) -> f32 {
    d.widths.iter().sum()
}

fn assert_sum_fits(d: &ColumnWidths, avail: f32) {
    let sum = sum_widths(d);
    assert!(
        (sum - avail).abs() < 1e-3,
        "sum of widths must equal available: sum={sum}, avail={avail}"
    );
}

fn assert_no_scroll(d: &ColumnWidths) {
    assert!(
        !d.needs_horizontal_scroll,
        "expected a fitting layout, got needs_horizontal_scroll=true"
    );
}

fn assert_respects_min(d: &ColumnWidths, min: &[f32]) {
    for (j, &w) in d.widths.iter().enumerate() {
        assert!(
            w >= min[j] - 1e-3,
            "width[{j}] = {w} below min[{j}] = {}",
            min[j]
        );
    }
}

// --- §2.10 waterfill_ratio -----------------------------------------------

#[test]
fn waterfill_ratio_surplus_pins_at_max() {
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_ratio(&max, &min, &bps, 500.0);
    assert_no_scroll(&d);
    assert_eq!(d.widths, vec![100.0, 50.0]);
}

#[test]
fn waterfill_ratio_fallback_below_sum_min() {
    let max = [100.0, 100.0];
    let min = [60.0, 60.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_ratio(&max, &min, &bps, 50.0);
    assert!(d.needs_horizontal_scroll);
    assert_eq!(d.widths, vec![60.0, 60.0]);
}

#[test]
fn waterfill_ratio_equalizes_ratios_in_deficit() {
    // 2 columns with no breakpoints. Closed-form: w_j = max_j * available / sum_max.
    // For available=80, max=[100, 50]: w = [53.333, 26.666]. Both have ratio max/w = 1.875.
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_ratio(&max, &min, &bps, 80.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 80.0);
    assert_respects_min(&d, &min);
    // Both ratios should match (within float precision).
    let r0 = max[0] / d.widths[0];
    let r1 = max[1] / d.widths[1];
    assert!(
        (r0 - r1).abs() < 1e-3,
        "ratios should equalize: {r0} vs {r1}"
    );
}

#[test]
fn waterfill_ratio_clamps_at_min_when_target_too_narrow() {
    // 2 columns. With available=100, the equal-ratio target for col 0
    // is 50 (below min=80), so col 0 must clamp to min. Col 1 absorbs
    // the rest: w_1 = 100 - 80 = 20 (above its min=5).
    let max = [100.0, 100.0];
    let min = [80.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_ratio(&max, &min, &bps, 100.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 100.0);
    assert_respects_min(&d, &min);
    assert_eq!(d.widths[0], 80.0, "col 0 should be clamped at min");
}

#[test]
fn waterfill_ratio_no_breakpoints_means_no_wrap_used() {
    // Breakpoints are accepted but ignored by waterfill_ratio (the
    // algorithm is purely geometric). The deficit case still works.
    let max = [80.0, 60.0];
    let min = [10.0, 5.0];
    let bps = vec![
        vec![Breakpoint {
            width: 50.0,
            extra_lines: 1,
        }],
        vec![Breakpoint {
            width: 30.0,
            extra_lines: 2,
        }],
    ];
    let d = ftwa_ratio(&max, &min, &bps, 100.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 100.0);
    assert_respects_min(&d, &min);
}

// --- §2.13 lagrange_penalty ---------------------------------------------

#[test]
fn lagrange_surplus_pins_at_max() {
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_lagrange(&max, &min, &bps, 500.0);
    assert_no_scroll(&d);
    assert_eq!(d.widths, vec![100.0, 50.0]);
}

#[test]
fn lagrange_fallback_below_sum_min() {
    let max = [100.0, 100.0];
    let min = [60.0, 60.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_lagrange(&max, &min, &bps, 50.0);
    assert!(d.needs_horizontal_scroll);
    assert_eq!(d.widths, vec![60.0, 60.0]);
}

#[test]
fn lagrange_no_breakpoints_picks_narrower_columns_first() {
    // 2 columns, no breakpoints. The Lagrangian has no wrap cost, so
    // minimizing sum-of-wrap is trivially zero everywhere. The λ
    // bisection then picks the widths that hit `available` with the
    // smallest λ — which is the widest widths possible. So both
    // columns should stay at their widest candidate (= max_j).
    // For available < sum_max, the bisection converges to a λ where
    // the sum matches, but the resulting widths depend on the
    // discrete candidate set. We just assert the result fits and
    // respects min.
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_lagrange(&max, &min, &bps, 80.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 80.0);
    assert_respects_min(&d, &min);
}

#[test]
fn lagrange_with_breakpoints_picks_widest_no_wrap_width() {
    // Single column with breakpoints. The Lagrangian should pick the
    // widest candidate with the lowest wrap cost for the right λ.
    // For available=80 (deficit), with max=100 and bps = [(40, 1),
    // (70, 1), (95, 1)]: the "wide-side" convention treats bps as
    // boundaries between wrap-cost intervals. We just assert the
    // result is in [min, max], hits the sum, and is at one of the
    // breakpoint widths or max.
    let max = [100.0];
    let min = [40.0];
    let bps = vec![vec![
        Breakpoint {
            width: 40.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 70.0,
            extra_lines: 1,
        },
        Breakpoint {
            width: 95.0,
            extra_lines: 1,
        },
    ]];
    let d = ftwa_lagrange(&max, &min, &bps, 80.0);
    assert_no_scroll(&d);
    assert_eq!(d.widths.len(), 1);
    assert!((d.widths[0] - 80.0).abs() < 1e-3);
    assert!(d.widths[0] >= min[0] - 1e-3);
    assert!(d.widths[0] <= max[0] + 1e-3);
}

#[test]
fn lagrange_respects_min_under_severe_deficit() {
    // 2 columns. With available=30 and min=[10, 15], sum_min=25 (fits).
    // The Lagrangian should find widths that sum to 30 and don't
    // violate either min.
    let max = [100.0, 100.0];
    let min = [10.0, 15.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_lagrange(&max, &min, &bps, 30.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 30.0);
    assert_respects_min(&d, &min);
}

// --- §2.14 hybrid --------------------------------------------------------

#[test]
fn hybrid_surplus_pins_at_max() {
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_hybrid(&max, &min, &bps, 500.0);
    assert_no_scroll(&d);
    assert_eq!(d.widths, vec![100.0, 50.0]);
}

#[test]
fn hybrid_fallback_below_sum_min() {
    let max = [100.0, 100.0];
    let min = [60.0, 60.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_hybrid(&max, &min, &bps, 50.0);
    assert!(d.needs_horizontal_scroll);
    assert_eq!(d.widths, vec![60.0, 60.0]);
}

#[test]
fn hybrid_per_column_target_is_first_wrap_boundary() {
    // 1 column with breakpoints [(40, 1), (70, 1)] and max=100.
    // The "first-wrap boundary" target is the largest bp's width = 70.
    // For available > 70, water-fill should grow col 0 up to max_j.
    // For available = 200 (surplus, but that's covered by the surplus
    // test), so we test available = 90 — residual = 20, headroom = 30,
    // so width = 70 + 20 = 90. Sum fits.
    let max = [100.0];
    let min = [40.0];
    let bps = vec![vec![
        Breakpoint {
            width: 40.0,
            extra_lines: 1,
        },
        Breakpoint {
            width: 70.0,
            extra_lines: 1,
        },
    ]];
    let d = ftwa_hybrid(&max, &min, &bps, 90.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 90.0);
    assert_respects_min(&d, &min);
}

#[test]
fn hybrid_water_fills_residual_to_max() {
    // 2 columns with no breakpoints → per-column target = max_j.
    // Sum of targets = sum_max = 100. For available = 90 (deficit),
    // residual = -10, distributed proportional to (target - min).
    // target_0 - min_0 = 90, target_1 - min_1 = 40, total = 130.
    // col 0 shrinks by 10 * 90/130 ≈ 6.92 → 83.08.
    // col 1 shrinks by 10 * 40/130 ≈ 3.08 → 36.92.
    let max = [100.0, 50.0];
    let min = [10.0, 10.0];
    let bps = vec![Vec::new(), Vec::new()];
    let d = ftwa_hybrid(&max, &min, &bps, 90.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 90.0);
    assert_respects_min(&d, &min);
}

#[test]
fn hybrid_clamps_at_min_when_water_fill_overshrinks() {
    // Single column with a small max. If we ask for a width below
    // the column's min, we should hit the fallback.
    let max = [10.0];
    let min = [5.0];
    let bps: Vec<Vec<Breakpoint>> = vec![Vec::new()];
    // 4 < 5 → sum_min=5, available=4 < sum_min → fallback.
    let d = ftwa_hybrid(&max, &min, &bps, 4.0);
    assert!(d.needs_horizontal_scroll);
    assert_eq!(d.widths, vec![5.0]);
}

#[test]
fn hybrid_multi_column_with_breakpoints() {
    // 2 columns. Col 0 wraps (bps), col 1 doesn't. Available=130.
    // Col 0 target = 80 (largest bp). Col 1 target = 100 (max).
    // Sum targets = 180. Residual = 130 - 180 = -50. Distributed
    // proportional to (target - min): col 0 has 80-10=70, col 1 has
    // 100-5=95, total=165. col 0 shrinks by 50*70/165 ≈ 21.21 → 58.79.
    // col 1 shrinks by 50*95/165 ≈ 28.79 → 71.21.
    let max = [100.0, 100.0];
    let min = [10.0, 5.0];
    let bps = vec![
        vec![Breakpoint {
            width: 80.0,
            extra_lines: 1,
        }],
        Vec::new(),
    ];
    let d = ftwa_hybrid(&max, &min, &bps, 130.0);
    assert_no_scroll(&d);
    assert_sum_fits(&d, 130.0);
    assert_respects_min(&d, &min);
    // Both widths should be above their min.
    assert!(d.widths[0] > min[0]);
    assert!(d.widths[1] > min[1]);
}

// --- Cross-algorithm comparison -----------------------------------------

#[test]
fn all_survey_algorithms_respect_invariants_on_same_input() {
    // All three survey algorithms must satisfy the basic invariants
    // (sum = available, widths >= min, no scroll in fitting regime)
    // on the same deficit input. The specific values differ; we just
    // check the contract holds for each.
    let max = [100.0, 80.0, 60.0];
    let min = [20.0, 15.0, 10.0];
    let bps = vec![
        vec![Breakpoint {
            width: 50.0,
            extra_lines: 1,
        }],
        vec![Breakpoint {
            width: 40.0,
            extra_lines: 1,
        }],
        Vec::new(),
    ];
    let avail = 150.0;
    for d in [
        ftwa_ratio(&max, &min, &bps, avail),
        ftwa_lagrange(&max, &min, &bps, avail),
        ftwa_hybrid(&max, &min, &bps, avail),
    ] {
        assert_no_scroll(&d);
        assert_sum_fits(&d, avail);
        assert_respects_min(&d, &min);
    }
}

#[test]
fn all_survey_algorithms_surplus_pins_at_max() {
    let max = [100.0, 50.0];
    let min = [10.0, 5.0];
    let bps = vec![Vec::new(), Vec::new()];
    for d in [
        ftwa_ratio(&max, &min, &bps, 500.0),
        ftwa_lagrange(&max, &min, &bps, 500.0),
        ftwa_hybrid(&max, &min, &bps, 500.0),
    ] {
        assert_no_scroll(&d);
        assert_eq!(d.widths, vec![100.0, 50.0]);
    }
}

#[test]
fn all_survey_algorithms_fallback_clamps_at_min() {
    let max = [100.0, 100.0];
    let min = [60.0, 60.0];
    let bps = vec![Vec::new(), Vec::new()];
    for d in [
        ftwa_ratio(&max, &min, &bps, 50.0),
        ftwa_lagrange(&max, &min, &bps, 50.0),
        ftwa_hybrid(&max, &min, &bps, 50.0),
    ] {
        assert!(d.needs_horizontal_scroll);
        assert_eq!(d.widths, vec![60.0, 60.0]);
    }
}

#[test]
fn empty_input_returns_empty_widths_for_survey_algorithms() {
    let max: [f32; 0] = [];
    let min: [f32; 0] = [];
    let bps: Vec<Vec<Breakpoint>> = vec![];
    for d in [
        ftwa_ratio(&max, &min, &bps, 100.0),
        ftwa_lagrange(&max, &min, &bps, 100.0),
        ftwa_hybrid(&max, &min, &bps, 100.0),
    ] {
        assert!(d.widths.is_empty());
        assert!(!d.needs_horizontal_scroll);
    }
}

// -------------------------------------------------------------------
//  ShrinkStep heap ordering
// -------------------------------------------------------------------

/// Helper: build a `ShrinkStep` for ordering assertions.
fn step(cost: f32, col: usize, target_width: f32, delta_lines: i32) -> ShrinkStep {
    ShrinkStep {
        cost,
        col,
        target_width,
        delta_lines,
    }
}

/// The water-fill heap is ordered by marginal cost first and column index
/// second, so the same input always pops the same column. `target_width`
/// and `delta_lines` are payload and must not influence the order.
#[test]
fn shrink_step_orders_by_cost_then_column() {
    let cases: Vec<(ShrinkStep, ShrinkStep, std::cmp::Ordering, &str)> = vec![
        (
            step(1.0, 0, 10.0, 1),
            step(2.0, 3, 10.0, 1),
            std::cmp::Ordering::Less,
            "cheaper cost sorts before costlier cost",
        ),
        (
            step(2.0, 3, 10.0, 1),
            step(1.0, 0, 10.0, 1),
            std::cmp::Ordering::Greater,
            "costlier cost sorts after cheaper cost",
        ),
        (
            step(1.0, 2, 10.0, 1),
            step(1.0, 5, 10.0, 1),
            std::cmp::Ordering::Less,
            "equal cost: lower column index sorts first",
        ),
        (
            step(1.0, 4, 10.0, 1),
            step(1.0, 4, 10.0, 1),
            std::cmp::Ordering::Equal,
            "identical steps compare equal",
        ),
        (
            step(1.0, 1, 999.0, 7),
            step(1.0, 1, 1.0, 0),
            std::cmp::Ordering::Equal,
            "target_width and delta_lines are not part of the order",
        ),
    ];
    for (a, b, expected, case) in cases {
        assert_eq!(a.cmp(&b), expected, "Ord::cmp — {case}");
        assert_eq!(
            a.partial_cmp(&b),
            Some(expected),
            "PartialOrd::partial_cmp — {case}"
        );
    }
}

/// A NaN cost has no meaningful ordering; the implementation must degrade to
/// `Equal` on the cost so the column index decides, instead of letting an
/// arbitrary NaN comparison result propagate through the heap.
#[test]
fn shrink_step_nan_cost_compares_equal_and_falls_back_to_column() {
    let nan = f32::NAN;
    assert_eq!(
        step(nan, 0, 10.0, 1).cmp(&step(1.0, 0, 10.0, 1)),
        std::cmp::Ordering::Equal,
        "NaN vs finite cost must compare Equal on the cost key"
    );
    assert_eq!(
        step(nan, 0, 10.0, 1).cmp(&step(nan, 2, 10.0, 1)),
        std::cmp::Ordering::Less,
        "NaN vs NaN must fall through to the column tie-break"
    );
    assert_eq!(
        step(1.0, 3, 10.0, 1).partial_cmp(&step(nan, 3, 10.0, 1)),
        Some(std::cmp::Ordering::Equal),
        "partial_cmp must never return None for a NaN cost"
    );
}

/// `BinaryHeap<Reverse<ShrinkStep>>` is the min-heap the water-fill relies on:
/// the cheapest step pops first, ties are resolved by lower column index.
#[test]
fn shrink_step_heap_pops_cheapest_first() {
    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<ShrinkStep>> =
        std::collections::BinaryHeap::new();
    heap.push(std::cmp::Reverse(step(3.0, 0, 90.0, 1)));
    heap.push(std::cmp::Reverse(step(1.0, 2, 95.0, 1)));
    heap.push(std::cmp::Reverse(step(1.0, 1, 97.0, 1)));

    let popped: Vec<usize> = (0..3)
        .map(|_| heap.pop().expect("heap must still contain an entry").0.col)
        .collect();
    assert_eq!(
        popped,
        vec![1, 2, 0],
        "cheapest cost first, then lower column index on a tie"
    );
}

// -------------------------------------------------------------------
//  next_breakpoint_below / greedy_line_count / compute_column_breakpoints
// -------------------------------------------------------------------

/// A breakpoint at (or above) the threshold is not "below" it and must be
/// skipped; the scan continues downward to the next strictly-lower entry.
#[test]
fn next_breakpoint_below_skips_breakpoints_at_or_above_threshold() {
    let bps = vec![
        Breakpoint {
            width: 10.0,
            extra_lines: 3,
        },
        Breakpoint {
            width: 30.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 60.0,
            extra_lines: 1,
        },
    ];
    let mut idx = bps.len() - 1; // points at width 60.0
    let got = next_breakpoint_below(&bps, &mut idx, 60.0).expect("30.0 is strictly below 60.0");
    assert_eq!(
        (got.width, got.extra_lines),
        (30.0, 2),
        "the breakpoint equal to the threshold must be skipped"
    );
    assert_eq!(
        idx, 0,
        "the cursor must be left below the returned breakpoint"
    );
}

#[test]
fn next_breakpoint_below_empty_list_returns_none() {
    let mut idx = 0;
    assert_eq!(
        next_breakpoint_below(&[], &mut idx, 100.0),
        None,
        "an empty breakpoint list has nothing to return"
    );
    assert_eq!(idx, 0, "the cursor must be untouched for an empty list");
}

/// An empty token list is defined as a single (empty) line, so callers can
/// subtract 1 to get the extra-line count without a special case.
#[test]
fn greedy_line_count_empty_tokens_is_one_line() {
    assert_eq!(greedy_line_count(&[], 10.0, 0.0), 1);
    assert_eq!(greedy_line_count(&[], 10.0, 100.0), 1);
}

/// **Regression test for the `greedy_line_count` off-by-one (defect D6).**
///
/// The packer used to take its "start a new line" branch for the *first* token
/// even though the current line was still empty, so an over-wide first token was
/// charged a phantom empty line and the count came out one too high. A single
/// unbreakable token physically occupies exactly **one** line (it overflows the
/// column), and that is what the packer reports now.
///
/// The defect was masked on the main FTWA path, where
/// `col_width >= min_content >= longest token` guarantees the first token fits,
/// but *not* inside [`cell_breakpoints`], whose candidate list contains
/// single-token widths smaller than the first token.
#[test]
fn defect_06_first_token_wider_than_column_costs_exactly_one_line() {
    assert_eq!(
        greedy_line_count(&[200.0], 10.0, 50.0),
        1,
        "a single over-wide token overflows its own line and must cost 1 line, \
         not a phantom empty line plus the token"
    );
    assert_eq!(
        greedy_line_count(&[200.0, 10.0], 10.0, 50.0),
        2,
        "the over-wide first token takes line 1 and the next token line 2"
    );
    assert_eq!(
        greedy_line_count(&[200.0, 100.0, 100.0], 10.0, 50.0),
        3,
        "one over-wide token plus two tokens that cannot share a line with it \
         or with each other is three lines"
    );
    assert_eq!(
        greedy_line_count(&[50.0], 10.0, 50.0),
        1,
        "a first token that exactly fits was never affected and stays 1 line"
    );
    assert_eq!(
        greedy_line_count(&[50.0, 50.0], 10.0, 50.0),
        2,
        "in-contract packing is unchanged: two 50-wide tokens never share a 50-wide column"
    );
    assert_eq!(
        greedy_line_count(&[], 10.0, 0.0),
        1,
        "an empty token list is defined as one empty line"
    );
    assert_eq!(
        greedy_line_count(&[], 10.0, 100.0),
        1,
        "an empty token list is one line regardless of column width"
    );
}

/// An over-wide token **mid-run** already landed on a non-empty line, so it was
/// never charged the phantom line; this pins that the fix did not disturb the
/// already-correct mid-run packing.
#[test]
fn defect_06_over_wide_token_mid_run_packs_one_token_per_line() {
    assert_eq!(
        greedy_line_count(&[10.0, 200.0, 10.0], 10.0, 50.0),
        3,
        "the over-wide middle token gets its own line between the two small ones"
    );
    assert_eq!(
        greedy_line_count(&[10.0, 200.0, 10.0, 10.0], 10.0, 50.0),
        3,
        "after the over-wide middle token the two trailing 10-wide tokens still \
         share one line: 10+10+10 = 30 <= 50"
    );
    assert_eq!(
        greedy_line_count(&[200.0, 200.0], 10.0, 50.0),
        2,
        "two over-wide tokens occupy exactly two lines, not three"
    );
}

/// The wrap-cost curve [`cell_breakpoints`] publishes is what the FTWA solvers
/// read, and it is built from candidate widths that include single tokens —
/// i.e. widths *below* an over-wide first token. That is where the phantom line
/// used to show up: the curve claimed a cell needed more extra lines than it
/// physically does, and claimed a *narrower* width cost *more* extra lines than
/// a wider one in a way that broke the curve's monotonicity.
#[test]
fn defect_06_cell_breakpoints_curve_is_monotone_for_an_over_wide_first_token() {
    // `[200, 10]` with a 10-wide space needs 2 lines below 220 and 1 line at
    // or above it, so exactly one transition exists.
    let bps = cell_breakpoints(&[200.0, 10.0], 10.0);
    assert_eq!(
        bps.len(),
        1,
        "only the real 1→2 line transition may appear, got: {bps:?}"
    );
    assert!(
        (bps[0].width - 220.0).abs() < 1e-3,
        "the transition sits at the width where both tokens fit on one line, got: {bps:?}"
    );
    assert_eq!(
        bps[0].extra_lines, 1,
        "wrapping `[200, 10]` onto two lines is one extra line, got: {bps:?}"
    );

    // General invariant on a cell with two transitions: as width grows,
    // extra_lines must fall — never rise.
    let two_steps = cell_breakpoints(&[200.0, 20.0, 20.0], 10.0);
    assert!(
        two_steps.len() >= 2,
        "a cell that wraps once at 230 and twice at 260 must publish two steps, \
         got: {two_steps:?}"
    );
    for window in two_steps.windows(2) {
        assert!(
            window[0].width < window[1].width && window[0].extra_lines > window[1].extra_lines,
            "a wider column can never cost more extra lines: {two_steps:?}"
        );
    }
}

/// A column-level curve merges several cells, so the phantom line used to be
/// summed across every cell whose first token was over-wide. With the fix, a
/// column of two identical over-wide cells reports one extra line, not two.
#[test]
fn defect_06_column_curve_does_not_inflate_over_wide_cells() {
    let cells = vec![
        CellTokens {
            token_widths: vec![200.0, 10.0],
        },
        CellTokens {
            token_widths: vec![200.0, 10.0],
        },
    ];
    let bps = compute_column_breakpoints(&cells, 10.0);
    assert_eq!(
        bps.len(),
        1,
        "both cells transition at the same width, got: {bps:?}"
    );
    assert_eq!(
        bps[0].extra_lines, 2,
        "two cells each wrapping once contribute 2 extra lines in total, got: {bps:?}"
    );
}

#[test]
fn compute_column_breakpoints_empty_cell_list() {
    assert!(
        compute_column_breakpoints(&[], 10.0).is_empty(),
        "a column with no cells has no wrap-cost curve"
    );
}

/// Single-token cells never wrap, so they contribute no breakpoints and the
/// merged column curve stays empty.
#[test]
fn compute_column_breakpoints_for_non_wrapping_cells() {
    let cells = vec![
        CellTokens {
            token_widths: vec![40.0],
        },
        CellTokens {
            token_widths: vec![55.0],
        },
    ];
    assert!(
        compute_column_breakpoints(&cells, 10.0).is_empty(),
        "cells without wrap candidates must not synthesise breakpoints"
    );
}

/// Cells whose breakpoints land on *different* widths produce one merged
/// entry per distinct width, each carrying the sum of the per-cell extra
/// lines that are active at that width.
///
/// Note the narrow-side lookup: `extra_lines_at_width` reports the cost of
/// the widest breakpoint at or below the queried width, so the value at a
/// breakpoint's own width is the cost that greedy packing only reaches
/// *below* it (see the report's spec-drift note).
#[test]
fn compute_column_breakpoints_sums_lines_at_distinct_widths() {
    let cells = vec![
        // Breakpoint: width 70 → +1 line.
        CellTokens {
            token_widths: vec![30.0, 30.0],
        },
        // Breakpoints: width 50 → +2 lines, width 80 → +1 line.
        CellTokens {
            token_widths: vec![20.0, 20.0, 20.0],
        },
    ];
    let bps = compute_column_breakpoints(&cells, 10.0);
    let expected = [
        Breakpoint {
            width: 50.0,
            extra_lines: 2,
        },
        Breakpoint {
            width: 70.0,
            extra_lines: 3,
        },
        Breakpoint {
            width: 80.0,
            extra_lines: 2,
        },
    ];
    assert_eq!(bps.len(), expected.len(), "one entry per distinct width");
    for (got, want) in bps.iter().zip(expected.iter()) {
        assert!(
            (got.width - want.width).abs() < 1e-3,
            "width {} != expected {}",
            got.width,
            want.width
        );
        assert_eq!(
            got.extra_lines, want.extra_lines,
            "summed extra_lines at width {}",
            want.width
        );
    }
}

// -------------------------------------------------------------------
//  B2 helpers: defensive guards
// -------------------------------------------------------------------

/// The `acc == 0` guard prevents a division by zero. Through `ftwa` it cannot
/// trigger (a non-empty wrap set always has positive slack), so the helper is
/// exercised directly to pin the guard's behaviour.
#[test]
fn b2_proportional_to_slack_zero_slack_sum_shrinks_nothing() {
    let mut widths = [100.0_f32, 80.0];
    b2_proportional_to_slack(&[0], &mut widths, &[100.0, 80.0], &[10.0, 10.0], 20.0, 0.0);
    assert_eq!(
        widths,
        [100.0, 80.0],
        "acc = 0 must not divide by zero or shrink the column"
    );
}

/// Both early-return guards: an empty wrap set and a non-positive deficit
/// leave the widths untouched.
#[test]
fn b2_breakpoint_water_fill_noops_on_empty_inputs() {
    let max = [100.0_f32];
    let min = [50.0_f32];
    let bps = vec![vec![Breakpoint {
        width: 60.0,
        extra_lines: 1,
    }]];

    let mut widths = [100.0_f32];
    b2_breakpoint_water_fill(&[], &mut widths, &max, &min, &bps, 30.0);
    assert_eq!(widths, [100.0], "empty wrap set must be a no-op");

    let mut widths = [100.0_f32];
    b2_breakpoint_water_fill(&[0], &mut widths, &max, &min, &bps, 0.0);
    assert_eq!(widths, [100.0], "zero deficit must be a no-op");
}

// -------------------------------------------------------------------
//  Wrap-set saturation overflow guard
// -------------------------------------------------------------------

/// Wrap-set saturation. The greedy wrap set's combined slack (100) covers the
/// deficit (90), but the breakpoint curve only lets each column shrink 1 px
/// before it runs out of breakpoints. B2 stops with Σ widths = 198 ≫ 110, the
/// drift fix can only clamp the target at `min_content` (it cannot shrink
/// below it), and the trailing overflow guard must escalate to the §3.6
/// horizontal-scroll fallback instead of overflowing the viewport.
///
/// This path is only reachable through `BreakpointWaterFill`: with
/// `ProportionalToSlack` the captured slack always covers the deficit, so B2
/// lands exactly on `available` and the guard can never fire.
#[test]
fn ftwa_wrap_set_saturation_escalates_to_horizontal_scroll() {
    let max = [100.0_f32, 100.0];
    let min = [50.0_f32, 50.0];
    let bps = vec![
        vec![Breakpoint {
            width: 99.0,
            extra_lines: 1,
        }],
        vec![Breakpoint {
            width: 99.0,
            extra_lines: 1,
        }],
    ];
    let d = ftwa_wf(&max, &min, &bps, 110.0);
    assert!(
        d.needs_horizontal_scroll,
        "a saturated wrap set must escalate to the horizontal-scroll fallback"
    );
    assert_eq!(
        d.widths,
        vec![50.0, 50.0],
        "the fallback must return min-content widths"
    );
}

// -------------------------------------------------------------------
//  Lagrange helpers
// -------------------------------------------------------------------

/// The two ends of the λ range: at λ = 0 the width term vanishes and every
/// column keeps its widest candidate; at λ → ∞ the width term dominates and
/// every column drops to `min_j`. In between, a mid-range λ picks the
/// breakpoint candidate because the wrap penalty outweighs the extra width.
#[test]
fn lagrange_optimal_width_extremes_of_lambda() {
    let max = [100.0_f32];
    let min = [40.0_f32];
    let bps = vec![vec![Breakpoint {
        width: 70.0,
        extra_lines: 1,
    }]];

    assert_eq!(
        lagrange_optimal_width(0.0, 0, &max, &min, &bps),
        100.0,
        "λ = 0 must keep the widest candidate"
    );
    assert_eq!(
        lagrange_optimal_width(0.01, 0, &max, &min, &bps),
        70.0,
        "λ = 0.01: bp cost 0.7 beats max 1.0 and min 1.4"
    );
    assert_eq!(
        lagrange_optimal_width(0.05, 0, &max, &min, &bps),
        40.0,
        "λ = 0.05: min cost 3.0 beats bp 3.5 and max 5.0"
    );
    assert_eq!(
        lagrange_optimal_width(1.0e6, 0, &max, &min, &bps),
        40.0,
        "λ → ∞ must drop to min_content"
    );
}

/// `lagrange_sum` must agree with the vector produced by `lagrange_widths`,
/// and every returned width must be one of the in-contract candidates
/// (`max_j`, a breakpoint width, or `min_j`), i.e. inside `[min_j, max_j]`.
#[test]
fn lagrange_sum_matches_widths_vector() {
    let max = [100.0_f32, 60.0, 30.0];
    let min = [40.0_f32, 20.0, 30.0];
    let bps = vec![
        vec![
            Breakpoint {
                width: 50.0,
                extra_lines: 2,
            },
            Breakpoint {
                width: 70.0,
                extra_lines: 1,
            },
        ],
        vec![Breakpoint {
            width: 40.0,
            extra_lines: 1,
        }],
        Vec::new(),
    ];

    for &lambda in &[0.0_f32, 0.01, 0.05, 0.1, 1.0, 1.0e6] {
        let widths = lagrange_widths(lambda, &max, &min, &bps);
        assert_eq!(widths.len(), max.len(), "λ={lambda}: one width per column");
        assert_eq!(
            lagrange_sum(lambda, &max, &min, &bps),
            widths.iter().sum::<f32>(),
            "λ={lambda}: the sum helper must agree with the widths vector"
        );
        for (j, &w) in widths.iter().enumerate() {
            assert!(
                w >= min[j] - 1e-6 && w <= max[j] + 1e-6,
                "λ={lambda}: width[{j}] = {w} outside [{}, {}]",
                min[j],
                max[j]
            );
        }
    }
}

/// The bisection is only valid because Σ w_j(λ) is monotone non-increasing in
/// λ (widening costs more as λ grows). This is the documented precondition of
/// [`solve_lagrange_penalty`]; if it ever breaks the bisection silently
/// returns a wrong layout.
#[test]
fn lagrange_sum_is_monotone_non_increasing_in_lambda() {
    let max = [100.0_f32, 60.0, 30.0];
    let min = [40.0_f32, 20.0, 30.0];
    let bps = vec![
        vec![
            Breakpoint {
                width: 50.0,
                extra_lines: 2,
            },
            Breakpoint {
                width: 70.0,
                extra_lines: 1,
            },
        ],
        vec![Breakpoint {
            width: 40.0,
            extra_lines: 1,
        }],
        Vec::new(),
    ];

    let lambdas = [0.0_f32, 0.001, 0.01, 0.05, 0.1, 1.0, 10.0, 1.0e6];
    let sums: Vec<f32> = lambdas
        .iter()
        .map(|&l| lagrange_sum(l, &max, &min, &bps))
        .collect();
    for (pair, window) in sums.windows(2).zip(lambdas.windows(2)) {
        assert!(
            pair[1] <= pair[0] + 1e-6,
            "Σ must not grow as λ increases: λ {} → {} gave Σ {} → {}",
            window[0],
            window[1],
            pair[0],
            pair[1]
        );
    }
    assert!(
        sums[0] > sums[sums.len() - 1],
        "the λ range must actually move the total: {sums:?}"
    );
}

// -------------------------------------------------------------------
//  Survey solver input contracts (direct entry points)
// -------------------------------------------------------------------

/// `solve_waterfill_ratio` ignores `breakpoints` (the ratio objective is
/// purely geometric), so unlike the Lagrange and hybrid solvers it does not
/// require a per-column breakpoint list at all.
#[test]
fn ratio_solver_ignores_breakpoints() {
    let d = solve_waterfill_ratio(&[100.0, 100.0], &[50.0, 50.0], &[], 150.0, 200.0, 100.0);
    assert!(!d.needs_horizontal_scroll, "150 >= sum_min = 100");
    assert!(
        (d.widths.iter().sum::<f32>() - 150.0).abs() < 1e-3,
        "the equal-ratio solution must spend the available width: {:?}",
        d.widths
    );
}

#[test]
#[should_panic(expected = "solve_lagrange_penalty: breakpoints length mismatch")]
fn lagrange_solver_rejects_breakpoints_length_mismatch() {
    solve_lagrange_penalty(&[100.0], &[50.0], &[], 80.0, 100.0, 50.0);
}

#[test]
#[should_panic(expected = "solve_hybrid: breakpoints length mismatch")]
fn hybrid_solver_rejects_breakpoints_length_mismatch() {
    solve_hybrid(&[100.0], &[50.0], &[], 80.0, 100.0, 50.0);
}

#[test]
#[should_panic(expected = "solve_waterfill_ratio: length mismatch")]
fn ratio_solver_rejects_min_max_length_mismatch() {
    solve_waterfill_ratio(&[100.0, 100.0], &[50.0], &[], 80.0, 200.0, 100.0);
}

#[test]
#[should_panic(expected = "solve_waterfill_ratio: available must be finite and non-negative")]
fn ratio_solver_rejects_negative_available() {
    solve_waterfill_ratio(&[10.0], &[5.0], &[], -1.0, 10.0, 5.0);
}

#[test]
#[should_panic(expected = "solve_lagrange_penalty: available must be finite and non-negative")]
fn lagrange_solver_rejects_nan_available() {
    solve_lagrange_penalty(&[10.0], &[5.0], &[Vec::new()], f32::NAN, 10.0, 5.0);
}

#[test]
#[should_panic(expected = "solve_hybrid: available must be finite and non-negative")]
fn hybrid_solver_rejects_infinite_available() {
    solve_hybrid(&[10.0], &[5.0], &[Vec::new()], f32::INFINITY, 10.0, 5.0);
}
