//! Table-driven and sweep tests for the pure placement solver.
//!
//! Every flip boundary is pinned to the exact pixel at which the decision
//! changes, every degenerate input is asserted rather than assumed, and a
//! grid sweep proves the `UIF-VER-002` invariant — *the resolved card is
//! inside the viewport inset whenever the card can be* — holds down to
//! `640 × 480` and below.

use super::{Anchor, ControlAlignment, HorizontalAlign, Placement, VerticalAlign, solve};
use crate::token::space;
use iced::{Point, Rectangle, Size};

/// The `8px` viewport margin the solver reads from `space.sm`.
const MARGIN: f32 = space::SM;
/// The `2px` cursor offset the solver reads from `space.2xs`.
const OFFSET: f32 = space::XXS;
/// The `4px` trigger gap the solver reads from `space.xs`.
const GAP: f32 = space::XS;

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn size(width: f32, height: f32) -> Size {
    Size::new(width, height)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle::new(pt(x, y), size(width, height))
}

/// The canonical root-space viewport: origin `(0, 0)`, `UIF-WIN-002`.
fn viewport(width: f32, height: f32) -> Rectangle {
    Rectangle::with_size(size(width, height))
}

/// The inset edges every resolved card is measured against.
fn inset(viewport: Rectangle) -> (f32, f32, f32, f32) {
    (
        viewport.x + MARGIN,
        viewport.y + MARGIN,
        viewport.x + viewport.width - MARGIN,
        viewport.y + viewport.height - MARGIN,
    )
}

/// The core `UIF-VER-002` invariant, asserted across every sweep.
///
/// * the origin never sits inside the margin band — it is always at least
///   `8px` from the near edges;
/// * a card small enough to fit the inset is fully inside it;
/// * a card too large for the inset is pinned to the near edge rather
///   than pushed off-screen, and says so;
/// * `scrollable` is exactly "the bottom edge overflows the inset";
/// * nothing is ever `NaN`.
fn assert_inset(placement: &Placement, viewport: Rectangle, label: &str) {
    let (near_x, near_y, far_x, far_y) = inset(viewport);
    let width = placement.size.width;
    let height = placement.size.height;

    assert!(
        placement.origin.x.is_finite() && placement.origin.y.is_finite(),
        "{label}: non-finite origin {:?}",
        placement.origin
    );
    assert!(
        placement.origin.x >= near_x,
        "{label}: origin.x {} is inside the {}px left margin (near = {near_x})",
        placement.origin.x,
        MARGIN
    );
    assert!(
        placement.origin.y >= near_y,
        "{label}: origin.y {} is inside the {}px top margin (near = {near_y})",
        placement.origin.y,
        MARGIN
    );

    if width <= far_x - near_x {
        assert!(
            placement.origin.x + width <= far_x,
            "{label}: card of width {width} overruns the right inset: origin.x {} + width > far {far_x}",
            placement.origin.x
        );
    } else {
        assert!(
            placement.origin.x == near_x,
            "{label}: oversized card must pin to the left inset (near = {near_x}), got {}",
            placement.origin.x
        );
    }

    if height <= far_y - near_y {
        assert!(
            placement.origin.y + height <= far_y,
            "{label}: card of height {height} overruns the bottom inset: origin.y {} + height > far {far_y}",
            placement.origin.y
        );
    } else {
        assert!(
            placement.origin.y == near_y,
            "{label}: oversized card must pin to the top inset (near = {near_y}), got {}",
            placement.origin.y
        );
    }

    assert_eq!(
        placement.scrollable,
        placement.origin.y + height > far_y,
        "{label}: scrollable must report exactly whether the bottom edge overflows the inset"
    );

    if placement.scrollable {
        assert!(
            placement.clamped_vertical,
            "{label}: a truncated card must report the vertical axis as clamped"
        );
    }
}

// ---------------------------------------------------------------------
// Pointer anchor — the four flip combinations
// ---------------------------------------------------------------------

#[test]
fn pointer_anchor_opens_2px_right_and_below_the_cursor() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(102.0, 102.0));
    assert!(!placement.flipped_horizontal);
    assert!(!placement.flipped_vertical);
    assert!(!placement.clamped_horizontal);
    assert!(!placement.clamped_vertical);
    assert!(!placement.scrollable);
}

#[test]
fn pointer_anchor_reads_its_offset_from_the_space_2xs_token() {
    let point = pt(100.0, 100.0);
    let card = size(200.0, 150.0);
    let placement = solve(Anchor::Pointer { point }, card, viewport(1000.0, 700.0));
    assert_eq!(placement.origin.x, point.x + space::XXS);
    assert_eq!(placement.origin.y, point.y + space::XXS);

    let flipped = solve(
        Anchor::Pointer {
            point: pt(900.0, 100.0),
        },
        card,
        viewport(1000.0, 700.0),
    );
    assert_eq!(flipped.origin.x, 900.0 - card.width - space::XXS);
    assert_eq!(flipped.origin.y, 100.0 + space::XXS);
}

