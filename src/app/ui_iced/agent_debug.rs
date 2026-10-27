//! Agent debug window (UI-048).
//!
//! A **pure view over a plain snapshot**: [`agent_debug_card`] renders an
//! [`AgentDebugViewModel`] into an [`iced::Element`] of [`Message`] and
//! touches neither the bus, the disk nor the agent. The reducer owns the
//! window's own state ([`AgentDebugWindowState`]) and executes the
//! [`UserCommand`]s and deferred intents the card emits.
//!
//! # Debug-entry data source ([RUST-051])
//!
//! | Field | Source |
//! | --- | --- |
//! | Entries | `AgentSession::state().debug_entries` (`src/agent/session.rs:33`, appended by `push_debug_entry` at `:215`) |
//! | Types | `AgentDebugEntry` / `DebugEntryKind` / `DebugEntryRow` (`src/agent/events.rs:23-62`), reached through the existing bus re-export `crate::bus::events::debug` (`src/app/bus/events/debug.rs:3`); re-exported here, never re-declared |
//! | Search / auto-scroll / json-rows / open flag | `AgentPanelState` (`src/app/ui/agent/panel_state.rs:13-31`) — the UI-owned panel state, read here through the same fields |
//! | External-data markers | `crate::agent::datamark::{EXTERNAL_DATA_START, EXTERNAL_DATA_END}` (`src/agent/datamark.rs:27`, `:32`) — the envelope the agent itself writes, not re-typed literals ([RUST-021]) |
//!
//! **Live streaming needs no new plumbing.** The agent pushes entries onto
//! that same `Vec` while a turn runs; the subscription bridge turns each
//! inbound `AgentEvent::DebugEntry` into a redraw and the next snapshot
//! picks the new rows up. The view never pulls from a live stream at
//! render time and never writes to the buffer.
//!
//! # Disclosure — an explicit expanded-id set
//!
//! iced 0.14 has no `CollapsingHeader` widget, so disclosure takes the
//! shape of a disclose-arrow row:
//!
//! ```text
//! mouse_area(Row[ caret glyph, "HH:MM:SS.fff  [Kind]  summary" ])
//!     .on_press(Message::AgentDebug(AgentDebugMessage::EntryToggled { id }))
//! ```
//!
//! Which rows are open is a `HashSet<usize>` of row ids held by
//! [`AgentDebugWindowState::expanded_ids`] and copied into the snapshot
//! the card renders, so the disclosure state is plain data: inspectable,
//! replayable and unit-testable without a runtime.
//!
//! A row id is the entry's index in the **raw** debug-entry list — the
//! value carried by [`DebugEntryView::id`]. Two consequences the reducer
//! must honour ([RUST-041] drift note):
//!
//! * ids are **positional**, so `UserCommand::ClearDebugEntries` must be
//!   followed by [`AgentDebugWindowState::collapse_all`] — otherwise a
//!   stale id silently re-expands an unrelated new entry;
//! * nothing is expanded until the user clicks: [`AgentDebugWindowState::new`]
//!   starts from the empty set.
//!
//! # Auto-scroll — the log viewer's intent model
//!
//! iced 0.14 has no `stick_to_bottom` flag, so this window uses the
//! **same** rule [`crate::ui_iced::logs`] established for the log viewer
//! rather than a second copy of it:
//!
//! ```text
//! Scrollable::on_scroll ─▶ Message::AgentDebug(ScrollReported { .. })  (view emits)
//!                                   │
//!              logs::should_follow(auto_scroll, Some(snapshot))        (pure decision)
//!                                   │
//!   operation::scrollable::scroll_to(DEBUG_SCROLLABLE_ID, AbsoluteOffset { y: snapshot.max_offset() })
//!                                                    (reducer Task)
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
//! The setting is a **viewport size, not a truncation**. An expanded body
//! is capped at `json_rows` lines: it shows `min(lines, json_rows)` lines
//! and scrolls to reach the rest — both vertically and horizontally,
//! because the body is drawn with `Wrapping::None`.
//! [`json_body_height`] computes the cap and
//! [`visible_json_lines`] the count. The catalogue is
//! [`JSON_ROW_CATALOG`] (default `8` from `AgentPanelState::default`);
//! [`json_row_options`] additionally keeps an off-catalogue value
//! visible, which iced's `pick_list` cannot do on its own.
//!
//! # Message-channel choices
//!
//! * **Search, auto-scroll, json-rows, clear and close all stay on
//!   [`Message::Command`]** — `UserCommand::SetDebugSearchText` /
//!   `SetDebugAutoScroll` / `SetDebugJsonRows` / `ClearDebugEntries` /
//!   `ToggleDebugWindow(bool)` (`src/app/bus/events/user_command.rs:152-160`)
//!   are the commands that carry each intent, and each is self-contained.
//!   Nothing is duplicated.
//! * **Disclosure needs a new variant.** An entry's open/closed state has
//!   no [`UserCommand`] equivalent — it is view-local, not application
//!   state — so it arrives as [`AgentDebugMessage::EntryToggled`].
//! * **Copy-JSON is deferred intent.** iced exposes the clipboard to
//!   `update` only (`iced_core-0.14.0/src/clipboard.rs`), so the exact
//!   payload rides [`AgentDebugMessage::CopyJsonRequested`] — the same
//!   shape `RenderMessage::CopyCodeRequested` and
//!   `AboutMessage::CopyCommitHashRequested` use.
//! * **`ScrollReported` is a runtime report**, not intent — the same
//!   class of payload `LogsMessage::ScrollReported` and
//!   `LayoutMessage::CursorMoved` carry.
//! * **Dismissal is a command, not a new variant.** Closing publishes
//!   `UserCommand::ToggleDebugWindow(false)`, so the scrim click / Escape
//!   emits [`dismiss_command`] rather than inventing an
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

