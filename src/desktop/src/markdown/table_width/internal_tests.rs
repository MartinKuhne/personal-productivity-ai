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
    assert_eq!(
        DeficitStrategy::from_config("unknown"),
        DeficitStrategy::ProportionalToSlack
    );
    assert_eq!(
        DeficitStrategy::from_config(""),
        DeficitStrategy::ProportionalToSlack
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
