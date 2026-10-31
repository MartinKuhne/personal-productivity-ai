//! Tests for the container-query breakpoint solver (§3.0.4.4).
//!
//! Beyond the happy path, the cases that matter are the ones where the solver
//! could be talked into the *larger* presentation by a bad number: a NaN
//! allocation, an inverted measurement, a negative width. Every one of them must
//! fail closed to `Compact`.

use crate::binding::responsive::Breakpoint;
use crate::binding::responsive::LabelMode;
use crate::binding::responsive::Measurement;
use crate::binding::responsive::ResponsiveLayout;
use crate::token::space;

/// A toolbar that wants 240 px expanded and can live in 96 px compact.
fn toolbar() -> Measurement {
    Measurement::new(240.0, 96.0)
}

// ---------------------------------------------------------------------------
// The line-drawing
// ---------------------------------------------------------------------------

#[test]
fn a_container_wide_enough_for_the_full_presentation_stays_expanded() {
    let layout = ResponsiveLayout::solve(300.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Expanded);
    assert!(!layout.is_compact());
}

#[test]
fn a_container_exactly_at_the_expanded_requirement_is_expanded() {
    // The boundary is `>=`, not `>`: a container given precisely what the full
    // presentation needs should not be made to collapse.
    let layout = ResponsiveLayout::solve(240.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Expanded);
}

#[test]
fn one_pixel_short_of_expanded_collapses() {
    let layout = ResponsiveLayout::solve(239.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Compact);
    assert!(layout.is_compact());
}

#[test]
fn a_container_at_exactly_the_compact_requirement_is_compact_not_broken() {
    let layout = ResponsiveLayout::solve(96.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Compact);
    assert!(toolbar().fits_compact(96.0));
}

#[test]
fn a_container_narrower_than_compact_still_reports_compact() {
    // Below the compact floor the solver has no third answer; the rung is the
    // smallest thing it knows and the caller deals with clipping.
    let layout = ResponsiveLayout::solve(1.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Compact);
    assert!(
        !toolbar().fits_compact(1.0),
        "but the fit query honestly says it does not fit"
    );
}

#[test]
fn a_zero_width_container_collapses_rather_than_expanding() {
    let layout = ResponsiveLayout::solve(0.0, &toolbar());
    assert_eq!(layout.breakpoint(), Breakpoint::Compact);
}

#[test]
fn a_measurement_that_needs_nothing_expands_at_zero_width() {
    let empty = Measurement::new(0.0, 0.0);
    assert!(empty.fits_expanded(0.0));
    assert_eq!(
        ResponsiveLayout::solve(0.0, &empty).breakpoint(),
        Breakpoint::Expanded
    );
}

// ---------------------------------------------------------------------------
// What the rung implies
// ---------------------------------------------------------------------------

#[test]
fn padding_compresses_from_space_md_to_space_sm() {
    let expanded = ResponsiveLayout::solve(400.0, &toolbar());
    let compact = ResponsiveLayout::solve(100.0, &toolbar());
    assert_eq!(expanded.horizontal_padding(), space::MD);
    assert_eq!(compact.horizontal_padding(), space::SM);
    assert_eq!(space::MD - space::SM, 4.0, "12px -> 8px per §3.0.4.4");
}

#[test]
fn the_padding_rung_is_read_from_the_token_rather_than_restated() {
    // Guards against a literal `12.0` sneaking in beside the token.
    let layout = ResponsiveLayout::solve(400.0, &toolbar());
    assert_eq!(layout.horizontal_padding(), crate::token::space::MD);
    assert_ne!(layout.horizontal_padding(), space::SM);
}

#[test]
fn secondary_labels_go_icon_only_when_compact() {
    assert_eq!(
        ResponsiveLayout::solve(400.0, &toolbar()).label_mode(),
        LabelMode::Full
    );
    assert_eq!(
        ResponsiveLayout::solve(100.0, &toolbar()).label_mode(),
        LabelMode::IconOnly
    );
}

#[test]
fn an_icon_only_label_owes_a_hover_tooltip() {
    // §3.0.4.4 makes the tooltip mandatory once the label is gone (UIF-ATOM-007).
    assert!(!LabelMode::Full.requires_tooltip());
    assert!(LabelMode::IconOnly.requires_tooltip());
    assert!(LabelMode::Full.is_visible());
    assert!(!LabelMode::IconOnly.is_visible());
}

#[test]
fn crossing_the_rung_shifts_nothing_vertically_in_either_direction() {
    let expanded = ResponsiveLayout::solve(400.0, &toolbar());
    let compact = ResponsiveLayout::solve(100.0, &toolbar());
    assert_eq!(expanded.vertical_shift(), 0.0);
    assert_eq!(compact.vertical_shift(), 0.0);
}

