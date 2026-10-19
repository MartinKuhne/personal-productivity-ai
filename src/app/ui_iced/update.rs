//! The iced `update` reducer ([TASK-023]) — the one-to-one mirror of the
//! the legacy UI `FastMdApp::update_ui` frame body (`src/app/ui/app/update.rs`).
//!
//! This is the single place user intent is executed and runtime reports are
//! folded into [`AppState`]. The panels never publish to the command bus
//! themselves — they emit [`Message::Command`] and this reducer performs the
//! publish + execution ([FMD-068], [FMD-069]).
//!
//! # Frame shape
//!
//! [`update`] is `reduce` then `finish_frame`:
//!
//! 1. the channel-specific reducer mutates state and returns any iced
//!    [`Task`]s the transition produced;
//! 2. `finish_frame` runs the per-frame tail the legacy UI loop ran after the
//!    command drain — the tree-row cache transition (`refresh_tree_cache`),
//!    `AppOrchestrator::handle_file_selection`, the
//!    finished-batch join (`handle_deferred_actions`), the view-model
//!    rebuild, the follow-the-stream scroll tasks and the persisted
//!    panel-width snapshot (`update_persisted_ui_state`).
//!
//! the legacy UI's four per-frame drains (`drain_config_bus`,
//! `drain_background_channel`, `process_file_events`,
//! `drain_agent_event_bus`) are **event-driven** here: they run inside the
//! [`Message::Inbound`] reducer, because in the iced build the only inbound
//! path is the [TASK-019] [`subscription::BusReceivers`](crate::ui_iced::subscription::BusReceivers)
//! bridge ([FMD-065]). `Inbound(FileChanged)` therefore applies the event the
//! message **carries** through `AppOrchestrator::apply_file_event` rather than
//! re-reading a second bus subscriber — the orchestrator's own
//! `file_event_reader` is left inert for exactly that reason.
//!
//! # Command routing
//!
//! [`Message::Command`] is published on the orchestrator's
//! `user_command_bus` and drained/executed through
//! `AppOrchestrator::drain_user_command_bus`, exactly as the legacy UI frame
//! loop did — with three deliberate exceptions handled by `route_command`
//! *before* the executor sees them:
//!
//! * [`UserCommand::Delete`] opens the [TASK-017b1] delete-confirmation
//!   dialog and is **not** executed. The confirmed intent is published later
//!   by the dialog reducer (`reduce_dialog`), so the destructive action stays
//!   self-contained
//!   and replayable ([FMD-068]). the legacy UI deleted without asking; the iced port
//!   asks (new behaviour, recorded as drift in the migration map).
//! * the dialog-opening commands (`Rename`, `Move`, `CreateDirectory`,
//!   `CreateDocument`, `OpenBatchDialog`, `OpenToolsDialog`,
//!   `OpenAboutDialog`) execute normally **and** open the matching iced
//!   window. The iced layer never reads `orchestrator.dialogs.*` back, so
//!   the flags the executor sets are inert and cannot re-open a dismissed
//!   window.
//! * [`UserCommand::OpenInEditor`] executes, then lifts the freshly
//!   populated `TextBuffer` into an [`EditorViewModel`].
//!
//! Unit tests live in the sibling `update_tests.rs` sidecar.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use iced::widget::operation;

use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::BackgroundEvent;
use crate::bus::events::user_command::UserCommand;
use crate::config::AppConfig;
use crate::markdown::document::DocumentContent;
use crate::ui::tree_search::find_search_results_scoped;
use crate::ui_iced::agent::transcript::AGENT_TRANSCRIPT_SCROLLABLE_ID;
use crate::ui_iced::agent_debug::DEBUG_SCROLLABLE_ID;
use crate::ui_iced::app::{AppState, ContextMenu};
use crate::ui_iced::batch_dialog::{BatchDialogViewModel, index_of_selected};
use crate::ui_iced::dialogs::{DialogKind, SubmitOutcome};
use crate::ui_iced::editor::{EditorViewModel, apply_editor_message};
use crate::ui_iced::logs::{self, LOG_SCROLLABLE_ID, ScrollSnapshot};
use crate::ui_iced::message::{
    AboutMessage, AgentDebugMessage, AgentMessage, BatchDialogMessage, BootMessage, BusSource,
    DialogMessage, EditorMessage, InboundMessage, LayoutMessage, LogsMessage, MenuMessage, Message,
    OverlayMessage, RenderMessage, SearchMessage, TabMessage, ThemeMessage, ToolsDialogMessage,
    TreeMessage, WindowMessage,
};
use crate::ui_iced::os_shell;
use crate::ui_iced::panels::bottom::submit_commands;
use crate::ui_iced::panels::left::ROW_TEXT_SIZE;
use crate::ui_iced::persisted;
use crate::ui_iced::table_width::fira_sans_width;

/// NFR-001/NFR-004 error-code name for a failed font registration.
const FONT_LOAD_ERROR_CODE: &str = "ui_iced.boot.font_load_failed";

/// NFR-004 error-code name for a failed deferred-intent file read.
const READ_FAILED_ERROR_CODE: &str = "ui_iced.file.read_failed";

/// NFR-004 error-code name for a print request with no background channel.
const PRINT_NO_CHANNEL_ERROR_CODE: &str = "ui_iced.file.print_no_channel";

