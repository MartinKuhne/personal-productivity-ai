//! Tests for [`RangeModel`] — the numeric half of `UIF-ATOM-005`.
//!
//! Four groups, in order:
//!
//! 1. **The interval.** Inverted bounds, non-finite bounds, degeneracy —
//!    the four policies §3.5 does not state, each pinned.
//! 2. **Clamp and snap.** That finiteness is tested *before* the clamp,
//!    and that a malformed step reads as continuous rather than fatal.
//! 3. **Stepping and keys.** The whole keyboard contract, plus the drift
//!    walk: a thousand steps up and a thousand back must land exactly
//!    where they started.
//! 4. **Position and text.** `fraction` / `value_at_fraction` round
//!    trips, and the display/parse pair that must reject the words `nan`
//!    and `inf` a user can type.

use crate::widget::range_model::{PAGE_STEP_MULTIPLIER, RangeKey, RangeModel};

/// Assert two `f64` agree to within a hair.
macro_rules! assert_close {
    ($actual:expr, $expected:expr $(,)?) => {{
        let actual = $actual;
        let expected = $expected;
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }};
    ($actual:expr, $expected:expr, $what:expr $(,)?) => {{
        let actual = $actual;
        let expected = $expected;
        assert!(
            (actual - expected).abs() < 1e-9,
            "{}: expected {}, got {}",
            $what,
            expected,
            actual
        );
    }};
}

/// A stepped model over `0..=100` in `5`s, starting at `50`.
fn model() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0).with_step(Some(5.0))
}

// ---------------------------------------------------------------------------
// 1. The interval
// ---------------------------------------------------------------------------

#[test]
fn accessors_echo_what_the_host_declared() {
    let m = RangeModel::new(3.0, 1.0, 9.0)
        .with_step(Some(2.0))
        .with_precision(1)
        .with_unit_suffix("px")
        .with_validation("too small")
        .with_disabled(true);
    assert_close!(m.value(), 3.0, "value");
    assert_close!(m.min(), 1.0, "min");
    assert_close!(m.max(), 9.0, "max");
    assert_close!(m.step().unwrap(), 2.0, "step");
    assert_eq!(m.precision(), 1);
    assert_eq!(m.unit_suffix(), Some("px"));
    assert_eq!(m.validation(), Some("too small"));
    assert!(m.disabled_flag());
}

#[test]
fn a_plain_model_is_continuous_and_enabled() {
    let m = RangeModel::new(0.0, 0.0, 1.0);
    assert_eq!(m.step(), None);
    assert!(!m.disabled_flag());
    assert_eq!(m.unit_suffix(), None);
    assert_eq!(m.validation(), None);
}

#[test]
fn the_constructor_stores_its_arguments_rather_than_rewriting_them() {
    // The inversion has to survive construction so a host can see it. A
    // constructor that silently swapped would erase the mistake worth fixing.
    let m = RangeModel::new(0.0, 9.0, 1.0);
    assert_close!(m.min(), 9.0, "min stays what the host said");
    assert_close!(m.max(), 1.0, "max stays what the host said");
    assert!(m.is_inverted(), "and the inversion is reported");
}

#[test]
fn ordered_bounds_swap_an_inverted_range() {
    let m = RangeModel::new(0.0, 9.0, 1.0);
    assert_close!(m.low(), 1.0, "inverted range normalises by swapping");
    assert_close!(m.high(), 9.0);
    assert!(!m.is_degenerate());
}

#[test]
fn a_well_formed_range_is_never_inverted() {
    let m = RangeModel::new(0.0, 1.0, 9.0);
    assert!(!m.is_inverted());
    assert_close!(m.low(), 1.0, "low");
    assert_close!(m.high(), 9.0, "high");
}

#[test]
fn equal_bounds_are_not_inverted_but_are_degenerate() {
    let m = RangeModel::new(0.0, 5.0, 5.0);
    assert!(!m.is_inverted(), "5 > 5 is false");
    assert!(m.is_degenerate(), "but there is nothing to travel");
}

