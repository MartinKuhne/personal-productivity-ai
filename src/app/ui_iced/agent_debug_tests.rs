//! Sidecar tests for [`crate::ui_iced::agent_debug`] ([TASK-017b4],
//! UI-048 / REQ-008).
//!
//! Covers the pieces the legacy UI window kept inside its widget tree and its
//! render closure — the search predicate, the session-scoped tools
//! de-duplication, the JSON un-escape pass, the `json_rows` viewport
//! decision, the explicit expanded-id set that replaces
//! `CollapsingHeader`, the reused auto-scroll intent and the
//! message-channel split — as pure data tests.
//!
//! The legacy UI originals (`src/app/ui/agent_debug_window.rs:275-411`) were
//! `run_ui_test` smoke tests over a live `Context`; every rule they
//! pinned is asserted here directly, without a context, a bus or a running
//! agent. Fixtures build [`AgentDebugEntry`] values in memory and the one
//! `FastMdApp` used is the in-memory `empty_state` fixture — no files, no
//! user paths ([RUST-006]).

use std::collections::HashSet;

use chrono::{DateTime, Local, TimeZone, Timelike};
use serde_json::{Value, json};

use crate::agent::datamark::{EXTERNAL_DATA_END, EXTERNAL_DATA_START};
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui_iced::logs::ScrollSnapshot;
use crate::ui_iced::message::{AgentDebugMessage, Message};

use super::{
    AgentDebugViewModel, AgentDebugWindowState, DebugEntryView, JSON_LINE_HEIGHT, JSON_ROW_CATALOG,
    JsonRowsOption, OMITTED_TOOLS_PLACEHOLDER, TOOLS_KEY, agent_debug_card, auto_scroll_command,
    chevron_glyph, clear_command, copy_json_command, dismiss_command, display_json,
    duplicate_tool_indices, entry_list, entry_matches, entry_row, filter_debug_entries,
    header_text, json_body_height, json_line_count, json_row_options, kind_label,
    normalized_search, scroll_reported_message, search_command, toggle_entry_command,
    unescape_json_strings, visible_json_lines,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A fixed local timestamp so header formatting is exact.
fn stamp(millis: u32) -> DateTime<Local> {
    let base = Local.with_ymd_and_hms(2026, 10, 18, 12, 4, 31).unwrap();
    base.with_nanosecond(millis * 1_000_000).unwrap()
}

/// A normal collapsible entry with an explicit payload.
fn entry_with(
    turn: usize,
    kind: DebugEntryKind,
    summary: &str,
    content: Option<Value>,
) -> AgentDebugEntry {
    AgentDebugEntry {
        turn,
        timestamp: stamp(220),
        kind,
        summary: summary.to_string(),
        content,
        row_type: DebugEntryRow::Entry,
    }
}

/// A normal entry carrying a small JSON object.
fn entry(turn: usize, kind: DebugEntryKind, summary: &str) -> AgentDebugEntry {
    entry_with(turn, kind, summary, Some(json!({"key": "value"})))
}

/// A non-interactive session divider.
fn boundary(session: usize) -> AgentDebugEntry {
    AgentDebugEntry {
        turn: 0,
        timestamp: stamp(0),
        kind: DebugEntryKind::Outgoing,
        summary: format!("Session {session}"),
        content: None,
        row_type: DebugEntryRow::SessionBoundary,
    }
}

/// An outgoing entry carrying a `tools` array — the payload the
/// de-duplication pass keys on. The tool name is repeated in the summary
/// so a search can select the row (only the summary is searchable).
fn tools_entry(turn: usize, tool_name: &str) -> AgentDebugEntry {
    entry_with(
        turn,
        DebugEntryKind::Outgoing,
        &format!("Turn {turn} — Outgoing ({tool_name})"),
        Some(json!({
            "tools": [{ "name": tool_name }],
            "messages": []
        })),
    )
}

/// A viewport inside a 200 pt window over a 1000 pt list.
fn viewport(offset_y: f32) -> ScrollSnapshot {
    ScrollSnapshot {
        offset_y,
        viewport_height: 200.0,
        content_height: 1000.0,
    }
}

/// The row ids a filtered list carries, in order.
fn ids(filtered: &[(usize, &AgentDebugEntry)]) -> Vec<usize> {
    filtered.iter().map(|(id, _)| *id).collect()
}

// ---------------------------------------------------------------------------
// Search filter
// ---------------------------------------------------------------------------

#[test]
fn empty_search_shows_every_entry_in_order() {
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "alpha"),
        entry(2, DebugEntryKind::Incoming, "beta"),
    ];
    let filtered = filter_debug_entries(&entries, "");
    assert_eq!(ids(&filtered), vec![0, 1, 2]);
}

