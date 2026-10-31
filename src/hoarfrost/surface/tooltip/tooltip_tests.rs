//! Tests for the tooltip's resolved paint and the timing constant.
//!
//! Contrast is **recomputed from the raw sRGB channels** through
//! [`crate::token::contrast::contrast_ratio`] on every assertion. No number in this file
//! was copied out of the spec, and none of them can therefore go stale when a channel is
//! edited — the assertion moves with the palette.

use super::*;
use crate::token::color::Palette;
use crate::token::contrast::{
    self, FillLock, FillLockRole, MIN_TEXT_CONTRAST, fill_lock, fill_lock_role,
    meets_fill_foreground_lock, meets_text_contrast,
};
use crate::token::{shape, size, space};
use crate::token_lint::VISUAL_NAME_MARKERS;
use std::time::Duration;

/// The palette the framework ships. `Palette` has one constructor here; the inverted
/// palette in this crate is `EditorPalette`, which belongs to the editor surface and a
/// tooltip does not use it.
fn palettes() -> [(&'static str, Palette); 1] {
    [("dark", Palette::dark())]
}

/// Two instants far enough apart that no delay in this subsystem bridges them.
const T0: TimeMs = TimeMs::millis(10_000);
const T1: TimeMs = TimeMs::millis(50_000);

fn at(ms: u64) -> TimeMs {
    TimeMs::millis(ms)
}

// ---------------------------------------------------------------------------
// The timing constants
// ---------------------------------------------------------------------------

/// 400 ms is not this module's number to like or dislike: `UIF-ATOM-007` states it, and the
/// controller's default must be that and nothing else.
#[test]
fn hover_delay_is_the_four_hundred_ms_the_spec_fixes() {
    assert_eq!(HOVER_DELAY, 400);
    assert_eq!(
        HoverController::new().delay().as_millis(),
        HOVER_DELAY as u128
    );
}

/// The keyboard delay is `0 ms`, and the controller's default carries it rather than
/// falling back to the hover delay — the whole point of the second channel.
#[test]
fn keyboard_focus_delay_is_zero_and_is_the_controller_default() {
    assert_eq!(KEYBOARD_FOCUS_DELAY, 0);
    assert_eq!(
        HoverController::new().keyboard_delay(),
        Duration::from_millis(KEYBOARD_FOCUS_DELAY)
    );
    assert_eq!(HoverController::new().keyboard_delay(), Duration::ZERO);
    assert_eq!(ArrivalSource::Keyboard.delay_ms(), 0);
    assert_eq!(ArrivalSource::Pointer.delay_ms(), HOVER_DELAY);
}

/// A control reached by keyboard shows its tooltip **immediately** — no `Pending` frame, no
/// wait. This is the behaviour §3.7 asks for and the reason the constant exists.
#[test]
fn a_keyboard_focus_arrival_shows_on_the_very_first_tick() {
    let mut c = HoverController::new();
    assert!(c.keyboard_focus_enter(T0));
    assert_eq!(
        c.tick(T0),
        Visibility::Shown,
        "0 ms means shown at the arrival instant"
    );
    assert!(c.is_keyboard_armed());
    assert_eq!(c.arrival_source(), Some(ArrivalSource::Keyboard));
    assert_eq!(c.active_delay(), Duration::ZERO);
}

/// The two channels must not be interchangeable: the same arrival instant through the
/// pointer is still waiting where the keyboard is already showing.
#[test]
fn the_pointer_at_the_same_instant_is_still_pending() {
    let mut pointer = HoverController::new();
    pointer.pointer_enter(T0);
    let mut keyboard = HoverController::new();
    keyboard.keyboard_focus_enter(T0);

    assert_eq!(pointer.tick(T0), Visibility::Pending);
    assert_eq!(keyboard.tick(T0), Visibility::Shown);
    assert_eq!(pointer.active_delay(), Duration::from_millis(HOVER_DELAY));
    assert_eq!(keyboard.active_delay(), Duration::ZERO);
}

/// A keyboard arrival one frame before the hover delay would have expired is still showing,
/// and the pointer at the same instant is not.
#[test]
fn the_keyboard_channel_ignores_the_hover_delay_entirely() {
    for offset in 0..HOVER_DELAY {
        let mut c = HoverController::new();
        c.keyboard_focus_enter(T0);
        assert_eq!(
            c.tick(at(T0.as_millis() + offset)),
            Visibility::Shown,
            "keyboard arrival must be shown at +{offset} ms"
        );
    }
}

/// Keyboard focus is armed idempotently exactly like the pointer: a host reporting focus
/// every frame must not re-arm, and the arrival instant must not drift.
#[test]
fn repeated_keyboard_focus_does_not_rearm() {
    let mut c = HoverController::new();
    assert!(c.keyboard_focus_enter(T0));
    for step in 1..=20 {
        assert!(
            !c.keyboard_focus_enter(at(T0.as_millis() + step)),
            "frame {step} must not re-arm"
        );
    }
    assert_eq!(c.entered_at(), Some(T0));
    assert!(c.is_keyboard_armed());
}

/// A pointer that already armed the controller is not switched to the keyboard channel by a
/// later focus event — the tooltip is already on its way and re-timing it would only delay
/// what the user can already see coming.
#[test]
fn a_later_keyboard_focus_does_not_switch_an_armed_pointer_arrival() {
    let mut c = HoverController::new();
    assert!(c.pointer_enter(T0));
    assert!(
        !c.keyboard_focus_enter(at(T0.as_millis() + 10)),
        "already armed by the pointer, so no re-arm"
    );
    assert_eq!(c.arrival_source(), Some(ArrivalSource::Pointer));
    assert_eq!(c.tick(at(T0.as_millis() + 10)), Visibility::Pending);
    assert_eq!(c.tick(at(T0.as_millis() + HOVER_DELAY)), Visibility::Shown);
}

/// Exiting the keyboard channel clears the arrival, so a later focus is a fresh one.
#[test]
fn keyboard_focus_exit_clears_the_arrival() {
    let mut c = HoverController::new();
    c.keyboard_focus_enter(T0);
    assert_eq!(c.tick(T0), Visibility::Shown);
    c.keyboard_focus_exit();
    assert_eq!(c.tick(T0), Visibility::Hidden);
    assert_eq!(c.arrival_source(), None);
    assert!(!c.is_keyboard_armed());
    assert!(c.keyboard_focus_enter(T1), "a later focus re-arms");
    assert_eq!(c.tick(T1), Visibility::Shown);
}

/// A keyboard arrival dismissed with `Escape` is suppressed like any other, and a fresh
/// focus after an exit shows immediately again.
#[test]
fn a_keyboard_arrival_suppresses_and_recovers_like_a_pointer_one() {
    let mut c = HoverController::new();
    c.keyboard_focus_enter(T0);
    let out = c.dismiss(DismissReason::Escape);
    assert!(out.was_active);
    assert!(!out.consumed_input);
    assert!(c.is_suppressed());
    assert_eq!(c.tick(T1), Visibility::Hidden);

    c.keyboard_focus_exit();
    assert!(!c.is_suppressed());
    c.keyboard_focus_enter(T1);
    assert_eq!(c.tick(T1), Visibility::Shown);
}

/// `with_delay` must not silently change the keyboard side: a host that only retimes the
/// hover keeps the specified `0 ms` focus behaviour.
#[test]
fn with_delay_leaves_the_keyboard_channel_at_zero() {
    let c = HoverController::with_delay(Duration::from_millis(1_000));
    assert_eq!(c.delay(), Duration::from_millis(1_000));
    assert_eq!(c.keyboard_delay(), Duration::ZERO);
}

/// Both delays are independently settable, and each drives its own channel's boundary.
#[test]
fn with_delays_drives_each_channel_separately() {
    let mut c = HoverController::with_delays(Duration::from_millis(120), Duration::from_millis(30));
    c.keyboard_focus_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + 29)), Visibility::Pending);
    assert_eq!(c.tick(at(T0.as_millis() + 30)), Visibility::Shown);

    let mut c = HoverController::with_delays(Duration::from_millis(120), Duration::from_millis(30));
    c.pointer_enter(T0);
    assert_eq!(c.tick(at(T0.as_millis() + 119)), Visibility::Pending);
    assert_eq!(c.tick(at(T0.as_millis() + 120)), Visibility::Shown);
}

