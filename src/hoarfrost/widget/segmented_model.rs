//! The segmented-control data model and its selection transitions —
//! `SegmentedModel<Id>` and the degenerate `ToggleButton`
//! (`UIF-ATOM-001b`, spec §3.1).
//!
//! §3.1b specifies one model with two modes and says a `ToggleButton` is
//! what that model becomes when it has exactly one segment. This module is
//! that sentence as code: the fields the spec names, the transition its
//! callback describes, and nothing else.
//!
//! # The selection is a set in both modes
//!
//! The spec types the selection as `BTreeSet<Id>` for *both*
//! `SingleSelect` and `MultiSelect`, and that is the load-bearing part of
//! the design rather than a convenience. A single-select control is a set
//! that never holds more than one member; a multi-select control is the
//! same set with the cap removed. One field, one transition function, and
//! the mode decides only whether the previous member is evicted when a new
//! one arrives.
//!
//! `BTreeSet` rather than `HashSet` is also load-bearing: the emission is
//! the selection, and an emission whose iteration order varies between two
//! identical user gestures is an emission a host cannot log, diff or
//! replay. A `BTreeSet` iterates in `Ord` order, so the same selection is
//! the same value every time.
//!
//! # Every transition is pure
//!
//! Same rule as [`toggle_model`](crate::widget::toggle_model): the value
//! is application-owned (§3.0.4.1), so no method here writes through
//! `&self`. [`SegmentedModel::toggle`] returns
//! `Option<Commit<BTreeSet<Id>>>` — the set the app is being *asked* to
//! adopt — and leaves the model untouched.
//!
//! # What does not emit
//!
//! A transition that would not change the selection emits nothing, which
//! is the same rule the radio group runs and for the same reason: an
//! emission means "something changed", and handing the host an event for a
//! value that did not move makes it re-run whatever a selection triggers.
//!
//! | Situation | Emits |
//! |---|---|
//! | control disabled, segment disabled, unknown id | nothing |
//! | `SingleSelect`, id already the sole selection | nothing |
//! | `SingleSelect`, any other live id | `{id}` |
//! | `MultiSelect`, live id not selected | old ∪ `{id}` |
//! | `MultiSelect`, live id already selected | old ∖ `{id}` |
//!
//! `MultiSelect` therefore always emits when the segment is live: adding
//! and removing are both changes. `SingleSelect` can be a no-op, and the
//! sidecar pins that rather than leaving it to be discovered when a
//! toolbar stops responding.
//!
//! # The `ToggleButton` is a one-segment strip, not a second control
//!
//! [`SegmentedModel::toggle_button`] builds the degenerate case the spec
//! names: one segment under [`MultiSelect`], whose id is
//! [`TOGGLE_SEGMENT_ID`]. It is not a special-cased type because there is
//! nothing in §3.1b that would justify one — the paint, the geometry, the
//! rove and the emission all come from the same code, and a
//! `ToggleButton` that was a separate implementation would be a second
//! thing to keep in agreement.
//!
//! **Why `MultiSelect` and not `SingleSelect`,** which is the one
//! non-obvious choice here. A toggle button has exactly two states and
//! clicking the lit one must turn it off. `SingleSelect`'s rule is that
//! re-picking the member you already hold changes nothing — that is what
//! makes a radio-style control behave — so a one-segment `SingleSelect`
//! strip could be switched on and then never switched off. `MultiSelect`
//! over one segment is precisely the machine a toggle is: the set flips
//! between empty and full, and with only one segment in the strip it can
//! never hold two members, so nothing the mode allows violates the
//! single-control intuition. The cap `SingleSelect` would add is
//! redundant at one segment; the removal rule `MultiSelect` adds is the
//! only thing that makes the control a toggle at all.
//!
//! [`MultiSelect`]: SelectionMode::MultiSelect
//!
//! # The rove is the shared walk
//!
//! §3.1b: "`Left`/`Right` arrow keys rove focus across segments". That
//! is §3.3's radio walk with the axis changed, so it is
//! [`rove_index`] — skip the disabled
//! members, wrap at both ends — with this model's roster handed in.
//!
//! Unit tests live in the sibling `segmented_model_tests.rs` sidecar.

use std::collections::BTreeSet;

use crate::binding::Commit;
use crate::widget::button_model::ButtonDensity;
use crate::widget::rove::{RoveDirection, rove_index};

