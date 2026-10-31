//! Empirical alignment audit — turns a rendered [`Probe`] into a report of
//! geometric alignment findings.
//!
//! The layout gate asks nine yes/no questions. This module asks the
//! fuzzier ones a designer asks with a ruler: do the controls in column
//! *k* actually line up across rows, is the whitespace between two
//! buttons a token or an accident, do the cards in one pane share a
//! frame, do the controls in a row sit on one density rung, and is the
//! gap between stacked cards a token.
//!
//! # Report, never assert
//!
//! Every rule is a pure function returning `Vec<Finding>`. Nothing here
//! panics on a bad measurement — whether a finding blocks a build is a
//! policy decision that belongs to the caller, and [`AuditReport::is_clean`]
//! exists so a test can make that decision explicitly. A checker that
//! fails the build the first time it sees half a pixel is a checker
//! somebody turns off.
//!
//! # Why `TOLERANCE` is 0.5 and not the gate's 0.01
//!
//! The gate's `EPSILON` answers "is this the *same* number twice", so it
//! is as tight as float equality can be. Alignment is a different
//! question: "did the layout mean 4px or 8px". A tolerance tight enough
//! for the first question reports sub-pixel accumulation as drift; one
//! loose enough to ignore that also cannot tell `space::XS` from
//! `space::SM`. `0.5` sits strictly below the smallest token step
//! ([`space::XXS`] = 2px), so it absorbs float noise while leaving every
//! real token boundary a full pixel of headroom on each side.
//!
//! # Attribution is earned, never assumed
//!
//! A finding says who is *provably* at fault from the geometry alone.
//! Uniform widgets scattered out of line implicate the composition
//! ([`Attribution::Demo`]); widgets that are not uniform could be
//! anything, so they are [`Attribution::Undetermined`]. This module never
//! attributes a finding to [`Attribution::Framework`] from the demo's own
//! bounds, because outer geometry cannot see how a widget was built.
//!
//! # What a `text_input` record actually bounds
//!
//! A `text_input` record is **not** the field's frame — it is the editable
//! text area *inside* that frame. `hoarfrost`'s `TextInput::layout`
//! (`widget/text_input.rs:1631`) resolves its own outer box, then moves
//! the inner iced editor to `text_area_bounds(outer, kind).position()`,
//! and the probe's `Candidate::TextInput` is that inner editor. The
//! field's painted frame is therefore wider on both sides than the record
//! reports, by the framework's own padding:
//!
//! ```text
//! left inset  = HORIZONTAL_PADDING                                  = 12
//! right inset = HORIZONTAL_PADDING + SLOT_GAP + trailing_slot_width = 44
//! ```
//!
//! Both are `space::MD` / `space::SM` / `size::HIT_TARGET_MIN`, and
//! `trailing_slot_width` is `HIT_TARGET_MIN` for every kind the demo
//! shows. This is why `demo.section.text-input` and
//! `demo.section.password` look as though they inset their content 12px
//! right of their caption while every other section sits flush:
//!
//! ```text
//! card padding (space::LG)      -> content starts at x = 16
//! caption                       -> x =  16.00   (flush)
//! field FRAME                   -> x =  16.00   w = 663.00  (flush)
//! field RECORD (inner text area)-> x =  28.00   w = 607.00
//!                                 16 + 12      = 28
//!                                 663 - 12 - 44 = 607
//! ```
//!
//! The frame is flush; only the record is inset. **This is not a
//! misalignment**, and
//! `text_field_frames_are_flush_with_their_section_caption` in the pane
//! sidecar pins the arithmetic so the distinction cannot be lost. Any
//! future rule that compares a section's content left edge against its
//! caption must un-inset a `text_input` record by
//! [`text_input::HORIZONTAL_PADDING`](hoarfrost::widget::text_input::HORIZONTAL_PADDING)
//! first, or it will report the framework's own field padding as a demo
//! bug.
//!
//! # Layout of the module
//!
//! * [`rules`](rules) — the five control-to-control checks (R1–R5).
//! * [`pairing`](pairing) — which control a label belongs to, decided once.
//! * [`label_rules`](label_rules) — the four label-to-control checks
//!   (R6, R8, R9, R10) and why R7 could not be built.
//! * [`geometry`](geometry) — containment, overlap, pane clustering, token lookup.
//! * [`text`](text) — the primitives the report renders with.
//!
//! Unit tests live in the sibling `alignment_audit_tests.rs` sidecar.

use std::collections::BTreeMap;

use iced::{Rectangle, Size};

