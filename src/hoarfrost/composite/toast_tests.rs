//! Tests for [`super::toast`] — the clock-free countdown, the pause that
//! preserves rather than resets, the severity gate that cannot be armed
//! for `Warning`/`Danger`, the max-visible policy that never destroys an
//! undismissed notification, the conditional `Escape`, and the geometry
//! a dismissal does not move (`UIF-COMP-002`, `UIF-ZORD-*`).

use crate::composite::info_bar::{action_focus_id, dismiss_focus_id};
use crate::composite::notification::{DEFAULT_AUTO_DISMISS_MS, NotificationId, Severity};
use crate::composite::toast::{
    ANCHOR_GUTTER, MAX_VISIBLE, PushOutcome, STACK_GAP, TimerState, ToastEntry, ToastStack,
    ToastTimer, WIDTH, WIDTH_CLAMP, anchor_bounds, stack_bounds, toast_width,
};
use crate::focus::{FocusId, FocusState, GroupId};
use crate::surface::zorder::{
    DeclineReason, PointerRoute, SurfaceId, SurfaceKind, SurfaceRegistry, UnwindAnswer, ZBand,
};
use crate::token::space;
use iced::{Point, Rectangle};

const FIVE_S: u64 = 5_000;

fn model(
    id: &str,
    severity: Severity,
    auto_ms: Option<u32>,
    action: Option<&'static str>,
    dismissible: bool,
) -> crate::composite::notification::NotificationModel<&'static str> {
    let mut m = crate::composite::notification::NotificationModel::new(
        id,
        severity,
        format!("{severity:?} title"),
        "body text",
    )
    .with_dismissible(dismissible);
    if let Some(a) = action {
        m = m.with_action(a, a);
    }
    if let Some(ms) = auto_ms {
        m = m.with_auto_dismiss(ms);
    }
    m
}

fn viewport(w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::ORIGIN, iced::Size::new(w, h))
}

// ---------------------------------------------------------------------------
// 1 — the constants
// ---------------------------------------------------------------------------

#[test]
fn the_stack_constants_are_section_3b_2s_figures() {
    assert_eq!(WIDTH, 360.0);
    assert_eq!(ANCHOR_GUTTER, 16.0);
    assert_eq!(ANCHOR_GUTTER, space::LG);
    assert_eq!(STACK_GAP, 8.0);
    assert_eq!(STACK_GAP, space::SM);
    assert_eq!(MAX_VISIBLE, 3);
}

#[test]
fn the_width_clamp_is_two_anchor_gutters() {
    assert_eq!(WIDTH_CLAMP, 32.0);
    assert_eq!(WIDTH_CLAMP, ANCHOR_GUTTER * 2.0);
}

// ---------------------------------------------------------------------------
// 2 — width resolution
// ---------------------------------------------------------------------------

#[test]
fn a_wide_viewport_gets_the_full_toast_width() {
    assert_eq!(toast_width(1_920.0), WIDTH);
}

#[test]
fn a_narrow_viewport_clamps_the_width_to_itself_minus_the_clamp() {
    assert_eq!(toast_width(300.0), 268.0);
    assert_eq!(toast_width(300.0), 300.0 - WIDTH_CLAMP);
}

#[test]
fn a_viewport_exactly_at_the_clamp_gives_zero_width_rather_than_negative() {
    let w = toast_width(WIDTH_CLAMP);
    assert_eq!(w, 0.0);
    assert!(w >= 0.0);
}

#[test]
fn a_viewport_narrower_than_the_clamp_still_never_goes_negative() {
    assert_eq!(toast_width(10.0), 0.0);
    assert_eq!(toast_width(0.0), 0.0);
}

#[test]
fn the_width_never_exceeds_the_declared_maximum() {
    for vw in [0.0, 100.0, 360.0, 800.0, 4_000.0] {
        assert!(toast_width(vw) <= WIDTH);
    }
}

// ---------------------------------------------------------------------------
// 3 — the anchor and the stack layout
// ---------------------------------------------------------------------------

#[test]
fn the_anchor_is_inset_from_the_right_and_above_the_barrier() {
    let vp = viewport(1_000.0, 800.0);
    let bar_top = 760.0;
    let rect = anchor_bounds(vp, bar_top, WIDTH, 80.0);
    assert_eq!(rect.x, 1_000.0 - ANCHOR_GUTTER - WIDTH);
    assert_eq!(
        rect.y + rect.height,
        bar_top - BARRIER_CLEARANCE,
        "16px above the bar"
    );
    assert_eq!(rect.y + rect.height + ANCHOR_GUTTER, bar_top);
}

const BARRIER_CLEARANCE: f32 = ANCHOR_GUTTER;

#[test]
fn the_anchor_never_runs_off_the_left_of_the_viewport() {
    let vp = viewport(100.0, 400.0);
    let rect = anchor_bounds(vp, 380.0, WIDTH, 80.0);
    assert!(rect.x >= vp.x);
}

#[test]
fn the_stack_grows_upward_from_the_anchor() {
    let vp = viewport(1_000.0, 800.0);
    let bar_top = 760.0;
    let rects = stack_bounds(vp, bar_top, 80.0, 3);
    assert_eq!(rects.len(), 3);
    // Last in is at the anchor; each earlier one sits above it.
    let anchor = anchor_bounds(vp, bar_top, toast_width(vp.width), 80.0);
    assert_eq!(rects[2].y, anchor.y);
    assert!(rects[1].y < rects[2].y);
    assert!(rects[0].y < rects[1].y);
}

