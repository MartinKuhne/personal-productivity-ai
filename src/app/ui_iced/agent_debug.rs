//! Agent debug window — the iced port of
//! `src/app/ui/agent_debug_window.rs` ([TASK-017b4], UI-048 / REQ-008).
//!
//! A **pure view over a plain snapshot**: [`agent_debug_card`] renders an
//! [`AgentDebugViewModel`] into an [`iced::Element`] of [`Message`] and
//! touches neither the bus, the disk nor the agent. The
//! [TASK-022]/[TASK-023] reducer owns the window's own state
//! ([`AgentDebugWindowState`]) and executes the [`UserCommand`]s and
//! deferred intents the card emits.
//!
//! # Debug-entry data source — reused unchanged ([RUST-051])
//!
//! | Field | Source |
//! | --- | --- |
//! | Entries | `AgentSession::state().debug_entries` (`src/agent/session.rs:33`, appended by `push_debug_entry` at `:215`) — the exact read the egui window performed (`src/app/ui/agent_debug_window.rs:21`) |
//! | Types | `AgentDebugEntry` / `DebugEntryKind` / `DebugEntryRow` (`src/agent/events.rs:23-62`), reached through the existing bus re-export `crate::bus::events::debug` (`src/app/bus/events/debug.rs:3`); re-exported here, never re-declared |
//! | Search / auto-scroll / json-rows / open flag | `AgentPanelState` (`src/app/ui/agent/panel_state.rs:13-31`) — the UI-owned panel state the egui window mutated in place, read here through the same fields |
//! | External-data markers | `crate::agent::datamark::{EXTERNAL_DATA_START, EXTERNAL_DATA_END}` (`src/agent/datamark.rs:27`, `:32`) — the envelope the agent itself writes, not re-typed literals ([RUST-021]) |
//!
//! **Live streaming needs no new plumbing.** The agent pushes entries onto
//! that same `Vec` while a turn runs; the [TASK-022] subscription bridge
//! turns each inbound `AgentEvent::DebugEntry` into a redraw and the next
//! snapshot picks the new rows up. The view never pulls from a live stream
//! at render time ([FMD-065]) and never writes to the buffer ([FMD-069]).
//!
//! # `CollapsingHeader` → an explicit expanded-id set
//!
//! `src/app/ui/agent_debug_window.rs:170` is the **only**
//! `CollapsingHeader` in the codebase (row A9 of
//! `doc/planning/iced-migration-map.md:30`, compile-checked in
//! TASK-001), and iced 0.14 has no equivalent widget. The mapping's
//! offered shapes — "`toggler`-driven branch, or `container` with a
//! disclose-arrow `Row`" — are taken in the second form:
//!
//! ```text
//! mouse_area(Row[ caret glyph, "HH:MM:SS.fff  [Kind]  summary" ])
//!     .on_press(Message::AgentDebug(AgentDebugMessage::EntryToggled { id }))
//! ```
//!
//! What egui kept inside the header widget's own memory is here a
//! `HashSet<usize>` of row ids held by
//! [`AgentDebugWindowState::expanded_ids`] and copied into the snapshot
//! the card renders, so the disclosure state is plain data: inspectable,
//! replayable and unit-testable without a runtime.
//!
//! A row id is the entry's index in the **raw** debug-entry list — egui's
//! `orig_idx` (`ui/agent_debug_window.rs:106`), the very value its
//! `push_id(orig_idx)` / `id_salt(("entry_header", id_salt))` used, so
//! row identity is identical across both builds. Two consequences the
//! reducer must honour ([RUST-041] drift note):
//!
//! * ids are **positional**, so `UserCommand::ClearDebugEntries` must be
//!   followed by [`AgentDebugWindowState::collapse_all`] — otherwise a
//!   stale id silently re-expands an unrelated new entry;
//! * egui's `.default_open(false)` maps to the empty set
//!   [`AgentDebugWindowState::new`] starts from: nothing is expanded
//!   until the user clicks.
//!
//! # Auto-scroll — the [TASK-016] intent model, reused verbatim
//!
//! egui got stick-to-bottom free from `ScrollArea::stick_to_bottom`
//! (`ui/agent_debug_window.rs:103`); iced 0.14 has no such flag, so this
//! window uses the **same** rule [`crate::ui_iced::logs`] established for
//! the log viewer rather than a second copy of it:
//!
//! ```text
//! Scrollable::on_scroll ─▶ Message::AgentDebug(ScrollReported { .. })  (view emits)
//!                                   │
//!              logs::should_follow(auto_scroll, Some(snapshot))        (pure decision)
//!                                   │
//!   operation::scrollable::scroll_to(DEBUG_SCROLLABLE_ID, AbsoluteOffset { y: snapshot.max_offset() })
//!                                                    (TASK-022/023 reducer Task)
//! ```
//!
//! [`AgentDebugViewModel::follow_intent`] delegates straight to
//! [`crate::ui_iced::logs::should_follow`] — one rule, two windows, so
//! "a user who scrolled up is never yanked back down" cannot drift
//! between them. The report type is [`ScrollSnapshot`], captured by the
//! reused [`crate::ui_iced::logs::scroll_snapshot`] helper.
//!
//! # `json_rows` semantics
//!
//! The setting is a **viewport size, not a truncation**. egui computed
//! `max_h = text_style_height(Monospace) × json_rows` and capped a
//! scrollable area with it
//! (`ui/agent_debug_window.rs:204-207`), so an expanded body shows
//! `min(lines, json_rows)` lines and scrolls to reach the rest — both
//! vertically and horizontally (`desired_width(f32::INFINITY)`).
//! [`json_body_height`] reproduces that cap and
//! [`visible_json_lines`] the count. The catalogue is egui's
//! `[8, 16, 24, 32, 64]` ([`JSON_ROW_CATALOG`], default `8` from
//! `AgentPanelState::default`); [`json_row_options`] additionally keeps
//! an off-catalogue value visible, which iced's `pick_list` cannot do on
//! its own.
//!
//! # Message-channel choices
//!
//! * **Search, auto-scroll, json-rows, clear and close all stay on
//!   [`Message::Command`]** — `UserCommand::SetDebugSearchText` /
//!   `SetDebugAutoScroll` / `SetDebugJsonRows` / `ClearDebugEntries` /
//!   `ToggleDebugWindow(bool)` (`src/app/bus/events/user_command.rs:152-160`)
//!   are exactly what the egui window published or mutated to the same
//!   effect, and each is self-contained ([FMD-068]). Nothing is
//!   duplicated.
//! * **Disclosure needed a new variant.** An entry's open/closed state has
//!   no [`UserCommand`] equivalent — it is view-local, not application
//!   state — so it arrives as [`AgentDebugMessage::EntryToggled`].
//! * **Copy-JSON is deferred intent.** egui called `Ui::copy_text` inside
//!   the click handler (`ui/agent_debug_window.rs:177-184`); iced exposes
//!   the clipboard to `update` only
//!   (`iced_core-0.14.0/src/clipboard.rs`), so the exact payload rides
//!   [`AgentDebugMessage::CopyJsonRequested`] — the same shape
//!   `RenderMessage::CopyCodeRequested` and
//!   `AboutMessage::CopyCommitHashRequested` use.
//! * **`ScrollReported` is a runtime report**, not intent — the same
//!   class of payload `LogsMessage::ScrollReported` and
//!   `LayoutMessage::CursorMoved` carry.
//! * **Dismissal is a command, not a new variant.** The egui window's
//!   close path published `UserCommand::ToggleDebugWindow(false)`
//!   (`ui/agent_debug_window.rs:149-153`), so the scrim click / Escape
//!   emits [`dismiss_command`] rather than inventing a
//!   `AgentDebugMessage::CloseRequested`.
//!
//! # Verified iced 0.14 APIs (local registry sources)
//!
//! | API | Citation |
//! | --- | --- |
//! | `mouse_area(w)` + `.on_press` + `.interaction` | `iced_widget-0.14.2/src/mouse_area.rs:68-71`, `:127-131` |
//! | `checkbox(is_checked)` + `.label` + `.on_toggle` | `iced_widget-0.14.2/src/helpers.rs:1281-1289`, `checkbox.rs:145`, `:155` |
//! | `pick_list(options, selected, on_select)`, `T: ToString + PartialEq + Clone` | `pick_list.rs:179-195` |
//! | `scrollable(...)` `.id` / `.height` / `.on_scroll` | `scrollable.rs:144-168` |
//! | `text(...).wrapping(Wrapping::None)` (default is `Word`) | `iced_core-0.14.0/src/text.rs:181-193`, `src/widget/text.rs:152-155` |
//! | default `LineHeight` is `Relative(1.3)` | `iced_core-0.14.0/src/text.rs:215-218` |
//! | `mouse::Interaction::Pointer` | `iced_core-0.14.0/src/mouse/interaction.rs:11` |
//! | `widget::Id: From<&'static str>` | `iced_core-0.14.0/src/widget/id.rs:26-30` |
//!
//! Unit tests live in the sibling `agent_debug_tests.rs` sidecar.

