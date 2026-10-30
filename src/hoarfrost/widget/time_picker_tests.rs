//! Unit tests for TimePicker (§3.11, `UIF-ATOM-011`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iced::Element;

use super::*;
use crate::test_helpers::{Harness, headless_renderer, moved, pressed};

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
    // (hour, minute, second, expected hour_12, expected meridian)
    const CASES: [(u8, u8, u8, u8, AmPm); 6] = [
        (0, 0, 0, 12, AmPm::Am),
        (1, 30, 0, 1, AmPm::Am),
        (11, 59, 59, 11, AmPm::Am),
        (12, 0, 0, 12, AmPm::Pm),
        (13, 15, 0, 1, AmPm::Pm),
        (23, 45, 0, 11, AmPm::Pm),
    ];
    for (hour, minute, second, hour_12, meridian) in CASES {
        let t = CivilTime::new(hour, minute, second).expect("fixture must be a valid time");
        assert_eq!(t.hour_12(), hour_12, "hour_12 of {t}");
        assert_eq!(t.am_pm(), meridian, "am_pm of {t}");
    }

    // The two named anchors must agree with the table above.
    assert_eq!(CivilTime::midnight(), CivilTime::new(0, 0, 0).unwrap());
    assert_eq!(CivilTime::noon(), CivilTime::new(12, 0, 0).unwrap());

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
    // (segment, use_24_hour, show_seconds, next, prev) for every combination.
    const CASES: [(TimeSegment, bool, bool, TimeSegment, TimeSegment); 16] = [
        (
            TimeSegment::Hour,
            true,
            false,
            TimeSegment::Minute,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::Hour,
            true,
            true,
            TimeSegment::Minute,
            TimeSegment::Second,
        ),
        (
            TimeSegment::Hour,
            false,
            false,
            TimeSegment::Minute,
            TimeSegment::AmPm,
        ),
        (
            TimeSegment::Hour,
            false,
            true,
            TimeSegment::Minute,
            TimeSegment::AmPm,
        ),
        (
            TimeSegment::Minute,
            true,
            false,
            TimeSegment::Hour,
            TimeSegment::Hour,
        ),
        (
            TimeSegment::Minute,
            true,
            true,
            TimeSegment::Second,
            TimeSegment::Hour,
        ),
        (
            TimeSegment::Minute,
            false,
            false,
            TimeSegment::AmPm,
            TimeSegment::Hour,
        ),
        (
            TimeSegment::Minute,
            false,
            true,
            TimeSegment::Second,
            TimeSegment::Hour,
        ),
        (
            TimeSegment::Second,
            true,
            false,
            TimeSegment::Hour,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::Second,
            true,
            true,
            TimeSegment::Hour,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::Second,
            false,
            false,
            TimeSegment::AmPm,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::Second,
            false,
            true,
            TimeSegment::AmPm,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::AmPm,
            true,
            false,
            TimeSegment::Hour,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::AmPm,
            true,
            true,
            TimeSegment::Hour,
            TimeSegment::Second,
        ),
        (
            TimeSegment::AmPm,
            false,
            false,
            TimeSegment::Hour,
            TimeSegment::Minute,
        ),
        (
            TimeSegment::AmPm,
            false,
            true,
            TimeSegment::Hour,
            TimeSegment::Second,
        ),
    ];

    for (segment, use_24_hour, show_seconds, next, prev) in CASES {
        let mode = format!("{segment:?} 24h={use_24_hour} secs={show_seconds}");
        assert_eq!(
            segment.next(use_24_hour, show_seconds),
            next,
            "{mode} must step forward to {next:?}"
        );
        assert_eq!(
            segment.prev(use_24_hour, show_seconds),
            prev,
            "{mode} must step back to {prev:?}"
        );
    }
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
    use super::DrumColumnKind as K;

    // (label, use_24_hour, show_seconds, expected columns)
    const CASES: &[(&str, bool, bool, &[K])] = &[
        ("24h", true, false, &[K::Hour, K::Minute]),
        ("24h+sec", true, true, &[K::Hour, K::Minute, K::Second]),
        ("12h", false, false, &[K::Hour, K::Minute, K::AmPm]),
        (
            "12h+sec",
            false,
            true,
            &[K::Hour, K::Minute, K::Second, K::AmPm],
        ),
    ];

    for (label, use_24_hour, show_seconds, expected) in CASES {
        assert_eq!(
            active_columns(*use_24_hour, *show_seconds),
            expected.to_vec(),
            "{label} must compose {expected:?}"
        );
    }
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

// ---------------------------------------------------------------------------
// Interaction harness
//
// The harness drives `Widget::update` directly so a test can read the
// `TimePickerState` the widget keeps in the tree back out again, which is what
// makes hover, focus and segment tracking observable, not just the messages.
// ---------------------------------------------------------------------------

/// Message vocabulary produced by the callbacks wired up by [`wired`].
#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    /// Flyout open state changed.
    Opened(bool),
    /// Live time change.
    Changed(CivilTime),
    /// Time committed.
    Committed(CivilTime),
    /// Interaction cancelled with the restored value.
    Cancelled(Option<CivilTime>),
}

/// A picker with every callback registered, so a test can assert on the full
/// Live / Commit / Cancel protocol rather than a subset of it.
fn wired(model: TimePickerModel) -> TimePicker<'static, TestMsg> {
    time_picker(model)
        .on_open_change(|live| TestMsg::Opened(live.into_value()))
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

/// Offset inside the hour segment of the segmented trigger.
const HOUR_X: f32 = 14.0;
/// Offset inside the minute segment of the segmented trigger.
const MINUTE_X: f32 = 40.0;
/// Offset inside the second segment of the segmented trigger.
const SECOND_X: f32 = 90.0;
/// Offset past every numeric segment, where the meridian segment sits.
const MERIDIAN_X: f32 = 130.0;

/// Total height of a picker laid out with its drum flyout open.
fn open_height() -> f32 {
    TRIGGER_HEIGHT + space::XS + TIME_FLYOUT_MAX_HEIGHT
}

/// Box of an open picker laid out at the viewport origin.
fn open_bounds() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(TIME_FLYOUT_WIDTH, open_height()))
}

/// Box of a closed picker laid out at the viewport origin.
fn closed_bounds() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(TIME_FLYOUT_WIDTH, TRIGGER_HEIGHT))
}

/// Top edge of the drum flyout card.
fn flyout_y() -> f32 {
    TRIGGER_HEIGHT + space::XS
}

/// Top edge of the scrollable drum area below the column headers.
fn drum_top() -> f32 {
    flyout_y() + HEADER_HEIGHT
}

/// Top edge of the central selection band inside the drum.
fn band_y() -> f32 {
    drum_top() + SELECTION_BAND_OFFSET_Y
}

/// Width of one pinned footer action button.
fn footer_button_w() -> f32 {
    (TIME_FLYOUT_WIDTH - 3.0 * space::SM) / 2.0
}

/// Top edge of a pinned footer action button.
fn footer_button_y() -> f32 {
    let footer_y = flyout_y() + TIME_FLYOUT_MAX_HEIGHT - FOOTER_HEIGHT;
    footer_y + (FOOTER_HEIGHT - FOOTER_BUTTON_HEIGHT) / 2.0
}