#[test]
fn pointer_anchor_flips_horizontally_only() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(900.0, 100.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(698.0, 102.0));
    assert!(placement.flipped_horizontal);
    assert!(!placement.flipped_vertical);
    assert!(!placement.was_clamped());
}

#[test]
fn pointer_anchor_flips_vertically_only() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 600.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(102.0, 448.0));
    assert!(!placement.flipped_horizontal);
    assert!(placement.flipped_vertical);
    assert!(!placement.was_clamped());
}

#[test]
fn pointer_anchor_flips_both_axes_at_once() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(900.0, 600.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(698.0, 448.0));
    assert!(placement.flipped_horizontal);
    assert!(placement.flipped_vertical);
    assert!(!placement.was_clamped());
}

#[test]
fn pointer_flip_is_per_axis_and_independent() {
    // Widening only the horizontal overflow must not disturb the vertical
    // decision, and vice versa.
    let wide_only = solve(
        Anchor::Pointer {
            point: pt(900.0, 100.0),
        },
        size(400.0, 100.0),
        viewport(1000.0, 700.0),
    );
    assert!(wide_only.flipped_horizontal);
    assert!(!wide_only.flipped_vertical);

    let tall_only = solve(
        Anchor::Pointer {
            point: pt(100.0, 650.0),
        },
        size(100.0, 400.0),
        viewport(1000.0, 700.0),
    );
    assert!(!tall_only.flipped_horizontal);
    assert!(tall_only.flipped_vertical);
}

// ---------------------------------------------------------------------
// Pointer anchor — the flip boundary itself
// ---------------------------------------------------------------------

#[test]
fn pointer_horizontal_flip_boundary_is_the_exact_pixel() {
    // viewport 1000, margin 8, offset 2, card 200 → the last x that does
    // not flip is 1000 - 8 - 2 - 200 = 790.
    let card = size(200.0, 150.0);
    let vp = viewport(1000.0, 700.0);

    let last_standing = solve(
        Anchor::Pointer {
            point: pt(790.0, 100.0),
        },
        card,
        vp,
    );
    assert_eq!(last_standing.origin, pt(792.0, 102.0));
    assert!(!last_standing.flipped_horizontal);

    let first_flipped = solve(
        Anchor::Pointer {
            point: pt(791.0, 100.0),
        },
        card,
        vp,
    );
    assert!(first_flipped.flipped_horizontal);
    assert_eq!(first_flipped.origin.x, 589.0);
}

#[test]
fn pointer_vertical_flip_boundary_is_the_exact_pixel() {
    // viewport 700, margin 8, offset 2, card 150 → the last y that does
    // not flip is 700 - 8 - 2 - 150 = 540.
    let card = size(200.0, 150.0);
    let vp = viewport(1000.0, 700.0);

    let last_standing = solve(
        Anchor::Pointer {
            point: pt(100.0, 540.0),
        },
        card,
        vp,
    );
    assert_eq!(last_standing.origin, pt(102.0, 542.0));
    assert!(!last_standing.flipped_vertical);

    let first_flipped = solve(
        Anchor::Pointer {
            point: pt(100.0, 541.0),
        },
        card,
        vp,
    );
    assert!(first_flipped.flipped_vertical);
    assert_eq!(first_flipped.origin.y, 389.0);
}

#[test]
fn pointer_flip_threshold_formula_holds_across_viewport_sizes() {
    let cases = [
        (1000.0, 700.0, 200.0, 150.0),
        (640.0, 480.0, 200.0, 150.0),
        (800.0, 600.0, 320.0, 200.0),
        (1440.0, 900.0, 260.0, 420.0),
    ];

    for (vw, vh, cw, ch) in cases {
        let vp = viewport(vw, vh);
        let card = size(cw, ch);
        let last_x = vw - MARGIN - OFFSET - cw;
        let last_y = vh - MARGIN - OFFSET - ch;

        let inside = solve(
            Anchor::Pointer {
                point: pt(last_x, last_y),
            },
            card,
            vp,
        );
        assert_eq!(
            inside.origin,
            pt(last_x + OFFSET, last_y + OFFSET),
            "{vw}x{vh} / {cw}x{ch}: last non-flipping pixel must keep the offset"
        );
        assert!(!inside.flipped_horizontal);
        assert!(!inside.flipped_vertical);

        let over_x = solve(
            Anchor::Pointer {
                point: pt(last_x + 1.0, last_y),
            },
            card,
            vp,
        );
        assert!(
            over_x.flipped_horizontal,
            "{vw}x{vh} / {cw}x{ch}: one pixel past the threshold must flip horizontally"
        );
        assert_eq!(over_x.origin.x, last_x + 1.0 - cw - OFFSET);
        assert!(
            !over_x.flipped_vertical,
            "{vw}x{vh} / {cw}x{ch}: the horizontal overflow must not flip the vertical axis"
        );

        let over_y = solve(
            Anchor::Pointer {
                point: pt(last_x, last_y + 1.0),
            },
            card,
            vp,
        );
        assert!(
            over_y.flipped_vertical,
            "{vw}x{vh} / {cw}x{ch}: one pixel past the threshold must flip vertically"
        );
        assert_eq!(over_y.origin.y, last_y + 1.0 - ch - OFFSET);
        assert!(
            !over_y.flipped_horizontal,
            "{vw}x{vh} / {cw}x{ch}: the vertical overflow must not flip the horizontal axis"
        );
    }
}

