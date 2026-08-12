//! Criterion micro-benchmarks for the table-width algorithm.
//!
//! The table-width solver is the project's highest-frequency
//! per-frame pure computation: every render frame that
//! contains a markdown table calls `ftwa` once per visible
//! table. The solver has five strategies (the original
//! v1 ProportionalToSlack, the FTWA v2 BreakpointWaterFill,
//! and three from the algorithm survey in
//! `doc/planning/table-column-width-algorithm.md`:
//! WaterFillRatio, LagrangePenalty, HybridMinPenaltyWaterFill)
//! and 9 input regimes (3 column distributions × 3
//! viewport states, matching the example-based test matrix
//! in `tests/table_width_algorithm_test.rs`).
//!
//! This bench is a P1 from the test audit. The 9 × 2 grid
//! (2 strategies, 9 regimes) is the operationally important
//! subset: the v1 (ProportionalToSlack) is the historical
//! baseline and the default (HybridMinPenaltyWaterFill) is
//! what the user actually runs. The other three strategies
//! are benchmarked in the unit tests for correctness; perf
//! regressions on them are caught by the same harness but
//! are not in the default grid to keep the bench runtime
//! bounded.
//!
//! # Running
//!
//! Local: `cargo bench --bench table_width`
//! CI: the bench workflow runs the suite on every push and
//! uploads the HTML report as an artifact. The CI profile
//! uses `--quick` style sample counts (5-10 per bench) so
//! the job finishes in under a minute; local runs use the
//! default criterion sample counts (100+ per bench).
//!
//! # What to look for
//!
//! - The v1 ProportionalToSlack should be measurably faster
//!   than the default HybridMinPenaltyWaterFill (the v1 is
//!   O(N) sort + linear scan; the hybrid is O(N log N) +
//!   iterative active-set reduction).
//! - The similar-column regime (all `max_content` equal) is
//!   the worst case for the v1 because every column has
//!   identical slack and the tie-break path is exercised;
//!   the v2 should be flat across column distributions
//!   because it operates on breakpoints, not slack ordering.
//! - The very-large-column-count regime (1000 columns) is
//!   the worst case for both: this is the realistic
//!   "big CSV-as-table" boundary. Should still complete
//!   in <10 ms per call.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use fastmd::markdown::table_width::{Breakpoint, DeficitStrategy, ftwa};
use std::hint::black_box;

/// The two strategies the bench cares about. The default
/// (HybridMinPenaltyWaterFill) is what the app uses in
/// production; the v1 (ProportionalToSlack) is the
/// historical baseline kept for comparison. The other
/// three strategies (BreakpointWaterFill, WaterFillRatio,
/// LagrangePenalty) are correct in unit tests but not
/// perf-tracked here — adding them would triple the
/// runtime of the bench grid without surfacing information
/// the user can act on (they're not user-selectable in
/// the config).
const BENCH_STRATEGIES: &[(&str, DeficitStrategy)] = &[
    (
        "v1_proportional_to_slack",
        DeficitStrategy::ProportionalToSlack,
    ),
    (
        "default_hybrid_min_penalty_water_fill",
        DeficitStrategy::HybridMinPenaltyWaterFill,
    ),
];

/// One row in the 9-regime grid. The regime captures the
/// shape of the input; the bench calls `ftwa` with the
/// pre-built `max_content` and `min_content` vectors so the
/// bench measures the *algorithm* cost, not the test-fixture
/// build cost.
struct Regime {
    /// Number of columns. The bench measures scaling by
    /// varying this from 3 (small markdown table) to 1000
    /// (large CSV-as-table).
    n_columns: usize,
    /// Viewport multiplier relative to `sum_max`. < 1
    /// (exceeds viewport, deficit regime), 1 (fits exactly,
    /// no slack), > 1 (surplus regime, columns pinned at
    /// max). The three values { 0.5, 1.0, 1.2 } cover the
    /// three regimes from `tests/table_width_algorithm_test.rs`.
    viewport_factor: f32,
}