use crate::app::DemoApp;
use crate::layout_probe::{Entry, Probe};
use crate::section::{CAPTION_ID_SUFFIX, ROW_ID_SUFFIX, is_label_id};

pub mod declared_rows;
pub mod geometry;
pub mod label_rules;
pub mod pairing;
mod rules;
mod text;

pub use label_rules::check_labels;
pub use rules::{check_columns, check_density, check_frames, check_gaps, check_rhythm};

use geometry::{contains, is_finite, same_bounds};
use pairing::pair_labels;
use text::{fill, totals, yes_no};

/// Slack allowed on every geometric comparison in this audit.
///
/// Strictly below the 2px `space::XXS` step, so a 4px gap and an 8px gap
/// remain distinguishable while float accumulation is absorbed.
pub const TOLERANCE: f32 = 0.5;

/// Prefix identifying a demo section card.
const SECTION_PREFIX: &str = "demo.section.";
/// Owner reported for controls no section card contains.
const CHROME_OWNER: &str = "chrome";
/// Pane label used when cards cannot be clustered under one x-range.
const UNPANNED: &str = "pane:unbounded";

/// Kinds the probe records for something a user operates.
const CONTROL_KINDS: [&str; 2] = ["focusable", "text_input"];
/// Kinds the probe records for a box that holds other things.
const CONTAINER_KINDS: [&str; 2] = ["container", "scrollable"];

/// Report header.
const REPORT_HEADER: &str = "hoarfrost alignment audit";
/// One-line totals under the header.
const TOTALS_LINE: &str =
    "findings {findings} | skipped {skipped} | orphan labels {orphans} | clean {clean}";
/// Per-rule totals.
const RULE_TOTALS: &str = "by rule: {counts}";
/// Per-attribution totals.
const ATTRIBUTION_TOTALS: &str = "by attribution: {counts}";
/// Printed when nothing was found.
const NO_FINDINGS: &str = "no findings";
/// Section heading prefix in the grouped body.
const GROUP_PREFIX: &str = "== ";

/// Which alignment rule produced a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// R1 — the *k*-th control across comparable rows shares one left guide.
    ColumnAlignment,
    /// R2 — the horizontal gap between adjacent controls is a space token.
    GapTokens,
    /// R3 — section cards in one pane share a left edge and a width.
    SectionFrames,
    /// R4 — controls in one row sit at one density rung.
    DensityHomogeneity,
    /// R5 — the vertical gap between stacked section cards is a space token.
    VerticalRhythm,
    /// R6 — a label sharing a row with its control is vertically centred on it.
    LabelVerticalAlignment,
    /// R8 — the horizontal gap between a label and its control is a space token.
    LabelControlGap,
    /// R9 — a label's box does not overlap the box of the control it labels.
    LabelControlOverlap,
    /// R10 — a section's label rows share one column guide.
    LabelColumnAlignment,
}

impl Rule {
    /// Every rule, in report order.
    pub const ALL: [Self; 9] = [
        Self::ColumnAlignment,
        Self::GapTokens,
        Self::SectionFrames,
        Self::DensityHomogeneity,
        Self::VerticalRhythm,
        Self::LabelVerticalAlignment,
        Self::LabelControlGap,
        Self::LabelControlOverlap,
        Self::LabelColumnAlignment,
    ];

    /// Stable machine-readable name for grouping and reports.
    ///
    /// `R7` is absent by design: a baseline-agreement rule needs a font
    /// ascent, and the probe does not record one. See the
    /// [`label_rules`](label_rules) module doc for why that rule was
    /// dropped rather than faked. The number is left unused so the slugs
    /// keep pointing at the brief they came from.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ColumnAlignment => "R1-column-alignment",
            Self::GapTokens => "R2-gap-tokens",
            Self::SectionFrames => "R3-section-frames",
            Self::DensityHomogeneity => "R4-density-homogeneity",
            Self::VerticalRhythm => "R5-vertical-rhythm",
            Self::LabelVerticalAlignment => "R6-label-vertical-alignment",
            Self::LabelControlGap => "R8-label-control-gap",
            Self::LabelControlOverlap => "R9-label-control-overlap",
            Self::LabelColumnAlignment => "R10-label-column-alignment",
        }
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// Who the geometry provably implicates.
///
/// The ladder deliberately stops short of [`Attribution::Framework`]:
/// an outer bounding box says nothing about how a widget was constructed,
/// so nothing in this module is allowed to infer framework fault from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attribution {
    /// The demo's own composition produced this.
    Demo,
    /// The framework's widget construction produced this.
    Framework,
    /// The probe could not see a widget that sits in this measurement.
    ToolingBlindspot,
    /// The geometry does not settle who is at fault.
    Undetermined,
}