#[test]
fn search_is_case_insensitive_on_the_summary() {
    let entries = vec![entry(
        1,
        DebugEntryKind::Outgoing,
        "Turn 1 — Outgoing (+2 messages)",
    )];

    assert_eq!(filter_debug_entries(&entries, "turn 1").len(), 1);
    assert_eq!(filter_debug_entries(&entries, "TURN 1").len(), 1);
    assert_eq!(filter_debug_entries(&entries, "OuTgOiNg").len(), 1);
    assert_eq!(filter_debug_entries(&entries, "2 messages").len(), 1);
}

#[test]
fn a_non_matching_search_hides_the_entry() {
    let entries = vec![entry(1, DebugEntryKind::Outgoing, "alpha")];
    assert_eq!(filter_debug_entries(&entries, "zulu").len(), 0);
    assert!(!entry_matches(&entries[0], "zulu"));
}

#[test]
fn session_boundaries_survive_any_search() {
    // The dividers stay put so a filtered list keeps its session
    // structure instead of collapsing into one undifferentiated run.
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "alpha"),
        boundary(2),
        entry(2, DebugEntryKind::Incoming, "beta"),
    ];
    let filtered = filter_debug_entries(&entries, "beta");

    assert_eq!(ids(&filtered), vec![0, 2, 3]);
    for (_, entry) in &filtered {
        if matches!(entry.row_type, DebugEntryRow::SessionBoundary) {
            continue;
        }
        assert!(entry.summary.contains("beta"));
    }
}

#[test]
fn search_never_matches_the_json_body() {
    // the legacy UI searched `summary` only (ui/agent_debug_window.rs:73); the
    // payload is display-only.
    let entry = entry_with(
        1,
        DebugEntryKind::Outgoing,
        "nothing to see here",
        Some(json!({ "needle": "found" })),
    );
    assert!(!entry_matches(&entry, "found"));
    assert!(!entry_matches(&entry, "needle"));
    assert!(entry_matches(&entry, "nothing"));
}

#[test]
fn a_whitespace_only_search_is_not_treated_as_empty() {
    // Parity: the legacy UI tested `search_lower.is_empty()`, so " " filters.
    let entries = vec![entry(1, DebugEntryKind::Outgoing, "alpha")];
    assert_eq!(filter_debug_entries(&entries, " ").len(), 0);
}

#[test]
fn normalized_search_lowercases_the_needle_once() {
    assert_eq!(normalized_search("Turn 1 — OUTGOING"), "turn 1 — outgoing");
    assert_eq!(normalized_search(""), "");
    assert_eq!(normalized_search("Already lower"), "already lower");
}

// ---------------------------------------------------------------------------
// Session-scoped `tools` de-duplication
// ---------------------------------------------------------------------------

#[test]
fn the_first_tools_entry_of_a_session_is_kept() {
    let entries = vec![boundary(1), tools_entry(1, "read")];
    let filtered = filter_debug_entries(&entries, "");
    assert!(
        duplicate_tool_indices(&filtered).is_empty(),
        "the first tools catalog a session shows is never elided"
    );
}

#[test]
fn later_tools_entries_in_the_same_session_are_omitted() {
    let entries = vec![
        boundary(1),
        tools_entry(1, "read"),
        tools_entry(2, "read"),
        tools_entry(3, "read"),
    ];
    let filtered = filter_debug_entries(&entries, "");
    assert_eq!(duplicate_tool_indices(&filtered), HashSet::from([2, 3]));
}

#[test]
fn a_session_boundary_resets_the_tools_flag() {
    let entries = vec![
        boundary(1),
        tools_entry(1, "read"),
        tools_entry(2, "read"),
        boundary(2),
        tools_entry(3, "read"),
    ];
    let filtered = filter_debug_entries(&entries, "");
    assert_eq!(
        duplicate_tool_indices(&filtered),
        HashSet::from([2]),
        "a new session shows its own tools again"
    );
}

