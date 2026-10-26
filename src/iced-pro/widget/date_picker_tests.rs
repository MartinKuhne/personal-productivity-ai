//! Unit tests for DatePicker, DateRangePicker & CalendarView (§3.10, `UIF-ATOM-010`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iced::Element;

use super::*;
use crate::test_helpers::Harness;

#[test]
fn test_civil_date_validation_and_leap_years() {
    // Valid dates
    assert!(CivilDate::new(2024, 2, 29).is_some());
    assert!(CivilDate::new(2026, 10, 26).is_some());
    assert!(CivilDate::new(2000, 1, 1).is_some());

    // Non-leap year Feb 29 is invalid
    assert!(CivilDate::new(2023, 2, 29).is_none());
    assert!(CivilDate::new(1900, 2, 29).is_none());

    // Month out of bounds
    assert!(CivilDate::new(2024, 0, 1).is_none());
    assert!(CivilDate::new(2024, 13, 1).is_none());

    // Day out of bounds
    assert!(CivilDate::new(2024, 1, 0).is_none());
    assert!(CivilDate::new(2024, 1, 32).is_none());
    assert!(CivilDate::new(2024, 4, 31).is_none()); // April has 30 days

    // Leap year rules: divisible by 4, not 100 unless divisible by 400
    assert!(CivilDate::is_leap_year(2000));
    assert!(CivilDate::is_leap_year(2024));
    assert!(CivilDate::is_leap_year(2028));
    assert!(!CivilDate::is_leap_year(1900));
    assert!(!CivilDate::is_leap_year(2021));
    assert!(!CivilDate::is_leap_year(2022));
    assert!(!CivilDate::is_leap_year(2023));
}

#[test]
fn test_civil_date_days_in_month() {
    assert_eq!(CivilDate::days_in_month(2024, 1), 31);
    assert_eq!(CivilDate::days_in_month(2024, 2), 29); // leap
    assert_eq!(CivilDate::days_in_month(2023, 2), 28); // non-leap
    assert_eq!(CivilDate::days_in_month(2024, 3), 31);
    assert_eq!(CivilDate::days_in_month(2024, 4), 30);
    assert_eq!(CivilDate::days_in_month(2024, 5), 31);
    assert_eq!(CivilDate::days_in_month(2024, 6), 30);
    assert_eq!(CivilDate::days_in_month(2024, 7), 31);
    assert_eq!(CivilDate::days_in_month(2024, 8), 31);
    assert_eq!(CivilDate::days_in_month(2024, 9), 30);
    assert_eq!(CivilDate::days_in_month(2024, 10), 31);
    assert_eq!(CivilDate::days_in_month(2024, 11), 30);
    assert_eq!(CivilDate::days_in_month(2024, 12), 31);
}

#[test]
fn test_civil_date_day_of_week_sakamoto() {
    // 2026-10-26 is Monday
    let d1 = CivilDate::new(2026, 10, 26).unwrap();
    assert_eq!(d1.day_of_week(), Weekday::Mon);

    // 2000-01-01 is Saturday
    let d2 = CivilDate::new(2000, 1, 1).unwrap();
    assert_eq!(d2.day_of_week(), Weekday::Sat);

    // 1970-01-01 is Thursday
    let d3 = CivilDate::new(1970, 1, 1).unwrap();
    assert_eq!(d3.day_of_week(), Weekday::Thu);

    // 2024-02-29 is Thursday
    let d4 = CivilDate::new(2024, 2, 29).unwrap();
    assert_eq!(d4.day_of_week(), Weekday::Thu);

    // 2023-12-25 is Monday
    let d5 = CivilDate::new(2023, 12, 25).unwrap();
    assert_eq!(d5.day_of_week(), Weekday::Mon);

    // 2023-12-31 is Sunday
    let d6 = CivilDate::new(2023, 12, 31).unwrap();
    assert_eq!(d6.day_of_week(), Weekday::Sun);
}

