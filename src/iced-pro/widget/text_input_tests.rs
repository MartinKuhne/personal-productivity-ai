//! Tests for the text input atom — `UIF-ATOM-002`, the §3.2 matrix, the
//! zero-shift slot and the `UIF-VER-001` contrast audit over both.
//!
//! Five groups, in order:
//!
//! 1. **The matrix.** Every cell of §3.2 transcribed as an expected
//!    `(background, text, placeholder, icon, border)` quintuple and
//!    asserted against [`resolve_input_style`], so the resolver is
//!    checked against the spec rather than against itself. Every cell is
//!    asserted by **token name**, never by hex: a retune that moves a
//!    token moves the expectation with it, and a cell that lands on an
//!    unmapped colour fails loudly instead of quietly matching.
//! 2. **The contested perimeter.** Invalid and Focused both claim a 2px
//!    stroke. These tests pin the decision the module doc argues for —
//!    the error keeps the border, focus relocates outside — rather than
//!    leaving it a prose claim.
//! 3. **The reserved slot.** Slot *content* is a value ([`slot_content`])
//!    and slot *geometry* is a function of `(bounds, kind)` alone, so the
//!    zero-shift invariant is checked by trying to break it: every kind
//!    against every thing that can sit in its slot.
//! 4. **Real layout.** The pure functions say the text area does not
//!    move; the harness reads back what iced actually laid the editor out
//!    into, which is the assertion that would fail if the wrapper ever
//!    delegated the reservation to its content.
//! 5. **The contrast audit.** Every `(fg, bg)` pair the resolver can
//!    paint, measured with the `INC-05` solver and classified by role,
//!    with failures pinned rather than waived.
//!
//! # The audit's result
//!
//! Unlike `INC-07`'s button audit, this one finds nothing to pin. Every
//! judged pair clears its rule on the `G-7`-retuned palette — the widest
//! margin is the selection lock at `4.8020:1` over its `4.5:1` floor,
//! the tightest chrome pair is `border.strong` on `surface.code` at
//! `4.1672:1` over `3.0:1`. The two `border.subtle` rows are reported
//! as resting dividers, which `UIF-TOK-006` rule 2 does not govern.
//! [`accepted_failures`] is therefore empty, and the ledger type is kept
//! so a future unreadable pairing still has to arrive as a named, argued
//! record.

use super::*;
use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::test_helpers::{Harness, centre, iced_id, moved};
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, composite_over, contrast_ratio, meets_chrome_contrast,
    meets_text_contrast,
};
use crate::token::{shape, size, space};
use iced::Color;
use iced::Point;
use iced::widget::column;
use iced_test::renderer::Renderer as HeadlessRenderer;

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `4.8020` from `4.5000`.
const RATIO_EPS: f32 = 1e-3;

/// Tolerance on a colour-channel comparison used to name a token.
const CHANNEL_EPS: f32 = 1e-6;

/// The three §3.2 kinds.
const KINDS: [InputKind; 3] = [
    InputKind::TextEntry,
    InputKind::Search,
    InputKind::DropdownTrigger,
];

/// The dark palette, taken once so every table reads as token names.
fn palette() -> Palette {
    Palette::dark()
}

/// A resolved input style at an explicit state.
fn style_for(kind: InputKind, base: BaseState, focused: bool) -> InputStyle {
    resolve_input_style(kind, VisualState { base, focused }, &palette())
}