impl Attribution {
    /// Every attribution, in report order.
    pub const ALL: [Self; 4] = [
        Self::Demo,
        Self::Framework,
        Self::ToolingBlindspot,
        Self::Undetermined,
    ];

    /// Stable machine-readable name for grouping and reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::Framework => "framework",
            Self::ToolingBlindspot => "tooling-blindspot",
            Self::Undetermined => "undetermined",
        }
    }
}

impl std::fmt::Display for Attribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// One reported alignment problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The rule that fired.
    pub rule: Rule,
    /// The section id, pane descriptor or row descriptor it fired on.
    pub subject: String,
    /// Human-readable evidence, with the exact numbers in it.
    pub detail: String,
    /// Who the evidence implicates.
    pub attribution: Attribution,
}

impl Finding {
    /// One finding, assembled from the rule that produced it.
    #[must_use]
    pub fn new(rule: Rule, subject: String, detail: String, attribution: Attribution) -> Self {
        Self {
            rule,
            subject,
            detail,
            attribution,
        }
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} [{}] {}",
            self.rule.slug(),
            self.subject,
            self.attribution.slug(),
            self.detail
        )
    }
}

/// A probe entry after deduplication and classification.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The widget's stable name.
    pub id: String,
    /// The probe kind that survived deduplication.
    pub kind: &'static str,
    /// The widget's layout bounds.
    pub bounds: Rectangle,
    /// How many raw probe entries collapsed into this one.
    pub merged: usize,
}

impl Record {
    /// Whether this record is something a user operates.
    #[must_use]
    pub fn is_control(&self) -> bool {
        CONTROL_KINDS.contains(&self.kind)
    }

    /// Whether this record is a box that holds other things.
    #[must_use]
    pub fn is_container(&self) -> bool {
        CONTAINER_KINDS.contains(&self.kind)
    }

    /// The area the record covers; how "big" a container is when two
    /// of them both claim to own the same control.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.bounds.width * self.bounds.height
    }

    /// Left edge.
    #[must_use]
    pub fn left(&self) -> f32 {
        self.bounds.x
    }

    /// Right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.bounds.x + self.bounds.width
    }

    /// Top edge.
    #[must_use]
    pub fn top(&self) -> f32 {
        self.bounds.y
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.bounds.y + self.bounds.height
    }
}

/// What [`check_gaps`] produced: the violations, and how many pairs the
/// exemptions removed from its view.
///
/// The count is carried separately rather than being quietly dropped so a
/// reader can see how much of the row the rule actually looked at.
#[derive(Clone, Debug, Default)]
pub struct GapAudit {
    /// Gaps that are neither flush nor a space token.
    pub findings: Vec<Finding>,
    /// Parts withheld from measurement because they are not a spacing
    /// decision: a part of a control whose sibling is invisible to the
    /// probe, or one half of a control that publishes several parts.
    pub skipped: usize,
}

/// The whole audit's output.
#[derive(Clone, Debug, Default)]
pub struct AuditReport {
    /// Every finding, in the order the rules produced them.
    pub findings: Vec<Finding>,
    /// Pairs the R2 exemption withheld from measurement.
    pub skipped: usize,
    /// Labels the pairing could not attach to any control.
    ///
    /// Counted, never reported as a finding. A section heading, a
    /// widget-count badge and a paragraph of prose are all labels with no
    /// control to own, and turning those into findings would bury the
    /// rules under thousands of non-events. The number is still surfaced
    /// so a reader can see how much of the page the label rules had
    /// nothing to say about.
    pub orphans: usize,
}

impl AuditReport {
    /// Whether the audit found nothing.
    ///
    /// `skipped` and `orphans` deliberately do not affect this: a
    /// withheld pair and an unattached label are both evidence of
    /// nothing, and a caller that wants to treat "we could not look" as
    /// a problem should check [`Self::skipped`] and [`Self::orphans`]
    /// itself.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Every finding for one rule.
    #[must_use]
    pub fn findings_for(&self, rule: Rule) -> Vec<&Finding> {
        self.findings.iter().filter(|f| f.rule == rule).collect()
    }

