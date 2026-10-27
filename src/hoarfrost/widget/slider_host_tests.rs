//! Tests for [`SliderHost`] — the §3.5 scrub contract, `UIF-ATOM-005`.
//!
//! Five groups, in order:
//!
//! 1. **The tiers.** `Live` per frame, `Commit` exactly once on release,
//!    `Cancel` on `Escape` carrying the pre-drag value.
//! 2. **Exactly-once.** Every way a settle could be attempted twice, and
//!    the slot's refusal each time.
//! 3. **Pointer capture.** The thumb keeps tracking when the pointer
//!    leaves the track, and the capture ends with the settle rather than
//!    with the geometry.
//! 4. **The keyboard.** Every keyed move is a `Commit`, and a key that
//!    arrives mid-drag settles *through* the drag rather than beside it.
//! 5. **Refusals and teardown.** Disabled models, stray events, and the
//!    teardown path that owes nobody a settle.

use crate::widget::range_model::RangeKey;
use crate::widget::range_model::RangeModel;
use crate::widget::slider_host::{SliderEmission, SliderHost, SliderOutcome};

/// A stepped model resting at `50` on `0..=100` in `5`s.
fn model() -> RangeModel {
    RangeModel::new(50.0, 0.0, 100.0).with_step(Some(5.0))
}

/// The same stepped model resting at `value`.
///
/// This is the shape a controlled host is handed between commits: the
/// app has applied the last `Commit` and passes the updated model back
/// in before the next event arrives.
fn model_at(value: f64) -> RangeModel {
    RangeModel::new(value, 0.0, 100.0).with_step(Some(5.0))
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

/// The `f64` a `Live` frame carries, panicking if the tier is wrong.
fn live_value(outcome: &SliderOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [SliderEmission::Changed(tick)] => *tick.value(),
        other => panic!("expected one Live frame, got {other:?}"),
    }
}

/// The `f64` a `Commit` carries, panicking if the tier is wrong.
fn commit_value(outcome: &SliderOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [SliderEmission::Committed(commit)] => *commit.value(),
        other => panic!("expected one Commit, got {other:?}"),
    }
}

/// The value a `Cancel` restores, panicking if the tier is wrong.
fn cancel_value(outcome: &SliderOutcome) -> f64 {
    match outcome.emissions.as_slice() {
        [SliderEmission::Cancelled(cancel)] => *cancel.restored(),
        other => panic!("expected one Cancel, got {other:?}"),
    }
}

/// Press, scrub through `fractions`, and release.
fn scrub(host: &mut SliderHost, model: &RangeModel, fractions: &[f32]) -> Vec<SliderOutcome> {
    let mut out = Vec::new();
    if let Some(first) = fractions.first() {
        out.push(host.press(model, *first));
        for f in &fractions[1..] {
            out.push(host.drag(model, *f));
        }
    }
    out.push(host.release(model));
    out
}

