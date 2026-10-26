//! The iced `update` reducer.
//!
//! This is the single place user intent is executed and runtime reports are
//! folded into [`AppState`]. The panels never publish to the command bus
//! themselves — they emit [`Message::Command`] and this reducer performs the
//! publish + execution.
//!
//! # Frame shape
//!
//! [`update`] is `reduce` then `finish_frame`:
//!
//! 1. the channel-specific reducer mutates state and returns any iced
//!    [`Task`]s the transition produced;
//! 2. `finish_frame` runs the per-frame tail — `refresh_tree_cache`,
//!    `AppOrchestrator::handle_file_selection`, `join_finished_batch`, the
//!    view-model rebuild (`rebuild_view_models`), the follow-the-stream
//!    scroll tasks (`follow_scroll_tasks`) and the persisted panel-width
//!    sync (`sync_persisted_panel_widths`).
//!
//! The four orchestrator drains (`drain_config_bus`,
//! `drain_background_channel`, `process_file_events`,
//! `drain_agent_event_bus`) are **event-driven** here: they run inside the
//! [`Message::Inbound`] reducer, because the only inbound path is the
//! [`subscription::BusReceivers`](crate::ui_iced::subscription::BusReceivers)
//! bridge. `Inbound(FileChanged)` therefore applies the event the message
//! **carries** through `AppOrchestrator::apply_file_changed` rather than
//! re-reading a second bus subscriber — the orchestrator's own
//! `file_event_reader` is left inert for exactly that reason.
//!
//! # Command routing
//!
//! [`Message::Command`] is published on the orchestrator's
//! `user_command_bus` and drained/executed through
//! `AppOrchestrator::drain_user_command_bus`, with three deliberate
//! exceptions handled by `route_command` *before* the executor sees them:
//!
//! * [`UserCommand::Delete`] opens the delete-confirmation dialog and is
//!   **not** executed. The confirmed intent is published later by the dialog
//!   reducer (`reduce_dialog`), so the destructive action stays
//!   self-contained and replayable.
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
use iced_pro::focus::FocusState;
use iced_pro::surface::menu::cascade::{CascadeOutcome, MenuKey, MenuTreeKind};
use iced_pro::surface::menu::model::{Menu, MenuId};
use iced_pro::surface::modal::Recovery;
use iced_pro::surface::movable::PointerPhase;
use iced_pro::surface::movable::SurfaceTarget;
use iced_pro::surface::tooltip::{DismissReason, TimeMs};

use crate::bus::events::typed::{BackgroundEvent, ProcessEvent};
use crate::bus::events::user_command::UserCommand;
use crate::config::AppConfig;
use crate::markdown::document::DocumentContent;
use crate::ui::tree_search::find_search_results_scoped;
use crate::ui_iced::agent::transcript::AGENT_TRANSCRIPT_SCROLLABLE_ID;
use crate::ui_iced::agent_debug::DEBUG_SCROLLABLE_ID;
use crate::ui_iced::app::{AppState, ContextMenu, OverlayState, TooltipId, guard_editor_leave};
use crate::ui_iced::batch_dialog::{BatchDialogViewModel, index_of_selected};
use crate::ui_iced::context_menu::ContextTarget;
use crate::ui_iced::dialogs::{DialogKind, SubmitOutcome};
use crate::ui_iced::editor::{self, EditorViewModel, LeaveGesture, apply_editor_message};
use crate::ui_iced::hamburger::{command_for_activation, hamburger_menu};
use crate::ui_iced::logs::{self, LOG_SCROLLABLE_ID, ScrollSnapshot};
use crate::ui_iced::menu_aim;
use crate::ui_iced::message::{
    AboutMessage, AgentDebugMessage, AgentMessage, BatchDialogMessage, BootMessage, BusSource,
    DialogMessage, EditorMessage, InboundMessage, LayoutMessage, LogsMessage, MenuMessage, Message,
    MovableCard, MovableMessage, OverlayMessage, RenderMessage, SearchMessage, TabMessage,
    ThemeMessage, ToolsDialogMessage, TreeMessage, WindowMessage,
};
use crate::ui_iced::os_shell;
use crate::ui_iced::panels::bottom::submit_commands;
use crate::ui_iced::panels::left::ROW_TEXT_SIZE;
use crate::ui_iced::persisted;
use crate::ui_iced::persisted::UiStateGeometryStorage;
use crate::ui_iced::table_width::fira_sans_width;

/// NFR-001/NFR-004 error-code name for a failed font registration.
const FONT_LOAD_ERROR_CODE: &str = "ui_iced.boot.font_load_failed";

/// NFR-004 error-code name for a print request with no background channel.
const PRINT_NO_CHANNEL_ERROR_CODE: &str = "ui_iced.file.print_no_channel";

/// Top-level iced reducer.
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C:
/// Into<Task<Message>>` (`iced-0.14.0/src/application.rs:609-617`).
pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let mut tasks: Vec<Task<Message>> = Vec::new();

    // A context-menu row carries its own intent message, so the reducer has to
    // dismiss the menu when it handles one — the way a native dropdown closes
    // on selection. The hamburger card already does this itself in
    // [`reduce_menu`]; the tree and tab menus are covered here.
    if dismisses_context_menu(&message) {
        close_context_menu(state);
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
        Message::Movable(message) => tasks.push(reduce_movable(state, message)),
        Message::Boot(message) => tasks.push(reduce_boot(message)),
    }

    tasks.extend(finish_frame(state));
    Task::batch(tasks)
}

// ───────────────────────── movable Band-1 utility cards ──────────────────────