#[cfg(test)]
#[path = "agent_debug_tests.rs"]
mod tests;

use std::collections::HashSet;

use iced::widget::{
    Column, Row, button, checkbox, container, mouse_area, pick_list, rule, scrollable, space, text,
    text_input,
};
use iced::{Alignment, Element, Font, Length};

use crate::agent::datamark::{EXTERNAL_DATA_END, EXTERNAL_DATA_START};
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::bus::events::user_command::UserCommand;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logs::{self, ScrollSnapshot};
use crate::ui_iced::message::{AgentDebugMessage, Message};
use crate::ui_iced::panels::top::phosphor_font;
use crate::ui_iced::theme::to_iced_color;

/// Width of the debug card — egui parity (`default_size: [600.0, …]`,
/// `src/app/ui/agent_debug_window.rs:19`).
pub const CARD_WIDTH: f32 = 600.0;

/// Logical point size of an entry's header row (egui's `Body` style, the
/// size [`crate::ui_iced::render::code`] recorded for it).
pub const ENTRY_TEXT_SIZE: f32 = 14.0;

/// Logical point size of a session-boundary label — egui parity
/// (`RichText::new(..).size(12.0)`, `src/app/ui/agent_debug_window.rs:114`).
pub const BOUNDARY_TEXT_SIZE: f32 = 12.0;

/// Logical point size of the card title.
pub const TITLE_SIZE: f32 = 18.0;

/// Logical point size of the expanded JSON body (egui rendered it in
/// `TextStyle::Monospace`, `src/app/ui/agent_debug_window.rs:210`).
pub const JSON_TEXT_SIZE: f32 = 14.0;

/// Height of one JSON line in logical points.
///
/// iced's default [`LineHeight`](iced::widget::text::LineHeight) is
/// `Relative(1.3)` (`iced_core-0.14.0/src/text.rs:215-218`), so a
/// [`JSON_TEXT_SIZE`] line occupies `JSON_TEXT_SIZE × 1.3` points. This
/// is the iced stand-in for egui's
/// `ui.text_style_height(&TextStyle::Monospace)`
/// (`src/app/ui/agent_debug_window.rs:204`) and is what makes the
/// `json_rows` setting mean "N visible lines".
pub const JSON_LINE_HEIGHT: f32 = JSON_TEXT_SIZE * 1.3;

/// The `json_rows` catalogue the pick list offers — egui parity
/// (`[8usize, 16, 24, 32, 64]`, `src/app/ui/agent_debug_window.rs:37`).
pub const JSON_ROW_CATALOG: [usize; 5] = [8, 16, 24, 32, 64];

