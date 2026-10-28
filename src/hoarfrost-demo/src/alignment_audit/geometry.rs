//! Pure geometry for the alignment audit: containment, overlap, pane
//! clustering and space-token lookup.
//!
//! Nothing here knows what a widget *is*. Every function takes edges or
//! [`Record`]s and answers a question about where they are, which is what
//! lets the rules read as statements about layout rather than as a pile
//! of float comparisons.

use hoarfrost::token::space;
use iced::Rectangle;

use super::text::{fill, px};
use super::{Record, TOLERANCE, UNPANNED};

/// The seven space tokens, by name, as the gap rules read them.
const SPACE_TOKENS: [(&str, f32); 7] = [
    ("xxs", space::XXS),
    ("xs", space::XS),
    ("sm", space::SM),
    ("md", space::MD),
    ("lg", space::LG),
    ("xl", space::XL),
    ("xxl", space::XXL),
];

/// Pane descriptor template.
const PANE_SUBJECT: &str = "pane x=[{left}..{right}]";

/// Whether `outer` covers `inner`, allowing [`TOLERANCE`] on every side.
#[must_use]
pub fn contains(outer: &Record, inner: &Record) -> bool {
    outer.left() - TOLERANCE <= inner.left()
        && outer.top() - TOLERANCE <= inner.top()
        && outer.right() + TOLERANCE >= inner.right()
        && outer.bottom() + TOLERANCE >= inner.bottom()
}

/// Whether two vertical spans overlap by more than half the shorter one.
#[must_use]
pub fn overlaps_half_v(a: &Record, b: &Record) -> bool {
    overlaps_half(a.top(), a.bottom(), b.top(), b.bottom())
}

/// Whether two horizontal spans overlap by more than half the shorter one.
#[must_use]
pub fn overlaps_half_h(a: &Record, b: &Record) -> bool {
    overlaps_half(a.left(), a.right(), b.left(), b.right())
}

/// Whether two intervals overlap by more than half the shorter one.
///
/// Exactly half is *not* enough: the rule is "more than 50%", and a
/// boundary that silently included itself would merge rows the layout
/// never meant to join.
#[must_use]
pub fn overlaps_half(a0: f32, a1: f32, b0: f32, b1: f32) -> bool {
    let smaller = (a1 - a0).min(b1 - b0);
    smaller > 0.0 && (a1.min(b1) - a0.max(b0)) > smaller * 0.5
}

/// Whether two rectangles are the same measurement, bit for bit.
///
/// Deliberately exact. Deduplication asks "did the probe write this twice",
/// and a tolerance there would merge two genuinely different placements of
/// one id into a single record.
#[must_use]
pub fn same_bounds(a: Rectangle, b: Rectangle) -> bool {
    a.x == b.x && a.y == b.y && a.width == b.width && a.height == b.height
}

/// Whether every component of a rectangle is a finite number.
#[must_use]
pub fn is_finite(rect: Rectangle) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

/// The space token a measured gap is nearest to.
///
/// Returned together with its distance so a caller can report how far off
/// the nearest token a gap was rather than only that it was off.
#[must_use]
pub fn closest_token(gap: f32) -> (&'static str, f32) {
    SPACE_TOKENS
        .iter()
        .min_by(|a, b| (gap - a.1).abs().total_cmp(&(gap - b.1).abs()))
        .map(|(name, value)| (*name, *value))
        .unwrap_or(("none", 0.0))
}

/// Groups cards into panes by horizontal overlap.
///
/// A pane is not a widget the probe can name — the demo's scrollable
/// bounds stop at the viewport while its content runs thousands of
/// pixels further — so panes are recovered from the x-ranges the cards
/// actually occupy. Two cards are in one pane when their horizontal spans
/// overlap by more than half the narrower one, which keeps the demo's
/// two side-by-side columns apart while still grouping a card that has
/// drifted a few pixels inside its own column.
#[must_use]
pub fn cluster_panes(sections: &[Record]) -> Vec<(String, Vec<Record>)> {
    let mut sorted: Vec<Record> = sections.to_vec();
    sorted.sort_by(|a, b| {
        a.left()
            .total_cmp(&b.left())
            .then_with(|| a.top().total_cmp(&b.top()))
    });

    let mut clusters: Vec<Vec<Record>> = Vec::new();
    for card in sorted {
        match clusters.last_mut() {
            Some(cluster) if cluster.iter().any(|seen| overlaps_half_h(seen, &card)) => {
                cluster.push(card);
            }
            _ => clusters.push(vec![card]),
        }
    }
    clusters
        .into_iter()
        .map(|cluster| (pane_subject(&cluster), cluster))
        .collect()
}

/// The x-range a cluster of cards spans, as its report label.
fn pane_subject(cluster: &[Record]) -> String {
    let left = cluster
        .iter()
        .map(Record::left)
        .fold(f32::INFINITY, f32::min);
    let right = cluster
        .iter()
        .map(Record::right)
        .fold(f32::NEG_INFINITY, f32::max);
    if left.is_finite() && right.is_finite() {
        fill(PANE_SUBJECT, &[("left", &px(left)), ("right", &px(right))])
    } else {
        UNPANNED.to_string()
    }
}
