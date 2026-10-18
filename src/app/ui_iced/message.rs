//! Top-level iced `Message` enum — the single message type driving the
//! FastMD Elm loop ([TASK-021], `doc/planning/iced.md` PAT-001).
//!
//! # Channel design
//!
//! [`Message`] has **one variant per logical interaction channel**. Each
//! channel is its own small sub-enum (`Debug`/`Clone`, plus `Copy` wherever
//! every payload field is `Copy`), so the top level stays flat and readable
//! while each panel owns a vocabulary it can grow independently
//! ([TASK-011]..[TASK-020]).
//!
//! Two routing rules keep the channels from overlapping ([FMD-068]):
//!
//! 1. **User intent rides [`Message::Command`].** Anything the user asked the
//!    *application* to do — open a file, close a tab, run the agent, rename
//!    a path — is already a self-contained [`UserCommand`] carried by the
//!    existing command bus (`src/app/bus/events/user_command.rs`), and the
//!    egui panels publish exactly those. The iced panels keep doing that;
//!    they do NOT re-invent a parallel intent vocabulary per panel.
//! 2. **Panel channels carry view-local state transitions only** — the state
//!    egui kept implicitly in immediate-mode memory (which row's context
//!    menu is open, what the search box currently holds, the editor's
//!    undo request) and the runtime reports the window layer hands us.
//!
//! One deliberate exception to rule 2 lives in [`TreeMessage`]
//! ([TASK-012]): three file-tree context-menu rows — "Run as prompt…",
//! "Print…" and the multi-select "Delete" — have **no single
//! [`UserCommand`] equivalent**, and the egui originals read files from
//! inside the click handler (`src/app/ui/tree/render.rs:143-173`,
//! `:108-115`). A pure view must not do I/O, so those rows emit a
//! *deferred-intent* variant that names the target and leaves the read and
//! the resulting command(s) to the [TASK-022] reducer. They are still user
//! intent — they are here only because the command channel cannot express
//! them atomically.
//!
//! Consequently some channels are deliberately thin: they exist because a
//! panel needs explicit state that egui used to hide in widget memory, and
//! they carry nothing more. Each variant below names the egui behaviour it
//! replaces, so a reviewer can check the grounding against
//! `src/app/ui/`.
//!
//! # Grounding status
//!
//! - **Grounded in shipped behaviour:** `Window`, `Layout`, `Command`,
//!   `Editor`, `Search`, `Logs`, `Agent`, `Tab`, `Tree`, `Render`,
//!   `Dialog` ([TASK-017b1] — the small name-entry / move /
//!   delete-confirmation dialogs; the delete *confirmation* itself is new,
//!   see the `DialogMessage` docs and the migration-map drift note) and
//!   `About` ([TASK-017b2] — the About window plus the third-party
//!   attributions window; the credits list itself is unchanged, only its
//!   placement moved out of the About dialog), `Batch` ([TASK-017b3] — the
//!   batch dialog's composed selection only; open / start / cancel stay on
//!   the command channel), `Tools` ([TASK-017b3] — the tools dialog's
//!   dismiss; every tool-group action stays on the command channel) and
//!   `AgentDebug` ([TASK-017b4] — the agent debug window's disclosure
//!   state plus its two deferred intents; search, auto-scroll, the JSON
//!   row count, Clear and close all stay on the command channel).
//! - **Minimal on purpose:** `Theme` — UI-002 pins the FastMD dark scheme
//!   and no light/dark toggle exists in the egui build, so the only
//!   runtime-mutable theme state is the DEC-008 user scale multiplier.
//! - **Added by [TASK-019]:** `Inbound` — the *background → UI*
//!   notification channel (`ConfigArrived`, `FileEvent`, `AgentEvent`,
//!   `BackgroundEvent`, plus the bus lag report). It is a **dedicated
//!   channel** rather than new variants on `Logs`/`Agent`/`Tree`, for two
//!   reasons documented on [`InboundMessage`]: those panel channels are
//!   widget-only by rule 2 above, and the bus payload enums
//!   (`AgentEvent`, `BackgroundEvent`, `BackgroundLogEntry`, `AppConfig`)
//!   do not implement `PartialEq`, so they cannot ride a channel that
//!   derives it.
//!
//! # How the pilot's messages fold in
//!
//! The `fastmd_iced` pilot's private `Message` enum
//! (`src/app/bin/fastmd_iced.rs:239-267`) maps 1:1 onto the `Window` and
//! `Layout` channels, which is why those variants keep the pilot's shapes:
//!
//! | Pilot variant | This enum |
//! | --- | --- |
//! | `FontLoadFinished(Option<String>)` | `Message::Boot(BootMessage::FontLoadFinished(..))` — [TASK-022] gave the font bootstrap its own boot-infrastructure channel |
//! | `CloseRequested(Id)` | `Message::Window(WindowMessage::CloseRequested(id))` |
//! | `CloseSnapshotCaptured { id, size, position }` | `Message::Window(WindowMessage::CloseSnapshotCaptured(WindowSnapshot { .. }))` |
//! | `ScaleBaseline(f32)` | `Message::Window(WindowMessage::ScaleBaseline(scale))` |
//! | `WindowResized(Size)` | `Message::Window(WindowMessage::Resized { id, size })` — the pilot dropped the window id; `resize_events()` delivers it, so [TASK-022] carries it |
//! | `PaneResized(pane_grid::ResizeEvent)` | `Message::Layout(LayoutMessage::PaneResized(event))` |
//!
//! Unit tests live in the sibling `message_tests.rs` sidecar.

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;

