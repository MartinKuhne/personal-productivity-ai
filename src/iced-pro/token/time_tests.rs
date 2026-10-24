//! Tests for [`time`] — the duration scale and its easing curves
//! (`UIF-TOK-007`).
//!
//! Four layers of assertion.
//!
//! 1. **Value pins.** Every rung is pinned to the literal duration and
//!    the easing curve §2.6's table gives it, so a silent retune fails.
//! 2. **Curve behaviour.** The three easings are checked as *functions*,
//!    not names: endpoints, monotonicity, clamping and the continuity of
//!    the piecewise curve.
//! 3. **The reduced-motion contract.** `resolve_ms` is checked over the
//!    whole rung table, so the mandatory fallback fires on every
//!    transition and touches no timer — in both directions.
//! 4. **Cross-references.** Every pre-existing scattered constant is
//!    asserted equal to the rung it now reads through, which is what
//!    makes "one source of truth" a checked fact rather than a claim.
//!
//! The last test reads this module's own source text and asserts it
//! contains no clock-reading identifier, so the "no clock reads here"
//! rule cannot be broken by a future edit that nobody notices.

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;
use crate::composite::notification::DEFAULT_AUTO_DISMISS_MS;
use crate::surface::tooltip::{HOVER_DELAY, KEYBOARD_FOCUS_DELAY};
use crate::token_lint::redact_comments_and_strings;
use crate::widget::auto_suggest_model::DEBOUNCE_MS;

/// Tolerance for easing arithmetic on `f32`.
const EPS: f32 = 1e-6;

/// The `(spec token, ms, easing)` triples §2.6's table prints, in table
/// order.
///
/// Shared by the value pins so each invariant is checked over every rung
/// rather than a hand-picked subset, and so a rung missing from the
/// transcription fails the length assertion rather than going unnoticed.
fn spec_table() -> [(&'static str, u64, Option<Easing>); 10] {
    [
        ("time.instant", 0, Some(Easing::Linear)),
        ("time.fast", 100, Some(Easing::EaseOutCubic)),
        ("time.normal", 180, Some(Easing::EaseOutCubic)),
        ("time.slow", 240, Some(Easing::EaseInOutCubic)),
        ("time.debounce.live", 200, None),
        ("time.delay.submenu", 150, None),
        ("time.delay.tooltip_open", 400, None),
        ("time.delay.tooltip_warm", 100, None),
        ("time.hold.copied", 1_500, None),
        ("time.toast.auto_dismiss", 5_000, None),
    ]
}

/// Every rung, paired with its spec row by name.
fn rung_rows() -> Vec<(&'static str, DurationToken)> {
    DurationToken::ALL.iter().map(|r| (r.name, *r)).collect()
}

// ---------------------------------------------------------------------------
// 1. Value pins
// ---------------------------------------------------------------------------

/// Every rung carries the exact duration and the exact easing curve the
/// `UIF-TOK-007` table specifies.
#[test]
fn test_time_values_match_spec_table() {
    let rows = rung_rows();
    for (name, ms, easing) in spec_table() {
        let rung = rows
            .iter()
            .find(|(rung_name, _)| *rung_name == name)
            .unwrap_or_else(|| panic!("{name} is missing from DurationToken::ALL"));
        assert_eq!(rung.1.ms, ms, "{name} must be {ms}ms, found {}", rung.1.ms);
        assert_eq!(
            rung.1.easing, easing,
            "{name} must carry {easing:?}, found {:?}",
            rung.1.easing
        );
    }
}

/// The transcription is exhaustive in the other direction too: no rung
/// carries a dotted name §2.6 does not print, and no rung is listed
/// twice.
#[test]
fn test_rung_names_are_exactly_the_spec_names() {
    let expected: Vec<&str> = spec_table().iter().map(|(name, _, _)| *name).collect();
    let actual = rung_rows();
    assert_eq!(
        actual.len(),
        expected.len(),
        "DurationToken::ALL must carry exactly the ten rungs §2.6 prints"
    );
    for (name, rung) in &actual {
        assert!(
            expected.contains(name),
            "{name} is not a `time.*` token §2.6 defines"
        );
        assert_eq!(
            rung.name, *name,
            "a rung's `name` field must be the key it is listed under"
        );
    }
    let unique: std::collections::BTreeSet<&str> = actual.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        unique.len(),
        expected.len(),
        "a dotted token name appears twice in the scale"
    );
}

