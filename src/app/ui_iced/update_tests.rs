//! Unit tests for the iced reducer (`src/app/ui_iced/update.rs`).
//!
//! Channel-by-channel coverage of the reducer. The Elm loop's runtime tasks
//! (window close, font load) need
//! a live iced runtime and stay manual-verify items; everything else —
//! state reduction, bus publish/execute,
//! deferred-intent file I/O and the view-model rebuild — is exercised here.
//!
//! [RUST-006]: every filesystem touch runs inside a `tempfile::TempDir`;
//! persistence goes through `InMemoryConfigStorage` /
//! `InMemoryUiStateStorage`. No real user path is ever read or written.

use super::*;
use crate::agent::batch::types::{BatchDialogConfig, PromptInfo};
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::messages::LogCategory;
use crate::bus::events::typed::{BackgroundEvent, FsEvent, McpAuthEvent, ProcessEvent};
use crate::bus::events::user_command::UserCommand;
use crate::config::{AppConfig, ContentLibrary, InMemoryConfigStorage};
use crate::error_codes::menu as menu_codes;
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui::tree::flatten::FlatRow;
use crate::ui_iced::app::{AppState, OverlayLayer, ViewModels, build_state};
use crate::ui_iced::batch_dialog::BatchDialogViewModel;
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::menu_surface::log_capture::{Capture, with_capture};
use crate::ui_iced::message::BootMessage;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use hoarfrost::surface::menu::model::{Row, submenu_id};
use iced::widget::pane_grid::ResizeEvent;
use iced::{Point, Size};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const VIEWPORT: f32 = 1000.0;

/// A populated persisted snapshot for seeding the booted state.
fn populated_persisted() -> crate::ui::PersistedUiState {
    crate::ui::PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(998.0),
        window_height: Some(688.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    }
}

/// A fresh `AppState` over an in-memory UI-state store whose concrete handle
/// is returned alongside (for close/save assertions).
fn fresh_state() -> (AppState, Arc<InMemoryUiStateStorage>) {
    let storage = Arc::new(InMemoryUiStateStorage::new());
    let state = build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::clone(&storage) as Arc<dyn crate::ui_iced::persisted::UiStateStorage>,
        populated_persisted(),
    );
    (state, storage)
}

/// An extra subscriber on the orchestrator's user-command bus, so a test
/// can see exactly which intents the reducer published.
fn command_spy(state: &AppState) -> BusReader<UserCommand> {
    state.app.orchestrator.user_command_bus.subscribe()
}

/// Drains a spy reader into a `Vec` of the commands it saw.
fn drained(spy: &BusReader<UserCommand>) -> Vec<UserCommand> {
    let mut seen = Vec::new();
    while let Ok(command) = spy.try_recv() {
        seen.push(command);
    }
    seen
}

fn user_file_operation_events(
    state: &AppState,
) -> tokio::sync::broadcast::Receiver<BackgroundEvent> {
    state.app.orchestrator.tx.subscribe()
}

fn receive_user_file_operation(
    receiver: &mut tokio::sync::broadcast::Receiver<BackgroundEvent>,
) -> BackgroundEvent {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match receiver.recv().await {
                    Ok(
                        event @ BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
                            _,
                        )),
                    ) => break event,
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        panic!("background event bus closed before the operation completed")
                    }
                }
            }
        })
        .await
        .expect("background file operation timed out")
    })
}

fn apply_user_file_operation(state: &mut AppState, event: BackgroundEvent) {
    let _ = update(
        state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );
}

/// Resizes the state's viewport so width-dependent reductions have a
/// defined geometry.
fn with_viewport(state: &mut AppState, width: f32) {
    state.viewport = Size::new(width, 700.0);
}

/// Seeds the tree with rows **the way the reducer does**: into the
/// `FastMdApp::cached_tree_rows` cache with `tree_dirty` cleared, which is
/// exactly the cache transition `finish_frame` performs.
///
/// Seeding only `models.left.rows` would not survive the view-model rebuild that
/// every `update` runs - the rebuild would re-derive the rows from the real
/// filesystem and the seeded rows would be gone before the message under test is
/// reduced.
fn seed_rows(state: &mut AppState, rows: Vec<FlatRow>) {
    let rows = Arc::new(rows);
    state.app.cached_tree_rows = Some(Arc::clone(&rows));
    state.app.orchestrator.selection.tree_dirty = false;
    state.models.left.rows = rows;
}

fn file_row(path: &str) -> FlatRow {
    FlatRow {
        depth: 0,
        name: PathBuf::from(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        path: PathBuf::from(path),
        is_dir: false,
        is_expanded: false,
    }
}

fn dir_row(path: &str) -> FlatRow {
    FlatRow {
        depth: 0,
        name: PathBuf::from(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        path: PathBuf::from(path),
        is_dir: true,
        is_expanded: true,
    }
}

/// A prompt candidate identified by `name`, for the batch re-index tests.
fn prompt(name: &str) -> PromptInfo {
    PromptInfo {
        path: PathBuf::from("/prompts").join(name),
        display_name: name.to_string(),
        library_name: "System".to_string(),
        content: format!("{name} body"),
    }
}

// ───────────────────────────── Window ─────────────────────────────

#[test]
fn test_update_window_resized_tracks_viewport() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::Resized {
            id: iced::window::Id::unique(),
            size: Size::new(VIEWPORT, 700.0),
        }),
    );
    assert_eq!(state.viewport, Size::new(VIEWPORT, 700.0));
}

#[test]
fn test_update_scale_baseline_stores_os_scale() {
    let (mut state, _storage) = fresh_state();
    assert!(state.os_scale.is_none());
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::ScaleBaseline(1.5)),
    );
    assert_eq!(state.os_scale, Some(1.5));
    assert_eq!(
        state.effective_scale_factor(),
        1.5 * 1.25,
        "on-screen scale is baseline × persisted multiplier"
    );
    assert_eq!(
        state.requested_scale_factor(),
        1.25,
        "the iced request must stay the user multiplier: iced applies the OS factor itself"
    );
}

#[test]
fn test_update_pane_resized_applies_clamped_widths_and_syncs_persisted() {
    let (mut state, _storage) = fresh_state();
    with_viewport(&mut state, VIEWPORT);

    let event = ResizeEvent {
        split: state.band.left_split,
        ratio: 0.25,
    };
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::PaneResized(event)),
    );

    assert_eq!(
        state.app.layout().left_panel_width,
        Some(200.0),
        "25% of 1000 clamps to the 20% cap ([UI-016])"
    );
    assert_eq!(
        state.app.persisted_ui_state.left_panel_width,
        Some(200.0),
        "the persisted snapshot stays in sync for the close-time save"
    );
}

#[test]
fn test_close_snapshot_persists_before_close_reduction() {
    let (mut state, storage) = fresh_state();
    state.app.layout.set_width(Some(184.0));

    let _ = update(
        &mut state,
        Message::Window(WindowMessage::CloseSnapshotCaptured(
            crate::ui_iced::message::WindowSnapshot {
                id: iced::window::Id::unique(),
                size: Size::new(1234.0, 567.0),
                position: Some(Point::new(11.0, 22.0)),
            },
        )),
    );

    let json = storage.saved_json().expect("state must be persisted");
    assert!(json.contains("1234"), "saved payload: {json}");
    assert!(json.contains("567"), "saved payload: {json}");
    assert!(
        json.contains("184"),
        "panel width rides the snapshot: {json}"
    );
}

#[test]
fn test_close_metrics_carry_over_expanded_dirs_and_about_stamp() {
    let (mut state, storage) = fresh_state();
    let mut dirs = std::collections::HashSet::new();
    dirs.insert(PathBuf::from("/notes/work"));
    state.app.persisted_ui_state.expanded_dirs = dirs;
    state.app.persisted_ui_state.about_shown_for_version = Some("0.2.0".to_owned());

    let _ = update(
        &mut state,
        Message::Window(WindowMessage::CloseSnapshotCaptured(
            crate::ui_iced::message::WindowSnapshot {
                id: iced::window::Id::unique(),
                size: Size::new(500.0, 400.0),
                position: None,
            },
        )),
    );

    let json = storage.saved_json().expect("payload saved");
    assert!(json.contains("notes/work"));
    assert!(json.contains("0.2.0"));
    assert!(json.contains("\"window_x\":null"), "saved payload: {json}");
}

// ───────────────────────────── Layout ─────────────────────────────

#[test]
fn test_cursor_moved_is_stored_as_the_overlay_anchor_source() {
    let (mut state, _storage) = fresh_state();
    assert!(state.overlay.cursor.is_none());
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(37.0, 91.0),
        }),
    );
    assert_eq!(state.overlay.cursor, Some(Point::new(37.0, 91.0)));
}

#[test]
fn test_tree_auto_fit_sets_and_persists_the_left_panel_width() {
    let (mut state, _storage) = fresh_state();
    with_viewport(&mut state, VIEWPORT);
    seed_rows(
        &mut state,
        vec![dir_row("/notes/a-very-long-directory-name-that-needs-room")],
    );

    let _ = update(&mut state, Message::Layout(LayoutMessage::TreeAutoFit));

    let width = state
        .app
        .layout()
        .left_panel_width
        .expect("auto-fit must set a width");
    assert!(width > 0.0, "auto-fit produced {width}");
    assert!(
        width <= VIEWPORT * 0.2 + f32::EPSILON,
        "auto-fit respects the UI-016 20% cap, got {width}"
    );
    assert_eq!(state.app.persisted_ui_state.left_panel_width, Some(width));
}

#[test]
fn test_tree_auto_fit_is_a_noop_on_a_non_positive_viewport() {
    let (mut state, _storage) = fresh_state();
    state.viewport = Size::new(0.0, 0.0);
    state.app.layout.set_width(Some(222.0));

    let _ = update(&mut state, Message::Layout(LayoutMessage::TreeAutoFit));

    assert_eq!(
        state.app.layout().left_panel_width,
        Some(222.0),
        "a zero-width viewport must not clobber the remembered width"
    );
}

// ───────────────────────────── Command ─────────────────────────────

#[test]
fn test_command_channel_publishes_and_executes() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    assert!(state.app.tags().selected_tag.is_none());

    let _ = update(
        &mut state,
        Message::Command(UserCommand::SelectTagFilter(Some("iced".to_string()))),
    );

    assert_eq!(
        state.app.tags().selected_tag.as_deref(),
        Some("iced"),
        "the command must be executed via the orchestrator drain"
    );
    assert_eq!(
        drained(&spy),
        vec![UserCommand::SelectTagFilter(Some("iced".to_string()))],
        "the intent must also be observable on the bus"
    );
}

#[test]
fn test_delete_is_intercepted_and_deferred_to_the_confirm_dialog() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let path = PathBuf::from("/notes/gone.md");

    let _ = update(
        &mut state,
        Message::Command(UserCommand::Delete(path.clone())),
    );

    assert_eq!(
        state.overlay.dialog.kind(),
        Some(&DialogKind::DeleteConfirm { path: path.clone() }),
        "the iced port asks before deleting (new behaviour)"
    );
    assert_eq!(
        drained(&spy),
        Vec::<UserCommand>::new(),
        "an unconfirmed delete must never reach the executor"
    );

    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert_eq!(
        drained(&spy),
        vec![UserCommand::Delete(path)],
        "confirming publishes the self-contained intent"
    );
    assert!(
        !state.overlay.dialog.is_open(),
        "a completed submit closes the dialog"
    );
}

#[test]
fn test_rename_opens_the_dialog_prefilled_with_the_current_name() {
    let (mut state, _storage) = fresh_state();
    let path = PathBuf::from("/notes/report.md");

    let _ = update(
        &mut state,
        Message::Command(UserCommand::Rename(path.clone())),
    );

    assert_eq!(
        state.overlay.dialog.kind(),
        Some(&DialogKind::Rename { path: path.clone() })
    );
    assert_eq!(
        state.overlay.dialog.pending_name(),
        crate::ui::tree::flatten::initial_rename_value(&path, "report.md"),
        "the rename reducer must pre-fill the dialog field"
    );
}

#[test]
fn test_move_dialog_gets_the_visible_directories_as_destinations() {
    let (mut state, _storage) = fresh_state();
    seed_rows(
        &mut state,
        vec![dir_row("/notes/work"), file_row("/notes/a.md")],
    );

    let _ = update(
        &mut state,
        Message::Command(UserCommand::Move(PathBuf::from("/notes/a.md"))),
    );

    assert_eq!(
        state.overlay.dialog.destinations(),
        &[PathBuf::from("/notes/work")],
        "only directory rows are move destinations"
    );
}

#[test]
fn test_create_commands_open_their_name_entry_dialogs() {
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Command(UserCommand::CreateDirectory {
            parent: PathBuf::from("/notes"),
        }),
    );
    assert_eq!(
        state.overlay.dialog.kind(),
        Some(&DialogKind::CreateDirectory {
            parent: PathBuf::from("/notes")
        })
    );

    let _ = update(
        &mut state,
        Message::Command(UserCommand::CreateDocument {
            parent: PathBuf::from("/docs"),
        }),
    );
    assert_eq!(
        state.overlay.dialog.kind(),
        Some(&DialogKind::CreateDocument {
            parent: PathBuf::from("/docs")
        })
    );
}

#[test]
fn an_absorbed_card_press_leaves_the_dialog_open_and_unchanged() {
    // UIF-INP-002: the card's hit-capture boundary reports the press it
    // swallowed, and the reducer's answer is deliberately nothing. A
    // click on a disabled `Ok`, on the padding ring, or on dead space
    // between rows must not read as a dismissal.
    //
    // `CreateDirectory` is the fixture precisely because it opens with an
    // empty field: `can_confirm()` is false, so this is the exact state
    // in which a disabled-`Ok` click happens.
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Command(UserCommand::CreateDirectory {
            parent: PathBuf::from("/notes"),
        }),
    );
    let before = state.overlay.dialog.clone();
    assert!(before.is_open());
    assert!(
        !before.can_confirm(),
        "an empty create field must render `Ok` disabled"
    );

    let _ = update(&mut state, Message::Dialog(DialogMessage::Absorbed));

    assert!(
        state.overlay.dialog.is_open(),
        "absorption is not dismissal"
    );
    assert_eq!(state.overlay.dialog, before);
}

#[test]
fn a_blur_message_commits_the_field_through_the_reducer() {
    // The framework field reports `FocusChanged(false)`; the reducer turns
    // that into the commit boundary `UIF-PRIN-009` names, and the card
    // shows what it had been withholding.
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Command(UserCommand::CreateDirectory {
            parent: PathBuf::from("/notes"),
        }),
    );
    assert!(!state.overlay.dialog.is_touched());
    assert!(state.overlay.dialog.painted_error().is_none());

    let _ = update(&mut state, Message::Dialog(DialogMessage::Blurred));

    assert!(state.overlay.dialog.is_touched());
    assert_eq!(
        state.overlay.dialog.painted_error(),
        Some(crate::ui::strings::DIALOG_NAME_EMPTY_ERROR),
        "leaving the field makes the withheld invalid visible"
    );
}

