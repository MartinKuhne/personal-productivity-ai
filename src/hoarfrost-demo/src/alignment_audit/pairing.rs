//! The explicit label↔control pairing every label rule reads.
//!
//! R1–R5 reason control-to-control. They cannot answer "is this label
//! misaligned with the thing it labels" because they never decide which
//! thing a label labels. This module makes that decision once, in one
//! place, so the rules downstream are statements about a *pair* rather
//! than four independent guesses about adjacency.
//!
//! # The pairing, stated exactly
//!
//! A **label** is a `text` record that is not the caption of a control.
//! A **control** is a [`Record::is_control`] record (`focusable` or
//! `text_input`). A label's control is the control in the label's own
//! section that is, in order of preference:
//!
//! 1. **row partner** — on the label's row ([`overlaps_half_v`]) and
//!    starting at or after the label's left edge; the one with the
//!    smallest left edge wins, i.e. the first control you hit scanning
//!    right.
//! 2. **below partner** — starting at or below the label's bottom edge,
//!    horizontally overlapping it by more than half the narrower box,
//!    and no further below than [`MAX_BELOW_GAP`]; the highest wins.
//!
//! A label with neither is **orphaned**.
//!
//! # Why a caption is not a label
//!
//! A button's own caption lives *inside* the button's box. Left in, every
//! button in the demo would pair with the next button along and the audit
//! would report the gap between two buttons as a label-to-control gap.
//! Excluding text a control already contains ([`contains`], same
//! tolerance as everywhere else) is what keeps a caption from being read
//! as a label.
//!
//! # Why "below" needs a distance bound
//!
//! A section heading sits above the section's first control at exactly
//! [`crate::section::HEADING_GAP`], which is a real token and therefore
//! indistinguishable from a field label's own lead-in by token identity
//! alone. The bound keeps the pairing to *adjacent* rows; it cannot
//! distinguish a heading from a field label, which is why a below
//! pairing only ever feeds the overlap rule and never the row rules.
//!
//! Unit tests live in the sibling `alignment_audit_tests.rs` sidecar.

use std::collections::{BTreeMap, BTreeSet};

use hoarfrost::identity::split_sub_part_name;
use hoarfrost::token::space;

use crate::section::is_label_id;

use super::geometry::{contains, overlaps_half_h, overlaps_half_v};
use super::{Record, TOLERANCE};

/// How far below a label a control may sit and still be *its* control.
///
/// One `space::MD` — the distance a label sits above a field it owns.
/// Anything further down belongs to the layout, not to the label.
pub const MAX_BELOW_GAP: f32 = space::MD;

/// How far to the right of a label's left edge a control may sit and
/// still be *immediately* to its right.
///
/// A label that owns a control sits within one comfortable reading gap of
/// it. Past `space::LG` the space between the two is fluid — a `Fill`
/// pushing a right-aligned control away from a left-aligned label — and
/// the geometry no longer evidences that the two belong together. Without
/// this bound the demo's five navbar captions pair with the Close button
/// 1,000px away and the audit reports the width of the window as a
/// label-to-control gap.
///
/// Measured on the 1400x900 demo, the navbar captions (`Hoarfrost`,
/// `File`, `Edit`, `View`, `Help`) sit at `cy = 13.10` while
/// `demo.chrome.close` sits at `cy = 18.00`. Widening this bound past
/// the fluid spacer therefore does not surface a misalignment so much as
/// invent a pairing between a menu bar and a window control that no
/// reader would call a labelled field.
///
/// The cost of the bound is coverage, and it is worth naming: the demo's
/// real form (`demo.section.settings-rows-uif-comp-017`) separates each
/// title/subtitle block from its toggle by ~380px of `Fill`, so those
/// rows arrive here as orphans rather than pairs. Read off raw geometry
/// they are exact — row 1's label block spans `y = 3908..3944`
/// (`cy = 3926.00`) against `demo.composites.setting.0` at
/// `cy = 3926.00`, row 2's spans `3972..4008` (`cy = 3990.00`) against
/// `demo.composites.setting.1` at `cy = 3990.00`, and every label's left
/// edge is `737.00`. Closing that gap needs the *row structure* the
/// probe does not expose, not a larger number here.
pub const MAX_ROW_GAP: f32 = space::LG;