#[test]
fn entries_without_a_tools_key_never_trigger_the_dedup() {
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "a"),
        entry(2, DebugEntryKind::Incoming, "b"),
    ];
    let filtered = filter_debug_entries(&entries, "");
    assert!(duplicate_tool_indices(&filtered).is_empty());
}

#[test]
fn the_omitted_tools_placeholder_reaches_the_display_json() {
    let entries = vec![boundary(1), tools_entry(1, "read"), tools_entry(2, "read")];
    let view_model = AgentDebugViewModel::from_entries(&entries, true, "", true, 8);

    assert_eq!(view_model.rows.len(), 3);
    assert!(
        view_model.rows[1].json.contains("read"),
        "the first entry keeps its tools: {}",
        view_model.rows[1].json
    );
    assert!(
        !view_model.rows[2].json.contains("read"),
        "the duplicate's tools must be gone: {}",
        view_model.rows[2].json
    );
    assert!(view_model.rows[2].json.contains(OMITTED_TOOLS_PLACEHOLDER));
}

#[test]
fn the_dedup_runs_on_the_filtered_list_not_the_raw_buffer() {
    // Searching the first tools entry away promotes the second to
    // "first visible", so it keeps its tools — legacy parity: the pass
    // walks `filtered` (ui/agent_debug_window.rs:79).
    let entries = vec![
        boundary(1),
        tools_entry(1, "alpha-tool"),
        tools_entry(2, "beta-tool"),
    ];
    let view_model = AgentDebugViewModel::from_entries(&entries, true, "beta", true, 8);

    assert_eq!(view_model.rows.len(), 2, "boundary + the beta turn");
    assert!(view_model.rows[1].json.contains("beta-tool"));
    assert!(!view_model.rows[1].json.contains(OMITTED_TOOLS_PLACEHOLDER));
}

// ---------------------------------------------------------------------------
// JSON un-escaping
// ---------------------------------------------------------------------------

#[test]
fn a_double_encoded_json_string_becomes_real_structure() {
    let mut value = json!({ "body": "{\"a\": 1}" });
    unescape_json_strings(&mut value);
    assert_eq!(value["body"], json!({ "a": 1 }));
}

#[test]
fn objects_and_arrays_are_walked_recursively() {
    let mut value = json!({
        "outer": { "inner": "[{\"deep\": true}]" },
        "list": ["{\"n\": 1}", "plain", 42]
    });
    unescape_json_strings(&mut value);

    assert_eq!(value["outer"]["inner"], json!([{ "deep": true }]));
    assert_eq!(value["list"][0], json!({ "n": 1 }));
    assert_eq!(value["list"][1], json!("plain"));
    assert_eq!(value["list"][2], json!(42));
}

#[test]
fn an_unparseable_bracketed_string_is_left_alone() {
    let mut value = json!("{ not valid json }");
    unescape_json_strings(&mut value);
    assert_eq!(value, json!("{ not valid json }"));
}

#[test]
fn an_external_data_envelope_is_restructured() {
    let raw = format!(
        "{EXTERNAL_DATA_START}\nprovenance=tool:web_fetch\n{{\"ok\": true}}\n{EXTERNAL_DATA_END}"
    );
    let mut value = json!({ "content": raw });
    unescape_json_strings(&mut value);

    let envelope = &value["content"]["EXTERNAL_DATA"];
    assert_eq!(envelope["provenance"], json!("provenance=tool:web_fetch"));
    assert_eq!(envelope["data"], json!({ "ok": true }));
}

#[test]
fn an_envelope_without_a_provenance_header_is_left_alone() {
    let raw =
        format!("{EXTERNAL_DATA_START}\nno header here\n{{\"ok\": true}}\n{EXTERNAL_DATA_END}");
    let mut value = json!(raw);
    unescape_json_strings(&mut value);
    assert_eq!(value, json!(raw));
}

#[test]
fn an_envelope_carrying_non_json_data_is_left_alone() {
    let raw = format!("{EXTERNAL_DATA_START}\nprovenance=user_md\njust prose\n{EXTERNAL_DATA_END}");
    let mut value = json!(raw);
    unescape_json_strings(&mut value);
    assert_eq!(value, json!(raw));
}