#[test]
fn a_disallowed_character_reaches_the_card_without_a_commit() {
    // The keystroke itself, not any commit boundary, puts the red on the
    // screen: the reducer only stores what the framework said.
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Command(UserCommand::Rename(PathBuf::from("/notes/a.md"))),
    );
    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::NameChanged {
            text: "a/b".to_string(),
        }),
    );

    assert!(
        !state.overlay.dialog.is_touched(),
        "typing never commits the field"
    );
    assert_eq!(
        state.overlay.dialog.painted_error(),
        Some(crate::ui::strings::DIALOG_NAME_UNSAFE_ERROR),
        "and the card still paints the disallowed-character flag"
    );
    assert!(!state.overlay.dialog.can_confirm());
}

#[test]
fn test_window_opening_commands_open_their_iced_windows() {
    let (mut state, _storage) = fresh_state();

    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));
    assert!(state.overlay.batch.is_open());

    let _ = update(&mut state, Message::Command(UserCommand::OpenToolsDialog));
    assert!(state.overlay.tools.is_open());

    let _ = update(&mut state, Message::Command(UserCommand::OpenAboutDialog));
    assert!(state.overlay.about.is_about());
}

#[test]
fn test_cancel_batch_closes_the_dialog_after_cancelling() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    state.overlay.batch.open(state.models.batch.clone());

    let _ = update(&mut state, Message::Command(UserCommand::CancelBatch));

    assert_eq!(drained(&spy), vec![UserCommand::CancelBatch]);
    assert!(
        !state.overlay.batch.is_open(),
        "CancelBatch dismisses the dialog"
    );
}

#[test]
fn test_clear_debug_entries_collapses_the_expanded_rows() {
    let (mut state, _storage) = fresh_state();
    state.overlay.debug.toggle_entry(3);
    state.overlay.debug.toggle_entry(7);
    assert_eq!(state.overlay.debug.expanded_count(), 2);

    let _ = update(&mut state, Message::Command(UserCommand::ClearDebugEntries));

    assert_eq!(
        state.overlay.debug.expanded_count(),
        0,
        "row ids are positional; a stale expansion would open the wrong entry"
    );
}

#[test]
fn test_open_in_editor_lifts_the_text_buffer_into_an_editor_view_model() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("note.md");
    std::fs::write(&path, "---\ntitle: Hi\n---\nBody text\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    let vm = state.overlay.editor.as_ref().expect("editor must open");
    assert_eq!(vm.path(), path.as_path());
    // `TextBuffer::open` stores `DocumentContent::parse(raw).body`
    // verbatim (`ui/text_buffer.rs:302-304`), and that body keeps the newline the
    // parser leaves after the closing `---`. The front matter is the full fenced
    // block, exactly as `EditorViewModel`'s own contract pins it
    // (`editor_tests::open_shows_only_the_body_and_caches_the_front_matter`).
    assert_eq!(vm.text(), "\nBody text\n");
    assert_eq!(vm.front_matter(), Some("---\ntitle: Hi\n---"));
    assert!(
        !vm.text().contains("title: Hi"),
        "front matter must never reach the editable body"
    );
}

#[test]
fn test_editor_open_then_save_round_trips_the_file_byte_for_byte() {
    // The leading newline is load-bearing, not cosmetic: `DocumentContent`'s
    // `Display` joins front matter and body with NO separator
    // (`markdown/document.rs:64-72`), so stripping it would write
    // `---\ntitle: Hi\n---Body text` back to disk and break the front-matter
    // fence for every other consumer.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let original = "---\ntitle: Hi\n---\nBody text\n";
    let path = dir.path().join("roundtrip.md");
    std::fs::write(&path, original).expect("write seed");

    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);
    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested { path: path.clone() }),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        original,
        "an untouched open then save must not change a single byte"
    );
}

#[test]
fn editor_save_completes_through_a_background_result() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("deferred-save.md");
    let original = "original";
    std::fs::write(&path, original).expect("write seed");
    let (mut state, _storage) = fresh_state();
    state.overlay.editor = Some(crate::ui_iced::editor::EditorViewModel::open(
        &path, "updated",
    ));
    let mut events = user_file_operation_events(&state);

    let task = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested { path: path.clone() }),
    );

    assert_eq!(task.units(), 0, "the reducer only queues background I/O");
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "updated");
}

#[test]
fn test_open_in_editor_does_not_mount_when_background_read_fails() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(PathBuf::from("/notes/a.md"))),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    assert!(
        state.overlay.editor.is_none(),
        "a failed background read must not mount an editor"
    );
}

#[test]
fn editor_open_result_for_a_superseded_path_does_not_mount_the_editor() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    let old_path = PathBuf::from("/tmp/old-editor.md");
    let current_path = PathBuf::from("/tmp/current-editor.md");
    state.app.orchestrator.pending_editor_open = Some(current_path.clone());

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(
            BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
                crate::background::UserFileOperationResult::EditorOpened {
                    path: old_path,
                    content: Ok(Some("stale contents".to_string())),
                },
            )),
        ))),
    );

    assert!(state.overlay.editor.is_none());
    assert_eq!(
        state.app.orchestrator.pending_editor_open,
        Some(current_path)
    );
}

// ───────────────────────────── Tree ─────────────────────────────

/// The visible labels of the mounted context menu's root card, separators
/// excluded — i.e. the rows the user can actually pick.
fn context_menu_labels(state: &AppState) -> Vec<String> {
    let menu = state
        .overlay
        .context_menu
        .as_ref()
        .expect("a context menu is open");
    let root = crate::ui_iced::context_menu::root_id(&menu.target);
    menu.cascade
        .card_menu(&root)
        .expect("the root card is open")
        .rows()
        .iter()
        .filter(|row| !row.is_separator())
        .map(|row| row.label().to_owned())
        .collect()
}

#[test]
fn test_tree_context_menu_is_built_from_the_row_and_anchored_at_the_cursor() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(64.0, 128.0),
        }),
    );

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );

    let menu = state.overlay.context_menu.as_ref().expect("menu opens");
    assert_eq!(menu.anchor, Point::new(64.0, 128.0));
    assert!(
        !context_menu_labels(&state).is_empty(),
        "a file row always has context-menu rows"
    );
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::ContextMenu]);
}

#[test]
fn test_tree_context_menu_for_an_unknown_row_mounts_nothing() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/missing.md"),
        }),
    );
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "an empty menu must not block input"
    );
}

#[test]
fn test_tree_context_menu_closes() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );
    let _ = update(&mut state, Message::Tree(TreeMessage::ContextMenuClosed));
    assert!(state.overlay.context_menu.is_none());
}

/// Regression: picking a context-menu row ran the action but left the card
/// pinned on screen. A menu row carries its own intent, so handling
/// one must also dismiss the menu the way a native dropdown closes.
#[test]
fn test_activating_a_context_menu_row_dismisses_the_menu() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );
    assert!(
        state.overlay.context_menu.is_some(),
        "precondition: the menu is open"
    );

    // An empty multi-select delete is the benign stand-in for a picked row: it
    // rides the same dismissal arm without touching the filesystem.
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::DeleteSelectedRequested { paths: Vec::new() }),
    );

    assert!(
        state.overlay.context_menu.is_none(),
        "picking a row must close the menu"
    );
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "a dismissed menu must stop blocking input"
    );
}

/// A picked `Message::Command` row dismisses the menu too — that is the channel
/// every application intent in both the tree and the tab menu rides.
#[test]
fn test_command_row_dismisses_the_context_menu() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );

    // `SelectFile` is the in-process, side-effect-free member of the row set.
    let _ = update(
        &mut state,
        Message::Command(UserCommand::SelectFile {
            path: PathBuf::from("/notes/a.md"),
            multi: false,
        }),
    );

    assert!(
        state.overlay.context_menu.is_none(),
        "a command row must close the menu"
    );
}

/// The full row vocabulary the two menu builders emit, pinned against the pure
/// predicate so a newly added menu row kind cannot silently skip dismissal.
#[test]
fn dismisses_context_menu_covers_the_row_message_vocabulary() {
    // Every intent a menu row can carry.
    assert!(dismisses_context_menu(&Message::Command(
        UserCommand::CopyPath(PathBuf::from("/notes/a.md"))
    )));
    assert!(dismisses_context_menu(&Message::Tree(
        TreeMessage::RunAsPromptRequested {
            path: PathBuf::from("/notes/a.md")
        }
    )));
    assert!(dismisses_context_menu(&Message::Tree(
        TreeMessage::SkillRunRequested {
            skill: PathBuf::from("/skills/s.md"),
            target_dir: None,
            target_file: None,
        }
    )));
    assert!(dismisses_context_menu(&Message::Tree(
        TreeMessage::PrintRequested {
            path: PathBuf::from("/notes/a.md")
        }
    )));
    assert!(dismisses_context_menu(&Message::Tree(
        TreeMessage::DeleteSelectedRequested {
            paths: vec![PathBuf::from("/notes/a.md")]
        }
    )));

    // Non-activation traffic must leave an open menu alone.
    assert!(!dismisses_context_menu(&Message::Tree(
        TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md")
        }
    )));
    assert!(!dismisses_context_menu(&Message::Tree(
        TreeMessage::ContextMenuClosed
    )));
    assert!(!dismisses_context_menu(&Message::Layout(
        LayoutMessage::CursorMoved {
            point: Point::new(1.0, 2.0)
        }
    )));
    assert!(!dismisses_context_menu(&Message::Menu(MenuMessage::Closed)));
}

#[test]
fn test_run_as_prompt_reads_the_body_and_runs_it_against_the_file() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("prompt.md");
    std::fs::write(&path, "---\ntitle: T\n---\nSummarise this\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RunAsPromptRequested { path: path.clone() }),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    assert_eq!(
        drained(&spy),
        vec![UserCommand::RunSkillPrompt {
            // the reducer publishes `DocumentContent::parse(&raw).body` verbatim
            // (`ui/tree/render.rs:145`), and that body keeps the newline the
            // parser leaves after the closing `---`.
            content: "\nSummarise this\n".to_string(),
            target_dir: None,
            target_file: Some(path),
        }],
        "front matter is stripped and the body becomes the prompt"
    );
}

#[test]
fn run_as_prompt_defers_file_read_until_its_background_task_runs() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("deferred-prompt.md");
    std::fs::write(&path, "Read me").expect("write seed");
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let mut events = user_file_operation_events(&state);

    let task = update(
        &mut state,
        Message::Tree(TreeMessage::RunAsPromptRequested { path }),
    );

    assert_eq!(task.units(), 0, "the UI only queues the file read");
    assert!(
        drained(&spy).is_empty(),
        "prompt execution waits for file read"
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);
    assert_eq!(drained(&spy).len(), 1, "prompt command follows file read");
}

#[test]
fn test_run_as_prompt_on_a_missing_file_logs_and_publishes_nothing() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: PathBuf::from("/definitely/not/here.md"),
        }),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    assert_eq!(
        drained(&spy),
        Vec::<UserCommand>::new(),
        "a failed read must not publish a half-built prompt"
    );
}

#[test]
fn test_skill_run_carries_its_target_through_to_the_command() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let skill = dir.path().join("skill.md");
    std::fs::write(&skill, "Do the thing").expect("write seed");
    let target = dir.path().join("notes");

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: skill.clone(),
            target_dir: Some(target.clone()),
            target_file: None,
        }),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    assert_eq!(
        drained(&spy),
        vec![UserCommand::RunSkillPrompt {
            content: "Do the thing".to_string(),
            target_dir: Some(target),
            target_file: None,
        }]
    );
}

#[test]
fn test_print_requested_hands_the_job_to_a_task_not_the_ui_thread() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "# Doc\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    // There is no `UserCommand::Print`, so nothing may be published. The
    // render-and-open work must ride a lazy iced Task: `units() > 0` proves a
    // real task was produced, and because the reducer never polls it no
    // browser is launched here.
    let spy = command_spy(&state);
    let task = update(
        &mut state,
        Message::Tree(TreeMessage::PrintRequested { path }),
    );
    assert_eq!(drained(&spy), Vec::<UserCommand>::new());
    assert!(task.units() > 0, "print must produce a Task, not none()");
}

#[test]
fn test_print_finished_reports_failure_without_publishing() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let task = update(
        &mut state,
        Message::Tree(TreeMessage::PrintFinished {
            path: PathBuf::from("/notes/a.md"),
            result: Err("no background channel".to_owned()),
        }),
    );
    // The failure is logged, never re-routed into a command.
    assert_eq!(task.units(), 0, "settling a print spawns no further work");
    assert_eq!(drained(&spy), Vec::<UserCommand>::new());
}

#[test]
fn test_delete_selected_fans_out_and_clears_the_selection() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    state
        .app
        .orchestrator
        .selection
        .selected_files
        .insert(PathBuf::from("/notes/a.md"));
    state
        .app
        .orchestrator
        .selection
        .selected_files
        .insert(PathBuf::from("/notes/b.md"));

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::DeleteSelectedRequested {
            paths: vec![PathBuf::from("/notes/a.md"), PathBuf::from("/notes/b.md")],
        }),
    );

    assert_eq!(
        drained(&spy),
        vec![
            UserCommand::Delete(PathBuf::from("/notes/a.md")),
            UserCommand::Delete(PathBuf::from("/notes/b.md")),
        ],
        "one Delete per selected path, no confirm dialog"
    );
    assert!(state.app.orchestrator.selection.selected_files.is_empty());
    assert!(
        !state.overlay.dialog.is_open(),
        "the multi-select path deliberately bypasses the confirm dialog"
    );
}

// ───────────────────────────── Tab ─────────────────────────────

#[test]
fn test_tab_context_menu_opens_and_closes() {
    let (mut state, _storage) = fresh_state();
    state
        .app
        .orchestrator
        .tabs
        .tabs
        .push(PathBuf::from("/notes/a.md"));
    state.rebuild_view_models();
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(12.0, 34.0),
        }),
    );

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::ContextMenuOpened { index: 0 }),
    );
    let menu = state.overlay.context_menu.as_ref().expect("menu opens");
    assert_eq!(menu.anchor, Point::new(12.0, 34.0));
    assert!(
        !context_menu_labels(&state).is_empty(),
        "the tab menu must have rows"
    );

    let _ = update(&mut state, Message::Tab(TabMessage::ContextMenuClosed));
    assert!(state.overlay.context_menu.is_none());
}

#[test]
fn test_tab_context_menu_for_a_missing_tab_mounts_nothing() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Tab(TabMessage::ContextMenuOpened { index: 9 }),
    );
    assert_eq!(state.mounted_layers(), Vec::<OverlayLayer>::new());
}

// ───────────────────────────── Menu (hamburger) ─────────────────────────────

