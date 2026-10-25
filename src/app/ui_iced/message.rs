//! Top-level iced `Message` enum — the single message type driving the
//! FastMD Elm loop.
//!
//! # Channel design
//!
//! [`Message`] has **one variant per logical interaction channel**. Each
//! channel is its own small sub-enum (`Debug`/`Clone`, plus `Copy` wherever
//! every payload field is `Copy`), so the top level stays flat and readable
//! while each panel owns a vocabulary it can grow independently.
//!
//! Two routing rules keep the channels from overlapping:
//!
//! 1. **User intent rides [`Message::Command`].** Anything the user asked
//!    the *application* to do — open a file, close a tab, run the agent,
//!    rename a path — is a self-contained [`UserCommand`] carried by the
//!    command bus (`src/app/bus/events/user_command.rs`). Panels emit those
//!    directly and never re-invent a parallel intent vocabulary per panel.
//! 2. **Panel channels carry view-local state transitions only** — state no
//!    widget holds on its own behalf (which row's context menu is open, what
//!    the search box currently holds, the editor's undo request) plus the
//!    runtime reports the window layer hands us.
//!
//! One deliberate exception to rule 2 lives in [`TreeMessage`]: three
//! file-tree context-menu rows — "Run as prompt…", "Print…" and the
//! multi-select "Delete" — have **no single [`UserCommand`] equivalent**
//! and would have to read files from inside the click handler. A pure view
//! must not do I/O, so those rows emit a *deferred-intent* variant that
//! names the target and leaves the read and the resulting command(s) to the
//! reducer. They are still user intent — they are here only because the
//! command channel cannot express them atomically.
//!
//! Consequently some channels are deliberately thin: they exist because a
//! panel needs explicit state a retained widget would otherwise hide, and
//! they carry nothing more.
//!
//! Unit tests live in the sibling `message_tests.rs` sidecar.

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;

use crate::bus::events::AgentEvent;
use crate::bus::events::directory::DirectoryChanged;
use crate::bus::events::file::FileChanged;
use crate::bus::events::messages::LogCategory;
use crate::bus::events::typed::BackgroundEvent;
use crate::bus::events::user_command::UserCommand;
use crate::config::AppConfig;
use crate::ui::tree_search::SearchResultEntry;
use iced::widget::pane_grid;
use iced_pro::surface::menu::cascade::MenuKey;
use iced_pro::surface::menu::model::MenuId;
use std::path::PathBuf;
use std::sync::Arc;

/// The single top-level message of the iced Elm loop.
///
/// `Message` is `Clone` but not `Copy`: the [`UserCommand`] payload carries
/// `String`/`PathBuf`/`Vec` fields ([`UserCommand`] derives `Clone, Debug,
/// PartialEq` only), which makes the whole enum non-`Copy`. The individual
/// channel enums are `Copy` wherever their payloads allow — see
/// [`WindowMessage`], [`LayoutMessage`], [`TabMessage`] and
/// [`ThemeMessage`].
///
/// `PartialEq` is implemented on every channel (and on this enum): derived
/// where every payload supports it, hand-written where it does not (see
/// [`LayoutMessage`] and [`InboundMessage`]). Message equality is what lets
/// a replay/dedup check compare a recorded message against a freshly
/// produced one.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Window-lifecycle channel (close requests, close snapshot, scale
    /// baseline, resizes).
    Window(WindowMessage),
    /// Pane-geometry channel (pane-grid drags, tree auto-fit).
    Layout(LayoutMessage),
    /// The command channel: user intent forwarded to the orchestrator's
    /// `UserCommand` executor unchanged.
    Command(UserCommand),
    /// File-tree panel channel.
    Tree(TreeMessage),
    /// Tab-strip / center panel channel.
    Tab(TabMessage),
    /// Inline editor channel.
    Editor(EditorMessage),
    /// Agent panel / command-input channel.
    Agent(AgentMessage),
    /// Workspace content-search channel (UI-050).
    Search(SearchMessage),
    /// Background-logs panel channel (UI-047).
    Logs(LogsMessage),
    /// Theme / UI-scale channel.
    Theme(ThemeMessage),
    /// Markdown render-path channel.
    Render(RenderMessage),
    /// Small individual dialogs — create-directory / create-document /
    /// rename / move / delete-confirmation (UI-011/UI-015).
    Dialog(DialogMessage),
    /// The About window and the third-party attributions window.
    About(AboutMessage),
    /// The batch prompt-processing dialog's own selection state
    /// (BATCH-001..BATCH-014).
    Batch(BatchDialogMessage),
    /// The tools dialog's window visibility (UI-051..UI-060).
    Tools(ToolsDialogMessage),
    /// The agent debug window's own disclosure state (UI-048).
    AgentDebug(AgentDebugMessage),
    /// Inbound *background → UI* notification channel — the only channel
    /// no widget can produce.
    Inbound(InboundMessage),
    /// The top-bar hamburger menu channel.
    Menu(MenuMessage),
    /// The floating-overlay / keyboard-modifier channel (UI-056, UI-022) —
    /// the runtime keyboard reports the overlay host and the modifier-aware
    /// tree selection need, produced only by the subscription bridge's
    /// keyboard lane.
    Overlay(OverlayMessage),
    /// Boot-infrastructure reports — the terminal outcomes of the one-shot
    /// tasks the `boot` closure returns (font registration). Not a panel
    /// channel and not user intent: the producer is the boot task chain
    /// itself.
    Boot(BootMessage),
}

/// Boot-infrastructure channel.
///
/// The `boot` closure returns a batched [`iced::Task`] whose members must
/// report back through the top-level `Message` type. The font-registration
/// report is one such report. It is deliberately **not** folded into
/// [`WindowMessage`] (font loading is not a window concern) nor into
/// [`InboundMessage`] (it is produced by the boot task chain, not by a
/// background bus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootMessage {
    /// One font collection settled registering. `Some` holds the failure
    /// description, logged as an `ERROR` ([NFR-001]) but non-fatal; `None`
    /// means the collection loaded cleanly.
    FontLoadFinished(Option<String>),
}

/// Wraps a [`UserCommand`] as the [`Message::Command`] channel value, so a
/// panel can produce intent without naming the wrapper at every call site.
impl From<UserCommand> for Message {
    fn from(command: UserCommand) -> Self {
        Self::Command(command)
    }
}

