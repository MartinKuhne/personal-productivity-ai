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
//! exception is dismissal: the card carries its own `×` and footer
//! `Close`, and both ride
//! [`ToolsDialogMessage::CloseRequested`] — the same single close
//! transition the scrim click and Escape emit.
//!
//! Unit tests live in the sibling `tools_dialog_tests.rs` sidecar.

#[cfg(test)]
#[path = "tools_dialog_tests.rs"]
mod tests;

use std::collections::HashMap;

use crate::ui_iced::app::TooltipId;
use crate::ui_iced::message::{LayoutMessage, Message, ToolsDialogMessage};
use iced::font::Weight;
use iced::widget::{Column, Row, Space, container, mouse_area, scrollable, stack, table, text};
use iced::{Alignment, Background, Element, Length, Point, Size};

use hoarfrost::composite::list_tile;
use hoarfrost::surface::modal::{
    AxisPolicy, Cap, Desired, EDITOR_WIDTH_MIN, ErrorLocation, ModalError, ModalFrame, ModalSizing,
    Recovery, ResizeFloors, TOOLS_HEIGHT_MIN,
};
use hoarfrost::token::{shape, space, typography};
use hoarfrost::widget::badge;
use hoarfrost::widget::badge_model::{BadgeModel, BadgeTone, StatusIcon};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;
use hoarfrost::widget::toggle_model::CheckState;
use hoarfrost::widget::toggle_widget::Checkbox;

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::overlay::{
    modal_card, modal_centered, modal_divider, modal_resize_capture_layer, modal_resize_grip,
};

/// Tool-group identity, kind and state — reused unchanged from the tool
/// registry ([RUST-051]); the same types the `UserCommand`s carry.
pub use crate::agent::tools::registry::{
    InternalToolGroup, ToolErrorKind, ToolGroupError, ToolGroupId, ToolGroupKind, ToolGroupState,
};

/// The fraction of the viewport the Tools card fills on **both** axes
/// (defect #4 — "use 80% of the viewport").
///
/// Named rather than inlined ([RUST-021]): the one ratio drives the width
/// policy, the height policy, and every test that pins the measured card.
pub const VIEWPORT_FILL: f32 = 0.8;

/// The Tools card's sizing policy: [`VIEWPORT_FILL`] of the window on
/// **both** axes (defect #4 — "use 80% of the viewport").
///
/// Held as the policy object rather than a set of numbers, so the card
/// states what it wants and the framework does the clamping.
///
/// **This is an app-side override of §4.6.2, not a restatement of it.**
/// The framework's [`ModalSizing::tools_and_integrations`] still describes
/// the spec table verbatim — 720px wide, content-fit tall — and the
/// owner's decision on defect #4 replaces that with a viewport-driven box.
/// The override is assembled here from the framework's own
/// [`AxisPolicy`] parts rather than by editing the spec table in
/// `hoarfrost`, so the framework keeps saying what the spec says while
/// this card says what its owner asked for.
///
/// * **Width** — 80% of the window, no floor: the retired 720px figure
///   was the card's *target*, never a minimum, and a floor would let it
///   silently win at ordinary window sizes.
/// * **Height** — 80% of the window, floored at
///   [`TOOLS_HEIGHT_MIN`]. The floor still means something: below a
///   ~400px-tall window 80% is less than 320, and the floor holds the
///   card at the spec's minimum until the gutter takes over.
/// * **Both axes** — capped at the same ratio and then clamped by the
///   framework's `VIEWPORT_GUTTER`, which is applied last and always
///   wins, so 80% of a tiny viewport cannot overflow or clip.
pub const TOOLS_SIZING: ModalSizing = ModalSizing::new(
    AxisPolicy {
        desired: Desired::Viewport(VIEWPORT_FILL),
        min: 0.0,
        max: Cap::Viewport(VIEWPORT_FILL),
    },
    AxisPolicy {
        desired: Desired::Viewport(VIEWPORT_FILL),
        min: TOOLS_HEIGHT_MIN,
        max: Cap::Viewport(VIEWPORT_FILL),
    },
);

/// Stable id of the Tools card box itself, so a layout test can address
/// the rendered card rather than only its contents (AGENTS §5).
pub const TOOLS_CARD_ID: &str = "tools.card";

/// Stable id of the scrollable group list.
///
/// AGENTS §5: the list keeps its scroll position across rebuilds because
/// it is mounted under this id unconditionally — including when the row
/// set changes or an error banner appears above it.
pub const TOOLS_LIST_ID: &str = "tools.list";

/// Stable focus id of the error banner's `Retry` control.
pub const TOOLS_ERROR_RETRY_ID: &str = "tools.error.retry";

/// Stable focus id of the title band's `×` dismiss control.
pub const TOOLS_CLOSE_BUTTON_ID: &str = "tools_dialog_close";

/// Stable focus id of the footer's `Close` button.
pub const TOOLS_FOOTER_CLOSE_BUTTON_ID: &str = "tools_dialog_footer_close";

/// Stable id of the footer's resize grip (defect #6, owner ruling D-3).
pub const TOOLS_RESIZE_GRIP_ID: &str = "tools.resize.grip";

/// Stable id of the full-area layer that carries a resize while the grip is
/// held.
///
/// It is mounted only for as long as the button is down, so this id is never
/// live beside a second copy of itself (AGENTS §5).
pub const TOOLS_RESIZE_CAPTURE_ID: &str = "tools.resize.capture";

/// Key prefix under which a centred modal remembers a user-set size in
/// [`PersistedUiState::overlay_geometry`](crate::ui::persisted::PersistedUiState::overlay_geometry).
///
/// The map's keys name the *kind* of surface, the same way
/// [`movable::storage_key`](hoarfrost::surface::movable::storage_key) names a
/// movable one with `movable:`. A centred modal is not movable — it has no
/// position the user controls — so it does not borrow that prefix.
pub const MODAL_GEOMETRY_KEY_PREFIX: &str = "modal:";

