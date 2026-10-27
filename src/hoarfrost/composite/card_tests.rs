//! Tests for [`super::card`] — the grouping surface's tokens, its two
//! border elevations and the inset its content is owed (`UIF-COMP-005`).

use crate::composite::card::{
    BACKGROUND_TOKEN, BORDER_WIDTH, CardElevation, PADDING, RADIUS, card_layer, content_bounds,
    resolve_style,
};
use crate::token::color::Palette;
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, contrast_ratio, meets_chrome_contrast,
};
use crate::token::{shape, space};
use iced::{Color, Point, Rectangle};

fn bounds() -> Rectangle {
    Rectangle::new(Point::new(4.0, 4.0), iced::Size::new(320.0, 200.0))
}

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to; `f32` does not reproduce a printed decimal exactly.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

// ---------------------------------------------------------------------------
// 1 — tokens
// ---------------------------------------------------------------------------

#[test]
fn every_card_measurement_is_a_token_read() {
    assert_eq!(RADIUS, shape::RADIUS_CARD);
    assert_eq!(PADDING, space::LG);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(RADIUS, 6.0);
    assert_eq!(PADDING, 16.0);
}

#[test]
fn the_fill_is_named_and_is_the_same_at_both_elevations() {
    let palette = Palette::dark();
    let rest = resolve_style(CardElevation::Rest, &palette);
    let raised = resolve_style(CardElevation::Elevated, &palette);

    assert_eq!(BACKGROUND_TOKEN, "surface.raised");
    assert_eq!(rest.background, palette.surface_raised);
    assert_eq!(
        raised.background, rest.background,
        "§3B.5 lifts a card with its border, not with a brighter surface"
    );
}

// ---------------------------------------------------------------------------
// 2 — the two elevations
// ---------------------------------------------------------------------------

#[test]
fn the_resting_card_wears_border_subtle() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Rest, &palette);

    assert_eq!(style.border_color, palette.border_subtle);
    assert!(!CardElevation::Rest.is_active_indicator());
}

#[test]
fn the_elevated_card_wears_border_strong() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Elevated, &palette);

    assert_eq!(style.border_color, palette.border_strong);
    assert!(CardElevation::Elevated.is_active_indicator());
    assert_eq!(style.border_width, BORDER_WIDTH);
    assert_eq!(style.radius, RADIUS);
    assert_eq!(style.padding, PADDING);
}

#[test]
fn the_elevated_border_clears_the_chrome_floor() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Elevated, &palette);
    let ratio = contrast_ratio(style.border_color, style.background);

    assert!(
        meets_chrome_contrast(style.border_color, style.background),
        "an active border measured {ratio}, under the {MIN_CHROME_CONTRAST} floor"
    );
    assert!(approx(ratio, 4.2379));
}

#[test]
fn the_resting_border_is_quiet_by_design() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Rest, &palette);

    assert!(
        approx(contrast_ratio(style.border_color, style.background), 1.2691),
        "a resting card edge separates; it is not an indicator"
    );
}

// ---------------------------------------------------------------------------
// 3 — the content inset
// ---------------------------------------------------------------------------

#[test]
fn content_is_inset_by_the_padding_and_the_stroke() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Rest, &palette);
    let outer = bounds();
    let inner = content_bounds(outer, &style);
    let inset = PADDING + BORDER_WIDTH;

    assert_eq!(inner.x, outer.x + inset);
    assert_eq!(inner.y, outer.y + inset);
    assert_eq!(inner.width, outer.width - inset - inset);
    assert_eq!(inner.height, outer.height - inset - inset);
}

#[test]
fn the_inset_grows_with_the_stroke_so_a_child_ring_never_sits_on_the_border() {
    let palette = Palette::dark();
    let rest = content_bounds(bounds(), &resolve_style(CardElevation::Rest, &palette));
    let raised = content_bounds(bounds(), &resolve_style(CardElevation::Elevated, &palette));

    assert_eq!(rest, raised, "both elevations stroke at 1px");
    assert!(rest.x > bounds().x + PADDING);
}

#[test]
fn a_card_too_small_for_its_own_padding_clamps_instead_of_going_negative() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Rest, &palette);
    let tiny = Rectangle::new(Point::ORIGIN, iced::Size::new(10.0, 10.0));
    let inner = content_bounds(tiny, &style);

    assert_eq!(inner.width, 0.0);
    assert_eq!(inner.height, 0.0);
}

#[test]
fn a_zero_size_card_still_resolves() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Elevated, &palette);
    let inner = content_bounds(Rectangle::new(Point::ORIGIN, iced::Size::ZERO), &style);

    assert_eq!(inner.width, 0.0);
    assert_eq!(inner.height, 0.0);
}

// ---------------------------------------------------------------------------
// 4 — the painted layer
// ---------------------------------------------------------------------------

#[test]
fn the_card_layer_carries_every_resolved_channel() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Elevated, &palette);
    let layer = card_layer(bounds(), &style);

    assert_eq!(layer.bounds, bounds());
    assert_eq!(layer.fill, palette.surface_raised);
    assert_eq!(layer.border.color, palette.border_strong);
    assert_eq!(layer.border.width, BORDER_WIDTH);
    assert_eq!(layer.border.radius.top_left, RADIUS);
}

#[test]
fn the_two_elevations_paint_different_strokes_and_the_same_quad() {
    let palette = Palette::dark();
    let rest = card_layer(bounds(), &resolve_style(CardElevation::Rest, &palette));
    let raised = card_layer(bounds(), &resolve_style(CardElevation::Elevated, &palette));

    assert_eq!(rest.bounds, raised.bounds);
    assert_eq!(rest.fill, raised.fill);
    assert_ne!(rest.border.color, raised.border.color);
}

// ---------------------------------------------------------------------------
// 5 — what sits on the card
// ---------------------------------------------------------------------------

#[test]
fn the_text_roles_a_card_holds_all_clear_the_floor() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Rest, &palette);

    for (name, colour) in [
        ("text.primary", palette.text_primary),
        ("text.strong", palette.text_strong),
        ("text.muted", palette.text_muted),
        ("status.danger.text", palette.status_danger_text),
    ] {
        assert!(
            contrast_ratio(colour, style.background) >= MIN_TEXT_CONTRAST,
            "{name} on a card is {}",
            contrast_ratio(colour, style.background)
        );
    }
}

#[test]
fn a_card_style_value_survives_a_copy_unchanged() {
    let palette = Palette::dark();
    let style = resolve_style(CardElevation::Elevated, &palette);
    let copy = style;

    assert_eq!(copy, style);
    assert_eq!(copy.border_color, Color::from_rgb8(120, 120, 120));
}
