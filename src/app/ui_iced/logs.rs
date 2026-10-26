//! Background-process log viewer (UI-047).
//!
//! # One ring buffer, read-only
//!
//! The rows rendered here come from a single source: [`BackgroundLogs`]
//! (`src/app/background/logs.rs:262-337`) — a
//! `VecDeque<BackgroundLogEntry>` bounded by `MAX_LOG_ENTRIES` (10 000,
//! `logs.rs:14`) shared as `SharedBackgroundLogs = Arc<Mutex<BackgroundLogs>>`
//! (`logs.rs:339`). REQ-460..465 own that buffer and this module leaves them
//! untouched: [`BackgroundLogsViewModel::from_logs`] takes `&BackgroundLogs`
//! and calls only [`BackgroundLogs::get_logs`] (`logs.rs:301`). It never
//! calls `push_log` / `clear_logs`, never locks, never reads a file and never
//! touches the bus — the reducer snapshots the buffer and hands the snapshot
//! down.
//!
//! # Reused, not reimplemented ([RUST-051])
//!
//! The filter predicate is [`is_log_visible`] / [`filter_logs`]. Neither takes
//! a UI-framework type, so the pair lives here and every caller filters
//! through literally the same code, which keeps UI-047's filtering behaviour
//! identical everywhere it is used.
//!
//! # Auto-scroll is an intent, not an action
//!
//! iced 0.14 has no built-in stick-to-bottom flag — programmatic scrolling is
//! a runtime *operation* dispatched from `update`
//! (`iced::widget::operation::scrollable::scroll_to`,
//! `iced_runtime-0.14.0/src/widget/operation.rs:31-39`). The pure layer
//! therefore computes **whether** to follow and the reducer performs it:
//!
//! ```text
//! Scrollable::on_scroll ─▶ Message::Logs(ScrollReported { .. })   (view emits)
//!                                   │
//!              should_follow(auto_scroll, Some(snapshot))          (pure decision)
//!                                   │
//!   scroll_to(LOG_SCROLLABLE_ID, AbsoluteOffset { y: snapshot.max_offset() })
//!                                                         (reducer Task)
//! ```
//!
//! The rule: follow only while the preference is on **and** the viewport was
//! already at the end — or the content is shorter than the viewport, the
//! negative-offset "stuck" case. A user who has scrolled up is never yanked
//! back down. With no report yet ([`should_follow`] gets `None`) the intent is
//! *false*: the view must not invent a scroll position it has never been told
//! about.
//!
//! # Row colour tokens — and why there is no severity mapping
//!
//! There is no level to map: [`BackgroundLogEntry`] carries `timestamp`,
//! `category` and `message` only (`src/app/bus/events/messages.rs:63-68`) —
//! no severity field exists in the ring buffer. Every row is painted with the
//! same two tokens (`palette.log_timestamp` on the time column,
//! `palette.log_category` on the `[Category]` tag, default body text for the
//! message). [`category_color`] is still an exhaustive `match` over all seven
//! [`LogCategory`] variants so the choice is explicit and a future variant is
//! a compile error rather than a silent fall-through ([RUST-041] drift note).
//! No numeric colour is introduced anywhere.
//!
//! # Verified iced 0.14 APIs (local registry sources)
//!
//! | API | Citation |
//! | --- | --- |
//! | `table(columns, rows)` / `column(header, view)` | `iced_widget-0.14.2/src/table.rs:18-28` / `:34-49` |
//! | `Column::width` | `iced_widget-0.14.2/src/table.rs:650-653` |
//! | `table::Style` carries only `separator_x`/`separator_y` | `iced_widget-0.14.2/src/table.rs:676-681` |
//! | `scrollable(...)` builder: `.id` / `.height` / `.on_scroll` | `iced_widget-0.14.2/src/scrollable.rs:144-168` |
//! | `scrollable::Viewport::{absolute_offset, bounds, content_bounds}` | `iced_widget-0.14.2/src/scrollable.rs:1739-1782` |
//! | `checkbox(is_checked)` + `.label` + `.on_toggle` | `iced_widget-0.14.2/src/helpers.rs:1281-1289`, `checkbox.rs:145`, `:155` |
//! | `widget::Id: From<&'static str>` | `iced_core-0.14.0/src/widget/id.rs:26-30` |
//! | reducer-side `operation::scrollable::{scroll_to, snap_to_end}` | `iced_runtime-0.14.0/src/widget/operation.rs:23-39` |
//!
//! Unit tests live in the sibling `logs_tests.rs` sidecar.

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;

