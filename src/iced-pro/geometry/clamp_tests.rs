//! Boundary, sweep and degenerate-input tests for the movable-overlay
//! reachability solver (`UIF-MOV-002`).
//!
//! Every threshold is pinned to the exact pixel at which the verdict
//! changes, the invariant is swept across a grid of cards, viewports and
//! positions down to `640 × 480` and below, and every degenerate input
//! has an asserted result rather than a tolerated one.
//!
//! The expectations here are written from the spec's own numbers — `120px`
//! of visible title bar, a `36px` navbar, a `32px` title bar, a
//! `420 × 260` card floor — restated as test-local constants so a wrong
//! constant in the solver fails a test instead of moving both together.

use super::{
    ClampRule, ResizeHandle, SizePolicy, clamp_position, clamp_rect, clamp_size, drag_position,
    is_reachable, resize, title_bar_rect, visible_title_width,
};
use crate::token::size;
use iced::{Point, Rectangle, Size, Vector};

/// `UIF-MOV-002` — the visible title-bar floor.
const MIN_VISIBLE: f32 = 120.0;
/// `UIF-MOV-002` — the navbar height (`size.control.lg`).
const NAVBAR: f32 = 36.0;
/// `UIF-MOV-002` — the title bar height (`size.control.md`).
const TITLE_BAR: f32 = 32.0;
/// §4.7.1 — the minimum overlay card size.
const CARD_MIN: Size = Size::new(420.0, 260.0);
/// The rule under test.
const RULE: ClampRule = ClampRule::MOVABLE;

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn size(w: f32, h: f32) -> Size {
    Size::new(w, h)
}

fn vec(x: f32, y: f32) -> Vector {
    Vector::new(x, y)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(pt(x, y), size(w, h))
}

/// The canonical root-space viewport: origin `(0, 0)`, `UIF-WIN-002`.
fn viewport(w: f32, h: f32) -> Rectangle {
    Rectangle::with_size(size(w, h))
}

/// The legal span for a card's left edge, derived from the spec's numbers
/// rather than from the solver.
fn horizontal_span(viewport: Rectangle, card_width: f32) -> (f32, f32) {
    let required = MIN_VISIBLE.min(card_width).min(viewport.width);
    (
        viewport.x + required - card_width,
        viewport.x + viewport.width - required,
    )
}

/// The legal span for a card's top edge, derived from the spec's numbers.
fn vertical_span(viewport: Rectangle) -> (f32, f32) {
    let top = viewport.y + NAVBAR;
    (top, (viewport.y + viewport.height - TITLE_BAR).max(top))
}

/// The `UIF-MOV-002` invariant, asserted from the spec's numbers rather
/// than from anything the solver computed.
///
/// * at least `min(120, card width, viewport width)` of title bar lies
///   inside the viewport horizontally;
/// * the card's top edge sits between the navbar and the last row on
///   which the title bar still touches the bottom of the screen.
///
/// Neither clause mentions the size policy, so this helper proves the same
/// thing under a custom [`SizePolicy`] as under [`ClampRule::MOVABLE`].
fn assert_reachable(card: Rectangle, viewport: Rectangle, label: &str) {
    let required = MIN_VISIBLE.min(card.width).min(viewport.width);
    let visible = visible_title_width(card, viewport);
    assert!(
        visible >= required,
        "{label}: only {visible}px of a {}px title bar is visible in a {}px-wide viewport (need {required}px) - {:?}",
        card.width,
        viewport.width,
        card
    );

    let (top, bottom) = vertical_span(viewport);
    assert!(
        card.y >= top && card.y <= bottom,
        "{label}: card top {} is outside [{top}, {bottom}] (navbar {NAVBAR}, title bar {TITLE_BAR}) in a {}px-tall viewport - {:?}",
        card.y,
        viewport.height,
        card
    );

    assert!(
        is_reachable(card, viewport, RULE),
        "{label}: is_reachable disagrees with the spec arithmetic for {:?} in {:?}",
        card,
        viewport
    );
}

/// An inclusive stepped range. Steps are integers so every value is
/// exactly representable in `f32` and the boundary assertions are exact.
fn stepped(from: f32, to: f32, step: f32) -> Vec<f32> {
    let mut values = Vec::new();
    let mut value = from;
    while value <= to {
        values.push(value);
        value += step;
    }
    values
}

const SWEEP_VIEWPORTS: [Size; 11] = [
    Size::new(1920.0, 1080.0),
    Size::new(1280.0, 800.0),
    Size::new(1000.0, 700.0),
    Size::new(800.0, 600.0),
    Size::new(640.0, 480.0),
    Size::new(480.0, 360.0),
    Size::new(320.0, 240.0),
    Size::new(128.0, 96.0),
    Size::new(64.0, 48.0),
    Size::new(40.0, 40.0),
    Size::ZERO,
];

const SWEEP_CARDS: [Size; 7] = [
    Size::new(680.0, 440.0),
    Size::new(420.0, 260.0),
    Size::new(200.0, 150.0),
    Size::new(120.0, 120.0),
    Size::new(50.0, 50.0),
    Size::ZERO,
    Size::new(2000.0, 1500.0),
];

const RESIZE_DELTAS: [Vector; 7] = [
    Vector::new(0.0, 0.0),
    Vector::new(40.0, 30.0),
    Vector::new(-40.0, -30.0),
    Vector::new(1000.0, 800.0),
    Vector::new(-1000.0, -800.0),
    Vector::new(5000.0, 5000.0),
    Vector::new(-5000.0, -5000.0),
];

// ---------------------------------------------------------------------
// The rule itself
// ---------------------------------------------------------------------

#[test]
fn the_movable_rule_carries_the_spec_numbers() {
    assert_eq!(RULE.min_visible_title, MIN_VISIBLE);
    assert_eq!(RULE.top_bound, NAVBAR);
    assert_eq!(RULE.title_bar_height, TITLE_BAR);
    assert_eq!(RULE.size_policy.min, CARD_MIN);
    assert_eq!(RULE.size_policy.max, None);
    assert_eq!(ClampRule::default(), RULE);
}

#[test]
fn the_vertical_bounds_are_read_from_size_tokens() {
    // 36px is `size.control.lg` (the Top Navbar height) and 32px is
    // `size.control.md`; neither is a bare literal in the solver.
    assert_eq!(RULE.top_bound, size::CONTROL_LG);
    assert_eq!(RULE.title_bar_height, size::CONTROL_MD);
}

#[test]
fn the_visible_title_floor_is_not_a_token_and_is_named_once() {
    // 120px is a reachability requirement, not a visual size, so no
    // token exists for it. Pinning it here means changing it is a spec
    // change, not a typo.
    assert_eq!(RULE.min_visible_title, 120.0);
    assert_eq!(RULE.size_policy.min.width, 420.0);
    assert_eq!(RULE.size_policy.min.height, 260.0);
}

// ---------------------------------------------------------------------
// Horizontal — the exact pixel where the verdict changes
// ---------------------------------------------------------------------