/// Window-lifecycle channel.
///
/// Every variant is a *runtime report* from the iced window layer — a
/// subscription or a chained `Task` result — not user intent. The user's
/// decision to save-and-close is application intent and rides
/// [`Message::Command`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowMessage {
    /// The window manager asked to close a window (title-bar close /
    /// `Alt+F4`). With `exit_on_close_request: false` the runtime waits for
    /// the app to persist state and call `iced::window::close`.
    CloseRequested(iced::window::Id),
    /// The metrics needed by the close-time snapshot have arrived, reported
    /// through the chained `window::size` → `window::position` tasks.
    CloseSnapshotCaptured(WindowSnapshot),
    /// The OS-reported baseline scale factor arrived; the composed factor is
    /// `baseline × user multiplier`.
    ScaleBaseline(f32),
    /// A window changed size ([`iced::window::resize_events`] delivers
    /// `(Id, Size)`; the id is carried so a multi-window runtime can route
    /// the event).
    Resized {
        /// The window that changed size.
        id: iced::window::Id,
        /// Its new logical size in points.
        size: iced::Size,
    },
}

/// The window metrics captured at close time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowSnapshot {
    /// The window the snapshot belongs to.
    pub id: iced::window::Id,
    /// Logical window size reported by the runtime.
    pub size: iced::Size,
    /// Window position in points, when the runtime reports one (`None` for
    /// platforms/states without a known position).
    pub position: Option<iced::Point>,
}

/// Pane and pointer geometry channel.
///
/// The three-band `PaneGrid` geometry lives in [`crate::ui_iced::layout`];
/// this channel is how its widget events, re-fit requests and root-space
/// pointer position reach `update`.
///
/// It also owns the **pointer position** ([`Self::CursorMoved`]). That is
/// the honest home for it: like a split drag and the tree auto-fit, a cursor
/// report is geometry, not what the user asked the application to do — so it is neither
/// [`Message::Command`] nor any panel's private view state. It has to live
/// somewhere the reducer reads before it opens an overlay, because iced's
/// `MouseArea::on_right_press` takes a fixed `Message` and cannot carry the
/// press point (`iced_widget-0.14.2/src/mouse_area.rs:68-71`), which is
/// exactly the gap [`crate::ui_iced::overlay::context_menu_trigger`]
/// documents.
#[derive(Debug, Clone, Copy)]
pub enum LayoutMessage {
    /// A pane-grid split was dragged ([`pane_grid::ResizeEvent`]:
    /// `split` + `ratio`), fed through `PaneGrid::on_resize`. Reduces onto
    /// `PanelLayout` widths under the UI-016 clamps.
    PaneResized(pane_grid::ResizeEvent),
    /// The tree panel should re-fit its width to the longest visible row
    /// (UI-016). In the Elm loop this is an explicit message so the width
    /// change is replayable.
    TreeAutoFit,
    /// The pointer moved to `point`.
    ///
    /// The reducer stores this and reads it back when a
    /// [`TreeMessage::ContextMenuOpened`] / [`TabMessage::ContextMenuOpened`]
    /// arrives, then feeds it to
    /// [`crate::ui_iced::overlay::OverlaySlot::open_context_menu`].
    /// `iced::Point` is `Copy` (`iced_core-0.14.0/src/point.rs:7`), so this
    /// variant does not cost the channel its `Copy`.
    ///
    /// Coordinates are **root-space logical pixels** from the full-window
    /// content `MouseArea`, matching the coordinate system used by the menu
    /// overlay and viewport clamp.
    CursorMoved {
        /// Pointer position relative to the full-window content root.
        point: iced::Point,
    },
}

/// Field-wise equality for [`LayoutMessage`].
///
/// `pane_grid::ResizeEvent` only derives `Debug, Clone, Copy`
/// (`iced_widget-0.14.2/src/pane_grid.rs:1226-1237`), so the channel
/// cannot derive `PartialEq`. The comparison is exact on `ratio`: both sides
/// carry the same runtime-produced `f32`, so bit equality is what a replay
/// or dedup check wants (and stays transitive, unlike a tolerance).
impl PartialEq for LayoutMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PaneResized(a), Self::PaneResized(b)) => {
                a.split == b.split && a.ratio == b.ratio
            }
            (Self::TreeAutoFit, Self::TreeAutoFit) => true,
            (Self::CursorMoved { point: a }, Self::CursorMoved { point: b }) => a == b,
            _ => false,
        }
    }
}

/// File-tree panel channel.
///
/// Deliberately thin: row selection and directory expansion are *user
/// intent* and ride [`Message::Command`] as `UserCommand::SelectFile` /
/// `UserCommand::SelectDirectory`. What a retained `context_menu` widget
/// would hold implicitly is explicit state under the iced
/// `overlay::menu` construction, so that is what this channel carries.
///
/// The last four variants are **deferred intent** (see the module header):
/// the menu actions would perform file I/O inside the click, which a pure
/// view may not do, and no single [`UserCommand`] covers them. Each names
/// its target so the reducer can perform the read and publish the real
/// command(s) — the intent stays self-contained and replayable.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeMessage {
    /// A right-click opened the row context menu for `path`; the overlay
    /// host renders it.
    ContextMenuOpened {
        /// The row whose menu is open.
        path: PathBuf,
    },
    /// The row context menu was dismissed (click-away or menu selection).
    ContextMenuClosed,
    /// "Run as prompt…" on a single file row (UI-020). The reducer reads
    /// the file and publishes `RunSkillPrompt`.
    RunAsPromptRequested {
        /// The markdown file whose body becomes the prompt.
        path: PathBuf,
    },
    /// A skill row was picked from a directory or file context menu
    /// (VFS-121 note skills, VFS-122 folder skills). The reducer reads
    /// `skill` and publishes `RunSkillPrompt { content, target_dir,
    /// target_file }`.
    SkillRunRequested {
        /// Path of the skill file to read.
        skill: PathBuf,
        /// Directory the skill runs against — set for folder skills.
        target_dir: Option<PathBuf>,
        /// File the skill runs against — set for note skills.
        target_file: Option<PathBuf>,
    },
    /// "Print…" on a file row (UI-014). There is **no** `UserCommand::Print`,
    /// so the reducer owns the whole job.
    ///
    /// The render, temp-file write and browser hand-off all block, so they
    /// ride an iced `Task` rather than running on the UI thread, and report
    /// back through [`TreeMessage::PrintFinished`].
    PrintRequested {
        /// The file to print.
        path: PathBuf,
    },
    /// The `iced::Task` spawned for a [`TreeMessage::PrintRequested`] has settled.
    /// Carries the outcome so a failure is logged from the reducer instead of
    /// the UI thread ever having touched the browser itself.
    PrintFinished {
        /// The file the print was attempted for.
        path: PathBuf,
        /// `Ok(())` once the browser has been handed the rendered document.
        result: Result<(), String>,
    },
    /// "Delete" on a multi-selected file row (UI-025), fanned out into one
    /// `UserCommand::Delete` per path followed by a selection clear; one
    /// menu row can only emit one message, so the whole set travels with it.
    DeleteSelectedRequested {
        /// Every selected path, in the order the panel rendered them.
        paths: Vec<PathBuf>,
    },
}

