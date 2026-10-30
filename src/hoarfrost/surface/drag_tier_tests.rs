//! Tests for the movable drag tiers — the `on_drag_settle` contract
//! (§3.0.4.2, `UIF-MOV-001`).

use crate::binding::GestureStart;
use crate::surface::drag_tier::DragGesture;
use crate::surface::movable::MovableGeometry;
use iced::{Point, Size};

fn geometry_at(x: f32, y: f32) -> MovableGeometry {
    MovableGeometry::new(Point::new(x, y), Size::new(680.0, 440.0))
}

fn dragged() -> DragGesture {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(100.0, 100.0));
    let _ = drag.live(geometry_at(140.0, 120.0));
    let _ = drag.live(geometry_at(180.0, 140.0));
    drag
}

// -- the opening -----------------------------------------------------------

#[test]
fn a_fresh_gesture_is_closed_and_has_emitted_no_tier() {
    let drag = DragGesture::new();
    assert!(!drag.is_open());
    assert_eq!(drag.live_ticks(), 0);
    assert!(!drag.settled());
    assert_eq!(drag.last_tier(), "none");
    assert_eq!(drag.snapshot(), None);
}

#[test]
fn beginning_captures_the_geometry_the_drag_grabbed() {
    let mut drag = DragGesture::new();
    assert_eq!(drag.begin(geometry_at(100.0, 100.0)), GestureStart::Started);
    assert!(drag.is_open());
    assert_eq!(drag.snapshot(), Some(geometry_at(100.0, 100.0)));
}

#[test]
fn a_nested_begin_is_refused_and_the_outer_snapshot_still_governs() {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(100.0, 100.0));
    assert_eq!(
        drag.begin(geometry_at(999.0, 999.0)),
        GestureStart::AlreadyOpen
    );
    assert_eq!(
        drag.snapshot(),
        Some(geometry_at(100.0, 100.0)),
        "the inner begin must not re-capture the revert target"
    );

    let cancel = drag.cancel().expect("the outer gesture cancels");
    assert_eq!(cancel.restored(), &geometry_at(100.0, 100.0));
}

// -- the live tier ---------------------------------------------------------

#[test]
fn every_drag_frame_emits_one_live_tick() {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(0.0, 0.0));

    for step in 1..=12 {
        let tick = drag
            .live(geometry_at(step as f32, 0.0))
            .expect("an open drag emits");
        assert_eq!(tick.value(), &geometry_at(step as f32, 0.0));
    }
    assert_eq!(drag.live_ticks(), 12);
    assert_eq!(drag.last_tier(), "live");
    assert!(!drag.settled());
}

#[test]
fn a_closed_gesture_emits_no_live_tick_and_counts_nothing() {
    let mut drag = DragGesture::new();
    assert!(drag.live(geometry_at(1.0, 1.0)).is_none());
    assert_eq!(drag.live_ticks(), 0);
}

#[test]
fn live_ticks_carry_the_whole_value_so_dropping_any_of_them_is_safe() {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(0.0, 0.0));
    let _ = drag.live(geometry_at(50.0, 50.0));
    let last = drag
        .live(geometry_at(90.0, 90.0))
        .expect("open drag emits a tick");
    // The surviving tick alone is enough to reconstruct where the card is.
    assert_eq!(last.value().position, Point::new(90.0, 90.0));
}

// -- the settle tier -------------------------------------------------------

#[test]
fn the_release_commits_exactly_once() {
    let mut drag = dragged();
    let commit = drag
        .commit(geometry_at(180.0, 140.0))
        .expect("the release settles the drag");
    assert_eq!(commit.value(), &geometry_at(180.0, 140.0));
    assert!(drag.committed());
    assert!(!drag.is_open());
    assert_eq!(drag.last_tier(), "commit");

    assert!(
        drag.commit(geometry_at(180.0, 140.0)).is_none(),
        "a second settle through the same gesture has nothing to settle"
    );
    assert_eq!(drag.live_ticks(), 2, "the transcript is not rewritten");
}

#[test]
fn committing_a_drag_that_never_began_emits_nothing() {
    let mut drag = DragGesture::new();
    assert!(drag.commit(geometry_at(10.0, 10.0)).is_none());
    assert!(!drag.committed());
}

#[test]
fn a_commit_closes_the_gesture_so_a_later_cancel_emits_nothing() {
    let mut drag = dragged();
    drag.commit(geometry_at(180.0, 140.0))
        .expect("settles once");
    assert!(
        drag.cancel().is_none(),
        "the two settling paths are mutually exclusive"
    );
    assert!(!drag.cancelled());
}

// -- the revert tier -------------------------------------------------------

#[test]
fn escape_restores_the_geometry_from_before_the_first_delta() {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(100.0, 100.0));
    let _ = drag.live(geometry_at(400.0, 400.0));
    let _ = drag.live(geometry_at(700.0, 700.0));

    let cancel = drag.cancel().expect("an open drag cancels");
    assert_eq!(
        cancel.restored(),
        &geometry_at(100.0, 100.0),
        "the revert target is the grab point, not the last live frame"
    );
    assert!(drag.cancelled());
    assert!(!drag.committed());
    assert_eq!(drag.last_tier(), "cancel");
}

#[test]
fn a_cancelled_drag_cannot_be_committed_afterwards() {
    let mut drag = dragged();
    drag.cancel().expect("cancels");
    assert!(drag.commit(geometry_at(180.0, 140.0)).is_none());
}

#[test]
fn cancelling_a_closed_gesture_emits_nothing() {
    let mut drag = DragGesture::new();
    assert!(drag.cancel().is_none());
    assert!(!drag.cancelled());
}

// -- teardown ------------------------------------------------------------

#[test]
fn abort_drops_the_gesture_without_emitting_either_tier() {
    let mut drag = dragged();
    drag.abort();
    assert!(!drag.is_open());
    assert!(!drag.committed());
    assert!(!drag.cancelled());
    assert!(!drag.settled());
    assert_eq!(
        drag.last_tier(),
        "live",
        "the transcript still shows the frames"
    );
    assert_eq!(drag.snapshot(), None);
}

// -- transcript / equality -----------------------------------------------

#[test]
fn the_transcript_distinguishes_live_only_from_settled() {
    let mut live_only = DragGesture::new();
    live_only.begin(geometry_at(0.0, 0.0));
    let _ = live_only.live(geometry_at(1.0, 1.0));

    let mut settled = DragGesture::new();
    settled.begin(geometry_at(0.0, 0.0));
    let _ = settled.live(geometry_at(1.0, 1.0));
    settled.commit(geometry_at(1.0, 1.0));

    assert!(!live_only.settled());
    assert!(settled.settled());
    assert_ne!(live_only, settled);
}

#[test]
fn two_identical_transcripts_compare_equal() {
    let a = dragged();
    let b = dragged();
    assert_eq!(a, b);
}

#[test]
fn a_clone_is_independent_of_its_original() {
    let a = dragged();
    let mut b = a.clone();
    assert_eq!(a, b);

    b.commit(geometry_at(180.0, 140.0))
        .expect("the clone settles its own gesture");
    assert_ne!(a, b);
    assert!(a.is_open(), "the original's gesture is untouched");
}

#[test]
fn the_snapshot_is_readable_without_settling() {
    let mut drag = DragGesture::new();
    drag.begin(geometry_at(42.0, 24.0));
    let _ = drag.live(geometry_at(80.0, 80.0));
    assert_eq!(drag.snapshot(), Some(geometry_at(42.0, 24.0)));
    assert!(drag.is_open(), "reading the snapshot settles nothing");
}
