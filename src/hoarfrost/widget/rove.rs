//! The roving-focus walk shared by every single-focus group in the crate.
//!
//! A roving group is a set of controls that share **one** keyboard focus:
//! the arrow keys move it between members instead of `Tab` doing so. §3.3
//! specifies that walk for a radio group ("`Up`/`Down`/`Left`/`Right`
//! roves focus … within the group") and §3.1 specifies the same walk for
//! a segmented control ("`Left`/`Right` arrow keys rove focus across
//! segments"). They are the same rule, so they are the same function.
//!
//! # The rule, in the two sentences that matter
//!
//! * a disabled member is **skipped**, not landed on and refused —
//!   landing on a control the user then cannot activate is worse than
//!   stepping past it;
//! * the walk **wraps** at both ends, because a group with no ends is the
//!   only behaviour that never leaves a keyboard user stuck at an edge
//!   with nothing left to try.
//!
//! # Why this is a function over indices rather than a method on a model
//!
//! Every model that roves has already computed *which* members the walk
//! may land on — [`RadioGroupModel::enabled_indices`] for the radio
//! group, [`SegmentedModel::enabled_indices`] for the segmented control.
//! What is identical across them is not the roster but the **walk**: the
//! skip, the wrap, and the answer to "where do I go when the caller is
//! standing on a member that has since been disabled". Taking the
//! resolved `&[usize]` keeps that shared logic in one place and leaves
//! each model free to decide its own eligibility, which is the part
//! that actually differs.
//!
//! [`RadioGroupModel::enabled_indices`]: crate::widget::toggle_model::RadioGroupModel::enabled_indices
//! [`SegmentedModel::enabled_indices`]: crate::widget::segmented_model::SegmentedModel::enabled_indices
//!
//! Unit tests live in the sibling `rove_tests.rs` sidecar.

/// Which way the roving focus moves through a single-focus group.
///
/// `Down`/`Right` are `Forward` and `Up`/`Left` are `Backward`, for
/// every orientation. §3.3 does not split the axes by layout, and a
/// group whose arrow mapping changed with its layout would make muscle
/// memory layout-dependent. The direction is therefore a property of
/// the *key*, never of the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoveDirection {
    /// Toward higher indices.
    Forward,
    /// Toward lower indices.
    Backward,
}

/// Where the roving focus lands, walking `enabled` from `from`.
///
/// `enabled` must be the indices the walk is allowed to land on, in
/// ascending order — the caller's [`enabled_indices`] in display order.
/// The walk skips everything else because it is not in the list at all,
/// so "skip disabled" is achieved by absence rather than by a
/// conditional that a later edit could invert.
///
/// `from` is the current focus, not the current *selection*: they
/// coincide after a rove but not after a click on an already-focused
/// group, and conflating them would make the first arrow key jump back
/// to the selected member.
///
/// Returns `None` only when there is nowhere to land — an empty roster.
/// A `from` that is missing from `enabled` (the member under focus was
/// disabled, or the list shrank underneath it) is not an error: the
/// walk lands on the next live index in the direction of travel, which
/// is the insertion point wrapped.
///
/// [`enabled_indices`]: crate::widget::toggle_model::RadioGroupModel::enabled_indices
#[must_use]
pub fn rove_index(
    enabled: &[usize],
    from: Option<usize>,
    direction: RoveDirection,
) -> Option<usize> {
    if enabled.is_empty() {
        return None;
    }

    let Some(from) = from else {
        return match direction {
            RoveDirection::Forward => enabled.first().copied(),
            RoveDirection::Backward => enabled.last().copied(),
        };
    };

    // Walk the enabled ring rather than the raw index space, so a
    // disabled run between two live members is stepped over in one move
    // and wrapping is a position lookup rather than a special case at
    // each end.
    let position = match enabled.binary_search(&from) {
        Ok(position) => position,
        // `from` is on a disabled member (or past the end): land on the
        // next live one in the direction of travel, which is the
        // insertion point wrapped.
        Err(position) => position % enabled.len(),
    };

    let next = match direction {
        RoveDirection::Forward => (position + 1) % enabled.len(),
        RoveDirection::Backward => (position + enabled.len() - 1) % enabled.len(),
    };

    enabled.get(next).copied()
}

#[cfg(test)]
#[path = "rove_tests.rs"]
mod rove_tests;