// ---------------------------------------------------------------------
// Pointer anchor — clamping and the scrollable fallback
// ---------------------------------------------------------------------

#[test]
fn pointer_inside_the_near_margin_is_clamped_to_it() {
    // The spec's flip rule only looks at the far edge, so a cursor at the
    // origin does not flip — the 8px margin clamp is what moves it.
    let placement = solve(
        Anchor::Pointer {
            point: pt(0.0, 0.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(MARGIN, MARGIN));
    assert!(!placement.flipped_horizontal);
    assert!(!placement.flipped_vertical);
    assert!(placement.clamped_horizontal);
    assert!(placement.clamped_vertical);
    assert!(!placement.scrollable);
}

#[test]
fn pointer_outside_the_viewport_is_pulled_back_inside() {
    let card = size(200.0, 150.0);
    let vp = viewport(1000.0, 700.0);

    let top_left = solve(
        Anchor::Pointer {
            point: pt(-500.0, -500.0),
        },
        card,
        vp,
    );
    assert_eq!(top_left.origin, pt(8.0, 8.0));
    assert!(!top_left.was_flipped());
    assert!(top_left.was_clamped());

    let bottom_right = solve(
        Anchor::Pointer {
            point: pt(5000.0, 5000.0),
        },
        card,
        vp,
    );
    assert_eq!(bottom_right.origin, pt(792.0, 542.0));
    assert!(bottom_right.flipped_horizontal);
    assert!(bottom_right.flipped_vertical);
    assert!(bottom_right.was_clamped());
    assert_inset(&bottom_right, vp, "pointer far off the bottom-right");
}

#[test]
fn fallback_chooses_the_roomier_side_when_neither_side_fits() {
    // 640×480: inset is 624×464. A 600×400 card fits neither side of a
    // cursor at (100,100) — 102+600 > 632 and 100-602 < 8 — but there is
    // far more room below/right than above/left.
    let card = size(600.0, 400.0);
    let vp = viewport(640.0, 480.0);

    let roomier_below = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        card,
        vp,
    );
    assert!(!roomier_below.flipped_horizontal);
    assert!(!roomier_below.flipped_vertical);
    assert!(roomier_below.was_clamped());
    // Pulled back to the far inset: 632 - 600 = 32, 472 - 400 = 72.
    assert_eq!(roomier_below.origin, pt(32.0, 72.0));

    let roomier_above = solve(
        Anchor::Pointer {
            point: pt(500.0, 400.0),
        },
        card,
        vp,
    );
    assert!(roomier_above.flipped_horizontal);
    assert!(roomier_above.flipped_vertical);
    assert_eq!(roomier_above.origin, pt(8.0, 8.0));
    assert_inset(&roomier_above, vp, "fallback roomier above");
}

#[test]
fn fallback_ties_keep_the_preferred_side() {
    // A cursor dead centre of the 640×480 inset leaves both sides exactly
    // equally roomy; the spec's "Right and Below" preference breaks the tie.
    let placement = solve(
        Anchor::Pointer {
            point: pt(320.0, 240.0),
        },
        size(500.0, 400.0),
        viewport(640.0, 480.0),
    );

    assert!(
        !placement.flipped_horizontal,
        "an exact tie must keep the preferred (non-flipped) side"
    );
    assert!(
        !placement.flipped_vertical,
        "an exact tie must keep the preferred (non-flipped) side"
    );
    assert!(placement.was_clamped());
    assert_eq!(placement.origin, pt(132.0, 72.0));
}

#[test]
fn fallback_engages_without_scrolling_when_the_card_still_fits() {
    // Neither side of the cursor can hold a 400×300 card in 640×480, but
    // the clamp brings the whole card inside, so there is nothing to scroll.
    let placement = solve(
        Anchor::Pointer {
            point: pt(320.0, 240.0),
        },
        size(400.0, 300.0),
        viewport(640.0, 480.0),
    );

    assert!(placement.was_clamped());
    assert!(
        !placement.scrollable,
        "a card the clamp pulled fully inside must not claim to need scrolling"
    );
    assert_eq!(placement.origin, pt(232.0, 172.0));
    assert_eq!(placement.visible_height(viewport(640.0, 480.0)), 300.0);
}

#[test]
fn fallback_marks_an_oversized_card_scrollable() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(320.0, 240.0),
        },
        size(800.0, 600.0),
        viewport(640.0, 480.0),
    );

    assert_eq!(placement.origin, pt(8.0, 8.0));
    assert!(placement.clamped_horizontal);
    assert!(placement.clamped_vertical);
    assert!(
        placement.scrollable,
        "a card taller than the inset must ask for vertical scroll"
    );
    // The inset height is 480 - 2*8 = 464 of the 600px card.
    assert_eq!(placement.visible_height(viewport(640.0, 480.0)), 464.0);
}