/// Picks the root card's `Windows` row.
///
/// Under the framework engine there is no separate "open submenu" message:
/// a submenu card appears because its own row was activated, so this is the
/// one way the tests navigate a level down.
fn open_windows_submenu(state: &mut AppState) {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_WINDOWS};
    use hoarfrost::surface::menu::model::MenuId;

    let root = MenuId::new(HAMBURGER_MENU_ID);
    let row = state
        .overlay
        .hamburger_cascade
        .card_menu(&root)
        .expect("the root card is open")
        .rows()
        .iter()
        .position(|row| row.key() == KEY_WINDOWS)
        .expect("the Windows row lives in the root card");
    let _ = update(
        state,
        Message::Menu(MenuMessage::RowPicked { card: root, row }),
    );
}

#[test]
fn test_menu_opened_flips_the_flag() {
    let (mut state, _storage) = fresh_state();
    assert!(!state.overlay.hamburger_open);
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    assert!(state.overlay.hamburger_open, "Opened must show the card");
    assert!(
        state.overlay.hamburger_cascade.is_open(),
        "the cascade must be mounted alongside the flag, not left empty"
    );
}

#[test]
fn test_menu_closed_clears_the_flag() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    open_windows_submenu(&mut state);
    let _ = update(&mut state, Message::Menu(MenuMessage::Closed));
    assert!(
        !state.overlay.hamburger_open,
        "Closed must unmount the card"
    );
    assert!(
        !state.overlay.hamburger_cascade.is_open(),
        "the cascade must be unwound, not merely hidden"
    );
}

#[test]
fn test_submenu_requires_an_open_tree_and_stacks_rather_than_replacing() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_WINDOWS};
    use hoarfrost::surface::menu::model::{MenuId, submenu_id};

    let windows_card = submenu_id(&MenuId::new(HAMBURGER_MENU_ID), KEY_WINDOWS);

    // A row pick naming a card that was never mounted changes nothing: the
    // cascade has no root card to hang a child on.
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowPicked {
            card: windows_card.clone(),
            row: 0,
        }),
    );
    assert!(
        !state.overlay.hamburger_cascade.is_open(),
        "a submenu request cannot open the tree by itself"
    );
    assert!(
        state.overlay.hamburger_rects.is_empty(),
        "nothing is registered while the tree is closed"
    );

    // Picking the root card's Windows row mounts the child card beside the
    // root instead of replacing it.
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    open_windows_submenu(&mut state);
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the submenu card stacks above the root"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &windows_card,
        "the deep card is the Windows submenu"
    );
    assert!(
        state.overlay.hamburger_open,
        "stepping into a submenu does not dismiss the surface"
    );
}

#[test]
fn test_menu_row_activation_routes_the_command_and_closes() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_ABOUT};
    use hoarfrost::surface::menu::model::MenuId;

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));

    let root = MenuId::new(HAMBURGER_MENU_ID);
    let row = state
        .overlay
        .hamburger_cascade
        .card_menu(&root)
        .expect("the root card is open")
        .rows()
        .iter()
        .position(|row| row.key() == KEY_ABOUT)
        .expect("the About row lives in the root card");
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowPicked { card: root, row }),
    );

    assert!(
        !state.overlay.hamburger_open,
        "a row pick must close the menu in the same reduction"
    );
    assert!(
        drained(&spy).contains(&UserCommand::OpenAboutDialog),
        "the row's own command must be routed"
    );
}

#[test]
fn test_menu_toggle_routes_its_new_value_and_stays_open() {
    use crate::ui_iced::hamburger::{
        HAMBURGER_MENU_ID, KEY_BACKGROUND_OPERATIONS, KEY_WINDOWS, hamburger_menu,
    };
    use hoarfrost::surface::menu::model::{MenuId, submenu_id};

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let windows_card = submenu_id(&MenuId::new(HAMBURGER_MENU_ID), KEY_WINDOWS);
    let menu = hamburger_menu(&state.models.top);
    let windows = menu
        .rows()
        .iter()
        .find(|row| row.key() == KEY_WINDOWS)
        .and_then(|row| row.child())
        .expect("the Windows submenu");
    let toggle_row = windows
        .rows()
        .iter()
        .position(|row| row.key() == KEY_BACKGROUND_OPERATIONS)
        .expect("the background-operations row");

    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    open_windows_submenu(&mut state);
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowPicked {
            card: windows_card.clone(),
            row: toggle_row,
        }),
    );

    assert!(
        state.overlay.hamburger_open,
        "a Check row's StayOpen policy keeps the menu open"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the active submenu card stays mounted after a toggle"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &windows_card,
        "the toggle must not swap the branch it was picked from"
    );
    assert!(
        state
            .overlay
            .hamburger_cascade
            .card_menu(&windows_card)
            .and_then(|menu| menu.row(toggle_row))
            .is_some_and(|row| row.is_checked()),
        "the cascade committed the new checked value into its own model copy"
    );
    assert!(drained(&spy).contains(&UserCommand::ToggleBackgroundLogs(true)));
}

#[test]
fn test_menu_opening_dismisses_a_live_context_menu() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );
    assert!(state.overlay.context_menu.is_some());

    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    open_windows_submenu(&mut state);
    assert!(
        state.overlay.context_menu.is_none(),
        "the two menus never share the screen"
    );
}

#[test]
fn test_context_menu_opening_dismisses_the_hamburger() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    open_windows_submenu(&mut state);
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the fixture has two cards mounted"
    );

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );
    assert!(
        !state.overlay.hamburger_open,
        "opening a context menu must dismiss the hamburger"
    );
    assert!(
        !state.overlay.hamburger_cascade.is_open(),
        "the whole hamburger tree goes, not only the card on screen"
    );
}

// ─────────────────── Menu hover / aiming corridor (INT-9) ───────────────────

/// Row index of `key` inside the open hamburger root card.
fn root_row_of(state: &AppState, key: &str) -> usize {
    use crate::ui_iced::hamburger::HAMBURGER_MENU_ID;
    use hoarfrost::surface::menu::model::MenuId;

    state
        .overlay
        .hamburger_cascade
        .card_menu(&MenuId::new(HAMBURGER_MENU_ID))
        .expect("the root card is open")
        .rows()
        .iter()
        .position(|row| row.key() == key)
        .expect("the row lives in the root card")
}

/// Reports the pointer arriving at the root card's `key` row.
fn hover_root(state: &mut AppState, key: &str) -> Task<Message> {
    use hoarfrost::surface::menu::model::MenuId;

    let card = MenuId::new(crate::ui_iced::hamburger::HAMBURGER_MENU_ID);
    let row = root_row_of(state, key);
    update(state, Message::Menu(MenuMessage::RowEntered { card, row }))
}

#[test]
fn test_hovering_a_submenu_parent_opens_its_branch_without_a_click() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_CHAT_MODELS};
    use hoarfrost::surface::menu::model::{MenuId, submenu_id};

    let chat_card = submenu_id(&MenuId::new(HAMBURGER_MENU_ID), KEY_CHAT_MODELS);
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    assert_eq!(state.overlay.hamburger_cascade.depth(), 1);

    let _ = hover_root(&mut state, KEY_CHAT_MODELS);

    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the hover must mount the branch on its own"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &chat_card,
        "and the branch it mounts is the one the pointer is on"
    );
    assert!(
        drained(&spy).is_empty(),
        "hovering opens a card; it never publishes a command"
    );
    assert_eq!(
        state.overlay.hamburger_aim.open_trigger(),
        Some(root_row_of(&state, KEY_CHAT_MODELS)),
        "the corridor guards the row that was just hovered"
    );
}

#[test]
fn test_a_sibling_hover_is_held_and_owes_a_wake_up() {
    use crate::ui_iced::hamburger::{KEY_CHAT_MODELS, KEY_WINDOWS};

    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut state, KEY_WINDOWS);
    assert_eq!(state.overlay.hamburger_cascade.depth(), 2);

    let held = hover_root(&mut state, KEY_CHAT_MODELS).units();

    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the corridor is open, so the branch must not switch yet"
    );
    assert_eq!(
        state.overlay.hamburger_aim.open_trigger(),
        Some(root_row_of(&state, KEY_WINDOWS)),
        "the corridor still guards the branch that is showing"
    );

    // `settle` always appends the focus projection, so the raw task count
    // is not a bare "is a wake owed". The control is the same reduction
    // that owes nothing: the pointer lands back on the row whose branch
    // is already open, so the corridor answers `Preserve` and schedules
    // no wake-up. One extra unit is the wake-up.
    let (mut control, _control_storage) = fresh_state();
    let _ = update(&mut control, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut control, KEY_WINDOWS);
    let owed_nothing = hover_root(&mut control, KEY_WINDOWS).units();

    assert_eq!(
        held,
        owed_nothing + 1,
        "a held switch must schedule its own wake-up, or it only fires \
         when the user jiggles the mouse"
    );
}

#[test]
fn test_the_held_switch_completes_when_the_deadline_message_lands() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_CHAT_MODELS, KEY_WINDOWS};
    use hoarfrost::surface::menu::aim::grace_ms;
    use hoarfrost::surface::menu::model::{MenuId, submenu_id};
    use std::time::Duration;

    let chat_card = submenu_id(&MenuId::new(HAMBURGER_MENU_ID), KEY_CHAT_MODELS);
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut state, KEY_WINDOWS);

    let _ = hover_root(&mut state, KEY_CHAT_MODELS);
    assert_eq!(
        state.overlay.hamburger_aim.open_trigger(),
        Some(root_row_of(&state, KEY_WINDOWS)),
        "still guarding Windows: the switch is being held"
    );

    // This leg runs on the **real** clock, which is the point: it proves
    // the reducer's `TimeMs::now()` and the corridor's arithmetic agree
    // end to end, not just against a timestamp a test invented.
    std::thread::sleep(Duration::from_millis(grace_ms() + 60));
    let card = MenuId::new(crate::ui_iced::hamburger::HAMBURGER_MENU_ID);
    let _ = update(&mut state, Message::Menu(MenuMessage::AimExpired { card }));

    assert_eq!(
        state.overlay.hamburger_aim.open_trigger(),
        Some(root_row_of(&state, KEY_CHAT_MODELS)),
        "the held switch must mature once the window has elapsed"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &chat_card,
        "and the card that ends up mounted is the one the pointer is on"
    );
}

#[test]
fn test_the_pointer_leaving_a_row_never_dismisses_the_menu() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_WINDOWS};
    use hoarfrost::surface::menu::model::{MenuId, submenu_id};

    let windows_card = submenu_id(&MenuId::new(HAMBURGER_MENU_ID), KEY_WINDOWS);
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut state, KEY_WINDOWS);
    assert_eq!(state.overlay.hamburger_cascade.depth(), 2);

    // Sweep the pointer out of everything without clicking.
    let card = MenuId::new(HAMBURGER_MENU_ID);
    let _ = update(&mut state, Message::Menu(MenuMessage::RowLeft { card }));

    assert!(
        state.overlay.hamburger_open,
        "a pointer-out report must never dismiss the surface"
    );
    assert!(
        state.overlay.hamburger_cascade.is_open(),
        "the tree must still be mounted"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "and the open branch must still be there"
    );
    assert!(
        state.overlay.hamburger_cascade.is_card_open(&windows_card),
        "the branch the pointer left is exactly the one that survives"
    );
    assert!(
        !state.overlay.hamburger_rects.is_empty(),
        "the registered geometry is untouched"
    );
}

#[test]
fn test_a_deadline_that_lands_after_the_menu_closed_does_nothing() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_CHAT_MODELS, KEY_WINDOWS};
    use hoarfrost::surface::menu::model::MenuId;

    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut state, KEY_WINDOWS);
    let _ = hover_root(&mut state, KEY_CHAT_MODELS);

    let _ = update(&mut state, Message::Menu(MenuMessage::Closed));
    assert!(!state.overlay.hamburger_open);

    let card = MenuId::new(HAMBURGER_MENU_ID);
    let _ = update(&mut state, Message::Menu(MenuMessage::AimExpired { card }));

    assert!(
        !state.overlay.hamburger_open,
        "a wake-up cannot resurrect a dismissed menu"
    );
    assert!(
        !state.overlay.hamburger_cascade.is_open(),
        "and it must not mount a card under a closed surface"
    );
}

#[test]
fn test_the_hamburger_button_is_still_click_to_open_and_hover_cannot_open_it() {
    use crate::ui_iced::hamburger::{HAMBURGER_MENU_ID, KEY_WINDOWS};
    use hoarfrost::surface::menu::model::MenuId;

    let (mut state, _storage) = fresh_state();

    // A stray hover report while the menu is closed must not open it.
    // Hover drives submenu branches **inside** an open menu and nothing
    // else; opening the ☰ menu stays a click.
    let card = MenuId::new(HAMBURGER_MENU_ID);
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowEntered {
            card: card.clone(),
            row: 2,
        }),
    );
    assert!(
        !state.overlay.hamburger_open,
        "hover cannot open the hamburger from a closed state"
    );
    assert!(!state.overlay.hamburger_cascade.is_open());

    // And the click path is exactly what it was before this increment.
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    assert!(state.overlay.hamburger_open);
    assert_eq!(state.overlay.hamburger_cascade.depth(), 1);

    let row = root_row_of(&state, KEY_WINDOWS);
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowPicked { card, row }),
    );
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "clicking a submenu parent still opens it exactly as before"
    );
}

#[test]
fn test_reopening_the_hamburger_clears_a_stale_aim_guard() {
    use crate::ui_iced::hamburger::KEY_WINDOWS;

    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    let _ = hover_root(&mut state, KEY_WINDOWS);
    assert!(state.overlay.hamburger_aim.open_trigger().is_some());

    let _ = update(&mut state, Message::Menu(MenuMessage::Closed));
    assert!(
        state.overlay.hamburger_aim.open_trigger().is_none(),
        "closing the tree must drop the corridor's trigger index"
    );

    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    assert_eq!(
        state.overlay.hamburger_aim.open_trigger(),
        None,
        "a reopened menu starts with a clean corridor, or its first hover \
         would be answered Preserve and never open"
    );
}

#[test]
fn test_hovering_a_plain_action_row_opens_nothing() {
    use crate::ui_iced::hamburger::KEY_ABOUT;

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));

    let _ = hover_root(&mut state, KEY_ABOUT);

    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        1,
        "a plain action row has no branch behind it to open"
    );
    assert!(
        drained(&spy).is_empty(),
        "hovering is not activation: no command may be published"
    );
    assert!(
        state.overlay.hamburger_open,
        "and hovering an action row must not close the menu either"
    );
}

// ───────────────────────────── Editor ─────────────────────────────