use crate::bus::events::AgentEvent;
use crate::bus::events::file::FileEvent;
use crate::bus::events::messages::LogCategory;
use crate::bus::events::typed::BackgroundEvent;
use crate::bus::events::user_command::UserCommand;
use crate::config::AppConfig;
use crate::ui::tree_search::SearchResultEntry;
use iced::widget::pane_grid;
use std::path::PathBuf;
use std::sync::Arc;

/// The single top-level message of the iced Elm loop ([TASK-021]).
///
/// `Message` is `Clone` but not `Copy`: the [`UserCommand`] payload carries
/// `String`/`PathBuf`/`Vec` fields ([`UserCommand`] derives `Clone, Debug,
/// PartialEq` only), which makes the whole enum non-`Copy`. The individual
/// channel enums are `Copy` wherever their payloads allow — see
/// [`WindowMessage`], [`LayoutMessage`], [`TabMessage`] and
/// [`ThemeMessage`].
///
/// `PartialEq` is derived on every channel (and on this enum) because every
/// payload supports it; message equality is what lets a replay/dedup check —
/// and the [TASK-021] sidecar tests — compare a recorded message against a
/// freshly produced one.
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
    /// File-tree panel channel ([TASK-012]).
    Tree(TreeMessage),
    /// Tab-strip / center panel channel ([TASK-013]).
    Tab(TabMessage),
    /// Inline editor channel ([TASK-014], DEC-007).
    Editor(EditorMessage),
    /// Agent panel / command-input channel ([TASK-018]).
    Agent(AgentMessage),
    /// Workspace content-search channel ([TASK-019], UI-050).
    Search(SearchMessage),
    /// Background-logs panel channel ([TASK-016], UI-047).
    Logs(LogsMessage),
    /// Theme / UI-scale channel ([TASK-008], DEC-008).
    Theme(ThemeMessage),
    /// Markdown render-path channel ([TASK-015]).
    Render(RenderMessage),
    /// Small individual dialogs — create-directory / create-document /
    /// rename / move / delete-confirmation ([TASK-017b1], UI-011/UI-015).
    Dialog(DialogMessage),
    /// The About window and the third-party attributions window
    /// ([TASK-017b2]).
    About(AboutMessage),
    /// The batch prompt-processing dialog's own selection state
    /// ([TASK-017b3], BATCH-001..BATCH-014).
    Batch(BatchDialogMessage),
    /// The tools dialog's window visibility ([TASK-017b3], UI-051..UI-060).
    Tools(ToolsDialogMessage),
    /// The agent debug window's own disclosure state ([TASK-017b4],
    /// UI-048 / REQ-008).
    AgentDebug(AgentDebugMessage),
    /// Inbound *background → UI* notification channel ([TASK-019],
    /// FMD-065..FMD-067) — the only channel no widget can produce.
    Inbound(InboundMessage),
    /// Boot-infrastructure reports ([TASK-022]) — the terminal outcomes of
    /// the one-shot tasks the `boot` closure returns (font registration,
    /// the DEC-009 `ConfigArrived` publish). Not a panel channel and not
    /// user intent: the producer is the boot task chain itself.
    Boot(BootMessage),
}

/// Boot-infrastructure channel ([TASK-022]).
///
/// The `boot` closure returns a batched [`iced::Task`] whose members must
/// report back through the top-level `Message` type. These two variants are
/// those reports. They are deliberately **not** folded into [`WindowMessage`]
/// (font loading and the config handoff are not window concerns) nor into
/// [`InboundMessage`] (they are produced by the boot task chain, not by a
/// background bus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootMessage {
    /// One font collection settled registering ([TASK-007]). `Some` holds
    /// the failure description (logged as an [NFR-001] `ERROR`, non-fatal
    /// per [FMD-064]); `None` means the collection loaded cleanly. This is
    /// the home the pilot's `FontLoadFinished` was promised in the
    /// module-header fold table.
    FontLoadFinished(Option<String>),
    /// The DEC-009 startup `ConfigArrived` publish returned. The publish
    /// runs in a `Task::perform` so background workers observe the arrival
    /// event *after* every subscriber was constructed in `boot`; this
    /// variant is its terminal report (the UI itself was direct-injected
    /// and does not depend on it).
    ConfigPublished,
}

