//! Tests for [`NumberBoxHost`] — the stepper and entry tiers, `UIF-ATOM-005`.
//!
//! Five groups, in order:
//!
//! 1. **The stepper's tiers.** One step on press, more on hold, one
//!    `Commit` on release.
//! 2. **The hold's arithmetic.** The cadence arrives as two values and
//!    nothing else; the same elapsed time never ticks twice.
//! 3. **Exactly-once.** Every way a settle could be attempted twice.
//! 4. **The entry.** Typed text, keyboard steps, focus loss.
//! 5. **Refusals and teardown.** Disabled and stepless models, stray
//!    events, and the no-clock determinism check.

use crate::widget::number_box::StepDirection;
use crate::widget::number_box_host::{NumberBoxEmission, NumberBoxHost, NumberBoxOutcome};
use crate::widget::range_model::{RangeKey, RangeModel};

/// A stepped model resting at `50` on `0..=100` in `5`s.
fn model() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0).with_step(Some(5.0))
}

/// A continuous model — no step at all.
fn continuous() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0)
}

/// A disabled model.
fn disabled() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0)
        .with_step(Some(5.0))
        .with_disabled(true)
}

/// A model carrying a unit suffix, for the typed-entry tests.
fn with_unit() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0)
        .with_step(Some(5.0))
        .with_unit_suffix("px")
}

/// The `f64` a `Live` frame carries.
fn live_value(outcome: &NumberBoxOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [NumberBoxEmission::Changed(tick)] => *tick.value(),
        other => panic!("expected one Live frame, got {other:?}"),
    }
}

/// The `f64` a `Commit` carries.
fn commit_value(outcome: &NumberBoxOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [NumberBoxEmission::Committed(commit)] => *commit.value(),
        other => panic!("expected one Commit, got {other:?}"),
    }
}

/// The value a `Cancel` restores.
fn cancel_value(outcome: &NumberBoxOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [NumberBoxEmission::Cancelled(cancel)] => *cancel.restored(),
        other => panic!("expected one Cancel, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. The stepper's tiers
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_host_holds_nothing() {
    let host = NumberBoxHost::new();
    assert!(!host.is_open());
    assert_eq!(host.held_direction(), None);
    assert_eq!(host.current(), None);
    assert_eq!(host.steps(), 0);
    assert_eq!(host.snapshot(), None);
    assert_eq!(host.live_ticks(), 0);
    assert!(!host.settled());
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn a_press_takes_its_step_at_once_rather_than_waiting_for_the_hold() {
    // A press that showed nothing until the repeat interval elapsed would
    // read as a dropped click.
    let mut host = NumberBoxHost::new();
    let m = model();
    let out = host.press_stepper(&m, StepDirection::Increment);
    assert_eq!(live_value(&out), 55.0, "one step, immediately");
    assert_eq!(host.steps(), 1, "the press is step one");
    assert_eq!(host.held_direction(), Some(StepDirection::Increment));
    assert!(host.pressed());
}

#[test]
fn a_press_opens_the_gesture_over_the_value_from_before_the_click() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(host.is_open());
    assert_eq!(host.current(), Some(55.0), "the hold has moved");
    assert_eq!(
        host.snapshot(),
        Some(50.0),
        "the revert target is what the field showed before the click"
    );
}

#[test]
fn the_decrement_direction_moves_the_other_way() {
    let mut host = NumberBoxHost::new();
    let m = model();
    assert_eq!(
        live_value(&host.press_stepper(&m, StepDirection::Decrement)),
        45.0
    );
    assert_eq!(
        host.held_direction(),
        Some(StepDirection::Decrement),
        "and the shape-independent direction is what is remembered"
    );
}

#[test]
fn release_commits_the_value_the_press_reached() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    let out = host.release_stepper(&m);
    assert_eq!(commit_value(&out), 55.0);
    assert!(host.committed());
    assert_eq!(host.last_tier(), "commit");
    assert_eq!(host.held_direction(), None, "the hold is over");
    assert!(!host.pressed());
}

#[test]
fn a_hold_then_release_commits_the_value_the_hold_reached() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.hold_stepper(&m, StepDirection::Increment, 500, 100);
    // Read the reached value before the release: releasing is what clears
    // the host, and the point of the test is that the release carries the
    // hold's own value rather than the model's original one.
    let reached = host.current().expect("the hold has a value");
    assert_eq!(reached, 80.0, "press plus five crossed intervals");
    assert_eq!(
        commit_value(&host.release_stepper(&m)),
        reached,
        "the commit is the value the hold reached, not the value the press started from"
    );
}

