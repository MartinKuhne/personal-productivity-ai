//! Unit tests for DatePicker, DateRangePicker & CalendarView (§3.10, `UIF-ATOM-010`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iced::Element;

use super::*;
use crate::test_helpers::{Harness, headless_renderer, moved, pressed};

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
    // (year, month, expected days) — February exercises both leap and common years.
    const CASES: &[(i32, u8, u8)] = &[
        (2024, 1, 31),
        (2024, 2, 29), // leap
        (2023, 2, 28), // non-leap
        (2024, 3, 31),
        (2024, 4, 30),
        (2024, 5, 31),
        (2024, 6, 30),
        (2024, 7, 31),
        (2024, 8, 31),
        (2024, 9, 30),
        (2024, 10, 31),
        (2024, 11, 30),
        (2024, 12, 31),
    ];

    for (year, month, want) in CASES {
        assert_eq!(
            CivilDate::days_in_month(*year, *month),
            *want,
            "{year}-{month:02} must have {want} days"
        );
    }
}

#[test]
fn test_civil_date_day_of_week_sakamoto() {
    // (year, month, day, expected weekday)
    const CASES: &[(i32, u8, u8, Weekday)] = &[
        (2026, 10, 26, Weekday::Mon),
        (2000, 1, 1, Weekday::Sat),
        (1970, 1, 1, Weekday::Thu),
        (2024, 2, 29, Weekday::Thu),
        (2023, 12, 25, Weekday::Mon),
        (2023, 12, 31, Weekday::Sun),
    ];

    for (year, month, day, want) in CASES {
        let date = CivilDate::new(*year, *month, *day)
            .unwrap_or_else(|| panic!("{year}-{month:02}-{day:02} must be a valid date"));
        assert_eq!(
            date.day_of_week(),
            *want,
            "{year}-{month:02}-{day:02} must fall on {want:?}"
        );
    }
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

// ---------------------------------------------------------------------------
// Direct `Widget::update` driver
//
// The headless `Harness` proves the paint path runs; it cannot read the
// widget's own `DatePickerState` back out. These tests call `Widget::update`
// directly on a laid-out widget so hover, focus and keyboard state are
// observable, not just the messages the callbacks publish.
// ---------------------------------------------------------------------------

/// Message vocabulary produced by the callbacks wired up by [`wired`].
#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    /// Flyout open state changed.
    Opened(bool),
    /// Drill-down view mode changed.
    Mode(CalendarViewMode),
    /// The displayed month was navigated to.
    Navigated(YearMonth),
    /// Live selection change.
    Changed(DateSelection),
    /// Selection committed.
    Committed(DateSelection),
    /// Interaction cancelled with the restored selection.
    Cancelled(DateSelection),
}

/// A picker with every callback registered, so a test can assert on the full
/// Live / Commit / Cancel protocol rather than a subset of it.
fn wired(model: DatePickerModel) -> DatePicker<'static, TestMsg> {
    date_picker(model)
        .on_open_change(|live| TestMsg::Opened(live.into_value()))
        .on_view_mode_change(|live| TestMsg::Mode(live.into_value()))
        .on_month_navigate(|live| TestMsg::Navigated(live.into_value()))
        .on_change(|live| TestMsg::Changed(live.into_value()))
        .on_commit(|commit| TestMsg::Committed(commit.into_value()))
        .on_cancel(|cancel| TestMsg::Cancelled(cancel.into_restored()))
}

/// A `KeyPressed` event for a named key, matching what a real backend sends.
fn key_pressed(key: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(key),
        modified_key: Key::Named(key),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

/// Total height of a picker laid out with its flyout open.
fn open_height() -> f32 {
    TRIGGER_HEIGHT + space::XS + calendar_flyout_height()
}

/// Box of an open picker laid out at the viewport origin.
fn open_bounds() -> Rectangle {
    Rectangle::new(
        Point::ORIGIN,
        Size::new(CALENDAR_FLYOUT_WIDTH, open_height()),
    )
}

/// Box of a closed picker laid out at the viewport origin.
fn closed_bounds() -> Rectangle {
    Rectangle::new(
        Point::ORIGIN,
        Size::new(CALENDAR_FLYOUT_WIDTH, TRIGGER_HEIGHT),
    )
}

/// Top edge of the flyout card.
fn flyout_y() -> f32 {
    TRIGGER_HEIGHT + space::XS
}

/// Top edge of the header navigation row inside the flyout.
fn nav_y() -> f32 {
    flyout_y() + FLYOUT_PADDING
}

/// Centre of the trailing calendar toggle on the trigger box.
fn trailing_centre() -> Point {
    Point::new(
        CALENDAR_FLYOUT_WIDTH - TRAILING_BUTTON_SIZE - space::SM + TRAILING_BUTTON_SIZE / 2.0,
        (TRIGGER_HEIGHT - TRAILING_BUTTON_SIZE) / 2.0 + TRAILING_BUTTON_SIZE / 2.0,
    )
}

/// Centre of the `<` navigation button.
fn nav_prev_centre() -> Point {
    Point::new(
        FLYOUT_PADDING + NAV_BUTTON_SIZE / 2.0,
        nav_y() + NAV_BUTTON_SIZE / 2.0,
    )
}

/// Centre of the `>` navigation button.
fn nav_next_centre() -> Point {
    Point::new(
        CALENDAR_FLYOUT_WIDTH - FLYOUT_PADDING - NAV_BUTTON_SIZE / 2.0,
        nav_y() + NAV_BUTTON_SIZE / 2.0,
    )
}

/// Centre of the drill-down title button between the navigation chevrons.
fn title_centre() -> Point {
    let width = CALENDAR_FLYOUT_WIDTH - 2.0 * (FLYOUT_PADDING + NAV_BUTTON_SIZE);
    Point::new(
        FLYOUT_PADDING + NAV_BUTTON_SIZE + width / 2.0,
        nav_y() + NAV_BUTTON_SIZE / 2.0,
    )
}

/// Centre of the day-grid cell at `row` / `col`.
fn cell_centre(row: usize, col: usize) -> Point {
    let grid_x = (CALENDAR_FLYOUT_WIDTH - GRID_COLS as f32 * CELL_SIZE) / 2.0;
    let grid_y = nav_y() + NAV_BUTTON_SIZE + space::SM + WEEKDAY_ROW_HEIGHT + space::XS;
    Point::new(
        grid_x + col as f32 * CELL_SIZE + CELL_SIZE / 2.0,
        grid_y + row as f32 * CELL_SIZE + CELL_SIZE / 2.0,
    )
}

/// A point inside the flyout card that is on no control at all: the weekday
/// header row sits between the navigation row and the day grid.
fn dead_zone_centre() -> Point {
    let body_y = nav_y() + NAV_BUTTON_SIZE + space::SM;
    Point::new(
        CALENDAR_FLYOUT_WIDTH / 2.0,
        body_y + WEEKDAY_ROW_HEIGHT / 2.0,
    )
}

/// Centre of the `slot`-th (0-based) cell of the 4x3 Months / Years drill-down.
fn drill_centre(slot: u32) -> Point {
    let body_y = nav_y() + NAV_BUTTON_SIZE + space::SM;
    let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
    let row_h = (calendar_flyout_height() - (body_y - flyout_y()) - FLYOUT_PADDING) / 4.0;
    Point::new(
        FLYOUT_PADDING + (slot % 3) as f32 * col_w + col_w / 2.0,
        body_y + (slot / 3) as f32 * row_h + row_h / 2.0,
    )
}

/// A point inside a drill-down card that is on no month or year cell: the
/// four rows stop `FLYOUT_PADDING` above the bottom edge of the card, so that
/// strip belongs to no slot.
fn drill_dead_zone_centre() -> Point {
    Point::new(
        CALENDAR_FLYOUT_WIDTH / 2.0,
        flyout_y() + calendar_flyout_height() - FLYOUT_PADDING / 2.0,
    )
}

/// Row and column of the grid cell that displays `date` for `ym`.
fn cell_of(ym: YearMonth, date: CivilDate) -> (usize, usize) {
    let cells = displayed_month_cells(ym, Weekday::Mon);
    let idx = cells
        .iter()
        .position(|cell| cell.date == date)
        .unwrap_or_else(|| panic!("{date} must be rendered by the {ym} grid"));
    (idx / GRID_COLS, idx % GRID_COLS)
}

/// A [`DatePicker`] driven through `Widget::update` directly, keeping the
/// [`Tree`] so the widget's own state is readable after every event.
struct Driver<'a> {
    widget: DatePicker<'a, TestMsg>,
    tree: Tree,
    renderer: iced::Renderer,
    bounds: Rectangle,
    messages: Vec<TestMsg>,
}

impl<'a> Driver<'a> {
    /// Wrap `widget` laid out at [`open_bounds`].
    fn new(widget: DatePicker<'a, TestMsg>) -> Self {
        Self::in_box(widget, open_bounds())
    }

    /// Wrap `widget` laid out at an explicit box.
    fn in_box(widget: DatePicker<'a, TestMsg>, bounds: Rectangle) -> Self {
        let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
        Self {
            widget,
            tree,
            renderer: headless_renderer(),
            bounds,
            messages: Vec::new(),
        }
    }

    /// The interaction state the widget keeps in the tree.
    fn state(&self) -> &DatePickerState {
        self.tree.state.downcast_ref::<DatePickerState>()
    }

    /// Give the trigger keyboard focus, as the focus router would.
    fn set_focus(&mut self) {
        let state = self.tree.state.downcast_mut::<DatePickerState>();
        <FocusableState as operation::Focusable>::focus(&mut state.focusable);
    }