#[test]
fn the_gap_between_stacked_toasts_is_exactly_space_sm() {
    let vp = viewport(1_000.0, 800.0);
    let rects = stack_bounds(vp, 760.0, 80.0, 3);
    let gap = rects[2].y - (rects[1].y + rects[1].height);
    assert_eq!(gap, STACK_GAP);
    let gap2 = rects[1].y - (rects[0].y + rects[0].height);
    assert_eq!(gap2, STACK_GAP);
}

#[test]
fn the_stack_never_overlaps_itself() {
    let vp = viewport(1_000.0, 800.0);
    let rects = stack_bounds(vp, 760.0, 80.0, MAX_VISIBLE);
    for pair in rects.windows(2) {
        let lower = &pair[1];
        let upper = &pair[0];
        assert!(upper.y + upper.height <= lower.y + 0.001);
    }
}

#[test]
fn an_empty_stack_lays_out_no_rectangles() {
    let rects = stack_bounds(viewport(1_000.0, 800.0), 760.0, 80.0, 0);
    assert!(rects.is_empty());
}

#[test]
fn the_stack_is_right_aligned_at_every_count() {
    let vp = viewport(1_000.0, 800.0);
    for count in 0..=MAX_VISIBLE {
        for rect in stack_bounds(vp, 760.0, 80.0, count) {
            assert_eq!(rect.x + rect.width, vp.x + vp.width - ANCHOR_GUTTER);
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — the timer: no clock, pure tick
// ---------------------------------------------------------------------------

#[test]
fn a_never_timer_cannot_expire_at_any_instant() {
    let timer = ToastTimer::never();
    assert!(timer.is_never());
    for ms in [0, 1, FIVE_S, 1_000_000, u64::MAX / 2] {
        assert_eq!(
            timer.tick(crate::surface::tooltip::TimeMs::millis(ms)),
            TimerState::Never
        );
        assert_eq!(
            timer.remaining_at(crate::surface::tooltip::TimeMs::millis(ms)),
            None
        );
    }
}

#[test]
fn a_never_timer_refuses_to_be_paused() {
    let mut timer = ToastTimer::never();
    assert!(
        !timer.pause(crate::surface::tooltip::TimeMs::millis(1_000)),
        "a timer with no budget has nothing to hold"
    );
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(1_000)),
        TimerState::Never
    );
}

#[test]
fn an_armed_timer_counts_down() {
    let t0 = crate::surface::tooltip::TimeMs::millis(1_000);
    let timer = ToastTimer::armed(FIVE_S, t0);
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(2_000)),
        TimerState::Running {
            remaining_ms: 4_000
        }
    );
}

#[test]
fn a_timer_expires_exactly_at_its_budget() {
    let t0 = crate::surface::tooltip::TimeMs::millis(0);
    let mut timer = ToastTimer::armed(FIVE_S, t0);
    assert!(
        !timer.advance(crate::surface::tooltip::TimeMs::millis(FIVE_S - 1)),
        "one millisecond short is not expired"
    );
    assert!(
        timer.advance(crate::surface::tooltip::TimeMs::millis(FIVE_S)),
        "exactly at budget it fires"
    );
    assert!(timer.has_expired());
}

#[test]
fn tick_is_pure_and_repeatable() {
    let t0 = crate::surface::tooltip::TimeMs::millis(0);
    let timer = ToastTimer::armed(FIVE_S, t0);
    let probe = crate::surface::tooltip::TimeMs::millis(3_000);
    let first = timer.tick(probe);
    for _ in 0..10 {
        assert_eq!(timer.tick(probe), first, "tick must not mutate the timer");
    }
    assert_eq!(
        first,
        TimerState::Running {
            remaining_ms: 2_000
        }
    );
}

#[test]
fn advance_is_idempotent_once_spent() {
    let mut timer = ToastTimer::armed(1_000, crate::surface::tooltip::TimeMs::millis(0));
    assert!(timer.advance(crate::surface::tooltip::TimeMs::millis(1_000)));
    assert!(!timer.advance(crate::surface::tooltip::TimeMs::millis(2_000)));
    assert!(!timer.advance(crate::surface::tooltip::TimeMs::millis(3_000)));
}

#[test]
fn a_zero_budget_timer_is_expired_the_instant_it_is_armed() {
    let timer = ToastTimer::armed(0, crate::surface::tooltip::TimeMs::millis(500));
    assert!(timer.has_expired());
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(500)),
        TimerState::Expired
    );
}

#[test]
fn a_clock_that_went_backwards_ages_the_timer_by_nothing() {
    let mut timer = ToastTimer::armed(FIVE_S, crate::surface::tooltip::TimeMs::millis(10_000));
    // A host that hands over readings out of order gets "no time has
    // passed", not a panic and not a wrap-around.
    assert!(
        !timer.advance(crate::surface::tooltip::TimeMs::millis(5_000)),
        "an earlier reading must not expire the timer"
    );
    assert_eq!(
        timer.remaining_at(crate::surface::tooltip::TimeMs::millis(5_000)),
        Some(FIVE_S)
    );
}

// ---------------------------------------------------------------------------
// 5 — pause preserves; it does not reset
// ---------------------------------------------------------------------------

#[test]
fn a_pause_banks_the_remainder_and_a_resume_continues_from_it() {
    let t0 = crate::surface::tooltip::TimeMs::millis(0);
    let mut timer = ToastTimer::armed(FIVE_S, t0);

    // Hover at 2s of 5s.
    assert!(timer.pause(crate::surface::tooltip::TimeMs::millis(2_000)));
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(2_000)),
        TimerState::Paused {
            remaining_ms: 3_000
        }
    );

    // Held for eight seconds of wall time...
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(10_000)),
        TimerState::Paused {
            remaining_ms: 3_000
        },
        "a held timer does not tick down at all"
    );

    // ...and resuming leaves exactly the three seconds that were left.
    assert!(timer.resume(crate::surface::tooltip::TimeMs::millis(10_000)));
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(12_500)),
        TimerState::Running { remaining_ms: 500 }
    );
    assert!(timer.advance(crate::surface::tooltip::TimeMs::millis(13_000)));
}