#[test]
fn a_live_frame_is_not_a_value_change_but_a_commit_is() {
    let mut host = NumberBoxHost::new();
    let m = model();
    let live = host.press_stepper(&m, StepDirection::Increment);
    assert!(!live.emissions[0].is_value_change(), "Live previews only");
    let commit = host.release_stepper(&m).emissions.remove(0);
    assert!(commit.is_value_change(), "Commit persists");
}

#[test]
fn escape_restores_the_pre_press_value() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.hold_stepper(&m, StepDirection::Increment, 2_000, 100);
    let out = host.cancel();
    assert_eq!(
        cancel_value(&out),
        50.0,
        "back to before the click, not back one step"
    );
    assert!(host.cancelled());
    assert_eq!(host.last_tier(), "cancel");
    assert_eq!(host.held_direction(), None, "and the hold is dropped");
}

// ---------------------------------------------------------------------------
// 2. The hold's arithmetic
// ---------------------------------------------------------------------------

#[test]
fn a_hold_adds_one_step_per_crossed_interval() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    for tick in 1_i64..=5 {
        let out = host.hold_stepper(&m, StepDirection::Increment, (tick as u64) * 100, 100);
        assert_eq!(
            live_value(&out),
            50.0 + 5.0 * (tick as f64 + 1.0),
            "press plus {tick} repeats"
        );
        assert_eq!(host.steps(), tick + 1);
    }
}

#[test]
fn the_same_elapsed_time_reported_twice_never_ticks_twice() {
    // The cadence is a pure function of the values handed in, so a host
    // that polls twice at the same instant must get nothing the second time.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(
        !host
            .hold_stepper(&m, StepDirection::Increment, 500, 100)
            .is_noop()
    );
    assert!(
        host.hold_stepper(&m, StepDirection::Increment, 500, 100)
            .is_noop(),
        "the same instant must not produce a second step"
    );
    assert_eq!(host.steps(), 6, "one press plus five crossed intervals");
}

#[test]
fn elapsed_time_short_of_the_first_interval_emits_nothing() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(
        host.hold_stepper(&m, StepDirection::Increment, 99, 100)
            .is_noop()
    );
    assert_eq!(host.steps(), 1, "still just the press");
}

#[test]
fn a_hold_can_catch_up_several_intervals_at_once() {
    // A host that was busy and reports a long elapsed time gets the whole
    // catch-up in one frame rather than one step per poll.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    let out = host.hold_stepper(&m, StepDirection::Increment, 500, 100);
    assert_eq!(live_value(&out), 80.0, "press plus five crossed intervals");
    assert_eq!(host.steps(), 6, "all five credited in one frame");
    assert_eq!(
        host.live_ticks(),
        2,
        "and still only two Live frames emitted, not seven"
    );
}

#[test]
fn a_catch_up_that_runs_past_the_top_stops_there_rather_than_overshooting() {
    // The same catch-up with enough intervals to leave the range. The
    // value lands on `max` and the frame still carries a real number —
    // the clamp happens before the emission, so no caller ever sees a
    // value outside `[min, max]`.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    let out = host.hold_stepper(&m, StepDirection::Increment, 10_000, 100);
    assert_eq!(live_value(&out), 100.0, "pinned at the top");
    assert!(
        live_value(&out) <= m.high(),
        "never past the declared maximum"
    );
    assert_eq!(commit_value(&host.release_stepper(&m)), 100.0);
}

#[test]
fn a_zero_repeat_interval_is_no_cadence_not_an_infinitely_fast_hold() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(
        host.hold_stepper(&m, StepDirection::Increment, 10_000, 0)
            .is_noop()
    );
    assert_eq!(host.steps(), 1, "and no division by zero");
}

#[test]
fn a_hold_in_the_other_direction_than_the_one_held_emits_nothing() {
    // The button that is down is the button that repeats.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(
        host.hold_stepper(&m, StepDirection::Decrement, 1_000, 100)
            .is_noop()
    );
    assert_eq!(host.current(), Some(55.0), "and the value did not move");
}

#[test]
fn a_hold_with_nothing_pressed_emits_nothing() {
    let mut host = NumberBoxHost::new();
    assert!(
        host.hold_stepper(&model(), StepDirection::Increment, 1_000, 100)
            .is_noop()
    );
    assert!(!host.is_open());
}

#[test]
fn a_long_hold_clamps_at_the_top_of_the_range() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    let out = host.hold_stepper(&m, StepDirection::Increment, 1_000_000, 100);
    assert_eq!(live_value(&out), 100.0, "the range still bounds the hold");
    assert_eq!(commit_value(&host.release_stepper(&m)), 100.0);
}