/// `chrono` format of the row timestamp, carried over verbatim from the
/// egui header (`entry.timestamp.format("%H:%M:%S%.3f")`,
/// `src/app/ui/agent_debug_window.rs:165`).
pub const TIMESTAMP_FORMAT: &str = "%H:%M:%S%.3f";

/// The top-level JSON key whose duplicate occurrences are elided
/// (`content.as_object()…contains_key("tools")`,
/// `src/app/ui/agent_debug_window.rs:87`).
pub const TOOLS_KEY: &str = "tools";

/// The `provenance=` header prefix the external-data envelope starts with.
///
/// `crate::agent::datamark` builds this inline
/// (`format!("provenance=tool:{}", name)`, `src/agent/datamark.rs:72`)
/// and exports no constant, so the prefix is named here rather than
/// repeated as a literal ([RUST-021]).
pub const PROVENANCE_PREFIX: &str = "provenance=";

/// What replaces a `tools` array an earlier entry in the same session
/// already showed ([`strings::DEBUG_OMITTED_TOOLS_PLACEHOLDER`], the
/// same text the egui row inserted at `ui/agent_debug_window.rs:128`).
pub const OMITTED_TOOLS_PLACEHOLDER: &str = strings::DEBUG_OMITTED_TOOLS_PLACEHOLDER;

