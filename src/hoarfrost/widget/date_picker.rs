//! The DatePicker, DateRangePicker & CalendarView widgets (§3.10, `UIF-ATOM-010`).
//!
//! A date picker pairs a 32px trigger box (`UIF-TOK-002`) with an anchored 256px
//! calendar flyout card (§3.10). The calendar view features:
//! - Pure civil calendar arithmetic (zero external dependencies).
//! - Fixed 6x7 day grid (exactly 42 cells) ensuring 0px layout shift across
//!   all 28-day, 29-day, 30-day, and 31-day months (`UIF-PRIN-003`).
//! - Redundant visual cues for today, selection endpoints, in-range interior,
//!   and blackout/out-of-bounds dates (`UIF-PRIN-005`).
//! - Drill-down views (`Days` -> `Months` -> `Years`).
//! - Full single-date and two-click range selection with hover preview.
//! - Three-tier interaction callbacks ([`Live`], [`Commit`], [`Cancel`]).
//!
//! Unit tests live in the sibling `date_picker_tests.rs` sidecar.

use std::collections::BTreeSet;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text as text_core;
use iced::advanced::widget::operation::{self, Operation};
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
// String constants (§3.10, [RUST-021])
// ---------------------------------------------------------------------------

/// Month name for January.
pub const MONTH_JAN: &str = "January";
/// Month name for February.
pub const MONTH_FEB: &str = "February";
/// Month name for March.
pub const MONTH_MAR: &str = "March";
/// Month name for April.
pub const MONTH_APR: &str = "April";
/// Month name for May.
pub const MONTH_MAY: &str = "May";
/// Month name for June.
pub const MONTH_JUN: &str = "June";
/// Month name for July.
pub const MONTH_JUL: &str = "July";
/// Month name for August.
pub const MONTH_AUG: &str = "August";
/// Month name for September.
pub const MONTH_SEP: &str = "September";
/// Month name for October.
pub const MONTH_OCT: &str = "October";
/// Month name for November.
pub const MONTH_NOV: &str = "November";
/// Month name for December.
pub const MONTH_DEC: &str = "December";

/// Short month name abbreviations.
pub const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Weekday short name for Monday.
pub const WEEKDAY_MON: &str = "Mo";
/// Weekday short name for Tuesday.
pub const WEEKDAY_TUE: &str = "Tu";
/// Weekday short name for Wednesday.
pub const WEEKDAY_WED: &str = "We";
/// Weekday short name for Thursday.
pub const WEEKDAY_THU: &str = "Th";
/// Weekday short name for Friday.
pub const WEEKDAY_FRI: &str = "Fr";
/// Weekday short name for Saturday.
pub const WEEKDAY_SAT: &str = "Sa";
/// Weekday short name for Sunday.
pub const WEEKDAY_SUN: &str = "Su";

/// Calendar trigger icon glyph (§3.10).
pub const CALENDAR_ICON: &str = "📅";

/// Previous navigation glyph.
pub const NAV_PREV: &str = "<";

/// Next navigation glyph.
pub const NAV_NEXT: &str = ">";

/// Default date placeholder string (§3.10).
pub const PLACEHOLDER_DATE: &str = "YYYY-MM-DD";

/// Date range separator string (§3.10).
pub const RANGE_SEPARATOR: &str = " - ";

// ---------------------------------------------------------------------------
// Layout & geometry constants (§3.10, UIF-ATOM-010, UIF-PRIN-001 §1.1)
// ---------------------------------------------------------------------------

/// Standard trigger control height — `size::CONTROL_MD` (32px) (§3.10, `UIF-ATOM-010`, `UIF-TOK-002`).
pub const TRIGGER_HEIGHT: f32 = size::CONTROL_MD;

/// Trailing calendar toggle button hit-target size — `size::HIT_TARGET_MIN` (24px) (§3.10, `UIF-ATOM-010`, `UIF-ATOM-002b`).
pub const TRAILING_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Calendar flyout fixed width — 256px (§3.10, `UIF-ATOM-010`, `UIF-PRIN-001` §1.1).
pub const CALENDAR_FLYOUT_WIDTH: f32 = 256.0;

/// Alias for [`CALENDAR_FLYOUT_WIDTH`] (§3.10).
pub const CALENDAR_FLYOUT_W: f32 = CALENDAR_FLYOUT_WIDTH;

/// Header navigation row height — `size::CONTROL_MD` (32px) (§3.10, `UIF-ATOM-010`).
pub const HEADER_HEIGHT: f32 = size::CONTROL_MD;

/// Month/Year navigation button size — `size::CONTROL_SM` (28px) (§3.10, `UIF-ATOM-010`).
pub const NAV_BUTTON_SIZE: f32 = size::CONTROL_SM;

/// Weekday column header row height — `size::BAR_STATUS` (24px) (§3.10, `UIF-ATOM-010`).
pub const WEEKDAY_ROW_HEIGHT: f32 = size::BAR_STATUS;

/// Grid day cell square size — `size::CONTROL_MD` (32px) (§3.10, `UIF-ATOM-010`).
pub const CELL_SIZE: f32 = size::CONTROL_MD;

/// Number of columns in the calendar grid (7 days per week) (§3.10, `UIF-ATOM-010`).
pub const GRID_COLS: usize = 7;

/// Number of rows in the fixed calendar grid (always 6 rows = 42 cells) (§3.10, `UIF-ATOM-010`, `UIF-PRIN-003`).
pub const GRID_ROWS: usize = 6;

/// Flyout card border radius — `shape::RADIUS_CARD` (6px) (§3.10, `UIF-ATOM-010`).
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// Flyout card internal padding — `space::MD` (12px) (§3.10, `UIF-ATOM-010`).
pub const FLYOUT_PADDING: f32 = space::MD;

/// Calculate the total fixed height of the calendar flyout card (§3.10, `UIF-ATOM-010`, `UIF-PRIN-003`).
#[must_use]
pub fn calendar_flyout_height() -> f32 {
    FLYOUT_PADDING * 2.0
        + HEADER_HEIGHT
        + space::SM
        + WEEKDAY_ROW_HEIGHT
        + space::XS
        + (GRID_ROWS as f32 * CELL_SIZE)
}

// ---------------------------------------------------------------------------
// Pure civil calendar arithmetic (zero external dependencies)
// ---------------------------------------------------------------------------

/// Convert civil year/month/day to days since Unix epoch (1970-01-01)
/// using Howard Hinnant's Euclidean affine civil date algorithm.
const fn days_from_civil(y: i32, m: u8, d: u8) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
    let yoe = (y - era * 400) as u32;
    let m = m as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as u32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

/// Convert days since Unix epoch back to civil (year, month, day)
/// using Howard Hinnant's Euclidean affine civil date algorithm.
const fn civil_from_days(z: i32) -> (i32, u8, u8) {
    let z = z + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// A calendar date in the proleptic Gregorian calendar (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CivilDate {
    /// Year (proleptic Gregorian, can be negative).
    pub year: i32,
    /// Month of the year (1..=12).
    pub month: u8,
    /// Day of the month (1..=31).
    pub day: u8,
}