#[test]
fn a_non_finite_bound_collapses_onto_the_bound_that_is_finite() {
    let m = RangeModel::new(0.0, f64::NAN, 40.0);
    assert_close!(m.low(), 40.0, "the NaN end collapses onto the finite one");
    assert_close!(m.high(), 40.0);
    assert!(m.is_degenerate());

    let m = RangeModel::new(0.0, 40.0, f64::INFINITY);
    assert_close!(m.low(), 40.0, "an infinite end collapses the same way");
    assert_close!(m.high(), 40.0);

    let m = RangeModel::new(0.0, f64::NEG_INFINITY, 7.0);
    assert_close!(m.low(), 7.0, "and so does negative infinity");
    assert_close!(m.high(), 7.0);
}

#[test]
fn no_finite_bound_at_all_falls_back_to_zero() {
    let m = RangeModel::new(0.0, f64::NAN, f64::NAN);
    assert_close!(m.low(), 0.0, "no interval at all → the identity range");
    assert_close!(m.high(), 0.0);
    assert!(m.is_degenerate());

    let m = RangeModel::new(0.0, f64::INFINITY, f64::NEG_INFINITY);
    assert_close!(m.low(), 0.0, "two infinities are not an interval either");
    assert_close!(m.high(), 0.0);
}

#[test]
fn is_inverted_needs_both_bounds_finite() {
    // `NaN > 5` is false, and reporting an inversion the host could not have
    // meant would send them looking for the wrong bug.
    assert!(!RangeModel::new(0.0, f64::NAN, 5.0).is_inverted());
    assert!(!RangeModel::new(0.0, 5.0, f64::NAN).is_inverted());
}

// ---------------------------------------------------------------------------
// 2. Clamp and snap
// ---------------------------------------------------------------------------

#[test]
fn clamp_pulls_inside_and_passes_through() {
    let m = RangeModel::new(0.0, 0.0, 10.0);
    assert_close!(m.clamp(-5.0), 0.0, "below");
    assert_close!(m.clamp(5.0), 5.0, "inside");
    assert_close!(m.clamp(50.0), 10.0, "above");
}

#[test]
fn clamp_of_a_non_finite_number_lands_on_the_low_end_before_the_clamp_runs() {
    // The order is the whole point: `f64::clamp` passes NaN straight through,
    // so a clamp that narrowed first would hand back a NaN and every pixel
    // derived from it would be a NaN too.
    let m = RangeModel::new(0.0, 2.0, 8.0);
    assert!(m.clamp(f64::NAN).is_finite(), "NaN must not survive");
    assert_close!(m.clamp(f64::NAN), 2.0, "NaN → low end");
    assert_close!(m.clamp(f64::INFINITY), 2.0, "+inf → low end, not high");
    assert_close!(m.clamp(f64::NEG_INFINITY), 2.0, "-inf → low end");
}

#[test]
fn clamped_value_reads_the_stored_value() {
    let m = RangeModel::new(500.0, 0.0, 100.0);
    assert_close!(m.value(), 500.0, "the stored value is untouched");
    assert_close!(m.clamped_value(), 100.0, "what paints is narrowed");

    let m = RangeModel::new(f64::NAN, 0.0, 100.0);
    assert_close!(m.clamped_value(), 0.0, "a NaN value paints the low end");
}

#[test]
fn effective_step_reads_zero_as_continuous() {
    let m = RangeModel::new(0.0, 0.0, 10.0).with_step(Some(0.0));
    assert_eq!(m.effective_step(), None, "a zero step quantises nothing");
    assert_close!(m.snap(3.25), 3.25, "and so does not move a value");
}

