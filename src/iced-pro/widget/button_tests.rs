//! Tests for the button atom — `UIF-ATOM-001`, plus the `INC-07`
//! contrast audit.
//!
//! Four groups, in order:
//!
//! 1. **The matrix.** Every cell of §3.1 transcribed as an expected
//!    `(background, foreground, border, ring)` quadruple and asserted
//!    against `resolve_style`, so the resolver is checked against the
//!    spec rather than against itself.
//! 2. **Focus, opacity and the unpainted fill.** The rules that are not
//!    plain table lookups: the ring is additive, the pressed fill carries
//!    the spec's 85% alpha, and a transparent background means the
//!    container is what the label sits on.
//! 3. **Geometry.** The §3.1 geometry paragraph, asserted to be the
//!    tokens it is sourced from rather than restated numbers.
//! 4. **The contrast audit (`UIF-VER-001`).** Every foreground and
//!    background combination `resolve_style` can actually paint,
//!    measured with the `INC-05` solver and classified against the
//!    threshold its role carries.
//!
//! # Why the audit exists
//!
//! §3.1 introduces pairings the `INC-05` mandated-pairing table never
//! enumerated — a `text.strong` label on an opaque `status.danger` fill,
//! a `border.strong` on `surface.hover`, an accent ring on a hover
//! fill. Those were not in the token-level gate, so nothing had measured
//! them. This sidecar measures all of them before a widget gets the chance
//! to ship an unreadable label.
//!
//! # The audit's failures
//!
//! Three judged pairs do not clear the rule their role carries. All three
//! are pinned at their measured value rather than waived, and no token was
//! retuned to make them pass — that is a supervisor decision, not an
//! increment-owning one.
//!
//! | Pair | Cell | Measured | Rule | Short by |
//! |---|---|---|---|---|
//! | `text.strong` on `status.danger` | Danger `Hovered` | `3.8753:1` | `4.5:1` | `0.6247` |
//! | `accent.primary` ring on `status.danger` | Danger `Hovered + ring` | `1.1779:1` | `3.0:1` | `1.8221` |
//! | `accent.primary` ring on `status.danger @ 85%` | Danger `Pressed + ring` | `1.0609:1` | `3.0:1` | `1.9391` |
//!
//! The first is the pair the increment brief predicted. Measured, white on
//! the retuned `RGB(255, 28, 2)` is `3.8753:1` — the brief's `~3.88:1`
//! estimate, to two decimals.
//!
//! The other two are the finding the brief did not predict, and they are
//! worse. They exist only because the two-part state model lets a
//! keyboard-focused button *also* be hovered or pressed — combinations
//! §3.1 never snapshots, because it spells its Focused cell as "Rest +
//! ring". Inherit the hover fill instead of Rest, as `Hovered + focused`
//! must, and the `accent.primary` ring lands on a red fill at roughly
//! `1.18:1`. The focus indicator is invisible precisely when the user is
//! doing the thing it exists to report.
//!
//! # Why the pressed fills are measured composited
//!
//! Primary and Danger press at 85% alpha. [`contrast_ratio`] ignores
//! alpha by design, so a naive reading of the Danger pressed label is
//! the *opaque* pairing — `3.8753:1`, a failure that never appears on
//! screen. Laid over the surfaces a button can actually be mounted to,
//! the 85% fill lands darker and the same label reads `4.8427:1` at its
//! worst. The audit measures the painted colour and reports the naive
//! figure alongside it, because the gap between the two is exactly the
//! kind of mistake this gate exists to catch.

use super::*;
use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, composite_over, contrast_ratio, meets_chrome_contrast,
    meets_text_contrast,
};
use crate::token::{shape, size, space};
use iced::Color;

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `3.8753` from `4.5000`.
const RATIO_EPS: f32 = 1e-3;

/// Tolerance on a colour-channel comparison used to name a token.
const CHANNEL_EPS: f32 = 1e-6;

/// The four §3.1 variants.
const VARIANTS: [ButtonVariant; 4] = [
    ButtonVariant::Primary,
    ButtonVariant::Secondary,
    ButtonVariant::Ghost,
    ButtonVariant::Danger,
];

/// The dark palette, taken once so every table reads as token names.
fn palette() -> Palette {
    Palette::dark()
}

/// Every surface token in the elevation ladder, darkest to lightest.
///
/// Also the container set a translucent fill can be mounted over: the
/// audit measures a pressed fill against all seven and judges the worst.
fn all_surfaces() -> [(&'static str, Color); 7] {
    let p = palette();
    [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ]
}

/// A colour as the spec writes it, for assertion messages.
fn rgb8(color: Color) -> String {
    format!(
        "RGB({}, {}, {})",
        (color.r * 255.0).round(),
        (color.g * 255.0).round(),
        (color.b * 255.0).round()
    )
}

/// Do two colours share their channels, ignoring alpha?
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < CHANNEL_EPS
        && (a.g - b.g).abs() < CHANNEL_EPS
        && (a.b - b.b).abs() < CHANNEL_EPS
}

/// Name the token a resolved colour came from.
///
/// Recovered by matching against the palette rather than carried through
/// the resolver, so the audit reports *token names* and a renamed field
/// cannot make a failing row anonymous. A channel match with a different
/// alpha is reported as the token at that opacity — the pressed fills.
fn token_name(color: Color) -> String {
    if color.a == 0.0 {
        return "transparent".to_string();
    }
    let p = palette();
    let table: [(&str, Color); 19] = [
        ("surface.base", p.surface_base),
        ("surface.sunken", p.surface_sunken),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
        ("border.subtle", p.border_subtle),
        ("border.strong", p.border_strong),
        ("accent.primary", p.accent_primary),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.success", p.status_success),
        ("status.warning", p.status_warning),
        ("status.danger", p.status_danger),
        ("surface.scrim", p.surface_scrim),
    ];
    for (name, token) in table {
        if same_channels(color, token) {
            if (color.a - token.a).abs() < CHANNEL_EPS {
                return name.to_string();
            }
            return format!("{name} @ {:.0}%", color.a * 100.0);
        }
    }
    format!("UNMAPPED {}", rgb8(color))
}