#[test]
fn the_right_edge_boundary_is_the_exact_pixel() {
    // A 680px card in a 1000px viewport: the last x that keeps 120px
    // showing is 880, the first that does not is 881.
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);

    assert_eq!(clamp_position(pt(879.0, 100.0), card, vp, RULE).x, 879.0);
    assert_eq!(clamp_position(pt(880.0, 100.0), card, vp, RULE).x, 880.0);
    assert_eq!(clamp_position(pt(881.0, 100.0), card, vp, RULE).x, 880.0);

    assert!(is_reachable(rect(880.0, 100.0, 680.0, 440.0), vp, RULE));
    assert!(!is_reachable(rect(881.0, 100.0, 680.0, 440.0), vp, RULE));
    assert_eq!(
        visible_title_width(rect(880.0, 100.0, 680.0, 440.0), vp),
        120.0
    );
    assert_eq!(
        visible_title_width(rect(881.0, 100.0, 680.0, 440.0), vp),
        119.0
    );
}

#[test]
fn the_left_edge_boundary_is_the_exact_pixel() {
    // Symmetrically: -560 keeps exactly 120px, -561 keeps 119px.
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);

    assert_eq!(clamp_position(pt(-559.0, 100.0), card, vp, RULE).x, -559.0);
    assert_eq!(clamp_position(pt(-560.0, 100.0), card, vp, RULE).x, -560.0);
    assert_eq!(clamp_position(pt(-561.0, 100.0), card, vp, RULE).x, -560.0);

    assert!(is_reachable(rect(-560.0, 100.0, 680.0, 440.0), vp, RULE));
    assert!(!is_reachable(rect(-561.0, 100.0, 680.0, 440.0), vp, RULE));
    assert_eq!(
        visible_title_width(rect(-560.0, 100.0, 680.0, 440.0), vp),
        120.0
    );
    assert_eq!(
        visible_title_width(rect(-561.0, 100.0, 680.0, 440.0), vp),
        119.0
    );
}

#[test]
fn the_horizontal_boundaries_hold_across_card_widths() {
    // `120 - width` is the last legal x at the left edge and
    // `viewport.width - 120` at the right edge, for every card width.
    let vp = viewport(1000.0, 700.0);
    let widths = [120.0, 200.0, 420.0, 680.0, 900.0, 1200.0, 2000.0];
    let right_edge = 1000.0 - MIN_VISIBLE;

    for width in widths {
        let left_edge = MIN_VISIBLE - width;

        assert_eq!(
            clamp_position(pt(left_edge, 100.0), size(width, 300.0), vp, RULE).x,
            left_edge,
            "a {width}px card must keep its leftmost legal x"
        );
        assert_eq!(
            clamp_position(pt(left_edge - 1.0, 100.0), size(width, 300.0), vp, RULE).x,
            left_edge,
            "a {width}px card one pixel further left must be pulled back"
        );
        assert_eq!(
            clamp_position(pt(right_edge, 100.0), size(width, 300.0), vp, RULE).x,
            right_edge,
            "a {width}px card must keep its rightmost legal x"
        );
        assert_eq!(
            clamp_position(pt(right_edge + 1.0, 100.0), size(width, 300.0), vp, RULE).x,
            right_edge,
            "a {width}px card one pixel further right must be pulled back"
        );

        assert_reachable(rect(left_edge, 100.0, width, 300.0), vp, "left boundary");
        assert_reachable(rect(right_edge, 100.0, width, 300.0), vp, "right boundary");
        assert!(
            !is_reachable(rect(left_edge - 1.0, 100.0, width, 300.0), vp, RULE),
            "a {width}px card at {} must NOT be reachable",
            left_edge - 1.0
        );
        assert!(
            !is_reachable(rect(right_edge + 1.0, 100.0, width, 300.0), vp, RULE),
            "a {width}px card at {} must NOT be reachable",
            right_edge + 1.0
        );
        assert_eq!(
            visible_title_width(rect(left_edge, 100.0, width, 300.0), vp),
            MIN_VISIBLE
        );
        assert_eq!(
            visible_title_width(rect(right_edge, 100.0, width, 300.0), vp),
            MIN_VISIBLE
        );
    }
}

#[test]
fn the_clamp_lands_exactly_on_the_derived_span() {
    let card = size(680.0, 440.0);
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        let (lower, upper) = horizontal_span(vp, card.width);
        let upper_bound = upper.max(lower);

        assert_eq!(
            clamp_position(pt(1.0e6, 100.0), card, vp, RULE).x,
            upper_bound,
            "pushed right in a {}px viewport",
            vp_size.width
        );
        assert_eq!(
            clamp_position(pt(-1.0e6, 100.0), card, vp, RULE).x,
            lower,
            "pushed left in a {}px viewport",
            vp_size.width
        );
    }
}

#[test]
fn a_card_narrower_than_the_visible_minimum_is_clamped_fully_visible() {
    // A 50px card cannot show 120px of title bar. The requirement
    // degrades to the card's own width: all 50px of it must show.
    let vp = viewport(1000.0, 700.0);
    let card = size(50.0, 200.0);

    assert_eq!(clamp_position(pt(-1.0, 100.0), card, vp, RULE).x, 0.0);
    assert_eq!(clamp_position(pt(950.0, 100.0), card, vp, RULE).x, 950.0);
    assert_eq!(clamp_position(pt(951.0, 100.0), card, vp, RULE).x, 950.0);
    assert_eq!(clamp_position(pt(-500.0, 100.0), card, vp, RULE).x, 0.0);

    assert_reachable(rect(0.0, 100.0, 50.0, 200.0), vp, "narrow card left");
    assert_reachable(rect(950.0, 100.0, 50.0, 200.0), vp, "narrow card right");
    assert!(!is_reachable(rect(-1.0, 100.0, 50.0, 200.0), vp, RULE));
    assert!(!is_reachable(rect(951.0, 100.0, 50.0, 200.0), vp, RULE));
}

#[test]
fn a_viewport_narrower_than_the_requirement_pins_the_card_to_cover_it() {
    // 100px of viewport cannot show 120px of anything. The requirement
    // degrades to the viewport width, and the pinned position covers all
    // of it - the most title bar any position can show.
    let vp = viewport(100.0, 700.0);
    let card = size(680.0, 440.0);

    let clamped = clamp_position(pt(5.0, 100.0), card, vp, RULE);
    assert_eq!(clamped.x, 0.0);
    assert_eq!(
        visible_title_width(rect(clamped.x, 100.0, 680.0, 440.0), vp),
        100.0
    );

    let pinned = clamp_position(pt(-5000.0, 100.0), card, vp, RULE);
    assert_eq!(pinned.x, -580.0);
    assert_eq!(
        visible_title_width(rect(pinned.x, 100.0, 680.0, 440.0), vp),
        100.0
    );
    assert_reachable(rect(pinned.x, 100.0, 680.0, 440.0), vp, "tiny viewport");
}

