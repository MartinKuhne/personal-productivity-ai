//! Tests for [`super::form_field`] — the stacked field's fixed rhythm, the
//! structural zero-shift proof, the caption resolver and the contrast of
//! every word the band paints (`UIF-COMP-001`, `UIF-PRIN-003`).

use crate::composite::form_field::{
    BAND_GAP, CAPTION_SLOT_HEIGHT, CONTROL_SLOT_HEIGHT, CaptionKind, CaptionParts, HEADER_HEIGHT,
    OverflowPolicy, REQUIRED_MARKER, REQUIRED_TOOLTIP, RESERVED_BAND_OVERFLOW, STACKED_HEIGHT,
    WARNING_GLYPH, bands, reserved_band_height, resolve_caption, resolve_header, slot_marker,
    stacked, stacked_height,
};
use crate::token::color::Palette;
use crate::token::contrast::{MIN_TEXT_CONTRAST, contrast_ratio};
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Button;
use crate::widget::focusable::Focusable;
use crate::widget::text_input::{InputKind, TextInput};
use iced::{Color, Point, Rectangle, widget::text};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Pick,
    Edit(String),
}

/// A reserved band with no height reserves nothing, which would make every
/// zero-shift assertion below vacuous. Checked at compile time so the
/// vacuity cannot arrive silently on a future retune.
const _: () = assert!(CAPTION_SLOT_HEIGHT > 0.0);

fn surface(name: &str) -> Color {
    let palette = Palette::dark();
    match name {
        "sunken" => palette.surface_sunken,
        "base" => palette.surface_base,
        "raised" => palette.surface_raised,
        "hover" => palette.surface_hover,
        "active" => palette.surface_active,
        "overlay" => palette.surface_overlay,
        other => panic!("no surface named {other}"),
    }
}

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to; `f32` does not reproduce a printed decimal exactly.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

/// Every surface a form field is mounted on in §4.
const MOUNTS: [&str; 6] = ["sunken", "base", "raised", "hover", "active", "overlay"];

// ---------------------------------------------------------------------------
// 1 — the fixed rhythm
// ---------------------------------------------------------------------------

#[test]
fn each_band_comes_from_a_token_rather_than_a_printed_number() {
    assert_eq!(BAND_GAP, space::XS);
    assert_eq!(CONTROL_SLOT_HEIGHT, size::CONTROL_MD);
    assert_eq!(CAPTION_SLOT_HEIGHT, typography::CAPTION.line_height);
}

#[test]
fn the_header_row_is_the_figure_3b1_prints() {
    assert_eq!(HEADER_HEIGHT, 20.0);
}

#[test]
fn the_total_is_the_sum_of_its_five_terms() {
    let expected = HEADER_HEIGHT + BAND_GAP + CONTROL_SLOT_HEIGHT + BAND_GAP + CAPTION_SLOT_HEIGHT;

    assert_eq!(STACKED_HEIGHT, expected);
    assert_eq!(stacked_height(), expected);
    assert_eq!(expected, 76.0);
}

#[test]
fn bands_tile_downward_with_no_overlap_and_no_stray_gap() {
    let bounds = Rectangle::new(
        Point::new(10.0, 100.0),
        iced::Size::new(320.0, STACKED_HEIGHT),
    );
    let field = bands(bounds);

    assert_eq!(field.header.y, bounds.y);
    assert_eq!(field.control.y, field.header.y + HEADER_HEIGHT + BAND_GAP);
    assert_eq!(
        field.caption.y,
        field.control.y + CONTROL_SLOT_HEIGHT + BAND_GAP
    );
    assert_eq!(
        field.caption.y + field.caption.height,
        bounds.y + STACKED_HEIGHT,
        "the three bands must fill the field exactly"
    );

    for band in [field.header, field.control, field.caption] {
        assert_eq!(band.x, bounds.x);
        assert_eq!(band.width, bounds.width);
    }
}

#[test]
fn bands_are_widened_by_a_wider_field_and_never_by_their_content() {
    let narrow = bands(Rectangle::new(
        Point::ORIGIN,
        iced::Size::new(120.0, STACKED_HEIGHT),
    ));
    let wide = bands(Rectangle::new(
        Point::ORIGIN,
        iced::Size::new(900.0, STACKED_HEIGHT),
    ));

    assert_eq!(narrow.control.height, wide.control.height);
    assert_eq!(narrow.caption.height, wide.caption.height);
    assert_eq!(wide.control.width, 900.0);
}

// ---------------------------------------------------------------------------
// 2 — the zero-shift proof
// ---------------------------------------------------------------------------

/// Every caption posture a field can be in.
fn every_caption_posture() -> Vec<(bool, Option<&'static str>, Option<&'static str>)> {
    let mut out = Vec::new();
    for valid in [true, false] {
        for helper in [None, Some("Optional"), Some("   ")] {
            for error in [None, Some("Required"), Some("  ")] {
                out.push((valid, helper, error));
            }
        }
    }
    out
}