/// Centre of the trailing clock toggle on the trigger box.
fn trailing_centre() -> Point {
    Point::new(
        TIME_FLYOUT_WIDTH - TRAILING_BUTTON_SIZE - space::SM + TRAILING_BUTTON_SIZE / 2.0,
        (TRIGGER_HEIGHT - TRAILING_BUTTON_SIZE) / 2.0 + TRAILING_BUTTON_SIZE / 2.0,
    )
}

/// Centre of the `Cancel` footer button.
fn cancel_centre() -> Point {
    Point::new(
        space::SM + footer_button_w() / 2.0,
        footer_button_y() + FOOTER_BUTTON_HEIGHT / 2.0,
    )
}

/// Centre of the `✓ Apply` footer button.
fn apply_centre() -> Point {
    Point::new(
        TIME_FLYOUT_WIDTH - space::SM - footer_button_w() / 2.0,
        footer_button_y() + FOOTER_BUTTON_HEIGHT / 2.0,
    )
}

/// Centre of the drum cell in column `col` of a `num_cols` column layout,
/// `row_offset` rows away from the central selection band.
fn drum_centre(col: usize, num_cols: usize, row_offset: i32) -> Point {
    let col_w = (TIME_FLYOUT_WIDTH - (num_cols - 1) as f32 * HAIRLINE_WIDTH) / num_cols as f32;
    Point::new(
        col as f32 * (col_w + HAIRLINE_WIDTH) + col_w / 2.0,
        band_y() + row_offset as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0,
    )
}

/// A point on the segmented trigger `rel_x` pixels right of the text inset.
fn trigger_x(rel_x: f32) -> f32 {
    space::MD + rel_x
}

/// A [`TimePicker`] driven through `Widget::update` directly, keeping the
/// [`Tree`] so the widget's own state is readable after every event.
struct Driver<'a> {
    widget: TimePicker<'a, TestMsg>,
    tree: Tree,
    renderer: iced::Renderer,
    bounds: Rectangle,
    messages: Vec<TestMsg>,
}

impl<'a> Driver<'a> {
    /// Wrap `widget` laid out at [`open_bounds`].
    fn new(widget: TimePicker<'a, TestMsg>) -> Self {
        Self::in_box(widget, open_bounds())
    }

    /// Wrap `widget` laid out at an explicit box.
    fn in_box(widget: TimePicker<'a, TestMsg>, bounds: Rectangle) -> Self {
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
    fn state(&self) -> &TimePickerState {
        self.tree.state.downcast_ref::<TimePickerState>()
    }

    /// The model the widget owns.
    fn model(&self) -> &TimePickerModel {
        self.widget.model()
    }

    /// Give the trigger keyboard focus, as the focus router would.
    fn set_focus(&mut self) {
        let state = self.tree.state.downcast_mut::<TimePickerState>();
        <FocusableState as operation::Focusable>::focus(&mut state.focusable);
    }

    /// Move the keyboard cursor to a segment, as clicking it would.
    fn focus_segment(&mut self, segment: TimeSegment) {
        let state = self.tree.state.downcast_mut::<TimePickerState>();
        state.focused_segment = segment;
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

    /// Paint the widget with whatever state the events have built up.
    fn paint(&mut self) {
        let node = layout::Node::new(self.bounds.size());
        let layout = Layout::new(&node);
        let style = renderer::Style {
            text_color: Color::WHITE,
        };
        let widget = &self.widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
        widget.draw(
            &self.tree,
            &mut self.renderer,
            &iced::Theme::Dark,
            &style,
            layout,
            mouse::Cursor::Unavailable,
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

    /// Scroll a wheel in lines with the cursor on `at`.
    fn scroll_lines(&mut self, at: Point, y: f32) {
        self.deliver(
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y },
            }),
            mouse::Cursor::Available(at),
        );
    }

    /// Scroll a wheel in pixels with the cursor on `at`.
    fn scroll_pixels(&mut self, at: Point, y: f32) {
        self.deliver(
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y },
            }),
            mouse::Cursor::Available(at),
        );
    }

    /// Take every message published since the last drain.
    fn drain(&mut self) -> Vec<TestMsg> {
        std::mem::take(&mut self.messages)
    }
}

// ---------------------------------------------------------------------------
// Meridian, stepping and parsing
// ---------------------------------------------------------------------------

#[test]
fn test_ampm_labels_toggle_and_default() {
    assert_eq!(AmPm::Am.as_str(), LABEL_AM);
    assert_eq!(AmPm::Pm.as_str(), LABEL_PM);
    assert_eq!(format!("{}", AmPm::Am), LABEL_AM);
    assert_eq!(format!("{}", AmPm::Pm), LABEL_PM);
    assert_eq!(AmPm::Am.toggle(), AmPm::Pm);
    assert_eq!(AmPm::Pm.toggle(), AmPm::Am);
    assert_eq!(
        AmPm::default(),
        AmPm::Am,
        "an unspecified meridian must default to AM"
    );
}

#[test]
fn test_step_hour_wraps_beyond_a_single_day() {
    let t = CivilTime::new(10, 30, 15).unwrap();
    assert_eq!(
        t.step_hour(24),
        t,
        "a whole day of hours must land back on the same time"
    );
    assert_eq!(
        t.step_hour(25),
        CivilTime::new(11, 30, 15).unwrap(),
        "25 hours must land one hour ahead"
    );
    assert_eq!(
        t.step_hour(-25),
        CivilTime::new(9, 30, 15).unwrap(),
        "-25 hours must land one hour back"
    );
    assert_eq!(
        t.step_hour(-11),
        CivilTime::new(23, 30, 15).unwrap(),
        "stepping back across midnight must wrap the hour only"
    );
}

#[test]
fn test_step_minute_treats_a_zero_increment_as_one() {
    let t = CivilTime::new(10, 10, 0).unwrap();
    assert_eq!(
        t.step_minute(3, 0),
        CivilTime::new(10, 13, 0).unwrap(),
        "a zero increment must fall back to one minute"
    );
    assert_eq!(
        t.step_minute(-1, 0),
        CivilTime::new(10, 9, 0).unwrap(),
        "the fallback must step backwards too"
    );
}

#[test]
fn test_step_second_wraps_in_both_directions() {
    let t = CivilTime::new(10, 10, 10).unwrap();
    assert_eq!(
        t.step_second(60),
        t,
        "a whole minute of seconds must land back on the same time"
    );
    assert_eq!(
        t.step_second(-61),
        CivilTime::new(10, 10, 9).unwrap(),
        "-61 seconds must land one second back"
    );
}

#[test]
fn test_civil_time_parse_accepts_the_shapes_it_claims_to() {
    // (input, expected)
    let accepts: &[(&str, Option<CivilTime>)] = &[
        ("5", CivilTime::new(5, 0, 0)),
        ("2:05 pm", CivilTime::new(14, 5, 0)),
        ("12:00PM", CivilTime::new(12, 0, 0)),
        ("12:00 am", CivilTime::new(0, 0, 0)),
        ("01:02:03", CivilTime::new(1, 2, 3)),
        ("  09:30  ", CivilTime::new(9, 30, 0)),
    ];
    for (input, expected) in accepts {
        assert_eq!(CivilTime::parse(input), *expected, "must parse {input:?}");
    }
}