impl CivilDate {
    /// Construct and validate a civil date (§3.10, `UIF-ATOM-010`).
    ///
    /// Returns `Some(CivilDate)` if valid, or `None` if `month` is not in `1..=12`
    /// or `day` is outside the valid range for that month.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if (1..=12).contains(&month) && day >= 1 && day <= Self::days_in_month(year, month) {
            Some(Self { year, month, day })
        } else {
            None
        }
    }

    /// Check whether a Gregorian year is a leap year (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub const fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Number of days in the specified month of a given year (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub const fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 => 31,
            2 => {
                if Self::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            3 => 31,
            4 => 30,
            5 => 31,
            6 => 30,
            7 => 31,
            8 => 31,
            9 => 30,
            10 => 31,
            11 => 30,
            12 => 31,
            _ => 0,
        }
    }

    /// Calculate the day of the week using Sakamoto's algorithm (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn day_of_week(&self) -> Weekday {
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = self.year;
        if self.month < 3 {
            y -= 1;
        }
        let m_idx = (self.month.saturating_sub(1) as usize).min(11);
        let dow = (y + y.div_euclid(4) - y.div_euclid(100)
            + y.div_euclid(400)
            + t[m_idx]
            + self.day as i32)
            .rem_euclid(7);
        match dow {
            0 => Weekday::Sun,
            1 => Weekday::Mon,
            2 => Weekday::Tue,
            3 => Weekday::Wed,
            4 => Weekday::Thu,
            5 => Weekday::Fri,
            6 => Weekday::Sat,
            _ => unreachable!(),
        }
    }

    /// Advance to the next calendar day (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn next_day(&self) -> CivilDate {
        if self.day < Self::days_in_month(self.year, self.month) {
            CivilDate {
                year: self.year,
                month: self.month,
                day: self.day + 1,
            }
        } else if self.month < 12 {
            CivilDate {
                year: self.year,
                month: self.month + 1,
                day: 1,
            }
        } else {
            CivilDate {
                year: self.year + 1,
                month: 1,
                day: 1,
            }
        }
    }

    /// Retreat to the preceding calendar day (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn prev_day(&self) -> CivilDate {
        if self.day > 1 {
            CivilDate {
                year: self.year,
                month: self.month,
                day: self.day - 1,
            }
        } else if self.month > 1 {
            let prev_m = self.month - 1;
            CivilDate {
                year: self.year,
                month: prev_m,
                day: Self::days_in_month(self.year, prev_m),
            }
        } else {
            CivilDate {
                year: self.year - 1,
                month: 12,
                day: 31,
            }
        }
    }

    /// Add an integer offset of days (positive or negative) in O(1) time (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn add_days(&self, days: i32) -> CivilDate {
        let z = days_from_civil(self.year, self.month, self.day);
        let (y, m, d) = civil_from_days(z + days);
        CivilDate {
            year: y,
            month: m,
            day: d,
        }
    }

    /// Format this date in standard ISO `YYYY-MM-DD` representation (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn format_iso(&self) -> String {
        if self.year < 0 {
            format!("-{:04}-{:02}-{:02}", self.year.abs(), self.month, self.day)
        } else {
            format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }

    /// Parse a date from ISO `YYYY-MM-DD` format (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn parse_iso(s: &str) -> Option<CivilDate> {
        let s = s.trim();
        let (neg, s) = if let Some(rest) = s.strip_prefix('-') {
            (true, rest)
        } else {
            (false, s)
        };
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let mut year: i32 = parts[0].parse().ok()?;
        if neg {
            year = -year;
        }
        let month: u8 = parts[1].parse().ok()?;
        let day: u8 = parts[2].parse().ok()?;
        CivilDate::new(year, month, day)
    }
}

impl std::fmt::Display for CivilDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_iso())
    }
}

/// Day of the week (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weekday {
    /// Monday
    #[default]
    Mon,
    /// Tuesday
    Tue,
    /// Wednesday
    Wed,
    /// Thursday
    Thu,
    /// Friday
    Fri,
    /// Saturday
    Sat,
    /// Sunday
    Sun,
}

impl Weekday {
    /// Short 2-letter abbreviation for column header (`UIF-ATOM-010`).
    #[must_use]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Mon => WEEKDAY_MON,
            Self::Tue => WEEKDAY_TUE,
            Self::Wed => WEEKDAY_WED,
            Self::Thu => WEEKDAY_THU,
            Self::Fri => WEEKDAY_FRI,
            Self::Sat => WEEKDAY_SAT,
            Self::Sun => WEEKDAY_SUN,
        }
    }

    /// Return the 7 weekdays in order starting from `self` (`UIF-ATOM-010`).
    #[must_use]
    pub fn week_order(self) -> [Weekday; 7] {
        let all = [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ];
        let start_idx = match self {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        let mut res = [Weekday::Mon; 7];
        for (i, slot) in res.iter_mut().enumerate() {
            *slot = all[(start_idx + i) % 7];
        }
        res
    }

    /// Offset from `self` to `other` (0..=6) (`UIF-ATOM-010`).
    #[must_use]
    pub fn days_until(self, other: Weekday) -> u8 {
        let idx = |w: Weekday| match w {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        (idx(other) + 7 - idx(self)) % 7
    }
}

impl std::fmt::Display for Weekday {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// A calendar year and month combination (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct YearMonth {
    /// Gregorian year.
    pub year: i32,
    /// Month of the year (1..=12).
    pub month: u8,
}

impl YearMonth {
    /// Construct and validate a `YearMonth` (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn new(year: i32, month: u8) -> Option<Self> {
        if (1..=12).contains(&month) {
            Some(Self { year, month })
        } else {
            None
        }
    }

    /// Next calendar month (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn next_month(&self) -> YearMonth {
        if self.month < 12 {
            YearMonth {
                year: self.year,
                month: self.month + 1,
            }
        } else {
            YearMonth {
                year: self.year + 1,
                month: 1,
            }
        }
    }

    /// Preceding calendar month (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn prev_month(&self) -> YearMonth {
        if self.month > 1 {
            YearMonth {
                year: self.year,
                month: self.month - 1,
            }
        } else {
            YearMonth {
                year: self.year - 1,
                month: 12,
            }
        }
    }

    /// First day of this month (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn first_date(&self) -> CivilDate {
        CivilDate {
            year: self.year,
            month: self.month,
            day: 1,
        }
    }

    /// Days in this month (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn days_in_month(&self) -> u8 {
        CivilDate::days_in_month(self.year, self.month)
    }

    /// Human-readable label for header display, e.g. "October 2026" (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn format_month_year(&self) -> String {
        format!("{} {:04}", month_name(self.month), self.year)
    }
}

impl std::fmt::Display for YearMonth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_month_year())
    }
}

/// Helper returning the full name of a month (1..=12) (§3.10).
#[must_use]
pub fn month_name(month: u8) -> &'static str {
    match month {
        1 => MONTH_JAN,
        2 => MONTH_FEB,
        3 => MONTH_MAR,
        4 => MONTH_APR,
        5 => MONTH_MAY,
        6 => MONTH_JUN,
        7 => MONTH_JUL,
        8 => MONTH_AUG,
        9 => MONTH_SEP,
        10 => MONTH_OCT,
        11 => MONTH_NOV,
        12 => MONTH_DEC,
        _ => "",
    }
}

/// Calendar drill-down view modes (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CalendarViewMode {
    /// Standard 6x7 day grid view.
    #[default]
    Days,
    /// 12-month picker view.
    Months,
    /// 10-12 year decade picker view.
    Years,
}

/// Selection mode for date picking (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateSelection {
    /// Single date selection mode.
    SingleDate(Option<CivilDate>),
    /// Date range selection mode with start and end dates.
    DateRange {
        /// Range start date.
        start: Option<CivilDate>,
        /// Range end date.
        end: Option<CivilDate>,
    },
}

