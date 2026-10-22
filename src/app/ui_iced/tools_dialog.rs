//! Tools dialog — the tool-group table with per-group enable, error-restart
//! and MCP OAuth controls (UI-051..UI-060).
//!
//! A **pure view over a plain snapshot**: [`tools_dialog_card`] renders a
//! [`ToolsDialogViewModel`] into an [`iced::Element`] of [`Message`] and
//! touches neither the bus, the disk nor `FastMdApp`. The reducer owns the
//! open/close state ([`ToolsDialogState`]) and executes the [`UserCommand`]s
//! the rows emit.
//!
//! # Data source — the tool-group registry, reused unchanged ([RUST-051])
//!
//! | Field | Source |
//! | --- | --- |
//! | Group rows | [`crate::agent::tools::registry::ToolRegistry::groups`] (`src/agent/tools/registry/mod.rs:268-341`) — internal families first, MCP servers sorted by name |
//! | Row shape | [`ToolGroupState`] / [`ToolGroupId`] / [`ToolGroupKind`] / [`ToolGroupError`] (`src/agent/tools/registry/groups.rs:9-90`), re-exported here, never re-declared |
//! | Prompt char count | [`ToolGroupState::prompt_char_count`] (`groups.rs:99-104`) — the subsystem's own summing helper, fed the registry's `tool_char_count` (`registry/mod.rs:255-263`) with the empty prompt context [`PROMPT_CONTEXT`] carries |
//! | OAuth eligibility | [`crate::agent::lib::mcp::McpClients::needs_authentication`] (`src/agent/lib/mcp/mod.rs:320-327`) — the canonical rule (SSE without a static `Authorization` header), called directly so no call site re-implements it |
//! | OAuth in flight | `Dialogs::is_oauth_in_progress(server)` (`src/app/ui/dialogs.rs:112-117`), snapshotted into [`McpAuthFacts`] by the reducer |
//!
//! # Table rendering
//!
//! The table is an `iced::widget::table`, and the six columns map one-for-one:
//!
//! | # | Column | iced widget | Width |
//! | --- | --- | --- | --- |
//! | 1 | enable checkbox | `checkbox` + `.on_toggle` (`iced_widget-0.14.2/src/checkbox.rs:119`, `:155`) | `Shrink` |
//! | 2 | group | strong `text` name (the error rides the `⚠` tooltip, not this cell) | `Fill` |
//! | 3 | kind | `text` from [`kind_label`] | `Shrink` |
//! | 4 | tools | comma-joined `text` | `Fill` |
//! | 5 | prompt char count | `text` of the summed count + the `✓ parallel` chip (see below) | `Shrink` |
//! | 6 | actions | `Row` of [`ToolAction`] buttons, led by the `⚠` tooltip marker | `Fill` |
//!
//! Verified `iced::widget::table` API: `table(columns, rows)`
//! `iced_widget-0.14.2/src/table.rs:18-28`, `column(header, view)`
//! `:34-49`, `Column::width` `:650-653`, and the all-`Shrink` promotion
//! rule `:129-133` (three explicit `Fill`s here keep the wide columns
//! wide and the short ones tight).
//!
//! There is no row striping and no user-resizable column: iced 0.14's
//! `table` exposes only `separator_x`/`separator_y` in its style
//! (`table.rs:676-681`), and `Column` takes a single `Length` with no
//! minimum. Column proportions are therefore expressed as `Fill`/`Shrink`
//! ratios rather than pixel floors.
//!
//! # Error highlighting — which token, and why two
//!
//! * the `⚠` marker ([`strings::TOOLS_ERROR_ICON`]) uses
//!   [`Palette::warning`].
//! * the tooltip **body** — the `Kind: message` line the marker carries —
//!   uses [`Palette::error`], not `warning`. iced 0.14 ships a tooltip
//!   widget, [`iced::widget::tooltip::Tooltip`]
//!   (`iced_widget-0.14.2/src/tooltip.rs:94-109`, `Position` `:407-412`),
//!   so the message is revealed on hover rather than pinned inline in the
//!   group cell. Hovered text that describes a *failure* reads correctly in
//!   the stronger `error` token next to a warning-coloured marker. Both come
//!   from the shared palette; no numeric literal is introduced.
//!
//! # The parallel-safe chip ([UI-058])
//!
//! A group whose `parallel_safe` is `true` gets a `✓ parallel` chip
//! ([`strings::TOOLS_PARALLEL_CHIP`]) after its char count, painted with
//! [`Palette::success`] inside a styled `container`. The chip is
//! informational: a row without one is **not** marked unsafe, so nothing is
//! rendered in its place and no "not parallel" chip exists.
//! [`parallel_chip_label`] is the pure rule the renderer and the tests
//! share.
//!
//! # Message channel
//!
//! Every row action rides [`Message::Command`]:
//! `UserCommand::SetToolGroupEnabled` / `ClearToolGroupError` /
//! `StartMcpAuth` / `ForgetMcpAuth`. Each carries its own [`ToolGroupId`]
//! or server name, so the intent is self-contained and replayable from the
//! command alone. The dialog keeps **no view-local state** — no search box,
//! no selection, no expansion — so nothing else needs a channel. The single
//! exception is dismissal: an in-window modal has no window-frame `X`, so
//! the scrim click / Escape emits
//! [`ToolsDialogMessage::CloseRequested`].
//!
//! Unit tests live in the sibling `tools_dialog_tests.rs` sidecar.