#[test]
fn fallback_never_engages_when_the_preferred_side_fits() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert!(!placement.was_clamped());
    assert!(!placement.was_flipped());
    assert!(!placement.scrollable);
}

#[test]
fn a_card_that_fits_the_inset_needs_no_clamp_only_a_flip() {
    // 300×200 inside 640×480: every anchor resolves by flipping, never
    // by clamping, because the inset can always hold the card.
    let card = size(300.0, 200.0);
    let vp = viewport(640.0, 480.0);

    for x in [0.0, 160.0, 320.0, 480.0, 640.0] {
        for y in [0.0, 120.0, 240.0, 360.0, 480.0] {
            let placement = solve(Anchor::Pointer { point: pt(x, y) }, card, vp);
            assert_inset(&placement, vp, &format!("300x200 at {x},{y}"));
        }
    }
}

// ---------------------------------------------------------------------
// Control anchor — fixed menus and the submenu cascade
// ---------------------------------------------------------------------

#[test]
fn control_anchor_defaults_to_below_the_trigger_with_a_4px_gap() {
    let trigger = rect(100.0, 36.0, 120.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_start(),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(100.0, 68.0));
    assert_eq!(
        placement.origin.y - (trigger.y + trigger.height),
        GAP,
        "the trigger gap must be the space.xs token"
    );
    assert_eq!(placement.origin.x, trigger.x, "left edges stay level");
    assert!(!placement.was_flipped());
    assert!(!placement.was_clamped());
}

#[test]
fn control_anchor_aligns_its_right_edge_for_a_navbar_trigger() {
    let trigger = rect(960.0, 4.0, 28.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_end(),
        },
        size(220.0, 300.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(768.0, 36.0));
    assert_eq!(
        placement.origin.x + placement.size.width,
        trigger.x + trigger.width,
        "the card's right edge must be level with the trigger's right edge"
    );
    assert!(!placement.was_flipped());
}

#[test]
fn control_anchor_right_edge_alignment_flips_to_start_near_the_left_edge() {
    let trigger = rect(20.0, 4.0, 28.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_end(),
        },
        size(220.0, 300.0),
        viewport(1000.0, 700.0),
    );

    assert!(
        placement.flipped_horizontal,
        "a right-edge-aligned card that would hang off the left edge must flip to left-edge alignment"
    );
    assert!(!placement.flipped_vertical);
    assert_eq!(placement.origin, pt(20.0, 36.0));
    assert_eq!(placement.origin.x, trigger.x);
}

#[test]
fn control_anchor_above_keeps_its_gap_when_it_fits() {
    let trigger = rect(100.0, 300.0, 120.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::new(VerticalAlign::Above, HorizontalAlign::Start),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(100.0, 146.0));
    assert_eq!(
        trigger.y - (placement.origin.y + placement.size.height),
        GAP,
        "the gap above the trigger must be the same space.xs token"
    );
    assert!(!placement.was_flipped());
}

