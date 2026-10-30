//! Tests for [`state`] — the interaction state machine behind
//! `UIF-ATOM-001`, `UIF-ATOM-002` and `UIF-VER-003`.
//!
//! Four groups, in order:
//!
//! 1. **The truth table.** All 2^5 = 32 combinations of [`InteractionInput`],
//!    each paired with the [`VisualState`] [`resolve`] must produce and the rule
//!    that fired. This is the authoritative statement of the precedence chain.
//!    It is checked for completeness and uniqueness first, so a typo that
//!    duplicates or drops a row fails loudly rather than silently shrinking the
//!    sweep.
//! 2. **The precedence laws**, one test each — disabled dominance, invalid
//!    over hover/press/focus, pressed over hovered, hover over rest. These
//!    restate the interesting parts of the table as properties, so a failure
//!    names the law that broke rather than row 23 of a table.
//! 3. **Focus orthogonality** — the property the two-part model exists to
//!    preserve — plus `ring_visible` agreement across the whole table.
//! 4. **Snapshot vocabulary** and **purity**.
//!
//! # What the table pins that a flat enum could not
//!
//! Two rows exist only because the state is `base` + `focused` rather than one
//! six-value enum:
//!
//! | Row | Resolves to | Spec cell |
//! |---|---|---|
//! | enabled + focused | `Rest` + ring | §3.1 *Keyboard Focused* — "Rest + 2px … focus ring" |
//! | enabled + hovered + focused | `Hovered` + ring | §4.9.2 *Keyboard Focused* — `surface.hover` fill **and** an accent outline, at once |
//!
//! Two more are combinations the spec never snapshots at all — `Hovered +
//! focused` and `Pressed + focused`. They are named rather than hidden; see
//! `test_snapshot_label_maps_every_resolvable_state`.

use super::*;

/// Shorthand so each of the 32 truth-table rows fits on one line and the
/// columns stay aligned. `T` is `true`, `F` is `false`, nothing more.
const T: bool = true;
/// Inverse of [`T`].
const F: bool = false;

/// Build an input from the five raw signals, in the order the resolver reads
/// them: enabled, hovered, pressed, focused, invalid.
const fn input(
    enabled: bool,
    hovered: bool,
    pressed: bool,
    focused: bool,
    invalid: bool,
) -> InteractionInput {
    InteractionInput {
        enabled,
        hovered,
        pressed,
        focused,
        invalid,
    }
}

/// Build a resolved state directly, for exercising [`VisualState`]'s API over
/// states `resolve` itself will not hand back.
const fn vs(base: BaseState, focused: bool) -> VisualState {
    VisualState { base, focused }
}

/// One row of the authoritative truth table.
struct Row {
    /// The raw observations fed to [`resolve`].
    input: InteractionInput,
    /// The base state the resolver must produce.
    base: BaseState,
    /// Whether the focus ring must be drawn.
    ring: bool,
    /// The rule that fired — printed when the row fails, so a red test says
    /// which law broke instead of dumping two structs and leaving the reader
    /// to diff them.
    rule: &'static str,
}

/// Build a truth-table row from the grouped observations, the expected
/// outcome and the rule that fired.
const fn row(input: InteractionInput, base: BaseState, ring: bool, rule: &'static str) -> Row {
    Row {
        input,
        base,
        ring,
        rule,
    }
}