/// Top-level iced reducer ([TASK-023]).
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C:
/// Into<Task<Message>>` (`iced-0.14.0/src/application.rs:609-617`).
pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let mut tasks: Vec<Task<Message>> = Vec::new();

    // A context-menu row carries its own intent message ([FMD-068]), so the
    // reducer has to dismiss the menu when it handles one — the way a native
    // dropdown closes on selection. The hamburger card already does this itself
    // in [`reduce_menu`]; the tree and tab menus had no equivalent, which is why
    // they stayed pinned open after a pick.
    if dismisses_context_menu(&message) {
        state.overlay.context_menu = None;
    }

    match message {
        Message::Window(message) => tasks.push(reduce_window(state, message)),
        Message::Layout(message) => reduce_layout(state, message),
        Message::Command(command) => tasks.extend(route_command(state, command)),
        Message::Tree(message) => tasks.extend(reduce_tree(state, message)),
        Message::Tab(message) => reduce_tab(state, message),
        Message::Editor(message) => tasks.extend(reduce_editor(state, message)),
        Message::Agent(message) => reduce_agent(state, message),
        Message::Search(message) => tasks.extend(reduce_search(state, message)),
        Message::Logs(message) => reduce_logs(state, message),
        Message::Theme(message) => reduce_theme(state, message),
        Message::Render(message) => tasks.extend(reduce_render(state, message)),
        Message::Dialog(message) => tasks.extend(reduce_dialog(state, message)),
        Message::About(message) => tasks.extend(reduce_about(state, message)),
        Message::Batch(message) => reduce_batch(state, message),
        Message::Tools(message) => reduce_tools(state, message),
        Message::AgentDebug(message) => tasks.extend(reduce_agent_debug(state, message)),
        Message::Inbound(message) => reduce_inbound(state, message),
        Message::Menu(message) => tasks.extend(reduce_menu(state, message)),
        Message::Overlay(message) => reduce_overlay(state, message),
        Message::Boot(message) => tasks.push(reduce_boot(message)),
    }

    tasks.extend(finish_frame(state));
    Task::batch(tasks)
}

// ───────────────────────────── overlay dismissal ─────────────────────────────

/// Whether `message` is user intent that a floating tree or tab context-menu row
/// can emit, and must therefore dismiss the menu ([FMD-068]).
///
/// The row vocabulary is exactly what the two menu builders produce:
/// [`Message::Command`] for every application intent
/// ([`crate::ui_iced::panels::left::context_menu_items`] and
/// [`crate::ui_iced::panels::center::tab_context_menu_items`]) plus the four
/// [`TreeMessage`] kinds that read files or fan out and so cannot ride a
/// `UserCommand`.
///
/// Anything else — pointer motion, an explicit close, an inbound event — leaves
/// the menu alone. [`TreeMessage::ContextMenuOpened`] and
/// [`TreeMessage::ContextMenuClosed`] are deliberately absent: the first
/// replaces the menu and the second closes it through its own arm.
///
/// [RUST-020]: pure.
#[must_use]
fn dismisses_context_menu(message: &Message) -> bool {
    match message {
        Message::Command(_) => true,
        Message::Tree(message) => matches!(
            message,
            TreeMessage::RunAsPromptRequested { .. }
                | TreeMessage::SkillRunRequested { .. }
                | TreeMessage::PrintRequested { .. }
                | TreeMessage::DeleteSelectedRequested { .. }
        ),
        _ => false,
    }
}

/// Floating-overlay / keyboard-modifier reductions (UI-056, UI-022).
///
/// Both arms mutate the overlay only — no publish, no I/O — so the whole
/// channel returns no [`Task`].
///
/// * [`OverlayMessage::EscapePressed`] delegates the whole "which layer is
///   on top" decision to [`OverlayState::close_topmost`], keeping the
///   reverse-z-order rule in one unit-testable place instead of spread
///   across the reducer. The return value is deliberately dropped here:
///   there is no user-visible difference between "closed the batch dialog"
///   and "closed the context menu", and the layer name is available to any
///   caller that wants it.
/// * [`OverlayMessage::ModifiersChanged`] sets or clears **only** the
///   `SHIFT` bit. Every other modifier bit is left exactly as it is, so
///   widening the tracked set later cannot silently clobber a modifier this
///   lane does not observe.
fn reduce_overlay(state: &mut AppState, message: OverlayMessage) {
    match message {
        OverlayMessage::EscapePressed => {
            state.overlay.close_topmost();
        }
        OverlayMessage::ModifiersChanged { shift } => {
            if shift {
                state
                    .overlay
                    .modifiers
                    .insert(iced::keyboard::Modifiers::SHIFT);
            } else {
                state
                    .overlay
                    .modifiers
                    .remove(iced::keyboard::Modifiers::SHIFT);
            }
        }
    }
}

// ───────────────────────────── frame tail ─────────────────────────────

/// The per-frame tail the legacy UI `update_ui` ran after the drains and before
/// the panels were drawn ([TASK-023]).
///
/// Order matters and mirrors the legacy UI: the tree-row cache is refreshed first so the
/// view-model rebuild reads through the cache instead of re-walking the
/// workspace, the file-selection load is kicked off next so a selection change
/// is in flight before the snapshot is taken, the finished batch is joined
/// after that (it changes the batch status the card renders), and only then is
/// every view-model rebuilt.
fn finish_frame(state: &mut AppState) -> Vec<Task<Message>> {
    let mut tasks = Vec::new();

    refresh_tree_cache(state);
    state.app.orchestrator.handle_file_selection();
    join_finished_batch(state);
    state.rebuild_view_models();
    tasks.extend(follow_scroll_tasks(state));
    sync_persisted_panel_widths(state);

    tasks
}

/// the legacy UI's tree-row cache transition (`ui/panels/left.rs:197-212`): rebuild the
/// flattened rows only while the tree is dirty (or nothing is cached yet), then
/// publish them as the cache and clear the flag.
///
/// Without this the iced build re-walks the whole workspace on **every**
/// message: nothing else in the iced path ever fills
/// [`FastMdApp::cached_tree_rows`], so `panels::left::tree_rows` always falls
/// through to `build_workspace_tree`. Running the transition here also makes the
/// rows the reducer renders the authoritative cache, which is what the
/// context-menu and move-destination reductions read back.
fn refresh_tree_cache(state: &mut AppState) {
    if !state.app.orchestrator.selection.tree_dirty && state.app.cached_tree_rows.is_some() {
        return;
    }
    let rows = crate::ui_iced::panels::left::tree_rows(&state.app);
    state.app.cached_tree_rows = Some(rows);
    state.app.orchestrator.selection.tree_dirty = false;
}

/// the legacy UI `FastMdApp::handle_deferred_actions` — join a batch thread that has
/// finished and drop its cancel flag.
fn join_finished_batch(state: &mut AppState) {
    if let Some(handle) = state.app.orchestrator.dialogs.batch_handle.take() {
        if handle.thread.is_finished() {
            let result = handle.join();
            state.app.orchestrator.dialogs.batch_cancel_flag = None;
            tracing::info!(
                name = "ui_iced.batch.joined",
                result = ?result,
                "Batch completed"
            );
        } else {
            state.app.orchestrator.dialogs.batch_handle = Some(handle);
        }
    }
}