    /// Move the keyboard cursor to a day cell, as clicking it would.
    fn focus_date(&mut self, date: CivilDate) {
        let state = self.tree.state.downcast_mut::<DatePickerState>();
        state.focused_date = Some(date);
    }

    /// Write a published selection back into the widget's model.
    ///
    /// The widget is declarative: a day click publishes the new selection and
    /// keeps nothing for itself, so the next frame only sees it once the host
    /// has stored it. A test that wants two clicks to build one range has to
    /// play the host between them.
    fn host_selection(&mut self, selection: DateSelection) {
        self.widget.model.selection = selection;
    }

    /// Deliver one event with the cursor where the test says.
    fn deliver(&mut self, event: Event, cursor: mouse::Cursor) {
        let node = layout::Node::new(self.bounds.size());
        let layout = Layout::new(&node);
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut shell = Shell::new(&mut self.messages);
        let widget = &mut self.widget as &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
        widget.update(
            &mut self.tree,
            &event,
            layout,
            cursor,
            &self.renderer,
            &mut clipboard,
            &mut shell,
            &self.bounds,
        );
    }

    /// Move the cursor to `at` and let the widget see it there.
    fn hover(&mut self, at: Point) {
        self.deliver(moved(at), mouse::Cursor::Available(at));
    }

    /// Press the left button with the cursor on `at`.
    fn click(&mut self, at: Point) {
        self.deliver(pressed(), mouse::Cursor::Available(at));
    }

    /// Press a named key with no cursor over the widget.
    fn key(&mut self, key: Named) {
        self.deliver(key_pressed(key), mouse::Cursor::Unavailable);
    }

    /// Take every message published since the last drain.
    fn drain(&mut self) -> Vec<TestMsg> {
        std::mem::take(&mut self.messages)
    }
}

// ---------------------------------------------------------------------------
// Pure civil calendar arithmetic
// ---------------------------------------------------------------------------

#[test]
fn test_days_from_civil_and_civil_from_days_known_epoch_days() {
    // (year, month, day, days since 1970-01-01). The pre-1970 rows drive the
    // negative-era branches of both halves of Hinnant's algorithm.
    const CASES: &[(i32, u8, u8, i32)] = &[
        (1970, 1, 1, 0),
        (1969, 12, 31, -1),
        (1970, 1, 2, 1),
        (2000, 1, 1, 10957),
        (2024, 1, 1, 19723),
        (2024, 2, 29, 19782),
        (2026, 10, 26, 20752),
        (1, 1, 1, -719162),
        (0, 1, 1, -719528),
        (-1, 1, 1, -719893),
    ];

    for (year, month, day, epoch_day) in CASES {
        assert_eq!(
            days_from_civil(*year, *month, *day),
            *epoch_day,
            "days_from_civil({year}-{month:02}-{day:02})"
        );
        assert_eq!(
            civil_from_days(*epoch_day),
            (*year, *month, *day),
            "civil_from_days({epoch_day}) must invert"
        );
    }
}

#[test]
fn test_add_days_agrees_with_single_day_stepping() {
    // Anchors chosen so the walk crosses a month end, a year end and a leap
    // day, and so the pre-1970 rows exercise negative epoch days.
    const ANCHORS: &[(i32, u8, u8)] = &[
        (1969, 12, 30),
        (2000, 2, 27),
        (2024, 2, 27),
        (2026, 10, 29),
        (0, 2, 27),
        (-1, 12, 30),
    ];

    for (year, month, day) in ANCHORS {
        let start = CivilDate::new(*year, *month, *day)
            .unwrap_or_else(|| panic!("{year}-{month:02}-{day:02} must be valid"));

        assert_eq!(
            start.add_days(0),
            start,
            "add_days(0) is identity for {start}"
        );

        let mut forward = start;
        for step in 1..=5 {
            forward = forward.next_day();
            assert_eq!(
                start.add_days(step),
                forward,
                "add_days({step}) from {start} must match stepping one day at a time"
            );
        }

        let mut backward = start;
        for step in 1..=5 {
            backward = backward.prev_day();
            assert_eq!(
                start.add_days(-step),
                backward,
                "add_days(-{step}) from {start} must match stepping back one day at a time"
            );
        }

        assert_eq!(
            start.add_days(400).add_days(-400),
            start,
            "a 400-year round trip must return to {start}"
        );
    }
}

#[test]
fn test_civil_date_next_and_prev_day_rollovers() {
    // A calendar date as a plain (year, month, day) triple, for case tables.
    type Ymd = (i32, u8, u8);
    // One case: (start, next day, previous day).
    type DayStep = (Ymd, Ymd, Ymd);

    const CASES: &[DayStep] = &[
        // Ordinary mid-month step.
        ((2026, 10, 14), (2026, 10, 15), (2026, 10, 13)),
        // Month end into a 31-day month.
        ((2026, 4, 30), (2026, 5, 1), (2026, 4, 29)),
        // Month start back into a 30-day month.
        ((2026, 5, 1), (2026, 5, 2), (2026, 4, 30)),
        // Leap-day boundary both ways.
        ((2024, 2, 29), (2024, 3, 1), (2024, 2, 28)),
        ((2023, 2, 28), (2023, 3, 1), (2023, 2, 27)),
        // Year boundary both ways.
        ((2026, 12, 31), (2027, 1, 1), (2026, 12, 30)),
        ((2027, 1, 1), (2027, 1, 2), (2026, 12, 31)),
        // A common year's February is only 28 days long.
        ((2023, 2, 28), (2023, 3, 1), (2023, 2, 27)),
    ];

    for ((sy, sm, sd), (ny, nm, nd), (py, pm, pd)) in CASES {
        let start = CivilDate::new(*sy, *sm, *sd).expect("case start must be valid");
        assert_eq!(
            start.next_day(),
            CivilDate::new(*ny, *nm, *nd).expect("case next must be valid"),
            "next_day of {start}"
        );
        assert_eq!(
            start.prev_day(),
            CivilDate::new(*py, *pm, *pd).expect("case prev must be valid"),
            "prev_day of {start}"
        );
    }
}

#[test]
fn test_days_in_month_rejects_out_of_range_months() {
    for month in [0u8, 13, 200, u8::MAX] {
        assert_eq!(
            CivilDate::days_in_month(2024, month),
            0,
            "days_in_month must report 0 for month {month}"
        );
        assert!(
            CivilDate::new(2024, month, 1).is_none(),
            "month {month} must not build a date"
        );
    }
}

#[test]
fn test_format_iso_covers_negative_and_zero_years() {
    const CASES: &[(CivilDate, &str)] = &[
        (
            CivilDate {
                year: 2026,
                month: 10,
                day: 26,
            },
            "2026-10-26",
        ),
        (
            CivilDate {
                year: 7,
                month: 1,
                day: 1,
            },
            "0007-01-01",
        ),
        (
            CivilDate {
                year: 0,
                month: 1,
                day: 2,
            },
            "0000-01-02",
        ),
        (
            CivilDate {
                year: -1,
                month: 12,
                day: 31,
            },
            "-0001-12-31",
        ),
        (
            CivilDate {
                year: -44,
                month: 3,
                day: 15,
            },
            "-0044-03-15",
        ),
    ];

    for (date, want) in CASES {
        assert_eq!(date.format_iso(), *want, "format_iso of {date:?}");
        assert_eq!(
            date.to_string(),
            *want,
            "Display must mirror format_iso for {date:?}"
        );
    }
}

#[test]
fn test_parse_iso_accepts_the_shapes_it_claims_to() {
    const GOOD: &[(&str, i32, u8, u8)] = &[
        ("2026-10-26", 2026, 10, 26),
        ("  2026-10-26  ", 2026, 10, 26),
        ("2026-1-5", 2026, 1, 5),
        ("+2026-10-26", 2026, 10, 26),
        ("0000-01-02", 0, 1, 2),
        ("-0044-03-15", -44, 3, 15),
    ];

    for (text, year, month, day) in GOOD {
        assert_eq!(
            CivilDate::parse_iso(text),
            CivilDate::new(*year, *month, *day),
            "parse_iso({text:?}) must parse"
        );
    }
}