/// The authoritative truth table: every one of the 2^5 = 32 input
/// combinations, in binary counting order with `enabled` as the high bit.
///
/// ```text
/// enabled hovered pressed focused invalid │ base      ring │ rule that fired
/// ```
const TRUTH_TABLE: [Row; 32] = [
    // ── enabled = false ─────────────────────────────────────────────
    // The disabled skin is absolute, and so is the absence of a ring.
    row(
        input(F, F, F, F, F),
        BaseState::Disabled,
        false,
        "not enabled, nothing else observed",
    ),
    row(
        input(F, F, F, F, T),
        BaseState::Disabled,
        false,
        "disabled beats invalid",
    ),
    row(
        input(F, F, F, T, F),
        BaseState::Disabled,
        false,
        "disabled kills the ring",
    ),
    row(
        input(F, F, F, T, T),
        BaseState::Disabled,
        false,
        "disabled beats focus and invalid",
    ),
    row(
        input(F, F, T, F, F),
        BaseState::Disabled,
        false,
        "disabled beats a press",
    ),
    row(
        input(F, F, T, F, T),
        BaseState::Disabled,
        false,
        "disabled beats press and invalid",
    ),
    row(
        input(F, F, T, T, F),
        BaseState::Disabled,
        false,
        "disabled beats press and ring",
    ),
    row(
        input(F, F, T, T, T),
        BaseState::Disabled,
        false,
        "disabled beats press, ring, invalid",
    ),
    row(
        input(F, T, F, F, F),
        BaseState::Disabled,
        false,
        "disabled beats hover",
    ),
    row(
        input(F, T, F, F, T),
        BaseState::Disabled,
        false,
        "disabled beats hover and invalid",
    ),
    row(
        input(F, T, F, T, F),
        BaseState::Disabled,
        false,
        "disabled beats hover and ring",
    ),
    row(
        input(F, T, F, T, T),
        BaseState::Disabled,
        false,
        "disabled beats hover, ring, invalid",
    ),
    row(
        input(F, T, T, F, F),
        BaseState::Disabled,
        false,
        "disabled beats hover and press",
    ),
    row(
        input(F, T, T, F, T),
        BaseState::Disabled,
        false,
        "disabled beats all four at once",
    ),
    row(
        input(F, T, T, T, F),
        BaseState::Disabled,
        false,
        "disabled beats hover, press, ring",
    ),
    row(
        input(F, T, T, T, T),
        BaseState::Disabled,
        false,
        "disabled beats every signal set",
    ),
    // ── enabled = true ──────────────────────────────────────────────
    // The chain: Invalid > Pressed > Hovered > Rest, ring resolved apart.
    row(
        input(T, F, F, F, F),
        BaseState::Rest,
        false,
        "nothing observed: the floor",
    ),
    row(
        input(T, F, F, F, T),
        BaseState::Invalid,
        false,
        "invalid alone",
    ),
    row(
        input(T, F, F, T, F),
        BaseState::Rest,
        true,
        "keyboard focus only: rest + ring",
    ),
    row(
        input(T, F, F, T, T),
        BaseState::Invalid,
        true,
        "invalid outranks focus; ring lives",
    ),
    row(
        input(T, F, T, F, F),
        BaseState::Pressed,
        false,
        "press without hover is still press",
    ),
    row(
        input(T, F, T, F, T),
        BaseState::Invalid,
        false,
        "invalid outranks a press",
    ),
    row(
        input(T, F, T, T, F),
        BaseState::Pressed,
        true,
        "press and ring coexist",
    ),
    row(
        input(T, F, T, T, T),
        BaseState::Invalid,
        true,
        "invalid outranks press; ring lives",
    ),
    row(
        input(T, T, F, F, F),
        BaseState::Hovered,
        false,
        "hover alone",
    ),
    row(
        input(T, T, F, F, T),
        BaseState::Invalid,
        false,
        "invalid outranks hover",
    ),
    row(
        input(T, T, F, T, F),
        BaseState::Hovered,
        true,
        "§4.9.2 keyboard focused: hover+ring",
    ),
    row(
        input(T, T, F, T, T),
        BaseState::Invalid,
        true,
        "invalid outranks hover and focus",
    ),
    row(
        input(T, T, T, F, F),
        BaseState::Pressed,
        false,
        "pressed outranks hovered",
    ),
    row(
        input(T, T, T, F, T),
        BaseState::Invalid,
        false,
        "invalid outranks hover and press",
    ),
    row(
        input(T, T, T, T, F),
        BaseState::Pressed,
        true,
        "press beats hover; ring orthogonal",
    ),
    row(
        input(T, T, T, T, T),
        BaseState::Invalid,
        true,
        "invalid wins; ring survives all",
    ),
];

/// The five observations packed into a 5-bit key: `ehpfi`, `enabled` at bit 4
/// down to `invalid` at bit 0.
fn key(input: &InteractionInput) -> u8 {
    (u8::from(input.enabled) << 4)
        | (u8::from(input.hovered) << 3)
        | (u8::from(input.pressed) << 2)
        | (u8::from(input.focused) << 1)
        | u8::from(input.invalid)
}

/// Every base state, for exhaustive sweeps over the resolved type.
const ALL_BASES: [BaseState; 5] = [
    BaseState::Rest,
    BaseState::Hovered,
    BaseState::Pressed,
    BaseState::Disabled,
    BaseState::Invalid,
];

// ---------------------------------------------------------------------------
// The truth table itself
// ---------------------------------------------------------------------------