/// How a label attached to its control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairKind {
    /// The control sits on the label's row, to its right.
    Row,
    /// The label and the control share a row the demo declared, separated
    /// by a fluid `Fill` spacer.
    ///
    /// Kept distinct from [`PairKind::Row`] because the gap between the
    /// two is not a value the layout chose: R6 and R10 may read this pair,
    /// R8 may not.
    Filled,
    /// The control sits directly beneath the label.
    Below,
    /// The label *is* a part of the control: its id is
    /// `<control-id>.<role>`, published by the widget that drew it.
    ///
    /// Bound by identity rather than by proximity, so no distance bound
    /// applies and none is needed. The caption sits inside its widget by
    /// construction, which makes the gap and overlap rules meaningless
    /// for it — a checkbox's caption overlapping the checkbox's own hit
    /// box is the design, not a crush. Only the vertical-centring rule
    /// reads this kind.
    Derived,
}

impl PairKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Filled => "filled",
            Self::Below => "below",
            Self::Derived => "derived",
        }
    }
}

impl std::fmt::Display for PairKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// One label attached to the control it labels.
#[derive(Clone, Debug)]
pub struct Pair {
    /// The `text` record doing the labelling.
    pub label: Record,
    /// The control being labelled.
    pub control: Record,
    /// Whether they share a row or the control sits below.
    pub kind: PairKind,
    /// The section both of them live in.
    pub owner: String,
}

impl Pair {
    /// Whether the label and its control share a row.
    #[must_use]
    pub fn is_row(&self) -> bool {
        self.kind == PairKind::Row
    }

    /// Whether the label and its control sit side by side, whether the gap
    /// between them was declared ([`PairKind::Row`]) or merely left over
    /// by a `Fill` ([`PairKind::Filled`]).
    ///
    /// Side-by-side is what a shared vertical centre and a shared column
    /// guide are properties *of*, so the row-oriented rules read this
    /// rather than [`Pair::is_row`].
    #[must_use]
    pub fn is_side_by_side(&self) -> bool {
        matches!(self.kind, PairKind::Row | PairKind::Filled)
    }

    /// Whether the label is a named part of the control itself.
    ///
    /// See [`PairKind::Derived`] for why such a pair answers the
    /// vertical-centring question and not the gap or overlap ones.
    #[must_use]
    pub fn is_derived(&self) -> bool {
        self.kind == PairKind::Derived
    }

    /// Horizontal gap from the label's right edge to the control's left
    /// edge. Negative when the two boxes overlap.
    #[must_use]
    pub fn gap(&self) -> f32 {
        self.control.left() - self.label.right()
    }

    /// Vertical gap from the label's bottom edge to the control's top
    /// edge. Negative when the two boxes overlap.
    #[must_use]
    pub fn vertical_gap(&self) -> f32 {
        self.control.top() - self.label.bottom()
    }

    /// Vertical centre of the label's box.
    #[must_use]
    pub fn label_centre(&self) -> f32 {
        centre(&self.label)
    }

    /// Vertical centre of the control's box.
    #[must_use]
    pub fn control_centre(&self) -> f32 {
        centre(&self.control)
    }

    /// How far the two vertical centres disagree.
    #[must_use]
    pub fn centre_delta(&self) -> f32 {
        self.label_centre() - self.control_centre()
    }

    /// The horizontal and vertical extent the two boxes share, `(0.0,
    /// 0.0)` when they do not overlap.
    ///
    /// Both axes are reported rather than only the area: a 0.4px graze on
    /// one axis and a 40px crush are both "overlap" by area alone, and
    /// only one of them is a real finding.
    #[must_use]
    pub fn overlap_extent(&self) -> (f32, f32) {
        let width = f32::min(self.label.right(), self.control.right())
            - f32::max(self.label.left(), self.control.left());
        let height = f32::min(self.label.bottom(), self.control.bottom())
            - f32::max(self.label.top(), self.control.top());
        (width.max(0.0), height.max(0.0))
    }

