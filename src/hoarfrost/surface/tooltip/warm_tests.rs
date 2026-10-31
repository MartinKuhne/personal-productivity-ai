//! Tests for the tooltip warm-handoff session (`UIF-TOK-007`).
//!
//! Every test supplies its own clock. None of them sleeps, and none of them
//! reads `TimeMs::now`.
//!
//! The decision table is pinned three ways: at the boundary itself, exhaustively
//! across every gap that matters, and by replaying a whole host loop twice so
//! determinism is *observed* rather than asserted.

use super::{ArrivalWarmth, WarmSession, cold_wait_ms, handoff_window_ms, warm_wait_ms};
use crate::surface::tooltip::{HOVER_DELAY, HoverController, TimeMs, Visibility};
use crate::token::time;

const WINDOW: u64 = 100;
const T0: u64 = 10_000;

fn at(ms: u64) -> TimeMs {
    TimeMs::millis(ms)
}

/// A session whose last visible tooltip was at [`T0`].
fn warm_at_t0() -> WarmSession {
    let mut session = WarmSession::new();
    session.note_visible(at(T0));
    session
}

/// The per-frame loop a real host runs: arm whatever the pointer is on with the
/// session's verdict, tick everything, then feed any visible tooltip back into
/// the session.
///
/// The verdict is read **before** this frame's visibility is recorded, which is
/// what the ordering has to be: an arrival cannot be handed off by the tooltip
/// its own arrival is only now producing.
fn host_frame(
    session: &mut WarmSession,
    controllers: &mut [HoverController; 3],
    hovered: Option<usize>,
    ms: u64,
) -> Vec<Visibility> {
    let now = at(ms);
    for (index, controller) in controllers.iter_mut().enumerate() {
        if Some(index) == hovered {
            controller.pointer_enter_warm(now, session.warmth_for(now));
        } else {
            controller.pointer_exit();
        }
    }
    let painted: Vec<Visibility> = controllers.iter().map(|c| c.tick(now)).collect();
    if painted.iter().any(|visibility| visibility.is_visible()) {
        session.note_visible(now);
    }
    painted
}

/// Run a whole hover script and return the per-frame paint trace.
fn run_script(script: &[(u64, Option<usize>)]) -> Vec<Vec<Visibility>> {
    let mut session = WarmSession::new();
    let mut controllers = [
        HoverController::new(),
        HoverController::new(),
        HoverController::new(),
    ];
    script
        .iter()
        .map(|(ms, hovered)| host_frame(&mut session, &mut controllers, *hovered, *ms))
        .collect()
}

/// Frames every 16ms (60fps) from `from` to `to` inclusive.
fn frames(from: u64, to: u64) -> Vec<u64> {
    (from..=to).step_by(16).collect()
}

// ---------------------------------------------------------------------------
// The rungs the decision reads
// ---------------------------------------------------------------------------

#[test]
fn the_window_is_read_from_the_warm_rung() {
    assert_eq!(handoff_window_ms(), time::DELAY_TOOLTIP_WARM.ms);
    assert_eq!(handoff_window_ms(), WINDOW);
}

#[test]
fn the_warm_wait_is_the_instant_rung() {
    assert_eq!(warm_wait_ms(), time::INSTANT.ms);
    assert_eq!(warm_wait_ms(), 0);
}

#[test]
fn the_cold_wait_is_the_hover_delay_rung() {
    assert_eq!(cold_wait_ms(), HOVER_DELAY);
    assert_eq!(cold_wait_ms(), time::DELAY_TOOLTIP_OPEN.ms);
}

#[test]
fn each_verdict_carries_its_own_wait() {
    assert_eq!(ArrivalWarmth::Cold.wait_ms(), cold_wait_ms());
    assert_eq!(ArrivalWarmth::Warm.wait_ms(), warm_wait_ms());
    assert_eq!(ArrivalWarmth::Cold.wait_ms(), 400);
    assert_eq!(ArrivalWarmth::Warm.wait_ms(), 0);
}