#[test]
fn effective_step_reads_a_negative_step_as_its_magnitude() {
    // A step is a distance. The sign is a transcription error, not a request
    // to invert the axis.
    let m = RangeModel::new(0.0, 0.0, 10.0).with_step(Some(-2.0));
    assert_close!(m.effective_step().unwrap(), 2.0, "magnitude");
    assert_close!(m.snap(3.0), 4.0, "and it snaps normally");
}

#[test]
fn effective_step_reads_a_non_finite_step_as_continuous() {
    for step in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let m = RangeModel::new(0.0, 0.0, 10.0).with_step(Some(step));
        assert_eq!(m.effective_step(), None, "{step} is not a step");
        assert_close!(m.snap(3.25), 3.25, "and does not quantise");
    }
}

#[test]
fn snap_rounds_to_the_nearest_grid_point() {
    let m = model();
    assert_close!(m.snap(0.0), 0.0, "on the grid");
    assert_close!(m.snap(2.4), 0.0, "below the halfway mark");
    assert_close!(m.snap(2.5), 5.0, "a tie rounds away from zero");
    assert_close!(m.snap(7.4), 5.0, "below again");
    assert_close!(m.snap(7.5), 10.0, "tie again");
    assert_close!(m.snap(48.0), 50.0, "mid-range");
}

#[test]
fn snap_clamps_so_it_cannot_land_on_an_off_range_grid_point() {
    // min 0, max 10, step 3 → grid 0, 3, 6, 9. `10` must come back 9,
    // never 12.
    let m = RangeModel::new(0.0, 0.0, 10.0).with_step(Some(3.0));
    assert_close!(m.snap(10.0), 9.0, "clamped before rounding");
    assert_close!(m.snap(11.0), 9.0, "not 12");
    assert_close!(m.snap(-1.0), 0.0, "and at the low end");
}

#[test]
fn snap_is_idempotent() {
    let m = model();
    for raw in [-7.0, 0.0, 1.0, 2.5, 3.4, 49.9, 99.9, 1_000.0, f64::NAN] {
        let once = m.snap(raw);
        assert_close!(m.snap(once), once, "snapping twice must not move again");
    }
}

#[test]
fn snap_of_a_non_finite_input_lands_on_the_low_end() {
    let m = model();
    assert_close!(m.snap(f64::NAN), 0.0, "NaN");
    assert_close!(m.snap(f64::INFINITY), 0.0, "+inf");
    assert_close!(m.snap(f64::NEG_INFINITY), 0.0, "-inf");
}

#[test]
fn a_step_larger_than_the_range_still_reaches_both_ends() {
    let m = RangeModel::new(0.0, 0.0, 3.0).with_step(Some(10.0));
    assert_close!(m.snap(1.0), 0.0, "1.0 is nearer 0 than the off-range 10");
    assert_close!(m.snap(2.0), 0.0, "same at 2.0");
    assert_close!(
        m.step_by(0.0, 1).unwrap(),
        3.0,
        "End must still be reachable",
    );
    assert_close!(m.step_by(3.0, -1).unwrap(), 0.0, "and Home");
}

#[test]
fn a_step_on_a_degenerate_range_stays_put() {
    let m = RangeModel::new(4.0, 4.0, 4.0).with_step(Some(2.0));
    assert_close!(m.snap(0.0), 4.0, "nothing to snap onto but the one point");
    assert_close!(m.step_by(4.0, 1).unwrap(), 4.0, "and nowhere to step to");
    assert_close!(m.step_by(4.0, -1).unwrap(), 4.0);
}

// ---------------------------------------------------------------------------
// 3. Stepping and keys
// ---------------------------------------------------------------------------

#[test]
fn step_by_moves_by_whole_steps() {
    let m = model();
    assert_close!(m.step_by(20.0, 1).unwrap(), 25.0, "one up");
    assert_close!(m.step_by(20.0, 3).unwrap(), 35.0, "three up");
    assert_close!(m.step_by(20.0, -1).unwrap(), 15.0, "one down");
    assert_close!(m.step_by(20.0, -4).unwrap(), 0.0, "clamped at the low end");
    assert_close!(
        m.step_by(20.0, 100).unwrap(),
        100.0,
        "clamped at the high end",
    );
}