// ---------------------------------------------------------------------------
// 1. The tiers
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_host_holds_nothing() {
    let host = SliderHost::new();
    assert!(!host.is_open());
    assert!(!host.is_capturing());
    assert!(!host.hovered());
    assert!(!host.pressed());
    assert_eq!(host.current(), None);
    assert_eq!(host.snapshot(), None);
    assert_eq!(host.live_ticks(), 0);
    assert!(!host.settled());
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn a_press_jumps_the_value_to_where_the_pointer_landed() {
    // Positioning is absolute, not a delta from the thumb's previous spot:
    // clicking the groove goes to the groove, which is what a groove is for.
    let mut host = SliderHost::new();
    let m = model();
    let out = host.press(&m, 0.8);
    assert_eq!(live_value(&out), 80.0, "0.8 of 0..=100 snapped to 80");
    assert_eq!(host.current(), Some(80.0));
}

#[test]
fn a_press_opens_the_capture_and_arms_the_drag() {
    let mut host = SliderHost::new();
    host.press(&model(), 0.5);
    assert!(host.is_open(), "the gesture is held");
    assert!(
        host.is_capturing(),
        "and the pointer belongs to the thumb now"
    );
    assert!(host.pressed());
}

#[test]
fn the_snapshot_is_the_value_from_before_the_drag_not_its_first_frame() {
    // Escape has to hand back what the user saw when they grabbed the
    // control. Snapshotting after the move would revert them to where their
    // own click had already put them.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.9);
    assert_eq!(host.current(), Some(90.0), "the drag has moved");
    assert_eq!(
        host.snapshot(),
        Some(50.0),
        "the revert target is still the resting value"
    );
}

#[test]
fn each_drag_frame_produces_exactly_one_live_tick() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.2);
    for i in 0..20 {
        let out = host.drag(&m, 0.2 + i as f32 * 0.03);
        assert_eq!(out.emissions.len(), 1, "one frame in, one tick out");
        assert_eq!(out.emissions[0].tier(), "live");
    }
    assert_eq!(host.live_ticks(), 21, "the press's own frame plus twenty");
}

#[test]
fn live_frames_do_not_settle_the_gesture() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.3);
    for i in 0..10 {
        host.drag(&m, 0.3 + i as f32 * 0.05);
    }
    assert!(host.is_open(), "a scrub is not a decision until release");
    assert!(!host.settled());
    assert_eq!(host.last_tier(), "live");
}

#[test]
fn release_produces_exactly_one_commit_at_the_value_the_drag_reached() {
    let mut host = SliderHost::new();
    let m = model();
    let outs = scrub(&mut host, &m, &[0.2, 0.4, 0.6, 0.7]);
    assert_eq!(outs.last().unwrap().emissions.len(), 1);
    assert_eq!(commit_value(outs.last().unwrap()), 70.0);
    assert!(host.committed());
    assert_eq!(host.last_tier(), "commit");
}

#[test]
fn a_scrub_emits_many_live_frames_and_exactly_one_commit() {
    let mut host = SliderHost::new();
    let m = model();
    let outs = scrub(&mut host, &m, &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5]);
    let live = outs
        .iter()
        .flat_map(|o| &o.emissions)
        .filter(|e| matches!(e, SliderEmission::Changed(_)))
        .count();
    let commits = outs
        .iter()
        .flat_map(|o| &o.emissions)
        .filter(|e| matches!(e, SliderEmission::Committed(_)))
        .count();
    assert_eq!(live, 6, "one Live per frame");
    assert_eq!(commits, 1, "one Commit for the whole drag");
}

#[test]
fn escape_restores_the_pre_drag_value() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.9);
    host.drag(&m, 0.95);
    let out = host.cancel();
    assert_eq!(cancel_value(&out), 50.0, "back to where the user started");
    assert!(host.cancelled());
    assert_eq!(host.last_tier(), "cancel");
}

#[test]
fn escape_releases_the_capture() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    assert!(host.is_capturing());
    host.cancel();
    assert!(
        !host.is_capturing(),
        "the gesture is gone so the capture is too"
    );
    assert!(!host.pressed());
    assert_eq!(host.current(), None);
}

#[test]
fn a_live_frame_is_not_a_value_change_but_a_commit_is() {
    let mut host = SliderHost::new();
    let m = model();
    let live = host.press(&m, 0.4);
    assert!(!live.emissions[0].is_value_change(), "Live previews only");
    let commit = host.release(&m).emissions.remove(0);
    assert!(commit.is_value_change(), "Commit persists");
}

#[test]
fn a_cancel_is_a_value_change() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.4);
    let cancel = host.cancel().emissions.remove(0);
    assert!(cancel.is_value_change(), "the revert writes too");
}

// ---------------------------------------------------------------------------
// 2. Exactly-once
// ---------------------------------------------------------------------------