#[test]
fn visible_title_width_measures_the_horizontal_overlap() {
    let vp = viewport(1000.0, 700.0);
    let cases = [
        (rect(100.0, 0.0, 200.0, 10.0), 200.0),
        (rect(900.0, 0.0, 200.0, 10.0), 100.0),
        (rect(-100.0, 0.0, 50.0, 10.0), 0.0),
        (rect(-100.0, 0.0, 150.0, 10.0), 50.0),
        (rect(1050.0, 0.0, 100.0, 10.0), 0.0),
        (rect(-500.0, 0.0, 2000.0, 10.0), 1000.0),
        (rect(0.0, 0.0, 0.0, 10.0), 0.0),
    ];
    for (card, expected) in cases {
        assert_eq!(visible_title_width(card, vp), expected, "case {card:?}");
    }
}

// ---------------------------------------------------------------------
// Vertical — the navbar top and the title-bar bottom
// ---------------------------------------------------------------------

#[test]
fn the_top_bound_is_the_navbar_height() {
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);

    assert_eq!(clamp_position(pt(100.0, 37.0), card, vp, RULE).y, 37.0);
    assert_eq!(clamp_position(pt(100.0, 36.0), card, vp, RULE).y, 36.0);
    assert_eq!(clamp_position(pt(100.0, 35.0), card, vp, RULE).y, 36.0);
    assert_eq!(clamp_position(pt(100.0, -500.0), card, vp, RULE).y, 36.0);

    assert!(is_reachable(rect(100.0, 36.0, 680.0, 440.0), vp, RULE));
    assert!(!is_reachable(rect(100.0, 35.0, 680.0, 440.0), vp, RULE));
}

#[test]
fn the_bottom_bound_is_viewport_height_minus_the_title_bar() {
    // 700 - 32 = 668: the title bar's own bottom edge lands exactly on
    // the bottom of the screen. One further and the grip is gone.
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);

    assert_eq!(clamp_position(pt(100.0, 667.0), card, vp, RULE).y, 667.0);
    assert_eq!(clamp_position(pt(100.0, 668.0), card, vp, RULE).y, 668.0);
    assert_eq!(clamp_position(pt(100.0, 669.0), card, vp, RULE).y, 668.0);
    assert_eq!(clamp_position(pt(100.0, 5000.0), card, vp, RULE).y, 668.0);

    assert!(is_reachable(rect(100.0, 668.0, 680.0, 440.0), vp, RULE));
    assert!(!is_reachable(rect(100.0, 669.0, 680.0, 440.0), vp, RULE));
}

#[test]
fn the_vertical_bounds_hold_across_viewport_heights() {
    for height in [240.0, 480.0, 700.0, 1080.0] {
        let vp = viewport(1000.0, height);
        let bottom = height - TITLE_BAR;
        let card = size(680.0, 440.0);

        assert_eq!(clamp_position(pt(100.0, 35.0), card, vp, RULE).y, NAVBAR);
        assert_eq!(clamp_position(pt(100.0, bottom), card, vp, RULE).y, bottom);
        assert_eq!(
            clamp_position(pt(100.0, bottom + 1.0), card, vp, RULE).y,
            bottom
        );
        assert!(is_reachable(rect(100.0, bottom, 680.0, 440.0), vp, RULE));
        assert!(!is_reachable(
            rect(100.0, bottom + 1.0, 680.0, 440.0),
            vp,
            RULE
        ));
    }
}

#[test]
fn the_vertical_clamp_ignores_the_card_height() {
    // The thing being protected is 32px tall whichever card carries it.
    let vp = viewport(1000.0, 700.0);
    for height in [260.0, 440.0, 2000.0, 0.0] {
        let card = size(680.0, height);
        assert_eq!(
            clamp_position(pt(100.0, -100.0), card, vp, RULE).y,
            NAVBAR,
            "a {height}px card must still stop at the navbar"
        );
        assert_eq!(
            clamp_position(pt(100.0, 5000.0), card, vp, RULE).y,
            700.0 - TITLE_BAR,
            "a {height}px card must still stop at the title-bar bound"
        );
    }
}

#[test]
fn a_viewport_shorter_than_navbar_plus_title_bar_pins_to_the_top_bound() {
    // 36 + 32 = 68 > 60: the interval is empty. "Never above the
    // navbar" is the harder half of the rule, so it is the one kept.
    let vp = viewport(1000.0, 60.0);
    let card = size(680.0, 440.0);

    assert_eq!(clamp_position(pt(100.0, 0.0), card, vp, RULE).y, NAVBAR);
    assert_eq!(clamp_position(pt(100.0, 50.0), card, vp, RULE).y, NAVBAR);
    assert_eq!(clamp_position(pt(100.0, 500.0), card, vp, RULE).y, NAVBAR);
    assert_reachable(rect(100.0, NAVBAR, 680.0, 440.0), vp, "60px viewport");
}

// ---------------------------------------------------------------------
// The clamp protects the title bar, not the card
// ---------------------------------------------------------------------

#[test]
fn a_card_may_hang_off_the_bottom_and_right_while_the_title_bar_shows() {
    let vp = viewport(1000.0, 700.0);
    let card = rect(880.0, 668.0, 680.0, 440.0);

    assert!(is_reachable(card, vp, RULE));
    assert_eq!(clamp_rect(card, vp, RULE), card);
    assert!(card.x + card.width > vp.width);
    assert!(card.y + card.height > vp.height);
}

#[test]
fn a_card_taller_than_the_viewport_is_not_moved_at_all() {
    // A fit-the-card rule would have nowhere to put this; the title-bar
    // rule is satisfied exactly at the top bound and leaves it alone.
    let vp = viewport(640.0, 480.0);
    let card = rect(0.0, NAVBAR, 680.0, 440.0);

    assert!(is_reachable(card, vp, RULE));
    assert_eq!(clamp_rect(card, vp, RULE), card);
}

#[test]
fn the_title_bar_rect_is_the_top_strip_of_the_card() {
    let card = rect(100.0, 200.0, 680.0, 440.0);
    assert_eq!(title_bar_rect(card, RULE), rect(100.0, 200.0, 680.0, 32.0));
    assert_eq!(
        title_bar_rect(rect(0.0, 0.0, 0.0, 0.0), RULE),
        rect(0.0, 0.0, 0.0, 32.0)
    );
}

// ---------------------------------------------------------------------
// clamp_rect and clamp_size
// ---------------------------------------------------------------------

#[test]
fn clamp_rect_moves_only_the_origin() {
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_rect(rect(5000.0, 5000.0, 680.0, 440.0), vp, RULE);

    assert_eq!(clamped, rect(880.0, 668.0, 680.0, 440.0));
    assert_reachable(clamped, vp, "clamp_rect");
}

#[test]
fn clamp_rect_leaves_a_reachable_card_untouched() {
    let vp = viewport(1000.0, 700.0);
    let card = rect(123.0, 234.0, 680.0, 440.0);
    assert!(is_reachable(card, vp, RULE));
    assert_eq!(clamp_rect(card, vp, RULE), card);
}