/// Every surface token in the elevation ladder, darkest to lightest.
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
/// cannot make a failing row anonymous.
fn token_name(color: Color) -> String {
    if color.a == 0.0 {
        return "transparent".to_string();
    }
    let p = palette();
    let table: [(&str, Color); 23] = [
        ("surface.base", p.surface_base),
        ("surface.sunken", p.surface_sunken),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
        ("border.subtle", p.border_subtle),
        ("border.strong", p.border_strong),
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
        ("accent.border", p.accent_border),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.success", p.status_success),
        ("status.warning", p.status_warning),
        ("status.danger_text", p.status_danger_text),
        ("status.danger_fill", p.status_danger_fill),
        ("status.danger_fill_active", p.status_danger_fill_active),
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
/// reachable set.
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

/// A label for an assertion message: kind, state, and the field checked.
fn where_at(kind: InputKind, state: VisualState, field: &str) -> String {
    format!("{kind:?} x {} x {field}", state_name(state))
}

// ---------------------------------------------------------------------------
// 1 — the §3.2 matrix
// ---------------------------------------------------------------------------

/// The five channels §3.2 prints for one cell, plus its stroke weight.
///
/// A struct rather than six parameters so the cell reads as a row of the
/// spec's table instead of a positional call nobody can check against
/// the header.
struct ExpectedCell {
    background: Color,
    text: Color,
    placeholder: Color,
    icon: Color,
    border: Color,
    border_width: f32,
}

/// Assert one resolved style against the five channels §3.2 prints, by
/// token name, for every kind at once.
///
/// The kinds are looped rather than assumed because §3.2 specifies one
/// matrix, not three: if any kind ever drifted, the first cell it drifted
/// in would be reported with its kind in the message.
fn assert_cell(base: BaseState, focused: bool, want: ExpectedCell) {
    let p = palette();
    let ExpectedCell {
        background,
        text,
        placeholder,
        icon,
        border,
        border_width,
    } = want;

    for kind in KINDS {
        let state = VisualState { base, focused };
        let style = resolve_input_style(kind, state, &p);
        let at = where_at(kind, state, "");

        assert_eq!(
            token_name(style.background),
            token_name(background),
            "{at} background: expected {} but resolved {}",
            token_name(background),
            token_name(style.background),
        );
        assert_eq!(
            token_name(style.text),
            token_name(text),
            "{at} text: expected {} but resolved {}",
            token_name(text),
            token_name(style.text),
        );
        assert_eq!(
            token_name(style.placeholder),
            token_name(placeholder),
            "{at} placeholder: expected {} but resolved {}",
            token_name(placeholder),
            token_name(style.placeholder),
        );
        assert_eq!(
            token_name(style.icon),
            token_name(icon),
            "{at} icon: expected {} but resolved {}",
            token_name(icon),
            token_name(style.icon),
        );
        assert_eq!(
            token_name(style.border_color),
            token_name(border),
            "{at} border: expected {} but resolved {}",
            token_name(border),
            token_name(style.border_color),
        );
        assert_eq!(
            style.border_width, border_width,
            "{at} border width: expected {border_width} but resolved {}",
            style.border_width,
        );
    }
}

#[test]
fn test_rest_cell_is_the_spec_row() {
    let p = palette();
    assert_cell(
        BaseState::Rest,
        false,
        ExpectedCell {
            background: p.surface_code,
            text: p.text_primary,
            placeholder: p.text_disabled,
            icon: p.text_muted,
            border: p.border_subtle,
            border_width: shape::BORDER_HAIRLINE,
        },
    );
}

#[test]
fn test_hovered_cell_is_the_spec_row() {
    let p = palette();
    assert_cell(
        BaseState::Hovered,
        false,
        ExpectedCell {
            background: p.surface_code,
            text: p.text_primary,
            placeholder: p.text_muted,
            icon: p.text_primary,
            border: p.border_strong,
            border_width: shape::BORDER_HAIRLINE,
        },
    );
}

#[test]
fn test_focused_cell_is_the_spec_row() {
    let p = palette();
    assert_cell(
        BaseState::Rest,
        true,
        ExpectedCell {
            background: p.surface_base,
            text: p.text_strong,
            placeholder: p.text_muted,
            icon: p.text_primary,
            border: p.accent_border,
            border_width: shape::BORDER_FOCUS,
        },
    );
}

#[test]
fn test_invalid_cell_is_the_spec_row() {
    let p = palette();
    assert_cell(
        BaseState::Invalid,
        false,
        ExpectedCell {
            background: p.surface_base,
            text: p.text_strong,
            placeholder: p.text_muted,
            icon: p.status_danger_text,
            border: p.status_danger_text,
            border_width: shape::BORDER_FOCUS,
        },
    );
}

#[test]
fn test_disabled_cell_is_the_spec_row() {
    let p = palette();
    assert_cell(
        BaseState::Disabled,
        false,
        ExpectedCell {
            background: p.surface_raised,
            text: p.text_disabled,
            placeholder: p.text_disabled,
            icon: p.text_disabled,
            border: p.border_subtle,
            border_width: shape::BORDER_HAIRLINE,
        },
    );
}

#[test]
fn test_hovered_and_focused_is_the_focused_cell() {
    let focused = style_for(InputKind::TextEntry, BaseState::Rest, true);
    let hovered_focused = style_for(InputKind::TextEntry, BaseState::Hovered, true);

    assert_eq!(
        hovered_focused, focused,
        "a hovered-and-focused input paints the Focused row, not a blend"
    );
}

#[test]
fn test_pressed_folds_into_the_hovered_cell() {
    // §3.2 gives an input no Pressed cell, because pressing one places a
    // caret rather than committing an action. `state::resolve` can still
    // produce `Pressed`, so the resolver has to answer it with something,
    // and the answer is Hovered rather than a sixth invented skin.
    let hovered = style_for(InputKind::Search, BaseState::Hovered, false);
    let pressed = style_for(InputKind::Search, BaseState::Pressed, false);

    assert_eq!(
        pressed, hovered,
        "Pressed must resolve to the Hovered cell, not to a new one"
    );
}

#[test]
fn test_pressed_and_focused_is_the_focused_cell() {
    let focused = style_for(InputKind::TextEntry, BaseState::Rest, true);
    let pressed_focused = style_for(InputKind::TextEntry, BaseState::Pressed, true);

    assert_eq!(pressed_focused, focused);
}

#[test]
fn test_invalid_outranks_the_pointer_states() {
    // The same ruling `state::resolve` makes for buttons, checked at the
    // paint level: clicking at an error must not visually clear it.
    let invalid = style_for(InputKind::TextEntry, BaseState::Invalid, false);

    for base in [BaseState::Hovered, BaseState::Pressed] {
        let pointer = style_for(InputKind::TextEntry, base, false);
        assert_ne!(
            pointer.border_color, invalid.border_color,
            "the {base:?} border must not overwrite the error border"
        );
    }
}

#[test]
fn test_disabled_outranks_everything() {
    let disabled = style_for(InputKind::TextEntry, BaseState::Disabled, false);

    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Invalid,
    ] {
        for focused in [false, true] {
            let other = style_for(InputKind::TextEntry, base, focused);
            assert_ne!(
                token_name(other.background),
                token_name(disabled.background),
                "a {base:?} + focused:{focused} input must not paint the \
                 disabled fill, and vice versa"
            );
        }
    }
}

#[test]
fn test_focus_is_a_replacement_here_not_an_addition() {
    // §3.1 writes a focused button as "Rest + ring": the base cell
    // survives. §3.2 writes its Focused cell as a replacement — the
    // background moves and the border becomes the accent. If a future
    // edit made an input's focus additive the way a button's is, this
    // fails, which is the point: the two atoms are specified
    // differently and the difference is load-bearing.
    let focused = style_for(InputKind::TextEntry, BaseState::Rest, true);
    let rest = style_for(InputKind::TextEntry, BaseState::Rest, false);

    assert_ne!(
        token_name(rest.background),
        token_name(focused.background),
        "§3.2 replaces the background on focus; if these match, focus \
         became additive and the Focused row is no longer the row it \
         specifies"
    );
    assert_eq!(token_name(rest.background), "surface.code");
    assert_eq!(token_name(focused.background), "surface.base");
    assert_eq!(token_name(focused.border_color), "accent.border");
    assert_eq!(focused.border_width, shape::BORDER_FOCUS);
    assert_eq!(rest.border_width, shape::BORDER_HAIRLINE);
    assert_eq!(
        focused.focus_ring_width, 0.0,
        "with nothing competing for the perimeter the 2px accent border \
         IS the focus indicator; a ring on top of it would double the \
         stroke"
    );
}

#[test]
fn test_all_three_kinds_resolve_identically_for_every_reachable_state() {
    // The core claim of `UIF-ATOM-002`: the kind selects slot content,
    // never paint. Every kind across every reachable state must produce a
    // byte-identical `InputStyle`.
    let p = palette();

    for state in reachable_states() {
        let reference = resolve_input_style(InputKind::TextEntry, state, &p);

        for kind in KINDS {
            assert_eq!(
                resolve_input_style(kind, state, &p),
                reference,
                "{kind:?} diverged from TextEntry at {} — the kind may \
                 change what sits in the slot, not what the cell looks like",
                state_name(state),
            );
        }
    }
}

#[test]
fn test_the_selection_pair_is_the_lock_in_every_state() {
    // `UIF-TOK-006` rule 3: the selection foreground is `text.strong`
    // over `accent.fill`, in every state a selection can exist in.
    let p = palette();

    for state in reachable_states() {
        for kind in KINDS {
            let style = resolve_input_style(kind, state, &p);
            assert_eq!(
                token_name(style.selection_background),
                "accent.fill",
                "{} x {kind:?} selection background",
                state_name(state),
            );
            assert_eq!(
                token_name(style.selection_foreground),
                "text.strong",
                "{} x {kind:?} selection foreground",
                state_name(state),
            );
        }
    }
}

#[test]
fn test_resolve_input_style_is_pure_and_total() {
    let p = palette();

    for state in reachable_states() {
        for kind in KINDS {
            let once = resolve_input_style(kind, state, &p);
            let twice = resolve_input_style(kind, state, &p);
            assert_eq!(once, twice, "resolver is not pure at {kind:?}");
        }
    }
}

#[test]
fn test_every_reachable_state_projects_to_a_named_cell() {
    // Totality, but checked as a *named* mapping: each reachable state
    // must land on one of the six cells, and the set of cells the
    // reachable states actually hit is pinned.
    let mut hit: Vec<PaintedCell> = Vec::new();

    for state in reachable_states() {
        let cell = painted_cell(state);
        if !hit.contains(&cell) {
            hit.push(cell);
        }
    }

    for expected in [
        PaintedCell::Rest,
        PaintedCell::Hovered,
        PaintedCell::Focused,
        PaintedCell::Invalid,
        PaintedCell::InvalidFocused,
        PaintedCell::Disabled,
    ] {
        assert!(
            hit.contains(&expected),
            "cell {expected:?} is unreachable from the state machine — \
             a specified row of §3.2 has gone dead"
        );
    }
}

// ---------------------------------------------------------------------------
// 2 — the contested perimeter: invalid + focused
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_and_focused_keeps_the_error_border() {
    let p = palette();
    let style = style_for(InputKind::TextEntry, BaseState::Invalid, true);

    assert_eq!(
        token_name(style.border_color),
        "status.danger_text",
        "focus must not take the perimeter from the error"
    );
    assert_eq!(style.border_width, shape::BORDER_FOCUS);
    assert!(
        style.is_error_border(&p),
        "the resolved style must still report an error border while focused"
    );
}

#[test]
fn test_invalid_and_focused_relocates_focus_to_an_outside_ring() {
    let style = style_for(InputKind::TextEntry, BaseState::Invalid, true);

    assert_eq!(
        style.focus_ring_width,
        shape::BORDER_FOCUS,
        "the keyboard user still needs a focus signal once the error owns \
         the border"
    );
    assert_eq!(
        token_name(style.focus_ring_color),
        "accent.border",
        "accent.border clears surface.base at 6.6698:1, so it wins the ring"
    );
}

#[test]
fn test_the_ring_is_resolved_against_the_background_this_function_resolved() {
    // Not against the kind, not against a constant: against the
    // background the same resolution produced (`UIF-TOK-006` rule 2,
    // decision D-6).
    let p = palette();

    for state in reachable_states() {
        let style = resolve_input_style(InputKind::Search, state, &p);

        if style.focus_ring_width > 0.0 {
            assert_eq!(
                style.focus_ring_color,
                focus::resolve_ring_color_for_background(style.background, &p),
                "ring colour must come from the resolved background, not \
                 from anywhere else"
            );
        }
    }
}

#[test]
fn test_only_invalid_and_focused_produces_a_ring() {
    for state in reachable_states() {
        let style = resolve_input_style(InputKind::TextEntry, state, &palette());
        let expect_ring = state.base == BaseState::Invalid && state.focused;

        assert_eq!(
            style.focus_ring_width > 0.0,
            expect_ring,
            "ring presence wrong at {}",
            state_name(state),
        );
    }
}

#[test]
fn test_the_error_border_is_never_cleared_by_any_pointer_or_focus_state() {
    // The invariant `state::resolve` ranks for, re-asserted at the paint
    // level across every way of reaching an invalid input.
    for focused in [false, true] {
        for kind in KINDS {
            let style = style_for(kind, BaseState::Invalid, focused);
            assert_eq!(
                token_name(style.border_color),
                "status.danger_text",
                "invalid + focused:{focused} x {kind:?} lost the error border"
            );
            assert_eq!(
                token_name(style.icon),
                "status.danger_text",
                "invalid + focused:{focused} x {kind:?} lost the error glyph colour"
            );
        }
    }
}

#[test]
fn test_the_ring_layer_lives_outside_the_input_bounds() {
    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(200.0, HEIGHT_STANDARD));
    let style = style_for(InputKind::Search, BaseState::Invalid, true);

    let ring = focus_ring_layer(bounds, &style).expect("invalid + focused draws a ring");

    assert_eq!(
        ring.bounds,
        bounds.expand(shape::BORDER_FOCUS),
        "the ring must occupy the annulus one ring-width out, so the fill \
         and the error border are untouched"
    );
    assert_eq!(ring.fill.a, 0.0, "the ring layer paints no fill");
    assert_eq!(ring.border.color, style.focus_ring_color);
    assert_eq!(
        ring.border,
        Border::default()
            .color(style.focus_ring_color)
            .width(shape::BORDER_FOCUS)
            .rounded(RADIUS + shape::BORDER_FOCUS),
        "the ring's curve must stay concentric with the box it surrounds"
    );
}

