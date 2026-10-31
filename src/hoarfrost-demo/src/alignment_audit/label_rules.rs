//! The label alignment rules — R6, R8, R9 and R10.
//!
//! R1–R5 never ask what a label is. These four do, and they all read the
//! same [`Pairing`] so the pairing is decided once (see
//! [`pairing`](super::pairing)) rather than re-guessed per rule.
//!
//! # Which rule reads which pair kind
//!
//! * **R6** and **R8** and **R10** read *row* pairs, because a vertical
//!   centre, a horizontal gap and a shared column are all properties of
//!   two things sitting side by side.
//! * **R6** additionally reads *derived* pairs — a widget's own caption
//!   is bound to the widget by id, and whether that caption is vertically
//!   centred in its own control is exactly the question R6 asks.
//! * **R9** reads every pair **except** the derived ones. A caption
//!   inside its own control overlaps it completely by design; reading
//!   those would report every checkbox in the demo as crushed.
//!
//! # R7 is deliberately absent
//!
//! The brief asked for a baseline-agreement rule. It is not implementable
//! here and faking it would be worse than not having it. A baseline is a
//! font-metric fact — `baseline = top + ascent` — and the probe records
//! none of it: [`Entry`](crate::layout_probe::Entry) carries `id`,
//! `kind`, `bounds` and `visible`, and nothing else. There is no ascent,
//! no descent, no point size and no glyph origin in the candidate stream,
//! and the audit does not own the probe. Any "derived baseline" this
//! module produced would be a made-up ascent ratio dressed up as a
//! measurement.
//!
//! So **R6 is the load-bearing vertical rule**: it compares the label
//! block's centre against the control's centre, which is the strongest
//! vertical statement the probe can actually support. The rule number R7
//! is left unused rather than reused, so the numbering stays traceable to
//! the brief.
//!
//! Unit tests live in the sibling `alignment_audit_tests.rs` sidecar.

use super::geometry::closest_token;
use super::pairing::{LabelGroup, Pair, Pairing};
use super::rules::uniformity_attribution;
use super::text::{fill, px};
use super::{Attribution, Finding, Record, Rule, TOLERANCE};

/// A section needs at least this many label rows before a shared column
/// guide is a claim that can be false.
const MIN_FORM_ROWS: usize = 2;

/// R6 detail: the label block's centre does not meet the control's.
const R6_DETAIL: &str = "label block of {count} for {control} centres at {block_centre} against \
                        control centre {control_centre}: off by {delta}px (tolerance {tol}px); \
                        block spans {block_top}..{block_bottom}, control spans {control_top}..{control_bottom}";
/// R8 detail: the label-to-control gap is not a space token.
const R8_DETAIL: &str = "label→{control} gap of {gap}px is not a space token (nearest {nearest} = \
                        {token}px, off by {off}px)";
/// R9 detail: the label box and its control's box intersect.
const R9_DETAIL: &str = "label box overlaps {control} by {width}px x {height}px ({area}px^2), past \
                        the {tol}px tolerance [{kind} pair]";
/// R10 detail: no shared column guide across a section's label rows.
const R10_DETAIL: &str = "{rows} label rows share neither a left guide nor a right guide: left edge \
                         spreads {left_spread}px, right edge spreads {right_spread}px (tolerance \
                         {tol}px); lefts [{lefts}], rights [{rights}]";

/// Pair descriptor template.
const PAIR_SUBJECT: &str = "{owner}@label:{control}";

/// R6 — a label sharing a row with its control must be vertically centred
/// on it.
///
/// The comparison is made against the *label block*, not each individual
/// text: a title and a subtitle beside one toggle are centred as a pair
/// of lines and are not two misalignments. For the single-label case —
/// the canonical "label sits high next to the field" — the block is the
/// label, so this is exactly the stated rule.
///
/// Attribution: when the block and the control are the same height, two
/// equal boxes with different centres can only have been *placed*
/// differently, so the composition owns it. When the heights differ the
/// delta could come from either box's own sizing, which the outside of a
/// bounding box cannot see.
#[must_use]
pub fn check_label_vertical(group: &LabelGroup<'_>) -> Vec<Finding> {
    let delta = group.centre_delta();
    if delta.abs() <= TOLERANCE {
        return Vec::new();
    }

    let block_height = group.bottom() - group.top();
    let attribution = if (block_height - group.control.bounds.height).abs() <= TOLERANCE {
        Attribution::Demo
    } else {
        Attribution::Undetermined
    };

    vec![Finding::new(
        Rule::LabelVerticalAlignment,
        pair_subject(group.owner, group.control),
        fill(
            R6_DETAIL,
            &[
                ("count", &group.labels.len().to_string()),
                ("control", &group.control.id),
                ("block_centre", &px(group.centre())),
                ("control_centre", &px(group.control_centre())),
                ("delta", &px(delta.abs())),
                ("tol", &px(TOLERANCE)),
                ("block_top", &px(group.top())),
                ("block_bottom", &px(group.bottom())),
                ("control_top", &px(group.control.top())),
                ("control_bottom", &px(group.control.bottom())),
            ],
        ),
        attribution,
    )]
}