#[test]
fn test_civil_time_parse_rejects_malformed_and_out_of_range_input() {
    // (input, why it must be rejected)
    const REJECTS: &[(&str, &str)] = &[
        ("", "empty"),
        ("   ", "blank after trimming"),
        ("PM", "a bare meridian has no time"),
        ("5:", "an empty minute field"),
        ("1:2:3:4", "more than three fields"),
        ("13:00 PM", "12-hour values stop at 12"),
        ("0:00 AM", "12-hour values start at 1"),
        ("24:00", "hours stop at 23"),
        ("12:00:61 PM", "seconds stop at 59"),
    ];
    for (input, why) in REJECTS {
        assert_eq!(
            CivilTime::parse(input),
            None,
            "must reject {input:?}: {why}"
        );
    }
}

#[test]
fn test_format_display_dispatches_on_the_two_flags() {
    let t = CivilTime::new(14, 5, 9).unwrap();
    // (use_24_hour, show_seconds, expected)
    const CASES: &[(&str, bool, bool, &str)] = &[
        ("24h", true, false, "14:05"),
        ("24h+sec", true, true, "14:05:09"),
        ("12h", false, false, "02:05 PM"),
        ("12h+sec", false, true, "02:05:09 PM"),
    ];
    for (label, use_24_hour, show_seconds, expected) in CASES {
        assert_eq!(
            t.format_display(*use_24_hour, *show_seconds),
            *expected,
            "{label} must format as {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[test]
fn test_time_picker_model_default_is_an_empty_12h_control() {
    let model = TimePickerModel::default();
    assert_eq!(model, TimePickerModel::new(None));
    assert_eq!(model.value, None);
    assert!(!model.use_24_hour, "the default must be 12-hour");
    assert!(!model.show_seconds, "the default must hide seconds");
    assert_eq!(model.minute_increment, 1);
    assert!(!model.open, "the default must be closed");
    assert_eq!(model.validation, None);
    assert!(!model.disabled);
    assert_eq!(model.pre_open_snapshot, None);
}

#[test]
fn test_model_validation_opt_sets_and_clears() {
    let message = "Time must be in the future".to_owned();
    let set = TimePickerModel::new(None).with_validation_opt(Some(message.clone()));
    assert_eq!(set.validation.as_deref(), Some(message.as_str()));

    let cleared = set.with_validation_opt(None);
    assert_eq!(
        cleared.validation, None,
        "with_validation_opt(None) must clear the message"
    );
}

#[test]
fn test_model_placeholder_covers_all_four_shapes() {
    // (label, use_24_hour, show_seconds, expected placeholder)
    const CASES: &[(&str, bool, bool, &str)] = &[
        ("24h", true, false, PLACEHOLDER_24H),
        ("24h+sec", true, true, PLACEHOLDER_24H_SEC),
        ("12h", false, false, PLACEHOLDER_12H),
        ("12h+sec", false, true, PLACEHOLDER_12H_SEC),
    ];
    for (label, use_24_hour, show_seconds, expected) in CASES {
        let model = TimePickerModel::new(None)
            .with_24_hour(*use_24_hour)
            .with_show_seconds(*show_seconds);
        assert_eq!(
            model.format_display(),
            *expected,
            "{label} must show {expected} when unselected"
        );
    }
}

#[test]
fn test_model_minute_increment_accepts_only_the_documented_steps() {
    // (requested, expected)
    const CASES: [(u8, u8); 6] = [(1, 1), (5, 5), (10, 10), (15, 15), (30, 30), (7, 1)];
    for (requested, expected) in CASES {
        assert_eq!(
            TimePickerModel::new(None)
                .with_minute_increment(requested)
                .minute_increment,
            expected,
            "an increment of {requested} must resolve to {expected}"
        );
    }
}

#[test]
fn test_model_open_flyout_is_inert_while_disabled() {
    let mut model = TimePickerModel::new(Some(CivilTime::noon())).with_disabled(true);
    model.open_flyout();
    assert!(!model.open, "a disabled model must refuse to open");
    assert_eq!(
        model.pre_open_snapshot, None,
        "a refused open must capture no snapshot"
    );
}

#[test]
fn test_model_close_flyout_keeps_the_value_and_snapshot() {
    let value = CivilTime::new(8, 15, 0).unwrap();
    let mut model = TimePickerModel::new(Some(value));
    model.open_flyout();
    model.select_time(CivilTime::new(9, 45, 0).unwrap());
    model.close_flyout();

    assert!(!model.open, "close_flyout must close");
    assert_eq!(
        model.value,
        Some(CivilTime::new(9, 45, 0).unwrap()),
        "close_flyout must not roll anything back"
    );
    assert_eq!(
        model.pre_open_snapshot,
        Some(Some(value)),
        "close_flyout must leave the snapshot alone"
    );
}

#[test]
fn test_model_cancel_without_a_snapshot_keeps_the_current_value() {
    let value = CivilTime::new(16, 20, 0).unwrap();
    let mut model = TimePickerModel::new(Some(value));
    assert_eq!(
        model.cancel(),
        Some(value),
        "cancelling with no snapshot must fall back to the live value"
    );
    assert_eq!(model.value, Some(value));
    assert!(!model.open);

    let mut empty = TimePickerModel::new(None);
    assert_eq!(
        empty.cancel(),
        None,
        "cancelling an unselected empty model must restore nothing"
    );
}

#[test]
fn test_model_step_segment_initialises_an_unselected_value() {
    let mut utc = TimePickerModel::new(None).with_24_hour(true);
    utc.step_segment(TimeSegment::Hour, 1);
    assert_eq!(
        utc.value,
        Some(CivilTime::new(1, 0, 0).unwrap()),
        "a 24-hour picker must start stepping from midnight"
    );

    let mut clock12 = TimePickerModel::new(None);
    clock12.step_segment(TimeSegment::Hour, 1);
    assert_eq!(
        clock12.value,
        Some(CivilTime::new(13, 0, 0).unwrap()),
        "a 12-hour picker must start stepping from noon"
    );
}

#[test]
fn test_drum_column_labels() {
    use super::DrumColumnKind as K;

    assert_eq!(K::Hour.label(), LABEL_HR);
    assert_eq!(K::Minute.label(), LABEL_MIN);
    assert_eq!(K::Second.label(), LABEL_SEC);
    assert_eq!(K::AmPm.label(), LABEL_AM_PM);
}

#[test]
fn test_time_picker_size_reports_trigger_or_trigger_plus_flyout() {
    let value = CivilTime::new(7, 30, 0).unwrap();
    let closed = TimePicker::<TestMsg>::new(TimePickerModel::new(Some(value)));
    assert_eq!(
        closed.size().width,
        Length::Fixed(TIME_FLYOUT_WIDTH),
        "the trigger is exactly as wide as the drum card"
    );
    assert_eq!(
        closed.size().height,
        Length::Fixed(TRIGGER_HEIGHT),
        "a closed picker is exactly one trigger tall"
    );

    let open = TimePicker::<TestMsg>::new(TimePickerModel::new(Some(value)).with_open(true));
    assert_eq!(
        open.size().height,
        Length::Fixed(open_height()),
        "an open picker reserves the trigger plus the full drum card"
    );
}

#[test]
fn test_model_mut_edits_through_the_accessor() {
    let mut picker = TimePicker::<TestMsg>::new(TimePickerModel::new(None));
    picker.model_mut().select_time(CivilTime::noon());
    picker.model_mut().open_flyout();
    assert_eq!(
        picker.model().value,
        Some(CivilTime::noon()),
        "model_mut must write through to the widget's own model"
    );
    assert!(
        picker.model().open,
        "model_mut must expose the same model the widget reads"
    );
}

// ---------------------------------------------------------------------------
// Focus router and cursor affordance
// ---------------------------------------------------------------------------

#[test]
fn test_time_picker_registers_with_focus_router_unless_disabled() {
    let value = CivilTime::new(9, 15, 0).unwrap();

    let mut enabled = Harness::<(), _>::headless(move || {
        time_picker::<()>(TimePickerModel::new(Some(value))).into()
    });
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
        time_picker::<()>(TimePickerModel::new(Some(value)).with_disabled(true)).into()
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
/// `TimePicker::new` used to mint `widget::Id::unique()` on every construction,
/// and a host rebuilds its whole view on every update — so an id read in one
/// frame addressed nothing in the next, and `focus(id)` landed silently nowhere.
/// The harness rebuilds the tree on every step, which makes two reads from one
/// harness exactly two constructions of the widget.
#[test]
fn defect_03_the_time_picker_trigger_keeps_one_id_across_rebuilds() {
    let value = CivilTime::new(9, 15, 0).unwrap();
    let mut harness = Harness::<(), _>::headless(move || {
        time_picker::<()>(TimePickerModel::new(Some(value))).into()
    });

    let first = harness.registered();
    let second = harness.registered();
    assert_eq!(
        first, second,
        "defect 03: two rebuilds of the same picker must register the same id"
    );
    assert_eq!(
        first,
        vec![Some(iced::advanced::widget::Id::new(
            DEFAULT_TIME_PICKER_ID
        ))],
        "defect 03: the trigger registers under the documented default name"
    );

    // A second, unrelated tree — the same control built somewhere else — claims
    // the same name, which is what lets a bridge address it without a handle.
    let mut other = Harness::<(), _>::headless(move || {
        time_picker::<()>(TimePickerModel::new(Some(value))).into()
    });
    assert_eq!(
        other.registered(),
        first,
        "defect 03: the id comes from the widget, not from the tree it was built into"
    );

    // And the widget reports the very name the router sees.
    assert_eq!(
        time_picker::<()>(TimePickerModel::new(Some(value))).widget_id(),
        &iced::advanced::widget::Id::new(DEFAULT_TIME_PICKER_ID),
        "defect 03: widget_id reports the id the router sees"
    );
}

/// **Defect 03, fixed.** `with_id` names the control, which is what a host with
/// more than one picker on a screen needs: two controls sharing one id cannot be
/// told apart by an id-addressed focus request.
#[test]
fn defect_03_with_id_names_the_time_picker_for_the_focus_router() {
    let value = CivilTime::new(9, 15, 0).unwrap();
    let mut meeting = Harness::<(), _>::headless(move || {
        time_picker::<()>(TimePickerModel::new(Some(value)))
            .with_id("field.meeting_start")
            .into()
    });
    assert_eq!(
        meeting.registered(),
        vec![Some(iced::advanced::widget::Id::new("field.meeting_start"))],
        "defect 03: the router must see the name the host chose"
    );

    // An owned name registers under the same id a borrowed one would, so a host
    // that composes its field names at run time can still name its controls.
    let owned = String::from("field.meeting_end");
    let mut finish = Harness::<(), _>::headless(move || {
        time_picker::<()>(TimePickerModel::new(Some(value)))
            .with_id(owned.clone())
            .into()
    });
    assert_eq!(
        finish.registered(),
        vec![Some(iced::advanced::widget::Id::new("field.meeting_end"))],
        "defect 03: an owned name and a borrowed one of the same text are one id"
    );

    assert_ne!(
        meeting.registered(),
        finish.registered(),
        "defect 03: two differently named pickers are two distinct focus targets"
    );
    assert_eq!(
        time_picker::<()>(TimePickerModel::new(Some(value)))
            .with_id("field.meeting_start")
            .widget_id(),
        &iced::advanced::widget::Id::new("field.meeting_start"),
        "defect 03: widget_id reports the name `with_id` installed"
    );
}

#[test]
fn test_time_picker_mouse_interaction_tracks_disabled_and_cursor() {
    let value = CivilTime::new(9, 15, 0).unwrap();
    let renderer = headless_renderer();
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(400.0, 300.0));
    let node = layout::Node::new(Size::new(TIME_FLYOUT_WIDTH, TRIGGER_HEIGHT));

    for (disabled, want) in [
        (false, mouse::Interaction::Pointer),
        (true, mouse::Interaction::None),
    ] {
        let widget =
            time_picker::<TestMsg>(TimePickerModel::new(Some(value)).with_disabled(disabled));
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
fn test_time_picker_hover_tracks_the_trailing_toggle() {
    let value = CivilTime::new(9, 15, 0).unwrap();
    let mut driver = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());

    driver.hover(trailing_centre());
    assert!(
        driver.state().hovered_trailing,
        "the trailing toggle must light up under the cursor"
    );

    driver.hover(Point::new(trigger_x(HOUR_X), 16.0));
    assert!(
        !driver.state().hovered_trailing,
        "moving onto the trigger text must clear the trailing hover"
    );
}

#[test]
fn test_time_picker_hover_of_flyout_targets() {
    let value = CivilTime::new(9, 15, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));

    driver.hover(cancel_centre());
    assert!(
        driver.state().hovered_cancel,
        "the Cancel button must light up under the cursor"
    );
    assert!(
        !driver.state().hovered_apply && driver.state().hovered_column.is_none(),
        "the footer sits below the drum, so no column may be hovered"
    );
    assert!(
        driver.state().hovered_row.is_none(),
        "the footer sits below the drum, so no row may be hovered"
    );

    driver.hover(apply_centre());
    assert!(
        driver.state().hovered_apply && !driver.state().hovered_cancel,
        "only one footer button may be hovered at a time"
    );

    driver.hover(drum_centre(0, 2, 1));
    assert_eq!(
        driver.state().hovered_column,
        Some(0),
        "the left half of the drum is the hour column"
    );
    assert_eq!(
        driver.state().hovered_row,
        Some(1),
        "one row below the selection band is row offset 1"
    );
    assert!(
        !driver.state().hovered_cancel && !driver.state().hovered_apply,
        "hovering the drum must clear both footer buttons"
    );

    driver.hover(drum_centre(1, 2, -2));
    assert_eq!(
        driver.state().hovered_column,
        Some(1),
        "the right half of the drum is the minute column"
    );
    assert_eq!(
        driver.state().hovered_row,
        Some(-2),
        "two rows above the selection band is row offset -2"
    );

    // Inside the card but above the drum: the column header band.
    driver.hover(Point::new(110.0, flyout_y() + HEADER_HEIGHT / 2.0));
    assert!(
        driver.state().hovered_column.is_none() && driver.state().hovered_row.is_none(),
        "the header band belongs to no drum column"
    );
    assert!(
        !driver.state().hovered_trailing,
        "a cursor on the header band is not on the trailing toggle"
    );
}

#[test]
fn test_time_picker_hover_clears_when_the_cursor_leaves_or_the_flyout_is_closed() {
    let value = CivilTime::new(9, 15, 0).unwrap();

    let mut driver = Driver::new(wired(TimePickerModel::new(Some(value)).with_open(true)));
    driver.hover(cancel_centre());
    driver.hover(Point::new(300.0, 16.0));
    let state = driver.state();
    assert!(
        !state.hovered_trailing
            && !state.hovered_cancel
            && !state.hovered_apply
            && state.hovered_column.is_none()
            && state.hovered_row.is_none(),
        "a cursor outside the widget must clear every hover flag"
    );

    // A closed picker must not track footer or drum targets at all.
    let mut closed = Driver::in_box(
        wired(TimePickerModel::new(Some(value)).with_open(true)),
        closed_bounds(),
    );
    closed.hover(cancel_centre());
    let state = closed.state();
    assert!(
        !state.hovered_cancel && !state.hovered_apply && state.hovered_column.is_none(),
        "a closed picker must ignore flyout targets"
    );
}

#[test]
fn test_time_picker_trailing_toggle_opens_and_closes_the_flyout() {
    let value = CivilTime::new(9, 15, 0).unwrap();

    let mut driver = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    driver.click(trailing_centre());
    assert!(
        driver.model().open,
        "the trailing toggle must open the flyout"
    );
    assert_eq!(
        driver.state().pre_open_snapshot,
        Some(Some(value)),
        "opening must mirror the pre-open snapshot onto the state"
    );
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Opened(true)],
        "opening must publish the live open state"
    );

    driver.click(trailing_centre());
    assert!(
        !driver.model().open,
        "a second click must close the flyout again"
    );
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Opened(false)],
        "closing must publish the live open state"
    );

    // With no callback registered the click is inert rather than a panic.
    let mut mute = Driver::in_box(
        time_picker::<TestMsg>(TimePickerModel::new(Some(value))),
        closed_bounds(),
    );
    mute.click(trailing_centre());
    assert!(mute.model().open, "the toggle must still work unhooked");
    assert_eq!(
        mute.drain(),
        vec![],
        "an unhooked click must publish nothing"
    );
}

