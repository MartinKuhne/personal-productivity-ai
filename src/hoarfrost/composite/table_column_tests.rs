//! Tests for the column model and pure width resolver
//! ([`super::table_column`], `UIF-COMP-007`).

use super::*;

/// Tolerance wide enough for accumulated `f32` drift, narrow enough that a
/// real layout error of even a tenth of a pixel still fails.
const TOL: f32 = 1e-3;

fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < TOL
}

fn content(id: ColId, preferred: f32, min: f32) -> ColumnSpec {
    ColumnSpec::content(id, "c", preferred).with_min(min)
}

fn proportional(id: ColId, weight: f32, min: f32) -> ColumnSpec {
    ColumnSpec::proportional(id, "c", weight).with_min(min)
}

// ---------------------------------------------------------------------------
// ColumnSpec::bounds
// ---------------------------------------------------------------------------

#[test]
fn fixed_bounds_are_rigid() {
    let bounds = ColumnSpec::fixed(0, "c", 120.0).bounds();
    assert!(approx(bounds.floor, 120.0));
    assert!(approx(bounds.target, 120.0));
    assert!(approx(bounds.ceiling, 120.0));
    assert!(approx(bounds.weight, 0.0));
    assert!(
        !bounds.is_elastic(),
        "a rigid column must never take surplus"
    );
}

#[test]
fn content_bounds_never_exceed_preferred() {
    let bounds = content(0, 180.0, 40.0).bounds();
    assert!(approx(bounds.floor, 40.0));
    assert!(approx(bounds.target, 180.0));
    assert!(
        approx(bounds.ceiling, 180.0),
        "a content column must be capped at its own content"
    );
    assert!(!bounds.is_elastic());
}

#[test]
fn content_below_minimum_is_lifted_to_it() {
    let bounds = content(0, 10.0, 40.0).bounds();
    assert!(approx(bounds.target, 40.0));
    assert!(approx(bounds.ceiling, 40.0));
}

#[test]
fn content_above_maximum_is_capped() {
    let bounds = content(0, 500.0, 40.0).with_max(220.0).bounds();
    assert!(approx(bounds.target, 220.0));
    assert!(approx(bounds.ceiling, 220.0));
}

#[test]
fn proportional_bounds_are_elastic_and_unbounded_by_default() {
    let bounds = proportional(0, 2.0, 60.0).bounds();
    assert!(approx(bounds.floor, 60.0));
    assert!(approx(bounds.target, 60.0));
    assert!(bounds.ceiling.is_infinite());
    assert!(approx(bounds.weight, 2.0));
    assert!(bounds.is_elastic());
}

#[test]
fn proportional_with_max_at_min_is_inert() {
    let spec = proportional(0, 1.0, 60.0).with_max(60.0);
    assert!(!spec.bounds().is_elastic());
}

#[test]
fn proportional_with_zero_weight_takes_nothing() {
    let layout = resolve_widths(&[proportional(0, 0.0, 10.0)], 500.0);
    assert!(
        approx(layout.width_at(0), 10.0),
        "a zero-weight column must hold its floor, not divide by zero"
    );
    assert!(approx(layout.slack(), 490.0));
}

#[test]
fn a_max_below_the_min_is_dropped_not_inverted() {
    let bounds = content(0, 300.0, 100.0).with_max(20.0).bounds();
    assert!(
        approx(bounds.target, 300.0),
        "an unusable ceiling must be dropped, not drag the target under the floor"
    );
    assert!(
        bounds.ceiling >= bounds.floor,
        "the ceiling must never sit below the floor"
    );
    assert!(approx(bounds.floor, 100.0));
}

#[test]
fn a_usable_max_below_the_preferred_caps_the_target() {
    let bounds = content(0, 300.0, 100.0).with_max(180.0).bounds();
    assert!(approx(bounds.target, 180.0));
    assert!(approx(bounds.ceiling, 180.0));
}

