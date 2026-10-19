//! Background-process log viewer — the iced port of
//! `src/app/ui/background_logs.rs` ([TASK-016], UI-047).
//!
//! # Same ring buffer, read-only
//!
//! The rows rendered here come from the one source the legacy UI build read:
//! [`BackgroundLogs`] (`src/app/background/logs.rs:262-337`) — a
//! `VecDeque<BackgroundLogEntry>` bounded by `MAX_LOG_ENTRIES` (10 000,
//! `logs.rs:14`) shared as `SharedBackgroundLogs = Arc<Mutex<BackgroundLogs>>`
//! (`logs.rs:339`). REQ-460..465 own that buffer and this module leaves them
//! untouched: [`BackgroundLogsViewModel::from_logs`] takes `&BackgroundLogs`
//! and calls only [`BackgroundLogs::get_logs`] (`logs.rs:301`). It never
//! calls `push_log` / `clear_logs`, never locks, never reads a file and never
//! touches the bus — the [TASK-022]/[TASK-023] reducer snapshots the buffer
//! and hands the snapshot down ([FMD-065], [FMD-069]).
//!
//! # Reused, not reimplemented ([RUST-051])
//!
//! The filter predicate is the legacy UI build's own [`is_log_visible`] and
//! [`filter_logs`] (`src/app/ui/background_logs.rs:15-46`). Neither takes an
//! the legacy UI type, so they are re-exported here rather than copied: both UIs
//! filter through literally the same code, which is what keeps UI-047's
//! filtering behaviour identical across the migration.
//!
//! # Auto-scroll is an intent, not an action
//!
//! the legacy UI got stick-to-bottom for free from `ScrollArea::stick_to_bottom`
//! (`legacy-0.36.1/src/containers/scroll_area.rs:655-658`). iced 0.14 has no
//! such flag — programmatic scrolling is a runtime *operation* dispatched
//! from `update` (`iced::widget::operation::scrollable::scroll_to`,
//! `iced_runtime-0.14.0/src/widget/operation.rs:31-39`). The pure layer
//! therefore computes **whether** to follow and the reducer performs it:
//!
//! ```text
//! Scrollable::on_scroll ─▶ Message::Logs(ScrollReported { .. })   (view emits)
//!                                   │
//!              should_follow(auto_scroll, Some(snapshot))          (pure decision)
//!                                   │
//!   scroll_to(LOG_SCROLLABLE_ID, AbsoluteOffset { y: snapshot.max_offset() })
//!                                                    (TASK-022/023 reducer Task)
//! ```
//!
//! The decision reproduces the legacy UI's rule exactly: follow only while the
//! preference is on **and** the viewport was already at the end — or the
//! content is shorter than the viewport, the legacy UI's `available_offset < 0`
//! "stuck" case (`scroll_area.rs:1282-1284`, stuck recompute at
//! `:1543-1550`). A user who has scrolled up is never yanked back down.
//! With no report yet ([`should_follow`] gets `None`) the intent is *false*:
//! the view must not invent a scroll position it has never been told about.
//!
//! # Row colour tokens — and why there is no severity mapping
//!
//! The task brief asked for "level → colour". There is no level to map:
//! [`BackgroundLogEntry`] carries `timestamp`, `category` and `message` only
//! (`src/app/bus/events/messages.rs:63-68`) — no severity field exists in the
//! ring buffer, and the legacy UI viewer painted **every** row with the same two
//! tokens (`palette.log_timestamp` on the time column, `palette.log_category`
//! on the `[Category]` tag, default body text for the message;
//! `src/app/ui/background_logs.rs:138-151`). [`category_color`] is still an
//! exhaustive `match` over all seven [`LogCategory`] variants so the parity
//! choice is explicit and a future variant is a compile error rather than a
//! silent fall-through ([RUST-041] drift note). No numeric colour is
//! introduced anywhere ([GUD-003]).
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
    Column, Row, button, checkbox, container, pick_list, rule, scrollable, table, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::background::logs::BackgroundLogs;
use crate::bus::events::messages::{BackgroundLogEntry, LogCategory};
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{LogsMessage, Message};

/// Log filter predicate — owned by this module since [TASK-029b].
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

/// Filters an iterator of log entries based on category and search text —
/// owned by this module since [TASK-029b].
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

/// `chrono` format of the timestamp column, carried over verbatim from the
/// the legacy UI viewer (`log.timestamp.format("%H:%M:%S%.3f")`,
/// `src/app/ui/background_logs.rs:142`).
pub const TIMESTAMP_FORMAT: &str = "%H:%M:%S%.3f";

/// Logical point size of every log row (the legacy UI rendered at the `Body` text
/// style; iced needs the size stated explicitly).
pub const ROW_TEXT_SIZE: f32 = 14.0;

/// Horizontal padding inside a log-table cell.
pub const CELL_PADDING_X: f32 = 4.0;

/// Vertical padding inside a log-table cell.
pub const CELL_PADDING_Y: f32 = 1.0;

/// Padding around the whole viewer.
pub const PANEL_PADDING: f32 = 6.0;

/// Spacing between the controls in the filter row.
pub const FILTER_SPACING: f32 = 6.0;

/// Width of the horizontal rule under the filter row (the legacy UI's
/// `ui.separator()`).
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Slack, in logical points, allowed when deciding whether the viewport is
/// "at the bottom".
///
/// the legacy UI compared the offset to the available offset with exact `f32`
/// equality (`scroll_area.rs:1545`). iced reports the offset after its own
/// clamping and rounding, so an exact comparison would drop the stick on a
/// sub-pixel difference; 1 pt is far below what a user can perceive and far
/// above the accumulation error of a frame's worth of rows.
pub const BOTTOM_STICK_TOLERANCE: f32 = 1.0;

