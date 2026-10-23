//! Tests for the badge resolver and geometry — `UIF-ATOM-008`, §3.8.
//!
//! Five groups, in order:
//!
//! 1. **The boxes.** The `6` dot, the `18` pill, the `24` chip, each
//!    pinned to the number §3.8 prints.
//! 2. **The recomputed contrast.** Every chip pair measured from raw
//!    sRGB, including the three figures §3.8 prints wrong.
//! 3. **The fill lock.** The solid pill's `Locked` verdict and the
//!    dot's `ForegroundNotPainted` one.
//! 4. **Perimeter classification.** Which class each perimeter lands in
//!    and why.
//! 5. **Width and determinism.** The pill's floor, the chip's glyph
//!    charge, and the resolver's purity.

use super::*;
use crate::token::contrast;
use crate::widget::badge_model::StatusIcon;

/// Every tone, in declaration order.
const TONES: [BadgeTone; 5] = [
    BadgeTone::Neutral,
    BadgeTone::Accent,
    BadgeTone::Success,
    BadgeTone::Warning,
    BadgeTone::Danger,
];

/// The chip's mount — §3.8 paints chips on `surface.raised`.
fn mount(palette: &Palette) -> iced::Color {
    palette.surface_raised
}

/// Assert a measured ratio against its true value to four places.
fn assert_ratio(actual: f32, expected: f32, what: &str) {
    let measured = f64::from(actual);
    let wanted = f64::from(expected);
    assert!(
        (measured - wanted).abs() < 0.000_5,
        "{what}: expected {wanted:.4}, measured {measured:.4}"
    );
}

#[test]
fn the_dot_is_six_pixels_of_pill_and_draws_no_stroke() {
    let style = resolve_style(&BadgeModel::dot(), &Palette::dark());

    assert_eq!(style.height, DOT_SIZE);
    assert_eq!(DOT_SIZE, size::INDICATOR_DOT);
    assert_eq!(style.min_width, DOT_SIZE);
    assert_eq!(style.h_padding, 0.0);
    assert_eq!(style.radius, shape::RADIUS_PILL);
    assert_eq!(
        style.border_width, 0.0,
        "the dot has no perimeter stroke to draw"
    );
    assert!(!style.shows_glyph);
}

#[test]
fn the_solid_count_pill_is_accent_fill_behind_text_strong() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(12), &palette);

    assert_eq!(style.height, COUNT_HEIGHT);
    assert_eq!(COUNT_HEIGHT, size::BADGE_COUNT_HEIGHT);
    assert_eq!(style.min_width, COUNT_MIN_WIDTH);
    assert_eq!(style.h_padding, space::XS);
    assert_eq!(style.fill, palette.accent_fill);
    assert_eq!(
        style.border, style.fill,
        "the solid pill's perimeter is its fill, not a separate stroke"
    );
    assert_eq!(style.text, palette.text_strong);
    assert_eq!(style.weight, Weight::SemiBold);
    assert_eq!(style.font_size, 12.0);
}

#[test]
fn the_danger_solid_pill_is_danger_fill_behind_text_strong() {
    let palette = Palette::dark();
    let style = resolve_style(
        &BadgeModel::count(12).with_tone(BadgeTone::Danger),
        &palette,
    );

    assert_eq!(style.fill, palette.status_danger_fill);
    assert_eq!(
        style.border, style.fill,
        "the solid pill's perimeter is its fill, whatever tone it wears"
    );
    assert_eq!(style.text, palette.text_strong);

    let lock = contrast::fill_lock(style.fill, style.text, &palette);
    assert_eq!(lock, contrast::FillLock::Locked);
    assert!(lock.governed());
    assert!(lock.is_compliant());
    assert_ratio(
        contrast::contrast_ratio(style.text, style.fill),
        5.1270,
        "text.strong on status.danger.fill",
    );
}