/// The id of the single segment a `ToggleButton` is built from.
///
/// Named rather than inlined because the id is the value a host matches on
/// when it reads the selection set back out of
/// [`SegmentedModel::toggle`]'s emission. A string literal at each call
/// site is how a toggle button ends up reading a set that contains a
/// different id than the one it wrote.
pub const TOGGLE_SEGMENT_ID: &str = "toggle";

/// The label a `ToggleButton` carries when the caller does not supply
/// one.
///
/// §3.1b gives the toggle button no default text, and an empty label on a
/// control that must be identifiable is worse than a placeholder the host
/// is expected to replace.
pub const TOGGLE_DEFAULT_LABEL: &str = "Toggle";

/// How a segmented control treats its selection when a new segment is
/// picked (`UIF-ATOM-001b`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// At most one segment is selected; picking another replaces it.
    ///
    /// The mode a `ToggleButton` uses, and the mode a mutually-exclusive
    /// toolbar (alignment, view mode) uses.
    #[default]
    SingleSelect,
    /// Any number of segments may be selected; picking one flips just it.
    MultiSelect,
}

impl SelectionMode {
    /// Whether picking a segment evicts the ones already held.
    #[must_use]
    pub const fn replaces_selection(self) -> bool {
        matches!(self, Self::SingleSelect)
    }

    /// Whether the selection may hold more than one member.
    #[must_use]
    pub const fn allows_multiple(self) -> bool {
        matches!(self, Self::MultiSelect)
    }
}

/// One segment of a segmented control (§3.1b).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentItem<Id> {
    /// The caller's segment identity.
    pub id: Id,
    /// The segment's text.
    pub label: String,
    /// Whether this segment is inert.
    pub disabled: bool,
}

impl<Id> SegmentItem<Id> {
    /// A live segment.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            disabled: false,
        }
    }

    /// A disabled segment — skipped by the rove, untoggleable by click.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// The `SegmentedModel<Id>` of §3.1b.
///
/// ```text
/// segments ─────────── the strip, in display order
/// selected ─────────── the current selection set (empty is legal)
/// mode ─────────────── whether the set is capped at one
/// density ──────────── the rung the host declared (28px / 32px)
/// disabled ─────────── the whole strip is inert
/// ```
///
/// An empty `selected` is legal in both modes and is not an invalid
/// state: a toolbar with nothing chosen yet, and a `ToggleButton` in its
/// off position, are both the empty set. What is *not* legal is a
/// `selected` containing an id that is not in `segments` — see
/// [`toggle`](Self::toggle), which refuses rather than emitting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentedModel<Id> {
    /// The segments, in display order.
    pub segments: Vec<SegmentItem<Id>>,
    /// The current selection.
    pub selected: BTreeSet<Id>,
    /// Whether the selection is capped at one member.
    pub mode: SelectionMode,
    /// The rung the host declared.
    pub density: ButtonDensity,
    /// Whether the whole strip is inert.
    pub disabled: bool,
}

