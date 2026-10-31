//! The five alignment rules.
//!
//! Each rule is a pure function from already-grouped geometry to
//! `Vec<Finding>`. They read the same [`TOLERANCE`] the whole audit uses,
//! so "close enough" means one thing everywhere, and each one states in
//! its own doc what it can and cannot see.

use std::collections::BTreeMap;

use hoarfrost::layout::{ControlDensity, DensityBand, RowSlot, check_row_density};

use super::geometry::{closest_token, cluster_panes};
use super::text::{fill, px};
use super::{Attribution, Finding, GapAudit, Record, Rule, TOLERANCE};

/// A row must hold at least this many controls before its columns, gaps
/// and density mean anything.
const MIN_ROW_CONTROLS: usize = 2;
/// At least this many equally-sized rows are needed before a column can
/// drift relative to anything.
const MIN_COMPARABLE_ROWS: usize = 2;

/// Suffixes whose sibling variants are known to be invisible to the probe.
///
/// The demo renders each button style as a `live`, a *disabled* and a
/// `busy` variant side by side. A disabled hoarfrost `Button` registers
/// no focusable candidate at all — iced's own disabled handling, listed
/// under `UNADDRESSABLE` in `layout_gate` — so the probe sees
/// `…Primary.live` and `…Primary.busy` as *adjacent* when three widgets
/// are physically present. The measured gap therefore contains a widget
/// this tool cannot see, and charging it as a spacing violation would be
/// measuring the probe's blind spot rather than the layout.
const BLINDSPOT_SUFFIXES: [&str; 2] = ["live", "busy"];

/// Suffixes a hoarfrost widget appends to its own focus id when **one
/// control instance publishes more than one focusable part**.
///
/// `RangeSlider::with_focus_id` mints `{focus_id}.lower` and
/// `{focus_id}.upper` (`hoarfrost/src/widget/range_slider.rs:343-346`),
/// and `RangeSlider::operate` registers both halves as separate
/// `operation.focusable` candidates at `hit_rect(..)`
/// (`range_slider.rs:457-468`). The probe therefore sees one control as
/// two records — and both R2 and R4 read that pair as if a host had placed
/// two controls side by side, which is exactly wrong:
///
/// * **The distance between them is the widget's value, not a gap.** It
///   is `Δfraction × thumb_travel − hit_size`, a function of where the
///   model says the thumbs sit. Nobody chose it, so no token can be
///   missing from it.
/// * **The height of each is the pointer floor, not a control height.**
///   `THUMB_HIT_SIZE` is `size::HIT_TARGET_MIN` (`range_slider.rs:53`);
///   the visual thumb is `size::SLIDER_THUMB` (16px). The framework's own
///   geometry gallery files `RangeSlider` under
///   `declared: size::HIT_TARGET_MIN, claims_ladder: false`
///   (`hoarfrost/src/geometry_gallery.rs:451-457`), in the block headed
///   *"the hit-target floor, not a density rung"*. A rung the framework
///   says the control does not claim cannot be a rung the control failed to
///   sit on.
///
/// Two ids sharing one stem under two of these suffixes are therefore
/// **one control**, and neither rule may measure across it.
const WIDGET_PART_SUFFIXES: [&str; 2] = ["lower", "upper"];

/// The band every audited row is treated as, and the rung it declares.
///
/// `FormRow` / `Standard` is the demo's baseline: the audit asks whether
/// what was actually laid out matches what a form row is supposed to be.
const AUDIT_BAND: DensityBand = DensityBand::FormRow;
/// See [`AUDIT_BAND`].
const AUDIT_DENSITY: ControlDensity = ControlDensity::Standard;

/// Report class for a control on a rung other than its band's.
const CLASS_RUNG_MIX: &str = "rung-mix";
/// Report class for a control whose height is on no rung at all.
const CLASS_OFF_LADDER: &str = "off-ladder";

/// R1 detail: one column failed to collapse onto a single guide.
const R1_DETAIL: &str = "column {col} of {cols} spreads {spread}px (tolerance {tol}px): \
                        min {min} at {min_id}, max {max} at {max_id}; row lefts: {row_values}";