#[test]
fn pause_then_resume_does_not_extend_the_total_live_time() {
    // The whole point: 2s live + 8s held + 3s live = 5s of *live* time,
    // which is the budget. A reset-on-resume implementation would give
    // 2s + 5s = 7s and never expire on this timeline.
    let mut timer = ToastTimer::armed(FIVE_S, crate::surface::tooltip::TimeMs::millis(0));
    timer.pause(crate::surface::tooltip::TimeMs::millis(2_000));
    timer.resume(crate::surface::tooltip::TimeMs::millis(10_000));
    assert!(!timer.advance(crate::surface::tooltip::TimeMs::millis(12_999)));
    assert!(timer.advance(crate::surface::tooltip::TimeMs::millis(13_000)));
}

#[test]
fn repeated_hover_waves_cannot_keep_a_toast_alive_forever() {
    // Waving the pointer across the stack pauses and resumes without
    // granting extra budget: the total live time stays at five seconds.
    let mut timer = ToastTimer::armed(FIVE_S, crate::surface::tooltip::TimeMs::millis(0));
    let mut t = 0_u64;
    for _ in 0..50 {
        timer.pause(crate::surface::tooltip::TimeMs::millis(t));
        t += 100;
        timer.resume(crate::surface::tooltip::TimeMs::millis(t));
        t += 100;
    }
    // 50 cycles × 100ms live = 5_000ms of live time, so it is spent.
    assert!(timer.advance(crate::surface::tooltip::TimeMs::millis(t)));
}

#[test]
fn pausing_an_already_paused_timer_changes_nothing() {
    let mut timer = ToastTimer::armed(FIVE_S, crate::surface::tooltip::TimeMs::millis(0));
    assert!(timer.pause(crate::surface::tooltip::TimeMs::millis(1_000)));
    assert!(
        !timer.pause(crate::surface::tooltip::TimeMs::millis(2_000)),
        "a second pause on a held timer is not a new hold"
    );
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(2_000)),
        TimerState::Paused {
            remaining_ms: 4_000
        },
        "and the banked remainder is still the first one"
    );
}

#[test]
fn resuming_a_running_timer_changes_nothing() {
    let mut timer = ToastTimer::armed(FIVE_S, crate::surface::tooltip::TimeMs::millis(0));
    assert!(
        !timer.resume(crate::surface::tooltip::TimeMs::millis(1_000)),
        "already running"
    );
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(1_000)),
        TimerState::Running {
            remaining_ms: 4_000
        }
    );
}

#[test]
fn resuming_a_spent_timer_does_not_un_spend_it() {
    let mut timer = ToastTimer::armed(1_000, crate::surface::tooltip::TimeMs::millis(0));
    assert!(timer.advance(crate::surface::tooltip::TimeMs::millis(1_000)));
    assert!(
        !timer.resume(crate::surface::tooltip::TimeMs::millis(2_000)),
        "a spent timer cannot be restarted"
    );
    assert_eq!(
        timer.tick(crate::surface::tooltip::TimeMs::millis(2_000)),
        TimerState::Expired
    );
}

#[test]
fn the_severity_gate_hands_out_only_never_timers_for_the_two_that_must_stay() {
    for severity in [Severity::Warning, Severity::Danger] {
        for requested in [None, Some(1), Some(DEFAULT_AUTO_DISMISS_MS), Some(u32::MAX)] {
            let timer = ToastTimer::for_severity(
                severity,
                requested,
                crate::surface::tooltip::TimeMs::millis(0),
            );
            assert!(
                timer.is_never(),
                "{severity:?} with a request of {requested:?} must get a never-timer"
            );
            assert_eq!(
                timer.tick(crate::surface::tooltip::TimeMs::millis(u64::MAX / 4)),
                TimerState::Never
            );
        }
    }
}

#[test]
fn the_severity_gate_arms_the_two_that_may_time_out() {
    for severity in [Severity::Info, Severity::Success] {
        let timer = ToastTimer::for_severity(
            severity,
            Some(FIVE_S as u32),
            crate::surface::tooltip::TimeMs::millis(0),
        );
        assert!(!timer.is_never());
        assert_eq!(
            timer.tick(crate::surface::tooltip::TimeMs::millis(1_000)),
            TimerState::Running {
                remaining_ms: 4_000
            }
        );
    }
}

#[test]
fn the_severity_gate_passes_a_missing_request_through_as_a_never_timer() {
    for severity in [Severity::Info, Severity::Success] {
        let timer =
            ToastTimer::for_severity(severity, None, crate::surface::tooltip::TimeMs::millis(0));
        assert!(timer.is_never());
    }
}

// ---------------------------------------------------------------------------
// 6 — the stack: posting and ordering
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_stack_is_empty() {
    let stack: ToastStack<&str> = ToastStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.visible().len(), 0);
    assert_eq!(stack.pending_count(), 0);
}