/// Wraps a [`UserCommand`] as the [`Message::Command`] channel value, so a
/// panel can produce intent without naming the wrapper at every call site.
impl From<UserCommand> for Message {
    fn from(command: UserCommand) -> Self {
        Self::Command(command)
    }
}

/// Window-lifecycle channel ([TASK-021]; pilot parity per the module
/// header table).
///
/// Every variant is a *runtime report* from the iced window layer — a
/// subscription or a chained `Task` result — not user intent. The user's
/// decision to save-and-close is application intent and rides
/// [`Message::Command`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowMessage {
    /// The window manager asked to close a window (title-bar close /
    /// `Alt+F4`). With `exit_on_close_request: false` the runtime waits for
    /// the app to persist state and call `iced::window::close`
    /// ([TASK-009], DEC-006).
    CloseRequested(iced::window::Id),
    /// The metrics needed by the close-time snapshot have arrived, reported
    /// through the chained `window::size` → `window::position` tasks
    /// ([TASK-009] (a), DEC-006 phase 2).
    CloseSnapshotCaptured(WindowSnapshot),
    /// The OS-reported baseline scale factor arrived ([TASK-009], DEC-008);
    /// the composed factor is `baseline × user multiplier`.
    ScaleBaseline(f32),
    /// A window changed size ([`iced::window::resize_events`] delivers
    /// `(Id, Size)`; the id is carried so a multi-window runtime can route
    /// the event, which the single-window pilot dropped).
    Resized {
        /// The window that changed size.
        id: iced::window::Id,
        /// Its new logical size in points.
        size: iced::Size,
    },
}

/// The window metrics captured at close time for the DEC-006 standby save
/// ([TASK-009], [TASK-021]).
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