/// Dispatches the stick-to-bottom scroll operations the [`logs::should_follow`]
/// rule decided for each of the three streaming lists ([TASK-023]).
///
/// With no viewport report yet `should_follow` is `false`, so a freshly
/// booted app never scrolls anything blind.
fn follow_scroll_tasks(state: &AppState) -> Vec<Task<Message>> {
    let mut tasks = Vec::new();

    if state.models.logs.follow_intent(state.overlay.logs_scroll) {
        tasks.push(snap_to_end(LOG_SCROLLABLE_ID));
    }
    if state.models.agent.auto_scroll
        && state
            .overlay
            .agent_scroll
            .is_some_and(|snapshot| logs::should_follow(true, Some(snapshot)))
    {
        tasks.push(snap_to_end(AGENT_TRANSCRIPT_SCROLLABLE_ID));
    }
    if state.models.debug.follow_intent(state.overlay.debug_scroll) {
        tasks.push(snap_to_end(DEBUG_SCROLLABLE_ID));
    }

    tasks
}

/// Snaps one scrollable to its end ([TASK-023]).
fn snap_to_end(id: &'static str) -> Task<Message> {
    operation::snap_to_end(id)
}

/// the legacy UI `FastMdApp::update_persisted_ui_state` — keep the persisted
/// snapshot's panel widths in step with the live [`crate::ui::PanelLayout`]
/// so the next close-time save writes current values.
fn sync_persisted_panel_widths(state: &mut AppState) {
    state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
    state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
}

// ───────────────────────────── command routing ─────────────────────────────

/// Publishes `command` on the user-command bus and drains/executes it,
/// exactly as the legacy UI frame loop did ([FMD-066], [FMD-071]).
fn publish_and_drain(state: &mut AppState, command: UserCommand) {
    state.app.orchestrator.user_command_bus.publish(command);
    state.app.orchestrator.drain_user_command_bus();
}

/// Routes a user command: intercept the window-opening and confirmation
/// kinds, publish + execute everything else ([TASK-023]).
///
/// Returns any iced [`Task`]s the routing produced (clipboard writes ride
/// the individual channel reducers; the command path is synchronous).
fn route_command(state: &mut AppState, command: UserCommand) -> Vec<Task<Message>> {
    match command {
        // New in the iced port: deletion is confirmed first. The intent is
        // published by `reduce_dialog` once the user confirms.
        UserCommand::Delete(path) => {
            state
                .overlay
                .dialog
                .open(DialogKind::DeleteConfirm { path });
            state.overlay.dialog_anchor = state.overlay.cursor;
            Vec::new()
        }
        UserCommand::Rename(path) => {
            publish_and_drain(state, UserCommand::Rename(path.clone()));
            state
                .overlay
                .dialog
                .open(DialogKind::Rename { path: path.clone() });
            let initial = crate::ui::tree::flatten::initial_rename_value(
                &path,
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(""),
            );
            state.overlay.dialog.set_name(initial);
            state.overlay.dialog_anchor = state.overlay.cursor;
            Vec::new()
        }
        UserCommand::Move(path) => {
            let destinations = visible_directories(state);
            publish_and_drain(state, UserCommand::Move(path.clone()));
            state.overlay.dialog.open_move(path, destinations);
            state.overlay.dialog_anchor = state.overlay.cursor;
            Vec::new()
        }
        UserCommand::CreateDirectory { parent } => {
            publish_and_drain(
                state,
                UserCommand::CreateDirectory {
                    parent: parent.clone(),
                },
            );
            state
                .overlay
                .dialog
                .open(DialogKind::CreateDirectory { parent });
            state.overlay.dialog_anchor = state.overlay.cursor;
            Vec::new()
        }
        UserCommand::CreateDocument { parent } => {
            publish_and_drain(
                state,
                UserCommand::CreateDocument {
                    parent: parent.clone(),
                },
            );
            state
                .overlay
                .dialog
                .open(DialogKind::CreateDocument { parent });
            state.overlay.dialog_anchor = state.overlay.cursor;
            Vec::new()
        }
        UserCommand::OpenBatchDialog => {
            publish_and_drain(state, UserCommand::OpenBatchDialog);
            let fresh = state.models.batch.clone();
            let model = reindex_batch_selection(state.overlay.batch.model(), fresh);
            state.overlay.batch.open(model);
            Vec::new()
        }
        UserCommand::OpenToolsDialog => {
            publish_and_drain(state, UserCommand::OpenToolsDialog);
            state.overlay.tools.open();
            Vec::new()
        }
        UserCommand::OpenAboutDialog => {
            publish_and_drain(state, UserCommand::OpenAboutDialog);
            state.overlay.about.open_about();
            Vec::new()
        }
        UserCommand::CancelBatch => {
            publish_and_drain(state, UserCommand::CancelBatch);
            state.overlay.batch.close();
            Vec::new()
        }
        UserCommand::ClearDebugEntries => {
            publish_and_drain(state, UserCommand::ClearDebugEntries);
            // Row ids are positional; a stale expansion would open an
            // unrelated new entry after the buffer is wiped.
            state.overlay.debug.collapse_all();
            Vec::new()
        }
        UserCommand::OpenInEditor(path) => {
            publish_and_drain(state, UserCommand::OpenInEditor(path.clone()));
            open_editor_from_buffer(state, &path);
            Vec::new()
        }
        command => {
            publish_and_drain(state, command);
            Vec::new()
        }
    }
}