/// Every `VisualState` the state machine can actually produce.
///
/// Derived by running all 32 [`InteractionInput`] combinations through
/// [`resolve`] rather than written out, so the audit covers exactly the
/// reachable set. Two consequences worth having: `Disabled + focused` is
/// *not* in here, because `resolve` zeroes the ring on a dead control,
/// and the count is pinned so a change to the state machine shows up
/// here rather than silently shrinking the audit.
fn reachable_states() -> Vec<VisualState> {
    let mut out: Vec<VisualState> = Vec::new();
    for enabled in [false, true] {
        for hovered in [false, true] {
            for pressed in [false, true] {
                for focused in [false, true] {
                    for invalid in [false, true] {
                        let state = resolve(InteractionInput {
                            enabled,
                            hovered,
                            pressed,
                            focused,
                            invalid,
                        });
                        if !out.contains(&state) {
                            out.push(state);
                        }
                    }
                }
            }
        }
    }
    out
}

/// A state's name in audit rows: the base, plus the ring when it is on.
fn state_name(state: VisualState) -> String {
    if state.focused {
        format!("{:?} + ring", state.base)
    } else {
        format!("{:?}", state.base)
    }
}

// ---------------------------------------------------------------------------
// 1 — the §3.1 matrix
// ---------------------------------------------------------------------------

/// One cell of §3.1, transcribed from the spec.
struct Cell {
    variant: ButtonVariant,
    base: BaseState,
    focused: bool,
    background: Color,
    foreground: Color,
    border: Color,
    ring: Color,
}

/// Build a matrix cell.
fn cell(
    variant: ButtonVariant,
    base: BaseState,
    focused: bool,
    background: Color,
    foreground: Color,
    border: Color,
    ring: Color,
) -> Cell {
    Cell {
        variant,
        base,
        focused,
        background,
        foreground,
        border,
        ring,
    }
}

/// The 24 cells of §3.1: four variants across `Rest`, `Hovered`,
/// `Pressed`, `Keyboard Focused` and `Disabled`, plus the `Invalid`
/// mapping the spec does not cover.
///
/// The `Keyboard Focused` cells are written as `Rest + focused`, which is
/// literally how §3.1 spells them ("Rest + 2px … focus ring").
fn matrix_cells() -> Vec<Cell> {
    let p = palette();
    let pressed_accent = Color {
        a: PRESSED_FILL_ALPHA,
        ..p.accent_primary
    };
    let pressed_danger = Color {
        a: PRESSED_FILL_ALPHA,
        ..p.status_danger
    };
    let none = Color::TRANSPARENT;

    vec![
        // ---- Primary ----
        cell(
            ButtonVariant::Primary,
            BaseState::Rest,
            false,
            p.accent_primary,
            p.text_strong,
            p.accent_primary,
            none,
        ),
        cell(
            ButtonVariant::Primary,
            BaseState::Hovered,
            false,
            p.accent_primary,
            p.text_strong,
            p.text_strong,
            none,
        ),
        cell(
            ButtonVariant::Primary,
            BaseState::Pressed,
            false,
            pressed_accent,
            p.text_strong,
            p.accent_primary,
            none,
        ),
        cell(
            ButtonVariant::Primary,
            BaseState::Rest,
            true,
            p.accent_primary,
            p.text_strong,
            p.accent_primary,
            p.text_strong,
        ),
        cell(
            ButtonVariant::Primary,
            BaseState::Disabled,
            false,
            p.surface_raised,
            p.text_disabled,
            p.border_subtle,
            none,
        ),
        cell(
            ButtonVariant::Primary,
            BaseState::Invalid,
            false,
            p.accent_primary,
            p.text_strong,
            p.accent_primary,
            none,
        ),
        // ---- Secondary ----
        cell(
            ButtonVariant::Secondary,
            BaseState::Rest,
            false,
            p.surface_raised,
            p.text_primary,
            p.border_strong,
            none,
        ),
        cell(
            ButtonVariant::Secondary,
            BaseState::Hovered,
            false,
            p.surface_hover,
            p.text_strong,
            p.border_strong,
            none,
        ),
        cell(
            ButtonVariant::Secondary,
            BaseState::Pressed,
            false,
            p.surface_active,
            p.text_strong,
            p.accent_primary,
            none,
        ),
        cell(
            ButtonVariant::Secondary,
            BaseState::Rest,
            true,
            p.surface_raised,
            p.text_primary,
            p.border_strong,
            p.accent_primary,
        ),
        cell(
            ButtonVariant::Secondary,
            BaseState::Disabled,
            false,
            p.surface_base,
            p.text_disabled,
            p.border_subtle,
            none,
        ),
        cell(
            ButtonVariant::Secondary,
            BaseState::Invalid,
            false,
            p.surface_raised,
            p.text_primary,
            p.border_strong,
            none,
        ),
        // ---- Ghost ----
        cell(
            ButtonVariant::Ghost,
            BaseState::Rest,
            false,
            none,
            p.text_primary,
            none,
            none,
        ),
        cell(
            ButtonVariant::Ghost,
            BaseState::Hovered,
            false,
            p.surface_hover,
            p.text_strong,
            p.border_subtle,
            none,
        ),
        cell(
            ButtonVariant::Ghost,
            BaseState::Pressed,
            false,
            p.surface_active,
            p.text_strong,
            p.border_strong,
            none,
        ),
        cell(
            ButtonVariant::Ghost,
            BaseState::Rest,
            true,
            none,
            p.text_primary,
            none,
            p.accent_primary,
        ),
        cell(
            ButtonVariant::Ghost,
            BaseState::Disabled,
            false,
            none,
            p.text_disabled,
            none,
            none,
        ),
        cell(
            ButtonVariant::Ghost,
            BaseState::Invalid,
            false,
            none,
            p.text_primary,
            none,
            none,
        ),
        // ---- Danger ----
        cell(
            ButtonVariant::Danger,
            BaseState::Rest,
            false,
            p.surface_raised,
            p.status_danger,
            p.status_danger,
            none,
        ),
        cell(
            ButtonVariant::Danger,
            BaseState::Hovered,
            false,
            p.status_danger,
            p.text_strong,
            p.text_strong,
            none,
        ),
        cell(
            ButtonVariant::Danger,
            BaseState::Pressed,
            false,
            pressed_danger,
            p.text_strong,
            p.status_danger,
            none,
        ),
        cell(
            ButtonVariant::Danger,
            BaseState::Rest,
            true,
            p.surface_raised,
            p.status_danger,
            p.status_danger,
            p.accent_primary,
        ),
        cell(
            ButtonVariant::Danger,
            BaseState::Disabled,
            false,
            p.surface_base,
            p.text_disabled,
            p.border_subtle,
            none,
        ),
        cell(
            ButtonVariant::Danger,
            BaseState::Invalid,
            false,
            p.surface_raised,
            p.status_danger,
            p.status_danger,
            none,
        ),
    ]
}