/// Opens an editor over a temp file with the inline editor enabled.
fn open_editor(dir: &tempfile::TempDir) -> (AppState, PathBuf) {
    let path = dir.path().join("edit.md");
    std::fs::write(&path, "---\ntitle: Keep\n---\nOriginal body\n").expect("write seed");
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    state.app.orchestrator.tabs.loaded_path = Some(path.clone());
    let mut events = user_file_operation_events(&state);
    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);
    (state, path)
}

#[test]
fn test_editor_cancel_discards_the_buffer_and_clears_the_loaded_path() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (mut state, path) = open_editor(&dir);
    assert!(state.overlay.editor.is_some());

    let _ = update(&mut state, Message::Editor(EditorMessage::CancelRequested));

    assert!(state.overlay.editor.is_none());
    assert!(
        !state.app.orchestrator.text_buffer.is_open,
        "the domain open flag must clear too, or a file-change reload stays suppressed"
    );
    assert!(
        state.app.orchestrator.tabs.loaded_path.is_none(),
        "cancelling must clear the loaded path too"
    );
    assert!(path.exists(), "a cancel must never write anything to disk");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "---\ntitle: Keep\n---\nOriginal body\n"
    );
}

#[test]
fn test_editor_save_writes_the_document_and_closes() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (mut state, path) = open_editor(&dir);
    let mut events = user_file_operation_events(&state);

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested { path: path.clone() }),
    );
    let event = receive_user_file_operation(&mut events);
    apply_user_file_operation(&mut state, event);

    let written = std::fs::read_to_string(&path).expect("file saved");
    assert!(
        written.contains("title: Keep"),
        "front matter is re-attached on save: {written}"
    );
    assert!(written.contains("Original body"), "saved: {written}");
    assert!(state.overlay.editor.is_none(), "save closes the editor");
    assert!(!state.app.orchestrator.text_buffer.is_open);
}

#[test]
fn test_editor_save_for_a_stale_path_is_ignored() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (mut state, _path) = open_editor(&dir);
    let other = dir.path().join("other.md");

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested {
            path: other.clone(),
        }),
    );

    assert!(
        state.overlay.editor.is_some(),
        "a save aimed at another file must not close this buffer"
    );
    assert!(
        !other.exists(),
        "a stale save must not write to the wrong target"
    );
}

#[test]
fn test_editor_without_an_open_buffer_reduces_without_panicking() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Editor(EditorMessage::CancelRequested));
    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested {
            path: PathBuf::from("/notes/a.md"),
        }),
    );
    assert!(state.overlay.editor.is_none());
}

// ───────────────────────────── Agent ─────────────────────────────

#[test]
fn test_agent_input_changed_writes_the_command_input() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::InputChanged {
            text: "summarise ".to_string(),
        }),
    );
    assert_eq!(
        state.app.orchestrator.agent_panel_state.command_input,
        "summarise "
    );
}

#[test]
fn test_agent_submit_while_idle_runs_the_parsed_intent_and_clears() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::Submit {
            text: "hello world".to_string(),
        }),
    );

    let seen = drained(&spy);
    assert_eq!(
        seen.first(),
        Some(&UserCommand::RunAgent("hello world".into()))
    );
    assert!(
        seen.contains(&UserCommand::ClearCommandInput),
        "the clear rides with the intent so the reduction is replayable"
    );
}

#[test]
fn test_agent_submit_while_running_queues_instead() {
    let (mut state, _storage) = fresh_state();
    state.app.agent_mut().set_running(true);
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::Submit {
            text: "next thing".to_string(),
        }),
    );

    assert_eq!(
        drained(&spy),
        vec![
            UserCommand::QueueAgentPrompt("next thing".into()),
            UserCommand::ClearCommandInput
        ]
    );
}

#[test]
fn test_agent_submit_of_blank_text_publishes_nothing() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::Submit {
            text: "   ".to_string(),
        }),
    );
    assert_eq!(drained(&spy), Vec::<UserCommand>::new());
}

#[test]
fn test_thinking_toggle_lands_in_the_rebuilt_agent_view_model() {
    let (mut state, _storage) = fresh_state();
    assert!(
        state.models.agent.thinking_expanded,
        "the collapsing block starts open"
    );

    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::ThinkingToggled { expanded: false }),
    );

    assert!(!state.overlay.thinking_expanded);
    assert!(
        !state.models.agent.thinking_expanded,
        "the reducer-held disclosure must survive the view-model rebuild, \
         which otherwise re-applies from_app's hardcoded `true`"
    );
}

#[test]
fn test_agent_scroll_report_is_stored_for_the_follow_rule() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Agent(AgentMessage::ScrollReported {
            offset_y: 10.0,
            viewport_height: 100.0,
            content_height: 500.0,
        }),
    );
    let snapshot = state.overlay.agent_scroll.expect("snapshot stored");
    assert_eq!(snapshot.max_offset(), 400.0);
    assert!(
        !snapshot.is_at_bottom(),
        "scrolled to 10 of 400 is nowhere near the bottom"
    );
}

// ───────────────────────────── Search ─────────────────────────────

#[test]
fn test_search_query_change_does_not_start_a_scan() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::QueryChanged {
            text: "term".to_string(),
        }),
    );
    assert_eq!(state.overlay.search.query(), "term");
    assert!(
        !state.overlay.search.is_in_progress(),
        "typing must never trigger a workspace scan"
    );
}

#[test]
fn test_search_submit_marks_the_scan_in_progress() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::Submitted {
            query: "needle".to_string(),
        }),
    );
    assert_eq!(state.overlay.search.active_filter(), Some("needle"));
    assert!(state.overlay.search.is_in_progress());
    assert_eq!(
        state.overlay.search.result_count(),
        0,
        "a new term drops the previous results — showing them would be a lie"
    );
}

#[test]
fn test_search_submit_of_blank_text_clears_instead() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::Submitted {
            query: "  ".to_string(),
        }),
    );
    assert!(state.overlay.search.active_filter().is_none());
}

#[test]
fn test_search_results_ready_applies_and_reaches_the_left_panel_model() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::Submitted {
            query: "needle".to_string(),
        }),
    );

    let entry = crate::ui::tree_search::SearchResultEntry {
        path: PathBuf::from("/notes/a.md"),
        file_name: "a.md".to_string(),
        relative_path: "a.md".to_string(),
        snippet: "a needle here".to_string(),
        line_number: 1,
        match_count: 3,
    };
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::ResultsReady {
            query: "needle".to_string(),
            results: Arc::new(vec![entry]),
        }),
    );

    assert_eq!(state.overlay.search.result_count(), 1);
    assert!(!state.overlay.search.is_in_progress());
    assert_eq!(
        state.models.left.results.len(),
        1,
        "the rebuilt left-panel model must show the results"
    );
    assert!(state.models.left.is_searching);
    assert_eq!(state.models.left.query.value, "needle");
}

#[test]
fn test_stale_search_results_are_dropped() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::Submitted {
            query: "second".to_string(),
        }),
    );

    let _ = update(
        &mut state,
        Message::Search(SearchMessage::ResultsReady {
            query: "first".to_string(),
            results: Arc::new(vec![crate::ui::tree_search::SearchResultEntry {
                path: PathBuf::from("/notes/a.md"),
                file_name: "a.md".to_string(),
                relative_path: "a.md".to_string(),
                snippet: String::new(),
                line_number: 0,
                match_count: 1,
            }]),
        }),
    );

    assert_eq!(
        state.overlay.search.result_count(),
        0,
        "a superseded scan must not paint under the newer term"
    );
}

#[test]
fn test_search_cleared_returns_the_tree_view() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Search(SearchMessage::Submitted {
            query: "needle".to_string(),
        }),
    );
    let _ = update(&mut state, Message::Search(SearchMessage::Cleared));

    assert!(state.overlay.search.query().is_empty());
    assert!(!state.models.left.is_searching);
}

#[test]
fn batch_dialog_composition_and_candidates_survive_cancel_and_reopen() {
    let (mut state, _storage) = fresh_state();
    let prompt = PromptInfo {
        path: PathBuf::from("fixture/prompts/selected.md"),
        display_name: "Test / selected.md".to_string(),
        library_name: "Test".to_string(),
        content: "selected prompt body".to_string(),
    };
    let config = &mut state.app.orchestrator.dialogs.batch_dialog_config;
    config.available_dirs = vec![PathBuf::from("fixture/target")];
    config.available_prompts = vec![prompt.clone()];
    config.selected_dir_idx = Some(0);
    config.selected_prompt_idx = Some(0);

    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PatternChanged {
            text: "*.txt".to_string(),
        }),
    );
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::ModeChanged(
            crate::agent::batch::types::BatchMode::Directory,
        )),
    );
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::ConcurrencyChanged(7)),
    );
    let _ = update(&mut state, Message::Command(UserCommand::CancelBatch));
    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));

    assert_eq!(state.models.batch.pattern, "*.txt");
    assert_eq!(
        state.models.batch.mode,
        crate::agent::batch::types::BatchMode::Directory
    );
    assert_eq!(state.models.batch.concurrency, 7);
    assert_eq!(state.models.batch.available_prompts, vec![prompt]);
    assert_eq!(state.models.batch.selected_prompt_idx, Some(0));
}

// ───────────────────────────── Logs ─────────────────────────────

#[test]
fn test_log_filters_write_through_to_the_shared_ring_buffer() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::SearchChanged {
            text: "index".to_string(),
        }),
    );
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::CategoryFilterChanged(Some(
            LogCategory::Indexer,
        ))),
    );
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::AutoScrollToggled(false)),
    );

    let manager = state
        .app
        .orchestrator
        .background_manager
        .lock()
        .expect("logs lock");
    assert_eq!(manager.search_text, "index");
    assert_eq!(manager.filter_category, Some(LogCategory::Indexer));
    assert!(!manager.auto_scroll);
}

#[test]
fn test_log_filters_reach_the_rebuilt_view_model() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::SearchChanged {
            text: "index".to_string(),
        }),
    );
    assert_eq!(state.models.logs.search_text, "index");
    // `auto_scroll` only ever changes through its checkbox - a search edit
    // leaves it alone - so
    // the toggle, not the search, is what the rebuilt model must reflect.
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::AutoScrollToggled(false)),
    );
    assert!(
        !state.models.logs.auto_scroll,
        "the rebuilt snapshot must carry the toggled stick-to-bottom preference"
    );
}

#[test]
fn test_logs_cleared_empties_the_buffer() {
    let (mut state, _storage) = fresh_state();
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.push_log(crate::bus::events::messages::BackgroundLogEntry::new(
            LogCategory::Watcher,
            "noise".to_string(),
        ));
    }
    // `models.logs` is a snapshot, not a live view - re-snap it the way the
    // end of every `update` does before asserting against it.
    state.rebuild_view_models();
    assert_eq!(state.models.logs.total_entries, 1);

    let _ = update(&mut state, Message::Logs(LogsMessage::Cleared));

    assert_eq!(state.models.logs.total_entries, 0);
}

#[test]
fn test_logs_scroll_report_is_stored() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Logs(LogsMessage::ScrollReported {
            offset_y: 400.0,
            viewport_height: 100.0,
            content_height: 500.0,
        }),
    );
    assert!(
        state.overlay.logs_scroll.expect("stored").is_at_bottom(),
        "400 + 100 == 500 is at the bottom"
    );
}

// ───────────────────────────── Theme ─────────────────────────────

#[test]
fn test_scale_changed_stores_a_multiplier_in_range() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Theme(ThemeMessage::ScaleChanged { multiplier: 1.4 }),
    );
    assert_eq!(state.app.persisted_ui_state.font_size_scale, Some(1.4));
}

#[test]
fn test_near_unity_scale_is_stored_as_none() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Theme(ThemeMessage::ScaleChanged { multiplier: 1.0 }),
    );
    assert_eq!(
        state.app.persisted_ui_state.font_size_scale, None,
        "a fresh install's 1.0 scale is stored as None so it is never misread"
    );
}

#[test]
fn test_out_of_range_and_non_finite_scale_self_heal_to_none() {
    for value in [0.1_f32, 9.0, f32::NAN, f32::INFINITY] {
        let (mut state, _storage) = fresh_state();
        let _ = update(
            &mut state,
            Message::Theme(ThemeMessage::ScaleChanged { multiplier: value }),
        );
        assert_eq!(
            state.app.persisted_ui_state.font_size_scale, None,
            "{value} is outside the persisted range"
        );
    }
}

// ───────────────────────────── Render ─────────────────────────────

#[test]
fn test_task_toggle_mutates_the_open_document_in_memory_only() {
    let (mut state, _storage) = fresh_state();
    let document = PathBuf::from("/notes/tasks.md");
    state.app.orchestrator.tabs.loaded_path = Some(document.clone());
    state.app.orchestrator.tabs.current_markdown = "- [ ] first\n- [ ] second\n".to_string();

    let _ = update(
        &mut state,
        Message::Render(RenderMessage::TaskToggled {
            document: document.clone(),
            task_index: 1,
            checked: true,
        }),
    );

    assert_eq!(
        state.app.orchestrator.tabs.current_markdown, "- [ ] first\n- [x] second\n",
        "the toggle applies to the in-memory source"
    );
}

#[test]
fn test_task_toggle_for_a_different_document_is_ignored() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.tabs.loaded_path = Some(PathBuf::from("/notes/other.md"));
    state.app.orchestrator.tabs.current_markdown = "- [ ] first\n".to_string();

    let _ = update(
        &mut state,
        Message::Render(RenderMessage::TaskToggled {
            document: PathBuf::from("/notes/stale.md"),
            task_index: 0,
            checked: true,
        }),
    );

    assert_eq!(
        state.app.orchestrator.tabs.current_markdown,
        "- [ ] first\n"
    );
}

#[test]
fn test_copy_code_requested_returns_a_clipboard_task() {
    let (mut state, _storage) = fresh_state();
    let task = update(
        &mut state,
        Message::Render(RenderMessage::CopyCodeRequested {
            content: "fn main() {}".to_string(),
        }),
    );
    assert!(
        task.units() > 0,
        "the clipboard write is a runtime task the reducer must hand back"
    );
}

#[test]
fn test_anchor_link_resolves_against_the_toc() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.tabs.toc = vec![crate::markdown::ToCEntry::new(
        "Getting Started",
        1,
        "getting-started",
    )];

    let _ = update(
        &mut state,
        Message::Render(RenderMessage::LinkActivated {
            url: "#getting-started".to_string(),
            source_file: None,
        }),
    );

    assert_eq!(
        state.app.orchestrator.tabs.scroll_to_header_id.as_deref(),
        Some("getting-started")
    );
}

#[test]
fn test_workspace_markdown_link_selects_the_file_and_carries_the_anchor() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let target = dir.path().join("other.md");
    std::fs::write(&target, "# Other\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.content_libraries = vec![ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "Workspace".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    state.app.orchestrator.workspace.add_file(target.clone());
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Render(RenderMessage::LinkActivated {
            url: "other.md#section".to_string(),
            source_file: Some(dir.path().join("index.md")),
        }),
    );

    assert!(
        spy.try_recv()
            .is_ok_and(|command| matches!(command, UserCommand::SelectFile { .. })),
        "a workspace markdown link selects its target"
    );
    assert_eq!(
        state.app.orchestrator.tabs.scroll_to_header_id.as_deref(),
        Some("section")
    );
}