#[test]
fn a_long_hold_down_clamps_at_the_bottom_of_the_range() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Decrement);
    host.hold_stepper(&m, StepDirection::Decrement, 1_000_000, 100);
    assert_eq!(commit_value(&host.release_stepper(&m)), 0.0);
}

#[test]
fn a_hold_never_produces_a_non_finite_value() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    for elapsed in [1_u64, 999, 12_345, u64::MAX / 2] {
        let out = host.hold_stepper(&m, StepDirection::Increment, elapsed, 7);
        if !out.is_noop() {
            let v = live_value(&out);
            assert!(v.is_finite(), "elapsed {elapsed} produced {v}");
            assert!((0.0..=100.0).contains(&v), "and left the range");
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Exactly-once
// ---------------------------------------------------------------------------

#[test]
fn releasing_twice_commits_once() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert_eq!(host.release_stepper(&m).emissions.len(), 1);
    assert!(host.release_stepper(&m).is_noop(), "nothing left to settle");
    assert!(host.release_stepper(&m).is_noop());
}

#[test]
fn a_release_with_nothing_pressed_emits_nothing() {
    let mut host = NumberBoxHost::new();
    assert!(host.release_stepper(&model()).is_noop());
    assert!(!host.committed());
}

#[test]
fn cancelling_after_a_release_emits_nothing() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.release_stepper(&m);
    assert!(
        host.cancel().is_noop(),
        "a committed hold cannot be abandoned"
    );
    assert!(host.committed());
    assert!(!host.cancelled());
}

#[test]
fn releasing_after_a_cancel_emits_nothing() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.cancel();
    assert!(host.release_stepper(&m).is_noop());
    assert!(host.cancelled());
    assert!(!host.committed());
}

#[test]
fn a_second_hold_after_a_release_emits_nothing() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.release_stepper(&m);
    assert!(
        host.hold_stepper(&m, StepDirection::Increment, 10_000, 100)
            .is_noop()
    );
}

#[test]
fn a_second_press_after_a_settle_opens_a_fresh_gesture() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert_eq!(commit_value(&host.release_stepper(&m)), 55.0);

    host.press_stepper(&m, StepDirection::Decrement);
    assert_eq!(
        host.snapshot(),
        Some(50.0),
        "a new gesture over the resting value"
    );
    assert_eq!(commit_value(&host.release_stepper(&m)), 45.0);
}

#[test]
fn a_key_during_a_hold_settles_through_the_hold_slot() {
    // The pointer path and the keyboard path must not each settle the
    // same gesture once.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert!(host.is_open());
    assert_eq!(commit_value(&host.key(&m, RangeKey::Increase)), 55.0);
    assert!(!host.is_open(), "the key consumed the hold's gesture");
    assert!(
        host.release_stepper(&m).is_noop(),
        "so the release has nothing to do"
    );
}

// ---------------------------------------------------------------------------
// 4. The entry
// ---------------------------------------------------------------------------

#[test]
fn a_typed_entry_commits_what_it_parsed() {
    let mut host = NumberBoxHost::new();
    let m = model();
    assert_eq!(
        commit_value(&host.commit_text(&m, "73")),
        75.0,
        "snapped to the grid"
    );
    assert!(host.committed());
    assert_eq!(host.last_tier(), "commit");
}

#[test]
fn a_typed_entry_strips_its_own_unit_suffix() {
    let mut host = NumberBoxHost::new();
    let m = with_unit();
    assert_eq!(commit_value(&host.commit_text(&m, "  40 px  ")), 40.0);
}

#[test]
fn unparseable_text_commits_nothing() {
    // There is no callback for a rejected parse, and nothing sensible to
    // send: a `Commit` of the old value would be indistinguishable from
    // one that changed something.
    let mut host = NumberBoxHost::new();
    let m = model();
    for text in ["", "   ", "abc", "12.3.4", "nan", "inf", "--4"] {
        assert!(
            host.commit_text(&m, text).is_noop(),
            "`{text}` must produce no emission"
        );
    }
    assert!(!host.committed());
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn an_unparseable_entry_leaves_no_open_gesture_behind() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.commit_text(&m, "nonsense");
    assert!(!host.is_open(), "a rejected parse opens nothing");
    assert_eq!(host.current(), None);
    assert!(
        host.blur(&m).is_noop(),
        "and focus loss afterwards is silent"
    );
}

#[test]
fn a_typed_entry_after_a_hold_settles_through_the_hold_slot() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    assert_eq!(commit_value(&host.commit_text(&m, "30")), 30.0);
    assert!(!host.is_open());
    assert!(
        host.release_stepper(&m).is_noop(),
        "one settle for the whole gesture"
    );
}