#[test]
fn non_finite_inputs_become_zero_rather_than_poisoning() {
    let specs = [
        ColumnSpec::fixed(0, "c", f32::NAN),
        ColumnSpec::fixed(1, "c", f32::INFINITY),
        content(2, -50.0, f32::NAN),
        proportional(3, f32::NAN, -10.0),
    ];
    let layout = resolve_widths(&specs, 400.0);
    for width in layout.widths() {
        assert!(width.is_finite(), "NaN or inf escaped into {width}");
        assert!(*width >= 0.0, "negative width {width}");
    }
    assert!(layout.total_width().is_finite());
}

#[test]
fn a_non_finite_viewport_is_treated_as_zero() {
    let layout = resolve_widths(&[content(0, 100.0, 40.0)], f32::NAN);
    assert!(approx(layout.available(), 0.0));
    assert!(layout.oversubscribed());
    assert!(approx(layout.width_at(0), 40.0));
}

#[test]
fn a_negative_viewport_is_treated_as_zero() {
    let layout = resolve_widths(&[content(0, 100.0, 40.0)], -250.0);
    assert!(approx(layout.available(), 0.0));
    assert!(layout.oversubscribed());
}

// ---------------------------------------------------------------------------
// WidthPolicy
// ---------------------------------------------------------------------------

#[test]
fn only_proportional_carries_weight() {
    assert!(approx(WidthPolicy::Fixed(10.0).weight(), 0.0));
    assert!(approx(WidthPolicy::Content(10.0).weight(), 0.0));
    assert!(approx(WidthPolicy::Proportional(3.0).weight(), 3.0));
    assert!(approx(WidthPolicy::Proportional(-4.0).weight(), 0.0));
    assert!(approx(WidthPolicy::Proportional(f32::NAN).weight(), 0.0));
}

#[test]
fn policy_and_regime_slugs_are_stable() {
    assert_eq!(WidthPolicy::Fixed(1.0).slug(), "fixed");
    assert_eq!(WidthPolicy::Content(1.0).slug(), "content");
    assert_eq!(WidthPolicy::Proportional(1.0).slug(), "proportional");
    assert_eq!(WidthRegime::Surplus.slug(), "surplus");
    assert_eq!(WidthRegime::Deficit.slug(), "deficit");
    assert_eq!(WidthRegime::Oversubscribed.slug(), "oversubscribed");
}

// ---------------------------------------------------------------------------
// Surplus regime
// ---------------------------------------------------------------------------

#[test]
fn surplus_is_shared_by_weight() {
    let specs = [
        proportional(0, 1.0, 0.0),
        proportional(1, 1.0, 0.0),
        proportional(2, 2.0, 0.0),
    ];
    let layout = resolve_widths(&specs, 400.0);
    assert_eq!(layout.regime(), WidthRegime::Surplus);
    assert!(approx(layout.width_at(0), 100.0));
    assert!(approx(layout.width_at(1), 100.0));
    assert!(approx(layout.width_at(2), 200.0));
    assert!(approx(layout.total_width(), 400.0));
    assert!(approx(layout.slack(), 0.0));
    assert!(!layout.needs_horizontal_scroll());
}

#[test]
fn a_capped_column_hands_its_share_back_and_the_others_take_it() {
    let specs = [
        proportional(0, 1.0, 0.0).with_max(50.0),
        proportional(1, 1.0, 0.0),
        proportional(2, 2.0, 0.0),
    ];
    let layout = resolve_widths(&specs, 400.0);
    assert!(
        approx(layout.width_at(0), 50.0),
        "col 0 must sit on its max"
    );
    // The remaining 350 splits 1:2 across cols 1 and 2.
    assert!(approx(layout.width_at(1), 116.6667));
    assert!(approx(layout.width_at(2), 233.3333));
    assert!(approx(layout.total_width(), 400.0));
}