/// Stable iced widget id of the entry list's `scrollable`.
///
/// The [TASK-022]/[TASK-023] reducer must aim
/// `operation::scrollable::scroll_to` at a stable id
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`); a widget
/// without one cannot be scrolled programmatically at all. Named once so
/// view and reducer cannot drift ([RUST-021]).
pub const DEBUG_SCROLLABLE_ID: &str = "agent_debug_scroll";

/// Padding inside the card.
pub const CARD_PADDING: f32 = 12.0;

/// Vertical gap between card sections.
pub const SECTION_SPACING: f32 = 8.0;

/// Horizontal gap between the controls in the header row.
pub const CONTROL_SPACING: f32 = 6.0;

/// Vertical gap between rendered entry rows.
pub const ENTRY_SPACING: f32 = 2.0;

/// Gap between a row's header and its expanded body.
pub const BODY_SPACING: f32 = 4.0;

/// Gap between a row's chevron and its label.
pub const CHEVRON_SPACING: f32 = 6.0;

/// Vertical padding inside a clickable header row — the hit area egui's
/// `CollapsingHeader` title gave.
pub const HEADER_PADDING_Y: f32 = 4.0;

/// Width of the rules separating card sections and boundary rows.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// The search text in the form the filter compares against — lower-cased
/// once, exactly as the egui window did before filtering
/// (`src/app/ui/agent_debug_window.rs:58-62`).
///
/// Purity: pure.
#[must_use]
pub fn normalized_search(search: &str) -> String {
    search.to_lowercase()
}

/// Whether one debug entry passes the window's search filter.
///
/// The egui predicate, lifted out of the render closure verbatim
/// (`src/app/ui/agent_debug_window.rs:66-74`):
///
/// * a [`DebugEntryRow::SessionBoundary`] **always** passes, so the
///   filtered list keeps its session structure instead of collapsing into
///   an undifferentiated run of rows;
/// * an empty search passes everything;
/// * otherwise the row passes when its `summary` contains the needle —
///   i.e. the match is **case-insensitive** and **summary-only**. The
///   JSON body is not searched, egui parity.
///
/// `search_lower` must already be lower-cased ([`normalized_search`]).
///
/// Purity: pure function of `entry` and `search_lower`.
#[must_use]
pub fn entry_matches(entry: &AgentDebugEntry, search_lower: &str) -> bool {
    if matches!(entry.row_type, DebugEntryRow::SessionBoundary) {
        return true;
    }
    if search_lower.is_empty() {
        return true;
    }
    entry.summary.to_lowercase().contains(search_lower)
}

/// The filtered `(original index, entry)` pairs, in list order — egui's
/// `filtered` vector (`src/app/ui/agent_debug_window.rs:63-75`).
///
/// Original indices are carried, not re-numbered: they are the row ids
/// the expanded set keys on.
///
/// Purity: pure over `entries` and `search`.
#[must_use]
pub fn filter_debug_entries<'a>(
    entries: &'a [AgentDebugEntry],
    search: &str,
) -> Vec<(usize, &'a AgentDebugEntry)> {
    let needle = normalized_search(search);
    entries
        .iter()
        .enumerate()
        .filter(|(_index, entry)| entry_matches(entry, &needle))
        .collect()
}

/// Which filtered entries must have their `tools` array replaced because
/// an earlier entry in the same session already showed one.
///
/// Port of the egui pass at `src/app/ui/agent_debug_window.rs:77-97`.
/// Walking the filtered list in order, the **first** entry of a session
/// that carries a top-level `"tools"` key is kept whole; every later one
/// in that session is a duplicate — the agent re-sends the same tool
/// catalog on every turn — and gets
/// [`OMITTED_TOOLS_PLACEHOLDER`] instead. A
/// [`DebugEntryRow::SessionBoundary`] resets the flag, so a new session
/// shows its own tools again.
///
/// Returns **original** indices, matching [`filter_debug_entries`].
///
/// Purity: pure function of `filtered`.
#[must_use]
pub fn duplicate_tool_indices(filtered: &[(usize, &AgentDebugEntry)]) -> HashSet<usize> {
    let mut omitted = HashSet::new();
    let mut seen_tools_this_session = false;

    for (original_index, entry) in filtered {
        match entry.row_type {
            DebugEntryRow::SessionBoundary => {
                seen_tools_this_session = false;
            }
            DebugEntryRow::Entry => {
                if let Some(content) = &entry.content
                    && let Some(object) = content.as_object()
                    && object.contains_key(TOOLS_KEY)
                {
                    if seen_tools_this_session {
                        omitted.insert(*original_index);
                    } else {
                        seen_tools_this_session = true;
                    }
                }
            }
        }
    }

    omitted
}

/// Recursively un-escapes double-encoded JSON strings and external-data
/// envelopes inside a debug payload so the expanded body reads as real
/// structure instead of escaped text.
///
/// Verbatim port of `unescape_json_strings`
/// (`src/app/ui/agent_debug_window.rs:222-273`):
///
/// * objects and arrays recurse into their children;
/// * a string that trims to `{…}` / `[…]` is parsed and replaced by the
///   parsed value, recursively — this is what turns the agent's
///   string-encoded request/response bodies into pretty-printable JSON;
/// * a `<<<EXTERNAL_DATA>>> … <<<END_EXTERNAL_DATA>>>` envelope whose
///   first inner line is a `provenance=…` header is rewritten to
///   `{"EXTERNAL_DATA": {"provenance": …, "data": …}}` with the payload
///   parsed the same way;
/// * every other value is left exactly as it arrived.
///
/// The JSON branch returns before the envelope branch is tried, exactly as
/// the original did, so a string that parses as JSON is never mistaken
/// for an envelope.
///
/// The only effect is on `value`, as in the original.
pub fn unescape_json_strings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (_, item) in map.iter_mut() {
                unescape_json_strings(item);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                unescape_json_strings(item);
            }
        }
        serde_json::Value::String(string) => {
            let trimmed = string.trim();
            if is_json_container(trimmed)
                && let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(trimmed)
            {
                unescape_json_strings(&mut parsed);
                *value = parsed;
                return;
            }

            if trimmed.starts_with(EXTERNAL_DATA_START) && trimmed.ends_with(EXTERNAL_DATA_END) {
                let inner =
                    &trimmed[EXTERNAL_DATA_START.len()..trimmed.len() - EXTERNAL_DATA_END.len()];
                let inner = inner.trim();
                if let Some((provenance, data)) = inner.split_once('\n')
                    && provenance.starts_with(PROVENANCE_PREFIX)
                {
                    let data_trimmed = data.trim();
                    if is_json_container(data_trimmed)
                        && let Ok(mut parsed) =
                            serde_json::from_str::<serde_json::Value>(data_trimmed)
                    {
                        unescape_json_strings(&mut parsed);
                        *value = serde_json::json!({
                            "EXTERNAL_DATA": {
                                "provenance": provenance,
                                "data": parsed
                            }
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

/// Whether `trimmed` looks like a JSON object or array — the same
/// bracket test the egui un-escaper ran
/// (`src/app/ui/agent_debug_window.rs:236-237`, `:255-256`).
///
/// Purity: pure.
#[must_use]
fn is_json_container(trimmed: &str) -> bool {
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

/// The JSON an expanded row shows.
///
/// The egui display pipeline in the egui order
/// (`src/app/ui/agent_debug_window.rs:120-135`, `:181-188`, `:203`):
/// replace a duplicated `tools` array, un-escape the double-encoded
/// payloads, then `serde_json::to_string_pretty`. A serialization
/// failure renders as the empty string — the same `unwrap_or_default`
/// egui used, so a malformed payload degrades to a blank body rather
/// than taking the window down ([FMD-064]).
///
/// Purity: pure function of `content` and `omit_tools`.
#[must_use]
pub fn display_json(content: &serde_json::Value, omit_tools: bool) -> String {
    let mut display = content.clone();

    if omit_tools && let Some(object) = display.as_object_mut() {
        object.insert(
            TOOLS_KEY.to_string(),
            serde_json::Value::String(OMITTED_TOOLS_PLACEHOLDER.to_string()),
        );
    }

    unescape_json_strings(&mut display);
    serde_json::to_string_pretty(&display).unwrap_or_default()
}

/// How many lines a JSON body holds.
///
/// Counted on the rendered text because the body is drawn with
/// `Wrapping::None` (egui's `desired_width(f32::INFINITY)`), so one
/// source line is exactly one rendered line.
///
/// Purity: pure.
#[must_use]
pub fn json_line_count(json: &str) -> usize {
    json.lines().count()
}

/// How many of a body's `total_lines` the `json_rows` viewport shows.
///
/// `min(total_lines, json_rows)`: the setting is a viewport size, not a
/// truncation — the rest stays reachable by scrolling
/// (see the module header).
///
/// Boundaries: `json_rows == 0` shows nothing (egui's `max_h == 0`
/// collapsed the area; the pick list never offers 0 but the command
/// channel can carry it); `json_rows >= total_lines` shows everything
/// and the body does not scroll.
///
/// Purity: pure.
#[must_use]
pub fn visible_json_lines(total_lines: usize, json_rows: usize) -> usize {
    total_lines.min(json_rows)
}

/// Height, in logical points, of an expanded JSON body.
///
/// `visible_json_lines(total_lines, json_rows) × JSON_LINE_HEIGHT` — the
/// iced equivalent of egui's `ScrollArea::max_height(max_h)` with
/// `max_h = text_style_height(Monospace) × json_rows`
/// (`src/app/ui/agent_debug_window.rs:204-207`). A short body takes its
/// own height; a long one stops at the setting and scrolls.
///
/// Purity: pure.
#[must_use]
pub fn json_body_height(total_lines: usize, json_rows: usize) -> f32 {
    visible_json_lines(total_lines, json_rows) as f32 * JSON_LINE_HEIGHT
}

/// One entry of the "JSON rows" pick list ([TASK-017b4]).
///
/// Carries the number rather than its label so a selection can never be
/// mistaken for the value it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonRowsOption {
    /// How many JSON lines an expanded body shows when this is picked.
    pub rows: usize,
}

impl JsonRowsOption {
    /// The intent this option publishes when picked — the existing
    /// `UserCommand::SetDebugJsonRows`, i.e. what the egui combo wrote
    /// straight into `AgentPanelState::debug_json_rows`
    /// (`src/app/ui/agent_debug_window.rs:41-47`,
    /// `src/app/command_executor.rs:587`).
    #[must_use]
    pub fn command(self) -> Message {
        Message::Command(UserCommand::SetDebugJsonRows(self.rows))
    }
}

impl std::fmt::Display for JsonRowsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rows)
    }
}

/// The pick-list options for the current `json_rows` value.
///
/// The egui catalogue [`JSON_ROW_CATALOG`]
/// (`src/app/ui/agent_debug_window.rs:37-40`) plus the current value
/// inserted in ascending order when it is not already one of them.
/// `UserCommand::SetDebugJsonRows` accepts any `usize`, and iced's
/// `pick_list` can only display a value it can find among its options —
/// without this, an off-catalogue setting would render as the empty
/// placeholder and hide what the window is actually doing.
///
/// Purity: pure.
#[must_use]
pub fn json_row_options(current: usize) -> Vec<JsonRowsOption> {
    let mut options: Vec<JsonRowsOption> = JSON_ROW_CATALOG
        .iter()
        .map(|&rows| JsonRowsOption { rows })
        .collect();

    if !options.iter().any(|option| option.rows == current) {
        let position = options
            .iter()
            .position(|other| other.rows > current)
            .unwrap_or(options.len());
        options.insert(position, JsonRowsOption { rows: current });
    }

    options
}

/// The kind column's label, from `crate::ui::strings` ([GUD-002]).
///
/// Purity: pure.
#[must_use]
pub fn kind_label(kind: DebugEntryKind) -> &'static str {
    match kind {
        DebugEntryKind::Outgoing => strings::DEBUG_KIND_OUTGOING,
        DebugEntryKind::Incoming => strings::DEBUG_KIND_INCOMING,
        DebugEntryKind::ToolResults => strings::DEBUG_KIND_TOOL_RESULTS,
    }
}

/// The collapsed row's label: `HH:MM:SS.fff  [Kind]  summary`.
///
/// egui parity (`src/app/ui/agent_debug_window.rs:163-168`), separators
/// included.
///
/// Purity: pure function of `entry`.
#[must_use]
pub fn header_text(entry: &AgentDebugEntry) -> String {
    format!(
        "{}  [{}]  {}",
        entry.timestamp.format(TIMESTAMP_FORMAT),
        kind_label(entry.kind),
        entry.summary,
    )
}

/// The disclose arrow a row's header shows — the
/// `CollapsingHeader` triangle's iced stand-in, drawn from the Phosphor
/// set the top bar already loads ([`phosphor_font`]).
///
/// Purity: pure.
#[must_use]
pub fn chevron_glyph(expanded: bool) -> &'static str {
    if expanded {
        strings::ICON_CARET_DOWN
    } else {
        strings::ICON_CARET_RIGHT
    }
}

/// The `SetDebugSearchText` command the search field emits on every edit.
#[must_use]
pub fn search_command(text: String) -> Message {
    Message::Command(UserCommand::SetDebugSearchText(text))
}

/// The `SetDebugAutoScroll` command the auto-scroll checkbox emits.
#[must_use]
pub fn auto_scroll_command(enabled: bool) -> Message {
    Message::Command(UserCommand::SetDebugAutoScroll(enabled))
}

/// The `ClearDebugEntries` command the Clear button emits.
///
/// The reducer must follow it with
/// [`AgentDebugWindowState::collapse_all`]: row ids are positional, so
/// an emptied buffer invalidates every expanded id.
#[must_use]
pub fn clear_command() -> Message {
    Message::Command(UserCommand::ClearDebugEntries)
}

/// The dismiss message a scrim click / Escape emits — the egui window's
/// own close path (`UserCommand::ToggleDebugWindow(false)`,
/// `src/app/ui/agent_debug_window.rs:149-153`), so no new
/// `AgentDebugMessage` variant is needed for closing.
#[must_use]
pub fn dismiss_command() -> Message {
    Message::Command(UserCommand::ToggleDebugWindow(false))
}

/// The `EntryToggled` intent a row's header emits.
#[must_use]
pub fn toggle_entry_command(id: usize) -> Message {
    Message::AgentDebug(AgentDebugMessage::EntryToggled { id })
}

/// The `CopyJsonRequested` intent an expanded body's Copy button emits,
/// carrying the exact text to put on the clipboard.
#[must_use]
pub fn copy_json_command(content: String) -> Message {
    Message::AgentDebug(AgentDebugMessage::CopyJsonRequested { content })
}

/// The `ScrollReported` runtime report the entry list emits.
///
/// Purity: pure function of `snapshot`.
#[must_use]
pub fn scroll_reported_message(snapshot: &ScrollSnapshot) -> Message {
    Message::AgentDebug(AgentDebugMessage::ScrollReported {
        offset_y: snapshot.offset_y,
        viewport_height: snapshot.viewport_height,
        content_height: snapshot.content_height,
    })
}

/// One rendered row of the debug list ([TASK-017b4]).
///
/// The bus entry travels unchanged; only the display JSON is
/// pre-computed, so the view performs no serialization, no un-escaping
/// and no dedup pass at render time ([FMD-065]).
#[derive(Clone, Debug, PartialEq)]
pub struct DebugEntryView {
    /// Stable row id: the entry's index in the raw debug-entry list it
    /// was snapshotted from — egui's `orig_idx`
    /// (`src/app/ui/agent_debug_window.rs:106`) and the salt its
    /// `CollapsingHeader` used.
    pub id: usize,
    /// The bus entry, unchanged.
    pub entry: AgentDebugEntry,
    /// The expanded body: [`display_json`] with the duplicate-tools
    /// replacement already applied. Empty when the entry carries no
    /// content (always true for a session boundary).
    pub json: String,
}

impl DebugEntryView {
    /// Whether this row is a non-interactive session divider.
    #[must_use]
    pub fn is_boundary(&self) -> bool {
        matches!(self.entry.row_type, DebugEntryRow::SessionBoundary)
    }

    /// Whether the row has a JSON body to expand into.
    #[must_use]
    pub fn has_body(&self) -> bool {
        !self.json.is_empty()
    }

    /// How many lines the body holds.
    #[must_use]
    pub fn json_lines(&self) -> usize {
        json_line_count(&self.json)
    }
}

/// Everything the debug window needs to render, snapshotted off the
/// agent and the panel state.
///
/// Same shape as [`crate::ui_iced::logs::BackgroundLogsViewModel`]: the
/// widget tree reads only this struct, which is what makes the window
/// unit-testable without a live iced runtime, a bus or a running agent.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentDebugViewModel {
    /// Whether the window is shown (`AgentPanelState::show_debug_window`).
    pub visible: bool,
    /// The rows that passed the filter, in list order.
    pub rows: Vec<DebugEntryView>,
    /// Total entries before filtering, so the UI can report how much the
    /// filter hid ([FMD-067]).
    pub total_entries: usize,
    /// The free-text search filter.
    pub search_text: String,
    /// The expanded-body viewport size, in JSON lines.
    pub json_rows: usize,
    /// The json-rows pick-list options, owned by the snapshot so the
    /// selected option can be borrowed out of it for `pick_list` — the
    /// same shape
    /// [`crate::ui_iced::logs::BackgroundLogsViewModel::category_options`]
    /// uses. Kept in step with [`Self::json_rows`] by
    /// [`json_row_options`].
    pub json_row_options: Vec<JsonRowsOption>,
    /// The stick-to-bottom preference.
    pub auto_scroll: bool,
    /// Ids of the rows currently expanded — the explicit
    /// `CollapsingHeader` state (see the module header).
    pub expanded: HashSet<usize>,
}

impl AgentDebugViewModel {
    /// Builds a snapshot from the raw pieces.
    ///
    /// Runs the egui pipeline once, at snapshot time: filter → session
    /// tools-dedup → display JSON.
    ///
    /// Purity: pure over `entries` and the four settings.
    #[must_use]
    pub fn from_entries(
        entries: &[AgentDebugEntry],
        visible: bool,
        search_text: &str,
        auto_scroll: bool,
        json_rows: usize,
    ) -> Self {
        let filtered = filter_debug_entries(entries, search_text);
        let omitted = duplicate_tool_indices(&filtered);

        let rows = filtered
            .into_iter()
            .map(|(id, entry)| DebugEntryView {
                id,
                entry: entry.clone(),
                json: entry
                    .content
                    .as_ref()
                    .map(|content| display_json(content, omitted.contains(&id)))
                    .unwrap_or_default(),
            })
            .collect();

        Self {
            visible,
            rows,
            total_entries: entries.len(),
            search_text: search_text.to_string(),
            json_rows,
            json_row_options: json_row_options(json_rows),
            auto_scroll,
            expanded: HashSet::new(),
        }
    }

    /// Snapshots the window's data off `app` — the same accessors the
    /// egui window read (`app.orchestrator.agent.state().debug_entries`
    /// at `src/app/ui/agent_debug_window.rs:21` plus the four
    /// `agent_panel_state` fields), moved out of render so the card stays
    /// I/O-free ([FMD-065]).
    #[must_use]
    pub fn from_app(app: &FastMdApp) -> Self {
        let panel = &app.orchestrator.agent_panel_state;
        Self::from_entries(
            &app.orchestrator.agent.state().debug_entries,
            panel.show_debug_window,
            &panel.debug_search_text,
            panel.debug_auto_scroll,
            panel.debug_json_rows,
        )
    }

    /// Returns a copy carrying `expanded` — how the reducer combines its
    /// own [`AgentDebugWindowState`] with a freshly snapshotted buffer.
    #[must_use]
    pub fn with_expanded(mut self, expanded: HashSet<usize>) -> Self {
        self.expanded = expanded;
        self
    }

    /// Whether row `id` renders expanded.
    #[must_use]
    pub fn is_expanded(&self, id: usize) -> bool {
        self.expanded.contains(&id)
    }

    /// The [`JsonRowsOption`] matching [`Self::json_rows`].
    ///
    /// Borrows out of [`Self::json_row_options`] rather than rebuilding
    /// the list, so the reference is valid for the snapshot's lifetime —
    /// which is what `pick_list` needs (`V: Borrow<T> + 'a`,
    /// `iced_widget-0.14.2/src/pick_list.rs:179-195`). Always `Some`,
    /// because [`json_row_options`] guarantees the current value is
    /// among the options.
    #[must_use]
    pub fn json_rows_option(&self) -> Option<&JsonRowsOption> {
        self.json_row_options
            .iter()
            .find(|option| option.rows == self.json_rows)
    }

    /// Whether the filtered list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// How many entries the current filter hid from view.
    #[must_use]
    pub fn filtered_out(&self) -> usize {
        self.total_entries.saturating_sub(self.rows.len())
    }

    /// The auto-scroll **intent** for this snapshot's preference and the
    /// last reported viewport geometry.
    ///
    /// Delegates to [`crate::ui_iced::logs::should_follow`] — the
    /// [TASK-016] rule, reused rather than copied, so both stick-to-bottom
    /// windows follow identically. Executing it is reducer work against
    /// [`DEBUG_SCROLLABLE_ID`].
    #[must_use]
    pub fn follow_intent(&self, snapshot: Option<ScrollSnapshot>) -> bool {
        logs::should_follow(self.auto_scroll, snapshot)
    }

    /// Renders this snapshot as the debug card ([`agent_debug_card`]).
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        agent_debug_card(self)
    }
}