#[test]
fn keyboard_steps_are_commits_not_live_frames() {
    let mut host = NumberBoxHost::new();
    let m = model();
    let out = host.key(&m, RangeKey::Increase);
    assert_eq!(commit_value(&out), 55.0);
    assert_eq!(
        out.emissions
            .iter()
            .filter(|e| matches!(e, NumberBoxEmission::Changed(_)))
            .count(),
        0,
        "§3.5 lists the keys under on_commit"
    );
    assert_eq!(host.live_ticks(), 0);
}

#[test]
fn every_key_commits_the_value_it_names() {
    let cases = [
        (RangeKey::Decrease, 45.0),
        (RangeKey::Increase, 55.0),
        (RangeKey::PageDown, 0.0),
        (RangeKey::PageUp, 100.0),
        (RangeKey::Home, 0.0),
        (RangeKey::End, 100.0),
    ];
    for (key, expected) in cases {
        let mut host = NumberBoxHost::new();
        let m = model();
        assert_eq!(
            commit_value(&host.key(&m, key)),
            expected,
            "{key:?} must commit {expected}"
        );
    }
}

#[test]
fn a_key_on_a_stepless_model_emits_nothing_but_home_and_end_still_work() {
    let mut host = NumberBoxHost::new();
    let m = continuous();
    for key in [
        RangeKey::Decrease,
        RangeKey::Increase,
        RangeKey::PageDown,
        RangeKey::PageUp,
    ] {
        assert!(host.key(&m, key).is_noop(), "{key:?} has no increment");
    }
    assert_eq!(commit_value(&host.key(&m, RangeKey::Home)), 0.0);
    assert_eq!(commit_value(&host.key(&m, RangeKey::End)), 100.0);
}

#[test]
fn blur_settles_an_open_hold() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.hold_stepper(&m, StepDirection::Increment, 500, 100);
    let reached = host.current.expect("the hold has a value");
    let out = host.blur(&m);
    assert_eq!(
        commit_value(&out),
        reached,
        "focus loss persists what the hold reached"
    );
    assert!(!host.is_open());
    assert!(host.committed());
}

#[test]
fn blur_with_nothing_open_emits_nothing() {
    let mut host = NumberBoxHost::new();
    assert!(
        host.blur(&model()).is_noop(),
        "tabbing past must not wake persistence"
    );
    assert!(!host.committed());
}

#[test]
fn blur_clears_a_stale_hold_even_when_it_emits_nothing() {
    // What blur owns is the gesture and the pressed button: a hold that
    // outlived focus would keep repeating into a field nobody is in.
    // `hovered` is deliberately *not* cleared — the pointer can still be
    // over the field after focus leaves it, and hover is driven by pointer
    // events, not by focus. Clearing it here would drop a true signal and
    // leave the control painted unhovered under a cursor that is on it.
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.cancel();
    host.hover(true);
    assert!(host.blur(&m).is_noop());
    assert_eq!(host.held_direction(), None);
    assert!(!host.pressed());
    assert!(
        host.hovered(),
        "hover survives focus, because the pointer is still there"
    );
    host.hover(false);
    assert!(!host.hovered(), "and leaves when the pointer does");
}

// ---------------------------------------------------------------------------
// 5. Refusals and teardown
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_model_refuses_every_input() {
    let mut host = NumberBoxHost::new();
    let m = disabled();
    assert!(host.press_stepper(&m, StepDirection::Increment).is_noop());
    assert!(
        host.hold_stepper(&m, StepDirection::Increment, 1_000, 100)
            .is_noop()
    );
    assert!(host.release_stepper(&m).is_noop());
    assert!(host.key(&m, RangeKey::Increase).is_noop());
    assert!(host.commit_text(&m, "42").is_noop());
    assert!(host.blur(&m).is_noop());
    assert!(host.cancel().is_noop());
    assert!(!host.is_open());
    assert_eq!(host.live_ticks(), 0);
}

#[test]
fn a_model_disabled_mid_hold_stops_the_repeats() {
    let mut host = NumberBoxHost::new();
    host.press_stepper(&model(), StepDirection::Increment);
    assert_eq!(host.live_ticks(), 1);
    assert!(
        host.hold_stepper(&disabled(), StepDirection::Increment, 10_000, 100)
            .is_noop()
    );
    assert_eq!(host.live_ticks(), 1, "and adds none");
}