#[test]
fn test_time_picker_trigger_click_focuses_the_segment_under_the_cursor() {
    let value = CivilTime::new(9, 15, 30).unwrap();

    let mut driver = Driver::in_box(
        wired(
            TimePickerModel::new(Some(value))
                .with_24_hour(false)
                .with_show_seconds(true),
        ),
        closed_bounds(),
    );

    // (label, x offset, expected segment)
    const CASES: &[(&str, f32, TimeSegment)] = &[
        ("hour", HOUR_X, TimeSegment::Hour),
        ("minute", MINUTE_X, TimeSegment::Minute),
        ("second", SECOND_X, TimeSegment::Second),
        ("meridian", MERIDIAN_X, TimeSegment::AmPm),
    ];
    for (label, offset, want) in CASES {
        driver.click(Point::new(trigger_x(*offset), 16.0));
        assert_eq!(
            driver.state().focused_segment,
            *want,
            "a click over the {label} segment must focus it"
        );
        assert!(
            driver.state().focusable.is_focused(),
            "a click on the trigger must take keyboard focus"
        );
        assert_eq!(
            driver.drain(),
            vec![],
            "focusing a segment publishes nothing"
        );
    }

    // Past the last numeric segment of a 24-hour trigger there is nothing to focus.
    let mut utc = Driver::in_box(
        wired(TimePickerModel::new(Some(value)).with_24_hour(true)),
        closed_bounds(),
    );
    utc.click(Point::new(trigger_x(MINUTE_X), 16.0));
    utc.click(Point::new(trigger_x(MERIDIAN_X), 16.0));
    assert_eq!(
        utc.state().focused_segment,
        TimeSegment::Minute,
        "a 24-hour trigger has no meridian segment, so the focus must not move"
    );
}

