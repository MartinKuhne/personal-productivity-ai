//! Unit tests for TimePicker (§3.11, `UIF-ATOM-011`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iced::Element;

use super::*;
use crate::test_helpers::Harness;

#[test]
fn test_civil_time_validation_and_constructors() {
    // Valid times
    assert!(CivilTime::new(0, 0, 0).is_some());
    assert!(CivilTime::new(23, 59, 59).is_some());
    assert!(CivilTime::new(12, 30, 45).is_some());

    // Out of bounds
    assert!(CivilTime::new(24, 0, 0).is_none());
    assert!(CivilTime::new(0, 60, 0).is_none());
    assert!(CivilTime::new(0, 0, 60).is_none());

    // Aliases & shortcuts
    assert_eq!(CivilTime::from_hms(10, 20, 30), CivilTime::new(10, 20, 30));
    assert_eq!(CivilTime::from_hm(10, 20), CivilTime::new(10, 20, 0));
    assert_eq!(CivilTime::midnight(), CivilTime::new(0, 0, 0).unwrap());
    assert_eq!(CivilTime::noon(), CivilTime::new(12, 0, 0).unwrap());
}

#[test]
fn test_civil_time_12h_representation_and_am_pm() {
    // Midnight: 00:00:00 -> 12:00:00 AM
    let midnight = CivilTime::midnight();
    assert_eq!(midnight.hour_12(), 12);
    assert_eq!(midnight.am_pm(), AmPm::Am);

    // 01:30:00 -> 1:30:00 AM
    let t1 = CivilTime::new(1, 30, 0).unwrap();
    assert_eq!(t1.hour_12(), 1);
    assert_eq!(t1.am_pm(), AmPm::Am);

    // 11:59:59 -> 11:59:59 AM
    let t11 = CivilTime::new(11, 59, 59).unwrap();
    assert_eq!(t11.hour_12(), 11);
    assert_eq!(t11.am_pm(), AmPm::Am);

    // Noon: 12:00:00 -> 12:00:00 PM
    let noon = CivilTime::noon();
    assert_eq!(noon.hour_12(), 12);
    assert_eq!(noon.am_pm(), AmPm::Pm);

    // 13:15:00 -> 1:15:00 PM
    let t13 = CivilTime::new(13, 15, 0).unwrap();
    assert_eq!(t13.hour_12(), 1);
    assert_eq!(t13.am_pm(), AmPm::Pm);

    // 23:45:00 -> 11:45:00 PM
    let t23 = CivilTime::new(23, 45, 0).unwrap();
    assert_eq!(t23.hour_12(), 11);
    assert_eq!(t23.am_pm(), AmPm::Pm);

    // from_12h constructor
    assert_eq!(
        CivilTime::from_12h(12, 0, 0, AmPm::Am),
        Some(CivilTime::midnight())
    );
    assert_eq!(
        CivilTime::from_12h(12, 0, 0, AmPm::Pm),
        Some(CivilTime::noon())
    );
    assert_eq!(
        CivilTime::from_12h(1, 30, 0, AmPm::Am),
        Some(CivilTime::new(1, 30, 0).unwrap())
    );
    assert_eq!(
        CivilTime::from_12h(1, 30, 0, AmPm::Pm),
        Some(CivilTime::new(13, 30, 0).unwrap())
    );
    assert_eq!(
        CivilTime::from_12h(11, 45, 0, AmPm::Pm),
        Some(CivilTime::new(23, 45, 0).unwrap())
    );

    // from_12h bounds checks
    assert!(CivilTime::from_12h(0, 0, 0, AmPm::Am).is_none());
    assert!(CivilTime::from_12h(13, 0, 0, AmPm::Am).is_none());
    assert!(CivilTime::from_12h(5, 60, 0, AmPm::Am).is_none());
    assert!(CivilTime::from_12h(5, 0, 60, AmPm::Am).is_none());
}

#[test]
fn test_civil_time_arithmetic_and_stepping() {
    let t = CivilTime::new(23, 55, 59).unwrap();

    // Hour stepping with wrap-around
    assert_eq!(t.step_hour(1), CivilTime::new(0, 55, 59).unwrap());
    assert_eq!(t.step_hour(2), CivilTime::new(1, 55, 59).unwrap());
    assert_eq!(
        CivilTime::midnight().step_hour(-1),
        CivilTime::new(23, 0, 0).unwrap()
    );

    // Minute stepping with increment
    assert_eq!(t.step_minute(1, 5), CivilTime::new(23, 0, 59).unwrap());
    assert_eq!(t.step_minute(2, 5), CivilTime::new(23, 5, 59).unwrap());
    assert_eq!(
        CivilTime::midnight().step_minute(-1, 5),
        CivilTime::new(0, 55, 0).unwrap()
    );
    assert_eq!(
        CivilTime::midnight().step_minute(-1, 15),
        CivilTime::new(0, 45, 0).unwrap()
    );

    // Second stepping
    assert_eq!(t.step_second(1), CivilTime::new(23, 55, 0).unwrap());
    assert_eq!(
        CivilTime::midnight().step_second(-1),
        CivilTime::new(0, 0, 59).unwrap()
    );

    // Toggle AM/PM
    assert_eq!(
        CivilTime::new(8, 30, 0).unwrap().toggle_am_pm(),
        CivilTime::new(20, 30, 0).unwrap()
    );
    assert_eq!(
        CivilTime::new(20, 30, 0).unwrap().toggle_am_pm(),
        CivilTime::new(8, 30, 0).unwrap()
    );

    // Step segment
    let base = CivilTime::new(10, 20, 30).unwrap();
    assert_eq!(
        base.step_segment(TimeSegment::Hour, 1, 1),
        CivilTime::new(11, 20, 30).unwrap()
    );
    assert_eq!(
        base.step_segment(TimeSegment::Minute, 1, 5),
        CivilTime::new(10, 25, 30).unwrap()
    );
    assert_eq!(
        base.step_segment(TimeSegment::Second, 1, 1),
        CivilTime::new(10, 20, 31).unwrap()
    );
    assert_eq!(
        base.step_segment(TimeSegment::AmPm, 1, 1),
        CivilTime::new(22, 20, 30).unwrap()
    );
}