/// Renders the agent debug card ([TASK-017b4]).
///
/// Layout, mirroring the egui window body
/// (`src/app/ui/agent_debug_window.rs:23-146`):
///
/// ```text
/// Agent Debug
/// Search: [__________]  [x] Auto-scroll  JSON rows: [ 8 ▾]  [Clear]
/// ────────────────────────────────────────────────────────────────
/// ▸ 12:04:31.220  [Outgoing]  Turn 1 — Outgoing (+2 messages)
/// ▾ 12:04:31.480  [Incoming]  Turn 1 — Incoming (assistant OK)
///                                          [ Copy JSON ]
/// ┌ monospace JSON, json_rows lines tall, scrolls both ways ───────┐
/// ────────────────────────────────────────────────────────────────
/// ──            Session 1            ──            (boundary row)
/// ```
///
/// Emitted messages:
/// * search field → [`UserCommand::SetDebugSearchText`]
/// * auto-scroll checkbox → [`UserCommand::SetDebugAutoScroll`]
/// * json-rows pick → [`UserCommand::SetDebugJsonRows`]
/// * Clear → [`UserCommand::ClearDebugEntries`]
/// * a row header → [`AgentDebugMessage::EntryToggled`]
/// * Copy JSON → [`AgentDebugMessage::CopyJsonRequested`]
/// * any scroll of the list → [`AgentDebugMessage::ScrollReported`]
///
/// The card is the *content* only: the root view mounts it through
/// [`crate::ui_iced::overlay::modal`] with [`dismiss_command()`] as the
/// dismiss message so a scrim click or Escape closes the window the same
/// way the egui title-bar `X` did.
///
/// The control row and the list are **always allocated** — an empty
/// buffer renders an empty list rather than a different widget tree
/// (AGENTS §6). An expanded body *is* added to its own row on expand:
/// iced rebuilds the tree structurally every frame, so a disclosure
/// branch does not destabilise sibling identity the way an egui
/// conditional would (same reasoning
/// [`crate::ui_iced::render::code`] records for its caption).
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn agent_debug_card(view_model: &AgentDebugViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::AGENT_DEBUG_WINDOW)
        .size(TITLE_SIZE)
        .color(to_iced_color(palette.text_strong));

    let search_label = text(strings::SEARCH_LABEL)
        .size(ENTRY_TEXT_SIZE)
        .color(to_iced_color(palette.text));

    let search_field = text_input("", &view_model.search_text).on_input(search_command);

    let auto_scroll_toggle = checkbox(view_model.auto_scroll)
        .label(strings::AUTO_SCROLL_CHECKBOX)
        .on_toggle(auto_scroll_command);

    let rows_label = text(strings::DEBUG_JSON_ROWS_LABEL)
        .size(ENTRY_TEXT_SIZE)
        .color(to_iced_color(palette.text));

    let json_rows_pick: Element<'_, Message> = pick_list(
        view_model.json_row_options.clone(),
        view_model.json_rows_option(),
        JsonRowsOption::command,
    )
    .width(Length::Shrink)
    .into();

    let clear = button(text(strings::CLEAR_BUTTON)).on_press(clear_command());

    let controls = Row::with_children(vec![
        search_label.into(),
        search_field.width(Length::Fill).into(),
        auto_scroll_toggle.into(),
        rows_label.into(),
        json_rows_pick,
        clear.into(),
    ])
    .spacing(CONTROL_SPACING)
    .align_y(Alignment::Center);

    let list = scrollable(entry_list(view_model))
        .id(DEBUG_SCROLLABLE_ID)
        .height(Length::Fill)
        .on_scroll(|viewport| scroll_reported_message(&logs::scroll_snapshot(&viewport)));

    Column::with_children(vec![
        title.into(),
        controls.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        list.into(),
    ])
    .spacing(SECTION_SPACING)
    .padding(CARD_PADDING)
    .width(Length::Fixed(CARD_WIDTH))
    .into()
}