#[test]
fn test_time_picker_drum_click_steps_the_clicked_column() {
    let utc = TimePickerModel::new(Some(CivilTime::new(10, 0, 0).unwrap()))
        .with_24_hour(true)
        .with_open(true);
    let mut driver = Driver::new(wired(utc));
    driver.click(drum_centre(0, 2, 2));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(12, 0, 0).unwrap())],
        "clicking two rows below the band in the hour column must add two hours"
    );
    driver.click(drum_centre(1, 2, -1));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(12, 59, 0).unwrap())],
        "each drum column is its own wheel: one row up from :00 is :59 and the hour is untouched"
    );

    // A stepped minute column honours the configured increment.
    let stepped = TimePickerModel::new(Some(CivilTime::new(10, 0, 0).unwrap()))
        .with_24_hour(true)
        .with_open(true)
        .with_minute_increment(5);
    let mut driver = Driver::new(wired(stepped));
    driver.click(drum_centre(1, 2, -1));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(10, 55, 0).unwrap())],
        "one row up a 5-minute column must subtract five minutes inside the minute wheel"
    );

    // The seconds column steps the second component.
    let with_seconds = TimePickerModel::new(Some(CivilTime::new(10, 20, 30).unwrap()))
        .with_24_hour(true)
        .with_show_seconds(true)
        .with_open(true);
    let mut driver = Driver::new(wired(with_seconds));
    driver.click(drum_centre(2, 3, 3));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(10, 20, 33).unwrap())],
        "the third column of a 24-hour picker is seconds"
    );

    // The meridian column toggles, and a click on the band itself changes nothing.
    let clock12 = TimePickerModel::new(Some(CivilTime::new(10, 0, 0).unwrap())).with_open(true);
    let mut driver = Driver::new(wired(clock12));
    driver.click(drum_centre(2, 3, 1));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(22, 0, 0).unwrap())],
        "a meridian row off the band must flip AM to PM"
    );
    driver.click(drum_centre(2, 3, 0));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(22, 0, 0).unwrap())],
        "the selected meridian row must republish the unchanged value"
    );
    assert_eq!(
        driver.model().value,
        Some(CivilTime::new(22, 0, 0).unwrap()),
        "clicking the selected meridian must not flip it"
    );
}