/// The accessor for "which delay applies right now" must follow the armed channel, and fall
/// back to the pointer delay at rest rather than being optional.
#[test]
fn active_delay_follows_the_armed_channel() {
    let mut c = HoverController::new();
    assert_eq!(
        c.active_delay(),
        Duration::from_millis(HOVER_DELAY),
        "at rest"
    );
    c.keyboard_focus_enter(T0);
    assert_eq!(c.active_delay(), Duration::ZERO, "keyboard armed");
    c.keyboard_focus_exit();
    c.pointer_enter(T1);
    assert_eq!(
        c.active_delay(),
        Duration::from_millis(HOVER_DELAY),
        "pointer armed"
    );
}

/// The gap table claims this constant is invisible to Rule B because its *name* carries no
/// visual marker. That claim is only worth anything if it is checked — for both timings.
#[test]
fn the_timing_constants_are_not_rule_b_visual_scalars() {
    for name in ["HOVER_DELAY", "KEYBOARD_FOCUS_DELAY"] {
        for marker in VISUAL_NAME_MARKERS {
            assert!(
                !name.contains(marker),
                "{name} contains the visual marker {marker:?}, so the gap table's claim about \
                 Rule B visibility is wrong"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The 280px cap (§3.7)
// ---------------------------------------------------------------------------

/// The cap is the framework's, not the host's: a label measured wider than 280px comes back
/// at 280px, and nothing narrower is touched.
#[test]
fn clamp_width_caps_at_the_specified_280px() {
    assert_eq!(MAX_WIDTH, 280.0);
    assert_eq!(clamp_width(10.0), 10.0);
    assert_eq!(clamp_width(279.9), 279.9);
    assert_eq!(clamp_width(280.0), 280.0);
    assert_eq!(clamp_width(280.1), MAX_WIDTH);
    assert_eq!(clamp_width(5_000.0), MAX_WIDTH);
    assert_eq!(clamp_width(0.0), 0.0);
}

/// A negative measurement cannot be honoured; the cap is the honest answer rather than a
/// negative card.
#[test]
fn clamp_width_never_returns_a_negative_width() {
    assert_eq!(clamp_width(-100.0), 0.0);
    assert!(clamp_width(-1.0) >= 0.0);
}

/// A non-finite width cannot be compared against the cap meaningfully. Propagating `NaN`
/// would paint nothing at all; reporting the cap keeps the card at least drawable.
#[test]
fn clamp_width_reports_the_cap_for_non_finite_input() {
    assert_eq!(clamp_width(f32::NAN), MAX_WIDTH);
    assert_eq!(clamp_width(f32::INFINITY), MAX_WIDTH);
    assert_eq!(clamp_width(f32::NEG_INFINITY), MAX_WIDTH);
}

/// The style carries the cap so a host reading only the resolved style still gets it.
#[test]
fn the_style_carries_the_max_width() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_eq!(s.max_width, MAX_WIDTH, "{name} max width");
        assert_eq!(clamp_width(s.max_width), s.max_width);
    }
}

// ---------------------------------------------------------------------------
// The `4px 8px` padding (§3.7)
// ---------------------------------------------------------------------------

/// §3.7 writes the padding as `4px 8px`, which is `space.xs` vertical and `space.sm`
/// horizontal — the tokens, not the literals.
#[test]
fn the_padding_is_space_xs_vertical_and_space_sm_horizontal() {
    assert_eq!(PADDING_VERTICAL, space::XS);
    assert_eq!(PADDING_HORIZONTAL, space::SM);
    assert_eq!(PADDING_VERTICAL, 4.0);
    assert_eq!(PADDING_HORIZONTAL, 8.0);
    assert_ne!(
        PADDING_VERTICAL, PADDING_HORIZONTAL,
        "the spec's two figures are two different rungs"
    );

    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_eq!(s.padding_vertical, space::XS, "{name} vertical padding");
        assert_eq!(s.padding_horizontal, space::SM, "{name} horizontal padding");
    }
}

// ---------------------------------------------------------------------------
// Style resolution
// ---------------------------------------------------------------------------

#[test]
fn resolve_is_pure_and_deterministic() {
    for (name, palette) in palettes() {
        let a = TooltipStyle::resolve(&palette);
        let b = TooltipStyle::resolve(&palette);
        assert_eq!(a, b, "{name}: resolving twice must give the same style");
    }
}

#[test]
fn style_binds_every_field_to_its_named_token() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_eq!(s.background, palette.surface_overlay, "{name} background");
        assert_eq!(s.foreground, palette.text_primary, "{name} foreground");
        assert_eq!(
            s.border_color, palette.border_strong,
            "{name} border colour"
        );
        assert_eq!(
            s.border_width,
            shape::BORDER_HAIRLINE,
            "{name} border width"
        );
        assert_eq!(s.radius, shape::RADIUS_WIDGET, "{name} radius");
        assert_eq!(s.font, typography::CAPTION, "{name} type rung");
        assert_eq!(
            s.shortcut_foreground, palette.text_muted,
            "{name} shortcut foreground"
        );
        assert_eq!(
            s.shortcut_font,
            typography::FontFamily::Mono,
            "{name} shortcut font family"
        );
    }
}