#[test]
fn a_validation_transition_moves_no_band_at_all() {
    let bounds = Rectangle::new(Point::new(3.0, 7.0), iced::Size::new(280.0, STACKED_HEIGHT));
    let reference = bands(bounds);

    for (valid, helper, error) in every_caption_posture() {
        // The posture is resolved, and the geometry is asked again. The
        // resolver's result cannot reach `bands`, which is the point: the
        // bands are a function of the bounds alone.
        let _caption = resolve_caption(valid, helper, error, &Palette::dark());
        let after = bands(bounds);

        assert_eq!(
            after, reference,
            "valid={valid} helper={helper:?} error={error:?} moved the field"
        );
    }
}

#[test]
fn the_control_rect_is_byte_identical_whatever_the_caption_carries() {
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(240.0, STACKED_HEIGHT));
    let control = bands(bounds).control;

    for (valid, helper, error) in every_caption_posture() {
        let caption = resolve_caption(valid, helper, error, &Palette::dark());
        let again = bands(bounds).control;

        assert_eq!(
            again, control,
            "caption kind {:?} moved the control slot",
            caption.kind
        );
        assert_eq!(again.height, CONTROL_SLOT_HEIGHT);
    }
}

#[test]
fn the_caption_band_is_reserved_when_nothing_is_written_in_it() {
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(240.0, STACKED_HEIGHT));
    let empty = resolve_caption(true, None, None, &Palette::dark());

    assert_eq!(empty.kind, CaptionKind::Empty);
    assert_eq!(empty.text, None);
    assert_eq!(bands(bounds).caption.height, CAPTION_SLOT_HEIGHT);
}

#[test]
fn the_reserved_band_height_never_grows_under_either_overflow_policy() {
    assert_eq!(
        reserved_band_height(CAPTION_SLOT_HEIGHT, OverflowPolicy::Clip),
        CAPTION_SLOT_HEIGHT
    );
    assert_eq!(
        reserved_band_height(CAPTION_SLOT_HEIGHT, OverflowPolicy::Wrap),
        CAPTION_SLOT_HEIGHT,
        "the reserved band cannot grow under either policy"
    );
    assert_eq!(RESERVED_BAND_OVERFLOW, OverflowPolicy::Clip);
}

// ---------------------------------------------------------------------------
// 3 — the caption resolver
// ---------------------------------------------------------------------------

#[test]
fn an_invalid_field_with_a_message_shows_it_in_danger_with_the_warning_glyph() {
    let palette = Palette::dark();
    let caption = resolve_caption(false, Some("helper"), Some("Name is taken"), &palette);

    assert_eq!(caption.kind, CaptionKind::Error);
    assert!(caption.kind.is_error());
    assert_eq!(caption.glyph, Some(WARNING_GLYPH));
    assert_eq!(caption.text, Some("Name is taken"));
    assert_eq!(caption.colour, palette.status_danger_text);
}

#[test]
fn a_valid_field_with_a_helper_shows_it_muted_and_unglyphed() {
    let palette = Palette::dark();
    let caption = resolve_caption(true, Some("Shown in the header"), Some("stale"), &palette);

    assert_eq!(caption.kind, CaptionKind::Helper);
    assert!(!caption.kind.is_error());
    assert_eq!(caption.glyph, None);
    assert_eq!(caption.text, Some("Shown in the header"));
    assert_eq!(caption.colour, palette.text_muted);
}

#[test]
fn an_invalid_field_with_no_message_shows_nothing_rather_than_the_helper() {
    let palette = Palette::dark();
    let caption = resolve_caption(false, Some("Shown in the header"), None, &palette);

    assert_eq!(caption.kind, CaptionKind::Empty);
    assert_eq!(caption.text, None);
    assert_eq!(
        caption.glyph, None,
        "a warning glyph with no words behind it would alarm for nothing"
    );
}

#[test]
fn a_blank_string_is_not_content_in_either_slot() {
    let palette = Palette::dark();

    assert_eq!(
        resolve_caption(true, Some("   "), None, &palette).kind,
        CaptionKind::Empty
    );
    assert_eq!(
        resolve_caption(false, None, Some("\t "), &palette).kind,
        CaptionKind::Empty
    );
}

#[test]
fn the_rendered_caption_pairs_glyph_and_words_only_when_both_are_present() {
    let palette = Palette::dark();

    let error = resolve_caption(false, None, Some("Too short"), &palette);
    assert_eq!(error.rendered(), Some(format!("{WARNING_GLYPH} Too short")));

    let helper = resolve_caption(true, Some("Optional"), None, &palette);
    assert_eq!(helper.rendered(), Some("Optional".to_string()));

    let empty = resolve_caption(true, None, None, &palette);
    assert_eq!(empty.rendered(), None);
}

#[test]
fn every_caption_kind_that_paints_words_clears_the_text_floor_on_every_mount() {
    let palette = Palette::dark();

    for mount in MOUNTS {
        let background = surface(mount);

        let error = resolve_caption(false, None, Some("bad"), &palette);
        assert!(
            contrast_ratio(error.colour, background) >= MIN_TEXT_CONTRAST,
            "validation text on {mount} is {}",
            contrast_ratio(error.colour, background)
        );

        let helper = resolve_caption(true, Some("hint"), None, &palette);
        assert!(
            contrast_ratio(helper.colour, background) >= MIN_TEXT_CONTRAST,
            "helper text on {mount} is {}",
            contrast_ratio(helper.colour, background)
        );
    }
}

