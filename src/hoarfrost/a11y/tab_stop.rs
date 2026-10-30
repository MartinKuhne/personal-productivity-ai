//! The single-`Tab`-stop composite boundary, and the wrap-versus-clamp
//! rule that goes with it (`UIF-PRIN-008`).
//!
//! # The gap this closes
//!
//! [`rove_index`] has always supplied
//! the walk — skip disabled, wrap at the ends — and two models share it.
//! But a walk is not a *boundary*. Nothing in the crate could answer the
//! question a host actually has: "when this thing finishes handling
//! `Tab`, is it one control or forty?" That property was an emergent
//! accident of how `rove_index` happens to work, invisible to a test and
//! invisible to a host wiring a focus ring.
//!
//! [`CompositeBoundary`] makes it a declared fact. A composite says
//! what it is, how many members it has, which of them are live, which
//! one is active, and which edge rule it obeys. From that, the
//! stop-count, the entry member and the internal next-step are all
//! derivable and all checkable.
//!
//! # The three pins this type exists to hold
//!
//! * **Entry lands on the active member, not the first.** §1.1 says
//!   `Tab` enters "on its active/last-focused item". A composite whose
//!   entry fell to member zero every time would throw away the last
//!   thing the user did, so [`entry_member`](CompositeBoundary::entry_member)
//!   is the active member whenever there is one, and the first *enabled*
//!   member only when there is not.
//! * **A composite with nothing enabled contributes no stop.** A group
//!   of disabled members is not a stop the user can enter and be stuck
//!   inside. [`tab_stops`](CompositeBoundary::tab_stops) returns `0`.
//! * **N composites are N stops, not N-members-stops.**
//!   [`total_tab_stops`] sums the declared boundaries so a host can
//!   assert its focus ring is the size it thinks it is.
//!
//! # Bounded wraps, spatial clamps
//!
//! The second clause of the pillar splits collections by *shape*, and
//! the split is load-bearing in both directions. A `Menu` that clamped
//! would leave a keyboard user stranded at the bottom of a short list
//! with `Down` doing nothing. A `DataTable` that wrapped would turn one
//! held keypress at row 99,999 into a jump to row 0 — the exact
//! catastrophe §1.1 names.
//!
//! [`CompositeCategory`] carries the required edge rule, so a
//! contradiction between what a composite *is* and how it *behaves at
//! the edge* is a reported violation rather than a UX review finding.
//! The sidecar additionally pins the real implementations against the
//! rule: `rove_index` wraps, and `TreeView::move_focus` and
//! `TableView::move_row` clamp.
//!
//! Unit tests live in the sibling `tab_stop_tests.rs` sidecar.

use crate::widget::rove::{RoveDirection, rove_index};

/// How a composite behaves when the walk reaches its last member
/// (`UIF-PRIN-008`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeBehaviour {
    /// Last → first. The behaviour of a small bounded group, where
    /// there is nowhere else worth going.
    Wraps,
    /// Stay at the end. The behaviour of a spatial or virtualized
    /// collection, where wrapping means teleporting a viewport.
    Clamps,
}

impl EdgeBehaviour {
    /// Every behaviour.
    pub const ALL: [Self; 2] = [Self::Wraps, Self::Clamps];

    /// Whether the walk cycles back to the start.
    #[must_use]
    pub const fn is_wrapping(self) -> bool {
        matches!(self, Self::Wraps)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Wraps => "wraps",
            Self::Clamps => "clamps",
        }
    }
}

/// What kind of composite it is, which fixes the edge rule it must obey
/// (`UIF-PRIN-008`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompositeCategory {
    /// A small bounded group — `Menu`, `RadioGroup`, `SegmentedButton`,
    /// `NavigationRail`, `CommandPalette`. Arrow navigation **wraps**.
    BoundedGroup,
    /// A spatial or large virtualized collection — `TreeView`,
    /// `DataTable`, `GridView`, `Slider`. Arrow navigation **clamps**.
    SpatialCollection,
}

impl CompositeCategory {
    /// Every category.
    pub const ALL: [Self; 2] = [Self::BoundedGroup, Self::SpatialCollection];