#[cfg(test)]
#[path = "tools_dialog_tests.rs"]
mod tests;

use std::collections::HashMap;

use iced::widget::{
    Column, Row, button, checkbox, container, rule, scrollable, table, text, tooltip,
};
use iced::{Alignment, Element, Length, Size};

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{Message, ToolsDialogMessage};

/// Tool-group identity, kind and state — reused unchanged from the tool
/// registry ([RUST-051]); the same types the `UserCommand`s carry.
pub use crate::agent::tools::registry::{
    InternalToolGroup, ToolErrorKind, ToolGroupError, ToolGroupId, ToolGroupKind, ToolGroupState,
};

/// Default width of the tools card.
pub const DEFAULT_WIDTH: f32 = 720.0;

/// Minimum width of the tools card.
pub const MIN_WIDTH: f32 = 520.0;

/// Height budget for one tool-group row.
pub const ROW_HEIGHT: f32 = 24.0;

/// Height of the table header row.
pub const HEADER_HEIGHT: f32 = 20.0;

/// Title bar + window border + interior padding above the table.
pub const CHROME_HEIGHT: f32 = 56.0;

/// Extra padding for the dialog scroll area.
pub const EXTRA_PADDING: f32 = 8.0;

/// Minimum height of the tools dialog.
pub const MIN_HEIGHT: f32 = 200.0;

/// Hard cap so a 4K monitor does not produce a 2000-px-tall dialog.
pub const MAX_HEIGHT_HARD_CAP: f32 = 1200.0;

/// Maximum height of the scrollable group list inside the card.
pub const LIST_MAX_HEIGHT: f32 = MAX_HEIGHT_HARD_CAP;

/// Logical point size of the window title.
pub const TOOLS_TITLE_SIZE: f32 = 18.0;

/// Logical point size of a table header cell.
pub const TOOLS_HEADER_SIZE: f32 = 14.0;

/// Logical point size of a table body cell.
pub const TOOLS_ROW_SIZE: f32 = 14.0;

/// Logical point size of the error tooltip body.
pub const TOOLS_ERROR_SIZE: f32 = 12.0;

/// Logical point size of the `✓ parallel` chip.
pub const TOOLS_CHIP_SIZE: f32 = 12.0;

/// Padding inside the `✓ parallel` chip.
pub const CHIP_PADDING: f32 = 2.0;

/// Corner radius of the `✓ parallel` chip.
pub const CHIP_RADIUS: f32 = 3.0;

/// Gap between a row's char count and its `✓ parallel` chip.
pub const CHIP_SPACING: f32 = 6.0;

/// Gap between the `⚠` marker and its hover tooltip.
pub const TOOLTIP_GAP: f32 = 4.0;

