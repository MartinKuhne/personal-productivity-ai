//! Unit tests for [`crate::data`].

use super::*;

#[test]
fn navbar_triggers_have_unique_ids() {
    let mut ids: Vec<&str> = NAVBAR_TRIGGERS.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "navbar trigger ids must be unique");
}

#[test]
fn navbar_model_is_built_from_the_declared_triggers() {
    let model = navbar_model();
    assert_eq!(model.triggers.len(), NAVBAR_TRIGGERS.len());
    assert_eq!(model.brand.as_deref(), Some(BRAND));
}

#[test]
fn split_ratio_bounds_are_ordered_and_sane() {
    const _: () = {
        assert!(SPLIT_RATIO_MIN < SPLIT_RATIO_DEFAULT);
        assert!(SPLIT_RATIO_DEFAULT < SPLIT_RATIO_MAX);
        assert!(SPLIT_RATIO_MIN > 0.0);
        assert!(SPLIT_RATIO_MAX < 1.0);
    };
}

#[test]
fn building_the_model_twice_yields_equal_models() {
    assert_eq!(navbar_model(), navbar_model());
}

#[test]
fn combo_option_ids_are_unique() {
    let mut ids: Vec<&str> = COMBO_OPTIONS.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "combo option ids must be unique");
}

#[test]
fn combo_options_are_built_from_the_declared_fixture() {
    let options = combo_options();
    assert_eq!(options.len(), COMBO_OPTIONS.len());
    assert_eq!(options[0].label, COMBO_OPTIONS[0].1);
}

#[test]
fn every_default_selection_names_a_declared_option() {
    assert!(
        COMBO_OPTIONS.iter().any(|(id, _)| *id == COMBO_SELECTED),
        "COMBO_SELECTED must be one of COMBO_OPTIONS"
    );
    assert!(
        RADIO_OPTIONS.iter().any(|(id, _)| *id == RADIO_SELECTED),
        "RADIO_SELECTED must be one of RADIO_OPTIONS"
    );
    assert!(
        SEGMENT_ITEMS.iter().any(|(id, _)| *id == SEGMENT_SELECTED),
        "SEGMENT_SELECTED must be one of SEGMENT_ITEMS"
    );
}

#[test]
fn checkbox_rows_are_distinct_and_well_formed() {
    let mut labels: Vec<&str> = CHECKBOX_ROWS.iter().map(|row| row.0).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(labels.len(), before, "checkbox labels must be unique");
    assert_eq!(
        CHECKBOX_ROWS.len(),
        4,
        "off / on / indeterminate / disabled"
    );
}

#[test]
fn split_fixture_has_a_primary_and_menu_rows() {
    assert!(!SPLIT_PRIMARY_LABEL.is_empty());
    assert!(!SPLIT_MENU_ROWS.is_empty());
}

#[test]
fn sample_values_are_non_empty_display_text() {
    for sample in [
        TEXT_SAMPLE,
        TEXT_READONLY_SAMPLE,
        TEXT_INVALID_SAMPLE,
        PASSWORD_SAMPLE,
        SEARCH_SAMPLE,
        COMBO_PLACEHOLDER,
        COMBO_ERROR,
        TEXT_ERROR,
    ] {
        assert!(!sample.is_empty(), "fixtures must carry visible text");
    }
}

#[test]
fn every_fixture_date_is_a_real_calendar_day() {
    for ymd in [TODAY, DATE_SELECTED, DATE_RANGE.0, DATE_RANGE.1] {
        assert!(
            CivilDate::new(ymd.0, ymd.1, ymd.2).is_some(),
            "{ymd:?} must be a real date"
        );
    }
    for ymd in DATE_BLACKOUTS {
        assert!(CivilDate::new(ymd.0, ymd.1, ymd.2).is_some(), "{ymd:?}");
    }
}

#[test]
fn the_date_range_fixture_starts_before_it_ends() {
    let start = date(DATE_RANGE.0).format_iso();
    let end = date(DATE_RANGE.1).format_iso();
    assert!(start < end, "{start} must precede {end}");
}

#[test]
fn the_blackout_dates_fall_inside_the_fixture_month() {
    for ymd in DATE_BLACKOUTS {
        assert_eq!(ymd.0, TODAY.0);
        assert_eq!(ymd.1, TODAY.1);
    }
}

#[test]
fn every_fixture_time_is_a_real_time_of_day() {
    assert!(CivilTime::new(TIME_SELECTED.0, TIME_SELECTED.1, TIME_SELECTED.2).is_some());
}

#[test]
fn the_range_fixture_is_ordered_and_inside_its_bounds() {
    let (min, max, lower, upper) = RANGE_BOUNDS;
    assert!(min < max, "range bounds must be increasing");
    assert!(lower < upper, "thumbs must be ordered");
    assert!(
        lower >= min && upper <= max,
        "thumbs must sit inside the track"
    );
    const { assert!(RANGE_STEP > 0.0) };
}

#[test]
fn the_rating_fixture_is_inside_its_maximum() {
    assert!(RATING_VALUE <= RATING_MAX as f32);
}

#[test]
fn the_avatar_roster_has_unique_names() {
    let mut names: Vec<&str> = AVATAR_ROSTER.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "avatar display names must be unique");
}

#[test]
fn menu_row_keys_are_unique() {
    let mut keys: Vec<&str> = MENU_ROWS.iter().map(|(key, _, _)| *key).collect();
    keys.sort_unstable();
    let before = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), before, "menu row keys must be unique");
}

#[test]
fn every_menu_row_carries_a_declared_key() {
    // The publish sites build `<prefix><menu_row_key(i)>`, so a `None` or
    // an empty string here would surface as an id with an empty final
    // segment: the malformed `demo.composites.menu.` a keyless separator
    // used to produce. This is the guard that keeps that unreachable.
    let menu = menu_model();
    assert_eq!(
        menu.rows().len(),
        MENU_ROWS.len(),
        "the model must have one row per declared fixture entry"
    );

    for index in 0..menu.rows().len() {
        let key = menu_row_key(index).unwrap_or_else(|| {
            panic!("menu row {index} has no declared key; publishing it would name nothing")
        });
        assert!(
            !key.is_empty(),
            "menu row {index} declares an empty key; publishing it would leave the \
             id's final segment empty"
        );
    }
}

#[test]
fn separators_carry_their_declared_keys_through_the_model() {
    // `Row::separator()` is built keyless, so `Row::key()` is empty for a
    // separator. The demo's declared key is what the published id uses.
    let menu = menu_model();
    for (index, key) in [(2usize, "m-sep-1"), (4usize, "m-sep-2")] {
        assert_eq!(
            menu_row_key(index),
            Some(key),
            "the separator at {index} must publish under {key}"
        );
        let row = menu
            .rows()
            .get(index)
            .expect("the separator index is inside the model");
        assert!(
            row.is_separator(),
            "the fixture entry at {index} must be the separator {key}"
        );
        assert_eq!(
            row.key(),
            "",
            "the framework's separator is keyless, so the declared key rather \
             than `Row::key()` is what carries the separator's identity"
        );
    }
}

#[test]
fn menu_row_key_is_none_outside_the_fixture() {
    assert_eq!(menu_row_key(MENU_ROWS.len()), None);
    assert_eq!(menu_row_key(usize::MAX), None);
}