use iced::widget::{
    Column, Row, button, checkbox, container, pick_list, rule, scrollable, table, text,
};
use iced::{Alignment, Element, Length};
use iced_pro::widget::text_input::{InputKind, TextInput};

use crate::background::logs::BackgroundLogs;
use crate::bus::events::messages::{BackgroundLogEntry, LogCategory};
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{LogsMessage, Message};
use iced_pro::surface::movable::{AutoScrollSetting, FollowState, ScrollGeometry};
use iced_pro::token::shape;
use iced_pro::token::space;
use iced_pro::token::typography::{self, TypeRung};

/// Log filter predicate.
///
/// Determines if a log entry matches the given category and search text.
/// Inputs: `log` (the log entry), `category` (optional filter category), `search_lower` (lowercase search string).
/// Outputs: `bool` - true if the log passes the filters.
/// Purity: Pure.
/// Preconditions: `search_lower` must be pre-lowercased for optimal performance.
/// Postconditions: Returns deterministic boolean based on exact category match and substring search match.
#[must_use]
pub fn is_log_visible(
    log: &BackgroundLogEntry,
    category: Option<LogCategory>,
    search_lower: &str,
) -> bool {
    if let Some(cat) = category
        && log.category != cat
    {
        return false;
    }
    if !search_lower.is_empty() && !log.message.to_lowercase().contains(search_lower) {
        return false;
    }
    true
}

/// Filters an iterator of log entries based on category and search text.
/// Inputs: `logs` (iterator of log entries), `category` (optional filter), `search_text` (raw search string).
/// Outputs: `Vec<BackgroundLogEntry>` - a collection of cloned log entries that match the criteria.
/// Purity: Pure.
/// Preconditions: `logs` is a valid iterator.
/// Postconditions: Returns a freshly allocated `Vec` containing only the logs that passed `is_log_visible`.
pub fn filter_logs<'a>(
    logs: impl Iterator<Item = &'a BackgroundLogEntry>,
    category: Option<LogCategory>,
    search_text: &str,
) -> Vec<BackgroundLogEntry> {
    let search_lower = search_text.to_lowercase();
    logs.filter(|log| is_log_visible(log, category, &search_lower))
        .cloned()
        .collect()
}

/// `chrono` format of the timestamp column: `HH:MM:SS.mmm`.
pub const TIMESTAMP_FORMAT: &str = "%H:%M:%S%.3f";

/// The rung every log row paints at — `type.body` (`UIF-TOK-004`).
pub const ROW_RUNG: TypeRung = typography::BODY;

/// Logical point size of every log row.
pub const ROW_TEXT_SIZE: f32 = ROW_RUNG.size;

/// Horizontal padding inside a log-table cell — `space.xs` (`UIF-TOK-001`).
pub const CELL_PADDING_X: f32 = space::XS;

/// Vertical padding inside a log-table cell.
///
/// `1.0` is not a `space` rung — `space.2xs` is `2.0` and there is nothing
/// between it and zero. Binding it would move every log row by a pixel,
/// which is a density re-tune of the Logs panel rather than a token
/// binding, and that re-tune is not this increment's call. Recorded as an
/// enumerated exception in
/// [`APP_ALLOW_LIST`](crate::ui_iced::token_lint_gate::APP_ALLOW_LIST) so
/// the next pass over Logs inherits a named list instead of an unmeasured
/// one.
pub const CELL_PADDING_Y: f32 = 1.0;