#[test]
fn clamp_position_and_clamp_rect_agree_on_the_origin() {
    let vp = viewport(1280.0, 800.0);
    for y in stepped(-200.0, 1200.0, 50.0) {
        for x in stepped(-1200.0, 2200.0, 50.0) {
            let card = rect(x, y, 680.0, 440.0);
            assert_eq!(
                clamp_position(card.position(), card.size(), vp, RULE),
                clamp_rect(card, vp, RULE).position(),
                "clamp_position and clamp_rect must be the same rule"
            );
        }
    }
}

#[test]
fn clamp_size_enforces_the_minimum_card_size() {
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_size(rect(100.0, 100.0, 100.0, 100.0), vp, RULE);

    assert_eq!(clamped.size(), CARD_MIN);
    assert_eq!(clamped.position(), pt(100.0, 100.0));
    assert_reachable(clamped, vp, "clamped to the minimum");
}

#[test]
fn clamp_size_enforces_a_maximum_when_the_policy_has_one() {
    let rule = ClampRule {
        size_policy: SizePolicy::bounded(CARD_MIN, size(800.0, 500.0)),
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_size(rect(100.0, 100.0, 2000.0, 2000.0), vp, rule);

    assert_eq!(clamped.size(), size(800.0, 500.0));
    assert_reachable(clamped, vp, "clamped to the maximum");
}

#[test]
fn clamp_size_moves_the_origin_because_sizing_moves_the_title_bar() {
    // The card is off to the left and too narrow to be reachable. Sizing
    // it up to the 420px floor changes the width the visible-title rule
    // is measured against, so the origin is pulled right as well.
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_size(rect(-500.0, 100.0, 100.0, 100.0), vp, RULE);

    assert_eq!(clamped, rect(-300.0, 100.0, 420.0, 260.0));
    assert_reachable(clamped, vp, "sized then reached");
}

#[test]
fn clamp_size_uses_the_rules_policy_not_a_hardcoded_floor() {
    let rule = ClampRule {
        size_policy: SizePolicy {
            min: size(100.0, 50.0),
            max: None,
        },
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_size(rect(100.0, 100.0, 10.0, 10.0), vp, rule);

    assert_eq!(clamped.size(), size(100.0, 50.0));
    // A 100px card is narrower than the 120px visible-title floor, so
    // the requirement degrades to showing all 100px of it.
    assert_reachable(clamped, vp, "custom policy");
    assert_eq!(visible_title_width(clamped, vp), 100.0);
}

// ---------------------------------------------------------------------
// Resize — the eight drag directions
// ---------------------------------------------------------------------

#[test]
fn resizing_from_each_of_the_eight_handles_keeps_the_title_bar_reachable() {
    assert_eq!(ResizeHandle::ALL.len(), 8);
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    assert_reachable(start, vp, "resize start");

    for handle in ResizeHandle::ALL {
        for delta in RESIZE_DELTAS {
            let result = resize(start, handle, delta, vp, RULE);
            assert_reachable(result, vp, "eight-handle resize");
        }
    }
}

#[test]
fn the_eight_handles_partition_the_four_edges() {
    for handle in ResizeHandle::ALL {
        let drags_anything = handle.drags_start_x()
            || handle.drags_end_x()
            || handle.drags_start_y()
            || handle.drags_end_y();
        assert!(drags_anything, "{handle:?} must drag at least one edge");

        // A handle never drags both ends of the same axis.
        assert!(
            !(handle.drags_start_x() && handle.drags_end_x()),
            "{handle:?} cannot drag both horizontal edges"
        );
        assert!(
            !(handle.drags_start_y() && handle.drags_end_y()),
            "{handle:?} cannot drag both vertical edges"
        );
    }

    assert!(ResizeHandle::TopLeft.drags_start_x());
    assert!(ResizeHandle::TopLeft.drags_start_y());
    assert!(!ResizeHandle::TopLeft.drags_end_x());
    assert!(!ResizeHandle::TopLeft.drags_end_y());
    assert!(ResizeHandle::TopRight.drags_end_x());
    assert!(ResizeHandle::TopRight.drags_start_y());
    assert!(ResizeHandle::BottomRight.drags_end_x());
    assert!(ResizeHandle::BottomRight.drags_end_y());
    assert!(ResizeHandle::BottomLeft.drags_start_x());
    assert!(ResizeHandle::BottomLeft.drags_end_y());
    assert!(ResizeHandle::Left.drags_start_x());
    assert!(!ResizeHandle::Left.drags_start_y());
    assert!(!ResizeHandle::Left.drags_end_y());
    assert!(ResizeHandle::Right.drags_end_x());
    assert!(ResizeHandle::Top.drags_start_y());
    assert!(!ResizeHandle::Top.drags_end_x());
    assert!(ResizeHandle::Bottom.drags_end_y());
    assert!(!ResizeHandle::Bottom.drags_start_x());
}

#[test]
fn dragging_the_bottom_right_corner_never_moves_the_origin() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(start, ResizeHandle::BottomRight, vec(100.0, 50.0), vp, RULE);

    assert_eq!(result.position(), pt(200.0, 100.0));
    assert_eq!(result.size(), size(780.0, 490.0));
}

#[test]
fn dragging_the_top_edge_up_stops_at_the_navbar_and_holds_the_bottom_edge() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let held_bottom = start.y + start.height;

    let result = resize(start, ResizeHandle::Top, vec(0.0, -200.0), vp, RULE);

    assert_eq!(result.y, NAVBAR, "the top edge must stop at the navbar");
    assert_eq!(
        result.y + result.height,
        held_bottom,
        "the bottom edge the drag is not holding must stay exactly where it was"
    );
    assert_eq!(result.x, start.x);
    assert_eq!(result.width, start.width);
}

#[test]
fn dragging_the_top_edge_up_does_not_grow_the_card_downwards() {
    // The failure mode this guards: clamping the origin without
    // re-deriving the extent leaves the card taller, so dragging the top
    // edge up shoves the bottom edge down instead of stopping.
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let held_bottom = start.y + start.height;

    let result = resize(start, ResizeHandle::TopLeft, vec(0.0, -5000.0), vp, RULE);

    assert_eq!(result.y, NAVBAR);
    assert_eq!(result.y + result.height, held_bottom);
    assert_eq!(result.x, start.x);
    assert_eq!(result.width, start.width);
}

#[test]
fn dragging_the_top_edge_down_stops_at_the_minimum_height() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let held_bottom = start.y + start.height;

    let result = resize(start, ResizeHandle::Top, vec(0.0, 300.0), vp, RULE);

    assert_eq!(result.height, CARD_MIN.height);
    assert_eq!(result.y, held_bottom - CARD_MIN.height);
    assert_eq!(result.y + result.height, held_bottom);
    assert_reachable(result, vp, "shrunk to the minimum from the top");
}

#[test]
fn dragging_the_top_left_corner_down_and_right_moves_the_origin_and_shrinks() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(start, ResizeHandle::TopLeft, vec(100.0, 50.0), vp, RULE);

    assert_eq!(result, rect(300.0, 150.0, 580.0, 390.0));
    assert_reachable(result, vp, "top-left drag down/right");
}