#[test]
fn the_newest_toast_is_last_in_the_visible_list() {
    let mut stack = ToastStack::new();
    stack.push(model("a", Severity::Info, None, None, true), t(0));
    stack.push(model("b", Severity::Info, None, None, true), t(1));
    let ids: Vec<&str> = stack.visible().iter().map(|e| e.id().as_str()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn up_to_three_toasts_are_all_visible() {
    let mut stack = ToastStack::new();
    for (i, name) in ["a", "b", "c"].iter().enumerate() {
        assert!(matches!(
            stack.push(model(name, Severity::Info, None, None, true), t(i as u64)),
            PushOutcome::Shown { evicted: None }
        ));
    }
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(stack.pending_count(), 0);
}

#[test]
fn a_fourth_toast_evicts_the_oldest_dismissible_one() {
    let mut stack = ToastStack::new();
    for name in ["a", "b", "c"] {
        stack.push(model(name, Severity::Info, None, None, true), t(0));
    }
    let outcome = stack.push(model("d", Severity::Info, None, None, true), t(1));
    match outcome {
        PushOutcome::Shown { evicted } => {
            let evicted = evicted.expect("one was pushed off");
            assert_eq!(evicted.id().as_str(), "a");
        }
        PushOutcome::Queued => panic!("a dismissible toast should never be queued"),
    }
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(stack.pending_count(), 0);
}

#[test]
fn the_evicted_toast_is_the_oldest_not_the_newest() {
    let mut stack = ToastStack::new();
    for name in ["a", "b", "c"] {
        stack.push(model(name, Severity::Info, None, None, true), t(0));
    }
    stack.push(model("d", Severity::Info, None, None, true), t(1));
    let ids: Vec<&str> = stack.visible().iter().map(|e| e.id().as_str()).collect();
    assert_eq!(ids, vec!["b", "c", "d"]);
}

#[test]
fn a_persistent_toast_is_never_evicted_to_make_room() {
    // Three Warnings fill the stack; a new Info cannot displace any of
    // them, because pushing a `Warning` off without the user dismissing
    // it is an auto-dismiss, which §3B.2 forbids.
    let mut stack = ToastStack::new();
    for name in ["w1", "w2", "w3"] {
        stack.push(model(name, Severity::Warning, None, None, true), t(0));
    }
    assert!(matches!(
        stack.push(
            model("new", Severity::Info, Some(FIVE_S as u32), None, true),
            t(1)
        ),
        PushOutcome::Queued
    ));
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(stack.pending_count(), 1);
    let ids: Vec<&str> = stack.visible().iter().map(|e| e.id().as_str()).collect();
    assert_eq!(ids, vec!["w1", "w2", "w3"]);
}

#[test]
fn a_danger_toast_is_never_evicted_either() {
    let mut stack = ToastStack::new();
    for name in ["d1", "d2", "d3"] {
        stack.push(model(name, Severity::Danger, None, None, true), t(0));
    }
    assert!(matches!(
        stack.push(model("new", Severity::Info, None, None, true), t(1)),
        PushOutcome::Queued
    ));
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
}

#[test]
fn one_dismissible_among_three_persistent_is_the_one_that_goes() {
    let mut stack = ToastStack::new();
    stack.push(model("w", Severity::Warning, None, None, true), t(0));
    stack.push(model("i", Severity::Info, None, None, true), t(0));
    stack.push(model("d", Severity::Danger, None, None, true), t(0));
    let outcome = stack.push(model("new", Severity::Info, None, None, true), t(1));
    match outcome {
        PushOutcome::Shown { evicted } => {
            assert_eq!(evicted.expect("the Info was pushed off").id().as_str(), "i")
        }
        PushOutcome::Queued => panic!("a dismissible toast was on the stack"),
    }
}

#[test]
fn dismissing_a_visible_toast_admits_the_one_that_was_waiting() {
    let mut stack = ToastStack::new();
    for name in ["w1", "w2", "w3"] {
        stack.push(model(name, Severity::Warning, None, None, true), t(0));
    }
    stack.push(model("queued", Severity::Info, None, None, true), t(1));
    assert_eq!(stack.pending_count(), 1);

    stack.remove(&NotificationId::new("w1"), t(2));
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(stack.pending_count(), 0);
    assert!(stack.visible().iter().any(|e| e.id().as_str() == "queued"));
}

#[test]
fn a_queued_toast_countdown_starts_when_it_becomes_visible_not_when_it_was_posted() {
    let mut stack = ToastStack::new();
    for name in ["w1", "w2", "w3"] {
        stack.push(model(name, Severity::Warning, None, None, true), t(0));
    }
    // Posted at t=0 with a 5s window.
    stack.push(
        model("q", Severity::Info, Some(FIVE_S as u32), None, true),
        t(0),
    );

    // Admitted only at t=4s.
    stack.remove(&NotificationId::new("w1"), t(4_000));
    let queued = stack
        .visible()
        .iter()
        .find(|e| e.id().as_str() == "q")
        .expect("admitted");

    // At t=8s only 4s of its life has been spent, so it is still here.
    assert_eq!(
        queued.remaining_ms(t(8_000)),
        Some(1_000),
        "a toast the user never saw should not spend its life being unseen"
    );
    assert!(
        stack
            .advance(t(9_000))
            .iter()
            .any(|e| e.id().as_str() == "q")
    );
}

// ---------------------------------------------------------------------------
// 7 — the stack: auto-dismiss
// ---------------------------------------------------------------------------

fn t(ms: u64) -> crate::surface::tooltip::TimeMs {
    crate::surface::tooltip::TimeMs::millis(ms)
}

#[test]
fn an_info_toast_times_out_on_schedule() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), None, true),
        t(0),
    );
    assert!(stack.advance(t(4_999)).is_empty());
    let expired = stack.advance(t(5_000));
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].id().as_str(), "i");
}

#[test]
fn a_warning_never_times_out_however_long_the_host_waits() {
    let mut stack = ToastStack::new();
    stack.push(
        model("w", Severity::Warning, Some(FIVE_S as u32), None, true),
        t(0),
    );
    for ms in [1_000, 5_000, 60_000, 3_600_000] {
        assert!(
            stack.advance(t(ms)).is_empty(),
            "a Warning expired at {ms}ms; §3B.2 says it MUST NEVER auto-dismiss"
        );
    }
    assert_eq!(stack.visible().len(), 1);
}