// ───────────────────────────── Dialog ─────────────────────────────

#[test]
fn test_dialog_name_change_clears_a_previous_error() {
    let (mut state, _storage) = fresh_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));
    assert!(
        state.overlay.dialog.error().is_some(),
        "blank name rejected"
    );

    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::NameChanged {
            text: "new-folder".to_string(),
        }),
    );
    assert!(
        state.overlay.dialog.error().is_none(),
        "typing hides the stale error"
    );
}

#[test]
fn test_dialog_submit_publishes_the_confirmed_create_command() {
    let (mut state, _storage) = fresh_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    let spy = command_spy(&state);
    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::NameChanged {
            text: "reports".to_string(),
        }),
    );

    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert_eq!(
        drained(&spy),
        vec![UserCommand::ConfirmCreateDirectory {
            parent: PathBuf::from("/notes"),
            name: "reports".to_string(),
        }]
    );
    assert!(!state.overlay.dialog.is_open());
}

#[test]
fn test_dialog_submit_with_an_unsafe_name_keeps_the_dialog_open() {
    let (mut state, _storage) = fresh_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    let spy = command_spy(&state);
    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::NameChanged {
            text: "../escape".to_string(),
        }),
    );

    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert!(
        state.overlay.dialog.is_open(),
        "a traversal name is rejected and the dialog stays up"
    );
    assert_eq!(drained(&spy), Vec::<UserCommand>::new());
}

#[test]
fn test_move_submit_uses_the_picked_destination() {
    let (mut state, _storage) = fresh_state();
    state.overlay.dialog.open_move(
        PathBuf::from("/notes/a.md"),
        vec![PathBuf::from("/notes/work")],
    );
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::DestinationSelected {
            path: PathBuf::from("/notes/work"),
        }),
    );
    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert_eq!(
        drained(&spy),
        vec![UserCommand::ConfirmMove {
            file: PathBuf::from("/notes/a.md"),
            destination: PathBuf::from("/notes/work"),
        }]
    );
}

#[test]
fn test_move_submit_without_a_destination_is_rejected() {
    let (mut state, _storage) = fresh_state();
    state.overlay.dialog.open_move(
        PathBuf::from("/notes/a.md"),
        vec![PathBuf::from("/notes/work")],
    );
    let spy = command_spy(&state);

    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert!(state.overlay.dialog.is_open());
    assert!(state.overlay.dialog.error().is_some());
    assert_eq!(drained(&spy), Vec::<UserCommand>::new());
}

#[test]
fn test_cancel_and_dismiss_both_close_the_dialog_without_publishing() {
    for message in [DialogMessage::Cancelled, DialogMessage::Dismissed] {
        let (mut state, _storage) = fresh_state();
        state.overlay.dialog.open(DialogKind::Rename {
            path: PathBuf::from("/notes/a.md"),
        });
        let spy = command_spy(&state);

        let _ = update(&mut state, Message::Dialog(message.clone()));

        assert!(!state.overlay.dialog.is_open(), "{message:?} must close it");
        assert_eq!(drained(&spy), Vec::<UserCommand>::new());
    }
}

// ───────────────────────────── About / Batch / Tools / Debug ─────────────────────────────

#[test]
fn test_about_attributions_request_switches_the_window() {
    let (mut state, _storage) = fresh_state();
    state.overlay.about.open_about();

    let _ = update(
        &mut state,
        Message::About(AboutMessage::AttributionsRequested),
    );

    assert!(state.overlay.about.is_attributions());
    assert!(!state.overlay.about.is_about());
}

#[test]
fn test_about_close_hides_the_window() {
    let (mut state, _storage) = fresh_state();
    state.overlay.about.open_about();
    let _ = update(&mut state, Message::About(AboutMessage::CloseRequested));
    assert!(!state.overlay.about.is_open());
    assert_eq!(state.mounted_layers(), Vec::<OverlayLayer>::new());
}

#[test]
fn test_copy_commit_hash_returns_a_clipboard_task() {
    let (mut state, _storage) = fresh_state();
    state.overlay.about.open_about();
    let task = update(
        &mut state,
        Message::About(AboutMessage::CopyCommitHashRequested),
    );
    assert!(task.units() > 0);
}

#[test]
fn test_batch_selection_messages_compose_the_model() {
    let (mut state, _storage) = fresh_state();
    state.overlay.batch.open(BatchDialogViewModel {
        available_dirs: vec![PathBuf::from("/notes"), PathBuf::from("/docs")],
        ..Default::default()
    });

    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::DirectorySelected(Some(1))),
    );
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PatternChanged {
            text: "*.txt".to_string(),
        }),
    );
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::ConcurrencyChanged(6)),
    );

    let model = state.overlay.batch.model();
    assert_eq!(model.selected_dir_idx, Some(1));
    assert_eq!(model.pattern, "*.txt");
    assert_eq!(model.concurrency, 6);
}

#[test]
fn test_batch_open_drops_a_selection_that_no_longer_resolves() {
    let (mut state, _storage) = fresh_state();
    state.overlay.batch.open(BatchDialogViewModel {
        available_dirs: vec![PathBuf::from("/a"), PathBuf::from("/b")],
        selected_dir_idx: Some(1),
        ..Default::default()
    });
    assert_eq!(state.overlay.batch.model().selected_dir_idx, Some(1));

    state.overlay.batch.open(BatchDialogViewModel {
        available_dirs: vec![PathBuf::from("/only")],
        selected_dir_idx: Some(1),
        ..Default::default()
    });

    // `BatchDialogState::open`'s clamping is pinned by its own sidecar
    // (`batch_dialog_tests::open_clamps_selections_that_no_longer_resolve`) to
    // DROP an index that no longer resolves. Clamping it to the last candidate
    // instead would silently select a directory the user never picked, which for
    // a destructive batch run is the wrong failure mode.
    assert_eq!(
        state.overlay.batch.model().selected_dir_idx,
        None,
        "a stale index is dropped, not slid onto an unrelated candidate"
    );
    assert!(
        !state.overlay.batch.can_start(),
        "a dropped pick must disable the Process button"
    );
}

#[test]
fn test_open_batch_dialog_reindexes_the_previous_pick_by_path() {
    let (mut state, _storage) = fresh_state();
    state.overlay.batch.open(BatchDialogViewModel {
        available_dirs: vec![PathBuf::from("/a"), PathBuf::from("/b")],
        selected_dir_idx: Some(1),
        available_prompts: vec![prompt("Alpha"), prompt("Beta")],
        selected_prompt_idx: Some(1),
        ..Default::default()
    });

    // The candidate lists are rebuilt in a different order underneath the dialog.
    state.app.orchestrator.dialogs.batch_dialog_config = BatchDialogConfig {
        available_dirs: vec![PathBuf::from("/b"), PathBuf::from("/a")],
        available_prompts: vec![prompt("Beta"), prompt("Alpha")],
        selected_dir_idx: None,
        selected_prompt_idx: None,
        ..Default::default()
    };
    state.rebuild_view_models();

    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));

    let model = state.overlay.batch.model();
    assert_eq!(
        model.selected_directory(),
        Some(Path::new("/b")),
        "the directory pick follows the path, not the old position"
    );
    assert_eq!(
        model.selected_prompt().map(|p| p.display_name.as_str()),
        Some("Beta"),
        "the prompt pick follows the prompt path, not the old position"
    );
}

#[test]
fn test_open_batch_dialog_drops_a_previous_pick_that_is_gone() {
    let (mut state, _storage) = fresh_state();
    state.overlay.batch.open(BatchDialogViewModel {
        available_dirs: vec![PathBuf::from("/gone")],
        selected_dir_idx: Some(0),
        ..Default::default()
    });

    state.app.orchestrator.dialogs.batch_dialog_config = BatchDialogConfig {
        available_dirs: vec![PathBuf::from("/fresh")],
        ..Default::default()
    };
    state.rebuild_view_models();

    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));

    assert_eq!(state.overlay.batch.model().selected_directory(), None);
    assert!(
        !state.overlay.batch.can_start(),
        "a vanished pick leaves the dialog un-startable until the user re-picks"
    );
}

#[test]
fn test_tools_close_requested_hides_the_dialog() {
    let (mut state, _storage) = fresh_state();
    state.overlay.tools.open();
    let _ = update(
        &mut state,
        Message::Tools(ToolsDialogMessage::CloseRequested),
    );
    assert!(!state.overlay.tools.is_open());
}

#[test]
fn test_debug_entry_toggle_flips_one_row_only() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 2 }),
    );
    let _ = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 5 }),
    );
    assert!(state.overlay.debug.is_expanded(2));
    assert!(state.overlay.debug.is_expanded(5));

    let _ = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 2 }),
    );
    assert!(!state.overlay.debug.is_expanded(2));
    assert!(
        state.overlay.debug.is_expanded(5),
        "rows toggle independently per header"
    );
}

#[test]
fn test_debug_expanded_rows_reach_the_rebuilt_view_model() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 4 }),
    );
    assert!(state.models.debug.expanded.contains(&4));
}

#[test]
fn test_debug_copy_json_returns_a_clipboard_task() {
    let (mut state, _storage) = fresh_state();
    let task = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::CopyJsonRequested {
            content: "{}".to_string(),
        }),
    );
    assert!(task.units() > 0);
}

#[test]
fn test_debug_scroll_report_is_stored() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::AgentDebug(AgentDebugMessage::ScrollReported {
            offset_y: 1.0,
            viewport_height: 2.0,
            content_height: 3.0,
        }),
    );
    assert_eq!(
        state.overlay.debug_scroll.map(|s| s.max_offset()),
        Some(1.0)
    );
}

// ───────────────────────────── Inbound ─────────────────────────────

#[test]
fn test_config_arrival_projects_onto_the_orchestrator_and_the_agent() {
    let (mut state, _storage) = fresh_state();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        root_folder: "/tmp/library".to_string(),
        name: "Extra".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 1,
    });
    config.inline_editor_enabled = false;

    // Populate the tree cache first so the invalidation below is observable.
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );
    let before = state
        .app
        .cached_tree_rows
        .clone()
        .expect("the frame tail populated the tree cache");

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::ConfigArrived(Arc::new(config))),
    );

    assert!(
        state
            .app
            .orchestrator
            .content_libraries
            .iter()
            .any(|lib| lib.name == "Extra")
    );
    assert!(!state.app.orchestrator.inline_editor_enabled);
    // The arrival invalidates the tree cache, and `finish_frame` re-derives the
    // rows inside the same reduction - so the observable end state is a NEW
    // cache rather than a lingering `tree_dirty` flag. The frame tail clears
    // that flag before the view ever sees the frame.
    let after = state
        .app
        .cached_tree_rows
        .clone()
        .expect("the frame tail re-derived the tree cache");
    assert!(
        !Arc::ptr_eq(&before, &after),
        "the config arrival must invalidate the flattened tree rows"
    );
    assert!(
        state
            .app
            .orchestrator
            .dialogs
            .batch_dialog_config
            .available_dirs
            .contains(&PathBuf::from("/tmp/library"))
    );
}

#[test]
fn test_config_arrival_marks_the_tree_dirty() {
    // The projection itself, isolated from the frame tail that consumes the flag.
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.selection.tree_dirty = false;

    apply_config_arrival(&mut state, &AppConfig::default());

    assert!(
        state.app.orchestrator.selection.tree_dirty,
        "a new config must invalidate the tree so the next frame re-derives it"
    );
}

#[test]
fn test_file_change_inbound_drains_the_workspace() {
    let (mut state, _storage) = fresh_state();
    let event =
        crate::bus::events::file::FileChanged::discovered_one(PathBuf::from("/notes/new.md"));

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::FileChanged(event)),
    );

    assert!(
        state
            .app
            .orchestrator
            .workspace
            .all_files
            .iter()
            .any(|path| path == &PathBuf::from("/notes/new.md")),
        "the inbound trigger must run the per-frame drain"
    );
}

#[test]
fn test_background_process_file_load_populates_the_document() {
    let (mut state, _storage) = fresh_state();
    let path = PathBuf::from("/notes/loaded.md");
    // The reducer only ever sees a load result for the file the user selected
    // (`handle_file_selection` gates the spawn on it), so select first.
    state.app.orchestrator.selection.select_file(path.clone());
    let event = BackgroundEvent::Process(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Ok("---\ntitle: L\n---\nLoaded body".to_string()),
    });

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );

    assert_eq!(state.app.orchestrator.tabs.loaded_path, Some(path));
    // `handle_process_event` stores `Document::new(content).body()`
    // (`orchestrator.rs:537-539`), which keeps the newline after the closing
    // `---`. The viewer renders it as leading whitespace; the reducer must not
    // rewrite what the shared orchestrator produced.
    assert_eq!(
        state.app.orchestrator.tabs.current_markdown,
        "\nLoaded body"
    );
    assert!(state.app.orchestrator.tabs.current_yaml.is_some());
}

/// Regression: a late load result for a file the user has already switched away
/// from must leave the open document and the ToC snapshot untouched. This is the
/// reducer-level guard behind "the ToC pane randomly turns on and off".
#[test]
fn test_background_process_file_load_for_a_deslected_file_is_dropped() {
    let (mut state, _storage) = fresh_state();
    let abandoned = PathBuf::from("/notes/abandoned.md");
    let current = PathBuf::from("/notes/current.md");

    state
        .app
        .orchestrator
        .selection
        .select_file(current.clone());
    state.app.orchestrator.pending_file_load = Some(current.clone());

    let event = BackgroundEvent::Process(ProcessEvent::FileLoaded {
        path: abandoned,
        content: Ok("# Abandoned\n\nMust not appear".to_string()),
    });
    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );

    assert_eq!(
        state.app.orchestrator.tabs.loaded_path, None,
        "the abandoned read must not repoint the open document"
    );
    assert!(
        state.app.orchestrator.tabs.current_markdown.is_empty(),
        "the abandoned body must not reach the viewer"
    );
    assert!(
        state.app.orchestrator.tabs.toc.is_empty(),
        "the abandoned headings must not light the ToC pane up"
    );
    assert_eq!(
        state.app.orchestrator.pending_file_load,
        Some(current),
        "the load still pending for the current selection must survive"
    );
}

#[test]
fn test_background_fs_event_reduces_without_panicking() {
    let (mut state, _storage) = fresh_state();
    let event = BackgroundEvent::Fs(FsEvent::FileParsed {
        path: PathBuf::from("/notes/a.md"),
        tags: vec!["tag".to_string()],
    });
    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );
}