#[test]
fn test_no_ring_layer_exists_without_a_ring() {
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, HEIGHT_STANDARD));

    for state in reachable_states() {
        let style = resolve_input_style(InputKind::TextEntry, state, &palette());
        let ring = focus_ring_layer(bounds, &style);

        assert_eq!(
            ring.is_some(),
            style.focus_ring_width > 0.0,
            "ring layer presence disagrees with the resolved width at {}",
            state_name(state),
        );
    }
}

#[test]
fn test_every_cell_paints_a_background_layer() {
    // §3.2 has no transparent cell: an input always has a box, unlike a
    // Ghost button. A missing layer here would mean an input that reads as
    // whatever is behind it.
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, HEIGHT_STANDARD));

    for state in reachable_states() {
        for kind in KINDS {
            let style = resolve_input_style(kind, state, &palette());
            let layer = background_layer(bounds, &style);

            assert!(
                layer.is_some(),
                "{} x {kind:?} painted no background — every §3.2 cell has a box",
                state_name(state),
            );
            let layer = layer.expect("checked above");
            assert_eq!(layer.bounds, bounds);
            assert_eq!(layer.fill, style.background);
            assert_eq!(layer.border.color, style.border_color);
            assert_eq!(layer.border.width, style.border_width);
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — the reserved slot
// ---------------------------------------------------------------------------

#[test]
fn test_reserved_slot_width_per_kind() {
    assert_eq!(InputKind::Search.trailing_slot_width(), size::CONTROL_SM);
    assert_eq!(
        InputKind::TextEntry.trailing_slot_width(),
        size::HIT_TARGET_MIN
    );
    assert_eq!(
        InputKind::DropdownTrigger.trailing_slot_width(),
        size::HIT_TARGET_MIN
    );
}

#[test]
fn test_every_reserved_slot_clears_the_hit_target_floor() {
    for kind in KINDS {
        assert!(
            kind.trailing_slot_width() >= size::HIT_TARGET_MIN,
            "{kind:?} reserves {}pt, under the {}pt hit-target floor",
            kind.trailing_slot_width(),
            size::HIT_TARGET_MIN,
        );
    }
}

#[test]
fn test_the_search_reservation_is_sized_to_the_pinned_action_button() {
    // §4.3.1's pinned search footer puts a 28 × 28 action button in this
    // slot, so the search reservation is `size.control.sm` and not the
    // generic floor. If the reservation ever dropped to HIT_TARGET_MIN the
    // clear button would not fit and someone would "fix" it by moving the
    // text.
    assert_eq!(InputKind::Search.trailing_slot_width(), HEIGHT_COMPACT);
    assert!(InputKind::Search.trailing_slot_width() > InputKind::TextEntry.trailing_slot_width());
}

#[test]
fn test_text_area_is_identical_for_every_slot_content() {
    // The zero-shift invariant, stated the way the code enforces it:
    // `text_area_bounds` has no parameter for the slot's content, so
    // every content value must return the same rectangle.
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(320.0, HEIGHT_STANDARD));
    let contents = [
        SlotContent::Empty,
        SlotContent::Passive(GLYPH_SEARCH),
        SlotContent::Passive(GLYPH_ERROR),
        SlotContent::Passive(GLYPH_CHEVRON),
        SlotContent::Actionable(GLYPH_CLEAR),
    ];

    for kind in KINDS {
        let reference = text_area_bounds(bounds, kind);

        for content in contents {
            // `content` is deliberately unused by the function under
            // test; binding it here is what makes the attempt to shift
            // the text explicit rather than implied.
            let _ = content;
            assert_eq!(
                text_area_bounds(bounds, kind),
                reference,
                "{kind:?} text area moved with slot content {content:?}"
            );
        }
    }
}

#[test]
fn test_text_area_formula_is_padding_against_the_reservation() {
    let bounds = Rectangle::new(Point::new(5.0, 7.0), Size::new(300.0, HEIGHT_STANDARD));

    for kind in KINDS {
        let area = text_area_bounds(bounds, kind);
        let reserved = HORIZONTAL_PADDING + SLOT_GAP + kind.trailing_slot_width();

        assert_eq!(area.x, bounds.x + HORIZONTAL_PADDING);
        assert_eq!(area.y, bounds.y);
        assert_eq!(area.height, bounds.height);
        assert_eq!(area.width, bounds.width - HORIZONTAL_PADDING - reserved);
    }
}

#[test]
fn test_text_area_and_slot_partition_the_row() {
    // The two functions are complements: text + gap + slot + padding must
    // account for the row exactly, with no overlap and no leftover.
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(260.0, HEIGHT_COMPACT));

    for kind in KINDS {
        let area = text_area_bounds(bounds, kind);
        let slot = trailing_slot_bounds(bounds, kind);

        assert!(
            area.x + area.width + SLOT_GAP <= slot.x + f32::EPSILON,
            "{kind:?}: the text area overlaps the slot"
        );
        assert!(
            slot.x + slot.width <= bounds.x + bounds.width + f32::EPSILON,
            "{kind:?}: the slot escapes the field"
        );
        assert_eq!(
            slot.x + slot.width + HORIZONTAL_PADDING,
            bounds.x + bounds.width,
            "{kind:?}: the row is not fully accounted for"
        );
    }
}

#[test]
fn test_a_field_narrower_than_its_chrome_collapses_rather_than_going_negative() {
    // A viewport smaller than padding + gap + slot. The text area
    // collapses to zero; it must not go negative, which iced would turn
    // into a wrapped or inverted layout.
    let tight = Rectangle::new(Point::new(0.0, 0.0), Size::new(4.0, HEIGHT_COMPACT));

    for kind in KINDS {
        let area = text_area_bounds(tight, kind);
        assert!(
            area.width >= 0.0,
            "{kind:?} text width went negative: {}",
            area.width
        );
        assert_eq!(area.width, 0.0);

        let slot = trailing_slot_bounds(tight, kind);
        assert!(
            slot.x >= tight.x,
            "{kind:?} slot x escaped the field's left edge: {}",
            slot.x
        );
    }
}

#[test]
fn test_the_text_area_clears_the_widest_border_it_sits_inside() {
    // Every §3.2 cell strokes at most `shape::BORDER_FOCUS` on the
    // inside of the box. The inset is read off the geometry rather than
    // compared as two constants, so the check follows the function if the
    // function ever moves.
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, HEIGHT_STANDARD));

    for kind in KINDS {
        let inset = text_area_bounds(bounds, kind).x - bounds.x;

        assert!(
            inset > shape::BORDER_FOCUS,
            "{kind:?}: the text inset {inset} does not clear the widest \
             border the atom paints ({})",
            shape::BORDER_FOCUS,
        );
    }
}

#[test]
fn test_input_geometry_is_sourced_from_tokens_not_restated() {
    assert_eq!(HEIGHT_STANDARD, size::CONTROL_MD);
    assert_eq!(HEIGHT_COMPACT, size::CONTROL_SM);
    assert_eq!(HORIZONTAL_PADDING, space::MD);
    assert_eq!(SLOT_GAP, space::SM);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(SLOT_GLYPH_SIZE, size::ICON_MD);
}

const _: () = assert!(
    HEIGHT_STANDARD > HEIGHT_COMPACT,
    "the two control rungs must be distinct"
);
const _: () = assert!(
    size::HIT_TARGET_MIN <= HEIGHT_COMPACT,
    "the hit-target floor must fit inside the compact input height"
);
const _: () = assert!(
    HORIZONTAL_PADDING > shape::BORDER_FOCUS,
    "text must clear the widest border the atom paints"
);

