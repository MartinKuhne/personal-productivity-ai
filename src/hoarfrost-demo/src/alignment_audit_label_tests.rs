//! Unit tests for the label rules and the label-to-control pairing.
//!
//! Split out of `alignment_audit_tests.rs`: the label half of the audit
//! asks a different question from the control half. R1-R5 compare
//! controls to controls; these compare a label to the thing it labels.
//! The shared fixture helpers (`entry`, `control`, `card`, `audit`,
//! `fired`, `silent`) come from the parent test module through `super`.
//!
//! Same discipline as the control rules: every rule gets a fixture that
//! must make it fire and a fixture that must leave it silent.

use super::*;

use crate::section::{DEMO_LABEL_ID_SUFFIX, ROW_ID_SUFFIX};

/// A label. The probe's only label signal is the `text` kind.
fn label(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    entry(id, "text", x, y, w, h)
}

/// A form card every label fixture is built inside, so ownership resolves
/// to a named section rather than `chrome`.
fn form() -> Entry {
    card("demo.section.form", 0.0, 0.0, 600.0, 400.0)
}

/// The single row pair a one-label fixture should produce.
fn only_pair(entries: &[Entry]) -> pairing::Pair {
    let records = deduplicate(entries);
    let sections = sections_of(&records);
    let pairs = pairing::pair_labels(&records, &sections).pairs;
    assert_eq!(
        pairs.len(),
        1,
        "the fixture is meant to produce exactly one pair"
    );
    pairs.into_iter().next().expect("checked above")
}

// ---------------------------------------------------------------------------
// The pairing itself — every label rule reads this, so it gets its own
// controls
// ---------------------------------------------------------------------------

#[test]
fn pairing_binds_a_label_to_the_control_sharing_its_row() {
    let pair = only_pair(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);

    assert_eq!(pair.label.id, "name");
    assert_eq!(pair.control.id, "field");
    assert_eq!(pair.kind, pairing::PairKind::Row);
    assert_eq!(pair.owner, "demo.section.form");
    assert_eq!(pair.gap(), 8.0);
    assert_eq!(pair.label_centre(), 29.0);
    assert_eq!(pair.control_centre(), 32.0);
    assert_eq!(pair.centre_delta(), -3.0, "the label sits 3px high");
    assert_eq!(
        pair.overlap_extent(),
        (0.0, 18.0),
        "a row pair always overlaps vertically; R9 needs both axes"
    );
}

#[test]
fn pairing_binds_a_label_to_the_control_directly_below_it() {
    let pair = only_pair(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 16.0, 42.0, 200.0, 32.0),
    ]);

    assert_eq!(pair.kind, pairing::PairKind::Below);
    assert_eq!(pair.vertical_gap(), 4.0);
}