#[test]
fn test_matrix_covers_every_variant_and_state() {
    let cells = matrix_cells();
    assert_eq!(
        cells.len(),
        24,
        "§3.1 has 4 variants x 5 spec cells + 4 Invalid mappings = 24"
    );
    for variant in VARIANTS {
        for base in [
            BaseState::Rest,
            BaseState::Hovered,
            BaseState::Pressed,
            BaseState::Disabled,
            BaseState::Invalid,
        ] {
            assert!(
                cells
                    .iter()
                    .any(|c| c.variant == variant && c.base == base && !c.focused),
                "{variant:?} x {base:?} is missing from the matrix table"
            );
        }
        assert!(
            cells
                .iter()
                .any(|c| c.variant == variant && c.base == BaseState::Rest && c.focused),
            "{variant:?} x Rest + ring (the Keyboard Focused cell) is missing"
        );
    }
}

#[test]
fn test_every_matrix_cell_resolves_to_the_specified_paint() {
    for c in matrix_cells() {
        let state = VisualState {
            base: c.base,
            focused: c.focused,
        };
        let style = resolve_style(c.variant, state, &palette());
        let label = format!("{:?} x {}", c.variant, state_name(state));

        assert_eq!(
            style.background,
            c.background,
            "{label}: background is {}, the spec says {}",
            rgb8(style.background),
            rgb8(c.background),
        );
        assert_eq!(
            style.foreground,
            c.foreground,
            "{label}: foreground is {}, the spec says {}",
            rgb8(style.foreground),
            rgb8(c.foreground),
        );
        assert_eq!(
            style.border_color,
            c.border,
            "{label}: border is {}, the spec says {}",
            rgb8(style.border_color),
            rgb8(c.border),
        );

        let expected_ring_width = if c.focused { FOCUS_RING_WIDTH } else { 0.0 };
        assert_eq!(
            style.focus_ring_width, expected_ring_width,
            "{label}: ring width must follow `focused`"
        );
        if c.focused {
            assert_eq!(
                style.focus_ring_color,
                c.ring,
                "{label}: ring colour is {}, the spec says {}",
                rgb8(style.focus_ring_color),
                rgb8(c.ring),
            );
        } else {
            assert_eq!(
                style.focus_ring_color.a, 0.0,
                "{label}: an unfocused style must carry no ring colour"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2 — focus, opacity, the unpainted fill
// ---------------------------------------------------------------------------

#[test]
fn test_focus_ring_is_additive_and_never_replaces_the_base_paint() {
    let p = palette();

    // The rule the two-part state model exists to express: `focused` adds
    // a ring and leaves the base alone. Hovered + focused must keep the
    // hover fill, not fall back to Rest.
    let hovered_only = resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Hovered,
            focused: false,
        },
        &p,
    );
    let hovered_focused = resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Hovered,
            focused: true,
        },
        &p,
    );

    assert_eq!(
        hovered_focused.background, hovered_only.background,
        "focus must not change the background — §3.1 says 'Rest + ring', \
         so the base survives the ring"
    );
    assert_eq!(
        hovered_focused.foreground, hovered_only.foreground,
        "focus must not change the foreground"
    );
    assert_eq!(
        hovered_focused.border_color, hovered_only.border_color,
        "focus must not change the border"
    );
    assert_eq!(
        hovered_focused.background, p.surface_hover,
        "and the base that survives is the hover fill, not Rest"
    );
    assert!(hovered_focused.focus_ring_width > 0.0);
    assert_eq!(hovered_only.focus_ring_width, 0.0);

    // Same for a pressed control: the press repaints, the ring rides on top.
    let pressed_focused = resolve_style(
        ButtonVariant::Danger,
        VisualState {
            base: BaseState::Pressed,
            focused: true,
        },
        &p,
    );
    assert_eq!(
        pressed_focused.background.a, PRESSED_FILL_ALPHA,
        "a pressed Danger button keeps its 85% fill even while focused"
    );
    assert_eq!(pressed_focused.focus_ring_color, p.accent_primary);
}

