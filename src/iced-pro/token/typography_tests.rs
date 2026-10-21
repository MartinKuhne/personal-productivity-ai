//! Tests for [`typography`] — the type scale (`UIF-TOK-004`).
//!
//! Every field of every rung is pinned to the spec table, so a silent
//! retune fails loudly. On top of the pins, the tests enforce the
//! structural rules the scale implies: the 12px absolute floor, sizes
//! that strictly increase, tracking that tightens as type grows, resting
//! weights that never get lighter going up the scale, and emphasis that
//! is never weaker than the weight it emphasises.

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;

/// Tolerance for exact `f32` arithmetic on logical pixels.
const EPS: f32 = 1e-6;

/// Tolerance for the line-height multipliers the spec prints.
///
/// The spec rounds each multiplier to two decimals (`1.33×`, `1.38×`,
/// `1.43×`, `1.50×`, `1.40×`, `1.33×`), so a rounded figure can sit at
/// most half of that last digit away from the exact quotient: `0.005`.
/// The tightest case on the scale is `type.ui_compact`, where
/// `18 / 13 = 1.3846…` lands `0.0046` from the printed `1.38×` —
/// inside the rounding bound, but only just. That is why the tolerance is
/// pinned to the spec's rounding error rather than the far tighter
/// [`EPS`]: a looser bound would fail on values the spec legitimately
/// rounds, and a much looser one would stop detecting a real retune.
const RATIO_EPS: f32 = 5e-3;

/// A row of the `UIF-TOK-004` table transcribed verbatim from
/// `doc/planning/ui-framework.md` §2.4, paired with the constant that
/// implements it.
///
/// Keeping the transcription next to the rung means retuning either side
/// shows up as a failing row naming its own token, not a diff nobody
/// noticed.
#[derive(Clone, Copy)]
struct SpecRow {
    /// The spec token, e.g. `type.caption`.
    token: &'static str,
    /// The `iced-pro` constant implementing it.
    rung: TypeRung,
    /// The `Size (px)` column.
    size: f32,
    /// The `Line Height (px)` column.
    line_height: f32,
    /// The resting half of the `Weight` column.
    weight: Weight,
    /// The emphasis half of the `Weight` column — the same weight where
    /// the spec lists only one.
    emphasis_weight: Weight,
    /// The `Tracking` column, in em.
    tracking: f32,
    /// The multiplier the spec prints next to the line height, rounded
    /// to two decimals.
    stated_ratio: f32,
}

/// The six rungs against the spec table, in ascending size order.
///
/// Every table-driven test walks this same list, so no rung can be
/// skipped by a test that only checks a hand-picked subset.
fn spec_rows() -> [SpecRow; 6] {
    [
        SpecRow {
            token: "type.caption",
            rung: CAPTION,
            size: 12.0,
            line_height: 16.0,
            weight: Weight::Regular,
            emphasis_weight: Weight::Medium,
            tracking: 0.01,
            stated_ratio: 1.33,
        },
        SpecRow {
            token: "type.ui_compact",
            rung: UI_COMPACT,
            size: 13.0,
            line_height: 18.0,
            weight: Weight::Regular,
            emphasis_weight: Weight::Medium,
            tracking: 0.0,
            stated_ratio: 1.38,
        },
        SpecRow {
            token: "type.body",
            rung: BODY,
            size: 14.0,
            line_height: 20.0,
            weight: Weight::Regular,
            emphasis_weight: Weight::Regular,
            tracking: 0.0,
            stated_ratio: 1.43,
        },
        SpecRow {
            token: "type.title_sm",
            rung: TITLE_SM,
            size: 16.0,
            line_height: 24.0,
            weight: Weight::SemiBold,
            emphasis_weight: Weight::Bold,
            tracking: -0.01,
            stated_ratio: 1.50,
        },
        SpecRow {
            token: "type.title_md",
            rung: TITLE_MD,
            size: 20.0,
            line_height: 28.0,
            weight: Weight::Bold,
            emphasis_weight: Weight::Bold,
            tracking: -0.015,
            stated_ratio: 1.40,
        },
        SpecRow {
            token: "type.title_lg",
            rung: TITLE_LG,
            size: 24.0,
            line_height: 32.0,
            weight: Weight::Bold,
            emphasis_weight: Weight::Bold,
            tracking: -0.02,
            stated_ratio: 1.33,
        },
    ]
}