#[test]
fn the_two_verdicts_are_distinct() {
    assert_ne!(ArrivalWarmth::Cold, ArrivalWarmth::Warm);
    assert!(!ArrivalWarmth::Cold.is_warm());
    assert!(ArrivalWarmth::Warm.is_warm());
}

// ---------------------------------------------------------------------------
// Cold: no memory
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_session_has_no_memory_and_is_cold() {
    let session = WarmSession::new();
    assert!(!session.has_memory());
    assert_eq!(session.last_visible(), None);
    assert_eq!(session.warmth_for(at(T0)), ArrivalWarmth::Cold);
    assert!(!session.is_warm(at(T0)));
}

#[test]
fn default_and_new_agree() {
    assert_eq!(WarmSession::default(), WarmSession::new());
}

#[test]
fn a_session_with_no_memory_is_cold_at_every_instant() {
    let session = WarmSession::new();
    for ms in [0, 1, T0, T0 + WINDOW, T0 + 1_000_000] {
        assert_eq!(
            session.warmth_for(at(ms)),
            ArrivalWarmth::Cold,
            "cold expected at {ms}"
        );
    }
}

#[test]
fn a_pending_hover_grants_no_warmth() {
    // The user hovered, but nothing was ever *shown*, so the next control must
    // not be let off for a wait the user never benefited from.
    let session = WarmSession::new();
    let mut first = HoverController::new();
    first.pointer_enter(at(T0));
    assert_eq!(first.tick(at(T0 + WINDOW - 1)), Visibility::Pending);
    assert_eq!(session.warmth_for(at(T0 + 1)), ArrivalWarmth::Cold);
    assert!(!session.has_memory());
}

// ---------------------------------------------------------------------------
// Warm: inside the window
// ---------------------------------------------------------------------------

#[test]
fn an_arrival_inside_the_window_is_warm() {
    let session = warm_at_t0();
    assert!(session.has_memory());
    for gap in [0, 1, 50, WINDOW - 1, WINDOW] {
        assert_eq!(
            session.warmth_for(at(T0 + gap)),
            ArrivalWarmth::Warm,
            "gap {gap} must be warm"
        );
    }
}

#[test]
fn the_window_boundary_is_inclusive() {
    let session = warm_at_t0();
    assert_eq!(
        session.warmth_for(at(T0 + WINDOW)),
        ArrivalWarmth::Warm,
        "§2.6 says 'within 100ms', so exactly 100 is inside"
    );
}

#[test]
fn one_millisecond_past_the_window_is_cold() {
    let session = warm_at_t0();
    assert_eq!(session.warmth_for(at(T0 + WINDOW + 1)), ArrivalWarmth::Cold);
}

#[test]
fn warmth_lapses_with_no_event_at_all() {
    // Nothing is called between the visible frame and the arrival: the decay is
    // purely a function of the clock the caller supplies, not of a timer the
    // host has to remember to start.
    let session = warm_at_t0();
    assert_eq!(session.warmth_for(at(T0 + 50)), ArrivalWarmth::Warm);
    assert_eq!(session.warmth_for(at(T0 + 5_000)), ArrivalWarmth::Cold);
    assert!(
        session.has_memory(),
        "cooling must not erase the memory — only `reset` does"
    );
}

#[test]
fn a_cooled_session_hands_off_cold_again() {
    let session = warm_at_t0();
    assert_eq!(session.warmth_for(at(T0 + 10)), ArrivalWarmth::Warm);
    assert_eq!(session.warmth_for(at(T0 + 10_000)), ArrivalWarmth::Cold);
    // And it stays cold: cooling is not a one-shot.
    assert_eq!(session.warmth_for(at(T0 + 10_001)), ArrivalWarmth::Cold);
}

// ---------------------------------------------------------------------------
// Refreshing warmth
// ---------------------------------------------------------------------------