/// The scrollable entry list, one [`entry_row`] per filtered row.
///
/// Purity: pure over `view_model`.
#[must_use]
pub fn entry_list(view_model: &AgentDebugViewModel) -> Column<'_, Message> {
    view_model
        .rows
        .iter()
        .fold(Column::new().spacing(ENTRY_SPACING), |rows, row| {
            rows.push(entry_row(
                row,
                view_model.is_expanded(row.id),
                view_model.json_rows,
            ))
        })
}

/// One debug row: its clickable header, plus the JSON body when expanded.
///
/// `json_rows` is the current viewport setting
/// ([`AgentDebugViewModel::json_rows`]), passed through so the expanded
/// body can size itself without the row having to own a copy of a
/// window-level setting.
///
/// A [`DebugEntryRow::SessionBoundary`] renders the egui divider instead
/// — rule, centred muted summary, rule (`ui/agent_debug_window.rs:108-118`) —
/// and is deliberately **not** clickable, because egui's divider had no
/// disclosure either.
///
/// Purity: pure view over `row`.
#[must_use]
pub fn entry_row(row: &DebugEntryView, expanded: bool, json_rows: usize) -> Element<'_, Message> {
    if row.is_boundary() {
        return boundary_row(row);
    }

    let mut parts: Vec<Element<'_, Message>> = vec![entry_header(row, expanded)];
    if expanded && row.has_body() {
        parts.push(expanded_body(row, json_rows));
    }

    Column::with_children(parts).spacing(BODY_SPACING).into()
}