#[test]
fn releasing_twice_commits_once() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    assert_eq!(
        host.release(&m).emissions.len(),
        1,
        "the first release settles"
    );
    assert!(
        host.release(&m).is_noop(),
        "the second has nothing to settle"
    );
    assert!(host.release(&m).is_noop(), "and a third");
}

#[test]
fn cancelling_after_a_release_emits_nothing() {
    // The two settling paths are mutually exclusive: a drag that committed
    // cannot also be abandoned.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.release(&m);
    assert!(host.cancel().is_noop(), "no gesture left to abandon");
    assert!(host.committed());
    assert!(
        !host.cancelled(),
        "and the transcript says commit, not cancel"
    );
}

#[test]
fn releasing_after_a_cancel_emits_nothing() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.cancel();
    assert!(host.release(&m).is_noop(), "the cancel already settled it");
    assert!(host.cancelled());
    assert!(!host.committed());
}

#[test]
fn blurring_after_a_release_emits_nothing() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.release(&m);
    assert!(
        host.blur(&m).is_noop(),
        "focus loss after a commit is silent"
    );
}

#[test]
fn a_drag_with_no_press_emits_nothing() {
    // A position event nobody pressed over must not invent a tick.
    let mut host = SliderHost::new();
    let m = model();
    assert!(host.drag(&m, 0.5).is_noop());
    assert!(!host.is_open());
    assert_eq!(host.live_ticks(), 0);
}

#[test]
fn a_release_with_no_press_emits_nothing() {
    let mut host = SliderHost::new();
    assert!(host.release(&model()).is_noop());
    assert!(!host.committed());
}

#[test]
fn escape_on_a_resting_slider_emits_nothing() {
    let mut host = SliderHost::new();
    assert!(host.cancel().is_noop(), "no gesture, nothing to abandon");
    assert!(!host.cancelled());
}

#[test]
fn a_second_drag_after_a_settle_opens_a_fresh_gesture() {
    // Exactly-once is per gesture, not per host lifetime.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.2);
    assert_eq!(commit_value(&host.release(&m)), 20.0);

    host.press(&m, 0.8);
    assert!(host.is_open(), "a new gesture opened");
    assert_eq!(
        host.snapshot(),
        Some(50.0),
        "over the model's resting value"
    );
    assert_eq!(commit_value(&host.release(&m)), 80.0);
}

// ---------------------------------------------------------------------------
// 3. Pointer capture
// ---------------------------------------------------------------------------

#[test]
fn the_thumb_keeps_tracking_after_the_pointer_leaves_the_track() {
    // The whole reason `release` takes no `inside` flag. A pointer dragged
    // past the end of the groove still has a value, and the control still
    // has to settle it.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.hover(false);
    let out = host.drag(&m, 1.0);
    assert_eq!(
        live_value(&out),
        100.0,
        "still tracking while the pointer is off-control"
    );
    assert!(host.is_capturing(), "and still captured");
    assert_eq!(
        commit_value(&host.release(&m)),
        100.0,
        "released outside, still settled"
    );
}

#[test]
fn a_release_outside_the_track_leaves_no_open_gesture() {
    // The leaked-drag failure: a commit gated on the pointer being inside
    // would leave this gesture open forever, `Live`-only, its value never
    // persisted.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.drag(&m, 1.4);
    host.hover(false);
    assert_eq!(host.release(&m).emissions.len(), 1, "it settles");
    assert!(!host.is_open(), "and nothing is left open");
    assert!(!host.is_capturing(), "and the capture is released");
    assert!(host.committed());
}

#[test]
fn a_pointer_dragged_far_off_both_ends_still_settles_to_the_nearest_end() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, -3.0);
    assert_eq!(host.current(), Some(0.0), "far left reads the low end");
    host.drag(&m, 7.5);
    assert_eq!(host.current(), Some(100.0), "far right reads the high end");
    assert_eq!(commit_value(&host.release(&m)), 100.0);
}