#[test]
fn test_time_picker_cancel_button_rolls_the_edit_back() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(TimePickerModel::new(Some(value)).with_24_hour(true)));

    // Opening through the trigger is what captures the pre-open snapshot that
    // Cancel restores from.
    driver.click(trailing_centre());
    assert!(
        driver.model().open,
        "the trailing toggle must open the card"
    );
    assert_eq!(
        driver.state().pre_open_snapshot,
        Some(Some(value)),
        "opening the card must capture the pre-open snapshot"
    );
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Opened(true)],
        "the trailing toggle must report the open"
    );

    driver.click(drum_centre(0, 2, 2));
    assert_eq!(
        driver.model().value,
        Some(CivilTime::new(12, 0, 0).unwrap())
    );
    driver.drain();

    driver.click(cancel_centre());
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Cancelled(Some(value)), TestMsg::Opened(false),],
        "Cancel must restore the pre-open value and report the close"
    );
    assert_eq!(
        driver.model().value,
        Some(value),
        "the edit must be rolled back"
    );
    assert!(!driver.model().open, "Cancel must close the flyout");
    assert_eq!(
        driver.model().pre_open_snapshot,
        None,
        "Cancel must consume the snapshot"
    );
}

/// **A defect fixed, and the test flipped to assert the correct behaviour.**
///
/// `TimePickerModel::with_open(true)` used to set the flag and nothing else,
/// while `ColorPickerModel::with_open` captured the pre-open snapshot. A card
/// the host declared open therefore had nothing to roll back to: Cancel
/// reported the live, edited value as the "restored" one, and the edit
/// silently survived the button the user had just pressed to abandon it.
///
/// `with_open` now captures the snapshot exactly as `open_flyout` does.
#[test]
fn defect_04_a_card_the_host_declared_open_rolls_back_on_cancel() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    assert_eq!(
        driver.model().pre_open_snapshot,
        Some(Some(value)),
        "defect 04: declaring the card open captures the pre-open value, the way `open_flyout` does"
    );

    driver.click(drum_centre(0, 2, 2));
    assert_eq!(
        driver.model().value,
        Some(CivilTime::new(12, 0, 0).unwrap()),
        "the drum is usable even when the card was declared open by the host"
    );
    driver.drain();

    driver.click(cancel_centre());
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Cancelled(Some(value)), TestMsg::Opened(false),],
        "defect 04: Cancel reports the pre-open value, not the edit"
    );
    assert_eq!(
        driver.model().value,
        Some(value),
        "defect 04: the edit is rolled back to what the host opened with"
    );
    assert!(!driver.model().open, "Cancel must close the flyout");
    assert_eq!(
        driver.model().pre_open_snapshot,
        None,
        "defect 04: Cancel consumes the snapshot"
    );
}

/// The other half of the rollback contract: a card the host declared open still
/// commits what the user left in it when they confirm instead of cancelling.
#[test]
fn defect_04_a_card_the_host_declared_open_keeps_what_apply_confirms() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));

    driver.click(drum_centre(0, 2, 2));
    assert_eq!(
        driver.model().value,
        Some(CivilTime::new(12, 0, 0).unwrap()),
        "the edit is live before Apply"
    );
    driver.drain();

    driver.click(apply_centre());
    assert_eq!(
        driver.drain(),
        vec![
            TestMsg::Committed(CivilTime::new(12, 0, 0).unwrap()),
            TestMsg::Opened(false),
        ],
        "defect 04: Apply commits the edit even though a snapshot was taken"
    );
    assert_eq!(
        driver.model().value,
        Some(CivilTime::new(12, 0, 0).unwrap()),
        "defect 04: the confirmed value survives"
    );
    assert_eq!(
        driver.model().pre_open_snapshot,
        None,
        "defect 04: Apply drops the snapshot"
    );
}

/// The model-level contract, without a widget in the way: `with_open` and
/// `open_flyout` must record the same thing, and `with_open(false)` must
/// record nothing.
#[test]
fn defect_04_with_open_records_the_snapshot_open_flyout_records() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let later = CivilTime::new(11, 30, 0).unwrap();

    // Declared open by the host.
    let declared = TimePickerModel::new(Some(value)).with_open(true);
    assert!(declared.open, "defect 04: with_open(true) opens the card");
    assert_eq!(
        declared.pre_open_snapshot,
        Some(Some(value)),
        "defect 04: and captures the value it opened with"
    );

    // Opened through the interaction path, for comparison.
    let mut interacted = TimePickerModel::new(Some(value));
    interacted.open_flyout();
    assert_eq!(
        interacted.pre_open_snapshot, declared.pre_open_snapshot,
        "defect 04: both open paths record the same snapshot"
    );

    // Declared closed: no flag, no snapshot.
    let shut = TimePickerModel::new(Some(value)).with_open(false);
    assert!(
        !shut.open,
        "defect 04: with_open(false) leaves the card shut"
    );
    assert_eq!(
        shut.pre_open_snapshot, None,
        "defect 04: closing records nothing"
    );

    // An empty value snapshots as `Some(None)` — open, and there was nothing.
    let empty = TimePickerModel::new(None).with_open(true);
    assert_eq!(
        empty.pre_open_snapshot,
        Some(None),
        "defect 04: an unselected picker snapshots its emptiness, so cancel restores empty"
    );

    // And the rollback actually happens at the model level.
    let mut model = TimePickerModel::new(Some(value)).with_open(true);
    model.select_time(later);
    assert_eq!(model.value, Some(later), "the edit is live");
    assert_eq!(
        model.cancel(),
        Some(value),
        "defect 04: cancel returns the pre-open value, not the edit"
    );
    assert_eq!(model.value, Some(value), "defect 04: and leaves it there");
    assert!(!model.open, "defect 04: cancel closes the card");
    assert_eq!(model.pre_open_snapshot, None, "defect 04: consumed");

    // Confirming keeps the edit.
    let mut confirmed = TimePickerModel::new(Some(value)).with_open(true);
    confirmed.select_time(later);
    confirmed.commit();
    assert_eq!(
        confirmed.value,
        Some(later),
        "defect 04: commit keeps the edited value"
    );
    assert_eq!(
        confirmed.pre_open_snapshot, None,
        "defect 04: commit drops it"
    );
}

#[test]
fn test_time_picker_apply_button_commits_and_defaults_an_empty_value() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    driver.click(apply_centre());
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Committed(value), TestMsg::Opened(false),],
        "Apply must commit the live value and report the close"
    );
    assert_eq!(
        driver.model().pre_open_snapshot,
        None,
        "Apply must drop the snapshot"
    );

    // Applying an unselected 12-hour picker falls back to noon.
    let mut clock12 = Driver::new(wired(TimePickerModel::new(None).with_open(true)));
    clock12.click(apply_centre());
    assert_eq!(
        clock12.drain(),
        vec![
            TestMsg::Committed(CivilTime::noon()),
            TestMsg::Opened(false),
        ],
        "an empty 12-hour Apply must default to noon"
    );
    assert_eq!(clock12.model().value, Some(CivilTime::noon()));

    // ... and an unselected 24-hour picker to midnight.
    let mut utc = Driver::new(wired(
        TimePickerModel::new(None)
            .with_24_hour(true)
            .with_open(true),
    ));
    utc.click(apply_centre());
    assert_eq!(
        utc.drain(),
        vec![
            TestMsg::Committed(CivilTime::midnight()),
            TestMsg::Opened(false),
        ],
        "an empty 24-hour Apply must default to midnight"
    );
}

