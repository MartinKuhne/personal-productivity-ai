//! Unit tests for the [TASK-023] iced reducer (`src/app/ui_iced/update.rs`).
//!
//! One-to-one coverage of the egui `FastMdApp::update_ui` mirror, channel
//! by channel. The Elm loop's runtime tasks (window close, font load) need
//! a live iced runtime and stay manual-verify items ([TASK-009] testing
//! rule 3); everything else — state reduction, bus publish/execute,
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
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui::tree::flatten::FlatRow;
use crate::ui_iced::app::{AppState, OverlayLayer, build_state};
use crate::ui_iced::batch_dialog::BatchDialogViewModel;
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::message::BootMessage;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
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
/// can see exactly which intents the reducer published ([FMD-066]).
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

/// Resizes the state's viewport so width-dependent reductions have a
/// defined geometry.
fn with_viewport(state: &mut AppState, width: f32) {
    state.viewport = Size::new(width, 700.0);
}

/// Seeds the tree with rows **the way the reducer does**: into the
/// `FastMdApp::cached_tree_rows` cache with `tree_dirty` cleared, which is
/// exactly the cache transition `finish_frame` performs (egui's own,
/// `ui/panels/left.rs:209-210`).
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
        state.composed_scale_factor(),
        1.5 * 1.25,
        "DEC-008: baseline × persisted multiplier"
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
        "the intent must also be observable on the bus ([FMD-066])"
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
        "confirming publishes the self-contained intent ([FMD-068])"
    );
    assert!(
        !state.overlay.dialog.is_open(),
        "a completed submit closes the dialog"
    );
}

#[test]
fn test_rename_opens_the_dialog_prefilled_with_eguis_initial_value() {
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
        "egui pre-filled the rename field; the iced reducer must too"
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
        "CancelBatch dismisses the dialog, as the egui window's close did"
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

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );

    let vm = state.overlay.editor.as_ref().expect("editor must open");
    assert_eq!(vm.path(), path.as_path());
    // egui parity: `TextBuffer::open` stores `DocumentContent::parse(raw).body`
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

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );
    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested { path: path.clone() }),
    );

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        original,
        "an untouched open   save must not change a single byte"
    );
}

#[test]
fn test_open_in_editor_does_not_mount_when_the_buffer_refused() {
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = false;

    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(PathBuf::from("/notes/a.md"))),
    );

    assert!(
        state.overlay.editor.is_none(),
        "with the inline editor disabled the command opens the OS editor instead"
    );
}

// ───────────────────────────── Tree ─────────────────────────────

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
        !menu.items.is_empty(),
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

#[test]
fn test_run_as_prompt_reads_the_body_and_runs_it_against_the_file() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("prompt.md");
    std::fs::write(&path, "---\ntitle: T\n---\nSummarise this\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RunAsPromptRequested { path: path.clone() }),
    );

    assert_eq!(
        drained(&spy),
        vec![UserCommand::RunSkillPrompt {
            // egui publishes `DocumentContent::parse(&raw).body` verbatim
            // (`ui/tree/render.rs:145`), and that body keeps the newline the
            // parser leaves after the closing `---`. One-to-one mirror.
            content: "\nSummarise this\n".to_string(),
            target_dir: None,
            target_file: Some(path),
        }],
        "front matter is stripped and the body becomes the prompt"
    );
}

#[test]
fn test_run_as_prompt_on_a_missing_file_logs_and_publishes_nothing() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: PathBuf::from("/definitely/not/here.md"),
        }),
    );

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

    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: skill.clone(),
            target_dir: Some(target.clone()),
            target_file: None,
        }),
    );

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
fn test_print_requested_hands_the_job_to_the_background_channel() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "# Doc\n").expect("write seed");

    let (mut state, _storage) = fresh_state();
    // No assertion on the print result is possible without the Typst CLI;
    // the contract under test is that the reduction completes without
    // panicking and without publishing a user command (there is none).
    let spy = command_spy(&state);
    let _ = update(
        &mut state,
        Message::Tree(TreeMessage::PrintRequested { path }),
    );
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
        "egui parity: one Delete per selected path, no confirm dialog"
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
    assert!(!menu.items.is_empty());

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

// ───────────────────────────── Editor ─────────────────────────────

/// Opens an editor over a temp file with the inline editor enabled.
fn open_editor(dir: &tempfile::TempDir) -> (AppState, PathBuf) {
    let path = dir.path().join("edit.md");
    std::fs::write(&path, "---\ntitle: Keep\n---\nOriginal body\n").expect("write seed");
    let (mut state, _storage) = fresh_state();
    state.app.orchestrator.inline_editor_enabled = true;
    state.app.orchestrator.tabs.loaded_path = Some(path.clone());
    let _ = update(
        &mut state,
        Message::Command(UserCommand::OpenInEditor(path.clone())),
    );
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
        "egui render.rs:43-45 cleared the same pair"
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

    let _ = update(
        &mut state,
        Message::Editor(EditorMessage::SaveRequested { path: path.clone() }),
    );

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
        "egui's collapsing block starts open"
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
    assert_eq!(state.models.left.query, "needle");
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
        "a superseded scan must not paint under the newer term ([FMD-062])"
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
    // egui only ever changes `auto_scroll` through its checkbox
    // (`ui/background_logs.rs:115-118`) - a search edit leaves it alone - so
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
        "egui stores a fresh install's 1.0 as None so it is never misread"
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
        "egui applied the toggle to the in-memory source (P0-2)"
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
    state
        .app
        .orchestrator
        .file_processor
        .add_file(target.clone());
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
        "rows toggle independently, as egui's per-header memory did"
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
    // cache rather than a lingering `tree_dirty` flag. egui cleared that flag in
    // the left panel's render pass (`ui/panels/left.rs:210`); the iced frame
    // tail does it before the view ever sees the frame.
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
fn test_file_change_inbound_drains_the_file_processor() {
    let (mut state, _storage) = fresh_state();
    let event = crate::bus::events::file::FileEvent::discovered_one(PathBuf::from("/notes/new.md"));

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::FileChanged(event)),
    );

    assert!(
        state
            .app
            .orchestrator
            .file_processor
            .all_files
            .iter()
            .any(|path| path == &PathBuf::from("/notes/new.md")),
        "the inbound trigger must run the same drain egui ran per frame"
    );
}

#[test]
fn test_background_process_file_load_populates_the_document() {
    let (mut state, _storage) = fresh_state();
    let path = PathBuf::from("/notes/loaded.md");
    let event = BackgroundEvent::Process(ProcessEvent::FileLoaded {
        path: path.clone(),
        content: Ok("---\ntitle: L\n---\nLoaded body".to_string()),
    });

    let _ = update(
        &mut state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );

    assert_eq!(state.app.orchestrator.tabs.loaded_path, Some(path));
    // egui parity: `handle_process_event` stores `Document::new(content).body()`
    // (`orchestrator.rs:537-539`), which keeps the newline after the closing
    // `---`. The viewer renders it as leading whitespace; the reducer must not
    // rewrite what the shared orchestrator produced.
    assert_eq!(
        state.app.orchestrator.tabs.current_markdown,
        "\nLoaded body"
    );
    assert!(state.app.orchestrator.tabs.current_yaml.is_some());
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
        "[FMD-067] the loss must be visible, not silent"
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
        BootMessage::ConfigPublished,
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
        "the egui cache transition (`ui/panels/left.rs:209`) must run in the tail"
    );
    assert!(
        !state.app.orchestrator.selection.tree_dirty,
        "a freshly built tree is no longer dirty"
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
        "egui's update_persisted_ui_state ran every frame; so does finish_frame"
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
