//! Rows the demo *declares*, and the label↔control binding that implies.
//!
//! # The problem this solves
//!
//! [`pairing`](super::pairing) decides from geometry alone that a label
//! sits beside a control. That works when the two are close, and stops
//! working the moment a `Fill` spacer separates them — which is exactly
//! how a settings row is built: a filling title/description column and a
//! shrinking toggle pinned to the trailing edge, ~380px apart. Widening
//! the geometric bound to reach them would also bind the demo's five
//! navbar captions to the window Close button 1,000px away, so the bound
//! stays where it is and this module reaches the row a different way.
//!
//! # The discriminator, stated exactly
//!
//! The demo wraps a row in a padding-free container whose id ends in
//! [`ROW_ID_SUFFIX`]. That container is a *declaration*: "the label and
//! the control inside this box belong together." It is turned into a pair
//! only when all three of these hold:
//!
//! 1. **exactly one control** sits inside the row — with none there is
//!    nothing to label, and with two the row cannot say which the words
//!    belong to, so neither case produces a pair;
//! 2. the label is a [`text`](super::pairing) record or a container
//!    carrying a label suffix
//!    ([`is_label_id`](crate::section::is_label_id)), and is **not** the
//!    control's own caption;
//! 3. the label is **inside** the row container.
//!
//! Nothing here measures distance, so nothing here can reach across one.
//! The navbar has no declared rows, which is *why* this cannot pair it
//! with Close — the soundness of the rule is the demo's own structure,
//! not a tuned number.
//!
//! # Why the gap is not judged
//!
//! A declared row's label-to-control gap is whatever `Fill` happened to
//! leave at the current viewport width. It is not a value the layout
//! chose, so R8 is not allowed to call it a token violation. Declared
//! pairs therefore enter the pairing as
//! [`PairKind::Filled`](super::pairing::PairKind::Filled), which R6 and
//! R10 read and R8 skips.

use std::collections::BTreeSet;

use crate::section::{ROW_ID_SUFFIX, is_label_id};

use super::geometry::contains;
use super::pairing::{Pair, PairKind};
use super::{Record, owner_id};

/// One row the demo declared, resolved to its control and its labels.
#[derive(Clone, Debug)]
pub struct DeclaredRow<'a> {
    /// The `*.row` container the demo declared.
    pub row: &'a Record,
    /// The single control the row owns.
    pub control: &'a Record,
    /// The label records inside the row, ordered by top edge.
    pub labels: Vec<&'a Record>,
    /// The section the row lives in.
    pub owner: String,
}

/// Finds every declared row among `records`.
///
/// A `*.row` container with anything other than exactly one control is
/// skipped rather than guessed at, so it contributes no pair. Callers
/// that want to know how many rows the demo declared in total can compare
/// [`declared_count`] against the number of `*.row` records in the probe.
#[must_use]
pub fn find<'a>(records: &'a [Record], sections: &[Record]) -> Vec<DeclaredRow<'a>> {
    let mut rows: Vec<DeclaredRow<'a>> = Vec::new();
    for row in records.iter().filter(|record| is_declared_row(record)) {
        let controls: Vec<&'a Record> = inside(row, records)
            .into_iter()
            .filter(|record| record.is_control())
            .collect();
        if controls.len() != 1 {
            continue;
        }
        let control = controls[0];
        let mut labels: Vec<&'a Record> = inside(row, records)
            .into_iter()
            .filter(|record| is_row_label(record, control))
            .collect();
        labels.sort_by(|a, b| a.top().total_cmp(&b.top()));

        rows.push(DeclaredRow {
            row,
            control,
            labels,
            owner: owner_id(row, sections),
        });
    }
    rows.sort_by(|a, b| a.row.top().total_cmp(&b.row.top()));
    rows
}

/// Turns declared rows into pairs, plus the label ids they consumed.
///
/// The consumed set lets [`pair_labels`](super::pairing::pair_labels)
/// skip those labels in the geometric pass instead of pairing them twice.
#[must_use]
pub fn pairs(rows: &[DeclaredRow<'_>]) -> (Vec<Pair>, BTreeSet<String>) {
    let mut pairs = Vec::new();
    let mut consumed = BTreeSet::new();
    for row in rows {
        for label in &row.labels {
            pairs.push(Pair {
                label: (*label).clone(),
                control: (*row.control).clone(),
                kind: PairKind::Filled,
                owner: row.owner.clone(),
            });
            consumed.insert(label.id.clone());
        }
    }
    (pairs, consumed)
}

/// How many `*.row` containers the demo declared, resolvable or not.
///
/// Compare against [`find`] to see how many declared rows carried exactly
/// one control and how many were skipped for having none or several.
#[must_use]
pub fn declared_count(records: &[Record]) -> usize {
    records
        .iter()
        .filter(|record| is_declared_row(record))
        .count()
}

/// Whether `record` is a container the demo declared to be a row.
fn is_declared_row(record: &Record) -> bool {
    record.kind == CONTAINER_KIND && record.id.ends_with(ROW_ID_SUFFIX)
}

/// Whether `record` can be the label of a declared row.
///
/// Either flavour of label the demo knows about, as long as it is not
/// drawn inside the control it would be paired with.
fn is_row_label(record: &Record, control: &Record) -> bool {
    let is_label_shaped = record.kind == TEXT_KIND || is_label_id(&record.id);
    is_label_shaped && !contains(control, record)
}

/// Every record strictly inside `outer`, excluding `outer` itself.
fn inside<'a>(outer: &Record, records: &'a [Record]) -> Vec<&'a Record> {
    records
        .iter()
        .filter(|record| record.id != outer.id && contains(outer, record))
        .collect()
}

/// The probe kind of a box.
const CONTAINER_KIND: &str = "container";
/// The probe kind of a run of text.
const TEXT_KIND: &str = "text";