#[test]
fn test_parse_iso_rejects_malformed_and_out_of_range_input() {
    const BAD: &[&str] = &[
        "",
        " ",
        "-",
        "--2026-10-26",
        "2026-10",
        "2026-10-26-01",
        "2026--05",
        "2026-02-30",
        "2023-02-29",
        "2026-13-01",
        "2026-10-00",
        "2026-10-32",
        "99999999999999-01-01",
        "2026 - 10 - 26",
        "abcd-ef-gh",
    ];

    for text in BAD {
        assert_eq!(
            CivilDate::parse_iso(text),
            None,
            "parse_iso({text:?}) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// Weekday, month and YearMonth helpers
// ---------------------------------------------------------------------------

#[test]
fn test_weekday_short_names_and_display() {
    const CASES: &[(Weekday, &str)] = &[
        (Weekday::Mon, "Mo"),
        (Weekday::Tue, "Tu"),
        (Weekday::Wed, "We"),
        (Weekday::Thu, "Th"),
        (Weekday::Fri, "Fr"),
        (Weekday::Sat, "Sa"),
        (Weekday::Sun, "Su"),
    ];

    for (weekday, want) in CASES {
        assert_eq!(weekday.short_name(), *want, "short_name of {weekday:?}");
        assert_eq!(
            weekday.to_string(),
            *want,
            "Display must mirror short_name for {weekday:?}"
        );
    }

    assert_eq!(
        Weekday::default(),
        Weekday::Mon,
        "the week must default to a Monday start"
    );
}

#[test]
fn test_weekday_week_order_is_a_rotation_of_seven_days() {
    const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    for start in ALL {
        let order = start.week_order();
        assert_eq!(order[0], start, "the week order must start at {start:?}");

        for day in ALL {
            assert_eq!(
                order.iter().filter(|d| **d == day).count(),
                1,
                "{day:?} must appear exactly once in the {start:?} week order"
            );
        }

        for idx in 0..7 {
            assert_eq!(
                order[idx].days_until(order[(idx + 1) % 7]),
                1,
                "adjacent columns must be one day apart in the {start:?} order"
            );
        }
    }
}

#[test]
fn test_weekday_days_until_wraps_the_cycle() {
    const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    for a in ALL {
        assert_eq!(a.days_until(a), 0, "{a:?} is zero days from itself");
        assert_eq!(
            a.days_until(a.week_order()[1]),
            1,
            "the day after {a:?} must be one day away"
        );

        for b in ALL {
            if a != b {
                assert_eq!(
                    a.days_until(b) + b.days_until(a),
                    7,
                    "the cycle {a:?} -> {b:?} -> {a:?} must span a whole week"
                );
            }
        }
    }

    assert_eq!(Weekday::Mon.days_until(Weekday::Sun), 6);
    assert_eq!(Weekday::Sun.days_until(Weekday::Sat), 6);
    assert_eq!(Weekday::Fri.days_until(Weekday::Sat), 1);
}

#[test]
fn test_month_names_short_labels_and_header_format() {
    const FULL: [&str; 12] = [
        MONTH_JAN, MONTH_FEB, MONTH_MAR, MONTH_APR, MONTH_MAY, MONTH_JUN, MONTH_JUL, MONTH_AUG,
        MONTH_SEP, MONTH_OCT, MONTH_NOV, MONTH_DEC,
    ];

    for (idx, name) in FULL.iter().enumerate() {
        let month = idx as u8 + 1;
        assert_eq!(month_name(month), *name, "month_name({month})");
        assert_eq!(
            SHORT_MONTHS[idx],
            &name[..3],
            "SHORT_MONTHS[{idx}] must abbreviate {name}"
        );

        let ym = YearMonth::new(2026, month).expect("every 1..=12 month must build");
        assert_eq!(
            ym.format_month_year(),
            format!("{name} 2026"),
            "header label for {name}"
        );
        assert_eq!(
            ym.to_string(),
            ym.format_month_year(),
            "Display must mirror format_month_year for {name}"
        );
    }

    for month in [0u8, 13, u8::MAX] {
        assert_eq!(month_name(month), "", "month_name({month}) must be empty");
    }
}

#[test]
fn test_year_month_validation_and_year_rollover() {
    for month in [0u8, 13, 99] {
        assert!(
            YearMonth::new(2026, month).is_none(),
            "YearMonth must reject month {month}"
        );
    }

    assert_eq!(
        YearMonth::new(2026, 1).unwrap().prev_month(),
        YearMonth::new(2025, 12).unwrap()
    );
    assert_eq!(
        YearMonth::new(2026, 12).unwrap().next_month(),
        YearMonth::new(2027, 1).unwrap()
    );
    assert_eq!(YearMonth::new(2024, 2).unwrap().days_in_month(), 29);
    assert_eq!(YearMonth::new(2023, 2).unwrap().days_in_month(), 28);
    assert_eq!(
        YearMonth::new(-5, 11).unwrap().first_date(),
        CivilDate {
            year: -5,
            month: 11,
            day: 1,
        }
    );
}

#[test]
fn test_calendar_flyout_geometry_is_the_fixed_grid_sum() {
    assert_eq!(
        CALENDAR_FLYOUT_W, CALENDAR_FLYOUT_WIDTH,
        "the width alias must name the same 256px card"
    );
    assert_eq!(
        calendar_flyout_height(),
        284.0,
        "padding + header + gap + weekday row + gap + six 32px rows must total 284px"
    );
    assert_eq!(
        GRID_ROWS * GRID_COLS,
        42,
        "the grid is fixed at 42 cells so no month shifts the layout"
    );
}

#[test]
fn test_displayed_month_cells_for_every_week_start() {
    const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    let ym = YearMonth::new(2026, 10).unwrap();
    for start in ALL {
        let cells = displayed_month_cells(ym, start);
        assert_eq!(
            cells.len(),
            42,
            "a {start:?} start must still fill 42 cells"
        );
        assert_eq!(
            cells[0].date.day_of_week(),
            start,
            "the first cell must sit in the {start:?} column"
        );
        assert!(
            cells[0].date <= ym.first_date(),
            "the grid must never begin after the 1st for a {start:?} start"
        );
        assert_eq!(
            cells.iter().filter(|c| c.is_current_month).count(),
            31,
            "October must show 31 in-month cells for a {start:?} start"
        );

        for idx in 1..42 {
            assert_eq!(
                cells[idx].date,
                cells[idx - 1].date.next_day(),
                "cells must run consecutively for a {start:?} start at index {idx}"
            );
            assert_eq!(
                cells[idx].is_current_month,
                cells[idx].date.year == 2026 && cells[idx].date.month == 10,
                "the in-month flag must track the date at index {idx}"
            );
        }
    }

    // October 2026 starts on a Thursday, so a Sunday start pulls back to Sep 27.
    let sun = displayed_month_cells(ym, Weekday::Sun);
    assert_eq!(sun[0].date, CivilDate::new(2026, 9, 27).unwrap());
    assert!(!sun[0].is_current_month);
    assert_eq!(sun[4].date, CivilDate::new(2026, 10, 1).unwrap());
    assert!(sun[4].is_current_month);

    // January pulls its leading cells from the previous year.
    let jan = displayed_month_cells(YearMonth::new(2027, 1).unwrap(), Weekday::Mon);
    assert_eq!(jan[0].date, CivilDate::new(2026, 12, 28).unwrap());
    assert!(!jan[3].is_current_month);
    assert_eq!(jan[4].date, CivilDate::new(2027, 1, 1).unwrap());
    assert_eq!(jan.iter().filter(|c| c.is_current_month).count(), 31);
}

// ---------------------------------------------------------------------------
// DateSelection shapes
// ---------------------------------------------------------------------------

#[test]
fn test_date_selection_format_display_covers_every_shape() {
    let d1 = CivilDate::new(2026, 10, 10).unwrap();
    let d2 = CivilDate::new(2026, 10, 20).unwrap();

    let cases: &[(&str, DateSelection, &str)] = &[
        (
            "single-set",
            DateSelection::SingleDate(Some(d1)),
            "2026-10-10",
        ),
        ("single-empty", DateSelection::SingleDate(None), ""),
        (
            "range-settled",
            DateSelection::DateRange {
                start: Some(d1),
                end: Some(d2),
            },
            "2026-10-10 - 2026-10-20",
        ),
        (
            "range-awaiting-end",
            DateSelection::DateRange {
                start: Some(d1),
                end: None,
            },
            "2026-10-10 - ",
        ),
        (
            "range-orphan-end",
            DateSelection::DateRange {
                start: None,
                end: Some(d2),
            },
            "... - 2026-10-20",
        ),
        (
            "range-empty",
            DateSelection::DateRange {
                start: None,
                end: None,
            },
            "",
        ),
    ];

    for (label, selection, want) in cases {
        assert_eq!(
            selection.format_display(),
            *want,
            "{label} must format as {want:?}"
        );
        assert_eq!(
            selection.to_string(),
            *want,
            "{label} must Display as {want:?}"
        );
    }
}

#[test]
fn test_date_selection_endpoint_and_interior_branches() {
    let early = CivilDate::new(2026, 10, 10).unwrap();
    let late = CivilDate::new(2026, 10, 20).unwrap();
    let inside = CivilDate::new(2026, 10, 15).unwrap();
    let outside = CivilDate::new(2026, 10, 25).unwrap();

    // An empty single selection marks nothing.
    let empty_single = DateSelection::SingleDate(None);
    assert!(
        !empty_single.is_selected_endpoint(early),
        "an empty single selection has no endpoint"
    );
    assert!(
        !empty_single.is_range_interior(inside, Some(late)),
        "a single selection never has range interior"
    );
    assert!(
        !empty_single.is_range_preview_endpoint(late, Some(late)),
        "a single selection never previews a range"
    );

    // A range with no start marks only its orphan end.
    let orphan = DateSelection::DateRange {
        start: None,
        end: Some(late),
    };
    assert!(
        orphan.is_selected_endpoint(late),
        "an orphan end is still an endpoint"
    );
    assert!(
        !orphan.is_selected_endpoint(early),
        "an unselected date is not an endpoint"
    );
    assert!(
        !orphan.is_range_interior(inside, Some(late)),
        "a range with no start has no interior"
    );
    assert!(
        !orphan.is_range_preview_endpoint(late, Some(late)),
        "a range with no start never previews"
    );

    // A settled range normalises a reversed pair for the interior test.
    let reversed = DateSelection::DateRange {
        start: Some(late),
        end: Some(early),
    };
    assert!(
        reversed.is_range_interior(inside, None),
        "a reversed settled range must still fill between its endpoints"
    );
    assert!(
        !reversed.is_range_interior(outside, None),
        "dates outside a reversed range are not interior"
    );
    assert!(
        !reversed.is_range_interior(early, None),
        "an endpoint is never interior"
    );
    assert!(
        !reversed.is_range_preview_endpoint(inside, Some(inside)),
        "a settled range never previews"
    );

    // A degenerate range spans a single day and has no interior at all.
    let degenerate = DateSelection::DateRange {
        start: Some(early),
        end: Some(early),
    };
    assert!(
        degenerate.is_selected_endpoint(early),
        "both endpoints of a one-day range are that day"
    );
    assert!(
        !degenerate.is_range_interior(early, None),
        "a one-day range has no interior"
    );

    // An open range previews only against a hover, and never onto its own start.
    let open_range = DateSelection::DateRange {
        start: Some(early),
        end: None,
    };
    assert!(
        !open_range.is_range_interior(inside, None),
        "without a hover there is nothing to fill up to"
    );
    assert!(
        !open_range.is_range_preview_endpoint(inside, None),
        "without a hover there is no preview endpoint"
    );
    assert!(
        !open_range.is_range_preview_endpoint(early, Some(early)),
        "hovering back onto the start is not a preview endpoint"
    );
    assert!(
        open_range.is_range_preview_endpoint(late, Some(late)),
        "the hovered far date is the preview endpoint"
    );
    assert!(
        open_range.is_range_interior(inside, Some(late)),
        "the hovered far date fills up to itself"
    );

    // A hover on the far side of the start normalises and fills backwards.
    let back_hover = CivilDate::new(2026, 10, 5).unwrap();
    let between_back = CivilDate::new(2026, 10, 7).unwrap();
    assert!(
        open_range.is_range_interior(between_back, Some(back_hover)),
        "a hover before the start must fill backwards between the hover and the start"
    );
    assert!(
        !open_range.is_range_interior(inside, Some(back_hover)),
        "a backwards hover must not fill forward past the start"
    );
    assert!(
        !open_range.is_range_interior(outside, Some(back_hover)),
        "a backwards hover must not fill past the start"
    );
}

// ---------------------------------------------------------------------------
// DatePickerModel builders and selection transitions
// ---------------------------------------------------------------------------

#[test]
fn test_date_picker_model_builder_setters() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let other = CivilDate::new(2025, 3, 4).unwrap();
    let ym = YearMonth::new(2025, 3).unwrap();
    let selection = DateSelection::SingleDate(Some(other));

    let model = DatePickerModel::new(today)
        .with_selection(selection.clone())
        .with_hover_preview_date(Some(other))
        .with_displayed_month(ym)
        .with_view_mode(CalendarViewMode::Years)
        .with_min_date(other)
        .with_max_date(today)
        .with_blackout_dates([other])
        .with_first_day_of_week(Weekday::Sun)
        .with_open(true)
        .with_validation(Some("pick a later date".to_owned()))
        .with_disabled(true);

    assert_eq!(model.selection, selection, "with_selection must store it");
    assert_eq!(
        model.hover_preview_date,
        Some(other),
        "with_hover_preview_date must store it"
    );
    assert_eq!(
        model.displayed_month, ym,
        "with_displayed_month must store it"
    );
    assert_eq!(
        model.view_mode,
        CalendarViewMode::Years,
        "with_view_mode must store it"
    );
    assert_eq!(model.min_date, Some(other), "with_min_date must store it");
    assert_eq!(model.max_date, Some(today), "with_max_date must store it");
    assert!(
        model.blackout_dates.contains(&other),
        "with_blackout_dates must store the set"
    );
    assert_eq!(
        model.first_day_of_week,
        Weekday::Sun,
        "with_first_day_of_week must store it"
    );
    assert!(model.open, "with_open(true) must open the flyout");
    assert_eq!(
        model.validation.as_deref(),
        Some("pick a later date"),
        "with_validation must store the message"
    );
    assert!(model.disabled, "with_disabled must store it");

    // Every setter is clearable, and a fresh model starts closed and enabled.
    let cleared = DatePickerModel::new(today)
        .with_open(true)
        .with_open(false)
        .with_min_date(None::<CivilDate>)
        .with_max_date(None::<CivilDate>)
        .with_blackout_dates([])
        .with_hover_preview_date(None::<CivilDate>)
        .with_validation(Option::<String>::None);
    assert!(
        !cleared.open,
        "with_open(false) must close the flyout again"
    );
    assert!(cleared.min_date.is_none(), "with_min_date(None) must clear");
    assert!(cleared.max_date.is_none(), "with_max_date(None) must clear");
    assert!(
        cleared.blackout_dates.is_empty(),
        "with_blackout_dates([]) must clear"
    );
    assert!(
        cleared.hover_preview_date.is_none(),
        "with_hover_preview_date(None) must clear"
    );
    assert!(
        cleared.validation.is_none(),
        "with_validation(None) must clear the message"
    );
    assert_eq!(
        DatePickerModel::new(today).view_mode,
        CalendarViewMode::Days,
        "a fresh model opens on the day grid"
    );
    assert_eq!(
        CalendarViewMode::default(),
        CalendarViewMode::Days,
        "the default drill-down is the day grid"
    );
}

#[test]
fn test_date_picker_model_select_date_refuses_disabled_dates() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let blackout = CivilDate::new(2026, 10, 28).unwrap();
    let min = CivilDate::new(2026, 10, 20).unwrap();
    let max = CivilDate::new(2026, 10, 30).unwrap();

    let mut blackout_model = DatePickerModel::new(today).with_blackout_dates([blackout]);
    blackout_model.select_date(blackout);
    assert_eq!(
        blackout_model.selection,
        DateSelection::SingleDate(None),
        "a blackout date must never be selected"
    );

    let mut bounded = DatePickerModel::new(today)
        .with_min_date(min)
        .with_max_date(max);
    bounded.select_date(CivilDate::new(2026, 10, 1).unwrap());
    bounded.select_date(CivilDate::new(2026, 12, 1).unwrap());
    assert_eq!(
        bounded.selection,
        DateSelection::SingleDate(None),
        "dates outside min/max must never be selected"
    );
    bounded.select_date(today);
    assert_eq!(
        bounded.selection,
        DateSelection::SingleDate(Some(today)),
        "a date inside the bounds must be selectable"
    );

    let mut disabled = DatePickerModel::new(today)
        .with_disabled(true)
        .with_selection(DateSelection::SingleDate(Some(min)));
    disabled.select_date(today);
    assert_eq!(
        disabled.selection,
        DateSelection::SingleDate(Some(min)),
        "a disabled model must not change its selection at all"
    );
}

#[test]
fn test_date_picker_model_select_date_restarts_a_finished_range() {
    let d1 = CivilDate::new(2026, 10, 10).unwrap();
    let d2 = CivilDate::new(2026, 10, 20).unwrap();
    let d3 = CivilDate::new(2026, 10, 25).unwrap();

    // A third click after a settled range starts a brand new range.
    let mut finished = DatePickerModel::new(d1).with_selection(DateSelection::DateRange {
        start: Some(d1),
        end: Some(d2),
    });
    finished.select_date(d3);
    assert_eq!(
        finished.selection,
        DateSelection::DateRange {
            start: Some(d3),
            end: None,
        },
        "a click after a settled range must restart it"
    );

    // A range holding only an end is not a usable start, so it restarts too.
    let mut orphan = DatePickerModel::new(d1).with_selection(DateSelection::DateRange {
        start: None,
        end: Some(d2),
    });
    orphan.select_date(d3);
    assert_eq!(
        orphan.selection,
        DateSelection::DateRange {
            start: Some(d3),
            end: None,
        },
        "a range with only an end must restart on the next click"
    );

    // Clicking the same day twice closes a one-day range.
    let mut one_day = DatePickerModel::new(d1).with_selection(DateSelection::DateRange {
        start: None,
        end: None,
    });
    one_day.select_date(d2);
    one_day.select_date(d2);
    assert_eq!(
        one_day.selection,
        DateSelection::DateRange {
            start: Some(d2),
            end: Some(d2),
        },
        "two clicks on the same day must close a one-day range"
    );

    // A single selection is replaced, never toggled off.
    let mut single = DatePickerModel::new(d1);
    single.select_date(d1);
    single.select_date(d2);
    assert_eq!(
        single.selection,
        DateSelection::SingleDate(Some(d2)),
        "a second single click must move the selection, not clear it"
    );
}

// ---------------------------------------------------------------------------
// Widget construction, sizing and factory helpers
// ---------------------------------------------------------------------------

#[test]
fn test_date_picker_size_reports_trigger_or_trigger_plus_flyout() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    let closed = date_picker::<()>(DatePickerModel::new(today));
    let closed_size = closed.size();
    assert_eq!(
        closed_size.width,
        Length::Fixed(CALENDAR_FLYOUT_WIDTH),
        "the trigger is as wide as the flyout"
    );
    assert_eq!(
        closed_size.height,
        Length::Fixed(TRIGGER_HEIGHT),
        "a closed picker occupies only the 32px trigger"
    );

    let open = date_picker::<()>(DatePickerModel::new(today).with_open(true));
    assert_eq!(
        open.size().height,
        Length::Fixed(TRIGGER_HEIGHT + space::XS + calendar_flyout_height()),
        "an open picker must reserve the trigger, the gap and the whole card"
    );
}