/// Tab-strip / center panel channel.
///
/// Same split as [`TreeMessage`]: selecting a tab, closing one
/// (`×` / middle-click), closing others/all, copying the path and revealing
/// it in the explorer are all `UserCommand`s already. The tab strip's own
/// view state is which tab's context menu is showing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabMessage {
    /// A right-click opened the context menu of the tab at `index`
    /// (index as rendered in the strip that frame, matching
    /// `UserCommand::CloseTab`'s indexing convention).
    ContextMenuOpened {
        /// The tab index whose menu is open.
        index: usize,
    },
    /// The tab context menu was dismissed.
    ContextMenuClosed,
}

/// Inline editor channel.
///
/// Save / Cancel buttons plus the live buffer, with every `Action::Edit`
/// intercepted so the undo history lives at OUR layer
/// (`iced::widget::text_editor` has no native undo/redo).
///
/// # Why `CancelRequested` and `DismissRequested` are separate messages
///
/// They look like the same intent and are not. `CancelRequested` is the
/// footer button: the user has said "throw the buffer away" explicitly, so
/// `UIF-MOD-002`'s guard lets it through. `DismissRequested` is a
/// *reflex* — the scrim (and, via the escape walk, `Escape`) — and must
/// never be able to discard unsaved work without asking. Folding the two
/// into one message is exactly how the pre-INT-5b editor lost edits on a
/// keypress, so the cause travels with the intent ([FMD-068]) and the
/// reducer decides through
/// [`guard_leave`](crate::ui_iced::editor::guard_leave).
#[derive(Debug, Clone, PartialEq)]
pub enum EditorMessage {
    /// An [`iced::widget::text_editor::Action`] was performed in the buffer
    /// — the interception point. `Action::is_edit()` separates
    /// content-changing actions (push an undo snapshot) from caret/selection
    /// actions.
    Action(iced::widget::text_editor::Action),
    /// The user asked to save the buffer to `path` (UI-043). The path is
    /// carried so the write target travels with the intent.
    SaveRequested {
        /// File the buffer belongs to.
        path: PathBuf,
    },
    /// The user cancelled the editor: discard the buffer, close the overlay
    /// (UI-045).
    ///
    /// This is the **explicit** discard the unsaved-edit guard names as its
    /// escape hatch — it is never what a scrim click or `Escape` sends.
    CancelRequested,
    /// A dismiss gesture that must pass the unsaved-edit guard before it
    /// may close anything (`UIF-MOD-002`).
    ///
    /// `cause` says which gesture it was, because the answer depends on it:
    /// a clean buffer closes, a dirty one is refused and the discard
    /// confirmation is raised instead.
    DismissRequested {
        /// The gesture being guarded.
        cause: crate::ui_iced::editor::LeaveGesture,
    },
    /// A press landed on the card's own boundary — its padding ring, a
    /// disabled `Save`, dead space between rows — and was absorbed there.
    ///
    /// Deliberately changes nothing: the whole contract is that such a
    /// press must not reach the scrim and must not close the editor
    /// (`UIF-VER-003`, `UIF-INP-002`).
    Absorbed,
    /// The user pressed the error banner's recovery affordance
    /// (`UIF-MOD-003`).
    ///
    /// `recovery` travels with the intent so the reducer performs *the
    /// affordance that was rendered* rather than re-deriving one from
    /// state that may since have moved.
    RecoveryRequested {
        /// File the failed action was aimed at.
        path: PathBuf,
        /// The affordance the banner was showing.
        recovery: iced_pro::surface::modal::Recovery,
    },
    /// Replay one snapshot back from the per-tab undo stack (UI-042).
    /// Per-tab stacks make the tab path part of the intent.
    Undo {
        /// The tab whose buffer is being undone.
        path: PathBuf,
    },
    /// Replay one snapshot forward on the per-tab redo stack (UI-042).
    Redo {
        /// The tab whose buffer is being redone.
        path: PathBuf,
    },
}