#[test]
fn test_slot_content_maps_to_glyph_and_actionability() {
    assert_eq!(SlotContent::Empty.glyph(), None);
    assert!(!SlotContent::Empty.is_actionable());

    assert_eq!(SlotContent::Passive(GLYPH_ERROR).glyph(), Some(GLYPH_ERROR));
    assert!(!SlotContent::Passive(GLYPH_ERROR).is_actionable());

    assert_eq!(
        SlotContent::Actionable(GLYPH_CLEAR).glyph(),
        Some(GLYPH_CLEAR)
    );
    assert!(SlotContent::Actionable(GLYPH_CLEAR).is_actionable());
}

#[test]
fn test_text_entry_slot_is_empty_until_it_is_invalid() {
    for has_value in [false, true] {
        assert_eq!(
            slot_content(InputKind::TextEntry, has_value, false),
            SlotContent::Empty,
            "a plain entry reserves the slot but shows nothing in it \
             (has_value: {has_value})",
        );
        assert_eq!(
            slot_content(InputKind::TextEntry, has_value, true),
            SlotContent::Passive(GLYPH_ERROR),
        );
    }
}

#[test]
fn test_search_slot_is_the_search_mark_when_empty_and_clear_when_filled() {
    assert_eq!(
        slot_content(InputKind::Search, false, false),
        SlotContent::Passive(GLYPH_SEARCH)
    );
    assert_eq!(
        slot_content(InputKind::Search, true, false),
        SlotContent::Actionable(GLYPH_CLEAR)
    );
}

#[test]
fn test_dropdown_slot_always_carries_the_chevron() {
    for has_value in [false, true] {
        for invalid in [false, true] {
            let content = slot_content(InputKind::DropdownTrigger, has_value, invalid);
            if invalid {
                assert_eq!(content, SlotContent::Passive(GLYPH_ERROR));
            } else {
                assert_eq!(content, SlotContent::Passive(GLYPH_CHEVRON));
            }
        }
    }
}

#[test]
fn test_validation_outranks_the_clear_affordance() {
    // The one precedence rule in `slot_content`, pinned directly: an
    // invalid field shows the error even when it also holds a value that
    // could be cleared, because the error is why the user cannot proceed.
    let filled_and_invalid = slot_content(InputKind::Search, true, true);

    assert_eq!(filled_and_invalid, SlotContent::Passive(GLYPH_ERROR));
    assert!(
        !filled_and_invalid.is_actionable(),
        "an invalid search field must not offer a clear in place of its \
         error — the clear would hide the reason the value is wrong"
    );
}

#[test]
fn test_only_the_search_clear_is_actionable() {
    for kind in KINDS {
        for has_value in [false, true] {
            for invalid in [false, true] {
                let content = slot_content(kind, has_value, invalid);
                assert_eq!(
                    content.is_actionable(),
                    kind == InputKind::Search && has_value && !invalid,
                    "{kind:?} has_value:{has_value} invalid:{invalid} \
                     actionability wrong",
                );
            }
        }
    }
}

#[test]
fn test_the_error_glyph_is_the_same_for_every_kind() {
    // The glyph is a named constant rather than three literals, so a
    // future dedicated error glyph changes one line.
    for kind in KINDS {
        assert_eq!(
            slot_content(kind, true, true),
            SlotContent::Passive(GLYPH_ERROR),
            "{kind:?} reports a different error mark than the constant",
        );
    }
}

#[test]
fn test_the_four_glyphs_are_distinct_characters() {
    let glyphs = [GLYPH_SEARCH, GLYPH_CLEAR, GLYPH_ERROR, GLYPH_CHEVRON];
    for (i, a) in glyphs.iter().enumerate() {
        for (j, b) in glyphs.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "two slot glyphs share a character");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4 — real layout read-back
// ---------------------------------------------------------------------------

/// The message type a host would drive an input with.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Edit(String),
    Clear,
}

/// Build one input as a host would, at a fixed width.
fn field(
    kind: InputKind,
    value: &str,
    invalid: bool,
    enabled: bool,
    compact: bool,
    width: f32,
) -> Element<'static, Msg, crate::test_helpers::Theme, HeadlessRenderer> {
    let value = value.to_owned();
    let mut input =
        TextInput::new(kind, "hint", value, Msg::Edit, "field").width(Length::Fixed(width));

    if compact {
        input = input.compact();
    }
    if invalid {
        input = input.invalid();
    }
    if !enabled {
        input = input.disabled();
    }

    input.into()
}

/// The outer bounds a `field` of `width` lays out to at the root.
fn outer(width: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::new(0.0, 0.0), Size::new(width, height))
}

/// The editor's registered bounds, which is where iced actually put it.
fn editor_bounds(
    kind: InputKind,
    value: &str,
    invalid: bool,
    enabled: bool,
    width: f32,
) -> Rectangle {
    let value = value.to_owned();
    let mut harness =
        Harness::headless(move || field(kind, &value, invalid, enabled, false, width));
    let recorded = harness.recorded();

    assert_eq!(
        recorded.len(),
        1,
        "a text input registers exactly one focusable node, got {recorded:?}"
    );
    recorded[0].1
}

#[test]
fn test_the_input_registers_with_the_router_under_its_bridge_id() {
    let mut harness =
        Harness::headless(|| field(InputKind::TextEntry, "", false, true, false, 240.0));
    let recorded = harness.recorded();

    assert_eq!(
        recorded[0].0,
        Some(iced_id("field")),
        "the editor must register at the same address the bridge pushes to"
    );
}

#[test]
fn test_the_editor_lays_out_in_the_text_area_not_the_whole_field() {
    let width = 240.0;

    for kind in KINDS {
        let bounds = editor_bounds(kind, "", false, true, width);
        let want = text_area_bounds(outer(width, HEIGHT_STANDARD), kind);

        assert_eq!(
            bounds.x, want.x,
            "{kind:?}: the editor's left edge is not the text area's"
        );
        assert_eq!(
            bounds.width, want.width,
            "{kind:?}: the editor's width is not the text area's — if it \
             were the whole field, the reserved slot would be painted over"
        );
    }
}

#[test]
fn test_the_editor_bounds_do_not_move_when_the_slot_content_changes() {
    // THE zero-shift assertion, at real-layout level. Three states of a
    // search field: idle 🔍, clear ×, and error ⚠ — three different
    // glyphs of three different widths, and iced laid the editor out into
    // the same rectangle for all of them.
    let width = 240.0;
    let idle = editor_bounds(InputKind::Search, "", false, true, width);
    let clear = editor_bounds(InputKind::Search, "a filter", false, true, width);
    let error = editor_bounds(InputKind::Search, "a filter", true, true, width);

    assert_eq!(
        idle, clear,
        "swapping 🔍 for × moved the editor: {idle:?} -> {clear:?}"
    );
    assert_eq!(
        idle, error,
        "swapping 🔍 for ⚠ moved the editor: {idle:?} -> {error:?}"
    );
}

#[test]
fn test_the_editor_bounds_do_not_move_across_kinds_at_the_same_width() {
    // Only the reservation differs between kinds, and that difference is
    // known to the geometry: what must not vary is the *glyph*. So the
    // three kinds differ in width by exactly the difference in their
    // reservations, and by nothing else.
    let width = 240.0;
    let entry = editor_bounds(InputKind::TextEntry, "", false, true, width);
    let search = editor_bounds(InputKind::Search, "", false, true, width);
    let trigger = editor_bounds(InputKind::DropdownTrigger, "", false, true, width);

    assert_eq!(
        entry.width - search.width,
        InputKind::Search.trailing_slot_width() - InputKind::TextEntry.trailing_slot_width(),
        "the width difference between kinds must be exactly the \
         reservation difference"
    );
    assert_eq!(
        entry, trigger,
        "TextEntry and DropdownTrigger reserve the \
                               same width and must lay out identically"
    );
}

#[test]
fn test_the_editor_bounds_do_not_move_as_the_value_grows() {
    // A long value must not push the field's geometry either: the editor
    // scrolls, the box does not resize.
    let width = 240.0;
    let short = editor_bounds(InputKind::TextEntry, "a", false, true, width);
    let long = editor_bounds(
        InputKind::TextEntry,
        "a very much longer value than the field was ever sized for",
        false,
        true,
        width,
    );

    assert_eq!(short, long, "a longer value resized the editor's box");
}