#[test]
fn a_subtle_count_pill_sits_on_the_active_surface_behind_a_hairline() {
    let palette = Palette::dark();

    for tone in [BadgeTone::Neutral, BadgeTone::Success, BadgeTone::Warning] {
        let style = resolve_style(&BadgeModel::count(12).with_tone(tone), &palette);

        // §3.8's subtle branch names one background and one foreground
        // for every tone, so the pill looks the same whichever tone it
        // carries — the tone selects the solid branch's fill and the
        // chip's perimeter, not this.
        assert_eq!(style.fill, palette.surface_active, "{tone:?}");
        assert_eq!(style.text, palette.text_strong, "{tone:?}");
        assert_eq!(style.border, palette.border_subtle, "{tone:?}");
        assert_eq!(style.border_width, CHIP_BORDER_WIDTH, "{tone:?}");
        assert_eq!(style.border_width, 1.0, "{tone:?}");
        assert_ne!(
            style.border, style.fill,
            "{tone:?} must draw a boundary against its own fill"
        );
        assert_ratio(
            contrast::contrast_ratio(style.text, style.fill),
            14.2946,
            "the subtle pill's text.strong on surface.active",
        );
    }
}

#[test]
fn the_status_chip_is_twenty_four_tall_and_always_carries_its_glyph() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::status(StatusIcon::Success, "Saved"), &palette);

    assert_eq!(style.height, CHIP_HEIGHT);
    assert_eq!(CHIP_HEIGHT, size::BADGE_CHIP_HEIGHT);
    assert_eq!(style.h_padding, space::SM);
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.fill, palette.surface_raised);
    assert_eq!(style.weight, Weight::Medium);
    assert_eq!(style.font_size, 12.0);
    assert!(style.shows_glyph, "§3.8 makes the leading glyph mandatory");
}

#[test]
fn a_zero_count_paints_nothing_at_all() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(0), &palette);

    assert_eq!(style.fill, Color::TRANSPARENT);
    assert_eq!(style.border, Color::TRANSPARENT);
    assert_eq!(style.text, Color::TRANSPARENT);
    assert_eq!(style.glyph, Color::TRANSPARENT);
    assert_eq!(style.border_width, 0.0);
    assert!(
        style.fill.a < 1.0 && style.text.a < 1.0,
        "nothing visible may survive the empty-count override"
    );
}

#[test]
fn the_recomputed_chip_contrasts_are_the_true_ones_not_the_printed_ones() {
    let palette = Palette::dark();
    let surface = mount(&palette);

    // §3.8 prints 13.52 / 6.10 / 9.96 beside these three. All three
    // are wrong: the first two were measured against `surface.overlay`
    // (13.5237 and 6.1048) and the last is reproduced by no surface at
    // all. The true figures are pinned here so a future retune is
    // measured rather than copied forward.
    assert_ratio(
        contrast::contrast_ratio(palette.status_success, surface),
        14.2825,
        "status.success on surface.raised",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.status_warning, surface),
        6.4474,
        "status.warning on surface.raised",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.accent_text, surface),
        10.0215,
        "accent.text on surface.raised",
    );
    // This one the spec prints correctly.
    assert_ratio(
        contrast::contrast_ratio(palette.status_danger_text, surface),
        6.1812,
        "status.danger.text on surface.raised",
    );
}

#[test]
fn every_chip_label_clears_the_text_contrast_floor() {
    let palette = Palette::dark();
    let surface = mount(&palette);

    for tone in TONES {
        let style = resolve_style(
            &BadgeModel::status(StatusIcon::Info, "x").with_tone(tone),
            &palette,
        );
        let ratio = contrast::contrast_ratio(style.text, surface);
        assert!(
            ratio >= contrast::MIN_TEXT_CONTRAST,
            "{tone:?} chip label reaches only {ratio:.4}:1 against the mount"
        );
    }
}