/// Agent panel / command-input channel.
///
/// Running, cancelling, queueing a prompt and clearing the input are all
/// `UserCommand`s
/// (`RunAgent` / `CancelAgent` / `QueueAgentPrompt` / `ClearCommandInput`),
/// produced by the reducer after parsing the submitted text with the
/// existing `parse_command_intent` helper. The agent *debug* window's
/// controls are likewise `UserCommand`s already
/// (`ToggleDebugWindow`/`ClearDebugEntries`/`SetDebugJsonRows`/
/// `SetDebugSearchText`/`SetDebugAutoScroll`), so nothing is duplicated
/// here.
///
/// Two pieces of view-local state back the agent session view, plus the
/// transcript's viewport report:
///
/// * [`Self::ThinkingToggled`] drives the "Thinking Process" disclosure,
///   which iced has no built-in equivalent for — the disclosure is
///   view-local, not application state, so it is deliberately **not** a
///   [`UserCommand`].
/// * [`Self::ScrollReported`] is the transcript-list counterpart of
///   [`LogsMessage::ScrollReported`]: iced exposes scroll geometry only
///   through `Scrollable::on_scroll`
///   (`iced_widget-0.14.2/src/scrollable.rs:164-168`), so the viewport has
///   to reach `update` as a message before the shared
///   [`crate::ui_iced::logs::should_follow`] rule can decide anything. A
///   runtime report, not user intent — the same class of payload
///   [`LayoutMessage::CursorMoved`] carries.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentMessage {
    /// The command-input field text changed while the user types.
    InputChanged {
        /// The field's new contents.
        text: String,
    },
    /// The user submitted the command input (`Enter` without `Shift`).
    /// The reducer parses the intent into one or more [`UserCommand`]s.
    Submit {
        /// The submitted text, exactly as typed (trimming happens in the
        /// parser, not the widget callback).
        text: String,
    },
    /// The "Thinking Process" disclosure was expanded or collapsed (UI-048).
    /// View-local disclosure state.
    ThinkingToggled {
        /// The new expanded state of the thinking block.
        expanded: bool,
    },
    /// The transcript list's viewport geometry changed.
    ///
    /// The reducer stores this and feeds it to
    /// [`crate::ui_iced::logs::should_follow`] together with the
    /// follow-the-stream preference; when the intent is `true` it dispatches
    /// `operation::scrollable::scroll_to` against
    /// [`crate::ui_iced::agent::transcript::AGENT_TRANSCRIPT_SCROLLABLE_ID`]
    /// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`).
    ScrollReported {
        /// How far the content is scrolled down, in logical points.
        offset_y: f32,
        /// Height of the visible viewport, in logical points.
        viewport_height: f32,
        /// Total height of the scrolled content, in logical points.
        content_height: f32,
    },
}

/// Workspace content-search channel (UI-050).
///
/// Backed by `crate::ui::tree_search` (`src/app/ui/tree_search.rs:37-146`):
/// the query text is kept separate from the committed filter so a partial
/// edit never re-scans the workspace.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMessage {
    /// The search-box text changed while typing; does NOT start a scan.
    QueryChanged {
        /// The box's new contents.
        text: String,
    },
    /// The query was committed (`Enter` or the magnifier button) and the
    /// results replace the tree view.
    Submitted {
        /// The committed query text.
        query: String,
    },
    /// The search was cleared (`✕` button) — the tree view returns
    /// (`TreeSearch::clear`).
    Cleared,
    /// The content scan the reducer dispatched has finished.
    ///
    /// The reducer runs the engine
    /// (`crate::ui::tree_search::find_search_results_scoped`) in an iced
    /// `Task` so a large workspace never blocks the UI, and reports the
    /// outcome through this variant, which
    /// [`crate::ui_iced::search::SearchViewModel::apply_results`] then
    /// snapshots — dropping it if a newer committed filter has superseded
    /// the scan.
    ResultsReady {
        /// The term the scan actually ran for, echoed back so the reducer can
        /// detect a superseded (stale) result set.
        query: String,
        /// One entry per matching file, exactly as the reused engine returned
        /// it. Shared so the panel can take a pointer bump per render.
        results: Arc<Vec<SearchResultEntry>>,
    },
}

/// Background-logs panel channel (UI-047).
///
/// The filter row is a text search, a category combo and an auto-scroll
/// checkbox, plus a Clear button. Window visibility is NOT here — it is
/// `UserCommand::ToggleBackgroundLogs`.
///
/// iced exposes scroll geometry only through `Scrollable::on_scroll`
/// (`iced_widget-0.14.2/src/scrollable.rs:164-168`), so the viewport has
/// to reach `update` as a message before the pure
/// [`crate::ui_iced::logs::should_follow`] rule can decide anything. It is
/// a runtime report about pane geometry, not user intent — the same class of
/// payload [`LayoutMessage::CursorMoved`] carries for overlay anchoring.
#[derive(Debug, Clone, PartialEq)]
pub enum LogsMessage {
    /// The free-text log filter changed.
    SearchChanged {
        /// The filter box's new contents.
        text: String,
    },
    /// The category filter changed (`None` = show all categories).
    CategoryFilterChanged(Option<LogCategory>),
    /// The auto-scroll (stick-to-bottom) preference changed.
    AutoScrollToggled(bool),
    /// The Clear button emptied the visible log list.
    Cleared,
    /// The log list's viewport geometry changed.
    ///
    /// The reducer stores this and feeds it to
    /// [`crate::ui_iced::logs::should_follow`] together with the
    /// auto-scroll preference; when the intent is `true` it dispatches
    /// `operation::scrollable::scroll_to` against
    /// [`crate::ui_iced::logs::LOG_SCROLLABLE_ID`]
    /// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`).
    ScrollReported {
        /// How far the content is scrolled down, in logical points.
        offset_y: f32,
        /// Height of the visible viewport, in logical points.
        viewport_height: f32,
        /// Total height of the scrolled content, in logical points.
        content_height: f32,
    },
}

/// Theme / UI-scale channel.
///
/// Minimal by design: UI-002 pins the FastMD dark scheme
/// (`crate::ui::theme::install_dark`, mirrored by
/// [`crate::ui_iced::theme::fastmd_dark_theme`]) and there is no
/// light/dark toggle, so no toggle variant exists. The only
/// runtime-mutable theme state is the user scale multiplier, composed with
/// the OS baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeMessage {
    /// The user-chosen UI scale multiplier changed; the value handed to
    /// `.scale_factor(...)` is `os_baseline × multiplier`, and the
    /// multiplier is what `PersistedUiState::font_size_scale` stores.
    ScaleChanged {
        /// The user multiplier, sanitised to the persisted scale range.
        multiplier: f32,
    },
}