#[test]
fn test_truth_table_covers_every_combination_exactly_once() {
    // A table that silently lost or duplicated a row would still pass the
    // sweep below while quietly testing less than it claims. Count the input
    // patterns so "all 32" means all 32.
    let mut seen = [0u16; 32];
    for case in &TRUTH_TABLE {
        seen[key(&case.input) as usize] += 1;
    }

    assert_eq!(
        TRUTH_TABLE.len(),
        32,
        "five booleans must produce 2^5 = 32 rows"
    );
    for (pattern, count) in seen.iter().enumerate() {
        assert_eq!(
            *count, 1,
            "input pattern {pattern:05b} (enabled hovered pressed focused invalid) \
             appears {count} times in the truth table"
        );
    }
}

#[test]
fn test_resolve_matches_the_truth_table() {
    for case in &TRUTH_TABLE {
        let actual = resolve(case.input);
        assert_eq!(
            actual,
            vs(case.base, case.ring),
            "rule `{}` did not fire for input {:?}",
            case.rule,
            case.input,
        );
    }
}

// ---------------------------------------------------------------------------
// The precedence laws, restated as properties
// ---------------------------------------------------------------------------

#[test]
fn test_disabled_dominance_is_total_over_every_other_signal() {
    // Not merely "disabled wins the base" — the ring goes too. A control
    // that cannot take interaction cannot hold keyboard focus either, so a
    // ring on it would advertise an affordance that cannot be used.
    let mut checked = 0;
    for hovered in [F, T] {
        for pressed in [F, T] {
            for focused in [F, T] {
                for invalid in [F, T] {
                    let case = input(F, hovered, pressed, focused, invalid);
                    let resolved = resolve(case);
                    assert_eq!(
                        resolved.base,
                        BaseState::Disabled,
                        "enabled=false must yield Disabled regardless of the \
                         other four signals, got {:?} for {:?}",
                        resolved,
                        case,
                    );
                    assert!(
                        !resolved.focused,
                        "a disabled control cannot hold keyboard focus, so it \
                         must draw no ring: got {:?} for {:?}",
                        resolved, case,
                    );
                    assert!(
                        !resolved.ring_visible(),
                        "ring_visible must agree with the suppressed focus on a \
                         disabled control: got {:?} for {:?}",
                        resolved,
                        case,
                    );
                    checked += 1;
                }
            }
        }
    }

    assert_eq!(
        checked, 16,
        "the sweep must cover every combination of hovered/pressed/focused/invalid"
    );
}

#[test]
fn test_invalid_outranks_hover_press_and_focus_but_not_disabled() {
    // Validation is a property of the data, not of the pointer. Clearing the
    // error visual because the user moved the mouse over the field, or is
    // mid-click on it, would tell them the problem went away when it has not.
    for hovered in [F, T] {
        for pressed in [F, T] {
            for focused in [F, T] {
                let live = resolve(input(T, hovered, pressed, focused, T));
                assert_eq!(
                    live.base,
                    BaseState::Invalid,
                    "invalid must outrank hovered={hovered} pressed={pressed} \
                     focused={focused} on a live control"
                );
                assert_eq!(
                    live.focused, focused,
                    "showing the error must not swallow the focus ring: \
                     hovered={hovered} pressed={pressed} focused={focused}"
                );

                // …but Disabled still sits above it.
                let dead = resolve(input(F, hovered, pressed, focused, T));
                assert_eq!(
                    dead.base,
                    BaseState::Disabled,
                    "invalid must not outrank disabled at hovered={hovered} \
                     pressed={pressed} focused={focused}"
                );
                assert!(
                    !dead.focused,
                    "disabled must still zero the ring even with invalid set"
                );
            }
        }
    }
}

#[test]
fn test_pressed_outranks_hovered() {
    // The pointer is committed, which outranks the mere fact of being over
    // the control — with or without the ring.
    for focused in [F, T] {
        let resolved = resolve(input(T, T, T, focused, F));
        assert_eq!(
            resolved.base,
            BaseState::Pressed,
            "pressed must outrank hovered at focused={focused}, got {:?}",
            resolved,
        );
        assert_eq!(
            resolved.focused, focused,
            "the press must not swallow the ring"
        );
    }
}

#[test]
fn test_pressed_without_hover_resolves_to_pressed() {
    // A real combination, not a missing input: the pointer dragged in from
    // outside and released, or a synthesised event from a test harness or an
    // accessibility layer. The press is the committed fact; `hovered` only
    // records whether the pointer was already inside the bounds. Resolving to
    // Hovered or Rest here would drop press feedback exactly when the user is
    // pressing.
    let bare = resolve(input(T, F, T, F, F));
    assert_eq!(
        bare,
        vs(BaseState::Pressed, false),
        "pressed without hovered must still resolve to Pressed, not fall \
         through to Rest"
    );

    let bare_and_ring = resolve(input(T, F, T, T, F));
    assert_eq!(
        bare_and_ring,
        vs(BaseState::Pressed, true),
        "a drag-in press with keyboard focus must keep both the press and the ring"
    );
}

