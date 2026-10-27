//! Tests for the hover-delay state machine.
//!
//! Every test passes its own clock. None of them sleeps.

use super::*;
use crate::surface::tooltip::HOVER_DELAY;
use crate::surface::tooltip::warm::ArrivalWarmth;
use std::time::Duration;

const D: Duration = Duration::from_millis(HOVER_DELAY);
const T0: TimeMs = TimeMs::millis(10_000);

fn at(ms: u64) -> TimeMs {
    TimeMs::millis(ms)
}

fn controller() -> HoverController {
    HoverController::with_delay(D)
}

// ---------------------------------------------------------------------------
// TimeMs
// ---------------------------------------------------------------------------

#[test]
fn time_ms_round_trips_through_millis() {
    assert_eq!(TimeMs::millis(1234).as_millis(), 1234);
    assert_eq!(TimeMs(5678).as_millis(), 5678);
}

#[test]
fn elapsed_since_measures_forward_distance() {
    assert_eq!(at(1_000).elapsed_since(at(400)), 600);
    assert_eq!(at(1_000).elapsed_since(at(1_000)), 0);
}

/// A host that hands over two readings out of order must get "no time passed", not a
/// wrapped `u64` and not a panic mid-frame.
#[test]
fn elapsed_since_saturates_against_a_backwards_clock() {
    let later = at(1_000);
    let earlier = at(4_000);
    assert_eq!(earlier.elapsed_since(later), 3_000);
    assert_eq!(later.elapsed_since(earlier), 0);
    assert_ne!(later.elapsed_since(earlier), u64::MAX);
}

#[test]
fn time_ms_orders() {
    assert!(at(1) < at(2));
    assert_eq!(TimeMs::default(), TimeMs::millis(0));
}

/// The clock the host reads must be monotonic across successive calls, otherwise the whole
/// model is built on sand.
#[test]
fn now_is_monotonic() {
    let a = TimeMs::now();
    let b = TimeMs::now();
    assert!(
        b >= a,
        "clock went backwards between two reads: {a:?} then {b:?}"
    );
}

// ---------------------------------------------------------------------------
// Resting state
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_controller_is_hidden_at_any_time() {
    let c = controller();
    for t in [TimeMs::millis(0), T0, at(1_000_000)] {
        assert_eq!(c.tick(t), Visibility::Hidden, "ticked at {t:?}");
    }
    assert!(!c.is_active());
    assert!(!c.is_suppressed());
    assert_eq!(c.entered_at(), None);
}

#[test]
fn default_and_new_agree() {
    assert_eq!(
        HoverController::default().delay(),
        HoverController::new().delay()
    );
    assert_eq!(HoverController::new().delay(), D);
}

// ---------------------------------------------------------------------------
// The delay itself
// ---------------------------------------------------------------------------

#[test]
fn one_millisecond_short_of_the_delay_is_pending() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(
        c.tick(at(T0.as_millis() + HOVER_DELAY - 1)),
        Visibility::Pending
    );
}

#[test]
fn exactly_at_the_delay_it_shows() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);
}

#[test]
fn long_after_the_delay_it_stays_shown() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + 90_000)), Visibility::Shown);
}

#[test]
fn pending_paints_nothing_and_shown_does() {
    assert!(!Visibility::Pending.is_visible());
    assert!(!Visibility::Hidden.is_visible());
    assert!(Visibility::Shown.is_visible());
}

#[test]
fn a_zero_delay_shows_on_the_first_tick() {
    // A legitimate configuration: a user who turned hover delays off.
    let mut c = HoverController::with_delay(Duration::ZERO);
    c.pointer_enter(T0);
    assert_eq!(c.tick(T0), Visibility::Shown);
}

/// The clock may be handed over out of order without the machine underflowing into
/// "shown" — a backwards reading means no time has passed, so it stays pending.
#[test]
fn a_backwards_clock_cannot_shortcut_the_delay() {
    let mut c = controller();
    c.pointer_enter(at(10_000));
    assert_eq!(c.tick(at(9_000)), Visibility::Pending);
    assert_eq!(c.tick(at(0)), Visibility::Pending);
    assert_eq!(c.tick(at(10_000 + HOVER_DELAY)), Visibility::Shown);
}