#[test]
fn a_danger_never_times_out_however_long_the_host_waits() {
    let mut stack = ToastStack::new();
    stack.push(model("d", Severity::Danger, Some(1), None, true), t(0));
    assert!(stack.advance(t(10_000_000)).is_empty());
    assert_eq!(stack.visible().len(), 1);
}

#[test]
fn a_toast_posted_without_a_timer_never_times_out() {
    let mut stack = ToastStack::new();
    stack.push(model("i", Severity::Info, None, None, true), t(0));
    assert!(stack.advance(t(1_000_000)).is_empty());
}

#[test]
fn advancing_an_empty_stack_is_a_no_op_not_a_panic() {
    let mut stack: ToastStack<&str> = ToastStack::new();
    assert!(stack.advance(t(0)).is_empty());
    assert!(stack.advance(t(999_999)).is_empty());
}

#[test]
fn several_toasts_can_expire_in_one_frame() {
    let mut stack = ToastStack::new();
    stack.push(model("a", Severity::Info, Some(1_000), None, true), t(0));
    stack.push(model("b", Severity::Info, Some(1_000), None, true), t(0));
    stack.push(model("c", Severity::Warning, None, None, true), t(0));
    let expired = stack.advance(t(1_000));
    assert_eq!(expired.len(), 2);
    assert_eq!(stack.visible().len(), 1);
}

// ---------------------------------------------------------------------------
// 8 — hover and focus pause the countdown
// ---------------------------------------------------------------------------

#[test]
fn hovering_a_toast_stops_its_countdown() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), None, true),
        t(0),
    );
    let id = NotificationId::new("i");
    assert!(stack.set_hovered(&id, true, t(2_000)));
    assert_eq!(
        stack.visible()[0].timer_state(t(4_000)),
        TimerState::Paused {
            remaining_ms: 3_000
        }
    );
    assert!(
        stack.advance(t(10_000)).is_empty(),
        "a hovered toast cannot expire"
    );
}

#[test]
fn unhovering_continues_from_the_banked_remainder() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), None, true),
        t(0),
    );
    let id = NotificationId::new("i");
    stack.set_hovered(&id, true, t(2_000));
    stack.set_hovered(&id, false, t(10_000));
    // 3s left, so it survives to 12_999 and dies at 13_000.
    assert!(stack.advance(t(12_999)).is_empty());
    assert_eq!(stack.advance(t(13_000)).len(), 1);
}

#[test]
fn focusing_a_toast_also_stops_its_countdown() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), Some("act"), true),
        t(0),
    );
    let id = NotificationId::new("i");
    assert!(stack.set_focused(&id, true, t(1_000)));
    assert_eq!(
        stack.visible()[0].timer_state(t(9_000)),
        TimerState::Paused {
            remaining_ms: 4_000
        }
    );
}

#[test]
fn focus_leaving_while_still_hovered_keeps_the_toast_held() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), Some("act"), true),
        t(0),
    );
    let id = NotificationId::new("i");
    stack.set_hovered(&id, true, t(1_000));
    stack.set_focused(&id, true, t(1_500));
    // Focus leaves but the pointer is still on it.
    stack.set_focused(&id, false, t(2_000));
    assert!(stack.visible()[0].is_hovered());
    assert!(stack.visible()[0].is_held());
    assert_eq!(
        stack.visible()[0].timer_state(t(9_000)),
        TimerState::Paused {
            remaining_ms: 4_000
        },
        "the hover must still hold it; resuming because focus left would ignore the pointer"
    );
}

#[test]
fn hover_leaving_while_still_focused_keeps_the_toast_held() {
    let mut stack = ToastStack::new();
    stack.push(
        model("i", Severity::Info, Some(FIVE_S as u32), Some("act"), true),
        t(0),
    );
    let id = NotificationId::new("i");
    stack.set_focused(&id, true, t(1_000));
    stack.set_hovered(&id, true, t(1_500));
    stack.set_hovered(&id, false, t(2_000));
    assert!(stack.visible()[0].is_focused());
    assert!(stack.visible()[0].is_held());
    assert!(stack.advance(t(9_000)).is_empty());
}

#[test]
fn hovering_a_persistent_toast_reports_held_but_there_was_nothing_to_pause() {
    let mut stack = ToastStack::new();
    stack.push(model("d", Severity::Danger, None, Some("act"), true), t(0));
    let id = NotificationId::new("d");
    assert!(stack.set_hovered(&id, true, t(0)));
    assert!(stack.visible()[0].is_held());
    assert_eq!(stack.visible()[0].timer_state(t(9_000)), TimerState::Never);
}

#[test]
fn hovering_a_toast_that_is_not_on_the_stack_is_reported_not_remembered() {
    let mut stack: ToastStack<&str> = ToastStack::new();
    assert!(
        !stack.set_hovered(&NotificationId::new("ghost"), true, t(0)),
        "the stack has no such toast"
    );
    assert!(stack.is_empty());
}

#[test]
fn dismissing_mid_hover_strands_neither_the_pointer_nor_a_timer() {
    let mut stack = ToastStack::new();
    stack.push(
        model("a", Severity::Info, Some(FIVE_S as u32), Some("act"), true),
        t(0),
    );
    stack.push(
        model("b", Severity::Info, Some(FIVE_S as u32), Some("act"), true),
        t(0),
    );
    let id = NotificationId::new("a");
    stack.set_hovered(&id, true, t(1_000));
    assert!(stack.visible()[0].is_held());

    // The toast goes away while the pointer is still on it.
    let removed = stack.remove(&id, t(2_000)).expect("removed");
    assert!(
        removed.is_hovered(),
        "the removed entry carries the hover out"
    );
    assert!(
        !stack.visible().iter().any(|e| e.id() == &id),
        "and the stack no longer holds it"
    );
    // A late hover-exit for the gone toast is a no-op, not a panic and
    // not a resurrection.
    assert!(!stack.set_hovered(&id, false, t(3_000)));
    // The surviving toast is untouched and still counts down.
    assert_eq!(stack.visible().len(), 1);
    assert_eq!(
        stack.visible()[0].timer_state(t(4_000)),
        TimerState::Running {
            remaining_ms: 1_000
        }
    );
    assert_eq!(stack.advance(t(5_000)).len(), 1);
}