/// Re-finds the batch dialog's previous directory / prompt picks inside the
/// freshly built candidate lists, so a reopen follows the **value** the user
/// chose rather than a stale positional index (the legacy UI refresh rule,
/// `ui/app/render.rs:102-108`).
///
/// A pick that no longer resolves is deliberately left alone so
/// [`BatchDialogState::open`] drops it: silently sliding the selection onto an
/// unrelated candidate would let a batch run against a directory the user never
/// picked.
///
/// [RUST-020]: pure.
#[must_use]
fn reindex_batch_selection(
    previous: &BatchDialogViewModel,
    mut fresh: BatchDialogViewModel,
) -> BatchDialogViewModel {
    let previous_dir = previous
        .selected_dir_idx
        .and_then(|idx| previous.available_dirs.get(idx));

    if let Some(index) = index_of_selected(previous_dir, &fresh.available_dirs) {
        fresh.selected_dir_idx = Some(index);
    }
    // `PromptInfo` is not `PartialEq`, so the prompt pick is re-found by its
    // `path` — the field that identifies the prompt file.
    if let Some(prompt) = previous.selected_prompt() {
        let path = prompt.path.as_path();
        if let Some(index) = fresh
            .available_prompts
            .iter()
            .position(|candidate| candidate.path.as_path() == path)
        {
            fresh.selected_prompt_idx = Some(index);
        }
    }
    fresh
}

/// Lifts the `TextBuffer` the executor just populated into an
/// [`EditorViewModel`] ([TASK-023]).
///
/// The legacy UI editor read `TextBuffer::content` (the parsed body) plus the
/// preserved front matter straight out of the buffer; rebuilding the raw
/// text through [`DocumentContent`]'s `Display` avoids a second read from
/// disk while giving [`EditorViewModel::open`] exactly the bytes it expects.
fn open_editor_from_buffer(state: &mut AppState, path: &Path) {
    let buffer = &state.app.orchestrator.text_buffer;
    if !buffer.is_open {
        return;
    }
    let raw = DocumentContent {
        front_matter: buffer.front_matter.clone(),
        body: buffer.content.clone(),
    }
    .to_string();
    state.overlay.editor = Some(EditorViewModel::open(path, &raw));
}

/// The directory rows currently visible in the tree — the move dialog's
/// destination candidates and the content search's scope.
///
/// The legacy UI left panel built the same set by walking the built workspace
/// tree (`ui/panels/left.rs:247-249`); the flattened row snapshot is the
/// iced equivalent, so collapsed directories are excluded exactly as they
/// were there.
fn visible_directories(state: &AppState) -> Vec<PathBuf> {
    state
        .models
        .left
        .rows
        .iter()
        .filter(|row| row.is_dir)
        .map(|row| row.path.clone())
        .collect()
}

// ───────────────────────────── window / layout ─────────────────────────────

/// Window-lifecycle reductions ([TASK-009], DEC-006/DEC-008).
fn reduce_window(state: &mut AppState, message: WindowMessage) -> Task<Message> {
    match message {
        WindowMessage::CloseRequested(id) => {
            // DEC-006 phase 1: BEFORE closing, report the current window
            // metrics through the chained runtime tasks so the next message
            // can persist + close in one reduction.
            iced::window::size(id).then(move |size| {
                iced::window::position(id).map(move |position| {
                    Message::Window(WindowMessage::CloseSnapshotCaptured(
                        crate::ui_iced::message::WindowSnapshot { id, size, position },
                    ))
                })
            })
        }
        WindowMessage::CloseSnapshotCaptured(snapshot) => {
            // DEC-006 phase 2: FIRST persist, THEN close. Panel widths are
            // copied from the live `PanelLayout` right before serialising —
            // the faithful port of the legacy UI `save()` sync
            // (`ui/app/mod.rs:251-252`).
            state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
            state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
            persisted::apply_close_metrics(
                &mut state.app.persisted_ui_state,
                snapshot.size,
                snapshot.position,
            );
            match persisted::save_ui_state(state.storage.as_ref(), &state.app.persisted_ui_state) {
                Ok(path) => tracing::info!(?path, "Persisted UI state saved on close"),
                Err(error) => {
                    tracing::error!(
                        name = persisted::UI_STATE_SAVE_ERROR_CODE,
                        %error,
                        "Failed to persist UI state on close"
                    );
                }
            }
            iced::window::close(snapshot.id)
        }
        WindowMessage::ScaleBaseline(baseline) => {
            // DEC-008: store the OS baseline; `.scale_factor(...)` composes
            // it with the persisted multiplier per frame ([FMD-069]).
            state.os_scale = Some(baseline);
            Task::none()
        }
        WindowMessage::Resized { size, .. } => {
            // Keep the logical viewport current ([TASK-010] input).
            state.viewport = size;
            Task::none()
        }
    }
}

/// Pane-geometry and cursor reductions ([TASK-010], [TASK-012]).
fn reduce_layout(state: &mut AppState, message: LayoutMessage) {
    match message {
        LayoutMessage::PaneResized(event) => {
            // Reduce the drag onto the UI-016-clamped widths, replay the
            // CLAMPED ratio into the pane grid, and keep the persisted
            // snapshot consistent for the next close-time save.
            if let Some((split, ratio)) = crate::ui_iced::layout::apply_pane_resize(
                &state.band,
                event,
                state.viewport.width,
                &mut state.app.layout,
            ) {
                state.panes.resize(split, ratio);
                state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
                state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
            }
        }
        LayoutMessage::TreeAutoFit => {
            auto_fit_tree(state);
        }
        LayoutMessage::CursorMoved { point } => {
            state.overlay.cursor = Some(point);
        }
    }
}

/// The legacy UI left panel's auto-fit-on-dirty transition
/// (`ui/panels/left.rs:139-186`), reduced from an explicit
/// [`LayoutMessage::TreeAutoFit`] so the width change is replayable
/// ([TASK-023]).
///
/// The measurement uses the real font-metrics measurer [`fira_sans_width`]
/// ([TASK-015]), which shapes against the same Fira Sans bytes iced renders
/// with (`iced_graphics::text::FIRA_SANS_REGULAR`) rather than a
/// character-count average. Faces we hold no bytes for fall back to
/// `heuristic_width` inside the measurer, so the call is total and never
/// measures the wrong face.
fn auto_fit_tree(state: &mut AppState) {
    let viewport = state.viewport.width;
    if viewport <= 0.0 {
        return;
    }
    let rows = state.models.left.rows.clone();
    let width = crate::ui_iced::panels::left::auto_fit_width(&rows, viewport, |text: &str| {
        fira_sans_width(text, iced::Font::DEFAULT, ROW_TEXT_SIZE)
    });
    state.app.layout.set_width(Some(width));
    state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
    let ratio = width / viewport;
    state.panes.resize(state.band.left_split, ratio);
}