#[test]
fn test_hover_outranks_rest() {
    for focused in [F, T] {
        let resolved = resolve(input(T, T, F, focused, F));
        assert_eq!(
            resolved.base,
            BaseState::Hovered,
            "hovered must outrank rest at focused={focused}"
        );
    }
}

#[test]
fn test_rest_is_the_floor_of_an_enabled_control() {
    assert_eq!(
        resolve(input(T, F, F, F, F)),
        vs(BaseState::Rest, false),
        "an enabled control with no signals at all rests"
    );
}

#[test]
fn test_default_input_resolves_to_disabled() {
    // The derived Default is all-false, i.e. never told it is enabled. The
    // fail-safe reading wins: show the disabled skin rather than pretending
    // to be live.
    assert_eq!(
        resolve(InteractionInput::default()),
        vs(BaseState::Disabled, false),
        "an all-false input must resolve to Disabled, not Rest"
    );
}

// ---------------------------------------------------------------------------
// Focus orthogonality
// ---------------------------------------------------------------------------

#[test]
fn test_focus_is_orthogonal_to_hover_and_press() {
    // The whole reason the state is two parts rather than one six-value enum:
    // the ring coexists with the pointer states instead of displacing them.
    let cases: [(InteractionInput, BaseState, &str); 4] = [
        (
            input(T, F, F, T, F),
            BaseState::Rest,
            "§3.1 Keyboard Focused — Rest + 2px focus ring",
        ),
        (
            input(T, T, F, T, F),
            BaseState::Hovered,
            "§4.9.2 Keyboard Focused — surface.hover fill AND an accent outline",
        ),
        (
            input(T, T, T, T, F),
            BaseState::Pressed,
            "press committed while keyboard-focused",
        ),
        (
            input(T, F, T, T, F),
            BaseState::Pressed,
            "drag-in press while keyboard-focused",
        ),
    ];

    for (case, expected_base, why) in cases {
        let resolved = resolve(case);
        assert_eq!(resolved.base, expected_base, "{why}: wrong base");
        assert!(
            resolved.focused,
            "{why}: the ring must survive the base, got {:?}",
            resolved,
        );
        assert!(resolved.ring_visible(), "{why}: ring_visible must be true");
    }
}

#[test]
fn test_focus_never_changes_the_base_state() {
    // Flipping `focused` on an otherwise identical live input must change
    // `focused` and nothing else — the formal statement that focus is not a
    // rung in the precedence chain.
    for hovered in [F, T] {
        for pressed in [F, T] {
            for invalid in [F, T] {
                let unfocused = resolve(input(T, hovered, pressed, F, invalid));
                let focused = resolve(input(T, hovered, pressed, T, invalid));
                assert_eq!(
                    unfocused.base, focused.base,
                    "focus must not move the base at hovered={hovered} \
                     pressed={pressed} invalid={invalid}"
                );
                assert!(
                    !unfocused.focused && focused.focused,
                    "focus must be the only difference between the two resolutions"
                );
            }
        }
    }
}

#[test]
fn test_ring_visible_agrees_with_focused_across_the_table() {
    for case in &TRUTH_TABLE {
        let resolved = resolve(case.input);
        assert_eq!(
            resolved.ring_visible(),
            resolved.focused,
            "ring_visible disagrees with the focused field for rule `{}`",
            case.rule,
        );
        assert_eq!(
            resolved.ring_visible(),
            case.ring,
            "ring_visible disagrees with the truth table for rule `{}`",
            case.rule,
        );
    }
}

// ---------------------------------------------------------------------------
// Snapshot vocabulary (UIF-VER-004)
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_label_only_ever_returns_a_spec_name() {
    let mut offenders = Vec::new();
    for case in &TRUTH_TABLE {
        let label = resolve(case.input).snapshot_label();
        if !SNAPSHOT_STATE_NAMES.contains(&label) {
            offenders.push((case.rule, label));
        }
    }

    assert!(
        offenders.is_empty(),
        "snapshot_label left the six-name vocabulary: {offenders:?}"
    );
}