#[test]
fn a_toast_dismissed_mid_hover_cannot_be_dismissed_twice() {
    let mut stack = ToastStack::new();
    stack.push(
        model("a", Severity::Info, Some(1_000), Some("act"), true),
        t(0),
    );
    let id = NotificationId::new("a");
    stack.set_hovered(&id, true, t(500));
    assert!(stack.remove(&id, t(600)).is_some());
    assert!(stack.remove(&id, t(700)).is_none());
    assert!(stack.advance(t(1_000)).is_empty());
}

// ---------------------------------------------------------------------------
// 9 — focus membership and the conditional Escape
// ---------------------------------------------------------------------------

#[test]
fn a_toast_contributes_its_action_then_its_dismiss_as_tab_stops() {
    let m = model("t", Severity::Info, None, Some("Open"), true);
    let members = ToastStack::focus_members(&m);
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].id, action_focus_id(&m.id));
    assert_eq!(members[1].id, dismiss_focus_id(&m.id));
}

#[test]
fn a_toast_with_no_controls_contributes_no_tab_stops() {
    let m = model("t", Severity::Info, None, None, false);
    assert!(ToastStack::focus_members(&m).is_empty());
}

#[test]
fn the_toast_body_is_not_a_tab_stop() {
    let m = model("toast-7", Severity::Info, None, Some("Open"), true);
    let members = ToastStack::focus_members(&m);
    for member in &members {
        assert_ne!(member.id.as_str(), "toast-7");
    }
}

#[test]
fn an_empty_stack_holds_no_focus() {
    let stack: ToastStack<&str> = ToastStack::new();
    let focus = FocusState::new();
    assert!(!stack.holds_focus(&focus));
}

#[test]
fn a_showing_toast_does_not_hold_focus_by_itself() {
    // The condition is focus, not visibility. A toast that merely
    // appeared has not taken the keyboard.
    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Info, None, Some("Open"), true), t(0));
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("workspace"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.focus(&FocusId::new("pane"));
    assert!(!stack.holds_focus(&focus));
}

#[test]
fn focus_inside_a_toast_is_focus_inside_the_stack() {
    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Info, None, Some("Open"), true), t(0));
    let m = stack.visible()[0].model().clone();
    let mut focus = FocusState::new();
    focus.register_group(
        ToastStack::<&str>::group_for(&m.id),
        ToastStack::focus_members(&m),
    );
    focus.focus(&action_focus_id(&m.id));
    assert!(stack.holds_focus(&focus));
}

#[test]
fn escape_dismisses_the_focused_toast_and_only_that_one() {
    let mut stack = ToastStack::new();
    stack.push(model("a", Severity::Info, None, Some("Open"), true), t(0));
    stack.push(model("b", Severity::Info, None, Some("Open"), true), t(0));
    let b = stack.visible()[1].model().clone();
    let mut focus = FocusState::new();
    focus.register_group(
        ToastStack::<&str>::group_for(&b.id),
        ToastStack::focus_members(&b),
    );
    focus.focus(&dismiss_focus_id(&b.id));

    assert_eq!(stack.unwind_answer(&focus, t(1)), UnwindAnswer::Dismissed);
    assert_eq!(stack.visible().len(), 1);
    assert_eq!(stack.visible()[0].id().as_str(), "a");
}

#[test]
fn escape_is_declined_when_focus_is_outside_the_stack() {
    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Info, None, Some("Open"), true), t(0));
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("ws"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.focus(&FocusId::new("pane"));
    assert_eq!(
        stack.unwind_answer(&focus, t(1)),
        UnwindAnswer::Declined(DeclineReason::NotFocused),
        "§5: Escape unwinds only when focus is inside the overlay"
    );
    assert_eq!(stack.visible().len(), 1, "and nothing was dismissed");
}

#[test]
fn escape_is_declined_when_nothing_is_focused_at_all() {
    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Info, None, Some("Open"), true), t(0));
    let focus = FocusState::new();
    assert_eq!(
        stack.unwind_answer(&focus, t(1)),
        UnwindAnswer::Declined(DeclineReason::NotFocused)
    );
}

#[test]
fn escape_is_declined_when_the_stack_is_empty() {
    let mut stack: ToastStack<&str> = ToastStack::new();
    let focus = FocusState::new();
    assert_eq!(
        stack.unwind_answer(&focus, t(1)),
        UnwindAnswer::Declined(DeclineReason::NotOpen)
    );
}

#[test]
fn a_toast_with_no_focusable_controls_can_never_be_escape_dismissed() {
    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Danger, None, None, false), t(0));
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("ws"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.focus(&FocusId::new("pane"));
    assert_eq!(
        stack.unwind_answer(&focus, t(1)),
        UnwindAnswer::Declined(DeclineReason::NotFocused)
    );
}