/// R2/R5 detail: a measured gap is not a space token.
const GAP_DETAIL: &str = "gap of {gap}px between {prev} and {next} is not a space token \
                         (nearest {nearest} = {token}px, off by {off}px)";
/// R3 detail: card left edges disagree.
const R3_LEFT_DETAIL: &str = "card left edges spread {spread}px: min {min} at {min_id}, \
                             max {max} at {max_id}";
/// R3 detail: card widths disagree.
const R3_WIDTH_DETAIL: &str = "card widths spread {spread}px: min {min} at {min_id}, \
                              max {max} at {max_id}";
/// R4 detail for a control on the wrong rung.
const R4_MIX_DETAIL: &str = "{class}: {violation}";
/// R4 detail for a row that physically mixes more than one rung.
const R4_MIX_ROW_DETAIL: &str = "{class}: row physically mixes rungs [{spread}]";
/// R4 detail for a control that is on no rung at all.
const R4_OFF_DETAIL: &str = "{class}: {control} measures {height}px, which is on no density \
                            rung (ladder: {ladder})";
/// Row descriptor template.
const ROW_SUBJECT: &str = "{owner}@row{top}";

/// A measured value paired with the id it came from.
#[derive(Clone)]
struct Measured {
    id: String,
    value: f32,
}

/// R1: rows that hold the same number of controls must align column-wise.
///
/// Only rows with an identical control count are compared — a section
/// whose rows hold 2, 2 and 3 controls has its two 2-control rows checked
/// against each other, and the 3-control row checked against nothing.
/// A column drifts when its left edges do not collapse onto one guide.
#[must_use]
pub fn check_columns(owner: &str, rows: &[Vec<Record>]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut by_width: BTreeMap<usize, Vec<&Vec<Record>>> = BTreeMap::new();
    for row in rows {
        by_width.entry(row.len()).or_default().push(row);
    }

    for (count, group) in by_width {
        if count < MIN_ROW_CONTROLS || group.len() < MIN_COMPARABLE_ROWS {
            continue;
        }
        for column in 0..count {
            let cells: Vec<&Record> = group.iter().map(|row| &row[column]).collect();
            let Some((min, max)) = extremes(&cells, Record::left) else {
                continue;
            };
            let spread = max.value - min.value;
            if spread <= TOLERANCE {
                continue;
            }
            findings.push(Finding::new(
                Rule::ColumnAlignment,
                owner.to_string(),
                fill(
                    R1_DETAIL,
                    &[
                        ("col", &(column + 1).to_string()),
                        ("cols", &count.to_string()),
                        ("spread", &px(spread)),
                        ("tol", &px(TOLERANCE)),
                        ("min", &px(min.value)),
                        ("min_id", &min.id),
                        ("max", &px(max.value)),
                        ("max_id", &max.id),
                        ("row_values", &column_values(&cells, column)),
                    ],
                ),
                uniformity_attribution(&cells),
            ));
        }
    }
    findings
}

/// The lowest and highest projected value in `records`, with their ids.
fn extremes(records: &[&Record], project: fn(&Record) -> f32) -> Option<(Measured, Measured)> {
    let mut min: Option<Measured> = None;
    let mut max: Option<Measured> = None;
    for record in records {
        let measured = Measured {
            id: record.id.clone(),
            value: project(record),
        };
        if min
            .as_ref()
            .is_none_or(|current| measured.value < current.value)
        {
            min = Some(measured.clone());
        }
        if max
            .as_ref()
            .is_none_or(|current| measured.value > current.value)
        {
            max = Some(measured);
        }
    }
    match (min, max) {
        (Some(min), Some(max)) => Some((min, max)),
        _ => None,
    }
}

