//! Tests for [`rove_index`] — the shared single-focus walk.
//!
//! The rule is small enough that every branch is reachable, so this sidecar
//! is exhaustive over the roster shapes a roving group can present:
//! empty, one member, gaps, a focus standing on a member that has since
//! been removed from the roster, and both wrap directions.

use super::RoveDirection::{Backward, Forward};
use super::rove_index;

#[test]
fn forward_from_nothing_lands_on_the_first_live_member() {
    assert_eq!(rove_index(&[0, 1, 2], None, Forward), Some(0));
    assert_eq!(rove_index(&[2, 5], None, Forward), Some(2));
}

#[test]
fn backward_from_nothing_lands_on_the_last_live_member() {
    assert_eq!(rove_index(&[0, 1, 2], None, Backward), Some(2));
    assert_eq!(rove_index(&[2, 5], None, Backward), Some(5));
}

#[test]
fn forward_steps_one_live_member_at_a_time() {
    assert_eq!(rove_index(&[0, 1, 2, 3], Some(0), Forward), Some(1));
    assert_eq!(rove_index(&[0, 1, 2, 3], Some(2), Forward), Some(3));
}

#[test]
fn backward_steps_one_live_member_at_a_time() {
    assert_eq!(rove_index(&[0, 1, 2, 3], Some(3), Backward), Some(2));
    assert_eq!(rove_index(&[0, 1, 2, 3], Some(1), Backward), Some(0));
}

#[test]
fn the_walk_wraps_at_both_ends() {
    assert_eq!(rove_index(&[0, 1, 2], Some(2), Forward), Some(0));
    assert_eq!(rove_index(&[0, 1, 2], Some(0), Backward), Some(2));
}

#[test]
fn a_disabled_run_is_stepped_over_in_one_move() {
    // Roster holds only the live members, so 1 and 2 are disabled and the
    // step from 0 goes straight to 3.
    assert_eq!(rove_index(&[0, 3], Some(0), Forward), Some(3));
    assert_eq!(rove_index(&[0, 3], Some(3), Backward), Some(0));
}

#[test]
fn a_gap_before_the_current_index_still_moves_one_step() {
    assert_eq!(rove_index(&[0, 3, 7], Some(0), Forward), Some(3));
    assert_eq!(rove_index(&[0, 3, 7], Some(3), Forward), Some(7));
    assert_eq!(rove_index(&[0, 3, 7], Some(7), Forward), Some(0));
}

#[test]
fn a_current_index_that_is_no_longer_live_lands_on_the_next_one_forward() {
    // Focus was on 1, which has since been disabled and dropped from the
    // roster. The insertion point for 1 in [0, 2, 3] is 1, so forward
    // steps to 3 and backward steps to 0 — the same answer the pinned
    // radio-group case gives.
    assert_eq!(rove_index(&[0, 2, 3], Some(1), Forward), Some(3));
    assert_eq!(rove_index(&[0, 2, 3], Some(1), Backward), Some(0));
}

#[test]
fn a_current_index_past_the_end_restarts_the_ring_at_the_insertion_point() {
    // The roster shrank underneath the focus. `position % len` puts the
    // walk at the head of the ring, and the step then moves one from
    // there rather than panicking or returning nothing.
    assert_eq!(rove_index(&[0, 1, 2], Some(9), Forward), Some(1));
    assert_eq!(rove_index(&[0, 1, 2], Some(9), Backward), Some(2));
}

#[test]
fn an_empty_roster_has_nowhere_to_go_in_either_direction() {
    assert_eq!(rove_index(&[], None, Forward), None);
    assert_eq!(rove_index(&[], Some(0), Forward), None);
    assert_eq!(rove_index(&[], Some(3), Backward), None);
}

#[test]
fn a_single_member_group_returns_to_itself() {
    assert_eq!(rove_index(&[4], Some(4), Forward), Some(4));
    assert_eq!(rove_index(&[4], Some(4), Backward), Some(4));
    assert_eq!(rove_index(&[4], None, Forward), Some(4));
}

#[test]
fn walking_the_full_ring_returns_to_the_start() {
    let roster = [1, 3, 5, 7, 9];
    let mut at = Some(1);
    for _ in 0..roster.len() {
        at = rove_index(&roster, at, Forward);
    }
    assert_eq!(at, Some(1));
}

#[test]
fn the_walk_is_pure_and_total_over_its_inputs() {
    let roster = [0, 2, 4, 6];
    for from in [None, Some(0), Some(1), Some(2), Some(5), Some(7)] {
        for direction in [Forward, Backward] {
            let first = rove_index(&roster, from, direction);
            let second = rove_index(&roster, from, direction);
            assert_eq!(first, second, "same inputs must give the same answer");
            assert!(first.is_some(), "a non-empty roster always has an answer");
            assert!(
                roster.contains(&first.unwrap()),
                "the walk may only land on a live member"
            );
        }
    }
}

#[test]
fn the_two_directions_are_never_both_the_same_move_on_a_multi_member_ring() {
    let roster = [0, 1, 2];
    for from in [Some(0), Some(1), Some(2)] {
        assert_ne!(
            rove_index(&roster, from, Forward),
            rove_index(&roster, from, Backward),
            "with three live members, forward and backward cannot agree"
        );
    }
}