use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;
use hoarfrost::widget::text_input::{InputKind, TextInput};
use iced::widget::{
    Column, Row, Space, checkbox, container, mouse_area, pick_list, rule, scrollable, text,
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
use hoarfrost::token::typography::{self, TypeRung};
use hoarfrost::token::{shape, space};

/// Width of the debug card, in logical pixels.
///
/// App-owned: §4.7.1 sets the movable-overlay default at `680 x 440`, and
/// this card is deliberately narrower because its rows are single-line
/// headers rather than a wide JSON pane. See the enumerated entry in
/// [`APP_ALLOW_LIST`](crate::ui_iced::token_lint_gate::APP_ALLOW_LIST).
pub const CARD_WIDTH: f32 = 600.0;

/// The rung an entry's header row paints at — `type.body` (`UIF-TOK-004`).
pub const ENTRY_RUNG: TypeRung = typography::BODY;

/// Logical point size of an entry's header row.
pub const ENTRY_TEXT_SIZE: f32 = ENTRY_RUNG.size;

/// The rung a session-boundary label paints at — `type.caption`.
pub const BOUNDARY_RUNG: TypeRung = typography::CAPTION;

/// Logical point size of a session-boundary label.
pub const BOUNDARY_TEXT_SIZE: f32 = BOUNDARY_RUNG.size;

/// The rung the card title paints at — `type.title_sm` (`UIF-TOK-004`).
///
/// **Movement: 18 → 16 (−2).** `18` is not a rung: `TITLE_SM` is 16 and
/// `TITLE_MD` is 20, so nothing on the scale produced that number. §2.4
/// assigns `type.title_sm` to panel and overlay headers, which is what
/// this is. INT-7c fixed the identical off-scale `18` on
/// [`AGENT_HEADER_SIZE`](crate::ui_iced::panels::center::AGENT_HEADER_SIZE)
/// and on the agent panel's own header.
pub const TITLE_RUNG: TypeRung = typography::TITLE_SM;

/// Logical point size of the card title.
pub const TITLE_SIZE: f32 = TITLE_RUNG.size;

/// The rung the expanded JSON body paints at — `type.body`, in a mono face.
pub const JSON_RUNG: TypeRung = typography::BODY;

/// Logical point size of the expanded JSON body, rendered in a monospace face.
pub const JSON_TEXT_SIZE: f32 = JSON_RUNG.size;

/// Height of one JSON line in logical points — the `type.body` line box.
///
/// **Movement: 18.2 → 20.0 (+1.8).** This used to be
/// `JSON_TEXT_SIZE × 1.3`, i.e. iced's default `Relative(1.3)`
/// (`iced_core-0.14.0/src/text.rs:215-218`) re-derived by hand. Two
/// things are wrong with that: the multiplier is a bare literal with no
/// owner, and a `f32` handed to iced's `LineHeight` reads as a
/// *multiplier*, not pixels — the exact defect class `bb18c2d` exists to
/// prevent.
///
/// The JSON body paints at `type.body`, so a line occupies the `type.body`
/// line box, which is what makes `json_rows` mean "N visible lines". The
/// row-count math in [`json_body_height`] is expressed in whole lines, so
/// it stays correct at the rung's own height.
pub const JSON_LINE_HEIGHT: f32 = JSON_RUNG.line_height;

/// The `json_rows` catalogue the pick list offers: 8, 16, 24, 32 and 64.
pub const JSON_ROW_CATALOG: [usize; 5] = [8, 16, 24, 32, 64];

/// `chrono` format of the row timestamp: hour, minute, second and
/// millisecond.
pub const TIMESTAMP_FORMAT: &str = "%H:%M:%S%.3f";

/// The top-level JSON key whose duplicate occurrences are elided.
pub const TOOLS_KEY: &str = "tools";

/// The `provenance=` header prefix the external-data envelope starts with.
///
/// `crate::agent::datamark` builds this inline
/// (`format!("provenance=tool:{}", name)`, `src/agent/datamark.rs:72`)
/// and exports no constant, so the prefix is named here rather than
/// repeated as a literal ([RUST-021]).
pub const PROVENANCE_PREFIX: &str = "provenance=";

/// What replaces a `tools` array an earlier entry in the same session
/// already showed.
pub const OMITTED_TOOLS_PLACEHOLDER: &str = strings::DEBUG_OMITTED_TOOLS_PLACEHOLDER;

/// Stable iced widget id of the entry list's `scrollable`.
///
/// The reducer must aim
/// `operation::scrollable::scroll_to` at a stable id
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`); a widget
/// without one cannot be scrolled programmatically at all. Named once so
/// view and reducer cannot drift ([RUST-021]).
pub const DEBUG_SCROLLABLE_ID: &str = "agent_debug_scroll";

/// Stable focus id for the agent-debug search input (UI-048).
pub const AGENT_DEBUG_SEARCH_INPUT_FOCUS_ID: &str = "agent_debug_search_input";

/// Stable focus id for the Clear button in the agent debug controls.
pub const AGENT_DEBUG_CLEAR_BUTTON_ID: &str = "agent_debug_clear_button";

/// Stable focus id for the Copy JSON button in expanded debug entries.
pub const AGENT_DEBUG_COPY_JSON_BUTTON_ID: &str = "agent_debug_copy_json_button";

/// Padding inside the card — `space.md` (`UIF-TOK-001`).
pub const CARD_PADDING: f32 = space::MD;

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

/// Vertical padding inside a clickable header row, i.e. the height of its
/// hit area — `space.xs` (`UIF-TOK-001`).
pub const HEADER_PADDING_Y: f32 = space::XS;

/// Width of the rules separating card sections and boundary rows —
/// `shape.border.hairline` (`UIF-TOK-003`).
pub const SEPARATOR_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The search text in the form the filter compares against: lower-cased once.
///
/// Purity: pure.
#[must_use]
pub fn normalized_search(search: &str) -> String {
    search.to_lowercase()
}

/// Whether one debug entry passes the window's search filter.
///
/// A row passes when any of the following holds:
///
/// * a [`DebugEntryRow::SessionBoundary`] **always** passes, so the
///   filtered list keeps its session structure instead of collapsing into
///   an undifferentiated run of rows;
/// * an empty search passes everything;
/// * otherwise the row passes when its `summary` contains the needle —
///   i.e. the match is **case-insensitive** and **summary-only**. The
///   JSON body is not searched.
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

/// The filtered `(original index, entry)` pairs, in list order.
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
/// The JSON branch returns before the envelope branch is tried, so a
/// string that parses as JSON is never mistaken for an envelope.
///
/// The only effect is on `value`.
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

/// Whether `trimmed` looks like a JSON object or array.
///
/// Purity: pure.
#[must_use]
fn is_json_container(trimmed: &str) -> bool {
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

/// The JSON an expanded row shows.
///
/// The display pipeline: replace a duplicated `tools` array, un-escape the
/// double-encoded payloads, then `serde_json::to_string_pretty`. A
/// serialization failure renders as the empty string, so a malformed
/// payload degrades to a blank body rather than taking the window down.
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
/// `Wrapping::None`, so one source line is exactly one rendered line.
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
/// Boundaries: `json_rows == 0` shows nothing (the pick list never offers
/// 0, but the command channel can carry it); `json_rows >= total_lines`
/// shows everything and the body does not scroll.
///
/// Purity: pure.
#[must_use]
pub fn visible_json_lines(total_lines: usize, json_rows: usize) -> usize {
    total_lines.min(json_rows)
}

/// Height, in logical points, of an expanded JSON body.
///
/// `visible_json_lines(total_lines, json_rows) × JSON_LINE_HEIGHT`. A short
/// body takes its own height; a long one stops at the setting and scrolls.
///
/// Purity: pure.
#[must_use]
pub fn json_body_height(total_lines: usize, json_rows: usize) -> f32 {
    visible_json_lines(total_lines, json_rows) as f32 * JSON_LINE_HEIGHT
}

/// One entry of the "JSON rows" pick list.
///
/// Carries the number rather than its label so a selection can never be
/// mistaken for the value it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonRowsOption {
    /// How many JSON lines an expanded body shows when this is picked.
    pub rows: usize,
}

impl JsonRowsOption {
    /// The intent this option publishes when picked:
    /// `UserCommand::SetDebugJsonRows`, which writes the value into
    /// `AgentPanelState::debug_json_rows`.
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
/// [`JSON_ROW_CATALOG`] plus the current value inserted in ascending order
/// when it is not already one of them. `UserCommand::SetDebugJsonRows`
/// accepts any `usize`, and iced's `pick_list` can only display a value it
/// can find among its options — without this, an off-catalogue setting
/// would render as the empty placeholder and hide what the window is
/// actually doing.
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

/// The kind column's label, taken from `crate::ui::strings`.
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
/// Separators included.
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

/// The dismiss message a scrim click / Escape emits:
/// `UserCommand::ToggleDebugWindow(false)`, so no `AgentDebugMessage`
/// variant is needed for closing.
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

/// One rendered row of the debug list.
///
/// The bus entry travels unchanged; only the display JSON is
/// pre-computed, so the view performs no serialization, no un-escaping
/// and no dedup pass at render time.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugEntryView {
    /// Stable row id: the entry's index in the raw debug-entry list it
    /// was snapshotted from.
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
    /// filter hid.
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
    /// Runs the whole pipeline once, at snapshot time: filter → session
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

    /// The snapshot for a window nobody can see.
    ///
    /// The filter settings and counters still ride along so the pick-lists stay
    /// coherent, but the per-entry deep clone plus `serde_json::to_string_pretty`
    /// that [`Self::from_entries`] performs is skipped. `debug_entries` is an
    /// uncapped buffer, so building rows for a closed window is the single
    /// largest avoidable cost on the UI thread during a workspace scan.
    ///
    /// Purity: pure over its arguments.
    #[must_use]
    pub fn closed(
        total_entries: usize,
        search_text: &str,
        auto_scroll: bool,
        json_rows: usize,
    ) -> Self {
        Self {
            visible: false,
            rows: Vec::new(),
            total_entries,
            search_text: search_text.to_string(),
            json_rows,
            json_row_options: json_row_options(json_rows),
            auto_scroll,
            expanded: HashSet::new(),
        }
    }

    /// Snapshots the window's data off `app`: the debug-entry buffer from
    /// `app.orchestrator.agent.state().debug_entries` plus the four
    /// `agent_panel_state` fields, read outside render so the card stays
    /// I/O-free.
    ///
    /// While the window is closed the row pipeline is skipped entirely — see
    /// [`Self::closed`].
    #[must_use]
    pub fn from_app(app: &FastMdApp) -> Self {
        let panel = &app.orchestrator.agent_panel_state;
        let entries = &app.orchestrator.agent.state().debug_entries;
        if !panel.show_debug_window {
            return Self::closed(
                entries.len(),
                &panel.debug_search_text,
                panel.debug_auto_scroll,
                panel.debug_json_rows,
            );
        }
        Self::from_entries(
            entries,
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
    /// Delegates to [`crate::ui_iced::logs::should_follow`], reused rather
    /// than copied, so both stick-to-bottom windows follow identically.
    /// Executing it is reducer work against [`DEBUG_SCROLLABLE_ID`].
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

/// Renders the agent debug card.
///
/// Layout:
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
/// dismiss message so a scrim click or Escape closes the window.
///
/// The control row and the list are **always allocated** — an empty
/// buffer renders an empty list rather than a different widget tree
/// (AGENTS §6). An expanded body *is* added to its own row on expand:
/// iced rebuilds the tree structurally every frame, so a disclosure
/// branch does not destabilise sibling identity.
///
/// The debug card's filter / control bar contents (§4.7.1 band 2).
///
/// Extracted from [`agent_debug_card`] so the movable Band-1 shell
/// ([`movable_shell`](crate::ui_iced::movable_shell::movable_shell)) can
/// host the same controls in its own filter band without a second copy of
/// the widget list drifting away from this one ([RUST-021]).
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn agent_debug_controls(view_model: &AgentDebugViewModel) -> Row<'_, Message> {
    let palette = Palette::fastmd_dark();

    let search_label = text(strings::SEARCH_LABEL)
        .size(ENTRY_TEXT_SIZE)
        .color(palette.text);

    let search_field = TextInput::new(
        InputKind::TextEntry,
        "",
        &view_model.search_text,
        search_command,
        AGENT_DEBUG_SEARCH_INPUT_FOCUS_ID,
    )
    .compact()
    .width(Length::Fill)
    .palette(Palette::pro());

    let auto_scroll_toggle = checkbox(view_model.auto_scroll)
        .label(strings::AUTO_SCROLL_CHECKBOX)
        .on_toggle(auto_scroll_command);

    let rows_label = text(strings::DEBUG_JSON_ROWS_LABEL)
        .size(ENTRY_TEXT_SIZE)
        .color(palette.text);

    let json_rows_pick: Element<'_, Message> = pick_list(
        view_model.json_row_options.clone(),
        view_model.json_rows_option(),
        JsonRowsOption::command,
    )
    .width(Length::Shrink)
    .into();

    let clear = Button::new(
        ButtonVariant::Secondary,
        text(strings::CLEAR_BUTTON)
            .size(ENTRY_TEXT_SIZE)
            .color(palette.text_strong),
        clear_command(),
        AGENT_DEBUG_CLEAR_BUTTON_ID,
    )
    .palette(Palette::pro());

    Row::with_children(vec![
        search_label.into(),
        search_field.into(),
        auto_scroll_toggle.into(),
        rows_label.into(),
        json_rows_pick,
        clear.into(),
    ])
    .spacing(CONTROL_SPACING)
    .align_y(Alignment::Center)
}

/// The debug card's scrollable entry list (§4.7.1 band 3).
///
/// Carries the stable [`DEBUG_SCROLLABLE_ID`] so the reducer can aim a
/// scroll operation at it whichever frame the card is mounted in
/// (AGENTS §5).
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn agent_debug_viewport(view_model: &AgentDebugViewModel) -> Element<'_, Message> {
    scrollable(entry_list(view_model))
        .id(DEBUG_SCROLLABLE_ID)
        .height(Length::Fill)
        .on_scroll(|viewport| scroll_reported_message(&logs::scroll_snapshot(&viewport)))
        .into()
}

/// The Agent Debug Inspector rendered in the §4.7.1 movable Band-1
/// silhouette (`UIF-MOV-001` … `UIF-MOV-004`, INT-6).
///
/// Same content as [`agent_debug_card`] — the controls come from
/// [`agent_debug_controls`] and the list from [`agent_debug_viewport`] —
/// but framed by the shared shell, which supplies the draggable title bar,
/// the sunken monospace viewport and the footer with its ◢ resize grip,
/// sized from the host's own geometry rather than this module's retired
/// `CARD_WIDTH` constant.
///
/// Purity: pure view over `host` and `view_model`.
#[must_use]
pub fn agent_debug_movable_card<'a>(
    host: &hoarfrost::surface::movable::MovableHost,
    view_model: &'a AgentDebugViewModel,
) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let status = text(strings::movable_row_count(view_model.rows.len()))
        .size(ENTRY_TEXT_SIZE)
        .color(palette.text_muted);

    crate::ui_iced::movable_shell::movable_shell(
        host,
        crate::ui_iced::message::MovableCard::AgentDebug,
        agent_debug_controls(view_model),
        agent_debug_viewport(view_model),
        status.into(),
    )
}

/// The debug card's filter / control bar contents (§4.7.1 band 2).
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn agent_debug_card(view_model: &AgentDebugViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::AGENT_DEBUG_WINDOW)
        .size(TITLE_SIZE)
        .color(palette.text_strong);

    let controls = agent_debug_controls(view_model);

    let list = agent_debug_viewport(view_model);

    Column::with_children(vec![
        title.into(),
        controls.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        list,
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
/// A [`DebugEntryRow::SessionBoundary`] renders a divider instead — rule,
/// centred muted summary, rule — and is deliberately **not** clickable,
/// because a divider has nothing to disclose.
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
        .color(Palette::fastmd_dark().text_muted)
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

/// The clickable header row that replaces `CollapsingHeader`'s
/// title: chevron + `HH:MM:SS.fff  [Kind]  summary`, wrapped in a
/// `MouseArea` whose press toggles this row's disclosure.
fn entry_header(row: &DebugEntryView, expanded: bool) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let caret = text(chevron_glyph(expanded))
        .font(phosphor_font())
        .size(ENTRY_TEXT_SIZE)
        .color(palette.text_muted);

    let label = text(header_text(&row.entry))
        .size(ENTRY_TEXT_SIZE)
        .color(palette.text);

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
/// single-line shape and the `scrollable` scrolls horizontally for it.
fn expanded_body(row: &DebugEntryView, json_rows: usize) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let copy = Button::new(
        ButtonVariant::Secondary,
        text(strings::DEBUG_COPY_JSON_BUTTON)
            .size(JSON_TEXT_SIZE)
            .color(palette.text_strong),
        copy_json_command(row.json.clone()),
        AGENT_DEBUG_COPY_JSON_BUTTON_ID,
    )
    .palette(Palette::pro());

    let actions = Row::with_children(vec![Space::new().width(Length::Fill).into(), copy.into()])
        .align_y(Alignment::Center);

    let body = text(row.json.clone())
        .font(Font::MONOSPACE)
        .size(JSON_TEXT_SIZE)
        .color(palette.text)
        .wrapping(iced::widget::text::Wrapping::None);

    let scroll =
        scrollable(body).height(Length::Fixed(json_body_height(row.json_lines(), json_rows)));

    Column::with_children(vec![actions.into(), scroll.into()])
        .spacing(BODY_SPACING)
        .into()
}

/// Which debug rows are open and whether the window is showing — the pure
/// state the reducer holds.
///
/// Mirrors [`crate::ui_iced::about::AboutWindowState`] for the open flag
/// and adds the expanded-row set. `close()` deliberately **keeps** the
/// expanded set: hiding the window leaves the disclosure states untouched,
/// and reopening shows what the user had open. Clearing the buffer is the
/// one transition that invalidates the ids, which is what
/// [`Self::collapse_all`] is for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentDebugWindowState {
    open: bool,
    expanded: HashSet<usize>,
}

impl AgentDebugWindowState {
    /// A closed window with nothing expanded.
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
    /// Rows toggle independently: opening one never touches another.
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
    /// No widget emits this — it exists for the reducer, which must call it
    /// whenever the debug buffer is cleared. Row ids are positional, so
    /// after `UserCommand::ClearDebugEntries` a stale id would silently
    /// expand an unrelated new entry.
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
