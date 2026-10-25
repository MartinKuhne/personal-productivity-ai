//! Sidecar tests for [`crate::ui_iced::logs`] (UI-047).
//!
//! Covers the filter predicate, the auto-scroll intent rule and its
//! boundaries, the ring-buffer snapshot, the category→colour-token map over
//! every category the buffer can produce, and the view builders.
//!
//! Fixtures build [`BackgroundLogs`] and [`BackgroundLogEntry`] values
//! directly — no files, no bus, no worker threads and no user paths
//! ([RUST-006]).

use chrono::{DateTime, Local, TimeZone, Timelike};

use crate::background::logs::BackgroundLogs;
use crate::bus::events::messages::{BackgroundLogEntry, LogCategory};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{LogsMessage, Message};

use super::{
    BOTTOM_STICK_TOLERANCE, BackgroundLogsViewModel, CategoryFilterOption, ScrollSnapshot,
    auto_scroll_setting, category_color, category_filter_options, category_tag, filter_logs,
    follow_state, is_log_visible, log_table, message_color, scroll_snapshot, should_follow,
    timestamp_color, timestamp_text,
};

/// Every category the ring buffer can produce, in declaration order.
const ALL_CATEGORIES: [LogCategory; 7] = [
    LogCategory::Indexer,
    LogCategory::Watcher,
    LogCategory::PdfConverter,
    LogCategory::ImageVision,
    LogCategory::LlmTools,
    LogCategory::Print,
    LogCategory::Batch,
];

/// A log entry stamped with a fixed local time so formatting is exact.
fn stamped(category: LogCategory, message: &str, millis: u32) -> BackgroundLogEntry {
    let base = Local.with_ymd_and_hms(2026, 10, 18, 12, 4, 31).unwrap();
    BackgroundLogEntry {
        timestamp: base.with_nanosecond(millis * 1_000_000).unwrap(),
        category,
        message: message.to_string(),
    }
}

/// A ring buffer seeded with `entries`, default filters.
fn buffer_with(entries: &[BackgroundLogEntry]) -> BackgroundLogs {
    let mut logs = BackgroundLogs::new();
    for entry in entries {
        logs.push_log(entry.clone());
    }
    logs
}

/// A viewport sitting exactly at the end of a 1000 pt list in a 200 pt pane.
fn at_bottom() -> ScrollSnapshot {
    ScrollSnapshot {
        offset_y: 800.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    }
}

// ---------------------------------------------------------------------------
// Timestamp / category formatting
// ---------------------------------------------------------------------------

#[test]
fn timestamp_text_renders_hhmmss_with_milliseconds() {
    let entry = stamped(LogCategory::Indexer, "Indexing workspace...", 220);
    assert_eq!(timestamp_text(&entry.timestamp), "12:04:31.220");
}

#[test]
fn timestamp_text_pads_sub_millisecond_values() {
    let entry = stamped(LogCategory::Watcher, "File changed", 5);
    assert_eq!(timestamp_text(&entry.timestamp), "12:04:31.005");
}

#[test]
fn timestamp_format_constant_renders_hhmmssfff() {
    // `TIMESTAMP_FORMAT` is what the timestamp column renders through;
    // pinning the constant pins the column.
    let stamp: DateTime<Local> = Local
        .with_ymd_and_hms(2026, 10, 18, 9, 8, 7)
        .unwrap()
        .with_nanosecond(123_000_000)
        .unwrap();
    assert_eq!(
        stamp.format(super::TIMESTAMP_FORMAT).to_string(),
        "09:08:07.123"
    );
}

#[test]
fn category_tag_wraps_the_bus_display_label_in_brackets() {
    for category in ALL_CATEGORIES {
        assert_eq!(category_tag(category), format!("[{category}]"));
    }
    assert_eq!(category_tag(LogCategory::PdfConverter), "[PDF Converter]");
}

// ---------------------------------------------------------------------------
// Category → colour token mapping (every category the buffer can produce)
// ---------------------------------------------------------------------------