/// Pane-geometry channel ([TASK-021]).
///
/// The three-band `PaneGrid` geometry lives in [TASK-010]
/// ([`crate::ui_iced::layout`]); this channel is how its widget events and
/// its re-fit requests reach `update`.
///
/// It also owns the **pointer position** ([`Self::CursorMoved`],
/// [TASK-012]). That is the honest home for it: like a split drag and the
/// tree auto-fit, a cursor report is a statement about *where in the pane
/// geometry something is*, not about what the user asked the application to
/// do — so it is neither [`Message::Command`] nor any panel's private view
/// state. It has to live somewhere the reducer reads before it opens an
/// overlay, because iced's `MouseArea::on_right_press` takes a fixed
/// `Message` and cannot carry the press point
/// (`iced_widget-0.14.2/src/mouse_area.rs:68-71`), which is exactly the
/// gap [`crate::ui_iced::overlay::context_menu_trigger`] documents.
#[derive(Debug, Clone, Copy)]
pub enum LayoutMessage {
    /// A pane-grid split was dragged ([`pane_grid::ResizeEvent`]:
    /// `split` + `ratio`), fed through `PaneGrid::on_resize`
    /// ([TASK-010]). Reduces onto `PanelLayout` widths under the UI-016
    /// clamps.
    PaneResized(pane_grid::ResizeEvent),
    /// The tree panel should re-fit its width to the longest visible row
    /// (UI-016; the `Message::AutoFitTree` reducer named in [TASK-012]).
    /// In the egui build this ran inline in `show_left_panel` on the
    /// indexing-finished / `left_panel_dirty` transition
    /// (`src/app/ui/panels/left.rs:139-186`); in the Elm loop it is an
    /// explicit message so the width change is replayable.
    TreeAutoFit,
    /// The pointer moved to `point` ([TASK-012], DEC-005).
    ///
    /// The reducer stores this and reads it back when a
    /// [`TreeMessage::ContextMenuOpened`] / [`TabMessage::ContextMenuOpened`]
    /// arrives, then feeds it to
    /// [`crate::ui_iced::overlay::OverlaySlot::open_context_menu`].
    /// `iced::Point` is `Copy` (`iced_core-0.14.0/src/point.rs:7`), so this
    /// variant does not cost the channel its `Copy`.
    ///
    /// Coordinates are **widget-relative** to the element whose
    /// `MouseArea::on_move` produced them
    /// (`mouse_area.rs:358-360`); the overlay must be mounted against that
    /// same element, or the offset added, for the anchor to land correctly.
    CursorMoved {
        /// Pointer position in the reporting widget's coordinate space.
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

/// File-tree panel channel ([TASK-021], [TASK-012]).
///
/// Deliberately thin: row selection and directory expansion are *user
/// intent* and ride [`Message::Command`] as `UserCommand::SelectFile` /
/// `UserCommand::SelectDirectory` — exactly what the egui tree publishes
/// (`src/app/ui/tree/handlers.rs:51-69`). What egui kept implicitly in the
/// `context_menu` widget is explicit state under iced's DEC-005
/// `overlay::menu` construction, so that is what this channel carries.
///
/// The last four variants are **deferred intent** (see the module header):
/// the egui menu performs file I/O inside the click, which a pure view may
/// not do, and no single [`UserCommand`] covers them. Each names its target
/// so the [TASK-022] reducer can perform the read and publish the real
/// command(s) — the intent stays self-contained and replayable
/// ([FMD-068]).
#[derive(Debug, Clone, PartialEq)]
pub enum TreeMessage {
    /// A right-click opened the row context menu for `path` (DEC-005;
    /// [TASK-017] renders the overlay).
    ContextMenuOpened {
        /// The row whose menu is open.
        path: PathBuf,
    },
    /// The row context menu was dismissed (click-away or menu selection).
    ContextMenuClosed,
    /// "Run as prompt…" on a single file row (UI-020). The egui handler
    /// reads the file and publishes `RunSkillPrompt` inline
    /// (`src/app/ui/tree/render.rs:143-160`); here the reducer does it.
    RunAsPromptRequested {
        /// The markdown file whose body becomes the prompt.
        path: PathBuf,
    },
    /// A skill row was picked from a directory or file context menu
    /// (VFS-121 note skills, VFS-122 folder skills). The reducer reads
    /// `skill` and publishes `RunSkillPrompt { content, target_dir,
    /// target_file }` (`src/app/ui/tree/render.rs:311-337`).
    SkillRunRequested {
        /// Path of the skill file to read.
        skill: PathBuf,
        /// Directory the skill runs against — set for folder skills.
        target_dir: Option<PathBuf>,
        /// File the skill runs against — set for note skills.
        target_file: Option<PathBuf>,
    },
    /// "Print…" on a file row (UI-014). There is **no** `UserCommand::Print`;
    /// the egui handler hands the job to the background channel directly
    /// (`src/app/ui/tree/render.rs:161-173`), so the reducer owns it.
    PrintRequested {
        /// The file to print.
        path: PathBuf,
    },
    /// "Delete" on a multi-selected file row (UI-025). egui fans this out
    /// into one `UserCommand::Delete` per path and then clears the
    /// selection (`src/app/ui/tree/render.rs:108-115`); one menu row can
    /// only emit one message, so the whole set travels with it.
    DeleteSelectedRequested {
        /// Every selected path, in the order the panel rendered them.
        paths: Vec<PathBuf>,
    },
}

/// Tab-strip / center panel channel ([TASK-021], [TASK-013]).
///
/// Same split as [`TreeMessage`]: selecting a tab, closing one
/// (`×` / middle-click), closing others/all, copying the path and revealing
/// it in the explorer are all `UserCommand`s already
/// (`src/app/ui/panels/center.rs:208-315`). The tab strip's own view state
/// is which tab's context menu is showing.
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

/// Inline editor channel ([TASK-021], [TASK-014], DEC-007).
///
/// Grounded in the egui editor (`src/app/ui/editor_egui.rs:173-190`):
/// Save / Cancel buttons plus the live buffer, and in DEC-007's rule that
/// every `Action::Edit` is intercepted so the undo history lives at OUR
/// layer (`iced::widget::text_editor` has no native undo/redo).
#[derive(Debug, Clone, PartialEq)]
pub enum EditorMessage {
    /// An [`iced::widget::text_editor::Action`] was performed in the buffer
    /// — the DEC-007 interception point. `Action::is_edit()` separates
    /// content-changing actions (push an undo snapshot) from caret/selection
    /// actions.
    Action(iced::widget::text_editor::Action),
    /// The user asked to save the buffer to `path` (UI-043). The path is
    /// carried so the write target travels with the intent ([FMD-068]).
    SaveRequested {
        /// File the buffer belongs to.
        path: PathBuf,
    },
    /// The user cancelled the editor: discard the buffer, close the overlay
    /// (UI-045).
    CancelRequested,
    /// Replay one snapshot back from the per-tab undo stack (UI-042,
    /// DEC-007). Per-tab stacks make the tab path part of the intent.
    Undo {
        /// The tab whose buffer is being undone.
        path: PathBuf,
    },
    /// Replay one snapshot forward on the per-tab redo stack (UI-042,
    /// DEC-007).
    Redo {
        /// The tab whose buffer is being redone.
        path: PathBuf,
    },
}

/// Agent panel / command-input channel ([TASK-021], [TASK-018]).
///
/// Grounded in `src/app/ui/panels/bottom.rs:226-294` and
/// `AgentPanelState::command_input`. Running, cancelling, queueing a prompt
/// and clearing the input are all `UserCommand`s
/// (`RunAgent` / `CancelAgent` / `QueueAgentPrompt` / `ClearCommandInput`),
/// produced by the reducer after parsing the submitted text with the
/// existing `parse_command_intent` helper. The agent *debug* window's
/// controls are likewise `UserCommand`s already
/// (`ToggleDebugWindow`/`ClearDebugEntries`/`SetDebugJsonRows`/
/// `SetDebugSearchText`/`SetDebugAutoScroll`), so nothing is duplicated
/// here.
///
/// [TASK-018] adds the two pieces of view-local state egui kept in widget
/// memory for the agent session view, plus the transcript's viewport report:
///
/// * [`Self::ThinkingToggled`] replaces the `egui::collapsing` disclosure
///   of the "Thinking Process" block (`src/app/ui/panels/center.rs:143`),
///   which iced has no built-in equivalent for — the disclosure is
///   view-local, not application state, so it is deliberately **not** a
///   [`UserCommand`].
/// * [`Self::ScrollReported`] is the transcript-list counterpart of
///   [`LogsMessage::ScrollReported`]: egui applied `stick_to_bottom(true)`
///   inside its own `ScrollArea`
///   (`src/app/ui/panels/center.rs:140`), whereas iced exposes the geometry
///   only through `Scrollable::on_scroll`
///   (`iced_widget-0.14.2/src/scrollable.rs:164-168`), so the viewport has
///   to reach `update` as a message before the reused
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
    /// The "Thinking Process" disclosure was expanded or collapsed
    /// ([TASK-018], UI-048). View-local disclosure state, replacing egui's
    /// `collapsing` widget memory.
    ThinkingToggled {
        /// The new expanded state of the thinking block.
        expanded: bool,
    },
    /// The transcript list's viewport geometry changed ([TASK-018]).
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

/// Workspace content-search channel ([TASK-021], [TASK-019], UI-050).
///
/// Grounded in `src/app/ui/panels/left.rs:230-265` and
/// `TreeSearch` (`src/app/ui/tree_search.rs:37-146`): the query text is
/// kept separate from the committed filter so a partial edit never
/// re-scans the workspace.
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
    /// The content scan the reducer dispatched has finished ([TASK-019]).
    ///
    /// **New for the iced port.** egui ran the scan synchronously inside
    /// the left panel's paint pass (`src/app/ui/panels/left.rs:246-252`),
    /// so no completion message existed — and a large workspace froze the UI
    /// while it ran, against [FMD-060]/[FMD-061]. Here the reducer runs the
    /// *same* reused engine (`crate::ui::tree_search::find_search_results_scoped`)
    /// in a iced `Task` and reports the outcome through this variant, which
    /// [`crate::ui_iced::search::SearchViewModel::apply_results`] then
    /// snapshots — dropping it if a newer committed filter has superseded
    /// the scan ([FMD-062]).
    ResultsReady {
        /// The term the scan actually ran for, echoed back so the reducer can
        /// detect a superseded (stale) result set.
        query: String,
        /// One entry per matching file, exactly as the reused engine returned
        /// it. Shared so the panel can take a pointer bump per render.
        results: Arc<Vec<SearchResultEntry>>,
    },
}

/// Background-logs panel channel ([TASK-021], [TASK-016], UI-047).
///
/// Grounded in `src/app/ui/background_logs.rs:70-123`: the filter row is
/// a text search, a category combo and an auto-scroll checkbox, plus a
/// Clear button. Window visibility is NOT here — it is
/// `UserCommand::ToggleBackgroundLogs`.
///
/// `ScrollReported` was added by [TASK-016] (the channel brief allowed
/// extending it honestly). egui kept the scroll position inside
/// `ScrollArea`'s own state and applied `stick_to_bottom` there
/// (`egui-0.36.1/src/containers/scroll_area.rs:1282-1284`); iced exposes
/// the geometry only through `Scrollable::on_scroll`
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
    /// The log list's viewport geometry changed ([TASK-016]).
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

/// Theme / UI-scale channel ([TASK-021], [TASK-008], DEC-008).
///
/// Minimal by design: UI-002 pins the FastMD dark scheme
/// (`crate::ui::theme::install_dark`, mirrored by
/// [`crate::ui_iced::theme::fastmd_dark_theme`]) and the egui build has no
/// light/dark toggle, so no toggle variant is invented. The only
/// runtime-mutable theme state is the user scale multiplier that DEC-008
/// composes with the OS baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeMessage {
    /// The user-chosen UI scale multiplier changed (DEC-008); the value
    /// handed to `.scale_factor(...)` is `os_baseline × multiplier`, and the
    /// multiplier is what `PersistedUiState::font_size_scale` stores.
    ScaleChanged {
        /// The user multiplier, sanitised to the persisted scale range.
        multiplier: f32,
    },
}