/// Horizontal padding inside a table cell.
pub const CELL_PADDING_X: f32 = 6.0;

/// Vertical padding inside a table cell.
pub const CELL_PADDING_Y: f32 = 2.0;

/// Padding inside the card.
pub const CARD_PADDING: f32 = 12.0;

/// Vertical gap between card sections.
pub const SECTION_SPACING: f32 = 8.0;

/// Width of the table and card separators.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// The prompt context the char-count column is measured against.
///
/// The column is measured against the empty string and is informational;
/// the value is carried here so the measurement stays named.
pub const PROMPT_CONTEXT: &str = "";

/// The OAuth facts one MCP row needs.
///
/// Both halves are read off live subsystems at snapshot time — the
/// config's transport/header shape and the dialog manager's in-flight
/// flag — so the pure row mapping never has to reach for either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct McpAuthFacts {
    /// The server config is eligible for the OAuth 2.1 flow (SSE with no
    /// static `Authorization` header) —
    /// [`crate::agent::lib::mcp::McpClients::needs_authentication`].
    pub eligible: bool,
    /// A background OAuth thread is currently running for it —
    /// `Dialogs::is_oauth_in_progress`.
    pub in_progress: bool,
}

/// What a group row can do about authentication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolAuthState {
    /// Not an MCP group — internal families never authenticate.
    #[default]
    NotApplicable,
    /// MCP group that has never been observed to require auth.
    NotRequired,
    /// Auth was observed as required, but the server config cannot run the
    /// OAuth flow (stdio, or SSE carrying a static `Authorization`
    /// header, or the server entry has gone away).
    NotEligible,
    /// Eligible and idle — the Authenticate button fires.
    Available,
    /// Eligible and already running — the button is shown but inert so a
    /// second click cannot double-spawn the flow.
    InProgress,
}

impl ToolAuthState {
    /// Whether the row shows the Authenticate / Forget controls.
    #[must_use]
    pub fn shows_controls(self) -> bool {
        matches!(self, Self::Available | Self::InProgress)
    }

    /// Whether the Authenticate control accepts a press.
    #[must_use]
    pub fn can_authenticate(self) -> bool {
        self == Self::Available
    }

    /// The Authenticate control's label for this state.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => strings::TOOLS_AUTH_BUTTON,
            Self::InProgress => strings::TOOLS_AUTH_RUNNING,
            Self::NotApplicable | Self::NotRequired | Self::NotEligible => "",
        }
    }
}

/// One rendered row of the tools table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolGroupRow {
    /// The group's stable identity, carried by every action.
    pub id: ToolGroupId,
    /// Display name (family name or MCP server name).
    pub display_name: String,
    /// Internal vs MCP stdio vs MCP remote.
    pub kind: ToolGroupKind,
    /// The kind column's label, from `crate::ui::strings`.
    pub kind_label: &'static str,
    /// Whether the group is currently enabled.
    pub enabled: bool,
    /// The auth affordance this row gets.
    pub auth_state: ToolAuthState,
    /// Comma-joined tool names (column 4).
    pub tools_label: String,
    /// Summed prompt char count (column 5).
    pub prompt_char_count: usize,
    /// Whether every tool in the group is `ReadOnly`, i.e. the group may
    /// run concurrently — carried through from
    /// [`ToolGroupState::parallel_safe`] and rendered as the `✓ parallel`
    /// chip beside the char count ([UI-058], TOOL-020).
    pub parallel_safe: bool,
    /// The rendered `Kind: message` error line, `None` when clean.
    pub error: Option<String>,
}

impl ToolGroupRow {
    /// Whether this row carries a recorded error.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// The MCP server name, `None` for internal groups.
    #[must_use]
    pub fn mcp_server(&self) -> Option<&str> {
        match &self.id {
            ToolGroupId::Mcp(name) => Some(name),
            ToolGroupId::Internal(_) => None,
        }
    }
}