#[test]
fn test_background_mcp_auth_event_reduces_without_panicking() {
    let (mut state, _storage) = fresh_state();
    let event = BackgroundEvent::McpAuth(McpAuthEvent::Completed {
        server_name: "srv".to_string(),
        error: None,
    });
    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );
}

#[test]
fn test_agent_bus_lag_marks_the_transcript_truncated() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::EventsDropped {
            source: BusSource::Agent,
            count: 12,
        }),
    );
    assert!(
        state
            .app
            .orchestrator
            .agent_transcript
            .thinking
            .contains(crate::orchestrator::LAG_TRUNCATION_MARKER),
        "the loss must be visible, not silent"
    );
}

#[test]
fn test_non_agent_bus_lag_does_not_touch_the_transcript() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::EventsDropped {
            source: BusSource::File,
            count: 3,
        }),
    );
    assert!(state.app.orchestrator.agent_transcript.thinking.is_empty());
}

// ───────────────────────────── Boot ─────────────────────────────

#[test]
fn test_boot_reports_reduce_without_panicking() {
    let (mut state, _storage) = fresh_state();
    for message in [
        BootMessage::FontLoadFinished(Some("boom".to_string())),
        BootMessage::FontLoadFinished(None),
    ] {
        let _ = update(&mut state, Message::Boot(message.clone()));
    }
}

// ───────────────────────────── frame tail ─────────────────────────────

#[test]
fn test_frame_tail_populates_the_tree_cache_and_clears_the_dirty_flag() {
    let (mut state, _storage) = fresh_state();
    assert!(state.app.cached_tree_rows.is_none());
    assert!(state.app.orchestrator.selection.tree_dirty);

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    assert!(
        state.app.cached_tree_rows.is_some(),
        "the cache transition must run in the tail"
    );
    assert!(
        !state.app.orchestrator.selection.tree_dirty,
        "a freshly built tree is no longer dirty"
    );
}

#[test]
fn test_frame_tail_adds_new_paths_to_the_existing_tree_index() {
    let (mut state, _storage) = fresh_state();
    let library_root = PathBuf::from("/virtual/library");
    state.app.orchestrator.content_libraries = vec![ContentLibrary {
        root_folder: library_root.to_string_lossy().into_owned(),
        name: "Library".to_string(),
        kind: "text".to_string(),
        readonly: true,
        priority: 0,
    }];
    state
        .app
        .orchestrator
        .selection
        .expanded_dirs
        .insert(library_root.clone());

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );
    let initial_tree = state
        .app
        .workspace_tree_cache
        .root
        .as_deref()
        .expect("the frame tail builds the tree index") as *const _;

    let docs = library_root.join("docs");
    let note = docs.join("new.md");
    assert!(state.app.orchestrator.workspace.add_dir(docs.clone()));
    assert!(state.app.orchestrator.workspace.add_file(note.clone()));
    state
        .app
        .orchestrator
        .selection
        .expanded_dirs
        .insert(docs.clone());
    state.app.orchestrator.selection.tree_dirty = true;

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    let updated_tree = state
        .app
        .workspace_tree_cache
        .root
        .as_deref()
        .expect("the tree index remains available") as *const _;
    assert_eq!(
        initial_tree, updated_tree,
        "additions must preserve the tree index"
    );
    assert!(state.models.left.rows.iter().any(|row| row.path == docs));
    assert!(state.models.left.rows.iter().any(|row| row.path == note));
}

#[test]
fn test_frame_tail_rebuilds_the_tree_index_after_a_file_removal() {
    let (mut state, _storage) = fresh_state();
    let library_root = PathBuf::from("/virtual/library");
    state.app.orchestrator.content_libraries = vec![ContentLibrary {
        root_folder: library_root.to_string_lossy().into_owned(),
        name: "Library".to_string(),
        kind: "text".to_string(),
        readonly: true,
        priority: 0,
    }];
    let note = library_root.join("old.md");
    assert!(state.app.orchestrator.workspace.add_file(note.clone()));
    state
        .app
        .orchestrator
        .selection
        .expanded_dirs
        .insert(library_root.clone());
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );
    let initial_reset_revision = state
        .app
        .workspace_tree_cache
        .reset_revision
        .expect("the frame tail records the tree index revision");
    let initial_file_count = state.app.workspace_tree_cache.indexed_file_count;

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::FileChanged(
            crate::bus::events::file::FileChanged::removed_one(note.clone()),
        )),
    );

    assert_eq!(state.app.workspace_tree_cache.indexed_file_count, 0);
    assert_ne!(
        state.app.workspace_tree_cache.reset_revision,
        Some(initial_reset_revision)
    );
    assert_eq!(initial_file_count, 1);
    assert!(
        state.models.left.rows.iter().all(|row| row.path != note),
        "removed files must not remain in the flattened tree"
    );
}

#[test]
fn test_frame_tail_does_not_rebuild_the_tree_while_it_is_clean() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    let rows = state.app.cached_tree_rows.clone().expect("cache present");
    assert_eq!(rows.len(), 1);
    assert_eq!(state.models.left.rows.len(), 1);
    assert_eq!(state.models.left.rows[0].path, PathBuf::from("/notes/a.md"));
}

#[test]
fn test_a_dirty_tree_is_re_derived_rather_than_served_from_the_stale_cache() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );
    let before = state.app.cached_tree_rows.clone().expect("cached");

    state.app.orchestrator.selection.tree_dirty = true;
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    let after = state.app.cached_tree_rows.clone().expect("cached again");
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a dirty tree must be re-derived, not served from the stale cache"
    );
    assert!(!state.app.orchestrator.selection.tree_dirty);
}

#[test]
fn test_every_update_rebuilds_the_view_models() {
    let (mut state, _storage) = fresh_state();
    let before = state.models.center.tabs.len();

    state
        .app
        .orchestrator
        .tabs
        .tabs
        .push(PathBuf::from("/notes/a.md"));
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    assert_eq!(
        state.models.center.tabs.len(),
        before + 1,
        "a domain change must be visible in the view-model after one message"
    );
}

#[test]
fn test_frame_tail_syncs_panel_widths_into_the_persisted_snapshot() {
    let (mut state, _storage) = fresh_state();
    state.app.layout.set_width(Some(211.0));
    state.app.layout.set_right_width(Some(161.0));

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    assert_eq!(state.app.persisted_ui_state.left_panel_width, Some(211.0));
    assert_eq!(
        state.app.persisted_ui_state.right_panel_width,
        Some(161.0),
        "finish_frame persists the UI state every frame"
    );
}

#[test]
fn test_frame_tail_joins_a_finished_batch_and_drops_the_cancel_flag() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.dialogs.batch_handle = Some(crate::agent::batch::types::BatchHandle {
        thread: finished_batch_handle(),
        cancel_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(0.0, 0.0),
        }),
    );

    assert!(
        state.app.orchestrator.dialogs.batch_handle.is_none(),
        "a finished batch is joined and released"
    );
    assert!(state.app.orchestrator.dialogs.batch_cancel_flag.is_none());
}

/// Spawns an already-finished batch worker and hands its join handle over.
///
/// `JoinHandle::join` consumes the handle, so the only way to give the reducer a
/// handle that reports `is_finished()` without consuming it is to spin on that
/// flag until the worker has exited — which keeps the frame-tail join
/// deterministic instead of racing the thread.
fn finished_batch_handle() -> std::thread::JoinHandle<crate::agent::batch::types::BatchResult> {
    let handle = std::thread::spawn(|| crate::agent::batch::types::BatchResult {
        total_jobs: 1,
        completed: 1,
        failed: 0,
        cancelled: 0,
        duration: std::time::Duration::from_millis(1),
    });
    while !handle.is_finished() {
        std::thread::yield_now();
    }
    handle
}

// --- Overlay channel: Esc dismissal + Shift tracking (UI-056, UI-022) ------

/// `Esc` reduced through the top-level `update` closes the top-most modal
/// and leaves the one beneath alone.
#[test]
fn test_escape_pressed_reduces_to_the_topmost_layer_only() {
    let (mut state, _storage) = fresh_state();
    state.overlay.tools.open();
    state.overlay.batch.open(BatchDialogViewModel::default());

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert!(
        !state.overlay.batch.is_open(),
        "the batch dialog sat on top and must have closed"
    );
    assert!(
        state.overlay.tools.is_open(),
        "the tools dialog underneath must survive one Esc"
    );
}

/// `Esc` with nothing open is a no-op reduction, not a panic.
#[test]
fn test_escape_pressed_with_nothing_open_is_a_no_op() {
    let (mut state, _storage) = fresh_state();
    let before = state.overlay.hamburger_open;

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert_eq!(state.overlay.hamburger_open, before);
    assert_eq!(state.overlay.close_topmost(), None);
}

/// The docked log viewer is not in the `Esc` stack: reducing `Esc` with a
/// visible log viewer and nothing else must leave it visible.
#[test]
fn test_escape_pressed_does_not_close_the_docked_logs_panel() {
    let (mut state, _storage) = fresh_state();
    state
        .app
        .orchestrator
        .background_manager
        .lock()
        .expect("background log buffer")
        .show_background_logs = true;
    state.rebuild_view_models();
    assert!(
        state.models.logs.visible,
        "fixture must show the log viewer"
    );

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert!(
        state.models.logs.visible,
        "a docked panel is not a modal and must not be Esc-closed"
    );
}

/// Reducing `ModifiersChanged { shift: true }` sets the `SHIFT` bit.
#[test]
fn test_modifiers_changed_sets_the_shift_bit_when_held() {
    let (mut state, _storage) = fresh_state();
    assert!(!state.overlay.modifiers.shift());

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: true }),
    );

    assert!(state.overlay.modifiers.shift(), "the SHIFT bit must be set");
}

/// Reducing `ModifiersChanged { shift: false }` clears the `SHIFT` bit.
#[test]
fn test_modifiers_changed_clears_the_shift_bit_when_released() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: true }),
    );
    assert!(state.overlay.modifiers.shift());

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: false }),
    );

    assert!(
        !state.overlay.modifiers.shift(),
        "the SHIFT bit must be cleared on release"
    );
}

/// Only the `SHIFT` bit is touched: a modifier the lane does not observe is
/// preserved across a Shift update.
#[test]
fn test_modifiers_changed_leaves_other_bits_untouched() {
    let (mut state, _storage) = fresh_state();
    state
        .overlay
        .modifiers
        .insert(iced::keyboard::Modifiers::CTRL);

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: true }),
    );
    assert!(state.overlay.modifiers.control(), "CTRL must survive");

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: false }),
    );
    assert!(
        state.overlay.modifiers.control() && !state.overlay.modifiers.shift(),
        "clearing Shift must not clear CTRL"
    );
}

/// UI-022 end to end at the view-model boundary: a held Shift projects into
/// `left.multi_select`, a released Shift does not.
#[test]
fn test_tracked_shift_propagates_into_left_panel_multi_select() {
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: true }),
    );
    assert!(
        state.models.left.multi_select,
        "Shift held must project into multi_select"
    );

    let _ = update(
        &mut state,
        Message::Overlay(OverlayMessage::ModifiersChanged { shift: false }),
    );
    assert!(
        !state.models.left.multi_select,
        "Shift released must clear multi_select"
    );
}

/// `ViewModels::from_app` overrides the `false` that
/// `LeftPanelViewModel::from_app` hardcodes, in both directions.
#[test]
fn test_view_models_from_app_overrides_multi_select_from_the_overlay() {
    let (mut state, _storage) = fresh_state();

    state.overlay.modifiers = iced::keyboard::Modifiers::empty();
    let models = ViewModels::from_app(&state.app, &state.overlay);
    assert!(!models.left.multi_select);

    state.overlay.modifiers = iced::keyboard::Modifiers::SHIFT;
    let models = ViewModels::from_app(&state.app, &state.overlay);
    assert!(
        models.left.multi_select,
        "the overlay modifier snapshot must win over from_app's default"
    );
}

/// An `Esc` message must not be mistaken for context-menu-dismissing intent
/// by the pre-match guard — `close_topmost` owns that transition instead.
#[test]
fn test_escape_pressed_is_not_a_context_menu_dismissing_command() {
    assert!(
        !dismisses_context_menu(&Message::Overlay(OverlayMessage::EscapePressed)),
        "the generic dismiss guard must not pre-empt close_topmost"
    );
    assert!(!dismisses_context_menu(&Message::Overlay(
        OverlayMessage::ModifiersChanged { shift: true }
    )));
}

// =========================================================================
// UIF-MOD-002 - the editor's unsaved-edit guard on the scrim / dialog paths
// =========================================================================

/// Open a clean editor buffer on a fixed path.
fn seed_clean_editor(state: &mut AppState) {
    state.overlay.editor = Some(EditorViewModel::open("M:\\lib\\note.md", "body"));
}

/// Dirty the open buffer through the real keystroke path.
fn seed_dirty_editor(state: &mut AppState) {
    seed_clean_editor(state);
    state
        .overlay
        .editor
        .as_mut()
        .expect("editor open")
        .apply_action(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Paste(Arc::new(" unsaved".to_string())),
        ));
    assert!(
        state.overlay.editor.as_ref().unwrap().is_dirty(),
        "the fixture must leave unsaved edits"
    );
}

/// The buffer's current text, for before/after comparisons.
fn editor_text(state: &AppState) -> String {
    state.overlay.editor.as_ref().expect("editor open").text()
}

/// A scrim click on a **clean** buffer still closes the editor - the
/// guard must not turn every dismissal into a question.
#[test]
fn scrim_dismiss_on_a_clean_editor_closes_it() {
    let (mut state, _storage) = fresh_state();
    seed_clean_editor(&mut state);

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        }),
    );

    assert!(
        state.overlay.editor.is_none(),
        "a clean buffer must dismiss on a scrim click exactly as before"
    );
    assert!(
        !state.overlay.dialog.is_open(),
        "a clean dismissal must not raise a confirmation"
    );
}

/// The core regression: a scrim click on a **dirty** buffer must not
/// discard it. It raises the discard confirmation instead.
#[test]
fn scrim_dismiss_on_a_dirty_editor_raises_the_confirmation_instead_of_discarding() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    let before = editor_text(&state);

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        }),
    );

    assert!(
        state.overlay.editor.is_some(),
        "UIF-MOD-002: a scrim click over unsaved edits must keep the editor open"
    );
    assert_eq!(editor_text(&state), before, "the buffer must be untouched");
    assert_eq!(
        state.overlay.dialog.kind(),
        Some(&DialogKind::DiscardEditorChanges {
            path: PathBuf::from("M:\\lib\\note.md"),
        }),
        "the refusal must ask through the INT-4 discard confirmation"
    );
    assert_eq!(
        state.overlay.dialog.kind().map(DialogKind::button_intent),
        Some(hoarfrost::surface::input_dialog::ButtonIntent::Danger),
        "discarding unsaved work must wear the Danger variant, not a neutral one"
    );
}