#[test]
fn test_date_picker_layout_node_matches_size() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::new(0.0, 0.0), Size::new(2000.0, 2000.0));

    for (open, want_height) in [
        (false, TRIGGER_HEIGHT),
        (true, TRIGGER_HEIGHT + space::XS + calendar_flyout_height()),
    ] {
        let mut widget = date_picker::<()>(DatePickerModel::new(today).with_open(open));
        let mut tree = Tree::new(&widget as &dyn Widget<(), iced::Theme, iced::Renderer>);
        let widget = &mut widget as &mut dyn Widget<(), iced::Theme, iced::Renderer>;
        let node = widget.layout(&mut tree, &renderer, &limits);
        assert_eq!(
            node.size(),
            Size::new(CALENDAR_FLYOUT_WIDTH, want_height),
            "an open={open} picker must lay out to the trigger plus card height"
        );
    }
}

#[test]
fn test_date_range_picker_and_calendar_view_factories() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let d1 = CivilDate::new(2026, 10, 10).unwrap();

    // A single-date model handed to the range factory is converted, not kept.
    let converted = date_range_picker::<()>(
        DatePickerModel::new(today).with_selection(DateSelection::SingleDate(Some(d1))),
    );
    assert_eq!(
        converted.model().selection,
        DateSelection::DateRange {
            start: None,
            end: None,
        },
        "the range factory must replace a single-date selection with an empty range"
    );
    assert!(
        !converted.model().open,
        "the range factory must not force the flyout open"
    );

    // An existing range survives the factory untouched.
    let kept = date_range_picker::<()>(DatePickerModel::new(today).with_selection(
        DateSelection::DateRange {
            start: Some(d1),
            end: Some(today),
        },
    ));
    assert_eq!(
        kept.model().selection,
        DateSelection::DateRange {
            start: Some(d1),
            end: Some(today),
        },
        "the range factory must preserve a range it was already given"
    );

    // The embedded calendar view is always open.
    let calendar: CalendarView<'_, ()> = calendar_view::<()>(DatePickerModel::new(today));
    assert!(
        calendar.model().open,
        "calendar_view must force the flyout open"
    );
    let range_alias: DateRangePicker<'_, ()> = date_range_picker::<()>(DatePickerModel::new(today));
    assert_eq!(
        range_alias.model().selection,
        DateSelection::DateRange {
            start: None,
            end: None,
        },
        "the DateRangePicker alias must build the same range widget"
    );

    // `with_today` is what drives the today cue during a draw.
    let with_today = date_picker::<()>(DatePickerModel::new(today)).with_today(today);
    assert_eq!(
        with_today.model().displayed_month,
        YearMonth::new(2026, 10).unwrap(),
        "with_today must not disturb the displayed month"
    );
    assert_eq!(
        date_picker::<()>(DatePickerModel::new(today))
            .with_today(None::<CivilDate>)
            .model()
            .displayed_month
            .month,
        10,
        "with_today(None) must clear the cue without disturbing the model"
    );
}