/// The geometry key the dragged Tools card size is remembered under.
///
/// Derived from [`SURFACE_ID_TOOLS`](crate::ui_iced::app::SURFACE_ID_TOOLS)
/// rather than spelled out, so the key cannot drift from the surface it names.
#[must_use]
pub fn resize_storage_key() -> String {
    format!(
        "{MODAL_GEOMETRY_KEY_PREFIX}{}",
        crate::ui_iced::app::SURFACE_ID_TOOLS
    )
}

/// The bounds a **user-dragged** Tools card may not pass (defect #6, owner
/// ruling D-3).
///
/// * **Width floor** — [`EDITOR_WIDTH_MIN`], the framework's own §4.6.2 modal
///   width floor. It is used rather than a card-local number because the card
///   has no width minimum of its own to defend: the retired 720px figure was a
///   target, never a floor (see [`TOOLS_SIZING`]), and a drag needs *some*
///   floor so a flick of the grip cannot collapse the card to nothing.
///   `EDITOR_WIDTH_MIN` is the only modal width floor the framework offers, and
///   it is wider than the card's own content needs, so it is the conservative
///   choice.
/// * **Height floor** — [`TOOLS_HEIGHT_MIN`], unchanged from the initial-fill
///   policy: the same 320px that keeps the list legible.
/// * **Ceiling** — none of the card's own. [`TOOLS_SIZING`]'s
///   `Cap::Viewport(VIEWPORT_FILL)` is the *initial* fill and must not become
///   a maximum, or a card that was opened at 80% could never be dragged past
///   80%. The only ceiling is the framework's `VIEWPORT_GUTTER`, which
///   [`ResizeFloors`] applies last and which always wins.
pub const TOOLS_RESIZE_FLOORS: ResizeFloors = ResizeFloors::new(EDITOR_WIDTH_MIN, TOOLS_HEIGHT_MIN);

/// `NFR-004` error code for a failed MCP authentication surfaced in the
/// card's error banner (`UIF-MOD-003`).
///
/// `5001` is the client-side "upstream auth was refused" slot: the flow
/// ran and the server said no, which is a server-side fault reported
/// through the client rather than a bad request the app made.
pub const TOOLS_AUTH_ERROR_CODE: &str = "TOOLS-5001";

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
    /// The recorded error's **kind**, carried through from
    /// [`ToolGroupState::last_error`] so the dialog-level
    /// [`modal_error`](ToolsDialogViewModel::modal_error) can tell an
    /// authentication failure from an execution failure without parsing
    /// the rendered line back apart.
    pub error_kind: Option<ToolErrorKind>,
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
        error_kind: group.last_error.as_ref().map(|err| err.kind),
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