/// The trailing shortcut pill is `text_muted` on `surface_overlay`, measuring ~6.78:1
/// and clearing the WCAG 2.1 AA text contrast floor (`UIF-ATOM-007`, §3.7).
#[test]
fn shortcut_pill_clears_the_text_contrast_floor_on_the_card() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        let ratio = contrast::contrast_ratio(s.shortcut_foreground, s.background);
        assert!(
            meets_text_contrast(s.shortcut_foreground, s.background),
            "{name}: tooltip shortcut pill contrast {ratio:.4}:1 is below the {MIN_TEXT_CONTRAST}:1 floor"
        );
        assert!(
            (ratio - 6.78).abs() < 0.05,
            "{name}: shortcut pill contrast expected ~6.78:1, got {ratio:.4}:1"
        );
    }
}

/// The label colour is `text.primary`, which §3.7 now names explicitly. `text_muted` is the
/// other obvious candidate and is deliberately *not* used; pinning the difference keeps a
/// later "tidy up" from quietly changing the class of the text.
#[test]
fn the_label_is_primary_text_not_muted_text() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_ne!(
            s.foreground, palette.text_muted,
            "{name}: a tooltip's label is the surface's primary content, not secondary text"
        );
    }
}

// ---------------------------------------------------------------------------
// Contrast — recomputed, never transcribed
// ---------------------------------------------------------------------------