// ---------------------------------------------------------------------------
// Idempotent arrival
// ---------------------------------------------------------------------------

/// The single most important property for a real host: a caller that reports "pointer is
/// over the control" **every frame** must not keep resetting the timer, or the tooltip
/// never appears at all.
#[test]
fn repeated_enter_does_not_rearm_the_delay() {
    let mut c = controller();
    assert!(c.pointer_enter(T0), "first arrival re-arms");
    for step in 1..=50 {
        assert!(
            !c.pointer_enter(at(T0.as_millis() + step)),
            "frame {step} must not re-arm"
        );
    }
    assert_eq!(c.entered_at(), Some(T0), "arrival instant must not drift");
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);
}

#[test]
fn enter_after_exit_rearms_from_the_new_instant() {
    let mut c = controller();
    c.pointer_enter(T0);
    c.pointer_exit();
    let t1 = at(T0.as_millis() + 4_000);
    assert!(c.pointer_enter(t1));
    assert_eq!(c.entered_at(), Some(t1));
    assert_eq!(
        c.tick(at(t1.as_millis() + HOVER_DELAY - 1)),
        Visibility::Pending
    );
    assert_eq!(c.tick(at(t1.as_millis() + HOVER_DELAY)), Visibility::Shown);
}

// ---------------------------------------------------------------------------
// Purity of `tick`
// ---------------------------------------------------------------------------

/// `tick` answers without mutating, so a host may probe the same controller at many
/// instants and the answers must be stable and repeatable.
#[test]
fn tick_is_pure_and_repeatable() {
    let mut c = controller();
    c.pointer_enter(T0);
    let probe = [
        (at(0), Visibility::Pending),
        (at(T0.as_millis() - 1), Visibility::Pending),
        (at(T0.as_millis() + HOVER_DELAY), Visibility::Shown),
        (at(999_999), Visibility::Shown),
    ];
    for _ in 0..3 {
        for (when, want) in probe {
            assert_eq!(
                c.tick(when),
                want,
                "re-probe at {when:?} must give the same answer"
            );
        }
    }
    // And the controller is unchanged: the same sequence still resolves identically.
    assert_eq!(c.entered_at(), Some(T0));
    assert!(!c.is_suppressed());
}

// ---------------------------------------------------------------------------
// Exit
// ---------------------------------------------------------------------------

#[test]
fn exit_hides_immediately_from_shown() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);
    c.pointer_exit();
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Hidden);
}

#[test]
fn exit_hides_a_pending_tooltip() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(c.tick(T0), Visibility::Pending);
    c.pointer_exit();
    assert_eq!(c.tick(T0), Visibility::Hidden);
}

/// Exit takes no clock because the delay is a property of *arrival*. Exiting at any
/// instant must leave the machine in the same state.
#[test]
fn exit_is_time_invariant() {
    for exit_at in [TimeMs::millis(0), T0, at(10_000_000)] {
        let mut c = controller();
        c.pointer_enter(T0);
        c.pointer_exit();
        assert_eq!(c.entered_at(), None, "exit at {exit_at:?}");
        assert_eq!(c.tick(T0), Visibility::Hidden, "exit at {exit_at:?}");
        assert!(!c.is_suppressed(), "exit at {exit_at:?} must not suppress");
    }
}

#[test]
fn exit_from_rest_is_a_no_op() {
    let mut c = controller();
    c.pointer_exit();
    assert_eq!(c.tick(T0), Visibility::Hidden);
    assert!(!c.is_suppressed());
}

// ---------------------------------------------------------------------------
// Dismissal taxonomy
// ---------------------------------------------------------------------------