/// Markdown render-path channel ([TASK-021], [TASK-015]).
///
/// Every variant is **deferred intent** in the same sense as the last four
/// [`TreeMessage`] variants: the markdown viewer's links, task checkboxes
/// and copy-code buttons are user intent, but no [`UserCommand`] expresses
/// them atomically and the view may not perform the work itself.
///
/// * **Links** — the egui renderer called `ui.hyperlink_to`, which pushed
///   `egui::OutputCommand::OpenUrl` onto the platform output
///   (`src/app/ui/render/inline.rs:160-163`, dispatched in
///   `src/app/ui/app/update.rs:73`). iced's `Rich` text hands the clicked
///   `Link` payload back as a message instead, and resolving it needs the
///   workspace file list and content roots — state the pure view does not
///   own. The reducer calls the existing pure
///   [`crate::ui::link_resolver::resolve_link`] and performs the open,
///   the tab selection or the scroll.
/// * **Task checkboxes** — `UserCommand::ApplyTaskToggles` exists but is
///   wired to the *agent transcript* content
///   (`src/app/command_executor.rs:568-576`), not to the open document,
///   which egui mutated in place (`src/app/ui/panels/center.rs:382-383`).
///   The document path therefore travels with the toggle so [TASK-022] can
///   pick the right target.
/// * **Copy code** — egui copied inside the click handler via
///   `Ui::copy_text` (`src/app/ui/render/code.rs:78-83`). iced exposes the
///   clipboard only to `update` (`iced_core-0.14.0/src/clipboard.rs`), so
///   the content rides the message and the reducer writes it.
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
    /// A task-list checkbox was toggled in the document viewer (the P0-2
    /// write-back egui collected into `pending_toggles`).
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