#[test]
fn the_stack_sits_in_band_1_and_keeps_escape_rank_1() {
    // G-13: escape rank is not band rank. The toast stack borrows Band 1
    // unchanged, so the ordering Menu(3) -> Modal(2) -> Toast(1) ->
    // Workspace(0) still holds with it present.
    assert_eq!(SurfaceKind::ToastStack.band(), ZBand::UtilityOverlay);
    assert_eq!(ZBand::UtilityOverlay.escape_rank(), 1);
    assert_eq!(ZBand::Menu.escape_rank(), 3);
    assert_eq!(ZBand::Modal.escape_rank(), 2);
    assert_eq!(ZBand::Workspace.escape_rank(), 0);
}

#[test]
fn the_registry_walks_a_toast_stack_at_band_1_rank_behind_menus_and_modals() {
    let mut reg = SurfaceRegistry::new();
    reg.register(
        SurfaceId::new("ws"),
        SurfaceKind::Workspace,
        vec![viewport(1_000.0, 800.0)],
    );
    reg.register(SurfaceId::new("toasts"), SurfaceKind::ToastStack, vec![]);
    reg.register(SurfaceId::new("modal"), SurfaceKind::Modal, vec![]);
    reg.register(SurfaceId::new("menu"), SurfaceKind::MenuCard, vec![]);

    // The walk must reach the menu first, then the modal, then the toast.
    let mut order: Vec<String> = Vec::new();
    let outcome = reg.escape(|surface| {
        order.push(surface.id().as_str().to_string());
        UnwindAnswer::Declined(DeclineReason::NotFocused)
    });
    assert_eq!(order, vec!["menu", "modal", "toasts", "ws"]);
    assert!(matches!(
        outcome,
        crate::surface::zorder::EscapeOutcome::Ignored { .. }
    ));
}

#[test]
fn a_focused_toast_wins_escape_over_the_workspace_and_loses_it_to_a_menu() {
    let mut reg = SurfaceRegistry::new();
    reg.register(SurfaceId::new("ws"), SurfaceKind::Workspace, vec![]);
    reg.register(SurfaceId::new("toasts"), SurfaceKind::ToastStack, vec![]);
    reg.register(SurfaceId::new("menu"), SurfaceKind::MenuCard, vec![]);

    let outcome = reg.escape(|surface| match surface.kind() {
        SurfaceKind::ToastStack => UnwindAnswer::Dismissed,
        _ => UnwindAnswer::Declined(DeclineReason::NotFocused),
    });
    assert_eq!(outcome.consumed_by(), Some(&SurfaceId::new("toasts")));
    assert!(outcome.dismissed());
}

// ---------------------------------------------------------------------------
// 10 — pointer pass-through
// ---------------------------------------------------------------------------

#[test]
fn a_click_outside_the_toast_stack_passes_through_to_the_workspace() {
    let mut reg = SurfaceRegistry::new();
    reg.register(
        SurfaceId::new("ws"),
        SurfaceKind::Workspace,
        vec![viewport(1_000.0, 800.0)],
    );
    let toast_rect = stack_bounds(viewport(1_000.0, 800.0), 760.0, 80.0, 1)[0];
    reg.register(
        SurfaceId::new("toasts"),
        SurfaceKind::ToastStack,
        vec![toast_rect],
    );

    let route: PointerRoute = reg.route_pointer(Point::new(100.0, 100.0));
    assert_eq!(
        route.target(),
        &crate::surface::zorder::PointerTarget::Surface(SurfaceId::new("ws")),
        "a click away from the toast must reach the workspace surface below"
    );
    assert!(route.passed_through().contains(&ZBand::UtilityOverlay));
}

#[test]
fn a_click_inside_the_toast_is_captured_by_the_stack() {
    let mut reg = SurfaceRegistry::new();
    reg.register(
        SurfaceId::new("ws"),
        SurfaceKind::Workspace,
        vec![viewport(1_000.0, 800.0)],
    );
    let toast_rect = stack_bounds(viewport(1_000.0, 800.0), 760.0, 80.0, 1)[0];
    reg.register(
        SurfaceId::new("toasts"),
        SurfaceKind::ToastStack,
        vec![toast_rect],
    );

    let inside = Point::new(toast_rect.x + 10.0, toast_rect.y + 10.0);
    let route = reg.route_pointer(inside);
    assert_eq!(
        route.target(),
        &crate::surface::zorder::PointerTarget::Surface(SurfaceId::new("toasts"))
    );
    assert!(!route.reaches_workspace());
}

#[test]
fn the_toast_stack_never_owns_a_backdrop() {
    // §5's Band 1 row: "None (pass-through)". A toast must not dim or
    // catch anything behind it.
    assert_eq!(
        ZBand::UtilityOverlay.backdrop(),
        crate::surface::zorder::Backdrop::None
    );
    assert!(!ZBand::UtilityOverlay.blocks_lower_bands());
}

// ---------------------------------------------------------------------------
// 11 — never steal focus
// ---------------------------------------------------------------------------