#[test]
fn the_danger_chip_writes_both_its_word_and_its_edge_in_the_text_token() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::status(StatusIcon::Danger, "Failed"), &palette);

    // §3.8 names `status.danger.text` for the chip's perimeter and never
    // names the fill there at all — the fill token appears only as the
    // count pill's solid body, where rule 4 locks `text.strong` over it.
    assert_eq!(style.text, palette.status_danger_text);
    assert_eq!(style.border, palette.status_danger_text);

    let surface = mount(&palette);
    let as_fill = contrast::contrast_ratio(palette.status_danger_fill, surface);
    let as_text = contrast::contrast_ratio(palette.status_danger_text, surface);
    assert!(
        as_fill < contrast::MIN_TEXT_CONTRAST,
        "the fill colour would not have been readable as a label at {as_fill:.4}:1"
    );
    assert!(as_text >= contrast::MIN_TEXT_CONTRAST);
}

#[test]
fn the_neutral_chip_wears_border_strong_so_it_stays_measured() {
    let palette = Palette::dark();
    let style = resolve_style(
        &BadgeModel::status(StatusIcon::Info, "Note").with_tone(BadgeTone::Neutral),
        &palette,
    );

    assert_eq!(style.border, palette.border_strong);
    assert_ne!(
        style.border, palette.border_subtle,
        "border.subtle would classify as a resting divider and drop out of \
         measurement, letting an indicator hide behind a decorative exemption"
    );
    assert!(badge_chrome_class(&style, &palette) == ChromeClass::Judged);
}

#[test]
fn the_solid_pill_is_verbally_locked_by_rule_four() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(9), &palette);

    let lock = contrast::fill_lock(style.fill, style.text, &palette);
    assert_eq!(lock, contrast::FillLock::Locked);
    assert!(
        lock.governed(),
        "the lock must have applied, not been skipped"
    );
    assert!(lock.is_compliant());
    assert_ratio(
        contrast::contrast_ratio(style.text, style.fill),
        4.8020,
        "text.strong on accent.fill",
    );
}

#[test]
fn the_dot_sits_outside_the_fill_lock_because_it_wears_no_locked_fill() {
    let palette = Palette::dark();

    for tone in TONES {
        let style = resolve_style(&BadgeModel::dot().with_tone(tone), &palette);
        let lock = contrast::fill_lock(style.fill, Color::TRANSPARENT, &palette);

        // §3.8 paints the dot in `accent.border`, `status.warning`,
        // `status.success` and `status.danger.text` — none of which is
        // one of the four solid fills rule 4 governs. The rule therefore
        // never reaches the dot, which is the honest verdict: it is not
        // that the dot passes the lock, it is that the lock has no
        // business with it.
        assert_eq!(lock, contrast::FillLock::NotSolidFill, "{tone:?} dot");
        assert!(lock.is_compliant(), "{tone:?} dot");
        assert!(
            !lock.governed(),
            "{tone:?} dot must not be counted as an exercised fill-lock, \
             because no locked fill is involved"
        );
    }
}

#[test]
fn a_locked_fill_with_nothing_painted_reports_foreground_not_painted() {
    let palette = Palette::dark();

    // The `ForegroundNotPainted` verdict is still reachable — a solid
    // pill with its label suppressed is exactly that — even though no
    // badge resolves to it today.
    for fill in [
        palette.accent_fill,
        palette.accent_fill_active,
        palette.status_danger_fill,
        palette.status_danger_fill_active,
    ] {
        let lock = contrast::fill_lock(fill, Color::TRANSPARENT, &palette);
        assert_eq!(lock, contrast::FillLock::ForegroundNotPainted);
        assert!(lock.governed());
        assert!(lock.is_compliant());
    }
}

#[test]
fn a_non_solid_badge_is_outside_the_fill_lock_entirely() {
    let palette = Palette::dark();

    for model in [
        BadgeModel::status(StatusIcon::Success, "Saved").with_tone(BadgeTone::Warning),
        BadgeModel::count(4).with_tone(BadgeTone::Neutral),
    ] {
        let style = resolve_style(&model, &palette);
        let lock = contrast::fill_lock(style.fill, style.text, &palette);
        assert_eq!(
            lock,
            contrast::FillLock::NotSolidFill,
            "a surface behind a label is not a locked fill"
        );
    }
}

