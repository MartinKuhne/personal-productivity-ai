//! Tests for the Pre-Rank pointer-capture ledger (§3.0.4.2, `UIF-WIN-003`
//! step 1).
//!
//! Four groups, in order:
//!
//! 1. **The slot** — taking it, refusing a second taker, and releasing it
//!    only for the control that holds it.
//! 2. **The outcome** — which answers consume the `Escape` and which let it
//!    travel, and the state of the slot after each.
//! 3. **Cancel, never commit** — the ledger driving *real* drag controls
//!    through their own cancel paths, so the revert is proven against a live
//!    [`GestureSlot`](crate::binding::GestureSlot) rather than a stub.
//! 4. **Honesty** — the ledger reports only what the host told it.

use std::cell::Cell;
use std::cell::RefCell;

use super::*;
use crate::composite::scroll_view::ThumbDrag;
use crate::widget::range_model::RangeModel;
use crate::widget::slider_host::SliderEmission;
use crate::widget::slider_host::SliderHost;

fn slider_capture() -> DragCapture {
    DragCapture::new("volume", DragKind::Slider)
}

// ---------------------------------------------------------------------------
// 1. The slot
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_ledger_reports_no_capture() {
    let ledger = PreRankCoordinator::new();
    assert!(!ledger.is_captured());
    assert_eq!(ledger.capture(), None);
}

#[test]
fn a_default_ledger_is_empty_too() {
    let ledger = PreRankCoordinator::default();
    assert!(!ledger.is_captured());
}

#[test]
fn begin_takes_an_empty_slot() {
    let mut ledger = PreRankCoordinator::new();
    assert_eq!(ledger.begin(slider_capture()), CaptureStart::Started);
    assert!(ledger.is_captured());
    assert_eq!(ledger.capture(), Some(&slider_capture()));
}

#[test]
fn a_second_begin_is_refused_and_names_the_incumbent() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let splitter = DragCapture::new("pane", DragKind::SplitDivider);
    let start = ledger.begin(splitter.clone());
    assert_eq!(
        start,
        CaptureStart::AlreadyHeld {
            held: slider_capture()
        },
        "one pointer captures one drag, so the incumbent keeps the slot"
    );
    assert_eq!(start.held_by(), Some(&slider_capture()));
}

#[test]
fn a_refused_begin_does_not_move_the_incumbent() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    let _ = ledger.begin(DragCapture::new("pane", DragKind::SplitDivider));

    assert_eq!(
        ledger.capture(),
        Some(&slider_capture()),
        "overwriting the incumbent would make its drag invisible to Escape"
    );
}

#[test]
fn a_started_flag_distinguishes_the_two_answers() {
    let mut ledger = PreRankCoordinator::new();
    assert!(ledger.begin(slider_capture()).is_started());
    assert!(!ledger.begin(slider_capture()).is_started());
}

#[test]
fn release_clears_the_slot_for_the_incumbent() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    assert!(ledger.release(&CaptureId::new("volume")));
    assert!(!ledger.is_captured());
}

#[test]
fn release_by_another_control_leaves_the_capture_in_place() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    assert!(
        !ledger.release(&CaptureId::new("pane")),
        "a settling control must not be able to drop somebody else's capture"
    );
    assert_eq!(ledger.capture(), Some(&slider_capture()));
}

#[test]
fn release_on_an_empty_slot_reports_nothing_was_cleared() {
    let mut ledger = PreRankCoordinator::new();
    assert!(!ledger.release(&CaptureId::new("volume")));
}

#[test]
fn clear_drops_whatever_is_held_and_hands_it_back() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    assert_eq!(ledger.clear(), Some(slider_capture()));
    assert!(!ledger.is_captured());
    assert_eq!(ledger.clear(), None);
}

#[test]
fn a_released_slot_can_be_taken_again() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    let _ = ledger.release(&CaptureId::new("volume"));
    assert!(
        ledger
            .begin(DragCapture::new("pane", DragKind::SplitDivider))
            .is_started()
    );
}

// ---------------------------------------------------------------------------
// 2. The outcome
// ---------------------------------------------------------------------------