/// Markdown render-path channel.
///
/// Every variant is **deferred intent** in the same sense as the last four
/// [`TreeMessage`] variants: the markdown viewer's links, task checkboxes
/// and copy-code buttons are user intent, but no [`UserCommand`] expresses
/// them atomically and the view may not perform the work itself.
///
/// * **Links** — iced's `Rich` text hands the clicked `Link` payload back
///   as a message, and resolving it needs the workspace file list and
///   content roots — state the pure view does not own. The reducer calls
///   the existing pure [`crate::ui::link_resolver::resolve_link`] and
///   performs the open, the tab selection or the scroll.
/// * **Task checkboxes** — `UserCommand::ApplyTaskToggles` exists but is
///   wired to the *agent transcript* content
///   (`src/app/command_executor.rs:568-576`), not to the open document.
///   The document path therefore travels with the toggle so the reducer can
///   pick the right target.
/// * **Copy code** — iced exposes the clipboard only to `update`
///   (`iced_core-0.14.0/src/clipboard.rs`), so the content rides the
///   message and the reducer writes it.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderMessage {
    /// A markdown link span was clicked; `url` is the raw link target
    /// exactly as the document wrote it (`https://…`, `#anchor`,
    /// `wikilink:…`, a relative path).
    LinkActivated {
        /// Raw link target from the document.
        url: String,
        /// File the link was rendered from, `None` when the text being
        /// rendered is not backed by a file (e.g. an agent transcript).
        source_file: Option<PathBuf>,
    },
    /// A task-list checkbox was toggled in the document viewer; the reducer
    /// writes the change back to that document's markdown.
    TaskToggled {
        /// The document whose markdown carries the checkbox.
        document: PathBuf,
        /// Zero-based index of the task item within that document, the
        /// same ordinal `apply_task_toggle` consumes.
        task_index: usize,
        /// The new checked state.
        checked: bool,
    },
    /// The copy button on a fenced code block was pressed.
    CopyCodeRequested {
        /// The exact code-block content to place on the clipboard.
        content: String,
    },
}

/// Small-dialog channel (UI-011/UI-015).
///
/// The open flag, the typed name and the selected destination are held
/// explicitly in [`crate::ui_iced::dialogs::DialogState`], and this channel
/// is the only way the rendered card mutates it.
///
/// The confirm outcome is deliberately **not** a [`UserCommand`] here: the
/// trimmed name is validated only in the command executor
/// (`src/app/command_executor.rs:308-371` via
/// `utils::path::is_safe_basename`), so the view must not pre-decide
/// validity. The reducer runs
/// [`crate::ui_iced::dialogs::DialogState::submit`], and only a valid
/// [`crate::ui_iced::dialogs::SubmitOutcome::Completed`] becomes the
/// matching `Confirm*` command — the intent stays self-contained and
/// replayable.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogMessage {
    /// The pending name text changed while the user types.
    NameChanged {
        /// The field's new contents.
        text: String,
    },
    /// The confirm button or `Enter` was pressed; the reducer validates
    /// via `DialogState::submit` and either publishes the `Confirm*`
    /// command or sets the dialog's error line.
    Submitted,
    /// The Cancel button was pressed.
    Cancelled,
    /// The name field lost focus (`UIF-PRIN-009`).
    ///
    /// Blur is one of the three commit boundaries the 'Punish Late' pillar
    /// names, so the reducer routes it to
    /// [`crate::ui_iced::dialogs::DialogState::blur`] and the field stops
    /// being pristine from this frame on. It changes no value and closes
    /// nothing — it only changes what the card is allowed to show.
    Blurred,
    /// A press landed inside the card perimeter and the card swallowed it
    /// (`UIF-INP-002`).
    ///
    /// Emitted for the padding ring, a **disabled** confirm button, and
    /// dead space between rows — every press the card's own controls did
    /// not want. The reducer deliberately does nothing with it: the point
    /// of the message is that it is *not* [`Self::Dismissed`], so the dialog
    /// stays open. It exists as a discrete event so the absorption is
    /// observable rather than merely silent ([FMD-071]).
    Absorbed,
    /// The scrim was clicked (or Escape pressed) — the dialog is
    /// dismissed without confirming. Kept distinct from
    /// [`Self::Cancelled`] so the reducer can tell an explicit cancel
    /// from a click-away, the same distinction
    /// [`crate::ui_iced::overlay::OverlaySlot::close`] reports for
    /// Escape consumption.
    Dismissed,
    /// A destination folder row was picked in the move dialog.
    DestinationSelected {
        /// The folder the user picked as the move target.
        path: PathBuf,
    },
}

/// About / attributions window channel.
///
/// This is a **separate channel from [`DialogMessage`]**, on purpose:
/// `DialogMessage`'s vocabulary is name-entry shaped (`NameChanged`,
/// `Submitted`, `DestinationSelected`) and drives
/// [`crate::ui_iced::dialogs::DialogState`]'s validation, none of which
/// an About or credits window has. Merging them would put two unrelated
/// state models behind one message type; the reducer would have to
/// discriminate on payload kind anyway.
///
/// Opening the About window is **not** here — that is user intent and
/// already rides [`Message::Command`] as `UserCommand::OpenAboutDialog`
/// (`src/app/bus/events/user_command.rs:68`), which the hamburger emits
/// (`ui_iced/panels/top.rs:439`). The reducer maps that command onto
/// [`crate::ui_iced::about::AboutWindowState::open_about`]. What lives
/// here is only what the rendered cards can do to *their own* visibility
/// and the side effects iced only allows in `update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AboutMessage {
    /// The "View third-party attributions…" button was pressed in the
    /// About card — show
    /// [`crate::ui_iced::attributions::attributions_view`].
    AttributionsRequested,
    /// The commit-hash cell was clicked; iced exposes the clipboard only to
    /// `update` (`iced_core-0.14.0/src/clipboard.rs`), so the reducer
    /// performs the write. The cell never emits this on an `"unknown"`
    /// build (spec FR-007) — see
    /// [`crate::ui_iced::about::commit_copy_press`].
    CopyCommitHashRequested,
    /// A repository link in the attributions list was clicked; `url` is
    /// the catalog's repository URL verbatim. Deferred intent in the same
    /// sense as [`RenderMessage::LinkActivated`]: a pure view may not open
    /// a browser, so the URL travels with the message and the reducer opens
    /// it.
    RepositoryOpened {
        /// The repository URL to open.
        url: String,
    },
    /// The Close button, a scrim click or Escape — hide whichever identity
    /// window is showing. Kept as one variant because the two windows are
    /// mutually exclusive, so the reducer needs exactly one "hide it"
    /// transition ([`crate::ui_iced::about::AboutWindowState::close`]).
    CloseRequested,
    /// A press landed inside the open card's perimeter but on nothing that
    /// acts — its padding ring, dead space between sections, or a
    /// control with no handler. A deliberate **no-op**: the press is
    /// consumed so it cannot reach the scrim and dismiss the window
    /// (`UIF-VER-003`), and the window stays open.
    ///
    /// It is a message rather than nothing so the absorption is
    /// observable: a test can assert the reducer received it and changed
    /// nothing, which is the difference between "the click was swallowed
    /// by the card" and "the click never arrived"
    /// ([FMD-071]).
    Absorbed,
}