#[test]
fn dragging_the_top_left_corner_far_off_pins_at_the_bounds_and_holds_the_far_corner() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let held_right = start.x + start.width;
    let held_bottom = start.y + start.height;

    let result = resize(
        start,
        ResizeHandle::TopLeft,
        vec(-2000.0, -2000.0),
        vp,
        RULE,
    );

    // The drag grows the card to 2680px wide, which is legal all the way
    // down to `120 - 2680 = -2560`, so the left edge is *not* pinned -
    // 880px of title bar is showing, far more than the 120px floor. The
    // top edge does pin, because the navbar bound does not care how tall
    // the card is.
    assert_eq!(result.x, -1800.0);
    assert_eq!(result.width, 2680.0);
    assert_eq!(result.y, NAVBAR, "the top edge pins at the navbar");
    assert_eq!(result.x + result.width, held_right);
    assert_eq!(result.y + result.height, held_bottom);
    assert_eq!(visible_title_width(result, vp), 880.0);
    assert!(visible_title_width(result, vp) >= MIN_VISIBLE);
}

#[test]
fn dragging_the_bottom_left_corner_up_shrinks_without_touching_the_title_bar() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(
        start,
        ResizeHandle::BottomLeft,
        vec(100.0, -100.0),
        vp,
        RULE,
    );

    assert_eq!(result, rect(300.0, 100.0, 580.0, 340.0));
    assert_eq!(result.y, start.y, "a bottom drag never moves the title bar");
}

#[test]
fn dragging_the_bottom_left_corner_up_past_the_minimum_stops_at_it() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(
        start,
        ResizeHandle::BottomLeft,
        vec(1000.0, -1000.0),
        vp,
        RULE,
    );

    assert_eq!(result.size(), CARD_MIN);
    assert_eq!(result.y, start.y);
    assert_eq!(result.x + result.width, start.x + start.width);
    assert_reachable(result, vp, "bottom-left shrunk to the minimum");
}

#[test]
fn dragging_the_left_edge_past_the_minimum_stops_at_it_and_holds_the_right_edge() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(start, ResizeHandle::Left, vec(600.0, 0.0), vp, RULE);

    assert_eq!(result.width, CARD_MIN.width);
    assert_eq!(result.x + result.width, start.x + start.width);
    assert_eq!(result.y, start.y);
}

#[test]
fn dragging_the_left_edge_far_left_grows_the_card_and_holds_the_right_edge() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(start, ResizeHandle::Left, vec(-2000.0, 0.0), vp, RULE);

    // The card grew to 2680px and its left edge is free to sit at -1800:
    // 880px of title bar still shows, so the 120px floor never binds.
    // What the drag may not do is move the edge it is holding.
    assert_eq!(result.x, -1800.0);
    assert_eq!(result.width, 2680.0);
    assert_eq!(result.x + result.width, start.x + start.width);
    assert_eq!(visible_title_width(result, vp), 880.0);
    assert!(visible_title_width(result, vp) >= MIN_VISIBLE);
    assert_reachable(result, vp, "left edge dragged far out");
}

#[test]
fn resizing_a_card_hanging_off_the_right_keeps_the_title_reachable() {
    let vp = viewport(1000.0, 700.0);
    let start = clamp_rect(rect(700.0, 100.0, 680.0, 440.0), vp, RULE);
    assert!(start.x + start.width > vp.width);

    for handle in ResizeHandle::ALL {
        for delta in RESIZE_DELTAS {
            let result = resize(start, handle, delta, vp, RULE);
            assert_reachable(result, vp, "hanging card resize");
        }
    }
}

#[test]
fn resize_honours_a_maximum_size_policy() {
    let rule = ClampRule {
        size_policy: SizePolicy::bounded(CARD_MIN, size(800.0, 500.0)),
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(
        start,
        ResizeHandle::BottomRight,
        vec(1000.0, 1000.0),
        vp,
        rule,
    );

    assert_eq!(result.size(), size(800.0, 500.0));
    assert_eq!(result.position(), pt(200.0, 100.0));
}

#[test]
fn resize_from_an_unreachable_start_is_pulled_back() {
    let vp = viewport(1000.0, 700.0);
    let unreachable = rect(5000.0, 5000.0, 680.0, 440.0);
    assert!(!is_reachable(unreachable, vp, RULE));

    let result = resize(
        unreachable,
        ResizeHandle::BottomRight,
        vec(10.0, 10.0),
        vp,
        RULE,
    );
    assert_reachable(result, vp, "resized from an unreachable start");
    assert_eq!(result.position(), pt(880.0, 668.0));
}

#[test]
fn the_title_bar_outranks_the_held_edge_when_the_two_conflict() {
    // A narrow card whose held right edge is itself off-screen cannot
    // satisfy the visible-title rule from anywhere while keeping that
    // edge. The invariant wins and the held edge shifts.
    let rule = ClampRule {
        size_policy: SizePolicy {
            min: size(50.0, 50.0),
            max: None,
        },
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let start = rect(990.0, 100.0, 50.0, 300.0);
    assert!(!is_reachable(start, vp, RULE));

    let result = resize(start, ResizeHandle::Left, vec(-10.0, 0.0), vp, rule);

    assert_reachable(result, vp, "narrow card, held edge off-screen");
    assert_eq!(result.x, 900.0);
    assert!(
        result.x + result.width < start.x + start.width,
        "the held edge must shift left to keep the title bar visible"
    );
}

#[test]
fn under_the_movable_policy_a_start_drag_holds_the_edge_it_can() {
    // The edge the user is holding is the one thing a resize must not move
    // on its own. Re-deriving the extent from the held edge after the
    // invariant has had its say makes that exact - as long as the start
    // card was big enough for the held edge to be compatible with the
    // invariant in the first place:
    //
    // * horizontally the card must be at least `120px` wide, or holding
    //   its right edge leaves no position that shows `120px` of title bar;
    // * vertically it must be at least `260px` tall, or the navbar bound
    //   and the height floor squeeze the leftover short of the minimum.
    //
    // Where they are not compatible the held edge gives way, which is what
    // `the_title_bar_outranks_the_held_edge_when_the_two_conflict` and
    // `a_re_derived_extent_never_falls_below_the_size_policy_floor` pin.
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            let start = clamp_rect(
                rect(vp.width / 4.0, vp.height / 4.0, card.width, card.height),
                vp,
                RULE,
            );
            for handle in ResizeHandle::ALL {
                for delta in RESIZE_DELTAS {
                    let result = resize(start, handle, delta, vp, RULE);
                    if handle.drags_start_x() && card.width >= MIN_VISIBLE {
                        assert_eq!(
                            result.x + result.width,
                            start.x + start.width,
                            "a {handle:?} drag must hold the right edge still"
                        );
                    }
                    if handle.drags_start_y() && card.height >= CARD_MIN.height {
                        assert_eq!(
                            result.y + result.height,
                            start.y + start.height,
                            "a {handle:?} drag must hold the bottom edge still"
                        );
                    }
                    assert_reachable(result, vp, "held-edge sweep");
                }
            }
        }
    }
}