#[test]
fn posting_a_toast_never_touches_the_focus_state() {
    let mut stack = ToastStack::new();
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("ws"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.focus(&FocusId::new("pane"));

    stack.push(
        model("t", Severity::Info, Some(1_000), Some("Open"), true),
        t(0),
    );
    stack.push(model("u", Severity::Danger, None, Some("Fix"), true), t(1));

    assert_eq!(focus.current(), Some(&FocusId::new("pane")));
    assert!(!stack.holds_focus(&focus));
}

#[test]
fn a_toast_may_not_take_focus_while_a_modal_trap_is_open() {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("ws"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.register_group(
        GroupId::new("modal"),
        vec![
            crate::focus::FocusMember::new("ok"),
            crate::focus::FocusMember::new("cancel"),
        ],
    );
    focus.focus_first_in_group(&GroupId::new("modal"));
    focus.push_trap(GroupId::new("modal"));
    assert!(focus.is_trapped());

    assert!(
        !ToastStack::<&str>::may_take_focus(&focus),
        "with a modal trap open a toast must not be given focus"
    );
    assert_eq!(focus.current(), Some(&FocusId::new("ok")));
}

#[test]
fn a_toast_may_take_focus_when_no_trap_is_open() {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("ws"),
        vec![crate::focus::FocusMember::new("pane")],
    );
    focus.focus(&FocusId::new("pane"));
    assert!(ToastStack::<&str>::may_take_focus(&focus));
}

#[test]
fn a_toast_cannot_break_a_modal_focus_trap_by_being_posted() {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("modal"),
        vec![
            crate::focus::FocusMember::new("ok"),
            crate::focus::FocusMember::new("cancel"),
        ],
    );
    focus.focus_first_in_group(&GroupId::new("modal"));
    focus.push_trap(GroupId::new("modal"));

    let mut stack = ToastStack::new();
    stack.push(model("t", Severity::Danger, None, Some("Fix"), true), t(0));
    let m = stack.visible()[0].model().clone();
    focus.register_group(
        ToastStack::<&str>::group_for(&m.id),
        ToastStack::focus_members(&m),
    );

    // Tabbing stays inside the modal; the toast's controls are unreachable.
    let next = focus.focus_next();
    assert_eq!(next, Some(FocusId::new("cancel")));
    assert!(focus.is_trapped());
    assert_eq!(focus.trap_depth(), 1);
}

// ---------------------------------------------------------------------------
// 12 — what a dismissal moves
// ---------------------------------------------------------------------------

#[test]
fn the_anchor_position_is_invariant_across_dismissals() {
    // The anchor is a pure function of the viewport and the barrier, so
    // a host that re-lays-out after every dismissal sees no drift: the
    // next toast that takes the anchor lands exactly where the last one
    // was.
    let vp = viewport(1_280.0, 900.0);
    let bar_top = 860.0;
    let first = stack_bounds(vp, bar_top, 80.0, 1)[0];
    let second = stack_bounds(vp, bar_top, 80.0, 1)[0];
    assert_eq!(first, second);
}

#[test]
fn a_dismissal_moves_nothing_outside_the_stack() {
    // The stack reserves no layout: `stack_bounds` takes no container,
    // no sibling, and no workspace rectangle, so there is no call path
    // from a toast leaving to a box outside the stack changing.
    let vp = viewport(1_280.0, 900.0);
    let before = stack_bounds(vp, 860.0, 80.0, 2);
    let after = stack_bounds(vp, 860.0, 80.0, 1);
    // The remaining toast (index 0 in both) keeps its own size; only the
    // number of rectangles changed.
    assert_eq!(before[1].height, after[0].height);
    assert_eq!(before[1].width, after[0].width);
    assert_eq!(before.len(), 2);
    assert_eq!(after.len(), 1);
}

#[test]
fn removing_the_bottom_toast_drops_the_ones_above_by_one_slot() {
    let vp = viewport(1_280.0, 900.0);
    let three = stack_bounds(vp, 860.0, 80.0, 3);
    let two = stack_bounds(vp, 860.0, 80.0, 2);
    // Index 0 is the topmost toast and the stack is anchored at its
    // bottom edge, so losing one toast pulls the top down by exactly one
    // toast-height plus one gap.
    assert_eq!(three[0].y - two[0].y, -(80.0 + STACK_GAP));
    assert_eq!(two[0].y - three[0].y, 80.0 + STACK_GAP);
}

#[test]
fn the_stack_never_grows_past_max_visible_entries() {
    let mut stack = ToastStack::new();
    for i in 0..20 {
        stack.push(
            model(&format!("t{i}"), Severity::Info, None, Some("Open"), true),
            t(i as u64),
        );
    }
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(stack.pending_count(), 0);
}

#[test]
fn a_long_run_of_persistent_toasts_queues_rather_than_losing_any() {
    let mut stack = ToastStack::new();
    for i in 0..10 {
        stack.push(
            model(&format!("w{i}"), Severity::Warning, None, Some("Ack"), true),
            t(i as u64),
        );
    }
    assert_eq!(stack.visible().len(), MAX_VISIBLE);
    assert_eq!(
        stack.pending_count(),
        7,
        "nothing was destroyed: the surplus waits"
    );
}

// ---------------------------------------------------------------------------
// 13 — entry accessors
// ---------------------------------------------------------------------------

#[test]
fn an_entry_exposes_its_model_identity_severity_and_holds() {
    let mut stack = ToastStack::new();
    stack.push(
        model("x", Severity::Success, Some(2_000), Some("Go"), false),
        t(0),
    );
    let entry: &ToastEntry<&str> = &stack.visible()[0];
    assert_eq!(entry.id().as_str(), "x");
    assert_eq!(entry.severity(), Severity::Success);
    assert!(!entry.is_hovered());
    assert!(!entry.is_focused());
    assert!(!entry.is_held());
    assert_eq!(entry.remaining_ms(t(500)), Some(1_500));
    assert_eq!(entry.model().action.as_ref().unwrap().label, "Go");
}

#[test]
fn a_persistent_entry_reports_no_remaining_time_rather_than_zero() {
    let mut stack = ToastStack::new();
    stack.push(model("d", Severity::Danger, Some(1), None, true), t(0));
    assert_eq!(
        stack.visible()[0].remaining_ms(t(9_000)),
        None,
        "`None` says there is no countdown; `Some(0)` would say it is about to fire"
    );
}

#[test]
fn a_timer_state_reports_whether_it_can_still_fire() {
    assert!(TimerState::Running { remaining_ms: 1 }.is_live());
    assert!(TimerState::Paused { remaining_ms: 1 }.is_live());
    assert!(!TimerState::Expired.is_live());
    assert!(!TimerState::Never.is_live());
}