/// Batch prompt-processing dialog channel (BATCH-001..BATCH-014).
///
/// This channel carries **only the dialog's own selection state** — the
/// directory, prompt, wildcard pattern, mode and concurrency the user is
/// composing, held in `BatchDialogConfig`
/// (`src/app/agent/batch/types.rs:224-253`).
///
/// Everything else the batch dialog does already rides [`Message::Command`]
/// and is deliberately **not** duplicated here:
///
/// * open — `UserCommand::OpenBatchDialog` (`user_command.rs:64`);
/// * start — `UserCommand::StartBatch(BatchConfig)` (`:134`), which the
///   Process button emits directly because the composed config *is* the
///   self-contained intent;
/// * cancel / close — `UserCommand::CancelBatch` (`:136`), published by
///   both the Cancel button and the scrim dismiss.
///
/// So unlike [`DialogMessage`], no validation round-trip lives here: the
/// Process button is gated by
/// [`crate::ui_iced::batch_dialog::BatchDialogViewModel::can_start`] at
/// render time, and an invalid composition simply never emits `StartBatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchDialogMessage {
    /// The directory pick-list selection changed (`None` clears it).
    DirectorySelected(Option<usize>),
    /// A prompt-candidate row was picked (`None` clears the selection).
    PromptSelected(Option<usize>),
    /// The wildcard pattern field text changed (File mode only).
    PatternChanged {
        /// The field's new contents.
        text: String,
    },
    /// The File / Directory mode radio changed.
    ModeChanged(crate::agent::batch::types::BatchMode),
    /// The concurrency pick-list selection changed (1..=8).
    ConcurrencyChanged(u8),
    /// A press landed on the card's own boundary — its padding ring, a
    /// disabled `Process` button, or dead space between rows — and was
    /// absorbed there (`UIF-VER-003`).
    ///
    /// The variant exists so the card can be mounted through
    /// [`crate::ui_iced::overlay::modal_centered`], whose `mouse_area`
    /// boundary needs somewhere to send what it captures. Without it a
    /// click on the card's padding would fall through to the scrim and
    /// cancel a batch the user never meant to leave.
    ///
    /// It changes nothing: its whole job is to have arrived.
    Absorbed,
}

/// Tools dialog window channel (UI-051..UI-060).
///
/// Deliberately a **single-variant** channel. Every action the tools dialog
/// performs is already a self-contained [`UserCommand`] —
/// `SetToolGroupEnabled` / `ClearToolGroupError` / `StartMcpAuth` /
/// `ForgetMcpAuth` (`user_command.rs:80-88`) — and the card emits those
/// through [`Message::Command`]. The dialog keeps **no view-local state at
/// all**: no search box, no selection, no expansion.
///
/// The one thing with no [`UserCommand`] equivalent is *closing* it. An
/// in-window iced modal has no title-bar `X`, so the scrim click / Escape
/// needs its own dismiss message. Opening is
/// `UserCommand::OpenToolsDialog` (`:66`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsDialogMessage {
    /// The scrim was clicked or Escape pressed — hide the tools dialog.
    CloseRequested,
    /// A press landed on the card's own boundary — its padding ring, a
    /// disabled action button, or dead space between rows — and was
    /// absorbed there (`UIF-VER-003`).
    ///
    /// Needed so the card can be mounted through
    /// [`crate::ui_iced::overlay::modal_centered`], whose `mouse_area`
    /// boundary needs somewhere to send what it captures. Without it a
    /// click on the card's padding would fall through to the scrim and
    /// close a dialog the user never meant to leave.
    ///
    /// It changes nothing: its whole job is to have arrived.
    Absorbed,
}

/// Agent debug window channel (UI-048).
///
/// The window renders the debug-entry stream
/// (`AgentSession::state().debug_entries`, `src/agent/session.rs:33`).
///
/// This channel carries **only what the command channel cannot express**.
/// Everything else the window does already rides [`Message::Command`]:
///
/// * search text — `UserCommand::SetDebugSearchText` (`user_command.rs:158`);
/// * auto-scroll — `UserCommand::SetDebugAutoScroll` (`:160`);
/// * the JSON row count — `UserCommand::SetDebugJsonRows` (`:156`);
/// * Clear — `UserCommand::ClearDebugEntries` (`:154`);
/// * close — `UserCommand::ToggleDebugWindow(false)` (`:152`), which is
///   literally what the window's dismiss publishes, so no
///   `CloseRequested` variant exists here.
///
/// What is left is the per-entry disclosure state plus the two things iced
/// will not let a view do: copy to the clipboard and report its own
/// viewport geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentDebugMessage {
    /// The clickable header of entry `id` was pressed. `id` is the entry's
    /// index in the raw debug-entry list, and the reducer flips it in
    /// [`crate::ui_iced::agent_debug::AgentDebugWindowState`].
    ///
    /// View-local disclosure, not application state, so it is
    /// deliberately **not** a [`UserCommand`].
    EntryToggled {
        /// The row whose expansion flipped.
        id: usize,
    },
    /// The "Copy JSON" button in an expanded entry body was pressed;
    /// `content` is the exact pretty-printed JSON to place on the
    /// clipboard.
    ///
    /// Deferred intent in the same sense as
    /// [`RenderMessage::CopyCodeRequested`] and
    /// [`AboutMessage::CopyCommitHashRequested`]: iced exposes the
    /// clipboard to `update` only
    /// (`iced_core-0.14.0/src/clipboard.rs`), so the payload travels
    /// with the message and the reducer writes it.
    CopyJsonRequested {
        /// The JSON text to copy.
        content: String,
    },
    /// The entry list's viewport geometry changed.
    ///
    /// The auto-scroll counterpart of [`LogsMessage::ScrollReported`]:
    /// iced exposes scroll geometry only through `Scrollable::on_scroll`
    /// (`iced_widget-0.14.2/src/scrollable.rs:164-168`), so the
    /// viewport has to reach `update` as a message before the shared
    /// [`crate::ui_iced::logs::should_follow`] rule can decide
    /// anything. A runtime report, not user intent — the same class of
    /// payload [`LayoutMessage::CursorMoved`] carries.
    ScrollReported {
        /// How far the content is scrolled down, in logical points.
        offset_y: f32,
        /// Height of the visible viewport, in logical points.
        viewport_height: f32,
        /// Total height of the scrolled content, in logical points.
        content_height: f32,
    },
}

