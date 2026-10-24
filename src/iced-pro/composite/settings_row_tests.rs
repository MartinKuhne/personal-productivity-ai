//! Tests for [`super::settings_row`] — the horizontal split's floor, its
//! column reservation and the contrast of both text voices
//! (`UIF-COMP-001`).

use crate::composite::settings_row::{
    BACKGROUND_TOKEN, BORDER_INSET_RESERVE, BORDER_WIDTH, COLUMN_GAP, DEFAULT_CONTROL_HEIGHT,
    DESCRIPTION_RUNG, MIN_HEIGHT, PADDING_HORIZONTAL, PADDING_VERTICAL, RADIUS, SettingsBands,
    TITLE_RUNG, bands, card_layer, description_present, horizontal, resolve_style, row_height,
    text_column_height,
};
use crate::token::color::Palette;
use crate::token::contrast::{
    DISABLED_TEXT_MAX_CONTRAST, DISABLED_TEXT_MIN_CONTRAST, MIN_TEXT_CONTRAST, contrast_ratio,
};
use crate::token::{shape, size, space, typography};
use iced::{Point, Rectangle, widget::text};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Toggled(bool),
}

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to; `f32` does not reproduce a printed decimal exactly.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

// ---------------------------------------------------------------------------
// 1 — token bindings
// ---------------------------------------------------------------------------

#[test]
fn every_measurement_is_a_token_read() {
    assert_eq!(PADDING_VERTICAL, space::MD);
    assert_eq!(PADDING_HORIZONTAL, space::LG);
    assert_eq!(COLUMN_GAP, space::LG);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(DEFAULT_CONTROL_HEIGHT, size::CONTROL_MD);
}

#[test]
fn the_two_text_rungs_are_the_ones_3b1_names() {
    assert_eq!(TITLE_RUNG.size, typography::BODY.size);
    assert_eq!(TITLE_RUNG.line_height, 20.0);
    assert_eq!(DESCRIPTION_RUNG.size, typography::CAPTION.size);
    assert_eq!(DESCRIPTION_RUNG.line_height, 16.0);
    assert_eq!(BACKGROUND_TOKEN, "surface.raised");
}

#[test]
fn the_printed_floor_is_48() {
    assert_eq!(MIN_HEIGHT, 48.0);
}

// ---------------------------------------------------------------------------
// 2 — the floor is a floor
// ---------------------------------------------------------------------------

#[test]
fn the_floor_is_the_sum_of_the_parts_3b1_prints() {
    // §3B.1 prints "Min height 48px" and, in the same block, the parts
    // that make it: 12px top padding + 20px `type.body` title + 12px
    // bottom padding + 4px border/inset reserve. MIN_HEIGHT is that sum
    // rather than the printed 48 on its own, so the total is checkable
    // against the row it describes instead of being asserted beside it.
    assert_eq!(TITLE_RUNG.line_height, 20.0, "the title's line box");
    assert_eq!(PADDING_VERTICAL, space::MD, "12px");
    assert_eq!(BORDER_INSET_RESERVE, space::XS, "4px");
    assert_eq!(
        MIN_HEIGHT,
        PADDING_VERTICAL + TITLE_RUNG.line_height + PADDING_VERTICAL + BORDER_INSET_RESERVE
    );
    assert_eq!(MIN_HEIGHT, 48.0);
    // The reserve is slack, not content: inside the padding there is
    // exactly the title's line box plus the reserve left over.
    let interior = MIN_HEIGHT - PADDING_VERTICAL - PADDING_VERTICAL;
    assert_eq!(interior, TITLE_RUNG.line_height + BORDER_INSET_RESERVE);
    // And 48 is a floor, not a fixed height. A 32px control beside a
    // title-only row pushes the row past the printed figure, which is
    // the correct reading of the section.
    assert_eq!(row_height(false, size::CONTROL_MD), 56.0);
    assert!(row_height(false, size::CONTROL_MD) > MIN_HEIGHT);
}