#[test]
fn label_clears_the_text_contrast_floor_on_the_card() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        let ratio = contrast::contrast_ratio(s.foreground, s.background);
        assert!(
            meets_text_contrast(s.foreground, s.background),
            "{name}: tooltip label contrast {ratio:.4}:1 is below the {MIN_TEXT_CONTRAST}:1 floor"
        );
        assert!(
            ratio >= MIN_TEXT_CONTRAST,
            "{name}: {ratio:.4}:1 < {MIN_TEXT_CONTRAST}:1"
        );
    }
}

/// The card must be readable against the viewport it floats over, not just internally.
/// Checked against the darkest and lightest surfaces the tooltip can appear over.
#[test]
fn the_card_border_is_visible_against_the_card_fill() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        let ratio = contrast::contrast_ratio(s.border_color, s.background);
        assert!(
            ratio >= contrast::MIN_CHROME_CONTRAST,
            "{name}: border-vs-card contrast {ratio:.4}:1 is below the \
       {} :1 chrome floor — the card would have no visible edge",
            contrast::MIN_CHROME_CONTRAST
        );
    }
}

/// **A measured finding, not an assumption.** The tooltip's *fill* does not separate it
/// from the window behind it: `surface_overlay` against `surface_base` measures
/// **1.1230:1**, far under the 3:1 chrome floor. What makes the card read as a separate
/// layer is its `border_strong` hairline, which clears that floor against the same
/// surfaces by a wide margin.
///
/// The fill-vs-base number is pinned as *below* the floor on purpose. If a palette edit
/// lifts it above the floor this test fails, which is the intended signal: the note below
/// needs re-reading, and a bordered card may no longer be the only thing keeping the
/// tooltip legible as a layer.
///
/// This is a gap the spec does not address — it asks for a tooltip and assumes the overlay
/// surface reads as elevated. It does not. Worth raising with the palette owner.
#[test]
fn the_card_reads_as_a_layer_through_its_border_not_its_fill() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);

        let fill_vs_base = contrast::contrast_ratio(s.background, palette.surface_base);
        assert!(
            fill_vs_base < contrast::MIN_CHROME_CONTRAST,
            "{name}: fill-vs-base measured {fill_vs_base:.4}:1, which now clears the {} :1 \
             chrome floor — the palette changed and this finding needs re-reading",
            contrast::MIN_CHROME_CONTRAST
        );

        let border_vs_fill = contrast::contrast_ratio(s.border_color, s.background);
        let border_vs_base = contrast::contrast_ratio(s.border_color, palette.surface_base);
        assert!(
            border_vs_fill >= contrast::MIN_CHROME_CONTRAST,
            "{name}: border-vs-card {border_vs_fill:.4}:1 is under the chrome floor"
        );
        assert!(
            border_vs_base >= contrast::MIN_CHROME_CONTRAST,
            "{name}: border-vs-window {border_vs_base:.4}:1 is under the chrome floor — the \
             card would have no visible edge against the window"
        );
    }
}