#[test]
fn test_compact_and_standard_inputs_centre_their_editor_in_their_own_height() {
    // The editor node is the text's natural height in both rungs; what
    // differs is the box it is centred in. `align(Center)` puts the
    // editor's centre on the area's centre line, which is the outer
    // height over two — so the two rungs are distinguishable by the
    // editor's `y`, not by its `height`.
    let width = 200.0;
    let standard = editor_bounds(InputKind::TextEntry, "", false, true, width);
    let compact = {
        let mut harness =
            Harness::headless(move || field(InputKind::TextEntry, "", false, true, true, width));
        harness.recorded()[0].1
    };

    let centre_of = |r: Rectangle| r.y + r.height / 2.0;

    assert!(
        (centre_of(standard) - HEIGHT_STANDARD / 2.0).abs() < 1.0,
        "standard editor centre {} is not on the {HEIGHT_STANDARD}pt \
         centre line",
        centre_of(standard),
    );
    assert!(
        (centre_of(compact) - HEIGHT_COMPACT / 2.0).abs() < 1.0,
        "compact editor centre {} is not on the {HEIGHT_COMPACT}pt \
         centre line",
        centre_of(compact),
    );
    assert_ne!(
        standard.y, compact.y,
        "the two rungs centred the editor at the same height"
    );
    assert_eq!(standard.x, compact.x);
    assert_eq!(standard.width, compact.width);
}

#[test]
fn test_an_enabled_input_holds_keyboard_focus() {
    let mut harness =
        Harness::headless(|| field(InputKind::TextEntry, "", false, true, false, 240.0));
    let id = iced_id("field");

    assert!(
        !harness.is_focused(&id),
        "registration alone focuses nothing"
    );
    harness.focus(&id);
    assert!(
        harness.is_focused(&id),
        "an enabled input must be able to hold focus — it is one of the \
         two Focusable widgets this framework leans on"
    );
}

#[test]
fn test_a_disabled_input_cannot_hold_keyboard_focus() {
    let mut harness =
        Harness::headless(|| field(InputKind::TextEntry, "", false, false, false, 240.0));
    let id = iced_id("field");

    harness.focus(&id);

    assert!(
        !harness.is_focused(&id),
        "a disabled input must be evicted from the address it registers at"
    );
    assert_eq!(
        harness.focused(),
        None,
        "nothing may report focus while the only input is disabled"
    );
}

#[test]
fn test_disabling_evicts_a_focus_the_input_already_held() {
    // The regression the `operate` eviction exists for: focused while
    // enabled, disabled afterwards. Without the eviction the input keeps
    // a focus nothing can see or leave.
    let mut harness =
        Harness::headless(|| field(InputKind::TextEntry, "", false, true, false, 240.0));
    let id = iced_id("field");

    harness.focus(&id);
    assert!(harness.is_focused(&id));

    harness.set_view(|| field(InputKind::TextEntry, "", false, false, false, 240.0));
    assert!(
        !harness.is_focused(&id),
        "a field disabled after being focused must drop the focus on the \
         next pass, not keep a lit ring nothing can leave"
    );
}

#[test]
fn test_a_disabled_input_publishes_no_message_at_all() {
    let mut harness =
        Harness::headless(|| field(InputKind::Search, "value", false, false, false, 240.0));
    let bounds = outer(240.0, HEIGHT_STANDARD);
    let slot = trailing_slot_bounds(bounds, InputKind::Search);

    let mut messages = Vec::new();
    messages.extend(harness.hover(centre(bounds)));
    messages.extend(harness.press(centre(bounds)));
    messages.extend(harness.release(centre(bounds)));
    messages.extend(harness.press(centre(slot)));
    messages.extend(harness.release(centre(slot)));

    assert!(
        messages.is_empty(),
        "a disabled input published {messages:?}"
    );
}

#[test]
fn test_pressing_the_clear_slot_publishes_clear_and_nothing_else() {
    let mut harness = Harness::headless(|| {
        TextInput::new(InputKind::Search, "hint", "a filter", Msg::Edit, "field")
            .on_clear(Msg::Clear)
            .width(Length::Fixed(240.0))
            .into()
    });
    let slot = trailing_slot_bounds(outer(240.0, HEIGHT_STANDARD), InputKind::Search);

    let pressed = harness.press(centre(slot));
    let released = harness.release(centre(slot));

    assert!(
        pressed.is_empty(),
        "the press must not publish; the release commits"
    );
    assert_eq!(
        released,
        vec![Msg::Clear],
        "press and release inside the actionable slot publishes the clear message"
    );
}

#[test]
fn test_pressing_the_text_area_never_publishes_clear() {
    let mut harness = Harness::headless(|| {
        TextInput::new(InputKind::Search, "hint", "a filter", Msg::Edit, "field")
            .on_clear(Msg::Clear)
            .width(Length::Fixed(240.0))
            .into()
    });
    let area = text_area_bounds(outer(240.0, HEIGHT_STANDARD), InputKind::Search);
    let point = centre(area);

    let messages = [
        harness.hover(point),
        harness.press(point),
        harness.release(point),
    ]
    .concat();

    assert!(
        !messages.contains(&Msg::Clear),
        "the clear affordance fired outside its own slot: {messages:?}"
    );
}

#[test]
fn test_a_passive_slot_publishes_nothing() {
    // The idle 🔍 and the error ⚠ are not buttons.
    for (value, invalid) in [("", false), ("bad", true)] {
        let mut harness = Harness::headless(move || {
            let input = TextInput::new(InputKind::Search, "hint", value, Msg::Edit, "field")
                .on_clear(Msg::Clear)
                .width(Length::Fixed(240.0));

            if invalid {
                input.invalid().into()
            } else {
                input.into()
            }
        });
        let slot = trailing_slot_bounds(outer(240.0, HEIGHT_STANDARD), InputKind::Search);

        let messages = [
            harness.hover(centre(slot)),
            harness.press(centre(slot)),
            harness.release(centre(slot)),
        ]
        .concat();

        assert!(
            messages.is_empty(),
            "a passive slot (value {value:?}, invalid {invalid}) published {messages:?}"
        );
    }
}

#[test]
fn test_the_widget_draws_through_the_real_paint_path_in_every_state() {
    // Nothing is asserted about pixels: the claim is that `draw` runs to
    // completion against a real renderer — background layer, ring layer,
    // inner editor and slot glyph — for every kind in every state.
    for kind in KINDS {
        for invalid in [false, true] {
            for enabled in [false, true] {
                let mut harness = Harness::headless(move || {
                    field(kind, "some value", invalid, enabled, false, 240.0)
                });
                harness.render();
            }
        }
    }
}

#[test]
fn test_the_widget_survives_a_hover_and_focus_cycle() {
    let mut harness =
        Harness::headless(|| field(InputKind::Search, "a filter", false, true, false, 240.0));
    let id = iced_id("field");
    let bounds = outer(240.0, HEIGHT_STANDARD);

    harness.hover(Point::new(-50.0, -50.0));
    harness.focus(&id);
    harness.hover(centre(bounds));
    harness.render();
    harness.hover(Point::new(500.0, 500.0));
    harness.render();

    assert!(harness.is_focused(&id), "hover must not steal focus away");
}

#[test]
fn test_two_inputs_in_a_column_both_register() {
    let mut harness = Harness::headless(|| {
        column(vec![
            field(InputKind::TextEntry, "", false, true, false, 240.0),
            field(InputKind::Search, "", false, true, false, 240.0),
        ])
        .into()
    });

    let recorded = harness.recorded();
    assert_eq!(recorded.len(), 2, "both inputs must register");
    assert_eq!(
        recorded[0].1.x, recorded[1].1.x,
        "both inputs start their text area at the same inset"
    );
    assert!(
        recorded[0].1.width > recorded[1].1.width,
        "the entry reserves less than the search field, so its text area \
         must be wider"
    );
}

#[test]
fn test_cursor_events_outside_the_field_are_left_alone() {
    let mut harness =
        Harness::headless(|| field(InputKind::Search, "v", false, true, false, 240.0));
    let messages = [
        harness.hover(Point::new(1_000.0, 1_000.0)),
        harness.press(Point::new(1_000.0, 1_000.0)),
        harness.release(Point::new(1_000.0, 1_000.0)),
    ]
    .concat();

    assert!(
        !messages.contains(&Msg::Clear),
        "a release far outside the field published {messages:?}"
    );
}

#[test]
fn test_moved_event_helper_reaches_the_widget() {
    // Guards against a harness refactor silently turning `hover` into a
    // no-op: the same cursor delivered through `pointer` and `hover`
    // must produce the same message list.
    let mut a = Harness::headless(|| field(InputKind::Search, "v", false, true, false, 240.0));
    let mut b = Harness::headless(|| field(InputKind::Search, "v", false, true, false, 240.0));
    let at = Point::new(30.0, 16.0);

    assert_eq!(
        a.pointer(&[moved(at)], mouse::Cursor::Available(at)),
        b.hover(at)
    );
}