#[test]
fn category_color_covers_every_category_with_the_log_category_token() {
    let expected = super::Palette::fastmd_dark().log_category;
    for category in ALL_CATEGORIES {
        assert_eq!(
            category_color(category),
            expected,
            "{category} must use the parity `log_category` token"
        );
    }
}

#[test]
fn timestamp_and_message_columns_use_their_own_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(timestamp_color(), palette.log_timestamp);
    assert_eq!(message_color(), palette.text);
    assert_ne!(
        timestamp_color(),
        message_color(),
        "the time column must stay visually distinct from the message"
    );
}

// ---------------------------------------------------------------------------
// Category filter options
// ---------------------------------------------------------------------------

#[test]
fn category_filter_options_lists_all_plus_the_five_categories() {
    let options = category_filter_options();
    let categories: Vec<Option<LogCategory>> =
        options.iter().map(|option| option.category).collect();

    assert_eq!(
        categories,
        vec![
            None,
            Some(LogCategory::Indexer),
            Some(LogCategory::Watcher),
            Some(LogCategory::PdfConverter),
            Some(LogCategory::ImageVision),
            Some(LogCategory::LlmTools),
        ],
        "the pick list must keep the fixed order and membership"
    );
    assert_eq!(options[0].label, strings::LOG_CATEGORY_ALL);
}

#[test]
fn category_filter_labels_agree_with_the_bus_display_impl() {
    // Drift guard: the `strings` constants and `LogCategory::Display` are two
    // sources for the same label; if one changes the row tag and the combo
    // would disagree.
    let pairs = [
        (LogCategory::Indexer, strings::LOG_CATEGORY_INDEXER),
        (LogCategory::Watcher, strings::LOG_CATEGORY_WATCHER),
        (
            LogCategory::PdfConverter,
            strings::LOG_CATEGORY_PDF_CONVERTER,
        ),
        (LogCategory::ImageVision, strings::LOG_CATEGORY_IMAGE_VISION),
        (LogCategory::LlmTools, strings::LOG_CATEGORY_LLM_TOOLS),
    ];
    for (category, label) in pairs {
        assert_eq!(category.to_string(), label);
    }
}

#[test]
fn category_filter_option_command_carries_its_category() {
    assert_eq!(
        CategoryFilterOption::all().command(),
        Message::Logs(LogsMessage::CategoryFilterChanged(None))
    );
    assert_eq!(
        CategoryFilterOption::for_category(LogCategory::Batch, "Batch").command(),
        Message::Logs(LogsMessage::CategoryFilterChanged(Some(LogCategory::Batch)))
    );
}

#[test]
fn category_filter_option_displays_its_label() {
    assert_eq!(CategoryFilterOption::all().to_string(), "All");
}

// ---------------------------------------------------------------------------
// Filter predicate (exercised through this layer)
// ---------------------------------------------------------------------------

#[test]
fn no_filters_shows_every_entry() {
    for category in ALL_CATEGORIES {
        let entry = stamped(category, "anything", 1);
        assert!(is_log_visible(&entry, None, ""));
    }
}

#[test]
fn category_filter_matches_only_its_own_category() {
    let watcher = stamped(LogCategory::Watcher, "File changed", 1);
    let indexer = stamped(LogCategory::Indexer, "File changed", 1);
    assert!(is_log_visible(&watcher, Some(LogCategory::Watcher), ""));
    assert!(!is_log_visible(&indexer, Some(LogCategory::Watcher), ""));
}

#[test]
fn search_is_case_insensitive_substring_of_the_message() {
    let entry = stamped(LogCategory::Indexer, "Found 42 files", 1);
    assert!(is_log_visible(&entry, None, "found"));
    assert!(is_log_visible(&entry, None, "42"));
    assert!(!is_log_visible(&entry, None, "missing"));
}

#[test]
fn empty_search_string_matches_every_message() {
    let entry = stamped(LogCategory::LlmTools, "anything at all", 1);
    assert!(is_log_visible(&entry, None, ""));
}