/// `row@top:colN=value` for every cell in one column, so a reader can see
/// which row contributed which value.
fn column_values(cells: &[&Record], column: usize) -> String {
    cells
        .iter()
        .map(|cell| {
            format!(
                "row@{}:col{}={}",
                px(cell.top()),
                column + 1,
                px(cell.left())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Attributes a scatter among widgets that all measure the same.
///
/// If every member is the same width and the same height, the members
/// cannot be why they are out of line — identical things do not move
/// differently. The scatter therefore comes from how they were composed.
/// Non-uniform members could differ for any number of reasons, so they
/// settle nothing.
///
/// Shared with [`label_rules`](super::label_rules) so the label rules
/// attribute with exactly the same evidence standard as R1 and R3.
pub(super) fn uniformity_attribution(items: &[&Record]) -> Attribution {
    let Some(first) = items.first() else {
        return Attribution::Undetermined;
    };
    let uniform = items.iter().all(|item| {
        (item.bounds.width - first.bounds.width).abs() <= TOLERANCE
            && (item.bounds.height - first.bounds.height).abs() <= TOLERANCE
    });
    if uniform {
        Attribution::Demo
    } else {
        Attribution::Undetermined
    }
}

/// R2: the whitespace between adjacent controls in a row is flush or a
/// space token.
///
/// Two kinds of pair are *skipped* rather than passed, because in both
/// cases the measured distance is not a spacing decision anyone made:
/// a pair the probe cannot fully see ([`BLINDSPOT_SUFFIXES`]) and a pair
/// that is one control's internal halves ([`WIDGET_PART_SUFFIXES`]).
/// Skips are counted so the exemption cannot quietly eat a real finding.
#[must_use]
pub fn check_gaps(owner: &str, rows: &[Vec<Record>]) -> GapAudit {
    let mut audit = GapAudit::default();
    for row in rows {
        let units = collapse_widget_parts(row);
        audit.skipped += units.iter().filter(|unit| unit.internal).count();
        let row: Vec<Record> = units.into_iter().map(|unit| unit.record).collect();
        for pair in row.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);
            if is_blindspot_pair(&prev.id, &next.id) {
                audit.skipped += 1;
                continue;
            }
            let gap = next.left() - prev.right();
            if gap <= TOLERANCE {
                continue;
            }
            let (token, value) = closest_token(gap);
            if (gap - value).abs() <= TOLERANCE {
                continue;
            }
            audit.findings.push(Finding::new(
                Rule::GapTokens,
                row_subject(owner, &row),
                fill(
                    GAP_DETAIL,
                    &[
                        ("gap", &px(gap)),
                        ("prev", &prev.id),
                        ("next", &next.id),
                        ("nearest", token),
                        ("token", &px(value)),
                        ("off", &px((gap - value).abs())),
                    ],
                ),
                Attribution::Undetermined,
            ));
        }
    }
    audit
}

/// A stable descriptor for one row inside one owner.
fn row_subject(owner: &str, row: &[Record]) -> String {
    let top = row.iter().map(Record::top).fold(f32::INFINITY, f32::min);
    fill(ROW_SUBJECT, &[("owner", owner), ("top", &px(top))])
}

/// Whether two ids are the same stem under two *different* members of a
/// suffix set — the shape both exemptions share.
///
/// The separator has to survive stripping, or `foo-live` and `bar-busy`
/// would read as one stem.
fn is_variant_pair(a: &str, b: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|first| {
        suffixes
            .iter()
            .any(|second| *first != *second && same_stem(a, b, first, second))
    })
}

/// Whether two ids are the same widget stem under two probe-visible
/// variants of a triple whose middle member is unaddressable.
fn is_blindspot_pair(a: &str, b: &str) -> bool {
    is_variant_pair(a, b, &BLINDSPOT_SUFFIXES)
}

fn same_stem(a: &str, b: &str, first: &str, second: &str) -> bool {
    match (a.strip_suffix(first), b.strip_suffix(second)) {
        (Some(stem_a), Some(stem_b)) => stem_a == stem_b && stem_a.ends_with('.'),
        _ => false,
    }
}

/// Whether two ids are two published parts of the **same** control
/// instance — see [`WIDGET_PART_SUFFIXES`].
///
/// Deliberately as narrow as [`is_blindspot_pair`]: the suffixes must
/// differ and the stem in front of them must be byte-identical, so
/// `s1.lower` and `s2.upper` are two unrelated widgets and stay under
/// measurement.
fn is_widget_part_pair(a: &str, b: &str) -> bool {
    is_variant_pair(a, b, &WIDGET_PART_SUFFIXES)
}

/// A row with every widget-internal part group merged into one unit.
///
/// A group collapses to the **union** of its parts, not to its leftmost
/// member: the group's right edge is where the control actually ends, so
/// a following independent control is still measured from real geometry.
///
/// The collapsed row is what both R2 and R4 must look at. Measuring the
/// raw row instead charges one control's internal geometry against the
/// host's spacing, which is the audit inventing a violation rather than
/// finding one.
fn collapse_widget_parts(row: &[Record]) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::with_capacity(row.len());
    let mut skip = vec![false; row.len()];
    for (index, record) in row.iter().enumerate() {
        if skip[index] {
            continue;
        }
        let mut group = record.clone();
        let mut internal = false;
        for other in (index + 1)..row.len() {
            if skip[other] || !is_widget_part_pair(&group.id, &row[other].id) {
                continue;
            }
            skip[other] = true;
            internal = true;
            group.bounds = group.bounds.union(&row[other].bounds);
        }
        units.push(Unit {
            record: group,
            internal,
        });
    }
    units
}

