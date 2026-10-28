//! Unit tests for the alignment audit.
//!
//! Every rule ships with a positive and a negative control: a fixture
//! that must make it fire, and a fixture that must leave it silent. A
//! checker that only ever passes is not a checker — it is a comment with
//! a function signature, and the failure mode is a green suite that has
//! never once looked at anything.

use std::path::{Path, PathBuf};

use hoarfrost::token::space;
use iced::{Rectangle, Size};

use super::*;

/// Setting this to `1` rewrites the audit golden instead of comparing it.
const GOLDEN_ENV: &str = "HOARFROST_DEMO_GOLDEN";
/// Where the goldens live, relative to the crate root.
const GOLDEN_DIR: &str = "tests/golden";
/// The audit's own golden, kept beside the gate's.
const AUDIT_GOLDEN: &str = "alignment-audit-root-1400x900.txt";
/// The demo window's reference viewport.
const ROOT: Size = Size::new(1400.0, 900.0);

/// Builds an entry with the geometry a test wants to reason about.
fn entry(id: &str, kind: &'static str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    let bounds = Rectangle::new(iced::Point::new(x, y), Size::new(w, h));
    Entry {
        id: id.to_string(),
        kind,
        bounds,
        visible: Some(bounds),
    }
}

/// A control the audit should classify as operable.
fn control(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    entry(id, "focusable", x, y, w, h)
}

/// A section card.
fn card(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    entry(id, "container", x, y, w, h)
}

/// Runs the whole audit over a fixture.
fn audit(entries: &[Entry]) -> AuditReport {
    audit_entries(entries)
}

/// Every finding for one rule in a fixture.
fn fired(report: &AuditReport, rule: Rule) -> Vec<&Finding> {
    let findings = report.findings_for(rule);
    assert!(
        !findings.is_empty(),
        "{rule} was expected to fire but stayed silent; report:\n{}",
        report.render()
    );
    findings
}

/// Asserts one rule produced nothing, without constraining the others.
fn silent(report: &AuditReport, rule: Rule) {
    assert_eq!(
        report.findings_for(rule),
        Vec::<&Finding>::new(),
        "{rule} was expected to stay silent; report:\n{}",
        report.render()
    );
}

/// Path to a golden file inside the crate.
fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(GOLDEN_DIR)
        .join(name)
}

/// Whether the goldens should be rewritten rather than compared.
fn regen_requested() -> bool {
    std::env::var(GOLDEN_ENV).is_ok_and(|value| value == "1")
}

/// Compares `actual` against the golden `name`, or rewrites it on request.
///
/// Same convention as the layout gate's helper: a missing golden is a
/// hard failure with instructions, never a silent pass.
fn check_golden(name: &str, actual: &str) {
    let path = golden_path(name);

    if regen_requested() {
        let dir = path
            .parent()
            .expect("a golden path always has a parent directory");
        std::fs::create_dir_all(dir).expect("the golden directory must be creatable");
        std::fs::write(&path, actual).expect("the audit golden must be writable");
        return;
    }

    match std::fs::read_to_string(&path) {
        Ok(expected) => assert_eq!(
            expected,
            actual,
            "golden `{}` differs. If the change is intended, re-run with \
             `$env:HOARFROST_DEMO_GOLDEN='1'` and review the diff.",
            path.display()
        ),
        Err(error) => panic!(
            "golden `{}` is missing ({error}). A missing golden is a FAIL, never \
             an auto-pass. Write it deliberately with \
             `$env:HOARFROST_DEMO_GOLDEN='1'` and review what it captured.",
            path.display()
        ),
    }
}

// ---------------------------------------------------------------------------
// Preprocessing — deduplication
// ---------------------------------------------------------------------------

#[test]
fn deduplication_collapses_one_widget_recorded_twice() {
    let live_input = entry(
        "demo.atoms.text.live",
        "text_input",
        28.0,
        1118.0,
        607.0,
        20.0,
    );
    let live_focus = entry(
        "demo.atoms.text.live",
        "focusable",
        28.0,
        1118.0,
        607.0,
        20.0,
    );

    let records = deduplicate(&[live_input, live_focus]);

    assert_eq!(records.len(), 1, "one widget must not count as two");
    assert_eq!(records[0].id, "demo.atoms.text.live");
    assert_eq!(records[0].merged, 2);
    assert_eq!(records[0].kind, "text_input");
}

