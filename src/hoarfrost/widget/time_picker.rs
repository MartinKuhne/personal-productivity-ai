//! The TimePicker widget with civil time arithmetic, segmented trigger, drum flyout and three-tier callbacks (§3.11, `UIF-ATOM-011`).
//!
//! A time picker pairs a 32px segmented monospace trigger box (`UIF-TOK-002`) with an
//! anchored 220px drum flyout card (§3.11). Features include:
//! - Pure civil wall-clock time arithmetic (zero external dependencies).
//! - 12-hour (with AM/PM) and 24-hour display and editing modes.
//! - Configurable minute stepping interval (1, 5, 10, 15, 30 minutes).
//! - Optional seconds display and editing.
//! - Monospace segmented trigger (`HH : MM [: SS] [AM/PM]`, 14px tabular numerals) with per-segment keyboard and wheel navigation (`UIF-PRIN-003`).
//! - 220x260px drum flyout card (`Z-Band 2`, `Escape Rank 3`) with column headers, parallel scrollable columns, central 32px selection band, and pinned 36px footer.
//! - Three-tier interaction callbacks ([`Live`], [`Commit`], [`Cancel`]) with pre-open snapshot restoration on Escape or Cancel (`UIF-PRIN-005`).
//!
//! Unit tests live in the sibling `time_picker_tests.rs` sidecar.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text as text_core;
use iced::advanced::widget::operation::{self, Focusable, Operation};
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::event::Event;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::widget;
use iced::{Background, Border, Color, Element, Length, Pixels, Point, Rectangle, Shadow, Size};

use crate::binding::snapshot::GestureSlot;
use crate::binding::{Cancel, Commit, Live};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::focus::{self, FocusPlacement};
use crate::widget::focusable::FocusableState;

// ---------------------------------------------------------------------------
// String constants (§3.11, [RUST-021])
// ---------------------------------------------------------------------------

/// Clock trigger icon glyph (§3.11, `UIF-ATOM-011`).
pub const CLOCK_ICON: &str = "🕒";

/// Colon separator string for time components (§3.11).
pub const TIME_SEP: &str = ":";

/// Colon separator with spacing for segmented trigger display (§3.11).
pub const TIME_SEP_SPACED: &str = " : ";

/// Space separator string (§3.11).
pub const SPACE_SEP: &str = " ";

/// Column header label for Hours (§3.11).
pub const LABEL_HR: &str = "Hr";

/// Column header label for Minutes (§3.11).
pub const LABEL_MIN: &str = "Min";

/// Column header label for Seconds (§3.11).
pub const LABEL_SEC: &str = "Sec";

/// Column header label for AM/PM (§3.11).
pub const LABEL_AM_PM: &str = "AM/PM";

/// String literal for AM (§3.11).
pub const LABEL_AM: &str = "AM";

/// String literal for PM (§3.11).
pub const LABEL_PM: &str = "PM";

/// String literal for Cancel action button (§3.11).
pub const LABEL_CANCEL: &str = "Cancel";

/// String literal for Apply action button (§3.11).
pub const LABEL_APPLY: &str = "✓ Apply";

/// Placeholder string for 24-hour time without seconds (§3.11).
pub const PLACEHOLDER_24H: &str = "--:--";

/// Placeholder string for 24-hour time with seconds (§3.11).
pub const PLACEHOLDER_24H_SEC: &str = "--:--:--";

/// Placeholder string for 12-hour time without seconds (§3.11).
pub const PLACEHOLDER_12H: &str = "--:-- --";

/// Placeholder string for 12-hour time with seconds (§3.11).
pub const PLACEHOLDER_12H_SEC: &str = "--:--:-- --";

// ---------------------------------------------------------------------------
// Layout & geometry constants (§3.11, UIF-ATOM-011, UIF-TOK-002, UIF-PRIN-001 §1.1)
// ---------------------------------------------------------------------------

/// Standard trigger control height — `size::CONTROL_MD` (32px) (§3.11, `UIF-ATOM-011`, `UIF-TOK-002`).
pub const TRIGGER_HEIGHT: f32 = size::CONTROL_MD;

/// Trailing clock toggle button hit-target size — `size::HIT_TARGET_MIN` (24px) (§3.11, `UIF-ATOM-011`, `UIF-ATOM-002b`).
pub const TRAILING_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Drum flyout fixed card width — 220px (§3.11, `UIF-ATOM-011`, `UIF-PRIN-001` §1.1).
pub const TIME_FLYOUT_WIDTH: f32 = 220.0;

/// Drum flyout fixed maximum card height — 260px (§3.11, `UIF-ATOM-011`, `UIF-PRIN-001` §1.1).
pub const TIME_FLYOUT_MAX_HEIGHT: f32 = 260.0;

/// Column header band height — `size::BAR_STATUS` (24px) (§3.11, `UIF-ATOM-011`).
pub const HEADER_HEIGHT: f32 = size::BAR_STATUS;

/// Drum column row height and central selection band height — `size::CONTROL_MD` (32px) (§3.11, `UIF-ATOM-011`).
pub const ROW_HEIGHT: f32 = size::CONTROL_MD;

/// Pinned footer action bar height — `size::CONTROL_LG` (36px) (§3.11, `UIF-ATOM-011`).
pub const FOOTER_HEIGHT: f32 = size::CONTROL_LG;

/// Action button height inside the footer — `size::CONTROL_SM` (28px) (§3.11, `UIF-ATOM-011`).
pub const FOOTER_BUTTON_HEIGHT: f32 = size::CONTROL_SM;

/// Flyout card border radius — `shape::RADIUS_CARD` (6px) (§3.11, `UIF-ATOM-011`).
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// Vertical hairline divider width — `shape::BORDER_HAIRLINE` (1px) (§3.11, `UIF-ATOM-011`).
pub const HAIRLINE_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Selection band vertical offset from drum top (§3.11, `UIF-ATOM-011`).
///
/// Drum height is `TIME_FLYOUT_MAX_HEIGHT - HEADER_HEIGHT - FOOTER_HEIGHT = 200px`.
/// The 32px selection band is centered at `(200 - 32) / 2 = 84px`.
pub const SELECTION_BAND_OFFSET_Y: f32 = 84.0;