/// One measured unit in a row after [`collapse_widget_parts`].
struct Unit {
    /// The geometry the unit occupies: a single record, or the union of
    /// one control's published parts.
    record: Record,
    /// Whether this unit absorbed at least one sibling part.
    ///
    /// Such a unit's height is the widget's pointer hit target, which the
    /// framework declares is *not* a density rung
    /// (`geometry_gallery.rs:451-457`), so R4 must not judge it against
    /// the ladder.
    internal: bool,
}

/// R3: cards inside one pane share one left edge and one width.
#[must_use]
pub fn check_frames(sections: &[Record]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (pane, cards) in cluster_panes(sections) {
        let cards: Vec<&Record> = cards.iter().collect();
        if cards.len() < MIN_COMPARABLE_ROWS {
            continue;
        }
        if let Some(finding) = frame_edge_finding(&pane, &cards) {
            findings.push(finding);
        }
        if let Some(finding) = frame_width_finding(&pane, &cards) {
            findings.push(finding);
        }
    }
    findings
}

/// The left-edge half of R3.
fn frame_edge_finding(pane: &str, cards: &[&Record]) -> Option<Finding> {
    let (min, max) = extremes(cards, Record::left)?;
    let spread = max.value - min.value;
    if spread <= TOLERANCE {
        return None;
    }
    Some(Finding::new(
        Rule::SectionFrames,
        pane.to_string(),
        fill(
            R3_LEFT_DETAIL,
            &[
                ("spread", &px(spread)),
                ("min", &px(min.value)),
                ("min_id", &min.id),
                ("max", &px(max.value)),
                ("max_id", &max.id),
            ],
        ),
        uniformity_attribution(cards),
    ))
}

/// The width half of R3.
fn frame_width_finding(pane: &str, cards: &[&Record]) -> Option<Finding> {
    let (min, max) = extremes(cards, |card| card.bounds.width)?;
    let spread = max.value - min.value;
    if spread <= TOLERANCE {
        return None;
    }
    Some(Finding::new(
        Rule::SectionFrames,
        pane.to_string(),
        fill(
            R3_WIDTH_DETAIL,
            &[
                ("spread", &px(spread)),
                ("min", &px(min.value)),
                ("min_id", &min.id),
                ("max", &px(max.value)),
                ("max_id", &max.id),
            ],
        ),
        // A width difference is a sizing question, and sizing is not
        // visible from the outside.
        Attribution::Undetermined,
    ))
}