/// Every reason, and what it does to suppression. Table-driven so adding a variant to
/// [`DismissReason`] without deciding its class shows up here as a missing row.
#[test]
fn every_dismissal_reason_and_its_suppression_class() {
    let cases = [
        (DismissReason::PointerExit, false),
        (DismissReason::MousePress, true),
        (DismissReason::AnchorGone, false),
        (DismissReason::FocusMoved, true),
        (DismissReason::Escape, true),
        (DismissReason::LeftWindow, false),
    ];

    for (reason, expect_suppressed) in cases {
        let mut c = controller();
        c.pointer_enter(T0);
        assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);

        let out = c.dismiss(reason);
        assert_eq!(out.reason, reason);
        assert!(
            out.was_active,
            "{reason:?} on a shown tooltip must report active"
        );
        assert!(!out.consumed_input, "{reason:?} must never consume input");
        assert_eq!(
            c.is_suppressed(),
            expect_suppressed,
            "{reason:?} suppression"
        );
        assert_eq!(
            c.tick(at(T0.as_millis() + HOVER_DELAY * 4)),
            Visibility::Hidden,
            "{reason:?} must hide the tooltip"
        );
    }
}

/// The taxonomy predicate must agree with the behaviour above, for all six variants.
#[test]
fn clears_suppression_matches_the_machine() {
    let all = [
        DismissReason::PointerExit,
        DismissReason::MousePress,
        DismissReason::AnchorGone,
        DismissReason::FocusMoved,
        DismissReason::Escape,
        DismissReason::LeftWindow,
    ];
    for reason in all {
        let mut c = controller();
        c.pointer_enter(T0);
        let _ = c.dismiss(reason);
        assert_eq!(
            c.is_suppressed(),
            !reason.clears_suppression(),
            "{reason:?}: clears_suppression() disagrees with the machine"
        );
    }
}

/// A suppressed tooltip stays hidden **even past the delay** — the whole point is that it
/// does not come back while the cursor sits where the user just clicked.
#[test]
fn a_suppressed_tooltip_does_not_come_back_when_the_delay_elapses() {
    let mut c = controller();
    c.pointer_enter(T0);
    let _ = c.dismiss(DismissReason::MousePress);
    for extra in [0, 1, HOVER_DELAY, HOVER_DELAY * 10, 1_000_000] {
        assert_eq!(c.tick(at(T0.as_millis() + extra)), Visibility::Hidden);
    }
}

/// Suppression is not permanent: leaving the control clears it, so the next arrival is a
/// genuinely new one.
#[test]
fn suppression_clears_on_exit_and_a_re_enter_shows_again() {
    let mut c = controller();
    c.pointer_enter(T0);
    let _ = c.dismiss(DismissReason::Escape);
    assert!(c.is_suppressed());

    c.pointer_exit();
    assert!(!c.is_suppressed(), "exit must clear suppression");

    let t1 = at(50_000);
    c.pointer_enter(t1);
    assert_eq!(c.tick(at(50_000 + HOVER_DELAY - 1)), Visibility::Pending);
    assert_eq!(c.tick(at(50_000 + HOVER_DELAY)), Visibility::Shown);
}

/// A suppressed controller that is re-entered without an intervening exit must not show —
/// the pointer never left, so nothing new arrived.
#[test]
fn re_enter_without_exit_does_not_lift_suppression() {
    let mut c = controller();
    c.pointer_enter(T0);
    let _ = c.dismiss(DismissReason::FocusMoved);
    // Host keeps calling enter (it is idempotent); suppression must hold.
    assert!(!c.pointer_enter(at(20_000)));
    assert!(c.is_suppressed());
    assert_eq!(c.tick(at(20_000 + HOVER_DELAY * 4)), Visibility::Hidden);
}

#[test]
fn dismissing_a_pending_tooltip_reports_armed_and_still_suppresses() {
    let mut c = controller();
    c.pointer_enter(T0);
    assert_eq!(c.tick(T0), Visibility::Pending);
    let out = c.dismiss(DismissReason::MousePress);
    // `was_active` reports *armed*, not *visible*: `dismiss` takes no clock, so it cannot
    // know whether the delay had elapsed. The hover was live, so the dismissal acted.
    assert!(
        out.was_active,
        "the hover was armed, so there was something to dismiss"
    );
    assert!(!out.consumed_input);
    assert!(c.is_suppressed());
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Hidden);
}