#[test]
fn test_snapshot_label_maps_every_resolvable_state() {
    // All 5 bases × 2 focus values — the whole constructible space of
    // `VisualState`, including the pairs `resolve` will never hand back.
    let cases: [(BaseState, bool, &str, &str); 10] = [
        (BaseState::Rest, F, SNAPSHOT_REST, "plain rest"),
        (
            BaseState::Rest,
            T,
            SNAPSHOT_FOCUSED,
            "§3.1 Keyboard Focused: rest + ring",
        ),
        (BaseState::Hovered, F, SNAPSHOT_HOVERED, "plain hover"),
        (
            BaseState::Hovered,
            T,
            SNAPSHOT_FOCUSED,
            "OFF-MATRIX: the spec snapshots no 'hovered AND focused' row; it is \
             named Focused because §4.9.2's Keyboard Focused row already IS the \
             hover fill with a ring on it",
        ),
        (BaseState::Pressed, F, SNAPSHOT_PRESSED, "plain press"),
        (
            BaseState::Pressed,
            T,
            SNAPSHOT_PRESSED,
            "OFF-MATRIX: press owns the name over the ring, per the ledger's \
             chain ranking Pressed above Focused",
        ),
        (BaseState::Disabled, F, SNAPSHOT_DISABLED, "disabled"),
        (
            BaseState::Disabled,
            T,
            SNAPSHOT_DISABLED,
            "unreachable from resolve, but the base owns the name regardless",
        ),
        (BaseState::Invalid, F, SNAPSHOT_INVALID, "invalid"),
        (
            BaseState::Invalid,
            T,
            SNAPSHOT_INVALID,
            "unreachable from resolve, but the base owns the name regardless",
        ),
    ];

    for (base, focused, expected, why) in cases {
        assert_eq!(
            vs(base, focused).snapshot_label(),
            expected,
            "{why}: {base:?} + focused={focused} must be named {expected}",
        );
    }
}

#[test]
fn test_every_spec_snapshot_name_is_reachable() {
    // A gate that iterates the six names must be able to reach all of them;
    // an unreachable name would make the iteration vacuous.
    for name in SNAPSHOT_STATE_NAMES {
        let reachable = TRUTH_TABLE
            .iter()
            .any(|case| resolve(case.input).snapshot_label() == name);
        assert!(
            reachable,
            "`{name}` is in the spec's snapshot vocabulary but no input \
             resolves to it — the gate would never exercise it"
        );
    }
}

#[test]
fn test_snapshot_state_names_are_the_six_spec_names_in_spec_order() {
    let expected: [&str; 6] = [
        "Rest", "Hovered", "Pressed", "Focused", "Disabled", "Invalid",
    ];
    assert_eq!(
        SNAPSHOT_STATE_NAMES, expected,
        "UIF-VER-004 lists the snapshot states in this order"
    );

    let mut distinct: Vec<&str> = SNAPSHOT_STATE_NAMES.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        6,
        "the six snapshot names must be six distinct strings"
    );
}

// ---------------------------------------------------------------------------
// Purity
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_is_pure_and_holds_no_transition_state() {
    // Interleave: resolve `a`, then a different input, then `a` again. If
    // `resolve` remembered anything between calls, the second `a` would come
    // out differently — which is exactly the implicit-transition failure this
    // gate exists to catch.
    for case in &TRUTH_TABLE {
        let first = resolve(case.input);
        let unrelated = input(
            T,
            !case.input.hovered,
            !case.input.pressed,
            !case.input.focused,
            !case.input.invalid,
        );
        let _distractor = resolve(unrelated);
        let again = resolve(case.input);

        assert_eq!(
            first, again,
            "resolve is not a pure function of its input: rule `{}` resolved \
             differently on the second call",
            case.rule,
        );
        assert_eq!(
            first,
            vs(case.base, case.ring),
            "the repeated call also drifted from the truth table for rule `{}`",
            case.rule,
        );
    }
}

#[test]
fn test_resolved_states_are_distinct_and_usable_as_map_keys() {
    // `VisualState` derives `Hash` + `Eq` so a snapshot harness can key a map
    // by state; `Hovered + focused` and `Pressed + focused` must not collide
    // even though both are off-matrix.
    let mut seen = std::collections::HashSet::new();
    for base in ALL_BASES {
        for focused in [F, T] {
            assert!(
                seen.insert(vs(base, focused)),
                "VisualState {base:?} + focused={focused} collided with another \
                 resolved state — a (Variant × State) map would lose a cell"
            );
        }
    }

    assert_eq!(
        seen.len(),
        10,
        "5 bases × 2 focus values = 10 distinct states"
    );
}