/// Padding around the whole viewer.
///
/// `6.0` sits exactly between `space.xs` (4) and `space.sm` (8) and is on
/// neither. INT-7c broke that tie on *role* everywhere else — `SM` for
/// sibling gaps, `XS` for inner padding — but the viewer's outer padding
/// is neither, and moving it changes the panel's whole measure. Enumerated
/// in
/// [`APP_ALLOW_LIST`](crate::ui_iced::token_lint_gate::APP_ALLOW_LIST)
/// rather than guessed at.
pub const PANEL_PADDING: f32 = 6.0;

/// Spacing between the controls in the filter row.
pub const FILTER_SPACING: f32 = 6.0;

/// Width of the horizontal rule under the filter row —
/// `shape.border.hairline` (`UIF-TOK-003`).
pub const SEPARATOR_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Slack, in logical points, allowed when deciding whether the viewport is
/// "at the bottom".
///
/// iced reports the offset after its own clamping and rounding, so an exact
/// `f32` comparison would drop the stick on a sub-pixel difference; 1 pt is
/// far below what a user can perceive and far above the accumulation error
/// of a frame's worth of rows.
///
/// # Delta against the framework's `AT_BOTTOM_TOLERANCE`
///
/// `iced_pro::surface::movable::AT_BOTTOM_TOLERANCE` is `space::XXS` =
/// **2.0 pt**. This margin stays **1.0 pt**, deliberately narrower, and the
/// follow decision therefore uses this value rather than the framework's
/// `ScrollGeometry::at_bottom`. Adopting 2.0 would widen the band in which
/// a viewport still counts as "at the tail", so a log the user has scrolled
/// up by 1.5 pt would keep being yanked down — precisely the behaviour
/// `logs_tests::just_short_of_the_end_is_at_bottom` and
/// `logs_tests::past_the_margin_is_not_at_bottom` pin from the other side
/// (the latter asserts a viewport 2.0 pt short is **not** at the bottom,
/// which under the framework's tolerance would flip to at-bottom).
///
/// Widening it is a behaviour change that needs its own decision; INT-6
/// unifies the decision *matrix* (`FollowState::resolve`) without silently
/// moving the position threshold underneath the existing pins.
pub const BOTTOM_STICK_TOLERANCE: f32 = 1.0;