#[test]
fn cascading_caps_settle_in_one_pass_per_column() {
    let specs = [
        proportional(0, 1.0, 0.0).with_max(50.0),
        proportional(1, 1.0, 0.0).with_max(150.0),
        proportional(2, 1.0, 0.0),
    ];
    let layout = resolve_widths(&specs, 600.0);
    assert!(approx(layout.width_at(0), 50.0));
    assert!(approx(layout.width_at(1), 150.0));
    assert!(approx(layout.width_at(2), 400.0));
    assert!(approx(layout.total_width(), 600.0));
}

#[test]
fn every_column_capped_leaves_the_remainder_as_slack() {
    let specs = [
        proportional(0, 1.0, 10.0).with_max(50.0),
        proportional(1, 1.0, 10.0).with_max(50.0),
    ];
    let layout = resolve_widths(&specs, 500.0);
    assert!(approx(layout.width_at(0), 50.0));
    assert!(approx(layout.width_at(1), 50.0));
    assert!(approx(layout.total_width(), 100.0));
    assert!(approx(layout.slack(), 400.0));
    assert!(
        !layout.needs_horizontal_scroll(),
        "an under-filled table scrolls vertically, not sideways"
    );
}

#[test]
fn rigid_and_content_columns_never_drift_from_their_target() {
    let specs = [
        ColumnSpec::fixed(0, "c", 80.0),
        content(1, 120.0, 30.0),
        proportional(2, 1.0, 20.0),
    ];
    let layout = resolve_widths(&specs, 600.0);
    assert!(approx(layout.width_at(0), 80.0));
    assert!(approx(layout.width_at(1), 120.0));
    assert!(approx(layout.width_at(2), 400.0));
    assert!(approx(layout.total_width(), 600.0));
}

#[test]
fn exact_fit_reports_surplus_with_no_slack() {
    let specs = [
        ColumnSpec::fixed(0, "c", 100.0),
        ColumnSpec::fixed(1, "c", 200.0),
    ];
    let layout = resolve_widths(&specs, 300.0);
    assert_eq!(layout.regime(), WidthRegime::Surplus);
    assert!(approx(layout.slack(), 0.0));
    assert!(approx(layout.total_width(), 300.0));
}

// ---------------------------------------------------------------------------
// Deficit regime
// ---------------------------------------------------------------------------

#[test]
fn deficit_scales_every_column_between_floor_and_target() {
    let specs = [content(0, 100.0, 40.0), content(1, 200.0, 60.0)];
    // F = 100, T = 300, available = 200 -> scale = 0.5
    let layout = resolve_widths(&specs, 200.0);
    assert_eq!(layout.regime(), WidthRegime::Deficit);
    assert!(approx(layout.width_at(0), 70.0));
    assert!(approx(layout.width_at(1), 130.0));
    assert!(approx(layout.total_width(), 200.0));
    assert!(!layout.needs_horizontal_scroll());
}

#[test]
fn deficit_lands_exactly_on_available_despite_float_drift() {
    // 1/3 cannot be represented; the residual must be absorbed, not lost.
    let specs = [
        content(0, 100.0, 0.0),
        content(1, 100.0, 0.0),
        content(2, 100.0, 0.0),
    ];
    let layout = resolve_widths(&specs, 100.0);
    assert_eq!(layout.regime(), WidthRegime::Deficit);
    assert!(
        approx(layout.total_width(), 100.0),
        "drift left the total at {}",
        layout.total_width()
    );
    assert!(
        !layout.needs_horizontal_scroll(),
        "sub-pixel drift must not be read as over-subscription"
    );
}

#[test]
fn deficit_never_puts_a_column_below_its_floor() {
    let specs = [
        content(0, 100.0, 90.0),
        content(1, 400.0, 20.0),
        content(2, 300.0, 30.0),
    ];
    let layout = resolve_widths(&specs, 141.0);
    for (index, spec) in specs.iter().enumerate() {
        assert!(
            layout.width_at(index) >= spec.min_width - TOL,
            "column {index} fell below its floor: {}",
            layout.width_at(index)
        );
    }
    assert!(approx(layout.total_width(), 141.0));
}