// ───────────────────────────── tree / tab ─────────────────────────────

/// File-tree panel reductions ([TASK-012], [TASK-023]).
fn reduce_tree(state: &mut AppState, message: TreeMessage) -> Vec<Task<Message>> {
    match message {
        TreeMessage::ContextMenuOpened { path } => {
            open_tree_context_menu(state, &path);
            Vec::new()
        }
        TreeMessage::ContextMenuClosed => {
            state.overlay.context_menu = None;
            Vec::new()
        }
        TreeMessage::RunAsPromptRequested { path } => {
            run_file_as_prompt(state, &path);
            Vec::new()
        }
        TreeMessage::SkillRunRequested {
            skill,
            target_dir,
            target_file,
        } => {
            run_skill(state, &skill, target_dir, target_file);
            Vec::new()
        }
        TreeMessage::PrintRequested { path } => vec![print_task(state, path)],
        TreeMessage::PrintFinished { path, result } => {
            if let Err(error) = result {
                tracing::error!(
                    name = PRINT_NO_CHANNEL_ERROR_CODE,
                    path = %path.display(),
                    %error,
                    "Print request could not be handed to the background channel."
                );
            }
            Vec::new()
        }
        TreeMessage::DeleteSelectedRequested { paths } => {
            // legacy parity (`ui/tree/render.rs:108-115`): fan out one
            // `Delete` per path and clear the selection. Deliberately
            // un-confirmed — `DialogKind::DeleteConfirm` carries a single
            // path, so a multi-select cannot route through it. Recorded as
            // an asymmetry in the migration map.
            for path in paths {
                publish_and_drain(state, UserCommand::Delete(path));
            }
            state.app.orchestrator.selection.selected_files.clear();
            Vec::new()
        }
    }
}

/// Builds the row's context-menu rows from the left-panel view-model and
/// anchors them at the last reported cursor position ([TASK-023]).
fn open_tree_context_menu(state: &mut AppState, path: &PathBuf) {
    let anchor = state.overlay.cursor.unwrap_or_default();
    let items = state
        .models
        .left
        .rows
        .iter()
        .find(|row| &row.path == path)
        .map(|row| state.models.left.menu_items_for(row))
        .unwrap_or_default();
    state.overlay.hamburger_open = false;
    state.overlay.context_menu = Some(ContextMenu { anchor, items });
}

/// the legacy UI `RUN_AS_PROMPT_ACTION`: read the file, take the body as the prompt
/// and run it against that file (`ui/tree/render.rs:143-160`).
fn run_file_as_prompt(state: &mut AppState, path: &PathBuf) {
    match std::fs::read_to_string(path) {
        Ok(raw) => {
            let content = DocumentContent::parse(&raw).body;
            publish_and_drain(
                state,
                UserCommand::RunSkillPrompt {
                    content,
                    target_dir: None,
                    target_file: Some(path.clone()),
                },
            );
        }
        Err(error) => {
            tracing::error!(
                name = READ_FAILED_ERROR_CODE,
                path = %path.display(),
                %error,
                "Failed to read file content to run as prompt."
            );
        }
    }
}

/// the legacy UI `publish_skill` (`ui/tree/render.rs:311-337`): read a skill file
/// and run its body against the menu's target.
fn run_skill(
    state: &mut AppState,
    skill: &PathBuf,
    target_dir: Option<PathBuf>,
    target_file: Option<PathBuf>,
) {
    match std::fs::read_to_string(skill) {
        Ok(raw) => {
            let content = DocumentContent::parse(&raw).body;
            publish_and_drain(
                state,
                UserCommand::RunSkillPrompt {
                    content,
                    target_dir,
                    target_file,
                },
            );
        }
        Err(error) => {
            tracing::error!(
                name = READ_FAILED_ERROR_CODE,
                path = %skill.display(),
                %error,
                "Failed to read skill file content to run as prompt."
            );
        }
    }
}

/// the legacy UI `PRINT_ACTION` (`ui/tree/render.rs:161-173`): hand the job to the
/// background channel. There is no `UserCommand::Print`, so the reducer owns it.
///
/// Rendering, the temp-file write and the browser hand-off all block, so they
/// ride an iced [`Task`] instead of running on the UI thread ([FMD-060],
/// [FMD-061]); the outcome comes back as [`TreeMessage::PrintFinished`].
///
/// The laziness of [`Task`] is also what keeps this testable: reducing a
/// [`TreeMessage::PrintRequested`] only *builds* the task, so a unit test never
/// launches a real browser.
fn print_task(state: &AppState, path: PathBuf) -> Task<Message> {
    let tx = state.app.orchestrator.tx.clone();
    let work_path = path.clone();
    Task::perform(
        async move {
            let job = crate::export::print::PrintJob::new(work_path);
            crate::export::print::execute_print_blocking(job, Some(tx))
        },
        move |result| Message::Tree(TreeMessage::PrintFinished { path, result }),
    )
}

/// Tab-strip panel reductions ([TASK-013], [TASK-023]).
fn reduce_tab(state: &mut AppState, message: TabMessage) {
    match message {
        TabMessage::ContextMenuOpened { index } => {
            let anchor = state.overlay.cursor.unwrap_or_default();
            let items = state.models.center.menu_items_for(index);
            state.overlay.hamburger_open = false;
            state.overlay.context_menu = Some(ContextMenu { anchor, items });
        }
        TabMessage::ContextMenuClosed => {
            state.overlay.context_menu = None;
        }
    }
}