/// The message the corner grip emits when it is taken (defect #6).
///
/// Named rather than inlined so the grip, the reducer and the tests all speak
/// about the same transition ([RUST-021]).
#[must_use]
pub fn resize_press_message() -> Message {
    Message::Tools(ToolsDialogMessage::ResizeGripPressed)
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

/// The Tools card's **outer** width for a viewport width.
///
/// [`VIEWPORT_FILL`] — 80% — of the window (defect #4), clamped to
/// `viewport.width - VIEWPORT_GUTTER` (`space::XXL`, 32px) on a window
/// too narrow to hold it. The card's padding is *inside* this figure — see
/// [`crate::ui_iced::overlay::modal_card`].
#[must_use]
pub fn tools_card_width(viewport_width: f32) -> f32 {
    TOOLS_SIZING.width.resolve(viewport_width, 0.0)
}

/// The natural height of the card's table region for `row_count` groups
/// plus a header row and an optional error banner.
///
/// Since defect #4 the card is sized by the viewport, **not** by this
/// figure. What remains is the content's own extent: the room the list
/// *would* need if it were allowed to lay every row out at rest, which is
/// how the tests say "the list got more space than its content needs"
/// without restating the arithmetic.
///
/// Every figure is a framework constant rather than a card-local number:
/// the row height is [`list_tile`]'s single-line tile — the same
/// `size::CONTROL_MD` grid every selectable row in the app sits on, and
/// exactly what the Actions cell's 32px `Button` needs — and the text
/// bands are the type rungs they are rendered at.
#[must_use]
pub const fn tools_content_height(row_count: usize, shows_error: bool) -> f32 {
    let rows = list_tile::SINGLE_LINE_HEIGHT * (row_count as f32 + 1.0);
    let banner = if shows_error {
        typography::BODY.line_height + space::SM
    } else {
        0.0
    };
    typography::TITLE_SM.line_height
        + typography::BODY.line_height
        + shape::BORDER_HAIRLINE
        + rows
        + banner
}

/// The Tools card's **outer** height for a viewport (defect #4).
///
/// [`VIEWPORT_FILL`] of the window, floored at [`TOOLS_HEIGHT_MIN`] and
/// clamped last by the framework's `VIEWPORT_GUTTER` so a tiny window can
/// never be overflowed. The card no longer fits its content: the row
/// count is irrelevant to the box, which is the whole point of the
/// decision — a two-group list and a forty-group list get the same card,
/// and the list scrolls inside it.
#[must_use]
pub fn tools_card_height(viewport: Size) -> f32 {
    // `Desired::Viewport` ignores the content extent; `0.0` is passed so
    // the policy's floor and gutter are the only things that can move it.
    TOOLS_SIZING.height.resolve(viewport.height, 0.0)
}

/// The Tools card's outer size for a viewport, and the size the user dragged
/// it to if there is one (defect #4 sets the fill, defect #6 the override).
///
/// `user == None` is the policy from defect #4, unchanged: [`VIEWPORT_FILL`] on
/// both axes, so a card that has never been dragged still measures exactly 80%
/// of the window.
///
/// `user == Some(size)` is a drag. The wanted size is the user's, and only the
/// two framework bounds remain — [`TOOLS_RESIZE_FLOORS`] from below and the
/// `VIEWPORT_GUTTER` from above, applied last so the card can never overflow
/// the window (owner ruling D-3: the 80% fill is the *initial* size, not a
/// ceiling).
///
/// The card's position is never a parameter: a centred modal is centred, and
/// the grip changes width and height only.
#[must_use]
pub fn tools_card_size(viewport: Size, user: Option<Size>) -> Size {
    match user {
        None => Size::new(
            tools_card_width(viewport.width),
            tools_card_height(viewport),
        ),
        Some(wanted) => TOOLS_RESIZE_FLOORS.resolve(wanted, viewport),
    }
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
    ///
    /// The count text comes from [`strings::tools_group_count`], which
    /// pluralises. It used to be `format!("{} {}", count,
    /// strings::TOOLS_GROUP_COLUMN)` — the *column header* — so ten groups
    /// read "10 Group".
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            strings::TOOLS_EMPTY_STATE.to_string()
        } else {
            strings::tools_group_count(self.count())
        }
    }

    /// The structured failure this card surfaces (`UIF-MOD-003`).
    ///
    /// A failed MCP authentication is the failure this dialog actually has:
    /// the registry records it on the group as a
    /// [`ToolErrorKind::Authentication`] `last_error`, and the row mapping
    /// already carries that kind through. Deriving the banner from the
    /// rows rather than a separate slot means the banner changes on
    /// exactly two events — the failure being recorded, and the user
    /// clearing it with `Restart` — which is the same discipline
    /// `ModalHost`'s `set_error` / `clear_error` pair enforces
    /// explicitly.
    ///
    /// `Recovery::Retry` because the flow is re-entrant: pressing Retry
    /// re-issues `StartMcpAuth` for the same server, which is exactly
    /// what the Authenticate button does.
    ///
    /// Purity: pure function of the rows.
    #[must_use]
    pub fn modal_error(&self) -> Option<ModalError> {
        let row = self.rows.iter().find(|row| {
            row.error_kind == Some(ToolErrorKind::Authentication) && row.mcp_server().is_some()
        })?;
        let server = row.mcp_server().unwrap_or_default();
        Some(
            ModalError::new(
                TOOLS_AUTH_ERROR_CODE,
                strings::format_tools_auth_error(row.display_name.as_str()),
            )
            .at(ErrorLocation::Region(format!(
                "{}: {}",
                strings::TOOLS_KIND_COLUMN,
                server
            )))
            .with_recovery(Recovery::Retry),
        )
    }

    /// Whether the error banner occupies its band this frame.
    #[must_use]
    pub fn shows_error(&self) -> bool {
        self.modal_error().is_some()
    }

    /// The message the banner's `Retry` affordance emits: re-run the
    /// authentication that failed.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn retry_message(&self) -> Option<Message> {
        let row = self.rows.iter().find(|row| {
            row.error_kind == Some(ToolErrorKind::Authentication) && row.mcp_server().is_some()
        })?;
        Some(start_auth_command(row.mcp_server().unwrap_or_default()))
    }

    /// The card's outer size for a viewport **when it has never been
    /// dragged** — the window's [`VIEWPORT_FILL`], independent of how many
    /// rows this snapshot holds (defect #4).
    ///
    /// A dragged size is overlay state, not snapshot state, so it is not
    /// reachable from here; see [`tools_card_size`] for the full rule.
    #[must_use]
    pub fn size_for(&self, viewport: Size) -> Size {
        tools_card_size(viewport, None)
    }

    /// Renders this snapshot as the tools card ([`tools_dialog_card`]), at the
    /// undragged fill.
    #[must_use]
    pub fn view(&self, viewport: Size) -> Element<'_, Message> {
        tools_dialog_card(self, viewport, None)
    }
}