#[test]
fn numbers_booleans_and_nulls_are_untouched() {
    let mut value = json!({ "a": 1, "b": true, "c": null });
    unescape_json_strings(&mut value);
    assert_eq!(value, json!({ "a": 1, "b": true, "c": null }));
}

#[test]
fn is_json_container_recognises_objects_and_arrays_only() {
    assert!(super::is_json_container("{}"));
    assert!(super::is_json_container("[]"));
    assert!(super::is_json_container("{\"a\":1}"));
    assert!(!super::is_json_container(""));
    assert!(!super::is_json_container("{oops"));
    assert!(!super::is_json_container("oops]"));
    assert!(!super::is_json_container("hello"));
}

// ---------------------------------------------------------------------------
// display_json — the full display pipeline
// ---------------------------------------------------------------------------

#[test]
fn display_json_pretty_prints_and_unescapes() {
    let rendered = display_json(&json!({ "body": "{\"a\": 1}" }), false);
    let parsed: Value = serde_json::from_str(&rendered).expect("valid pretty JSON");
    assert_eq!(parsed, json!({ "body": { "a": 1 } }));
    assert!(rendered.contains('\n'), "pretty output is multi-line");
}

#[test]
fn display_json_replaces_an_omitted_tools_array_with_the_placeholder() {
    let rendered = display_json(&json!({ TOOLS_KEY: [{ "name": "read" }] }), true);
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(
        parsed[TOOLS_KEY],
        Value::String(OMITTED_TOOLS_PLACEHOLDER.into())
    );
}

#[test]
fn display_json_keeps_the_tools_array_when_not_omitted() {
    let rendered = display_json(&json!({ TOOLS_KEY: [{ "name": "read" }] }), false);
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert!(parsed[TOOLS_KEY].is_array());
}

// ---------------------------------------------------------------------------
// json_rows — the viewport decision
// ---------------------------------------------------------------------------

#[test]
fn visible_json_lines_caps_at_the_setting() {
    assert_eq!(visible_json_lines(100, 8), 8);
    assert_eq!(visible_json_lines(8, 8), 8);
    assert_eq!(
        visible_json_lines(3, 8),
        3,
        "a short body shows all of itself"
    );
}

#[test]
fn visible_json_lines_boundaries() {
    assert_eq!(visible_json_lines(0, 0), 0);
    assert_eq!(
        visible_json_lines(10, 0),
        0,
        "a zero-row viewport shows nothing"
    );
    assert_eq!(visible_json_lines(0, 64), 0);
    assert_eq!(visible_json_lines(1, 1), 1);
    assert_eq!(visible_json_lines(1, 0), 0);
    assert_eq!(visible_json_lines(1000, usize::MAX), 1000);
}

#[test]
fn json_body_height_scales_with_the_line_height() {
    assert_eq!(json_body_height(4, 8), 4.0 * JSON_LINE_HEIGHT);
    assert_eq!(json_body_height(20, 8), 8.0 * JSON_LINE_HEIGHT);
    assert_eq!(json_body_height(20, 64), 20.0 * JSON_LINE_HEIGHT);
    assert_eq!(json_body_height(20, 0), 0.0);
}

#[test]
fn json_line_count_counts_rendered_lines() {
    assert_eq!(json_line_count(""), 0);
    assert_eq!(json_line_count("one"), 1);
    assert_eq!(json_line_count("one\ntwo\nthree"), 3);
    assert_eq!(
        json_line_count("one\n"),
        1,
        "a trailing newline adds no line"
    );
}

#[test]
fn json_row_options_are_the_legacy_catalogue() {
    let rows: Vec<usize> = json_row_options(8)
        .iter()
        .map(|option| option.rows)
        .collect();
    assert_eq!(rows, JSON_ROW_CATALOG.to_vec());
    assert_eq!(JSON_ROW_CATALOG, [8, 16, 24, 32, 64]);
}

