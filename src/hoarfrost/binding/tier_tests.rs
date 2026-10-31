//! Tests for the §3.0.4.2 callback tiers and the live coalescer.
//!
//! The load-bearing test here is the one that proves `Live` and `Commit` are
//! genuinely different types rather than two names for one thing, because the
//! whole section depends on a consumer being able to tell them apart.

use std::any::TypeId;

use crate::binding::tier::Commit;
use crate::binding::tier::Live;
use crate::binding::tier::LiveCoalescer;

// ---------------------------------------------------------------------------
// The tiers are distinct types
// ---------------------------------------------------------------------------

#[test]
fn live_and_commit_are_different_types_not_two_names_for_one_type() {
    // If a shared `Payload` trait ever made these interchangeable at the type
    // level, their TypeIds would still differ — but a generic bound that accepted
    // both would stop being caught here, which is why the assertion is paired with
    // the no-common-trait note in the module docs.
    assert_ne!(TypeId::of::<Live<String>>(), TypeId::of::<Commit<String>>());
    assert_ne!(TypeId::of::<Live<i32>>(), TypeId::of::<Commit<i32>>());
}

#[test]
fn a_commit_carries_its_value_to_the_persistence_layer() {
    let commit = Commit::new("settled".to_string());
    assert_eq!(commit.value(), "settled");
    assert_eq!(commit.into_value(), "settled");
}

#[test]
fn a_live_tick_carries_its_value_to_the_preview_layer() {
    let live = Live::new(3.5_f32);
    assert_eq!(*live.value(), 3.5);
    assert_eq!(live.into_value(), 3.5);
}

#[test]
fn both_tiers_render_their_payload_in_debug() {
    assert!(format!("{:?}", Live::new("v")).contains('v'));
    assert!(format!("{:?}", Commit::new("c")).contains('c'));
}

// ---------------------------------------------------------------------------
// LiveCoalescer: at most one pending tick, lossless
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_coalescer_holds_nothing() {
    let mut coalescer = LiveCoalescer::<String>::new();
    assert!(!coalescer.is_pending());
    assert_eq!(coalescer.coalesced(), 0);
    assert_eq!(coalescer.take(), None);
}

#[test]
fn the_default_coalescer_behaves_like_a_fresh_one() {
    let mut coalescer = LiveCoalescer::<u32>::default();
    assert!(!coalescer.is_pending());
    assert!(!coalescer.push(7), "an empty slot displaces nothing");
    assert_eq!(coalescer.take(), Some(7));
}

#[test]
fn the_first_push_is_not_a_coalescing_event() {
    let mut coalescer = LiveCoalescer::<u32>::new();
    assert!(!coalescer.push(1), "nothing was displaced");
    assert_eq!(coalescer.coalesced(), 0);
    assert!(coalescer.is_pending());
}

#[test]
fn a_second_push_displaces_the_first_and_is_counted() {
    let mut coalescer = LiveCoalescer::<u32>::new();
    coalescer.push(1);
    assert!(coalescer.push(2), "the older pending value was displaced");
    assert_eq!(coalescer.coalesced(), 1);
    assert_eq!(coalescer.take(), Some(2), "the newest value survives");
}

#[test]
fn a_long_scrub_collapses_to_its_last_value_and_nothing_else() {
    // 144 Hz across a slider: many ticks, one intent. Because `Live` carries the
    // whole value, dropping the intermediates loses no information.
    let mut coalescer = LiveCoalescer::<usize>::new();
    let ticks: usize = 200;
    for tick in 1..=ticks {
        coalescer.push(tick);
    }
    assert_eq!(coalescer.coalesced(), ticks - 1);
    assert_eq!(coalescer.take(), Some(ticks));
    assert!(!coalescer.is_pending());
}

#[test]
fn taking_twice_yields_nothing_the_second_time() {
    let mut coalescer = LiveCoalescer::<u32>::new();
    coalescer.push(9);
    assert_eq!(coalescer.take(), Some(9));
    assert_eq!(coalescer.take(), None);
}

#[test]
fn pushing_after_a_drain_is_not_a_coalescing_event() {
    let mut coalescer = LiveCoalescer::<u32>::new();
    coalescer.push(1);
    coalescer.take();
    assert!(
        !coalescer.push(2),
        "the slot was empty, so nothing was displaced"
    );
    assert_eq!(coalescer.coalesced(), 0, "the counter must not drift");
}

#[test]
fn coalescing_survives_repeated_fill_and_drain_cycles() {
    let mut coalescer = LiveCoalescer::<u32>::new();
    for cycle in 0..5_u32 {
        let base = cycle * 10;
        coalescer.push(base + 1);
        coalescer.push(base + 2);
        assert_eq!(coalescer.take(), Some(base + 2));
    }
    assert_eq!(coalescer.coalesced(), 5, "one displacement per cycle");
}

#[test]
fn a_coalescer_never_grows_beyond_one_value() {
    // The memory bound is the point of the type: 10 000 pushes must not retain
    // 10 000 values.
    let mut coalescer = LiveCoalescer::<String>::new();
    for index in 0..10_000_u32 {
        coalescer.push(format!("v{index}"));
    }
    assert_eq!(coalescer.take(), Some("v9999".to_string()));
    assert_eq!(coalescer.coalesced(), 9_999);
}