/// Stable iced widget id of the log list's `scrollable`.
///
/// The [TASK-022]/[TASK-023] reducer needs a stable id to aim a
/// `operation::scrollable::scroll_to` at
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`); a widget without
/// an explicit id cannot be scrolled programmatically at all. Named once
/// here so the view and the reducer cannot drift apart ([RUST-021]).
pub const LOG_SCROLLABLE_ID: &str = "background_logs_scroll";

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
/// (`src/app/bus/events/messages.rs:41-54`) — the same one the legacy UI row
/// formatted (`format!("[{}]", log.category)`,
/// `src/app/ui/background_logs.rs:147`) — so no label literal is added to
/// this layer ([GUD-002]) and every category the buffer can produce is
/// covered by construction.
///
/// Purity: pure function of `category`.
#[must_use]
pub fn category_tag(category: LogCategory) -> String {
    format!("[{category}]")
}

/// Colour token of the category tag.
///
/// Parity: the legacy UI viewer used the single `log_category` token for **every**
/// category (`src/app/ui/background_logs.rs:146-149`); there is no
/// per-severity colour in the shipping build and none is invented here. The
/// `match` is exhaustive over all seven [`LogCategory`] variants anyway, so
/// differentiating a category later is a deliberate, compiler-checked edit
/// rather than a fall-through ([GUD-003]).
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

/// Colour token of the timestamp column (`palette.log_timestamp`, the legacy UI
/// row's own token).
///
/// Purity: pure.
#[must_use]
pub fn timestamp_color() -> iced::Color {
    Palette::fastmd_dark().log_timestamp
}

/// Colour token of the message column (the default body `text` token — the legacy UI
/// applied no colour to the message label at all).
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

/// The category filter options, in the legacy UI build's order
/// (`src/app/ui/background_logs.rs:84-106`): "All" first, then the five
/// categories the shipping combo offered, each labelled from
/// [`crate::ui::strings`] ([GUD-002]).
///
/// **Parity note:** `LogCategory::Print` and `LogCategory::Batch` are *not*
/// offered here because the legacy UI combo never offered them, even though the
/// ring buffer can produce both (`logs.rs:51-54` routes `print`/`batch`
/// targets into them). Their rows still render and are reachable through the
/// "All" view and the text search. Widening the list is a product decision
/// (it needs two new `strings` constants), not a port decision — flagged in
/// the TASK-016 verification log.
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
    /// True when the content fits ([`Self::fits`]) — the legacy UI treated
    /// `available_offset < 0` as stuck (`scroll_area.rs:1543-1549`) — or
    /// when the offset is within [`BOTTOM_STICK_TOLERANCE`] of
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

/// The auto-scroll **intent**: should the reducer scroll the log list to the
/// end this frame?
///
/// `true` only when the preference is on **and** the viewport was already at
/// the end. This is the legacy UI's `stick_to_end` rule
/// (`if stick_to_end[d] && state.scroll_stuck_to_end[d] { offset = content - viewport }`,
/// `legacy-0.36.1/src/containers/scroll_area.rs:1282-1284`): a user who has
/// scrolled up to read history is never yanked back down, and turning the
/// preference off stops the follow immediately.
///
/// `None` (no viewport report yet) is deliberately **false** — with no
/// geometry there is nothing to follow, and scrolling blind would fight the
/// widget's own initial layout.
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
    snapshot.is_some_and(|viewport| auto_scroll && viewport.is_at_bottom())
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
    /// much the filter hid ([FMD-067]'s "report the loss, do not hide it").
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
    /// filter fields, exactly as the legacy UI viewer did
    /// (`src/app/ui/background_logs.rs:127`).
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

    /// Snapshots the buffer off `app`.
    ///
    /// Unlike the legacy UI build — which called `.lock().unwrap()` and would
    /// panic the whole UI on a poisoned lock
    /// (`src/app/ui/background_logs.rs:49-64`) — a poisoned buffer degrades
    /// to [`Self::unavailable`] so one wedged worker cannot take the UI down
    /// ([FMD-064]).
    #[must_use]
    pub fn from_app(app: &FastMdApp) -> Self {
        match app.orchestrator.background_manager.lock() {
            Ok(manager) => Self::from_logs(&manager),
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

/// Renders the background-process log viewer from a snapshot
/// ([TASK-016], UI-047).
///
/// Layout, mirroring the legacy UI window body
/// (`src/app/ui/background_logs.rs:76-154`):
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

    let search_field = text_input("", &view_model.search_text)
        .on_input(|text| Message::Logs(LogsMessage::SearchChanged { text }));

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
        search_field.width(Length::Fill).into(),
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
/// Three columns matching the legacy UI row layout — timestamp, `[Category]`,
/// message — built with `iced::widget::table` as the migration mapping
/// requires (`doc/planning/iced.md` A5: `Table`/`TableBuilder` →
/// `iced::widget::table`).
///
/// The message column is `Length::Fill` explicitly: iced forces the *first*
/// all-`Shrink` column to `Fill` instead
/// (`iced_widget-0.14.2/src/table.rs:129-133`), which would pin the
/// timestamp wide and starve the message.
///
/// iced's `table` requires a header element per column; the legacy UI build had
/// none, so the headers are empty text — allocated, not removed (AGENTS §6).
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