#[test]
fn test_time_picker_click_outside_an_open_flyout_cancels() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(TimePickerModel::new(Some(value)).with_24_hour(true)));
    driver.click(trailing_centre());
    driver.drain();
    driver.click(drum_centre(0, 2, 2));
    driver.drain();

    driver.click(Point::new(300.0, 16.0));
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Cancelled(Some(value)), TestMsg::Opened(false),],
        "a click beside the card must cancel the gesture"
    );
    assert_eq!(driver.model().value, Some(value));
    assert!(!driver.model().open);
}

#[test]
fn test_time_picker_click_on_a_closed_trigger_is_not_a_cancel() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    driver.click(Point::new(300.0, 16.0));
    assert_eq!(
        driver.drain(),
        vec![],
        "with the flyout closed there is nothing to cancel"
    );
}

// ---------------------------------------------------------------------------
// Wheel interaction
// ---------------------------------------------------------------------------

#[test]
fn test_time_picker_wheel_scrolls_the_focused_segment() {
    let value = CivilTime::new(10, 20, 30).unwrap();
    let mut driver = Driver::in_box(
        wired(
            TimePickerModel::new(Some(value))
                .with_24_hour(true)
                .with_minute_increment(5),
        ),
        closed_bounds(),
    );
    driver.set_focus();
    let on_trigger = Point::new(100.0, 16.0);

    driver.scroll_lines(on_trigger, 1.0);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(11, 20, 30).unwrap())],
        "a wheel up must step the focused hour forward"
    );
    driver.scroll_lines(on_trigger, -1.0);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(value)],
        "a wheel down must step the focused hour back"
    );
    driver.scroll_pixels(on_trigger, 2.5);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(11, 20, 30).unwrap())],
        "a pixel wheel up counts as one step"
    );
    driver.scroll_pixels(on_trigger, -2.5);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(value)],
        "a pixel wheel down counts as one step back"
    );

    driver.focus_segment(TimeSegment::Minute);
    driver.scroll_lines(on_trigger, 1.0);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(10, 25, 30).unwrap())],
        "scrolling the minute segment must use the configured increment"
    );
}

#[test]
fn test_time_picker_wheel_scrolls_a_drum_column_against_the_rows() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    let on_hour = drum_centre(0, 2, 0);

    driver.scroll_lines(on_hour, 1.0);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(9, 0, 0).unwrap())],
        "scrolling up over a column must move the rows down, i.e. one hour back"
    );
    driver.scroll_lines(on_hour, -1.0);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(value)],
        "scrolling down over a column must advance one hour"
    );

    // The meridian column ignores the scroll direction and simply flips.
    let mut clock12 = Driver::new(wired(TimePickerModel::new(Some(value)).with_open(true)));
    let on_meridian = drum_centre(2, 3, 0);
    clock12.scroll_pixels(on_meridian, 3.0);
    assert_eq!(
        clock12.drain(),
        vec![TestMsg::Changed(CivilTime::new(22, 0, 0).unwrap())],
        "scrolling the meridian column must flip AM to PM"
    );
    clock12.scroll_pixels(on_meridian, -3.0);
    assert_eq!(
        clock12.drain(),
        vec![TestMsg::Changed(value)],
        "scrolling the meridian column back must flip PM to AM"
    );
}

#[test]
fn test_time_picker_wheel_ignores_a_zero_delta_and_a_point_outside() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    driver.set_focus();

    driver.scroll_lines(Point::new(100.0, 16.0), 0.0);
    driver.scroll_pixels(Point::new(100.0, 16.0), 0.0);
    driver.scroll_lines(Point::new(300.0, 16.0), 1.0);
    driver.scroll_lines(Point::new(110.0, 400.0), -1.0);
    assert_eq!(
        driver.drain(),
        vec![],
        "a zero delta or a cursor off the widget must not step anything"
    );
    assert_eq!(
        driver.model().value,
        Some(value),
        "the value must be untouched"
    );
}

// ---------------------------------------------------------------------------
// Keyboard interaction
// ---------------------------------------------------------------------------

#[test]
fn test_time_picker_escape_cancels_only_an_open_flyout() {
    let value = CivilTime::new(10, 0, 0).unwrap();

    let mut open = Driver::new(wired(TimePickerModel::new(Some(value)).with_24_hour(true)));
    open.click(trailing_centre());
    open.drain();
    open.click(drum_centre(0, 2, 2));
    open.drain();
    open.key(Named::Escape);
    assert_eq!(
        open.drain(),
        vec![TestMsg::Cancelled(Some(value)), TestMsg::Opened(false),],
        "Escape must roll the edit back and report the close"
    );

    let mut closed = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    closed.set_focus();
    closed.key(Named::Escape);
    assert_eq!(
        closed.drain(),
        vec![],
        "Escape on an already closed picker must publish nothing"
    );
    assert!(
        !closed.model().open,
        "Escape must leave a closed picker closed"
    );
}

#[test]
fn test_time_picker_enter_opens_then_commits() {
    let value = CivilTime::new(10, 0, 0).unwrap();

    let mut closed = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    closed.set_focus();
    closed.key(Named::Enter);
    assert!(closed.model().open, "Enter must open the flyout");
    assert_eq!(
        closed.state().pre_open_snapshot,
        Some(Some(value)),
        "Enter must capture the pre-open snapshot"
    );
    assert_eq!(
        closed.drain(),
        vec![TestMsg::Opened(true)],
        "Enter must publish the live open state"
    );

    let mut open = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    open.key(Named::Enter);
    assert_eq!(
        open.drain(),
        vec![TestMsg::Committed(value), TestMsg::Opened(false),],
        "Enter on an open picker must commit and close"
    );

    let mut empty = Driver::new(wired(
        TimePickerModel::new(None)
            .with_24_hour(true)
            .with_open(true),
    ));
    empty.key(Named::Enter);
    assert_eq!(
        empty.drain(),
        vec![
            TestMsg::Committed(CivilTime::midnight()),
            TestMsg::Opened(false),
        ],
        "Enter on an unselected 24-hour picker must commit midnight"
    );
}

#[test]
fn test_time_picker_space_opens_only_when_closed() {
    let value = CivilTime::new(10, 0, 0).unwrap();

    let mut closed = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    closed.set_focus();
    closed.key(Named::Space);
    assert!(closed.model().open, "Space must open the flyout");
    assert_eq!(
        closed.drain(),
        vec![TestMsg::Opened(true)],
        "Space must publish the live open state"
    );

    let mut open = Driver::new(wired(TimePickerModel::new(Some(value)).with_open(true)));
    open.key(Named::Space);
    assert_eq!(
        open.drain(),
        vec![],
        "Space on an open picker must be inert"
    );
    assert!(open.model().open, "Space must not close the flyout");
}