/// One button in a row's Actions cell.
///
/// `press == None` renders an inert button (`Button::on_press_maybe`,
/// `iced_widget-0.14.2/src/button.rs:170-173`), which is how the
/// "Authenticating…" state stays allocated without being clickable.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolAction {
    /// The button label, from `crate::ui::strings`.
    pub label: &'static str,
    /// The message the button emits, `None` when inert.
    pub press: Option<Message>,
}

impl ToolAction {
    /// A pressable action.
    #[must_use]
    pub fn new(label: &'static str, message: Message) -> Self {
        Self {
            label,
            press: Some(message),
        }
    }

    /// An always-visible, never-pressable action.
    #[must_use]
    pub fn inert(label: &'static str) -> Self {
        Self { label, press: None }
    }

    /// Whether the action accepts a press.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.press.is_some()
    }
}

/// Maps a group's identity, its live `needs_auth` flag and the server's
/// OAuth facts into the row's auth state.
///
/// The conjunction: an MCP group **and** `needs_auth` **and** a
/// config-eligible server.
///
/// Purity: pure function of the three inputs.
#[must_use]
pub fn auth_state_for(group: &ToolGroupState, facts: McpAuthFacts) -> ToolAuthState {
    if !matches!(group.id, ToolGroupId::Mcp(_)) {
        return ToolAuthState::NotApplicable;
    }
    if !group.needs_auth {
        return ToolAuthState::NotRequired;
    }
    if !facts.eligible {
        return ToolAuthState::NotEligible;
    }
    if facts.in_progress {
        ToolAuthState::InProgress
    } else {
        ToolAuthState::Available
    }
}

/// The kind column's label for a [`ToolGroupKind`], from
/// `crate::ui::strings`.
///
/// Purity: pure.
#[must_use]
pub fn kind_label(kind: ToolGroupKind) -> &'static str {
    match kind {
        ToolGroupKind::Internal => strings::TOOLS_KIND_INTERNAL,
        ToolGroupKind::McpStdio => strings::TOOLS_KIND_MCP_STDIO,
        ToolGroupKind::McpRemote => strings::TOOLS_KIND_MCP_REMOTE,
    }
}

/// Renders one recorded group error as the `Kind: message` line shown in
/// the `⚠` marker's hover tooltip ([UI-057]).
///
/// Purity: pure.
#[must_use]
pub fn error_line_text(error: &ToolGroupError) -> String {
    strings::format_tool_group_error(&format!("{:?}", error.kind), &error.message)
}

/// The body a row's `⚠` hover tooltip shows — the `Kind: message` line —
/// and `None` when the row is clean and no marker is rendered at all
/// ([UI-057]).
///
/// Purity: pure function of the row.
#[must_use]
pub fn error_tooltip_body(row: &ToolGroupRow) -> Option<&str> {
    row.error.as_deref()
}

/// The chip label a row's Prompt column renders, `Some` only when the
/// group is flagged parallel-safe ([UI-058]).
///
/// `None` means "render no chip", **not** "render an unsafe marker": the
/// chip's absence carries no negative signal.
///
/// Purity: pure function of the row.
#[must_use]
pub fn parallel_chip_label(row: &ToolGroupRow) -> Option<&'static str> {
    row.parallel_safe.then_some(strings::TOOLS_PARALLEL_CHIP)
}

/// Maps one [`ToolGroupState`] to its row model.
///
/// `char_count_for_tool` is the registry's per-tool schema-size lookup,
/// fed to the subsystem's own [`ToolGroupState::prompt_char_count`]
/// helper rather than re-implementing the sum here ([RUST-051]).
///
/// Purity: pure over `group` and the supplied lookups.
#[must_use]
pub fn row_from_group(
    group: &ToolGroupState,
    facts: McpAuthFacts,
    char_count_for_tool: &dyn Fn(&str) -> Option<usize>,
) -> ToolGroupRow {
    ToolGroupRow {
        id: group.id.clone(),
        display_name: group.display_name.clone(),
        kind: group.kind,
        kind_label: kind_label(group.kind),
        enabled: group.enabled,
        auth_state: auth_state_for(group, facts),
        tools_label: group.tool_names.join(", "),
        prompt_char_count: group.prompt_char_count(char_count_for_tool),
        parallel_safe: group.parallel_safe,
        error: group.last_error.as_ref().map(error_line_text),
    }
}