#[test]
fn a_re_derived_extent_never_falls_below_the_size_policy_floor() {
    // The invariant pushing the leading edge back can leave less than the
    // policy minimum between it and the held edge. The floor is re-applied
    // with the origin where the invariant put it, so the axis being dragged
    // is never returned under the policy minimum however degenerate the
    // start. The axis the drag does not touch is left exactly as it was -
    // clamping it too would resize an edge nobody is holding.
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            let start = clamp_rect(
                rect(vp.width / 4.0, vp.height / 4.0, card.width, card.height),
                vp,
                RULE,
            );
            for handle in ResizeHandle::ALL {
                for delta in RESIZE_DELTAS {
                    let result = resize(start, handle, delta, vp, RULE);
                    if handle.drags_start_x() || handle.drags_end_x() {
                        assert!(
                            result.width >= CARD_MIN.width,
                            "{handle:?} {delta:?} from {start:?} in {vp_size:?} gave width {}",
                            result.width
                        );
                    }
                    if handle.drags_start_y() || handle.drags_end_y() {
                        assert!(
                            result.height >= CARD_MIN.height,
                            "{handle:?} {delta:?} from {start:?} in {vp_size:?} gave height {}",
                            result.height
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn shrinking_from_the_right_pulls_the_origin_back_when_the_title_bar_needs_it() {
    // The mirror of the held-edge rule, and the reason the resize case is
    // not simply "clamp the size": the card hangs off the left with 180px
    // of title bar showing. Shrinking it to the 420px floor from the
    // right would leave it spanning -500..-80 - reachable no more - so
    // the origin is pulled right to keep the 120px.
    let vp = viewport(1000.0, 700.0);
    let start = rect(-500.0, 100.0, 680.0, 440.0);
    assert_eq!(visible_title_width(start, vp), 180.0);
    assert!(is_reachable(start, vp, RULE));

    let result = resize(start, ResizeHandle::Right, vec(-260.0, 0.0), vp, RULE);

    assert_eq!(result.width, CARD_MIN.width);
    assert_eq!(result.x, -300.0);
    assert_eq!(visible_title_width(result, vp), MIN_VISIBLE);
    assert_reachable(result, vp, "shrunk from the right");
}

#[test]
fn a_zero_delta_resize_is_a_no_op_for_a_reachable_card() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    for handle in ResizeHandle::ALL {
        assert_eq!(
            resize(start, handle, Vector::new(0.0, 0.0), vp, RULE),
            start,
            "{handle:?} with no delta must not move anything"
        );
    }
}

// ---------------------------------------------------------------------
// The overshoot contract
// ---------------------------------------------------------------------

#[test]
fn repeated_blocked_drag_deltas_do_not_accumulate() {
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);
    let left_bound = -560.0;

    let mut position = pt(200.0, 100.0);
    for _ in 0..5 {
        position = drag_position(position, vec(-500.0, 0.0), card, vp, RULE);
    }
    assert_eq!(position.x, left_bound);

    // One pixel back is one pixel of movement - the 2500px of overshoot
    // is gone, not banked up to be paid back.
    position = drag_position(position, vec(10.0, 0.0), card, vp, RULE);
    assert_eq!(position.x, left_bound + 10.0);

    position = drag_position(position, vec(-10.0, 0.0), card, vp, RULE);
    assert_eq!(position.x, left_bound);
}

#[test]
fn the_naive_total_delta_would_make_the_user_re_travel_the_overshoot() {
    // The contrast case that motivates the contract: clamping from the
    // drag-start origin plus a running total leaves the card pinned long
    // after the pointer has come back.
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);
    let drag_start = pt(200.0, 100.0);
    let total = vec(-500.0 * 5.0 + 10.0, 0.0);

    let naive = clamp_position(
        pt(drag_start.x + total.x, drag_start.y + total.y),
        card,
        vp,
        RULE,
    );
    assert_eq!(
        naive.x, -560.0,
        "still pinned: the pointer is ~2480px from home"
    );

    let mut incremental = drag_start;
    for _ in 0..5 {
        incremental = drag_position(incremental, vec(-500.0, 0.0), card, vp, RULE);
    }
    incremental = drag_position(incremental, vec(10.0, 0.0), card, vp, RULE);
    assert_eq!(
        incremental.x, -550.0,
        "one pixel of pointer travel must be one pixel of card travel"
    );
}

#[test]
fn a_blocked_frame_is_visible_as_a_difference_from_the_requested_position() {
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);
    let at_bound = pt(-560.0, 300.0);

    let requested = pt(at_bound.x - 100.0, at_bound.y);
    let clamped = drag_position(at_bound, vec(-100.0, 0.0), card, vp, RULE);
    assert_eq!(clamped, at_bound);
    assert_ne!(
        clamped, requested,
        "a blocked axis must differ from the requested position"
    );

    let free = pt(at_bound.x + 100.0, at_bound.y);
    let clamped_free = drag_position(at_bound, vec(100.0, 0.0), card, vp, RULE);
    assert_eq!(clamped_free, free);
}

#[test]
fn drag_position_clamps_its_current_input_too() {
    // The contract holds even when the caller hands over an unclamped
    // current position.
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);
    let clamped = drag_position(pt(999_999.0, 100.0), Vector::new(0.0, 0.0), card, vp, RULE);
    // x was far out and gets pulled back to the right-hand bound; y was
    // already legal at 100 and must not move.
    assert_eq!(clamped, pt(880.0, 100.0));
}