#[test]
fn test_civil_date_arithmetic_next_prev_add() {
    let d = CivilDate::new(2024, 2, 28).unwrap();
    assert_eq!(d.next_day(), CivilDate::new(2024, 2, 29).unwrap());
    assert_eq!(d.next_day().next_day(), CivilDate::new(2024, 3, 1).unwrap());

    let year_end = CivilDate::new(2023, 12, 31).unwrap();
    assert_eq!(year_end.next_day(), CivilDate::new(2024, 1, 1).unwrap());
    assert_eq!(year_end.next_day().prev_day(), year_end);

    let leap_mar1 = CivilDate::new(2024, 3, 1).unwrap();
    assert_eq!(leap_mar1.prev_day(), CivilDate::new(2024, 2, 29).unwrap());

    // add_days O(1)
    let d = CivilDate::new(2026, 10, 26).unwrap();
    assert_eq!(d.add_days(0), d);
    assert_eq!(d.add_days(1), d.next_day());
    assert_eq!(d.add_days(-1), d.prev_day());
    assert_eq!(d.add_days(7), CivilDate::new(2026, 11, 2).unwrap());
    assert_eq!(d.add_days(-26), CivilDate::new(2026, 9, 30).unwrap());

    // Round-trip large day addition
    assert_eq!(d.add_days(1000).add_days(-1000), d);
}

#[test]
fn test_civil_date_iso_format_and_parse() {
    let d = CivilDate::new(2026, 10, 26).unwrap();
    assert_eq!(d.format_iso(), "2026-10-26");
    assert_eq!(CivilDate::parse_iso("2026-10-26"), Some(d));
    assert_eq!(CivilDate::parse_iso("  2026-10-26  "), Some(d));

    // Invalid ISO strings
    assert_eq!(CivilDate::parse_iso(""), None);
    assert_eq!(CivilDate::parse_iso("2026-10"), None);
    assert_eq!(CivilDate::parse_iso("invalid"), None);
    assert_eq!(CivilDate::parse_iso("2023-02-29"), None); // Non-leap year
    assert_eq!(CivilDate::parse_iso("2024-13-01"), None); // Invalid month
}

#[test]
fn test_year_month_and_weekday_helpers() {
    let ym = YearMonth::new(2026, 10).unwrap();
    assert_eq!(ym.format_month_year(), "October 2026");
    assert_eq!(ym.days_in_month(), 31);
    assert_eq!(ym.first_date(), CivilDate::new(2026, 10, 1).unwrap());

    let next_ym = ym.next_month();
    assert_eq!(next_ym, YearMonth::new(2026, 11).unwrap());
    let prev_ym = ym.prev_month();
    assert_eq!(prev_ym, YearMonth::new(2026, 9).unwrap());

    let dec = YearMonth::new(2023, 12).unwrap();
    assert_eq!(dec.next_month(), YearMonth::new(2024, 1).unwrap());
    let jan = YearMonth::new(2024, 1).unwrap();
    assert_eq!(jan.prev_month(), YearMonth::new(2023, 12).unwrap());

    // Weekdays
    assert_eq!(Weekday::Mon.short_name(), "Mo");
    assert_eq!(Weekday::Sun.short_name(), "Su");
    assert_eq!(Weekday::Mon.days_until(Weekday::Mon), 0);
    assert_eq!(Weekday::Mon.days_until(Weekday::Thu), 3);
    assert_eq!(Weekday::Sun.days_until(Weekday::Mon), 1);

    let order = Weekday::Mon.week_order();
    assert_eq!(
        order,
        [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun
        ]
    );
}