#[test]
fn resolving_every_kind_and_tone_honours_the_fill_lock() {
    let palette = Palette::dark();

    for tone in TONES {
        for model in [
            BadgeModel::dot().with_tone(tone),
            BadgeModel::count(0).with_tone(tone),
            BadgeModel::count(7).with_tone(tone),
            BadgeModel::count(150).with_tone(tone),
            BadgeModel::status(StatusIcon::Info, "Note").with_tone(tone),
        ] {
            let style = resolve_style(&model, &palette);
            let foreground = if matches!(model.kind(), BadgeKind::Dot) {
                Color::TRANSPARENT
            } else {
                style.text
            };
            assert!(
                contrast::fill_lock(style.fill, foreground, &palette).is_compliant(),
                "{tone:?} on {:?} breached rule 4",
                model.kind()
            );
        }
    }
}

#[test]
fn the_dot_and_the_solid_pill_are_coincident_with_their_fill() {
    let palette = Palette::dark();

    for model in [BadgeModel::dot(), BadgeModel::count(3)] {
        let style = resolve_style(&model, &palette);
        assert_eq!(
            badge_chrome_class(&style, &palette),
            ChromeClass::CoincidentWithFill,
            "{:?} draws no boundary of its own",
            model.kind()
        );
        assert_eq!(
            ChromeClass::CoincidentWithFill.minimum_contrast(),
            Some(contrast::MIN_COINCIDENT_FILL_CONTRAST)
        );
    }
}

#[test]
fn the_coincident_fill_reaches_the_floor_it_now_owes() {
    let palette = Palette::dark();

    for tone in TONES {
        let model = BadgeModel::count(3).with_tone(tone);
        let style = resolve_style(&model, &palette);
        if badge_chrome_class(&style, &palette) != ChromeClass::CoincidentWithFill {
            continue;
        }
        for surface in [
            palette.surface_base,
            palette.surface_raised,
            palette.surface_overlay,
        ] {
            let ratio = contrast::contrast_ratio(style.fill, surface);
            assert!(
                ratio >= contrast::MIN_COINCIDENT_FILL_CONTRAST,
                "{tone:?} coincident fill reaches only {ratio:.4}:1 on a mount"
            );
        }
    }
}

#[test]
fn every_chips_status_border_is_judged_against_the_chrome_floor() {
    let palette = Palette::dark();

    for tone in TONES {
        let style = resolve_style(
            &BadgeModel::status(StatusIcon::Info, "Note").with_tone(tone),
            &palette,
        );
        let class = badge_chrome_class(&style, &palette);
        assert_eq!(class, ChromeClass::Judged, "{tone:?}");
        let ratio = contrast::contrast_ratio(style.border, palette.surface_raised);
        assert!(
            ratio >= contrast::MIN_CHROME_CONTRAST,
            "{tone:?} chip border reaches only {ratio:.4}:1 against the mount"
        );
    }
}

#[test]
fn a_subtle_border_is_still_classified_as_a_resting_divider() {
    let palette = Palette::dark();
    let mut style = resolve_style(&BadgeModel::status(StatusIcon::Info, "Note"), &palette);
    style.border = palette.border_subtle;

    assert_eq!(
        badge_chrome_class(&style, &palette),
        ChromeClass::RestingDivider,
        "the classifier must stay honest about a border no badge resolves to today"
    );
    assert!(ChromeClass::RestingDivider.is_exempt());
}

#[test]
fn the_pill_holds_its_floor_for_a_single_digit() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(1), &palette);

    assert_eq!(pill_width(&style, 6.0), COUNT_MIN_WIDTH);
    assert_eq!(pill_width(&style, 0.0), COUNT_MIN_WIDTH);
    // The exact boundary: a 10px glyph plus two 4px paddings is 18px,
    // so the floor and the content meet here and nowhere else.
    assert_eq!(pill_width(&style, 10.0), COUNT_MIN_WIDTH);
    assert!(pill_width(&style, 10.5) > COUNT_MIN_WIDTH);
}