#[test]
fn clamping_is_idempotent() {
    let vp = viewport(1000.0, 700.0);
    for card in SWEEP_CARDS {
        for y in stepped(-800.0, 1500.0, 40.0) {
            for x in stepped(-2000.0, 2500.0, 40.0) {
                let once = clamp_position(pt(x, y), card, vp, RULE);
                let twice = clamp_position(once, card, vp, RULE);
                assert_eq!(once, twice, "clamping twice must not drift (card {card:?})");

                let rect_once = clamp_rect(rect(x, y, card.width, card.height), vp, RULE);
                let rect_twice = clamp_rect(rect_once, vp, RULE);
                assert_eq!(
                    rect_once, rect_twice,
                    "clamping a rect twice must not drift"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// Sweeps - the UIF-VER-002 boundary gate
// ---------------------------------------------------------------------

#[test]
fn every_position_on_a_grid_clamps_to_a_reachable_card() {
    let mut checked = 0_usize;
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            for y in stepped(-600.0, vp.height + 600.0, 40.0) {
                for x in stepped(-600.0, vp.width + 600.0, 40.0) {
                    let clamped = clamp_rect(rect(x, y, card.width, card.height), vp, RULE);
                    assert_reachable(clamped, vp, "grid sweep");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 10_000, "sweep must be substantial, {checked}");
}

#[test]
fn the_640x480_minimum_viewport_never_loses_the_title_bar() {
    // UIF-WIN-001's minimum window, swept fine: every card class, the
    // whole window and a margin outside it.
    let vp = viewport(640.0, 480.0);
    let cards = [
        size(680.0, 440.0),
        size(420.0, 260.0),
        size(200.0, 150.0),
        size(120.0, 120.0),
        size(50.0, 50.0),
    ];

    let mut checked = 0_usize;
    for y in stepped(-200.0, 680.0, 10.0) {
        for x in stepped(-300.0, 940.0, 10.0) {
            for card in cards {
                let clamped = clamp_rect(rect(x, y, card.width, card.height), vp, RULE);
                assert_reachable(clamped, vp, "640x480 sweep");
                checked += 1;
            }
        }
    }
    assert!(checked > 30_000, "640x480 sweep must be fine, {checked}");
}

#[test]
fn the_invariant_holds_for_a_viewport_that_does_not_start_at_the_origin() {
    // UIF-WIN-002 allows a content area offset from (0, 0); the bounds
    // are relative to the viewport rectangle, not to the world origin.
    let vp = rect(100.0, 50.0, 1000.0, 700.0);
    for card in SWEEP_CARDS {
        for y in stepped(-200.0, 1000.0, 40.0) {
            for x in stepped(-800.0, 1600.0, 40.0) {
                let clamped = clamp_rect(rect(x, y, card.width, card.height), vp, RULE);
                assert_reachable(clamped, vp, "offset viewport");
            }
        }
    }

    // The exact bounds in the offset frame: x in [100 + 120 - 680,
    // 100 + 1000 - 120] = [-460, 980], y in [50 + 36, 50 + 700 - 32]
    // = [86, 718].
    let card = size(680.0, 440.0);
    assert_eq!(clamp_position(pt(0.0, 0.0), card, vp, RULE), pt(0.0, 86.0));
    assert_eq!(
        clamp_position(pt(-1.0e6, -1.0e6), card, vp, RULE),
        pt(-460.0, 86.0)
    );
    assert_eq!(
        clamp_position(pt(1.0e6, 1.0e6), card, vp, RULE),
        pt(980.0, 718.0)
    );
}

#[test]
fn resizing_keeps_the_title_bar_reachable_across_every_combination() {
    let mut checked = 0_usize;
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            // Start from a reachable card: the invariant is about never
            // *losing* the title bar, not about rescuing one already lost.
            let start = clamp_rect(
                rect(vp.width / 4.0, vp.height / 4.0, card.width, card.height),
                vp,
                RULE,
            );
            assert_reachable(start, vp, "resize sweep start");

            for handle in ResizeHandle::ALL {
                for delta in RESIZE_DELTAS {
                    let result = resize(start, handle, delta, vp, RULE);
                    assert_reachable(result, vp, "resize sweep");
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 3_000,
        "resize sweep must be substantial, {checked}"
    );
}

#[test]
fn a_card_that_is_already_reachable_is_never_moved_by_the_clamp() {
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            let clamped = clamp_rect(
                rect(vp.width / 3.0, vp.height / 3.0, card.width, card.height),
                vp,
                RULE,
            );
            if is_reachable(clamped, vp, RULE) {
                assert_eq!(
                    clamp_rect(clamped, vp, RULE),
                    clamped,
                    "a reachable card must be a fixed point of the clamp"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// Degenerate inputs
// ---------------------------------------------------------------------

#[test]
fn a_zero_size_card_is_clamped_into_the_viewport() {
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_position(pt(1500.0, 1500.0), Size::ZERO, vp, RULE);

    // A zero-width card requires zero visible title bar, so the legal
    // range is the whole viewport and the card lands on its right edge.
    assert_eq!(clamped, pt(1000.0, 668.0));
    assert_reachable(Rectangle::new(clamped, Size::ZERO), vp, "zero-size card");
    assert_eq!(
        clamp_position(pt(-5.0, -5.0), Size::ZERO, vp, RULE),
        pt(0.0, 36.0)
    );
}

#[test]
fn a_zero_viewport_is_defined() {
    let vp = viewport(0.0, 0.0);
    let clamped = clamp_position(pt(500.0, 500.0), size(680.0, 440.0), vp, RULE);

    // Nothing is visible anywhere, so the requirement degrades to zero
    // and the card lands on the only x the viewport has. Vertically the
    // interval is empty, so the navbar bound wins.
    assert_eq!(clamped, pt(0.0, NAVBAR));
    assert_reachable(
        Rectangle::new(clamped, size(680.0, 440.0)),
        vp,
        "zero viewport",
    );
}

#[test]
fn negative_extents_are_treated_as_zero() {
    let vp = viewport(1000.0, 700.0);
    let negative = clamp_rect(rect(500.0, 500.0, -100.0, -50.0), vp, RULE);

    assert_eq!(negative.width, 0.0);
    assert_eq!(negative.height, 0.0);
    // A zero-extent card at (500, 500) is already legal, so only the
    // extents are rewritten - the origin is left alone.
    assert_eq!(negative.x, 500.0);
    assert_eq!(negative.y, 500.0);
    assert!(negative.x.is_finite() && negative.y.is_finite());

    let pushed = clamp_rect(rect(5000.0, 5000.0, -100.0, -50.0), vp, RULE);
    assert_eq!(pushed.x, 1000.0);
    assert_eq!(pushed.y, 668.0);
}

#[test]
fn a_card_far_outside_the_viewport_is_pulled_back_to_the_nearest_legal_position() {
    let vp = viewport(1000.0, 700.0);
    let card = size(680.0, 440.0);

    assert_eq!(
        clamp_position(pt(1.0e6, 1.0e6), card, vp, RULE),
        pt(880.0, 668.0)
    );
    assert_eq!(
        clamp_position(pt(-1.0e6, -1.0e6), card, vp, RULE),
        pt(-560.0, 36.0)
    );
    assert_eq!(
        clamp_position(pt(1.0e6, -1.0e6), card, vp, RULE),
        pt(880.0, 36.0)
    );
    assert_eq!(
        clamp_position(pt(-1.0e6, 1.0e6), card, vp, RULE),
        pt(-560.0, 668.0)
    );
}

#[test]
fn a_viewport_smaller_than_the_minimum_card_size_still_keeps_the_title_reachable() {
    let vp = viewport(300.0, 200.0);
    let clamped = clamp_size(rect(0.0, 0.0, 10.0, 10.0), vp, RULE);

    assert_eq!(clamped.size(), CARD_MIN);
    assert!(
        clamped.width > vp.width,
        "the card is wider than the window"
    );
    assert_reachable(clamped, vp, "300x200 viewport");
    assert!(visible_title_width(clamped, vp) >= MIN_VISIBLE);
}

#[test]
fn a_viewport_smaller_than_the_visible_title_requirement_still_keeps_the_title_reachable() {
    let vp = viewport(80.0, 200.0);
    let clamped = clamp_rect(rect(500.0, 500.0, 420.0, 260.0), vp, RULE);

    // 80px of viewport cannot show 120px of anything: the requirement
    // degrades to the viewport width, and the card covers all of it.
    assert_eq!(clamped.x, 0.0);
    assert_eq!(visible_title_width(clamped, vp), 80.0);
    assert_reachable(clamped, vp, "80px-wide viewport");
}

#[test]
fn a_non_finite_maximum_size_is_treated_as_unbounded() {
    let rule = ClampRule {
        size_policy: SizePolicy::bounded(CARD_MIN, size(f32::INFINITY, f32::NAN)),
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let clamped = clamp_size(rect(100.0, 100.0, 5000.0, 5000.0), vp, rule);

    assert_eq!(clamped.width, 5000.0);
    assert_eq!(clamped.height, 5000.0);
}

#[test]
fn a_maximum_below_the_minimum_raises_to_the_minimum() {
    let rule = ClampRule {
        size_policy: SizePolicy::bounded(CARD_MIN, size(100.0, 50.0)),
        ..RULE
    };
    let vp = viewport(1000.0, 700.0);
    let grown = clamp_size(rect(100.0, 100.0, 2000.0, 2000.0), vp, rule);
    assert_eq!(grown.size(), CARD_MIN);

    let shrunk = clamp_size(rect(100.0, 100.0, 10.0, 10.0), vp, rule);
    assert_eq!(shrunk.size(), CARD_MIN);
}

#[test]
fn resizing_a_zero_size_card_stops_at_the_policy_minimum() {
    let vp = viewport(1000.0, 700.0);
    let result = resize(
        rect(100.0, 100.0, 0.0, 0.0),
        ResizeHandle::BottomRight,
        vec(10.0, 10.0),
        vp,
        RULE,
    );

    assert_eq!(result.size(), CARD_MIN);
    assert_reachable(result, vp, "resized from nothing");
}

#[test]
fn resizing_past_the_held_edge_does_not_turn_the_card_inside_out() {
    let vp = viewport(1000.0, 700.0);
    let start = rect(200.0, 100.0, 680.0, 440.0);
    let result = resize(start, ResizeHandle::Left, vec(5000.0, 0.0), vp, RULE);

    assert!(result.width >= CARD_MIN.width);
    assert_eq!(result.x + result.width, start.x + start.width);
    assert_reachable(result, vp, "dragged across the held edge");
}

#[test]
fn non_finite_inputs_never_produce_nan() {
    let nasty = [
        (pt(0.0, 0.0), size(680.0, 440.0), viewport(1000.0, 700.0)),
        (
            pt(f32::NAN, f32::NAN),
            size(f32::NAN, f32::NAN),
            viewport(1000.0, 700.0),
        ),
        (
            pt(f32::INFINITY, f32::NEG_INFINITY),
            size(f32::INFINITY, f32::INFINITY),
            viewport(f32::NAN, f32::NAN),
        ),
        (
            pt(100.0, 100.0),
            size(680.0, 440.0),
            Rectangle::new(pt(f32::INFINITY, f32::INFINITY), size(f32::NAN, 700.0)),
        ),
    ];

    for (position, card, vp) in nasty {
        let clamped = clamp_position(position, card, vp, RULE);
        assert!(
            clamped.x.is_finite() && clamped.y.is_finite(),
            "{clamped:?}"
        );

        let rect_clamped = clamp_rect(Rectangle::new(position, card), vp, RULE);
        assert!(
            rect_clamped.x.is_finite()
                && rect_clamped.y.is_finite()
                && rect_clamped.width.is_finite()
                && rect_clamped.height.is_finite(),
            "{rect_clamped:?}"
        );

        let sized = clamp_size(Rectangle::new(position, card), vp, RULE);
        assert!(
            sized.x.is_finite()
                && sized.y.is_finite()
                && sized.width.is_finite()
                && sized.height.is_finite(),
            "{sized:?}"
        );

        let dragged = drag_position(position, vec(f32::NAN, f32::INFINITY), card, vp, RULE);
        assert!(
            dragged.x.is_finite() && dragged.y.is_finite(),
            "{dragged:?}"
        );

        for handle in ResizeHandle::ALL {
            let resized = resize(
                Rectangle::new(position, card),
                handle,
                vec(f32::NEG_INFINITY, f32::NAN),
                vp,
                RULE,
            );
            assert!(
                resized.x.is_finite()
                    && resized.y.is_finite()
                    && resized.width.is_finite()
                    && resized.height.is_finite(),
                "{handle:?} produced {resized:?}"
            );
        }

        // A predicate must answer, not propagate.
        let _ = is_reachable(Rectangle::new(position, card), vp, RULE);
        let stretched = Rectangle::new(position, size(vp.width, vp.height));
        assert!(visible_title_width(stretched, vp).is_finite());
        assert!(
            title_bar_rect(Rectangle::new(position, card), RULE)
                .x
                .is_finite()
        );
    }
}

// ---------------------------------------------------------------------
// Purity
// ---------------------------------------------------------------------

#[test]
fn every_solver_is_pure() {
    let vp = viewport(640.0, 480.0);
    for card in SWEEP_CARDS {
        for y in stepped(-200.0, 700.0, 40.0) {
            for x in stepped(-800.0, 900.0, 40.0) {
                let start = rect(x, y, card.width, card.height);

                assert_eq!(
                    clamp_rect(start, vp, RULE),
                    clamp_rect(start, vp, RULE),
                    "clamp_rect must be pure"
                );
                assert_eq!(
                    clamp_position(start.position(), card, vp, RULE),
                    clamp_position(start.position(), card, vp, RULE),
                    "clamp_position must be pure"
                );
                assert_eq!(
                    clamp_size(start, vp, RULE),
                    clamp_size(start, vp, RULE),
                    "clamp_size must be pure"
                );
                assert_eq!(
                    is_reachable(start, vp, RULE),
                    is_reachable(start, vp, RULE),
                    "is_reachable must be pure"
                );
                assert_eq!(
                    visible_title_width(start, vp),
                    visible_title_width(start, vp),
                    "visible_title_width must be pure"
                );
                for handle in ResizeHandle::ALL {
                    assert_eq!(
                        resize(start, handle, vec(123.0, -45.0), vp, RULE),
                        resize(start, handle, vec(123.0, -45.0), vp, RULE),
                        "resize must be pure"
                    );
                }
            }
        }
    }
}

#[test]
fn the_solver_never_mutates_its_inputs() {
    // Every function takes `Copy` values, so mutation is impossible by
    // construction - but pin it anyway: the caller's rectangle survives
    // the clamp untouched.
    let vp = viewport(1000.0, 700.0);
    let start = rect(5000.0, 5000.0, 680.0, 440.0);
    let result = clamp_rect(start, vp, RULE);

    assert_eq!(start.x, 5000.0);
    assert_eq!(start.y, 5000.0);
    assert_eq!(start.width, 680.0);
    assert_eq!(start.height, 440.0);
    assert_ne!(result.x, start.x);
    assert_ne!(result.y, start.y);
}