#[test]
fn a_stepless_model_refuses_the_stepper_entirely() {
    // There is no distance to step by. Refusing before the snapshot is
    // taken is what keeps the release from settling a value that never
    // moved.
    let mut host = NumberBoxHost::new();
    let m = continuous();
    assert!(host.press_stepper(&m, StepDirection::Increment).is_noop());
    assert!(!host.is_open(), "and no gesture was opened");
    assert_eq!(host.steps(), 0);
    assert!(
        host.release_stepper(&m).is_noop(),
        "so nothing settles on release"
    );
}

#[test]
fn a_zero_step_model_refuses_the_stepper_too() {
    let mut host = NumberBoxHost::new();
    let m = RangeModel::new(50.0, 0.0, 100.0).with_step(Some(0.0));
    assert!(host.press_stepper(&m, StepDirection::Increment).is_noop());
    assert!(!host.is_open());
}

#[test]
fn reset_drops_everything_without_emitting() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.hold_stepper(&m, StepDirection::Increment, 1_000, 100);
    host.hover(true);
    host.reset();
    assert!(!host.is_open());
    assert_eq!(host.held_direction(), None);
    assert!(!host.pressed());
    assert!(!host.hovered());
    assert_eq!(host.current(), None);
    assert_eq!(host.steps(), 0);
    assert_eq!(host.snapshot(), None);
    assert!(!host.settled(), "the teardown owes nobody a settle");
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn reset_mid_hold_prevents_any_later_settle() {
    let mut host = NumberBoxHost::new();
    let m = model();
    host.press_stepper(&m, StepDirection::Increment);
    host.reset();
    assert!(host.release_stepper(&m).is_noop());
    assert!(host.cancel().is_noop());
    assert!(host.blur(&m).is_noop());
}

#[test]
fn the_transcript_matches_what_was_actually_emitted() {
    // The counters are a record, not the enforcement: every `Live` frame
    // this host emitted is counted, and the release adds exactly one
    // `Commit` on top of them.
    let mut host = NumberBoxHost::new();
    let m = model();
    let mut live_produced = 0;
    for tick in 0..=8 {
        let out = if tick == 0 {
            host.press_stepper(&m, StepDirection::Increment)
        } else {
            host.hold_stepper(&m, StepDirection::Increment, tick * 100, 100)
        };
        live_produced += out
            .emissions
            .iter()
            .filter(|e| matches!(e, NumberBoxEmission::Changed(_)))
            .count();
    }
    let outs = host.release_stepper(&m);
    assert_eq!(
        host.live_ticks(),
        live_produced,
        "the Live counter is exactly the Live frames emitted"
    );
    assert_eq!(
        outs.emissions.len(),
        1,
        "and the release adds exactly one Commit"
    );
    assert_eq!(outs.emissions[0].tier(), "commit");
    assert_eq!(host.last_tier(), "commit");
    assert!(host.committed());
}

#[test]
fn the_same_script_produces_the_same_transcript_twice() {
    let script = |host: &mut NumberBoxHost, m: &RangeModel| {
        let mut tiers = Vec::new();
        tiers.push(host.press_stepper(m, StepDirection::Increment).emissions[0].tier());
        for tick in 1..=6 {
            let out = host.hold_stepper(m, StepDirection::Increment, tick * 250, 250);
            if !out.is_noop() {
                tiers.push(out.emissions[0].tier());
            }
        }
        tiers.push(host.release_stepper(m).emissions[0].tier());
        tiers
    };
    let m = model();
    let mut a = NumberBoxHost::new();
    let mut b = NumberBoxHost::new();
    assert_eq!(script(&mut a, &m), script(&mut b, &m));
    assert_eq!(a.live_ticks(), b.live_ticks());
    assert_eq!(a.last_tier(), b.last_tier());
}

#[test]
fn emission_tier_names_are_the_three_spec_tiers() {
    let mut host = NumberBoxHost::new();
    let m = model();
    assert_eq!(
        host.press_stepper(&m, StepDirection::Increment).emissions[0].tier(),
        "live"
    );
    assert_eq!(host.release_stepper(&m).emissions[0].tier(), "commit");

    let mut host = NumberBoxHost::new();
    host.press_stepper(&m, StepDirection::Increment);
    assert_eq!(host.cancel().emissions[0].tier(), "cancel");
}

#[test]
fn an_outcome_reports_whether_anything_happened() {
    assert!(NumberBoxOutcome::noop().is_noop());
    assert!(NumberBoxOutcome::emissions(Vec::new()).is_noop());
    let mut host = NumberBoxHost::new();
    assert!(
        !host
            .press_stepper(&model(), StepDirection::Increment)
            .is_noop()
    );
}