#[test]
fn test_focus_ring_colour_is_variant_specific() {
    let p = palette();
    let focused = VisualState {
        base: BaseState::Rest,
        focused: true,
    };

    // Primary's fill is already the accent, so an accent ring on it would
    // be invisible — §3.1 gives it a text.strong ring instead.
    assert_eq!(
        resolve_style(ButtonVariant::Primary, focused, &p).focus_ring_color,
        p.text_strong
    );
    for variant in [
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ] {
        assert_eq!(
            resolve_style(variant, focused, &p).focus_ring_color,
            p.accent_primary,
            "{variant:?} takes the accent focus border §3.1 specifies"
        );
    }
}

#[test]
fn test_focus_ring_is_twice_the_resting_border_width() {
    assert_eq!(FOCUS_RING_WIDTH, BORDER_WIDTH * 2.0);
    assert_eq!(FOCUS_RING_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
}

#[test]
fn test_disabled_control_never_gets_a_ring_through_the_state_machine() {
    let p = palette();
    // `resolve` zeroes `focused` for a dead control, so no input at all
    // can put a ring on a disabled button. The resolver trusts that and
    // asks `ring_visible`, so the rule has one owner.
    for variant in VARIANTS {
        for hovered in [false, true] {
            for pressed in [false, true] {
                for focused in [false, true] {
                    for invalid in [false, true] {
                        let state = resolve(InteractionInput {
                            enabled: false,
                            hovered,
                            pressed,
                            focused,
                            invalid,
                        });
                        let style = resolve_style(variant, state, &p);
                        assert_eq!(
                            style.focus_ring_width, 0.0,
                            "{variant:?} disabled must advertise no focus ring, \
                             whatever the pointer or keyboard were doing"
                        );
                        assert_eq!(state.base, BaseState::Disabled);
                    }
                }
            }
        }
    }
}

#[test]
fn test_pressed_fill_uses_the_spec_opacity_and_keeps_its_channels() {
    let p = palette();
    let pressed = VisualState {
        base: BaseState::Pressed,
        focused: false,
    };

    let primary = resolve_style(ButtonVariant::Primary, pressed, &p);
    assert_eq!(primary.background.a, PRESSED_FILL_ALPHA);
    assert_eq!(
        (
            primary.background.r,
            primary.background.g,
            primary.background.b
        ),
        (p.accent_primary.r, p.accent_primary.g, p.accent_primary.b),
        "the pressed fill is the token at 85% alpha, not a different colour"
    );

    let danger = resolve_style(ButtonVariant::Danger, pressed, &p);
    assert_eq!(danger.background.a, PRESSED_FILL_ALPHA);
    assert_eq!(
        (
            danger.background.r,
            danger.background.g,
            danger.background.b
        ),
        (p.status_danger.r, p.status_danger.g, p.status_danger.b),
        "the pressed fill is the token at 85% alpha, not a different colour"
    );

    // Only those two variants press translucent; the others use opaque
    // surfaces, because §3.1 only states an opacity for these two.
    for variant in [ButtonVariant::Secondary, ButtonVariant::Ghost] {
        assert_eq!(
            resolve_style(variant, pressed, &p).background.a,
            1.0,
            "{variant:?} presses on an opaque surface"
        );
    }
    assert_eq!(PRESSED_FILL_ALPHA, 0.85);
}

#[test]
fn test_only_ghost_paints_no_background() {
    let p = palette();
    for variant in VARIANTS {
        for state in reachable_states() {
            let style = resolve_style(variant, state, &p);
            let is_ghost_quiet_cell = variant == ButtonVariant::Ghost
                && matches!(
                    state.base,
                    BaseState::Rest | BaseState::Disabled | BaseState::Invalid
                );
            assert_eq!(
                style.is_transparent_background(),
                is_ghost_quiet_cell,
                "{variant:?} x {}: transparency must be exactly the Ghost \
                 rest/disabled cells",
                state_name(state),
            );
        }
    }
}

#[test]
fn test_invalid_paints_exactly_like_rest_because_the_spec_has_no_invalid_cell() {
    let p = palette();
    // §3.1 gives a button no invalid skin. The resolver maps Invalid onto
    // Rest rather than inventing one; this pins the mapping so a future
    // invalid-button requirement is a deliberate edit rather than a
    // surprise when a control starts painting the wrong thing.
    for variant in VARIANTS {
        let rest = resolve_style(
            variant,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
            &p,
        );
        let invalid = resolve_style(
            variant,
            VisualState {
                base: BaseState::Invalid,
                focused: false,
            },
            &p,
        );
        assert_eq!(
            invalid, rest,
            "{variant:?}: Invalid must currently equal Rest — §3.1 defines no \
             invalid cell for a button"
        );
    }
}

#[test]
fn test_resolve_style_is_pure_and_total() {
    let p = palette();
    for variant in VARIANTS {
        for state in reachable_states() {
            let first = resolve_style(variant, state, &p);
            let second = resolve_style(variant, state, &p);
            assert_eq!(
                first, second,
                "resolve_style must be a pure function of its inputs"
            );
            assert!(
                first.background.a.is_finite()
                    && first.foreground.a.is_finite()
                    && first.border_width.is_finite()
                    && first.focus_ring_width.is_finite(),
                "no resolved channel may be NaN or infinite"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — geometry
// ---------------------------------------------------------------------------

#[test]
fn test_button_geometry_is_sourced_from_tokens_not_restated() {
    assert_eq!(HEIGHT_COMPACT, size::CONTROL_SM);
    assert_eq!(HEIGHT_STANDARD, size::CONTROL_MD);
    assert_eq!(HORIZONTAL_PADDING, space::MD);
    assert_eq!(ICON_LABEL_GAP, space::SM);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(FOCUS_RING_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(ICON_ONLY_MIN_WIDTH, size::CONTROL_SM);
    assert_eq!(ICON_ONLY_MIN_HEIGHT, size::CONTROL_SM);
}

/// The icon-only floor clears the hit-target rule in both axes, checked at
/// compile time because both sides are constants.
const _: () = assert!(
    ICON_ONLY_MIN_WIDTH >= size::HIT_TARGET_MIN,
    "icon-only min width must clear the hit-target floor"
);
const _: () = assert!(
    ICON_ONLY_MIN_HEIGHT >= size::HIT_TARGET_MIN,
    "icon-only min height must clear the hit-target floor"
);

/// Ledger gap **G-2**: the atom's `28` floor is the stricter of the two
/// rules and is the one a navbar-class control must satisfy.
const _: () = assert!(
    ICON_ONLY_MIN_WIDTH > size::HIT_TARGET_MIN,
    "the atom rule must be strictly stricter than the general hit-target floor"
);

#[test]
fn test_icon_only_button_clears_the_hit_target_floor_in_both_axes() {
    // The compile-time blocks above are the gate. This keeps the rule in
    // the reported test set; the axes are read through a slice so the
    // assertion is about the pair of them rather than two constants.
    let axes = [ICON_ONLY_MIN_WIDTH, ICON_ONLY_MIN_HEIGHT];
    for axis in axes {
        assert!(axis >= size::HIT_TARGET_MIN);
    }
    assert_eq!(axes.len(), 2);
    assert_eq!(ICON_ONLY_MIN_WIDTH, size::CONTROL_SM);
    assert_eq!(ICON_ONLY_MIN_HEIGHT, size::CONTROL_SM);
}

/// Both button heights clear the hit-target floor, at compile time.
const _: () = assert!(
    HEIGHT_COMPACT >= size::HIT_TARGET_MIN,
    "the compact button row must clear the hit-target floor"
);
const _: () = assert!(
    HEIGHT_STANDARD >= size::HIT_TARGET_MIN,
    "the standard button row must clear the hit-target floor"
);
const _: () = assert!(
    HEIGHT_STANDARD > HEIGHT_COMPACT,
    "the standard row must be taller than the compact one"
);

#[test]
fn test_both_button_heights_clear_the_hit_target_floor() {
    let heights = [HEIGHT_COMPACT, HEIGHT_STANDARD];
    for height in heights {
        assert!(height >= size::HIT_TARGET_MIN);
    }
    // The strict ordering between the two rungs is the const block's job;
    // here it is read off the array so the check is not a constant.
    assert!(heights[1] > heights[0]);
}

// ---------------------------------------------------------------------------
// 4 — the contrast audit (UIF-VER-001)
// ---------------------------------------------------------------------------

/// How a produced colour pair is judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// Judged against a threshold: label text owes
    /// [`MIN_TEXT_CONTRAST`], active chrome owes [`MIN_CHROME_CONTRAST`].
    Judged,
    /// `border.subtle` — a **resting** divider. `UIF-TOK-006` rule 2
    /// governs *active* chrome, and `contrast` already establishes that
    /// this token is not active indicator material. Reported, not gated.
    RestingDivider,
    /// The stroke carries the same channels as the fill it sits on, so it
    /// paints nothing visible. It exists to hold geometry
    /// (`UIF-PRIN-003`), not to signal a state, so the chrome floor does
    /// not describe it.
    CoincidentWithFill,
    /// Nothing is painted behind this channel, so its contrast is the
    /// container's business rather than the style's.
    UnpaintedBackground,
}

/// One measured pair from the resolver's output.
///
/// `bg_color` is the **painted** background: for a translucent fill that
/// is the composite over the worst surface in the ladder, because the
/// style does not know what it is mounted to and the audit must not
/// assume the friendliest answer. `bg` keeps the token name so the row
/// still says where the colour came from.
struct AuditRow {
    channel: &'static str,
    fg: String,
    bg: String,
    fg_color: Color,
    bg_color: Color,
    ratio: f32,
    min: f32,
    verdict: Verdict,
    cells: Vec<String>,
}

impl AuditRow {
    fn key(&self) -> (String, String, String) {
        (self.channel.to_string(), self.fg.clone(), self.bg.clone())
    }

    fn passes(&self) -> bool {
        match self.verdict {
            Verdict::Judged => self.ratio >= self.min,
            _ => true,
        }
    }

    /// The role's own predicate applied to the painted colours, so the
    /// threshold and the measurement are cross-checked rather than one
    /// of them being taken on trust.
    fn role_predicate(&self) -> bool {
        if self.min >= MIN_TEXT_CONTRAST {
            meets_text_contrast(self.fg_color, self.bg_color)
        } else {
            meets_chrome_contrast(self.fg_color, self.bg_color)
        }
    }
}

/// Classify and measure one produced pair, returning the ratio and the
/// painted background the ratio was measured against.
fn classify(channel: &'static str, fg: Color, bg: Color) -> (f32, Color, Verdict) {
    let p = palette();
    if bg.a == 0.0 {
        return (f32::NAN, bg, Verdict::UnpaintedBackground);
    }
    if same_channels(fg, bg) {
        return (f32::NAN, bg, Verdict::CoincidentWithFill);
    }
    if channel == "border" && same_channels(fg, p.border_subtle) {
        return (contrast_ratio(fg, bg), bg, Verdict::RestingDivider);
    }
    if bg.a >= 1.0 {
        return (contrast_ratio(fg, bg), bg, Verdict::Judged);
    }
    // Translucent fill: judge the darkest thing it can actually paint,
    // i.e. the worst container in the ladder.
    let mut worst_ratio = f32::MAX;
    let mut worst_painted = bg;
    for (_, surface) in all_surfaces() {
        let painted = composite_over(bg, surface);
        let ratio = contrast_ratio(fg, painted);
        if ratio < worst_ratio {
            worst_ratio = ratio;
            worst_painted = painted;
        }
    }
    (worst_ratio, worst_painted, Verdict::Judged)
}

/// Every `(foreground, background)` pair `resolve_style` can paint, across
/// all four variants and every reachable state, deduplicated by
/// `(channel, fg, bg)` with the cells that produce each one recorded.
fn audit_rows() -> Vec<AuditRow> {
    let p = palette();
    let mut rows: Vec<AuditRow> = Vec::new();

    for variant in VARIANTS {
        for state in reachable_states() {
            let style = resolve_style(variant, state, &p);
            let at = format!("{variant:?} x {}", state_name(state));

            let produced: [(&str, Color, Color, f32); 3] = [
                (
                    "label",
                    style.foreground,
                    style.background,
                    MIN_TEXT_CONTRAST,
                ),
                (
                    "border",
                    style.border_color,
                    style.background,
                    MIN_CHROME_CONTRAST,
                ),
                (
                    "ring",
                    style.focus_ring_color,
                    style.background,
                    MIN_CHROME_CONTRAST,
                ),
            ];

            for (channel, fg, bg, min) in produced {
                if channel != "label" {
                    // A zero-width channel paints nothing; there is
                    // nothing to measure.
                    let width = match channel {
                        "border" => style.border_width,
                        _ => style.focus_ring_width,
                    };
                    if width <= 0.0 {
                        continue;
                    }
                }

                let (ratio, painted_bg, verdict) = classify(channel, fg, bg);
                let (fg_name, bg_name) = (token_name(fg), token_name(bg));
                let key = (channel.to_string(), fg_name.clone(), bg_name.clone());
                if let Some(existing) = rows.iter_mut().find(|r| r.key() == key) {
                    existing.cells.push(at.clone());
                } else {
                    rows.push(AuditRow {
                        channel,
                        fg: fg_name,
                        bg: bg_name,
                        fg_color: fg,
                        bg_color: painted_bg,
                        ratio,
                        min,
                        verdict,
                        cells: vec![at.clone()],
                    });
                }
            }
        }
    }
    rows
}

/// Render the audit table for the report.
fn render_audit(rows: &[AuditRow]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n{:<7} {:<26} {:<26} {:>8} {:>5}  {:<20} {}\n",
        "CHANNEL", "FOREGROUND", "BACKGROUND", "RATIO", "MIN", "VERDICT", "CELLS"
    ));
    out.push_str(&"-".repeat(120));
    out.push('\n');
    for r in rows {
        let ratio = if r.ratio.is_finite() {
            format!("{:.4}", r.ratio)
        } else {
            "—".to_string()
        };
        let verdict = match r.verdict {
            Verdict::Judged if r.passes() => "PASS".to_string(),
            Verdict::Judged => "FAIL".to_string(),
            Verdict::RestingDivider => "resting divider".to_string(),
            Verdict::CoincidentWithFill => "coincident w/ fill".to_string(),
            Verdict::UnpaintedBackground => "no bg painted".to_string(),
        };
        out.push_str(&format!(
            "{:<7} {:<26} {:<26} {:>8} {:>5}  {:<20} {}\n",
            r.channel,
            r.fg,
            r.bg,
            ratio,
            r.min,
            verdict,
            r.cells.join(", "),
        ));
    }
    out
}

#[test]
fn test_the_reachable_state_set_is_the_one_the_state_machine_produces() {
    // The audit's coverage depends on this. Nine states: Disabled alone
    // (the ring is zeroed), and Rest / Hovered / Pressed / Invalid each
    // with and without the ring.
    let states = reachable_states();
    assert_eq!(
        states.len(),
        9,
        "reachable states changed: {:?}",
        states.iter().map(|s| state_name(*s)).collect::<Vec<_>>()
    );
    assert!(
        !states
            .iter()
            .any(|s| s.base == BaseState::Disabled && s.focused),
        "a disabled control must never reach the resolver with a ring on"
    );
}

#[test]
fn test_the_audit_covers_every_variant_and_reachable_state() {
    let rows = audit_rows();
    // Every (variant, state) pair contributes a label row, so the
    // union of recorded cells must cover all 4 x 9 combinations.
    let mut covered: Vec<String> = Vec::new();
    for r in &rows {
        for c in &r.cells {
            if !covered.contains(c) {
                covered.push(c.clone());
            }
        }
    }
    for variant in VARIANTS {
        for state in reachable_states() {
            let at = format!("{variant:?} x {}", state_name(state));
            assert!(
                covered.contains(&at),
                "{at} produced no audited channel at all"
            );
        }
    }
    assert!(
        rows.len() >= 20,
        "the audit table looks too short to be complete: {} rows",
        rows.len()
    );
}

#[test]
fn test_contrast_audit_reports_every_produced_pair() {
    let rows = audit_rows();
    print!("{}", render_audit(&rows));

    let judged = rows.iter().filter(|r| r.verdict == Verdict::Judged).count();
    assert!(
        judged > 10,
        "only {judged} judged pairs — the audit is not reaching the matrix"
    );

    for r in rows.iter().filter(|r| r.verdict == Verdict::Judged) {
        assert_eq!(
            r.role_predicate(),
            r.passes(),
            "role predicate disagrees with the measurement for {} {} on {}",
            r.channel,
            r.fg,
            r.bg,
        );
        if !r.passes() {
            // Failures are allowed only when pinned; see the next test.
            assert!(
                pinned_failures()
                    .iter()
                    .any(|p| p.channel == r.channel && p.fg == r.fg && p.bg == r.bg),
                "UNPINNED FAILURE: {} {} on {} measures {:.4}:1 against a \
                 {:.1}:1 rule — a new unreadable pairing has arrived",
                r.channel,
                r.fg,
                r.bg,
                r.ratio,
                r.min,
            );
        }
    }
}

/// A judged pairing that does **not** clear the threshold its role
/// carries, pinned at its measured value.
struct PinnedFailure {
    channel: &'static str,
    fg: &'static str,
    bg: &'static str,
    cell: &'static str,
    min: f32,
    measured: f32,
    why: &'static str,
}

/// The judged pairs that fail, and what they measure.
///
/// **These are not waived and the tokens are not retuned.** `INC-07` is a
/// resolver and an audit, not a palette decision. Every row here is
/// pending a supervisor decision on whether the token moves, the cell is
/// repainted, or the shortfall is accepted and recorded against
/// `UIF-VER-001`.
///
/// Note that two of the three are the *focus ring*, not the label. They
/// exist only because the two-part state model lets a keyboard-focused
/// button also be hovered or pressed — combinations §3.1 never snapshots,
/// because it spells its Focused cell as "Rest + ring". The ring those
/// cells inherit is `accent.primary`, and indigo on red is not a focus
/// indicator.
fn pinned_failures() -> Vec<PinnedFailure> {
    vec![
        PinnedFailure {
            channel: "label",
            fg: "text.strong",
            bg: "status.danger",
            cell: "Danger x Hovered",
            min: MIN_TEXT_CONTRAST,
            measured: 3.8753,
            why: "§3.1's Danger Hovered cell paints the label in text.strong on \
                 an opaque status.danger fill; the G-7-retuned RGB(255, 28, 2) \
                 is too light to carry white body-size text",
        },
        PinnedFailure {
            channel: "ring",
            fg: "accent.primary",
            bg: "status.danger",
            cell: "Danger x Hovered + ring",
            min: MIN_CHROME_CONTRAST,
            measured: 1.1779,
            why: "a keyboard-focused Danger button the pointer is also over \
                 paints the accent focus ring directly on the danger fill. \
                 Indigo on red at 1.18:1 is not a visible focus indicator by \
                 any reading — the ring is effectively absent exactly when the \
                 user needs it",
        },
        PinnedFailure {
            channel: "ring",
            fg: "accent.primary",
            bg: "status.danger @ 85%",
            cell: "Danger x Pressed + ring",
            min: MIN_CHROME_CONTRAST,
            measured: 1.0609,
            why: "same pairing at the pressed 85% opacity, judged against the \
                 worst container in the ladder. Worse than the hovered case: \
                 the translucent red lands within 6% of the accent's own \
                 luminance",
        },
    ]
}

#[test]
fn test_the_failing_pairs_are_pinned_pending_a_supervisor_decision() {
    let rows = audit_rows();
    let failures: Vec<&AuditRow> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::Judged && !r.passes())
        .collect();
    let pinned = pinned_failures();

    assert_eq!(
        failures.len(),
        pinned.len(),
        "the set of failing judged pairs changed size. Now failing: {:?} \
         (pinned: {})",
        failures
            .iter()
            .map(|r| format!("{} {} on {}", r.channel, r.fg, r.bg))
            .collect::<Vec<_>>(),
        pinned.len(),
    );

    for p in &pinned {
        let row = failures
            .iter()
            .find(|r| r.channel == p.channel && r.fg == p.fg && r.bg == p.bg)
            .unwrap_or_else(|| {
                panic!(
                    "pinned failure {} {} on {} is no longer failing — \
                     either it was fixed or the resolver changed",
                    p.channel, p.fg, p.bg
                )
            });

        assert!(
            (row.ratio - p.measured).abs() < RATIO_EPS,
            "{} {} on {}: measures {:.4}, pinned at {:.4}",
            p.channel,
            p.fg,
            p.bg,
            row.ratio,
            p.measured,
        );
        assert!(
            row.ratio < p.min,
            "{} {} on {} must still be below its {:.1}:1 rule to stay pinned",
            p.channel,
            p.fg,
            p.bg,
            p.min,
        );
        let shortfall = p.min - row.ratio;
        assert!(
            shortfall > 0.5,
            "{} {} on {}: shortfall {} — the pinned record says the gap is \
             material, and it must stay that way to be worth a decision",
            p.channel,
            p.fg,
            p.bg,
            shortfall,
        );
        assert!(
            row.cells.contains(&p.cell.to_string()),
            "{} is not among the cells producing this pair: {:?}",
            p.cell,
            row.cells,
        );
        println!(
            "\nPINNED (pending supervisor decision): {} {} on {} at {} — \
             {:.4}:1 against a {:.1}:1 rule, short by {:.4}. {}",
            p.channel, p.fg, p.bg, p.cell, row.ratio, p.min, shortfall, p.why,
        );
    }
}

#[test]
fn test_the_danger_hovered_failure_measures_what_the_brief_predicted() {
    let p = palette();
    let style = resolve_style(
        ButtonVariant::Danger,
        VisualState {
            base: BaseState::Hovered,
            focused: false,
        },
        &p,
    );
    let ratio = contrast_ratio(style.foreground, style.background);

    // The brief's estimate was "~3.88:1". Measured: 3.8753:1. Same to
    // two decimals, so the suspicion was right — and the shortfall is
    // 0.6247 of a ratio point, not a rounding artefact.
    assert!(
        (ratio - 3.8753).abs() < RATIO_EPS,
        "text.strong on the retuned status.danger measures {ratio:.4}:1"
    );
    assert!(
        !meets_text_contrast(style.foreground, style.background),
        "and it fails the 4.5:1 normal-text rule"
    );
    assert!(
        meets_chrome_contrast(style.foreground, style.background),
        "the same pairing clears the 3.0:1 chrome rule, which is why the \
         Danger Hovered *border* (also text.strong on status.danger) passes"
    );
    assert!(
        (MIN_TEXT_CONTRAST - ratio - 0.6247).abs() < RATIO_EPS,
        "shortfall against rule 1 is {:.4}",
        MIN_TEXT_CONTRAST - ratio,
    );
}

#[test]
fn test_the_translucent_pressed_fills_pass_once_they_are_composited() {
    let p = palette();
    let pressed = VisualState {
        base: BaseState::Pressed,
        focused: false,
    };

    for variant in [ButtonVariant::Primary, ButtonVariant::Danger] {
        let style = resolve_style(variant, pressed, &p);
        assert!(style.background.a < 1.0);

        // The naive, alpha-blind reading.
        let naive = contrast_ratio(style.foreground, style.background);

        // What actually gets painted, over every surface a button can sit on.
        let mut worst = f32::MAX;
        let mut worst_on = "";
        for (name, surface) in all_surfaces() {
            let painted = composite_over(style.background, surface);
            let ratio = contrast_ratio(style.foreground, painted);
            if ratio < worst {
                worst = ratio;
                worst_on = name;
            }
        }

        println!(
            "\n{variant:?} pressed fill: naive {:.4}:1, composited worst \
             {:.4}:1 (on {worst_on}), rule {:.1}:1",
            naive, worst, MIN_TEXT_CONTRAST,
        );

        assert!(
            worst >= MIN_TEXT_CONTRAST,
            "{variant:?} pressed label reads {worst:.4}:1 at its worst over \
             {worst_on} — below the {:.1}:1 text rule",
            MIN_TEXT_CONTRAST,
        );
        assert!(
            worst > naive,
            "{variant:?}: compositing must improve on the naive reading, \
             got naive {naive:.4} vs painted {worst:.4}"
        );
    }
}

#[test]
fn test_resting_dividers_and_coincident_borders_are_reported_not_gated() {
    let rows = audit_rows();

    let resting: Vec<&AuditRow> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::RestingDivider)
        .collect();
    assert!(
        !resting.is_empty(),
        "the Ghost hovered border and the disabled borders use border.subtle \
         and must show up as resting dividers"
    );
    for r in &resting {
        assert_eq!(r.channel, "border");
        assert_eq!(r.fg, "border.subtle");
        assert!(
            r.ratio < MIN_CHROME_CONTRAST,
            "{} on {} measures {:.4}:1 — if border.subtle ever clears 3.0 \
             the resting-divider classification needs re-arguing",
            r.fg,
            r.bg,
            r.ratio,
        );
    }

    let coincident: Vec<&AuditRow> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::CoincidentWithFill)
        .collect();
    assert!(
        coincident.len() >= 3,
        "Primary rest/press and Danger press all give the border the fill's \
         own channels; found {}",
        coincident.len(),
    );
    for r in &coincident {
        // Invisible by construction: the stroke carries the fill's own
        // channels, so it cannot signal a state change and the chrome
        // floor is not a rule about it.
        assert!(
            same_channels(r.fg_color, r.bg_color),
            "{} on {} is classed coincident but its channels differ",
            r.fg,
            r.bg,
        );
    }
}