/// A rung with only the two dimensions that matter to the arithmetic
/// under test, so the ratio tests do not have to spell out weights.
fn rung(size: f32, line_height: f32) -> TypeRung {
    TypeRung {
        size,
        line_height,
        weight: Weight::Regular,
        emphasis_weight: Weight::Regular,
        tracking: 0.0,
    }
}

/// Every field of every rung carries the exact value the `UIF-TOK-004`
/// table specifies.
#[test]
fn test_every_rung_matches_spec_table() {
    for row in spec_rows() {
        assert_eq!(
            row.rung.size, row.size,
            "{} size must be {}px",
            row.token, row.size
        );
        assert_eq!(
            row.rung.line_height, row.line_height,
            "{} line height must be {}px",
            row.token, row.line_height
        );
        assert_eq!(
            row.rung.weight, row.weight,
            "{} must rest at {:?} weight",
            row.token, row.weight
        );
        assert_eq!(
            row.rung.emphasis_weight, row.emphasis_weight,
            "{} must emphasise to {:?} weight",
            row.token, row.emphasis_weight
        );
        assert_eq!(
            row.rung.tracking, row.tracking,
            "{} tracking must be {}em",
            row.token, row.tracking
        );
    }
}

/// Each rung's derived multiplier matches the one the spec prints next to
/// the line height, within the spec's own rounding tolerance.
#[test]
fn test_line_height_ratio_matches_stated_multiplier() {
    for row in spec_rows() {
        let actual = row.rung.line_height_ratio();
        assert!(
            (actual - row.stated_ratio).abs() <= RATIO_EPS,
            "{} derives {}x from {}px on {}px, but the spec states {}x (tolerance {RATIO_EPS})",
            row.token,
            actual,
            row.line_height,
            row.size,
            row.stated_ratio
        );
    }
}

/// Every rung clears the 12px absolute floor: "No text anywhere in the
/// interface may render smaller than 12px."
#[test]
fn test_every_rung_clears_the_12px_floor() {
    for row in spec_rows() {
        assert!(
            row.rung.size >= MIN_SIZE,
            "{} ({:.1}px) is below the {MIN_SIZE}px absolute text floor",
            row.token,
            row.rung.size
        );
    }
}

/// `type.caption` sits exactly on the floor. As the smallest rung it is
/// the one that carries the rule: any rung smaller than it would break
/// the floor by definition.
#[test]
fn test_caption_sits_exactly_on_the_floor() {
    assert_eq!(
        CAPTION.size, MIN_SIZE,
        "type.caption must sit exactly on the {MIN_SIZE}px floor, got {}px",
        CAPTION.size
    );
    let caption_size = CAPTION.size;
    for row in spec_rows() {
        assert!(
            caption_size <= row.rung.size,
            "type.caption ({caption_size}px) must be the smallest rung, but {} is {}px",
            row.token,
            row.rung.size
        );
    }
}

/// Sizes strictly increase up the scale, so a rung name always means
/// strictly larger type than the rung below it.
#[test]
fn test_rung_sizes_strictly_increase() {
    assert!(
        CAPTION.size < UI_COMPACT.size,
        "type.caption ({}) must be smaller than type.ui_compact ({})",
        CAPTION.size,
        UI_COMPACT.size
    );
    assert!(
        UI_COMPACT.size < BODY.size,
        "type.ui_compact ({}) must be smaller than type.body ({})",
        UI_COMPACT.size,
        BODY.size
    );
    assert!(
        BODY.size < TITLE_SM.size,
        "type.body ({}) must be smaller than type.title_sm ({})",
        BODY.size,
        TITLE_SM.size
    );
    assert!(
        TITLE_SM.size < TITLE_MD.size,
        "type.title_sm ({}) must be smaller than type.title_md ({})",
        TITLE_SM.size,
        TITLE_MD.size
    );
    assert!(
        TITLE_MD.size < TITLE_LG.size,
        "type.title_md ({}) must be smaller than type.title_lg ({})",
        TITLE_MD.size,
        TITLE_LG.size
    );
}

/// The spec table is ordered by ascending size, which is the precondition
/// every other table-driven invariant relies on. If a rung is inserted out
/// of order, this fails first and the ordering tests below stay honest.
#[test]
fn test_spec_table_is_ordered_by_ascending_size() {
    for pair in spec_rows().windows(2) {
        let (lower, upper) = (pair[0], pair[1]);
        assert!(
            lower.size < upper.size,
            "{} ({}) must come before {} ({}) only while it is smaller",
            lower.token,
            lower.size,
            upper.token,
            upper.size
        );
    }
}