#[test]
fn hover_events_emit_nothing() {
    let mut host = SliderHost::new();
    host.hover(true);
    host.hover(false);
    assert_eq!(host.live_ticks(), 0, "hover paints, it does not emit");
    assert!(!host.is_open());
}

#[test]
fn leaving_the_control_mid_drag_does_not_end_the_capture() {
    // The point of the capture is that leaving changes nothing.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.hover(false);
    assert!(host.is_capturing(), "hover-leave is not a release");
    assert!(host.is_open());
    assert_eq!(
        live_value(&host.drag(&m, 0.7)),
        70.0,
        "and the drag still moves"
    );
}

#[test]
fn the_capture_ends_at_the_settle_not_at_the_geometry() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    assert!(host.is_capturing());
    host.release(&m);
    assert!(!host.is_capturing(), "released");

    let mut host = SliderHost::new();
    host.press(&m, 0.5);
    host.cancel();
    assert!(!host.is_capturing(), "cancelled");
}

#[test]
fn a_drag_event_after_the_capture_ends_emits_nothing() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.release(&m);
    assert!(host.drag(&m, 0.9).is_noop(), "the capture is closed");
}

// ---------------------------------------------------------------------------
// 4. The keyboard
// ---------------------------------------------------------------------------

#[test]
fn a_keyboard_step_is_a_commit_not_a_live_frame() {
    // §3.5 lists the keys under `on_commit`. A keyed step is a decided
    // value, not a preview of one.
    let mut host = SliderHost::new();
    let m = model();
    let out = host.key(&m, RangeKey::Increase);
    assert_eq!(commit_value(&out), 55.0);
    assert_eq!(
        out.emissions
            .iter()
            .filter(|e| matches!(e, SliderEmission::Changed(_)))
            .count(),
        0,
        "no Live tier for a keyed step"
    );
    assert_eq!(host.live_ticks(), 0, "and none counted");
    assert!(host.committed());
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
        let mut host = SliderHost::new();
        let m = model();
        assert_eq!(
            commit_value(&host.key(&m, key)),
            expected,
            "{key:?} must commit {expected}"
        );
    }
}

#[test]
fn a_run_of_held_keys_is_a_run_of_commits() {
    // Holding `→` produces repeated commits rather than one scrub with a
    // settle at the end, which is what putting the keys under `on_commit`
    // means in practice.
    //
    // The model is re-seeded between presses because that is the contract
    // a controlled host runs under: the app applies each `Commit` and
    // hands the updated model back before the next key arrives. A host
    // that stepped from a value it retained for itself instead would drift
    // out of step with anything the app changed for another reason, and
    // would be unable to tell the difference between its own last move and
    // somebody else's.
    let mut host = SliderHost::new();
    for step in 1..=10 {
        let m = model_at(50.0 + 5.0 * (f64::from(step) - 1.0));
        let out = host.key(&m, RangeKey::Increase);
        assert_eq!(
            commit_value(&out),
            50.0 + 5.0 * f64::from(step),
            "press {step}"
        );
    }
    assert_eq!(host.live_ticks(), 0, "never a Live tier");
    assert!(host.committed(), "and the last thing it did was commit");
}

#[test]
fn a_key_mid_drag_settles_through_the_drag_slot() {
    // The two input paths must not each settle the same gesture once.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    assert!(host.is_open());
    assert_eq!(commit_value(&host.key(&m, RangeKey::Increase)), 55.0);
    assert!(!host.is_open(), "the key consumed the drag's gesture");
    assert!(
        host.release(&m).is_noop(),
        "so the release has nothing to settle"
    );
}

#[test]
fn a_key_mid_drag_releases_the_capture() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    assert!(host.is_capturing());
    host.key(&m, RangeKey::Increase);
    assert!(
        !host.is_capturing(),
        "the keyed settle ends the pointer capture too"
    );
    assert!(!host.pressed());
    assert_eq!(host.current(), None);
}