    /// A grouped, sorted, human-readable rendering.
    #[must_use]
    pub fn render(&self) -> String {
        let mut sorted: Vec<&Finding> = self.findings.iter().collect();
        sorted.sort_by(|a, b| {
            (&a.rule, &a.subject, &a.detail).cmp(&(&b.rule, &b.subject, &b.detail))
        });

        let mut out = String::new();
        out.push_str(REPORT_HEADER);
        out.push('\n');
        out.push_str(&fill(
            TOTALS_LINE,
            &[
                ("findings", &self.findings.len().to_string()),
                ("skipped", &self.skipped.to_string()),
                ("orphans", &self.orphans.to_string()),
                ("clean", yes_no(self.is_clean())),
            ],
        ));
        out.push('\n');
        out.push_str(&fill(
            RULE_TOTALS,
            &[(
                "counts",
                &totals(&Rule::ALL, Rule::slug, |rule| self.count_rule(rule)),
            )],
        ));
        out.push('\n');
        out.push_str(&fill(
            ATTRIBUTION_TOTALS,
            &[(
                "counts",
                &totals(&Attribution::ALL, Attribution::slug, |attribution| {
                    self.count_attribution(attribution)
                }),
            )],
        ));

        if sorted.is_empty() {
            out.push('\n');
            out.push_str(NO_FINDINGS);
            out.push('\n');
            return out;
        }

        let mut current: Option<Rule> = None;
        for finding in sorted {
            if current != Some(finding.rule) {
                current = Some(finding.rule);
                out.push('\n');
                out.push_str(GROUP_PREFIX);
                out.push_str(finding.rule.slug());
                out.push('\n');
            }
            out.push_str("  ");
            out.push_str(&finding.subject);
            out.push('\n');
            out.push_str("    ");
            out.push_str(&finding.detail);
            out.push_str(" [");
            out.push_str(finding.attribution.slug());
            out.push_str("]\n");
        }
        out
    }

    fn count_rule(&self, rule: Rule) -> usize {
        self.findings_for(rule).len()
    }