/// The tracking direction of each rung matches the sign the spec gives
/// it: `type.caption` is the only positive rung, the two smallest-body
/// rungs are exactly zero, and every title tracks negative.
#[test]
fn test_tracking_signs_match_spec() {
    assert!(
        CAPTION.tracking > 0.0,
        "type.caption must track positive, got {}em",
        CAPTION.tracking
    );
    assert_eq!(
        UI_COMPACT.tracking, 0.0,
        "type.ui_compact must track exactly zero, got {}em",
        UI_COMPACT.tracking
    );
    assert_eq!(
        BODY.tracking, 0.0,
        "type.body must track exactly zero, got {}em",
        BODY.tracking
    );
    assert!(
        TITLE_SM.tracking < 0.0,
        "type.title_sm must track negative, got {}em",
        TITLE_SM.tracking
    );
    assert!(
        TITLE_MD.tracking < 0.0,
        "type.title_md must track negative, got {}em",
        TITLE_MD.tracking
    );
    assert!(
        TITLE_LG.tracking < 0.0,
        "type.title_lg must track negative, got {}em",
        TITLE_LG.tracking
    );
}

/// Tracking gets progressively tighter as the type gets larger: never
/// looser going up the scale, and strictly tighter by the top rung.
#[test]
fn test_tracking_gets_progressively_tighter() {
    for pair in spec_rows().windows(2) {
        let (lower, upper) = (pair[0], pair[1]);
        assert!(
            upper.tracking <= lower.tracking,
            "{} tracks {}em, which must not be looser than {} at {}em",
            upper.token,
            upper.tracking,
            lower.token,
            lower.tracking
        );
    }
    assert!(
        TITLE_LG.tracking < CAPTION.tracking,
        "type.title_lg ({}em) must be tighter than type.caption ({}em)",
        TITLE_LG.tracking,
        CAPTION.tracking
    );
}

/// Resting weight never gets lighter as the scale ascends. Compared by
/// [`Weight::rank`] rather than a derived `Ord`, because the ordering is
/// a stated property of the scale, not a consequence of declaration
/// order.
#[test]
fn test_resting_weight_never_gets_lighter() {
    for pair in spec_rows().windows(2) {
        let (lower, upper) = (pair[0], pair[1]);
        assert!(
            upper.weight.rank() >= lower.weight.rank(),
            "{} rests at {:?} (rank {}), which must not be lighter than {} at {:?} (rank {})",
            upper.token,
            upper.weight,
            upper.weight.rank(),
            lower.token,
            lower.weight,
            lower.weight.rank()
        );
    }
}

/// Emphasis never resolves to a lighter weight than the rung's resting
/// weight — emphasising text must never make it recede.
#[test]
fn test_emphasis_never_weaker_than_resting() {
    for row in spec_rows() {
        assert!(
            row.rung.emphasis_weight.rank() >= row.rung.weight.rank(),
            "{} emphasises to {:?} (rank {}), which must not be lighter than its resting {:?} (rank {})",
            row.token,
            row.rung.emphasis_weight,
            row.rung.emphasis_weight.rank(),
            row.rung.weight,
            row.rung.weight.rank()
        );
    }
}

/// The shape of the spec's `Weight` column is preserved: the three
/// single-weight rungs carry the same weight in both fields (emphasis is
/// a deliberate no-op, not an undefined gap), and the three
/// two-weight rungs step up exactly one rank.
#[test]
fn test_single_weight_rungs_and_two_weight_rungs_have_the_specified_shape() {
    for row in spec_rows() {
        // Signed so a rung that emphasised *lighter* reports through the
        // assertion below rather than as an arithmetic underflow panic.
        let step = row.rung.emphasis_weight.rank() as i8 - row.rung.weight.rank() as i8;
        match row.token {
            "type.body" | "type.title_md" | "type.title_lg" => assert_eq!(
                step, 0,
                "{} is a single-weight rung: emphasis {:?} must equal resting {:?}",
                row.token, row.rung.emphasis_weight, row.rung.weight
            ),
            _ => assert_eq!(
                step, 1,
                "{} is a two-weight rung: emphasis {:?} must be exactly one rank above resting {:?}",
                row.token, row.rung.emphasis_weight, row.rung.weight
            ),
        }
    }
}