/// Small-dialog channel ([TASK-017b1], UI-011/UI-015).
///
/// Grounded in the egui name-entry / move / delete dialogs
/// (`src/app/ui/modals.rs:26-68`, `:80-158`, `:160-322`) and the
/// `Dialogs` state model (`src/app/ui/dialogs.rs:25-63`). egui kept the
/// open flag, the typed name and the selected destination in the
/// `Dialogs` struct plus single-line widget memory; under iced that state
/// is explicit in [`crate::ui_iced::dialogs::DialogState`] and this
/// channel is the only way the rendered card mutates it.
///
/// The confirm outcome is deliberately **not** a [`UserCommand`] here:
/// the egui build validates the trimmed name only in the command
/// executor (`src/app/command_executor.rs:308-371` via
/// `utils::path::is_safe_basename`), so the view must not pre-decide
/// validity. The [TASK-022]/[TASK-023] reducer runs
/// [`crate::ui_iced::dialogs::DialogState::submit`], and only a valid
/// [`crate::ui_iced::dialogs::SubmitOutcome::Completed`] becomes the
/// matching `Confirm*` command — the intent stays self-contained and
/// replayable ([FMD-068]).
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
    /// The scrim was clicked (or Escape pressed) — the dialog is
    /// dismissed without confirming. Kept distinct from
    /// [`Self::Cancelled`] so the reducer can tell an explicit cancel
    /// from a click-away, the same distinction
    /// [`crate::ui_iced::overlay::OverlaySlot::close`] reports for
    /// Escape consumption.
    Dismissed,
    /// A destination folder row was picked in the move dialog (the
    /// `selectable_label` equivalent of `ui/modals.rs:116-124`).
    DestinationSelected {
        /// The folder the user picked as the move target.
        path: PathBuf,
    },
}