/// Stable iced widget id of the log list's `scrollable`.
///
/// The reducer needs a stable id to aim a
/// `operation::scrollable::scroll_to` at
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`); a widget without
/// an explicit id cannot be scrolled programmatically at all. Named once
/// here so the view and the reducer cannot drift apart ([RUST-021]).
pub const LOG_SCROLLABLE_ID: &str = "background_logs_scroll";

/// Stable focus id for the background-log search input (UI-047).
pub const LOG_SEARCH_INPUT_FOCUS_ID: &str = "background_logs_search_input";

/// Formatted timestamp of a log row ([`TIMESTAMP_FORMAT`]).
///
/// Purity: pure function of `timestamp`.
#[must_use]
pub fn timestamp_text(timestamp: &chrono::DateTime<chrono::Local>) -> String {
    timestamp.format(TIMESTAMP_FORMAT).to_string()
}

/// The bracketed category tag of a log row, e.g. `[Indexer]`.
///
/// The label comes from [`LogCategory`]'s own `Display` impl
/// (`src/app/bus/events/messages.rs:41-54`), so no label literal is added
/// to this layer and every category the buffer can produce is covered by
/// construction.
///
/// Purity: pure function of `category`.
#[must_use]
pub fn category_tag(category: LogCategory) -> String {
    format!("[{category}]")
}

/// Colour token of the category tag.
///
/// The single `log_category` token is used for **every** category; there is
/// no per-severity colour in the data and none is invented here. The
/// `match` is exhaustive over all seven [`LogCategory`] variants anyway, so
/// differentiating a category later is a deliberate, compiler-checked edit
/// rather than a fall-through.
///
/// Purity: pure function of `category`.
#[must_use]
pub fn category_color(category: LogCategory) -> iced::Color {
    let palette = Palette::fastmd_dark();
    match category {
        LogCategory::Indexer
        | LogCategory::Watcher
        | LogCategory::PdfConverter
        | LogCategory::ImageVision
        | LogCategory::LlmTools
        | LogCategory::Print
        | LogCategory::Batch => palette.log_category,
    }
}

/// Colour token of the timestamp column (`palette.log_timestamp`).
///
/// Purity: pure.
#[must_use]
pub fn timestamp_color() -> iced::Color {
    Palette::fastmd_dark().log_timestamp
}

/// Colour token of the message column (the default body `text` token; no
/// colour is applied to the message label beyond it).
///
/// Purity: pure.
#[must_use]
pub fn message_color() -> iced::Color {
    Palette::fastmd_dark().text
}

/// One entry in the category filter pick list (UI-047's category combo).
///
/// The `category` field — not the label — is what the message carries, so a
/// label can never be mistaken for the "show everything" entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryFilterOption {
    /// Label rendered on the combo button and in the dropdown.
    pub label: String,
    /// The category this option selects; `None` shows every category.
    pub category: Option<LogCategory>,
}

impl CategoryFilterOption {
    /// The "show every category" entry ([`strings::LOG_CATEGORY_ALL`]).
    #[must_use]
    pub fn all() -> Self {
        Self {
            label: strings::LOG_CATEGORY_ALL.to_string(),
            category: None,
        }
    }

    /// A single-category entry labelled with `label`.
    #[must_use]
    pub fn for_category(category: LogCategory, label: &str) -> Self {
        Self {
            label: label.to_string(),
            category: Some(category),
        }
    }

    /// The intent this option publishes when picked.
    #[must_use]
    pub fn command(&self) -> Message {
        Message::Logs(LogsMessage::CategoryFilterChanged(self.category))
    }
}

impl std::fmt::Display for CategoryFilterOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// The category filter options, in display order: "All" first, then the
/// five categories the combo offers, each labelled from
/// [`crate::ui::strings`].
///
/// `LogCategory::Print` and `LogCategory::Batch` are *not* offered here,
/// even though the ring buffer can produce both (`logs.rs:51-54` routes
/// `print`/`batch` targets into them). Their rows still render and are
/// reachable through the "All" view and the text search. Widening the list
/// is a product decision — it needs two new `strings` constants.
#[must_use]
pub fn category_filter_options() -> Vec<CategoryFilterOption> {
    vec![
        CategoryFilterOption::all(),
        CategoryFilterOption::for_category(LogCategory::Indexer, strings::LOG_CATEGORY_INDEXER),
        CategoryFilterOption::for_category(LogCategory::Watcher, strings::LOG_CATEGORY_WATCHER),
        CategoryFilterOption::for_category(
            LogCategory::PdfConverter,
            strings::LOG_CATEGORY_PDF_CONVERTER,
        ),
        CategoryFilterOption::for_category(
            LogCategory::ImageVision,
            strings::LOG_CATEGORY_IMAGE_VISION,
        ),
        CategoryFilterOption::for_category(LogCategory::LlmTools, strings::LOG_CATEGORY_LLM_TOOLS),
    ]
}

/// The vertical scroll geometry of one frame of the log list, in logical
/// points.
///
/// A plain data mirror of `iced::widget::scrollable::Viewport`
/// (`iced_widget-0.14.2/src/scrollable.rs:1730-1782`) so the auto-scroll
/// rule is testable: `Viewport`'s own fields are private and it has no public
/// constructor, so a unit test cannot build one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollSnapshot {
    /// How far the content is scrolled down from the top.
    pub offset_y: f32,
    /// Height of the visible viewport.
    pub viewport_height: f32,
    /// Total height of the scrolled content.
    pub content_height: f32,
}

impl ScrollSnapshot {
    /// Offset of the last row's bottom edge: `content - viewport`, floored
    /// at `0.0` so content shorter than the viewport cannot go negative.
    ///
    /// This is the `y` the reducer passes to
    /// `operation::scrollable::scroll_to` to reach the bottom.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Whether every row already fits inside the viewport (nothing to scroll).
    ///
    /// Purity: pure.
    #[must_use]
    pub fn fits(&self) -> bool {
        self.content_height <= self.viewport_height
    }

    /// Whether the viewport sits at the end of the content.
    ///
    /// True when the content fits ([`Self::fits`]) — nothing left to scroll —
    /// or when the offset is within [`BOTTOM_STICK_TOLERANCE`] of
    /// [`Self::max_offset`].
    ///
    /// Purity: pure.
    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.fits() || self.offset_y >= self.max_offset() - BOTTOM_STICK_TOLERANCE
    }

    /// Whether this frame's geometry should be treated as "the user is
    /// following the tail" given the auto-scroll preference — the full
    /// follow rule, as a method for call-site convenience.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn follows_with(&self, auto_scroll: bool) -> bool {
        should_follow(auto_scroll, Some(*self))
    }

    /// The same frame as the framework's [`ScrollGeometry`], for handing to
    /// [`MovableHost::on_scroll`](iced_pro::surface::movable::MovableHost).
    ///
    /// Note the field order differs from this struct's: the framework's
    /// constructor is `(offset, content, viewport)`, not
    /// `(offset, viewport, content)`. Mapping it here rather than at each
    /// call site keeps that transposition from being re-derived wrong
    /// ([RUST-021]).
    ///
    /// Purity: pure.
    #[must_use]
    pub const fn to_geometry(&self) -> ScrollGeometry {
        ScrollGeometry::new(self.offset_y, self.content_height, self.viewport_height)
    }
}

/// Captures the vertical geometry of an iced scrollable viewport report.
///
/// Called from `Scrollable::on_scroll`; reads only, allocates nothing but
/// the snapshot.
///
/// Purity: pure function of `viewport`.
#[must_use]
pub fn scroll_snapshot(viewport: &scrollable::Viewport) -> ScrollSnapshot {
    ScrollSnapshot {
        offset_y: viewport.absolute_offset().y,
        viewport_height: viewport.bounds().height,
        content_height: viewport.content_bounds().height,
    }
}

/// The auto-scroll preference translated into the framework's vocabulary
/// (`UIF-MOV-004`).
///
/// The app has carried this as a `bool` since before `iced-pro` named it;
/// going through this one function means the translation exists in exactly
/// one place ([RUST-021]) and the framework's `AutoScrollSetting` is what
/// every follow decision is actually made against.
///
/// Purity: pure.
#[must_use]
pub const fn auto_scroll_setting(auto_scroll: bool) -> AutoScrollSetting {
    if auto_scroll {
        AutoScrollSetting::Enabled
    } else {
        AutoScrollSetting::Disabled
    }
}

/// The follow rule as the framework's own [`FollowState`] (`UIF-MOV-004`).
///
/// Same matrix the framework defines — following requires *both* permission
/// (the setting is on) and position (the viewport is at the tail) — with the
/// position test still supplied by [`ScrollSnapshot::is_at_bottom`] and its
/// [`BOTTOM_STICK_TOLERANCE`]. See that constant for why the app's margin
/// stays tighter than the framework's
/// [`AT_BOTTOM_TOLERANCE`](iced_pro::surface::movable::AT_BOTTOM_TOLERANCE).
///
/// Exposed alongside [`should_follow`] because the two questions are
/// different: `should_follow` answers "scroll now?", `follow_state` answers
/// "what state is this view in", which is what the footer's
/// "N behind" affordance needs.
///
/// Purity: pure.
#[must_use]
pub fn follow_state(auto_scroll: bool, snapshot: Option<ScrollSnapshot>) -> FollowState {
    let at_bottom = snapshot.is_some_and(|viewport| viewport.is_at_bottom());
    FollowState::resolve(auto_scroll_setting(auto_scroll), at_bottom)
}

/// The auto-scroll **intent**: should the reducer scroll the log list to the
/// end this frame?
///
/// `true` only when the preference is on **and** the viewport was already at
/// the end: a user who has scrolled up to read history is never yanked back
/// down, and turning the preference off stops the follow immediately.
///
/// `None` (no viewport report yet) is deliberately **false** — with no
/// geometry there is nothing to follow, and scrolling blind would fight the
/// widget's own initial layout.
///
/// As of INT-6 this is [`follow_state`]'s projection to a bool rather than a
/// locally-written `&&`: the decision matrix is the framework's
/// (`UIF-MOV-004`), and this layer contributes only the position test.
/// Every existing pin — at-bottom follows, scrolled-up does not, setting off
/// does not, no report does not — is exactly one of the framework's four
/// cells, so the unification changes no behaviour.
///
/// Executing the intent is reducer work:
/// `operation::scrollable::scroll_to(LOG_SCROLLABLE_ID.into(),
/// AbsoluteOffset { x: 0.0, y: snapshot.max_offset() })`
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`), or
/// `snap_to_end` for the same result without computing the offset
/// (`:23-28`).
///
/// Purity: pure.
#[must_use]
pub fn should_follow(auto_scroll: bool, snapshot: Option<ScrollSnapshot>) -> bool {
    follow_state(auto_scroll, snapshot).is_following()
}

/// Everything the log viewer needs to render, snapshotted off the ring
/// buffer.
///
/// Mirrors [`super::panels::top::TopBarViewModel`]: the widget tree reads
/// only this struct, which is what makes the viewer unit-testable without a
/// live iced runtime, a bus or a background worker.
///
/// `Clone + Debug` only: [`BackgroundLogEntry`] is a bus type that derives no
/// `PartialEq` and REQ-460..465 keep it that way, so the snapshot cannot
/// derive it either — the sidecar tests assert field by field instead.
#[derive(Clone, Debug)]
pub struct BackgroundLogsViewModel {
    /// Whether the Background Processes window is currently shown
    /// (`BackgroundLogs::show_background_logs`).
    pub visible: bool,
    /// The entries that passed the filter, in ring-buffer order (oldest
    /// first, newest last — the order the tail of the list is read in).
    pub entries: Vec<BackgroundLogEntry>,
    /// Total entries in the buffer before filtering, so the UI can show how
    /// much the filter hid rather than hiding the loss.
    pub total_entries: usize,
    /// The free-text search filter.
    pub search_text: String,
    /// The category filter, `None` = every category.
    pub category_filter: Option<LogCategory>,
    /// The pick-list options, owned by the snapshot so the selected option
    /// can be borrowed out of it for `pick_list` (same shape as
    /// [`super::panels::top::TopBarViewModel::tag_options`]).
    pub category_options: Vec<CategoryFilterOption>,
    /// The stick-to-bottom preference.
    pub auto_scroll: bool,
    /// The buffer could not be read (poisoned lock). The view then renders
    /// [`strings::BACKGROUND_MGR_ACCESS_ERROR`] instead of a table.
    pub access_error: bool,
}

impl BackgroundLogsViewModel {
    /// Snapshots a ring-buffer borrow.
    ///
    /// Read-only: calls [`BackgroundLogs::get_logs`] and reads the public
    /// filter fields.
    ///
    /// Purity: read-only over `logs`.
    #[must_use]
    pub fn from_logs(logs: &BackgroundLogs) -> Self {
        Self {
            visible: logs.show_background_logs,
            entries: filter_logs(
                logs.get_logs().iter(),
                logs.filter_category,
                &logs.search_text,
            ),
            total_entries: logs.get_logs().len(),
            search_text: logs.search_text.clone(),
            category_filter: logs.filter_category,
            category_options: category_filter_options(),
            auto_scroll: logs.auto_scroll,
            access_error: false,
        }
    }

    /// The snapshot for a closed log viewer.
    ///
    /// The counters and filter settings ride along so the header and pick-lists
    /// stay coherent, but the whole-buffer clone [`Self::from_logs`] performs is
    /// skipped. The ring buffer holds up
    /// [`MAX_LOG_ENTRIES`](crate::background::logs::MAX_LOG_ENTRIES) entries and
    /// a workspace scan fills it, so cloning it for a window nobody is looking at
    /// is both wasted work and needless contention with the worker thread that
    /// pushes into the same mutex.
    ///
    /// Purity: read-only over `logs`.
    #[must_use]
    pub fn closed(logs: &BackgroundLogs) -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            total_entries: logs.get_logs().len(),
            search_text: logs.search_text.clone(),
            category_filter: logs.filter_category,
            category_options: category_filter_options(),
            auto_scroll: logs.auto_scroll,
            access_error: false,
        }
    }

    /// Snapshots the buffer off `app`.
    ///
    /// A poisoned lock degrades to [`Self::unavailable`] rather than
    /// panicking, so one wedged worker cannot take the UI down.
    ///
    /// While the viewer is closed the entry clone is skipped — see
    /// [`Self::closed`].
    #[must_use]
    pub fn from_app(app: &FastMdApp) -> Self {
        match app.orchestrator.background_manager.lock() {
            Ok(manager) if manager.show_background_logs => Self::from_logs(&manager),
            Ok(manager) => Self::closed(&manager),
            Err(_) => Self::unavailable(),
        }
    }

    /// The fail-closed snapshot used when the buffer cannot be read.
    ///
    /// `visible` is `false` on purpose: an unreadable buffer must not force a
    /// window open that the user had closed. `access_error` still rides along
    /// so a viewer that *is* mounted says why it is empty.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            total_entries: 0,
            search_text: String::new(),
            category_filter: None,
            category_options: category_filter_options(),
            auto_scroll: false,
            access_error: true,
        }
    }

    /// Whether the filtered list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many entries the current filter hid from view.
    #[must_use]
    pub fn filtered_out(&self) -> usize {
        self.total_entries.saturating_sub(self.entries.len())
    }

    /// The [`CategoryFilterOption`] matching [`Self::category_filter`], if
    /// that category is still offered in the pick list.
    ///
    /// Borrows out of [`Self::category_options`] rather than rebuilding the
    /// list, so the reference is valid for the snapshot's lifetime — which is
    /// what `pick_list` needs (`V: Borrow<T> + 'a`,
    /// `iced_widget-0.14.2/src/pick_list.rs:179-195`).
    #[must_use]
    pub fn category_filter_option(&self) -> Option<&CategoryFilterOption> {
        self.category_options
            .iter()
            .find(|option| option.category == self.category_filter)
    }

    /// The auto-scroll intent for this snapshot's preference and the last
    /// reported viewport geometry.
    #[must_use]
    pub fn follow_intent(&self, snapshot: Option<ScrollSnapshot>) -> bool {
        should_follow(self.auto_scroll, snapshot)
    }

    /// Renders the viewer ([`background_logs_view`]) from this snapshot.
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        background_logs_view(self)
    }
}