/// Renders the tools card: at [`VIEWPORT_FILL`] of the viewport when `user` is
/// `None`, and at the size the user dragged it to when it is `Some` (defect #6).
///
/// ```text
/// ┌─ Tools card · 80% of the viewport · radius 8 ───────────────────┐
/// │ Tools                                                        × │
/// │ 9 groups                                                       │
/// │ ─────────────────────────────────────────────────────────────── │
/// │ [!] MCP authentication failed for `srv`.        [ Retry ]       │
/// │ ─────────────────────────────────────────────────────────────── │
/// │ ☑ │ Group      │ Kind        │ Tools  │ Prompt      │ Actions    │
/// │ ☑ │ Filesystem │ Internal    │ read…  │ 1204 (✓)  │            │
/// │ ☑ │ my-server  │ MCP (remote)│ fetch  │  612       │ ⚠ [Auth]… │
/// │ ─────────────────────────────────────────────────────────────── │
/// │                                                    [ Close ]  ◢ │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// The `◢` in the last row is [`modal_resize_grip`]: the card's only resize
/// affordance, and the only cell of the footer that is not a button.
///
/// The parallel-safe marker renders only on `parallel_safe` rows
/// ([UI-058]); the row-level error text renders only inside the `⚠`
/// marker's hover tooltip ([UI-057]), while a *failed authentication*
/// additionally raises the card-level banner (`UIF-MOD-003`).
///
/// The card is the *content* only: [`tools_modal`] mounts it through
/// [`crate::ui_iced::overlay::modal_centered`], which supplies the scrim
/// and the card-boundary absorption.
///
/// Emitted messages:
/// * enable checkbox → [`UserCommand::SetToolGroupEnabled`]
/// * Restart → [`UserCommand::ClearToolGroupError`]
/// * Authenticate → [`UserCommand::StartMcpAuth`] (inert while running)
/// * Forget → [`UserCommand::ForgetMcpAuth`]
/// * Retry → [`UserCommand::StartMcpAuth`] for the failed server
/// * title-band `×` and footer `Close` → [`dismiss_message()`]
///
/// Purity: pure view over `view_model` and `viewport`.
#[must_use]
pub fn tools_dialog_card(
    view_model: &ToolsDialogViewModel,
    viewport: Size,
    user: Option<Size>,
) -> Element<'_, Message> {
    let frame = modal_frame();
    // One solve for both axes, so the width and the height can never be
    // measured against two different policies (defect #6).
    let size = tools_card_size(viewport, user);

    let banner = view_model
        .modal_error()
        .map(|error| error_banner(&error, view_model.retry_message()))
        .unwrap_or_else(|| Space::new().height(Length::Shrink).into());

    let list = scrollable(tools_table(view_model.rows()))
        .id(TOOLS_LIST_ID)
        .height(Length::Fill)
        .width(Length::Fill);

    // The body spans the card's inner width rather than shrinking to its
    // widest band, so the list and the footer ride the card's own edges
    // instead of leaving the old 720px block centred inside a bigger box
    // (defect #4).
    let body = Column::with_children(vec![
        title_band(),
        rung(
            view_model.summary(),
            typography::BODY,
            Palette::fastmd_dark().text_muted,
        )
        .into(),
        modal_divider(frame).into(),
        banner,
        list.into(),
        modal_divider(frame).into(),
        footer(),
    ])
    .width(Length::Fill)
    .spacing(space::SM);

    modal_card(
        body,
        frame,
        TOOLS_CARD_ID,
        Length::Fixed(size.width),
        Length::Fixed(size.height),
    )
    .into()
}

/// The card's title band: the title on the leading edge and the `×`
/// dismiss control on the trailing edge, on one row (defect #7).
///
/// Kept out of [`tools_dialog_card`] so the card stays a short list of
/// bands ([RUST-023]) and the close control can be asserted on its own.
///
/// Purity: pure.
#[must_use]
fn title_band() -> Element<'static, Message> {
    Row::with_children(vec![
        rung(
            strings::TOOLS_DIALOG_TITLE,
            typography::TITLE_SM,
            Palette::fastmd_dark().text_strong,
        )
        .into(),
        Space::new().width(Length::Fill).into(),
        close_button().into(),
    ])
    .align_y(Alignment::Center)
    .into()
}

/// The title band's `×` dismiss control (defect #7).
///
/// The [`strings::ICON_CLOSE`] glyph measures below §3.1's icon-only floor,
/// so this control *is* an icon-only button and `UIF-ATOM-001`'s icon-only
/// cell makes the tooltip mandatory — it is the only place the label
/// exists. Built with [`Button::with_tooltip`] carrying
/// [`strings::TOOLS_CLOSE_TOOLTIP`], the same shape that retired the
/// identical crash on the movable card's Close button; the pinned test in
/// the sidecar asserts the icon-only rule is both engaged and satisfied.
///
/// Wired to [`dismiss_message()`] — the same single close transition the
/// scrim click and Escape use, so no second dismiss vocabulary exists.
///
/// Purity: pure.
#[must_use]
fn close_button() -> Button<'static, Message> {
    Button::new(
        ButtonVariant::Ghost,
        rung(
            strings::ICON_CLOSE,
            typography::TITLE_SM,
            Palette::fastmd_dark().text_muted,
        ),
        dismiss_message(),
        TOOLS_CLOSE_BUTTON_ID,
    )
    .palette(Palette::pro())
    .with_tooltip(strings::TOOLS_CLOSE_TOOLTIP)
}

/// The card's footer: the `Close` button and the resize grip on the trailing
/// edge (defect #8, defect #6), the same `[ Space Fill, button ]` shape the
/// Batch card's footer uses with the grip appended.
///
/// The grip is the last cell, so it sits at the card's bottom-right corner —
/// where a resize handle is expected, and where the movable shells put theirs.
/// Being a child of the card it is inside the card's own hit boundary, so a
/// press on it is absorbed by the card and never reaches the scrim
/// (`UIF-VER-003`).
///
/// Purity: pure.
#[must_use]
fn footer() -> Element<'static, Message> {
    Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        footer_close_button().into(),
        modal_resize_grip(TOOLS_RESIZE_GRIP_ID, resize_press_message()),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// The footer's `Close` button (defect #8).
///
/// Always enabled: leaving the card is never conditional on the rows, and a
/// disabled dismiss would trap the user in a modal whose only other exit is
/// the scrim.
///
/// Purity: pure.
#[must_use]
fn footer_close_button() -> Button<'static, Message> {
    footer_button_widget(
        ButtonVariant::Secondary,
        strings::CLOSE_BUTTON,
        dismiss_message(),
        TOOLS_FOOTER_CLOSE_BUTTON_ID,
    )
}

/// The card-level structured failure banner (`UIF-MOD-003`).
///
/// The banner states **what** failed, **where** it happened and **what
/// the user can do** — the three things §4.6.3 names — and the modal
/// stays open showing it. `Retry` is live whenever the retry command
/// exists; when it does not, the control stays visible and inert rather
/// than disappearing (AGENTS §6), so the banner never reads as if an
/// action were available when none is.
fn error_banner(error: &ModalError, retry: Option<Message>) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let detail = match error.location() {
        Some(ErrorLocation::Region(region)) => format!("{} — {}", error.message(), region),
        _ => error.message().to_string(),
    };

    let can_retry = retry.is_some();

    Row::with_children(vec![
        rung(detail, typography::BODY, palette.error).into(),
        Space::new().width(Length::Fill).into(),
        footer_button(
            ButtonVariant::Danger,
            strings::TOOLS_RETRY_BUTTON,
            retry.unwrap_or_else(dismiss_message),
            TOOLS_ERROR_RETRY_ID,
            can_retry,
        ),
    ])
    .spacing(space::MD)
    .align_y(Alignment::Center)
    .into()
}