#[test]
fn control_anchor_above_flips_to_below_when_there_is_no_room() {
    let trigger = rect(100.0, 20.0, 120.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::new(VerticalAlign::Above, HorizontalAlign::Start),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert!(placement.flipped_vertical);
    assert!(!placement.flipped_horizontal);
    assert_eq!(placement.origin, pt(100.0, 52.0));
    assert_eq!(placement.origin.y, trigger.y + trigger.height + GAP);
}

#[test]
fn submenu_opens_beside_its_parent_aligned_to_the_row_top() {
    let parent_row = rect(100.0, 200.0, 220.0, 32.0);
    let placement = solve(
        Anchor::Control {
            rect: parent_row,
            alignment: ControlAlignment::submenu(),
        },
        size(200.0, 250.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(322.0, 200.0));
    assert_eq!(
        placement.origin.y, parent_row.y,
        "the submenu must be aligned to the parent row's top edge"
    );
    assert_eq!(
        placement.origin.x - (parent_row.x + parent_row.width),
        OFFSET,
        "the submenu must sit one space.2xs gap clear of the parent card"
    );
    assert!(!placement.was_flipped());
}

#[test]
fn submenu_opens_to_the_left_near_the_right_viewport_edge() {
    let parent_row = rect(760.0, 200.0, 220.0, 32.0);
    let placement = solve(
        Anchor::Control {
            rect: parent_row,
            alignment: ControlAlignment::submenu(),
        },
        size(200.0, 250.0),
        viewport(1000.0, 700.0),
    );

    assert!(
        placement.flipped_horizontal,
        "near the right edge the submenu must open to the left of its parent"
    );
    assert!(!placement.flipped_vertical);
    assert_eq!(placement.origin, pt(558.0, 200.0));
    assert_eq!(
        placement.origin.x + placement.size.width + OFFSET,
        parent_row.x,
        "the submenu's right edge must sit one space.2xs clear of the parent's left edge"
    );
    assert_eq!(placement.origin.y, parent_row.y);
}

#[test]
fn submenu_top_alignment_flips_to_bottom_when_it_would_overrun() {
    let parent_row = rect(100.0, 600.0, 220.0, 32.0);
    let placement = solve(
        Anchor::Control {
            rect: parent_row,
            alignment: ControlAlignment::submenu(),
        },
        size(200.0, 250.0),
        viewport(1000.0, 700.0),
    );

    assert!(!placement.flipped_horizontal);
    assert!(
        placement.flipped_vertical,
        "a submenu that cannot hang below the parent row's top must align to the bottom instead"
    );
    assert_eq!(placement.origin, pt(322.0, 382.0));
    assert_eq!(
        placement.origin.y + placement.size.height,
        parent_row.y + parent_row.height,
        "the flipped submenu's bottom edge is level with the parent row's bottom edge"
    );
}

#[test]
fn centered_control_alignment_clamps_instead_of_flipping() {
    let trigger = rect(900.0, 300.0, 100.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::new(VerticalAlign::Middle, HorizontalAlign::Center),
        },
        size(300.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert!(
        !placement.flipped_horizontal,
        "a centred card has no mirrored alternative, so it clamps rather than flips"
    );
    assert!(placement.clamped_horizontal);
    assert_eq!(placement.origin, pt(692.0, 239.0));
    assert_inset(
        &placement,
        viewport(1000.0, 700.0),
        "centred control anchor",
    );
}

#[test]
fn control_anchor_on_a_zero_size_trigger_is_defined() {
    let trigger = rect(500.0, 300.0, 0.0, 0.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_start(),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(500.0, 304.0));
    assert!(!placement.was_flipped());
    assert!(!placement.was_clamped());
}

#[test]
fn control_anchor_below_a_trigger_at_the_bottom_flips_above() {
    let trigger = rect(100.0, 660.0, 120.0, 28.0);
    let placement = solve(
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_start(),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );

    assert!(placement.flipped_vertical);
    assert_eq!(placement.origin, pt(100.0, 506.0));
    assert_eq!(
        trigger.y - (placement.origin.y + placement.size.height),
        GAP,
        "the flipped card keeps the same space.xs gap on the other side"
    );
}

// ---------------------------------------------------------------------
// Cursor-proximal anchor — input dialogs
// ---------------------------------------------------------------------

#[test]
fn input_dialog_opens_2px_from_the_pointer() {
    let placement = solve(
        Anchor::CursorProximal {
            point: pt(200.0, 200.0),
            keyboard_invoked: false,
        },
        size(320.0, 200.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(202.0, 202.0));
    assert!(!placement.was_flipped());
    assert!(!placement.was_clamped());
    assert!(!placement.scrollable);
}

#[test]
fn input_dialog_clamps_to_the_margin_instead_of_flipping() {
    let placement = solve(
        Anchor::CursorProximal {
            point: pt(900.0, 650.0),
            keyboard_invoked: false,
        },
        size(320.0, 200.0),
        viewport(1000.0, 700.0),
    );

    assert!(
        !placement.was_flipped(),
        "an input dialog never flips: UIF-INP-001 clamps it"
    );
    assert!(placement.clamped_horizontal);
    assert!(placement.clamped_vertical);
    assert_eq!(placement.origin, pt(672.0, 492.0));
    assert_inset(&placement, viewport(1000.0, 700.0), "input dialog clamped");
}

#[test]
fn keyboard_invoked_input_dialog_centres_in_the_viewport() {
    let card = size(320.0, 200.0);
    let vp = viewport(1000.0, 700.0);

    for point in [
        pt(0.0, 0.0),
        pt(999.0, 699.0),
        pt(500.0, 100.0),
        pt(-50.0, 900.0),
    ] {
        let placement = solve(
            Anchor::CursorProximal {
                point,
                keyboard_invoked: true,
            },
            card,
            vp,
        );
        assert_eq!(
            placement.origin,
            pt(340.0, 250.0),
            "a keyboard-invoked dialog must centre regardless of the point {point:?}"
        );
        assert!(!placement.was_flipped());
        assert!(!placement.was_clamped());
    }
}

#[test]
fn keyboard_invoked_centring_in_a_640x480_viewport() {
    let placement = solve(
        Anchor::CursorProximal {
            point: pt(10.0, 10.0),
            keyboard_invoked: true,
        },
        size(320.0, 200.0),
        viewport(640.0, 480.0),
    );

    assert_eq!(placement.origin, pt(160.0, 140.0));
    assert!(!placement.was_clamped());
    assert!(!placement.scrollable);
}

#[test]
fn keyboard_invoked_centring_still_respects_the_margin() {
    let placement = solve(
        Anchor::CursorProximal {
            point: pt(500.0, 350.0),
            keyboard_invoked: true,
        },
        size(1200.0, 900.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.origin, pt(8.0, 8.0));
    assert!(placement.was_clamped());
    assert!(placement.scrollable);
    assert_inset(
        &placement,
        viewport(1000.0, 700.0),
        "oversized centred dialog",
    );
}

#[test]
fn pointer_invoked_dialog_in_a_small_viewport_clamps_to_the_margins() {
    let placement = solve(
        Anchor::CursorProximal {
            point: pt(600.0, 450.0),
            keyboard_invoked: false,
        },
        size(320.0, 200.0),
        viewport(640.0, 480.0),
    );

    assert_eq!(placement.origin, pt(312.0, 272.0));
    assert!(!placement.was_flipped());
    assert!(placement.was_clamped());
    assert_inset(
        &placement,
        viewport(640.0, 480.0),
        "input dialog in 640x480",
    );
}

#[test]
fn keyboard_invoked_and_pointer_invoked_are_genuinely_different_branches() {
    let point = pt(900.0, 650.0);
    let card = size(320.0, 200.0);
    let vp = viewport(1000.0, 700.0);

    let pointer = solve(
        Anchor::CursorProximal {
            point,
            keyboard_invoked: false,
        },
        card,
        vp,
    );
    let keyboard = solve(
        Anchor::CursorProximal {
            point,
            keyboard_invoked: true,
        },
        card,
        vp,
    );

    assert_ne!(
        pointer.origin, keyboard.origin,
        "the keyboard branch must not follow the cursor"
    );
    assert_eq!(keyboard.origin, pt(340.0, 250.0));
    assert_eq!(pointer.origin, pt(672.0, 492.0));
}

// ---------------------------------------------------------------------
// Degenerate inputs
// ---------------------------------------------------------------------

#[test]
fn zero_size_card_resolves_to_the_preferred_origin() {
    let pointer = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        Size::ZERO,
        viewport(1000.0, 700.0),
    );
    assert_eq!(pointer.origin, pt(102.0, 102.0));
    assert_eq!(pointer.size, Size::ZERO);
    assert!(!pointer.was_flipped());
    assert!(!pointer.was_clamped());
    assert!(!pointer.scrollable);

    let control = solve(
        Anchor::Control {
            rect: rect(100.0, 36.0, 120.0, 28.0),
            alignment: ControlAlignment::below_start(),
        },
        Size::ZERO,
        viewport(1000.0, 700.0),
    );
    assert_eq!(control.origin, pt(100.0, 68.0));
    assert!(!control.was_clamped());

    let keyboard = solve(
        Anchor::CursorProximal {
            point: pt(10.0, 10.0),
            keyboard_invoked: true,
        },
        Size::ZERO,
        viewport(1000.0, 700.0),
    );
    assert_eq!(keyboard.origin, pt(500.0, 350.0));
    assert!(!keyboard.scrollable);
}

#[test]
fn a_card_larger_than_the_viewport_is_pinned_to_the_near_margin() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(300.0, 200.0),
        },
        size(2000.0, 1500.0),
        viewport(640.0, 480.0),
    );

    assert_eq!(placement.origin, pt(8.0, 8.0));
    assert!(placement.was_clamped());
    assert!(placement.scrollable);
    assert_eq!(placement.visible_height(viewport(640.0, 480.0)), 464.0);
    assert_inset(&placement, viewport(640.0, 480.0), "2000x1500 in 640x480");
}

#[test]
fn a_card_exactly_the_size_of_the_inset_fits_exactly() {
    // 640×480 with an 8px margin has a 624×464 inset.
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        size(624.0, 464.0),
        viewport(640.0, 480.0),
    );

    assert_eq!(placement.origin, pt(8.0, 8.0));
    assert!(!placement.scrollable);
    assert_eq!(placement.visible_height(viewport(640.0, 480.0)), 464.0);
}