#[test]
fn the_pill_widens_for_the_folded_count() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(150), &palette);
    let folded_text_width = 22.0;

    assert_eq!(
        pill_width(&style, folded_text_width),
        folded_text_width + space::XS * 2.0
    );
    assert!(pill_width(&style, folded_text_width) > COUNT_MIN_WIDTH);
}

#[test]
fn the_chip_always_charges_its_glyph_box() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::status(StatusIcon::Success, "Saved"), &palette);

    let with_glyph = chip_width(&style, 12.0, 40.0);
    assert_eq!(
        with_glyph,
        12.0 + GLYPH_LABEL_GAP + 40.0 + style.h_padding * 2.0
    );

    let no_glyph_style = resolve_style(&BadgeModel::count(150), &palette);
    assert!(!no_glyph_style.shows_glyph);
    assert_eq!(
        chip_width(&no_glyph_style, 12.0, 40.0),
        40.0 + no_glyph_style.h_padding * 2.0,
        "a style that shows no glyph must not charge for one"
    );
}

#[test]
fn the_rung_permits_both_weights_the_spec_asks_for() {
    assert!(TYPE_RUNG.allows(COUNT_WEIGHT));
    assert!(TYPE_RUNG.allows(CHIP_WEIGHT));
    assert_eq!(TYPE_RUNG.size, 12.0);
    assert_eq!(TYPE_RUNG, typography::CAPTION);
}

#[test]
fn resolve_tone_maps_each_tone_to_its_palette_entries() {
    let palette = Palette::dark();

    let neutral = resolve_tone(BadgeTone::Neutral, &palette);
    assert_eq!(neutral.text, palette.text_muted);
    assert_eq!(neutral.border, palette.border_strong);

    let accent = resolve_tone(BadgeTone::Accent, &palette);
    assert_eq!(accent.text, palette.accent_text);
    assert_eq!(accent.border, palette.accent_text);
    assert_eq!(accent.dot_fill, palette.accent_border);
    assert_eq!(accent.solid_fill, palette.accent_fill);

    let success = resolve_tone(BadgeTone::Success, &palette);
    assert_eq!(success.text, palette.status_success);
    assert_eq!(success.border, palette.status_success);

    let warning = resolve_tone(BadgeTone::Warning, &palette);
    assert_eq!(warning.text, palette.status_warning);
    assert_eq!(warning.border, palette.status_warning);

    let danger = resolve_tone(BadgeTone::Danger, &palette);
    assert_eq!(danger.text, palette.status_danger_text);
    assert_eq!(danger.border, palette.status_danger_text);
    assert_eq!(danger.dot_fill, palette.status_danger_text);
    assert_eq!(danger.solid_fill, palette.status_danger_fill);
}

#[test]
fn the_resolver_is_deterministic_across_every_cell() {
    let palette = Palette::dark();

    for tone in TONES {
        for model in [
            BadgeModel::dot().with_tone(tone),
            BadgeModel::count(42).with_tone(tone),
            BadgeModel::status(StatusIcon::Warning, "Slow").with_tone(tone),
        ] {
            assert_eq!(
                resolve_style(&model, &palette),
                resolve_style(&model, &palette),
                "{tone:?} on {:?} resolved twice differently",
                model.kind()
            );
        }
    }
}

#[test]
fn no_badge_style_carries_an_unnamed_zero_where_a_property_should_be_absent() {
    let palette = Palette::dark();
    let style = resolve_style(&BadgeModel::count(8), &palette);

    assert!(style.height > 0.0);
    assert!(style.min_width > 0.0);
    assert!(style.font_size >= typography::MIN_SIZE);
    assert_eq!(style.radius, shape::RADIUS_PILL);
}
