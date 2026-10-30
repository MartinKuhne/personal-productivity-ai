//! Tests for the teaching popover's style, width band and perimeter —
//! `UIF-ATOM-007`, §3.7.
//!
//! Four groups:
//!
//! 1. **The style.** Every channel §3.7 prints, asserted against the
//!    palette rather than against a transcription of the section.
//! 2. **The recomputed contrast table.** The ratio §3.7 prints, plus
//!    the supporting pairs the card leans on.
//! 3. **The width band.** §3.7's `280..360` as an enforced floor and
//!    ceiling, including the non-finite input.
//! 4. **The perimeter class.** That the card's `accent.border` edge is
//!    judged and not exempt.

use super::*;
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::typography::{self, Weight};
use crate::token::{shape, space};
use iced::Color;

/// Tolerance for a ratio asserted to four decimal places.
const RATIO_TOLERANCE: f32 = 1e-3;

fn palette() -> Palette {
    Palette::dark()
}

fn style() -> PopoverStyle {
    PopoverStyle::resolve(&palette())
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < RATIO_TOLERANCE
}

fn ratio(a: Color, b: Color) -> f32 {
    contrast::contrast_ratio(a, b)
}

// ---------------------------------------------------------------------------
// 1. The style
// ---------------------------------------------------------------------------

#[test]
fn the_card_is_the_overlay_surface_with_an_accent_edge() {
    let p = palette();
    let s = style();
    assert_eq!(s.background, p.surface_overlay);
    assert_eq!(s.border_color, p.accent_border);
    assert_eq!(s.border_width, shape::BORDER_HAIRLINE);
    assert_eq!(s.radius, shape::RADIUS_CARD);
    assert_eq!(s.padding, space::MD);
}

#[test]
fn the_title_is_13_semi_bold_strong_and_the_body_13_regular_primary() {
    let p = palette();
    let s = style();
    assert_eq!(s.title_color, p.text_strong);
    assert_eq!(s.title_size, 13.0);
    assert_eq!(s.title_weight, Weight::SemiBold);
    assert_eq!(s.body_color, p.text_primary);
    assert_eq!(s.body_size, 13.0);
    assert_eq!(s.body_weight, Weight::Regular);
}

#[test]
fn both_text_channels_stay_on_one_type_rung() {
    let s = style();
    assert_eq!(TYPE_RUNG.size, 13.0);
    assert_eq!(s.title_size, TYPE_RUNG.size);
    assert_eq!(s.body_size, TYPE_RUNG.size);
    assert!(TYPE_RUNG.allows(TITLE_WEIGHT));
    assert!(TYPE_RUNG.allows(BODY_WEIGHT));
    assert_eq!(TYPE_RUNG, typography::UI_COMPACT);
}

#[test]
fn the_radius_is_one_rung_up_from_the_tooltip() {
    // §3.7 gives the tooltip `radius.widget` and the popover
    // `radius.card`, and the difference is the card's own size.
    assert_eq!(RADIUS, shape::RADIUS_CARD);
    assert_eq!(
        RADIUS.partial_cmp(&shape::RADIUS_WIDGET),
        Some(std::cmp::Ordering::Greater),
        "the card radius must exceed the tooltip's widget radius"
    );
    assert_eq!(
        RADIUS.partial_cmp(&shape::RADIUS_WINDOW),
        Some(std::cmp::Ordering::Less),
        "the card radius must stay below the window radius"
    );
}

#[test]
fn the_style_is_deterministic() {
    assert_eq!(style(), style());
    assert_eq!(
        PopoverStyle::resolve(&Palette::dark()),
        PopoverStyle::resolve(&Palette::dark()),
    );
}

#[test]
fn the_width_band_is_carried_on_the_style() {
    let s = style();
    assert_eq!(s.width_min, WIDTH_MIN);
    assert_eq!(s.width_max, WIDTH_MAX);
    assert!(s.width_min < s.width_max);
}

// ---------------------------------------------------------------------------
// 2. The recomputed contrast table
// ---------------------------------------------------------------------------

#[test]
fn the_ratio_3_7_prints_is_reproduced_from_raw_srgb() {
    let p = palette();
    let got = ratio(p.accent_border, p.surface_overlay);
    assert!(
        close(got, 5.9394),
        "accent.border / surface.overlay recomputed {got:.4}, §3.7 prints 5.94",
    );
    assert!(contrast::meets_chrome_contrast(
        p.accent_border,
        p.surface_overlay
    ));
}