#[test]
fn deduplication_keeps_the_same_id_at_different_bounds() {
    let here = entry("demo.atoms.text.live", "focusable", 10.0, 20.0, 100.0, 32.0);
    let there = entry("demo.atoms.text.live", "focusable", 10.0, 60.0, 100.0, 32.0);

    let records = deduplicate(&[here, there]);

    assert_eq!(
        records.len(),
        2,
        "the same id at two different places is two measurements, not one"
    );
    assert!(records.iter().all(|record| record.merged == 1));
}

#[test]
fn deduplication_drops_non_finite_geometry() {
    let broken = entry("demo.broken", "focusable", f32::INFINITY, 0.0, 100.0, 32.0);
    let sound = control("demo.sound", 0.0, 0.0, 100.0, 32.0);

    let records = deduplicate(&[broken, sound]);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "demo.sound");
}

// ---------------------------------------------------------------------------
// Preprocessing — section ownership
// ---------------------------------------------------------------------------

#[test]
fn ownership_resolves_to_the_smallest_containing_section() {
    let outer = card("demo.section.outer", 0.0, 0.0, 600.0, 400.0);
    let inner = card("demo.section.inner", 50.0, 50.0, 200.0, 100.0);
    let deep = control("deep", 60.0, 60.0, 100.0, 24.0);
    let shallow = control("shallow", 10.0, 300.0, 50.0, 24.0);
    let orphan = control("orphan", 700.0, 700.0, 50.0, 24.0);

    let records = deduplicate(&[outer, inner, deep, shallow, orphan]);
    let sections = sections_of(&records);
    let by_id = |wanted: &str| -> Record {
        records
            .iter()
            .find(|record| record.id == wanted)
            .cloned()
            .unwrap_or_else(|| panic!("`{wanted}` is missing from the fixture"))
    };

    assert_eq!(
        sections.len(),
        2,
        "only the two cards are section containers"
    );
    assert_eq!(
        owner_id(&by_id("deep"), &sections),
        "demo.section.inner",
        "a control inside nested cards belongs to the inner one"
    );
    assert_eq!(
        owner_id(&by_id("shallow"), &sections),
        "demo.section.outer",
        "a control inside only the outer card belongs to it"
    );
    assert_eq!(
        owner_id(&by_id("orphan"), &sections),
        CHROME_OWNER,
        "a control no card owns belongs to the chrome"
    );
}

// ---------------------------------------------------------------------------
// Preprocessing — row grouping
// ---------------------------------------------------------------------------

#[test]
fn row_grouping_joins_overlapping_spans_and_splits_the_rest() {
    let a = control("a", 0.0, 0.0, 50.0, 32.0);
    let b = control("b", 60.0, 8.0, 50.0, 32.0);
    let c = control("c", 0.0, 100.0, 50.0, 32.0);

    let rows = group_rows(&deduplicate(&[a, b, c]));

    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"],
        "the two overlapping controls form the first row, ordered by left edge"
    );
    assert_eq!(
        rows[1].iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["c"]
    );
}

#[test]
fn row_grouping_needs_more_than_half_the_shorter_span() {
    let a = control("a", 0.0, 0.0, 50.0, 32.0);
    let exactly_half = control("half", 60.0, 16.0, 50.0, 32.0);
    let just_over = control("over", 60.0, 15.0, 50.0, 32.0);

    let half_only = group_rows(&deduplicate(&[a.clone(), exactly_half]));
    assert_eq!(
        half_only.len(),
        2,
        "exactly 50% overlap is not *more than* 50%, so these are two rows"
    );

    let joined = group_rows(&deduplicate(&[a, just_over]));
    assert_eq!(joined.len(), 1, "51% overlap is more than half");
    assert_eq!(joined[0].len(), 2);
}

#[test]
fn row_grouping_pairs_a_short_control_with_a_tall_neighbour() {
    let checkbox = control("checkbox", 0.0, 4.0, 50.0, 24.0);
    let button = control("button", 60.0, 0.0, 80.0, 32.0);

    let rows = group_rows(&deduplicate(&[checkbox, button]));

    assert_eq!(
        rows.len(),
        1,
        "a 24px checkbox beside a 32px button shares its row"
    );
}