    /// The area the two boxes share, or `0.0` when they do not overlap.
    #[must_use]
    pub fn overlap_area(&self) -> f32 {
        let (width, height) = self.overlap_extent();
        width * height
    }
}

/// Every row-paired label that shares one control.
///
/// A settings row puts a title *and* a subtitle beside one toggle. Each
/// text's own centre is nowhere near the toggle's, and the row is correct
/// — the two lines are centred as a block. Checking the block, not the
/// line, is what keeps a two-line row from reading as two misalignments.
#[derive(Clone, Debug)]
pub struct LabelGroup<'a> {
    /// The section the group lives in.
    pub owner: &'a str,
    /// The control the group labels.
    pub control: &'a Record,
    /// The row pairs in the group, ordered by top edge.
    pub labels: Vec<&'a Pair>,
}

impl<'a> LabelGroup<'a> {
    /// Top of the label block.
    #[must_use]
    pub fn top(&self) -> f32 {
        self.labels
            .iter()
            .map(|pair| pair.label.top())
            .fold(f32::INFINITY, f32::min)
    }

    /// Bottom of the label block.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.labels
            .iter()
            .map(|pair| pair.label.bottom())
            .fold(f32::NEG_INFINITY, f32::max)
    }

    /// Vertical centre of the label block.
    #[must_use]
    pub fn centre(&self) -> f32 {
        (self.top() + self.bottom()) / 2.0
    }

    /// Vertical centre of the control.
    #[must_use]
    pub fn control_centre(&self) -> f32 {
        centre(self.control)
    }

    /// How far the block's centre sits from the control's centre.
    #[must_use]
    pub fn centre_delta(&self) -> f32 {
        self.centre() - self.control_centre()
    }

    /// Left edge of the block.
    #[must_use]
    pub fn left(&self) -> f32 {
        self.labels
            .iter()
            .map(|pair| pair.label.left())
            .fold(f32::INFINITY, f32::min)
    }

    /// Right edge of the block.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.labels
            .iter()
            .map(|pair| pair.label.right())
            .fold(f32::NEG_INFINITY, f32::max)
    }
}

/// What [`pair_labels`] produced.
#[derive(Clone, Debug, Default)]
pub struct Pairing {
    /// Every label that found a control.
    pub pairs: Vec<Pair>,
    /// Every label that found none.
    pub orphans: Vec<Record>,
}

impl Pairing {
    /// Only the pairs that share a *tightly* judged row.
    ///
    /// A `Filled` pair's gap is whatever `Fill` left over at the current
    /// viewport width, so the gap rule must not see it; everything that
    /// asks about side-by-side geometry wants
    /// [`Pairing::side_by_side_pairs`] instead.
    #[must_use]
    pub fn row_pairs(&self) -> Vec<&Pair> {
        self.pairs.iter().filter(|pair| pair.is_row()).collect()
    }

    /// Every pair whose label and control sit side by side.
    #[must_use]
    pub fn side_by_side_pairs(&self) -> Vec<&Pair> {
        self.pairs
            .iter()
            .filter(|pair| pair.is_side_by_side())
            .collect()
    }

    /// Every label bound to the widget whose id it names.
    ///
    /// These are the captions a hoarfrost widget publishes for itself
    /// (`<widget-id>.<role>`). They are the only pairs whose binding is
    /// a stated fact rather than an inference, and the only ones the
    /// vertical rule reads that [`Pairing::label_groups`] does not carry.
    #[must_use]
    pub fn derived_pairs(&self) -> Vec<&Pair> {
        self.pairs.iter().filter(|pair| pair.is_derived()).collect()
    }