// ---------------------------------------------------------------------------
// 5 — the contrast audit (UIF-VER-001)
// ---------------------------------------------------------------------------

/// How a produced colour pair is judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// Judged against a threshold: text owes [`MIN_TEXT_CONTRAST`],
    /// active chrome owes [`MIN_CHROME_CONTRAST`].
    Judged,
    /// `border.subtle` — a **resting** divider. `UIF-TOK-006` rule 2
    /// governs *active* chrome, and `contrast` already establishes that
    /// this token is not active indicator material. Reported, not gated.
    RestingDivider,
    /// The stroke carries the same channels as the fill it sits on, so it
    /// paints nothing visible.
    CoincidentWithFill,
    /// Nothing is painted behind this channel.
    UnpaintedBackground,
}

/// One measured pair from the resolver's output.
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

/// Classify and measure one produced pair.
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
    // Translucent fill: judge the worst container in the ladder.
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

/// Every `(foreground, background)` pair `resolve_input_style` can paint,
/// across all three kinds and every reachable state, deduplicated by
/// `(channel, fg, bg)` with the cells that produce each one recorded.
fn audit_rows() -> Vec<AuditRow> {
    let p = palette();
    let mut rows: Vec<AuditRow> = Vec::new();

    for kind in KINDS {
        for state in reachable_states() {
            let style = resolve_input_style(kind, state, &p);
            let at = format!("{kind:?} x {}", state_name(state));

            let produced: [(&str, Color, Color, f32); 5] = [
                ("value", style.text, style.background, MIN_TEXT_CONTRAST),
                (
                    "placeholder",
                    style.placeholder,
                    style.background,
                    MIN_TEXT_CONTRAST,
                ),
                ("icon", style.icon, style.background, MIN_TEXT_CONTRAST),
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
                if channel == "border" && style.border_width <= 0.0 {
                    continue;
                }
                if channel == "ring" && style.focus_ring_width <= 0.0 {
                    continue;
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
        "\n{:<12} {:<26} {:<26} {:>8} {:>5}  {:<20} {}\n",
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
            "{:<12} {:<26} {:<26} {:>8} {:>5}  {:<20} {}\n",
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
fn test_the_audit_covers_every_kind_and_reachable_state() {
    let rows = audit_rows();
    let cells: Vec<&str> = rows
        .iter()
        .flat_map(|r| r.cells.iter().map(String::as_str))
        .collect();

    for kind in KINDS {
        for state in reachable_states() {
            let want = format!("{kind:?} x {}", state_name(state));
            assert!(
                cells.contains(&want.as_str()),
                "the audit never reached {want}"
            );
        }
    }
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
/// **`INC-08b` found none.** Every pair §3.2 can paint clears its rule on
/// the `G-7`-retuned palette; the tightest is `border.strong` on
/// `surface.code` at `4.1672:1` against a `3.0:1` chrome floor, and the
/// tightest text pair is the selection lock at `4.8020:1` over `4.5:1`.
///
/// The type and the guard that reads it are kept rather than deleted on
/// purpose. An unreadable pairing should still fail into a named, argued
/// record that a human has to add by hand — not into a bare assertion
/// someone can satisfy by deleting the threshold.
fn accepted_failures() -> Vec<AcceptedFailure> {
    Vec::new()
}

#[test]
fn test_the_accepted_failure_ledger_is_empty_and_fully_specified() {
    let accepted = accepted_failures();
    assert!(
        accepted.is_empty(),
        "INC-08b found no unreadable pairing; a new entry must be a \
         supervisor decision, not a convenience — found {:?}",
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

#[test]
fn test_the_selection_lock_clears_the_text_rule() {
    // `UIF-TOK-006` rule 4, measured rather than trusted: `text.strong`
    // on `accent.fill` is the tightest text pair this atom produces, and
    // the revised §2.5 values put it at 4.8020:1.
    let p = palette();
    let ratio = contrast_ratio(p.text_strong, p.accent_fill);

    assert!(
        (ratio - 4.8020).abs() < RATIO_EPS,
        "the selection lock measures {ratio:.4}:1, not the 4.8020:1 the revised \
         spec pins — a token moved"
    );
    assert!(
        meets_text_contrast(p.text_strong, p.accent_fill),
        "the selection lock must clear 4.5:1; it measures {ratio:.4}:1"
    );
}

#[test]
fn test_every_focus_ring_clears_the_chrome_rule() {
    let p = palette();

    for kind in KINDS {
        for state in reachable_states() {
            let style = resolve_input_style(kind, state, &p);
            if style.focus_ring_width <= 0.0 {
                continue;
            }

            assert!(
                meets_chrome_contrast(style.focus_ring_color, style.background),
                "{} x {kind:?}: the focus ring {} on {} measures {:.4}:1, \
                 under the 3.0:1 chrome floor",
                state_name(state),
                token_name(style.focus_ring_color),
                token_name(style.background),
                contrast_ratio(style.focus_ring_color, style.background),
            );
        }
    }
}

#[test]
fn test_the_error_border_clears_the_chrome_rule_on_its_own_cell() {
    let p = palette();
    let style = style_for(InputKind::TextEntry, BaseState::Invalid, false);

    assert!(
        meets_chrome_contrast(style.border_color, style.background),
        "status.danger.text on surface.base measures {:.4}:1",
        contrast_ratio(style.border_color, style.background),
    );
    assert!(
        (contrast_ratio(p.status_danger_text, p.surface_base) - 6.5725).abs() < RATIO_EPS,
        "the error border's measured ratio moved off 6.5725:1"
    );
}

#[test]
fn test_resting_dividers_are_reported_not_gated() {
    let rows = audit_rows();
    let dividers: Vec<&AuditRow> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::RestingDivider)
        .collect();

    assert!(
        !dividers.is_empty(),
        "the Rest/Disabled rows stroke border.subtle; if none is \
         classified a resting divider, the classification broke"
    );

    for r in &dividers {
        assert_eq!(
            r.fg, "border.subtle",
            "only border.subtle is a resting divider"
        );
        assert!(
            r.ratio < MIN_CHROME_CONTRAST,
            "border.subtle on {} measures {:.4}:1 — if that ever clears \
             3.0:1 the classification is stale, not the token",
            r.bg,
            r.ratio,
        );
    }
}

#[test]
fn test_no_judged_pair_is_left_failing() {
    // A direct restatement of the gate, so a change to `accepted_failures`
    // cannot be made without also changing this test's name and intent.
    let failing: Vec<String> = audit_rows()
        .iter()
        .filter(|r| r.verdict == Verdict::Judged && !r.passes())
        .map(|r| format!("{} {} on {} = {:.4}", r.channel, r.fg, r.bg, r.ratio))
        .collect();

    assert!(
        failing.is_empty(),
        "PENDING SUPERVISOR DECISION — judged pairs below their floor: {failing:?}"
    );
}

// ---------------------------------------------------------------------------
// 6 — iced status mapping
// ---------------------------------------------------------------------------

#[test]
fn test_visual_from_status_maps_iced_statuses() {
    let p = |enabled: bool, invalid: bool| {
        move |status: EditorStatus| visual_from_status(status, enabled, invalid)
    };

    let resolve_active = p(true, false);
    assert_eq!(
        resolve_active(EditorStatus::Active),
        VisualState {
            base: BaseState::Rest,
            focused: false
        },
    );
    assert_eq!(
        resolve_active(EditorStatus::Hovered),
        VisualState {
            base: BaseState::Hovered,
            focused: false
        },
    );
    assert_eq!(
        resolve_active(EditorStatus::Focused { is_hovered: true }),
        VisualState {
            base: BaseState::Hovered,
            focused: true
        },
    );
    assert_eq!(
        resolve_active(EditorStatus::Focused { is_hovered: false }),
        VisualState {
            base: BaseState::Rest,
            focused: true
        },
    );
}

#[test]
fn test_visual_from_status_never_reports_pressed() {
    for status in [
        EditorStatus::Active,
        EditorStatus::Hovered,
        EditorStatus::Focused { is_hovered: true },
        EditorStatus::Focused { is_hovered: false },
        EditorStatus::Disabled,
    ] {
        let visual = visual_from_status(status, true, false);
        assert_ne!(
            visual.base,
            BaseState::Pressed,
            "§3.2 gives an input no pressed cell, so {status:?} must not \
             produce one"
        );
    }
}

#[test]
fn test_the_wrapper_enabled_flag_is_the_authority_over_iced_status() {
    // `enabled: false` dominates `state::resolve`, so the wrapper's own
    // flag wins whether iced agrees or not.
    let live_but_disabled = visual_from_status(EditorStatus::Active, false, false);
    assert_eq!(live_but_disabled.base, BaseState::Disabled);

    let dead_but_hovered = visual_from_status(EditorStatus::Disabled, true, false);
    assert_eq!(
        dead_but_hovered.base,
        BaseState::Rest,
        "iced's Disabled status carries no hover, so an enabled wrapper \
         over a dead status lands at Rest rather than at Disabled"
    );
}

#[test]
fn test_invalid_is_carried_from_outside_regardless_of_status() {
    for status in [
        EditorStatus::Active,
        EditorStatus::Hovered,
        EditorStatus::Focused { is_hovered: false },
    ] {
        let visual = visual_from_status(status, true, true);
        assert_eq!(
            visual.base,
            BaseState::Invalid,
            "{status:?} with invalid:true must land on Invalid"
        );
    }
}

#[test]
fn test_the_inner_editor_style_is_transparent_so_the_wrapper_owns_the_chrome() {
    // The wrapper paints the box; the inner editor contributes only
    // text, caret and selection. If the inner editor ever painted its own
    // background the two would double up at the text area's edges.
    let mut harness =
        Harness::headless(|| field(InputKind::TextEntry, "", false, true, false, 240.0));
    let area = text_area_bounds(outer(240.0, HEIGHT_STANDARD), InputKind::TextEntry);
    let bounds = harness.recorded()[0].1;

    assert_eq!(
        (bounds.x, bounds.width),
        (area.x, area.width),
        "the inner editor occupies exactly the text area horizontally — \
         its own chrome must therefore paint nothing outside it"
    );
    assert!(
        bounds.x + bounds.width <= area.x + area.width + f32::EPSILON,
        "the editor spills into the reserved slot"
    );
}

// ---------------------------------------------------------------------------
// 7 — the controlled field, wired (`UIF-ATOM-002a`)
// ---------------------------------------------------------------------------
//
// Everything above audits what a field *looks* like. This section drives
// what it *says*: a gesture goes in, a tier comes out. The host's message
// type is the controller's own emission, so what the harness collects is
// exactly what a host would receive — there is no shim in the middle to
// disagree with about what got routed.
//
// The order each assertion expects is the order the signals actually happen
// in (`route_tiers`): a gesture settles whatever is open, then the focus
// edge moves, then the surviving live tick is delivered.

use crate::binding::Cancel;
use crate::binding::Commit;
use crate::binding::Live;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::ControlledModel;
use crate::widget::text_input_model::TextInputModel;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::Location;
use iced::keyboard::Modifiers;
use iced::keyboard::key;
use iced::mouse;
use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

/// A bound field's host message type is the emission itself.
type Tiered = Element<'static, TextInputEmission, crate::test_helpers::Theme, HeadlessRenderer>;

/// The same field before it is turned into an `Element`.
type BoundField =
    TextInput<'static, TextInputEmission, crate::test_helpers::Theme, HeadlessRenderer>;

/// A host that owns the model a field borrows.
///
/// A controlled field re-seeds its editor from its model on every pass,
/// which is iced's own contract: the host owns the text and the control
/// only reports what happened to it. A test that types more than one
/// character therefore has to play its half, and the only way to hand
/// the same `TextInputModel` out as `'static` while still writing to it
/// is a leaked allocation behind a pointer. Nothing here is ever freed,
/// and nothing here outlives the process.
struct Host {
    model: NonNull<TextInputModel>,
}

impl Host {
    /// A host holding `model`, shared between the view and the test.
    fn new(model: TextInputModel) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            model: NonNull::from(Box::leak(Box::new(model))),
        }))
    }

    /// The projection a field is built from.
    fn view(&self) -> ControlledModel<'static> {
        // SAFETY: the allocation is leaked, never moved and never freed.
        unsafe { self.model.as_ref() }.controlled()
    }

    /// The value the host holds right now.
    fn value(&self) -> String {
        // SAFETY: as above.
        unsafe { self.model.as_ref() }.value.clone()
    }

    /// The host changing its own mind — a sync landing, a paste made
    /// somewhere else, a background refresh writing into the same field.
    fn push(&mut self, value: &str) {
        // SAFETY: as above.
        unsafe { self.model.as_mut() }.value = value.to_owned();
    }

    /// Apply a tier the way the app does: every tier that carries a
    /// value becomes the host's value, and `Cancel` restores the one it
    /// carried rather than the one the field happened to end on.
    fn apply(&mut self, emission: &TextInputEmission) {
        // SAFETY: as above.
        let model = unsafe { self.model.as_mut() };

        match emission {
            TextInputEmission::Changed(live) => model.value = live.value().to_owned(),
            TextInputEmission::Committed(commit) => model.value = commit.value().to_owned(),
            TextInputEmission::Cleared(_) => model.value = String::new(),
            TextInputEmission::Cancelled(cancel) => model.value = cancel.restored().to_owned(),
            TextInputEmission::FocusChanged(_) => {}
        }
    }
}

/// A search field carrying the app's own placeholder.
fn search(value: &str) -> TextInputModel {
    TextInputModel::new(value).with_placeholder("Search notes")
}

/// One bound field at a fixed width, wired to the tiers through a host
/// that writes every tier straight back into its own model.
fn bound(host: Rc<RefCell<Host>>, kind: InputKind) -> Tiered {
    let mirror = host.clone();

    TextInput::controlled(kind, host.borrow().view(), "bound", move |emission| {
        mirror.borrow_mut().apply(&emission);
        Some(emission)
    })
    .width(Length::Fixed(240.0))
    .into()
}

/// A harness around one bound field of `kind`, driven by `host`.
fn bound_harness(
    host: Rc<RefCell<Host>>,
    kind: InputKind,
) -> Harness<TextInputEmission, HeadlessRenderer> {
    Harness::headless(move || bound(host.clone(), kind))
}

/// A bound search field that holds the keyboard.
///
/// The focus edge is drained here rather than in every test: it only
/// becomes observable once an event arrives to carry it, and what each
/// test below wants to assert about is its *own* gesture.
fn focused_search(host: Rc<RefCell<Host>>) -> Harness<TextInputEmission, HeadlessRenderer> {
    let mut harness = bound_harness(host, InputKind::Search);
    harness.focus(&iced_id("bound"));

    assert_eq!(
        drift(&mut harness),
        vec![focus(true)],
        "taking the keyboard reports exactly one edge"
    );
    harness
}

fn changed(value: &str) -> TextInputEmission {
    TextInputEmission::Changed(Live::new(value.to_owned()))
}

fn committed(value: &str) -> TextInputEmission {
    TextInputEmission::Committed(Commit::new(value.to_owned()))
}

fn cleared() -> TextInputEmission {
    TextInputEmission::Cleared(Commit::new(String::new()))
}

fn cancelled(restored: &str) -> TextInputEmission {
    TextInputEmission::Cancelled(Cancel::new(restored.to_owned()))
}

fn focus(on: bool) -> TextInputEmission {
    TextInputEmission::FocusChanged(on)
}

/// A key press carrying no text — Enter, Escape, Backspace.
fn key_event(key: Key, text: Option<&str>) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
        location: Location::Standard,
        modifiers: Modifiers::NONE,
        text: text.map(str::to_owned).map(Into::into),
        repeat: false,
    })
}