/// The four transition rungs are exactly the four §2.6 gives a curve to,
/// and the six timer rungs are exactly the six it dashes.
///
/// This is the partition the reduced-motion rule is scoped to, so it is
/// pinned independently of the resolver: if a timer rung were ever given
/// a curve it would silently fall inside the fallback, and if a
/// transition lost its curve it would silently escape it.
#[test]
fn test_transition_and_timer_rungs_are_the_two_halves_the_table_draws() {
    let transitions: Vec<&str> = rung_rows()
        .iter()
        .filter(|(_, rung)| rung.is_transition())
        .map(|(name, _)| *name)
        .collect();
    let timers: Vec<&str> = rung_rows()
        .iter()
        .filter(|(_, rung)| !rung.is_transition())
        .map(|(name, _)| *name)
        .collect();

    assert_eq!(
        transitions,
        vec!["time.instant", "time.fast", "time.normal", "time.slow",],
        "only the four rungs §2.6 gives an easing to may be transitions"
    );
    assert_eq!(
        timers,
        vec![
            "time.debounce.live",
            "time.delay.submenu",
            "time.delay.tooltip_open",
            "time.delay.tooltip_warm",
            "time.hold.copied",
            "time.toast.auto_dismiss",
        ],
        "the six dashed rungs stay timers and must never be treated as motion"
    );
    assert_eq!(
        transitions.len() + timers.len(),
        DurationToken::ALL.len(),
        "every rung must land on exactly one side of the partition"
    );
}

/// No rung has a negative duration, and only `time.instant` is zero.
///
/// A negative duration would make `elapsed / duration` negative and send
/// an interpolation weight outside the range every curve is defined on.
#[test]
fn test_rung_durations_are_non_negative_and_only_instant_is_zero() {
    for (name, rung) in rung_rows() {
        assert!(rung.ms < i64::MAX as u64, "{name} ({}) is absurd", rung.ms);
        if name == "time.instant" {
            assert_eq!(rung.ms, 0, "time.instant must be exactly 0ms");
        } else {
            assert!(
                rung.ms > 0,
                "{name} must be strictly positive; only time.instant is 0ms"
            );
        }
    }
}