#[test]
fn dismissing_a_resting_controller_reports_inactive() {
    for reason in [
        DismissReason::PointerExit,
        DismissReason::MousePress,
        DismissReason::AnchorGone,
        DismissReason::FocusMoved,
        DismissReason::Escape,
        DismissReason::LeftWindow,
    ] {
        let mut c = controller();
        let out = c.dismiss(reason);
        assert!(!out.was_active, "{reason:?} on a resting controller");
        assert!(!out.consumed_input);
    }
}

/// An exit-class dismissal while already hidden must still leave the machine clean, so a
/// host that fires `AnchorGone` defensively cannot wedge it.
#[test]
fn exit_class_dismissals_never_wedge_the_machine() {
    for reason in [
        DismissReason::PointerExit,
        DismissReason::AnchorGone,
        DismissReason::LeftWindow,
    ] {
        let mut c = controller();
        let _ = c.dismiss(reason);
        let _ = c.dismiss(reason);
        assert!(!c.is_suppressed(), "{reason:?} must not suppress");
        c.pointer_enter(T0);
        assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);
    }
}

// ---------------------------------------------------------------------------
// `is_active`
// ---------------------------------------------------------------------------

#[test]
fn is_active_tracks_armed_and_unsuppressed() {
    let mut c = controller();
    assert!(!c.is_active());
    c.pointer_enter(T0);
    assert!(
        c.is_active(),
        "armed counts as active even before the delay"
    );
    let _ = c.dismiss(DismissReason::MousePress);
    assert!(!c.is_active(), "suppressed is not active");
    c.pointer_exit();
    assert!(!c.is_active());
}

#[test]
fn delay_accessor_reports_what_was_configured() {
    assert_eq!(
        HoverController::new().delay(),
        Duration::from_millis(HOVER_DELAY)
    );
    let c = HoverController::with_delay(Duration::from_millis(37));
    assert_eq!(c.delay(), Duration::from_millis(37));
    assert_eq!(c.delay().as_millis(), 37);
}

/// A non-default delay must actually drive the machine, not just be reported.
#[test]
fn a_custom_delay_drives_the_boundary() {
    let mut c = HoverController::with_delay(Duration::from_millis(37));
    c.pointer_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + 36)), Visibility::Pending);
    assert_eq!(c.tick(at(T0.as_millis() + 37)), Visibility::Shown);
}

// ---------------------------------------------------------------------------
// The wait rides on the arrival (`UIF-TOK-007` warm handoff)
// ---------------------------------------------------------------------------

/// The cold arm of the warm-handoff entry point must cost **exactly** what a plain
/// pointer arrival costs, at every instant. If the two ever drifted, a host would
/// be able to change a control's hover delay by choosing which method it called.
#[test]
fn a_cold_handoff_and_a_plain_enter_agree_at_every_instant() {
    let probes = [
        0,
        1,
        HOVER_DELAY / 2,
        HOVER_DELAY - 1,
        HOVER_DELAY,
        HOVER_DELAY + 1,
    ];
    for offset in probes {
        let mut plain = controller();
        plain.pointer_enter(T0);
        let mut handed_off = controller();
        handed_off.pointer_enter_warm(T0, ArrivalWarmth::Cold);
        assert_eq!(
            handed_off.tick(at(T0.as_millis() + offset)),
            plain.tick(at(T0.as_millis() + offset)),
            "cold handoff must match a plain enter at +{offset}ms"
        );
        assert_eq!(handed_off.active_delay(), plain.active_delay());
    }
}

/// A warm handoff is a **pointer** arrival that happens to carry no wait. It must
/// not arrive looking like a keyboard arrival, because a host that distinguishes
/// the channels — a navbar that reports how the tooltip was reached — would be
/// told the wrong thing.
#[test]
fn a_warm_handoff_shows_at_once_but_stays_a_pointer_arrival() {
    let mut c = controller();
    assert!(c.pointer_enter_warm(T0, ArrivalWarmth::Warm));
    assert_eq!(c.tick(T0), Visibility::Shown);
    assert_eq!(c.arrival_source(), Some(ArrivalSource::Pointer));
    assert!(!c.is_keyboard_armed());
    assert_eq!(c.active_delay(), Duration::ZERO);
    assert_eq!(c.entered_at(), Some(T0));
}