/// Every rung sets its line taller than the glyphs it carries, so no
/// rung renders with leading that clips ascenders or descenders.
#[test]
fn test_line_height_always_exceeds_size() {
    for row in spec_rows() {
        assert!(
            row.rung.line_height > row.rung.size,
            "{} line height ({}px) must exceed its size ({}px)",
            row.token,
            row.rung.line_height,
            row.rung.size
        );
        assert!(
            row.rung.line_height_ratio() > 1.0,
            "{} derives a ratio of {}, which must be greater than 1.0",
            row.token,
            row.rung.line_height_ratio()
        );
    }
}

/// The four weights rank strictly and distinctly, which is what makes
/// rank comparisons over the scale meaningful.
#[test]
fn test_weight_rank_is_strictly_ordered() {
    assert!(Weight::Regular.rank() < Weight::Medium.rank());
    assert!(Weight::Medium.rank() < Weight::SemiBold.rank());
    assert!(Weight::SemiBold.rank() < Weight::Bold.rank());
    assert_eq!(Weight::Regular.rank(), Weight::Regular.rank());
    assert_ne!(Weight::Regular.rank(), Weight::Bold.rank());
}

/// A hand-built rung returns the plain quotient of its own fields: the
/// helper is arithmetic, not a lookup into the six constants.
#[test]
fn test_line_height_ratio_on_hand_built_rungs() {
    let exact = rung(18.0, 27.0);
    assert!(
        (exact.line_height_ratio() - 1.5).abs() < EPS,
        "27px on 18px must derive a 1.50x ratio, got {}",
        exact.line_height_ratio()
    );

    let repeating = rung(7.0, 10.0);
    assert!(
        (repeating.line_height_ratio() - 10.0 / 7.0).abs() < EPS,
        "10px on 7px must derive the exact quotient, got {}",
        repeating.line_height_ratio()
    );

    let unity = rung(10.0, 10.0);
    assert!(
        (unity.line_height_ratio() - 1.0).abs() < EPS,
        "equal size and line height must derive 1.00x, got {}",
        unity.line_height_ratio()
    );
}

/// The documented zero-size guard: a rung with no size returns `0.0`
/// rather than the `NaN` or `inf` the raw division would produce, and it
/// does so without panicking on a render path.
#[test]
fn test_line_height_ratio_zero_size_guard_is_finite() {
    let zero_line = rung(0.0, 16.0);
    let ratio = zero_line.line_height_ratio();
    assert!(
        ratio.is_finite(),
        "a zero-size rung must not escape a non-finite ratio, got {ratio}"
    );
    assert_eq!(
        ratio, 0.0,
        "the documented guard value for a non-positive size is 0.0, got {ratio}"
    );

    let zero_both = rung(0.0, 0.0);
    assert!(
        zero_both.line_height_ratio().is_finite(),
        "0px on 0px must not produce NaN, got {}",
        zero_both.line_height_ratio()
    );
}

/// A negative size is guarded the same way as a zero one: the guard tests
/// `> 0.0`, not `!= 0.0`, so a nonsensical negative rung cannot slip a
/// negative ratio through into layout.
#[test]
fn test_line_height_ratio_negative_size_is_guarded() {
    let negative = rung(-12.0, 16.0);
    let ratio = negative.line_height_ratio();
    assert!(
        ratio.is_finite(),
        "a negative-size rung must not escape a non-finite ratio, got {ratio}"
    );
    assert_eq!(
        ratio, 0.0,
        "the documented guard value for a negative size is 0.0, got {ratio}"
    );
}

/// The guard's sentinel is unreachable from the real scale: every actual
/// rung derives a ratio above 1.0, so `0.0` unambiguously means "this
/// rung had no size" rather than a plausible measurement.
#[test]
fn test_guard_value_is_unreachable_from_real_rungs() {
    for row in spec_rows() {
        assert_ne!(
            row.rung.line_height_ratio(),
            0.0,
            "{} derives 0.0, which would make the guard sentinel ambiguous",
            row.token
        );
    }
}

/// The floor token itself is the 12px the spec names, and is positive so
/// the rule it encodes stays enforceable.
#[test]
fn test_min_size_is_twelve_pixels() {
    assert_eq!(MIN_SIZE, 12.0, "the absolute text floor must be 12px");
    assert!(MIN_SIZE > 0.0, "the text floor must be strictly positive");
}