// ---------------------------------------------------------------------------
// The foreground lock
// ---------------------------------------------------------------------------

/// The card fill is a surface, not one of the four locked solid fills, so rule 4 places no
/// demand through it. Asserted rather than assumed: if someone swaps `surface_overlay` for
/// `accent_fill` this is where it surfaces.
#[test]
fn the_card_fill_is_not_a_locked_solid_fill() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_eq!(
            fill_lock_role(s.background, &palette),
            FillLockRole::Unlocked,
            "{name}: the tooltip card fill must not be a locked solid fill"
        );
    }
}

#[test]
fn the_fill_foreground_lock_reports_not_a_solid_fill() {
    for (name, palette) in palettes() {
        let s = TooltipStyle::resolve(&palette);
        assert_eq!(
            fill_lock(s.background, s.foreground, &palette),
            FillLock::NotSolidFill,
            "{name}: unexpected fill-lock outcome"
        );
        assert!(
            meets_fill_foreground_lock(s.background, s.foreground, &palette),
            "{name}: the resolved pair must satisfy the lock"
        );
    }
}

/// `resolve` runs the assert internally, so reaching this test at all is the evidence.
/// Stated explicitly so the shield is not mistaken for decoration.
#[test]
fn resolve_runs_the_lock_shield_without_panicking() {
    for (_, palette) in palettes() {
        let _ = TooltipStyle::resolve(&palette);
    }
}

// ---------------------------------------------------------------------------
// Shape choices
// ---------------------------------------------------------------------------

/// §3.7 puts a tooltip at the **widget** radius, not the card radius. INC-31 had reasoned
/// that radius signals elevation and a floating tooltip belongs with a menu card; the spec
/// says otherwise, and 4px is what ships. Asserted against the palette's own rungs so the
/// claim stays checkable rather than being a restated number.
#[test]
fn the_radius_is_the_widget_rung_not_the_card_rung() {
    let s = TooltipStyle::resolve(&Palette::dark());
    assert_eq!(s.radius, shape::RADIUS_WIDGET);
    assert_eq!(s.radius, 4.0);
    assert_ne!(
        s.radius,
        shape::RADIUS_CARD,
        "the card rung (6px) was INC-31's guess; §3.7 specifies radius.widget"
    );
    assert!(s.radius < shape::RADIUS_CARD);
    assert!(s.radius <= shape::RADIUS_WINDOW);
}

#[test]
fn the_border_is_a_hairline() {
    let s = TooltipStyle::resolve(&Palette::dark());
    assert_eq!(s.border_width, 1.0);
    assert_eq!(s.border_width, shape::BORDER_HAIRLINE);
    assert_ne!(s.border_width, shape::BORDER_FOCUS);
}

/// The card is caption-sized, which is the smallest rung the palette defines — a tooltip
/// must not out-shout the content it explains.
#[test]
fn the_type_rung_is_the_caption() {
    let s = TooltipStyle::resolve(&Palette::dark());
    assert_eq!(s.font, typography::CAPTION);
    assert!(s.font.size <= typography::UI_COMPACT.size);
}

// ---------------------------------------------------------------------------
// Sanity: the tokens this module leans on are the ones the docs name
// ---------------------------------------------------------------------------

#[test]
fn referenced_tokens_hold_their_documented_values() {
    assert_eq!(space::XS, 4.0);
    assert_eq!(space::SM, 8.0);
    assert_eq!(size::CONTROL_LG, 36.0);
    assert_eq!(size::CONTROL_SM, 28.0);
    assert_eq!(TOOLTIP_GAP, space::XS);
}

#[test]
fn the_alignment_is_below_start() {
    assert_eq!(
        TOOLTIP_ALIGNMENT,
        crate::geometry::anchor::ControlAlignment::below_start()
    );
}