/// `Cancel` is the explicit discard the requirement names as its escape
/// hatch: the same dirty buffer the scrim refuses, Cancel closes.
#[test]
fn the_cancel_button_still_closes_a_dirty_editor() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);

    let _ = update(&mut state, Message::Editor(EditorMessage::CancelRequested));

    assert!(
        state.overlay.editor.is_none(),
        "Cancel is the user saying throw-it-away explicitly; the guard must not block it"
    );
    assert!(
        !state.overlay.dialog.is_open(),
        "an explicit Cancel must not escalate into a confirmation"
    );
}

/// Answering the confirmation "Discard" closes the buffer.
#[test]
fn confirming_the_discard_closes_the_editor() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        }),
    );
    assert!(state.overlay.dialog.is_open());

    let _ = update(&mut state, Message::Dialog(DialogMessage::Submitted));

    assert!(
        state.overlay.editor.is_none(),
        "Discard must actually discard the buffer"
    );
    assert!(
        !state.overlay.dialog.is_open(),
        "and the question must close with it"
    );
}

/// Answering the confirmation "Cancel" closes only the question. The
/// edits stay exactly where they were.
#[test]
fn cancelling_the_discard_confirmation_leaves_the_editor_open() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    let before = editor_text(&state);
    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        }),
    );

    let _ = update(&mut state, Message::Dialog(DialogMessage::Cancelled));

    assert!(
        state.overlay.editor.is_some(),
        "backing out of the question must never discard the work it asked about"
    );
    assert_eq!(editor_text(&state), before);
    assert!(!state.overlay.dialog.is_open());
}

/// A press absorbed by the card boundary changes nothing at all - not the
/// buffer, not the overlay, not the dialog (`UIF-INP-002`).
#[test]
fn an_absorbed_press_changes_nothing() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    let before = editor_text(&state);

    let _ = update(&mut state, Message::Editor(EditorMessage::Absorbed));

    assert_eq!(editor_text(&state), before);
    assert!(!state.overlay.dialog.is_open());
    assert!(state.overlay.editor.is_some());
}

/// `Escape` through the reducer takes the same guarded path as the scrim:
/// a dirty buffer survives it.
#[test]
fn escape_through_the_reducer_does_not_discard_a_dirty_editor() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    let before = editor_text(&state);

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert!(
        state.overlay.editor.is_some(),
        "UIF-MOD-002: the reducer's Escape path must not drop unsaved edits either"
    );
    assert_eq!(editor_text(&state), before);
    assert!(state.overlay.dialog.is_open());
}

/// A retry the worker accepted clears the banner: the user must not be
/// left looking at a stale failure after a save that worked.
#[test]
fn recovery_retry_clears_the_banner_when_the_worker_accepts() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    state
        .overlay
        .editor
        .as_mut()
        .expect("editor open")
        .set_save_error(crate::ui_iced::editor::save_queue_error());
    assert!(
        state
            .overlay
            .editor
            .as_ref()
            .unwrap()
            .save_error()
            .is_some()
    );

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::RecoveryRequested {
            path: PathBuf::from("M:\\lib\\note.md"),
            recovery: Recovery::Retry,
        }),
    );

    assert!(
        state
            .overlay
            .editor
            .as_ref()
            .unwrap()
            .save_error()
            .is_none(),
        "an accepted retry must clear the failure the banner is showing"
    );
}

/// `Recovery::FocusControl` must produce exactly one runtime task - a
/// recovery affordance that hands back nothing is a button that does
/// nothing.
///
/// Driven through `run_recovery` directly rather than `update`, because
/// `iced::Task` exposes no way to inspect what it carries once it is
/// wrapped; the dispatch's own `Vec` is the last place the count is
/// observable.
#[test]
fn recovery_focus_control_produces_exactly_one_task() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);

    let tasks = run_recovery(
        &mut state,
        &PathBuf::from("M:\\lib\\note.md"),
        Recovery::FocusControl(hoarfrost::focus::FocusId::new(
            crate::ui_iced::editor::EDITOR_TEXT_ID,
        )),
    );

    assert_eq!(
        tasks.len(),
        1,
        "the focus recovery must hand iced exactly one operation"
    );
    assert!(
        state
            .overlay
            .editor
            .as_ref()
            .unwrap()
            .save_error()
            .is_none(),
        "focusing the buffer must not touch the banner"
    );
}

/// `Recovery::Retry` re-runs the save rather than focusing anything, so
/// it queues work and returns no task of its own.
#[test]
fn recovery_retry_re_runs_the_save() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);

    let tasks = run_recovery(
        &mut state,
        &PathBuf::from("M:\\lib\\note.md"),
        Recovery::Retry,
    );

    assert!(tasks.is_empty(), "the retry is queued, not deferred");
    assert!(
        state
            .overlay
            .editor
            .as_ref()
            .unwrap()
            .save_error()
            .is_none(),
        "an accepted retry must clear the failure the banner is showing"
    );
}

/// `Recovery::Acknowledge` clears the banner without re-running anything.
#[test]
fn recovery_acknowledge_clears_the_banner() {
    let (mut state, _storage) = fresh_state();
    seed_dirty_editor(&mut state);
    state
        .overlay
        .editor
        .as_mut()
        .expect("editor open")
        .set_save_error(crate::ui_iced::editor::save_queue_error());

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::RecoveryRequested {
            path: PathBuf::from("M:\\lib\\note.md"),
            recovery: Recovery::Acknowledge,
        }),
    );

    assert!(
        state
            .overlay
            .editor
            .as_ref()
            .unwrap()
            .save_error()
            .is_none()
    );
}

/// Both halves of the save-outcome decision, pinned directly: the worker
/// queue has no seam for forcing a refusal, so the refused branch is
/// covered here rather than left assumed.
#[test]
fn record_save_outcome_surfaces_a_refusal_and_clears_an_acceptance() {
    let mut vm = EditorViewModel::open("M:\\lib\\note.md", "body");

    record_save_outcome(&mut vm, true);
    assert!(
        vm.save_error().is_none(),
        "an accepted save must leave no banner behind"
    );

    record_save_outcome(&mut vm, false);
    let error = vm.save_error().expect("a refused save must be surfaced");
    assert_eq!(error.code(), crate::ui_iced::editor::EDITOR_SAVE_ERROR_CODE);
    assert_eq!(error.recovery(), &Recovery::Retry);

    record_save_outcome(&mut vm, true);
    assert!(
        vm.save_error().is_none(),
        "a later accepted save must clear the earlier refusal"
    );
}

// ---------------------------------------------------------------------------
// INT-10 — tree-row hover reduction (UIF-TOK-001 hover rung)
// ---------------------------------------------------------------------------

/// Rows seeded so the hover prune has something to keep.
fn int10_rows() -> Vec<FlatRow> {
    vec![file_row("/ws/alpha.md"), file_row("/ws/beta.md")]
}

#[test]
fn hovering_a_tree_row_records_the_path_and_publishes_no_command() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());
    let spy = command_spy(&state);

    let _tasks = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md")),
        "the reducer must own the pointer, since the pure view cannot"
    );
    // `iced::Task` carries nothing inspectable once wrapped (see
    // `recovery_focus_control_produces_exactly_one_task`), so "no side
    // effect" is asserted where it is observable: the command bus.
    assert!(
        drained(&spy).is_empty(),
        "hovering a row must not publish anything on the user-command bus"
    );
}

#[test]
fn the_hovered_path_reaches_the_pure_view_model() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/beta.md"),
        }),
    );

    // §6: the view branches on the view-model, never on widget state.
    // This projection is the whole bridge from the pointer to the band.
    assert_eq!(
        state.models.left.hovered_row,
        Some(PathBuf::from("/ws/beta.md")),
        "the snapshot the panel renders must carry the hovered path"
    );
}

#[test]
fn leaving_the_hovered_row_clears_it() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowLeft {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tree_row, None,
        "crossing out of the hovered row must clear it"
    );
    assert_eq!(
        state.models.left.hovered_row, None,
        "and the view-model must follow, or the highlight strands"
    );
}

#[test]
fn leaving_a_different_row_cannot_wipe_the_highlight_that_just_arrived() {
    // The upward-move race. iced publishes both crossings in one event
    // pass in tree order, so moving the pointer UP from beta to alpha
    // delivers alpha's RowEntered BEFORE beta's RowLeft. A reducer that
    // cleared unconditionally would erase alpha in the same tick.
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowLeft {
            path: PathBuf::from("/ws/beta.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md")),
        "a stale exit from the row we came from must not clear the new hover"
    );
}

#[test]
fn re_entering_after_leaving_records_the_row_again() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowLeft {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    assert_eq!(state.overlay.hovered_tree_row, None, "mid-check");

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md")),
        "re-entry must re-report; a one-shot latch would leave the tree dead"
    );
}

#[test]
fn a_hover_path_naming_a_row_that_no_longer_exists_is_pruned() {
    // The row's `mouse_area` cannot report an exit it no longer has:
    // the file was deleted, or a filter hid it. The prune is what stops
    // the path lingering in the reducer.
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md"))
    );

    // The tree changes underneath the hover.
    seed_rows(&mut state, vec![file_row("/ws/beta.md")]);
    let _ = update(&mut state, Message::Tree(TreeMessage::ContextMenuClosed));

    assert_eq!(
        state.overlay.hovered_tree_row, None,
        "a hover path the tree cannot render must be pruned, not carried"
    );
}

#[test]
fn the_prune_keeps_a_hover_that_is_still_renderable() {
    // Non-vacuity for the prune above: the same message with the row
    // still present must leave the hover intact.
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(&mut state, Message::Tree(TreeMessage::ContextMenuClosed));

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md")),
        "the prune must not be a blanket clear"
    );
}

#[test]
fn repeated_hover_reports_are_idempotent_and_owe_nothing() {
    // The tree rebuilds every frame. Even if the same enter were
    // delivered repeatedly, the state must not drift or accumulate.
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());
    let spy = command_spy(&state);

    for _ in 0..5 {
        let _tasks = update(
            &mut state,
            Message::Tree(TreeMessage::RowEntered {
                path: PathBuf::from("/ws/alpha.md"),
            }),
        );
    }

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/alpha.md")),
        "five identical enters leave exactly the same state as one"
    );
    assert!(
        drained(&spy).is_empty(),
        "and still nothing reached the command bus"
    );
}

#[test]
fn hover_survives_a_re_select_because_it_is_not_selection_state() {
    // Hover and selection are independent axes: the row under the
    // cursor stays highlighted while the selection changes elsewhere,
    // and `resolve_style` is what ranks them, not the reducer.
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, int10_rows());

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RowEntered {
            path: PathBuf::from("/ws/beta.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Command(UserCommand::SelectFile {
            path: PathBuf::from("/ws/alpha.md"),
            multi: false,
        }),
    );

    assert_eq!(
        state.overlay.hovered_tree_row,
        Some(PathBuf::from("/ws/beta.md")),
        "selecting another file must not steal the pointer's row"
    );
    assert_eq!(
        state.models.left.selected_file,
        Some(PathBuf::from("/ws/alpha.md")),
        "and the selection did land"
    );
}

// ---------------------------------------------------------------------------
// INT-12 — the tab strip's hover crossing (§4.4.1 slot 1's third state)
// ---------------------------------------------------------------------------

#[test]
fn entering_a_tab_records_it_as_hovered() {
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Entered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tab,
        Some(PathBuf::from("/ws/alpha.md")),
        "the pointer's tab must reach overlay state"
    );
    assert_eq!(
        state.models.center.hovered_tab,
        Some(PathBuf::from("/ws/alpha.md")),
        "and be projected into the center view-model the panel reads"
    );
}