    /// The edge rule this category is required to use.
    ///
    /// The rule is a function of the category rather than a field a
    /// caller picks freely, because the two halves of §1.1's second
    /// clause are not preferences: wrapping a virtualized table and
    /// clamping a menu are both specified failures.
    #[must_use]
    pub const fn required_edge(self) -> EdgeBehaviour {
        match self {
            Self::BoundedGroup => EdgeBehaviour::Wraps,
            Self::SpatialCollection => EdgeBehaviour::Clamps,
        }
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BoundedGroup => "bounded-group",
            Self::SpatialCollection => "spatial-collection",
        }
    }
}

/// A declared single-`Tab`-stop composite (`UIF-PRIN-008`).
///
/// `enabled` holds the indices the walk may land on, in ascending
/// order — the same shape [`rove_index`] takes, so a host that already
/// computes `enabled_indices` for a model hands the same slice over.
/// A disabled member is absent rather than flagged, which is how the
/// walk skips it by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositeBoundary<'a> {
    /// What to call this composite in a report.
    pub id: &'a str,
    /// What kind of collection it is.
    pub category: CompositeCategory,
    /// The edge rule it actually applies.
    pub edge: EdgeBehaviour,
    /// Total members, enabled or not.
    pub member_count: usize,
    /// The indices the walk may land on, ascending.
    pub enabled: &'a [usize],
    /// The active / last-focused member, if the composite has one.
    pub active: Option<usize>,
}

impl<'a> CompositeBoundary<'a> {
    /// Declare a composite.
    #[must_use]
    pub const fn new(
        id: &'a str,
        category: CompositeCategory,
        edge: EdgeBehaviour,
        member_count: usize,
        enabled: &'a [usize],
        active: Option<usize>,
    ) -> Self {
        Self {
            id,
            category,
            edge,
            member_count,
            enabled,
            active,
        }
    }

    /// How many `Tab` stops this composite contributes to the surface's
    /// focus ring.
    ///
    /// Exactly one while any member is live, and **zero** when none is —
    /// a group whose every member is disabled must not be a stop the
    /// user can enter and find nothing inside.
    #[must_use]
    pub const fn tab_stops(&self) -> usize {
        if self.enabled.is_empty() { 0 } else { 1 }
    }

    /// Whether the composite is a single stop.
    #[must_use]
    pub const fn is_single_stop(&self) -> bool {
        self.tab_stops() == 1
    }

    /// Which member `Tab` lands on when it enters this composite.
    ///
    /// The active member when there is one — §1.1's "enters on its
    /// active/last-focused item" — and otherwise the first *enabled*
    /// member. Never the first member by position: falling back to
    /// index zero would ignore the enabled list and could land the
    /// keyboard on a disabled control.
    #[must_use]
    pub fn entry_member(&self) -> Option<usize> {
        match self.active {
            Some(active) if self.enabled.binary_search(&active).is_ok() => Some(active),
            _ => self.enabled.first().copied(),
        }
    }

    /// Where the arrow walk goes next, inside this composite.
    ///
    /// A wrapping composite delegates to [`rove_index`], so the shared
    /// skip-and-wrap walk stays the single implementation of that
    /// behaviour. A clamping composite stops at the end instead: the
    /// index cannot pass the last enabled member, and asking to go
    /// further returns the nearest enabled member rather than wrapping
    /// or falling off the end.
    #[must_use]
    pub fn next_within(&self, direction: RoveDirection) -> Option<usize> {
        if self.enabled.is_empty() {
            return None;
        }
        match self.edge {
            EdgeBehaviour::Wraps => rove_index(self.enabled, self.active, direction),
            EdgeBehaviour::Clamps => {
                let last = self.enabled.len() - 1;
                let Some(active) = self.active else {
                    return match direction {
                        RoveDirection::Forward => self.enabled.first().copied(),
                        RoveDirection::Backward => self.enabled.last().copied(),
                    };
                };
                let position = match self.enabled.binary_search(&active) {
                    Ok(position) => position,
                    Err(insertion) => insertion.min(last),
                };
                let target = match direction {
                    RoveDirection::Forward => position + 1,
                    RoveDirection::Backward => position.saturating_sub(1),
                };
                // Past either end, the walk stays on the nearest enabled
                // member rather than wrapping or falling off.
                self.enabled
                    .get(target.min(last))
                    .copied()
                    .or_else(|| self.enabled.get(position).copied())
            }
        }
    }
}