// ---------------------------------------------------------------------------
// R1 — column alignment across rows
// ---------------------------------------------------------------------------

/// Two rows of two uniform controls, the second shifted right by 4px.
fn drifting_columns() -> Vec<Entry> {
    vec![
        card("demo.section.cols", 0.0, 0.0, 600.0, 200.0),
        control("r1c1", 10.0, 20.0, 100.0, 32.0),
        control("r1c2", 120.0, 20.0, 100.0, 32.0),
        control("r2c1", 14.0, 60.0, 100.0, 32.0),
        control("r2c2", 124.0, 60.0, 100.0, 32.0),
    ]
}

#[test]
fn r1_fires_on_a_drifting_column() {
    let report = audit(&drifting_columns());
    let findings = fired(&report, Rule::ColumnAlignment);

    assert_eq!(findings.len(), 2, "both columns drift by the same 4px");
    assert_eq!(findings[0].subject, "demo.section.cols");
    assert!(
        findings[0].detail.contains("spreads 4.00px"),
        "the detail must carry the exact spread: {}",
        findings[0].detail
    );
    assert!(
        findings[0].detail.contains("r1c1") && findings[0].detail.contains("r2c1"),
        "the detail must name the rows that disagree: {}",
        findings[0].detail
    );
}

#[test]
fn r1_stays_silent_on_aligned_columns() {
    let report = audit(&[
        card("demo.section.cols", 0.0, 0.0, 600.0, 200.0),
        control("r1c1", 10.0, 20.0, 100.0, 32.0),
        control("r1c2", 118.0, 20.0, 100.0, 32.0),
        control("r2c1", 10.0, 60.0, 100.0, 32.0),
        control("r2c2", 118.0, 60.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::ColumnAlignment);
    assert!(
        report.is_clean(),
        "this fixture is good on every rule, so the whole report must be clean:\n{}",
        report.render()
    );
}

#[test]
fn r1_does_not_compare_rows_of_different_sizes() {
    let report = audit(&[
        card("demo.section.cols", 0.0, 0.0, 600.0, 200.0),
        control("solo", 40.0, 20.0, 100.0, 32.0),
        control("pair_a", 10.0, 60.0, 100.0, 32.0),
        control("pair_b", 118.0, 60.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::ColumnAlignment);
}

#[test]
fn r1_attributes_a_uniform_scatter_to_the_demo() {
    let report = audit(&drifting_columns());
    let findings = fired(&report, Rule::ColumnAlignment);

    assert!(
        findings
            .iter()
            .all(|finding| finding.attribution == Attribution::Demo),
        "identical widgets cannot be why they are out of line: {:?}",
        findings.iter().map(|f| &f.detail).collect::<Vec<_>>()
    );
}

#[test]
fn r1_attributes_a_non_uniform_scatter_to_undetermined() {
    let report = audit(&[
        card("demo.section.cols", 0.0, 0.0, 600.0, 200.0),
        control("r1c1", 10.0, 20.0, 100.0, 32.0),
        control("r1c2", 200.0, 20.0, 100.0, 32.0),
        control("r2c1", 14.0, 60.0, 110.0, 32.0),
        control("r2c2", 200.0, 60.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::ColumnAlignment);

    assert_eq!(
        findings.len(),
        1,
        "only the first column drifts; the second is aligned"
    );
    assert_eq!(findings[0].subject, "demo.section.cols");
    assert_eq!(
        findings[0].attribution,
        Attribution::Undetermined,
        "widgets of different widths could differ for reasons the audit cannot see"
    );
}

// ---------------------------------------------------------------------------
// R2 — horizontal gap conformance
// ---------------------------------------------------------------------------

#[test]
fn r2_fires_on_a_gap_that_is_not_a_space_token() {
    let report = audit(&[
        card("demo.section.gap", 0.0, 0.0, 600.0, 100.0),
        control("a", 10.0, 20.0, 100.0, 32.0),
        control("b", 115.0, 20.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::GapTokens);

    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].detail.contains("gap of 5.00px"),
        "the detail must carry the exact gap: {}",
        findings[0].detail
    );
    assert!(
        findings[0].detail.contains("nearest xs = 4.00px"),
        "the detail must name the nearest token: {}",
        findings[0].detail
    );
}

#[test]
fn r2_stays_silent_when_the_gap_is_a_space_token() {
    let report = audit(&[
        card("demo.section.gap", 0.0, 0.0, 600.0, 100.0),
        control("a", 10.0, 20.0, 100.0, 32.0),
        control("b", 118.0, 20.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::GapTokens);
    assert_eq!(
        space::SM,
        118.0 - (10.0 + 100.0),
        "the fixture's gap is exactly space::SM"
    );
}

#[test]
fn r2_treats_a_flush_pair_as_conformant() {
    let report = audit(&[
        card("demo.section.gap", 0.0, 0.0, 600.0, 100.0),
        control("a", 10.0, 20.0, 100.0, 32.0),
        control("b", 110.0, 20.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::GapTokens);
}

#[test]
fn r2_exempts_a_blindspot_pair_and_counts_the_skip() {
    let report = audit(&[
        control("demo.atoms.button.Primary.live", 10.0, 20.0, 88.0, 32.0),
        control("demo.atoms.button.Primary.busy", 302.0, 20.0, 88.0, 32.0),
    ]);

    assert_eq!(
        report.findings_for(Rule::GapTokens),
        Vec::<&Finding>::new(),
        "the disabled variant between them is invisible to the probe"
    );
    assert_eq!(
        report.skipped, 1,
        "the exemption must be visible as a count, not a silent pass"
    );
}

#[test]
fn r2_does_not_exempt_a_pair_that_is_not_blindspot_shaped() {
    let report = audit(&[
        control("demo.atoms.text.live", 10.0, 20.0, 88.0, 32.0),
        control("demo.atoms.text.readonly", 302.0, 20.0, 88.0, 32.0),
    ]);

    fired(&report, Rule::GapTokens);
    assert_eq!(report.skipped, 0);
}

#[test]
fn r2_does_not_exempt_lookalike_stems_across_different_parents() {
    let report = audit(&[
        control("demo.atoms.alpha.live", 10.0, 20.0, 88.0, 32.0),
        control("demo.atoms.beta.busy", 302.0, 20.0, 88.0, 32.0),
    ]);

    fired(&report, Rule::GapTokens);
    assert_eq!(report.skipped, 0);
}

#[test]
fn r2_exempts_one_control_that_publishes_two_parts() {
    // A range slider's two thumbs are one control. The distance between
    // them is wherever the model says the values sit — nobody laid it out,
    // so no token can be missing from it. 20px is deliberately not a
    // space token: without the exemption this fixture would fire.
    let report = audit(&[
        control("demo.atoms.range.lower", 10.0, 20.0, 24.0, 24.0),
        control("demo.atoms.range.upper", 54.0, 20.0, 24.0, 24.0),
    ]);

    assert_eq!(
        report.findings_for(Rule::GapTokens),
        Vec::<&Finding>::new(),
        "a control's internal geometry is not a spacing violation"
    );
    assert_eq!(
        report.skipped, 1,
        "the exemption must be visible as a count, not a silent pass"
    );
}

#[test]
fn r4_exempts_one_control_that_publishes_two_parts() {
    // 24px is the pointer hit target, which the framework files as
    // `claims_ladder: false`. A rung the control never claimed cannot be
    // a rung it failed to sit on.
    let report = audit(&[
        control("demo.atoms.range.lower", 10.0, 20.0, 24.0, 24.0),
        control("demo.atoms.range.upper", 54.0, 20.0, 24.0, 24.0),
    ]);

    assert_eq!(
        report.findings_for(Rule::DensityHomogeneity),
        Vec::<&Finding>::new(),
        "a hit-target floor is not an off-ladder control height"
    );
}

#[test]
fn a_part_group_measures_the_next_control_from_its_union_not_its_left_half() {
    // Same fixture, but with a genuinely independent control after the
    // pair. Collapsing to the *leftmost* member would measure 10..34 and
    // report a 76px gap; the group's right edge is the right thumb, so
    // the real gap is 32px — `xxl`.
    let report = audit(&[
        control("demo.atoms.range.lower", 10.0, 20.0, 24.0, 24.0),
        control("demo.atoms.range.upper", 54.0, 20.0, 24.0, 24.0),
        control("demo.atoms.range.value", 110.0, 20.0, 40.0, 24.0),
    ]);

    assert_eq!(
        report.findings_for(Rule::GapTokens),
        Vec::<&Finding>::new(),
        "the group must collapse to 10..78, leaving a 32px token gap"
    );
    assert_eq!(report.skipped, 1);
}

#[test]
fn identical_geometry_under_independent_names_still_fires() {
    // Negative control: byte-identical boxes to the exempt fixtures above,
    // but the names say three separately placed widgets. Exempting part
    // pairs must not soften R2 or R4 for a real offender.
    let report = audit(&[
        control("demo.atoms.a", 10.0, 20.0, 24.0, 24.0),
        control("demo.atoms.b", 54.0, 20.0, 24.0, 24.0),
        control("demo.atoms.c", 110.0, 20.0, 40.0, 24.0),
    ]);

    let gaps = fired(&report, Rule::GapTokens);
    assert!(
        gaps.iter().any(|f| f.detail.contains("gap of 20.00px")),
        "the 20px gap must still be charged: {:?}",
        gaps
    );
    assert_eq!(
        report.skipped, 0,
        "nothing here is a widget-internal part pair"
    );
    fired(&report, Rule::DensityHomogeneity);
}

#[test]
fn lookalike_parts_under_different_parents_are_not_a_pair() {
    // `.lower` / `.upper` pair only under one identical stem. Two unrelated
    // widgets that borrow the suffixes must not cancel each other out.
    let report = audit(&[
        control("demo.atoms.s1.lower", 10.0, 20.0, 24.0, 24.0),
        control("demo.atoms.s2.upper", 54.0, 20.0, 24.0, 24.0),
    ]);

    fired(&report, Rule::GapTokens);
    assert_eq!(report.skipped, 0);
}

// ---------------------------------------------------------------------------
// R3 — section frame uniformity
// ---------------------------------------------------------------------------

#[test]
fn r3_fires_on_misaligned_cards_in_one_pane() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 4.0, 112.0, 600.0, 100.0),
    ]);
    let findings = fired(&report, Rule::SectionFrames);

    assert_eq!(findings.len(), 1, "only the left edge drifts");
    assert!(
        findings[0].detail.contains("left edges spread 4.00px"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r3_fires_on_mismatched_card_widths() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 0.0, 112.0, 580.0, 100.0),
    ]);
    let findings = fired(&report, Rule::SectionFrames);

    assert_eq!(findings.len(), 1, "only the width differs");
    assert!(
        findings[0].detail.contains("widths spread 20.00px"),
        "{}",
        findings[0].detail
    );
    assert_eq!(
        findings[0].attribution,
        Attribution::Undetermined,
        "a width difference is a sizing question, and sizing is not visible from outside"
    );
}

#[test]
fn r3_stays_silent_on_uniform_cards() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 0.0, 112.0, 600.0, 100.0),
    ]);

    silent(&report, Rule::SectionFrames);
}

#[test]
fn r3_keeps_separate_columns_in_separate_panes() {
    let report = audit(&[
        card("demo.section.left", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.right", 700.0, 0.0, 600.0, 100.0),
    ]);

    silent(&report, Rule::SectionFrames);
}

// ---------------------------------------------------------------------------
// R4 — density homogeneity
// ---------------------------------------------------------------------------

#[test]
fn r4_fires_on_a_row_that_mixes_rungs() {
    let report = audit(&[
        card("demo.section.dense", 0.0, 0.0, 600.0, 100.0),
        control("compact", 10.0, 20.0, 100.0, 28.0),
        control("standard", 118.0, 20.0, 100.0, 32.0),
    ]);
    let findings = fired(&report, Rule::DensityHomogeneity);

    assert!(
        findings.iter().all(|f| f.subject.ends_with("#rung-mix")),
        "a wrong rung is the mix class, never the off-ladder class: {:?}",
        findings.iter().map(|f| &f.subject).collect::<Vec<_>>()
    );
    assert!(
        findings
            .iter()
            .any(|f| f.detail.contains("mixes rungs") && f.detail.contains("compact")),
        "the row-level mix finding must name the rungs present: {:?}",
        findings.iter().map(|f| &f.detail).collect::<Vec<_>>()
    );
}

#[test]
fn r4_keeps_off_ladder_heights_in_their_own_class() {
    let report = audit(&[
        card("demo.section.small", 0.0, 0.0, 600.0, 100.0),
        control("box_a", 10.0, 20.0, 100.0, 24.0),
        control("box_b", 118.0, 20.0, 100.0, 24.0),
    ]);
    let findings = fired(&report, Rule::DensityHomogeneity);

    assert!(
        findings.iter().all(|f| f.subject.ends_with("#off-ladder")),
        "a 24px control is on no rung; that is not a rung mix: {:?}",
        findings.iter().map(|f| &f.subject).collect::<Vec<_>>()
    );
    assert_eq!(findings.len(), 2, "one finding per off-ladder control");
    assert!(
        findings[0].detail.contains("24.00px") && findings[0].detail.contains("no density"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r4_stays_silent_on_a_single_rung_row() {
    let report = audit(&[
        card("demo.section.dense", 0.0, 0.0, 600.0, 100.0),
        control("a", 10.0, 20.0, 100.0, 32.0),
        control("b", 118.0, 20.0, 100.0, 32.0),
    ]);

    silent(&report, Rule::DensityHomogeneity);
}

#[test]
fn r4_ignores_rows_with_one_control() {
    let report = audit(&[
        card("demo.section.solo", 0.0, 0.0, 600.0, 200.0),
        control("a", 10.0, 20.0, 100.0, 24.0),
        control("b", 10.0, 100.0, 100.0, 40.0),
    ]);

    silent(&report, Rule::DensityHomogeneity);
}

// ---------------------------------------------------------------------------
// R5 — vertical rhythm between cards
// ---------------------------------------------------------------------------

#[test]
fn r5_fires_on_a_non_token_gap_between_cards() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 0.0, 110.0, 600.0, 100.0),
    ]);
    let findings = fired(&report, Rule::VerticalRhythm);

    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].detail.contains("gap of 10.00px"),
        "{}",
        findings[0].detail
    );
    assert!(
        findings[0].detail.contains("demo.section.one")
            && findings[0].detail.contains("demo.section.two"),
        "{}",
        findings[0].detail
    );
}