// ---------------------------------------------------------------------------
// Civil Time Representation (§3.11, UIF-ATOM-011)
// ---------------------------------------------------------------------------

/// Meridian indicator (AM or PM) for 12-hour time representation (§3.11, `UIF-ATOM-011`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AmPm {
    /// Ante meridiem (before noon: 00:00:00 up to 11:59:59) (§3.11).
    #[default]
    Am,
    /// Post meridiem (after noon: 12:00:00 up to 23:59:59) (§3.11).
    Pm,
}

impl AmPm {
    /// Return standard two-character string representation ("AM" or "PM") (§3.11).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Am => LABEL_AM,
            Self::Pm => LABEL_PM,
        }
    }

    /// Toggle between AM and PM (§3.11).
    #[must_use]
    pub const fn toggle(&self) -> Self {
        match self {
            Self::Am => Self::Pm,
            Self::Pm => Self::Am,
        }
    }
}

impl std::fmt::Display for AmPm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A civil wall-clock time representation with second precision (§3.11, `UIF-ATOM-011`).
///
/// Invariants:
/// - `hour < 24`
/// - `minute < 60`
/// - `second < 60`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CivilTime {
    /// Hour of the day in 24-hour representation (`0..=23`).
    pub hour: u8,
    /// Minute of the hour (`0..=59`).
    pub minute: u8,
    /// Second of the minute (`0..=59`).
    pub second: u8,
}

impl CivilTime {
    /// Construct and validate a civil time representation (§3.11, `UIF-ATOM-011`).
    ///
    /// Returns `Some(CivilTime)` if `hour < 24`, `minute < 60`, and `second < 60`; otherwise `None`.
    #[must_use]
    pub fn new(hour: u8, minute: u8, second: u8) -> Option<Self> {
        if hour < 24 && minute < 60 && second < 60 {
            Some(Self {
                hour,
                minute,
                second,
            })
        } else {
            None
        }
    }

    /// Construct a civil time from hours, minutes, and seconds (§3.11).
    #[must_use]
    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Option<Self> {
        Self::new(hour, minute, second)
    }

    /// Construct a civil time from hours and minutes with `second = 0` (§3.11).
    #[must_use]
    pub fn from_hm(hour: u8, minute: u8) -> Option<Self> {
        Self::new(hour, minute, 0)
    }

    /// Construct midnight (`00:00:00`) (§3.11).
    #[must_use]
    pub const fn midnight() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    /// Construct noon (`12:00:00`) (§3.11).
    #[must_use]
    pub const fn noon() -> Self {
        Self {
            hour: 12,
            minute: 0,
            second: 0,
        }
    }

    /// Construct civil time from 12-hour values (`hour_12` in `1..=12`) and AM/PM (§3.11).
    #[must_use]
    pub fn from_12h(hour_12: u8, minute: u8, second: u8, am_pm: AmPm) -> Option<Self> {
        if !(1..=12).contains(&hour_12) || minute >= 60 || second >= 60 {
            return None;
        }
        let hour = match (hour_12, am_pm) {
            (12, AmPm::Am) => 0,
            (h, AmPm::Am) => h,
            (12, AmPm::Pm) => 12,
            (h, AmPm::Pm) => h + 12,
        };
        Some(Self {
            hour,
            minute,
            second,
        })
    }

    /// Meridian indicator (AM or PM) for this time (§3.11).
    #[must_use]
    pub const fn am_pm(&self) -> AmPm {
        if self.hour < 12 { AmPm::Am } else { AmPm::Pm }
    }

    /// 12-hour representation of the hour (`1..=12`) (§3.11).
    #[must_use]
    pub const fn hour_12(&self) -> u8 {
        match self.hour {
            0 => 12,
            1..=12 => self.hour,
            _ => self.hour - 12,
        }
    }

    /// Step the hour component by an integer delta with 24-hour wrap-around (§3.11).
    #[must_use]
    pub fn step_hour(&self, delta: i32) -> Self {
        let h = (self.hour as i32 + delta).rem_euclid(24) as u8;
        Self { hour: h, ..*self }
    }

    /// Step the minute component by an increment with 60-minute wrap-around (§3.11).
    #[must_use]
    pub fn step_minute(&self, delta: i32, increment: u8) -> Self {
        let inc = increment.max(1) as i32;
        let m = (self.minute as i32 + delta * inc).rem_euclid(60) as u8;
        Self { minute: m, ..*self }
    }

    /// Step the second component by an integer delta with 60-second wrap-around (§3.11).
    #[must_use]
    pub fn step_second(&self, delta: i32) -> Self {
        let s = (self.second as i32 + delta).rem_euclid(60) as u8;
        Self { second: s, ..*self }
    }

    /// Toggle AM/PM by advancing or retreating 12 hours (§3.11).
    #[must_use]
    pub fn toggle_am_pm(&self) -> Self {
        let h = (self.hour + 12) % 24;
        Self { hour: h, ..*self }
    }

    /// Step a specific segment value by delta (§3.11).
    #[must_use]
    pub fn step_segment(&self, segment: TimeSegment, delta: i32, increment: u8) -> Self {
        match segment {
            TimeSegment::Hour => self.step_hour(delta),
            TimeSegment::Minute => self.step_minute(delta, increment),
            TimeSegment::Second => self.step_second(delta),
            TimeSegment::AmPm => self.toggle_am_pm(),
        }
    }

    /// Format time as 24-hour string `HH:MM` or `HH:MM:SS` (§3.11).
    #[must_use]
    pub fn to_string_24h(&self, show_seconds: bool) -> String {
        if show_seconds {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        } else {
            format!("{:02}:{:02}", self.hour, self.minute)
        }
    }