/// The six-column group table — see the module header for the column
/// mapping and the verified iced citations. With zero rows the header still
/// renders, so the empty state needs no separate widget.
///
/// Every cell is fixed to [`list_tile::SINGLE_LINE_HEIGHT`] so the rendered
/// rows are the same height the card's own height budget
/// ([`tools_content_height`]) charged for them, and the Actions cell's 32px
/// buttons sit on the same `size::CONTROL_MD` grid as every other
/// selectable row in the app.
///
/// Purity: pure over `rows`.
#[must_use]
pub fn tools_table(rows: &[ToolGroupRow]) -> Element<'static, Message> {
    let columns = vec![
        table::column(
            header_cell(""),
            move |row: ToolGroupRow| -> Element<'static, Message> { enable_cell(&row) },
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
                cell(rung(
                    row.kind_label,
                    typography::BODY,
                    Palette::fastmd_dark().text,
                ))
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::TOOLS_LIST_COLUMN),
            move |row: ToolGroupRow| -> Element<'static, Message> {
                cell(rung(
                    row.tools_label.clone(),
                    typography::BODY,
                    Palette::fastmd_dark().text,
                ))
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
        .padding_x(space::XS)
        .padding_y(0.0)
        .separator_x(shape::BORDER_HAIRLINE)
        .separator_y(shape::BORDER_HAIRLINE)
        .width(Length::Fill)
        .into()
}