#[test]
fn pairing_prefers_the_row_partner_over_a_control_below() {
    let records = deduplicate(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("row", 84.0, 16.0, 100.0, 32.0),
        control("under", 16.0, 44.0, 200.0, 32.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert_eq!(pairing.pairs.len(), 1);
    assert_eq!(pairing.pairs[0].control.id, "row", "the row wins");
}

#[test]
fn pairing_never_binds_a_control_to_its_own_caption() {
    let records = deduplicate(&[
        form(),
        control("field", 16.0, 20.0, 200.0, 32.0),
        label("caption", 24.0, 27.0, 60.0, 18.0),
        label("heading", 16.0, 200.0, 200.0, 24.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert!(
        pairing.pairs.is_empty(),
        "a caption inside its control's box labels nothing"
    );
    assert_eq!(
        pairing.orphans.len(),
        1,
        "the caption is not a label at all, so it is not even an orphan"
    );
    assert_eq!(pairing.orphans[0].id, "heading");
}

#[test]
fn pairing_refuses_a_control_that_is_not_immediately_to_the_right() {
    let records = deduplicate(&[
        form(),
        label("title", 16.0, 20.0, 60.0, 18.0),
        control("far", 400.0, 16.0, 100.0, 32.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert!(
        pairing.pairs.is_empty(),
        "324px of `Fill` is not a label attached to a control"
    );
    assert_eq!(pairing.orphans.len(), 1);
}

#[test]
fn pairing_refuses_a_control_further_below_than_a_label_owns() {
    let records = deduplicate(&[
        form(),
        label("title", 16.0, 20.0, 60.0, 18.0),
        control("lower", 16.0, 80.0, 200.0, 32.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert!(pairing.pairs.is_empty());
    assert_eq!(pairing.orphans.len(), 1);
}

#[test]
fn pairing_does_not_reach_across_sections() {
    let records = deduplicate(&[
        card("demo.section.a", 0.0, 0.0, 280.0, 400.0),
        card("demo.section.b", 300.0, 0.0, 280.0, 400.0),
        label("orphan", 16.0, 20.0, 60.0, 18.0),
        control("elsewhere", 316.0, 16.0, 100.0, 32.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert!(
        pairing.pairs.is_empty(),
        "a control in another card cannot be this label's control"
    );
}

// ---------------------------------------------------------------------------
// R6 — label vertical alignment
// ---------------------------------------------------------------------------

#[test]
fn r6_fires_when_a_label_sits_high_beside_its_control() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelVerticalAlignment);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].subject, "demo.section.form@label:field");
    assert!(
        findings[0].detail.contains("off by 3.00px"),
        "the detail must carry the exact centre delta: {}",
        findings[0].detail
    );
}

#[test]
fn r6_stays_silent_on_a_centred_label() {
    let report = audit(&[
        form(),
        label("name", 16.0, 23.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelVerticalAlignment);
}

#[test]
fn r6_measures_a_two_line_label_as_one_block() {
    let report = audit(&[
        form(),
        label("title", 16.0, 22.0, 120.0, 20.0),
        label("subtitle", 16.0, 42.0, 120.0, 16.0),
        control("toggle", 144.0, 20.0, 100.0, 40.0),
    ]);

    silent(&report, Rule::LabelVerticalAlignment);
    // title 22..42 + subtitle 42..58 -> block centre 40; toggle 20..60 -> centre 40.
}

#[test]
fn r6_fires_on_a_two_line_block_that_is_off_centre() {
    let report = audit(&[
        form(),
        label("title", 16.0, 28.0, 120.0, 20.0),
        label("subtitle", 16.0, 48.0, 120.0, 16.0),
        control("toggle", 144.0, 20.0, 100.0, 40.0),
    ]);
    let findings = fired(&report, Rule::LabelVerticalAlignment);

    assert_eq!(findings.len(), 1, "one block, one finding");
    assert!(
        findings[0].detail.contains("label block of 2"),
        "{}",
        findings[0].detail
    );
    assert!(
        findings[0].detail.contains("off by 6.00px"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r6_attributes_an_equal_height_misalignment_to_the_demo() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 32.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelVerticalAlignment);

    assert_eq!(
        findings[0].attribution,
        Attribution::Demo,
        "two equal-height boxes with different centres were placed apart"
    );
}

#[test]
fn r6_attributes_an_unequal_height_misalignment_to_undetermined() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelVerticalAlignment);

    assert_eq!(
        findings[0].attribution,
        Attribution::Undetermined,
        "the block is not the control's height, so either box's size could explain it"
    );
}

// ---------------------------------------------------------------------------
// R8 — label↔control gap
// ---------------------------------------------------------------------------

#[test]
fn r8_fires_on_an_off_token_label_gap() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 81.0, 16.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelControlGap);

    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].detail.contains("gap of 5.00px"),
        "{}",
        findings[0].detail
    );
    assert!(
        findings[0]
            .detail
            .contains("nearest xs = 4.00px, off by 1.00px"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r8_stays_silent_on_a_token_label_gap() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelControlGap);
    assert_eq!(
        space::SM,
        84.0 - (16.0 + 60.0),
        "the fixture gap is space::SM"
    );
}

#[test]
fn r8_treats_a_flush_label_as_conformant() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 76.0, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelControlGap);
}

// ---------------------------------------------------------------------------
// R9 — label / control overlap
// ---------------------------------------------------------------------------

#[test]
fn r9_fires_when_a_control_is_drawn_over_its_label() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 60.0, 16.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelControlOverlap);

    assert_eq!(findings.len(), 1);
    assert!(
        findings[0]
            .detail
            .contains("overlaps field by 16.00px x 18.00px"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r9_stays_silent_when_the_pair_only_grazes() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 75.6, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelControlOverlap);
}

#[test]
fn r9_stays_silent_on_a_clean_adjacent_pair() {
    let report = audit(&[
        form(),
        label("name", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelControlOverlap);
}

#[test]
fn r9_catches_a_label_crushed_by_a_control_below_it() {
    let pair = only_pair(&[
        form(),
        label("name", 16.0, 20.0, 200.0, 18.0),
        control("field", 16.0, 30.0, 200.0, 32.0),
    ]);
    assert_eq!(pair.kind, pairing::PairKind::Below);

    let findings = label_rules::check_label_overlap(&pair);
    assert_eq!(findings.len(), 1, "the below pair still overlaps");
    assert!(
        findings[0].detail.contains("[below pair]"),
        "{}",
        findings[0].detail
    );
}

// ---------------------------------------------------------------------------
// R10 — label column alignment within a section
// ---------------------------------------------------------------------------

#[test]
fn r10_fires_when_no_label_edge_collapses_to_one_guide() {
    let report = audit(&[
        form(),
        label("a", 16.0, 20.0, 60.0, 18.0),
        control("fa", 84.0, 16.0, 100.0, 32.0),
        label("b", 22.0, 60.0, 70.0, 18.0),
        control("fb", 100.0, 56.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelColumnAlignment);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].subject, "demo.section.form");
    assert!(
        findings[0].detail.contains("left edge spreads 6.00px"),
        "{}",
        findings[0].detail
    );
    assert!(
        findings[0].detail.contains("right edge spreads 16.00px"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r10_accepts_a_left_aligned_label_column() {
    let report = audit(&[
        form(),
        label("a", 16.0, 20.0, 60.0, 18.0),
        control("fa", 84.0, 16.0, 100.0, 32.0),
        label("b", 16.0, 60.0, 70.0, 18.0),
        control("fb", 94.0, 56.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelColumnAlignment);
}

#[test]
fn r10_accepts_a_right_aligned_label_column() {
    let report = audit(&[
        form(),
        label("a", 16.0, 20.0, 60.0, 18.0),
        control("fa", 84.0, 16.0, 100.0, 32.0),
        label("b", 6.0, 60.0, 70.0, 18.0),
        control("fb", 84.0, 56.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelColumnAlignment);
}

#[test]
fn r10_needs_two_label_rows_before_it_can_call_a_column() {
    let report = audit(&[
        form(),
        label("a", 16.0, 20.0, 60.0, 18.0),
        control("fa", 84.0, 16.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::LabelColumnAlignment);
}

#[test]
fn r10_attributes_uniform_labels_to_the_demo() {
    let report = audit(&[
        form(),
        label("a", 16.0, 20.0, 60.0, 18.0),
        control("fa", 84.0, 16.0, 100.0, 32.0),
        label("b", 22.0, 60.0, 60.0, 18.0),
        control("fb", 90.0, 56.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::LabelColumnAlignment);

    assert_eq!(
        findings[0].attribution,
        Attribution::Demo,
        "identical labels cannot be why their column drifted"
    );
}

// ---------------------------------------------------------------------------
// Orphans are counted, never reported
// ---------------------------------------------------------------------------

#[test]
fn an_unpaired_label_is_counted_and_does_not_dirty_the_report() {
    let report = audit(&[
        form(),
        label("heading", 16.0, 20.0, 200.0, 24.0),
        label("prose", 16.0, 60.0, 400.0, 18.0),
    ]);

    assert_eq!(report.orphans, 2);
    assert!(
        report.is_clean(),
        "a heading is not a misalignment:\n{}",
        report.render()
    );
    assert!(report.render().contains("orphan labels 2"));
}

// ---------------------------------------------------------------------------
// Declared rows — the demo says so, so the geometry does not have to guess
// ---------------------------------------------------------------------------

/// A row the demo declared with [`ROW_ID_SUFFIX`], wrapping label and control.
fn declared_row(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    entry(&format!("{id}{ROW_ID_SUFFIX}"), "container", x, y, w, h)
}

/// A label the demo authored itself, under [`DEMO_LABEL_ID_SUFFIX`].
fn named_label(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    entry(
        &format!("{id}{DEMO_LABEL_ID_SUFFIX}"),
        "container",
        x,
        y,
        w,
        h,
    )
}

/// Resolves the declared rows for a fixture.
///
/// Takes the entries by value so the returned rows borrow records that
/// outlive them in every caller.
fn declared(entries: Vec<Entry>) -> (Vec<Record>, Vec<Record>, Vec<String>) {
    let records = deduplicate(&entries);
    let sections = sections_of(&records);
    let rows = declared_rows::find(&records, &sections);
    let ids = rows
        .iter()
        .map(|row| format!("{}->{}", row.labels.len(), row.control.id))
        .collect();
    (records, sections, ids)
}

#[test]
fn a_declared_row_binds_its_label_to_its_single_control() {
    let entries = vec![
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 28.0, 120.0, 20.0),
        control("toggle", 400.0, 38.0, 104.0, 24.0),
    ];
    let records = deduplicate(&entries);
    let sections = sections_of(&records);
    let rows = declared_rows::find(&records, &sections);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].control.id, "toggle");
    assert_eq!(rows[0].owner, "demo.section.form");
    assert_eq!(rows[0].labels.len(), 1);

    let (pairs, consumed) = declared_rows::pairs(&rows);
    assert_eq!(pairs.len(), 1);
    assert_eq!(
        pairs[0].kind,
        pairing::PairKind::Filled,
        "a declared row's gap is whatever Fill left, so it is not a Row pair"
    );
    assert_eq!(pairs[0].gap(), 256.0);
    assert_eq!(consumed.len(), 1);
    assert_eq!(consumed.iter().next().expect("checked above"), "title");
}

#[test]
fn a_declared_row_with_two_controls_binds_nothing() {
    let (_, _, ids) = declared(vec![
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 28.0, 120.0, 20.0),
        control("left", 400.0, 38.0, 40.0, 24.0),
        control("right", 460.0, 38.0, 40.0, 24.0),
    ]);

    assert!(
        ids.is_empty(),
        "a row holding two controls cannot say which the words belong to"
    );
}

#[test]
fn a_declared_row_with_no_control_binds_nothing() {
    let (_, _, ids) = declared(vec![
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 28.0, 120.0, 20.0),
    ]);

    assert!(ids.is_empty(), "there is nothing in the row to label");
}

#[test]
fn a_controls_own_caption_is_not_its_rows_label() {
    let (_, _, ids) = declared(vec![
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        control("toggle", 400.0, 38.0, 104.0, 24.0),
        label("caption", 410.0, 42.0, 60.0, 16.0),
    ]);

    assert_eq!(
        ids,
        vec!["0->toggle"],
        "the row resolves its control but no label"
    );
}

#[test]
fn declared_count_sees_rows_the_resolver_rejects() {
    let entries = [
        form(),
        declared_row("good", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 28.0, 120.0, 20.0),
        control("toggle", 400.0, 38.0, 104.0, 24.0),
        declared_row("ambiguous", 16.0, 100.0, 500.0, 60.0),
        control("a", 400.0, 118.0, 40.0, 24.0),
        control("b", 460.0, 118.0, 40.0, 24.0),
    ];
    let records = deduplicate(&entries);
    let sections = sections_of(&records);

    assert_eq!(
        declared_rows::declared_count(&records),
        2,
        "both row containers were declared"
    );
    assert_eq!(
        declared_rows::find(&records, &sections).len(),
        1,
        "only the unambiguous one resolves"
    );
}

#[test]
fn a_declared_label_is_never_paired_twice() {
    // The label is close enough that the geometric pass would pair it too.
    let pair_count = {
        let records = deduplicate(&[
            form(),
            declared_row("r", 16.0, 20.0, 500.0, 60.0),
            label("title", 24.0, 28.0, 60.0, 18.0),
            control("toggle", 92.0, 25.0, 104.0, 24.0),
        ]);
        let sections = sections_of(&records);
        pairing::pair_labels(&records, &sections).pairs
    };

    assert_eq!(
        pair_count.len(),
        1,
        "the declaration consumes the label; the geometric pass must not re-add it"
    );
    assert_eq!(pair_count[0].kind, pairing::PairKind::Filled);
}

#[test]
fn a_filled_pair_is_side_by_side_but_not_a_tight_row() {
    let records = deduplicate(&[
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 28.0, 60.0, 18.0),
        control("toggle", 92.0, 25.0, 104.0, 24.0),
    ]);
    let sections = sections_of(&records);
    let pairing = pairing::pair_labels(&records, &sections);

    assert_eq!(pairing.row_pairs().len(), 0, "R8 must not see a Fill gap");
    assert_eq!(
        pairing.side_by_side_pairs().len(),
        1,
        "R6 and R10 still own the centring"
    );
    assert!(pairing.pairs[0].is_side_by_side());
    assert!(!pairing.pairs[0].is_row());
}

#[test]
fn a_declared_row_judges_centring_but_not_its_gap() {
    // Two lines centred as a block on the control, 256px of Fill between.
    let report = audit(&[
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 32.0, 120.0, 20.0),
        label("subtitle", 24.0, 52.0, 120.0, 16.0),
        control("toggle", 400.0, 38.0, 104.0, 24.0),
    ]);

    silent(&report, Rule::LabelVerticalAlignment);
    silent(&report, Rule::LabelControlGap);
    assert!(
        report.is_clean(),
        "a block-centred label across a Fill gap is not a misalignment:\n{}",
        report.render()
    );
}

#[test]
fn a_declared_row_still_reports_a_label_that_is_not_centred() {
    let report = audit(&[
        form(),
        declared_row("r", 16.0, 20.0, 500.0, 60.0),
        label("title", 24.0, 24.0, 120.0, 20.0),
        control("toggle", 400.0, 38.0, 104.0, 24.0),
    ]);

    let findings = fired(&report, Rule::LabelVerticalAlignment);
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].detail.contains("off by 16.00px"),
        "label block centre 34.00 against control centre 50.00: {}",
        findings[0].detail
    );
}

#[test]
fn a_named_label_swallows_the_anonymous_text_inside_it() {
    let records = deduplicate(&[
        form(),
        named_label("name", 16.0, 20.0, 60.0, 18.0),
        label("#1 (text)", 16.0, 20.0, 60.0, 18.0),
        control("field", 84.0, 16.0, 100.0, 32.0),
    ]);
    let sections = sections_of(&records);
    let pairing = pairing::pair_labels(&records, &sections);

    assert_eq!(
        pairing.pairs.len(),
        1,
        "one visible label must produce one pair, not two"
    );
    assert_eq!(
        pairing.pairs[0].label.id,
        format!("name{DEMO_LABEL_ID_SUFFIX}")
    );
    assert_eq!(
        pairing.orphans.len(),
        0,
        "the inner text is not a second orphan either"
    );
}

#[test]
fn a_row_container_is_never_mistaken_for_a_section_card() {
    let records = deduplicate(&[
        form(),
        declared_row("demo.section.form.r", 16.0, 20.0, 500.0, 60.0),
        named_label("demo.section.form.t", 24.0, 28.0, 120.0, 20.0),
        entry(
            "demo.section.form.h.caption",
            "container",
            24.0,
            4.0,
            80.0,
            16.0,
        ),
    ]);

    let ids: Vec<String> = sections_of(&records).into_iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec!["demo.section.form"],
        "a .row, .label or .caption container shares the section prefix but is not a card"
    );
}

// ---------------------------------------------------------------------------
// Derived identity — a widget's own published caption, bound by id
// ---------------------------------------------------------------------------

/// A widget's published caption, `<widget-id>.label`, bound to the
/// widget itself rather than inferred from where the two boxes sit.
#[test]
fn pairing_binds_a_published_sub_part_to_the_widget_that_published_it() {
    let pair = only_pair(&[
        form(),
        control("demo.w", 16.0, 20.0, 200.0, 32.0),
        label("demo.w.label", 40.0, 27.0, 60.0, 18.0),
    ]);

    assert_eq!(pair.kind, pairing::PairKind::Derived);
    assert_eq!(pair.control.id, "demo.w");
    assert_eq!(pair.owner, "demo.section.form");
    assert!(pair.is_derived());
    assert!(
        !pair.is_side_by_side(),
        "a caption inside its own widget is not a side-by-side row"
    );
}

/// An indexed role (`label-0`, `label-1`) carries the same base, so each
/// option of a radio group binds to the group.
#[test]
fn pairing_binds_an_indexed_role_to_the_same_widget() {
    let records = deduplicate(&[
        form(),
        control("demo.radio", 16.0, 20.0, 300.0, 24.0),
        label("demo.radio.label-0", 40.0, 20.0, 60.0, 24.0),
        label("demo.radio.label-1", 120.0, 20.0, 60.0, 24.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert_eq!(pairing.derived_pairs().len(), 2);
    assert!(
        pairing
            .derived_pairs()
            .iter()
            .all(|pair| pair.control.id == "demo.radio")
    );
    assert!(
        pairing.orphans.is_empty(),
        "both captions are claimed by their widget"
    );
}

/// The point of binding by identity: a caption the geometry could never
/// reach is still bound, because its id names its widget.
#[test]
fn a_derived_caption_too_far_away_to_pair_geometrically_is_still_not_an_orphan() {
    let records = deduplicate(&[
        form(),
        control("demo.w", 400.0, 200.0, 100.0, 32.0),
        label("demo.w.title", 20.0, 20.0, 60.0, 18.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert_eq!(pairing.pairs.len(), 1);
    assert_eq!(pairing.pairs[0].kind, pairing::PairKind::Derived);
    assert!(
        pairing.orphans.is_empty(),
        "identity outranks the distance bounds that would have orphaned it"
    );
}

/// A sub-part id whose base is not a control is not a derived pair: the
/// binding needs a widget to bind to, and a plain box is not one.
#[test]
fn a_sub_part_of_a_non_control_is_not_derived() {
    let records = deduplicate(&[
        form(),
        card("demo.box", 16.0, 20.0, 200.0, 32.0),
        label("demo.box.title", 40.0, 27.0, 60.0, 18.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));

    assert!(
        pairing.derived_pairs().is_empty(),
        "a title on a box that is not a control has no widget to bind to"
    );
}

/// R6 reads derived pairs: whether a widget's own caption is vertically
/// centred inside the widget is exactly the question the rule asks.
#[test]
fn r6_fires_on_a_derived_caption_that_is_not_centred_in_its_widget() {
    let records = deduplicate(&[
        form(),
        control("demo.w", 16.0, 20.0, 200.0, 32.0),
        label("demo.w.label", 40.0, 20.0, 60.0, 18.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));
    let findings = label_rules::check_labels(&pairing);

    assert_eq!(
        findings.len(),
        1,
        "only the vertical rule should speak to a derived pair"
    );
    assert_eq!(findings[0].rule, Rule::LabelVerticalAlignment);
    assert!(
        findings[0].detail.contains("demo.w"),
        "the finding names the widget, got {}",
        findings[0].detail
    );
}

/// A centred derived caption is clean — and critically, R9 must stay
/// silent: a caption inside its own control overlaps it completely by
/// design, so reading derived pairs there would report every checkbox
/// in the demo as crushed.
#[test]
fn a_centred_derived_caption_is_clean_and_never_reads_as_an_overlap() {
    let records = deduplicate(&[
        form(),
        control("demo.w", 16.0, 20.0, 200.0, 32.0),
        label("demo.w.label", 40.0, 27.0, 60.0, 18.0),
    ]);
    let pairing = pairing::pair_labels(&records, &sections_of(&records));
    let findings = label_rules::check_labels(&pairing);

    assert_eq!(
        pairing.derived_pairs()[0].overlap_extent(),
        (60.0, 18.0),
        "the caption sits fully inside its control"
    );
    assert!(
        findings.is_empty(),
        "a caption inside its own widget is the design, not a crush; got {findings:?}"
    );
}

/// The publish → probe → pair chain, end to end on the real demo.
///
/// Every other derived test reasons from a fixture. This one proves the
/// wiring reaches the shipped widgets: if a widget's `operate` hook
/// stopped publishing, the probe stopped carrying the id, or the pairing
/// stopped honouring it, the demo would fall back to exposing no
/// widget-internal captions at all and this fails.
#[test]
fn the_real_demo_publishes_captions_bound_to_the_widget_that_published_them() {
    let app = DemoApp::boot().0;
    let probe = Probe::capture(ROOT, app.view());
    let records = deduplicate(probe.entries());
    let pairing = pairing::pair_labels(&records, &sections_of(&records));
    let derived = pairing.derived_pairs();

    // Checkboxes, radio options, switches, menu rows, the form's valid
    // control and the settings rows all publish. A floor rather than an
    // exact count so the gate survives demo edits but not a broken chain.
    assert!(
        derived.len() >= 15,
        "the demo's widgets must publish captions the audit can bind, got {}",
        derived.len()
    );

    for pair in &derived {
        let (base, role) = hoarfrost::identity::split_sub_part_name(&pair.label.id)
            .unwrap_or_else(|| panic!("derived caption {:?} carries no role", pair.label.id));
        assert_eq!(
            base, pair.control.id,
            "a caption must bind to the widget whose id it extends"
        );
        assert!(
            hoarfrost::identity::is_role(&role),
            "{} does not end in a published role",
            pair.label.id
        );
        assert_eq!(pair.kind, pairing::PairKind::Derived);
    }
}