    fn count_attribution(&self, attribution: Attribution) -> usize {
        self.findings
            .iter()
            .filter(|f| f.attribution == attribution)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Preprocessing
// ---------------------------------------------------------------------------

/// Collapses the duplicate records the probe writes for one widget.
///
/// A single `text_input` arrives twice — once as `text_input`, once as
/// `focusable` — under the same id with byte-identical bounds. Left
/// uncollapsed, every control in a text field would count twice against
/// the row rules. Non-finite rectangles are dropped here rather than
/// poisoning every area and containment test downstream.
#[must_use]
pub fn deduplicate(entries: &[Entry]) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::with_capacity(entries.len());
    for entry in entries {
        if !is_finite(entry.bounds) {
            continue;
        }
        if let Some(existing) = out
            .iter_mut()
            .find(|record| record.id == entry.id && same_bounds(record.bounds, entry.bounds))
        {
            existing.merged += 1;
            // A control kind wins the merge: an id that shows up as both a
            // box and an operable thing is the operable thing.
            if !CONTROL_KINDS.contains(&existing.kind) && CONTROL_KINDS.contains(&entry.kind) {
                existing.kind = entry.kind;
            }
            continue;
        }
        out.push(Record {
            id: entry.id.clone(),
            kind: entry.kind,
            bounds: entry.bounds,
            merged: 1,
        });
    }
    out
}

/// Every section card among `records`.
///
/// A card is a `demo.section.*` container that is not itself a decoration
/// hanging off one. The demo names a section heading
/// `{section}.caption`, which shares the card prefix; without this
/// exclusion every heading would be counted as a card, R3 would report a
/// heading's text width as a frame violation and R5 would measure the gap
/// from a heading to the next card. The label and `.row` suffixes are
/// excluded on the same grounds — [`is_label_id`] covers both the
/// framework's derived `.label` captions and the demo's authored
/// `.demo-label` containers.
#[must_use]
pub fn sections_of(records: &[Record]) -> Vec<Record> {
    records
        .iter()
        .filter(|record| is_section_card(record))
        .cloned()
        .collect()
}

/// Whether `record` is a section card rather than a decoration inside one.
fn is_section_card(record: &Record) -> bool {
    record.is_container()
        && record.id.starts_with(SECTION_PREFIX)
        && !record.id.ends_with(CAPTION_ID_SUFFIX)
        && !is_label_id(&record.id)
        && !record.id.ends_with(ROW_ID_SUFFIX)
}

/// The smallest-area section card that contains `target`.
///
/// "Smallest" is what makes nesting work: a card inside a card belongs to
/// the inner one, because that is the container actually laying it out.
/// Controls no card owns belong to `chrome`.
#[must_use]
pub fn owner_id(target: &Record, sections: &[Record]) -> String {
    sections
        .iter()
        .filter(|section| contains(section, target))
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .map(|section| section.id.clone())
        .unwrap_or_else(|| CHROME_OWNER.to_string())
}

/// Groups controls into rows by vertical overlap.
///
/// Two controls are in the same row when their vertical spans overlap by
/// more than half the *shorter* one, which is what keeps a 24px checkbox
/// and a 32px button beside it in one row while a 32px row stacked under
/// it stays separate. Rows come back ordered by top edge, controls within
/// a row by left edge.
#[must_use]
pub fn group_rows(controls: &[Record]) -> Vec<Vec<Record>> {
    let mut sorted: Vec<Record> = controls.to_vec();
    sorted.sort_by(|a, b| {
        a.top()
            .total_cmp(&b.top())
            .then_with(|| a.left().total_cmp(&b.left()))
    });

    let mut rows: Vec<Vec<Record>> = Vec::new();
    for control in sorted {
        match rows.last_mut() {
            Some(row)
                if row
                    .iter()
                    .any(|seen| geometry::overlaps_half_v(seen, &control)) =>
            {
                row.push(control);
            }
            _ => rows.push(vec![control]),
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.left().total_cmp(&b.left()));
    }
    rows
}

/// Owner → rows, for every owner present in `records`.
fn rows_by_owner(records: &[Record], sections: &[Record]) -> BTreeMap<String, Vec<Vec<Record>>> {
    let mut grouped: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for record in records {
        if record.is_control() {
            grouped
                .entry(owner_id(record, sections))
                .or_default()
                .push(record.clone());
        }
    }
    grouped
        .into_iter()
        .map(|(owner, controls)| (owner, group_rows(&controls)))
        .collect()
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// Runs the control rules (R1–R5) over an already-deduplicated set.
///
/// Split out from [`audit_entries`] so [`audit_probe`] can feed the
/// control rules and the label rules from two *different* record sets
/// without running R1–R5 twice.
#[must_use]
pub fn audit_controls(records: &[Record], sections: &[Record]) -> AuditReport {
    let mut report = AuditReport::default();

    for (owner, rows) in rows_by_owner(records, sections) {
        report.findings.extend(check_columns(&owner, &rows));
        let gaps = check_gaps(&owner, &rows);
        report.findings.extend(gaps.findings);
        report.skipped += gaps.skipped;
        report.findings.extend(check_density(&owner, &rows));
    }
    report.findings.extend(check_frames(sections));
    report.findings.extend(check_rhythm(sections));
    report
}

/// Runs the label rules (R6, R8, R9, R10) over an already-deduplicated
/// set and folds the result into `report`.
pub fn audit_labels(records: &[Record], sections: &[Record], report: &mut AuditReport) {
    let pairing = pair_labels(records, sections);
    report.orphans += pairing.orphans.len();
    report.findings.extend(check_labels(&pairing));
}

/// Runs every rule over raw probe entries.
#[must_use]
pub fn audit_entries(entries: &[Entry]) -> AuditReport {
    let records = deduplicate(entries);
    let sections = sections_of(&records);
    let mut report = audit_controls(&records, &sections);
    audit_labels(&records, &sections, &mut report);
    report
}

/// Runs every rule over a captured [`Probe`].
///
/// The two halves read two different streams, and that is the whole
/// point. R1–R5 read the **named** entries, because a control rule that
/// reported an anonymous `#4127 (container)` is a finding nobody can
/// act on. The label rules read the **full candidate stream**, because
/// this demo gives no stable `Id` to any `Text` widget: `entries()`
/// contains 91 records and not one of them is a `text`, so a label rule
/// reading the named stream would be a rule reading nothing.
///
/// The label half therefore runs over anonymous records, and every
/// finding it produces is anchored to a *named* section — the subject is
/// `demo.section.buttons`, never `#29 (text)`.
#[must_use]
pub fn audit_probe(probe: &Probe) -> AuditReport {
    let named = deduplicate(probe.entries());
    let named_sections = sections_of(&named);
    let mut report = audit_controls(&named, &named_sections);

    let everything = deduplicate(probe.all_entries());
    let all_sections = sections_of(&everything);
    audit_labels(&everything, &all_sections, &mut report);
    report
}

/// Captures and audits the whole demo root at `size`.
///
/// The gate's own `capture_root` is private to `layout_gate` and that
/// file is read-only to this module, so the three lines that build and
/// probe the demo are repeated here rather than shared.
#[must_use]
pub fn audit_demo(size: Size) -> AuditReport {
    let app = DemoApp::boot().0;
    audit_probe(&Probe::capture(size, app.view()))
}

#[cfg(test)]
#[path = "alignment_audit_tests.rs"]
mod tests;