/// Nowhere near the field, so a cursor position never enters the result.
fn away() -> Point {
    Point::new(-4000.0, -4000.0)
}

/// Push one event at the field and collect what it published.
fn send(
    harness: &mut Harness<TextInputEmission, HeadlessRenderer>,
    event: Event,
) -> Vec<TextInputEmission> {
    harness.pointer(&[event], mouse::Cursor::Available(away()))
}

/// A mouse move with no gesture in it. The widget still runs a pass,
/// which is when a focus edge becomes observable.
fn drift(harness: &mut Harness<TextInputEmission, HeadlessRenderer>) -> Vec<TextInputEmission> {
    harness.pointer(&[moved(away())], mouse::Cursor::Available(away()))
}

/// A printable character. Iced inserts from `text`, not from the key
/// code, so a faithful keystroke has to carry it.
fn typed(
    harness: &mut Harness<TextInputEmission, HeadlessRenderer>,
    c: char,
) -> Vec<TextInputEmission> {
    let text = c.to_string();
    send(
        harness,
        key_event(Key::Character(text.as_str().into()), Some(&text)),
    )
}

fn named(
    harness: &mut Harness<TextInputEmission, HeadlessRenderer>,
    name: key::Named,
) -> Vec<TextInputEmission> {
    send(harness, key_event(Key::Named(name), None))
}