/// One table cell pinned to the framework's single-line row height.
fn cell(content: text::Text<'static>) -> Element<'static, Message> {
    container(content)
        .height(Length::Fixed(list_tile::SINGLE_LINE_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

/// A column-header cell.
fn header_cell(label: &'static str) -> Element<'static, Message> {
    container(rung_weighted(
        label,
        typography::UI_COMPACT,
        Palette::fastmd_dark().text_strong,
        typography::Weight::SemiBold,
    ))
    .height(Length::Fixed(list_tile::SINGLE_LINE_HEIGHT))
    .align_y(Alignment::Center)
    .into()
}

/// The enable control: Hoarfrost's checkbox atom (`UIF-ATOM-003`).
fn enable_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    let check_state = if row.enabled {
        CheckState::Checked
    } else {
        CheckState::Unchecked
    };

    let id = row.id.clone();
    let focus_id = format!("tools.enable.{}", row.display_name);

    Checkbox::standalone("", check_state, focus_id, move |state| {
        let toggled = matches!(state, CheckState::Checked);
        Some(set_enabled_command(&id, toggled))
    })
    .with_palette(Palette::pro())
    .into()
}

/// The Prompt column's cell: the summed char count, followed by the
/// `✓ parallel` marker when — and only when — the group is parallel-safe
/// ([UI-058]). The marker's absence is not a negative signal, so a
/// non-parallel row renders the count alone.
fn char_count_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    let count: Element<'static, Message> = rung(
        row.prompt_char_count.to_string(),
        typography::BODY,
        Palette::fastmd_dark().text_value,
    )
    .into();

    match parallel_chip_label(row) {
        Some(label) => cell_in_row(
            Row::with_children(vec![count, parallel_chip(label)])
                .spacing(space::XS)
                .align_y(Alignment::Center),
        ),
        None => cell_in_row(count),
    }
}

/// The `✓ parallel` marker, resolved as a framework **status badge**
/// (`UIF-ATOM-008`) rather than an ad-hoc styled box.
///
/// `widget::chip` (`UIF-ATOM-006`) was the first candidate and is the
/// wrong one: its three variants are `Assist`, `Filter` and `Input`, all
/// of which describe *what pressing the chip does*, and none of them
/// carries a status colour at all — a chip's selected polarity resolves
/// to `accent.fill`, not to success. A `✓` that means "this group is
/// parallel-safe" is a judgement, and §3.8's status chip is the framework's
/// component for a judgement: [`BadgeKind::Status`] with
/// [`StatusIcon::Success`] on [`BadgeTone::Success`] yields the pill's
/// height, padding, radius, stroke and colour from one resolver call.
///
/// The pinned label [`strings::TOOLS_PARALLEL_CHIP`] already carries its
/// own `✓`, so the badge's separate status glyph is not drawn on top of
/// it — the badge supplies the box and the tone, the string supplies the
/// mark.
fn parallel_chip(label: &'static str) -> Element<'static, Message> {
    let model = BadgeModel::status(StatusIcon::Success, label).with_tone(BadgeTone::Success);
    let style = badge::resolve_style(&model, &Palette::pro());

    container(rung_weighted(
        label,
        typography::CAPTION,
        style.text,
        style.weight,
    ))
    .height(Length::Fixed(style.height))
    .padding(iced::Padding::from([0.0, style.h_padding]))
    .align_y(Alignment::Center)
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(style.fill)),
        border: iced::Border {
            color: style.border,
            width: style.border_width,
            radius: style.radius.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A row-height wrapper for content that is already a `Row`.
fn cell_in_row(content: impl Into<Element<'static, Message>>) -> Element<'static, Message> {
    container(content)
        .height(Length::Fixed(list_tile::SINGLE_LINE_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

/// The group-name cell: the strong display name on a single line. The
/// row's error is deliberately **not** rendered here — [UI-057] carries it
/// in the `⚠` marker's hover tooltip (see [`actions_cell`]), so the cell
/// keeps the same shape whether or not an error was recorded.
fn group_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    cell(rung(
        row.display_name.clone(),
        typography::BODY,
        Palette::fastmd_dark().text_strong,
    ))
}

/// The Actions cell: the `⚠` marker with hover tooltip ([UI-057]),
/// plus the row's mapped action buttons. A clean row keeps the marker
/// slot empty and mounts no tooltip.
///
/// `Forget` is the one `Danger` control on this card. It discards a stored
/// OAuth credential, which is the only action in the dialog that makes the
/// user give something up that they cannot re-derive by pressing a button
/// — `Restart` and `Authenticate` both just try again. `Restart` and
/// `Authenticate` are `Secondary` for that reason.
fn actions_cell(row: &ToolGroupRow) -> Element<'static, Message> {
    let marker = match error_tooltip_body(row) {
        Some(body) => {
            let marker_content = rung(
                strings::TOOLS_ERROR_ICON,
                typography::BODY,
                error_marker_color(),
            );
            let marker_with_hover = mouse_area(marker_content)
                .on_enter(Message::Layout(LayoutMessage::TooltipHoverEnter {
                    id: TooltipId::ToolsError,
                    tooltip_text: Some(body.to_string()),
                }))
                .on_exit(Message::Layout(LayoutMessage::TooltipHoverExit {
                    id: TooltipId::ToolsError,
                }));
            marker_with_hover.into()
        }
        None => Space::new().width(Length::Shrink).into(),
    };

    let buttons = row_actions(row).into_iter().fold(
        Row::<Message>::new().spacing(space::SM),
        |buttons, action| buttons.push(action_button(&action)),
    );

    cell_in_row(
        Row::with_children(vec![marker, buttons.into()])
            .spacing(space::SM)
            .align_y(Alignment::Center),
    )
}

/// One action button, variant chosen by what the action costs the user.
fn action_button(action: &ToolAction) -> Element<'static, Message> {
    let variant = if action.label == strings::TOOLS_FORGET {
        ButtonVariant::Danger
    } else {
        ButtonVariant::Secondary
    };

    footer_button(
        variant,
        action.label,
        action.press.clone().unwrap_or_else(dismiss_message),
        action_focus_id(action.label),
        action.is_enabled(),
    )
}

/// The stable focus id for an action label.
///
/// Ids are derived from the label rather than the row so the same control
/// keeps the same id across rebuilds. Two live buttons never share one id
/// because a row renders each label at most once — `Restart`,
/// `Authenticate` / `Authenticating…` and `Forget` are distinct labels —
/// and the dialog shows one table.
#[must_use]
pub fn action_focus_id(label: &str) -> &'static str {
    if label == strings::TOOLS_FORGET {
        "tools.action.forget"
    } else if label == strings::TOOLS_RESTART {
        "tools.action.restart"
    } else if label == strings::TOOLS_AUTH_BUTTON {
        "tools.action.authenticate"
    } else {
        "tools.action.running"
    }
}

/// A footer-width framework button carrying `label`, **before** its enabled
/// state is applied.
///
/// Split out of [`footer_button`] so a test can read the control's own
/// reported state — `is_enabled`, `variant`, `focus_id`, `tooltip_text` —
/// instead of digging it back out of an [`Element`], which exposes no such
/// accessors.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH)
/// box for the reason [`footer_button`] states: the content can never
/// measure icon-only.
fn footer_button_widget(
    variant: ButtonVariant,
    label: impl Into<String>,
    on_press: Message,
    focus_id: &'static str,
) -> Button<'static, Message> {
    let content = Column::new()
        .width(Length::Fixed(
            hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH,
        ))
        .align_x(Alignment::Center)
        .push(rung(
            label,
            typography::BODY,
            Palette::fastmd_dark().text_strong,
        ));

    Button::new(variant, content, on_press, focus_id).palette(Palette::pro())
}

/// A footer-width framework button carrying `label`.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH)
/// box on purpose. `Button::is_icon_only` is a **geometric** test — any
/// content no wider and no taller than `size::CONTROL_SM` (28px) is
/// classed icon-only, and an icon-only button without a tooltip trips a
/// `debug_assert!` in `outer_size`. A short label like `'Forget'` sits
/// close enough to that line that a different font measurement would cross
/// it, so the label is given a width the test cannot fail.
fn footer_button(
    variant: ButtonVariant,
    label: impl Into<String>,
    on_press: Message,
    focus_id: &'static str,
    enabled: bool,
) -> Element<'static, Message> {
    let button = footer_button_widget(variant, label, on_press, focus_id);

    if enabled {
        button.into()
    } else {
        button.disabled().into()
    }
}

/// Paints `value` on a type rung at its resting weight.
///
/// The single place this card turns a rung into a `text` widget, so no
/// call site restates a point size ([RUST-021], `UIF-TOK-004`).
///
/// **`line_height` must be handed to iced as a resolved `LineHeight`, not
/// a bare `f32`.** `impl From<f32> for iced::LineHeight` produces
/// `LineHeight::Relative(factor)` — a *multiple* of the font size — so
/// `.line_height(20.0)` on a 14px rung means 280px per line.
/// [`TypeRung::iced_line_height`] is the crossing point that makes picking
/// the wrong unit impossible.
fn rung(
    value: impl Into<String>,
    rung: typography::TypeRung,
    color: iced::Color,
) -> text::Text<'static> {
    text(value.into())
        .size(rung.size)
        .line_height(rung.iced_line_height())
        .color(color)
}

/// [`rung`] at an explicit framework weight.
///
/// iced 0.14's `Text` has no `.weight()` builder — weight travels on the
/// [`iced::Font`] record — and the framework's own `typography::Weight` is
/// a distinct type from `iced::font::Weight`. This is the one place that
/// crosses between them, so no call site restates the mapping or the
/// struct ([RUST-021]).
fn rung_weighted(
    value: impl Into<String>,
    rung: typography::TypeRung,
    color: iced::Color,
    weight: typography::Weight,
) -> text::Text<'static> {
    text(value.into())
        .size(rung.size)
        .line_height(rung.iced_line_height())
        .font(iced::Font {
            weight: iced_weight(weight),
            ..iced::Font::with_name("Fira Sans")
        })
        .color(color)
}