    /// The side-by-side pairs collapsed into one [`LabelGroup`] per control.
    ///
    /// Ordered by owner then by the group's top edge, so a report reads
    /// top-to-bottom the way the form does.
    #[must_use]
    pub fn label_groups<'a>(&'a self) -> Vec<LabelGroup<'a>> {
        let mut grouped: BTreeMap<(&'a str, &'a str), Vec<&'a Pair>> = BTreeMap::new();
        for pair in self.side_by_side_pairs() {
            grouped
                .entry((pair.owner.as_str(), pair.control.id.as_str()))
                .or_default()
                .push(pair);
        }
        let mut groups: Vec<LabelGroup<'a>> = grouped
            .into_iter()
            .map(|((owner, control_id), mut labels)| {
                labels.sort_by(|a, b| a.label.top().total_cmp(&b.label.top()));
                let first: &&'a Pair = labels.first().expect("a group always carries a label");
                let control: &'a Record = &first.control;
                debug_assert_eq!(control.id, control_id);
                LabelGroup {
                    owner,
                    control,
                    labels,
                }
            })
            .collect();
        groups.sort_by(|a, b| {
            a.owner
                .cmp(b.owner)
                .then_with(|| a.top().total_cmp(&b.top()))
        });
        groups
    }

    /// The row-paired labels in one section, grouped by the control they
    /// share — i.e. the section's form rows.
    #[must_use]
    pub fn groups_for(&self, owner: &str) -> Vec<LabelGroup<'_>> {
        self.label_groups()
            .into_iter()
            .filter(|group| group.owner == owner)
            .collect()
    }

    /// Every owner that has at least one side-by-side label row.
    #[must_use]
    pub fn row_owners(&self) -> Vec<&str> {
        let mut owners: Vec<&str> = self
            .side_by_side_pairs()
            .iter()
            .map(|pair| pair.owner.as_str())
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }
}

/// Attaches every label in `records` to the control it labels.
///
/// Three sources, in precedence order:
///
/// 1. [`declared_rows`] — rows the demo wrapped in a `*.row` container.
///    A declaration outranks inference, and the labels a declared row
///    consumes are removed from the geometric pass so they are never
///    paired twice.
/// 2. **derived identity** — a text record whose id is
///    `<control-id>.<role>` belongs to that control, because the widget
///    that drew it said so. See [`PairKind::Derived`].
/// 3. the geometric rules described in this module's doc, for every
///    label neither source has already claimed.
#[must_use]
pub fn pair_labels(records: &[Record], sections: &[Record]) -> Pairing {
    let controls: Vec<&Record> = records
        .iter()
        .filter(|record| record.is_control())
        .collect();
    let control_by_id: BTreeMap<&str, &Record> = controls
        .iter()
        .copied()
        .map(|control| (control.id.as_str(), control))
        .collect();

    let mut pairing = Pairing::default();
    let declared = super::declared_rows::find(records, sections);
    let (declared_pairs, mut consumed) = super::declared_rows::pairs(&declared);
    pairing.pairs.extend(declared_pairs);

    let (derived_pairs, derived_consumed) = derived_bindings(records, &control_by_id, sections);
    pairing.pairs.extend(derived_pairs);
    consumed.extend(derived_consumed);

    let named_labels: Vec<&Record> = records
        .iter()
        .filter(|record| is_label_id(&record.id))
        .collect();

    let labels: Vec<&Record> = records
        .iter()
        .filter(|record| is_label(record, &controls))
        .filter(|record| !consumed.contains(&record.id))
        .filter(|record| !inside_named_label(record, &named_labels))
        .collect();

    for label in labels {
        let owner = super::owner_id(label, sections);
        let owned: Vec<&Record> = controls
            .iter()
            .copied()
            .filter(|control| super::owner_id(control, sections) == owner)
            .collect();

        if let Some(control) = row_partner(label, &owned) {
            pairing.pairs.push(Pair {
                label: (*label).clone(),
                control: (*control).clone(),
                kind: PairKind::Row,
                owner,
            });
        } else if let Some(control) = below_partner(label, &owned) {
            pairing.pairs.push(Pair {
                label: (*label).clone(),
                control: (*control).clone(),
                kind: PairKind::Below,
                owner,
            });
        } else {
            pairing.orphans.push((*label).clone());
        }
    }
    pairing
}