#[test]
fn viewport_smaller_than_twice_the_margin_is_defined() {
    // A 10px viewport cannot give a card 8px on both sides. The solver
    // pins the card to the near margin and reports it truncated.
    let placement = solve(
        Anchor::Pointer {
            point: pt(5.0, 5.0),
        },
        size(20.0, 20.0),
        viewport(10.0, 10.0),
    );

    assert_eq!(placement.origin, pt(MARGIN, MARGIN));
    assert!(placement.was_clamped());
    assert!(placement.scrollable);
    assert_eq!(placement.visible_height(viewport(10.0, 10.0)), 0.0);
}

#[test]
fn zero_viewport_is_defined() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        size(200.0, 150.0),
        viewport(0.0, 0.0),
    );

    assert_eq!(placement.origin, pt(MARGIN, MARGIN));
    assert!(placement.was_clamped());
    assert!(placement.scrollable);
    assert!(placement.origin.x.is_finite() && placement.origin.y.is_finite());
}

#[test]
fn negative_extents_are_treated_as_zero() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(100.0, 100.0),
        },
        size(-100.0, -50.0),
        viewport(1000.0, 700.0),
    );

    assert_eq!(placement.size, Size::ZERO);
    assert_eq!(placement.origin, pt(102.0, 102.0));
    assert!(!placement.was_clamped());
    assert!(!placement.scrollable);
}