#[test]
fn r5_stays_silent_on_a_token_gap_between_cards() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 0.0, 112.0, 600.0, 100.0),
    ]);

    silent(&report, Rule::VerticalRhythm);
}

#[test]
fn r5_treats_flush_cards_as_conformant() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 0.0, 100.0, 600.0, 100.0),
    ]);

    silent(&report, Rule::VerticalRhythm);
}

// ---------------------------------------------------------------------------
// Report rendering
// ---------------------------------------------------------------------------

#[test]
fn the_report_renders_a_clean_summary() {
    let report = AuditReport::default();
    let rendered = report.render();

    assert!(report.is_clean());
    assert!(rendered.contains("findings 0 | skipped 0 | orphan labels 0 | clean yes"));
    assert!(rendered.contains(NO_FINDINGS));
    for rule in Rule::ALL {
        assert!(
            rendered.contains(&format!("{}=0", rule.slug())),
            "every rule must appear in the totals: {rendered}"
        );
    }
    for attribution in Attribution::ALL {
        assert!(
            rendered.contains(&format!("{}=0", attribution.slug())),
            "every attribution must appear in the totals: {rendered}"
        );
    }
}

#[test]
fn the_report_groups_by_rule_then_subject() {
    let report = audit(&[
        card("demo.section.one", 0.0, 0.0, 600.0, 100.0),
        card("demo.section.two", 4.0, 112.0, 600.0, 100.0),
        control("a", 10.0, 20.0, 100.0, 28.0),
        control("b", 115.0, 20.0, 100.0, 32.0),
    ]);
    let rendered = report.render();

    let r1 = rendered
        .find(Rule::SectionFrames.slug())
        .expect("the frame rule must appear");
    let r2 = rendered
        .find(Rule::DensityHomogeneity.slug())
        .expect("the density rule must appear");
    assert!(r1 < r2, "rules must be rendered in rule order:\n{rendered}");
    assert!(rendered.contains("skipped 0"));
}