#[test]
fn the_text_pairs_the_card_leans_on_are_reproduced() {
    let p = palette();
    let table: [(&str, Color, Color, f32); 2] = [
        (
            "text.strong / surface.overlay",
            p.text_strong,
            p.surface_overlay,
            17.7168,
        ),
        (
            "text.primary / surface.overlay",
            p.text_primary,
            p.surface_overlay,
            11.7180,
        ),
    ];
    for (name, a, b, expected) in table {
        let got = ratio(a, b);
        assert!(close(got, expected), "{name}: recomputed {got:.4}");
        assert!(
            contrast::meets_text_contrast(a, b),
            "{name} must clear the 4.5:1 text floor",
        );
    }
}

#[test]
fn the_fill_carries_no_layer_separation_on_its_own() {
    // The finding §3.7 does not mention: `surface.overlay` against
    // `surface.base` is barely distinguishable, so the accent hairline
    // — not the fill — is what makes the card read as floating.
    let p = palette();
    let got = ratio(p.surface_overlay, p.surface_base);
    assert!(close(got, 1.1230), "overlay/base recomputed {got:.4}");
    assert!(
        got < 3.0,
        "the fill alone is far under the chrome floor, as measured",
    );
}

#[test]
fn the_card_is_not_a_locked_solid_fill() {
    let p = palette();
    let lock = contrast::fill_lock(p.surface_overlay, p.text_strong, &p);
    assert!(
        !lock.governed(),
        "surface.overlay is a surface, not one of the four locked fills: {lock:?}",
    );
}

// ---------------------------------------------------------------------------
// 3. The width band
// ---------------------------------------------------------------------------

#[test]
fn a_measurement_inside_the_band_passes_through() {
    assert_eq!(clamp_width(300.0), 300.0);
    assert_eq!(clamp_width(280.0), WIDTH_MIN);
    assert_eq!(clamp_width(360.0), WIDTH_MAX);
}

#[test]
fn a_narrower_measurement_is_raised_to_the_floor() {
    // The band is a floor as well as a ceiling: a 180px callout with
    // three lines of wrapped body is unreadable.
    assert_eq!(clamp_width(180.0), WIDTH_MIN);
    assert_eq!(clamp_width(0.0), WIDTH_MIN);
    assert_eq!(clamp_width(-50.0), WIDTH_MIN);
}

#[test]
fn a_wider_measurement_is_cut_to_the_ceiling() {
    assert_eq!(clamp_width(1000.0), WIDTH_MAX);
}

#[test]
fn a_non_finite_measurement_lands_on_the_floor_not_on_nan() {
    assert_eq!(clamp_width(f32::NAN), WIDTH_MIN);
    assert_eq!(clamp_width(f32::INFINITY), WIDTH_MIN);
    assert_eq!(clamp_width(f32::NEG_INFINITY), WIDTH_MIN);
}

#[test]
fn the_band_is_80px_wide() {
    assert_eq!(WIDTH_MAX - WIDTH_MIN, 80.0);
}

// ---------------------------------------------------------------------------
// 4. The perimeter class
// ---------------------------------------------------------------------------

#[test]
fn the_card_perimeter_is_judged_not_exempt() {
    let p = palette();
    let class = card_chrome_class(&style(), &p);
    assert_eq!(class, ChromeClass::Judged);
    assert!(!class.is_exempt());
    assert_eq!(class.minimum_contrast(), Some(3.0));
}

#[test]
fn a_coincident_or_subtle_edge_would_classify_away_from_judged() {
    // The classifier's other arms are reachable by a hypothetical style
    // and must classify honestly rather than default to `Judged`.
    let p = palette();
    let coincident = PopoverStyle {
        border_color: p.surface_overlay,
        ..style()
    };
    assert_eq!(
        card_chrome_class(&coincident, &p),
        ChromeClass::CoincidentWithFill,
    );

    let subtle = PopoverStyle {
        border_color: p.border_subtle,
        ..style()
    };
    assert_eq!(card_chrome_class(&subtle, &p), ChromeClass::RestingDivider);
}

#[test]
fn the_judged_perimeter_clears_its_floor_against_the_card() {
    let p = palette();
    let s = style();
    assert!(contrast::meets_chrome_contrast_as(
        card_chrome_class(&s, &p),
        s.border_color,
        s.background,
    ));
}