impl DateSelection {
    /// Check whether a given date is an active selection endpoint (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn is_selected_endpoint(&self, date: CivilDate) -> bool {
        match self {
            Self::SingleDate(Some(d)) => *d == date,
            Self::DateRange { start, end } => *start == Some(date) || *end == Some(date),
            _ => false,
        }
    }

    /// Check whether a date is in the interior between start and end (or start and hover preview)
    /// (§3.10, `UIF-ATOM-010`, `UIF-PRIN-005`).
    #[must_use]
    pub fn is_range_interior(&self, date: CivilDate, hover_preview: Option<CivilDate>) -> bool {
        match self {
            Self::DateRange {
                start: Some(s),
                end: Some(e),
            } => {
                let (min, max) = if s <= e { (*s, *e) } else { (*e, *s) };
                date > min && date < max
            }
            Self::DateRange {
                start: Some(s),
                end: None,
            } => {
                if let Some(h) = hover_preview {
                    let (min, max) = if s <= &h { (*s, h) } else { (h, *s) };
                    date > min && date < max
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check whether a date is a hover preview endpoint during range selection (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn is_range_preview_endpoint(
        &self,
        date: CivilDate,
        hover_preview: Option<CivilDate>,
    ) -> bool {
        match self {
            Self::DateRange {
                start: Some(s),
                end: None,
            } => {
                if let Some(h) = hover_preview {
                    date == h && date != *s
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Format selection for trigger display (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn format_display(&self) -> String {
        match self {
            Self::SingleDate(Some(d)) => d.format_iso(),
            Self::SingleDate(None) => String::new(),
            Self::DateRange {
                start: Some(s),
                end: Some(e),
            } => format!("{}{}{}", s.format_iso(), RANGE_SEPARATOR, e.format_iso()),
            Self::DateRange {
                start: Some(s),
                end: None,
            } => format!("{}{}", s.format_iso(), RANGE_SEPARATOR),
            Self::DateRange {
                start: None,
                end: Some(e),
            } => format!("...{}{}", RANGE_SEPARATOR, e.format_iso()),
            Self::DateRange {
                start: None,
                end: None,
            } => String::new(),
        }
    }
}

impl std::fmt::Display for DateSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_display())
    }
}

/// A single cell in the 42-cell 6x7 day grid (`UIF-ATOM-010`, `UIF-PRIN-003`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    /// The civil date for this cell.
    pub date: CivilDate,
    /// Whether this date belongs to the currently displayed month.
    pub is_current_month: bool,
}

/// Generate the exact 42 cells (6 rows × 7 columns) for `year_month` starting on `first_day_of_week`
/// (`UIF-ATOM-010`, `UIF-PRIN-003`).
///
/// Holds layout height completely invariant across 28-day, 29-day, 30-day and 31-day months (0px layout shift).
#[must_use]
pub fn displayed_month_cells(year_month: YearMonth, first_day_of_week: Weekday) -> [GridCell; 42] {
    let first = year_month.first_date();
    let first_dow = first.day_of_week();
    let leading = first_day_of_week.days_until(first_dow);
    let start_date = first.add_days(-(leading as i32));

    let mut cells = [GridCell {
        date: start_date,
        is_current_month: false,
    }; 42];
    for (i, cell) in cells.iter_mut().enumerate() {
        let date = start_date.add_days(i as i32);
        let is_current_month = date.year == year_month.year && date.month == year_month.month;
        *cell = GridCell {
            date,
            is_current_month,
        };
    }
    cells
}

// ---------------------------------------------------------------------------
// DatePickerModel
// ---------------------------------------------------------------------------

/// Declarative data model driving a [`DatePicker`] (§3.10, `UIF-ATOM-010`).
#[derive(Debug, Clone, PartialEq)]
pub struct DatePickerModel {
    /// Current selection: single date or date range.
    pub selection: DateSelection,
    /// Live hover preview date during interaction.
    pub hover_preview_date: Option<CivilDate>,
    /// The currently displayed calendar month and year.
    pub displayed_month: YearMonth,
    /// Calendar drill-down view mode (Days, Months, Years).
    pub view_mode: CalendarViewMode,
    /// Optional minimum selectable date.
    pub min_date: Option<CivilDate>,
    /// Optional maximum selectable date.
    pub max_date: Option<CivilDate>,
    /// Set of blackout / unselectable dates.
    pub blackout_dates: BTreeSet<CivilDate>,
    /// First day of the week for day grid column headers (default Mon).
    pub first_day_of_week: Weekday,
    /// Whether the calendar flyout is open.
    pub open: bool,
    /// Validation message or error status.
    pub validation: Option<String>,
    /// Disabled state.
    pub disabled: bool,
}

impl DatePickerModel {
    /// Construct a new `DatePickerModel` centered around `initial_date` (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn new(initial_date: CivilDate) -> Self {
        Self {
            selection: DateSelection::SingleDate(None),
            hover_preview_date: None,
            displayed_month: YearMonth {
                year: initial_date.year,
                month: initial_date.month,
            },
            view_mode: CalendarViewMode::Days,
            min_date: None,
            max_date: None,
            blackout_dates: BTreeSet::new(),
            first_day_of_week: Weekday::Mon,
            open: false,
            validation: None,
            disabled: false,
        }
    }

    /// Set current selection (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_selection(mut self, selection: DateSelection) -> Self {
        self.selection = selection;
        self
    }

    /// Set hover preview date (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_hover_preview_date(mut self, preview: Option<CivilDate>) -> Self {
        self.hover_preview_date = preview;
        self
    }

    /// Set displayed month (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_displayed_month(mut self, ym: YearMonth) -> Self {
        self.displayed_month = ym;
        self
    }

    /// Set calendar view mode (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_view_mode(mut self, mode: CalendarViewMode) -> Self {
        self.view_mode = mode;
        self
    }

    /// Set minimum selectable date (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_min_date(mut self, min: impl Into<Option<CivilDate>>) -> Self {
        self.min_date = min.into();
        self
    }

    /// Set maximum selectable date (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_max_date(mut self, max: impl Into<Option<CivilDate>>) -> Self {
        self.max_date = max.into();
        self
    }

    /// Set blackout dates (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_blackout_dates(mut self, dates: impl IntoIterator<Item = CivilDate>) -> Self {
        self.blackout_dates = dates.into_iter().collect();
        self
    }

    /// Set first day of week (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_first_day_of_week(mut self, first_day: Weekday) -> Self {
        self.first_day_of_week = first_day;
        self
    }

    /// Set flyout open state (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set validation message (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_validation(mut self, validation: impl Into<Option<String>>) -> Self {
        self.validation = validation.into();
        self
    }

    /// Set disabled state (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Check if a date is disabled (out of min/max bounds or blackout) (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn is_date_disabled(&self, date: CivilDate) -> bool {
        self.disabled
            || self.min_date.is_some_and(|m| date < m)
            || self.max_date.is_some_and(|m| date > m)
            || self.blackout_dates.contains(&date)
    }

    /// Select or toggle a date under the current selection mode (§3.10, `UIF-ATOM-010`).
    pub fn select_date(&mut self, date: CivilDate) {
        if self.is_date_disabled(date) {
            return;
        }
        match &self.selection {
            DateSelection::SingleDate(_) => {
                self.selection = DateSelection::SingleDate(Some(date));
            }
            DateSelection::DateRange { start, end } => {
                if start.is_none() || (start.is_some() && end.is_some()) {
                    self.selection = DateSelection::DateRange {
                        start: Some(date),
                        end: None,
                    };
                } else if let Some(s) = start {
                    let (min, max) = if date < *s { (date, *s) } else { (*s, date) };
                    self.selection = DateSelection::DateRange {
                        start: Some(min),
                        end: Some(max),
                    };
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Callbacks & DatePicker Widget
// ---------------------------------------------------------------------------

/// Callback invoked when flyout open state changes (§3.10, `UIF-ATOM-010`).
pub type OnOpenChange<'a, Message> = Box<dyn Fn(Live<bool>) -> Message + 'a>;

/// Callback invoked when calendar view mode changes (§3.10, `UIF-ATOM-010`).
pub type OnViewModeChange<'a, Message> = Box<dyn Fn(Live<CalendarViewMode>) -> Message + 'a>;

/// Callback invoked when navigating months/years (§3.10, `UIF-ATOM-010`).
pub type OnMonthNavigate<'a, Message> = Box<dyn Fn(Live<YearMonth>) -> Message + 'a>;

/// Callback invoked live during selection updates (§3.10, `UIF-ATOM-010`).
pub type OnChange<'a, Message> = Box<dyn Fn(Live<DateSelection>) -> Message + 'a>;

/// Callback invoked once when selection settles and commits (§3.10, `UIF-ATOM-010`).
pub type OnCommit<'a, Message> = Box<dyn Fn(Commit<DateSelection>) -> Message + 'a>;

/// Callback invoked when interaction is cancelled (§3.10, `UIF-ATOM-010`).
pub type OnCancel<'a, Message> = Box<dyn Fn(Cancel<DateSelection>) -> Message + 'a>;

/// Interactive DatePicker & Calendar widget (§3.10, `UIF-ATOM-010`).
pub struct DatePicker<'a, Message> {
    widget_id: widget::Id,
    model: DatePickerModel,
    palette: Palette,
    today: Option<CivilDate>,
    on_open_change: Option<OnOpenChange<'a, Message>>,
    on_view_mode_change: Option<OnViewModeChange<'a, Message>>,
    on_month_navigate: Option<OnMonthNavigate<'a, Message>>,
    on_change: Option<OnChange<'a, Message>>,
    on_commit: Option<OnCommit<'a, Message>>,
    on_cancel: Option<OnCancel<'a, Message>>,
}

impl<'a, Message> DatePicker<'a, Message> {
    /// Construct a new DatePicker from a model (§3.10, `UIF-ATOM-010`).
    #[must_use]
    pub fn new(model: DatePickerModel) -> Self {
        Self {
            widget_id: widget::Id::unique(),
            model,
            palette: Palette::dark(),
            today: None,
            on_open_change: None,
            on_view_mode_change: None,
            on_month_navigate: None,
            on_change: None,
            on_commit: None,
            on_cancel: None,
        }
    }

    /// Set today's date for rendering the accent cue bar (§3.10, `UIF-PRIN-005`).
    #[must_use]
    pub fn with_today(mut self, today: impl Into<Option<CivilDate>>) -> Self {
        self.today = today.into();
        self
    }

    /// Set palette.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Callback invoked when open status changes.
    #[must_use]
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(Live<bool>) -> Message + 'a,
    {
        self.on_open_change = Some(Box::new(callback));
        self
    }

    /// Callback invoked when view mode changes.
    #[must_use]
    pub fn on_view_mode_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(Live<CalendarViewMode>) -> Message + 'a,
    {
        self.on_view_mode_change = Some(Box::new(callback));
        self
    }

    /// Callback invoked when navigating months.
    #[must_use]
    pub fn on_month_navigate<F>(mut self, callback: F) -> Self
    where
        F: Fn(Live<YearMonth>) -> Message + 'a,
    {
        self.on_month_navigate = Some(Box::new(callback));
        self
    }

    /// Callback invoked live during selection updates.
    #[must_use]
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(Live<DateSelection>) -> Message + 'a,
    {
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Callback invoked on interaction commit.
    #[must_use]
    pub fn on_commit<F>(mut self, callback: F) -> Self
    where
        F: Fn(Commit<DateSelection>) -> Message + 'a,
    {
        self.on_commit = Some(Box::new(callback));
        self
    }

    /// Callback invoked on interaction cancel.
    #[must_use]
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(Cancel<DateSelection>) -> Message + 'a,
    {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    /// Reference to underlying model.
    #[must_use]
    pub fn model(&self) -> &DatePickerModel {
        &self.model
    }

    /// Resolve layout size for widget limits.
    #[must_use]
    pub fn size(&self) -> Size<Length> {
        let h = if self.model.open {
            TRIGGER_HEIGHT + space::XS + calendar_flyout_height()
        } else {
            TRIGGER_HEIGHT
        };
        Size::new(Length::Fixed(CALENDAR_FLYOUT_WIDTH), Length::Fixed(h))
    }
}

/// Construct a DatePicker for single date selection (§3.10, `UIF-ATOM-010`).
#[must_use]
pub fn date_picker<'a, Message>(model: DatePickerModel) -> DatePicker<'a, Message> {
    DatePicker::new(model)
}

/// Construct a DateRangePicker for date range selection (§3.10, `UIF-ATOM-010`).
#[must_use]
pub fn date_range_picker<'a, Message>(mut model: DatePickerModel) -> DatePicker<'a, Message> {
    if matches!(model.selection, DateSelection::SingleDate(_)) {
        model.selection = DateSelection::DateRange {
            start: None,
            end: None,
        };
    }
    DatePicker::new(model)
}

/// Alias for [`DatePicker`] used specifically for date range picking (§3.10, `UIF-ATOM-010`).
pub type DateRangePicker<'a, Message> = DatePicker<'a, Message>;

/// CalendarView mode or embedded calendar display (§3.10, `UIF-ATOM-010`).
pub type CalendarView<'a, Message> = DatePicker<'a, Message>;

/// Construct an always-open embedded CalendarView (§3.10, `UIF-ATOM-010`).
#[must_use]
pub fn calendar_view<'a, Message>(mut model: DatePickerModel) -> CalendarView<'a, Message> {
    model.open = true;
    DatePicker::new(model)
}

/// Internal state held in the iced widget tree across frames.
#[derive(Debug, Default)]
pub struct DatePickerState {
    /// Keyboard focus tracking.
    pub focusable: FocusableState,
    /// Currently focused day in the grid.
    pub focused_date: Option<CivilDate>,
    /// Currently hovered day in the grid.
    pub hovered_date: Option<CivilDate>,
    /// Hovered state for left navigation chevron.
    pub hovered_nav_prev: bool,
    /// Hovered state for right navigation chevron.
    pub hovered_nav_next: bool,
    /// Hovered state for center title button.
    pub hovered_title: bool,
    /// Hovered state for trailing calendar button on trigger.
    pub hovered_trailing: bool,
    /// Hovered month in Months view (1..=12).
    pub hovered_month: Option<u8>,
    /// Hovered year in Years view.
    pub hovered_year: Option<i32>,
    /// Gesture slot for Live/Commit/Cancel transitions.
    pub gesture: GestureSlot<DateSelection>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for DatePicker<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DatePickerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DatePickerState::default())
    }

    fn size(&self) -> Size<Length> {
        DatePicker::size(self)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let w = CALENDAR_FLYOUT_WIDTH;
        let h = if self.model.open {
            TRIGGER_HEIGHT + space::XS + calendar_flyout_height()
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
        let state = tree.state.downcast_mut::<DatePickerState>();
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

        let state = tree.state.downcast_mut::<DatePickerState>();
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
            width: CALENDAR_FLYOUT_WIDTH,
            height: calendar_flyout_height(),
        };

        let nav_y = flyout_bounds.y + FLYOUT_PADDING;
        let nav_prev_bounds = Rectangle {
            x: flyout_bounds.x + FLYOUT_PADDING,
            y: nav_y,
            width: NAV_BUTTON_SIZE,
            height: NAV_BUTTON_SIZE,
        };
        let nav_next_bounds = Rectangle {
            x: flyout_bounds.x + CALENDAR_FLYOUT_WIDTH - FLYOUT_PADDING - NAV_BUTTON_SIZE,
            y: nav_y,
            width: NAV_BUTTON_SIZE,
            height: NAV_BUTTON_SIZE,
        };
        let title_bounds = Rectangle {
            x: nav_prev_bounds.x + NAV_BUTTON_SIZE,
            y: nav_y,
            width: CALENDAR_FLYOUT_WIDTH - 2.0 * (FLYOUT_PADDING + NAV_BUTTON_SIZE),
            height: NAV_BUTTON_SIZE,
        };

        let grid_x =
            flyout_bounds.x + (CALENDAR_FLYOUT_WIDTH - (GRID_COLS as f32 * CELL_SIZE)) / 2.0;
        let grid_y = nav_y + NAV_BUTTON_SIZE + space::SM + WEEKDAY_ROW_HEIGHT + space::XS;

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    let trailing_bounds = Rectangle {
                        x: trigger_bounds.x + trigger_bounds.width
                            - TRAILING_BUTTON_SIZE
                            - space::SM,
                        y: trigger_bounds.y + (TRIGGER_HEIGHT - TRAILING_BUTTON_SIZE) / 2.0,
                        width: TRAILING_BUTTON_SIZE,
                        height: TRAILING_BUTTON_SIZE,
                    };
                    state.hovered_trailing = trailing_bounds.contains(pos);

                    if self.model.open && flyout_bounds.contains(pos) {
                        state.hovered_nav_prev = nav_prev_bounds.contains(pos);
                        state.hovered_nav_next = nav_next_bounds.contains(pos);
                        state.hovered_title = title_bounds.contains(pos);

                        if self.model.view_mode == CalendarViewMode::Days {
                            let mut found_date = None;
                            let cells = displayed_month_cells(
                                self.model.displayed_month,
                                self.model.first_day_of_week,
                            );
                            for (idx, cell) in cells.iter().enumerate() {
                                let row = idx / GRID_COLS;
                                let col = idx % GRID_COLS;
                                let cell_rect = Rectangle {
                                    x: grid_x + col as f32 * CELL_SIZE,
                                    y: grid_y + row as f32 * CELL_SIZE,
                                    width: CELL_SIZE,
                                    height: CELL_SIZE,
                                };
                                if cell_rect.contains(pos) {
                                    found_date = Some(cell.date);
                                    break;
                                }
                            }
                            if state.hovered_date != found_date {
                                state.hovered_date = found_date;
                                if let Some(preview_date) = found_date
                                    && let DateSelection::DateRange { start: Some(s), .. } =
                                        &self.model.selection
                                    && let Some(on_change) = &self.on_change
                                {
                                    shell.publish(on_change(Live::new(DateSelection::DateRange {
                                        start: Some(*s),
                                        end: Some(preview_date),
                                    })));
                                }
                            }
                        } else if self.model.view_mode == CalendarViewMode::Months {
                            let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                            let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                            let row_h = (flyout_bounds.height
                                - (body_y - flyout_bounds.y)
                                - FLYOUT_PADDING)
                                / 4.0;
                            let mut found_m = None;
                            for m in 1..=12 {
                                let r = (m - 1) / 3;
                                let c = (m - 1) % 3;
                                let m_rect = Rectangle {
                                    x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                                    y: body_y + r as f32 * row_h,
                                    width: col_w,
                                    height: row_h,
                                };
                                if m_rect.contains(pos) {
                                    found_m = Some(m as u8);
                                    break;
                                }
                            }
                            state.hovered_month = found_m;
                        } else if self.model.view_mode == CalendarViewMode::Years {
                            let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                            let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                            let row_h = (flyout_bounds.height
                                - (body_y - flyout_bounds.y)
                                - FLYOUT_PADDING)
                                / 4.0;
                            let start_year = (self.model.displayed_month.year / 10) * 10 - 1;
                            let mut found_y = None;
                            for i in 0..12i32 {
                                let r = i / 3;
                                let c = i % 3;
                                let y_rect = Rectangle {
                                    x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                                    y: body_y + r as f32 * row_h,
                                    width: col_w,
                                    height: row_h,
                                };
                                if y_rect.contains(pos) {
                                    found_y = Some(start_year + i);
                                    break;
                                }
                            }
                            state.hovered_year = found_y;
                        }
                    } else {
                        state.hovered_nav_prev = false;
                        state.hovered_nav_next = false;
                        state.hovered_title = false;
                        state.hovered_date = None;
                        state.hovered_month = None;
                        state.hovered_year = None;
                    }
                } else {
                    state.hovered_trailing = false;
                    state.hovered_nav_prev = false;
                    state.hovered_nav_next = false;
                    state.hovered_title = false;
                    state.hovered_date = None;
                    state.hovered_month = None;
                    state.hovered_year = None;
                }
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                state.hovered_trailing = false;
                state.hovered_nav_prev = false;
                state.hovered_nav_next = false;
                state.hovered_title = false;
                state.hovered_date = None;
                state.hovered_month = None;
                state.hovered_year = None;
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    if trigger_bounds.contains(pos) {
                        let new_open = !self.model.open;
                        if let Some(on_open) = &self.on_open_change {
                            shell.publish(on_open(Live::new(new_open)));
                        }
                    } else if self.model.open && flyout_bounds.contains(pos) {
                        if nav_prev_bounds.contains(pos) {
                            match self.model.view_mode {
                                CalendarViewMode::Days => {
                                    let prev = self.model.displayed_month.prev_month();
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(prev)));
                                    }
                                }
                                CalendarViewMode::Months => {
                                    let prev = YearMonth {
                                        year: self.model.displayed_month.year - 1,
                                        month: self.model.displayed_month.month,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(prev)));
                                    }
                                }
                                CalendarViewMode::Years => {
                                    let prev = YearMonth {
                                        year: self.model.displayed_month.year - 10,
                                        month: self.model.displayed_month.month,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(prev)));
                                    }
                                }
                            }
                        } else if nav_next_bounds.contains(pos) {
                            match self.model.view_mode {
                                CalendarViewMode::Days => {
                                    let next = self.model.displayed_month.next_month();
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(next)));
                                    }
                                }
                                CalendarViewMode::Months => {
                                    let next = YearMonth {
                                        year: self.model.displayed_month.year + 1,
                                        month: self.model.displayed_month.month,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(next)));
                                    }
                                }
                                CalendarViewMode::Years => {
                                    let next = YearMonth {
                                        year: self.model.displayed_month.year + 10,
                                        month: self.model.displayed_month.month,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(next)));
                                    }
                                }
                            }
                        } else if title_bounds.contains(pos) {
                            let next_mode = match self.model.view_mode {
                                CalendarViewMode::Days => CalendarViewMode::Months,
                                CalendarViewMode::Months => CalendarViewMode::Years,
                                CalendarViewMode::Years => CalendarViewMode::Days,
                            };
                            if let Some(on_view) = &self.on_view_mode_change {
                                shell.publish(on_view(Live::new(next_mode)));
                            }
                        } else if self.model.view_mode == CalendarViewMode::Days {
                            let cells = displayed_month_cells(
                                self.model.displayed_month,
                                self.model.first_day_of_week,
                            );
                            for (idx, cell) in cells.iter().enumerate() {
                                let row = idx / GRID_COLS;
                                let col = idx % GRID_COLS;
                                let cell_rect = Rectangle {
                                    x: grid_x + col as f32 * CELL_SIZE,
                                    y: grid_y + row as f32 * CELL_SIZE,
                                    width: CELL_SIZE,
                                    height: CELL_SIZE,
                                };
                                if cell_rect.contains(pos) {
                                    if !self.model.is_date_disabled(cell.date) {
                                        state.focused_date = Some(cell.date);
                                        let mut new_selection = self.model.selection.clone();
                                        match &new_selection {
                                            DateSelection::SingleDate(_) => {
                                                new_selection =
                                                    DateSelection::SingleDate(Some(cell.date));
                                                if let Some(on_change) = &self.on_change {
                                                    shell.publish(on_change(Live::new(
                                                        new_selection.clone(),
                                                    )));
                                                }
                                                if let Some(on_commit) = &self.on_commit {
                                                    shell.publish(on_commit(Commit::new(
                                                        new_selection,
                                                    )));
                                                }
                                                if let Some(on_open) = &self.on_open_change {
                                                    shell.publish(on_open(Live::new(false)));
                                                }
                                            }
                                            DateSelection::DateRange { start, end } => {
                                                if start.is_none()
                                                    || (start.is_some() && end.is_some())
                                                {
                                                    new_selection = DateSelection::DateRange {
                                                        start: Some(cell.date),
                                                        end: None,
                                                    };
                                                    if let Some(on_change) = &self.on_change {
                                                        shell.publish(on_change(Live::new(
                                                            new_selection,
                                                        )));
                                                    }
                                                } else if let Some(s) = start {
                                                    let (min, max) = if cell.date < *s {
                                                        (cell.date, *s)
                                                    } else {
                                                        (*s, cell.date)
                                                    };
                                                    new_selection = DateSelection::DateRange {
                                                        start: Some(min),
                                                        end: Some(max),
                                                    };
                                                    if let Some(on_change) = &self.on_change {
                                                        shell.publish(on_change(Live::new(
                                                            new_selection.clone(),
                                                        )));
                                                    }
                                                    if let Some(on_commit) = &self.on_commit {
                                                        shell.publish(on_commit(Commit::new(
                                                            new_selection,
                                                        )));
                                                    }
                                                    if let Some(on_open) = &self.on_open_change {
                                                        shell.publish(on_open(Live::new(false)));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        } else if self.model.view_mode == CalendarViewMode::Months {
                            let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                            let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                            let row_h = (flyout_bounds.height
                                - (body_y - flyout_bounds.y)
                                - FLYOUT_PADDING)
                                / 4.0;
                            for m in 1..=12 {
                                let r = (m - 1) / 3;
                                let c = (m - 1) % 3;
                                let m_rect = Rectangle {
                                    x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                                    y: body_y + r as f32 * row_h,
                                    width: col_w,
                                    height: row_h,
                                };
                                if m_rect.contains(pos) {
                                    let new_ym = YearMonth {
                                        year: self.model.displayed_month.year,
                                        month: m as u8,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(new_ym)));
                                    }
                                    if let Some(on_view) = &self.on_view_mode_change {
                                        shell.publish(on_view(Live::new(CalendarViewMode::Days)));
                                    }
                                    break;
                                }
                            }
                        } else if self.model.view_mode == CalendarViewMode::Years {
                            let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                            let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                            let row_h = (flyout_bounds.height
                                - (body_y - flyout_bounds.y)
                                - FLYOUT_PADDING)
                                / 4.0;
                            let start_year = (self.model.displayed_month.year / 10) * 10 - 1;
                            for i in 0..12i32 {
                                let r = i / 3;
                                let c = i % 3;
                                let y_rect = Rectangle {
                                    x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                                    y: body_y + r as f32 * row_h,
                                    width: col_w,
                                    height: row_h,
                                };
                                if y_rect.contains(pos) {
                                    let new_ym = YearMonth {
                                        year: start_year + i,
                                        month: self.model.displayed_month.month,
                                    };
                                    if let Some(on_nav) = &self.on_month_navigate {
                                        shell.publish(on_nav(Live::new(new_ym)));
                                    }
                                    if let Some(on_view) = &self.on_view_mode_change {
                                        shell.publish(on_view(Live::new(CalendarViewMode::Months)));
                                    }
                                    break;
                                }
                            }
                        }
                    }
                } else if self.model.open {
                    // Click outside closes flyout
                    if let Some(on_cancel) = &self.on_cancel {
                        shell.publish(on_cancel(Cancel::new(self.model.selection.clone())));
                    }
                    if let Some(on_open) = &self.on_open_change {
                        shell.publish(on_open(Live::new(false)));
                    }
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if *key == Key::Named(Named::Escape) && self.model.open {
                    if let Some(on_cancel) = &self.on_cancel {
                        shell.publish(on_cancel(Cancel::new(self.model.selection.clone())));
                    }
                    if let Some(on_open) = &self.on_open_change {
                        shell.publish(on_open(Live::new(false)));
                    }
                } else if (*key == Key::Named(Named::Enter) || *key == Key::Named(Named::Space))
                    && state.focusable.is_focused()
                {
                    if !self.model.open {
                        if let Some(on_open) = &self.on_open_change {
                            shell.publish(on_open(Live::new(true)));
                        }
                    } else if let Some(date) = state.focused_date
                        && !self.model.is_date_disabled(date)
                    {
                        let mut new_selection = self.model.selection.clone();
                        match &new_selection {
                            DateSelection::SingleDate(_) => {
                                new_selection = DateSelection::SingleDate(Some(date));
                                if let Some(on_commit) = &self.on_commit {
                                    shell.publish(on_commit(Commit::new(new_selection)));
                                }
                                if let Some(on_open) = &self.on_open_change {
                                    shell.publish(on_open(Live::new(false)));
                                }
                            }
                            DateSelection::DateRange { start, end } => {
                                if start.is_none() || (start.is_some() && end.is_some()) {
                                    new_selection = DateSelection::DateRange {
                                        start: Some(date),
                                        end: None,
                                    };
                                    if let Some(on_change) = &self.on_change {
                                        shell.publish(on_change(Live::new(new_selection)));
                                    }
                                } else if let Some(s) = start {
                                    let (min, max) =
                                        if date < *s { (date, *s) } else { (*s, date) };
                                    new_selection = DateSelection::DateRange {
                                        start: Some(min),
                                        end: Some(max),
                                    };
                                    if let Some(on_commit) = &self.on_commit {
                                        shell.publish(on_commit(Commit::new(new_selection)));
                                    }
                                    if let Some(on_open) = &self.on_open_change {
                                        shell.publish(on_open(Live::new(false)));
                                    }
                                }
                            }
                        }
                    }
                } else if self.model.open && self.model.view_mode == CalendarViewMode::Days {
                    let cur_date = state
                        .focused_date
                        .unwrap_or_else(|| self.model.displayed_month.first_date());
                    let new_date = match key {
                        Key::Named(Named::ArrowLeft) => Some(cur_date.prev_day()),
                        Key::Named(Named::ArrowRight) => Some(cur_date.next_day()),
                        Key::Named(Named::ArrowUp) => Some(cur_date.add_days(-7)),
                        Key::Named(Named::ArrowDown) => Some(cur_date.add_days(7)),
                        Key::Named(Named::PageUp) => {
                            let prev_ym = self.model.displayed_month.prev_month();
                            if let Some(on_nav) = &self.on_month_navigate {
                                shell.publish(on_nav(Live::new(prev_ym)));
                            }
                            Some(CivilDate {
                                year: prev_ym.year,
                                month: prev_ym.month,
                                day: cur_date.day.min(prev_ym.days_in_month()),
                            })
                        }
                        Key::Named(Named::PageDown) => {
                            let next_ym = self.model.displayed_month.next_month();
                            if let Some(on_nav) = &self.on_month_navigate {
                                shell.publish(on_nav(Live::new(next_ym)));
                            }
                            Some(CivilDate {
                                year: next_ym.year,
                                month: next_ym.month,
                                day: cur_date.day.min(next_ym.days_in_month()),
                            })
                        }
                        Key::Named(Named::Home) => Some(self.model.displayed_month.first_date()),
                        Key::Named(Named::End) => Some(CivilDate {
                            year: self.model.displayed_month.year,
                            month: self.model.displayed_month.month,
                            day: self.model.displayed_month.days_in_month(),
                        }),
                        _ => None,
                    };
                    if let Some(d) = new_date {
                        state.focused_date = Some(d);
                        if d.year != self.model.displayed_month.year
                            || d.month != self.model.displayed_month.month
                        {
                            let ym = YearMonth {
                                year: d.year,
                                month: d.month,
                            };
                            if let Some(on_nav) = &self.on_month_navigate {
                                shell.publish(on_nav(Live::new(ym)));
                            }
                        }
                    }
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
        let state = tree.state.downcast_ref::<DatePickerState>();
        let bounds = layout.bounds();

        // 1. Draw Trigger Box (32px height) (§3.10)
        let trigger_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: TRIGGER_HEIGHT,
        };

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

        // Trigger surface background
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

        // Focus indicator when focused
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

        // Trigger text
        let display_text = self.model.selection.format_display();
        let (content, text_color) = if display_text.is_empty() {
            (PLACEHOLDER_DATE.to_owned(), self.palette.text_muted)
        } else if self.model.disabled {
            (display_text, self.palette.text_disabled)
        } else {
            (display_text, self.palette.text_strong)
        };

        let text_area = Rectangle {
            x: trigger_bounds.x + space::MD,
            y: trigger_bounds.y,
            width: trigger_bounds.width - space::MD - TRAILING_BUTTON_SIZE - space::SM,
            height: trigger_bounds.height,
        };

        renderer.fill_text(
            text_core::Text {
                content,
                bounds: text_area.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(text_area.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(text_area.x, text_area.center_y()),
            text_color,
            *viewport,
        );

        // Trailing calendar button (24x24px zero-shift slot) (§3.10, `UIF-ATOM-002b`)
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
                content: CALENDAR_ICON.to_owned(),
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

        // 2. Draw Calendar Flyout Card when Open (§3.10)
        if !self.model.open {
            return;
        }

        let flyout_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y + TRIGGER_HEIGHT + space::XS,
            width: CALENDAR_FLYOUT_WIDTH,
            height: calendar_flyout_height(),
        };

        // Flyout card background
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout_bounds,
                border: Border::default()
                    .color(self.palette.border_subtle)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_overlay),
        );

        // Header Navigation Row (§3.10)
        let nav_y = flyout_bounds.y + FLYOUT_PADDING;
        let nav_prev_bounds = Rectangle {
            x: flyout_bounds.x + FLYOUT_PADDING,
            y: nav_y,
            width: NAV_BUTTON_SIZE,
            height: NAV_BUTTON_SIZE,
        };
        let nav_next_bounds = Rectangle {
            x: flyout_bounds.x + CALENDAR_FLYOUT_WIDTH - FLYOUT_PADDING - NAV_BUTTON_SIZE,
            y: nav_y,
            width: NAV_BUTTON_SIZE,
            height: NAV_BUTTON_SIZE,
        };
        let title_bounds = Rectangle {
            x: nav_prev_bounds.x + NAV_BUTTON_SIZE,
            y: nav_y,
            width: CALENDAR_FLYOUT_WIDTH - 2.0 * (FLYOUT_PADDING + NAV_BUTTON_SIZE),
            height: NAV_BUTTON_SIZE,
        };

        // Nav Prev button
        if state.hovered_nav_prev {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: nav_prev_bounds,
                    border: Border::default().rounded(shape::RADIUS_WIDGET),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.palette.surface_active),
            );
        }
        renderer.fill_text(
            text_core::Text {
                content: NAV_PREV.to_owned(),
                bounds: nav_prev_bounds.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(nav_prev_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            nav_prev_bounds.center(),
            self.palette.text_primary,
            *viewport,
        );

        // Nav Next button
        if state.hovered_nav_next {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: nav_next_bounds,
                    border: Border::default().rounded(shape::RADIUS_WIDGET),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.palette.surface_active),
            );
        }
        renderer.fill_text(
            text_core::Text {
                content: NAV_NEXT.to_owned(),
                bounds: nav_next_bounds.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(nav_next_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            nav_next_bounds.center(),
            self.palette.text_primary,
            *viewport,
        );

        // Title button (Days -> Months -> Years)
        if state.hovered_title {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: title_bounds,
                    border: Border::default().rounded(shape::RADIUS_WIDGET),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.palette.surface_active),
            );
        }

        let title_text = match self.model.view_mode {
            CalendarViewMode::Days => self.model.displayed_month.format_month_year(),
            CalendarViewMode::Months => format!("{:04}", self.model.displayed_month.year),
            CalendarViewMode::Years => {
                let start = (self.model.displayed_month.year / 10) * 10;
                format!("{:04} - {:04}", start, start + 9)
            }
        };

        renderer.fill_text(
            text_core::Text {
                content: title_text,
                bounds: title_bounds.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(title_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            title_bounds.center(),
            self.palette.text_strong,
            *viewport,
        );

        // View Mode Specific Content
        match self.model.view_mode {
            CalendarViewMode::Days => {
                // Weekday Column Header Row
                let weekday_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                let grid_w = GRID_COLS as f32 * CELL_SIZE;
                let grid_x = flyout_bounds.x + (CALENDAR_FLYOUT_WIDTH - grid_w) / 2.0;

                let weekdays = self.model.first_day_of_week.week_order();
                for (col, w) in weekdays.iter().enumerate() {
                    let w_bounds = Rectangle {
                        x: grid_x + col as f32 * CELL_SIZE,
                        y: weekday_y,
                        width: CELL_SIZE,
                        height: WEEKDAY_ROW_HEIGHT,
                    };
                    renderer.fill_text(
                        text_core::Text {
                            content: w.short_name().to_owned(),
                            bounds: w_bounds.size(),
                            size: Pixels(typography::CAPTION.size),
                            line_height: text_core::LineHeight::Absolute(Pixels(w_bounds.height)),
                            font: renderer.default_font(),
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        w_bounds.center(),
                        self.palette.text_muted,
                        *viewport,
                    );
                }

                // 6x7 Day Grid (42 Cells) (§3.10, `UIF-PRIN-003`, `UIF-PRIN-005`)
                let grid_y = weekday_y + WEEKDAY_ROW_HEIGHT + space::XS;
                let cells =
                    displayed_month_cells(self.model.displayed_month, self.model.first_day_of_week);

                for (idx, cell) in cells.iter().enumerate() {
                    let row = idx / GRID_COLS;
                    let col = idx % GRID_COLS;
                    let cell_rect = Rectangle {
                        x: grid_x + col as f32 * CELL_SIZE,
                        y: grid_y + row as f32 * CELL_SIZE,
                        width: CELL_SIZE,
                        height: CELL_SIZE,
                    };

                    let date = cell.date;
                    let is_disabled = self.model.is_date_disabled(date);
                    let is_today = self.today == Some(date);
                    let is_endpoint = self.model.selection.is_selected_endpoint(date);
                    let is_preview_endpoint = self
                        .model
                        .selection
                        .is_range_preview_endpoint(date, self.model.hover_preview_date);
                    let is_interior = self
                        .model
                        .selection
                        .is_range_interior(date, self.model.hover_preview_date);
                    let is_hovered = state.hovered_date == Some(date) && !is_disabled;

                    // 1. In-range interior background (§3.10, `UIF-PRIN-005`)
                    if is_interior {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cell_rect,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.surface_active),
                        );
                        // Top and bottom 1px border.strong
                        let top_line = Rectangle {
                            x: cell_rect.x,
                            y: cell_rect.y,
                            width: cell_rect.width,
                            height: shape::BORDER_HAIRLINE,
                        };
                        let bottom_line = Rectangle {
                            x: cell_rect.x,
                            y: cell_rect.y + cell_rect.height - shape::BORDER_HAIRLINE,
                            width: cell_rect.width,
                            height: shape::BORDER_HAIRLINE,
                        };
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: top_line,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.border_strong),
                        );
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: bottom_line,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.border_strong),
                        );
                    }

                    // 2. Selected / Range Endpoints background (`accent.fill`, 4.80:1 contrast)
                    if is_endpoint {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cell_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.accent_fill),
                        );
                    } else if is_preview_endpoint {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cell_rect,
                                border: Border::default()
                                    .color(self.palette.accent_border)
                                    .width(shape::BORDER_HAIRLINE)
                                    .rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.surface_active),
                        );
                    } else if is_hovered && !is_interior {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cell_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.surface_active),
                        );
                    }

                    // 3. Today Cue: 1px accent border + bottom 4x2px accent bar (`UIF-PRIN-005`)
                    if is_today && !is_endpoint {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: cell_rect,
                                border: Border::default()
                                    .color(self.palette.accent_border)
                                    .width(shape::BORDER_HAIRLINE)
                                    .rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(Color::TRANSPARENT),
                        );

                        let bar = Rectangle {
                            x: cell_rect.x + (CELL_SIZE - 4.0) / 2.0,
                            y: cell_rect.y + CELL_SIZE - 4.0,
                            width: 4.0,
                            height: 2.0,
                        };
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: bar,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.accent_fill),
                        );
                    }

                    // 4. Day Number Text
                    let text_color = if is_endpoint {
                        self.palette.text_strong
                    } else if is_disabled || !cell.is_current_month {
                        self.palette.text_disabled
                    } else if is_interior || is_preview_endpoint || is_hovered {
                        self.palette.text_strong
                    } else {
                        self.palette.text_primary
                    };

                    renderer.fill_text(
                        text_core::Text {
                            content: format!("{}", date.day),
                            bounds: cell_rect.size(),
                            size: Pixels(typography::BODY.size),
                            line_height: text_core::LineHeight::Absolute(Pixels(cell_rect.height)),
                            font: renderer.default_font(),
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        cell_rect.center(),
                        text_color,
                        *viewport,
                    );

                    // 5. Blackout / Disabled Cue: diagonal strikethrough (`UIF-PRIN-005`)
                    if is_disabled {
                        let strike = Rectangle {
                            x: cell_rect.x + 6.0,
                            y: cell_rect.y + CELL_SIZE / 2.0 - 0.5,
                            width: CELL_SIZE - 12.0,
                            height: 1.0,
                        };
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: strike,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.text_disabled),
                        );
                    }
                }
            }
            CalendarViewMode::Months => {
                let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                let row_h =
                    (flyout_bounds.height - (body_y - flyout_bounds.y) - FLYOUT_PADDING) / 4.0;

                for m in 1..=12 {
                    let r = (m - 1) / 3;
                    let c = (m - 1) % 3;
                    let m_rect = Rectangle {
                        x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                        y: body_y + r as f32 * row_h,
                        width: col_w,
                        height: row_h,
                    };

                    let is_sel = self.model.displayed_month.month == m as u8;
                    let is_hov = state.hovered_month == Some(m as u8);

                    if is_sel {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: m_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.accent_fill),
                        );
                    } else if is_hov {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: m_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.surface_active),
                        );
                    }

                    let color = if is_sel || is_hov {
                        self.palette.text_strong
                    } else {
                        self.palette.text_primary
                    };

                    let label = SHORT_MONTHS[(m - 1) as usize];
                    renderer.fill_text(
                        text_core::Text {
                            content: label.to_owned(),
                            bounds: m_rect.size(),
                            size: Pixels(typography::BODY.size),
                            line_height: text_core::LineHeight::Absolute(Pixels(m_rect.height)),
                            font: renderer.default_font(),
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        m_rect.center(),
                        color,
                        *viewport,
                    );
                }
            }
            CalendarViewMode::Years => {
                let body_y = nav_y + NAV_BUTTON_SIZE + space::SM;
                let col_w = (CALENDAR_FLYOUT_WIDTH - 2.0 * FLYOUT_PADDING) / 3.0;
                let row_h =
                    (flyout_bounds.height - (body_y - flyout_bounds.y) - FLYOUT_PADDING) / 4.0;
                let start_year = (self.model.displayed_month.year / 10) * 10 - 1;

                for i in 0..12i32 {
                    let r = i / 3;
                    let c = i % 3;
                    let y_rect = Rectangle {
                        x: flyout_bounds.x + FLYOUT_PADDING + c as f32 * col_w,
                        y: body_y + r as f32 * row_h,
                        width: col_w,
                        height: row_h,
                    };

                    let year = start_year + i;
                    let is_sel = self.model.displayed_month.year == year;
                    let is_hov = state.hovered_year == Some(year);

                    if is_sel {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: y_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.accent_fill),
                        );
                    } else if is_hov {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: y_rect,
                                border: Border::default().rounded(shape::RADIUS_CARD),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Background::Color(self.palette.surface_active),
                        );
                    }

                    let color = if is_sel || is_hov {
                        self.palette.text_strong
                    } else {
                        self.palette.text_primary
                    };

                    renderer.fill_text(
                        text_core::Text {
                            content: format!("{year}"),
                            bounds: y_rect.size(),
                            size: Pixels(typography::BODY.size),
                            line_height: text_core::LineHeight::Absolute(Pixels(y_rect.height)),
                            font: renderer.default_font(),
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        y_rect.center(),
                        color,
                        *viewport,
                    );
                }
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<DatePicker<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(picker: DatePicker<'a, Message>) -> Self {
        Element::new(picker)
    }
}

#[cfg(test)]
#[path = "date_picker_tests.rs"]
mod date_picker_tests;
