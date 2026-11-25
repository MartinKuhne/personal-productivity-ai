//! Inbound and frame-tail tests: the background → UI reductions, the tree-row
//! cache and the per-frame scroll/batch follow-up that `update_tests.rs` does
//! not reach.

use super::*;
use crate::agent::batch::types::{BatchHandle, BatchResult};
use crate::background::UserFileOperationResult;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::typed::BackgroundEvent;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::app::build_state;
use crate::ui_iced::persisted::{InMemoryUiStateStorage, UiStateStorage};
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh `AppState` over an in-memory UI-state store.
fn fresh_state() -> (AppState, Arc<InMemoryUiStateStorage>) {
    let storage = Arc::new(InMemoryUiStateStorage::new());
    let state = build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::clone(&storage) as Arc<dyn UiStateStorage>,
        crate::ui::PersistedUiState {
            schema_version: CURRENT_SCHEMA_VERSION,
            ..Default::default()
        },
    );
    (state, storage)
}

/// An extra subscriber on the orchestrator's user-command bus.
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

/// A viewport geometry that sits at the end of its content.
fn at_the_bottom() -> ScrollSnapshot {
    ScrollSnapshot {
        offset_y: 100.0,
        viewport_height: 100.0,
        content_height: 200.0,
    }
}

/// A viewport geometry the user has scrolled back up into.
fn scrolled_away() -> ScrollSnapshot {
    ScrollSnapshot {
        offset_y: 0.0,
        viewport_height: 100.0,
        content_height: 200.0,
    }
}

/// An idle batch result, for a worker the test parks on a gate.
fn idle_batch_result() -> BatchResult {
    BatchResult {
        total_jobs: 0,
        completed: 0,
        failed: 0,
        cancelled: 0,
        duration: std::time::Duration::ZERO,
    }
}

/// A handle whose worker is parked on a channel the test still holds open, so
/// `is_finished()` is guaranteed false for as long as the gate lives.
fn running_batch_handle() -> (BatchHandle, std::sync::mpsc::Sender<()>) {
    let (gate_tx, gate_rx) = std::sync::mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _ = gate_rx.recv();
        idle_batch_result()
    });
    (
        BatchHandle {
            thread,
            cancel_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
        gate_tx,
    )
}

// ───────────────────── the tree-row cache and the tag filter ─────────────────────

/// A tag-filtered view is a different tree from the indexed workspace, so it is
/// flattened from the filtered build and the filter it was built under is
/// recorded on the cache — that record is what makes the next change detectable.
#[test]
fn a_tag_filter_flattens_the_filtered_tree_and_is_recorded_on_the_cache() {
    let (mut state, _storage) = fresh_state();
    state.app.tags_mut().selected_tag = Some("project".to_string());
    state.app.orchestrator.selection.tree_dirty = true;

    refresh_tree_cache(&mut state);

    assert_eq!(
        state.app.workspace_tree_cache.row_tag_filter.as_deref(),
        Some("project"),
        "the cache must remember which filter its rows were flattened under"
    );
    assert!(
        state.app.cached_tree_rows.is_some(),
        "a filtered refresh still produces the row cache the panel draws"
    );
    assert!(
        !state.app.orchestrator.selection.tree_dirty,
        "and it consumes the dirty flag exactly like the unfiltered path"
    );
}

/// Changing the filter is an invalidation on its own: a cache that is otherwise
/// clean and un-dirty must still be rebuilt, or the panel keeps showing the
/// previous tag's rows.
#[test]
fn changing_the_tag_filter_invalidates_an_otherwise_clean_cache() {
    let (mut state, _storage) = fresh_state();
    state.app.tags_mut().selected_tag = Some("project".to_string());
    state.app.orchestrator.selection.tree_dirty = true;
    refresh_tree_cache(&mut state);
    let first = Arc::clone(
        state
            .app
            .cached_tree_rows
            .as_ref()
            .expect("the first refresh populated the cache"),
    );
    assert!(
        !state.app.orchestrator.selection.tree_dirty,
        "precondition: the cache is clean"
    );

    state.app.tags_mut().selected_tag = Some("other".to_string());
    refresh_tree_cache(&mut state);

    assert_eq!(
        state.app.workspace_tree_cache.row_tag_filter.as_deref(),
        Some("other"),
        "the new filter must be the one recorded on the cache"
    );
    let second = Arc::clone(
        state
            .app
            .cached_tree_rows
            .as_ref()
            .expect("the second refresh populated the cache"),
    );
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a changed filter must produce a freshly flattened row set, not reuse \
         the rows built under the old one"
    );
}

// ───────────────────── background process reports ─────────────────────

/// A prompt read that failed is reported, not run: the UI must not start an agent
/// turn with an empty or fabricated prompt body.
#[test]
fn a_failed_prompt_read_publishes_no_run_command() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);
    let event = BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
        UserFileOperationResult::PromptLoaded {
            content: Err("the prompt file could not be read".to_string()),
            target_dir: None,
            target_file: Some(PathBuf::from("/prompts/alpha.md")),
        },
    ));

    apply_background_event(&mut state, &event);

    assert_eq!(
        drained(&spy),
        Vec::<UserCommand>::new(),
        "a failed read has no body to run, so no `RunSkillPrompt` may be published"
    );
    assert!(
        state.overlay.editor.is_none(),
        "and it must not be mistaken for an editor-open result either"
    );
}