#[test]
fn step_by_from_an_off_grid_value_re_grids_first() {
    // The value is re-derived from the grid on every call rather than
    // accumulated, so an off-grid input is rounded to its *nearest* grid
    // point before the step is applied — which means the answer depends on
    // which side of the grid the stray value sits on. That is the point of
    // rounding instead of truncating: 24 is closer to 25 than to 20, and
    // pretending otherwise is how a slider ends up a tenth of a step off
    // after a long drag.
    let m = model(); // step 5, range 0..100
    assert_close!(m.step_by(22.0, 1).unwrap(), 25.0, "22 → 20 → 25");
    assert_close!(m.step_by(23.0, 1).unwrap(), 30.0, "23 → 25 → 30");
    assert_close!(m.step_by(24.0, 1).unwrap(), 30.0, "24 → 25 → 30");
    assert_close!(m.step_by(26.0, -1).unwrap(), 20.0, "26 → 25 → 20");
    assert_close!(m.step_by(27.0, -1).unwrap(), 20.0, "27 → 25 → 20");
    // And an exactly-on-grid value is untouched by the round trip.
    assert_close!(m.step_by(25.0, 1).unwrap(), 30.0, "already gridded");
}

#[test]
fn step_by_without_an_effective_step_is_no_stepping_at_all() {
    let m = RangeModel::new(0.0, 0.0, 10.0);
    assert_eq!(m.step_by(3.0, 1), None, "no step → no move, said as None");
    assert_eq!(m.step_by(3.0, -1), None);
    assert_eq!(m.step_by(3.0, 100), None);

    let m = RangeModel::new(0.0, 0.0, 10.0).with_step(Some(0.0));
    assert_eq!(m.step_by(3.0, 1), None, "a zero step is the same absence");
}

#[test]
fn a_thousand_steps_up_and_back_down_land_exactly_where_they_started() {
    // The drift test. `0.1` is not representable in binary, so an
    // implementation that accumulated `value += step` would be visibly
    // wrong long before a thousand presses.
    let m = RangeModel::new(0.0, 0.0, 1_000.0).with_step(Some(0.1));
    let mut v: f64 = 0.0;
    for _ in 0..1_000 {
        v = m.step_by(v, 1).unwrap();
    }
    assert!(v.is_finite(), "1 000 steps must not overflow to inf");
    assert!(
        (v - 100.0).abs() < 1e-9,
        "1 000 × 0.1 must read 100.0, got {v:?}"
    );
    for _ in 0..1_000 {
        v = m.step_by(v, -1).unwrap();
    }
    assert_eq!(v, 0.0, "and back down again, exactly — not 1e-13");
}

#[test]
fn a_long_run_stays_on_the_grid_at_every_press() {
    // Not just the endpoints: every intermediate value must be a whole
    // multiple of the step away from `low`, which is what "on the grid"
    // means and what an accumulating implementation fails mid-walk.
    let m = RangeModel::new(0.0, 0.0, 100.0).with_step(Some(0.2));
    let mut v: f64 = 0.0;
    for i in 0..500 {
        v = m.step_by(v, 1).unwrap();
        let index = (v / 0.2).round();
        assert!(
            (v - index * 0.2).abs() < 1e-9,
            "press {i} left {v:?} off the 0.2 grid"
        );
    }
}

#[test]
fn stepping_never_produces_a_non_finite_or_out_of_range_value() {
    let m = RangeModel::new(0.0, 0.0, 1.0).with_step(Some(0.3));
    for raw in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1e300,
        1e300,
        0.0,
    ] {
        for delta in [-1_i64, 1, 10, -10] {
            let v = m.step_by(raw, delta).unwrap();
            assert!(v.is_finite(), "step_by({raw}, {delta}) → {v}");
            assert!(v >= m.low() && v <= m.high(), "step escaped the range");
        }
    }
}