/// Maps a snapshot of group states to their rows, in the order the
/// registry returned them (internal families first, MCP servers sorted by
/// name — `registry/mod.rs:333-339`).
///
/// Purity: pure over `groups` and the supplied lookups.
#[must_use]
pub fn rows_from_groups(
    groups: &[ToolGroupState],
    facts: &HashMap<String, McpAuthFacts>,
    char_count_for_tool: &dyn Fn(&str) -> Option<usize>,
) -> Vec<ToolGroupRow> {
    groups
        .iter()
        .map(|group| {
            let group_facts = match &group.id {
                ToolGroupId::Mcp(name) => facts.get(name).copied().unwrap_or_default(),
                ToolGroupId::Internal(_) => McpAuthFacts::default(),
            };
            row_from_group(group, group_facts, char_count_for_tool)
        })
        .collect()
}

/// The actions a row's Actions cell renders, in render order: Restart when
/// an error is recorded, then Authenticate (or the inert in-progress
/// variant) plus Forget for an eligible MCP group. A clean internal row
/// gets nothing.
///
/// Purity: pure function of the row.
#[must_use]
pub fn row_actions(row: &ToolGroupRow) -> Vec<ToolAction> {
    let mut actions = Vec::new();

    if row.has_error() {
        actions.push(ToolAction::new(
            strings::TOOLS_RESTART,
            clear_error_command(&row.id),
        ));
    }

    if row.auth_state.shows_controls() {
        let server = row.mcp_server().unwrap_or_default();
        let action = if row.auth_state.can_authenticate() {
            ToolAction::new(strings::TOOLS_AUTH_BUTTON, start_auth_command(server))
        } else {
            ToolAction::inert(strings::TOOLS_AUTH_RUNNING)
        };
        actions.push(action);
        actions.push(ToolAction::new(
            strings::TOOLS_FORGET,
            forget_auth_command(server),
        ));
    }

    actions
}

/// The `ClearToolGroupError` command behind a Restart action.
#[must_use]
pub fn clear_error_command(id: &ToolGroupId) -> Message {
    Message::Command(UserCommand::ClearToolGroupError(id.clone()))
}

/// The `StartMcpAuth` command behind an Authenticate action.
#[must_use]
pub fn start_auth_command(server: &str) -> Message {
    Message::Command(UserCommand::StartMcpAuth(server.to_owned()))
}

/// The `ForgetMcpAuth` command behind a Forget action.
#[must_use]
pub fn forget_auth_command(server: &str) -> Message {
    Message::Command(UserCommand::ForgetMcpAuth(server.to_owned()))
}

/// The `SetToolGroupEnabled` command behind a row's enable checkbox.
#[must_use]
pub fn set_enabled_command(id: &ToolGroupId, enabled: bool) -> Message {
    Message::Command(UserCommand::SetToolGroupEnabled {
        id: id.clone(),
        enabled,
    })
}

/// The dismiss message the scrim click / Escape emits.
#[must_use]
pub fn dismiss_message() -> Message {
    Message::Tools(ToolsDialogMessage::CloseRequested)
}

/// The token the `⚠` error marker is painted with — `Palette::warning`.
#[must_use]
pub fn error_marker_color() -> iced::Color {
    Palette::fastmd_dark().warning
}

/// The token the error tooltip body is painted with — `Palette::error`.
/// See the module header for why this differs from the marker.
#[must_use]
pub fn error_text_color() -> iced::Color {
    Palette::fastmd_dark().error
}

/// The token the `✓ parallel` chip is painted with — `Palette::success`,
/// the palette's completion-role colour. Resolved through the shared
/// palette so no numeric literal reaches the panel.
#[must_use]
pub fn parallel_chip_color() -> iced::Color {
    Palette::fastmd_dark().success
}

/// The token the `✓ parallel` chip's surface uses.
#[must_use]
pub fn parallel_chip_surface_color() -> iced::Color {
    Palette::fastmd_dark().surface_variant
}