/// Top-bar hamburger-menu reductions.
///
/// Opened / Closed only flip [`OverlayState::hamburger_open`]; the view
/// mounts the card from the [`TopBarViewModel`] snapshot
/// ([`crate::ui_iced::panels::top::build_menu_entries`]) anchored under the
/// ☰ button. A row pick publishes the row's own [`UserCommand`] through
/// [`route_command`] and closes the menu in the same reduction — the row's
/// intent stays self-contained ([FMD-068]) and the click lands exactly like
/// a dropdown selection.
fn reduce_menu(state: &mut AppState, message: MenuMessage) -> Vec<Task<Message>> {
    match message {
        MenuMessage::Opened => {
            // The two menus never share the screen: opening the hamburger
            // dismisses any live tree/tab context menu.
            state.overlay.context_menu = None;
            state.overlay.hamburger_open = true;
            Vec::new()
        }
        MenuMessage::Closed => {
            state.overlay.hamburger_open = false;
            Vec::new()
        }
        MenuMessage::RowActivated(command) => {
            state.overlay.hamburger_open = false;
            route_command(state, command)
        }
    }
}

// ───────────────────────────── editor ─────────────────────────────

/// Inline editor reductions ([TASK-014], DEC-007, UI-042..UI-045).
fn reduce_editor(state: &mut AppState, message: EditorMessage) -> Vec<Task<Message>> {
    match message {
        EditorMessage::SaveRequested { path } => {
            save_editor(state, &path);
            Vec::new()
        }
        EditorMessage::CancelRequested => {
            close_editor(state);
            Vec::new()
        }
        other => {
            if let Some(vm) = state.overlay.editor.as_mut() {
                // The changed-flag is not needed: the view-model rebuild at
                // the end of every `update` re-projects the buffer anyway.
                let _changed = apply_editor_message(vm, &other);
            }
            Vec::new()
        }
    }
}

/// the legacy UI `TextBuffer::save` (`ui/text_buffer.rs:329-358`): write the buffer
/// (front matter re-attached), publish the update so the tree and tag
/// manager refresh, then close (UI-044, UI-045).
fn save_editor(state: &mut AppState, path: &PathBuf) {
    let Some(vm) = state.overlay.editor.as_ref() else {
        return;
    };
    if vm.path() != path.as_path() {
        return;
    }
    let full_text = vm.document().to_string();

    if let Err(error) = std::fs::write(path, &full_text) {
        tracing::error!(
            name = "editor.file.save_failed",
            path = %path.display(),
            %error,
            "Failed to save file from inline editor. Likely cause: disk full or permission denied. Operator should verify disk space and write permissions."
        );
        return;
    }

    state
        .app
        .orchestrator
        .file_event_bus
        .publish(FileEvent::updated_one(path.clone()));
    close_editor(state);
}

/// Discards the editor buffer and clears the domain open flags.
///
/// `text_buffer.is_open` and `tabs.loaded_path` both have to go: the file
/// reload rule in `AppOrchestrator::process_file_events`
/// (`orchestrator.rs:144-152`) only drops a stale loaded document while
/// the editor is closed, and the legacy UI's `render.rs:43-45` cleared the same pair.
fn close_editor(state: &mut AppState) {
    state.overlay.editor = None;
    state.app.orchestrator.text_buffer.is_open = false;
    state.app.orchestrator.tabs.loaded_path = None;
}

// ───────────────────────────── agent / search ─────────────────────────────

/// Agent panel and command-input reductions ([TASK-018], UI-041).
fn reduce_agent(state: &mut AppState, message: AgentMessage) {
    match message {
        AgentMessage::InputChanged { text } => {
            state.app.orchestrator.agent_panel_state.command_input = text;
        }
        AgentMessage::Submit { text } => {
            let commands = submit_commands(state.app.agent().state().running, &text);
            for command in commands {
                publish_and_drain(state, command);
            }
        }
        AgentMessage::ThinkingToggled { expanded } => {
            state.overlay.thinking_expanded = expanded;
        }
        AgentMessage::ScrollReported {
            offset_y,
            viewport_height,
            content_height,
        } => {
            state.overlay.agent_scroll = Some(ScrollSnapshot {
                offset_y,
                viewport_height,
                content_height,
            });
        }
    }
}

/// Workspace content-search reductions ([TASK-019], UI-050).
fn reduce_search(state: &mut AppState, message: SearchMessage) -> Vec<Task<Message>> {
    match message {
        SearchMessage::QueryChanged { text } => {
            state.overlay.search.set_query(text);
            Vec::new()
        }
        SearchMessage::Submitted { query } => {
            state.overlay.search.set_query(query);
            let Some(term) = state.overlay.search.submit() else {
                return Vec::new();
            };
            vec![content_scan_task(state, term)]
        }
        SearchMessage::Cleared => {
            state.overlay.search.clear();
            Vec::new()
        }
        SearchMessage::ResultsReady { query, results } => {
            state
                .overlay
                .search
                .apply_results(&query, Arc::unwrap_or_clone(results));
            Vec::new()
        }
    }
}

/// Runs the reused search engine off the UI thread and reports back through
/// [`SearchMessage::ResultsReady`] ([FMD-060], [FMD-061]).
///
/// The scan scope is the currently visible tree directories, matching the
/// the legacy UI panel's `apply_with_allowed_dirs` call
/// (`ui/panels/left.rs:246-253`).
fn content_scan_task(state: &AppState, term: String) -> Task<Message> {
    let all_files = state.app.orchestrator.file_processor.all_files.clone();
    let libraries = state.app.content_libraries().to_vec();
    let allowed: std::collections::HashSet<PathBuf> =
        visible_directories(state).into_iter().collect();
    let term_lower = term.to_lowercase();

    Task::perform(
        async move { find_search_results_scoped(&all_files, &term_lower, &libraries, Some(&allowed)) },
        move |results| {
            Message::Search(SearchMessage::ResultsReady {
                query: term,
                results: Arc::new(results),
            })
        },
    )
}

// ───────────────────────────── logs / theme ─────────────────────────────

/// Background-logs panel reductions ([TASK-016], UI-047).
///
/// The filter lives in the shared `BackgroundLogs` ring buffer, which the
/// the legacy UI viewer also mutated — so the reducer writes it there and the
/// view-model picks it up on the next rebuild.
fn reduce_logs(state: &mut AppState, message: LogsMessage) {
    match message {
        LogsMessage::SearchChanged { text } => {
            if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
                manager.search_text = text;
            }
        }
        LogsMessage::CategoryFilterChanged(category) => {
            if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
                manager.filter_category = category;
            }
        }
        LogsMessage::AutoScrollToggled(enabled) => {
            if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
                manager.auto_scroll = enabled;
            }
        }
        LogsMessage::Cleared => {
            if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
                manager.clear_logs();
            }
        }
        LogsMessage::ScrollReported {
            offset_y,
            viewport_height,
            content_height,
        } => {
            state.overlay.logs_scroll = Some(ScrollSnapshot {
                offset_y,
                viewport_height,
                content_height,
            });
        }
    }
}