/// Drive one pointer event through a Band-1 movable card's
/// [`MovableHost`] (`UIF-MOV-001`, `UIF-MOV-002`, INT-6).
///
/// The reducer holds no drag logic of its own: it hands the event to the
/// host and lets the framework decide the zone, apply the clamp and settle
/// the geometry. What this layer owns is *which* host, and the press
/// position — see [`MovableMessage`] for why the press reads the last
/// reported position rather than carrying one.
///
/// # Persistence
///
/// The host calls `save` only when a drag or resize **settles**, never per
/// frame, so the write-through to the injected handler is off the hot path
/// ([NFR-002]).
///
/// [RUST-020]: mutates only the host, the registry and the persisted
/// geometry via the injected handler. Publishes nothing.
fn reduce_movable(state: &mut AppState, message: MovableMessage) -> Task<Message> {
    let OverlayState {
        debug_movable,
        logs_movable,
        surfaces,
        utility_focus,
        cursor,
        ..
    } = &mut state.overlay;

    let slot = match message.card_kind() {
        MovableCard::Logs => logs_movable,
        MovableCard::AgentDebug => debug_movable,
    };

    let Some(host) = slot.as_mut() else {
        return Task::none();
    };

    let mut storage =
        UiStateGeometryStorage::new(&mut state.app.persisted_ui_state, state.storage.as_ref());

    // The press position is the last cursor the view reported for this
    // card. `mouse_area::on_press` carries no point in iced 0.14, so the
    // move lane is the only source of a coordinate (see `MovableMessage`).
    let at = cursor.unwrap_or_else(|| host.card().position());

    let (phase, target) = match &message {
        MovableMessage::Moved { .. } => (PointerPhase::Moved, SurfaceTarget::Outside),
        MovableMessage::TitlePressed { .. } => (PointerPhase::Pressed, SurfaceTarget::TitleBar),
        MovableMessage::BodyPressed { .. } => (PointerPhase::Pressed, SurfaceTarget::Content),
        MovableMessage::GripPressed { handle, .. } => {
            (PointerPhase::Pressed, SurfaceTarget::Resize(*handle))
        }
        MovableMessage::DockRequested { .. } => (PointerPhase::Pressed, SurfaceTarget::Dock),
        MovableMessage::CloseRequested { .. } => (PointerPhase::Pressed, SurfaceTarget::Close),
        MovableMessage::Released { .. } => (PointerPhase::Released, SurfaceTarget::Outside),
    };

    let response = host.on_pointer(phase, at, target, utility_focus, surfaces, &mut storage);

    // A `Close ×` on a Band-1 card closes the card, but the card's
    // visibility is application state the host does not own. Rather than
    // let the host unmount a card the view-model still reports visible, the
    // close is turned back into the same `UserCommand` the menu entry uses
    // ([FMD-068]); `sync_movable_hosts` then drops the host on the next
    // rebuild, so there is one path out and not two.
    if matches!(message, MovableMessage::CloseRequested { .. }) && !response.stays_open() {
        return close_command(message.card_kind());
    }

    Task::none()
}

/// The [`UserCommand`](crate::bus::events::user_command::UserCommand) that
/// hides a movable card, so the title-bar `Close ×` and the menu entry take
/// exactly the same path ([FMD-068]).
///
/// Deferred by one frame through `Task::perform` on a ready future because
/// the borrow of `state` held by [`reduce_movable`] must end before the
/// command can run against the whole app.
fn close_command(card: MovableCard) -> Task<Message> {
    let command = match card {
        MovableCard::Logs => UserCommand::ToggleBackgroundLogs(false),
        MovableCard::AgentDebug => UserCommand::ToggleDebugWindow(false),
    };
    Task::perform(std::future::ready(()), move |_| Message::Command(command))
}

// ───────────────────────────── overlay dismissal ─────────────────────────────

/// Whether `message` is user intent that must dismiss a floating tree or tab
/// context menu even though it did not come from one of the menu's own rows.
///
/// Since INT-3b a row pick no longer arrives here: the row emits
/// [`MenuMessage::RowPicked`](MenuMessage::RowPicked) and the cascade's own
/// [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy) closes the
/// tree, so the menu's dismissal is framework-driven. This pre-pass covers the
/// **other** half of the native-dropdown behaviour — an intent raised from
/// somewhere else (a keyboard shortcut, a panel button) while a menu happens
/// to be open still takes the menu down rather than leaving it hanging over
/// its own result.
///
/// The vocabulary is what the two menu builders produce: [`Message::Command`]
/// for every application intent
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
            // Tooltip dismissal is a pre-pass before z-band escape handling.
            // A tooltip must dismiss itself on Escape and still let the
            // keyboard owner underneath have the key.
            let tooltip = &mut state.overlay.tooltip;
            let _now = TimeMs::now();
            let mut dismissed = false;

            if tooltip.search_trigger_controller.is_active() {
                let _ = tooltip
                    .search_trigger_controller
                    .dismiss(DismissReason::Escape);
                dismissed = true;
            }
            if tooltip.search_clear_controller.is_active() {
                let _ = tooltip
                    .search_clear_controller
                    .dismiss(DismissReason::Escape);
                dismissed = true;
            }
            if tooltip.tools_error_controller.is_active() {
                let _ = tooltip
                    .tools_error_controller
                    .dismiss(DismissReason::Escape);
                dismissed = true;
            }

            if dismissed {
                tooltip.active_tooltip = None;
                tooltip.tools_error_tooltip = None;
                tooltip.tools_error_anchor = None;
                // Suppress until pointer exits - the controllers' suppress flag handles this
            }

            // Then proceed with normal z-band escape handling
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

/// The per-frame tail that runs after the reduction and before the panels are
/// drawn.
///
/// Order matters: the tree-row cache is refreshed first so the view-model
/// rebuild reads through the cache instead of re-walking the workspace, the
/// file-selection load is kicked off next so a selection change is in flight
/// before the snapshot is taken, the finished batch is joined after that (it
/// changes the batch status the card renders), and only then is every
/// view-model rebuilt.
fn finish_frame(state: &mut AppState) -> Vec<Task<Message>> {
    let mut tasks = Vec::new();

    refresh_tree_cache(state);
    prune_hovered_row(state);
    state.app.orchestrator.handle_file_selection();
    join_finished_batch(state);

    // Tooltip subsystem: tick all controllers with the current frame time.
    // This drives the hover-delay state machines and updates visibility.
    let now = TimeMs::now();
    let tooltip = &mut state.overlay.tooltip;
    let search_trigger_vis = tooltip.search_trigger_controller.tick(now);
    let search_clear_vis = tooltip.search_clear_controller.tick(now);
    let tools_error_vis = tooltip.tools_error_controller.tick(now);

    // If any tooltip is visible, note it in the warm session for cross-control
    // warm handoff (100ms window per time.delay.tooltip_warm).
    if search_trigger_vis.is_visible() {
        tooltip.warm_session.note_visible(now);
    }
    if search_clear_vis.is_visible() {
        tooltip.warm_session.note_visible(now);
    }
    if tools_error_vis.is_visible() {
        tooltip.warm_session.note_visible(now);
    }

    state.rebuild_view_models();
    tasks.extend(follow_scroll_tasks(state));
    sync_persisted_panel_widths(state);

    tasks
}