#[test]
fn test_focus_rings_clear_the_chrome_rule_except_where_pinned() {
    let p = palette();
    let pinned = pinned_failures();
    let mut checked = 0;

    for variant in VARIANTS {
        for state in reachable_states() {
            if !state.focused {
                continue;
            }
            let style = resolve_style(variant, state, &p);
            assert!(style.focus_ring_width > 0.0);
            let at = format!("{variant:?} x {}", state_name(state));

            if style.background.a == 0.0 {
                // Ghost at rest: nothing is painted behind the ring, so
                // its contrast is the container's. Check it against every
                // surface the framework could mount the button on.
                for (name, surface) in all_surfaces() {
                    assert!(
                        meets_chrome_contrast(style.focus_ring_color, surface),
                        "{at} focus ring fails against container {name} at \
                         {:.4}:1",
                        contrast_ratio(style.focus_ring_color, surface),
                    );
                }
                checked += 1;
                continue;
            }

            let (_, painted, verdict) = classify("ring", style.focus_ring_color, style.background);
            assert_eq!(verdict, Verdict::Judged);
            let ratio = contrast_ratio(style.focus_ring_color, painted);

            if pinned.iter().any(|f| f.channel == "ring" && f.cell == at) {
                assert!(
                    ratio < MIN_CHROME_CONTRAST,
                    "{at}: pinned as a failure but now measures {ratio:.4}:1, \
                     which clears the chrome rule — un-pin it deliberately"
                );
            } else {
                assert!(
                    ratio >= MIN_CHROME_CONTRAST,
                    "{at} ring reads {ratio:.4}:1 against its own fill, below \
                     the {:.1}:1 chrome rule and not pinned",
                    MIN_CHROME_CONTRAST,
                );
            }
            checked += 1;
        }
    }

    // 4 variants x 4 focused reachable states = 16 ring checks.
    assert_eq!(
        checked, 16,
        "the ring audit stopped covering every focused reachable state"
    );
}