/// The token a disabled / informational cell uses.
#[must_use]
pub fn muted_color() -> iced::Color {
    Palette::fastmd_dark().text_muted
}

/// The token a row's primary label uses.
#[must_use]
pub fn row_label_color() -> iced::Color {
    Palette::fastmd_dark().text_strong
}

/// The tools dialog's height budget for `row_count` rows in a `viewport`.
///
/// Preferred height is
/// `CHROME + HEADER + row_count * ROW_HEIGHT + EXTRA_PADDING`, clamped
/// into `[MIN_HEIGHT, min(viewport.height * 0.85, MAX_HEIGHT_HARD_CAP)]`
/// so the dialog grows to fit every row when the screen has room and
/// scrolls otherwise.
///
/// Returns `(default_size, min_size, max_height)`.
///
/// Purity: pure.
#[must_use]
pub fn compute_dialog_size(viewport: Size, row_count: usize) -> (Size, Size, f32) {
    let preferred_height =
        CHROME_HEIGHT + HEADER_HEIGHT + (row_count as f32) * ROW_HEIGHT + EXTRA_PADDING;
    let screen_max = (viewport.height * 0.85).max(MIN_HEIGHT);
    let max_height = screen_max.min(MAX_HEIGHT_HARD_CAP);
    let default_height = preferred_height.min(max_height).max(MIN_HEIGHT);
    let min_height = MIN_HEIGHT.min(default_height);

    (
        Size::new(DEFAULT_WIDTH, default_height),
        Size::new(MIN_WIDTH, min_height),
        max_height,
    )
}

/// Everything the tools card renders: the mapped group rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolsDialogViewModel {
    rows: Vec<ToolGroupRow>,
}

impl ToolsDialogViewModel {
    /// A snapshot with no groups registered.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a view model over already-mapped rows — the seam tests and
    /// any alternative source use.
    #[must_use]
    pub fn with_rows(rows: Vec<ToolGroupRow>) -> Self {
        Self { rows }
    }

    /// Snapshots the tool-group registry.
    ///
    /// `char_count_for_tool` is the registry's
    /// `ToolRegistry::tool_char_count` bound to the running config and
    /// [`PROMPT_CONTEXT`].
    ///
    /// Purity: pure over `groups` and the supplied lookups.
    #[must_use]
    pub fn from_groups(
        groups: &[ToolGroupState],
        facts: &HashMap<String, McpAuthFacts>,
        char_count_for_tool: &dyn Fn(&str) -> Option<usize>,
    ) -> Self {
        Self {
            rows: rows_from_groups(groups, facts, char_count_for_tool),
        }
    }

    /// Snapshots the dialog's data off `app` at snapshot time so the view
    /// stays I/O-free.
    #[must_use]
    pub fn from_app(app: &crate::ui::app::FastMdApp) -> Self {
        use crate::agent::lib::mcp::McpClients;

        let config = app.config().to_agent_config();
        let bundle = app.orchestrator.tool_context.load();
        let registry = &bundle.registry;

        let facts: HashMap<String, McpAuthFacts> = config
            .mcp_servers()
            .iter()
            .map(|(name, entry)| {
                (
                    name.clone(),
                    McpAuthFacts {
                        eligible: McpClients::needs_authentication(entry.config()),
                        in_progress: app.dialogs().is_oauth_in_progress(name),
                    },
                )
            })
            .collect();

        let groups = registry.groups(&config);
        Self::from_groups(&groups, &facts, &|name| {
            registry.tool_char_count(name, &config, PROMPT_CONTEXT)
        })
    }

    /// The rows to render, in render order.
    #[must_use]
    pub fn rows(&self) -> &[ToolGroupRow] {
        &self.rows
    }