/// Drop a hover pointer that no longer names a row the tree can render
/// (INT-10).
///
/// Two mechanisms already cover most of this: the row's own `mouse_area`
/// publishes `RowLeft` when the pointer crosses out, and iced re-evaluates
/// hover whenever a row's **bounds** move
/// (`iced_widget-0.14.2/src/mouse_area.rs:342-366`), so a row that
/// shifts under a stationary pointer still reports leaving. What neither
/// covers is the row that simply stops existing — deleted, filtered out,
/// or hidden because the search-result view replaced the tree. Its
/// `mouse_area` is gone before it can say goodbye and the recorded path
/// lingers.
///
/// A lingering path cannot paint a *wrong* highlight: the view compares
/// the hovered path against each row's own file, so a stale path matches
/// nothing. It is pruned anyway so the invariant the tests pin is the
/// strong one — `hovered_tree_row` is `None`, or names a row that is
/// actually renderable — rather than a weak one that only holds by
/// accident of the comparison.
///
/// Runs after [`refresh_tree_cache`] so it reads the rows the panel is
/// about to draw, and before the view-model rebuild that copies the field
/// into the snapshot.
///
/// Purity: mutates only `state.overlay.hovered_tree_row`.
fn prune_hovered_row(state: &mut AppState) {
    let Some(hovered) = state.overlay.hovered_tree_row.as_deref() else {
        return;
    };
    let rows = crate::ui_iced::panels::left::tree_rows(&state.app);
    if !rows.iter().any(|row| row.path.as_path() == hovered) {
        state.overlay.hovered_tree_row = None;
    }
}

/// Tree-row cache transition: rebuild the flattened rows only while the tree
/// is dirty (or nothing is cached yet), then publish them as the cache and
/// clear the flag.
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

/// Join a batch thread that has finished and drop its cancel flag.
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
/// rule decided for each of the three streaming lists.
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

/// Snaps one scrollable to its end.
fn snap_to_end(id: &'static str) -> Task<Message> {
    operation::snap_to_end(id)
}

/// Keep the persisted snapshot's panel widths in step with the live
/// [`crate::ui::PanelLayout`] so the next close-time save writes current
/// values.
fn sync_persisted_panel_widths(state: &mut AppState) {
    state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
    state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
}

// ───────────────────────────── command routing ─────────────────────────────

/// Publishes `command` on the user-command bus and drains/executes it.
fn publish_and_drain(state: &mut AppState, command: UserCommand) {
    state.app.orchestrator.user_command_bus.publish(command);
    state.app.orchestrator.drain_user_command_bus();
}

/// Routes a user command: intercept the window-opening and confirmation
/// kinds, publish + execute everything else.
///
/// Returns any iced [`Task`]s the routing produced (clipboard writes ride
/// the individual channel reducers; the command path is synchronous).
fn route_command(state: &mut AppState, command: UserCommand) -> Vec<Task<Message>> {
    match command {
        // Deletion is confirmed first. The intent is published by
        // `reduce_dialog` once the user confirms.
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
            let prompt_config = state.app.orchestrator.config.clone();
            let fresh = BatchDialogViewModel::from_config(
                &state.app.orchestrator.dialogs.batch_dialog_config,
            )
            .with_status(state.models.batch.status);
            let model = reindex_batch_selection(state.overlay.batch.model(), fresh);
            state.overlay.batch.open(model);
            vec![batch_prompt_discovery_task(
                prompt_config,
                state.overlay.batch.prompt_load_generation(),
            )]
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
/// chose rather than a stale positional index.
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

/// Discovers prompt candidates on iced's task executor, keeping library I/O
/// off the UI thread (FMD-060..061).
fn batch_prompt_discovery_task(config: crate::config::AppConfig, generation: u64) -> Task<Message> {
    Task::perform(
        async move { crate::agent::batch::prompts::discover_prompts(&config) },
        move |prompts| {
            Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
                generation,
                prompts,
            })
        },
    )
}

/// Lifts the `TextBuffer` the executor just populated into an
/// [`EditorViewModel`].
///
/// `TextBuffer::content` holds the parsed body and the front matter is
/// preserved alongside it; rebuilding the raw text through
/// [`DocumentContent`]'s `Display` avoids a second read from disk while
/// giving [`EditorViewModel::open`] exactly the bytes it expects.
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
/// Built from the flattened row snapshot, so collapsed directories are
/// excluded.
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

/// Window-lifecycle reductions.
fn reduce_window(state: &mut AppState, message: WindowMessage) -> Task<Message> {
    match message {
        WindowMessage::CloseRequested(id) => {
            // Phase 1: BEFORE closing, report the current window
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
            // Phase 2: FIRST persist, THEN close. Panel widths are
            // copied from the live `PanelLayout` right before serialising.
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
            // Store the OS baseline; `.scale_factor(...)` composes
            // it with the persisted multiplier per frame.
            state.os_scale = Some(baseline);
            Task::none()
        }
        WindowMessage::Resized { size, .. } => {
            // Keep the logical viewport current.
            state.viewport = size;
            Task::none()
        }
    }
}