#[test]
fn available_exactly_equal_to_the_minimums_is_not_over_subscribed() {
    let specs = [content(0, 100.0, 40.0), content(1, 200.0, 60.0)];
    let layout = resolve_widths(&specs, 100.0);
    assert!(!layout.oversubscribed());
    assert!(approx(layout.width_at(0), 40.0));
    assert!(approx(layout.width_at(1), 60.0));
    assert!(!layout.needs_horizontal_scroll());
}

#[test]
fn a_rigid_table_narrower_than_its_columns_is_over_subscribed() {
    // F == T here, so the over-subscription check must run first.
    let specs = [
        ColumnSpec::fixed(0, "c", 200.0),
        ColumnSpec::fixed(1, "c", 200.0),
    ];
    let layout = resolve_widths(&specs, 150.0);
    assert_eq!(layout.regime(), WidthRegime::Oversubscribed);
    assert!(approx(layout.width_at(0), 200.0));
    assert!(approx(layout.width_at(1), 200.0));
    assert!(approx(layout.total_width(), 400.0));
    assert!(layout.needs_horizontal_scroll());
}

// ---------------------------------------------------------------------------
// Over-subscription
// ---------------------------------------------------------------------------

#[test]
fn over_subscribed_pins_every_column_at_its_floor() {
    let specs = [
        content(0, 100.0, 40.0),
        content(1, 200.0, 60.0),
        proportional(2, 1.0, 50.0),
    ];
    let layout = resolve_widths(&specs, 50.0);
    assert_eq!(layout.regime(), WidthRegime::Oversubscribed);
    assert!(approx(layout.width_at(0), 40.0));
    assert!(approx(layout.width_at(1), 60.0));
    assert!(approx(layout.width_at(2), 50.0));
    assert!(approx(layout.sum_minimums(), 150.0));
    assert!(approx(layout.total_width(), 150.0));
    assert!(layout.needs_horizontal_scroll());
    assert!(approx(layout.slack(), 0.0), "nothing was left over");
}

#[test]
fn over_subscription_is_reported_even_with_zero_minimums() {
    let layout = resolve_widths(&[ColumnSpec::fixed(0, "c", 10.0)], 0.0);
    assert!(layout.oversubscribed());
    assert!(approx(layout.width_at(0), 10.0));
}

// ---------------------------------------------------------------------------
// Termination
// ---------------------------------------------------------------------------

#[test]
fn the_solver_terminates_on_a_pathological_cap_ladder() {
    // Every column caps on a different pass, which is the worst case the
    // water-fill can be handed. If the loop were unbounded this hangs.
    let specs: Vec<ColumnSpec> = (0..64)
        .map(|index| proportional(index as ColId, 1.0, 1.0).with_max(20.0 + index as f32 * 10.0))
        .collect();
    let sum_of_caps: f32 = specs.iter().map(|spec| spec.max_width.unwrap_or(0.0)).sum();
    let layout = resolve_widths(&specs, 100_000.0);
    assert!(layout.total_width().is_finite());
    assert!(
        approx(layout.total_width(), sum_of_caps),
        "every column is capped, so the table must stop at the sum of the ceilings"
    );
    for (index, spec) in specs.iter().enumerate() {
        assert!(
            approx(layout.width_at(index), spec.max_width.unwrap_or(0.0)),
            "column {index} should sit on its ceiling"
        );
    }
    assert!(approx(
        layout.total_width(),
        layout.widths().iter().sum::<f32>()
    ));
    assert!(approx(layout.slack(), 100_000.0 - sum_of_caps));
}

#[test]
fn the_solver_terminates_on_a_huge_over_subscribed_table() {
    let specs: Vec<ColumnSpec> = (0..256)
        .map(|index| content(index as ColId, 400.0, 100.0))
        .collect();
    let layout = resolve_widths(&specs, 1.0);
    assert_eq!(layout.regime(), WidthRegime::Oversubscribed);
    assert!(approx(layout.total_width(), 25_600.0));
    assert!(layout.needs_horizontal_scroll());
}