/// About / attributions window channel ([TASK-017b2]).
///
/// Grounded in the egui About dialog (`src/app/ui/about_dialog.rs`) and
/// the attribution catalog it renders (`src/app/ui/attributions.rs`).
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
/// and the two side effects egui performed inside its click handler that
/// iced only allows in `update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AboutMessage {
    /// The "View third-party attributions…" button was pressed in the
    /// About card — show
    /// [`crate::ui_iced::attributions::attributions_view`].
    AttributionsRequested,
    /// The commit-hash cell was clicked. egui copied the full hash with
    /// `ctx.copy_text` inside the click handler
    /// (`src/app/ui/about_dialog.rs:68-77`); iced exposes the clipboard
    /// only to `update` (`iced_core-0.14.0/src/clipboard.rs`), so the
    /// reducer performs the write. The cell never emits this on an
    /// `"unknown"` build (spec FR-007) — see
    /// [`crate::ui_iced::about::commit_copy_press`].
    CopyCommitHashRequested,
    /// A repository link in the attributions list was clicked; `url` is
    /// the catalog's repository URL verbatim. Deferred intent in the same
    /// sense as [`RenderMessage::LinkActivated`]: egui pushed
    /// `egui::OutputCommand::OpenUrl` from inside the click, while a
    /// pure view may not open a browser, so the URL travels with the
    /// message and the reducer opens it.
    RepositoryOpened {
        /// The repository URL to open.
        url: String,
    },
    /// The Close button, a scrim click or Escape — hide whichever identity
    /// window is showing. Kept as one variant because the two windows are
    /// mutually exclusive, so the reducer needs exactly one "hide it"
    /// transition ([`crate::ui_iced::about::AboutWindowState::close`]).
    CloseRequested,
}

/// Batch prompt-processing dialog channel ([TASK-017b3], BATCH-001..BATCH-014).
///
/// Grounded in the egui dialog (`src/app/ui/batch_dialog.rs:65-183`) and the
/// `BatchDialogConfig` state it mutates
/// (`src/app/agent/batch/types.rs:224-253`). This channel carries **only the
/// selection state egui kept in that config** — the directory, prompt,
/// wildcard pattern, mode and concurrency the user is composing.
///
/// Everything else the batch dialog does already rides [`Message::Command`]
/// and is deliberately **not** duplicated here:
///
/// * open — `UserCommand::OpenBatchDialog` (`user_command.rs:64`);
/// * start — `UserCommand::StartBatch(BatchConfig)` (`:134`), which the
///   Process button emits directly because the composed config *is* the
///   self-contained intent ([FMD-068]);
/// * cancel / close — `UserCommand::CancelBatch` (`:136`), which is what
///   the egui dialog published on both its Cancel button and on window
///   close (`ui/batch_dialog.rs:54-61`, `:174-178`).
///
/// So unlike [`DialogMessage`], no validation round-trip lives here: the
/// Process button is gated by
/// [`crate::ui_iced::batch_dialog::BatchDialogViewModel::can_start`] at
/// render time, mirroring egui's `is_config_valid` gate
/// (`ui/batch_dialog.rs:152`, `:232-240`), and an invalid composition
/// simply never emits `StartBatch`.
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
}

/// Tools dialog window channel ([TASK-017b3], UI-051..UI-060).
///
/// Deliberately a **single-variant** channel. Every action the tools dialog
/// performs is already a self-contained [`UserCommand`] —
/// `SetToolGroupEnabled` / `ClearToolGroupError` / `StartMcpAuth` /
/// `ForgetMcpAuth` (`user_command.rs:80-88`) — and the iced card emits
/// those through [`Message::Command`] exactly as the egui dialog published
/// them (`ui/tools_dialog.rs:197-293`). The egui dialog also keeps **no
/// view-local state at all**: no search box, no selection, no expansion.
///
/// The one thing with no [`UserCommand`] equivalent is *closing* it. The
/// egui dialog closed via its title-bar `X` (`ui/tools_dialog.rs:61-63`),
/// a window-frame widget that does not exist for an in-window iced modal,
/// so the scrim click / Escape needs its own dismiss message. Opening is
/// still `UserCommand::OpenToolsDialog` (`:66`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsDialogMessage {
    /// The scrim was clicked or Escape pressed — hide the tools dialog.
    CloseRequested,
}

/// Agent debug window channel ([TASK-017b4], UI-048 / REQ-008).
///
/// Grounded in the egui debug window (`src/app/ui/agent_debug_window.rs`)
/// and the debug-entry stream it renders
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
///   literally what the egui window published when its title-bar `X` was
///   used (`ui/agent_debug_window.rs:149-153`), so no
///   `CloseRequested` variant is invented here.
///
/// What is left is the disclosure state egui kept inside its
/// `CollapsingHeader` widget memory — the only one in the codebase
/// (`ui/agent_debug_window.rs:170`) — plus the two things iced will not
/// let a view do: copy to the clipboard and report its own viewport
/// geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentDebugMessage {
    /// The clickable header of entry `id` was pressed, replacing
    /// `egui::CollapsingHeader`'s built-in toggle. `id` is the entry's
    /// index in the raw debug-entry list — egui's `orig_idx`
    /// (`ui/agent_debug_window.rs:106`) — and the reducer flips it in
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
    /// [`AboutMessage::CopyCommitHashRequested`]: egui copied inside the
    /// click handler with `Ui::copy_text`
    /// (`src/app/ui/agent_debug_window.rs:177-184`), while iced exposes
    /// the clipboard to `update` only
    /// (`iced_core-0.14.0/src/clipboard.rs`), so the payload travels
    /// with the message and the reducer writes it.
    CopyJsonRequested {
        /// The JSON text to copy.
        content: String,
    },
    /// The entry list's viewport geometry changed.
    ///
    /// The auto-scroll counterpart of [`LogsMessage::ScrollReported`]:
    /// egui applied `stick_to_bottom` inside its own `ScrollArea` state
    /// (`ui/agent_debug_window.rs:103`), whereas iced exposes the
    /// geometry only through `Scrollable::on_scroll`
    /// (`iced_widget-0.14.2/src/scrollable.rs:164-168`), so the
    /// viewport has to reach `update` as a message before the reused
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