/// R8 — the horizontal gap between a label and its control is a space token.
///
/// A flush pair passes, exactly as in R2. The pairing already refuses to
/// join a label to a control more than [`MAX_ROW_GAP`](super::pairing::MAX_ROW_GAP)
/// away, so a gap measured here is one the layout actually declared
/// rather than a stretch of `Fill`.
#[must_use]
pub fn check_label_gap(pair: &Pair) -> Vec<Finding> {
    let gap = pair.gap();
    if gap <= TOLERANCE {
        return Vec::new();
    }
    let (token, value) = closest_token(gap);
    if (gap - value).abs() <= TOLERANCE {
        return Vec::new();
    }
    vec![Finding::new(
        Rule::LabelControlGap,
        pair_subject(&pair.owner, &pair.control),
        fill(
            R8_DETAIL,
            &[
                ("control", &pair.control.id),
                ("gap", &px(gap)),
                ("nearest", token),
                ("token", &px(value)),
                ("off", &px((gap - value).abs())),
            ],
        ),
        // A gap is a relationship between two boxes; neither box's own
        // size explains which side of the pair the layout moved.
        Attribution::Undetermined,
    )]
}

/// R9 — a label must not be crushed by, or draw over, its control.
///
/// Both axes must exceed [`TOLERANCE`] before this fires. A half-pixel
/// graze on one axis is float noise from the layout solver; a label with
/// real width and real height inside a control's box is the control being
/// drawn over the label.
#[must_use]
pub fn check_label_overlap(pair: &Pair) -> Vec<Finding> {
    let (width, height) = pair.overlap_extent();
    if width <= TOLERANCE || height <= TOLERANCE {
        return Vec::new();
    }
    vec![Finding::new(
        Rule::LabelControlOverlap,
        pair_subject(&pair.owner, &pair.control),
        fill(
            R9_DETAIL,
            &[
                ("control", &pair.control.id),
                ("width", &px(width)),
                ("height", &px(height)),
                ("area", &px(pair.overlap_area())),
                ("tol", &px(TOLERANCE)),
                ("kind", pair.kind.slug()),
            ],
        ),
        Attribution::Undetermined,
    )]
}

/// R10 — a section's label rows share one column guide.
///
/// A form either hangs its labels off a left guide or right-aligns them
/// against the controls. Either is fine; the rule picks whichever the
/// section actually uses and only fires when **neither** edge collapses.
/// Reporting both spreads rather than picking one is what lets a reader
/// tell "right-aligned but sloppy" from "not aligned at all".
#[must_use]
pub fn check_label_columns(owner: &str, groups: &[LabelGroup<'_>]) -> Vec<Finding> {
    if groups.len() < MIN_FORM_ROWS {
        return Vec::new();
    }
    let left_spread = spread(groups.iter().map(LabelGroup::left));
    let right_spread = spread(groups.iter().map(LabelGroup::right));
    if left_spread <= TOLERANCE || right_spread <= TOLERANCE {
        return Vec::new();
    }

    let labels: Vec<&Record> = groups
        .iter()
        .flat_map(|group| group.labels.iter())
        .map(|pair| &pair.label)
        .collect();

    vec![Finding::new(
        Rule::LabelColumnAlignment,
        owner.to_string(),
        fill(
            R10_DETAIL,
            &[
                ("rows", &groups.len().to_string()),
                ("left_spread", &px(left_spread)),
                ("right_spread", &px(right_spread)),
                ("tol", &px(TOLERANCE)),
                ("lefts", &joined(groups.iter().map(LabelGroup::left))),
                ("rights", &joined(groups.iter().map(LabelGroup::right))),
            ],
        ),
        uniformity_attribution(&labels),
    )]
}

/// Runs every label rule over one pairing.
#[must_use]
pub fn check_labels(pairing: &Pairing) -> Vec<Finding> {
    let mut findings = Vec::new();
    for group in pairing.label_groups() {
        findings.extend(check_label_vertical(&group));
    }
    for pair in pairing.derived_pairs() {
        findings.extend(check_label_vertical(&derived_group(pair)));
    }
    for pair in pairing.row_pairs() {
        findings.extend(check_label_gap(pair));
    }
    for pair in &pairing.pairs {
        if pair.is_derived() {
            continue;
        }
        findings.extend(check_label_overlap(pair));
    }
    for owner in pairing.row_owners() {
        findings.extend(check_label_columns(owner, &pairing.groups_for(owner)));
    }
    findings
}

/// A derived pair as a one-line [`LabelGroup`].
///
/// A widget publishes one caption per role, so the "block" R6 measures
/// is the single line — which is the canonical case the rule was stated
/// for.
fn derived_group<'a>(pair: &'a Pair) -> LabelGroup<'a> {
    LabelGroup {
        owner: &pair.owner,
        control: &pair.control,
        labels: vec![pair],
    }
}

/// The distance between the smallest and largest of `values`; `0.0` for
/// a single value.
fn spread(values: impl Iterator<Item = f32>) -> f32 {
    let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    for value in values {
        any = true;
        min = min.min(value);
        max = max.max(value);
    }
    if any { max - min } else { 0.0 }
}

/// A comma-separated list of coordinates for the report body.
fn joined(values: impl Iterator<Item = f32>) -> String {
    values.map(px).collect::<Vec<_>>().join(", ")
}

/// The report subject for one label↔control pair.
fn pair_subject(owner: &str, control: &Record) -> String {
    fill(
        PAIR_SUBJECT,
        &[("owner", owner), ("control", control.id.as_str())],
    )
}