/// Pane-geometry and cursor reductions.
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
        LayoutMessage::TooltipHoverEnter { id, tooltip_text } => {
            let now = TimeMs::now();
            match id {
                TooltipId::SearchTrigger => {
                    state
                        .overlay
                        .tooltip
                        .search_trigger_controller
                        .pointer_enter_warm(
                            now,
                            state.overlay.tooltip.warm_session.warmth_for(now),
                        );
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::SearchTrigger);
                }
                TooltipId::SearchClear => {
                    state
                        .overlay
                        .tooltip
                        .search_clear_controller
                        .pointer_enter_warm(
                            now,
                            state.overlay.tooltip.warm_session.warmth_for(now),
                        );
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::SearchClear);
                }
                TooltipId::ToolsError => {
                    state
                        .overlay
                        .tooltip
                        .tools_error_controller
                        .pointer_enter_warm(
                            now,
                            state.overlay.tooltip.warm_session.warmth_for(now),
                        );
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::ToolsError);
                    if let Some(text) = tooltip_text {
                        state.overlay.tooltip.tools_error_tooltip = Some(text);
                    }
                }
            }
        }
        LayoutMessage::TooltipHoverExit { id } => match id {
            TooltipId::SearchTrigger => {
                state
                    .overlay
                    .tooltip
                    .search_trigger_controller
                    .pointer_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::SearchTrigger) {
                    state.overlay.tooltip.active_tooltip = None;
                }
            }
            TooltipId::SearchClear => {
                state.overlay.tooltip.search_clear_controller.pointer_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::SearchClear) {
                    state.overlay.tooltip.active_tooltip = None;
                }
            }
            TooltipId::ToolsError => {
                state.overlay.tooltip.tools_error_controller.pointer_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::ToolsError) {
                    state.overlay.tooltip.active_tooltip = None;
                }
                state.overlay.tooltip.tools_error_tooltip = None;
                state.overlay.tooltip.tools_error_anchor = None;
            }
        },
        LayoutMessage::TooltipFocusEnter { id, tooltip_text } => {
            let now = TimeMs::now();
            match id {
                TooltipId::SearchTrigger => {
                    state
                        .overlay
                        .tooltip
                        .search_trigger_controller
                        .keyboard_focus_enter(now);
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::SearchTrigger);
                }
                TooltipId::SearchClear => {
                    state
                        .overlay
                        .tooltip
                        .search_clear_controller
                        .keyboard_focus_enter(now);
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::SearchClear);
                }
                TooltipId::ToolsError => {
                    state
                        .overlay
                        .tooltip
                        .tools_error_controller
                        .keyboard_focus_enter(now);
                    state.overlay.tooltip.active_tooltip = Some(TooltipId::ToolsError);
                    if let Some(text) = tooltip_text {
                        state.overlay.tooltip.tools_error_tooltip = Some(text);
                    }
                }
            }
        }
        LayoutMessage::TooltipFocusExit { id } => match id {
            TooltipId::SearchTrigger => {
                state
                    .overlay
                    .tooltip
                    .search_trigger_controller
                    .keyboard_focus_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::SearchTrigger) {
                    state.overlay.tooltip.active_tooltip = None;
                }
            }
            TooltipId::SearchClear => {
                state
                    .overlay
                    .tooltip
                    .search_clear_controller
                    .keyboard_focus_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::SearchClear) {
                    state.overlay.tooltip.active_tooltip = None;
                }
            }
            TooltipId::ToolsError => {
                state
                    .overlay
                    .tooltip
                    .tools_error_controller
                    .keyboard_focus_exit();
                if state.overlay.tooltip.active_tooltip == Some(TooltipId::ToolsError) {
                    state.overlay.tooltip.active_tooltip = None;
                }
                state.overlay.tooltip.tools_error_tooltip = None;
                state.overlay.tooltip.tools_error_anchor = None;
            }
        },
    }
}

/// The left panel's auto-fit-on-dirty transition, reduced from an explicit
/// [`LayoutMessage::TreeAutoFit`] so the width change is replayable.
///
/// The measurement uses the real font-metrics measurer [`fira_sans_width`],
/// which shapes against the same Fira Sans bytes iced renders
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

/// File-tree panel reductions.
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
        TreeMessage::RowEntered { path } => {
            state.overlay.hovered_tree_row = Some(path);
            Vec::new()
        }
        TreeMessage::RowLeft { path } => {
            // Conditional clear. A row-to-row move publishes both halves
            // in one iced event pass in tree order, so moving *upward*
            // delivers the lower row's `RowEntered` before this row's
            // `RowLeft`. Clearing unconditionally would erase the new
            // row's highlight in the same tick it appeared.
            if state.overlay.hovered_tree_row.as_deref() == Some(path.as_path()) {
                state.overlay.hovered_tree_row = None;
            }
            Vec::new()
        }
        TreeMessage::RunAsPromptRequested { path } => {
            let _queued = state.app.orchestrator.queue_user_file_operation(
                crate::background::UserFileOperation::LoadPrompt {
                    path: path.clone(),
                    target_dir: None,
                    target_file: Some(path.clone()),
                },
            );
            Vec::new()
        }
        TreeMessage::SkillRunRequested {
            skill,
            target_dir,
            target_file,
        } => {
            let _queued = state.app.orchestrator.queue_user_file_operation(
                crate::background::UserFileOperation::LoadPrompt {
                    path: skill,
                    target_dir,
                    target_file,
                },
            );
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
            // Fan out one `Delete` per path and clear the selection.
            // Deliberately un-confirmed — `DialogKind::DeleteConfirm` carries
            // a single path, so a multi-select cannot route through it.
            for path in paths {
                publish_and_drain(state, UserCommand::Delete(path));
            }
            state.app.orchestrator.selection.selected_files.clear();
            Vec::new()
        }
    }
}

/// Builds the row's context-menu rows from the left-panel view-model and
/// anchors them at the last reported cursor position (`UIF-CMENU-001`).
///
/// The hamburger tree is closed **through the cascade** rather than by
/// clearing its `hamburger_open` bool: `close_tree` pops the focus trap the
/// cascade pushed on open, so the two menu surfaces cannot leave a stale trap
/// behind in [`OverlayState::menu_focus`].
fn open_tree_context_menu(state: &mut AppState, path: &PathBuf) {
    let anchor = state.overlay.cursor.unwrap_or_default();
    let viewport = state.viewport;
    let opened = state
        .models
        .left
        .rows
        .iter()
        .find(|row| &row.path == path)
        .map(|row| {
            (
                state.models.left.menu_target_for(row),
                state.models.left.menu_items_for(row),
            )
        });

    close_hamburger_tree(state);
    state.overlay.context_menu =
        opened.and_then(|(target, rows)| ContextMenu::open(target, anchor, &rows, viewport));
}

/// Takes the hamburger tree down completely: cascade, focus trap, bool and
/// the registered card geometry.
///
/// Every path that hands the menu slot to another surface goes through here so
/// the cascade's trap cannot outlive the card it was pushed for.
fn close_hamburger_tree(state: &mut AppState) {
    state
        .overlay
        .hamburger_cascade
        .close_tree(&mut state.overlay.menu_focus);
    state.overlay.hamburger_open = false;
    state.overlay.hamburger_rects.clear();
}

/// Takes the context-menu surface down: cascade, focus trap and routing table
/// all live inside [`ContextMenu`], so dropping the slot drops them together.
fn close_context_menu(state: &mut AppState) {
    state.overlay.context_menu = None;
}

/// Hand the print job to the background channel. There is no
/// `UserCommand::Print`, so the reducer owns it.
///
/// Rendering, the temp-file write and the browser hand-off all block, so they
/// ride an iced [`Task`] instead of running on the UI thread; the outcome
/// comes back as [`TreeMessage::PrintFinished`].
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