#[test]
fn json_row_options_keep_an_off_catalogue_value_visible_in_order() {
    let rows: Vec<usize> = json_row_options(3)
        .iter()
        .map(|option| option.rows)
        .collect();
    assert_eq!(rows, vec![3, 8, 16, 24, 32, 64]);

    let rows: Vec<usize> = json_row_options(20)
        .iter()
        .map(|option| option.rows)
        .collect();
    assert_eq!(rows, vec![8, 16, 20, 24, 32, 64]);

    let rows: Vec<usize> = json_row_options(100)
        .iter()
        .map(|option| option.rows)
        .collect();
    assert_eq!(rows, vec![8, 16, 24, 32, 64, 100]);
}

#[test]
fn an_on_catalogue_value_adds_no_option() {
    for rows in JSON_ROW_CATALOG {
        assert_eq!(json_row_options(rows).len(), JSON_ROW_CATALOG.len());
    }
}

// ---------------------------------------------------------------------------
// Row model
// ---------------------------------------------------------------------------

#[test]
fn a_boundary_row_is_not_expandable_and_has_no_body() {
    let row = DebugEntryView {
        id: 0,
        entry: boundary(1),
        json: String::new(),
    };
    assert!(row.is_boundary());
    assert!(!row.has_body());
    assert_eq!(row.json_lines(), 0);
}

#[test]
fn an_entry_row_reports_its_body() {
    let row = DebugEntryView {
        id: 1,
        entry: entry(1, DebugEntryKind::Outgoing, "x"),
        json: "{\n  \"a\": 1\n}".to_string(),
    };
    assert!(!row.is_boundary());
    assert!(row.has_body());
    assert_eq!(row.json_lines(), 3);
}

#[test]
fn an_entry_with_no_content_renders_no_body() {
    let row = DebugEntryView {
        id: 1,
        entry: entry_with(1, DebugEntryKind::Incoming, "x", None),
        json: String::new(),
    };
    assert!(!row.has_body());
}

// ---------------------------------------------------------------------------
// Expand / collapse — the explicit CollapsingHeader replacement
// ---------------------------------------------------------------------------

#[test]
fn toggle_entry_adds_then_removes() {
    let mut state = AgentDebugWindowState::new();
    assert!(!state.is_expanded(0));

    assert!(state.toggle_entry(0), "first toggle expands");
    assert!(state.is_expanded(0));

    assert!(!state.toggle_entry(0), "second toggle collapses");
    assert!(!state.is_expanded(0));
    assert_eq!(state.expanded_count(), 0);
}

#[test]
fn rows_toggle_independently() {
    let mut state = AgentDebugWindowState::new();
    state.toggle_entry(1);
    state.toggle_entry(3);
    state.toggle_entry(5);

    assert!(state.is_expanded(1));
    assert!(state.is_expanded(3));
    assert!(state.is_expanded(5));
    assert!(!state.is_expanded(2));

    state.toggle_entry(3);
    assert!(!state.is_expanded(3));
    assert!(state.is_expanded(1));
    assert!(state.is_expanded(5));
    assert_eq!(state.expanded_ids(), &HashSet::from([1, 5]));
}

#[test]
fn collapse_all_clears_and_reports_the_count() {
    let mut state = AgentDebugWindowState::new();
    state.toggle_entry(0);
    state.toggle_entry(1);
    state.toggle_entry(2);

    assert_eq!(state.collapse_all(), 3);
    assert_eq!(state.expanded_count(), 0);
    assert_eq!(
        state.collapse_all(),
        0,
        "collapsing an already-collapsed set is a no-op"
    );
}

#[test]
fn closing_the_window_keeps_the_expanded_set() {
    let mut state = AgentDebugWindowState::new();
    state.open();
    state.toggle_entry(2);

    assert!(state.close());
    assert!(!state.is_open());
    assert!(state.is_expanded(2), "hiding is not collapsing");

    assert!(state.open());
    assert!(
        state.is_expanded(2),
        "reopening shows what the user had open"
    );
}

#[test]
fn open_close_and_toggle_transitions() {
    let mut state = AgentDebugWindowState::new();
    assert!(!state.is_open());

    assert!(state.open(), "closed -> open reports the change");
    assert!(!state.open(), "open -> open reports no change");
    assert!(state.is_open());

    assert!(state.close(), "open -> closed reports the change");
    assert!(!state.close(), "closed -> closed reports no change");

    assert!(state.toggle());
    assert!(!state.toggle());
}

