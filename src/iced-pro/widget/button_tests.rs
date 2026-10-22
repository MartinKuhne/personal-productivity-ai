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
//! # The audit's history — three failures, cleared in INC-08
//!
//! `INC-07` found three judged pairs that did not clear the rule their
//! role carries. They were pinned rather than waived, pending a supervisor
//! decision. Decisions **D-5** and **D-6** resolved both root causes, and
//! all three now pass. The old measurements are kept here and in
//! [`resolved_pairs`] so the history stays visible next to the numbers
//! that replaced them.
//!
//! | Pair | Cell | Was | Now | Rule |
//! |---|---|---|---|---|
//! | `text.strong` on `status.danger` | Danger `Hovered` | `3.8753:1` | `5.5185:1` on `status.danger_fill` | `4.5:1` |
//! | `accent.primary` ring on `status.danger` | Danger `Hovered + ring` | `1.1779:1` | `5.5185:1` — `text.strong` ring | `3.0:1` |
//! | `accent.primary` ring on `status.danger @ 85%` | Danger `Pressed + ring` | `1.0609:1` | `6.6216:1` — `text.strong` ring | `3.0:1` |
//!
//! The first was the pair the increment brief predicted: white on the
//! `G-7`-retuned `RGB(255, 28, 2)` is `3.8753:1`, the brief's
//! `~3.88:1` to two decimals. It is fixed by the **D-5** token split —
//! the Danger fill is now `status_danger_fill`, `RGB(208, 23, 2)`, and
//! the error-text red is no longer asked to be a background.
//!
//! The other two were the finding the brief did not predict, and they are
//! fixed by the **D-6** adaptive ring rather than by the new token. They
//! existed only because the two-part state model lets a keyboard-focused
//! button *also* be hovered or pressed — combinations §3.1 never
//! snapshots, because it spells its Focused cell as "Rest + ring".
//! Inheriting the hover fill instead of Rest, as `Hovered + focused`
//! must, dropped the `accent.primary` ring onto a red fill at roughly
//! `1.18:1`: an invisible focus indicator precisely when the user is
//! doing the thing it exists to report. Resolving the ring against the
//! actual background makes it `text.strong` there.
//!
//! Note that the second and third rows changed *foreground*, not just
//! value — the pair that is now measured is not the pair that failed.
//! [`resolved_pairs`] records both halves so the substitution is
//! auditable rather than hidden behind a matching row name.
//!
//! # Why the pressed fills are measured composited
//!
//! Primary and Danger press at 85% alpha. [`contrast_ratio`] ignores
//! alpha by design, so a naive reading of a pressed label is the
//! *opaque* pairing — `4.5648:1` for Primary, `5.5185:1` for Danger.
//! Laid over the surfaces a button can actually be mounted to, the 85%
//! fill lands darker and both labels read better than the naive figure:
//! `5.3967:1` and `6.6216:1` at their worst, both over `surface.active`.
//! The audit measures the painted colour and reports the naive figure
//! alongside it, because the gap between the two is exactly the kind of
//! mistake this gate exists to catch.

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