/// Theme / UI-scale reductions ([TASK-008], DEC-008).
fn reduce_theme(state: &mut AppState, message: ThemeMessage) {
    match message {
        ThemeMessage::ScaleChanged { multiplier } => {
            // A near-unity or out-of-range multiplier is stored as `None`
            // so a fresh install carries no redundant value and one
            // corrupt entry self-heals (the legacy UI `persist_font_scale`).
            state.app.persisted_ui_state.font_size_scale = sanitise_scale(multiplier);
        }
    }
}

/// the legacy UI `sanitise_font_scale` + the near-unity collapse
/// (`ui/app/update.rs:307-320`), expressed against the same bounds.
fn sanitise_scale(value: f32) -> Option<f32> {
    const MIN: f32 = 0.5;
    const MAX: f32 = 3.0;
    let usable = value.is_finite() && (MIN..=MAX).contains(&value) && (value - 1.0).abs() >= 1e-3;
    usable.then_some(value)
}

// ───────────────────────────── render path ─────────────────────────────

/// Markdown render-path reductions ([TASK-015], [TASK-023]).
fn reduce_render(state: &mut AppState, message: RenderMessage) -> Vec<Task<Message>> {
    match message {
        RenderMessage::LinkActivated { url, source_file } => {
            open_link(state, &url, source_file);
            Vec::new()
        }
        RenderMessage::TaskToggled {
            document,
            task_index,
            checked,
        } => {
            // the legacy UI mutated the open document in memory only
            // (`ui/panels/center.rs:382-383`); no file write.
            if state.app.orchestrator.tabs.loaded_path.as_deref() == Some(document.as_path()) {
                crate::markdown::document::apply_task_toggle(
                    &mut state.app.orchestrator.tabs.current_markdown,
                    task_index,
                    checked,
                );
            }
            Vec::new()
        }
        RenderMessage::CopyCodeRequested { content } => {
            vec![iced::clipboard::write::<Message>(content)]
        }
    }
}

/// the legacy UI `FastMdApp::handle_open_link` (`ui/app/update.rs:115-165`),
/// reduced from a [`RenderMessage::LinkActivated`] instead of an
/// `OutputCommand::OpenUrl` drain.
fn open_link(state: &mut AppState, link: &str, source_file: Option<PathBuf>) {
    let current_file = source_file.or_else(|| state.app.selection().selected_file().cloned());
    let content_roots: Vec<PathBuf> = state
        .app
        .orchestrator
        .content_libraries
        .iter()
        .map(|lib| PathBuf::from(&lib.root_folder))
        .collect();

    let action = crate::ui::resolve_link(
        link,
        current_file.as_deref(),
        &state.app.orchestrator.file_processor.all_files,
        &content_roots,
    );

    match action {
        crate::ui::LinkAction::OpenWorkspaceFile { path, anchor } => {
            let is_md = path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
            });
            if is_md {
                publish_and_drain(state, UserCommand::SelectFile { path, multi: false });
                if let Some(anchor) = anchor {
                    state.app.orchestrator.tabs.scroll_to_header_id = Some(anchor);
                }
            } else {
                os_shell::open_in_system_editor(&path);
            }
        }
        crate::ui::LinkAction::ScrollToAnchor { anchor } => {
            let heading_ids = state.app.orchestrator.tabs.heading_ids().to_vec();
            let matched = crate::ui_iced::panels::right::resolve_anchor(
                &state.app.orchestrator.tabs.toc,
                &heading_ids,
                &anchor,
            );
            state.app.orchestrator.tabs.scroll_to_header_id = Some(matched);
        }
        crate::ui::LinkAction::OpenExternal(url) => {
            os_shell::open_url(&url);
        }
    }
}

// ───────────────────────────── dialogs ─────────────────────────────

/// Small-dialog reductions ([TASK-017b1], UI-011/UI-015).
fn reduce_dialog(state: &mut AppState, message: DialogMessage) -> Vec<Task<Message>> {
    match message {
        DialogMessage::NameChanged { text } => {
            state.overlay.dialog.set_name(text);
            Vec::new()
        }
        DialogMessage::Submitted => {
            match state.overlay.dialog.submit() {
                SubmitOutcome::Completed {
                    kind,
                    name,
                    destination,
                } => {
                    publish_and_drain(state, confirmed_command(&kind, name, destination));
                }
                SubmitOutcome::Invalid { message } => {
                    state.overlay.dialog.set_error(message);
                }
            }
            Vec::new()
        }
        DialogMessage::Cancelled | DialogMessage::Dismissed => {
            state.overlay.dialog.cancel();
            Vec::new()
        }
        DialogMessage::DestinationSelected { path } => {
            state.overlay.dialog.select_destination(path);
            Vec::new()
        }
    }
}

/// The self-contained command a validated dialog submit produces
/// ([FMD-068]).
fn confirmed_command(kind: &DialogKind, name: String, destination: Option<PathBuf>) -> UserCommand {
    match kind {
        DialogKind::CreateDirectory { parent } => UserCommand::ConfirmCreateDirectory {
            parent: parent.clone(),
            name,
        },
        DialogKind::CreateDocument { parent } => UserCommand::ConfirmCreateDocument {
            parent: parent.clone(),
            name,
        },
        DialogKind::Rename { path } => UserCommand::ConfirmRename {
            path: path.clone(),
            new_name: name,
        },
        DialogKind::Move { path } => UserCommand::ConfirmMove {
            file: path.clone(),
            destination: destination.unwrap_or_default(),
        },
        DialogKind::DeleteConfirm { path } => UserCommand::Delete(path.clone()),
    }
}

// ───────────────────────────── about / batch / tools / debug ─────────────────────────────