#[test]
fn no_capture_means_no_cancel_closure_call() {
    let calls = Cell::new(0usize);
    let mut ledger = PreRankCoordinator::new();
    let outcome = ledger.pre_rank_cancel(|_| {
        calls.set(calls.get() + 1);
        true
    });
    assert_eq!(outcome, PreRankOutcome::NotCaptured);
    assert_eq!(
        calls.get(),
        0,
        "with nothing captured the closure must not run at all"
    );
}

#[test]
fn a_cancelled_capture_consumes_the_key_and_frees_the_slot() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let outcome = ledger.pre_rank_cancel(|capture| {
        assert_eq!(capture, &slider_capture());
        true
    });

    assert_eq!(
        outcome,
        PreRankOutcome::Cancelled {
            capture: slider_capture()
        }
    );
    assert!(outcome.is_cancelled());
    assert!(
        !outcome.proceed_with_rank_walk(),
        "one keypress cannot both revert a drag and dismiss a surface"
    );
    assert_eq!(outcome.capture(), Some(&slider_capture()));
    assert!(!ledger.is_captured(), "the cancelled capture is spent");
}

#[test]
fn a_control_with_nothing_open_makes_the_registration_stale() {
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let outcome = ledger.pre_rank_cancel(|_| false);

    assert_eq!(
        outcome,
        PreRankOutcome::Stale {
            capture: slider_capture()
        }
    );
    assert!(!outcome.is_cancelled(), "a ghost must not swallow the key");
    assert!(
        outcome.proceed_with_rank_walk(),
        "a dead registration must let the Escape continue to the ranks"
    );
    assert!(
        !ledger.is_captured(),
        "a provably dead registration is cleared"
    );
}

#[test]
fn the_cancel_closure_cannot_keep_the_capture_alive() {
    // The closure receives `&DragCapture`, never the ledger, so it cannot
    // re-take the slot, extend it, or leave it held. Whatever it does, the
    // slot is empty once the Pre-Rank step returns.
    let seen = RefCell::new(Vec::new());
    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let _ = ledger.pre_rank_cancel(|capture| {
        seen.borrow_mut().push(capture.clone());
        true
    });

    assert_eq!(seen.borrow().len(), 1);
    assert!(
        !ledger.is_captured(),
        "a settling cancel cannot leave the capture behind"
    );
}

#[test]
fn only_a_cancel_consumes_the_key() {
    for (outcome, consumed) in [
        (
            PreRankOutcome::Cancelled {
                capture: slider_capture(),
            },
            true,
        ),
        (
            PreRankOutcome::Stale {
                capture: slider_capture(),
            },
            false,
        ),
        (PreRankOutcome::NotCaptured, false),
    ] {
        assert_eq!(outcome.is_cancelled(), consumed, "{outcome:?}");
        assert_eq!(outcome.proceed_with_rank_walk(), !consumed, "{outcome:?}");
    }
}

#[test]
fn not_captured_is_the_only_outcome_about_nothing() {
    assert_eq!(PreRankOutcome::NotCaptured.capture(), None);
    assert!(
        PreRankOutcome::Cancelled {
            capture: slider_capture()
        }
        .capture()
        .is_some()
    );
}

// ---------------------------------------------------------------------------
// 3. Cancel, never commit — driven through real controls
// ---------------------------------------------------------------------------

/// Drive the ledger over a real slider mid-scrub and report what the slider
/// actually settled as.
fn cancel_slider_via_ledger(
    ledger: &mut PreRankCoordinator,
    slider: &mut SliderHost,
) -> (PreRankOutcome, Option<f64>) {
    let restored = Cell::<Option<f64>>::new(None);
    let outcome = ledger.pre_rank_cancel(|capture| {
        assert_eq!(capture.kind(), DragKind::Slider);
        let mut cancelled = false;
        for emission in slider.cancel().emissions {
            if let SliderEmission::Cancelled(cancel) = emission {
                restored.set(Some(*cancel.restored()));
                cancelled = true;
            }
        }
        cancelled
    });
    (outcome, restored.get())
}