    /// Format time as 12-hour string `HH:MM AM/PM` or `HH:MM:SS AM/PM` (§3.11).
    #[must_use]
    pub fn to_string_12h(&self, show_seconds: bool) -> String {
        if show_seconds {
            format!(
                "{:02}:{:02}:{:02} {}",
                self.hour_12(),
                self.minute,
                self.second,
                self.am_pm().as_str()
            )
        } else {
            format!(
                "{:02}:{:02} {}",
                self.hour_12(),
                self.minute,
                self.am_pm().as_str()
            )
        }
    }

    /// Format time according to configured display flags (§3.11).
    #[must_use]
    pub fn format_display(&self, use_24_hour: bool, show_seconds: bool) -> String {
        if use_24_hour {
            self.to_string_24h(show_seconds)
        } else {
            self.to_string_12h(show_seconds)
        }
    }

    /// Parse civil time from common 24h or 12h formatted strings (§3.11).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let upper = s.to_ascii_uppercase();
        let (time_part, am_pm_opt) = if upper.ends_with(LABEL_AM) {
            (upper.trim_end_matches(LABEL_AM).trim(), Some(AmPm::Am))
        } else if upper.ends_with(LABEL_PM) {
            (upper.trim_end_matches(LABEL_PM).trim(), Some(AmPm::Pm))
        } else {
            (s, None)
        };

        let parts: Vec<&str> = time_part.split(':').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }

        let hour: u8 = parts[0].trim().parse().ok()?;
        let minute: u8 = if parts.len() > 1 {
            parts[1].trim().parse().ok()?
        } else {
            0
        };
        let second: u8 = if parts.len() > 2 {
            parts[2].trim().parse().ok()?
        } else {
            0
        };

        if let Some(am_pm) = am_pm_opt {
            Self::from_12h(hour, minute, second, am_pm)
        } else {
            Self::new(hour, minute, second)
        }
    }
}

impl std::fmt::Display for CivilTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_24h(false))
    }
}

// ---------------------------------------------------------------------------
// Time Segment Navigation (§3.11, UIF-ATOM-011)
// ---------------------------------------------------------------------------

/// Time component segments within the segmented trigger input (§3.11, `UIF-ATOM-011`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TimeSegment {
    /// Hour segment (`HH` or `H`) (§3.11).
    #[default]
    Hour,
    /// Minute segment (`MM`) (§3.11).
    Minute,
    /// Optional second segment (`SS`) (§3.11).
    Second,
    /// Optional meridian segment (`AM` or `PM`) (§3.11).
    AmPm,
}

impl TimeSegment {
    /// Get the next segment in reading order based on configuration (§3.11).
    #[must_use]
    pub fn next(&self, use_24_hour: bool, show_seconds: bool) -> Self {
        match self {
            Self::Hour => Self::Minute,
            Self::Minute => {
                if show_seconds {
                    Self::Second
                } else if !use_24_hour {
                    Self::AmPm
                } else {
                    Self::Hour
                }
            }
            Self::Second => {
                if !use_24_hour {
                    Self::AmPm
                } else {
                    Self::Hour
                }
            }
            Self::AmPm => Self::Hour,
        }
    }