#[test]
fn test_time_picker_arrow_keys_cycle_the_focused_segment() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::in_box(
        wired(TimePickerModel::new(Some(value)).with_24_hour(false)),
        closed_bounds(),
    );
    driver.set_focus();

    // A 12-hour trigger without seconds reads Hour -> Minute -> AM/PM -> Hour.
    const FORWARD: [TimeSegment; 4] = [
        TimeSegment::Minute,
        TimeSegment::AmPm,
        TimeSegment::Hour,
        TimeSegment::Minute,
    ];
    for want in FORWARD {
        driver.key(Named::ArrowRight);
        assert_eq!(
            driver.state().focused_segment,
            want,
            "ArrowRight must advance to {want:?}"
        );
        assert_eq!(
            driver.drain(),
            vec![],
            "moving the segment publishes nothing"
        );
    }

    const BACK: [TimeSegment; 3] = [TimeSegment::Hour, TimeSegment::AmPm, TimeSegment::Minute];
    for want in BACK {
        driver.key(Named::ArrowLeft);
        assert_eq!(
            driver.state().focused_segment,
            want,
            "ArrowLeft must retreat to {want:?}"
        );
    }
}

#[test]
fn test_time_picker_arrow_keys_step_the_focused_segment() {
    let value = CivilTime::new(10, 20, 30).unwrap();
    let mut driver = Driver::in_box(
        wired(TimePickerModel::new(Some(value)).with_24_hour(true)),
        closed_bounds(),
    );
    driver.set_focus();

    driver.key(Named::ArrowUp);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(CivilTime::new(11, 20, 30).unwrap())],
        "ArrowUp must step the focused hour forward"
    );
    driver.key(Named::ArrowDown);
    assert_eq!(
        driver.drain(),
        vec![TestMsg::Changed(value)],
        "ArrowDown must step the focused hour back"
    );

    // Stepping an unselected value starts from the mode's default, not from zero.
    let mut empty = Driver::in_box(
        wired(TimePickerModel::new(None).with_24_hour(true)),
        closed_bounds(),
    );
    empty.set_focus();
    empty.key(Named::ArrowUp);
    assert_eq!(
        empty.drain(),
        vec![TestMsg::Changed(CivilTime::new(1, 0, 0).unwrap())],
        "ArrowUp on an unselected 24-hour picker must start at midnight"
    );
}

#[test]
fn test_time_picker_keyboard_needs_focus_or_an_open_flyout() {
    let value = CivilTime::new(10, 0, 0).unwrap();

    // Closed and unfocused: the widget must not react at all.
    let mut idle = Driver::in_box(wired(TimePickerModel::new(Some(value))), closed_bounds());
    for key in [
        Named::ArrowUp,
        Named::ArrowDown,
        Named::ArrowLeft,
        Named::ArrowRight,
        Named::Enter,
        Named::Space,
        Named::Escape,
    ] {
        idle.key(key);
        assert_eq!(idle.drain(), vec![], "{key:?} must be ignored while idle");
    }
    assert_eq!(
        idle.model().value,
        Some(value),
        "the value must be untouched"
    );
    assert!(!idle.model().open, "no key may open an idle picker");

    // Open but unfocused: the flyout itself keeps the keyboard alive.
    let mut open = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));
    open.key(Named::ArrowUp);
    assert_eq!(
        open.drain(),
        vec![TestMsg::Changed(CivilTime::new(11, 0, 0).unwrap())],
        "an open flyout must accept arrows without focus"
    );
}

#[test]
fn test_time_picker_ignores_unhandled_keys() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(TimePickerModel::new(Some(value)).with_open(true)));
    driver.deliver(
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
        driver.drain(),
        vec![],
        "an ordinary character must publish nothing"
    );
    assert_eq!(driver.model().value, Some(value));
    assert!(
        driver.model().open,
        "an ordinary character must not close the flyout"
    );
}

#[test]
fn test_time_picker_disabled_model_ignores_every_event() {
    let value = CivilTime::new(10, 0, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true)
            .with_disabled(true),
    ));

    driver.hover(trailing_centre());
    driver.click(trailing_centre());
    driver.click(cancel_centre());
    driver.click(drum_centre(0, 2, 2));
    driver.scroll_lines(Point::new(100.0, 16.0), 1.0);
    driver.set_focus();
    driver.key(Named::Enter);
    driver.key(Named::ArrowUp);

    assert_eq!(
        driver.drain(),
        vec![],
        "a disabled picker must publish nothing at all"
    );
    assert_eq!(
        driver.model().value,
        Some(value),
        "a disabled picker must not change its value"
    );
    assert!(
        driver.model().open,
        "a disabled picker must not change its open state"
    );
    let state = driver.state();
    assert!(
        !state.hovered_trailing && !state.hovered_cancel && state.hovered_column.is_none(),
        "a disabled picker must not track hover at all"
    );
}

// ---------------------------------------------------------------------------
// Paint
// ---------------------------------------------------------------------------

#[test]
fn test_time_picker_paint_walks_hover_and_focus_cues() {
    let value = CivilTime::new(14, 30, 0).unwrap();
    let mut driver = Driver::new(wired(
        TimePickerModel::new(Some(value))
            .with_24_hour(true)
            .with_open(true),
    ));

    driver.hover(trailing_centre());
    assert!(driver.state().hovered_trailing);
    driver.paint();

    driver.hover(cancel_centre());
    assert!(driver.state().hovered_cancel);
    driver.paint();

    driver.hover(apply_centre());
    assert!(driver.state().hovered_apply);
    driver.paint();

    driver.hover(drum_centre(0, 2, 1));
    assert_eq!(driver.state().hovered_column, Some(0));
    assert_eq!(driver.state().hovered_row, Some(1));
    driver.paint();

    driver.set_focus();
    assert!(driver.state().focusable.is_focused());
    driver.paint();
}

#[test]
fn test_headless_render_time_picker_unselected_and_stepped_states() {
    // 1. Unselected, closed, 24-hour: the trigger shows the bare placeholder.
    let empty_24 = TimePickerModel::new(None).with_24_hour(true);
    let mut closed = Harness::<(), _>::headless(move || time_picker::<()>(empty_24.clone()).into());
    closed.render();

    // 2. Unselected and open in 12-hour mode with seconds: the drum must still
    //    centre on the default meridian rather than a selected value.
    let empty_12 = TimePickerModel::new(None)
        .with_show_seconds(true)
        .with_open(true);
    let mut open12 = Harness::<(), _>::headless(move || time_picker::<()>(empty_12.clone()).into());
    open12.render();

    // 3. A stepped value with a validation message, a wide minute increment and
    //    all four drum columns active.
    let stepped = TimePickerModel::new(Some(CivilTime::new(23, 59, 59).unwrap()))
        .with_24_hour(true)
        .with_show_seconds(true)
        .with_minute_increment(15)
        .with_open(true)
        .with_validation("Outside the booking window");
    let mut wide = Harness::<(), _>::headless(move || time_picker::<()>(stepped.clone()).into());
    wide.render();

    // 4. The same card with the message cleared again, so the footer is the only cue.
    let clean = TimePickerModel::new(Some(CivilTime::new(23, 59, 59).unwrap()))
        .with_24_hour(true)
        .with_open(true)
        .with_validation_opt(None);
    let mut plain = Harness::<(), _>::headless(move || time_picker::<()>(clean.clone()).into());
    plain.render();
}