#[test]
fn the_solver_is_pure_and_deterministic() {
    let specs = [
        ColumnSpec::fixed(0, "a", 80.0),
        content(1, 120.0, 30.0).with_max(110.0),
        proportional(2, 3.0, 20.0),
        proportional(3, 1.0, 20.0).with_max(40.0),
    ];
    for available in [0.0, 90.0, 250.0, 640.0, 4096.0] {
        let first = resolve_widths(&specs, available);
        let second = resolve_widths(&specs, available);
        assert_eq!(first, second, "same input must give the same layout");
    }
}

// ---------------------------------------------------------------------------
// Degenerate inputs
// ---------------------------------------------------------------------------

#[test]
fn an_empty_column_list_resolves_to_an_empty_layout() {
    let layout = resolve_widths(&[], 800.0);
    assert!(layout.is_empty());
    assert_eq!(layout.len(), 0);
    assert!(approx(layout.total_width(), 0.0));
    assert!(approx(layout.sum_minimums(), 0.0));
    assert!(!layout.needs_horizontal_scroll());
    assert_eq!(layout.index_of(0), None);
    assert!(approx(layout.width_at(0), 0.0));
    assert!(approx(layout.x_at(7), 0.0));
    assert!(approx(layout.divider_x(3), 0.0));
}

#[test]
fn the_default_layout_is_empty() {
    assert!(ColumnLayout::default().is_empty());
    assert_eq!(ColumnLayout::default().regime(), WidthRegime::Surplus);
}

// ---------------------------------------------------------------------------
// Offsets and identity
// ---------------------------------------------------------------------------

#[test]
fn offsets_are_the_prefix_sum_of_widths() {
    let specs = [
        ColumnSpec::fixed(7, "a", 80.0),
        content(9, 120.0, 30.0),
        proportional(11, 1.0, 20.0),
    ];
    let layout = resolve_widths(&specs, 600.0);
    assert_eq!(layout.ids(), &[7, 9, 11]);
    assert!(approx(layout.x_at(0), 0.0));
    assert!(approx(layout.x_at(1), 80.0));
    assert!(approx(layout.x_at(2), 200.0));
    assert!(approx(layout.divider_x(0), 80.0));
    assert!(approx(layout.divider_x(2), layout.total_width()));
    assert_eq!(layout.len(), 3);
    assert!(!layout.is_empty());
}

#[test]
fn identity_lookups_find_the_right_column() {
    let specs = [
        ColumnSpec::fixed(3, "a", 80.0),
        content(5, 120.0, 30.0),
        proportional(8, 1.0, 20.0),
    ];
    let layout = resolve_widths(&specs, 600.0);
    assert_eq!(layout.index_of(3), Some(0));
    assert_eq!(layout.index_of(5), Some(1));
    assert_eq!(layout.index_of(8), Some(2));
    assert_eq!(layout.index_of(4), None);
    assert!(approx(layout.width_of(5), 120.0));
    assert!(approx(layout.width_of(99), 0.0));
}

#[test]
fn offsets_stay_aligned_after_a_resize_re_solve() {
    // The resize gesture changes one spec and re-solves; the header and the
    // body read the same layout, so alignment is structural.
    let before = resolve_widths(
        &[content(0, 100.0, 40.0), proportional(1, 1.0, 40.0)],
        400.0,
    );
    let after = resolve_widths(
        &[content(0, 160.0, 40.0), proportional(1, 1.0, 40.0)],
        400.0,
    );
    assert!(approx(before.width_of(0), 100.0));
    assert!(approx(after.width_of(0), 160.0));
    assert!(approx(after.x_at(1), 160.0));
    assert!(approx(after.total_width(), 400.0));
}

// ---------------------------------------------------------------------------
// CellAlign::place
// ---------------------------------------------------------------------------

#[test]
fn start_alignment_sits_on_the_leading_edge() {
    assert!(approx(CellAlign::Start.place(10.0, 100.0, 40.0), 10.0));
}