#[test]
fn arrow_keys_move_by_one_step() {
    let m = model();
    assert_close!(m.apply_key(RangeKey::Increase).unwrap(), 55.0, "→ / ↑");
    assert_close!(m.apply_key(RangeKey::Decrease).unwrap(), 45.0, "← / ↓");
}

#[test]
fn page_keys_move_by_ten_steps() {
    let m = model();
    assert_eq!(PAGE_STEP_MULTIPLIER, 10, "§3.5's ±10 × step");
    assert_close!(m.apply_key(RangeKey::PageUp).unwrap(), 100.0, "PageUp");
    assert_close!(m.apply_key(RangeKey::PageDown).unwrap(), 0.0, "PageDown");

    let m = RangeModel::new(10.0, 0.0, 100.0).with_step(Some(5.0));
    assert_close!(m.apply_key(RangeKey::PageUp).unwrap(), 60.0, "10 + 10×5");
    assert_close!(m.apply_key(RangeKey::PageDown).unwrap(), 0.0, "clamped");
}

#[test]
fn home_and_end_jump_to_the_bounds() {
    let m = model();
    assert_close!(m.apply_key(RangeKey::Home).unwrap(), 0.0, "Home = min");
    assert_close!(m.apply_key(RangeKey::End).unwrap(), 100.0, "End = max");
}

#[test]
fn home_and_end_work_on_a_continuous_model() {
    // They name the ends rather than a distance, so they need no step.
    let m = RangeModel::new(5.0, 2.0, 8.0);
    assert_close!(
        m.apply_key(RangeKey::Home).unwrap(),
        2.0,
        "Home without a step",
    );
    assert_close!(
        m.apply_key(RangeKey::End).unwrap(),
        8.0,
        "End without a step",
    );
}

#[test]
fn home_and_end_return_the_declared_bounds_not_the_nearest_grid_point() {
    // §3.5 says `Home = min`, not `Home = the first step`.
    let m = RangeModel::new(50.0, 1.0, 11.0).with_step(Some(5.0));
    assert_close!(m.apply_key(RangeKey::Home).unwrap(), 1.0, "min, not 0 or 6");
    assert_close!(
        m.apply_key(RangeKey::End).unwrap(),
        11.0,
        "max, not 11 off-grid",
    );
}

#[test]
fn arrow_and_page_keys_do_nothing_without_a_step() {
    // §3.5 defines every keyed move as a multiple of `step`, and there is
    // no multiple of nothing. Inventing one would be behaviour the spec
    // does not describe.
    let m = RangeModel::new(50.0, 0.0, 100.0);
    for key in [
        RangeKey::Decrease,
        RangeKey::Increase,
        RangeKey::PageDown,
        RangeKey::PageUp,
    ] {
        assert_eq!(m.apply_key(key), None, "{key:?} has no increment");
    }
}

#[test]
fn keys_clamp_at_both_ends() {
    let m = RangeModel::new(100.0, 0.0, 100.0).with_step(Some(5.0));
    assert_close!(
        m.apply_key(RangeKey::Increase).unwrap(),
        100.0,
        "at the top",
    );
    assert_close!(
        m.apply_key(RangeKey::PageUp).unwrap(),
        100.0,
        "PageUp at the top",
    );

    let m = RangeModel::new(0.0, 0.0, 100.0).with_step(Some(5.0));
    assert_close!(
        m.apply_key(RangeKey::Decrease).unwrap(),
        0.0,
        "at the bottom",
    );
    assert_close!(
        m.apply_key(RangeKey::PageDown).unwrap(),
        0.0,
        "PageDown at the bottom",
    );
}