/// Tab-strip panel reductions.
fn reduce_tab(state: &mut AppState, message: TabMessage) {
    match message {
        TabMessage::ContextMenuOpened { index } => {
            let anchor = state.overlay.cursor.unwrap_or_default();
            let viewport = state.viewport;
            let opened = state.models.center.tabs.get(index).map(|tab| {
                (
                    ContextTarget::Tab {
                        index,
                        path: tab.path.clone(),
                    },
                    state.models.center.menu_items_for(index),
                )
            });

            close_hamburger_tree(state);
            state.overlay.context_menu = opened
                .and_then(|(target, rows)| ContextMenu::open(target, anchor, &rows, viewport));
        }
        TabMessage::ContextMenuClosed => {
            close_context_menu(state);
        }
        TabMessage::Entered { path } => {
            state.overlay.hovered_tab = Some(path);
        }
        TabMessage::Left { path } => {
            // Conditional clear, the same shape INT-10 uses for tree rows.
            // Tab-to-tab movement publishes both halves in one iced event
            // pass in tree order, so moving toward the end of the strip
            // delivers the later tab's `Entered` before this tab's
            // `Left`. Clearing unconditionally would erase the highlight
            // that just arrived, in the same tick it appeared.
            if state.overlay.hovered_tab.as_deref() == Some(path.as_path()) {
                state.overlay.hovered_tab = None;
            }
        }
    }
}

/// Menu reductions for **both** framework-driven menu surfaces.
///
/// Openness lives in the surface's own
/// [`MenuCascade`](MenuCascade), not in a bool the view reads:
/// [`MenuMessage::Opened`] hands the built
/// [`Menu`](iced_pro::surface::menu::model::Menu) to
/// [`MenuCascade::open`](MenuCascade::open), a row pick runs
/// [`MenuCascade::activate_row`](MenuCascade::activate_row), and the row's
/// own [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy) decides
/// whether the tree survives — which is why a checkbox keeps the card open
/// and an action closes it without either behaviour being spelled out here.
///
/// [`hamburger_open`](OverlayState::hamburger_open) is written back from
/// `cascade.is_open()` after every hamburger transition rather than being set
/// independently, so the bool and the cascade cannot disagree about what is
/// mounted.
///
/// # Which surface a message belongs to
///
/// The two surfaces share one message channel because the keyboard lane that
/// produces [`MenuMessage::Key`] is mounted for whichever menu is open, and
/// [`MenuMessage::RowPicked`] carries a card path that only the owning cascade
/// can resolve. The app keeps them mutually exclusive — opening one closes the
/// other — so `hamburger_open` is a complete discriminator: true routes to the
/// hamburger cascade, false routes to the context surface if it exists.
///
/// `Escape` is **not** handled here. It arrives as
/// [`OverlayMessage::EscapePressed`](MenuMessage::Closed)
/// → `close_topmost`, so the `UIF-WIN-003` band walk stays the app's only
/// Escape path.
fn reduce_menu(state: &mut AppState, message: MenuMessage) -> Vec<Task<Message>> {
    match message {
        MenuMessage::Opened => {
            // The two menus never share the screen: opening the hamburger
            // dismisses any live tree/tab context menu.
            close_context_menu(state);
            let menu = hamburger_menu(&state.models.top);
            state.overlay.hamburger_cascade.open(
                &menu,
                MenuTreeKind::Fixed,
                &mut state.overlay.menu_focus,
            );
            // A fresh tree starts with a fresh corridor. Its trigger
            // indices described whatever cards were up last, and a stale
            // one landing on the same row of the new tree would answer
            // `Preserve` and stop the branch opening on hover at all.
            state.overlay.hamburger_aim.reset();
            settle(state, Vec::new())
        }
        MenuMessage::Closed => {
            state
                .overlay
                .hamburger_cascade
                .close_tree(&mut state.overlay.menu_focus);
            settle(state, Vec::new())
        }
        MenuMessage::RowPicked { card, row } => {
            if state.overlay.hamburger_open {
                // Snapshot the card **before** the cascade runs: a row whose
                // `ClosePolicy` closes the tree takes its own card — and with
                // it the key lookup — away, so resolving the key afterwards
                // would find nothing and silently drop the command.
                let snapshot = state.overlay.hamburger_cascade.card_menu(&card).cloned();
                let outcome = state.overlay.hamburger_cascade.activate_row(
                    &card,
                    row,
                    &mut state.overlay.menu_focus,
                );
                let tasks = activation_command(&snapshot, &outcome)
                    .map_or_else(Vec::new, |command| route_command(state, command));
                settle(state, tasks)
            } else {
                route_context_row(state, &card, row)
            }
        }
        MenuMessage::Key(key) => {
            if state.overlay.hamburger_open {
                // The card the key acts on is the active one *before* the
                // transition: `Enter` on a submenu parent activates that row
                // and then changes which card is active, so reading the active
                // card afterwards would name the wrong one.
                let active = state
                    .overlay
                    .hamburger_cascade
                    .active_card()
                    .map(|card| card.id().clone());
                let snapshot = active
                    .as_ref()
                    .and_then(|card| state.overlay.hamburger_cascade.card_menu(card))
                    .cloned();
                let outcome = state
                    .overlay
                    .hamburger_cascade
                    .on_key(key, &mut state.overlay.menu_focus);
                let tasks = activation_command(&snapshot, &outcome)
                    .map_or_else(Vec::new, |command| route_command(state, command));
                settle(state, tasks)
            } else {
                route_context_key(state, key)
            }
        }
        MenuMessage::RowEntered { card, row } => {
            if state.overlay.hamburger_open {
                // The clock is read once, here, in the reducer, and handed
                // down as a value. Nothing the corridor calls reads a
                // clock itself, which is what makes the whole path
                // testable at a timestamp of the test's choosing
                // (§2.6's injection rule).
                let now = TimeMs::now();
                let step = menu_aim::on_row_entered(
                    &mut state.overlay.hamburger_cascade,
                    &mut state.overlay.hamburger_aim,
                    &card,
                    row,
                    now,
                    &mut state.overlay.menu_focus,
                );
                settle(
                    state,
                    menu_aim::wake_task(&card, step.wake_after_ms)
                        .into_iter()
                        .collect(),
                )
            } else {
                Vec::new()
            }
        }
        MenuMessage::RowLeft { .. } => {
            if state.overlay.hamburger_open {
                // The corridor's "this branch would close" answer is
                // dropped on purpose. Dismissal belongs to the click-away
                // blocker `menu_surface::overlay` mounts; a hover path
                // that also closed menus would give one surface two owners,
                // and every menu in the app would shut when the mouse
                // swept past it.
                let _advisory_collapse =
                    menu_aim::on_pointer_left(&mut state.overlay.hamburger_aim, TimeMs::now());
                // Deliberately not `settle`: the cascade was not touched,
                // so openness, card geometry and the focus projection are
                // all exactly as they were.
                Vec::new()
            } else {
                Vec::new()
            }
        }
        MenuMessage::AimExpired { card } => {
            if state.overlay.hamburger_open {
                let step = menu_aim::on_deadline(
                    &mut state.overlay.hamburger_cascade,
                    &mut state.overlay.hamburger_aim,
                    &card,
                    TimeMs::now(),
                    &mut state.overlay.menu_focus,
                );
                // A hold that has still not elapsed re-arms rather than
                // being dropped, so a switch is never lost to a wake that
                // landed early.
                settle(
                    state,
                    menu_aim::wake_task(&card, step.wake_after_ms)
                        .into_iter()
                        .collect(),
                )
            } else {
                Vec::new()
            }
        }
        MenuMessage::InteractionIgnored => Vec::new(),
    }
}