/// The iced weight a framework [`typography::Weight`] renders as.
///
/// The four rungs of the `UIF-TOK-004` scale map one-to-one onto iced's
/// named classes; nothing here invents a weight the scale does not have.
#[must_use]
pub const fn iced_weight(weight: typography::Weight) -> Weight {
    match weight {
        typography::Weight::Regular => Weight::Normal,
        typography::Weight::Medium => Weight::Medium,
        typography::Weight::SemiBold => Weight::Semibold,
        typography::Weight::Bold => Weight::Bold,
    }
}

/// The frame this card is drawn with (§4.6.1).
#[must_use]
pub fn modal_frame() -> ModalFrame {
    ModalFrame::resolve(&Palette::pro())
}

/// The blocking tools modal: [`tools_dialog_card`] centred over a scrim by
/// [`crate::ui_iced::overlay::modal_centered`].
///
/// # What each click means
///
/// The scrim emits [`dismiss_message()`], the same single close
/// transition Escape uses. A press that lands on the card's padding ring,
/// on an inert `Authenticating…` button, or on dead space between rows is
/// absorbed by [`ToolsDialogMessage::Absorbed`] and never reaches the
/// scrim (`UIF-VER-003`) — before this increment the card was mounted
/// through the older `overlay::modal`, which had **no absorb boundary at
/// all**, so a press on dead space inside the card fell through and closed
/// the dialog.
///
/// # While a resize is in flight
///
/// `user` is the size the card has reached (the live drag, or the remembered
/// size — see [`ToolsDialogState::size_override`]), and `resizing` says whether
/// the grip is currently held. While it is, [`modal_resize_capture_layer`] is
/// stacked above the modal so the moves and the release keep arriving: a drag
/// that left the 16px grip would otherwise freeze, and releasing outside the
/// card would land on the scrim and dismiss the very card being resized.
/// The lane is mounted for no other frame.
#[must_use]
pub fn tools_modal(
    view_model: &ToolsDialogViewModel,
    viewport: Size,
    user: Option<Size>,
    resizing: bool,
) -> Element<'_, Message> {
    let modal = modal_centered(
        tools_dialog_card(view_model, viewport, user),
        Message::Tools(ToolsDialogMessage::Absorbed),
        dismiss_message(),
    );

    if !resizing {
        return modal;
    }

    stack(vec![
        modal,
        modal_resize_capture_layer(
            TOOLS_RESIZE_CAPTURE_ID,
            |at| Message::Tools(ToolsDialogMessage::ResizeMoved { at }),
            Message::Tools(ToolsDialogMessage::ResizeReleased),
        ),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The size a pointer at `pointer` asks a **centred** card for.
///
/// A centred card's bottom-right corner sits at `centre + size / 2`, so the
/// size that puts the corner under the pointer is twice the pointer's distance
/// from the centre. Stating the rule this way — rather than as a delta from
/// where the press began — is what makes the grip track the pointer one for
/// one: the corner is already under the pointer when it is taken, so the first
/// pixel of movement moves the corner that same pixel, and nothing has to be
/// remembered about the press.
///
/// It also keeps the card centred by construction, which is the whole of owner
/// ruling D-3: the grip changes width and height, never position.
///
/// The result can be negative (a pointer dragged left of centre); that is
/// deliberate — [`tools_card_size`] raises it to the floor, so an over-drag
/// shrinks the card to its minimum instead of inverting it.
#[must_use]
pub fn centred_size(pointer: Point, viewport: Size) -> Size {
    Size::new(
        2.0 * (pointer.x - viewport.width / 2.0),
        2.0 * (pointer.y - viewport.height / 2.0),
    )
}

/// One grip drag in flight: what the card measured when the grip was taken, and
/// what it measures now.
///
/// Transient by design (owner ruling D-3) — it is a gesture, not application
/// state, so it is never persisted and a half-finished drag that is abandoned
/// leaves nothing behind. `live` is the clamped size the card is showing right
/// now, so the view and the eventual settle agree on one number, and `start` is
/// what lets a press that never moved be recognised as not a resize.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ToolsResizeDrag {
    /// The card's size when the grip was taken.
    start: Size,
    /// The clamped size the drag has reached so far.
    live: Size,
}

/// What one resize message did to the card's geometry.
///
/// The reducer needs to know whether a drag *settled*, because that is the only
/// moment worth writing to storage ([NFR-002]); the view needs to know a drag is
/// moving. This is the difference, stated once instead of being re-derived at
/// each call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolsResizeEffect {
    /// Nothing to persist: not a resize message, a drag that has not settled,
    /// or a grip press that never turned into a drag.
    Idle,
    /// The drag moved the card to `size`. Render it; write nothing.
    Dragging {
        /// The clamped size the card now shows.
        size: Size,
    },
    /// The drag ended at `size` — this is the user's size now, and the one
    /// moment the reducer persists it.
    Settled {
        /// The size the drag ended on.
        size: Size,
    },
}