#[test]
fn a_key_from_a_non_finite_value_recovers_onto_the_range() {
    let m = RangeModel::new(f64::NAN, 0.0, 100.0).with_step(Some(5.0));
    assert_close!(
        m.apply_key(RangeKey::Increase).unwrap(),
        5.0,
        "NaN → low → one step",
    );
    assert_close!(m.apply_key(RangeKey::Home).unwrap(), 0.0, "Home from NaN");
    assert_close!(m.apply_key(RangeKey::End).unwrap(), 100.0, "End from NaN");
}

// ---------------------------------------------------------------------------
// 4. Position and text
// ---------------------------------------------------------------------------

#[test]
fn fraction_is_zero_at_the_low_end_and_one_at_the_high_end() {
    assert_close!(
        f64::from(RangeModel::new(0.0, 0.0, 100.0).fraction()),
        0.0,
        "low",
    );
    assert_close!(
        f64::from(RangeModel::new(100.0, 0.0, 100.0).fraction()),
        1.0,
        "high",
    );
    assert_close!(
        f64::from(RangeModel::new(25.0, 0.0, 100.0).fraction()),
        0.25,
        "quarter",
    );
    assert_close!(
        f64::from(RangeModel::new(-50.0, -50.0, 50.0).fraction()),
        0.0,
        "negative range low",
    );
    assert_close!(
        f64::from(RangeModel::new(0.0, -50.0, 50.0).fraction()),
        0.5,
        "negative range mid",
    );
}

#[test]
fn fraction_of_a_degenerate_range_is_zero_not_a_division_by_zero() {
    let m = RangeModel::new(5.0, 5.0, 5.0);
    let f = m.fraction();
    assert!(f.is_finite(), "0/0 must not escape as NaN");
    assert_close!(f64::from(f), 0.0, "one point sits at the start");
}

#[test]
fn fraction_clamps_its_input_through_the_value() {
    assert_close!(
        f64::from(RangeModel::new(-10.0, 0.0, 100.0).fraction()),
        0.0,
        "below",
    );
    assert_close!(
        f64::from(RangeModel::new(110.0, 0.0, 100.0).fraction()),
        1.0,
        "above",
    );
    assert_close!(
        f64::from(RangeModel::new(f64::NAN, 0.0, 100.0).fraction()),
        0.0,
        "NaN",
    );
}

#[test]
fn value_at_fraction_inverts_fraction() {
    for v in [0.0, 5.0, 35.0, 70.0, 100.0] {
        let probe = RangeModel::new(v, 0.0, 100.0).with_step(Some(5.0));
        assert_close!(
            probe.value_at_fraction(probe.fraction()),
            v,
            "value → fraction → value",
        );
    }
    for f in [0.0f32, 0.1, 0.5, 0.9, 1.0] {
        let m = RangeModel::new(0.0, 0.0, 100.0);
        assert_close!(
            f64::from(m.value_at_fraction(f).fraction_of(0.0, 100.0)),
            f64::from(f),
            "fraction → value → fraction",
        );
    }
}

/// Test-only helper: where a value sits on a plain `0..=100` span.
trait AsFraction {
    fn fraction_of(&self, low: f64, high: f64) -> f32;
}

impl AsFraction for f64 {
    fn fraction_of(&self, low: f64, high: f64) -> f32 {
        RangeModel::new(*self, low, high).fraction()
    }
}

#[test]
fn value_at_fraction_of_a_degenerate_range_is_the_bound() {
    let m = RangeModel::new(0.0, 3.0, 3.0);
    assert_close!(m.value_at_fraction(0.5), 3.0, "no width → the one point");
    assert_close!(m.value_at_fraction(0.0), 3.0);
    assert_close!(m.value_at_fraction(1.0), 3.0);
}