/// A button's resolved style at a base state, with no focus ring drawn.
fn style_for(variant: ButtonVariant, base: BaseState) -> ButtonStyle {
    resolve_style(
        variant,
        VisualState {
            base,
            focused: false,
        },
        &palette(),
    )
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
    let table: [(&str, Color); 20] = [
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
        ("status.danger_fill", p.status_danger_fill),
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
        ..p.status_danger_fill
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
            p.status_danger_fill,
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
            p.status_danger_fill,
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
// 2 — D-5: the Danger fill split
// ---------------------------------------------------------------------------

#[test]
fn test_danger_hovered_and_pressed_share_the_split_fill_and_resting_stays_on_surface() {
    let p = palette();
    let fill = p.status_danger_fill;

    let hovered = style_for(ButtonVariant::Danger, BaseState::Hovered);
    let pressed = style_for(ButtonVariant::Danger, BaseState::Pressed);
    let resting = style_for(ButtonVariant::Danger, BaseState::Rest);

    assert_eq!(
        hovered.background, fill,
        "D-5: Danger Hovered must fill with status.danger_fill, not the error-text red"
    );
    assert_eq!(
        hovered.background.a, 1.0,
        "D-5: the Hovered fill is opaque — only Pressed carries the spec's alpha"
    );
    assert!(
        same_channels(pressed.background, fill),
        "D-5: Danger Pressed fills with the same status.danger_fill channels, got {}",
        rgb8(pressed.background)
    );
    assert_eq!(
        pressed.background.a, PRESSED_FILL_ALPHA,
        "D-5: the Pressed fill keeps the §3.1 press opacity"
    );

    assert_eq!(
        resting.background, p.surface_raised,
        "D-5: only Hovered and Pressed change — the Danger resting surface is untouched"
    );
    assert_eq!(
        resting.foreground, p.status_danger,
        "D-5: the Danger resting label stays status.danger, the tinted signal"
    );
    assert_eq!(
        resting.border_color, p.status_danger,
        "D-5: the Danger resting border stays status.danger"
    );
    assert_eq!(
        hovered.border_color, p.text_strong,
        "D-5: the Danger Hovered border stays text.strong — the fill carries the weight"
    );
}

#[test]
fn test_danger_pressed_border_shares_the_fill_instead_of_the_error_red() {
    // The spec writes the Danger Pressed border as `status.danger`. Against the
    // darker pressed fill that is a bright edge rather than a signal: measured
    // below, the pairing lands under 2:1 on every surface in the ladder, well
    // short of the 3.0:1 chrome rule — so keeping it would have traded one audit
    // failure for another.
    //
    // Primary already follows the same rule (its pressed border shares its
    // fill's channels). Secondary and Ghost are the deliberate exceptions: they
    // press on a neutral surface, so their border is a real signal rather than
    // an edge on a coloured fill.
    let p = palette();
    let pressed = style_for(ButtonVariant::Danger, BaseState::Pressed);

    assert!(
        same_channels(pressed.border_color, p.status_danger_fill),
        "D-5: the Danger pressed border must share the fill, got {}",
        rgb8(pressed.border_color)
    );
    assert!(
        !same_channels(pressed.border_color, p.status_danger),
        "D-5: the bright error red must not edge the darker fill"
    );

    for (name, surface) in all_surfaces() {
        let mounted = composite_over(pressed.background, surface);
        let would_be = contrast_ratio(p.status_danger, mounted);
        assert!(
            would_be < MIN_CHROME_CONTRAST,
            "the pre-D-5 `status.danger` border over the pressed fill on {name} \
             measures {would_be:.4}:1 — it must not come back"
        );
    }

    let primary = style_for(ButtonVariant::Primary, BaseState::Pressed);
    assert!(
        same_channels(primary.border_color, primary.background),
        "Primary Pressed border {} shares its fill {}",
        rgb8(primary.border_color),
        rgb8(primary.background),
    );
}

#[test]
fn test_the_danger_fill_is_the_only_new_token_and_nothing_else_moved() {
    // The split must be paid for by adding a token, not by nudging an existing
    // one. Every value below is the literal `Palette::dark()` carries.
    let now = palette();

    assert_eq!(rgb8(now.surface_base), "RGB(9, 9, 11)");
    assert_eq!(rgb8(now.surface_sunken), "RGB(6, 6, 8)");
    assert_eq!(rgb8(now.surface_raised), "RGB(18, 18, 20)");
    assert_eq!(rgb8(now.surface_code), "RGB(20, 20, 22)");
    assert_eq!(rgb8(now.surface_overlay), "RGB(24, 24, 27)");
    assert_eq!(rgb8(now.surface_hover), "RGB(38, 38, 44)");
    assert_eq!(rgb8(now.surface_active), "RGB(43, 43, 51)");
    assert_eq!(rgb8(now.border_subtle), "RGB(40, 40, 40)");
    assert_eq!(rgb8(now.border_strong), "RGB(120, 120, 120)");
    assert_eq!(rgb8(now.accent_primary), "RGB(98, 100, 240)");
    assert_eq!(rgb8(now.accent_text), "RGB(100, 200, 255)");
    assert_eq!(rgb8(now.text_primary), "RGB(210, 210, 210)");
    assert_eq!(rgb8(now.text_strong), "RGB(255, 255, 255)");
    assert_eq!(rgb8(now.text_muted), "RGB(160, 160, 160)");
    assert_eq!(rgb8(now.text_disabled), "RGB(140, 140, 140)");
    assert_eq!(rgb8(now.status_success), "RGB(100, 255, 100)");
    assert_eq!(rgb8(now.status_warning), "RGB(220, 130, 0)");
    assert_eq!(rgb8(now.status_danger), "RGB(255, 28, 2)");
    assert_eq!(
        rgb8(now.status_danger_fill),
        "RGB(208, 23, 2)",
        "status.danger_fill is the single token INC-08 adds"
    );
}

// ---------------------------------------------------------------------------
// 3 — focus, opacity, the unpainted fill
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
    // The ring rides on top of that fill, and the adaptive rule is what
    // makes it visible: the accent cannot clear the chrome floor against
    // the danger fill, so the ring degrades to `text.strong` rather than
    // disappearing. See `test_every_focus_ring_clears_the_chrome_rule`.
    assert_eq!(pressed_focused.focus_ring_color, p.text_strong);
}

#[test]
fn test_the_ring_at_rest_reproduces_the_per_variant_colour_spec_3_1_gives() {
    let p = palette();
    let focused = VisualState {
        base: BaseState::Rest,
        focused: true,
    };

    // The adaptive rule is not a licence to repaint the spec's ring. At
    // Rest it lands on exactly the colour §3.1 names for each variant:
    // Primary's fill is the accent itself, so the accent cannot clear the
    // floor against it and the rule falls to `text.strong` — which is the
    // reason §3.1 gives Primary a white ring in the first place. The
    // other three rest on a surface the accent clears, so they keep it.
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
        (
            p.status_danger_fill.r,
            p.status_danger_fill.g,
            p.status_danger_fill.b
        ),
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
            // Failures are allowed only when recorded; see
            // `accepted_failures`, which `INC-08` emptied.
            assert!(
                accepted_failures()
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
/// carries, recorded at its measured value rather than waived.
struct AcceptedFailure {
    channel: &'static str,
    fg: &'static str,
    bg: &'static str,
    cell: &'static str,
    min: f32,
    measured: f32,
    why: &'static str,
}

/// Judged pairings the audit is allowed to leave failing.
///
/// **`INC-08` emptied this list.** The three rows `INC-07` recorded here
/// were two root causes, and decisions **D-5** (the `status.danger`
/// split) and **D-6** (the adaptive focus ring) cleared both. They are
/// carried in [`resolved_pairs`] with their old and new measurements so
/// the history stays next to the code that fixed it.
///
/// The type and the guard that reads it are kept rather than deleted on
/// purpose. An unreadable pairing should still fail into a named, argued
/// record that a human has to add by hand — not into a bare assertion
/// someone can satisfy by deleting the threshold.
fn accepted_failures() -> Vec<AcceptedFailure> {
    Vec::new()
}

/// The accepted-failure ledger is empty, and every record it could hold is
/// a fully argued gap rather than a bare colour pair.
///
/// The second half of this test runs over nothing today. It is there so
/// the record's shape stays honest: adding an `AcceptedFailure` later
/// requires naming the cell, the rule, the measurement and the argument,
/// not just the two colours that happen to clash.
#[test]
fn test_the_accepted_failure_ledger_is_empty_and_fully_specified() {
    let accepted = accepted_failures();
    assert!(
        accepted.is_empty(),
        "INC-08 cleared every accepted failure with D-5 and D-6; a new entry \
         must be a supervisor decision, not a convenience — found {:?}",
        accepted
            .iter()
            .map(|f| format!("{} {} on {}", f.channel, f.fg, f.bg))
            .collect::<Vec<_>>(),
    );

    for f in &accepted {
        assert!(!f.cell.is_empty(), "an accepted failure must name its cell");
        assert!(
            f.min > 0.0,
            "{} {} on {}: an accepted failure must carry the rule it misses",
            f.channel,
            f.fg,
            f.bg,
        );
        assert!(
            f.measured > 0.0 && f.measured < f.min,
            "{} {} on {}: the measured {:.4} must sit under the {:.1} it is \
             accepted against",
            f.channel,
            f.fg,
            f.bg,
            f.measured,
            f.min,
        );
        assert!(
            !f.why.is_empty(),
            "{} {} on {}: an accepted failure needs an argument, not just a number",
            f.channel,
            f.fg,
            f.bg,
        );
    }
}

/// A judged pairing that failed in `INC-07` and now clears its rule.
///
/// Both halves are recorded. Two of the three rows changed *foreground*
/// as well as value — the ring moved from `accent.primary` to
/// `text.strong` — so the pairing being measured today is not the pairing
/// that failed. Recording only the new row would hide that substitution.
struct ResolvedPair {
    channel: &'static str,
    /// The foreground that failed.
    was_fg: &'static str,
    /// The background that failed.
    was_bg: &'static str,
    /// The cell that produced the failing pairing.
    cell: &'static str,
    /// The threshold the role carries.
    min: f32,
    /// What the failing pairing measured.
    was_measured: f32,
    /// The foreground that replaced it.
    now_fg: &'static str,
    /// The background that replaced it.
    now_bg: &'static str,
    /// What the replacement measures, to four decimals.
    now_measured: f32,
    /// The decision that cleared it.
    fix: &'static str,
}

/// The three `INC-07` failures and what each one measures now.
fn resolved_pairs() -> Vec<ResolvedPair> {
    vec![
        ResolvedPair {
            channel: "label",
            was_fg: "text.strong",
            was_bg: "status.danger",
            cell: "Danger x Hovered",
            min: MIN_TEXT_CONTRAST,
            was_measured: 3.8753,
            now_fg: "text.strong",
            now_bg: "status.danger_fill",
            now_measured: 5.5185,
            fix: "D-5 split the destructive red in two. The Danger hover \
                 fill is now status_danger_fill, RGB(208, 23, 2), and the \
                 error-text red is no longer asked to be a background. The \
                 label moved from 0.6247 under the rule to 1.0185 over it.",
        },
        ResolvedPair {
            channel: "ring",
            was_fg: "accent.primary",
            was_bg: "status.danger",
            cell: "Danger x Hovered + ring",
            min: MIN_CHROME_CONTRAST,
            was_measured: 1.1779,
            now_fg: "text.strong",
            now_bg: "status.danger_fill",
            now_measured: 5.5185,
            fix: "D-6 resolves the ring against the actual resolved fill. \
                 The accent cannot clear 3.0 against the danger fill, so \
                 the ring degrades to text.strong — indigo on red at \
                 1.18:1 became white on dark red at 5.52:1.",
        },
        ResolvedPair {
            channel: "ring",
            was_fg: "accent.primary",
            was_bg: "status.danger @ 85%",
            cell: "Danger x Pressed + ring",
            min: MIN_CHROME_CONTRAST,
            was_measured: 1.0609,
            now_fg: "text.strong",
            now_bg: "status.danger_fill @ 85%",
            now_measured: 6.6216,
            fix: "D-5 and D-6 together, judged against the worst container \
                 in the ladder. The worst case moved from within 6% of the \
                 accent's own luminance to a clear 6.62:1.",
        },
    ]
}

#[test]
fn test_the_previously_failing_pairs_now_clear_their_thresholds() {
    let rows = audit_rows();

    // Nothing is left outstanding: the audit is clean, so nothing may be
    // recorded in `accepted_failures` either.
    let still_failing: Vec<String> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::Judged && !r.passes())
        .map(|r| {
            format!(
                "{} {} on {} at {:.4}:1 (rule {:.1}:1)",
                r.channel, r.fg, r.bg, r.ratio, r.min
            )
        })
        .collect();
    assert!(
        still_failing.is_empty(),
        "the audit still has failing judged pairs: {still_failing:?}",
    );
    assert!(
        accepted_failures().is_empty(),
        "accepted_failures is not empty while the audit is clean — a row was \
         added that no longer describes a real failure",
    );

    for p in resolved_pairs() {
        // The pairing that used to fail must no longer be produced at all.
        let old_still_there = rows
            .iter()
            .any(|r| r.channel == p.channel && r.fg == p.was_fg && r.bg == p.was_bg);
        assert!(
            !old_still_there,
            "the failing pairing {} {} on {} is still being produced — the fix \
             was reverted or the resolver changed shape",
            p.channel, p.was_fg, p.was_bg,
        );

        // The replacement must be produced, judged, and passing.
        let row = rows
            .iter()
            .find(|r| r.channel == p.channel && r.fg == p.now_fg && r.bg == p.now_bg)
            .unwrap_or_else(|| {
                panic!(
                    "the replacement pair {} {} on {} is not in the audit at all \
                     — the resolver no longer paints what D-5/D-6 decided",
                    p.channel, p.now_fg, p.now_bg,
                )
            });

        assert_eq!(
            row.verdict,
            Verdict::Judged,
            "{} {} on {} must be a judged pair, not exempted",
            p.channel,
            p.now_fg,
            p.now_bg,
        );
        assert!(
            row.cells.contains(&p.cell.to_string()),
            "{} is not among the cells producing this pair: {:?}",
            p.cell,
            row.cells,
        );
        assert!(
            (row.ratio - p.now_measured).abs() < RATIO_EPS,
            "{} {} on {}: measures {:.4}, recorded at {:.4}",
            p.channel,
            p.now_fg,
            p.now_bg,
            row.ratio,
            p.now_measured,
        );
        assert!(
            row.ratio >= p.min,
            "{} {} on {} measures {:.4}:1, still under its {:.1}:1 rule",
            p.channel,
            p.now_fg,
            p.now_bg,
            row.ratio,
            p.min,
        );
        assert!(
            row.ratio > p.was_measured,
            "{} {} on {} must be an improvement on the {:.4}:1 it replaces, got \
             {:.4}:1",
            p.channel,
            p.now_fg,
            p.now_bg,
            p.was_measured,
            row.ratio,
        );
        println!(
            "\nRESOLVED: {} {} on {} at {} — was {:.4}:1 (short by {:.4}), now \
             {:.4}:1 against a {:.1}:1 rule, {:.4} of headroom. {}",
            p.channel,
            p.now_fg,
            p.now_bg,
            p.cell,
            p.was_measured,
            p.min - p.was_measured,
            row.ratio,
            p.min,
            row.ratio - p.min,
            p.fix,
        );
    }
}

#[test]
fn test_the_danger_hovered_label_now_clears_the_text_rule() {
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

    // `INC-07` measured this cell at 3.8753:1 against the error-text red,
    // 0.6247 short of the 4.5:1 normal-text rule. The D-5 split put a
    // background red under the label instead, and the same cell now reads
    // 5.5185:1 — 1.0185 above the rule rather than below it.
    assert_eq!(style.background, p.status_danger_fill);
    assert!(
        (ratio - 5.5185).abs() < RATIO_EPS,
        "text.strong on status.danger_fill measures {ratio:.4}:1"
    );
    assert!(
        meets_text_contrast(style.foreground, style.background),
        "the Danger hover label must clear the 4.5:1 normal-text rule"
    );
    assert!(
        meets_chrome_contrast(style.foreground, style.background),
        "and the Danger Hovered *border*, the same pairing, clears the \
         3.0:1 chrome rule it always did"
    );
    assert!(
        (ratio - MIN_TEXT_CONTRAST - 1.0185).abs() < RATIO_EPS,
        "headroom against rule 1 is {:.4}",
        ratio - MIN_TEXT_CONTRAST,
    );

    // The old pairing is what the split exists to remove: white on the
    // error-text red is still unreadable, which is why that token must not
    // be used as a fill.
    let old = contrast_ratio(p.text_strong, p.status_danger);
    assert!(
        (old - 3.8753).abs() < RATIO_EPS,
        "text.strong on status.danger still measures {old:.4}:1 — the value is \
         unchanged, as D-5 requires"
    );
    assert!(
        !meets_text_contrast(p.text_strong, p.status_danger),
        "and it still fails, which is the whole reason status_danger_fill exists"
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
fn test_every_focus_ring_clears_the_chrome_rule() {
    let p = palette();
    let mut checked = 0;
    let mut report: Vec<String> = Vec::new();

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
                report.push(format!(
                    "  {at:<34} ring={} — sits on the container, clears every \
                     surface in the ladder",
                    token_name(style.focus_ring_color),
                ));
                checked += 1;
                continue;
            }

            let (_, painted, verdict) = classify("ring", style.focus_ring_color, style.background);
            assert_eq!(verdict, Verdict::Judged);
            let ratio = contrast_ratio(style.focus_ring_color, painted);
            assert!(
                ratio >= MIN_CHROME_CONTRAST,
                "{at} ring reads {ratio:.4}:1 against its own fill, below the \
                 {:.1}:1 chrome rule — the adaptive resolver picked a ring \
                 that cannot be seen",
                MIN_CHROME_CONTRAST,
            );
            report.push(format!(
                "  {at:<34} ring={} @ {:.4}:1 vs {}",
                token_name(style.focus_ring_color),
                ratio,
                token_name(style.background),
            ));
            checked += 1;
        }
    }

    // 4 variants x 4 focused reachable states = 16 ring checks.
    assert_eq!(
        checked, 16,
        "the ring audit stopped covering every focused reachable state"
    );
    println!("\nADAPTIVE RING OUTCOMES\n{}", report.join("\n"));
}

/// The ring colour each `(variant, base state)` combination is expected to
/// draw, and the ratio it clears by.
///
/// This is the **D-6** rule written out as a table, so a change in the
/// resolver shows up as a named row rather than a shifted number nobody
/// noticed. Every entry is the accent except the two Danger fills, where
/// the accent cannot clear the floor against a red background and the
/// ring degrades to `text.strong`.
#[test]
fn test_the_ring_winner_per_variant_and_base_state_is_the_expected_token() {
    let p = palette();
    let expected: &[(&str, ButtonVariant, BaseState, &str, f32)] = &[
        // Primary's fill *is* the accent, so an accent ring is invisible
        // against it in every state and the rule falls to text.strong.
        (
            "Primary",
            ButtonVariant::Primary,
            BaseState::Rest,
            "text.strong",
            4.5648,
        ),
        (
            "Primary",
            ButtonVariant::Primary,
            BaseState::Hovered,
            "text.strong",
            4.5648,
        ),
        (
            "Primary",
            ButtonVariant::Primary,
            BaseState::Pressed,
            "text.strong",
            4.5648,
        ),
        (
            "Primary",
            ButtonVariant::Primary,
            BaseState::Invalid,
            "text.strong",
            4.5648,
        ),
        // Secondary rests and works on surfaces the accent clears.
        (
            "Secondary",
            ButtonVariant::Secondary,
            BaseState::Rest,
            "accent.primary",
            4.0990,
        ),
        (
            "Secondary",
            ButtonVariant::Secondary,
            BaseState::Hovered,
            "accent.primary",
            3.2954,
        ),
        (
            "Secondary",
            ButtonVariant::Secondary,
            BaseState::Pressed,
            "accent.primary",
            3.0750,
        ),
        (
            "Secondary",
            ButtonVariant::Secondary,
            BaseState::Invalid,
            "accent.primary",
            4.0990,
        ),
        // Ghost's resting ring is measured against the container, not a
        // fill — see `resolve_ring_color_for_background`.
        (
            "Ghost",
            ButtonVariant::Ghost,
            BaseState::Rest,
            "accent.primary",
            4.3583,
        ),
        (
            "Ghost",
            ButtonVariant::Ghost,
            BaseState::Hovered,
            "accent.primary",
            3.2954,
        ),
        (
            "Ghost",
            ButtonVariant::Ghost,
            BaseState::Pressed,
            "accent.primary",
            3.0750,
        ),
        (
            "Ghost",
            ButtonVariant::Ghost,
            BaseState::Invalid,
            "accent.primary",
            4.3583,
        ),
        // Danger rests on surface.raised like Secondary, but its hover and
        // press fills swallow the accent.
        (
            "Danger",
            ButtonVariant::Danger,
            BaseState::Rest,
            "accent.primary",
            4.0990,
        ),
        (
            "Danger",
            ButtonVariant::Danger,
            BaseState::Hovered,
            "text.strong",
            5.5185,
        ),
        (
            "Danger",
            ButtonVariant::Danger,
            BaseState::Pressed,
            "text.strong",
            5.5185,
        ),
        (
            "Danger",
            ButtonVariant::Danger,
            BaseState::Invalid,
            "accent.primary",
            4.0990,
        ),
    ];

    for (label, variant, base, want_token, want_ratio) in expected {
        let style = resolve_style(
            *variant,
            VisualState {
                base: *base,
                focused: true,
            },
            &p,
        );
        assert_eq!(
            token_name(style.focus_ring_color),
            *want_token,
            "{label} x {base:?} + ring: the resolver picked {}, expected \
             {want_token}",
            token_name(style.focus_ring_color),
        );
        let measured = if style.background.a == 0.0 {
            // The Ghost resting ring sits on the container, not on a
            // fill. Measure it against `surface.base`, the root canvas a
            // Ghost button is normally mounted to; the full sweep across
            // every surface lives in
            // `test_every_focus_ring_clears_the_chrome_rule`, where the
            // tightest case is `surface.active` at 3.0750:1.
            contrast_ratio(style.focus_ring_color, p.surface_base)
        } else {
            contrast_ratio(style.focus_ring_color, style.background)
        };
        assert!(
            (measured - want_ratio).abs() < RATIO_EPS,
            "{label} x {base:?} + ring: {} measures {measured:.4}:1, expected \
             {want_ratio:.4}:1",
            *want_token,
        );
        assert!(
            measured >= MIN_CHROME_CONTRAST,
            "{label} x {base:?} + ring must clear the chrome floor, got \
             {measured:.4}:1",
        );
    }
}