/// Which tools dialog is open — the pure state the reducer holds.
///
/// One window at a time, `open` replaces whatever was showing, and `close`
/// reports whether anything was consumed so an Escape handler can tell a
/// consumed key from a no-op. Opening is user intent and rides
/// [`UserCommand::OpenToolsDialog`]; only the dismiss needs a message of
/// its own (see the module header).
///
/// # Size
///
/// The card also remembers the size the user dragged it to
/// ([`user_size`](Self::user_size)), and carries the drag itself while it is in
/// flight (its private `drag` field). Position is not here at all, and must not
/// be added: this card is a centred modal, so the grip changes width and height
/// only (owner ruling D-3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ToolsDialogState {
    open: bool,
    /// The size the user dragged the card to, if they ever did.
    user_size: Option<Size>,
    /// The grip drag in flight, if the button is down.
    drag: Option<ToolsResizeDrag>,
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
    ///
    /// A drag left over from a previous showing is dropped: it belongs to a
    /// gesture the user finished (or abandoned) last time.
    pub fn open(&mut self) -> bool {
        self.drag = None;
        if self.open {
            return false;
        }
        self.open = true;
        true
    }

    /// Hides the dialog. Returns whether one was open — the same
    /// "was anything consumed" contract as
    /// [`crate::ui_iced::overlay::OverlaySlot::close`].
    ///
    /// The in-flight drag is dropped with it: a release that never arrived must
    /// not leave the next showing mounted behind a capture lane. The remembered
    /// size survives — it is the user's, not the gesture's.
    pub fn close(&mut self) -> bool {
        let was_open = self.open;
        self.open = false;
        self.drag = None;
        was_open
    }

    /// Flips the visible state, returning the new state.
    pub fn toggle(&mut self) -> bool {
        if self.open {
            self.close();
        } else {
            self.open();
        }
        self.open
    }

    /// The size the user dragged the card to, if any.
    #[must_use]
    pub fn user_size(&self) -> Option<Size> {
        self.user_size
    }

    /// Adopts a remembered size, on open. `None` means "never dragged", which
    /// puts the card back on the viewport fill.
    pub fn set_user_size(&mut self, size: Option<Size>) {
        self.user_size = size;
    }

    /// Whether a grip drag is in flight — the only frame the capture lane is
    /// mounted for.
    #[must_use]
    pub fn is_resizing(&self) -> bool {
        self.drag.is_some()
    }

    /// The size the view should hand the card: the live drag while it runs,
    /// otherwise the remembered size, otherwise `None` for the viewport fill.
    #[must_use]
    pub fn size_override(&self) -> Option<Size> {
        self.drag.map(|drag| drag.live).or(self.user_size)
    }

    /// The size the card measures at right now for `viewport` — the same solve
    /// the view performs, exposed so the reducer and the tests measure one
    /// number rather than restating it.
    #[must_use]
    pub fn card_size(&self, viewport: Size) -> Size {
        tools_card_size(viewport, self.size_override())
    }

    /// Applies one [`ToolsDialogMessage`] to the **visibility** of the window
    /// and reports whether that changed.
    ///
    /// The resize events are deliberately inert here: they change geometry, not
    /// visibility, and are reduced by [`apply_resize`](Self::apply_resize).
    pub fn apply(&mut self, message: ToolsDialogMessage) -> bool {
        match message {
            ToolsDialogMessage::CloseRequested => self.close(),
            ToolsDialogMessage::Absorbed
            | ToolsDialogMessage::ResizeGripPressed
            | ToolsDialogMessage::ResizeMoved { .. }
            | ToolsDialogMessage::ResizeReleased => false,
        }
    }

    /// Applies one [`ToolsDialogMessage`] to the card's **geometry** and reports
    /// what it did.
    ///
    /// Only the three resize events mean anything here; every other message of
    /// the channel is inert, so the reducer can hand this state the whole
    /// channel without deciding what belongs to it.
    ///
    /// A move carries its own pointer position, and that is the only coordinate
    /// the drag needs: [`centred_size`] turns a pointer into a size outright, so
    /// nothing has to be remembered about where the press began — which matters,
    /// because `mouse_area::on_press` carries no point in iced 0.14 and a
    /// remembered cursor can be stale by the time a grip is taken.
    ///
    /// Every size that leaves here has been through [`tools_card_size`], so the
    /// floors and the viewport gutter are applied before the view ever sees a
    /// number — and before anything is persisted.
    ///
    /// A press that never moves is *not* a resize: releasing without changing the
    /// size leaves the card on the viewport policy rather than pinning it at
    /// whatever the window happened to be.
    pub fn apply_resize(
        &mut self,
        message: ToolsDialogMessage,
        viewport: Size,
    ) -> ToolsResizeEffect {
        match message {
            ToolsDialogMessage::ResizeGripPressed => {
                let start = self.card_size(viewport);
                self.drag = Some(ToolsResizeDrag { start, live: start });
                ToolsResizeEffect::Idle
            }
            ToolsDialogMessage::ResizeMoved { at } => {
                let Some(drag) = self.drag.as_mut() else {
                    return ToolsResizeEffect::Idle;
                };
                drag.live = tools_card_size(viewport, Some(centred_size(at, viewport)));
                ToolsResizeEffect::Dragging { size: drag.live }
            }
            ToolsDialogMessage::ResizeReleased => {
                let Some(drag) = self.drag.take() else {
                    return ToolsResizeEffect::Idle;
                };
                if drag.live == drag.start {
                    return ToolsResizeEffect::Idle;
                }
                self.user_size = Some(drag.live);
                ToolsResizeEffect::Settled { size: drag.live }
            }
            ToolsDialogMessage::CloseRequested | ToolsDialogMessage::Absorbed => {
                ToolsResizeEffect::Idle
            }
        }
    }
}