/// R4: a row's controls sit at the rung its band declares.
///
/// The verdict comes from [`check_row_density`], not from a re-derivation
/// here, so the audit cannot disagree with the framework about what a rung
/// is. Two classes are kept apart because they need different fixes:
/// `rung-mix` is a control built at the other rung, `off-ladder` is a
/// height the ladder never offered at all.
///
/// A control that publishes its own parts ([`WIDGET_PART_SUFFIXES`]) is
/// removed from the row before it is judged. Its published height is the
/// pointer hit target, and the framework records that as
/// `claims_ladder: false` — a rung the control never claimed cannot be a
/// rung it failed to sit on.
#[must_use]
pub fn check_density(owner: &str, rows: &[Vec<Record>]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for row in rows {
        let row: Vec<Record> = collapse_widget_parts(row)
            .into_iter()
            .filter(|unit| !unit.internal)
            .map(|unit| unit.record)
            .collect();
        if row.len() < MIN_ROW_CONTROLS {
            continue;
        }
        let slots: Vec<RowSlot<'_>> = row
            .iter()
            .map(|control| RowSlot::new(control.id.as_str(), control.bounds.height))
            .collect();
        let report = check_row_density(AUDIT_BAND, AUDIT_DENSITY, &slots);
        let subject = row_subject(owner, &row);

        for violation in report.violations.iter().filter(|v| !v.is_off_rung()) {
            findings.push(Finding::new(
                Rule::DensityHomogeneity,
                format!("{subject}#{CLASS_RUNG_MIX}"),
                fill(
                    R4_MIX_DETAIL,
                    &[
                        ("class", CLASS_RUNG_MIX),
                        ("violation", &violation.to_string()),
                    ],
                ),
                Attribution::Undetermined,
            ));
        }
        if report.mixes_rungs() {
            findings.push(Finding::new(
                Rule::DensityHomogeneity,
                format!("{subject}#{CLASS_RUNG_MIX}"),
                fill(
                    R4_MIX_ROW_DETAIL,
                    &[
                        ("class", CLASS_RUNG_MIX),
                        ("spread", &rung_spread(&report.rung_spread)),
                    ],
                ),
                Attribution::Undetermined,
            ));
        }
        for violation in report.violations.iter().filter(|v| v.is_off_rung()) {
            findings.push(Finding::new(
                Rule::DensityHomogeneity,
                format!("{subject}#{CLASS_OFF_LADDER}"),
                fill(
                    R4_OFF_DETAIL,
                    &[
                        ("class", CLASS_OFF_LADDER),
                        ("control", violation.control),
                        ("height", &px(violation.height)),
                        ("ladder", &density_ladder()),
                    ],
                ),
                Attribution::Undetermined,
            ));
        }
    }
    findings
}

fn rung_spread(rungs: &[ControlDensity]) -> String {
    rungs
        .iter()
        .map(|rung| rung.slug())
        .collect::<Vec<_>>()
        .join(", ")
}

fn density_ladder() -> String {
    ControlDensity::ALL
        .iter()
        .map(|rung| format!("{} = {}px", rung.slug(), px(rung.height())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// R5: the gap between consecutive cards in a pane is a space token.
///
/// A gap at or below [`TOLERANCE`] is flush and passes, the same way an
/// abutting pair does in R2.
#[must_use]
pub fn check_rhythm(sections: &[Record]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (pane, mut cards) in cluster_panes(sections) {
        cards.sort_by(|a, b| a.top().total_cmp(&b.top()));
        for pair in cards.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);
            let gap = next.top() - prev.bottom();
            if gap <= TOLERANCE {
                continue;
            }
            let (token, value) = closest_token(gap);
            if (gap - value).abs() <= TOLERANCE {
                continue;
            }
            findings.push(Finding::new(
                Rule::VerticalRhythm,
                pane.clone(),
                fill(
                    GAP_DETAIL,
                    &[
                        ("gap", &px(gap)),
                        ("prev", &prev.id),
                        ("next", &next.id),
                        ("nearest", token),
                        ("token", &px(value)),
                        ("off", &px((gap - value).abs())),
                    ],
                ),
                Attribution::Undetermined,
            ));
        }
    }
    findings
}