#[test]
fn a_title_only_row_without_a_control_sits_on_the_floor() {
    assert_eq!(text_column_height(false), 20.0);
    assert_eq!(row_height(false, 0.0), MIN_HEIGHT);
}

#[test]
fn a_control_taller_than_the_text_lifts_the_row_above_the_floor() {
    assert_eq!(row_height(false, size::CONTROL_MD), 56.0);
    assert_eq!(row_height(false, size::CONTROL_LG), 60.0);
}

#[test]
fn a_description_lifts_the_row_past_every_control_rung() {
    assert_eq!(text_column_height(true), 36.0);
    assert_eq!(row_height(true, size::CONTROL_MD), 60.0);
    assert_eq!(row_height(true, size::CONTROL_LG), 60.0);
    assert_eq!(row_height(true, 0.0), 60.0);
}

#[test]
fn the_row_never_clips_the_description_it_was_told_to_show() {
    let with_description = row_height(true, size::CONTROL_MD);
    let interior = with_description - PADDING_VERTICAL - PADDING_VERTICAL;

    assert!(
        interior >= text_column_height(true),
        "the interior must hold both lines"
    );
}

#[test]
fn a_zero_height_control_still_respects_the_floor() {
    assert_eq!(row_height(false, 0.0), 48.0);
    assert!(row_height(true, 0.0) > MIN_HEIGHT);
}

// ---------------------------------------------------------------------------
// 3 — the column reservation
// ---------------------------------------------------------------------------

fn row(width: f32) -> Rectangle {
    let height = row_height(true, size::CONTROL_MD);
    Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(width, height))
}

#[test]
fn the_control_column_is_reserved_from_the_right_edge() {
    let bounds = row(400.0);
    let split = bands(bounds, 96.0);

    assert_eq!(
        split.control.x + split.control.width,
        bounds.x + bounds.width - PADDING_HORIZONTAL
    );
    assert_eq!(split.control.width, 96.0);
}

#[test]
fn the_text_column_takes_the_remainder_minus_the_gap() {
    let bounds = row(400.0);
    let split = bands(bounds, 96.0);
    let interior = bounds.width - PADDING_HORIZONTAL - PADDING_HORIZONTAL;

    assert_eq!(split.text.x, bounds.x + PADDING_HORIZONTAL);
    assert_eq!(split.text.width, interior - 96.0 - COLUMN_GAP);
    assert_eq!(
        split.text.x + split.text.width + COLUMN_GAP,
        split.control.x
    );
}

#[test]
fn both_columns_share_the_interior_height() {
    let bounds = row(400.0);
    let split = bands(bounds, 96.0);
    let interior_height = bounds.height - PADDING_VERTICAL - PADDING_VERTICAL;

    assert_eq!(split.text.height, interior_height);
    assert_eq!(split.control.height, interior_height);
}

#[test]
fn a_control_wider_than_the_row_leaves_the_text_column_at_zero_not_negative() {
    let bounds = row(60.0);
    let split = bands(bounds, 500.0);

    assert_eq!(split.text.width, 0.0);
    assert!(split.control.width <= bounds.width);
}

#[test]
fn a_narrow_row_never_produces_a_negative_extent() {
    for width in [0.0, 8.0, 16.0, 32.0, 64.0, 128.0] {
        let split = bands(row(width), 48.0);
        assert!(
            split.text.width >= 0.0,
            "width {width} broke the text column"
        );
        assert!(
            split.control.width >= 0.0,
            "width {width} broke the control column"
        );
    }
}

#[test]
fn the_split_is_a_pure_function_of_its_inputs() {
    let a: SettingsBands = bands(row(400.0), 96.0);
    let b: SettingsBands = bands(row(400.0), 96.0);
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// 4 — description presence
// ---------------------------------------------------------------------------

#[test]
fn a_blank_description_is_not_content() {
    assert!(!description_present(None));
    assert!(!description_present(Some("")));
    assert!(!description_present(Some("   ")));
    assert!(!description_present(Some("\t\n")));
    assert!(description_present(Some("Sync every hour")));
}

// ---------------------------------------------------------------------------
// 5 — paint and contrast
// ---------------------------------------------------------------------------

#[test]
fn the_live_row_writes_strong_over_muted() {
    let palette = Palette::dark();
    let style = resolve_style(false, &palette);

    assert_eq!(style.background, palette.surface_raised);
    assert_eq!(style.border_color, palette.border_subtle);
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.radius, shape::RADIUS_WIDGET);
    assert_eq!(style.title_colour, palette.text_strong);
    assert_eq!(style.description_colour, palette.text_muted);
}