/// Which inbound bus a notification arrived from ([TASK-019]).
///
/// Names the four producer channels the bridge in
/// [`crate::ui_iced::subscription`] reads. The source travels with the
/// lag report so a warning says *which* bus dropped events, the way the
/// egui drain loops named them in their `tracing::warn!` calls
/// (`src/app/orchestrator.rs:97-101`, `:436-440`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusSource {
    /// `Bus<ConfigArrived>` — the one-shot configuration handoff (DEC-009).
    Config,
    /// `Bus<FileEvent>` — filesystem mutations from the watcher, the
    /// initial scan, tools and the agent.
    File,
    /// `Bus<AgentEvent>` — the agent's streaming turn events.
    Agent,
    /// The `std::sync::mpsc` background-event channel carrying
    /// [`BackgroundEvent`] (`Fs` / `Process` / `McpAuth`).
    Background,
}

/// Inbound *background → UI* notification channel ([TASK-019],
/// FMD-065..FMD-067).
///
/// Every other channel of [`Message`] is produced by a widget. This one is
/// produced only by the subscription bridge
/// ([`crate::ui_iced::subscription::bus_event_to_message`]), which is the
/// single place the iced layer touches a live receiver ([FMD-065]: the
/// notification runs one way — background → UI — and the UI never
/// publishes back through it; outbound intent rides [`Message::Command`]).
///
/// # Why a dedicated channel rather than new `Agent`/`Logs`/`Tree` variants
///
/// The [TASK-021] channel rule keeps panel channels for *view-local state*
/// only; a bus notification is neither view-local nor widget-produced.
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
/// * [`FileEvent`] and the lag report compare by **value** — both support
///   `PartialEq` natively.
/// * `AppConfig`, `AgentEvent` and `BackgroundEvent` compare by
///   **delivery identity** (`Arc::ptr_eq`): two messages are equal iff they
///   carry the same shared notification instance. Cloning a [`Message`]
///   preserves that identity, which is what the [TASK-021]
///   "clone must preserve the channel and its payload exactly" rule needs;
///   two independently delivered events are *two deliveries* and never
///   compare equal, which is the correct answer for a dedup check on
///   notifications.
#[derive(Debug, Clone)]
pub enum InboundMessage {
    /// DEC-009: the application configuration has been loaded and
    /// published on `Bus<ConfigArrived>`. The reducer projects it onto the
    /// agent config, content libraries, batch-directory list and inline
    /// editor flag exactly as `AppOrchestrator::drain_config_bus` does
    /// (`src/app/orchestrator.rs:287-331`).
    ConfigArrived(Arc<AppConfig>),
    /// A filesystem mutation arrived on `Bus<FileEvent>`.
    FileChanged(FileEvent),
    /// An agent lifecycle or streaming event arrived on
    /// `Bus<AgentEvent>`.
    Agent(Arc<AgentEvent>),
    /// A typed background event arrived on the `BackgroundEvent` channel —
    /// indexer/watcher `Fs` progress, a worker `Process` log entry or file
    /// load, or an `McpAuth` completion.
    Background(Arc<BackgroundEvent>),
    /// A subscriber fell behind a bus and `count` events were discarded
    /// ([FMD-067], [FMD-072]).
    ///
    /// Ported from the egui drain loops' `BroadcastRecvError::Lagged(n)`
    /// arms (`src/app/orchestrator.rs:96-103`, `:435-449`), which read
    /// the same count off the same tokio broadcast channel. The reducer
    /// should surface the loss to the user and re-synchronise on the next
    /// valid event, as [FMD-067] requires.
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
            (Self::Agent(a), Self::Agent(b)) => Arc::ptr_eq(a, b),
            (Self::Background(a), Self::Background(b)) => Arc::ptr_eq(a, b),
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