/// Renders the background-process log viewer from a snapshot (UI-047).
///
/// Layout:
///
/// ```text
/// Search: [__________] Category: [All ▾]  [x] Auto-scroll   [Clear]
/// ────────────────────────────────────────────────────────────────
/// 12:04:31.220  [Indexer]  Indexing workspace...
/// 12:04:31.480  [Watcher]  File modified: note.md
/// ```
///
/// Emitted messages:
/// * search box → [`LogsMessage::SearchChanged`]
/// * category pick → [`LogsMessage::CategoryFilterChanged`]
/// * auto-scroll checkbox → [`LogsMessage::AutoScrollToggled`]
/// * Clear → [`LogsMessage::Cleared`]
/// * any scroll of the list → [`LogsMessage::ScrollReported`], the input
///   the reducer feeds to [`should_follow`]
///
/// The filter row, the rule and the scrollable table are **always
/// allocated** — an empty list renders an empty table rather than a
/// different widget tree, so the shape never changes across the
/// empty/non-empty transition (AGENTS §6).
#[must_use]
pub fn background_logs_view(view_model: &BackgroundLogsViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let search_label = text(strings::SEARCH_LABEL)
        .size(ROW_TEXT_SIZE)
        .color(palette.text);

    let search_field = TextInput::new(
        InputKind::TextEntry,
        String::new(),
        &view_model.search_text,
        |text| Message::Logs(LogsMessage::SearchChanged { text }),
        LOG_SEARCH_INPUT_FOCUS_ID,
    )
    .compact()
    .width(Length::Fill)
    .palette(Palette::pro());

    let category_pick: Element<'_, Message> = pick_list(
        view_model.category_options.clone(),
        view_model.category_filter_option(),
        |option| option.command(),
    )
    .placeholder(strings::LOG_CATEGORY_ALL)
    .width(Length::Shrink)
    .into();

    let auto_scroll_toggle = checkbox(view_model.auto_scroll)
        .label(strings::AUTO_SCROLL_CHECKBOX)
        .on_toggle(|checked| Message::Logs(LogsMessage::AutoScrollToggled(checked)));

    let clear_button =
        button(text(strings::CLEAR_BUTTON)).on_press(Message::Logs(LogsMessage::Cleared));

    let filter_row = Row::with_children(vec![
        search_label.into(),
        search_field.into(),
        text(strings::CATEGORY_LABEL)
            .size(ROW_TEXT_SIZE)
            .color(palette.text)
            .into(),
        category_pick,
        auto_scroll_toggle.into(),
        clear_button.into(),
    ])
    .spacing(FILTER_SPACING)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = if view_model.access_error {
        container(
            text(strings::BACKGROUND_MGR_ACCESS_ERROR)
                .size(ROW_TEXT_SIZE)
                .color(palette.error),
        )
        .padding(PANEL_PADDING)
        .into()
    } else {
        log_table(&view_model.entries)
    };

    Column::with_children(vec![
        filter_row.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        body,
    ])
    .spacing(PANEL_PADDING)
    .padding(PANEL_PADDING)
    .into()
}