#[test]
fn category_and_search_must_both_pass() {
    let entry = stamped(LogCategory::ImageVision, "Processing image.jpg", 1);
    assert!(is_log_visible(
        &entry,
        Some(LogCategory::ImageVision),
        "image"
    ));
    assert!(!is_log_visible(&entry, Some(LogCategory::Watcher), "image"));
    assert!(!is_log_visible(
        &entry,
        Some(LogCategory::ImageVision),
        "pdf"
    ));
}

#[test]
fn filter_logs_keeps_matching_entries_in_order() {
    let logs = [
        stamped(LogCategory::Indexer, "Index 1", 1),
        stamped(LogCategory::Indexer, "Index 2", 2),
        stamped(LogCategory::Watcher, "Watch 1", 3),
    ];

    let by_category = filter_logs(logs.iter(), Some(LogCategory::Indexer), "");
    assert_eq!(by_category.len(), 2);
    assert_eq!(by_category[0].message, "Index 1");
    assert_eq!(by_category[1].message, "Index 2");

    let by_search = filter_logs(logs.iter(), None, "watch");
    assert_eq!(by_search.len(), 1);
    assert_eq!(by_search[0].message, "Watch 1");
}

#[test]
fn filter_logs_returns_nothing_when_no_entry_matches() {
    let logs = [
        stamped(LogCategory::Indexer, "Index 1", 1),
        stamped(LogCategory::Watcher, "Watch 1", 2),
    ];
    assert!(filter_logs(logs.iter(), Some(LogCategory::Batch), "").is_empty());
    assert!(filter_logs(logs.iter(), None, "zzz").is_empty());
}

#[test]
fn filter_logs_returns_everything_when_nothing_is_filtered() {
    let logs = [
        stamped(LogCategory::Print, "p1", 1),
        stamped(LogCategory::Batch, "b1", 2),
        stamped(LogCategory::LlmTools, "t1", 3),
    ];
    assert_eq!(filter_logs(logs.iter(), None, "").len(), 3);
}

#[test]
fn filter_logs_over_an_empty_iterator_is_empty() {
    let empty: [BackgroundLogEntry; 0] = [];
    assert!(filter_logs(empty.iter(), None, "").is_empty());
}

// ---------------------------------------------------------------------------
// Auto-scroll intent
// ---------------------------------------------------------------------------

#[test]
fn at_bottom_with_auto_scroll_follows() {
    assert!(should_follow(true, Some(at_bottom())));
}

