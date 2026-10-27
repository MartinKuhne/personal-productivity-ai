//! Tests for the badge data model — `UIF-ATOM-008`, §3.8.
//!
//! Three groups, in order:
//!
//! 1. **The constructors.** What each kind asserts, and the tone it
//!    arrives with.
//! 2. **The count ceiling.** The fold boundary, run at 99 and 100
//!    rather than described.
//! 3. **Value semantics.** Purity, equality, and the fact that nothing
//!    here mutates itself.

use super::*;

/// Every tone, in declaration order.
const TONES: [BadgeTone; 5] = [
    BadgeTone::Neutral,
    BadgeTone::Accent,
    BadgeTone::Success,
    BadgeTone::Warning,
    BadgeTone::Danger,
];

/// Every status glyph, in declaration order.
const ICONS: [StatusIcon; 4] = [
    StatusIcon::Success,
    StatusIcon::Warning,
    StatusIcon::Danger,
    StatusIcon::Info,
];

#[test]
fn the_dot_asserts_presence_and_arrives_in_the_accent_tone() {
    let model = BadgeModel::dot();

    assert_eq!(model.kind(), &BadgeKind::Dot);
    assert_eq!(model.tone(), BadgeTone::Accent);
    assert_eq!(model.text(), None, "a dot has no words to paint");
    assert!(
        !model.is_empty(),
        "a dot is never the empty case, even though it carries no count"
    );
}

#[test]
fn the_count_pill_keeps_the_raw_number_and_folds_only_at_paint_time() {
    let model = BadgeModel::count(137);

    assert_eq!(model.kind(), &BadgeKind::Count(137));
    assert_eq!(
        model.text().as_deref(),
        Some("99+"),
        "the model holds the real number and renders the folded one"
    );
}

#[test]
fn a_zero_count_is_the_only_empty_badge() {
    assert!(BadgeModel::count(0).is_empty());
    assert!(!BadgeModel::count(1).is_empty());
    assert!(!BadgeModel::dot().is_empty());
    assert!(!BadgeModel::status(StatusIcon::Success, "Saved").is_empty());
}

#[test]
fn a_zero_count_still_reports_its_text() {
    assert_eq!(BadgeModel::count(0).text().as_deref(), Some("0"));
}

#[test]
fn a_status_chip_takes_the_tone_its_glyph_implies() {
    for icon in ICONS {
        let model = BadgeModel::status(icon, "Done");

        assert_eq!(
            model.tone(),
            icon.implied_tone(),
            "{icon:?} must arrive in the tone that matches its glyph"
        );
        assert_eq!(
            model.kind(),
            &BadgeKind::Status {
                icon,
                label: "Done"
            }
        );
        assert_eq!(model.text().as_deref(), Some("Done"));
    }
}

#[test]
fn with_tone_replaces_the_implied_tone_and_nothing_else() {
    for tone in TONES {
        let model = BadgeModel::status(StatusIcon::Success, "Saved").with_tone(tone);

        assert_eq!(model.tone(), tone);
        assert_eq!(
            model.kind(),
            &BadgeKind::Status {
                icon: StatusIcon::Success,
                label: "Saved"
            },
            "changing the tone must not disturb what the badge asserts"
        );
    }
}

#[test]
fn with_tone_leaves_the_original_model_alone() {
    let original = BadgeModel::count(3);
    let changed = original.clone().with_tone(BadgeTone::Danger);

    assert_eq!(original.tone(), BadgeTone::Accent);
    assert_eq!(changed.tone(), BadgeTone::Danger);
}

#[test]
fn each_glyph_is_the_one_section_3_8_prints() {
    assert_eq!(StatusIcon::Success.glyph(), '✓');
    assert_eq!(StatusIcon::Warning.glyph(), '⚠');
    assert_eq!(StatusIcon::Danger.glyph(), '✕');
    assert_eq!(StatusIcon::Info.glyph(), '●');
}

#[test]
fn the_accent_and_danger_tones_are_the_two_allowed_a_solid_fill() {
    // §3.8 names exactly two solid pill backgrounds — `accent.fill` and
    // `status.danger.fill` — so exactly the tones that own them.
    assert!(BadgeTone::Accent.paints_solid());
    assert!(BadgeTone::Danger.paints_solid());
    assert!(!BadgeTone::Neutral.paints_solid());
    assert!(!BadgeTone::Success.paints_solid());
    assert!(!BadgeTone::Warning.paints_solid());
}

#[test]
fn neutral_is_the_default_tone() {
    assert_eq!(BadgeTone::default(), BadgeTone::Neutral);
}

#[test]
fn the_ceiling_does_not_fold() {
    assert_eq!(format_count(COUNT_CEILING), "99");
    assert_eq!(
        BadgeModel::count(COUNT_CEILING).text().as_deref(),
        Some("99")
    );
}

#[test]
fn one_above_the_ceiling_folds() {
    assert_eq!(format_count(COUNT_CEILING + 1), "99+");
}

#[test]
fn every_folded_form_is_the_same_string() {
    for n in [100_u32, 101, 999, 10_000, u32::MAX] {
        assert_eq!(
            format_count(n),
            "99+",
            "{n} must fold to the same glyph run"
        );
    }
}

#[test]
fn the_small_counts_print_themselves() {
    for n in 0_u32..=99 {
        assert_eq!(format_count(n), n.to_string(), "{n} must not fold");
    }
}

#[test]
fn constructing_twice_gives_an_equal_model() {
    assert_eq!(BadgeModel::count(7), BadgeModel::count(7));
    assert_eq!(BadgeModel::dot(), BadgeModel::dot());
    assert_eq!(
        BadgeModel::status(StatusIcon::Warning, "Slow"),
        BadgeModel::status(StatusIcon::Warning, "Slow")
    );
    assert_ne!(BadgeModel::count(7), BadgeModel::count(8));
    assert_ne!(
        BadgeModel::status(StatusIcon::Warning, "Slow"),
        BadgeModel::status(StatusIcon::Danger, "Slow")
    );
}

#[test]
fn every_kind_and_tone_combination_is_constructible() {
    for tone in TONES {
        for model in [
            BadgeModel::dot().with_tone(tone),
            BadgeModel::count(5).with_tone(tone),
            BadgeModel::status(StatusIcon::Info, "Note").with_tone(tone),
        ] {
            assert_eq!(model.tone(), tone);
        }
    }
}