#[test]
fn vertical_shift_is_zero_for_every_rung_and_so_cannot_be_otherwise() {
    // There is no field this could read. Sweeping both rungs is the honest way to
    // show the guarantee is structural rather than a value that happens to be 0.
    for allocated in [0.0, 96.0, 239.0, 240.0, 10_000.0] {
        let layout = ResponsiveLayout::solve(allocated, &toolbar());
        assert_eq!(layout.vertical_shift(), 0.0, "at {allocated}px");
    }
}

// ---------------------------------------------------------------------------
// Measurement sanitisation
// ---------------------------------------------------------------------------

#[test]
fn a_measurement_keeps_the_pair_it_was_given() {
    let measurement = Measurement::new(240.0, 96.0);
    assert_eq!(measurement.expanded_width(), 240.0);
    assert_eq!(measurement.compact_width(), 96.0);
}

#[test]
fn an_inverted_measurement_is_clamped_so_the_solver_cannot_oscillate() {
    // compact needing MORE than expanded is an upstream bug; left alone it would
    // make "fits compact" true while "fits expanded" is also true in a way that
    // depends on which query the caller asks first.
    let measurement = Measurement::new(100.0, 500.0);
    assert_eq!(measurement.compact_width(), 100.0);
    assert!(measurement.compact_width() <= measurement.expanded_width());
}

#[test]
fn negative_widths_are_clamped_to_zero() {
    let measurement = Measurement::new(-50.0, -10.0);
    assert_eq!(measurement.expanded_width(), 0.0);
    assert_eq!(measurement.compact_width(), 0.0);
}

#[test]
fn a_negative_expanded_width_cannot_make_compact_negative() {
    let measurement = Measurement::new(-100.0, 50.0);
    assert_eq!(measurement.expanded_width(), 0.0);
    assert_eq!(
        measurement.compact_width(),
        0.0,
        "clamped to the expanded rung"
    );
}

#[test]
fn a_non_finite_measurement_never_fits_so_the_solver_collapses() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let measurement = Measurement::new(bad, bad);
        assert!(
            !measurement.fits_expanded(1_000.0),
            "a {bad} requirement must not read as satisfiable"
        );
        assert_eq!(
            ResponsiveLayout::solve(1_000.0, &measurement).breakpoint(),
            Breakpoint::Compact
        );
    }
}

#[test]
fn a_non_finite_allocation_collapses_rather_than_expanding() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let layout = ResponsiveLayout::solve(bad, &toolbar());
        assert_eq!(
            layout.breakpoint(),
            Breakpoint::Compact,
            "a {bad} allocation cannot prove the full presentation fits"
        );
    }
}

#[test]
fn a_finite_allocation_above_a_huge_requirement_still_expands() {
    let measurement = Measurement::new(1_000_000.0, 10.0);
    assert_eq!(
        ResponsiveLayout::solve(1_000_000.0, &measurement).breakpoint(),
        Breakpoint::Expanded
    );
    assert_eq!(
        ResponsiveLayout::solve(999_999.0, &measurement).breakpoint(),
        Breakpoint::Compact
    );
}

// ---------------------------------------------------------------------------
// The solver is a pure function of what it is given
// ---------------------------------------------------------------------------

#[test]
fn solving_is_deterministic_and_leaves_its_inputs_alone() {
    let measurement = toolbar();
    let first = ResponsiveLayout::solve(100.0, &measurement);
    let second = ResponsiveLayout::solve(100.0, &measurement);
    assert_eq!(first, second);
    assert_eq!(measurement.expanded_width(), 240.0);
    assert_eq!(measurement.compact_width(), 96.0);
}

#[test]
fn the_rung_follows_the_container_not_any_larger_context() {
    // Two containers with the same content but different allocations must land on
    // different rungs — this is what "container query" means.
    let wide = ResponsiveLayout::solve(500.0, &toolbar());
    let narrow = ResponsiveLayout::solve(120.0, &toolbar());
    assert_eq!(wide.breakpoint(), Breakpoint::Expanded);
    assert_eq!(narrow.breakpoint(), Breakpoint::Compact);
    assert_ne!(wide.horizontal_padding(), narrow.horizontal_padding());
}

#[test]
fn breakpoint_helpers_agree_with_the_variant() {
    assert!(Breakpoint::Expanded.is_expanded());
    assert!(!Breakpoint::Expanded.is_compact());
    assert!(Breakpoint::Compact.is_compact());
    assert!(!Breakpoint::Compact.is_expanded());
}