/// Runs a context-menu row pick: read the row's intent, drive the cascade,
/// then dispatch the intent.
///
/// The intent is read **before** the cascade runs, because a row whose
/// [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy) closes the
/// tree takes its card — and therefore the key lookup — with it.
///
/// The surface is dropped when the cascade says the tree closed rather than
/// when the message is known, so a row that merely absorbs a click leaves the
/// card mounted (`UIF-CMENU-002`).
fn route_context_row(state: &mut AppState, card: &MenuId, row: usize) -> Vec<Task<Message>> {
    let message = state
        .overlay
        .context_menu
        .as_ref()
        .and_then(|menu| menu.row_message(card, row));
    let closes_tree = {
        let Some(menu) = state.overlay.context_menu.as_mut() else {
            return Vec::new();
        };
        let outcome = menu.cascade.activate_row(card, row, &mut menu.focus);
        outcome.activation().is_some_and(|a| a.closes_menu)
    };

    if closes_tree {
        close_context_menu(state);
    }
    message.map_or_else(Vec::new, |message| route_row_message(state, message))
}

/// Runs a context-menu keyboard navigation step (`UIF-CMENU-003`).
///
/// Mirrors the hamburger's arrow-key lane: the active card is read before the
/// transition so an `Enter` on a row resolves against the card the user was
/// actually in.
fn route_context_key(state: &mut AppState, key: MenuKey) -> Vec<Task<Message>> {
    let message = state.overlay.context_menu.as_mut().and_then(|menu| {
        let active = menu.cascade.active_card().map(|card| card.id().clone())?;
        let outcome = menu.cascade.on_key(key, &mut menu.focus);
        let activation = outcome.activation()?;
        menu.row_message(&active, activation.row)
    });

    if state
        .overlay
        .context_menu
        .as_ref()
        .is_some_and(|menu| !menu.cascade.is_open())
    {
        close_context_menu(state);
    }
    message.map_or_else(Vec::new, |message| route_row_message(state, message))
}