#[test]
fn the_default_state_is_closed_with_nothing_expanded() {
    let state = AgentDebugWindowState::default();
    assert!(!state.is_open());
    assert_eq!(state.expanded_count(), 0);
    assert_eq!(state, AgentDebugWindowState::new());
}

#[test]
fn apply_routes_every_variant_and_reports_visibility_change() {
    let mut state = AgentDebugWindowState::new();

    assert!(state.apply(AgentDebugMessage::EntryToggled { id: 4 }));
    assert!(state.is_expanded(4));

    assert!(state.apply(AgentDebugMessage::EntryToggled { id: 4 }));
    assert!(!state.is_expanded(4));

    assert!(
        !state.apply(AgentDebugMessage::CopyJsonRequested {
            content: "{\"a\":1}".to_string()
        }),
        "a deferred clipboard write changes no state"
    );
    assert!(
        !state.apply(AgentDebugMessage::ScrollReported {
            offset_y: 1.0,
            viewport_height: 2.0,
            content_height: 3.0,
        }),
        "a runtime report changes no state"
    );
}

// ---------------------------------------------------------------------------
// View model
// ---------------------------------------------------------------------------

#[test]
fn from_entries_filters_and_precomputes_the_display_json() {
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "Turn 1 — Outgoing"),
        entry(1, DebugEntryKind::Incoming, "Turn 1 — Incoming"),
    ];
    let view_model = AgentDebugViewModel::from_entries(&entries, true, "", true, 8);

    assert!(view_model.visible);
    assert_eq!(view_model.rows.len(), 3);
    assert_eq!(view_model.total_entries, 3);
    assert_eq!(view_model.filtered_out(), 0);

    assert!(view_model.rows[0].is_boundary());
    assert!(view_model.rows[0].json.is_empty(), "a divider has no body");

    assert_eq!(view_model.rows[1].id, 1);
    assert!(view_model.rows[1].has_body());
    assert!(view_model.rows[1].json.contains("\"key\": \"value\""));
}

#[test]
fn filtered_out_reports_what_the_search_hid() {
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "alpha"),
        entry(2, DebugEntryKind::Incoming, "beta"),
    ];
    let view_model = AgentDebugViewModel::from_entries(&entries, true, "alpha", true, 8);

    assert_eq!(view_model.rows.len(), 2, "divider + the alpha turn");
    assert_eq!(view_model.filtered_out(), 1);
}

#[test]
fn a_snapshot_starts_with_nothing_expanded() {
    let view_model = AgentDebugViewModel::from_entries(
        &[entry(1, DebugEntryKind::Outgoing, "a")],
        true,
        "",
        true,
        8,
    );
    assert!(view_model.expanded.is_empty());
    assert!(!view_model.is_expanded(0));
}

#[test]
fn with_expanded_drives_is_expanded() {
    let view_model = AgentDebugViewModel::from_entries(
        &[
            entry(1, DebugEntryKind::Outgoing, "a"),
            entry(2, DebugEntryKind::Incoming, "b"),
        ],
        true,
        "",
        true,
        8,
    )
    .with_expanded(HashSet::from([1]));

    assert!(!view_model.is_expanded(0));
    assert!(view_model.is_expanded(1));
}

#[test]
fn the_snapshot_carries_a_selectable_option_for_the_current_value() {
    let catalogue = AgentDebugViewModel::from_entries(&[], true, "", true, 32);
    assert_eq!(catalogue.json_row_options.len(), JSON_ROW_CATALOG.len());
    assert_eq!(
        catalogue.json_rows_option(),
        Some(&JsonRowsOption { rows: 32 })
    );

    let off_catalogue = AgentDebugViewModel::from_entries(&[], true, "", true, 3);
    assert_eq!(
        off_catalogue.json_row_options.len(),
        JSON_ROW_CATALOG.len() + 1
    );
    assert_eq!(
        off_catalogue.json_rows_option(),
        Some(&JsonRowsOption { rows: 3 })
    );
}

#[test]
fn an_empty_snapshot_renders_an_empty_list_rather_than_nothing() {
    let view_model = AgentDebugViewModel::from_entries(&[], true, "", true, 8);
    assert!(view_model.is_empty());

    let _card = agent_debug_card(&view_model);
    let _list = entry_list(&view_model);
    let _element = view_model.view();
}