/// The three easing variants in the set are exactly the three §2.6
/// names, and each reports its own spelling back.
#[test]
fn test_easing_set_matches_the_three_the_spec_names() {
    assert_eq!(Easing::ALL.len(), 3);
    assert_eq!(Easing::Linear.token_name(), "Linear");
    assert_eq!(Easing::EaseOutCubic.token_name(), "EaseOutCubic");
    assert_eq!(Easing::EaseInOutCubic.token_name(), "EaseInOutCubic");
    for easing in Easing::ALL {
        assert!(
            spec_table()
                .iter()
                .any(|(_, _, curve)| *curve == Some(easing)),
            "{easing:?} is in Easing::ALL but no rung uses it"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Curve behaviour
// ---------------------------------------------------------------------------

/// Every curve maps `0.0` to `0.0` and `1.0` to `1.0`.
///
/// These two endpoints are what make a curve usable as an interpolation
/// weight without checking it: `lerp(from, to, f(0))` is `from` and
/// `lerp(from, to, f(1))` is `to`.
#[test]
fn test_every_easing_maps_the_endpoints_exactly() {
    for easing in Easing::ALL {
        assert_eq!(
            easing.apply(0.0),
            0.0,
            "{}(0.0) must be exactly 0.0",
            easing.token_name()
        );
        assert_eq!(
            easing.apply(1.0),
            1.0,
            "{}(1.0) must be exactly 1.0",
            easing.token_name()
        );
    }
}

/// Every curve is monotonically non-decreasing across the interval.
///
/// A curve that went backwards would make an animation visibly retreat,
/// which no easing on this scale is allowed to do.
#[test]
fn test_every_easing_is_monotonically_non_decreasing() {
    const STEPS: u32 = 1_000;
    for easing in Easing::ALL {
        let mut previous = easing.apply(0.0);
        for step in 1..=STEPS {
            let t = step as f32 / STEPS as f32;
            let value = easing.apply(t);
            assert!(
                value >= previous - EPS,
                "{} decreases between t={:.4} and t={:.4} ({:.6} -> {:.6})",
                easing.token_name(),
                (step - 1) as f32 / STEPS as f32,
                t,
                previous,
                value
            );
            previous = value;
        }
    }
}

/// Every curve stays inside `[0.0, 1.0]` over the whole interval, so its
/// output is always a legal interpolation weight.
#[test]
fn test_every_easing_stays_within_unit_range() {
    const STEPS: u32 = 1_000;
    for easing in Easing::ALL {
        for step in 0..=STEPS {
            let t = step as f32 / STEPS as f32;
            let value = easing.apply(t);
            assert!(
                (0.0..=1.0).contains(&value),
                "{}({:.4}) = {value} is outside [0.0, 1.0]",
                easing.token_name(),
                t
            );
        }
    }
}

/// Out-of-range input is clamped rather than extrapolated.
///
/// A caller dividing `elapsed / duration` overshoots `1.0` as a matter
/// of course; extrapolating a cubic past the end would hand back a weight
/// above `1.0` and push an interpolated value past its own target.
#[test]
fn test_easing_clamps_input_outside_the_unit_interval() {
    for easing in Easing::ALL {
        assert_eq!(
            easing.apply(-1.0),
            0.0,
            "{} must clamp negative input to 0.0",
            easing.token_name()
        );
        assert_eq!(
            easing.apply(-0.0001),
            0.0,
            "{} must clamp just-below-zero input to 0.0",
            easing.token_name()
        );
        assert_eq!(
            easing.apply(1.0001),
            1.0,
            "{} must clamp just-past-one input to 1.0",
            easing.token_name()
        );
        assert_eq!(
            easing.apply(9_000.0),
            1.0,
            "{} must clamp far-past-one input to 1.0",
            easing.token_name()
        );
    }
}

/// `EaseOutCubic` is front-loaded: it covers more than half the distance
/// in the first half of the time.
///
/// That asymmetry is the whole reason the spec picks it for
/// pointer-driven changes — the response starts at full speed — and a
/// curve that happened to be symmetric would not be it.
#[test]
fn test_ease_out_cubic_is_front_loaded() {
    let half = Easing::EaseOutCubic.apply(0.5);
    assert!(
        half > 0.7,
        "EaseOutCubic must be clearly front-loaded; f(0.5) = {half}"
    );
    assert!(
        (half - 0.875).abs() < EPS,
        "EaseOutCubic(0.5) must be exactly 1 - 0.5³ = 0.875, got {half}"
    );
}

/// `EaseInOutCubic` is symmetric about the midpoint and passes through
/// `f(0.5) = 0.5` with no kink.
///
/// The curve is piecewise, so the two branches meeting is a real risk:
/// a discontinuity at the midpoint would show as a visible jump
/// half-way through a 240ms scrim fade.
#[test]
fn test_ease_in_out_cubic_is_continuous_and_symmetric_at_the_midpoint() {
    let mid = Easing::EaseInOutCubic.apply(0.5);
    assert!(
        (mid - 0.5).abs() < EPS,
        "EaseInOutCubic(0.5) must be exactly 0.5, got {mid}"
    );
    const STEPS: u32 = 500;
    for step in 0..STEPS {
        let lower = step as f32 / STEPS as f32;
        let upper = 1.0 - lower;
        let symmetric = Easing::EaseInOutCubic.apply(lower) + Easing::EaseInOutCubic.apply(upper);
        assert!(
            (symmetric - 1.0).abs() < 1e-4,
            "EaseInOutCubic is not symmetric about 0.5: f({lower:.4}) + f({upper:.4}) = {symmetric}"
        );
    }
    // The branches must not disagree at the seam.
    let just_below = Easing::EaseInOutCubic.apply(0.5 - 1e-6);
    let just_above = Easing::EaseInOutCubic.apply(0.5 + 1e-6);
    assert!(
        (just_below - just_above).abs() < 1e-4,
        "EaseInOutCubic has a kink at t = 0.5: {just_below} vs {just_above}"
    );
}

/// The two cubic curves are genuinely distinct, and `EaseInOutCubic`
/// earns its "ease in" by starting from a standstill.
///
/// `EaseOutCubic` leaves its start at full speed and coasts into the
/// end, so it is nearly home by `t = 0.9`. `EaseInOutCubic` accelerates
/// out of `t = 0` instead, so it sits *behind* the out-curve across the
/// whole open interval and only arrives as the clock runs out. Both
/// reach `1.0` at `t = 1.0`; what differs is how they get there, and
/// that is why §2.6 gives the 240ms scrim fade the in-out curve and the
/// 100/180ms pointer changes the out-curve.
#[test]
fn test_the_two_cubic_curves_are_distinct_at_both_ends() {
    let out_early = Easing::EaseOutCubic.apply(0.1);
    let in_out_early = Easing::EaseInOutCubic.apply(0.1);
    let out_late = Easing::EaseOutCubic.apply(0.9);
    let in_out_late = Easing::EaseInOutCubic.apply(0.9);

    assert!(
        in_out_early < out_early,
        "EaseInOutCubic must ease *in*: f(0.1) = {in_out_early} must trail EaseOutCubic's {out_early}"
    );
    assert!(
        in_out_early < 0.01,
        "EaseInOutCubic must barely have moved at t = 0.1, got {in_out_early}"
    );
    assert!(
        out_late > 0.99,
        "EaseOutCubic must be essentially home by t = 0.9, got {out_late}"
    );
    assert!(
        in_out_late < out_late,
        "EaseInOutCubic must still be catching up at t = 0.9: {in_out_late} vs {out_late}"
    );

    // The opening rates are the real difference: the out-curve leaves at
    // three times the average rate, the in-out curve leaves at rest.
    let step = 0.001_f32;
    let out_opening_rate = Easing::EaseOutCubic.apply(step) / step;
    let in_out_opening_rate = Easing::EaseInOutCubic.apply(step) / step;
    assert!(
        out_opening_rate > 2.9,
        "EaseOutCubic must leave its start at ~3x the average rate, got {out_opening_rate}"
    );
    assert!(
        in_out_opening_rate < 0.01,
        "EaseInOutCubic must leave its start at rest, got {in_out_opening_rate}"
    );

    // And both come to rest at the far end, which is the part they share.
    let out_closing_rate = (1.0 - Easing::EaseOutCubic.apply(1.0 - step)) / step;
    let in_out_closing_rate = (1.0 - Easing::EaseInOutCubic.apply(1.0 - step)) / step;
    assert!(
        out_closing_rate < 0.01,
        "EaseOutCubic must arrive at rest, got {out_closing_rate}"
    );
    assert!(
        in_out_closing_rate < 0.01,
        "EaseInOutCubic must arrive at rest, got {in_out_closing_rate}"
    );
}

/// `Linear` is the identity on the interval.
#[test]
fn test_linear_is_the_identity() {
    const STEPS: u32 = 100;
    for step in 0..=STEPS {
        let t = step as f32 / STEPS as f32;
        assert!(
            (Easing::Linear.apply(t) - t).abs() < EPS,
            "Linear({t}) must equal {t}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. The reduced-motion contract
// ---------------------------------------------------------------------------

/// Under `reduced_motion`, **every** transition rung resolves to
/// `time.instant` — the mandatory fallback §2.6 states outright.
#[test]
fn test_reduced_motion_falls_back_to_instant_for_every_transition() {
    for (name, rung) in rung_rows() {
        if !rung.is_transition() {
            continue;
        }
        assert_eq!(
            resolve_ms(rung, true),
            INSTANT.ms,
            "{name} must resolve to time.instant (0ms) under reduced_motion, got {}",
            resolve_ms(rung, true)
        );
        assert_eq!(
            resolve_ms(rung, true),
            0,
            "{name} under reduced_motion must be exactly 0ms"
        );
    }
}

/// Without the flag, every rung resolves to its own duration — the
/// fallback must not leak into normal motion.
#[test]
fn test_full_motion_resolves_to_the_rungs_own_duration() {
    for (name, rung) in rung_rows() {
        assert_eq!(
            resolve_ms(rung, false),
            rung.ms,
            "{name} without reduced_motion must keep its own {}ms, got {}",
            rung.ms,
            resolve_ms(rung, false)
        );
    }
}

/// Under `reduced_motion`, **no timer rung is touched**.
///
/// This is the direction that is easy to get wrong and expensive to get
/// wrong: zeroing `time.debounce.live` would fire a provider request on
/// every keystroke, and zeroing `time.delay.tooltip_open` would make a
/// tooltip appear on cursor transit — an accessibility setting that
/// produces *more* visual noise than the one it replaced.
#[test]
fn test_reduced_motion_never_shortens_a_timer() {
    for (name, rung) in rung_rows() {
        if rung.is_transition() {
            continue;
        }
        assert_eq!(
            resolve_ms(rung, true),
            rung.ms,
            "{name} is a timer, not motion: reduced_motion must leave its {}ms alone, got {}",
            rung.ms,
            resolve_ms(rung, true)
        );
        assert!(
            resolve_ms(rung, true) > 0,
            "{name} must never collapse to 0ms under reduced_motion"
        );
    }
}

/// The fallback is idempotent and total: resolving an already-resolved
/// value changes nothing further.
#[test]
fn test_resolve_is_idempotent() {
    for rung in DurationToken::ALL {
        let once = resolve_ms(rung, true);
        let twice_rung = DurationToken {
            name: rung.name,
            ms: once,
            easing: rung.easing,
            alias: rung.alias,
        };
        assert_eq!(
            resolve_ms(twice_rung, true),
            once,
            "{} must resolve to the same value when re-resolved",
            rung.name
        );
    }
}

/// Under reduced motion a transition is **finished**, not slow:
/// `eased_progress` returns `1.0` on the first call rather than
/// dividing by the zero duration the fallback produced.
#[test]
fn test_eased_progress_under_reduced_motion_is_immediately_complete() {
    for (name, rung) in rung_rows() {
        if !rung.is_transition() {
            continue;
        }
        assert_eq!(
            eased_progress(rung, 0, true),
            1.0,
            "{name} under reduced_motion must report complete at 0ms elapsed, got {}",
            eased_progress(rung, 0, true)
        );
        assert_eq!(
            eased_progress(rung, 12_345, true),
            1.0,
            "{name} under reduced_motion must report complete at any elapsed time"
        );
    }
}

/// `eased_progress` reaches exactly `1.0` at the rung's own duration and
/// never exceeds it past the end.
#[test]
fn test_eased_progress_reaches_one_at_the_end_of_the_rung() {
    for (name, rung) in rung_rows() {
        if rung.ms == 0 {
            continue;
        }
        assert_eq!(
            eased_progress(rung, 0, false),
            0.0,
            "{name} must report 0.0 at 0ms elapsed"
        );
        assert_eq!(
            eased_progress(rung, rung.ms, false),
            1.0,
            "{name} must report 1.0 at exactly its own {}ms",
            rung.ms
        );
        assert_eq!(
            eased_progress(rung, rung.ms * 4, false),
            1.0,
            "{name} must clamp to 1.0 well past its own duration"
        );
    }
}

/// `eased_progress` is monotonically non-decreasing and stays in range
/// across a rung's full duration.
#[test]
fn test_eased_progress_is_monotonic_and_in_range() {
    for (name, rung) in rung_rows() {
        if rung.ms == 0 {
            continue;
        }
        let mut previous = eased_progress(rung, 0, false);
        for elapsed in 1..=rung.ms {
            let value = eased_progress(rung, elapsed, false);
            assert!(
                (0.0..=1.0).contains(&value),
                "{name} at {elapsed}ms gave {value}, outside [0.0, 1.0]"
            );
            assert!(
                value >= previous - EPS,
                "{name} retreats between {prev_ms}ms and {elapsed}ms ({previous} -> {value})",
                prev_ms = elapsed - 1
            );
            previous = value;
        }
    }
}

/// A timer rung's progress is linear, because it has no curve and
/// `easing_or_linear` supplies the identity rather than inventing one.
#[test]
fn test_timer_rung_progress_is_linear() {
    let rung = DEBOUNCE_LIVE;
    assert!(!rung.is_transition());
    assert_eq!(rung.easing_or_linear(), Easing::Linear);
    let half = eased_progress(rung, rung.ms / 2, false);
    assert!(
        (half - 0.5).abs() < EPS,
        "a timer's progress at half its window must be 0.5, got {half}"
    );
    let quarter = eased_progress(rung, rung.ms / 4, false);
    assert!(
        (quarter - 0.25).abs() < EPS,
        "a timer's progress at a quarter of its window must be 0.25, got {quarter}"
    );
}

/// `easing_or_linear` returns the rung's own curve where it has one, and
/// never invents a curve for a rung that has none.
#[test]
fn test_easing_or_linear_returns_the_rungs_own_curve() {
    for (name, rung) in rung_rows() {
        match rung.easing {
            Some(curve) => assert_eq!(
                rung.easing_or_linear(),
                curve,
                "{name} must report its own curve"
            ),
            None => assert_eq!(
                rung.easing_or_linear(),
                Easing::Linear,
                "{name} has no curve and must fall back to Linear"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Cross-references to the constants that predate this module
// ---------------------------------------------------------------------------

/// `time.debounce.live` **must equal** the autosuggest debounce.
///
/// §2.6 states the equality outright — "`DEBOUNCE_MS = 200`" — so this
/// is a requirement, not a coincidence. The two numbers live in
/// different modules and nothing but this assertion stops them drifting
/// apart into two different debounce windows.
#[test]
fn test_debounce_live_equals_the_autosuggest_debounce() {
    assert_eq!(
        DEBOUNCE_LIVE.ms, DEBOUNCE_MS,
        "time.debounce.live ({}) and auto_suggest_model::DEBOUNCE_MS ({}) must be equal",
        DEBOUNCE_LIVE.ms, DEBOUNCE_MS
    );
    assert_eq!(DEBOUNCE_MS, 200, "the autosuggest debounce must be 200ms");
}

/// `time.delay.tooltip_open` agrees with the tooltip's existing
/// `HOVER_DELAY`.
#[test]
fn test_delay_tooltip_open_equals_the_existing_hover_delay() {
    assert_eq!(
        DELAY_TOOLTIP_OPEN.ms, HOVER_DELAY,
        "time.delay.tooltip_open ({}) and tooltip::HOVER_DELAY ({}) must be equal",
        DELAY_TOOLTIP_OPEN.ms, HOVER_DELAY
    );
    assert_eq!(HOVER_DELAY, 400, "the hover delay must be 400ms");
}

/// The keyboard-focus delay is `time.instant` — the `0ms` the same
/// §2.6 row attaches to `time.delay.tooltip_open`.
#[test]
fn test_keyboard_focus_delay_is_time_instant() {
    assert_eq!(
        KEYBOARD_FOCUS_DELAY, INSTANT.ms,
        "tooltip::KEYBOARD_FOCUS_DELAY ({}) must be time.instant ({}ms)",
        KEYBOARD_FOCUS_DELAY, INSTANT.ms
    );
    assert_eq!(
        KEYBOARD_FOCUS_DELAY, 0,
        "the keyboard focus delay must be 0ms"
    );
}

/// `time.toast.auto_dismiss` agrees with the notification default.
///
/// The consumer keeps a `u32` because that is the width its whole
/// auto-dismiss API is cut in; the equality is asserted widened to the
/// token's `u64` so a future retune past `u32::MAX` cannot truncate
/// silently through the re-export.
#[test]
fn test_toast_auto_dismiss_equals_the_notification_default() {
    assert_eq!(
        TOAST_AUTO_DISMISS.ms, DEFAULT_AUTO_DISMISS_MS as u64,
        "time.toast.auto_dismiss ({}) and notification::DEFAULT_AUTO_DISMISS_MS ({}) must be equal",
        TOAST_AUTO_DISMISS.ms, DEFAULT_AUTO_DISMISS_MS
    );
    assert_eq!(
        DEFAULT_AUTO_DISMISS_MS, 5_000,
        "the default auto-dismiss window must be 5000ms"
    );
    assert!(
        TOAST_AUTO_DISMISS.ms <= u32::MAX as u64,
        "time.toast.auto_dismiss must stay inside u32 or the u32 re-export truncates"
    );
}

/// Every alias §2.6 prints in parentheses is recorded, on the right
/// rung, and resolves to the same duration as its canonical name.
///
/// The aliases are names, not extra rungs: there are ten rungs and two
/// alternate spellings, never twelve rungs.
#[test]
fn test_the_two_spec_aliases_are_recorded_on_the_right_rungs() {
    let aliased: Vec<(&str, &str)> = rung_rows()
        .iter()
        .filter_map(|(name, rung)| rung.alias().map(|alias| (*name, alias)))
        .collect();
    assert_eq!(
        aliased,
        vec![
            ("time.delay.submenu", "time.menu.aim_grace"),
            ("time.delay.tooltip_open", "time.tooltip.hover_delay"),
        ],
        "exactly the two aliases §2.6 prints must be recorded, on the rungs it attaches them to"
    );
    for (canonical, alias) in &aliased {
        assert_ne!(
            canonical, alias,
            "an alias must not equal its canonical name"
        );
        let rung = DurationToken::ALL
            .iter()
            .find(|r| r.name == *canonical)
            .unwrap_or_else(|| panic!("{canonical} must exist"));
        assert_eq!(rung.alias(), Some(*alias), "{canonical} must carry {alias}");
    }
    // No alias collides with a canonical rung name.
    let canonical_names: Vec<&str> = rung_rows().iter().map(|(name, _)| *name).collect();
    for (_, alias) in &aliased {
        assert!(
            !canonical_names.contains(alias),
            "the alias {alias} must not also be a canonical rung name"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. The no-clock-read rule
// ---------------------------------------------------------------------------

/// `token/time.rs` reads no clock, and this test is what keeps it that
/// way.
///
/// The rule is the crate's own hard line: time enters as a value, so a
/// token module that called a clock would break headless determinism for
/// every consumer. A doc comment claiming the rule is not enforcement,
/// so this reads the module's own source with comments and strings
/// redacted — the same redaction `token_lint` uses, so prose about
/// clocks does not trip the scan — and asserts no clock-reading
/// identifier appears in the code.
#[test]
fn test_time_module_reads_no_clock() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/token/time.rs");
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {path} to verify it: {error}"));
    let code = redact_comments_and_strings(&source);

    let forbidden: [&str; 7] = [
        "Instant",
        "SystemTime",
        "std::time",
        "chrono",
        ".now()",
        ".elapsed()",
        "gettimeofday",
    ];
    for needle in forbidden {
        assert!(
            !code.contains(needle),
            "token/time.rs must not read a clock, but its code contains {needle:?}"
        );
    }
    // The scan must actually be looking: the file is non-empty and the
    // redaction left real code behind, so a silently-broken scan cannot
    // pass by finding nothing.
    assert!(
        code.contains("pub const fn resolve_ms"),
        "the redacted scan must still contain the resolver, or the scan is not working"
    );
    assert!(
        code.contains("pub fn apply"),
        "the redacted scan must still contain the easing function"
    );
    assert!(
        source.len() > 1_000,
        "the source read for the scan looks wrong ({} bytes)",
        source.len()
    );
}

/// The module doc's own claim — that this module has no import to forget
/// — is true: `time.rs` declares no `use` at all.
#[test]
fn test_time_module_has_no_imports() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/token/time.rs");
    let source = std::fs::read_to_string(path).expect("token/time.rs must be readable");
    let code = redact_comments_and_strings(&source);
    let use_lines: Vec<&str> = code
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use "))
        .collect();
    assert!(
        use_lines.is_empty(),
        "token/time.rs must import nothing to read a clock with, found {use_lines:?}"
    );
}