#[test]
fn test_fixed_6x7_day_grid_generator_invariance_uif_prin_003() {
    // October 2026: 31 days, starts on Thursday
    let ym_oct = YearMonth::new(2026, 10).unwrap();
    let cells_oct = displayed_month_cells(ym_oct, Weekday::Mon);
    assert_eq!(cells_oct.len(), 42); // Always exactly 42 cells

    // 2026-10-01 is Thursday. With Mon start, leading days = 3 (Sep 28, 29, 30)
    assert_eq!(cells_oct[0].date, CivilDate::new(2026, 9, 28).unwrap());
    assert!(!cells_oct[0].is_current_month);
    assert_eq!(cells_oct[2].date, CivilDate::new(2026, 9, 30).unwrap());
    assert!(!cells_oct[2].is_current_month);
    assert_eq!(cells_oct[3].date, CivilDate::new(2026, 10, 1).unwrap());
    assert!(cells_oct[3].is_current_month);

    // Exactly 31 current-month cells
    let oct_count = cells_oct.iter().filter(|c| c.is_current_month).count();
    assert_eq!(oct_count, 31);

    // February 2021: 28 days, non-leap year, starts on Monday
    let ym_feb21 = YearMonth::new(2021, 2).unwrap();
    let cells_feb21 = displayed_month_cells(ym_feb21, Weekday::Mon);
    assert_eq!(cells_feb21.len(), 42);
    assert_eq!(cells_feb21[0].date, CivilDate::new(2021, 2, 1).unwrap());
    assert!(cells_feb21[0].is_current_month);
    let feb21_count = cells_feb21.iter().filter(|c| c.is_current_month).count();
    assert_eq!(feb21_count, 28);
    // Trailing days reach into March
    assert_eq!(cells_feb21[28].date, CivilDate::new(2021, 3, 1).unwrap());
    assert!(!cells_feb21[28].is_current_month);

    // February 2024: 29 days, leap year
    let ym_feb24 = YearMonth::new(2024, 2).unwrap();
    let cells_feb24 = displayed_month_cells(ym_feb24, Weekday::Mon);
    assert_eq!(cells_feb24.len(), 42);
    let feb24_count = cells_feb24.iter().filter(|c| c.is_current_month).count();
    assert_eq!(feb24_count, 29);

    // All months produce 42 cells with zero height layout shift
    assert_eq!(cells_oct.len(), cells_feb21.len());
    assert_eq!(cells_oct.len(), cells_feb24.len());
}

#[test]
fn test_date_selection_modes() {
    let d1 = CivilDate::new(2026, 10, 10).unwrap();
    let d2 = CivilDate::new(2026, 10, 20).unwrap();
    let d_inside = CivilDate::new(2026, 10, 15).unwrap();
    let d_outside = CivilDate::new(2026, 10, 25).unwrap();

    // Single Date
    let sel_single = DateSelection::SingleDate(Some(d1));
    assert!(sel_single.is_selected_endpoint(d1));
    assert!(!sel_single.is_selected_endpoint(d2));
    assert!(!sel_single.is_range_interior(d_inside, None));
    assert_eq!(sel_single.format_display(), "2026-10-10");

    // Date Range Settled
    let sel_range = DateSelection::DateRange {
        start: Some(d1),
        end: Some(d2),
    };
    assert!(sel_range.is_selected_endpoint(d1));
    assert!(sel_range.is_selected_endpoint(d2));
    assert!(sel_range.is_range_interior(d_inside, None));
    assert!(!sel_range.is_range_interior(d_outside, None));
    assert_eq!(sel_range.format_display(), "2026-10-10 - 2026-10-20");

    // Date Range with Hover Preview
    let sel_range_open = DateSelection::DateRange {
        start: Some(d1),
        end: None,
    };
    assert!(sel_range_open.is_selected_endpoint(d1));
    assert!(sel_range_open.is_range_interior(d_inside, Some(d2)));
    assert!(sel_range_open.is_range_preview_endpoint(d2, Some(d2)));
}

#[test]
fn test_date_picker_model_blackout_and_bounds() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let blackout = CivilDate::new(2026, 10, 28).unwrap();
    let min = CivilDate::new(2026, 10, 5).unwrap();
    let max = CivilDate::new(2026, 10, 29).unwrap();

    let model = DatePickerModel::new(today)
        .with_min_date(min)
        .with_max_date(max)
        .with_blackout_dates([blackout]);

    assert!(!model.is_date_disabled(today));
    assert!(model.is_date_disabled(blackout)); // in blackout set
    assert!(model.is_date_disabled(CivilDate::new(2026, 10, 1).unwrap())); // before min
    assert!(model.is_date_disabled(CivilDate::new(2026, 10, 31).unwrap())); // after max

    // Disabled model disables all dates
    let disabled_model = model.clone().with_disabled(true);
    assert!(disabled_model.is_date_disabled(today));
}