/// Shared floating-menu interaction channel.
///
/// Opening a menu is a **view-local transition** — a menu is floating widget
/// state, not application state — so it must not ride [`Message::Command`]
/// (rule 1 in the module header). The channel carries what the two
/// framework-driven menu surfaces need in order to be driven from the view.
///
/// Since INT-3b this channel serves **both** the top-bar hamburger
/// ([`hamburger`](crate::ui_iced::hamburger)) and the file-tree / tab
/// [context menu](crate::ui_iced::context_menu). One channel is enough
/// because the app keeps the two surfaces mutually exclusive, so
/// [`OverlayState::hamburger_open`](crate::ui_iced::app::OverlayState::hamburger_open)
/// is a complete discriminator for which cascade a message belongs to.
///
/// * [`Self::Opened`] / [`Self::Closed`] — the hamburger's open ⇄ closed
///   bookends. A context menu is opened by its own
///   [`TreeMessage::ContextMenuOpened`] /
///   [`TabMessage::ContextMenuOpened`]
///   instead, because its open carries a target the ☰ button cannot supply.
/// * [`Self::RowPicked`] — the address of a row the user activated.
/// * [`Self::Key`] — a navigation key raised while a menu is mounted.
/// * [`Self::InteractionIgnored`] — a non-navigable row consumed the hit.
///
/// The older variants this enum used to carry (`RowActivated`,
/// `ToggleChanged`, `SubmenuOpened`/`SubmenuClosed` and the
/// context-submenu pair) are **gone**. Each one modelled a transition the
/// app-side menu engine had to be told about by hand; the
/// [`MenuCascade`](iced_pro::surface::menu::cascade::MenuCascade) now owns
/// submenu depth, checked state and close behaviour, so there is nothing
/// left for the view to report.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuMessage {
    /// The ☰ button opened the menu.
    Opened,
    /// The menu was dismissed (click-away) without picking a row.
    Closed,
    /// A row of a framework-driven menu tree was activated.
    ///
    /// Carries the **address**, not the intent: the reducer runs
    /// [`MenuCascade::activate_row`](iced_pro::surface::menu::cascade::MenuCascade::activate_row)
    /// on it and reads the resulting
    /// [`ActivationEffect`](iced_pro::surface::menu::model::ActivationEffect)
    /// to learn both the command and whether the row's
    /// [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy) keeps
    /// the menu open. A checkable row therefore does **not** close the
    /// card, which is the behaviour the previous
    /// address-and-close path could not express.
    RowPicked {
        /// Path of the card holding the row.
        card: MenuId,
        /// Row index within that card, furniture rows included.
        row: usize,
    },
    /// A menu-navigation key was pressed while a menu tree was open.
    ///
    /// Produced only by the keyboard lane's
    /// [`menu_key_to_message`](crate::ui_iced::subscription::menu_key_to_message)
    /// and only while a tree is mounted. `Escape` is deliberately **not**
    /// one of these: dismissal stays on
    /// [`OverlayMessage::EscapePressed`]
    /// → `close_topmost`, so the `UIF-WIN-003` band walk remains the only
    /// Escape path in the app and this increment does not fork it.
    Key(MenuKey),
    /// A disabled or informational row consumed input without changing state.
    InteractionIgnored,
}

/// Floating-overlay / keyboard-modifier channel ([UI-056], [UI-022]).
///
/// Both variants are **runtime keyboard reports** produced by the keyboard
/// lane of [`crate::ui_iced::subscription`], never by a widget: iced's
/// `MouseArea` click callbacks carry no modifier state
/// (`iced_widget-0.14.2/src/mouse_area.rs:38-41`) and its
/// `mouse::Event::ButtonPressed` carries none either
/// (`iced_core-0.14.0/src/mouse/event.rs:26`), so both facts have to be
/// observed off the event stream and folded into
/// [`OverlayState`](crate::ui_iced::app::OverlayState) before the click or
/// dismissal that depends on them is reduced.
///
/// * [`Self::EscapePressed`] closes the **top-most** open modal — one
///   `Esc`, one layer ([UI-056]: "the dialog shall provide a `Close` action
///   (button and `Esc` keypress…) that hides the dialog without persisting
///   further state"). Which layer that is, and why the docked log viewer is
///   excluded, is the rule owned by
///   [`OverlayState::close_topmost`](crate::ui_iced::app::OverlayState::close_topmost);
///   this channel only says "Escape went down".
/// * [`Self::ModifiersChanged`] mirrors the Shift key onto
///   [`OverlayState::modifiers`](crate::ui_iced::app::OverlayState::modifiers)
///   so `LeftPanelViewModel::multi_select` (UI-022) is current when a tree
///   row click is reduced.
///
/// It is **not** [`Message::Command`]: neither report is a request for the
/// *application* to do something (rule 1) — they are statements
/// about the input device, the same class of payload
/// [`LayoutMessage::CursorMoved`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMessage {
    /// `Escape` was pressed while the event was either ignored by a widget
    /// or captured by one (a focused `text_editor` still has to lose the
    /// modal), so the reducer closes the top-most open floating layer.
    EscapePressed,
    /// The tracked Shift modifier state changed.
    ModifiersChanged {
        /// Whether the Shift key is currently held down.
        shift: bool,
    },
}

/// Which inbound bus a notification arrived from.
/// Names the five producer channels the bridge in
/// [`crate::ui_iced::subscription`] reads. The source travels with the
/// lag report so a warning says *which* bus dropped events, as the drain
/// loops name them in their `tracing::warn!` calls
/// (`src/app/orchestrator.rs:97-101`, `:436-440`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusSource {
    /// `Bus<ConfigArrived>` — the one-shot configuration handoff.
    Config,
    /// `Bus<FileChanged>` — file mutations from the watcher, the
    /// initial scan, tools and the agent.
    File,
    /// `Bus<DirectoryChanged>` — directory lifecycle events from the
    /// watcher and the initial scan.
    Directory,
    /// `Bus<AgentEvent>` — the agent's streaming turn events.
    Agent,
    /// The `std::sync::mpsc` background-event channel carrying
    /// [`BackgroundEvent`] (`Fs` / `Process` / `McpAuth`).
    Background,
}