#[test]
fn scrolled_up_does_not_jump_back_down() {
    let scrolled_up = ScrollSnapshot {
        offset_y: 100.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(!should_follow(true, Some(scrolled_up)));
}

#[test]
fn auto_scroll_off_never_follows_even_at_the_bottom() {
    assert!(!should_follow(false, Some(at_bottom())));
}

#[test]
fn no_viewport_report_never_follows() {
    assert!(!should_follow(true, None));
    assert!(!should_follow(false, None));
}

#[test]
fn content_shorter_than_the_viewport_counts_as_at_bottom() {
    let short = ScrollSnapshot {
        offset_y: 0.0,
        viewport_height: 200.0,
        content_height: 40.0,
    };
    assert!(short.fits());
    assert!(short.is_at_bottom());
    assert!(should_follow(true, Some(short)));
}

#[test]
fn exactly_zero_content_is_at_bottom() {
    let empty = ScrollSnapshot {
        offset_y: 0.0,
        viewport_height: 200.0,
        content_height: 0.0,
    };
    assert_eq!(empty.max_offset(), 0.0);
    assert!(empty.is_at_bottom());
    assert!(should_follow(true, Some(empty)));
}

#[test]
fn max_offset_never_goes_negative() {
    let short = ScrollSnapshot {
        offset_y: 0.0,
        viewport_height: 500.0,
        content_height: 100.0,
    };
    assert_eq!(short.max_offset(), 0.0);
}

#[test]
fn tolerance_absorbs_a_sub_point_shortfall() {
    let just_short = ScrollSnapshot {
        offset_y: 800.0 - (BOTTOM_STICK_TOLERANCE / 2.0),
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(just_short.is_at_bottom());
    assert!(should_follow(true, Some(just_short)));
}

#[test]
fn beyond_tolerance_is_not_at_bottom() {
    let clearly_up = ScrollSnapshot {
        offset_y: 800.0 - (BOTTOM_STICK_TOLERANCE * 5.0),
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(!clearly_up.is_at_bottom());
    assert!(!should_follow(true, Some(clearly_up)));
}

#[test]
fn exactly_at_max_offset_is_at_bottom() {
    let snapshot = at_bottom();
    assert_eq!(snapshot.offset_y, snapshot.max_offset());
    assert!(snapshot.is_at_bottom());
}

#[test]
fn follows_with_method_agrees_with_the_free_function() {
    let snapshot = at_bottom();
    assert_eq!(
        snapshot.follows_with(true),
        should_follow(true, Some(snapshot))
    );
    assert_eq!(
        snapshot.follows_with(false),
        should_follow(false, Some(snapshot))
    );
}

#[test]
fn tolerance_absorbs_exactly_its_own_margin() {
    // The boundary itself: sitting `BOTTOM_STICK_TOLERANCE` short of the end
    // still counts as at the bottom, one point further does not.
    let on_the_margin = ScrollSnapshot {
        offset_y: 800.0 - BOTTOM_STICK_TOLERANCE,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(on_the_margin.is_at_bottom());

    let past_the_margin = ScrollSnapshot {
        offset_y: 800.0 - BOTTOM_STICK_TOLERANCE - 1.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(!past_the_margin.is_at_bottom());
}

// ---------------------------------------------------------------------------
// View-model snapshot
// ---------------------------------------------------------------------------

#[test]
fn from_logs_carries_entries_and_flags() {
    let mut logs = buffer_with(&[
        stamped(LogCategory::Indexer, "Indexing workspace...", 1),
        stamped(LogCategory::Watcher, "File modified", 2),
    ]);
    logs.show_background_logs = true;
    logs.auto_scroll = true;

    let view_model = BackgroundLogsViewModel::from_logs(&logs);

    assert!(view_model.visible);
    assert!(view_model.auto_scroll);
    assert!(!view_model.access_error);
    assert_eq!(view_model.entries.len(), 2);
    assert_eq!(view_model.total_entries, 2);
    assert_eq!(view_model.entries[0].category, LogCategory::Indexer);
    assert_eq!(view_model.entries[1].message, "File modified");
    assert_eq!(view_model.category_filter, None);
    assert_eq!(view_model.search_text, "");
    assert!(!view_model.is_empty());
    assert_eq!(view_model.filtered_out(), 0);
}

/// Regression guard: the viewer must not be cloned whole on every message
/// while it is closed. The ring buffer holds up 10 000 entries and a
/// workspace scan fills it, so that clone is both wasted work and
/// contention with the worker thread pushing into the same mutex.
#[test]
fn from_app_skips_the_entry_clone_while_the_viewer_is_closed() {
    let app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    {
        let mut manager = app
            .orchestrator
            .background_manager
            .lock()
            .expect("log buffer lock");
        manager.show_background_logs = false;
        for index in 0..50 {
            manager.push_log(stamped(LogCategory::Indexer, "Indexing file", index));
        }
    }

    let view_model = BackgroundLogsViewModel::from_app(&app);

    assert!(!view_model.visible);
    assert!(
        view_model.entries.is_empty(),
        "a closed viewer must not clone the ring buffer"
    );
    assert_eq!(
        view_model.total_entries, 50,
        "the counter still rides along so the header stays honest"
    );
    assert_eq!(
        view_model.category_options.len(),
        category_filter_options().len(),
        "the filter pick-list stays populated so the control does not render empty"
    );
    assert!(!view_model.access_error);
}

/// Opening the viewer must produce the entries again — the gate may not strand
/// the window on an empty snapshot.
#[test]
fn from_app_clones_entries_once_the_viewer_opens() {
    let app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    {
        let mut manager = app
            .orchestrator
            .background_manager
            .lock()
            .expect("log buffer lock");
        manager.show_background_logs = false;
        manager.push_log(stamped(LogCategory::Watcher, "File modified", 1));
    }

    assert!(BackgroundLogsViewModel::from_app(&app).entries.is_empty());

    {
        let mut manager = app
            .orchestrator
            .background_manager
            .lock()
            .expect("log buffer lock");
        manager.show_background_logs = true;
    }

    let view_model = BackgroundLogsViewModel::from_app(&app);
    assert!(view_model.visible);
    assert_eq!(
        view_model.entries.len(),
        1,
        "opening must populate the entries"
    );
    assert_eq!(view_model.entries[0].message, "File modified");
}

#[test]
fn closed_keeps_settings_and_reports_no_access_error() {
    let mut logs = buffer_with(&[stamped(LogCategory::Indexer, "Index 1", 1)]);
    logs.show_background_logs = true;
    logs.search_text = "index".to_string();
    logs.filter_category = Some(LogCategory::Indexer);
    logs.auto_scroll = true;

    let view_model = BackgroundLogsViewModel::closed(&logs);

    assert!(
        !view_model.visible,
        "`closed` always reports a closed window"
    );
    assert!(view_model.entries.is_empty());
    assert_eq!(view_model.total_entries, 1);
    assert_eq!(view_model.search_text, "index");
    assert_eq!(view_model.category_filter, Some(LogCategory::Indexer));
    assert!(view_model.auto_scroll);
    assert!(!view_model.access_error);
}

#[test]
fn from_logs_applies_the_buffers_own_filters() {
    let mut logs = buffer_with(&[
        stamped(LogCategory::Indexer, "Index 1", 1),
        stamped(LogCategory::Watcher, "Watch 1", 2),
        stamped(LogCategory::Indexer, "Index 2", 3),
    ]);
    logs.filter_category = Some(LogCategory::Indexer);
    logs.search_text = "index 2".to_string();

    let view_model = BackgroundLogsViewModel::from_logs(&logs);

    assert_eq!(view_model.entries.len(), 1);
    assert_eq!(view_model.entries[0].message, "Index 2");
    assert_eq!(view_model.total_entries, 3);
    assert_eq!(view_model.filtered_out(), 2);
}

#[test]
fn from_logs_never_mutates_the_buffer() {
    let entries = [
        stamped(LogCategory::LlmTools, "tool call", 1),
        stamped(LogCategory::Print, "print job", 2),
    ];
    let mut logs = buffer_with(&entries);

    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    assert_eq!(view_model.total_entries, 2);

    // The snapshot is a copy: draining the buffer afterwards leaves the
    // snapshot's own entries untouched, which is what lets the view hold it
    // across a frame boundary.
    logs.clear_logs();
    assert!(logs.get_logs().is_empty());
    assert_eq!(view_model.entries.len(), 2);
}

#[test]
fn from_logs_on_an_empty_buffer_is_empty_but_not_an_error() {
    let logs = buffer_with(&[]);
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    assert!(view_model.is_empty());
    assert!(!view_model.access_error);
    assert_eq!(view_model.total_entries, 0);
}

#[test]
fn unavailable_is_fail_closed() {
    let view_model = BackgroundLogsViewModel::unavailable();
    assert!(view_model.access_error);
    assert!(
        !view_model.visible,
        "an unreadable buffer must not force the window open"
    );
    assert!(view_model.is_empty());
    assert!(!view_model.auto_scroll);
    assert_eq!(view_model.filtered_out(), 0);
}

#[test]
fn category_filter_option_resolves_from_the_owned_options() {
    let mut logs = buffer_with(&[stamped(LogCategory::Watcher, "w", 1)]);
    logs.filter_category = Some(LogCategory::Watcher);
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    let option = view_model
        .category_filter_option()
        .expect("Watcher is offered");
    assert_eq!(option.category, Some(LogCategory::Watcher));
    assert_eq!(option.label, strings::LOG_CATEGORY_WATCHER);

    // "All" is the `None` filter.
    logs.filter_category = None;
    let all_model = BackgroundLogsViewModel::from_logs(&logs);
    let all = all_model.category_filter_option();
    assert_eq!(all.map(|option| option.category), Some(None));
}

#[test]
fn category_filter_option_is_none_for_an_unoffered_category() {
    let mut logs = buffer_with(&[stamped(LogCategory::Batch, "b", 1)]);
    logs.filter_category = Some(LogCategory::Batch);
    assert!(
        BackgroundLogsViewModel::from_logs(&logs)
            .category_filter_option()
            .is_none(),
        "Batch is not in the legacy UI build's pick list (parity gap, see the \
         TASK-016 verification log)"
    );
}

#[test]
fn follow_intent_uses_the_snapshot_preference_and_geometry() {
    let mut logs = buffer_with(&[stamped(LogCategory::Indexer, "i", 1)]);
    logs.auto_scroll = true;
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    assert!(view_model.follow_intent(Some(at_bottom())));
    assert!(!view_model.follow_intent(None));

    logs.auto_scroll = false;
    let off = BackgroundLogsViewModel::from_logs(&logs);
    assert!(!off.follow_intent(Some(at_bottom())));
}

#[test]
fn view_model_view_matches_the_free_view_builder() {
    let logs = buffer_with(&[stamped(LogCategory::Indexer, "i", 1)]);
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    // Both build without panicking; the element itself is opaque to a unit
    // test (iced 0.14 dropped `Widget::type_name`), so the assertion here is
    // "the snapshot path and the free function path both work".
    let _from_method = view_model.view();
    let _from_function = super::background_logs_view(&view_model);
}

// ---------------------------------------------------------------------------
// View builders
// ---------------------------------------------------------------------------

#[test]
fn background_logs_view_builds_for_a_populated_snapshot() {
    let mut logs = buffer_with(&[
        stamped(LogCategory::Indexer, "Indexing workspace...", 1),
        stamped(LogCategory::PdfConverter, "Converting doc.md", 2),
    ]);
    logs.show_background_logs = true;
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    let _element = super::background_logs_view(&view_model);
}

#[test]
fn background_logs_view_builds_for_an_empty_snapshot_with_the_same_shape() {
    let logs = buffer_with(&[]);
    let view_model = BackgroundLogsViewModel::from_logs(&logs);
    let _element = super::background_logs_view(&view_model);
}

#[test]
fn background_logs_view_builds_the_access_error_state() {
    let view_model = BackgroundLogsViewModel::unavailable();
    let _element = super::background_logs_view(&view_model);
}

#[test]
fn log_table_builds_over_empty_and_populated_entries() {
    let empty: [BackgroundLogEntry; 0] = [];
    let _empty_table = log_table(&empty);
    let populated = [
        stamped(LogCategory::Batch, "batch started", 1),
        stamped(LogCategory::Print, "print queued", 2),
    ];
    let _populated_table = log_table(&populated);
}

#[test]
fn scroll_snapshot_reads_the_three_axes_it_needs() {
    // `scrollable::Viewport` has private fields and no public constructor,
    // which is exactly why `ScrollSnapshot` exists: the rule under test is
    // exercised on the snapshot, and `scroll_snapshot` is the single place
    // that touches the iced type.
    let snapshot = ScrollSnapshot {
        offset_y: 12.5,
        viewport_height: 300.0,
        content_height: 1200.0,
    };
    assert_eq!(snapshot.max_offset(), 900.0);
    assert!(!snapshot.fits());
    assert!(!snapshot.is_at_bottom());
    let _ = scroll_snapshot; // the iced-facing reader; driven by the widget at runtime
}

// ─────────── INT-6: `should_follow` unified with `FollowState::resolve` ───────────

use iced_pro::surface::movable::{AT_BOTTOM_TOLERANCE, AutoScrollSetting, FollowState};

/// All four cells of the framework's pause/resume matrix (`UIF-MOV-004`),
/// reached through the app's own entry point. `should_follow` is now this
/// matrix's projection to a bool, so the two must agree cell for cell.
#[test]
fn follow_state_agrees_with_should_follow_across_all_four_cells() {
    let at_bottom = ScrollSnapshot {
        offset_y: 800.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    let scrolled_up = ScrollSnapshot {
        offset_y: 100.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    };

    for (auto_scroll, snapshot, expected) in [
        (true, Some(at_bottom), true),
        (true, Some(scrolled_up), false),
        (false, Some(at_bottom), false),
        (false, Some(scrolled_up), false),
    ] {
        let state = follow_state(auto_scroll, snapshot);
        assert_eq!(
            state.is_following(),
            should_follow(auto_scroll, snapshot),
            "follow_state and should_follow must agree for auto_scroll={auto_scroll}"
        );
        assert_eq!(
            state.is_following(),
            expected,
            "unexpected cell for auto_scroll={auto_scroll}, at_bottom={:?}",
            snapshot.map(|v| v.is_at_bottom())
        );
    }
}

/// No viewport report means there is nothing to follow — the framework's
/// `at_bottom = false` cell, not a special case.
#[test]
fn no_report_is_paused_not_following() {
    assert_eq!(follow_state(true, None), FollowState::Paused);
    assert_eq!(follow_state(false, None), FollowState::Paused);
    assert!(!should_follow(true, None));
}

/// The setting translation is the single place `bool` becomes the
/// framework's enum ([RUST-021]).
#[test]
fn the_bool_translates_to_the_framework_setting() {
    assert_eq!(auto_scroll_setting(true), AutoScrollSetting::Enabled);
    assert_eq!(auto_scroll_setting(false), AutoScrollSetting::Disabled);
    assert!(auto_scroll_setting(true).permits_following());
    assert!(!auto_scroll_setting(false).permits_following());
}

/// **Tolerance delta, pinned deliberately.** The app's bottom margin stays
/// `1.0` pt, narrower than the framework's `AT_BOTTOM_TOLERANCE` of
/// `space::XXS` = `2.0` pt. A viewport 1.5 pt short of the tail is
/// therefore NOT at the bottom here, where the framework's own
/// `ScrollGeometry::at_bottom` would call it bottom.
///
/// Adopting the framework's wider margin would keep yanking the view down
/// while the user is 1.5 pt into reading history. Widening it is a
/// behaviour change that needs its own decision, so INT-6 unified the
/// decision matrix without moving this threshold.
#[test]
#[allow(clippy::assertions_on_constants)]
fn the_app_keeps_a_tighter_bottom_margin_than_the_framework() {
    assert_eq!(BOTTOM_STICK_TOLERANCE, 1.0);
    assert_eq!(AT_BOTTOM_TOLERANCE, 2.0);
    assert!(
        BOTTOM_STICK_TOLERANCE < AT_BOTTOM_TOLERANCE,
        "the app margin must stay the narrower of the two"
    );

    let just_short = ScrollSnapshot {
        offset_y: 800.0 - 1.5,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert!(
        !just_short.is_at_bottom(),
        "1.5 pt short is outside the app's 1.0 pt margin"
    );
    assert!(
        just_short.to_geometry().at_bottom(),
        "the same frame reads as at-bottom under the framework's 2.0 pt margin"
    );
    assert!(!should_follow(true, Some(just_short)));
}

/// The framework-geometry mapping keeps the field transposition correct:
/// `ScrollSnapshot` is `(offset, viewport, content)`, the framework's
/// constructor is `(offset, content, viewport)`.
#[test]
fn to_geometry_maps_the_fields_in_the_frameworks_order() {
    let snapshot = ScrollSnapshot {
        offset_y: 120.0,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    let geometry = snapshot.to_geometry();

    assert_eq!(geometry.offset, 120.0);
    assert_eq!(geometry.content, 1000.0);
    assert_eq!(geometry.viewport, 200.0);
    assert_eq!(geometry.max_offset(), snapshot.max_offset());
}