#[test]
fn test_civil_time_formatting_and_parsing() {
    let t = CivilTime::new(14, 5, 9).unwrap();

    // 24h formatting
    assert_eq!(t.to_string_24h(false), "14:05");
    assert_eq!(t.to_string_24h(true), "14:05:09");

    // 12h formatting
    assert_eq!(t.to_string_12h(false), "02:05 PM");
    assert_eq!(t.to_string_12h(true), "02:05:09 PM");

    // Display implementation
    assert_eq!(format!("{t}"), "14:05");

    // Parsing
    assert_eq!(
        CivilTime::parse("14:05"),
        Some(CivilTime::new(14, 5, 0).unwrap())
    );
    assert_eq!(
        CivilTime::parse("14:05:09"),
        Some(CivilTime::new(14, 5, 9).unwrap())
    );
    assert_eq!(
        CivilTime::parse("2:05 PM"),
        Some(CivilTime::new(14, 5, 0).unwrap())
    );
    assert_eq!(
        CivilTime::parse("02:05:09 PM"),
        Some(CivilTime::new(14, 5, 9).unwrap())
    );
    assert_eq!(CivilTime::parse("12:00 AM"), Some(CivilTime::midnight()));
    assert_eq!(CivilTime::parse("12:00 PM"), Some(CivilTime::noon()));

    // Invalid parses
    assert_eq!(CivilTime::parse(""), None);
    assert_eq!(CivilTime::parse("invalid"), None);
    assert_eq!(CivilTime::parse("25:00"), None);
    assert_eq!(CivilTime::parse("12:65"), None);
}

#[test]
fn test_time_segment_navigation_cycles() {
    // 24-hour mode without seconds: Hour <-> Minute
    assert_eq!(TimeSegment::Hour.next(true, false), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.next(true, false), TimeSegment::Hour);
    assert_eq!(TimeSegment::Hour.prev(true, false), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.prev(true, false), TimeSegment::Hour);

    // 24-hour mode with seconds: Hour -> Minute -> Second -> Hour
    assert_eq!(TimeSegment::Hour.next(true, true), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.next(true, true), TimeSegment::Second);
    assert_eq!(TimeSegment::Second.next(true, true), TimeSegment::Hour);

    // 12-hour mode without seconds: Hour -> Minute -> AmPm -> Hour
    assert_eq!(TimeSegment::Hour.next(false, false), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.next(false, false), TimeSegment::AmPm);
    assert_eq!(TimeSegment::AmPm.next(false, false), TimeSegment::Hour);

    // 12-hour mode with seconds: Hour -> Minute -> Second -> AmPm -> Hour
    assert_eq!(TimeSegment::Hour.next(false, true), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.next(false, true), TimeSegment::Second);
    assert_eq!(TimeSegment::Second.next(false, true), TimeSegment::AmPm);
    assert_eq!(TimeSegment::AmPm.next(false, true), TimeSegment::Hour);
}

#[test]
fn test_time_picker_model_builder_and_state_transitions() {
    let t = CivilTime::new(9, 30, 0).unwrap();
    let model = TimePickerModel::new(Some(t))
        .with_24_hour(true)
        .with_show_seconds(true)
        .with_minute_increment(15)
        .with_open(false)
        .with_validation("Invalid time")
        .with_disabled(false);

    assert_eq!(model.value, Some(t));
    assert!(model.use_24_hour);
    assert!(model.show_seconds);
    assert_eq!(model.minute_increment, 15);
    assert!(!model.open);
    assert_eq!(model.validation.as_deref(), Some("Invalid time"));
    assert!(!model.disabled);

    // Format display
    assert_eq!(model.format_display(), "09:30:00");

    // Placeholder when None
    let empty_24 = TimePickerModel::new(None).with_24_hour(true);
    assert_eq!(empty_24.format_display(), PLACEHOLDER_24H);

    let empty_12 = TimePickerModel::new(None).with_24_hour(false);
    assert_eq!(empty_12.format_display(), PLACEHOLDER_12H);

    // Minute increment fallback
    let bad_inc = TimePickerModel::new(None).with_minute_increment(7);
    assert_eq!(bad_inc.minute_increment, 1);
}