/// The non-interactive session divider.
fn boundary_row(row: &DebugEntryView) -> Element<'_, Message> {
    let summary = text(row.entry.summary.clone())
        .size(BOUNDARY_TEXT_SIZE)
        .color(to_iced_color(Palette::fastmd_dark().text_muted))
        .width(Length::Fill);

    Column::with_children(vec![
        rule::horizontal(SEPARATOR_WIDTH).into(),
        container(summary)
            .align_x(iced::alignment::Horizontal::Center)
            .into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
    ])
    .spacing(2.0)
    .into()
}

/// The clickable header row that replaces `egui::CollapsingHeader`'s
/// title: chevron + `HH:MM:SS.fff  [Kind]  summary`, wrapped in a
/// `MouseArea` whose press toggles this row's disclosure.
fn entry_header(row: &DebugEntryView, expanded: bool) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let caret = text(chevron_glyph(expanded))
        .font(phosphor_font())
        .size(ENTRY_TEXT_SIZE)
        .color(to_iced_color(palette.text_muted));

    let label = text(header_text(&row.entry))
        .size(ENTRY_TEXT_SIZE)
        .color(to_iced_color(palette.text));

    let header = Row::with_children(vec![caret.into(), label.into()])
        .spacing(CHEVRON_SPACING)
        .align_y(Alignment::Center);

    mouse_area(
        container(header)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: HEADER_PADDING_Y,
                right: 0.0,
                bottom: HEADER_PADDING_Y,
                left: 0.0,
            }),
    )
    .on_press(toggle_entry_command(row.id))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