#[test]
fn a_key_on_a_continuous_model_emits_nothing() {
    // §3.5 defines each key as a multiple of `step`, and there is no
    // multiple of nothing.
    let mut host = SliderHost::new();
    let m = continuous();
    for key in [
        RangeKey::Decrease,
        RangeKey::Increase,
        RangeKey::PageDown,
        RangeKey::PageUp,
    ] {
        assert!(host.key(&m, key).is_noop(), "{key:?} on a stepless model");
    }
    assert!(!host.committed());
}

#[test]
fn home_and_end_work_on_a_continuous_model() {
    // They name the ends rather than a distance.
    let mut host = SliderHost::new();
    let m = continuous();
    assert_eq!(commit_value(&host.key(&m, RangeKey::Home)), 0.0);
    assert_eq!(commit_value(&host.key(&m, RangeKey::End)), 100.0);
}

#[test]
fn blur_settles_an_open_drag() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.6);
    let out = host.blur(&m);
    assert_eq!(commit_value(&out), 60.0, "focus loss is the spec's settle");
    assert!(!host.is_open());
    assert!(host.committed());
}

#[test]
fn blur_with_nothing_open_emits_nothing() {
    // Tabbing through a slider the user never touched must not wake a
    // persistence layer.
    let mut host = SliderHost::new();
    assert!(host.blur(&model()).is_noop());
    assert!(!host.committed());
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn blur_clears_the_pointer_even_when_it_emits_nothing() {
    // What blur owns is the drag and the capture: a capture that outlived
    // focus would keep the thumb tracking a pointer nobody is dragging.
    // `hovered` is deliberately *not* cleared — the pointer can still be
    // over the track after focus leaves, and hover is driven by pointer
    // events, not by focus. Clearing it here would drop a true signal and
    // leave the control painted unhovered under a cursor that is on it.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.cancel();
    host.hover(true);
    assert!(host.blur(&m).is_noop());
    assert!(!host.is_capturing(), "no capture survives");
    assert!(!host.pressed(), "and no press does");
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
    let mut host = SliderHost::new();
    let m = disabled();
    assert!(host.press(&m, 0.5).is_noop(), "press");
    assert!(host.drag(&m, 0.6).is_noop(), "drag");
    assert!(host.release(&m).is_noop(), "release");
    assert!(host.key(&m, RangeKey::Increase).is_noop(), "key");
    assert!(host.blur(&m).is_noop(), "blur");
    assert!(host.cancel().is_noop(), "cancel");
    assert!(!host.is_open());
    assert!(!host.is_capturing());
    assert_eq!(host.live_ticks(), 0);
}

#[test]
fn a_model_disabled_mid_drag_stops_the_scrub_emitting() {
    // The host disabled the control underneath an open drag. The drag must
    // not keep producing frames for a control that can no longer accept
    // them.
    let mut host = SliderHost::new();
    let live = model();
    host.press(&live, 0.5);
    assert_eq!(host.live_ticks(), 1);
    assert!(
        host.drag(&disabled(), 0.7).is_noop(),
        "a disabled model stops the frames"
    );
    assert_eq!(host.live_ticks(), 1, "and adds none");
}

#[test]
fn reset_drops_everything_without_emitting() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.drag(&m, 0.7);
    host.hover(true);
    host.reset();
    assert!(!host.is_open());
    assert!(!host.is_capturing());
    assert!(!host.pressed());
    assert!(!host.hovered());
    assert_eq!(host.current(), None);
    assert_eq!(host.snapshot(), None);
    assert!(!host.settled(), "the teardown owes nobody a settle");
    assert_eq!(host.last_tier(), "none");
}

#[test]
fn reset_mid_drag_prevents_any_later_settle() {
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.5);
    host.reset();
    assert!(host.release(&m).is_noop());
    assert!(host.cancel().is_noop());
    assert!(host.blur(&m).is_noop());
}