#[test]
fn a_long_hover_keeps_the_session_warm() {
    let mut session = WarmSession::new();
    for ms in frames(T0, T0 + 5_000) {
        session.note_visible(at(ms));
        // The frame after each visible frame is warm, because the last visible
        // instant is the frame the user left on.
        assert_eq!(
            session.warmth_for(at(ms + WINDOW)),
            ArrivalWarmth::Warm,
            "still warm after a visible frame at {ms}"
        );
    }
}

#[test]
fn note_visible_only_moves_the_record_forward() {
    let mut session = warm_at_t0();
    session.note_visible(at(T0 - 5_000));
    assert_eq!(
        session.last_visible(),
        Some(at(T0)),
        "an older report must not rewind warmth"
    );
    assert_eq!(session.warmth_for(at(T0 + 1)), ArrivalWarmth::Warm);

    session.note_visible(at(T0 + 3_000));
    assert_eq!(session.last_visible(), Some(at(T0 + 3_000)));
    assert_eq!(session.warmth_for(at(T0 + 3_050)), ArrivalWarmth::Warm);
}

#[test]
fn re_reporting_the_same_frame_is_idempotent() {
    let mut session = WarmSession::new();
    for _ in 0..50 {
        session.note_visible(at(T0));
    }
    assert_eq!(session.last_visible(), Some(at(T0)));
    assert_eq!(session.warmth_for(at(T0 + WINDOW)), ArrivalWarmth::Warm);
}

#[test]
fn a_backwards_arrival_instant_is_warm_rather_than_underflowed() {
    let session = warm_at_t0();
    assert_eq!(
        session.warmth_for(at(T0 - 9_000)),
        ArrivalWarmth::Warm,
        "a backwards reading means no time has passed, which is the warm side"
    );
    assert_ne!(session.warmth_remaining(at(T0 - 9_000)), u64::MAX);
}

// ---------------------------------------------------------------------------
// `warmth_remaining`
// ---------------------------------------------------------------------------

#[test]
fn warmth_remaining_counts_down_across_the_window() {
    let session = warm_at_t0();
    for gap in 0..=WINDOW {
        assert_eq!(
            session.warmth_remaining(at(T0 + gap)),
            WINDOW - gap,
            "gap {gap}"
        );
    }
}

#[test]
fn warmth_remaining_is_zero_when_there_is_nothing_to_wait_for() {
    assert_eq!(WarmSession::new().warmth_remaining(at(T0)), 0);
    let session = warm_at_t0();
    assert_eq!(session.warmth_remaining(at(T0 + WINDOW + 1)), 0);
    assert_eq!(session.warmth_remaining(at(T0 + 100_000)), 0);
}