#[test]
fn value_at_fraction_is_monotonic() {
    let m = RangeModel::new(0.0, 0.0, 100.0);
    let mut last = f64::MIN;
    for i in 0..=100 {
        let f = i as f32 / 100.0;
        let v = m.value_at_fraction(f);
        assert!(v >= last, "value_at_fraction must not go backwards at {f}");
        last = v;
    }
}

#[test]
fn a_fraction_outside_the_track_reads_the_nearest_end() {
    let m = model();
    assert_close!(m.value_at_fraction(-0.5), 0.0, "left of the track");
    assert_close!(m.value_at_fraction(1.5), 100.0, "right of the track");
}

#[test]
fn a_non_finite_fraction_reads_the_low_end() {
    let m = model();
    assert!(m.value_at_fraction(f32::NAN).is_finite(), "NaN fraction");
    assert_close!(m.value_at_fraction(f32::NAN), 0.0, "NaN → low end");
    assert_close!(m.value_at_fraction(f32::INFINITY), 0.0, "+inf → low end");
}

#[test]
fn value_at_fraction_lands_on_the_grid() {
    let m = model();
    for i in 0..=100 {
        let f = i as f32 / 100.0;
        let v = m.value_at_fraction(f);
        assert!(
            (v / 5.0).fract().abs() < 1e-9,
            "fraction {f} gave {v:?}, off the 5-unit grid"
        );
    }
}

#[test]
fn display_renders_at_the_stated_precision() {
    let m = RangeModel::new(12.3456, 0.0, 100.0);
    assert_eq!(m.display(), "12.35", "the default precision is 2");
    assert_eq!(m.clone().with_precision(0).display(), "12");
    assert_eq!(m.clone().with_precision(3).display(), "12.346");
    assert_eq!(m.clone().with_precision(4).display(), "12.3456");
}

#[test]
fn display_appends_the_unit_suffix_verbatim() {
    let m = RangeModel::new(24.0, 0.0, 100.0).with_unit_suffix("px");
    assert_eq!(m.display(), "24.00px", "no separator is invented");
    let m = m.with_unit_suffix(" px");
    assert_eq!(m.display(), "24.00 px", "the host may carry its own space");
}

#[test]
fn display_of_a_non_finite_value_renders_the_low_end() {
    let m = RangeModel::new(f64::NAN, 0.0, 100.0);
    assert_eq!(
        m.display(),
        "0.00",
        "never the word `NaN` in a numeric field"
    );
    let m = RangeModel::new(f64::INFINITY, 1.0, 9.0);
    assert_eq!(m.display(), "1.00", "nor `inf`");
}

#[test]
fn display_and_parse_round_trip() {
    for v in [0.0, 1.0, 12.34, 99.99, 100.0] {
        let m = RangeModel::new(v, 0.0, 100.0).with_unit_suffix("px");
        let text = m.display();
        let back = m
            .parse(&text)
            .unwrap_or_else(|| panic!("`{text}` must parse back"));
        assert_close!(back, v, format!("`{text}` should round trip"));
    }
}

#[test]
fn parse_strips_the_unit_and_surrounding_whitespace() {
    let m = RangeModel::new(0.0, 0.0, 100.0).with_unit_suffix("px");
    assert_close!(m.parse("  24 px  ").unwrap(), 24.0, "padded with the unit");
    assert_close!(m.parse("24px").unwrap(), 24.0, "flush");
    assert_close!(m.parse("24").unwrap(), 24.0, "unit omitted");
}

#[test]
fn parse_rejects_text_that_is_not_a_number() {
    let m = model();
    for text in ["", "   ", "abc", "12.3.4", "px", "1e", "--3", "1 2"] {
        assert_eq!(m.parse(text), None, "`{text}` must not parse");
    }
}

#[test]
fn parse_rejects_the_words_a_user_can_type_for_non_finite() {
    // `str::parse::<f64>` accepts all of these. A numeric field that takes
    // them is a numeric field that lies.
    let m = model();
    for text in [
        "nan", "NaN", "-nan", "inf", "Inf", "-inf", "infinity", "1e999",
    ] {
        assert_eq!(m.parse(text), None, "`{text}` must be rejected");
    }
}