#[test]
fn a_degenerate_range_produces_a_live_frame_at_the_one_value_and_still_settles() {
    // Nothing to travel, but the gesture still has to close rather than
    // hang open on a control with no width.
    let mut host = SliderHost::new();
    let m = RangeModel::new(7.0, 4.0, 4.0).with_step(Some(1.0));
    let out = host.press(&m, 0.5);
    assert_eq!(live_value(&out), 4.0, "the one value the range has");
    assert_eq!(commit_value(&host.release(&m)), 4.0, "and it settles");
    assert!(!host.is_open());
}

#[test]
fn a_scrub_on_an_inverted_declared_range_tracks_the_normalised_interval() {
    let mut host = SliderHost::new();
    let m = RangeModel::new(50.0, 100.0, 0.0).with_step(Some(5.0));
    host.press(&m, 0.0);
    assert_eq!(
        host.current(),
        Some(0.0),
        "the low end of the normalised range"
    );
    host.drag(&m, 1.0);
    assert_eq!(commit_value(&host.release(&m)), 100.0);
}

#[test]
fn the_transcript_describes_the_drag_and_does_not_enforce_it() {
    // The counters are a record. Walk a full drag and check the record
    // agrees with the emissions that were actually produced.
    let mut host = SliderHost::new();
    let m = model();
    let outs = scrub(&mut host, &m, &[0.1, 0.3, 0.5, 0.7, 0.9]);
    let produced_live = outs
        .iter()
        .flat_map(|o| &o.emissions)
        .filter(|e| matches!(e, SliderEmission::Changed(_)))
        .count();
    assert_eq!(
        host.live_ticks(),
        produced_live,
        "the counter matches what was emitted"
    );
    assert_eq!(host.last_tier(), "commit");
    assert!(host.settled());
    assert!(!host.cancelled());
}

#[test]
fn the_same_event_sequence_produces_the_same_transcript_twice() {
    // No clock, no hidden state: replaying the input gives the same output.
    let script = |host: &mut SliderHost, m: &RangeModel| {
        [
            host.press(m, 0.2).emissions[0].tier(),
            host.drag(m, 0.4).emissions[0].tier(),
            host.drag(m, 0.6).emissions[0].tier(),
            host.release(m).emissions[0].tier(),
        ]
    };
    let m = model();
    let mut a = SliderHost::new();
    let mut b = SliderHost::new();
    assert_eq!(script(&mut a, &m), script(&mut b, &m));
    assert_eq!(a.live_ticks(), b.live_ticks());
    assert_eq!(a.last_tier(), b.last_tier());
}

#[test]
fn emission_tier_names_are_the_three_spec_tiers() {
    let mut host = SliderHost::new();
    let m = model();
    let live = host.press(&m, 0.5);
    assert_eq!(live.emissions[0].tier(), "live");
    let commit = host.release(&m);
    assert_eq!(commit.emissions[0].tier(), "commit");

    let mut host = SliderHost::new();
    host.press(&m, 0.5);
    let cancel = host.cancel();
    assert_eq!(cancel.emissions[0].tier(), "cancel");
}

#[test]
fn an_outcome_reports_whether_anything_happened() {
    assert!(SliderOutcome::noop().is_noop());
    assert!(SliderOutcome::emissions(Vec::new()).is_noop());
    let mut host = SliderHost::new();
    assert!(!host.press(&model(), 0.5).is_noop());
}

#[test]
fn a_press_while_a_drag_is_open_keeps_the_outer_snapshot_governing() {
    // The slot's outermost-wins rule: a second press cannot re-snapshot
    // over the first, so Escape still restores the original value.
    let mut host = SliderHost::new();
    let m = model();
    host.press(&m, 0.2);
    host.press(&m, 0.9);
    assert_eq!(
        host.snapshot(),
        Some(50.0),
        "the outer snapshot still governs"
    );
    assert_eq!(
        host.current(),
        Some(90.0),
        "while the position still follows the pointer"
    );
    assert_eq!(
        cancel_value(&host.cancel()),
        50.0,
        "and Escape reverts to the original"
    );
}