#[test]
fn test_a_typed_character_becomes_one_live_tick() {
    let mut harness = focused_search(Host::new(search("")));

    assert_eq!(typed(&mut harness, 'a'), vec![changed("a")]);
    assert_eq!(typed(&mut harness, 'b'), vec![changed("ab")]);
}

#[test]
fn test_a_key_that_changes_nothing_emits_no_live_tick() {
    let mut harness = focused_search(Host::new(search("")));

    assert_eq!(
        named(&mut harness, key::Named::Backspace),
        vec![],
        "backspace on an empty field changes nothing, so the coalescer \
         must not wake the host with the string it already had"
    );
}

#[test]
fn test_enter_settles_the_edit_as_a_commit() {
    let mut harness = focused_search(Host::new(search("")));

    typed(&mut harness, 'h');
    typed(&mut harness, 'i');

    assert_eq!(
        named(&mut harness, key::Named::Enter),
        vec![committed("hi")],
        "Enter settles the session and supersedes the pending live tick, \
         so the host must not also be handed `Changed(\"hi\")`"
    );
}

#[test]
fn test_losing_focus_settles_the_edit_and_reports_the_edge() {
    let mut harness = focused_search(Host::new(search("")));

    typed(&mut harness, 'h');
    typed(&mut harness, 'i');
    harness.unfocus();

    assert_eq!(
        drift(&mut harness),
        vec![committed("hi"), focus(false)],
        "a blur commits before it closes the boundary, never after"
    );
}

#[test]
fn test_enter_settles_a_field_that_never_opened_a_session() {
    let mut harness = focused_search(Host::new(search("hello")));

    assert_eq!(
        named(&mut harness, key::Named::Enter),
        vec![committed("hello")],
        "Enter on an untouched field still settles: the host asked for a \
         value and is owed one"
    );
}

#[test]
fn test_releasing_the_clear_slot_settles_the_empty_value_as_a_clear() {
    let host = Host::new(search("hello"));
    let mut harness = bound_harness(host.clone(), InputKind::Search);
    let slot = centre(trailing_slot_bounds(
        outer(240.0, HEIGHT_STANDARD),
        InputKind::Search,
    ));

    harness.hover(slot);
    harness.press(slot);

    assert_eq!(
        harness.release(slot),
        vec![cleared()],
        "the `x` is a tier like every other settled signal, not a \
         `Commit` of an empty string that merely looks the same"
    );
    assert_eq!(
        host.borrow().value(),
        "",
        "the host took the clear as its own value, which is what the \
         next frame paints"
    );
}

#[test]
fn test_escape_reverts_to_the_snapshot_taken_at_the_first_edit() {
    let host = Host::new(search("hello"));
    let mut harness = focused_search(host.clone());

    typed(&mut harness, '!');
    assert_eq!(
        host.borrow().value(),
        "hello!",
        "the live tick moved the host on, as it does in the app"
    );

    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![cancelled("hello")],
        "the snapshot is what the field held when the gesture started, \
         not what it holds now"
    );
    assert_eq!(
        host.borrow().value(),
        "hello",
        "the host restored the value the `Cancel` tier carried, and the \
         field keeps the keyboard to retype into"
    );
}

#[test]
fn test_escape_with_no_session_clears_a_clearable_search() {
    let host = Host::new(search("hello"));
    let mut harness = focused_search(host.clone());

    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![cleared()],
        "with nothing to revert, Escape on a search field that can be \
         cleared clears it — the same affordance the `x` paints"
    );
    assert_eq!(host.borrow().value(), "");
}

#[test]
fn test_escape_never_clears_a_plain_text_entry() {
    let host = Host::new(search("hello"));
    let mut harness = bound_harness(host.clone(), InputKind::TextEntry);
    harness.focus(&iced_id("bound"));
    let _ = drift(&mut harness);

    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![],
        "a key that silently deletes what the user typed, with no `x` \
         ever having offered to, is a key that should not exist"
    );
    assert_eq!(host.borrow().value(), "hello", "left alone");
}

#[test]
fn test_escape_respects_a_field_that_cannot_be_cleared() {
    let host = Host::new(search("hello").with_clearable(false));
    let mut harness = bound_harness(host.clone(), InputKind::Search);
    harness.focus(&iced_id("bound"));
    let _ = drift(&mut harness);

    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![],
        "Escape acts on exactly the predicate that makes the `x` \
         actionable; a field with no affordance has no clear"
    );
}

#[test]
fn test_escape_does_not_revert_a_field_that_does_not_hold_the_keyboard() {
    let mut harness = focused_search(Host::new(search("")));
    let bounds = harness.recorded()[0].1;

    typed(&mut harness, 'x');
    harness.unfocus();
    assert_eq!(drift(&mut harness), vec![committed("x"), focus(false)]);

    // Hovered again, but the keyboard is long gone.
    harness.hover(centre(bounds));

    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![],
        "the wrapper is handed every event while its subtree is on \
         screen; acting on Escape without the focus gate would revert a \
         field the user was never inside"
    );
}

#[test]
fn test_a_disabled_bound_field_says_nothing_and_holds_no_focus() {
    let host = Host::new(search("hello").disabled());
    let mut harness = bound_harness(host.clone(), InputKind::Search);

    harness.focus(&iced_id("bound"));
    let _ = drift(&mut harness);

    assert_eq!(typed(&mut harness, 'z'), vec![]);
    assert_eq!(named(&mut harness, key::Named::Enter), vec![]);
    assert!(
        !harness.is_focused(&iced_id("bound")),
        "a disabled control must not keep a focus nothing can see or leave"
    );
}

#[test]
fn test_a_read_only_bound_field_says_nothing_but_says_so() {
    let host = Host::new(search("hello").read_only());
    let field: BoundField =
        TextInput::controlled(InputKind::Search, host.borrow().view(), "bound", |e| {
            Some(e)
        });

    assert!(
        field.is_read_only(),
        "a bound field answers read-only from its model"
    );

    let mut harness = bound_harness(host.clone(), InputKind::Search);
    harness.focus(&iced_id("bound"));
    let _ = drift(&mut harness);

    assert_eq!(
        typed(&mut harness, 'z'),
        vec![],
        "read-only withholds the callback iced gates edits on, so no \
         edit can reach the tiers"
    );
    assert_eq!(named(&mut harness, key::Named::Enter), vec![]);
}

#[test]
fn test_a_bound_field_reports_its_model_and_says_it_is_bound() {
    let host = Host::new(search("hello"));
    let field: BoundField =
        TextInput::controlled(InputKind::Search, host.borrow().view(), "bound", |e| {
            Some(e)
        });

    assert!(field.is_controlled());
    assert_eq!(
        field.model().map(|bound| bound.get().value.clone()),
        Some("hello".to_owned())
    );

    let legacy: TextInput<'static, Msg, crate::test_helpers::Theme, HeadlessRenderer> =
        TextInput::new(
            InputKind::Search,
            "Search notes",
            "hello",
            Msg::Edit,
            "legacy",
        );
    assert!(
        !legacy.is_controlled(),
        "the legacy path must not claim a binding it does not have"
    );
}

#[test]
fn test_binding_a_field_moves_no_geometry() {
    let width = 240.0;

    for kind in KINDS {
        let mut legacy = Harness::headless(move || {
            TextInput::new(kind, "Search notes", "hello", Msg::Edit, "legacy")
                .width(Length::Fixed(width))
                .into()
        });
        let host = Host::new(search("hello"));
        let mut bound_field = bound_harness(host.clone(), kind);

        assert_eq!(
            bound_field.recorded()[0].1,
            legacy.recorded()[0].1,
            "{kind:?}: putting a field on the binding foundation must \
             not shift a single pixel — `002b`'s zero-shift slot is \
             already shipped and paid for"
        );
    }
}

#[test]
fn test_a_host_pushing_a_new_value_while_focused_is_mirrored_not_reverted() {
    let host = Host::new(search(""));
    let mut harness = focused_search(host.clone());

    typed(&mut harness, 'x');
    host.borrow_mut().push("xy");

    // The host's newer value is observed; the session's snapshot is not
    // overwritten by it, because the snapshot belongs to the gesture.
    assert_eq!(
        named(&mut harness, key::Named::Escape),
        vec![cancelled("")],
        "the snapshot is taken at the first edit of the gesture, not \
         refreshed from the host mid-flight — a revert to a value the \
         user never typed is worse than no revert at all"
    );
    assert_eq!(
        host.borrow().value(),
        "",
        "the restore is the tier's value, not the host's newer one"
    );
}