#[test]
fn leaving_the_hovered_tab_clears_it() {
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Entered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Left {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(state.overlay.hovered_tab, None);
    assert_eq!(state.models.center.hovered_tab, None);
}

#[test]
fn leaving_a_different_tab_cannot_wipe_the_highlight_that_just_arrived() {
    // The same race INT-10 pinned for tree rows. iced publishes both
    // crossings in one event pass in tree order, so a move toward the
    // end of the strip delivers the later tab's `Entered` before the
    // earlier tab's `Left`. An unconditional clear would erase the
    // highlight in the same tick it appeared.
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Entered {
            path: PathBuf::from("/ws/beta.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Left {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.overlay.hovered_tab,
        Some(PathBuf::from("/ws/beta.md")),
        "a stale exit from the tab we came from must not clear the new hover"
    );
}

#[test]
fn a_hover_report_publishes_no_bus_command() {
    // The hover is view-local state, not user intent ([FMD-068]). It must
    // not be able to close, select or otherwise act on a tab.
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Entered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Left {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert!(
        drained(&spy).is_empty(),
        "a pointer crossing must not publish anything on the command bus"
    );
}

#[test]
fn hovering_a_tab_does_not_change_which_tab_is_active() {
    let (mut state, _storage) = fresh_state();

    let _ = update(
        &mut state,
        Message::Tab(TabMessage::Entered {
            path: PathBuf::from("/ws/alpha.md"),
        }),
    );

    assert_eq!(
        state.models.center.active_tab, None,
        "hover is not activation"
    );
}

// ------------------------- observability (NFR-001 / NFR-004) -----------------
//
// Each test forces one menu failure route and asserts the [NFR-004] code and the
// level it was chosen at. The level is part of the contract: [NFR-001] demands
// ERROR for failures, and a benign refusal logged at ERROR would be spam.

/// The trigger label the hamburger open route is expected to report.
const HAMBURGER_TRIGGER: &str = TRIGGER_HAMBURGER_OPEN;

#[test]
fn test_hamburger_open_success_logs_no_error_code() {
    let (mut state, _storage) = fresh_state();
    let capture = Capture::new();
    with_capture(&capture, || {
        let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    });
    assert!(
        state.overlay.hamburger_open,
        "the hamburger must actually open for this to prove silence-on-success"
    );
    assert!(
        capture.codes().is_empty(),
        "a successful open must log no error code, got {:?}",
        capture.codes()
    );
}

#[test]
fn test_hamburger_reopen_is_reported_as_already_open_at_debug() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    assert!(state.overlay.hamburger_open);

    let capture = Capture::new();
    with_capture(&capture, || {
        let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    });

    let event = capture
        .with_code(menu_codes::HAMBURGER_ALREADY_OPEN)
        .expect("a refused re-open must name itself");
    assert_eq!(
        event.level,
        tracing::Level::DEBUG,
        "an idempotent re-open is not a failure; ERROR here would be spam"
    );
    assert_eq!(event.field("trigger"), Some(HAMBURGER_TRIGGER));
    assert_eq!(event.field("surface"), Some("hamburger"));
    assert!(
        event.field("row_count").is_some(),
        "the line must report how many rows the menu had"
    );
    assert!(
        state.overlay.hamburger_open,
        "the refusal must leave the already-open menu mounted"
    );
}

#[test]
fn test_hamburger_open_with_no_focusable_row_is_reported_as_error() {
    // A real cascade refusal: a card whose every row is a heading or a
    // separator cannot hold the keyboard, so `open` refuses it.
    let menu = Menu::new(
        "menu.obs.no_rows",
        vec![Row::heading("Section"), Row::separator()],
    );
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let outcome = cascade.open(&menu, MenuTreeKind::Fixed, &mut focus);
    assert!(
        !cascade.is_open(),
        "precondition: the cascade must actually refuse this menu"
    );

    let capture = Capture::new();
    with_capture(&capture, || {
        log_hamburger_open(&menu, &outcome, Size::new(1200.0, 800.0));
    });

    let event = capture
        .with_code(menu_codes::HAMBURGER_NO_FOCUSABLE_ROWS)
        .expect("a menu that cannot mount must report ERROR ([NFR-001])");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(event.field("trigger"), Some(HAMBURGER_TRIGGER));
    assert_eq!(event.field("viewport_w"), Some("1200.0"));
    assert_eq!(event.field("viewport_h"), Some("800.0"));
    assert_eq!(event.field("row_count"), Some("2"));
}

#[test]
fn test_hamburger_open_refused_by_an_unexpected_reason_is_reported_as_error() {
    // MENU-1003: the builder and the cascade disagree. `open` itself only
    // returns AlreadyOpen / NoFocusableRows, so anything else reaching the
    // reporter is a contradiction and must be loud.
    let menu = Menu::new("menu.obs.contradiction", vec![Row::action("a", "A")]);
    for (reason, label, row) in [
        (
            IgnoredReason::UnknownCard {
                card: MenuId::new("menu.obs.gone"),
            },
            "unknown_card",
            "None",
        ),
        (
            IgnoredReason::RowOutOfRange {
                card: MenuId::new("menu.obs.contradiction"),
                row: 7,
            },
            "row_out_of_range",
            "Some(7)",
        ),
    ] {
        let capture = Capture::new();
        with_capture(&capture, || {
            log_hamburger_open(&menu, &CascadeOutcome::ignored(reason), Size::new(1.0, 1.0));
        });
        let event = capture
            .with_code(menu_codes::HAMBURGER_OPEN_REFUSED)
            .expect("an unexpected refusal must report ERROR ([NFR-001])");
        assert_eq!(event.level, tracing::Level::ERROR);
        assert_eq!(
            event.field("reason"),
            Some(label),
            "the reason must arrive as a stable label"
        );
        assert_eq!(event.field("reason_row"), Some(row));
    }
}

#[test]
fn test_hamburger_open_refusal_never_prints_a_path_bearing_id() {
    // The reporter is menu-agnostic: if a context-menu id ever reaches it,
    // the [NFR-009] redaction still has to hold.
    let marker = "secret-person";
    let menu = Menu::new(
        format!(
            "{}.dir.C:\\Users\\{marker}\\notes",
            crate::ui_iced::context_menu::CONTEXT_MENU_ID_PREFIX
        ),
        vec![Row::action("a", "A")],
    );
    let capture = Capture::new();
    with_capture(&capture, || {
        log_hamburger_open(
            &menu,
            &CascadeOutcome::ignored(IgnoredReason::NothingFocused {
                card: menu.id().clone(),
            }),
            Size::new(1.0, 1.0),
        );
    });
    let rendered = capture
        .events()
        .iter()
        .map(|event| format!("{} {:?}", event.message, event.fields))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        capture
            .with_code(menu_codes::HAMBURGER_OPEN_REFUSED)
            .is_some(),
        "the route under test must actually have fired"
    );
    assert!(
        !rendered.contains(marker),
        "no hamburger log line may carry the user's path, got {rendered}"
    );
}

#[test]
fn test_tree_context_menu_for_unknown_path_is_reported_as_error() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(40.0, 90.0),
        }),
    );

    let capture = Capture::new();
    with_capture(&capture, || {
        open_tree_context_menu(&mut state, &PathBuf::from("/notes/ghost.md"));
    });

    let event = capture
        .with_code(menu_codes::CONTEXT_TREE_ROW_MISSING)
        .expect("a context menu requested for a missing row must report ERROR");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(event.field("trigger"), Some(TRIGGER_TREE_CONTEXT_MENU));
    assert_eq!(event.field("path"), Some("/notes/ghost.md"));
    assert_eq!(event.field("anchor_x"), Some("40.0"));
    assert_eq!(event.field("anchor_y"), Some("90.0"));
    assert!(state.overlay.context_menu.is_none());
}

#[test]
fn test_tab_context_menu_for_unknown_index_is_reported_as_error() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(12.0, 34.0),
        }),
    );

    let capture = Capture::new();
    with_capture(&capture, || {
        reduce_tab(&mut state, TabMessage::ContextMenuOpened { index: 42 });
    });

    let event = capture
        .with_code(menu_codes::CONTEXT_TAB_MISSING)
        .expect("a tab menu for an out-of-range index must report ERROR");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(event.field("trigger"), Some(TRIGGER_TAB_CONTEXT_MENU));
    assert_eq!(event.field("index"), Some("42"));
    assert_eq!(event.field("tab_count"), Some("0"));
}

#[test]
fn test_missing_cursor_anchor_is_reported_as_warn_not_error() {
    let (mut state, _storage) = fresh_state();
    state.overlay.cursor = None;

    let capture = Capture::new();
    let anchor = with_capture(&capture, || {
        anchored_cursor_or_origin(&state, TRIGGER_TREE_CONTEXT_MENU)
    });

    assert_eq!(anchor, Point::default(), "the fallback is unchanged");
    let event = capture
        .with_code(menu_codes::CONTEXT_ANCHOR_MISSING)
        .expect("taking the origin fallback must be visible");
    assert_eq!(
        event.level,
        tracing::Level::WARN,
        "the menu still opens, so this is degraded rather than broken"
    );
    assert_eq!(event.field("trigger"), Some(TRIGGER_TREE_CONTEXT_MENU));
}

#[test]
fn test_reported_cursor_anchor_logs_nothing() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(7.0, 11.0),
        }),
    );
    let capture = Capture::new();
    let anchor = with_capture(&capture, || {
        anchored_cursor_or_origin(&state, TRIGGER_TREE_CONTEXT_MENU)
    });
    assert_eq!(anchor, Point::new(7.0, 11.0));
    assert!(
        capture.codes().is_empty(),
        "a reported cursor must not warn, got {:?}",
        capture.codes()
    );
}

#[test]
fn test_row_pick_with_no_context_menu_is_reported_as_warn() {
    let (mut state, _storage) = fresh_state();
    state.overlay.hamburger_open = false;
    state.overlay.context_menu = None;

    let capture = Capture::new();
    let tasks = with_capture(&capture, || {
        route_context_row(&mut state, &MenuId::new("menu.ghost"), 3)
    });

    assert!(tasks.is_empty(), "behaviour unchanged: nothing is routed");
    let event = capture
        .with_code(menu_codes::ROW_PICK_NO_SURFACE)
        .expect("a pick against a dismissed surface must name itself");
    assert_eq!(
        event.level,
        tracing::Level::WARN,
        "a stale click during the dismiss race is racy, not broken"
    );
    assert_eq!(event.field("trigger"), Some(TRIGGER_ROW_PICKED));
    assert_eq!(event.field("row"), Some("3"));
}

#[test]
fn test_context_row_missing_from_the_routing_table_is_reported_as_error() {
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row("/notes/a.md")]);
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(20.0, 30.0),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from("/notes/a.md"),
        }),
    );

    // Force the builder gap: a live card whose routing table cannot answer.
    let mut menu = state.overlay.context_menu.take().expect("the menu opened");
    let root = crate::ui_iced::context_menu::root_id(&menu.target);
    assert!(
        !menu.routes.is_empty(),
        "precondition: a real menu has routes to strip"
    );
    menu.routes.clear();
    state.overlay.context_menu = Some(menu);

    let capture = Capture::new();
    let tasks = with_capture(&capture, || route_context_row(&mut state, &root, 0));

    assert!(tasks.is_empty(), "behaviour unchanged: nothing is routed");
    let event = capture
        .with_code(menu_codes::ROW_INTENT_MISSING)
        .expect("a live row that resolves to no intent must report ERROR");
    assert_eq!(
        event.level,
        tracing::Level::ERROR,
        "the user clicked a live row and nothing happened: a failure"
    );
    assert_eq!(event.field("trigger"), Some(TRIGGER_ROW_PICKED));
    assert_eq!(event.field("row"), Some("0"));
}

#[test]
fn test_key_navigation_with_no_context_menu_is_reported_as_warn() {
    let (mut state, _storage) = fresh_state();
    state.overlay.context_menu = None;

    let capture = Capture::new();
    let tasks = with_capture(&capture, || {
        route_context_key(&mut state, hoarfrost::surface::menu::cascade::MenuKey::Down)
    });

    assert!(tasks.is_empty(), "behaviour unchanged: the key is dropped");
    let event = capture
        .with_code(menu_codes::KEY_NAV_NO_SURFACE)
        .expect("key nav against a dismissed surface must name itself");
    assert_eq!(event.level, tracing::Level::WARN);
    assert_eq!(event.field("trigger"), Some(TRIGGER_KEY));
}

#[test]
fn test_hamburger_activation_that_resolves_to_no_command_is_reported_as_error() {
    let (mut state, _storage) = fresh_state();
    let menu = Menu::new(
        "menu.obs.dead_row",
        vec![Row::action("no_such_command_key", "Dead row")],
    );
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.open(&menu, MenuTreeKind::Fixed, &mut focus);
    let card = menu.id().clone();
    let outcome = cascade.activate_row(&card, 0, &mut focus);
    assert!(
        outcome.activation().is_some(),
        "precondition: the row must actually activate"
    );

    let snapshot = Some(menu.clone());
    let capture = Capture::new();
    let tasks = with_capture(&capture, || {
        dispatch_activation(&mut state, &snapshot, &outcome, TRIGGER_ROW_PICKED)
    });

    assert!(
        tasks.is_empty(),
        "behaviour unchanged: no command is routed"
    );
    let event = capture
        .with_code(menu_codes::ACTIVATION_UNRESOLVED)
        .expect("an activation with no command must report ERROR");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(event.field("trigger"), Some(TRIGGER_ROW_PICKED));
    assert_eq!(event.field("row"), Some("0"));
    assert_eq!(event.field("card_snapshot_present"), Some("true"));
}

#[test]
fn test_non_activating_key_logs_no_unresolved_activation() {
    // The companion guard: `dispatch_activation` must not cry wolf when the
    // outcome simply carried no activation.
    let (mut state, _storage) = fresh_state();
    let outcome = CascadeOutcome::noop();
    let capture = Capture::new();
    let tasks = with_capture(&capture, || {
        dispatch_activation(&mut state, &None, &outcome, TRIGGER_KEY)
    });
    assert!(tasks.is_empty());
    assert!(
        capture.codes().is_empty(),
        "a no-op outcome must log nothing, got {:?}",
        capture.codes()
    );
}

#[test]
fn test_hover_while_hamburger_is_closed_is_reported_as_debug() {
    let (mut state, _storage) = fresh_state();
    state.overlay.hamburger_open = false;

    let capture = Capture::new();
    let tasks = with_capture(&capture, || {
        reduce_menu(
            &mut state,
            MenuMessage::RowEntered {
                card: MenuId::new("menu.ghost"),
                row: 0,
            },
        )
    });

    assert!(
        tasks.is_empty(),
        "behaviour unchanged: the hover is ignored"
    );
    assert!(
        capture.has_code_at(menu_codes::HOVER_WHEN_MENU_CLOSED, tracing::Level::DEBUG),
        "expected a DEBUG {} for the ignored hover, got {:?}",
        menu_codes::HOVER_WHEN_MENU_CLOSED,
        capture.codes()
    );
}

#[test]
fn test_submenu_hover_that_opens_nothing_is_reported_as_debug() {
    let root = MenuId::new("aim.obs");
    let child = Menu::new(submenu_id(&root, "child"), vec![Row::action("c1", "C1")]);
    let menu = Menu::new(
        root.clone(),
        vec![
            Row::action("first", "First"),
            Row::submenu("child", "Child", child),
        ],
    );
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.open(&menu, MenuTreeKind::Fixed, &mut focus);

    let capture = Capture::new();
    with_capture(&capture, || {
        // A submenu parent whose branch did not open: logs.
        log_submenu_hover(&cascade, &root, 1, false, TRIGGER_ROW_ENTERED);
        // A plain action row: nothing to report.
        log_submenu_hover(&cascade, &root, 0, false, TRIGGER_ROW_ENTERED);
        // A submenu parent that did switch: nothing to report.
        log_submenu_hover(&cascade, &root, 1, true, TRIGGER_ROW_ENTERED);
    });

    let codes = capture.codes();
    assert_eq!(
        codes,
        vec![menu_codes::SUBMENU_HOVER_NOT_OPENED.to_string()],
        "only the unopened submenu-parent hover may log"
    );
    assert!(capture.has_code_at(menu_codes::SUBMENU_HOVER_NOT_OPENED, tracing::Level::DEBUG));
    let event = capture
        .with_code(menu_codes::SUBMENU_HOVER_NOT_OPENED)
        .expect("recorded above");
    assert_eq!(event.field("row"), Some("1"));
    assert_eq!(event.field("trigger"), Some(TRIGGER_ROW_ENTERED));
}
#[test]
fn test_context_menu_failure_logs_do_not_leak_the_user_path() {
    // Context-menu `MenuId`s are built as `file.<path>` / `dir.<path>`, so a
    // line that prints the card id verbatim ships the user's path out of the
    // app boundary ([NFR-009]). Every context-menu route must identify the
    // failure without it.
    let marker = "PII-CANARY-should-not-appear";
    let path = format!("/Users/{marker}/a.md");
    let (mut state, _storage) = fresh_state();
    seed_rows(&mut state, vec![file_row(&path)]);
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(1.0, 2.0),
        }),
    );
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: PathBuf::from(&path),
        }),
    );

    let mut menu = state.overlay.context_menu.take().expect("the menu opened");
    let root = crate::ui_iced::context_menu::root_id(&menu.target);
    assert!(
        root.to_string().contains(marker),
        "precondition: the card id itself carries the user's path"
    );
    menu.routes.clear();
    state.overlay.context_menu = Some(menu);

    let capture = Capture::new();
    with_capture(&capture, || {
        let _ = route_context_row(&mut state, &root, 0);
        state.overlay.context_menu = None;
        let _ = route_context_row(&mut state, &root, 1);
        let _ = route_context_key(&mut state, MenuKey::Down);
    });

    let rendered = capture
        .events()
        .iter()
        .map(|event| format!("{} {:?}", event.message, event.fields))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        capture.codes().len() >= 2,
        "the routes under test must actually have fired, got {:?}",
        capture.codes()
    );
    assert!(
        !rendered.contains(marker),
        "no context-menu failure route may print the user's path, got:\n{rendered}"
    );
}