#[test]
fn rule_and_attribution_slugs_are_unique() {
    let mut seen: Vec<&str> = Rule::ALL.iter().map(|rule| rule.slug()).collect();
    let rule_count = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), rule_count, "rule slugs must be unique");

    let mut seen: Vec<&str> = Attribution::ALL
        .iter()
        .map(|attribution| attribution.slug())
        .collect();
    let attribution_count = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen.len(),
        attribution_count,
        "attribution slugs must be unique"
    );
}

#[cfg(test)]
#[path = "alignment_audit_label_tests.rs"]
mod label_tests;
// ---------------------------------------------------------------------------
// The real demo
// ---------------------------------------------------------------------------

/// Every `text` record in the named stream is a caption a hoarfrost
/// widget published for itself.
///
/// This used to assert zero, and for a while zero was the whole truth:
/// `iced_core::widget::Text` reports `operation.text(None, ..)`
/// (`iced_core-0.14.0/src/widget/text.rs:264`), so nothing in the demo
/// could name a label and the label rules had to read
/// [`Probe::all_entries`] to see any at all.
///
/// The framework has since closed that hole from its own side. A widget
/// that owns its `Widget::operate` publishes the text it drew under
/// `<widget-id>.<role>` (`hoarfrost::identity`), so the named stream
/// now carries those captions. What still has to hold — and what the
/// pairing depends on to bind them by identity rather than guess at
/// them — is that **every** named `text` record splits into a base and a
/// role. A named bare `text` with no base would mean something started
/// naming plain `Text` widgets in a way the convention does not produce,
/// and the derived pairing would silently stop covering it.
#[test]
fn the_named_probe_stream_carries_only_published_sub_part_captions() {
    let app = crate::app::DemoApp::boot().0;
    let probe = Probe::capture(ROOT, app.view());

    let named_text: Vec<&Entry> = probe
        .entries()
        .iter()
        .filter(|entry| entry.kind == "text")
        .collect();
    let live_text = probe
        .all_entries()
        .iter()
        .filter(|entry| entry.kind == "text")
        .count();

    assert!(
        !named_text.is_empty(),
        "hoarfrost publishes a caption for every widget that draws one; \
         the named stream should carry them"
    );
    for entry in &named_text {
        assert!(
            hoarfrost::identity::split_sub_part_name(&entry.id).is_some(),
            "named `text` record `{}` is not a `<widget-id>.<role>` sub-part",
            entry.id
        );
    }
    assert!(
        live_text > named_text.len(),
        "the full candidate stream must still see the demo's own \
         anonymous labels ({} named vs {} total)",
        named_text.len(),
        live_text
    );
}