    /// How many groups are listed.
    #[must_use]
    pub fn count(&self) -> usize {
        self.rows.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The summary line under the title: the count, or the empty-state
    /// line. Folding the empty state into the text rather than swapping in
    /// a different widget keeps the card's shape constant (AGENTS §6).
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            strings::TOOLS_EMPTY_STATE.to_string()
        } else {
            format!("{} {}", self.count(), strings::TOOLS_GROUP_COLUMN)
        }
    }

    /// The card's height budget for the current row count.
    #[must_use]
    pub fn size_for(&self, viewport: Size) -> (Size, Size, f32) {
        compute_dialog_size(viewport, self.count())
    }

    /// Renders this snapshot as the tools card ([`tools_dialog_card`]).
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        tools_dialog_card(self)
    }
}

/// Renders the tools dialog card.
///
/// ```text
/// Tools
/// 9 Group
/// ─────────────────────────────────────────────────────────────────
/// ☑ │ Filesystem        │ Internal    │ read, write │ 1204 [✓ parallel] │
/// ☑ │ my-server         │ MCP (remote)│ fetch       │  612 │ ⚠ →hover: "Execution: …"  [Restart] [Authenticate] [Forget]
/// ─────────────────────────────────────────────────────────────────
/// ```
///
/// The parallel-safe chip renders only on `parallel_safe` rows ([UI-058]);
/// the error text renders only inside the marker's tooltip ([UI-057]).
///
/// The card is the *content* only: the root view mounts it through
/// [`crate::ui_iced::overlay::modal`] with [`dismiss_message()`] as the
/// dismiss message so a scrim click or Escape closes it.
///
/// Emitted messages:
/// * enable checkbox → [`UserCommand::SetToolGroupEnabled`]
/// * Restart → [`UserCommand::ClearToolGroupError`]
/// * Authenticate → [`UserCommand::StartMcpAuth`] (inert while running)
/// * Forget → [`UserCommand::ForgetMcpAuth`]
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn tools_dialog_card(view_model: &ToolsDialogViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::TOOLS_DIALOG_TITLE)
        .size(TOOLS_TITLE_SIZE)
        .color(palette.text_strong);

    let summary = text(view_model.summary())
        .size(TOOLS_ROW_SIZE)
        .color(palette.text_muted);

    let list = scrollable(tools_table(view_model.rows()))
        .height(Length::Fixed(LIST_MAX_HEIGHT))
        .width(Length::Fill);

    Column::with_children(vec![
        title.into(),
        summary.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        list.into(),
    ])
    .spacing(SECTION_SPACING)
    .padding(CARD_PADDING)
    .width(Length::Fixed(DEFAULT_WIDTH))
    .into()
}