#[test]
fn test_time_picker_model_snapshot_rollback_commit() {
    let initial = CivilTime::new(10, 0, 0).unwrap();
    let mut model = TimePickerModel::new(Some(initial));

    // Open flyout: snapshot captured
    model.open_flyout();
    assert!(model.open);
    assert_eq!(model.pre_open_snapshot, Some(Some(initial)));

    // User edits live value
    let modified = CivilTime::new(11, 30, 0).unwrap();
    model.select_time(modified);
    assert_eq!(model.value, Some(modified));

    // Cancel: rollback restores initial value
    let restored = model.cancel();
    assert_eq!(restored, Some(initial));
    assert_eq!(model.value, Some(initial));
    assert!(!model.open);
    assert_eq!(model.pre_open_snapshot, None);

    // Open again, edit, then commit
    model.open_flyout();
    model.select_time(modified);
    model.commit();
    assert_eq!(model.value, Some(modified));
    assert!(!model.open);
    assert_eq!(model.pre_open_snapshot, None);
}

#[test]
fn test_active_columns_composition() {
    assert_eq!(
        active_columns(true, false),
        vec![DrumColumnKind::Hour, DrumColumnKind::Minute]
    );
    assert_eq!(
        active_columns(true, true),
        vec![
            DrumColumnKind::Hour,
            DrumColumnKind::Minute,
            DrumColumnKind::Second
        ]
    );
    assert_eq!(
        active_columns(false, false),
        vec![
            DrumColumnKind::Hour,
            DrumColumnKind::Minute,
            DrumColumnKind::AmPm
        ]
    );
    assert_eq!(
        active_columns(false, true),
        vec![
            DrumColumnKind::Hour,
            DrumColumnKind::Minute,
            DrumColumnKind::Second,
            DrumColumnKind::AmPm
        ]
    );
}

#[test]
fn test_headless_render_time_picker_across_states() {
    let sample = CivilTime::new(14, 30, 45).unwrap();

    // 1. Closed state (12-hour)
    let model_closed = TimePickerModel::new(Some(sample));
    let mut harness1 =
        Harness::<(), _>::headless(move || time_picker::<()>(model_closed.clone()).into());
    harness1.render();

    // 2. Open state (24-hour mode)
    let model_open_24 = TimePickerModel::new(Some(sample))
        .with_24_hour(true)
        .with_open(true);
    let mut harness2 =
        Harness::<(), _>::headless(move || time_picker::<()>(model_open_24.clone()).into());
    harness2.render();

    // 3. Open state (12-hour mode with seconds and minute increment)
    let model_open_12 = TimePickerModel::new(Some(sample))
        .with_24_hour(false)
        .with_show_seconds(true)
        .with_minute_increment(5)
        .with_open(true);
    let mut harness3 =
        Harness::<(), _>::headless(move || time_picker::<()>(model_open_12.clone()).into());
    harness3.render();

    // 4. Invalid validation state
    let model_invalid =
        TimePickerModel::new(Some(sample)).with_validation("Time must be in the future");
    let mut harness4 =
        Harness::<(), _>::headless(move || time_picker::<()>(model_invalid.clone()).into());
    harness4.render();

    // 5. Disabled state
    let model_disabled = TimePickerModel::new(Some(sample)).with_disabled(true);
    let mut harness5 =
        Harness::<(), _>::headless(move || time_picker::<()>(model_disabled.clone()).into());
    harness5.render();
}

#[test]
fn test_time_picker_callbacks_registration() {
    let sample = CivilTime::new(9, 15, 0).unwrap();
    let open_flag = Arc::new(AtomicBool::new(false));
    let open_target = open_flag.clone();

    let last_change = Arc::new(Mutex::new(None));
    let change_store = last_change.clone();

    let last_commit = Arc::new(Mutex::new(None));
    let commit_store = last_commit.clone();

    let last_cancel = Arc::new(Mutex::new(None));
    let cancel_store = last_cancel.clone();

    let picker = time_picker::<()>(TimePickerModel::new(Some(sample)))
        .on_open_change(move |_live| {
            open_target.store(true, Ordering::SeqCst);
        })
        .on_change(move |live| {
            *change_store.lock().unwrap() = Some(live.into_value());
        })
        .on_commit(move |commit| {
            *commit_store.lock().unwrap() = Some(commit.into_value());
        })
        .on_cancel(move |cancel| {
            *cancel_store.lock().unwrap() = Some(cancel.into_restored());
        });

    assert_eq!(picker.model().value, Some(sample));
}

#[test]
fn test_time_picker_element_conversion() {
    let sample = CivilTime::new(12, 0, 0).unwrap();
    let picker = time_picker::<()>(TimePickerModel::new(Some(sample)));
    let _element: Element<'_, ()> = picker.into();
}