/// About / attributions window reductions ([TASK-017b2]).
fn reduce_about(state: &mut AppState, message: AboutMessage) -> Vec<Task<Message>> {
    match message {
        AboutMessage::CopyCommitHashRequested => {
            let hash = state.models.about.commit_hash.to_string();
            vec![iced::clipboard::write::<Message>(hash)]
        }
        AboutMessage::RepositoryOpened { url } => {
            os_shell::open_url(&url);
            Vec::new()
        }
        other => {
            state.overlay.about.apply(other);
            Vec::new()
        }
    }
}

/// Batch dialog selection reductions ([TASK-017b3], BATCH-001..BATCH-014).
fn reduce_batch(state: &mut AppState, message: BatchDialogMessage) {
    state.overlay.batch.apply(message);
}

/// Tools dialog dismiss reduction ([TASK-017b3], UI-051..UI-060).
fn reduce_tools(state: &mut AppState, message: ToolsDialogMessage) {
    state.overlay.tools.apply(message);
}

/// Agent debug window reductions ([TASK-017b4], UI-048 / REQ-008).
fn reduce_agent_debug(state: &mut AppState, message: AgentDebugMessage) -> Vec<Task<Message>> {
    match message {
        AgentDebugMessage::CopyJsonRequested { content } => {
            vec![iced::clipboard::write::<Message>(content)]
        }
        AgentDebugMessage::ScrollReported {
            offset_y,
            viewport_height,
            content_height,
        } => {
            state.overlay.debug_scroll = Some(ScrollSnapshot {
                offset_y,
                viewport_height,
                content_height,
            });
            Vec::new()
        }
        other => {
            state.overlay.debug.apply(other);
            Vec::new()
        }
    }
}

// ───────────────────────────── inbound ─────────────────────────────

/// Inbound *background → UI* reductions ([TASK-019], FMD-065..FMD-067).
///
/// These replace the legacy UI's per-frame drains. Each arm runs the same
/// orchestrator routine the frame loop called, triggered by the
/// [TASK-019] bridge message that names the bus instead.
fn reduce_inbound(state: &mut AppState, message: InboundMessage) {
    match message {
        InboundMessage::ConfigArrived(config) => {
            apply_config_arrival(state, &config);
        }
        InboundMessage::FileChanged(event) => {
            // The bridge is the single inbound path ([FMD-065]), so the event
            // the message carries is applied here rather than re-read off a
            // second subscriber. The extra `process_file_events` call keeps the
            // `DirectoryTracker` drain the legacy UI frame ran per frame alive.
            state.app.orchestrator.apply_file_event(event);
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::FileChangedBatch(events) => {
            // Same semantics as the single-event arm, with the tracker drain
            // hoisted out of the loop: the whole burst lands in one tree
            // rebuild ([FMD-060]).
            for event in events {
                state.app.orchestrator.apply_file_event(event);
            }
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::Agent(_) => {
            state.app.orchestrator.drain_agent_event_bus();
        }
        InboundMessage::Background(event) => {
            apply_background_event(state, event.as_ref());
        }
        InboundMessage::BackgroundBatch(events) => {
            for event in &events {
                apply_background_event(state, event.as_ref());
            }
        }
        InboundMessage::EventsDropped { source, count } => {
            handle_bus_lag(state, source, count);
        }
    }
}

/// Applies one typed background event to the orchestrator — the body the
/// single and batched [`InboundMessage::Background`] arms share.
fn apply_background_event(state: &mut AppState, event: &BackgroundEvent) {
    match event {
        BackgroundEvent::Fs(fs_event) => state.app.orchestrator.handle_fs_event(fs_event.clone()),
        BackgroundEvent::Process(process_event) => {
            state
                .app
                .orchestrator
                .handle_process_event(process_event.clone());
        }
        BackgroundEvent::McpAuth(auth_event) => {
            state
                .app
                .orchestrator
                .handle_mcp_auth_event(auth_event.clone());
        }
    }
}

/// The iced equivalent of `AppOrchestrator::drain_config_bus`'s projection
/// arm (`orchestrator.rs:309-331`).
///
/// The orchestrator's own `config_reader` is `None` in the iced build (the
/// bridge owns the inbound path), so the projection the drain would have
/// performed has to be applied from the payload the bridge delivered.
fn apply_config_arrival(state: &mut AppState, config: &AppConfig) {
    let agent_config = config.to_agent_config();
    state.app.agent_mut().set_agent_config(agent_config);
    state.app.orchestrator.content_libraries = config.content_libraries.clone();
    state.app.orchestrator.selection.tree_dirty = true;
    state.app.orchestrator.inline_editor_enabled = config.inline_editor_enabled;
    state
        .app
        .orchestrator
        .dialogs
        .batch_dialog_config
        .available_dirs = config
        .content_libraries
        .iter()
        .map(|lib| PathBuf::from(&lib.root_folder))
        .collect();
    state.app.orchestrator.config = config.clone();
    tracing::info!(
        name = "config.arrived",
        "FastMdApp populated from ConfigArrived event"
    );
}

/// the legacy UI's `BroadcastRecvError::Lagged(n)` arms (`orchestrator.rs:96-103`,
/// `:435-449`): report the loss loudly and, for the agent stream, leave the
/// visible truncation marker the transcript reducer owns.
fn handle_bus_lag(state: &mut AppState, source: BusSource, count: u64) {
    tracing::warn!(
        name = "ui_iced.bus.lagged",
        source = ?source,
        count,
        "Subscriber fell behind its bus; events were discarded"
    );
    if source == BusSource::Agent {
        state
            .app
            .orchestrator
            .agent_transcript
            .thinking
            .push_str(crate::orchestrator::LAG_TRUNCATION_MARKER);
    }
}

/// Boot-infrastructure report reductions ([TASK-007]).
fn reduce_boot(message: BootMessage) -> Task<Message> {
    match message {
        BootMessage::FontLoadFinished(Some(error)) => {
            // NFR-001: a font-registration failure must produce an ERROR
            // log. Non-fatal: iced keeps its default font set ([FMD-064]).
            tracing::error!(name = FONT_LOAD_ERROR_CODE, %error, "Font registration failed");
            Task::none()
        }
        BootMessage::FontLoadFinished(None) => Task::none(),
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