#[test]
fn non_finite_inputs_never_produce_nan() {
    let vp = viewport(640.0, 480.0);
    let anchors = [
        Anchor::Pointer {
            point: pt(f32::NAN, f32::INFINITY),
        },
        Anchor::Pointer {
            point: pt(f32::NEG_INFINITY, 100.0),
        },
        Anchor::Control {
            rect: rect(f32::NAN, 10.0, f32::INFINITY, 28.0),
            alignment: ControlAlignment::below_end(),
        },
        Anchor::CursorProximal {
            point: pt(100.0, f32::NAN),
            keyboard_invoked: true,
        },
    ];
    let cards = [
        size(f32::NAN, 100.0),
        size(100.0, f32::MAX),
        size(f32::MAX, f32::MAX),
    ];

    for anchor in anchors {
        for card in cards {
            let placement = solve(anchor, card, vp);
            assert!(
                placement.origin.x.is_finite()
                    && placement.origin.y.is_finite()
                    && placement.size.width.is_finite()
                    && placement.size.height.is_finite(),
                "non-finite input produced a non-finite placement: {placement:?}"
            );
            assert_inset(&placement, vp, "non-finite input");
        }
    }
}

// ---------------------------------------------------------------------
// Sweeps — the UIF-VER-002 boundary gate
// ---------------------------------------------------------------------

/// Anchor points covering every corner of the viewport, the centre, the
/// margin band, and the area outside the viewport entirely.
fn anchor_points(viewport: Rectangle) -> Vec<Point> {
    let w = viewport.width;
    let h = viewport.height;
    let mut points = vec![
        pt(0.0, 0.0),
        pt(w, h),
        pt(w - 1.0, h - 1.0),
        pt(w / 2.0, h / 2.0),
        pt(-100.0, -100.0),
        pt(w + 100.0, h + 100.0),
        pt(-100.0, h + 100.0),
        pt(w + 100.0, -100.0),
        pt(MARGIN / 2.0, MARGIN / 2.0),
        pt(w - MARGIN / 2.0, h - MARGIN / 2.0),
    ];

    let mut y = -MARGIN;
    while y <= h + MARGIN {
        let mut x = -MARGIN;
        while x <= w + MARGIN {
            points.push(pt(x, y));
            x += 64.0;
        }
        y += 64.0;
    }
    points
}

/// One anchor of every kind, derived from the same point, so a sweep
/// exercises all three placement rules against the same geometry.
fn anchors_for(point: Point) -> Vec<Anchor> {
    let trigger = Rectangle::new(point, size(28.0, 28.0));
    vec![
        Anchor::Pointer { point },
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_start(),
        },
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::below_end(),
        },
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::submenu(),
        },
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::new(VerticalAlign::Above, HorizontalAlign::End),
        },
        Anchor::Control {
            rect: trigger,
            alignment: ControlAlignment::new(VerticalAlign::Middle, HorizontalAlign::Center),
        },
        Anchor::CursorProximal {
            point,
            keyboard_invoked: false,
        },
        Anchor::CursorProximal {
            point,
            keyboard_invoked: true,
        },
    ]
}

const SWEEP_CARDS: [Size; 5] = [
    Size::new(200.0, 150.0),
    Size::new(320.0, 200.0),
    Size::new(624.0, 464.0),
    Size::ZERO,
    Size::new(2000.0, 1500.0),
];

const SWEEP_VIEWPORTS: [Size; 8] = [
    Size::new(1000.0, 700.0),
    Size::new(640.0, 480.0),
    Size::new(480.0, 320.0),
    Size::new(320.0, 240.0),
    Size::new(128.0, 96.0),
    Size::new(16.0, 16.0),
    Size::new(1.0, 1.0),
    Size::ZERO,
];