#[test]
fn test_date_picker_registers_with_focus_router_unless_disabled() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    let mut enabled =
        Harness::<(), _>::headless(move || date_picker::<()>(DatePickerModel::new(today)).into());
    let registered = enabled.registered();
    assert_eq!(
        registered.len(),
        1,
        "an enabled picker must register exactly one focus target"
    );
    assert!(
        registered.first().is_some_and(Option::is_some),
        "the trigger must register under an id, not anonymously"
    );
    let idle = enabled.focus_count();
    assert_eq!(
        idle.total, 1,
        "the router must count the trigger as one focus stop"
    );
    assert_eq!(idle.focused, None, "nothing holds focus before a move");

    // The trigger registers under a name the host can choose, so the router's
    // own traversal and an id-addressed focus request name the same control.
    // `defect_03_*` below pins the stability that buys.
    enabled.iced_focus_next();
    assert_eq!(
        enabled.focus_count().focused,
        Some(0),
        "the router must be able to move focus onto the trigger"
    );
    assert!(
        enabled.focused().is_some(),
        "the router must report which target holds the focus"
    );
    enabled.unfocus();
    assert_eq!(
        enabled.focus_count().focused,
        None,
        "releasing focus must leave the trigger unfocused"
    );

    let mut disabled = Harness::<(), _>::headless(move || {
        date_picker::<()>(DatePickerModel::new(today).with_disabled(true)).into()
    });
    assert_eq!(
        disabled.registered(),
        vec![],
        "a disabled picker must be invisible to the focus router"
    );
    assert_eq!(
        disabled.focus_count().total,
        0,
        "a disabled picker must contribute no focus stop at all"
    );
}

/// **Defect 03, fixed.** The trigger registers under a name that survives a
/// rebuild, so an id-addressed focus request still names the same control on the
/// next frame.
///
/// `DatePicker::new` used to mint `widget::Id::unique()` on every construction,
/// and a host rebuilds its whole view on every update — so an id read in one
/// frame addressed nothing in the next, and `focus(id)` landed silently nowhere.
/// The harness rebuilds the tree on every step, which makes two reads from one
/// harness exactly two constructions of the widget.
#[test]
fn defect_03_the_date_picker_trigger_keeps_one_id_across_rebuilds() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut harness =
        Harness::<(), _>::headless(move || date_picker::<()>(DatePickerModel::new(today)).into());

    let first = harness.registered();
    let second = harness.registered();
    assert_eq!(
        first, second,
        "defect 03: two rebuilds of the same picker must register the same id"
    );

    let expected = iced::advanced::widget::Id::new(DEFAULT_DATE_PICKER_ID);
    assert_eq!(
        first,
        vec![Some(expected.clone())],
        "defect 03: the trigger registers under the documented default name"
    );

    // A second, unrelated tree — the same control built somewhere else — claims
    // the same name, which is what lets a bridge address it without a handle.
    let mut other =
        Harness::<(), _>::headless(move || date_picker::<()>(DatePickerModel::new(today)).into());
    assert_eq!(
        other.registered(),
        first,
        "defect 03: the id comes from the widget, not from the tree it was built into"
    );

    // And the widget reports the very name the router sees.
    let built = date_picker::<()>(DatePickerModel::new(today));
    assert_eq!(
        built.widget_id(),
        &expected,
        "defect 03: widget_id reports the id the router sees"
    );
}

/// **Defect 03, fixed.** `with_id` names the control, which is what a host with
/// more than one picker on a screen needs: two controls sharing one id cannot be
/// told apart by an id-addressed focus request.
#[test]
fn defect_03_with_id_names_the_date_picker_for_the_focus_router() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut start = Harness::<(), _>::headless(move || {
        date_picker::<()>(DatePickerModel::new(today))
            .with_id("field.start_date")
            .into()
    });
    let start_name = iced::advanced::widget::Id::new("field.start_date");
    assert_eq!(
        start.registered(),
        vec![Some(start_name.clone())],
        "defect 03: the router must see the name the host chose"
    );

    // An owned name registers under the same id a borrowed one would, so a host
    // that composes its field names at run time can still name its controls.
    let owned = String::from("field.end_date");
    let owned_id = iced::advanced::widget::Id::from(owned.clone());
    let mut end = Harness::<(), _>::headless(move || {
        date_picker::<()>(DatePickerModel::new(today))
            .with_id(owned.clone())
            .into()
    });
    assert_eq!(
        end.registered(),
        vec![Some(owned_id)],
        "defect 03: an owned name registers under the id its text spells"
    );
    assert_eq!(
        end.registered(),
        vec![Some(iced::advanced::widget::Id::new("field.end_date"))],
        "defect 03: an owned name and a borrowed one of the same text are one id"
    );

    assert_ne!(
        start.registered(),
        end.registered(),
        "defect 03: two differently named pickers are two distinct focus targets"
    );

    let plain = date_picker::<()>(DatePickerModel::new(today));
    let named = plain.with_id("field.start_date");
    assert_eq!(
        named.widget_id(),
        &start_name,
        "defect 03: widget_id reports the name `with_id` installed"
    );
}

#[test]
fn test_date_picker_mouse_interaction_tracks_disabled_and_cursor() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let renderer = headless_renderer();
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(400.0, 300.0));
    let node = layout::Node::new(Size::new(CALENDAR_FLYOUT_WIDTH, TRIGGER_HEIGHT));

    for (disabled, want) in [
        (false, mouse::Interaction::Pointer),
        (true, mouse::Interaction::None),
    ] {
        let widget = date_picker::<TestMsg>(DatePickerModel::new(today).with_disabled(disabled));
        let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
        let widget = &widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>;

        assert_eq!(
            widget.mouse_interaction(
                &tree,
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(100.0, 16.0)),
                &viewport,
                &renderer,
            ),
            want,
            "a cursor over a disabled={disabled} trigger must report {want:?}"
        );
        assert_eq!(
            widget.mouse_interaction(
                &tree,
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(300.0, 400.0)),
                &viewport,
                &renderer,
            ),
            mouse::Interaction::None,
            "a cursor outside the widget must never show a pointer"
        );
        assert_eq!(
            widget.mouse_interaction(
                &tree,
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &viewport,
                &renderer,
            ),
            mouse::Interaction::None,
            "an unavailable cursor must never show a pointer"
        );
    }
}

// ---------------------------------------------------------------------------
// Pointer interaction
// ---------------------------------------------------------------------------

#[test]
fn test_date_picker_hover_tracks_trailing_nav_and_title() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut driver = Driver::new(wired(DatePickerModel::new(today).with_open(true)));

    driver.hover(trailing_centre());
    assert!(
        driver.state().hovered_trailing,
        "the trailing toggle must light up under the cursor"
    );

    driver.hover(Point::new(100.0, 16.0));
    assert!(
        !driver.state().hovered_trailing,
        "the trailing toggle must clear when the cursor moves onto the text"
    );

    driver.hover(nav_prev_centre());
    assert!(
        driver.state().hovered_nav_prev && !driver.state().hovered_nav_next,
        "only the prev chevron may light up"
    );

    driver.hover(nav_next_centre());
    assert!(
        driver.state().hovered_nav_next && !driver.state().hovered_nav_prev,
        "only the next chevron may light up"
    );

    driver.hover(title_centre());
    assert!(
        driver.state().hovered_title,
        "the drill-down title must light up under the cursor"
    );
    assert!(
        !driver.state().hovered_nav_prev && !driver.state().hovered_nav_next,
        "the chevrons must clear when the cursor moves to the title"
    );
    assert_eq!(
        driver.drain(),
        vec![],
        "hovering the header must not publish any message"
    );
}