    /// Get the previous segment in reading order based on configuration (§3.11).
    #[must_use]
    pub fn prev(&self, use_24_hour: bool, show_seconds: bool) -> Self {
        match self {
            Self::Hour => {
                if !use_24_hour {
                    Self::AmPm
                } else if show_seconds {
                    Self::Second
                } else {
                    Self::Minute
                }
            }
            Self::Minute => Self::Hour,
            Self::Second => Self::Minute,
            Self::AmPm => {
                if show_seconds {
                    Self::Second
                } else {
                    Self::Minute
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TimePickerModel (§3.11, UIF-ATOM-011)
// ---------------------------------------------------------------------------

/// Declarative state model for the [`TimePicker`] widget (§3.11, `UIF-ATOM-011`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimePickerModel {
    /// Currently selected civil time, or `None` if unselected (§3.11).
    pub value: Option<CivilTime>,
    /// Whether to use 24-hour format instead of 12-hour AM/PM (§3.11).
    pub use_24_hour: bool,
    /// Whether to display and adjust seconds (§3.11).
    pub show_seconds: bool,
    /// Minute increment stepping interval (1, 5, 10, 15, 30) (§3.11).
    pub minute_increment: u8,
    /// Whether the drum flyout card is open (§3.11).
    pub open: bool,
    /// Optional validation message or error description (§3.11).
    pub validation: Option<String>,
    /// Whether the control is disabled (§3.11).
    pub disabled: bool,
    /// Pre-open snapshot for Escape rollback cancellation (§3.11).
    pub pre_open_snapshot: Option<Option<CivilTime>>,
}

impl Default for TimePickerModel {
    fn default() -> Self {
        Self::new(None)
    }
}

impl TimePickerModel {
    /// Construct a new `TimePickerModel` with an optional initial value (§3.11, `UIF-ATOM-011`).
    #[must_use]
    pub fn new(value: Option<CivilTime>) -> Self {
        Self {
            value,
            use_24_hour: false,
            show_seconds: false,
            minute_increment: 1,
            open: false,
            validation: None,
            disabled: false,
            pre_open_snapshot: None,
        }
    }

    /// Set whether 24-hour format is enabled (§3.11).
    #[must_use]
    pub fn with_24_hour(mut self, use_24_hour: bool) -> Self {
        self.use_24_hour = use_24_hour;
        self
    }

    /// Set whether seconds are displayed and editable (§3.11).
    #[must_use]
    pub fn with_show_seconds(mut self, show_seconds: bool) -> Self {
        self.show_seconds = show_seconds;
        self
    }

    /// Set minute increment stepping interval (validated: 1, 5, 10, 15, 30) (§3.11).
    #[must_use]
    pub fn with_minute_increment(mut self, minute_increment: u8) -> Self {
        self.minute_increment = match minute_increment {
            1 | 5 | 10 | 15 | 30 => minute_increment,
            _ => 1,
        };
        self
    }

    /// Set whether the flyout card is open (§3.11).
    #[must_use]
    pub fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set validation error message (§3.11).
    #[must_use]
    pub fn with_validation(mut self, validation: impl Into<String>) -> Self {
        self.validation = Some(validation.into());
        self
    }

    /// Clear or set optional validation error message (§3.11).
    #[must_use]
    pub fn with_validation_opt(mut self, validation: Option<String>) -> Self {
        self.validation = validation;
        self
    }

    /// Set disabled state (§3.11).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Open the flyout card, capturing a snapshot of the current value (§3.11).
    pub fn open_flyout(&mut self) {
        if !self.disabled {
            self.pre_open_snapshot = Some(self.value);
            self.open = true;
        }
    }

    /// Close the flyout card without reverting (§3.11).
    pub fn close_flyout(&mut self) {
        self.open = false;
    }

    /// Cancel flyout interaction, restoring the pre-open snapshot (§3.11).
    pub fn cancel(&mut self) -> Option<CivilTime> {
        let restored = self.pre_open_snapshot.take().unwrap_or(self.value);
        self.value = restored;
        self.open = false;
        restored
    }

    /// Commit the current value and close the flyout (§3.11).
    pub fn commit(&mut self) {
        self.pre_open_snapshot = None;
        self.open = false;
    }

    /// Step a segment value by delta, initializing to default if unselected (§3.11).
    pub fn step_segment(&mut self, segment: TimeSegment, delta: i32) {
        let current = self.value.unwrap_or_else(|| {
            if self.use_24_hour {
                CivilTime::midnight()
            } else {
                CivilTime::noon()
            }
        });
        self.value = Some(current.step_segment(segment, delta, self.minute_increment));
    }

    /// Select a specific time value (§3.11).
    pub fn select_time(&mut self, time: CivilTime) {
        self.value = Some(time);
    }

    /// Format display string for trigger input (§3.11).
    #[must_use]
    pub fn format_display(&self) -> String {
        match self.value {
            Some(time) => time.format_display(self.use_24_hour, self.show_seconds),
            None => {
                if self.use_24_hour {
                    if self.show_seconds {
                        PLACEHOLDER_24H_SEC.to_owned()
                    } else {
                        PLACEHOLDER_24H.to_owned()
                    }
                } else if self.show_seconds {
                    PLACEHOLDER_12H_SEC.to_owned()
                } else {
                    PLACEHOLDER_12H.to_owned()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Drum Column Layout & Navigation (§3.11, UIF-ATOM-011)
// ---------------------------------------------------------------------------

/// Column kind in the drum flyout card (§3.11, `UIF-ATOM-011`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumColumnKind {
    /// Hour column (`0..=23` or `1..=12`) (§3.11).
    Hour,
    /// Minute column (stepped by `minute_increment`) (§3.11).
    Minute,
    /// Second column (`0..=59`) (§3.11).
    Second,
    /// Meridian column (`AM` / `PM`) (§3.11).
    AmPm,
}

impl DrumColumnKind {
    /// Column header label (§3.11).
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Hour => LABEL_HR,
            Self::Minute => LABEL_MIN,
            Self::Second => LABEL_SEC,
            Self::AmPm => LABEL_AM_PM,
        }
    }
}

/// Active columns for the given model configuration (§3.11).
#[must_use]
pub fn active_columns(use_24_hour: bool, show_seconds: bool) -> Vec<DrumColumnKind> {
    let mut cols = vec![DrumColumnKind::Hour, DrumColumnKind::Minute];
    if show_seconds {
        cols.push(DrumColumnKind::Second);
    }
    if !use_24_hour {
        cols.push(DrumColumnKind::AmPm);
    }
    cols
}

// ---------------------------------------------------------------------------
// TimePicker Widget (§3.11, UIF-ATOM-011)
// ---------------------------------------------------------------------------

/// Callback closure type for open state changes.
pub type OnOpenChange<'a, Message> = Box<dyn Fn(Live<bool>) -> Message + 'a>;

/// Callback closure type for live time modifications.
pub type OnChange<'a, Message> = Box<dyn Fn(Live<CivilTime>) -> Message + 'a>;

/// Callback closure type for time commits.
pub type OnCommit<'a, Message> = Box<dyn Fn(Commit<CivilTime>) -> Message + 'a>;

/// Callback closure type for time cancellations.
pub type OnCancel<'a, Message> = Box<dyn Fn(Cancel<Option<CivilTime>>) -> Message + 'a>;

/// Interactive segmented TimePicker widget (§3.11, `UIF-ATOM-011`).
pub struct TimePicker<'a, Message> {
    widget_id: widget::Id,
    model: TimePickerModel,
    palette: Palette,
    on_open_change: Option<OnOpenChange<'a, Message>>,
    on_change: Option<OnChange<'a, Message>>,
    on_commit: Option<OnCommit<'a, Message>>,
    on_cancel: Option<OnCancel<'a, Message>>,
}

impl<'a, Message> TimePicker<'a, Message> {
    /// Construct a new TimePicker from a model (§3.11, `UIF-ATOM-011`).
    #[must_use]
    pub fn new(model: TimePickerModel) -> Self {
        Self {
            widget_id: widget::Id::unique(),
            model,
            palette: Palette::dark(),
            on_open_change: None,
            on_change: None,
            on_commit: None,
            on_cancel: None,
        }
    }

    /// Set theme color palette (§3.11).
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set callback fired when flyout open state changes (§3.11).
    #[must_use]
    pub fn on_open_change(mut self, f: impl Fn(Live<bool>) -> Message + 'a) -> Self {
        self.on_open_change = Some(Box::new(f));
        self
    }

    /// Set callback fired on live time edits (arrow spin or column scroll) (§3.11).
    #[must_use]
    pub fn on_change(mut self, f: impl Fn(Live<CivilTime>) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Set callback fired when value is committed (Enter or Apply) (§3.11).
    #[must_use]
    pub fn on_commit(mut self, f: impl Fn(Commit<CivilTime>) -> Message + 'a) -> Self {
        self.on_commit = Some(Box::new(f));
        self
    }

    /// Set callback fired when interaction is cancelled (Escape or Cancel) (§3.11).
    #[must_use]
    pub fn on_cancel(mut self, f: impl Fn(Cancel<Option<CivilTime>>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(f));
        self
    }

    /// Access the underlying model (§3.11).
    #[must_use]
    pub const fn model(&self) -> &TimePickerModel {
        &self.model
    }

    /// Access the underlying model mutably (§3.11).
    pub fn model_mut(&mut self) -> &mut TimePickerModel {
        &mut self.model
    }

    /// Natural layout size (§3.11).
    #[must_use]
    pub fn size(&self) -> Size<Length> {
        let h = if self.model.open {
            TRIGGER_HEIGHT + space::XS + TIME_FLYOUT_MAX_HEIGHT
        } else {
            TRIGGER_HEIGHT
        };
        Size::new(Length::Fixed(TIME_FLYOUT_WIDTH), Length::Fixed(h))
    }
}

/// Local interaction state for the [`TimePicker`] widget (§3.11, `UIF-ATOM-011`).
#[derive(Debug, Clone, Default)]
pub struct TimePickerState {
    /// Keyboard focus tracking.
    pub focusable: FocusableState,
    /// Currently focused segment within the segmented trigger input.
    pub focused_segment: TimeSegment,
    /// Hovered state for trailing clock toggle button.
    pub hovered_trailing: bool,
    /// Hovered state for [Cancel] footer button.
    pub hovered_cancel: bool,
    /// Hovered state for [✓ Apply] footer button.
    pub hovered_apply: bool,
    /// Hovered column index in the drum flyout.
    pub hovered_column: Option<usize>,
    /// Hovered row offset in the drum flyout (-2..=3).
    pub hovered_row: Option<i32>,
    /// Pre-open snapshot for Escape rollback cancellation.
    pub pre_open_snapshot: Option<Option<CivilTime>>,
    /// Gesture slot for Live/Commit/Cancel transitions.
    pub gesture: GestureSlot<CivilTime>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TimePicker<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TimePickerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TimePickerState::default())
    }

    fn size(&self) -> Size<Length> {
        TimePicker::size(self)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let w = TIME_FLYOUT_WIDTH;
        let h = if self.model.open {
            TRIGGER_HEIGHT + space::XS + TIME_FLYOUT_MAX_HEIGHT
        } else {
            TRIGGER_HEIGHT
        };
        let size = limits.resolve(Length::Fixed(w), Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<TimePickerState>();
        if !self.model.disabled {
            let trigger_bounds = Rectangle {
                x: layout.bounds().x,
                y: layout.bounds().y,
                width: layout.bounds().width,
                height: TRIGGER_HEIGHT,
            };
            operation.focusable(Some(&self.widget_id), trigger_bounds, &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.model.disabled {
            return mouse::Interaction::None;
        }
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if self.model.disabled {
            return;
        }

        let state = tree.state.downcast_mut::<TimePickerState>();
        let bounds = layout.bounds();
        let trigger_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: TRIGGER_HEIGHT,
        };

        let flyout_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y + TRIGGER_HEIGHT + space::XS,
            width: TIME_FLYOUT_WIDTH,
            height: TIME_FLYOUT_MAX_HEIGHT,
        };

        let footer_y = flyout_bounds.y + TIME_FLYOUT_MAX_HEIGHT - FOOTER_HEIGHT;
        let btn_w = (TIME_FLYOUT_WIDTH - 3.0 * space::SM) / 2.0;
        let cancel_bounds = Rectangle {
            x: flyout_bounds.x + space::SM,
            y: footer_y + (FOOTER_HEIGHT - FOOTER_BUTTON_HEIGHT) / 2.0,
            width: btn_w,
            height: FOOTER_BUTTON_HEIGHT,
        };
        let apply_bounds = Rectangle {
            x: flyout_bounds.x + TIME_FLYOUT_WIDTH - space::SM - btn_w,
            y: footer_y + (FOOTER_HEIGHT - FOOTER_BUTTON_HEIGHT) / 2.0,
            width: btn_w,
            height: FOOTER_BUTTON_HEIGHT,
        };

        let trailing_bounds = Rectangle {
            x: trigger_bounds.x + trigger_bounds.width - TRAILING_BUTTON_SIZE - space::SM,
            y: trigger_bounds.y + (TRIGGER_HEIGHT - TRAILING_BUTTON_SIZE) / 2.0,
            width: TRAILING_BUTTON_SIZE,
            height: TRAILING_BUTTON_SIZE,
        };

        let cols = active_columns(self.model.use_24_hour, self.model.show_seconds);
        let num_cols = cols.len();
        let col_w = (TIME_FLYOUT_WIDTH - (num_cols.saturating_sub(1) as f32 * HAIRLINE_WIDTH))
            / num_cols as f32;
        let drum_top = flyout_bounds.y + HEADER_HEIGHT;
        let drum_height = TIME_FLYOUT_MAX_HEIGHT - HEADER_HEIGHT - FOOTER_HEIGHT;

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    state.hovered_trailing = trailing_bounds.contains(pos);

                    if self.model.open && flyout_bounds.contains(pos) {
                        state.hovered_cancel = cancel_bounds.contains(pos);
                        state.hovered_apply = apply_bounds.contains(pos);

                        let drum_bounds = Rectangle {
                            x: flyout_bounds.x,
                            y: drum_top,
                            width: TIME_FLYOUT_WIDTH,
                            height: drum_height,
                        };

                        if drum_bounds.contains(pos) {
                            let rel_x = pos.x - flyout_bounds.x;
                            let col_idx = (rel_x / (col_w + HAIRLINE_WIDTH)).floor() as usize;
                            state.hovered_column = if col_idx < num_cols {
                                Some(col_idx)
                            } else {
                                None
                            };

                            let rel_y = pos.y - (drum_top + SELECTION_BAND_OFFSET_Y);
                            let row_offset = (rel_y / ROW_HEIGHT).floor() as i32;
                            state.hovered_row = Some(row_offset);
                        } else {
                            state.hovered_column = None;
                            state.hovered_row = None;
                        }
                    } else {
                        state.hovered_cancel = false;
                        state.hovered_apply = false;
                        state.hovered_column = None;
                        state.hovered_row = None;
                    }
                } else {
                    state.hovered_trailing = false;
                    state.hovered_cancel = false;
                    state.hovered_apply = false;
                    state.hovered_column = None;
                    state.hovered_row = None;
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if trailing_bounds.contains(pos) {
                        let will_open = !self.model.open;
                        if will_open {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                        } else {
                            self.model.close_flyout();
                        }
                        if let Some(cb) = &self.on_open_change {
                            shell.publish(cb(Live::new(will_open)));
                        }
                        shell.request_redraw();
                    } else if trigger_bounds.contains(pos) {
                        state.focusable.focus();
                        let text_left = trigger_bounds.x + space::MD;
                        let rel_x = pos.x - text_left;
                        let seg_w = 28.0;
                        if rel_x < seg_w {
                            state.focused_segment = TimeSegment::Hour;
                        } else if rel_x < seg_w * 2.0 + 16.0 {
                            state.focused_segment = TimeSegment::Minute;
                        } else if self.model.show_seconds && rel_x < seg_w * 3.0 + 32.0 {
                            state.focused_segment = TimeSegment::Second;
                        } else if !self.model.use_24_hour {
                            state.focused_segment = TimeSegment::AmPm;
                        }
                        shell.request_redraw();
                    } else if self.model.open && flyout_bounds.contains(pos) {
                        if cancel_bounds.contains(pos) {
                            let restored = self.model.cancel();
                            if let Some(cb) = &self.on_cancel {
                                shell.publish(cb(Cancel::new(restored)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        } else if apply_bounds.contains(pos) {
                            let current = self.model.value.unwrap_or_else(|| {
                                if self.model.use_24_hour {
                                    CivilTime::midnight()
                                } else {
                                    CivilTime::noon()
                                }
                            });
                            self.model.value = Some(current);
                            self.model.commit();
                            if let Some(cb) = &self.on_commit {
                                shell.publish(cb(Commit::new(current)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        } else {
                            let drum_bounds = Rectangle {
                                x: flyout_bounds.x,
                                y: drum_top,
                                width: TIME_FLYOUT_WIDTH,
                                height: drum_height,
                            };
                            if drum_bounds.contains(pos) {
                                let rel_x = pos.x - flyout_bounds.x;
                                let col_idx = (rel_x / (col_w + HAIRLINE_WIDTH)).floor() as usize;
                                let rel_y = pos.y - (drum_top + SELECTION_BAND_OFFSET_Y);
                                let row_offset = (rel_y / ROW_HEIGHT).floor() as i32;

                                if col_idx < num_cols {
                                    let col_kind = cols[col_idx];
                                    let current = self.model.value.unwrap_or_else(|| {
                                        if self.model.use_24_hour {
                                            CivilTime::midnight()
                                        } else {
                                            CivilTime::noon()
                                        }
                                    });

                                    let new_time = match col_kind {
                                        DrumColumnKind::Hour => current.step_hour(row_offset),
                                        DrumColumnKind::Minute => current
                                            .step_minute(row_offset, self.model.minute_increment),
                                        DrumColumnKind::Second => current.step_second(row_offset),
                                        DrumColumnKind::AmPm => {
                                            if row_offset != 0 {
                                                current.toggle_am_pm()
                                            } else {
                                                current
                                            }
                                        }
                                    };

                                    self.model.value = Some(new_time);
                                    if let Some(cb) = &self.on_change {
                                        shell.publish(cb(Live::new(new_time)));
                                    }
                                    shell.request_redraw();
                                }
                            }
                        }
                    } else if self.model.open {
                        let restored = self.model.cancel();
                        if let Some(cb) = &self.on_cancel {
                            shell.publish(cb(Cancel::new(restored)));
                        }
                        if let Some(cb) = &self.on_open_change {
                            shell.publish(cb(Live::new(false)));
                        }
                        shell.request_redraw();
                    }
                }
            }

            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if let Some(pos) = cursor.position() {
                    let scroll_delta = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => {
                            if *y > 0.0 {
                                1
                            } else if *y < 0.0 {
                                -1
                            } else {
                                0
                            }
                        }
                        mouse::ScrollDelta::Pixels { y, .. } => {
                            if *y > 0.0 {
                                1
                            } else if *y < 0.0 {
                                -1
                            } else {
                                0
                            }
                        }
                    };

                    if scroll_delta != 0 {
                        if trigger_bounds.contains(pos) {
                            self.model.step_segment(state.focused_segment, scroll_delta);
                            if let Some(new_time) = self.model.value
                                && let Some(cb) = &self.on_change
                            {
                                shell.publish(cb(Live::new(new_time)));
                            }
                            shell.request_redraw();
                        } else if self.model.open && flyout_bounds.contains(pos) {
                            let rel_x = pos.x - flyout_bounds.x;
                            let col_idx = (rel_x / (col_w + HAIRLINE_WIDTH)).floor() as usize;
                            if col_idx < num_cols {
                                let col_kind = cols[col_idx];
                                let current = self.model.value.unwrap_or_else(|| {
                                    if self.model.use_24_hour {
                                        CivilTime::midnight()
                                    } else {
                                        CivilTime::noon()
                                    }
                                });
                                let new_time = match col_kind {
                                    DrumColumnKind::Hour => current.step_hour(-scroll_delta),
                                    DrumColumnKind::Minute => current
                                        .step_minute(-scroll_delta, self.model.minute_increment),
                                    DrumColumnKind::Second => current.step_second(-scroll_delta),
                                    DrumColumnKind::AmPm => current.toggle_am_pm(),
                                };
                                self.model.value = Some(new_time);
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(new_time)));
                                }
                                shell.request_redraw();
                            }
                        }
                    }
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() || self.model.open =>
            {
                match key {
                    Key::Named(Named::Escape) => {
                        if self.model.open {
                            let restored = self.model.cancel();
                            if let Some(cb) = &self.on_cancel {
                                shell.publish(cb(Cancel::new(restored)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if self.model.open {
                            let current = self.model.value.unwrap_or_else(|| {
                                if self.model.use_24_hour {
                                    CivilTime::midnight()
                                } else {
                                    CivilTime::noon()
                                }
                            });
                            self.model.value = Some(current);
                            self.model.commit();
                            if let Some(cb) = &self.on_commit {
                                shell.publish(cb(Commit::new(current)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        } else {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(true)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::Space) => {
                        if !self.model.open {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(true)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::ArrowLeft) => {
                        state.focused_segment = state
                            .focused_segment
                            .prev(self.model.use_24_hour, self.model.show_seconds);
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowRight) => {
                        state.focused_segment = state
                            .focused_segment
                            .next(self.model.use_24_hour, self.model.show_seconds);
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowUp) => {
                        self.model.step_segment(state.focused_segment, 1);
                        if let Some(new_time) = self.model.value
                            && let Some(cb) = &self.on_change
                        {
                            shell.publish(cb(Live::new(new_time)));
                        }
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowDown) => {
                        self.model.step_segment(state.focused_segment, -1);
                        if let Some(new_time) = self.model.value
                            && let Some(cb) = &self.on_change
                        {
                            shell.publish(cb(Live::new(new_time)));
                        }
                        shell.request_redraw();
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<TimePickerState>();
        let bounds = layout.bounds();

        let trigger_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: TRIGGER_HEIGHT,
        };

        // 1. Draw Trigger Surface (§3.11, UIF-ATOM-011)
        let is_invalid = self.model.validation.is_some();
        let border_color = if self.model.disabled {
            self.palette.border_subtle
        } else if is_invalid {
            self.palette.status_danger_text
        } else if state.focusable.is_focused() {
            self.palette.accent_border
        } else {
            self.palette.border_subtle
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: trigger_bounds,
                border: Border::default()
                    .color(border_color)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_active),
        );

        // Focus indicator ring (§3.0.2, UIF-TOK-006)
        if state.focusable.is_focused() {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(trigger_bounds),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(shape::RADIUS_WIDGET)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // Monospace Segmented Trigger Display (§3.11)
        let display_text = self.model.format_display();
        let text_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.text_strong
        };

        let text_area = Rectangle {
            x: trigger_bounds.x + space::MD,
            y: trigger_bounds.y,
            width: trigger_bounds.width - space::MD - TRAILING_BUTTON_SIZE - space::SM,
            height: trigger_bounds.height,
        };

        let mono_font = iced::Font::MONOSPACE;

        renderer.fill_text(
            text_core::Text {
                content: display_text,
                bounds: text_area.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(text_area.height)),
                font: mono_font,
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(text_area.x, text_area.center_y()),
            text_color,
            *viewport,
        );

        // Trailing Clock Button (§3.11, UIF-ATOM-002b)
        let trailing_bounds = Rectangle {
            x: trigger_bounds.x + trigger_bounds.width - TRAILING_BUTTON_SIZE - space::SM,
            y: trigger_bounds.y + (TRIGGER_HEIGHT - TRAILING_BUTTON_SIZE) / 2.0,
            width: TRAILING_BUTTON_SIZE,
            height: TRAILING_BUTTON_SIZE,
        };

        let icon_color = if self.model.disabled {
            self.palette.text_disabled
        } else if state.hovered_trailing || self.model.open {
            self.palette.text_strong
        } else {
            self.palette.text_muted
        };

        renderer.fill_text(
            text_core::Text {
                content: CLOCK_ICON.to_owned(),
                bounds: trailing_bounds.size(),
                size: Pixels(size::ICON_SM),
                line_height: text_core::LineHeight::Absolute(Pixels(trailing_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            trailing_bounds.center(),
            icon_color,
            *viewport,
        );

        // 2. Draw Drum Flyout Card when Open (§3.11, Z-Band 2, Escape Rank 3)
        if !self.model.open {
            return;
        }

        let flyout_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y + TRIGGER_HEIGHT + space::XS,
            width: TIME_FLYOUT_WIDTH,
            height: TIME_FLYOUT_MAX_HEIGHT,
        };

        // Flyout Card Container
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout_bounds,
                border: Border::default()
                    .color(self.palette.border_strong)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_overlay),
        );

        // Column Header Band (24px height, surface.raised, border.subtle bottom) (§3.11)
        let header_bounds = Rectangle {
            x: flyout_bounds.x,
            y: flyout_bounds.y,
            width: TIME_FLYOUT_WIDTH,
            height: HEADER_HEIGHT,
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: header_bounds,
                border: Border::default()
                    .color(self.palette.border_subtle)
                    .width(shape::BORDER_HAIRLINE),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_raised),
        );

        let cols = active_columns(self.model.use_24_hour, self.model.show_seconds);
        let num_cols = cols.len();
        let col_w = (TIME_FLYOUT_WIDTH - (num_cols.saturating_sub(1) as f32 * HAIRLINE_WIDTH))
            / num_cols as f32;

        for (i, col) in cols.iter().enumerate() {
            let col_x = flyout_bounds.x + i as f32 * (col_w + HAIRLINE_WIDTH);
            let col_header_bounds = Rectangle {
                x: col_x,
                y: flyout_bounds.y,
                width: col_w,
                height: HEADER_HEIGHT,
            };

            renderer.fill_text(
                text_core::Text {
                    content: col.label().to_owned(),
                    bounds: col_header_bounds.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: text_core::LineHeight::Absolute(Pixels(HEADER_HEIGHT)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                col_header_bounds.center(),
                self.palette.text_muted,
                *viewport,
            );

            // Vertical Hairline Separator between columns
            if i + 1 < num_cols {
                let hairline_bounds = Rectangle {
                    x: col_x + col_w,
                    y: flyout_bounds.y + HEADER_HEIGHT,
                    width: HAIRLINE_WIDTH,
                    height: TIME_FLYOUT_MAX_HEIGHT - HEADER_HEIGHT - FOOTER_HEIGHT,
                };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: hairline_bounds,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(self.palette.border_strong),
                );
            }
        }

        // Central Selection Band (32px, surface.active, 1px accent.border) (§3.11)
        let drum_top = flyout_bounds.y + HEADER_HEIGHT;
        let band_bounds = Rectangle {
            x: flyout_bounds.x,
            y: drum_top + SELECTION_BAND_OFFSET_Y,
            width: TIME_FLYOUT_WIDTH,
            height: ROW_HEIGHT,
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: band_bounds,
                border: Border::default()
                    .color(self.palette.accent_border)
                    .width(shape::BORDER_HAIRLINE),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_active),
        );

        // Render Drum Column Values (§3.11)
        let current_time = self.model.value.unwrap_or_else(|| {
            if self.model.use_24_hour {
                CivilTime::midnight()
            } else {
                CivilTime::noon()
            }
        });

        for (col_idx, col) in cols.iter().enumerate() {
            let col_x = flyout_bounds.x + col_idx as f32 * (col_w + HAIRLINE_WIDTH);

            for offset in -2..=3 {
                let row_y = band_bounds.y + offset as f32 * ROW_HEIGHT;
                let cell_bounds = Rectangle {
                    x: col_x,
                    y: row_y,
                    width: col_w,
                    height: ROW_HEIGHT,
                };

                let cell_text = match col {
                    DrumColumnKind::Hour => {
                        let h = current_time.step_hour(offset);
                        if self.model.use_24_hour {
                            format!("{:02}", h.hour)
                        } else {
                            format!("{:02}", h.hour_12())
                        }
                    }
                    DrumColumnKind::Minute => {
                        let m = current_time.step_minute(offset, self.model.minute_increment);
                        format!("{:02}", m.minute)
                    }
                    DrumColumnKind::Second => {
                        let s = current_time.step_second(offset);
                        format!("{:02}", s.second)
                    }
                    DrumColumnKind::AmPm => {
                        if offset == 0 {
                            current_time.am_pm().as_str().to_owned()
                        } else {
                            current_time.am_pm().toggle().as_str().to_owned()
                        }
                    }
                };

                let cell_color = if offset == 0 {
                    self.palette.text_strong
                } else {
                    self.palette.text_muted
                };

                renderer.fill_text(
                    text_core::Text {
                        content: cell_text,
                        bounds: cell_bounds.size(),
                        size: Pixels(typography::BODY.size),
                        line_height: text_core::LineHeight::Absolute(Pixels(ROW_HEIGHT)),
                        font: mono_font,
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    cell_bounds.center(),
                    cell_color,
                    *viewport,
                );
            }
        }

        // Pinned Bottom Footer Bar (36px, [Cancel] / [✓ Apply]) (§3.11)
        let footer_y = flyout_bounds.y + TIME_FLYOUT_MAX_HEIGHT - FOOTER_HEIGHT;
        let footer_bounds = Rectangle {
            x: flyout_bounds.x,
            y: footer_y,
            width: TIME_FLYOUT_WIDTH,
            height: FOOTER_HEIGHT,
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: footer_bounds,
                border: Border::default()
                    .color(self.palette.border_subtle)
                    .width(shape::BORDER_HAIRLINE),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_overlay),
        );

        let btn_w = (TIME_FLYOUT_WIDTH - 3.0 * space::SM) / 2.0;
        let cancel_bounds = Rectangle {
            x: flyout_bounds.x + space::SM,
            y: footer_y + (FOOTER_HEIGHT - FOOTER_BUTTON_HEIGHT) / 2.0,
            width: btn_w,
            height: FOOTER_BUTTON_HEIGHT,
        };
        let apply_bounds = Rectangle {
            x: flyout_bounds.x + TIME_FLYOUT_WIDTH - space::SM - btn_w,
            y: footer_y + (FOOTER_HEIGHT - FOOTER_BUTTON_HEIGHT) / 2.0,
            width: btn_w,
            height: FOOTER_BUTTON_HEIGHT,
        };

        // Draw Cancel Button
        let cancel_bg = if state.hovered_cancel {
            self.palette.surface_active
        } else {
            self.palette.surface_base
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: cancel_bounds,
                border: Border::default()
                    .color(self.palette.border_subtle)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(cancel_bg),
        );
        renderer.fill_text(
            text_core::Text {
                content: LABEL_CANCEL.to_owned(),
                bounds: cancel_bounds.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: text_core::LineHeight::Absolute(Pixels(FOOTER_BUTTON_HEIGHT)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            cancel_bounds.center(),
            if state.hovered_cancel {
                self.palette.text_strong
            } else {
                self.palette.text_muted
            },
            *viewport,
        );

        // Draw Apply Button
        let apply_bg = if state.hovered_apply {
            self.palette.accent_fill_active
        } else {
            self.palette.accent_fill
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: apply_bounds,
                border: Border::default()
                    .color(self.palette.accent_border)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(apply_bg),
        );
        renderer.fill_text(
            text_core::Text {
                content: LABEL_APPLY.to_owned(),
                bounds: apply_bounds.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: text_core::LineHeight::Absolute(Pixels(FOOTER_BUTTON_HEIGHT)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            apply_bounds.center(),
            self.palette.text_strong,
            *viewport,
        );
    }
}

/// Construct a [`TimePicker`] widget from a model (§3.11, `UIF-ATOM-011`).
#[must_use]
pub fn time_picker<'a, Message>(model: TimePickerModel) -> TimePicker<'a, Message> {
    TimePicker::new(model)
}

impl<'a, Message, Theme, Renderer> From<TimePicker<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(picker: TimePicker<'a, Message>) -> Self {
        Element::new(picker)
    }
}

#[cfg(test)]
#[path = "time_picker_tests.rs"]
mod time_picker_tests;