/// The expanded body: the right-aligned Copy button over the
/// monospace JSON, capped at the `json_rows` viewport.
///
/// The body is drawn with `Wrapping::None` so a long line keeps its
/// single-line shape and the `scrollable` scrolls horizontally for it —
/// egui's `desired_width(f32::INFINITY)` + `ScrollArea::both()`
/// behaviour (`src/app/ui/agent_debug_window.rs:207-214`).
fn expanded_body(row: &DebugEntryView, json_rows: usize) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let copy =
        button(text(strings::DEBUG_COPY_JSON_BUTTON)).on_press(copy_json_command(row.json.clone()));

    let actions = Row::with_children(vec![
        space::Space::new().width(Length::Fill).into(),
        copy.into(),
    ])
    .align_y(Alignment::Center);

    let body = text(row.json.clone())
        .font(Font::MONOSPACE)
        .size(JSON_TEXT_SIZE)
        .color(to_iced_color(palette.text))
        .wrapping(iced::widget::text::Wrapping::None);

    let scroll =
        scrollable(body).height(Length::Fixed(json_body_height(row.json_lines(), json_rows)));

    Column::with_children(vec![actions.into(), scroll.into()])
        .spacing(BODY_SPACING)
        .into()
}

/// Which debug rows are open and whether the window is showing — the pure
/// state the reducer holds ([TASK-017b4]).
///
/// Mirrors [`crate::ui_iced::about::AboutWindowState`] for the open flag
/// and adds the expanded-row set that replaces egui's
/// `CollapsingHeader` widget memory. `close()` deliberately **keeps**
/// the expanded set: hiding the window in egui left the header states
/// untouched, and reopening shows what the user had open. Clearing the
/// buffer is the one transition that invalidates the ids, which is what
/// [`Self::collapse_all`] is for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentDebugWindowState {
    open: bool,
    expanded: HashSet<usize>,
}

impl AgentDebugWindowState {
    /// A closed window with nothing expanded — the parity state for
    /// egui's `.default_open(false)`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the window is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shows the window. Returns whether the visible state changed.
    pub fn open(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.open = true;
        true
    }

    /// Hides the window, keeping the expanded set (see the type docs).
    ///
    /// Returns whether one was open — the same "was anything consumed"
    /// contract as [`crate::ui_iced::overlay::OverlaySlot::close`], so
    /// an Escape handler can tell a consumed key from a no-op.
    pub fn close(&mut self) -> bool {
        let was_open = self.open;
        self.open = false;
        was_open
    }

    /// Flips the visible state, returning the new state.
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }

    /// Whether row `id` is expanded.
    #[must_use]
    pub fn is_expanded(&self, id: usize) -> bool {
        self.expanded.contains(&id)
    }

    /// The expanded row ids.
    #[must_use]
    pub fn expanded_ids(&self) -> &HashSet<usize> {
        &self.expanded
    }

    /// How many rows are expanded.
    #[must_use]
    pub fn expanded_count(&self) -> usize {
        self.expanded.len()
    }

    /// Toggles row `id` and returns its **new** expanded state.
    ///
    /// Rows toggle independently: opening one never touches another, the
    /// behaviour egui's per-header memory gave.
    pub fn toggle_entry(&mut self, id: usize) -> bool {
        if self.expanded.remove(&id) {
            false
        } else {
            self.expanded.insert(id);
            true
        }
    }

    /// Collapses every row and returns how many were open.
    ///
    /// egui has no collapse-all control, so no widget emits this — it
    /// exists for the reducer, which must call it whenever the debug
    /// buffer is cleared. Row ids are positional, so after
    /// `UserCommand::ClearDebugEntries` a stale id would silently expand
    /// an unrelated new entry.
    pub fn collapse_all(&mut self) -> usize {
        let count = self.expanded.len();
        self.expanded.clear();
        count
    }

    /// Applies one [`AgentDebugMessage`] to the window state and reports
    /// whether anything the user sees changed.
    ///
    /// [`AgentDebugMessage::CopyJsonRequested`] and
    /// [`AgentDebugMessage::ScrollReported`] change nothing here: the
    /// first is a deferred clipboard write, the second a runtime report
    /// the reducer keeps alongside the preference and feeds to
    /// [`crate::ui_iced::logs::should_follow`]
    /// ([`AgentDebugViewModel::follow_intent`]).
    pub fn apply(&mut self, message: AgentDebugMessage) -> bool {
        match message {
            AgentDebugMessage::EntryToggled { id } => {
                self.toggle_entry(id);
                true
            }
            AgentDebugMessage::CopyJsonRequested { .. }
            | AgentDebugMessage::ScrollReported { .. } => false,
        }
    }
}
