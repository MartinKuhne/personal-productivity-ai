//! Unit tests for [`RangeSliderModel`] and [`RangeThumb`] (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).

use super::*;
use crate::widget::range_model::RangeKey;

#[test]
fn test_default_construction_and_properties() {
    let m = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0);
    assert_eq!(m.min(), 0.0);
    assert_eq!(m.max(), 100.0);
    assert_eq!(m.lower(), 20.0);
    assert_eq!(m.upper(), 80.0);
    assert_eq!(m.min_span(), 0.0);
    assert_eq!(m.step(), None);
    assert_eq!(m.precision(), 2);
    assert_eq!(m.unit_suffix(), None);
    assert_eq!(m.validation(), None);
    assert!(!m.is_disabled());
}

#[test]
fn test_builders_configure_fields() {
    let m = RangeSliderModel::new(10.0, 200.0, 30.0, 150.0)
        .with_min_span(15.0)
        .with_step(Some(5.0))
        .with_precision(1)
        .with_unit_suffix("px")
        .with_validation("Value too high")
        .with_disabled(true);

    assert_eq!(m.min_span(), 15.0);
    assert_eq!(m.step(), Some(5.0));
    assert_eq!(m.precision(), 1);
    assert_eq!(m.unit_suffix(), Some("px"));
    assert_eq!(m.validation(), Some("Value too high"));
    assert!(m.is_disabled());
}

#[test]
fn test_bounds_normalization() {
    // Inverted bounds swap
    let m = RangeSliderModel::new(100.0, 0.0, 20.0, 80.0);
    assert_eq!(m.ordered_bounds(), (0.0, 100.0));
    assert_eq!(m.low(), 0.0);
    assert_eq!(m.high(), 100.0);

    // Non-finite bound collapses to finite end
    let m_nan_min = RangeSliderModel::new(f64::NAN, 50.0, 10.0, 40.0);
    assert_eq!(m_nan_min.ordered_bounds(), (50.0, 50.0));

    let m_nan_max = RangeSliderModel::new(10.0, f64::NAN, 10.0, 40.0);
    assert_eq!(m_nan_max.ordered_bounds(), (10.0, 10.0));

    let m_all_nan = RangeSliderModel::new(f64::NAN, f64::NAN, 10.0, 40.0);
    assert_eq!(m_all_nan.ordered_bounds(), (0.0, 0.0));
}

#[test]
fn test_non_crossing_invariant_and_min_span() {
    let m = RangeSliderModel::new(0.0, 100.0, 60.0, 40.0).with_min_span(10.0);
    let (l, u) = m.clamped_range();
    assert!(
        l <= u - 10.0,
        "lower {l} must not cross upper {u} with min_span 10"
    );

    // Clamping lower when exceeding upper - min_span
    let m = RangeSliderModel::new(0.0, 100.0, 20.0, 50.0).with_min_span(10.0);
    assert_eq!(m.clamp_lower(45.0), 40.0);
    assert_eq!(m.clamp_lower(-10.0), 0.0);
    assert_eq!(m.clamp_lower(f64::NAN), 0.0);

    // Clamping upper when under lower + min_span
    assert_eq!(m.clamp_upper(25.0), 30.0);
    assert_eq!(m.clamp_upper(110.0), 100.0);
    assert_eq!(m.clamp_upper(f64::NAN), 100.0);
}

#[test]
fn test_fractions_and_value_mapping() {
    let m = RangeSliderModel::new(0.0, 100.0, 25.0, 75.0);
    assert_eq!(m.lower_fraction(), 0.25);
    assert_eq!(m.upper_fraction(), 0.75);

    assert_eq!(m.value_at_fraction(0.5), 50.0);
    assert_eq!(m.value_at_fraction(-0.1), 0.0);
    assert_eq!(m.value_at_fraction(1.5), 100.0);
    assert_eq!(m.value_at_fraction(f32::NAN), 0.0);
}