#[test]
fn end_alignment_sits_on_the_trailing_edge() {
    assert!(approx(CellAlign::End.place(10.0, 100.0, 40.0), 70.0));
}

#[test]
fn centre_alignment_splits_the_spare() {
    assert!(approx(CellAlign::Center.place(10.0, 100.0, 40.0), 40.0));
}

#[test]
fn overflowing_content_starts_at_the_box_whatever_the_alignment() {
    // Truncating toward the alignment would hide the *beginning* of a long
    // string, which is the wrong end to lose.
    for align in [CellAlign::Start, CellAlign::Center, CellAlign::End] {
        assert!(
            approx(align.place(10.0, 30.0, 90.0), 10.0),
            "{align:?} shifted overflowing content out of its box"
        );
    }
}

#[test]
fn zero_width_box_and_zero_content_stay_inside() {
    for align in [CellAlign::Start, CellAlign::Center, CellAlign::End] {
        assert!(approx(align.place(5.0, 0.0, 0.0), 5.0));
    }
}

#[test]
fn default_alignment_is_start() {
    assert_eq!(CellAlign::default(), CellAlign::Start);
    assert_eq!(CellAlign::Start.slug(), "start");
    assert_eq!(CellAlign::Center.slug(), "center");
    assert_eq!(CellAlign::End.slug(), "end");
}

// ---------------------------------------------------------------------------
// The fastmd six-column shape
// ---------------------------------------------------------------------------

#[test]
fn the_tools_dialog_six_column_shape_resolves_at_a_dialog_width() {
    // The migration target from `src/app/ui_iced/tools_dialog.rs`:
    // checkbox Shrink / group Fill / kind Shrink / tools Fill /
    // char-count+chip Shrink / actions Fill.
    let specs = [
        ColumnSpec::fixed(0, "", 28.0),
        proportional(1, 3.0, 120.0),
        content(2, 84.0, 60.0),
        proportional(3, 5.0, 160.0),
        content(4, 132.0, 96.0),
        proportional(5, 2.0, 140.0),
    ];
    let layout = resolve_widths(&specs, 720.0);
    assert_eq!(layout.len(), 6);
    assert!(approx(layout.width_at(0), 28.0));
    assert!(approx(layout.width_at(2), 84.0));
    assert!(approx(layout.width_at(4), 132.0));
    assert!(approx(layout.total_width(), 720.0));
    assert!(!layout.needs_horizontal_scroll());
    // The three Shrink columns keep exactly what they asked for, and the
    // three Fill columns hold their minimum and share the surplus above
    // Σ targets in their 3:5:2 ratio. That is the dialog's behaviour:
    // the description columns grow, the fixed ones do not.
    let sum_targets = 28.0 + 120.0 + 84.0 + 160.0 + 132.0 + 140.0;
    let surplus = 720.0 - sum_targets;
    assert!(approx(layout.width_at(1), 120.0 + surplus * 3.0 / 10.0));
    assert!(approx(layout.width_at(3), 160.0 + surplus * 5.0 / 10.0));
    assert!(approx(layout.width_at(5), 140.0 + surplus * 2.0 / 10.0));
}

#[test]
fn the_tools_dialog_shape_over_subscribes_before_it_loses_a_column() {
    let specs = [
        ColumnSpec::fixed(0, "", 28.0),
        proportional(1, 3.0, 120.0),
        content(2, 84.0, 60.0),
        proportional(3, 5.0, 160.0),
        content(4, 132.0, 96.0),
        proportional(5, 2.0, 140.0),
    ];
    let layout = resolve_widths(&specs, 300.0);
    assert!(layout.oversubscribed());
    assert!(approx(
        layout.total_width(),
        28.0 + 120.0 + 60.0 + 160.0 + 96.0 + 140.0
    ));
    assert!(layout.needs_horizontal_scroll());
    for (index, spec) in specs.iter().enumerate() {
        assert!(layout.width_at(index) >= spec.min_width - TOL);
    }
}