#[test]
fn parse_snaps_what_it_reads() {
    // Typing `7` into a step-5 field yields 5: the field shows what the
    // control can actually hold.
    let m = model();
    assert_close!(m.parse("7").unwrap(), 5.0, "snapped down");
    assert_close!(m.parse("8").unwrap(), 10.0, "snapped up");
    assert_close!(m.parse("500").unwrap(), 100.0, "clamped");
    assert_close!(m.parse("-500").unwrap(), 0.0, "clamped low");
}

#[test]
fn parse_without_a_step_keeps_the_typed_value() {
    let m = RangeModel::new(0.0, 0.0, 100.0);
    assert_close!(
        m.parse("37.913").unwrap(),
        37.913,
        "continuous → no quantising",
    );
}

#[test]
fn display_trait_writes_the_same_text() {
    let m = RangeModel::new(8.5, 0.0, 100.0).with_unit_suffix("%");
    assert_eq!(m.to_string(), m.display(), "Display is the display() text");
}

// ---------------------------------------------------------------------------
// Whole-surface invariant
// ---------------------------------------------------------------------------

#[test]
fn no_public_method_returns_a_non_finite_or_out_of_range_number() {
    // A sweep over deliberately awkward configurations, checking that
    // nothing a caller can reach escapes the range or the number system.
    let configs = [
        (0.0, 100.0, Some(5.0)),
        (100.0, 0.0, Some(5.0)),
        (-50.0, 50.0, Some(0.1)),
        (0.0, 1.0, None),
        (5.0, 5.0, Some(2.0)),
        (0.0, 3.0, Some(10.0)),
        (-1.0, 0.0, Some(0.25)),
        (0.0, 1e6, Some(1e-3)),
        (f64::NAN, f64::NAN, Some(1.0)),
        (f64::INFINITY, 4.0, None),
    ];
    let probes = [-1e9, -100.0, -0.5, 0.0, 0.5, 1.0, 3.7, 1e9, f64::NAN];
    let keys = [
        RangeKey::Decrease,
        RangeKey::Increase,
        RangeKey::PageDown,
        RangeKey::PageUp,
        RangeKey::Home,
        RangeKey::End,
    ];
    for (low, high, step) in configs {
        let base = RangeModel::new(0.0, low, high).with_step(step);
        let (lo, hi) = base.ordered_bounds();
        assert!(lo.is_finite() && hi.is_finite(), "bounds escaped");
        assert!(lo <= hi, "ordered_bounds must be ordered");

        for probe in probes {
            let v = base.snap(probe);
            assert!(v.is_finite(), "snap({probe}) on {low}..={high} → {v}");
            assert!(v >= lo && v <= hi, "snap escaped the range");
            assert!(v >= base.clamp(v), "snap must not undo the clamp");

            let f = RangeModel::new(probe, low, high).fraction();
            assert!(f.is_finite(), "fraction({probe}) → {f}");
            assert!((0.0..=1.0).contains(&f), "fraction outside 0..=1");

            let back = base.value_at_fraction(f);
            assert!(back.is_finite(), "value_at_fraction({f}) → {back}");
            assert!(back >= lo && back <= hi, "round trip escaped the range");

            let text = RangeModel::new(probe, low, high).display();
            assert!(!text.is_empty(), "display must never be empty");
            assert!(
                !text.contains("NaN") && !text.contains("inf"),
                "display leaked a non-finite word: {text}"
            );
        }

        for key in keys {
            let m = RangeModel::new(50.0, low, high).with_step(step);
            if let Some(v) = m.apply_key(key) {
                assert!(v.is_finite(), "{key:?} on {low}..={high} → {v}");
                assert!(v >= lo && v <= hi, "{key:?} escaped the range");
            }
        }
    }
}
