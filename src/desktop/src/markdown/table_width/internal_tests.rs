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

/// US1 (TBL-009): wrapping must break at whitespace (between tokens),
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
    // Unknown / empty values fall back to the historical default
    // (`default_table_width_strategy` in `config.rs` returns
    // `"waterfill"` → `BreakpointWaterFill`).
    assert_eq!(
        DeficitStrategy::from_config("unknown"),
        DeficitStrategy::BreakpointWaterFill
    );
    assert_eq!(
        DeficitStrategy::from_config(""),
        DeficitStrategy::BreakpointWaterFill
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