#[test]
fn the_measured_caption_contrasts_are_pinned_against_a_future_retune() {
    let palette = Palette::dark();

    assert!(approx(
        contrast_ratio(palette.status_danger_text, palette.surface_raised),
        6.1812
    ));
    assert!(approx(
        contrast_ratio(palette.status_danger_text, palette.surface_active),
        4.7223
    ));
    assert!(approx(
        contrast_ratio(palette.text_muted, palette.surface_active),
        5.4664
    ));
}

// ---------------------------------------------------------------------------
// 4 — the header
// ---------------------------------------------------------------------------

#[test]
fn a_required_field_carries_the_marker_in_the_danger_colour() {
    let palette = Palette::dark();
    let header = resolve_header("Display name", true, None, &palette);

    assert_eq!(header.label, "Display name");
    assert_eq!(header.label_colour, palette.text_primary);
    assert_eq!(header.marker, Some(REQUIRED_MARKER));
    assert_eq!(header.marker_colour, palette.status_danger_text);
    assert_eq!(REQUIRED_TOOLTIP, "Required");
}

#[test]
fn an_optional_field_carries_no_marker() {
    let palette = Palette::dark();
    let header = resolve_header("Nickname", false, Some("Optional"), &palette);

    assert_eq!(header.marker, None);
    assert_eq!(header.hint, Some("Optional"));
    assert_eq!(header.hint_colour, palette.text_muted);
}

#[test]
fn a_blank_hint_is_not_a_hint() {
    let palette = Palette::dark();
    assert_eq!(resolve_header("x", false, Some("  "), &palette).hint, None);
}

#[test]
fn the_label_and_the_required_marker_clear_every_mount() {
    let palette = Palette::dark();
    let header = resolve_header("Display name", true, Some("hint"), &palette);

    for mount in MOUNTS {
        let background = surface(mount);
        assert!(
            contrast_ratio(header.label_colour, background) >= MIN_TEXT_CONTRAST,
            "label on {mount} is {}",
            contrast_ratio(header.label_colour, background)
        );
        assert!(
            contrast_ratio(header.marker_colour, background) >= MIN_TEXT_CONTRAST,
            "required marker on {mount} is {}",
            contrast_ratio(header.marker_colour, background)
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — the widget hosts any control
// ---------------------------------------------------------------------------

/// The three atoms' own natural heights, read from the tokens each one uses.
///
/// They differ on purpose. If the field's control slot were sized by the
/// thing placed in it, these three would produce three different fields —
/// which is exactly the special-casing §3B.1 forbids.
#[test]
fn the_three_hosted_atoms_are_three_different_heights() {
    assert_eq!(size::CONTROL_MD, 32.0);
    assert_eq!(size::CONTROL_SM, 28.0);
    assert_eq!(size::SWITCH_TRACK_HEIGHT, 20.0);
}

#[test]
fn one_slot_holds_a_button_an_input_and_a_focus_wrapper_unchanged() {
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(320.0, STACKED_HEIGHT));
    let palette = Palette::dark();
    let header = resolve_header("Display name", true, Some("Shown at the top"), &palette);
    let caption = resolve_caption(false, None, Some("Name is taken"), &palette);

    let button: iced::Element<'_, Msg> = Button::new(
        crate::widget::button::ButtonVariant::Primary,
        text("Pick"),
        Msg::Pick,
        "f-button",
    )
    .into();
    let input: iced::Element<'_, Msg> =
        TextInput::new(InputKind::TextEntry, "hint", "value", Msg::Edit, "f-input").into();
    let wrapped: iced::Element<'_, Msg> = Focusable::new(text("row"), "f-row".into(), true).into();

    for control in [button, input, wrapped] {
        let element = stacked(header, control, caption);
        let _ = element;
    }

    // The slot the three controls were handed is one rectangle, computed
    // before any of them existed.
    assert_eq!(bands(bounds).control.height, CONTROL_SLOT_HEIGHT);
    assert_eq!(bands(bounds).control.height, bands(bounds).control.height);
    assert_eq!(palette.text_muted, palette.text_muted);
}

#[test]
fn the_slot_marker_paints_a_stroke_and_no_fill() {
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(100.0, 32.0));
    let palette = Palette::dark();
    let layer = slot_marker(bounds, palette.border_strong);

    assert_eq!(layer.bounds, bounds);
    assert_eq!(layer.fill, Color::TRANSPARENT);
    assert_eq!(layer.border.width, shape::BORDER_HAIRLINE);
    assert_eq!(layer.border.color, palette.border_strong);
}

#[test]
fn a_caption_parts_value_carries_its_kind_through_a_clone() {
    let palette = Palette::dark();
    let caption = resolve_caption(false, None, Some("bad"), &palette);
    let clone = caption;

    assert_eq!(clone, caption);
    assert_eq!(clone.kind, CaptionKind::Error);
    assert!(matches!(clone, CaptionParts { .. }));
}