/// Every text record whose id names a part of one of `controls`, bound
/// to that control.
///
/// The binding is read off the id, not measured: `demo.atoms.checkbox.0.label`
/// belongs to `demo.atoms.checkbox.0` because
/// [`hoarfrost::identity`] says a sub-part id is `<widget-id>.<role>`,
/// and the widget published it while drawing. A part whose base is not a
/// control — a caption on a plain box — is left to the geometric pass,
/// which is the only thing that can say anything about it.
fn derived_bindings<'a>(
    records: &'a [Record],
    controls: &BTreeMap<&'a str, &'a Record>,
    sections: &[Record],
) -> (Vec<Pair>, BTreeSet<String>) {
    let mut pairs = Vec::new();
    let mut consumed = BTreeSet::new();

    for record in records {
        if record.kind != LABEL_KIND {
            continue;
        }
        let Some((base, _role)) = split_sub_part_name(&record.id) else {
            continue;
        };
        let Some(control) = controls.get(base.as_str()) else {
            continue;
        };

        pairs.push(Pair {
            label: (*record).clone(),
            control: (*control).clone(),
            kind: PairKind::Derived,
            owner: super::owner_id(record, sections),
        });
        consumed.insert(record.id.clone());
    }

    (pairs, consumed)
}

/// Whether `record` is a label rather than a control's own caption.
fn is_label(record: &Record, controls: &[&Record]) -> bool {
    is_label_shaped(record) && !controls.iter().any(|control| contains(control, record))
}

/// Whether `record` carries a label at all, in either of the two shapes
/// the demo produces.
///
/// * a bare `text` candidate — what iced reports for any `Text` widget,
///   and all the demo's framework-rendered labels arrive as;
/// * a container marked with a label suffix — the only way to put a
///   stable `Id` on a label in iced 0.14, since `iced_core::widget::Text`
///   reports `operation.text(None, ..)` and so can never be named.
///   [`is_label_id`] accepts both the framework's derived `.label` and
///   the demo's authored `.demo-label`.
fn is_label_shaped(record: &Record) -> bool {
    record.kind == LABEL_KIND || is_label_id(&record.id)
}

/// Whether `record` is the anonymous `text` candidate inside a label the
/// demo already named.
///
/// [`labeled`](crate::section::labeled) wraps a `Text` in a padding-free
/// container to give it a stable `Id`, so the probe reports that one
/// visible label twice — once as the named container, once as the
/// anonymous text inside it. Dropping the inner copy keeps the pairing at
/// one entry per label a reader can actually see.
fn inside_named_label(record: &Record, named: &[&Record]) -> bool {
    record.kind == LABEL_KIND && named.iter().any(|label| contains(label, record))
}

/// The probe kind that carries a label.
const LABEL_KIND: &str = "text";

/// The first control on the label's row, scanning right from its left edge.
fn row_partner<'a>(label: &Record, controls: &[&'a Record]) -> Option<&'a Record> {
    controls
        .iter()
        .copied()
        .filter(|control| control.left() >= label.left() - TOLERANCE)
        .filter(|control| control.left() - label.right() <= MAX_ROW_GAP + TOLERANCE)
        .filter(|control| overlaps_half_v(label, control))
        .min_by(|a, b| a.left().total_cmp(&b.left()))
}

/// The highest control beneath the label, within [`MAX_BELOW_GAP`].
///
/// The lower bound is the label's own *top*, not its bottom: a control
/// drawn down over a label starts above the label's bottom edge, and
/// cutting the pairing there would make the crush R9 exists to catch
/// structurally unreachable.
fn below_partner<'a>(label: &Record, controls: &[&'a Record]) -> Option<&'a Record> {
    controls
        .iter()
        .copied()
        .filter(|control| control.top() >= label.top() + TOLERANCE)
        .filter(|control| control.top() - label.bottom() <= MAX_BELOW_GAP + TOLERANCE)
        .filter(|control| overlaps_half_h(label, control))
        .min_by(|a, b| a.top().total_cmp(&b.top()))
}

/// Vertical centre of a record's box.
fn centre(record: &Record) -> f32 {
    record.top() + record.bounds.height / 2.0
}