/// The inclusive boundary makes `0` remaining and `Warm` coexist for exactly one
/// instant. Pinned so the accessor is never read as "cold".
#[test]
fn zero_remaining_still_means_the_boundary_is_warm() {
    let session = warm_at_t0();
    assert_eq!(session.warmth_remaining(at(T0 + WINDOW)), 0);
    assert_eq!(session.warmth_for(at(T0 + WINDOW)), ArrivalWarmth::Warm);
    assert_eq!(session.warmth_for(at(T0 + WINDOW + 1)), ArrivalWarmth::Cold);
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_erasures_the_memory_rather_than_cooling_it() {
    let mut session = warm_at_t0();
    session.reset();
    assert!(!session.has_memory());
    assert_eq!(session.last_visible(), None);
    assert_eq!(session.warmth_for(at(T0 + 1)), ArrivalWarmth::Cold);
    assert_eq!(session.warmth_remaining(at(T0 + 1)), 0);
}

// ---------------------------------------------------------------------------
// Session isolation
// ---------------------------------------------------------------------------

#[test]
fn two_sessions_share_nothing() {
    let warmed = warm_at_t0();
    let other = WarmSession::new();
    assert_eq!(warmed.warmth_for(at(T0 + 1)), ArrivalWarmth::Warm);
    assert_eq!(other.warmth_for(at(T0 + 1)), ArrivalWarmth::Cold);

    let mut other = warmed;
    other.reset();
    assert_eq!(
        warmed.warmth_for(at(T0 + 1)),
        ArrivalWarmth::Warm,
        "a copy must not be able to cool the original"
    );
}

// ---------------------------------------------------------------------------
// The whole decision table, exhaustively
// ---------------------------------------------------------------------------

#[test]
fn every_gap_either_side_of_the_window_matches_the_rule() {
    let session = warm_at_t0();
    for gap in 0..=250_u64 {
        let want = if gap <= WINDOW {
            ArrivalWarmth::Warm
        } else {
            ArrivalWarmth::Cold
        };
        assert_eq!(
            session.warmth_for(at(T0 + gap)),
            want,
            "gap {gap} ms must be {want:?}"
        );
        assert_eq!(session.is_warm(at(T0 + gap)), want.is_warm());
    }
}

#[test]
fn the_table_holds_at_large_gaps_too() {
    let session = warm_at_t0();
    for gap in [1_000, 5_000, 60_000, 3_600_000] {
        assert_eq!(session.warmth_for(at(T0 + gap)), ArrivalWarmth::Cold);
    }
}

// ---------------------------------------------------------------------------
// Controller integration: the verdict actually drives the machine
// ---------------------------------------------------------------------------

#[test]
fn a_warm_arrival_shows_on_the_first_tick() {
    let session = warm_at_t0();
    let mut controller = HoverController::new();
    let arrival = at(T0 + 40);
    assert!(controller.pointer_enter_warm(arrival, session.warmth_for(arrival)));
    assert_eq!(controller.tick(arrival), Visibility::Shown);
    assert_eq!(controller.active_delay().as_millis(), 0);
}

#[test]
fn a_cold_verdict_costs_the_configured_delay() {
    let session = WarmSession::new();
    let mut controller = HoverController::new();
    let arrival = at(T0);
    assert!(controller.pointer_enter_warm(arrival, session.warmth_for(arrival)));
    assert_eq!(controller.tick(arrival), Visibility::Pending);
    assert_eq!(
        controller.tick(at(T0 + HOVER_DELAY - 1)),
        Visibility::Pending
    );
    assert_eq!(
        controller.tick(at(T0 + HOVER_DELAY)),
        Visibility::Shown,
        "a cold handoff must cost exactly the full hover delay"
    );
}

/// The handoff may only ever *remove* a wait. A host that raised its hover delay
/// must not find the warm path overriding it in the other direction.
#[test]
fn a_warm_handoff_never_shortens_a_raised_host_delay() {
    let raised = std::time::Duration::from_millis(2_000);
    let mut controller = HoverController::with_delay(raised);
    let arrival = at(T0);
    assert!(controller.pointer_enter_warm(arrival, ArrivalWarmth::Cold));
    assert_eq!(controller.tick(at(T0 + 1_999)), Visibility::Pending);
    assert_eq!(controller.tick(at(T0 + 2_000)), Visibility::Shown);
    assert_eq!(controller.active_delay(), raised);
}

#[test]
fn a_warm_handoff_leaves_the_configured_delay_untouched() {
    let mut controller = HoverController::new();
    controller.pointer_enter_warm(at(T0), ArrivalWarmth::Warm);
    assert_eq!(
        controller.delay().as_millis() as u64,
        HOVER_DELAY,
        "the cold delay a later arrival pays must not have been changed"
    );
    controller.pointer_exit();
    controller.pointer_enter(at(T0 + 50_000));
    assert_eq!(
        controller.tick(at(T0 + 50_000 + HOVER_DELAY - 1)),
        Visibility::Pending
    );
}

#[test]
fn a_warm_re_enter_cannot_shorten_an_arrival_in_flight() {
    let session = warm_at_t0();
    let mut controller = HoverController::new();
    let cold_arrival = at(T0 - 5_000);
    assert!(controller.pointer_enter_warm(cold_arrival, ArrivalWarmth::Cold));
    // The session warms up while the pointer is already resting here; reporting
    // the arrival again must not re-time the wait that is already running.
    for step in 1..=20 {
        assert!(!controller.pointer_enter_warm(at(T0 + step), session.warmth_for(at(T0 + step))));
    }
    assert_eq!(controller.entered_at(), Some(cold_arrival));
    assert_eq!(
        controller.tick(at(cold_arrival.as_millis() + HOVER_DELAY - 1)),
        Visibility::Pending
    );
}

#[test]
fn active_delay_reports_the_live_arrivals_wait() {
    let mut controller = HoverController::new();
    assert_eq!(
        controller.active_delay().as_millis() as u64,
        HOVER_DELAY,
        "at rest the accessor reports the configured pointer delay"
    );
    controller.pointer_enter_warm(at(T0), ArrivalWarmth::Warm);
    assert_eq!(controller.active_delay().as_millis(), 0);
    controller.pointer_exit();
    controller.keyboard_focus_enter(at(T0 + 1_000));
    assert_eq!(controller.active_delay().as_millis(), 0);
    controller.pointer_exit();
    controller.pointer_enter_warm(at(T0 + 2_000), ArrivalWarmth::Cold);
    assert_eq!(controller.active_delay().as_millis() as u64, HOVER_DELAY);
}

// ---------------------------------------------------------------------------
// The headline: one session, two controls
// ---------------------------------------------------------------------------

#[test]
fn a_visible_tooltip_hands_off_to_the_next_control_at_zero_wait() {
    let mut session = WarmSession::new();
    let mut first = HoverController::new();
    let mut second = HoverController::new();

    // Cold arrival at the first icon: the full 400 ms.
    first.pointer_enter_warm(at(T0), session.warmth_for(at(T0)));
    assert_eq!(first.tick(at(T0)), Visibility::Pending);
    assert_eq!(first.tick(at(T0 + HOVER_DELAY)), Visibility::Shown);
    session.note_visible(at(T0 + HOVER_DELAY));

    // The pointer steps one icon along 30 ms later: no wait at all.
    let handoff = at(T0 + HOVER_DELAY + 30);
    second.pointer_enter_warm(handoff, session.warmth_for(handoff));
    assert_eq!(second.tick(handoff), Visibility::Shown);
    assert_eq!(second.active_delay().as_millis(), 0);
}

#[test]
fn a_gap_longer_than_the_window_makes_the_next_control_pay_in_full() {
    let mut session = WarmSession::new();
    let mut first = HoverController::new();
    let mut second = HoverController::new();

    first.pointer_enter_warm(at(T0), session.warmth_for(at(T0)));
    session.note_visible(at(T0 + HOVER_DELAY));

    let arrival = at(T0 + HOVER_DELAY + WINDOW + 1);
    assert_eq!(session.warmth_for(arrival), ArrivalWarmth::Cold);
    second.pointer_enter_warm(arrival, ArrivalWarmth::Cold);
    assert_eq!(second.tick(arrival), Visibility::Pending);
    assert_eq!(
        second.tick(at(arrival.as_millis() + HOVER_DELAY)),
        Visibility::Shown
    );
}

#[test]
fn a_dismissed_tooltip_stops_refreshing_warmth_but_does_not_cool_it_early() {
    // Dismissal is not a session event: the session simply stops being told the
    // tooltip is visible, so warmth decays from the last frame it actually was.
    let mut session = WarmSession::new();
    let mut controller = HoverController::new();
    controller.pointer_enter_warm(at(T0), ArrivalWarmth::Cold);
    session.note_visible(at(T0 + HOVER_DELAY));
    let last_seen = at(T0 + HOVER_DELAY + 60);
    session.note_visible(last_seen);
    let _ = controller.dismiss(crate::surface::tooltip::DismissReason::Escape);

    assert_eq!(session.last_visible(), Some(last_seen));
    assert_eq!(
        session.warmth_for(at(last_seen.as_millis() + WINDOW)),
        ArrivalWarmth::Warm
    );
    assert_eq!(
        session.warmth_for(at(last_seen.as_millis() + WINDOW + 1)),
        ArrivalWarmth::Cold
    );
}

// ---------------------------------------------------------------------------
// Determinism: replay the whole host loop
// ---------------------------------------------------------------------------

/// The full script: cold arrival, a long visible hover, a warm handoff to the
/// next icon, a gap long enough to cool, then a third arrival that pays again.
fn scripted_session() -> Vec<(u64, Option<usize>)> {
    let mut script = Vec::new();
    for ms in frames(T0, T0 + 1_000) {
        script.push((ms, Some(0)));
    }
    for ms in frames(T0 + 1_016, T0 + 1_100) {
        script.push((ms, Some(1)));
    }
    for ms in frames(T0 + 1_116, T0 + 1_400) {
        script.push((ms, None));
    }
    for ms in frames(T0 + 1_416, T0 + 1_900) {
        script.push((ms, Some(2)));
    }
    script
}

#[test]
fn the_same_script_replayed_twice_gives_identical_decisions() {
    let script = scripted_session();
    let first_run = run_script(&script);
    let second_run = run_script(&script);
    assert_eq!(
        first_run, second_run,
        "a replay of the same events at the same instants must be identical"
    );
    assert_eq!(first_run.len(), script.len());
}

#[test]
fn the_replayed_loop_shows_cold_then_warm_then_cold() {
    let script = scripted_session();
    let trace = run_script(&script);
    let painted_at = |frame: usize| trace[frame].iter().any(|v| v.is_visible());

    // The first icon is cold: nothing paints until the full delay has elapsed.
    let first_frame = 0;
    assert!(!painted_at(first_frame));
    let cold_shown = trace
        .iter()
        .position(|frame| frame[0].is_visible())
        .expect("the first icon must become visible");
    assert_eq!(script[cold_shown].0, T0 + HOVER_DELAY);

    // The handoff to the second icon paints on its very first frame, because the
    // first icon's tooltip was visible on the frame before it.
    let handoff_frame = trace
        .iter()
        .position(|frame| frame[1].is_visible())
        .expect("the second icon must become visible");
    assert_eq!(
        script[handoff_frame].0,
        T0 + 1_016,
        "the handoff must land on the arrival frame, not one delay later"
    );

    // After a 300 ms gap with nothing visible, the third icon pays the full
    // delay again.
    let third_arrival = script
        .iter()
        .position(|(_, hovered)| *hovered == Some(2))
        .expect("the third icon must be hovered");
    assert!(!trace[third_arrival][2].is_visible());
    let third_shown = trace[third_arrival..]
        .iter()
        .position(|frame| frame[2].is_visible())
        .expect("the third icon must eventually become visible");
    assert_eq!(
        script[third_arrival + third_shown].0,
        script[third_arrival].0 + HOVER_DELAY,
        "a cooled session must charge the full delay again"
    );
}

#[test]
fn the_loop_is_independent_of_frame_rate() {
    // The same three phases at 16ms and at 8ms frames must reach the same
    // conclusion: the decision is a function of instants, not of how often the
    // host happens to ask.
    let coarse: Vec<u64> = frames(T0, T0 + 1_000);
    let fine: Vec<u64> = (T0..=T0 + 1_000).step_by(8).collect();
    let coarse_trace = run_script(&coarse.iter().map(|ms| (*ms, Some(0))).collect::<Vec<_>>());
    let fine_trace = run_script(&fine.iter().map(|ms| (*ms, Some(0))).collect::<Vec<_>>());
    let coarse_first_paint = coarse_trace
        .iter()
        .position(|frame| frame[0].is_visible())
        .map(|index| coarse[index]);
    let fine_first_paint = fine_trace
        .iter()
        .position(|frame| frame[0].is_visible())
        .map(|index| fine[index]);
    assert_eq!(coarse_first_paint, Some(T0 + HOVER_DELAY));
    assert_eq!(coarse_first_paint, fine_first_paint);
}