/// Inbound *background → UI* notification channel.
///
/// Every other channel of [`Message`] is produced by a widget. This one is
/// produced only by the subscription bridge
/// ([`crate::ui_iced::subscription::bus_event_to_message`]), which is the
/// single place the iced layer touches a live receiver: the notification
/// runs one way — background → UI — and the UI never publishes back through
/// it; outbound intent rides [`Message::Command`].
///
/// # Why a dedicated channel rather than new `Agent`/`Logs`/`Tree` variants
///
/// The channel rule keeps panel channels for *view-local state* only; a bus
/// notification is neither view-local nor widget-produced.
/// Beyond that rule, the payloads simply could not ride them: `AgentEvent`
/// (`src/app/bus/events/agent.rs:20`), `BackgroundEvent`
/// (`src/app/bus/events/typed.rs:118`), `BackgroundLogEntry`
/// (`src/app/bus/events/messages.rs:63`) and `AppConfig`
/// (`src/app/config/config.rs:81`) derive `Clone` but **not** `PartialEq`,
/// while every panel channel derives it. Adding them would have meant
/// editing the bus — which this task is barred from doing — so the payloads
/// are carried here behind an [`Arc`] and compared by delivery identity.
///
/// # Equality
///
/// `PartialEq` is hand-written (see the impl) because the opaque payloads
/// cannot be compared by value:
///
/// * [`FileChanged`], [`DirectoryChanged`] and the lag report compare by
///   **value** — all support `PartialEq` natively.
/// * `AppConfig`, `AgentEvent` and `BackgroundEvent` compare by
///   **delivery identity** (`Arc::ptr_eq`): two messages are equal iff they
///   carry the same shared notification instance. Cloning a [`Message`]
///   preserves that identity, which is what the "clone must preserve the
///   channel and its payload exactly" rule needs; two independently
///   delivered events are *two deliveries* and never compare equal, which
///   is the correct answer for a dedup check on notifications.
#[derive(Debug, Clone)]
pub enum InboundMessage {
    /// The application configuration has been loaded and published on
    /// `Bus<ConfigArrived>`. The reducer projects it onto the agent config,
    /// content libraries, batch-directory list and inline editor flag as
    /// `AppOrchestrator::drain_config_bus` does
    /// (`src/app/orchestrator.rs:287-331`).
    ConfigArrived(Arc<AppConfig>),
    /// A file change arrived on `Bus<FileChanged>`.
    FileChanged(FileChanged),
    /// A burst of file changes drained off `Bus<FileChanged>` in one
    /// go.
    ///
    /// The bridge coalesces every event already queued when it wakes, so a
    /// watcher storm costs **one** reduction — and one tree rebuild —
    /// instead of one per event. Semantics are exactly the single-event
    /// arm applied in order, with the trailing `process_file_events()`
    /// call hoisted out of the loop.
    FileChangedBatch(Vec<FileChanged>),
    /// A directory lifecycle change arrived on `Bus<DirectoryChanged>`.
    DirectoryChanged(DirectoryChanged),
    /// A burst of directory lifecycle changes drained off
    /// `Bus<DirectoryChanged>` in one go — same semantics as the
    /// [`InboundMessage::DirectoryChanged`] arm applied in order.
    DirectoryChangedBatch(Vec<DirectoryChanged>),
    /// An agent lifecycle or streaming event arrived on
    /// `Bus<AgentEvent>`.
    Agent(Arc<AgentEvent>),
    /// A burst of agent events drained off `Bus<AgentEvent>` in one go —
    /// same semantics as the [`InboundMessage::Agent`] arm applied in
    /// order.
    AgentBatch(Vec<Arc<AgentEvent>>),
    /// A typed background event arrived on the `BackgroundEvent` channel —
    /// indexer/watcher `Fs` progress, a worker `Process` log entry or file
    /// load, or an `McpAuth` completion.
    Background(Arc<BackgroundEvent>),
    /// A burst of typed background events drained off the `BackgroundEvent`
    /// channel in one go — same semantics as the
    /// [`InboundMessage::Background`] arm applied in order.
    BackgroundBatch(Vec<Arc<BackgroundEvent>>),
    /// A subscriber fell behind a bus and `count` events were discarded.
    ///
    /// The count comes off the same tokio broadcast channel the drain loops
    /// read for their `BroadcastRecvError::Lagged(n)` arms
    /// (`src/app/orchestrator.rs:96-103`, `:435-449`). The reducer
    /// surfaces the loss to the user and re-synchronises on the next valid
    /// event.
    EventsDropped {
        /// The bus that discarded events.
        source: BusSource,
        /// How many events were discarded.
        count: u64,
    },
}

/// Field-wise equality for [`InboundMessage`] — see the type-level docs for
/// why this cannot be derived.
impl PartialEq for InboundMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ConfigArrived(a), Self::ConfigArrived(b)) => Arc::ptr_eq(a, b),
            (Self::FileChanged(a), Self::FileChanged(b)) => a == b,
            (Self::FileChangedBatch(a), Self::FileChangedBatch(b)) => a == b,
            (Self::DirectoryChanged(a), Self::DirectoryChanged(b)) => a == b,
            (Self::DirectoryChangedBatch(a), Self::DirectoryChangedBatch(b)) => a == b,
            (Self::Agent(a), Self::Agent(b)) => Arc::ptr_eq(a, b),
            (Self::Background(a), Self::Background(b)) => Arc::ptr_eq(a, b),
            (Self::BackgroundBatch(a), Self::BackgroundBatch(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| Arc::ptr_eq(x, y))
            }
            (
                Self::EventsDropped {
                    source: source_a,
                    count: count_a,
                },
                Self::EventsDropped {
                    source: source_b,
                    count: count_b,
                },
            ) => source_a == source_b && count_a == count_b,
            _ => false,
        }
    }
}