#[test]
fn the_real_demo_reports_its_unpaired_labels_as_a_count() {
    let report = audit_demo(ROOT);

    assert!(
        report.orphans > 0,
        "the demo is mostly headings, badges and prose, so the label rules \
         must report unpaired labels: {:?}",
        report.render()
    );
    assert!(report.render().contains("orphan labels "));
}

/// Whether the audit's own record set actually contains the section card
/// `id`.
///
/// The rendered report only names sections that produced a finding, so
/// once a section's debt is paid its id disappears from the text. Checking
/// the record set instead keeps "did the audit look at the demo" a
/// meaningful assertion that does not quietly depend on some defect still
/// being present.
fn audit_saw_section(id: &str) -> bool {
    let app = DemoApp::boot().0;
    let probe = Probe::capture(ROOT, app.view());
    sections_of(&deduplicate(probe.entries()))
        .iter()
        .any(|card| card.id == id)
}

#[test]
fn the_real_demo_audit_renders_and_matches_its_golden() {
    let report = audit_demo(ROOT);
    let rendered = report.render();

    assert!(!rendered.is_empty(), "the audit produced no report at all");
    assert!(
        audit_saw_section("demo.section.buttons"),
        "the audit must actually see the demo's buttons section"
    );
    check_golden(AUDIT_GOLDEN, &rendered);
}