/// An `EditorSaved` report only closes the card when it names the buffer that is
/// open **and** the bytes the worker wrote are the bytes still on screen. A
/// mismatch means the user typed since the save, and closing would drop those
/// edits silently.
#[test]
fn an_editor_save_report_for_a_changed_buffer_leaves_the_card_open() {
    let (mut state, _storage) = fresh_state();
    let path = PathBuf::from("M:\\lib\\note.md");
    state.overlay.editor = Some(EditorViewModel::open(&path, "body"));
    let event = BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
        UserFileOperationResult::EditorSaved {
            path: path.clone(),
            saved_content: "body typed before the last keystroke".to_string(),
            result: Ok(()),
        },
    ));

    apply_background_event(&mut state, &event);

    assert!(
        state.overlay.editor.is_some(),
        "the buffer no longer matches what was written, so the card must stay up"
    );
    assert_eq!(
        state.overlay.editor.as_ref().expect("still open").text(),
        "body",
        "and the edits on screen must be untouched by the report"
    );
}

/// The same report for a buffer that still matches does close the card — the
/// other half of the guard, pinned so the mismatch test above cannot pass by
/// the close never happening at all.
#[test]
fn an_editor_save_report_for_an_unchanged_buffer_closes_the_card() {
    let (mut state, _storage) = fresh_state();
    let path = PathBuf::from("M:\\lib\\note.md");
    state.overlay.editor = Some(EditorViewModel::open(&path, "body"));
    let event = BackgroundEvent::Process(ProcessEvent::UserFileOperationCompleted(
        UserFileOperationResult::EditorSaved {
            path: path.clone(),
            saved_content: state
                .overlay
                .editor
                .as_ref()
                .expect("editor open")
                .document()
                .to_string(),
            result: Ok(()),
        },
    ));

    apply_background_event(&mut state, &event);

    assert!(
        state.overlay.editor.is_none(),
        "a save the buffer still agrees with is the signal to close"
    );
}

// ───────────────────── the frame tail: joining a batch ─────────────────────

/// A batch thread that has not terminated is put back untouched: joining it here
/// would block the UI thread on the very work `FMD-060` says must not block it.
#[test]
fn an_unfinished_batch_thread_is_put_back_rather_than_joined() {
    let (mut state, _storage) = fresh_state();
    let (handle, gate) = running_batch_handle();
    let cancel_flag = Arc::clone(&handle.cancel_flag);
    state.app.orchestrator.dialogs.batch_cancel_flag = Some(Arc::clone(&cancel_flag));
    state.app.orchestrator.dialogs.batch_handle = Some(handle);

    join_finished_batch(&mut state);

    assert!(
        state.app.orchestrator.dialogs.batch_handle.is_some(),
        "a live worker keeps its handle so the next frame can try again"
    );
    assert!(
        state.app.orchestrator.dialogs.batch_cancel_flag.is_some(),
        "and its cancel flag stays paired with it"
    );
    assert!(
        !cancel_flag.load(std::sync::atomic::Ordering::SeqCst),
        "putting the handle back must not cancel anything"
    );

    // Release the parked worker and reap it so the test leaves no thread behind.
    let handle = state
        .app
        .orchestrator
        .dialogs
        .batch_handle
        .take()
        .expect("the handle was put back");
    drop(gate);
    let _ = handle.join();
}

// ───────────────────── the frame tail: following the streams ─────────────────────

/// Each of the three streaming lists contributes its snap independently, and only
/// when its own follow rule says the user is at the end of it.
#[test]
fn follow_scroll_tasks_snaps_exactly_the_lists_that_are_being_followed() {
    let (mut state, _storage) = fresh_state();
    state.models.logs.auto_scroll = true;
    state.overlay.logs_scroll = Some(at_the_bottom());
    state.models.agent.auto_scroll = true;
    state.overlay.agent_scroll = Some(at_the_bottom());
    state.models.debug.auto_scroll = true;
    state.overlay.debug_scroll = Some(at_the_bottom());

    let tasks = follow_scroll_tasks(&state);

    assert_eq!(
        tasks.len(),
        3,
        "all three lists are at their end with auto-scroll on, so all three \
         must be snapped"
    );
}

/// With no viewport report yet — the freshly booted app — nothing is snapped:
/// the reducer must not scroll a list it has never seen laid out.
#[test]
fn follow_scroll_tasks_snaps_nothing_without_a_viewport_report() {
    let (mut state, _storage) = fresh_state();
    state.models.logs.auto_scroll = true;
    state.models.agent.auto_scroll = true;
    state.models.debug.auto_scroll = true;

    let tasks = follow_scroll_tasks(&state);

    assert!(
        tasks.is_empty(),
        "a preference alone is not permission to scroll: with no geometry the \
         reducer has nothing to stick to"
    );
}

/// Auto-scroll off, or the user scrolled back up, and the list is left alone —
/// the stick-to-bottom rule must not yank a reader back to the tail.
#[test]
fn follow_scroll_tasks_leave_a_list_always_scrolled_away_or_unpreferred() {
    let (mut state, _storage) = fresh_state();
    state.models.logs.auto_scroll = false;
    state.overlay.logs_scroll = Some(at_the_bottom());
    state.models.agent.auto_scroll = true;
    state.overlay.agent_scroll = Some(scrolled_away());
    state.models.debug.auto_scroll = true;
    state.overlay.debug_scroll = Some(scrolled_away());

    let tasks = follow_scroll_tasks(&state);

    assert!(
        tasks.is_empty(),
        "the log list has auto-scroll off, and the other two have been scrolled \
         back up — none of them may be snapped"
    );
}