#[test]
fn test_date_picker_hover_clears_when_cursor_leaves() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut driver = Driver::new(wired(DatePickerModel::new(today).with_open(true)));

    driver.hover(cell_centre(2, 3));
    assert!(
        driver.state().hovered_date.is_some(),
        "a day cell must be hovered under the cursor"
    );

    driver.hover(dead_zone_centre());
    assert!(
        driver.state().hovered_date.is_none(),
        "moving onto the weekday row must unhover the day"
    );

    driver.hover(Point::new(300.0, 400.0));
    let state = driver.state();
    assert!(
        !state.hovered_trailing
            && !state.hovered_nav_prev
            && !state.hovered_nav_next
            && !state.hovered_title
            && state.hovered_date.is_none()
            && state.hovered_month.is_none()
            && state.hovered_year.is_none(),
        "a cursor outside the widget must clear every hover flag"
    );

    // A closed flyout clears the in-card hover even with the cursor inside.
    let mut closed = Driver::new(wired(DatePickerModel::new(today)));
    closed.hover(nav_prev_centre());
    let state = closed.state();
    assert!(
        !state.hovered_nav_prev && !state.hovered_title,
        "a closed flyout has nothing to hover"
    );

    driver.deliver(
        Event::Mouse(mouse::Event::CursorLeft),
        mouse::Cursor::Available(nav_prev_centre()),
    );
    let state = driver.state();
    assert!(
        !state.hovered_nav_prev && !state.hovered_trailing && state.hovered_date.is_none(),
        "CursorLeft must clear every hover flag even with a stale cursor position"
    );
}

#[test]
fn test_date_picker_hover_previews_range_and_publishes_live() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let start = CivilDate::new(2026, 10, 10).unwrap();
    let far = CivilDate::new(2026, 10, 15).unwrap();
    let ym = YearMonth::new(2026, 10).unwrap();

    let mut driver = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: Some(start),
                end: None,
            }),
    ));

    let (row, col) = cell_of(ym, far);
    driver.hover(cell_centre(row, col));
    assert_eq!(
        driver.state().hovered_date,
        Some(far),
        "the hovered cell must report the date it renders"
    );
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(DateSelection::DateRange {
            start: Some(start),
            end: Some(far),
        })],
        "hovering a far date must publish a live preview of the range"
    );

    // Re-hovering the same cell must not spam the live channel.
    driver.hover(cell_centre(row, col));
    assert_eq!(
        driver.drain(),
        vec![],
        "re-hovering the same cell must publish nothing"
    );

    // Hovering back onto the start collapses the preview to one day.
    let (srow, scol) = cell_of(ym, start);
    driver.hover(cell_centre(srow, scol));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(DateSelection::DateRange {
            start: Some(start),
            end: Some(start),
        })],
        "hovering the start must preview a one-day range"
    );

    // Hovering off the grid ends the preview without publishing.
    driver.hover(dead_zone_centre());
    assert!(
        driver.state().hovered_date.is_none(),
        "leaving the grid must drop the hover"
    );
    assert_eq!(
        driver.drain(),
        vec![],
        "leaving the grid must not publish a preview"
    );

    // A single-date selection never previews on hover.
    let mut single = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    single.hover(cell_centre(row, col));
    assert_eq!(
        single.state().hovered_date,
        Some(far),
        "a single-date hover must still track the hovered day"
    );
    assert_eq!(
        single.drain(),
        vec![],
        "a single-date hover must publish nothing"
    );
}

#[test]
fn test_date_picker_hover_in_drill_down_views() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    let mut months = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_view_mode(CalendarViewMode::Months),
    ));
    months.hover(drill_centre(4));
    assert_eq!(
        months.state().hovered_month,
        Some(5),
        "the fifth drill-down slot is May"
    );
    assert!(
        months.state().hovered_year.is_none(),
        "the Months view must not track a hovered year"
    );
    months.hover(drill_dead_zone_centre());
    assert!(
        months.state().hovered_month.is_none(),
        "hovering off the drill-down grid must clear the month"
    );

    let mut years = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_view_mode(CalendarViewMode::Years),
    ));
    years.hover(drill_centre(0));
    assert_eq!(
        years.state().hovered_year,
        Some(2019),
        "the decade grid starts one year before the decade"
    );
    years.hover(drill_centre(11));
    assert_eq!(
        years.state().hovered_year,
        Some(2030),
        "the twelfth slot is the last year of the grid"
    );
    assert!(
        years.state().hovered_month.is_none(),
        "the Years view must not track a hovered month"
    );
    years.hover(drill_dead_zone_centre());
    assert!(
        years.state().hovered_year.is_none(),
        "hovering off the decade grid must clear the year"
    );
    assert_eq!(
        years.drain(),
        vec![],
        "hovering a year must not publish anything"
    );
}

#[test]
fn test_date_picker_click_trigger_toggles_open_state() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    let mut closed = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    closed.click(Point::new(100.0, 16.0));
    assert_eq!(
        closed.drain(),
        vec![TestMsg::Opened(true)],
        "clicking a closed trigger must ask to open"
    );

    let mut open = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    open.click(Point::new(100.0, 16.0));
    assert_eq!(
        open.drain(),
        vec![TestMsg::Opened(false)],
        "clicking an open trigger must ask to close"
    );

    // The trailing toggle sits inside the trigger and shares its behaviour.
    let mut trailing = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    trailing.click(trailing_centre());
    assert_eq!(
        trailing.drain(),
        vec![TestMsg::Opened(true)],
        "the trailing toggle must open the flyout too"
    );

    // With no callback registered the click is inert rather than a panic.
    let mut mute = Driver::in_box(date_picker(DatePickerModel::new(today)), closed_bounds());
    mute.click(Point::new(100.0, 16.0));
    assert_eq!(
        mute.drain(),
        vec![],
        "an unhooked click must publish nothing"
    );
}

#[test]
fn test_date_picker_click_navigation_publishes_per_view_mode() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    const CASES: &[(&str, CalendarViewMode, YearMonth, YearMonth)] = &[
        (
            "days-prev-month",
            CalendarViewMode::Days,
            YearMonth {
                year: 2026,
                month: 9,
            },
            YearMonth {
                year: 2026,
                month: 11,
            },
        ),
        (
            "months-prev-year",
            CalendarViewMode::Months,
            YearMonth {
                year: 2025,
                month: 10,
            },
            YearMonth {
                year: 2027,
                month: 10,
            },
        ),
        (
            "years-prev-decade",
            CalendarViewMode::Years,
            YearMonth {
                year: 2016,
                month: 10,
            },
            YearMonth {
                year: 2036,
                month: 10,
            },
        ),
    ];

    for (label, mode, prev, next) in CASES {
        let mut driver = Driver::new(wired(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(*mode),
        ));
        driver.click(nav_prev_centre());
        assert_eq!(
            driver.drain(),
            vec![TestMsg::Navigated(*prev)],
            "{label} must navigate one step back"
        );

        let mut driver = Driver::new(wired(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(*mode),
        ));
        driver.click(nav_next_centre());
        assert_eq!(
            driver.drain(),
            vec![TestMsg::Navigated(*next)],
            "{label} must navigate one step forward"
        );
    }

    // An unhooked navigation click must stay silent rather than panic.
    let mut mute = Driver::new(date_picker(DatePickerModel::new(today).with_open(true)));
    mute.click(nav_prev_centre());
    mute.click(nav_next_centre());
    mute.click(title_centre());
    assert_eq!(
        mute.drain(),
        vec![],
        "navigation with no callbacks registered must publish nothing"
    );
}

#[test]
fn test_date_picker_title_cycles_drill_down_views() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    const CASES: &[(CalendarViewMode, CalendarViewMode)] = &[
        (CalendarViewMode::Days, CalendarViewMode::Months),
        (CalendarViewMode::Months, CalendarViewMode::Years),
        (CalendarViewMode::Years, CalendarViewMode::Days),
    ];

    for (from, want) in CASES {
        let mut driver = Driver::new(wired(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(*from),
        ));
        driver.click(title_centre());
        assert_eq!(
            driver.drain(),
            vec![TestMsg::Mode(*want)],
            "clicking the title in the {from:?} view must drill to {want:?}"
        );
    }
}

#[test]
fn test_date_picker_click_day_cell_single_date_commits_and_closes() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let picked = CivilDate::new(2026, 10, 15).unwrap();
    let ym = YearMonth::new(2026, 10).unwrap();

    let mut driver = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    let (row, col) = cell_of(ym, picked);
    driver.click(cell_centre(row, col));

    assert_eq!(
        driver.state().focused_date,
        Some(picked),
        "clicking a day must move the keyboard cursor onto it"
    );
    assert_eq!(
        driver.drain(),
        vec![
            TestMsg::Changed(DateSelection::SingleDate(Some(picked))),
            TestMsg::Committed(DateSelection::SingleDate(Some(picked))),
            TestMsg::Opened(false),
        ],
        "a single-date click must publish live, commit and close"
    );
}

#[test]
fn test_date_picker_click_day_cell_builds_a_range_in_two_clicks() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let first = CivilDate::new(2026, 10, 10).unwrap();
    let second = CivilDate::new(2026, 10, 20).unwrap();
    let ym = YearMonth::new(2026, 10).unwrap();
    let (first_pos, second_pos) = (cell_of(ym, first), cell_of(ym, second));

    let mut driver = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: None,
                end: None,
            }),
    ));

    driver.click(cell_centre(first_pos.0, first_pos.1));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(DateSelection::DateRange {
            start: Some(first),
            end: None,
        })],
        "the first range click must only open the range"
    );
    // The widget published the open range; the host stores it before redrawing.
    driver.host_selection(DateSelection::DateRange {
        start: Some(first),
        end: None,
    });

    driver.click(cell_centre(second_pos.0, second_pos.1));
    assert_eq!(
        driver.drain(),
        vec![
            TestMsg::Changed(DateSelection::DateRange {
                start: Some(first),
                end: Some(second),
            }),
            TestMsg::Committed(DateSelection::DateRange {
                start: Some(first),
                end: Some(second),
            }),
            TestMsg::Opened(false),
        ],
        "the second range click must settle, commit and close"
    );

    // Clicking backwards must still order the endpoints low to high.
    let mut inverted = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: Some(second),
                end: None,
            }),
    ));
    inverted.click(cell_centre(first_pos.0, first_pos.1));
    assert_eq!(
        inverted.drain(),
        vec![
            TestMsg::Changed(DateSelection::DateRange {
                start: Some(first),
                end: Some(second),
            }),
            TestMsg::Committed(DateSelection::DateRange {
                start: Some(first),
                end: Some(second),
            }),
            TestMsg::Opened(false),
        ],
        "a backwards second click must sort the range endpoints"
    );
}