#[test]
fn test_date_picker_model_select_date() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut model = DatePickerModel::new(today);

    // Single date selection
    model.select_date(today);
    assert_eq!(model.selection, DateSelection::SingleDate(Some(today)));

    // Range selection: click 1 sets start
    let mut range_model = DatePickerModel::new(today).with_selection(DateSelection::DateRange {
        start: None,
        end: None,
    });
    let d1 = CivilDate::new(2026, 10, 10).unwrap();
    range_model.select_date(d1);
    assert_eq!(
        range_model.selection,
        DateSelection::DateRange {
            start: Some(d1),
            end: None,
        }
    );

    // Range selection: click 2 sets end
    let d2 = CivilDate::new(2026, 10, 20).unwrap();
    range_model.select_date(d2);
    assert_eq!(
        range_model.selection,
        DateSelection::DateRange {
            start: Some(d1),
            end: Some(d2),
        }
    );

    // Inverted range click auto-sorts
    let mut inverted_model = DatePickerModel::new(today).with_selection(DateSelection::DateRange {
        start: Some(d2),
        end: None,
    });
    inverted_model.select_date(d1);
    assert_eq!(
        inverted_model.selection,
        DateSelection::DateRange {
            start: Some(d1),
            end: Some(d2),
        }
    );
}

#[test]
fn test_headless_render_date_picker_across_states() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let blackout = CivilDate::new(2026, 10, 28).unwrap();

    // 1. Closed state
    let model_closed = DatePickerModel::new(today);
    let mut harness1 =
        Harness::<(), _>::headless(move || date_picker::<()>(model_closed.clone()).into());
    harness1.render();

    // 2. Open state in Days view mode with today and blackout cue
    let model_days = DatePickerModel::new(today)
        .with_open(true)
        .with_view_mode(CalendarViewMode::Days)
        .with_blackout_dates([blackout]);
    let mut harness2 = Harness::<(), _>::headless(move || {
        date_picker::<()>(model_days.clone())
            .with_today(today)
            .into()
    });
    harness2.render();

    // 3. Open state in Months view mode
    let model_months = DatePickerModel::new(today)
        .with_open(true)
        .with_view_mode(CalendarViewMode::Months);
    let mut harness3 =
        Harness::<(), _>::headless(move || date_picker::<()>(model_months.clone()).into());
    harness3.render();

    // 4. Open state in Years view mode
    let model_years = DatePickerModel::new(today)
        .with_open(true)
        .with_view_mode(CalendarViewMode::Years);
    let mut harness4 =
        Harness::<(), _>::headless(move || date_picker::<()>(model_years.clone()).into());
    harness4.render();

    // 5. Open state with date range selection and preview
    let d_start = CivilDate::new(2026, 10, 10).unwrap();
    let d_end = CivilDate::new(2026, 10, 20).unwrap();
    let model_range =
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: Some(d_start),
                end: Some(d_end),
            });
    let mut harness5 =
        Harness::<(), _>::headless(move || date_range_picker::<()>(model_range.clone()).into());
    harness5.render();

    // 6. Disabled state
    let model_disabled = DatePickerModel::new(today).with_disabled(true);
    let mut harness6 =
        Harness::<(), _>::headless(move || date_picker::<()>(model_disabled.clone()).into());
    harness6.render();
}

#[test]
fn test_date_picker_from_for_element() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let picker = date_picker::<()>(DatePickerModel::new(today));
    let _element: Element<'_, ()> = picker.into();

    let cal = calendar_view::<()>(DatePickerModel::new(today));
    let _element_cal: Element<'_, ()> = cal.into();
}

#[test]
fn test_date_picker_callbacks_and_interaction() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let open_called = Arc::new(AtomicBool::new(false));
    let open_flag = open_called.clone();

    let last_change = Arc::new(Mutex::new(None));
    let change_store = last_change.clone();

    let last_commit = Arc::new(Mutex::new(None));
    let commit_store = last_commit.clone();

    let last_cancel = Arc::new(Mutex::new(None));
    let cancel_store = last_cancel.clone();

    let picker = date_picker::<()>(DatePickerModel::new(today))
        .on_open_change(move |_live| {
            open_flag.store(true, Ordering::SeqCst);
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

    assert_eq!(picker.model().displayed_month.year, 2026);
    assert_eq!(picker.model().displayed_month.month, 10);
}