/// The log table inside its scrollable ([`LOG_SCROLLABLE_ID`]).
///
/// Three columns — timestamp, `[Category]`, message — built with
/// `iced::widget::table`.
///
/// The message column is `Length::Fill` explicitly: iced forces the *first*
/// all-`Shrink` column to `Fill` instead
/// (`iced_widget-0.14.2/src/table.rs:129-133`), which would pin the
/// timestamp wide and starve the message.
///
/// iced's `table` requires a header element per column, so the headers are
/// empty text — allocated, not removed (AGENTS §6).
///
/// Purity: pure over `entries`; the only effect is the `ScrollReported`
/// message the scrollable emits when the user scrolls.
#[must_use]
pub fn log_table(entries: &[BackgroundLogEntry]) -> Element<'static, Message> {
    let empty_header = || text(String::new()).size(ROW_TEXT_SIZE);

    let columns = vec![
        table::column(
            empty_header(),
            move |entry: BackgroundLogEntry| -> Element<'static, Message> {
                text(timestamp_text(&entry.timestamp))
                    .size(ROW_TEXT_SIZE)
                    .color(timestamp_color())
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            empty_header(),
            move |entry: BackgroundLogEntry| -> Element<'static, Message> {
                text(category_tag(entry.category))
                    .size(ROW_TEXT_SIZE)
                    .color(category_color(entry.category))
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            empty_header(),
            move |entry: BackgroundLogEntry| -> Element<'static, Message> {
                text(entry.message)
                    .size(ROW_TEXT_SIZE)
                    .color(message_color())
                    .into()
            },
        )
        .width(Length::Fill),
    ];

    let grid = table::table(columns, entries.to_vec())
        .padding_x(CELL_PADDING_X)
        .padding_y(CELL_PADDING_Y)
        .separator_x(0.0)
        .separator_y(0.0)
        .width(Length::Fill);

    scrollable(grid)
        .id(LOG_SCROLLABLE_ID)
        .height(Length::Fill)
        .on_scroll(|viewport| {
            let snapshot = scroll_snapshot(&viewport);
            Message::Logs(LogsMessage::ScrollReported {
                offset_y: snapshot.offset_y,
                viewport_height: snapshot.viewport_height,
                content_height: snapshot.content_height,
            })
        })
        .into()
}
