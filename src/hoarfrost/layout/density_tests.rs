//! Tests for [`super::density`] — the row-density homogeneity rule
//! (`UIF-PRIN-006`).

use super::*;
use crate::token::size;

// ---------------------------------------------------------------------------
// The rungs are derived, never restated
// ---------------------------------------------------------------------------

#[test]
fn test_compact_rung_is_the_control_sm_token() {
    assert_eq!(ControlDensity::Compact.height(), size::CONTROL_SM);
}

#[test]
fn test_standard_rung_is_the_control_md_token() {
    assert_eq!(ControlDensity::Standard.height(), size::CONTROL_MD);
}

#[test]
fn test_rungs_are_distinct_and_ordered_compact_first() {
    assert_ne!(
        ControlDensity::Compact.height(),
        ControlDensity::Standard.height()
    );
    assert!(ControlDensity::Compact < ControlDensity::Standard);
}

#[test]
fn test_all_lists_both_rungs_ascending() {
    assert_eq!(
        ControlDensity::ALL,
        [ControlDensity::Compact, ControlDensity::Standard]
    );
}

#[test]
fn test_from_height_round_trips_every_rung() {
    for rung in ControlDensity::ALL {
        assert_eq!(ControlDensity::from_height(rung.height()), Some(rung));
    }
}

#[test]
fn test_from_height_rejects_a_height_on_no_rung() {
    for height in [0.0, 1.0, 24.0, 27.99, 28.01, 30.0, 31.99, 32.01, 40.0] {
        assert_eq!(
            ControlDensity::from_height(height),
            None,
            "{height}px must not resolve to a rung"
        );
    }
}

#[test]
fn test_slugs_are_unique_per_rung() {
    let slugs: Vec<&str> = ControlDensity::ALL.iter().map(|r| r.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), unique.len(), "rung slugs collide: {slugs:?}");
    assert_eq!(ControlDensity::Compact.slug(), "compact");
    assert_eq!(ControlDensity::Standard.slug(), "standard");
}