#[test]
fn test_date_picker_click_refuses_disabled_cells_and_empty_space() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let blackout = CivilDate::new(2026, 10, 15).unwrap();
    let ym = YearMonth::new(2026, 10).unwrap();

    let mut driver = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_blackout_dates([blackout]),
    ));
    let (row, col) = cell_of(ym, blackout);
    driver.click(cell_centre(row, col));
    assert_eq!(
        driver.drain(),
        vec![],
        "clicking a blackout day must publish nothing"
    );
    assert_eq!(
        driver.state().focused_date,
        None,
        "clicking a blackout day must not move the keyboard cursor"
    );

    // A click inside the card but on no control is inert.
    driver.click(dead_zone_centre());
    assert_eq!(
        driver.drain(),
        vec![],
        "clicking the weekday header must publish nothing"
    );
}

#[test]
fn test_date_picker_click_drill_down_cells_returns_to_days() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    let mut months = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_view_mode(CalendarViewMode::Months),
    ));
    months.click(drill_centre(4));
    assert_eq!(
        months.drain(),
        vec![
            TestMsg::Navigated(YearMonth {
                year: 2026,
                month: 5,
            }),
            TestMsg::Mode(CalendarViewMode::Days),
        ],
        "picking May must navigate there and drop back to the day grid"
    );

    let mut years = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_view_mode(CalendarViewMode::Years),
    ));
    years.click(drill_centre(0));
    assert_eq!(
        years.drain(),
        vec![
            TestMsg::Navigated(YearMonth {
                year: 2019,
                month: 10,
            }),
            TestMsg::Mode(CalendarViewMode::Months),
        ],
        "picking a year must navigate and drill up to the month grid"
    );
}

#[test]
fn test_date_picker_click_outside_an_open_flyout_cancels() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let selection = DateSelection::SingleDate(Some(today));

    let mut open = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(selection.clone()),
    ));
    open.click(Point::new(300.0, 400.0));
    assert_eq!(
        open.drain(),
        vec![
            TestMsg::Cancelled(selection.clone()),
            TestMsg::Opened(false)
        ],
        "a click outside an open flyout must cancel and close"
    );

    let mut closed = Driver::in_box(
        wired(DatePickerModel::new(today).with_selection(selection)),
        closed_bounds(),
    );
    closed.click(Point::new(300.0, 400.0));
    assert_eq!(
        closed.drain(),
        vec![],
        "a click outside a closed picker must do nothing at all"
    );
}

// ---------------------------------------------------------------------------
// Keyboard interaction
// ---------------------------------------------------------------------------

#[test]
fn test_date_picker_escape_cancels_only_while_open() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let selection = DateSelection::SingleDate(Some(today));

    let mut open = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(selection.clone()),
    ));
    open.set_focus();
    open.key(Named::Escape);
    assert_eq!(
        open.drain(),
        vec![TestMsg::Cancelled(selection), TestMsg::Opened(false)],
        "Escape must restore the selection and close the flyout"
    );

    let mut closed = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    closed.set_focus();
    closed.key(Named::Escape);
    assert_eq!(
        closed.drain(),
        vec![],
        "Escape on an already closed picker must publish nothing"
    );
}

#[test]
fn test_date_picker_enter_opens_then_commits_the_focused_day() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let picked = CivilDate::new(2026, 10, 15).unwrap();

    // Unfocused, Enter does nothing at all.
    let mut unfocused = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    unfocused.key(Named::Enter);
    assert_eq!(
        unfocused.drain(),
        vec![],
        "Enter without focus must not open the flyout"
    );

    // Focused and closed: Enter opens the flyout.
    let mut focused = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    focused.set_focus();
    focused.key(Named::Enter);
    assert_eq!(
        focused.drain(),
        vec![TestMsg::Opened(true)],
        "Enter on a focused closed picker must open it"
    );

    // Space opens it too, and does nothing once it is already open.
    let mut spaced = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    spaced.set_focus();
    spaced.key(Named::Space);
    assert_eq!(
        spaced.drain(),
        vec![TestMsg::Opened(true)],
        "Space on a focused closed picker must open it"
    );

    let mut already_open = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    already_open.set_focus();
    already_open.key(Named::Space);
    assert_eq!(
        already_open.drain(),
        vec![],
        "Space while open must be inert"
    );

    // Focused and open with a day under the cursor: single date commits.
    let mut commit = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    commit.set_focus();
    commit.focus_date(picked);
    commit.key(Named::Enter);
    assert_eq!(
        commit.drain(),
        vec![
            TestMsg::Committed(DateSelection::SingleDate(Some(picked))),
            TestMsg::Opened(false),
        ],
        "Enter must commit the focused day and close"
    );

    // A disabled day is refused.
    let mut blocked = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_blackout_dates([picked]),
    ));
    blocked.set_focus();
    blocked.focus_date(picked);
    blocked.key(Named::Enter);
    assert_eq!(
        blocked.drain(),
        vec![],
        "Enter on a blackout day must publish nothing"
    );

    // A day outside min/max is refused too.
    let beyond = CivilDate::new(2026, 10, 25).unwrap();
    let mut out_of_bounds = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_max_date(CivilDate::new(2026, 10, 20).unwrap()),
    ));
    out_of_bounds.set_focus();
    out_of_bounds.focus_date(beyond);
    out_of_bounds.key(Named::Enter);
    assert_eq!(
        out_of_bounds.drain(),
        vec![],
        "Enter on an out-of-bounds day must publish nothing"
    );
    assert_eq!(
        out_of_bounds.state().focused_date,
        Some(beyond),
        "a refused Enter must leave the keyboard cursor on the day it was given"
    );
}

#[test]
fn test_date_picker_enter_completes_a_range_in_two_presses() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let first = CivilDate::new(2026, 10, 10).unwrap();
    let second = CivilDate::new(2026, 10, 20).unwrap();

    // First press opens the range and only publishes live.
    let mut opener = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: None,
                end: None,
            }),
    ));
    opener.set_focus();
    opener.focus_date(first);
    opener.key(Named::Enter);
    assert_eq!(
        opener.drain(),
        vec![TestMsg::Changed(DateSelection::DateRange {
            start: Some(first),
            end: None,
        })],
        "the first Enter must open the range without committing"
    );

    // Second press settles it, sorting a backwards pick.
    let mut finisher = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_selection(DateSelection::DateRange {
                start: Some(second),
                end: None,
            }),
    ));
    finisher.set_focus();
    finisher.focus_date(first);
    finisher.key(Named::Enter);
    assert_eq!(
        finisher.drain(),
        vec![
            TestMsg::Committed(DateSelection::DateRange {
                start: Some(first),
                end: Some(second),
            }),
            TestMsg::Opened(false),
        ],
        "the second Enter must commit a sorted range and close"
    );
}

#[test]
fn test_date_picker_arrow_keys_move_the_focused_day() {
    let mid = CivilDate::new(2026, 10, 15).unwrap();

    const CASES: &[(&str, Named, CivilDate)] = &[
        (
            "left",
            Named::ArrowLeft,
            CivilDate {
                year: 2026,
                month: 10,
                day: 14,
            },
        ),
        (
            "right",
            Named::ArrowRight,
            CivilDate {
                year: 2026,
                month: 10,
                day: 16,
            },
        ),
        (
            "up",
            Named::ArrowUp,
            CivilDate {
                year: 2026,
                month: 10,
                day: 8,
            },
        ),
        (
            "down",
            Named::ArrowDown,
            CivilDate {
                year: 2026,
                month: 10,
                day: 22,
            },
        ),
        (
            "home",
            Named::Home,
            CivilDate {
                year: 2026,
                month: 10,
                day: 1,
            },
        ),
        (
            "end",
            Named::End,
            CivilDate {
                year: 2026,
                month: 10,
                day: 31,
            },
        ),
    ];

    for (label, key, want) in CASES {
        let mut driver = Driver::new(wired(DatePickerModel::new(mid).with_open(true)));
        driver.focus_date(mid);
        driver.key(*key);
        assert_eq!(
            driver.state().focused_date,
            Some(*want),
            "{label} must focus {want}"
        );
        assert_eq!(
            driver.drain(),
            vec![],
            "{label} stays inside October so no navigation is published"
        );
    }

    // End lands on the 29th of a leap February.
    let mut leap = Driver::new(wired(
        DatePickerModel::new(CivilDate::new(2024, 2, 10).unwrap()).with_open(true),
    ));
    leap.key(Named::End);
    assert_eq!(
        leap.state().focused_date,
        Some(CivilDate::new(2024, 2, 29).unwrap()),
        "End must land on the last day of a leap February"
    );
    assert_eq!(leap.drain(), vec![], "a same-month End publishes nothing");
}