#[test]
fn a_populated_snapshot_renders_collapsed_and_expanded_rows() {
    let entries = vec![
        boundary(1),
        entry(1, DebugEntryKind::Outgoing, "Turn 1 — Outgoing"),
        entry(1, DebugEntryKind::ToolResults, "Turn 1 — Tool results (1)"),
    ];
    let view_model = AgentDebugViewModel::from_entries(&entries, true, "", true, 8)
        .with_expanded(HashSet::from([2]));

    let _card = agent_debug_card(&view_model);
    let _list = entry_list(&view_model);

    let _collapsed = entry_row(&view_model.rows[1], false, 8);
    let _expanded = entry_row(&view_model.rows[2], true, 8);
    let _divider = entry_row(&view_model.rows[0], true, 8);
}

#[test]
fn from_app_reads_the_same_accessors_the_legacy_window_used() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.agent_panel_state.show_debug_window = true;
    app.orchestrator.agent_panel_state.debug_search_text = "incoming".to_string();
    app.orchestrator.agent_panel_state.debug_auto_scroll = false;
    app.orchestrator.agent_panel_state.debug_json_rows = 24;

    app.orchestrator
        .agent
        .state_mut()
        .debug_entries
        .push(boundary(1));
    app.orchestrator.agent.state_mut().debug_entries.push(entry(
        1,
        DebugEntryKind::Outgoing,
        "Turn 1 — Outgoing",
    ));
    app.orchestrator.agent.state_mut().debug_entries.push(entry(
        1,
        DebugEntryKind::Incoming,
        "Turn 1 — Incoming",
    ));

    let view_model = AgentDebugViewModel::from_app(&app);

    assert!(view_model.visible);
    assert_eq!(view_model.search_text, "incoming");
    assert!(!view_model.auto_scroll);
    assert_eq!(view_model.json_rows, 24);
    assert_eq!(view_model.total_entries, 3);
    assert_eq!(view_model.rows.len(), 2, "divider + the incoming turn");
    assert_eq!(
        view_model.json_rows_option().map(|option| option.rows),
        Some(24)
    );
}

// ---------------------------------------------------------------------------
// Auto-scroll — the reused [TASK-016] intent rule
// ---------------------------------------------------------------------------

#[test]
fn follow_intent_follows_only_while_already_at_the_bottom() {
    let view_model = AgentDebugViewModel::from_entries(&[], true, "", true, 8);

    assert!(view_model.follow_intent(Some(viewport(800.0))));
    assert!(
        !view_model.follow_intent(Some(viewport(100.0))),
        "a user who scrolled up is never yanked back down"
    );
    assert!(
        !view_model.follow_intent(None),
        "with no geometry reported yet there is nothing to follow"
    );
}

#[test]
fn follow_intent_stops_the_moment_the_preference_is_off() {
    let view_model = AgentDebugViewModel::from_entries(&[], true, "", false, 8);
    assert!(!view_model.follow_intent(Some(viewport(800.0))));
    assert!(!view_model.follow_intent(None));
}

#[test]
fn follow_intent_follows_when_the_content_fits_the_viewport() {
    let fits = ScrollSnapshot {
        offset_y: 0.0,
        viewport_height: 500.0,
        content_height: 100.0,
    };
    let view_model = AgentDebugViewModel::from_entries(&[], true, "", true, 8);
    assert!(view_model.follow_intent(Some(fits)));
}