impl<Id> SegmentedModel<Id>
where
    Id: Clone + Ord,
{
    /// A single-select strip of the given segments, nothing selected.
    #[must_use]
    pub fn new(segments: Vec<SegmentItem<Id>>) -> Self {
        Self {
            segments,
            selected: BTreeSet::new(),
            mode: SelectionMode::SingleSelect,
            density: ButtonDensity::Standard,
            disabled: false,
        }
    }

    /// A multi-select strip of the given segments, nothing selected.
    #[must_use]
    pub fn multi(segments: Vec<SegmentItem<Id>>) -> Self {
        Self {
            mode: SelectionMode::MultiSelect,
            ..Self::new(segments)
        }
    }

    /// Set the starting selection to a single id.
    #[must_use]
    pub fn with_selected(mut self, id: Id) -> Self {
        self.selected = BTreeSet::from([id]);
        self
    }

    /// Set the starting selection to a whole set.
    #[must_use]
    pub fn with_selection(mut self, selected: BTreeSet<Id>) -> Self {
        self.selected = selected;
        self
    }

    /// Set the mode.
    #[must_use]
    pub fn with_mode(mut self, mode: SelectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Declare the rung the host has room for.
    #[must_use]
    pub fn with_density(mut self, density: ButtonDensity) -> Self {
        self.density = density;
        self
    }

    /// Make the whole strip inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Whether the segment at `index` can be landed on or toggled.
    ///
    /// The strip's flag dominates: a disabled strip has no live segment
    /// whatever its members declare.
    #[must_use]
    pub fn segment_enabled(&self, index: usize) -> bool {
        !self.disabled && self.segments.get(index).is_some_and(|s| !s.disabled)
    }

    /// The indices the rove is allowed to land on, in display order.
    #[must_use]
    pub fn enabled_indices(&self) -> Vec<usize> {
        (0..self.segments.len())
            .filter(|&index| self.segment_enabled(index))
            .collect()
    }

    /// The display index of `id`, if it is in the strip.
    #[must_use]
    pub fn index_of(&self, id: &Id) -> Option<usize> {
        self.segments.iter().position(|s| &s.id == id)
    }

    /// Whether `id` is currently selected.
    #[must_use]
    pub fn is_selected(&self, id: &Id) -> bool {
        self.selected.contains(id)
    }

    /// How many segments are selected.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }
}

impl SegmentedModel<&'static str> {
    /// The `ToggleButton` of §3.1b — a one-segment single-select strip.
    ///
    /// `on` seeds the selection: `true` puts [`TOGGLE_SEGMENT_ID`] in
    /// the set, `false` leaves it empty. The segment is always live; a
    /// disabled toggle button is a [`disabled`](SegmentedModel::disabled)
    /// strip, which is how the rest of the crate spells it.
    ///
    /// It lives on the `&'static str` instantiation rather than the
    /// generic one because the whole point of the constructor is that it
    /// names the segment for you — and a name is only meaningful against
    /// a string id. A generic `toggle_button` would have to be handed an
    /// id, at which point it is just `new(vec![...])`.
    #[must_use]
    pub fn toggle_button(on: bool) -> Self {
        let mut model = Self::multi(vec![SegmentItem::new(
            TOGGLE_SEGMENT_ID,
            TOGGLE_DEFAULT_LABEL,
        )]);
        if on {
            model.selected.insert(TOGGLE_SEGMENT_ID);
        }
        model
    }

    /// Whether a `ToggleButton` strip is in its on position.
    ///
    /// Reads [`TOGGLE_SEGMENT_ID`] rather than "the first segment", so a
    /// host that built its own one-segment strip under a different id is
    /// not silently reported as off.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.is_selected(&TOGGLE_SEGMENT_ID)
    }
}

impl<Id> SegmentedModel<Id>
where
    Id: Clone + Ord,
{
    /// The index the roving focus lands on, given where it is now.
    ///
    /// [`rove_index`] over this strip's roster: disabled segments are
    /// skipped, the walk wraps at both ends, and a `from` that has since
    /// been disabled lands on the next live segment in the direction of
    /// travel.
    #[must_use]
    pub fn rove(&self, from: Option<usize>, direction: RoveDirection) -> Option<usize> {
        rove_index(&self.enabled_indices(), from, direction)
    }

    /// The `on_selection_change(BTreeSet<Id>)` an activation produces.
    ///
    /// `None` when the transition would not change the selection — see
    /// the [module docs](self#what-does-not-emit) for the full table.
    /// Otherwise the *whole new set*, not the delta: §3.1b's callback
    /// signature is the set, so a host never has to reconstruct one from
    /// a difference it was not told the rules for.
    #[must_use]
    pub fn toggle(&self, id: &Id) -> Option<Commit<BTreeSet<Id>>> {
        if self.disabled {
            return None;
        }
        if !self.segments.iter().any(|s| &s.id == id && !s.disabled) {
            return None;
        }

        let next = if self.mode.replaces_selection() {
            if self.selected.len() == 1 && self.selected.contains(id) {
                return None;
            }
            BTreeSet::from([id.clone()])
        } else if self.selected.contains(id) {
            self.selected
                .difference(&BTreeSet::from([id.clone()]))
                .cloned()
                .collect()
        } else {
            self.selected
                .union(&BTreeSet::from([id.clone()]))
                .cloned()
                .collect()
        };

        Some(Commit::new(next))
    }

    /// The `on_selection_change` a click on the segment at `index`
    /// produces.
    ///
    /// [`toggle`](Self::toggle) addressed by index rather than by id, for
    /// the widget layer, which resolves a pointer position to a segment
    /// before it knows what that segment is called.
    #[must_use]
    pub fn toggle_index(&self, index: usize) -> Option<Commit<BTreeSet<Id>>> {
        let segment = self.segments.get(index)?;
        self.toggle(&segment.id)
    }
}

#[cfg(test)]
#[path = "segmented_model_tests.rs"]
mod segmented_model_tests;