#[test]
fn test_is_compact_only_for_the_dense_rung() {
    assert!(ControlDensity::Compact.is_compact());
    assert!(!ControlDensity::Standard.is_compact());
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

#[test]
fn test_row_slot_at_density_reads_the_token() {
    let slot = RowSlot::at_density("save", ControlDensity::Compact);
    assert_eq!(slot.height, size::CONTROL_SM);
    assert_eq!(slot.rung(), Some(ControlDensity::Compact));
}

#[test]
fn test_row_slot_off_ladder_reports_no_rung() {
    let slot = RowSlot::new("legacy", 24.0);
    assert_eq!(slot.rung(), None);
}

// ---------------------------------------------------------------------------
// The band list is the spec's own
// ---------------------------------------------------------------------------

#[test]
fn test_every_forbidden_mixing_band_is_modelled() {
    let slugs: Vec<&str> = DensityBand::ALL.iter().map(|b| b.slug()).collect();
    for expected in [
        "navbar",
        "form-row",
        "overflow-toolbar",
        "filter-bar",
        "dialog-footer",
    ] {
        assert!(
            slugs.contains(&expected),
            "§1.1 names a band this check cannot report: {expected} (have {slugs:?})"
        );
    }
    assert_eq!(DensityBand::ALL.len(), 5, "§1.1 names exactly five bands");
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

#[test]
fn test_homogeneous_band_passes() {
    let slots = [
        RowSlot::at_density("a", ControlDensity::Compact),
        RowSlot::at_density("b", ControlDensity::Compact),
        RowSlot::at_density("c", ControlDensity::Compact),
    ];
    let report = check_row_density(DensityBand::Navbar, ControlDensity::Compact, &slots);
    assert!(report.is_homogeneous(), "{report}");
    assert!(!report.mixes_rungs());
    assert_eq!(report.off_rung_count(), 0);
    assert_eq!(report.rung_spread, vec![ControlDensity::Compact]);
}

#[test]
fn test_mixed_rungs_name_the_offender_and_both_values() {
    let slots = [
        RowSlot::at_density("keep", ControlDensity::Compact),
        RowSlot::at_density("intruder", ControlDensity::Standard),
    ];
    let report = check_row_density(DensityBand::Navbar, ControlDensity::Compact, &slots);

    assert!(!report.is_homogeneous());
    assert!(report.mixes_rungs());
    assert_eq!(report.violations.len(), 1);

    let violation = &report.violations[0];
    assert_eq!(violation.control, "intruder");
    assert_eq!(violation.band, DensityBand::Navbar);
    assert_eq!(violation.declared, ControlDensity::Compact);
    assert_eq!(violation.height, size::CONTROL_MD);
    assert_eq!(violation.rung, Some(ControlDensity::Standard));
    assert!(!violation.is_off_rung());

    let text = violation.to_string();
    assert!(text.contains("intruder"), "{text}");
    assert!(text.contains("standard"), "{text}");
    assert!(text.contains("compact"), "{text}");
}

#[test]
fn test_every_offending_control_is_named_not_just_counted() {
    let slots = [
        RowSlot::at_density("a", ControlDensity::Compact),
        RowSlot::at_density("b", ControlDensity::Standard),
        RowSlot::at_density("c", ControlDensity::Standard),
    ];
    let report = check_row_density(DensityBand::FilterBar, ControlDensity::Compact, &slots);
    let offenders: Vec<&str> = report.violations.iter().map(|v| v.control).collect();
    assert_eq!(offenders, vec!["b", "c"]);
}

#[test]
fn test_off_rung_control_is_flagged_as_different_from_wrong_rung() {
    let slots = [
        RowSlot::at_density("ok", ControlDensity::Compact),
        RowSlot::new("legacy", size::HIT_TARGET_MIN),
    ];
    let report = check_row_density(DensityBand::Navbar, ControlDensity::Compact, &slots);

    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.off_rung_count(), 1);
    assert!(report.violations[0].is_off_rung());
    assert_eq!(
        report.rung_spread,
        vec![ControlDensity::Compact],
        "an off-rung control contributes no rung to the spread"
    );
    assert!(
        !report.mixes_rungs(),
        "one real rung plus one off-ladder height is not a 28/32 mixture"
    );
}

#[test]
fn test_consistent_but_misdeclared_band_is_a_violation_that_is_not_mixing() {
    let slots = [
        RowSlot::at_density("a", ControlDensity::Compact),
        RowSlot::at_density("b", ControlDensity::Compact),
    ];
    let report = check_row_density(DensityBand::Navbar, ControlDensity::Standard, &slots);

    assert!(!report.is_homogeneous(), "the band is not what it declares");
    assert!(
        !report.mixes_rungs(),
        "but nothing is mixed: every control agrees with every other"
    );
    assert_eq!(report.rung_spread, vec![ControlDensity::Compact]);
    assert_eq!(report.violations.len(), 2);
}

#[test]
fn test_empty_band_is_homogeneous() {
    let report = check_row_density(DensityBand::FormRow, ControlDensity::Standard, &[]);
    assert!(report.is_homogeneous());
    assert!(!report.mixes_rungs());
    assert!(report.rung_spread.is_empty());
}

#[test]
fn test_rung_spread_is_sorted_and_deduplicated() {
    let slots = [
        RowSlot::at_density("a", ControlDensity::Standard),
        RowSlot::at_density("b", ControlDensity::Compact),
        RowSlot::at_density("c", ControlDensity::Standard),
        RowSlot::at_density("d", ControlDensity::Compact),
    ];
    let report = check_row_density(DensityBand::FilterBar, ControlDensity::Compact, &slots);
    assert_eq!(
        report.rung_spread,
        vec![ControlDensity::Compact, ControlDensity::Standard]
    );
}

#[test]
fn test_report_display_carries_band_spread_and_offenders() {
    let slots = [RowSlot::at_density("intruder", ControlDensity::Standard)];
    let report = check_row_density(DensityBand::FilterBar, ControlDensity::Compact, &slots);
    let text = report.to_string();
    assert!(text.contains("filter-bar"), "{text}");
    assert!(text.contains("intruder"), "{text}");
    assert!(text.contains("compact"), "{text}");
    assert!(text.contains("standard"), "{text}");
}

#[test]
fn test_homogeneous_report_display_says_so() {
    let slots = [RowSlot::at_density("a", ControlDensity::Standard)];
    let report = check_row_density(DensityBand::Navbar, ControlDensity::Standard, &slots);
    assert!(report.to_string().contains("homogeneous"), "{report}");
}

// ---------------------------------------------------------------------------
// Wiring to the composites §1.1 names
// ---------------------------------------------------------------------------

#[test]
fn test_overflow_toolbar_strip_declares_a_real_rung() {
    assert_eq!(
        overflow_toolbar_declared_density(),
        Some(ControlDensity::Compact),
        "the toolbar strip's own STRIP_HEIGHT must remain on the density ladder"
    );
}

#[test]
fn test_form_field_control_slot_declares_a_real_rung() {
    assert_eq!(
        form_field_declared_density(),
        Some(ControlDensity::Standard),
        "the form field's own CONTROL_SLOT_HEIGHT must remain on the density ladder"
    );
}

#[test]
fn test_check_overflow_toolbar_catches_a_mixed_strip() {
    let controls = [
        RowSlot::at_density("bold", ControlDensity::Compact),
        RowSlot::at_density("paste", ControlDensity::Standard),
    ];
    let report = check_overflow_toolbar(&controls).expect("the strip declares a rung");
    assert_eq!(report.band, DensityBand::OverflowToolbar);
    assert_eq!(report.declared, ControlDensity::Compact);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].control, "paste");
}

#[test]
fn test_check_form_field_row_catches_a_compact_control_in_a_standard_field() {
    let controls = [RowSlot::at_density("name", ControlDensity::Compact)];
    let report = check_form_field_row(&controls).expect("the field declares a rung");
    assert_eq!(report.band, DensityBand::FormRow);
    assert_eq!(report.declared, ControlDensity::Standard);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].control, "name");
}

#[test]
fn test_check_form_field_row_passes_a_matching_control() {
    let controls = [RowSlot::at_density("name", ControlDensity::Standard)];
    let report = check_form_field_row(&controls).expect("the field declares a rung");
    assert!(report.is_homogeneous(), "{report}");
}