#[test]
fn the_resolved_card_is_always_inside_the_inset_it_can_be() {
    let mut checked = 0_usize;
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            for point in anchor_points(vp) {
                for anchor in anchors_for(point) {
                    let placement = solve(anchor, card, vp);
                    assert_inset(
                        &placement,
                        vp,
                        &format!("{anchor:?} card {card:?} viewport {vp_size:?}"),
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 1000,
        "sweep must be substantial, checked {checked}"
    );
}

#[test]
fn the_640x480_minimum_viewport_never_loses_a_card() {
    // UIF-VER-002 names 640×480 explicitly: a fine grid across the whole
    // window, every corner, and outside it.
    let vp = viewport(640.0, 480.0);
    let cards = [
        size(200.0, 150.0),
        size(320.0, 200.0),
        size(624.0, 464.0),
        size(700.0, 600.0),
    ];

    let mut y = -50.0;
    while y <= 530.0 {
        let mut x = -50.0;
        while x <= 690.0 {
            for card in cards {
                for anchor in anchors_for(pt(x, y)) {
                    let placement = solve(anchor, card, vp);
                    assert_inset(&placement, vp, &format!("640x480 at {x},{y} card {card:?}"));
                }
            }
            x += 40.0;
        }
        y += 40.0;
    }
}

#[test]
fn a_card_that_fits_the_inset_is_never_reported_scrollable() {
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        let (near_x, near_y, far_x, far_y) = inset(vp);
        for card in SWEEP_CARDS {
            if card.width > far_x - near_x || card.height > far_y - near_y {
                continue;
            }
            for point in anchor_points(vp) {
                for anchor in anchors_for(point) {
                    let placement = solve(anchor, card, vp);
                    assert!(
                        !placement.scrollable,
                        "card {card:?} fits the {vp_size:?} inset and must not be marked scrollable: {placement:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn an_oversized_axis_is_always_pinned_to_the_near_margin() {
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        let (near_x, near_y, far_x, far_y) = inset(vp);
        for card in SWEEP_CARDS {
            let width_oversized = card.width > far_x - near_x;
            let height_oversized = card.height > far_y - near_y;
            if !width_oversized && !height_oversized {
                continue;
            }
            for point in anchor_points(vp) {
                for anchor in anchors_for(point) {
                    let placement = solve(anchor, card, vp);
                    if width_oversized {
                        assert!(
                            placement.clamped_horizontal,
                            "card {card:?} cannot fit the {vp_size:?} inset width and must report the horizontal axis clamped: {placement:?}"
                        );
                        assert_eq!(
                            placement.origin.x, near_x,
                            "an oversized width must pin the leading edge to the left inset: {placement:?}"
                        );
                    }
                    if height_oversized {
                        assert!(
                            placement.clamped_vertical && placement.scrollable,
                            "card {card:?} cannot fit the {vp_size:?} inset height and must report the vertical axis clamped and scrollable: {placement:?}"
                        );
                        assert_eq!(
                            placement.origin.y, near_y,
                            "an oversized height must pin the leading edge to the top inset: {placement:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn an_unclamped_card_sits_exactly_where_its_anchor_rule_put_it() {
    // When the viewport never had to intervene, the card is fully inside
    // the inset with no truncation — the flags and the geometry agree.
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        let (near_x, near_y, far_x, far_y) = inset(vp);
        for card in SWEEP_CARDS {
            for point in anchor_points(vp) {
                for anchor in anchors_for(point) {
                    let placement = solve(anchor, card, vp);
                    if placement.was_clamped() {
                        continue;
                    }
                    assert!(
                        placement.origin.x >= near_x
                            && placement.origin.y >= near_y
                            && placement.origin.x + card.width <= far_x
                            && placement.origin.y + card.height <= far_y,
                        "an unclamped placement must be fully inside the inset: {placement:?}"
                    );
                    assert!(!placement.scrollable);
                }
            }
        }
    }
}

#[test]
fn solve_is_pure() {
    let vp = viewport(640.0, 480.0);
    for point in anchor_points(vp) {
        for card in SWEEP_CARDS {
            for anchor in anchors_for(point) {
                let first = solve(anchor, card, vp);
                let second = solve(anchor, card, vp);
                assert_eq!(
                    first, second,
                    "solve must return the same placement for the same inputs"
                );
            }
        }
    }
}

#[test]
fn placement_rect_matches_the_reported_origin_and_size() {
    let placement = solve(
        Anchor::Pointer {
            point: pt(300.0, 200.0),
        },
        size(200.0, 150.0),
        viewport(1000.0, 700.0),
    );
    let rect = placement.rect();
    assert_eq!(rect.x, placement.origin.x);
    assert_eq!(rect.y, placement.origin.y);
    assert_eq!(rect.width, placement.size.width);
    assert_eq!(rect.height, placement.size.height);
}

#[test]
fn visible_height_never_exceeds_the_card_or_goes_negative() {
    for vp_size in SWEEP_VIEWPORTS {
        let vp = viewport(vp_size.width, vp_size.height);
        for card in SWEEP_CARDS {
            for point in anchor_points(vp) {
                let placement = solve(Anchor::Pointer { point }, card, vp);
                let visible = placement.visible_height(vp);
                assert!(
                    visible >= 0.0 && visible <= placement.size.height,
                    "visible height {visible} outside [0, {}] for {placement:?}",
                    placement.size.height
                );
            }
        }
    }
}