/// Routes a context-menu row's intent message to the reducer that owns it.
///
/// Context rows emit only [`Message::Command`], [`Message::Tree`] and
/// [`Message::Tab`] — that is the whole vocabulary the four builders
/// produce. Anything else is a builder bug and is dropped rather than
/// recursing into the top-level reducer.
fn route_row_message(state: &mut AppState, message: Message) -> Vec<Task<Message>> {
    match message {
        Message::Command(command) => route_command(state, command),
        Message::Tree(tree_message) => reduce_tree(state, tree_message),
        Message::Tab(tab_message) => {
            reduce_tab(state, tab_message);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Writes the cascade's post-transition openness back to the state,
/// refreshes the registered card geometry, and projects the framework
/// focus onto iced's router.
///
/// Every hamburger transition ends here so the three follow-on effects
/// cannot be forgotten by one arm of the match.
fn settle(state: &mut AppState, mut tasks: Vec<Task<Message>>) -> Vec<Task<Message>> {
    state.overlay.hamburger_open = state.overlay.hamburger_cascade.is_open();
    if !state.overlay.hamburger_open {
        // The corridor indexes the root card's rows, and those rows went
        // with the card. A guard left pointing at one would hold the next
        // menu's first hover for a grace window it never earned — or, if
        // the new tree put a submenu on the same index, answer `Preserve`
        // and stop that branch opening at all.
        state.overlay.hamburger_aim.reset();
    }
    state.refresh_hamburger_rects();
    tasks.extend(focus_projection(&state.overlay.menu_focus));
    tasks
}

/// Projects the framework focus model onto iced's focus router.
///
/// Only emits when the framework actually has a focused member: pushing a
/// focus task with nothing focused would ask iced to focus an id that no
/// live widget answers to.
fn focus_projection(focus: &FocusState) -> Vec<Task<Message>> {
    if focus.current().is_some() {
        vec![iced_pro::bridge::focus_task(focus)]
    } else {
        Vec::new()
    }
}

/// Resolves the [`UserCommand`] an activation publishes.
///
/// `card` is a **snapshot** of the card taken before the cascade was driven,
/// not a live borrow: an activation whose `ClosePolicy` closes the tree has
/// already removed the card by the time the outcome exists, so a live lookup
/// would return `None` and the user's pick would be silently dropped.
fn activation_command(card: &Option<Menu>, outcome: &CascadeOutcome) -> Option<UserCommand> {
    let activation = outcome.activation()?;
    let key = card.as_ref()?.row(activation.row)?.key().to_owned();
    command_for_activation(&key, &activation.effect)
}

// ───────────────────────────── editor ─────────────────────────────

/// Inline editor reductions (UI-042..UI-045, `UIF-MOD-002/003`).
fn reduce_editor(state: &mut AppState, message: EditorMessage) -> Vec<Task<Message>> {
    match message {
        EditorMessage::SaveRequested { path } => {
            save_editor(state, &path);
            Vec::new()
        }
        // The footer `Cancel` is the *explicit* discard: `guard_leave`
        // lets it through whatever the buffer holds, because pressing it
        // is the user saying "throw it away" in as many words.
        EditorMessage::CancelRequested => {
            close_editor(state);
            Vec::new()
        }
        // A reflex dismiss — the scrim. Guarded, unlike `Cancel`: on a
        // dirty buffer this closes nothing and raises the confirmation.
        EditorMessage::DismissRequested { cause } => {
            leave_editor(state, cause);
            Vec::new()
        }
        // UIF-INP-002 / UIF-VER-003: the card boundary took the press.
        // Deliberately no state change — a click on the padding ring or a
        // disabled `Save` must leave the editor open and the buffer intact.
        EditorMessage::Absorbed => Vec::new(),
        EditorMessage::RecoveryRequested { path, recovery } => run_recovery(state, &path, recovery),
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

/// Applies `UIF-MOD-002` to one reflex dismiss gesture on the scrim path.
///
/// The `Escape` path applies the same [`guard_leave`] from inside
/// [`OverlayState::close_topmost`]'s rank walk; both call the one rule so
/// the two paths cannot drift into answering differently.
fn leave_editor(state: &mut AppState, gesture: LeaveGesture) {
    let may_close = guard_editor_leave(
        &state.overlay.editor,
        &mut state.overlay.dialog,
        &mut state.overlay.dialog_anchor,
        gesture,
    );

    if may_close {
        close_editor(state);
    }
}

/// Performs the recovery affordance the error banner was showing
/// (`UIF-MOD-003`: a failure must offer a way out, not just a message).
fn run_recovery(state: &mut AppState, path: &Path, recovery: Recovery) -> Vec<Task<Message>> {
    match recovery {
        Recovery::Retry => {
            save_editor(state, path);
            Vec::new()
        }
        Recovery::FocusControl(_) => vec![focus_editor_buffer_task()],
        Recovery::Acknowledge => {
            if let Some(vm) = state.overlay.editor.as_mut() {
                vm.clear_save_error();
            }
            Vec::new()
        }
    }
}

/// A runtime task that moves focus to the editor's buffer — the action
/// `Recovery::FocusControl([EDITOR_TEXT_ID])` asks for.
///
/// Built directly on iced's focusable operation rather than through
/// [`iced_pro::bridge::focus_task`], because the bridge projects a
/// [`FocusState`](iced_pro::focus::FocusState) model and the editor card
/// registers no focus group (see the `UIF-MOD-001` note in
/// [`crate::ui_iced::editor`]'s module docs). Naming the widget id is the
/// honest minimum for "put the caret where the error is".
fn focus_editor_buffer_task() -> Task<Message> {
    iced::advanced::widget::operate(
        iced::advanced::widget::operation::focusable::focus::<Message>(iced::widget::Id::from(
            editor::EDITOR_TEXT_ID,
        )),
    )
}

/// Write the buffer (front matter re-attached), publish the update so the
/// tree and tag manager refresh, then close (UI-044, UI-045).
///
/// A save the worker will not accept is **surfaced, not swallowed**
/// (`UIF-MOD-003`): the buffer stays open with a `Recovery::Retry` banner
/// rather than looking saved when it is not.
fn save_editor(state: &mut AppState, path: &Path) {
    let Some(full_text) = state
        .overlay
        .editor
        .as_ref()
        .filter(|vm| vm.path() == path)
        .map(|vm| vm.document().to_string())
    else {
        return;
    };

    let queued = state.app.orchestrator.queue_user_file_operation(
        crate::background::UserFileOperation::SaveEditor {
            path: path.to_path_buf(),
            content: full_text,
        },
    );

    if let Some(vm) = state.overlay.editor.as_mut() {
        record_save_outcome(vm, queued);
    }
}

/// Records the outcome of one save attempt on the buffer (`UIF-MOD-003`).
///
/// Split out so both halves of the outcome are reachable in a test: the
/// worker's submission channel has no seam for forcing a refusal, so
/// `save_editor` itself can only ever be observed taking the accepted
/// branch. Naming the decision as its own pure function is what keeps the
/// refused half covered rather than assumed.
fn record_save_outcome(vm: &mut EditorViewModel, queued: bool) {
    if queued {
        vm.clear_save_error();
    } else {
        vm.note_save_failed();
    }
}

/// Discards the editor buffer and clears the domain open flags.
///
/// `text_buffer.is_open` and `tabs.loaded_path` both have to go: the file
/// reload rule in `AppOrchestrator::process_file_events`
/// (`orchestrator.rs:144-152`) only drops a stale loaded document while
/// the editor is closed.
fn close_editor(state: &mut AppState) {
    state.overlay.editor = None;
    state.app.orchestrator.text_buffer.is_open = false;
    state.app.orchestrator.tabs.loaded_path = None;
}

// ───────────────────────────── agent / search ─────────────────────────────

/// Agent panel and command-input reductions (UI-041).
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

/// Workspace content-search reductions (UI-050).
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
/// [`SearchMessage::ResultsReady`].
///
/// The scan scope is the currently visible tree directories.
fn content_scan_task(state: &AppState, term: String) -> Task<Message> {
    let all_files = state.app.orchestrator.workspace.all_files.clone();
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

/// Background-logs panel reductions (UI-047).
///
/// The filter lives in the shared `BackgroundLogs` ring buffer, so the
/// reducer writes it there and the view-model picks it up on the next
/// rebuild.
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

/// Theme / UI-scale reductions.
fn reduce_theme(state: &mut AppState, message: ThemeMessage) {
    match message {
        ThemeMessage::ScaleChanged { multiplier } => {
            // A near-unity or out-of-range multiplier is stored as `None`
            // so a fresh install carries no redundant value and one
            // corrupt entry self-heals.
            state.app.persisted_ui_state.font_size_scale = sanitise_scale(multiplier);
        }
    }
}

/// Clamp a UI-scale multiplier: keep the value only when it is finite,
/// inside the supported bounds, and not near unity.
fn sanitise_scale(value: f32) -> Option<f32> {
    const MIN: f32 = 0.5;
    const MAX: f32 = 3.0;
    let usable = value.is_finite() && (MIN..=MAX).contains(&value) && (value - 1.0).abs() >= 1e-3;
    usable.then_some(value)
}

// ───────────────────────────── render path ─────────────────────────────

/// Markdown render-path reductions.
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
            // Toggle the open document in memory only; no file write.
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

/// Opens an activated link, driven by [`RenderMessage::LinkActivated`]:
/// resolve it against the current file and the content roots, then select,
/// scroll or hand the URL to the OS.
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
        &state.app.orchestrator.workspace.all_files,
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

/// Small-dialog reductions (UI-011/UI-015).
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
                } => match kind {
                    // The discard confirmation resolved "yes": the buffer
                    // goes, and with it the guard that raised this dialog.
                    // Nothing is published — see `confirmed_command`.
                    DialogKind::DiscardEditorChanges { .. } => close_editor(state),
                    kind => {
                        if let Some(command) = confirmed_command(&kind, name, destination) {
                            publish_and_drain(state, command);
                        }
                    }
                },
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
        DialogMessage::Blurred => {
            // Blur is a commit boundary, not a value change: it decides
            // what the card may show from here on (`UIF-PRIN-009`).
            state.overlay.dialog.blur();
            Vec::new()
        }
        DialogMessage::Absorbed => {
            // UIF-INP-002: the card boundary took the press. Deliberately
            // no state change — the whole contract is that a click on a
            // disabled `Ok` or on the card's padding leaves the dialog
            // open and the tree behind it untouched.
            Vec::new()
        }
        DialogMessage::DestinationSelected { path } => {
            state.overlay.dialog.select_destination(path);
            Vec::new()
        }
    }
}

/// The self-contained command a validated dialog submit produces.
///
/// `None` for a kind whose confirmed action is **pure UI state** rather
/// than a workspace mutation: [`DialogKind::DiscardEditorChanges`] drops
/// an in-memory buffer and clears two open-flags, which is exactly what
/// the editor's own `Cancel` path already does. Publishing it as a
/// `UserCommand` would imply an executor and a bus event for something
/// that never leaves the UI thread ([FMD-065]).
fn confirmed_command(
    kind: &DialogKind,
    name: String,
    destination: Option<PathBuf>,
) -> Option<UserCommand> {
    match kind {
        DialogKind::CreateDirectory { parent } => Some(UserCommand::ConfirmCreateDirectory {
            parent: parent.clone(),
            name,
        }),
        DialogKind::CreateDocument { parent } => Some(UserCommand::ConfirmCreateDocument {
            parent: parent.clone(),
            name,
        }),
        DialogKind::Rename { path } => Some(UserCommand::ConfirmRename {
            path: path.clone(),
            new_name: name,
        }),
        DialogKind::Move { path } => Some(UserCommand::ConfirmMove {
            file: path.clone(),
            destination: destination.unwrap_or_default(),
        }),
        DialogKind::DeleteConfirm { path } => Some(UserCommand::Delete(path.clone())),
        DialogKind::DiscardEditorChanges { .. } => None,
    }
}

// ───────────────────────────── about / batch / tools / debug ─────────────────────────────

/// About / attributions window reductions.
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

/// Batch dialog selection reductions (BATCH-001..BATCH-014).
fn reduce_batch(state: &mut AppState, message: BatchDialogMessage) {
    match message {
        BatchDialogMessage::PromptCandidatesLoaded {
            generation,
            prompts,
        } => {
            if state
                .overlay
                .batch
                .apply_prompt_candidates(generation, prompts.clone())
            {
                let selected_prompt_idx = state.overlay.batch.model().selected_prompt_idx;
                let config = &mut state.app.orchestrator.dialogs.batch_dialog_config;
                config.available_prompts = prompts;
                config.selected_prompt_idx = selected_prompt_idx;
            }
        }
        other => {
            state.overlay.batch.apply(other);
            let model = state.overlay.batch.model();
            let config = &mut state.app.orchestrator.dialogs.batch_dialog_config;
            config.selected_dir_idx = model.selected_dir_idx;
            config.selected_prompt_idx = model.selected_prompt_idx;
            config.pattern.clone_from(&model.pattern);
            config.mode = model.mode;
            config.concurrency = model.concurrency;
        }
    }
}

/// Tools dialog dismiss reduction (UI-051..UI-060).
fn reduce_tools(state: &mut AppState, message: ToolsDialogMessage) {
    state.overlay.tools.apply(message);
}

/// Agent debug window reductions (UI-048).
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

/// Inbound *background → UI* reductions.
///
/// Each arm runs the orchestrator routine that consumes its bus, triggered
/// by the bridge message that names the bus.
fn reduce_inbound(state: &mut AppState, message: InboundMessage) {
    match message {
        InboundMessage::ConfigArrived(config) => {
            apply_config_arrival(state, &config);
        }
        InboundMessage::FileChanged(event) => {
            // The bridge is the single inbound path, so the event the message
            // carries is applied here rather than re-read off a second
            // subscriber. The extra `process_file_events` call keeps the
            // `Workspace` drains (both buses) running.
            state.app.orchestrator.apply_file_changed(event);
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::FileChangedBatch(events) => {
            // Same semantics as the single-event arm, with the workspace
            // drain hoisted out of the loop: the whole burst lands in one
            // tree rebuild.
            for event in events {
                state.app.orchestrator.apply_file_changed(event);
            }
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::DirectoryChanged(event) => {
            state.app.orchestrator.apply_directory_changed(event);
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::DirectoryChangedBatch(events) => {
            // Same semantics as the single-event arm, with the workspace
            // drain hoisted out of the loop: the whole burst lands in one
            // tree rebuild.
            for event in events {
                state.app.orchestrator.apply_directory_changed(event);
            }
            state.app.orchestrator.process_file_events();
        }
        InboundMessage::Agent(_) => {
            state.app.orchestrator.drain_agent_event_bus();
        }
        InboundMessage::AgentBatch(_events) => {
            // Same semantics as the single-event arm: the whole burst lands
            // in one drain call.
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
        BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
            crate::background::UserFileOperationResult::PromptLoaded {
                content: Ok(content),
                target_dir,
                target_file,
            },
        )) => {
            publish_and_drain(
                state,
                UserCommand::RunSkillPrompt {
                    content: content.clone(),
                    target_dir: target_dir.clone(),
                    target_file: target_file.clone(),
                },
            );
        }
        BackgroundEvent::Process(process_event @ ProcessEvent::UserFileOperationCompleted(_)) => {
            let editor_open = match process_event {
                ProcessEvent::UserFileOperationCompleted(
                    crate::background::UserFileOperationResult::EditorOpened {
                        path,
                        content: Ok(Some(_)),
                    },
                ) if state.app.orchestrator.pending_editor_open.as_ref() == Some(path) => {
                    Some(path.clone())
                }
                _ => None,
            };
            let editor_saved = match process_event {
                ProcessEvent::UserFileOperationCompleted(
                    crate::background::UserFileOperationResult::EditorSaved {
                        path,
                        saved_content,
                        result: Ok(()),
                    },
                ) => Some((path.clone(), saved_content.clone())),
                _ => None,
            };
            state
                .app
                .orchestrator
                .handle_process_event(process_event.clone());

            if let Some(path) = editor_open {
                open_editor_from_buffer(state, &path);
            }
            if let Some((path, saved_content)) = editor_saved
                && state.overlay.editor.as_ref().is_some_and(|editor| {
                    editor.path() == path.as_path()
                        && editor.document().to_string() == saved_content
                })
            {
                close_editor(state);
            }
        }
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

/// Handle a lagging bus subscriber: report the loss loudly and, for the
/// agent stream, leave the visible truncation marker the transcript reducer
/// owns.
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

/// Boot-infrastructure report reductions.
fn reduce_boot(message: BootMessage) -> Task<Message> {
    match message {
        BootMessage::FontLoadFinished(Some(error)) => {
            // NFR-001: a font-registration failure must produce an ERROR
            // log. Non-fatal: iced keeps its default font set.
            tracing::error!(name = FONT_LOAD_ERROR_CODE, %error, "Font registration failed");
            Task::none()
        }
        BootMessage::FontLoadFinished(None) => Task::none(),
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