#[test]
fn follow_intent_is_the_logs_rule_itself() {
    // Same inputs, same answer as the log viewer's rule — the reuse is
    // real, not a look-alike copy.
    let view_model = AgentDebugViewModel::from_entries(&[], true, "", true, 8);
    for offset in [0.0, 400.0, 799.0, 800.0, 801.0] {
        assert_eq!(
            view_model.follow_intent(Some(viewport(offset))),
            crate::ui_iced::logs::should_follow(true, Some(viewport(offset))),
            "offset {offset} must agree with logs::should_follow"
        );
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

#[test]
fn header_text_matches_the_legacy_shape() {
    let entry = entry(1, DebugEntryKind::ToolResults, "Turn 1 — Tool results (1)");
    assert_eq!(
        header_text(&entry),
        "12:04:31.220  [ToolResults]  Turn 1 — Tool results (1)"
    );
}

#[test]
fn kind_label_covers_every_kind_from_the_shared_strings() {
    assert_eq!(
        kind_label(DebugEntryKind::Outgoing),
        strings::DEBUG_KIND_OUTGOING
    );
    assert_eq!(
        kind_label(DebugEntryKind::Incoming),
        strings::DEBUG_KIND_INCOMING
    );
    assert_eq!(
        kind_label(DebugEntryKind::ToolResults),
        strings::DEBUG_KIND_TOOL_RESULTS
    );
}

#[test]
fn chevron_glyph_tracks_the_expanded_state() {
    assert_eq!(chevron_glyph(true), strings::ICON_CARET_DOWN);
    assert_eq!(chevron_glyph(false), strings::ICON_CARET_RIGHT);
    assert_ne!(chevron_glyph(true), chevron_glyph(false));
}

// ---------------------------------------------------------------------------
// Message-channel split
// ---------------------------------------------------------------------------

#[test]
fn window_settings_and_close_ride_the_command_channel() {
    assert_eq!(
        search_command("foo".to_string()),
        Message::Command(UserCommand::SetDebugSearchText("foo".to_string()))
    );
    assert_eq!(
        auto_scroll_command(true),
        Message::Command(UserCommand::SetDebugAutoScroll(true))
    );
    assert_eq!(
        auto_scroll_command(false),
        Message::Command(UserCommand::SetDebugAutoScroll(false))
    );
    assert_eq!(
        clear_command(),
        Message::Command(UserCommand::ClearDebugEntries)
    );
    assert_eq!(
        dismiss_command(),
        Message::Command(UserCommand::ToggleDebugWindow(false))
    );
    assert_eq!(
        JsonRowsOption { rows: 16 }.command(),
        Message::Command(UserCommand::SetDebugJsonRows(16))
    );
}

#[test]
fn disclosure_copy_and_scroll_are_their_own_channel_never_a_command() {
    let messages = [
        toggle_entry_command(3),
        copy_json_command("{\"a\": 1}".to_string()),
        scroll_reported_message(&viewport(800.0)),
    ];

    for message in messages {
        assert!(
            matches!(message, Message::AgentDebug(_)),
            "a debug-window transition must arrive on its own channel"
        );
        assert!(
            !matches!(message, Message::Command(_)),
            "a debug-window transition must not masquerade as a published command"
        );
    }
}

#[test]
fn the_channel_variants_stay_separable() {
    let all = [
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 1 }),
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 2 }),
        Message::AgentDebug(AgentDebugMessage::CopyJsonRequested {
            content: "{}".to_string(),
        }),
        Message::AgentDebug(AgentDebugMessage::CopyJsonRequested {
            content: "[]".to_string(),
        }),
        Message::AgentDebug(AgentDebugMessage::ScrollReported {
            offset_y: 1.0,
            viewport_height: 2.0,
            content_height: 3.0,
        }),
        Message::AgentDebug(AgentDebugMessage::ScrollReported {
            offset_y: 9.0,
            viewport_height: 2.0,
            content_height: 3.0,
        }),
    ];

    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "debug message {i} and {j} must stay separable");
            }
        }
    }
}

#[test]
fn scroll_reported_message_mirrors_the_snapshot_it_was_given() {
    let snapshot = ScrollSnapshot {
        offset_y: 12.5,
        viewport_height: 200.0,
        content_height: 1000.0,
    };
    assert_eq!(
        scroll_reported_message(&snapshot),
        Message::AgentDebug(AgentDebugMessage::ScrollReported {
            offset_y: 12.5,
            viewport_height: 200.0,
            content_height: 1000.0,
        })
    );
}

#[test]
fn toggle_entry_command_carries_the_row_id() {
    assert_eq!(
        toggle_entry_command(7),
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 7 })
    );
    assert_ne!(
        toggle_entry_command(7),
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 8 })
    );
}

#[test]
fn copy_json_command_carries_the_exact_payload() {
    let json = "{\n  \"a\": 1\n}".to_string();
    assert_eq!(
        copy_json_command(json.clone()),
        Message::AgentDebug(AgentDebugMessage::CopyJsonRequested { content: json })
    );
}