/// The six-column group table — see the module header for the column
/// mapping and the verified iced citations. With zero rows the header still
/// renders, so the empty state needs no separate widget.
///
/// Purity: pure over `rows`.
#[must_use]
pub fn tools_table(rows: &[ToolGroupRow]) -> Element<'static, Message> {
    let columns = vec![
        table::column(
            header_cell(""),
            move |row: ToolGroupRow| -> Element<'static, Message> {
                checkbox(row.enabled)
                    .on_toggle(move |enabled| set_enabled_command(&row.id, enabled))
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::TOOLS_GROUP_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> { group_cell(&row) },
        )
        .width(Length::Fill),
        table::column(
            header_cell(strings::TOOLS_KIND_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> {
                text(row.kind_label)
                    .size(TOOLS_ROW_SIZE)
                    .color(Palette::fastmd_dark().text)
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::TOOLS_LIST_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> {
                text(row.tools_label)
                    .size(TOOLS_ROW_SIZE)
                    .color(Palette::fastmd_dark().text)
                    .into()
            },
        )
        .width(Length::Fill),
        table::column(
            header_cell(strings::TOOLS_CHAR_COUNT_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> { char_count_cell(&row) },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::TOOLS_ACTIONS_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> { actions_cell(&row) },
        )
        .width(Length::Fill),
    ];

    table::table(columns, rows.to_vec())
        .padding_x(CELL_PADDING_X)
        .padding_y(CELL_PADDING_Y)
        .separator_x(SEPARATOR_WIDTH)
        .separator_y(SEPARATOR_WIDTH)
        .width(Length::Fill)
        .into()
}

/// A column-header cell.
fn header_cell(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(TOOLS_HEADER_SIZE)
        .color(row_label_color())
        .into()
}

/// The Prompt column's cell: the summed char count, followed by the
/// `✓ parallel` chip when — and only when — the group is parallel-safe
/// ([UI-058]). The chip's absence is not a negative signal, so a
/// non-parallel row renders the count alone.
fn char_count_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    let count: Element<'static, Message> = text(row.prompt_char_count.to_string())
        .size(TOOLS_ROW_SIZE)
        .color(Palette::fastmd_dark().text_value)
        .into();

    match parallel_chip_label(row) {
        Some(label) => Row::with_children(vec![count, parallel_chip(label)])
            .spacing(CHIP_SPACING)
            .align_y(Alignment::Center)
            .into(),
        None => count,
    }
}

/// The `✓ parallel` chip — a styled `container` around the label, painted
/// with [`Palette::success`] on a [`Palette::surface_variant`] fill.
/// Every colour comes from the shared palette.
fn parallel_chip(label: &'static str) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    container(
        text(label)
            .size(TOOLS_CHIP_SIZE)
            .color(parallel_chip_color()),
    )
    .padding(CHIP_PADDING)
    .style(move |_theme| container::Style {
        background: Some(parallel_chip_surface_color().into()),
        border: iced::Border {
            color: palette.border,
            width: SEPARATOR_WIDTH,
            radius: CHIP_RADIUS.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// The group-name cell: the strong display name on a single line. The
/// row's error is deliberately **not** rendered here — [UI-057] carries it
/// in the `⚠` marker's hover tooltip (see [`actions_cell`]), so the cell
/// keeps the same shape whether or not an error was recorded.
fn group_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    text(row.display_name.clone())
        .size(TOOLS_ROW_SIZE)
        .color(row_label_color())
        .into()
}

/// The Actions cell: the `⚠` marker wrapped in an
/// [`iced::widget::tooltip::Tooltip`] whose body is the row's
/// `Kind: message` line ([UI-057]), plus the row's mapped action buttons.
/// A clean row keeps the marker slot empty and mounts no tooltip.
fn actions_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    let marker = match error_tooltip_body(row) {
        Some(body) => tooltip::Tooltip::new(
            text(strings::TOOLS_ERROR_ICON)
                .size(TOOLS_ROW_SIZE)
                .color(error_marker_color()),
            text(body.to_owned())
                .size(TOOLS_ERROR_SIZE)
                .style(|_theme| iced::widget::text::Style {
                    color: Some(error_text_color()),
                }),
            tooltip::Position::Bottom,
        )
        .gap(TOOLTIP_GAP)
        .into(),
        None => text("").size(TOOLS_ROW_SIZE).into(),
    };

    let buttons = row_actions(row).into_iter().fold(
        Row::<Message>::new().spacing(SECTION_SPACING),
        |buttons, action| {
            buttons.push(button(text(action.label)).on_press_maybe(action.press.clone()))
        },
    );

    Row::with_children(vec![marker, buttons.into()])
        .spacing(SECTION_SPACING)
        .align_y(Alignment::Center)
        .into()
}

/// Which tools dialog is open — the pure state the reducer holds.
///
/// One window at a time, `open` replaces whatever was showing, and `close`
/// reports whether anything was consumed so an Escape handler can tell a
/// consumed key from a no-op. Opening is user intent and rides
/// [`UserCommand::OpenToolsDialog`]; only the dismiss needs a message of
/// its own (see the module header).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolsDialogState {
    open: bool,
}

impl ToolsDialogState {
    /// A state with the dialog closed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the dialog is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shows the dialog. Returns whether the visible state changed.
    pub fn open(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.open = true;
        true
    }

    /// Hides the dialog. Returns whether one was open — the same
    /// "was anything consumed" contract as
    /// [`crate::ui_iced::overlay::OverlaySlot::close`].
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

    /// Applies one [`ToolsDialogMessage`] to the window state and reports
    /// whether the visible state changed.
    pub fn apply(&mut self, message: ToolsDialogMessage) -> bool {
        match message {
            ToolsDialogMessage::CloseRequested => self.close(),
        }
    }
}