#[test]
fn a_disabled_row_dims_both_voices_together() {
    let palette = Palette::dark();
    let style = resolve_style(true, &palette);

    assert_eq!(style.title_colour, palette.text_disabled);
    assert_eq!(style.description_colour, palette.text_disabled);
    assert_eq!(
        style.background, palette.surface_raised,
        "the card itself does not change when the setting is unavailable"
    );
}

#[test]
fn both_text_voices_clear_the_text_floor_on_the_card() {
    let palette = Palette::dark();
    let live = resolve_style(false, &palette);

    assert!(contrast_ratio(live.title_colour, live.background) >= MIN_TEXT_CONTRAST);
    assert!(contrast_ratio(live.description_colour, live.background) >= MIN_TEXT_CONTRAST);
}

#[test]
fn the_disabled_voices_stay_inside_the_disabled_band() {
    let palette = Palette::dark();
    let dead = resolve_style(true, &palette);
    let ratio = contrast_ratio(dead.title_colour, dead.background);

    assert!(
        (DISABLED_TEXT_MIN_CONTRAST..=DISABLED_TEXT_MAX_CONTRAST).contains(&ratio),
        "disabled row contrast {ratio} is outside the band"
    );
}

#[test]
fn the_measured_row_contrasts_are_pinned_against_a_future_retune() {
    let palette = Palette::dark();

    assert!(approx(
        contrast_ratio(palette.text_strong, palette.surface_raised),
        18.7108
    ));
    assert!(approx(
        contrast_ratio(palette.text_muted, palette.surface_raised),
        7.1553
    ));
    assert!(approx(
        contrast_ratio(palette.text_disabled, palette.surface_raised),
        5.5643
    ));
}

#[test]
fn the_resting_border_is_separation_not_an_indicator() {
    let palette = Palette::dark();
    let style = resolve_style(false, &palette);
    let ratio = contrast_ratio(style.border_color, style.background);

    assert!(
        ratio < 3.0,
        "border.subtle is meant to be quiet; it measured {ratio}"
    );
    assert!(approx(ratio, 1.2691));
}

#[test]
fn the_card_layer_carries_the_resolved_fill_and_stroke() {
    let palette = Palette::dark();
    let style = resolve_style(false, &palette);
    let layer = card_layer(row(400.0), &style);

    assert_eq!(layer.fill, style.background);
    assert_eq!(layer.border.color, style.border_color);
    assert_eq!(layer.border.width, BORDER_WIDTH);
    assert_eq!(layer.bounds.height, MIN_HEIGHT.max(60.0));
}

// ---------------------------------------------------------------------------
// 6 — the widget
// ---------------------------------------------------------------------------

#[test]
fn the_row_builds_with_a_control_and_both_text_lines() {
    let palette = Palette::dark();
    let style = resolve_style(false, &palette);
    let control: iced::Element<'_, Msg> = crate::widget::button_widget::Button::new(
        crate::widget::button::ButtonVariant::Secondary,
        "switch",
        Msg::Toggled(true),
        "settings-row-switch",
    )
    .into();

    let _element: iced::Element<'_, Msg> =
        horizontal("Sync", Some("Every fifteen minutes"), control, &style);
}

#[test]
fn the_row_builds_with_a_blank_description_and_does_not_promise_a_line() {
    let palette = Palette::dark();
    let style = resolve_style(false, &palette);
    let control: iced::Element<'_, Msg> = text("toggle").into();

    assert!(!description_present(Some("  ")));
    let _element: iced::Element<'_, Msg> = horizontal("Sync", Some("  "), control, &style);
}