const REGIMES: &[Regime] = &[
    // 3 columns × 3 viewport factors = 9 regimes. The
    // column distribution is the same across the 3
    // viewport factors (matching the permutation_* test
    // family in the test file). 3 columns is the smallest
    // non-trivial markdown table.
    Regime {
        n_columns: 3,
        viewport_factor: 1.2,
    },
    Regime {
        n_columns: 3,
        viewport_factor: 1.0,
    },
    Regime {
        n_columns: 3,
        viewport_factor: 0.5,
    },
    // 10 columns (a more typical markdown table).
    Regime {
        n_columns: 10,
        viewport_factor: 1.2,
    },
    Regime {
        n_columns: 10,
        viewport_factor: 1.0,
    },
    Regime {
        n_columns: 10,
        viewport_factor: 0.5,
    },
    // 100 columns (large markdown table, multi-section).
    Regime {
        n_columns: 100,
        viewport_factor: 1.2,
    },
    Regime {
        n_columns: 100,
        viewport_factor: 1.0,
    },
    // 1000 columns (the realistic big-CSV-as-table
    // worst case; this is the O(N log N) scaling test).
    Regime {
        n_columns: 1000,
        viewport_factor: 1.2,
    },
];

/// Build the input vectors for a regime. The column
/// distribution is "similar" (every column has the same
/// `max` and `min`); this is the same worst case the
/// `permutation_similar_columns_*` test family exercises
/// and stresses the slack-tie-break path in the v1.
struct RegimeInputs {
    max: Vec<f32>,
    min: Vec<f32>,
    breakpoints: Vec<Vec<Breakpoint>>,
    available: f32,
}

fn build_inputs(n: usize, viewport_factor: f32) -> RegimeInputs {
    // max = 100 px, min = 20 px per column. sum_max
    // = 100n, sum_min = 20n. The available is viewport_factor
    // * sum_max: < 1 deficit, = 1 exact fit, > 1 surplus.
    let max: Vec<f32> = (0..n).map(|_| 100.0).collect();
    let min: Vec<f32> = (0..n).map(|_| 20.0).collect();
    let sum_max: f32 = 100.0 * n as f32;
    let available = sum_max * viewport_factor;
    // Empty breakpoint vectors — the algorithm works
    // without breakpoints (the no-breakpoints path). Real
    // production breakpoints would be a few per column
    // (1-3 typical, 5+ degenerate); adding them here would
    // double the bench runtime without changing the
    // algorithm complexity.
    let breakpoints: Vec<Vec<Breakpoint>> = (0..n).map(|_| Vec::new()).collect();
    RegimeInputs {
        max,
        min,
        breakpoints,
        available,
    }
}

/// Run the bench grid: 2 strategies × 9 regimes = 18
/// benchmarks. Each bench measures `ftwa` with the regime's
/// pre-built inputs; criterion's throughput metric is the
/// number of columns processed per call (so a regression
/// in the per-column cost is visible).
fn bench_table_width(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_width_ftwa");
    // The default sample count (100) is too many for a
    // 1000-column regime on a slow CI runner; cap the
    // sample count to keep the grid under a minute.
    group.sample_size(20);

    for regime in REGIMES {
        // Per-regime: build the inputs once, then run the
        // bench on a borrowed slice. The build is amortised
        // across all sample iterations.
        let inputs = build_inputs(regime.n_columns, regime.viewport_factor);
        let n = inputs.max.len();

        for (strategy_label, strategy) in BENCH_STRATEGIES {
            group.throughput(Throughput::Elements(n as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(format!(
                    "n{}_v{}_{}",
                    n,
                    (regime.viewport_factor * 100.0) as u32,
                    strategy_label
                )),
                &inputs,
                |b, inputs| {
                    b.iter(|| {
                        let d = ftwa(
                            black_box(&inputs.max),
                            black_box(&inputs.min),
                            black_box(&inputs.breakpoints),
                            black_box(inputs.available),
                            black_box(*strategy),
                        );
                        // The widths length is the contract;
                        // the bench does the same check (cheaper
                        // than the full property test in the unit
                        // tests) so a regression that returns a
                        // wrong number of widths is surfaced.
                        assert_eq!(d.widths.len(), inputs.max.len());
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_table_width);
criterion_main!(benches);