#[test]
fn test_date_picker_arrow_keys_ask_to_navigate_across_months() {
    let first = CivilDate::new(2026, 10, 1).unwrap();
    let last = CivilDate::new(2026, 10, 31).unwrap();

    // ArrowLeft off the 1st walks into the previous month.
    let mut back = Driver::new(wired(DatePickerModel::new(first).with_open(true)));
    back.focus_date(first);
    back.key(Named::ArrowLeft);
    assert_eq!(
        back.state().focused_date,
        Some(CivilDate::new(2026, 9, 30).unwrap()),
        "ArrowLeft off the 1st must land on the last day of September"
    );
    assert_eq!(
        back.drain(),
        vec![TestMsg::Navigated(YearMonth {
            year: 2026,
            month: 9,
        })],
        "crossing a month boundary must ask the host to navigate"
    );

    // ArrowDown off the last week walks into the next month.
    let mut fwd = Driver::new(wired(DatePickerModel::new(last).with_open(true)));
    fwd.focus_date(last);
    fwd.key(Named::ArrowDown);
    assert_eq!(
        fwd.state().focused_date,
        Some(CivilDate::new(2026, 11, 7).unwrap()),
        "ArrowDown from Oct 31 must land on Nov 7"
    );
    assert_eq!(
        fwd.drain(),
        vec![TestMsg::Navigated(YearMonth {
            year: 2026,
            month: 11,
        })],
        "crossing into November must ask the host to navigate"
    );

    // With no day focused yet, the arrows start from the 1st of the month.
    let mut fresh = Driver::new(wired(DatePickerModel::new(first).with_open(true)));
    fresh.key(Named::ArrowRight);
    assert_eq!(
        fresh.state().focused_date,
        Some(CivilDate::new(2026, 10, 2).unwrap()),
        "the arrows must start from the first of the displayed month"
    );
}

#[test]
fn test_date_picker_page_keys_step_a_month_and_clamp_the_day() {
    // March 31 has no April equivalent, so PageDown must clamp to April 30.
    let mut driver = Driver::new(wired(
        DatePickerModel::new(CivilDate::new(2026, 3, 31).unwrap()).with_open(true),
    ));
    driver.focus_date(CivilDate::new(2026, 3, 31).unwrap());
    driver.key(Named::PageDown);
    assert_eq!(
        driver.state().focused_date,
        Some(CivilDate::new(2026, 4, 30).unwrap()),
        "PageDown from Mar 31 must clamp to Apr 30"
    );
    let april = YearMonth {
        year: 2026,
        month: 4,
    };
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Navigated(april), TestMsg::Navigated(april)],
        "PageDown publishes the navigation it performs and the one the new focus implies"
    );

    // PageUp from a 31-day month into a 30-day month clamps the same way.
    let mut back = Driver::new(wired(
        DatePickerModel::new(CivilDate::new(2026, 5, 31).unwrap()).with_open(true),
    ));
    back.focus_date(CivilDate::new(2026, 5, 31).unwrap());
    back.key(Named::PageUp);
    assert_eq!(
        back.state().focused_date,
        Some(CivilDate::new(2026, 4, 30).unwrap()),
        "PageUp from May 31 must clamp to Apr 30"
    );
    back.drain();

    // PageUp across a year boundary keeps the day when both months have it.
    let mut jan = Driver::new(wired(
        DatePickerModel::new(CivilDate::new(2026, 1, 15).unwrap()).with_open(true),
    ));
    jan.focus_date(CivilDate::new(2026, 1, 15).unwrap());
    jan.key(Named::PageUp);
    assert_eq!(
        jan.state().focused_date,
        Some(CivilDate::new(2025, 12, 15).unwrap()),
        "PageUp across a year boundary must keep the day of month"
    );
    assert_eq!(
        jan.drain(),
        vec![
            TestMsg::Navigated(YearMonth {
                year: 2025,
                month: 12,
            }),
            TestMsg::Navigated(YearMonth {
                year: 2025,
                month: 12,
            }),
        ],
        "a year-boundary PageUp must report the December navigation"
    );
}

#[test]
fn test_date_picker_ignores_unhandled_keys_and_closed_or_drilled_states() {
    let today = CivilDate::new(2026, 10, 26).unwrap();

    // An ordinary character is not a navigation key.
    let mut typed = Driver::new(wired(DatePickerModel::new(today).with_open(true)));
    typed.focus_date(today);
    typed.deliver(
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Character("x".into()),
            modified_key: Key::Character("x".into()),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::NONE,
            text: Some("x".into()),
            repeat: false,
        }),
        mouse::Cursor::Unavailable,
    );
    assert_eq!(
        typed.state().focused_date,
        Some(today),
        "a character key must not move the focused day"
    );
    assert_eq!(
        typed.drain(),
        vec![],
        "a character key must publish nothing"
    );

    // Arrow keys are inert while the flyout is closed.
    let mut closed = Driver::in_box(wired(DatePickerModel::new(today)), closed_bounds());
    closed.set_focus();
    closed.key(Named::ArrowDown);
    assert_eq!(
        closed.state().focused_date,
        None,
        "arrows must not move focus while the flyout is closed"
    );
    assert_eq!(
        closed.drain(),
        vec![],
        "arrows must publish nothing while closed"
    );

    // ... and while drilled into the month or year grid.
    for mode in [CalendarViewMode::Months, CalendarViewMode::Years] {
        let mut drilled = Driver::new(wired(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(mode),
        ));
        drilled.key(Named::ArrowDown);
        assert_eq!(
            drilled.state().focused_date,
            None,
            "arrows must not move a day focus in the {mode:?} view"
        );
        assert_eq!(
            drilled.drain(),
            vec![],
            "arrows must publish nothing in the {mode:?} view"
        );
    }
}

#[test]
fn test_date_picker_disabled_model_ignores_every_event() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let mut driver = Driver::new(wired(
        DatePickerModel::new(today)
            .with_open(true)
            .with_disabled(true),
    ));
    driver.set_focus();

    driver.hover(trailing_centre());
    driver.click(cell_centre(2, 3));
    driver.click(nav_next_centre());
    driver.key(Named::ArrowDown);
    driver.key(Named::Enter);
    driver.key(Named::Escape);
    driver.deliver(
        Event::Mouse(mouse::Event::CursorLeft),
        mouse::Cursor::Unavailable,
    );

    assert_eq!(
        driver.drain(),
        vec![],
        "a disabled picker must publish nothing"
    );
    let state = driver.state();
    assert!(
        !state.hovered_trailing && !state.hovered_nav_next,
        "a disabled picker must not track hover"
    );
    assert!(
        state.hovered_date.is_none() && state.focused_date.is_none(),
        "a disabled picker must not track day hover or focus"
    );
}

// ---------------------------------------------------------------------------
// Paint-path states the existing render sweep does not reach
// ---------------------------------------------------------------------------

#[test]
fn test_headless_render_date_picker_hover_and_focus_states() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let ym = YearMonth::new(2026, 10).unwrap();

    // Hovered chevrons, title and day cell, plus the focus ring.
    let mut harness = Harness::<(), _>::headless(move || {
        date_picker::<()>(
            DatePickerModel::new(today)
                .with_open(true)
                .with_selection(DateSelection::SingleDate(Some(today))),
        )
        .with_today(today)
        .into()
    });
    let id = harness
        .registered()
        .first()
        .and_then(Option::as_ref)
        .expect("an enabled picker must register a focus target")
        .clone();
    harness.focus(&id);
    harness.render();

    for point in [
        nav_prev_centre(),
        nav_next_centre(),
        title_centre(),
        cell_centre(
            cell_of(ym, CivilDate::new(2026, 10, 15).unwrap()).0,
            cell_of(ym, CivilDate::new(2026, 10, 15).unwrap()).1,
        ),
    ] {
        harness.hover(point);
        harness.render();
    }

    // A hovered drill-down month and year.
    let mut months = Harness::<(), _>::headless(move || {
        date_picker::<()>(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(CalendarViewMode::Months),
        )
        .into()
    });
    months.hover(drill_centre(4));
    months.render();

    let mut years = Harness::<(), _>::headless(move || {
        date_picker::<()>(
            DatePickerModel::new(today)
                .with_open(true)
                .with_view_mode(CalendarViewMode::Years),
        )
        .into()
    });
    years.hover(drill_centre(3));
    years.render();
}

#[test]
fn test_headless_render_date_picker_range_preview_and_validation_states() {
    let today = CivilDate::new(2026, 10, 26).unwrap();
    let start = CivilDate::new(2026, 10, 10).unwrap();
    let preview = CivilDate::new(2026, 10, 18).unwrap();

    // An open range with a hover preview paints a preview endpoint cell.
    let mut previewing = Harness::<(), _>::headless(move || {
        date_range_picker::<()>(
            DatePickerModel::new(today)
                .with_open(true)
                .with_selection(DateSelection::DateRange {
                    start: Some(start),
                    end: None,
                })
                .with_hover_preview_date(Some(preview)),
        )
        .with_today(preview)
        .into()
    });
    previewing.render();

    // A validation message repaints the trigger border, and an empty
    // selection falls back to the placeholder text.
    let mut invalid = Harness::<(), _>::headless(move || {
        date_picker::<()>(
            DatePickerModel::new(today)
                .with_selection(DateSelection::SingleDate(None))
                .with_validation(Some("date is outside the allowed window".to_owned())),
        )
        .into()
    });
    invalid.render();

    // Out-of-bounds and blackout days paint the strike-through cue.
    let mut bounded = Harness::<(), _>::headless(move || {
        date_picker::<()>(
            DatePickerModel::new(today)
                .with_open(true)
                .with_min_date(CivilDate::new(2026, 10, 12).unwrap())
                .with_max_date(CivilDate::new(2026, 10, 20).unwrap())
                .with_blackout_dates([CivilDate::new(2026, 10, 15).unwrap()]),
        )
        .with_today(CivilDate::new(2026, 10, 15).unwrap())
        .into()
    });
    bounded.render();
}
