//! Tests for [`super::tabular`] — the tabular-numerals flag and the
//! boundary it cannot cross (`UIF-PRIN-007`).

use super::*;
use crate::token::typography::{Family, Weight};

fn readout(kind: ReadoutKind, tabular: TabularNums) -> NumericReadout<'static> {
    NumericReadout {
        id: "readout",
        kind,
        tabular,
        header_right_aligned: false,
        cell_right_aligned: false,
    }
}

fn column(header: bool, cell: bool) -> NumericReadout<'static> {
    NumericReadout {
        id: "amount",
        kind: ReadoutKind::NumericColumn,
        tabular: TabularNums::Required,
        header_right_aligned: header,
        cell_right_aligned: cell,
    }
}

// ---------------------------------------------------------------------------
// The flag resolves to a real token
// ---------------------------------------------------------------------------

#[test]
fn test_required_resolves_to_the_mono_family() {
    assert_eq!(TabularNums::Required.family(), Some(Family::Mono));
}

#[test]
fn test_not_required_resolves_to_no_family() {
    assert_eq!(TabularNums::NotRequired.family(), None);
}

#[test]
fn test_the_resolved_family_is_the_declared_mono_token() {
    let family = TabularNums::Required
        .family()
        .expect("Required must resolve");
    assert!(family.is_monospace());
    assert_eq!(family.token_name(), "font.family.mono");
}

#[test]
fn test_the_resolution_chain_reaches_an_iced_font() {
    // The whole point of this test: the mono route *exists*. What it
    // reaches is iced's generic Monospace, and `iced::Font` has four
    // fields — family, weight, stretch, style — with no OpenType
    // feature selector anywhere for `font-variant-numeric: tabular-nums`
    // to live in.
    let font = TabularNums::Required
        .family()
        .expect("Required must resolve")
        .font(Weight::Regular);
    assert_eq!(font.family, iced::font::Family::Monospace);
    assert_eq!(font.stretch, iced::font::Stretch::Normal);
    assert_eq!(font.style, iced::font::Style::Normal);
}

#[test]
fn test_flag_predicates_and_rungs() {
    assert!(TabularNums::Required.is_required());
    assert!(!TabularNums::NotRequired.is_required());
    assert_eq!(
        TabularNums::ALL,
        [TabularNums::Required, TabularNums::NotRequired]
    );
}

#[test]
fn test_flag_slugs_are_unique() {
    let slugs: Vec<&str> = TabularNums::ALL.iter().map(|t| t.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), unique.len(), "slugs collide: {slugs:?}");
}

// ---------------------------------------------------------------------------
// Readout kinds
// ---------------------------------------------------------------------------

#[test]
fn test_every_kind_requires_tabular() {
    for kind in ReadoutKind::ALL {
        assert!(
            kind.requires_tabular(),
            "{} is a live or aligned numeric value and must be tabular",
            kind.slug()
        );
    }
}

#[test]
fn test_only_a_numeric_column_carries_the_alignment_mandate() {
    for kind in ReadoutKind::ALL {
        assert_eq!(
            kind.must_right_align(),
            kind == ReadoutKind::NumericColumn,
            "{} must not carry the column alignment mandate",
            kind.slug()
        );
    }
}

#[test]
fn test_readout_kind_slugs_are_unique() {
    let slugs: Vec<&str> = ReadoutKind::ALL.iter().map(|k| k.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), unique.len(), "slugs collide: {slugs:?}");
}

// ---------------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------------

#[test]
fn test_a_tabular_non_column_readout_is_clean() {
    for kind in [
        ReadoutKind::Counter,
        ReadoutKind::Percentage,
        ReadoutKind::Timestamp,
        ReadoutKind::PaginationRange,
    ] {
        assert!(
            check_numeric_readout(&readout(kind, TabularNums::Required)).is_empty(),
            "{:?} should pass",
            kind
        );
    }
}

#[test]
fn test_a_non_tabular_readout_is_flagged_for_every_kind() {
    for kind in ReadoutKind::ALL {
        let violations = check_numeric_readout(&readout(kind, TabularNums::NotRequired));
        assert_eq!(
            violations[0],
            TabularViolation::NotTabular {
                id: "readout",
                kind
            },
            "{kind:?} declared NotRequired and must be reported"
        );
        // The helper leaves both alignment flags false, so only a numeric
        // column owes the two extra findings.
        let expected = if kind.must_right_align() { 3 } else { 1 };
        assert_eq!(violations.len(), expected, "{kind:?} → {violations:?}");
    }
}

#[test]
fn test_a_fully_aligned_numeric_column_is_clean() {
    assert!(check_numeric_readout(&column(true, true)).is_empty());
}

#[test]
fn test_a_numeric_column_with_a_left_aligned_header_is_flagged() {
    let violations = check_numeric_readout(&column(false, true));
    assert_eq!(
        violations,
        vec![TabularViolation::HeaderNotRightAligned { id: "amount" }]
    );
}

#[test]
fn test_a_numeric_column_with_left_aligned_cells_is_flagged() {
    let violations = check_numeric_readout(&column(true, false));
    assert_eq!(
        violations,
        vec![TabularViolation::CellNotRightAligned { id: "amount" }]
    );
}

#[test]
fn test_both_misalignments_are_reported_together() {
    let violations = check_numeric_readout(&column(false, false));
    assert_eq!(
        violations,
        vec![
            TabularViolation::HeaderNotRightAligned { id: "amount" },
            TabularViolation::CellNotRightAligned { id: "amount" },
        ]
    );
}

#[test]
fn test_alignment_fields_are_inert_for_non_column_kinds() {
    // A counter has no header to align, so both fields being false must
    // not produce an alignment finding.
    let mut model = readout(ReadoutKind::Counter, TabularNums::Required);
    model.header_right_aligned = false;
    model.cell_right_aligned = false;
    assert!(check_numeric_readout(&model).is_empty());
}

#[test]
fn test_a_non_tabular_numeric_column_reports_both_problems() {
    let model = NumericReadout {
        id: "amount",
        kind: ReadoutKind::NumericColumn,
        tabular: TabularNums::NotRequired,
        header_right_aligned: false,
        cell_right_aligned: false,
    };
    let violations = check_numeric_readout(&model);
    assert_eq!(violations.len(), 3, "{violations:?}");
    assert!(matches!(
        violations[0],
        TabularViolation::NotTabular {
            id: "amount",
            kind: ReadoutKind::NumericColumn
        }
    ));
}

#[test]
fn test_violations_render_readably() {
    for violation in [
        TabularViolation::NotTabular {
            id: "counter",
            kind: ReadoutKind::Counter,
        },
        TabularViolation::HeaderNotRightAligned { id: "amount" },
        TabularViolation::CellNotRightAligned { id: "amount" },
    ] {
        let text = violation.to_string();
        assert!(text.len() > 30, "message too thin: {text}");
    }
}