#[test]
fn a_pre_rank_cancel_reverts_a_real_slider_to_its_pre_press_value() {
    let model = RangeModel::new(50.0, 0.0, 100.0);
    let mut slider = SliderHost::new();
    slider.press(&model, 0.5);
    slider.drag(&model, 0.8);
    assert!(
        (slider.current().unwrap_or_default() - 80.0).abs() < 1e-3,
        "the scrub has moved, got {:?}",
        slider.current()
    );

    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let (outcome, restored) = cancel_slider_via_ledger(&mut ledger, &mut slider);

    assert!(outcome.is_cancelled());
    assert_eq!(
        restored,
        Some(50.0),
        "the revert target is the value the user saw when they grabbed the control"
    );
    assert!(
        !slider.is_open(),
        "the gesture is settled, not left hanging"
    );
    assert!(!slider.is_capturing(), "the pointer is no longer captured");
}

#[test]
fn a_pre_rank_cancel_never_settles_a_slider_as_a_commit() {
    let model = RangeModel::new(50.0, 0.0, 100.0);
    let mut slider = SliderHost::new();
    slider.press(&model, 0.5);
    slider.drag(&model, 0.8);

    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());
    let (outcome, _) = cancel_slider_via_ledger(&mut ledger, &mut slider);

    assert!(outcome.is_cancelled());
    assert!(slider.cancelled(), "the tier produced was Cancel");
    assert!(
        !slider.committed(),
        "Escape must not persist the scrubbed value as a Commit"
    );
    assert_eq!(slider.last_tier(), "cancel");
}

#[test]
fn a_pre_rank_cancel_reverts_a_scroll_thumb_to_its_captured_offset() {
    let mut thumb = ThumbDrag::new(120.0);
    let _ = thumb.begin();
    let _ = thumb.drag(400.0, 1000.0, 200.0);
    assert_eq!(thumb.offset(), 400.0);

    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(DragCapture::new("reader-scroll", DragKind::Scrollbar));

    let outcome = ledger.pre_rank_cancel(|capture| {
        assert_eq!(capture.kind(), DragKind::Scrollbar);
        thumb.cancel().is_some()
    });

    assert!(outcome.is_cancelled());
    assert_eq!(
        thumb.offset(),
        120.0,
        "the view goes back where the user was looking"
    );
    assert!(!thumb.is_active());
}

#[test]
fn a_slider_that_already_released_makes_the_capture_stale() {
    let model = RangeModel::new(50.0, 0.0, 100.0);
    let mut slider = SliderHost::new();
    slider.press(&model, 0.5);
    slider.release(&model);
    assert!(slider.committed(), "the release settled as a Commit");

    let mut ledger = PreRankCoordinator::new();
    let _ = ledger.begin(slider_capture());

    let (outcome, restored) = cancel_slider_via_ledger(&mut ledger, &mut slider);

    assert_eq!(
        outcome,
        PreRankOutcome::Stale {
            capture: slider_capture()
        },
        "a host that forgot to release on pointer-up must not eat the Escape"
    );
    assert!(
        restored.is_none(),
        "nothing was cancelled, so nothing was restored"
    );
    assert!(
        !slider.cancelled(),
        "the committed drag must not be retro-converted into a cancel"
    );
}

// ---------------------------------------------------------------------------
// 4. Honesty — the ledger reports only what the host told it
// ---------------------------------------------------------------------------

#[test]
fn a_host_that_never_registers_never_cancels() {
    let model = RangeModel::new(50.0, 0.0, 100.0);
    let mut slider = SliderHost::new();
    slider.press(&model, 0.5);

    let mut ledger = PreRankCoordinator::new();
    assert_eq!(
        ledger.pre_rank_cancel(|_| true),
        PreRankOutcome::NotCaptured
    );
    assert!(
        !slider.cancelled(),
        "an unregistered drag is untouched by the Pre-Rank"
    );
}

#[test]
fn the_ledger_names_the_class_of_drag_that_took_the_key() {
    for kind in DragKind::ALL {
        let mut ledger = PreRankCoordinator::new();
        let _ = ledger.begin(DragCapture::new("control", kind));
        let outcome = ledger.pre_rank_cancel(|_| true);
        assert_eq!(outcome.capture().map(DragCapture::kind), Some(kind));
        assert_eq!(kind.label(), kind.label());
    }
}

#[test]
fn a_capture_id_round_trips_through_display_and_from() {
    let id = CaptureId::from("volume");
    assert_eq!(id.as_str(), "volume");
    assert_eq!(id.to_string(), "volume");
    assert_eq!(CaptureId::new(String::from("volume")), id);
    assert_ne!(CaptureId::new("volume"), CaptureId::new("panes"));
}
