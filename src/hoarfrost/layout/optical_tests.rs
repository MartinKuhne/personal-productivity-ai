//! Tests for [`super::optical`] — the optical-box / em-box mechanism
//! (`UIF-PRIN-006`).
//!
//! These pin the *mechanism*. They deliberately do not assert any
//! per-glyph measurement, because this crate has no font and therefore no
//! metrics to measure with — see the module docs for that boundary.

use super::*;

// ---------------------------------------------------------------------------
// Slot constants
// ---------------------------------------------------------------------------

#[test]
fn test_small_slot_is_the_icon_md_token() {
    assert_eq!(GLYPH_SLOT_SM, size::ICON_MD);
    assert_eq!(GLYPH_SLOT_SM, 16.0, "§1.1's small optical slot is 16px");
}

#[test]
fn test_large_slot_is_the_hit_target_min_token() {
    assert_eq!(GLYPH_SLOT_MD, size::HIT_TARGET_MIN);
    assert_eq!(GLYPH_SLOT_MD, 24.0, "§1.1's large optical slot is 24px");
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

#[test]
fn test_zero_inset_is_the_identity() {
    let box_rect = optical_box(24.0, 24.0, OpticalInset::ZERO);
    assert_eq!(box_rect.x, 0.0);
    assert_eq!(box_rect.y, 0.0);
    assert_eq!(box_rect.width, 24.0);
    assert_eq!(box_rect.height, 24.0);
}

#[test]
fn test_zero_inset_produces_no_shift() {
    let shift = optical_shift(OpticalInset::ZERO);
    assert_eq!(shift.x, 0.0);
    assert_eq!(shift.y, 0.0);
}

#[test]
fn test_default_inset_is_the_zero_inset() {
    assert_eq!(OpticalInset::default(), OpticalInset::ZERO);
    assert_eq!(
        optical_shift(OpticalInset::default()),
        optical_shift(OpticalInset::ZERO)
    );
}

// ---------------------------------------------------------------------------
// Symmetry
// ---------------------------------------------------------------------------

#[test]
fn test_symmetric_inset_shrinks_the_box_evenly() {
    let inset = OpticalInset::symmetric(2.0, 4.0);
    let box_rect = optical_box(24.0, 24.0, inset);
    assert_eq!(box_rect.x, 2.0);
    assert_eq!(box_rect.width, 20.0);
    assert_eq!(
        24.0 - (box_rect.x + box_rect.width),
        2.0,
        "right gap mirrors left"
    );
    assert_eq!(box_rect.y, 4.0);
    assert_eq!(box_rect.height, 16.0);
    assert_eq!(
        24.0 - (box_rect.y + box_rect.height),
        4.0,
        "bottom gap mirrors top"
    );
}

#[test]
fn test_symmetric_inset_produces_no_shift() {
    for (h, v) in [(0.0, 0.0), (1.0, 1.0), (2.0, 4.0), (3.5, 0.5)] {
        let shift = optical_shift(OpticalInset::symmetric(h, v));
        assert_eq!(shift.x, 0.0, "symmetric horizontal gaps need no movement");
        assert_eq!(shift.y, 0.0, "symmetric vertical gaps need no movement");
    }
}

#[test]
fn test_is_symmetric_reads_both_axes() {
    assert!(OpticalInset::symmetric(2.0, 4.0).is_symmetric());
    assert!(OpticalInset::ZERO.is_symmetric());
    assert!(!OpticalInset::new(1.0, 2.0, 3.0, 2.0).is_symmetric());
    assert!(!OpticalInset::new(1.0, 2.0, 1.0, 5.0).is_symmetric());
}

// ---------------------------------------------------------------------------
// The centring property
// ---------------------------------------------------------------------------

/// The property the whole mechanism exists for: after applying
/// [`optical_shift`], the ink box's centre lands on the slot's centre.
fn assert_shift_centres(slot_w: f32, slot_h: f32, inset: OpticalInset) {
    let box_rect = optical_box(slot_w, slot_h, inset);
    let shift = optical_shift(inset);
    let ink_centre_x = box_rect.x + box_rect.width / 2.0 + shift.x;
    let ink_centre_y = box_rect.y + box_rect.height / 2.0 + shift.y;
    assert_eq!(ink_centre_x, slot_w / 2.0, "horizontal optical centring");
    assert_eq!(ink_centre_y, slot_h / 2.0, "vertical optical centring");
}

#[test]
fn test_shift_centres_an_asymmetric_directional_glyph() {
    // A `▸`-shaped ink: more room on the left than the right.
    assert_shift_centres(16.0, 16.0, OpticalInset::new(4.0, 2.0, 2.0, 2.0));
    assert_shift_centres(24.0, 24.0, OpticalInset::new(7.0, 4.0, 3.0, 4.0));
}

#[test]
fn test_shift_centres_a_symmetric_inset_trivially() {
    assert_shift_centres(24.0, 24.0, OpticalInset::ZERO);
    assert_shift_centres(16.0, 16.0, OpticalInset::symmetric(2.0, 2.0));
}

#[test]
fn test_shift_centres_a_pill_in_a_square_slot() {
    // A pill badge: wide and short, but still optically centred.
    let inset = OpticalInset::centring(24.0, 24.0, 20.0, 10.0);
    assert_shift_centres(24.0, 24.0, inset);
    let box_rect = optical_box(24.0, 24.0, inset);
    assert_eq!(box_rect.width, 20.0);
    assert_eq!(box_rect.height, 10.0);
}

#[test]
fn test_swapping_the_horizontal_edges_negates_the_shift() {
    let inset = OpticalInset::new(5.0, 1.0, 1.0, 1.0);
    let flipped = OpticalInset::new(1.0, 1.0, 5.0, 1.0);
    let a = optical_shift(inset);
    let b = optical_shift(flipped);
    assert_eq!(a.x, -b.x, "the shift must reverse with the asymmetry");
    assert_eq!(a.y, b.y, "the untouched axis must not move");
    assert_ne!(a.x, 0.0, "an asymmetric inset must actually shift");
}

#[test]
fn test_swapping_the_vertical_edges_negates_the_vertical_shift() {
    let inset = OpticalInset::new(1.0, 6.0, 1.0, 2.0);
    let flipped = OpticalInset::new(1.0, 2.0, 1.0, 6.0);
    let a = optical_shift(inset);
    let b = optical_shift(flipped);
    assert_eq!(a.y, -b.y);
    assert_eq!(a.x, b.x);
    assert_ne!(a.y, 0.0);
}

// ---------------------------------------------------------------------------
// The slot is a boundary
// ---------------------------------------------------------------------------

#[test]
fn test_optical_box_never_exceeds_its_slot() {
    let insets = [
        OpticalInset::ZERO,
        OpticalInset::symmetric(1.0, 1.0),
        OpticalInset::new(2.0, 3.0, 4.0, 5.0),
        OpticalInset::new(10.0, 10.0, 10.0, 10.0),
        OpticalInset::new(-4.0, -4.0, -4.0, -4.0),
        OpticalInset::new(100.0, 100.0, 100.0, 100.0),
    ];
    for inset in insets {
        let box_rect = optical_box(16.0, 24.0, inset);
        assert!(
            box_rect.width <= 16.0 && box_rect.height <= 24.0,
            "optical box escaped its slot: {box_rect:?} from {inset:?}"
        );
        assert!(
            box_rect.width >= 0.0 && box_rect.height >= 0.0,
            "optical box went negative: {box_rect:?} from {inset:?}"
        );
        assert!(box_rect.x >= 0.0 && box_rect.y >= 0.0, "negative origin");
    }
}

#[test]
fn test_negative_edges_fold_to_zero() {
    let clamped = OpticalInset::new(-5.0, 2.0, -1.0, 4.0).clamped();
    assert_eq!(clamped.left, 0.0);
    assert_eq!(clamped.top, 2.0);
    assert_eq!(clamped.right, 0.0);
    assert_eq!(clamped.bottom, 4.0);
}

#[test]
fn test_clamping_makes_the_box_larger_not_smaller() {
    // Folding a negative gap to zero returns the space that gap was
    // wrongly claiming, so the box grows back toward the slot.
    let raw = optical_box(16.0, 16.0, OpticalInset::new(-4.0, 0.0, 0.0, 0.0));
    let folded = optical_box(16.0, 16.0, OpticalInset::ZERO);
    assert_eq!(raw.width, folded.width, "the negative gap claims nothing");
    assert_eq!(raw.x, folded.x);
}

#[test]
fn test_exceeds_flags_an_impossible_inset() {
    assert!(OpticalInset::new(10.0, 0.0, 10.0, 0.0).exceeds(16.0, 16.0));
    assert!(OpticalInset::new(0.0, 9.0, 0.0, 9.0).exceeds(16.0, 16.0));
    assert!(!OpticalInset::new(8.0, 0.0, 8.0, 0.0).exceeds(16.0, 16.0));
    assert!(!OpticalInset::ZERO.exceeds(16.0, 16.0));
}

#[test]
fn test_an_exceeding_inset_still_produces_a_drawable_box() {
    // `exceeds` is how the caller learns the inset was impossible; the
    // geometry itself must never go negative.
    let inset = OpticalInset::new(20.0, 20.0, 20.0, 20.0);
    assert!(inset.exceeds(16.0, 16.0));
    let box_rect = optical_box(16.0, 16.0, inset);
    assert_eq!(box_rect.width, 0.0);
    assert_eq!(box_rect.height, 0.0);
}

// ---------------------------------------------------------------------------
// Centring constructors
// ---------------------------------------------------------------------------

#[test]
fn test_centring_square_is_symmetric_and_reproduces_the_ink() {
    let inset = OpticalInset::centring_square(24.0, 16.0);
    assert!(inset.is_symmetric());
    assert_eq!(inset.left, 4.0);
    let box_rect = optical_box(24.0, 24.0, inset);
    assert_eq!(box_rect.width, 16.0);
    assert_eq!(box_rect.height, 16.0);
    assert_eq!(optical_shift(inset), OpticalShift { x: 0.0, y: 0.0 });
}

#[test]
fn test_centring_rect_reproduces_an_arbitrary_ink_box() {
    let inset = OpticalInset::centring(24.0, 16.0, 18.0, 8.0);
    assert_eq!(inset.left, 3.0);
    assert_eq!(inset.right, 3.0);
    assert_eq!(inset.top, 4.0);
    assert_eq!(inset.bottom, 4.0);
    let box_rect = optical_box(24.0, 16.0, inset);
    assert_eq!(box_rect.width, 18.0);
    assert_eq!(box_rect.height, 8.0);
}

#[test]
fn test_centring_an_ink_as_large_as_the_slot_is_the_identity() {
    let inset = OpticalInset::centring(24.0, 24.0, 24.0, 24.0);
    assert_eq!(inset, OpticalInset::ZERO);
}

#[test]
fn test_totals_sum_each_axis() {
    let inset = OpticalInset::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(inset.total_horizontal(), 4.0);
    assert_eq!(inset.total_vertical(), 6.0);
}

// ---------------------------------------------------------------------------
// The directional-glyph case §1.1 actually names
// ---------------------------------------------------------------------------

#[test]
fn test_a_directional_glyph_centres_by_weight_not_by_box() {
    // Geometric centring alone leaves a `▸` looking off-centre; the
    // mechanism moves the em box so the ink lands true.
    let glyph_slot = GLYPH_SLOT_SM;
    let ink = OpticalInset::new(5.0, 3.0, 3.0, 3.0);

    let unshifted = optical_box(glyph_slot, glyph_slot, ink);
    let shift = optical_shift(ink);
    let centred_x = unshifted.x + unshifted.width / 2.0 + shift.x;

    assert_ne!(
        unshifted.x + unshifted.width / 2.0,
        glyph_slot / 2.0,
        "the ink must start off-centre or there would be nothing to correct"
    );
    assert_eq!(centred_x, glyph_slot / 2.0, "and the shift must correct it");
    assert_eq!(shift.x, -1.0, "ink heavy to the left moves the box left");
}

#[test]
fn test_a_glyph_with_no_measurement_gets_geometric_centring() {
    // The documented degradation: a host that has not measured its
    // typeface passes ZERO and gets exactly what it has today.
    let inset = OpticalInset::ZERO;
    assert_eq!(optical_shift(inset), OpticalShift { x: 0.0, y: 0.0 });
    assert_eq!(
        optical_box(GLYPH_SLOT_MD, GLYPH_SLOT_MD, inset).width,
        GLYPH_SLOT_MD
    );
}