/// One way a declared composite contradicts the pillar
/// (`UIF-PRIN-008`).
///
/// Borrows the composite's `id` rather than owning a `String`, so a
/// whole-roster report allocates one `Vec` per finding instead of a
/// string per finding, and the lifetime of the report is visibly tied
/// to the lifetime of the roster it describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabStopViolation<'a> {
    /// The composite's edge rule contradicts what its category requires.
    WrongEdgeBehaviour {
        /// The offending composite.
        id: &'a str,
        /// What it claimed to be.
        category: CompositeCategory,
        /// The edge rule it declared.
        declared: EdgeBehaviour,
        /// The edge rule its category demands.
        required: EdgeBehaviour,
    },
    /// More enabled indices than declared members.
    EnabledExceedsMembers {
        /// The offending composite.
        id: &'a str,
        /// Declared member count.
        member_count: usize,
        /// Declared enabled count.
        enabled_count: usize,
    },
    /// The declared active member is not one the walk can land on.
    ActiveMemberNotEnabled {
        /// The offending composite.
        id: &'a str,
        /// The active member that is not enabled.
        active: usize,
    },
}

impl std::fmt::Display for TabStopViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongEdgeBehaviour {
                id,
                category,
                declared,
                required,
            } => write!(
                f,
                "'{id}' is a {} but declares edge '{}'; §1.1 requires '{}'",
                category.slug(),
                declared.slug(),
                required.slug(),
            ),
            Self::EnabledExceedsMembers {
                id,
                member_count,
                enabled_count,
            } => write!(
                f,
                "'{id}' declares {enabled_count} enabled members but only {member_count} members in total"
            ),
            Self::ActiveMemberNotEnabled { id, active } => write!(
                f,
                "'{id}' names member {active} active, but the walk cannot land on it"
            ),
        }
    }
}

/// Check one declared composite against the pillar (`UIF-PRIN-008`).
///
/// Returns an empty `Vec` for a coherent, correctly-bounded composite.
/// The reported violations borrow the composite's own `id`, so no
/// string is allocated per finding and the report's lifetime is visibly
/// tied to the roster it describes.
#[must_use]
pub fn check_composite<'a>(boundary: &CompositeBoundary<'a>) -> Vec<TabStopViolation<'a>> {
    let mut violations = Vec::new();

    let required = boundary.category.required_edge();
    if boundary.edge != required {
        violations.push(TabStopViolation::WrongEdgeBehaviour {
            id: boundary.id,
            category: boundary.category,
            declared: boundary.edge,
            required,
        });
    }

    if boundary.enabled.len() > boundary.member_count {
        violations.push(TabStopViolation::EnabledExceedsMembers {
            id: boundary.id,
            member_count: boundary.member_count,
            enabled_count: boundary.enabled.len(),
        });
    }

    // `filter` rather than a `let`-chain so the check reads as one
    // predicate and does not nest.
    if let Some(active) = boundary
        .active
        .filter(|active| boundary.enabled.binary_search(active).is_err())
    {
        violations.push(TabStopViolation::ActiveMemberNotEnabled {
            id: boundary.id,
            active,
        });
    }

    violations
}

/// Check a whole surface's roster of composites (`UIF-PRIN-008`).
///
/// Findings are reported per composite rather than stopping at the
/// first, so one pass over a host's declarations lists everything wrong
/// with it.
#[must_use]
pub fn check_roster<'a>(boundaries: &[CompositeBoundary<'a>]) -> Vec<TabStopViolation<'a>> {
    boundaries
        .iter()
        .flat_map(|boundary| check_composite(boundary))
        .collect()
}

/// The total number of `Tab` stops a roster contributes.
///
/// This is the number a host's focus ring should be. It counts
/// composites with at least one live member, never members, which is
/// the difference between a toolbar of twelve buttons being one stop
/// and being twelve.
#[must_use]
pub fn total_tab_stops(boundaries: &[CompositeBoundary<'_>]) -> usize {
    boundaries.iter().map(CompositeBoundary::tab_stops).sum()
}

/// The total member count across a roster, for comparing against
/// [`total_tab_stops`] in a report.
#[must_use]
pub fn total_members(boundaries: &[CompositeBoundary<'_>]) -> usize {
    boundaries
        .iter()
        .map(|boundary| boundary.member_count)
        .sum()
}

#[cfg(test)]
#[path = "tab_stop_tests.rs"]
mod tab_stop_tests;