#[test]
fn test_endpoint_disambiguation() {
    // When separated: picks closest thumb
    let m = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0);
    assert_eq!(m.disambiguate_thumb(0.1), RangeThumb::Lower);
    assert_eq!(m.disambiguate_thumb(0.4), RangeThumb::Lower);
    assert_eq!(m.disambiguate_thumb(0.6), RangeThumb::Upper);
    assert_eq!(m.disambiguate_thumb(0.9), RangeThumb::Upper);

    // When coincident at 0.5:
    let m = RangeSliderModel::new(0.0, 100.0, 50.0, 50.0);
    // Click left of 0.5 -> Lower
    assert_eq!(m.disambiguate_thumb(0.3), RangeThumb::Lower);
    // Click right of 0.5 -> Upper
    assert_eq!(m.disambiguate_thumb(0.7), RangeThumb::Upper);
    // Click exactly at 0.5 -> Upper (room to move right)
    assert_eq!(m.disambiguate_thumb(0.5), RangeThumb::Upper);

    // When coincident at right edge (1.0): Upper has no room to move right, so picks Lower
    let m = RangeSliderModel::new(0.0, 100.0, 100.0, 100.0);
    assert_eq!(m.disambiguate_thumb(1.0), RangeThumb::Lower);
}

#[test]
fn test_drift_free_keyboard_stepping() {
    let m = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0).with_step(Some(0.1));

    // Lower steps
    let (new_l, new_u) = m.step_thumb(RangeThumb::Lower, RangeKey::Increase);
    assert!((new_l - 20.1).abs() < 1e-9);
    assert_eq!(new_u, 80.0);

    let (new_l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::Decrease);
    assert!((new_l - 19.9).abs() < 1e-9);

    let (new_l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::PageUp);
    assert!((new_l - 21.0).abs() < 1e-9);

    let (new_l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::PageDown);
    assert!((new_l - 19.0).abs() < 1e-9);

    let (new_l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::Home);
    assert_eq!(new_l, 0.0);

    let (new_l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::End);
    assert_eq!(new_l, 80.0);

    // Upper steps
    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::Increase);
    assert!((new_u - 80.1).abs() < 1e-9);

    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::Decrease);
    assert!((new_u - 79.9).abs() < 1e-9);

    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::PageUp);
    assert!((new_u - 81.0).abs() < 1e-9);

    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::PageDown);
    assert!((new_u - 79.0).abs() < 1e-9);

    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::Home);
    assert_eq!(new_u, 20.0);

    let (_, new_u) = m.step_thumb(RangeThumb::Upper, RangeKey::End);
    assert_eq!(new_u, 100.0);

    // Verify 1000 steps without cumulative floating-point drift
    let mut current_model = RangeSliderModel::new(0.0, 1000.0, 0.0, 1000.0).with_step(Some(0.1));
    for _ in 0..1000 {
        let (next_l, next_u) = current_model.step_thumb(RangeThumb::Lower, RangeKey::Increase);
        current_model.lower = next_l;
        current_model.upper = next_u;
    }
    // After 1000 steps of 0.1 from 0.0, exact answer is 100.0
    assert_eq!(current_model.lower, 100.0);
}

#[test]
fn test_stepping_respects_non_crossing_and_min_span() {
    let m = RangeSliderModel::new(0.0, 100.0, 48.0, 50.0)
        .with_min_span(2.0)
        .with_step(Some(1.0));

    // Lower cannot step past 48.0 because upper is 50.0 and min_span is 2.0
    let (l, _) = m.step_thumb(RangeThumb::Lower, RangeKey::Increase);
    assert_eq!(l, 48.0);

    // Upper cannot step below 50.0 because lower is 48.0 and min_span is 2.0
    let (_, u) = m.step_thumb(RangeThumb::Upper, RangeKey::Decrease);
    assert_eq!(u, 50.0);
}

#[test]
fn test_readout_formatting() {
    let m = RangeSliderModel::new(0.0, 100.0, 12.3456, 87.6543)
        .with_precision(1)
        .with_unit_suffix("ms");

    assert_eq!(m.format_value(12.3456), "12.3ms");
    assert_eq!(m.display_lower(), "12.3ms");
    assert_eq!(m.display_upper(), "87.7ms");
    assert_eq!(m.display(), ("12.3ms".to_string(), "87.7ms".to_string()));
}
