//! Tests for [`typography`] — the type scale (`UIF-TOK-004`).
//!
//! Every field of every rung is pinned to the spec table, so a silent
//! retune fails loudly. On top of the pins, the tests enforce the
//! structural rules the scale implies: the 12px absolute floor, sizes
//! that strictly increase, tracking that tightens as type grows, resting
//! weights that never get lighter going up the scale, and emphasis that
//! is never weaker than the weight it emphasises.
//!
//! The `Allowed Weights` column gets the same treatment as a set rather
//! than a pair: each rung's set is compared to the transcription exactly,
//! the full rung × weight matrix is swept in both directions, and the
//! resting and emphasis weights are held inside the set they belong to.
//! The two `font.family.*` tokens are checked against iced's own
//! constants — including that `font.family.mono` really is monospace.
//! Two of the composition tests are no-visual-change proofs for the
//! call sites that used to compose weights by hand, and the last group
//! pins what the bundled-font limitation does and does not let the crate
//! claim.

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
    /// The `Allowed Weights` column, verbatim and in the order the spec
    /// prints it (lightest first).
    allowed: &'static [Weight],
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
            allowed: &[Weight::Regular, Weight::Medium, Weight::SemiBold],
            tracking: 0.01,
            stated_ratio: 1.33,
        },
        SpecRow {
            token: "type.ui_compact",
            rung: UI_COMPACT,
            size: 13.0,
            line_height: 18.0,
            weight: Weight::Regular,
            emphasis_weight: Weight::SemiBold,
            allowed: &[
                Weight::Regular,
                Weight::Medium,
                Weight::SemiBold,
                Weight::Bold,
            ],
            tracking: 0.0,
            stated_ratio: 1.38,
        },
        SpecRow {
            token: "type.body",
            rung: BODY,
            size: 14.0,
            line_height: 20.0,
            weight: Weight::Regular,
            emphasis_weight: Weight::Medium,
            allowed: &[Weight::Regular, Weight::Medium, Weight::SemiBold],
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
            allowed: &[Weight::SemiBold, Weight::Bold],
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
            allowed: &[Weight::SemiBold, Weight::Bold],
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
            allowed: &[Weight::Bold],
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
        allowed_weights: &[Weight::Regular],
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

/// The shape of the spec's `Resting / Emphasis` column, rung by rung,
/// as §2.4 actually prints it.
///
/// The model this test used to defend — three single-weight rungs, and
/// every two-weight rung stepping exactly one rank — was an invention,
/// and the transcription was written to match the invention rather than
/// the document. §2.4 contradicts both halves of it. `type.body` is a
/// one-rank rung (`Regular / Medium`), not a single-weight one, and
/// `type.ui_compact` steps **two** ranks (`Regular / SemiBold`). Only
/// `type.title_md` and `type.title_lg` are single-weight.
///
/// The step is asserted per rung rather than by a rule, because the
/// document's column is a table of facts, not a generated pattern: if a
/// future revision moves one rung, this fails and forces the rung and the
/// table to be reconciled on purpose.
#[test]
fn test_rung_weight_shape_matches_the_spec_column() {
    // Emphasis step in ranks, exactly as §2.4's column reads.
    let expected_step: [(&str, i8); 6] = [
        ("type.caption", 1),    // Regular  -> Medium
        ("type.ui_compact", 2), // Regular  -> SemiBold
        ("type.body", 1),       // Regular  -> Medium
        ("type.title_sm", 1),   // SemiBold -> Bold
        ("type.title_md", 0),   // Bold     -> Bold
        ("type.title_lg", 0),   // Bold     -> Bold
    ];
    for row in spec_rows() {
        // Signed so a rung that emphasised *lighter* reports through the
        // assertion below rather than as an arithmetic underflow panic.
        let step = row.rung.emphasis_weight.rank() as i8 - row.rung.weight.rank() as i8;
        let expected = expected_step
            .iter()
            .find(|(token, _)| *token == row.token)
            .unwrap_or_else(|| panic!("no spec expectation for {}", row.token))
            .1;
        assert_eq!(
            step,
            expected,
            "{} emphasises {:?} (rank {}) from resting {:?} (rank {}), a step of \
             {step}; §2.4 prints a step of {expected}",
            row.token,
            row.rung.emphasis_weight,
            row.rung.emphasis_weight.rank(),
            row.rung.weight,
            row.rung.weight.rank()
        );
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

// ---------------------------------------------------------------------------
// Allowed-weight sets — the `Allowed Weights` column
// ---------------------------------------------------------------------------

/// The four weights the scale draws on, for matrix-style sweeps.
const ALL_WEIGHTS: [Weight; 4] = [
    Weight::Regular,
    Weight::Medium,
    Weight::SemiBold,
    Weight::Bold,
];

/// Every rung's permitted set is exactly the `Allowed Weights` column of
/// the §2.4 table — same members, same order, nothing extra.
#[test]
fn test_allowed_weight_set_matches_the_spec_column_exactly() {
    for row in spec_rows() {
        assert_eq!(
            row.rung.allowed_weights, row.allowed,
            "{} permits {:?}, but the spec's `Allowed Weights` column is {:?}",
            row.token, row.rung.allowed_weights, row.allowed
        );
    }
}

/// The full rung × weight matrix: `allows` agrees with the spec column
/// for all twenty-four pairs. Sweeping the refusals as well as the
/// permissions is the point — a rung that quietly widened its own set
/// still passes every positive case, so only the negative half catches
/// it.
#[test]
fn test_allows_agrees_with_the_spec_for_every_rung_and_weight() {
    for row in spec_rows() {
        for weight in ALL_WEIGHTS {
            let expected = row.allowed.contains(&weight);
            assert_eq!(
                row.rung.allows(weight),
                expected,
                "{} must {} {:?} — the spec column is {:?}",
                row.token,
                if expected { "permit" } else { "refuse" },
                weight,
                row.allowed
            );
        }
    }
}

/// The four named cases the gap is stated in, spelled out individually so
/// a regression names the rung that broke rather than a matrix cell.
#[test]
fn test_the_four_weight_predicates_the_gap_named() {
    assert!(
        CAPTION.allows(Weight::SemiBold),
        "type.caption must permit SemiBold — it is what §4.8.1's menu \
         section heading wears"
    );
    assert!(
        !BODY.allows(Weight::Bold),
        "type.body permits Regular and Medium only; Bold must be refused"
    );
    for weight in ALL_WEIGHTS {
        assert!(
            UI_COMPACT.allows(weight),
            "type.ui_compact permits all four weights, {:?} included",
            weight
        );
    }
    assert!(
        !TITLE_MD.allows(Weight::Regular),
        "type.title_md permits Bold alone; Regular must be refused"
    );
}

/// The pair cannot escape the set: both the resting weight and the
/// emphasis weight are members of their own rung's permitted set, for
/// every rung. This is what makes the set the authority rather than a
/// comment beside two independently chosen numbers.
#[test]
fn test_resting_and_emphasis_are_members_of_their_own_permitted_set() {
    for row in spec_rows() {
        assert!(
            row.rung.allows(row.rung.weight),
            "{} rests at {:?}, which its own permitted set {:?} does not contain",
            row.token,
            row.rung.weight,
            row.rung.allowed_weights
        );
        assert!(
            row.rung.allows(row.rung.emphasis_weight),
            "{} emphasises to {:?}, which its own permitted set {:?} does not \
             contain — a rung cannot advertise an emphasis the spec forbids",
            row.token,
            row.rung.emphasis_weight,
            row.rung.allowed_weights
        );
    }
}

/// Every rung rests at the bottom of its own permitted set, with exactly
/// one exception that §2.4 itself creates: `type.title_md` rests at Bold
/// while also permitting SemiBold, so its resting voice is heavier than
/// the lightest weight it may carry.
///
/// This test used to assert "resting is always the lightest permitted"
/// across all six rungs. That was never a property of the document — it
/// was a property of the code, which had narrowed `TITLE_MD`'s permitted
/// set to Bold alone in order to make the invariant hold. The set is now
/// what §2.4 prints, so the exception is named rather than designed away.
#[test]
fn test_only_title_md_rests_above_the_lightest_permitted_weight() {
    for row in spec_rows() {
        let lightest = row
            .rung
            .allowed_weights
            .iter()
            .map(|w| w.rank())
            .min()
            .unwrap_or(u8::MAX);
        if row.token == "type.title_md" {
            assert_eq!(
                row.rung.weight.rank() as i16 - lightest as i16,
                1,
                "{} is the spec's one rung resting above its lightest permitted \
                 weight: rests {:?} (rank {}), lightest permitted rank {lightest}",
                row.token,
                row.rung.weight,
                row.rung.weight.rank()
            );
        } else {
            assert_eq!(
                row.rung.weight.rank(),
                lightest,
                "{} rests at {:?} (rank {}) but its lightest permitted weight is \
                 rank {lightest}; only type.title_md may rest above the bottom of \
                 its set",
                row.token,
                row.rung.weight,
                row.rung.weight.rank()
            );
        }
    }
}

/// Emphasis stops at the top of the set: the heaviest thing a rung can
/// emphasise to is the heaviest thing it may carry.
#[test]
fn test_emphasis_never_exceeds_the_heaviest_permitted() {
    for row in spec_rows() {
        let heaviest = row
            .rung
            .allowed_weights
            .iter()
            .map(Weight::rank)
            .max()
            .unwrap_or(u8::MAX);
        assert!(
            row.rung.emphasis_weight.rank() <= heaviest,
            "{} emphasises to {:?} (rank {}), above the heaviest weight its \
             set permits (rank {})",
            row.token,
            row.rung.emphasis_weight,
            row.rung.emphasis_weight.rank(),
            heaviest
        );
    }
}

/// Each permitted set is ordered lightest-first and carries no duplicate,
/// which is what lets the set be read straight off the spec table.
#[test]
fn test_permitted_sets_are_ordered_lightest_first_and_deduplicated() {
    for row in spec_rows() {
        assert!(
            !row.rung.allowed_weights.is_empty(),
            "{} must permit at least one weight",
            row.token
        );
        for pair in row.rung.allowed_weights.windows(2) {
            assert!(
                pair[0].rank() < pair[1].rank(),
                "{}'s permitted set must ascend lightest-first, but {:?} precedes {:?}",
                row.token,
                pair[0],
                pair[1]
            );
        }
    }
}

/// The sets are genuinely wider than the pairs somewhere: `type.caption`
/// permits a weight that is neither its resting nor its emphasis weight.
/// Without that, the set would be redundant with the pair and this
/// increment would have added nothing.
#[test]
fn test_caption_permits_a_weight_that_is_neither_resting_nor_emphasis() {
    let neither = CAPTION
        .allowed_weights
        .iter()
        .filter(|w| **w != CAPTION.weight && **w != CAPTION.emphasis_weight)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        neither,
        vec![Weight::SemiBold],
        "type.caption's only weight beyond the resting/emphasis pair must be \
         SemiBold — the pairing G-12 could not express"
    );
}

/// A hand-built rung with an empty permitted set refuses everything,
/// including its own resting weight: `allows` is a lookup into the set,
/// not a fallback to whatever the rung happens to rest at.
#[test]
fn test_empty_permitted_set_refuses_everything() {
    let orphan = TypeRung {
        size: 14.0,
        line_height: 20.0,
        weight: Weight::Regular,
        emphasis_weight: Weight::Regular,
        allowed_weights: &[],
        tracking: 0.0,
    };
    for weight in ALL_WEIGHTS {
        assert!(
            !orphan.allows(weight),
            "an empty permitted set must refuse {:?}, not fall back to the \
             resting weight",
            weight
        );
    }
}

// ---------------------------------------------------------------------------
// Weight → iced mapping
// ---------------------------------------------------------------------------

/// The numeric weight classes are the four the spec names beside the
/// `weight.*` token names.
#[test]
fn test_weight_css_classes_match_the_spec() {
    assert_eq!(Weight::Regular.css(), 400);
    assert_eq!(Weight::Medium.css(), 500);
    assert_eq!(Weight::SemiBold.css(), 600);
    assert_eq!(Weight::Bold.css(), 700);
}

/// iced spells the 400 class `Normal` and the 600 class `Semibold`. The
/// mapping is total, exact, and injective — four distinct tokens, four
/// distinct iced weights, nothing collapsed.
#[test]
fn test_weight_iced_mapping_is_exact_and_injective() {
    assert_eq!(Weight::Regular.iced(), iced::font::Weight::Normal);
    assert_eq!(Weight::Medium.iced(), iced::font::Weight::Medium);
    assert_eq!(Weight::SemiBold.iced(), iced::font::Weight::Semibold);
    assert_eq!(Weight::Bold.iced(), iced::font::Weight::Bold);

    let mapped = ALL_WEIGHTS.map(|weight| weight.iced());
    for (i, a) in mapped.iter().enumerate() {
        for (j, b) in mapped.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "two distinct weight tokens must not collapse \
                                  onto the same iced weight"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Font family tokens
// ---------------------------------------------------------------------------

/// The two family tokens are the two §2.4 names, and each carries its own
/// dotted spec name.
#[test]
fn test_family_tokens_are_the_two_the_spec_names() {
    assert_eq!(FAMILY_SANS, Family::Sans);
    assert_eq!(FAMILY_MONO, Family::Mono);
    assert_eq!(FAMILY_SANS.token_name(), "font.family.sans");
    assert_eq!(FAMILY_MONO.token_name(), "font.family.mono");
    assert_ne!(
        FAMILY_SANS, FAMILY_MONO,
        "the two family tokens must be distinguishable"
    );
}

/// Each family resolves to the expected iced family.
#[test]
fn test_families_resolve_to_the_expected_iced_families() {
    assert_eq!(FAMILY_SANS.iced(), iced::font::Family::SansSerif);
    assert_eq!(FAMILY_MONO.iced(), iced::font::Family::Monospace);
}

/// The mono token really is fixed-pitch: it resolves to the very `Font`
/// iced itself names `MONOSPACE`, and the sans token is not it.
#[test]
fn test_mono_family_is_actually_monospace() {
    assert!(FAMILY_MONO.is_monospace());
    assert!(!FAMILY_SANS.is_monospace());
    assert_eq!(
        FAMILY_MONO.regular(),
        iced::Font::MONOSPACE,
        "font.family.mono at rest must be iced's own MONOSPACE font"
    );
    assert_ne!(FAMILY_SANS.regular(), iced::Font::MONOSPACE);
}

/// The sans family at rest is iced's default font — `SansSerif` at
/// `Normal` — which is why a widget that never sets a family keeps
/// rendering exactly as it did before the token existed.
#[test]
fn test_sans_family_at_rest_is_iced_default() {
    assert_eq!(
        FAMILY_SANS.regular(),
        iced::Font::DEFAULT,
        "font.family.sans at Regular is iced's DEFAULT font, so introducing \
         the token cannot shift any existing rendering"
    );
}

/// A family-plus-weight `Font` carries exactly those two things. Stretch
/// and style stay `Normal` because the scale specifies neither, and
/// inventing an italic or a condensed face in the token layer would be a
/// decision the spec has not made.
#[test]
fn test_family_font_carries_the_family_and_weight_and_nothing_else() {
    for family in [FAMILY_SANS, FAMILY_MONO] {
        for weight in ALL_WEIGHTS {
            let font = family.font(weight);
            assert_eq!(font.family, family.iced(), "{family:?} font family");
            assert_eq!(font.weight, weight.iced(), "{weight:?} weight");
            assert_eq!(
                font.stretch,
                iced::font::Stretch::Normal,
                "the scale specifies no stretch, so it must stay Normal"
            );
            assert_eq!(
                font.style,
                iced::font::Style::Normal,
                "the scale specifies no style, so it must stay Normal"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rung-carried compositions
// ---------------------------------------------------------------------------

/// A rung-carried font adds no fields of its own: `TypeRung::font` is the
/// rung's *permission* over the family's construction, so for every rung
/// and every weight it permits the result is identical to asking the
/// family directly.
#[test]
fn test_rung_font_delegates_to_the_family_it_is_handed() {
    for row in spec_rows() {
        for family in [FAMILY_SANS, FAMILY_MONO] {
            for weight in row.allowed {
                assert_eq!(
                    row.rung.font(family, *weight),
                    family.font(*weight),
                    "{} on {:?} at {:?} must be the family's own font",
                    row.token,
                    family,
                    weight
                );
            }
        }
    }
}

/// **No-visual-change proof for `surface/menu/view.rs`.** The 12px
/// SemiBold section heading used to build its font by hand as
/// `iced::Font { weight: Semibold, ..Font::DEFAULT }`. Resolved through
/// the scale instead — `CAPTION` on `font.family.sans` at SemiBold — it
/// produces the identical `iced::Font`, at the identical size and line
/// height. The composition moved into the token layer; the rendering did
/// not move.
#[test]
fn test_caption_semi_bold_composition_renders_exactly_as_before() {
    let before = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    };
    let after = CAPTION.font(FAMILY_SANS, Weight::SemiBold);

    assert_eq!(
        after, before,
        "the tokenised heading must render the same font the hand-composed one did"
    );
    assert_eq!(CAPTION.size, 12.0, "the heading size must not have moved");
    assert_eq!(
        CAPTION.line_height, 16.0,
        "the heading line height must not have moved"
    );
    assert_eq!(
        after.family,
        iced::font::Family::SansSerif,
        "the old literal used `..Font::DEFAULT`, which is SansSerif"
    );
    assert_eq!(after.weight, iced::font::Weight::Semibold);
}

/// **No-visual-change proof for `surface/movable.rs`.** The overlay
/// title is `type.ui_compact` at Bold: 13px on an 18px line, on the
/// sans family, at iced's `Bold`. That is exactly what the module drew
/// when the pairing was an undocumented composition, so tokenising it
/// changed the provenance and nothing else.
#[test]
fn test_ui_compact_bold_composition_renders_exactly_as_before() {
    let before = iced::Font {
        weight: iced::font::Weight::Bold,
        ..iced::Font::DEFAULT
    };
    let after = UI_COMPACT.font(FAMILY_SANS, Weight::Bold);

    assert_eq!(
        after, before,
        "the tokenised overlay title must render the same font the composed one did"
    );
    assert_eq!(UI_COMPACT.size, 13.0, "the title size must not have moved");
    assert_eq!(
        UI_COMPACT.line_height, 18.0,
        "the title line height must not have moved"
    );
    assert_eq!(after.weight, iced::font::Weight::Bold);
}

// ---------------------------------------------------------------------------
// The bundled-font limitation
// ---------------------------------------------------------------------------

/// What *is* verifiable about the limitation: neither family token names
/// a concrete face. Both resolve to iced's generic families, so the
/// token layer never claims a TTF it does not ship. Naming `"Fira Sans"`
/// would promise a four-weight family while `fira-sans` bundles
/// `FiraSans-Regular.ttf` alone.
#[test]
fn test_no_family_token_claims_a_named_face() {
    for family in [FAMILY_SANS, FAMILY_MONO] {
        assert!(
            !matches!(family.iced(), iced::font::Family::Name(_)),
            "{:?} must resolve to a generic iced family rather than a named \
             face: naming a face would claim weights the crate does not bundle",
            family
        );
    }
}

/// What is explicitly **not** claimed: that a Medium, SemiBold or Bold
/// face, or any monospace face, exists at render time. iced's `fira-sans`
/// feature ships `FiraSans-Regular.ttf` plus `Iced-Icons.ttf` — one text
/// face, and it is the Regular one — so every weight above Regular and
/// the whole mono family are answered by the host's system font
/// fallback, per machine.
///
/// The crate's promise is that all eight family × weight pairs are
/// expressible and distinct *as requests*. That is where its knowledge
/// ends, so this test asserts nothing about which face is rendered: no
/// assertion in the token layer could be evidence about that.
#[test]
fn test_glyph_availability_is_expressible_but_never_claimed() {
    let mut requests = Vec::new();
    for family in [FAMILY_SANS, FAMILY_MONO] {
        for weight in ALL_WEIGHTS {
            let font = family.font(weight);
            assert_eq!(font.family, family.iced());
            assert_eq!(font.weight, weight.iced());
            requests.push(font);
        }
    }

    assert_eq!(
        requests.len(),
        8,
        "both families must express all four weights"
    );
    for (i, a) in requests.iter().enumerate() {
        for (j, b) in requests.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "distinct family × weight pairs must be distinct requests, \
                     whether or not the host can honour them"
                );
            }
        }
    }
}
