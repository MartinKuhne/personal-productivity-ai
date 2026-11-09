//! Reducer-arm tests for the overlay, menu, movable, layout, about, batch and
//! render channels — the arms `update_tests.rs` does not reach.

use super::*;
use crate::agent::batch::types::PromptInfo;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::app::build_state;
use crate::ui_iced::menu_surface::log_capture::{Capture, Captured, with_capture};
use crate::ui_iced::persisted::{InMemoryUiStateStorage, UiStateStorage};
use hoarfrost::surface::tooltip::TooltipAnchor;
use iced::Rectangle;
use std::path::PathBuf;
use std::sync::Arc;

/// A viewport large enough that a utility card has slack on every side, so a
/// press is measured against the card's own anatomy and not against a clamp.
const ROOMY_VIEWPORT: Size = Size::new(2400.0, 1800.0);

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

/// A fresh `AppState` over an in-memory UI-state store.
fn fresh_state() -> (AppState, Arc<InMemoryUiStateStorage>) {
    let storage = Arc::new(InMemoryUiStateStorage::new());
    let state = build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::clone(&storage) as Arc<dyn UiStateStorage>,
        populated_persisted(),
    );
    (state, storage)
}

/// A fresh `AppState` over a caller-supplied UI-state store.
fn state_over(storage: Arc<dyn UiStateStorage>) -> AppState {
    build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        storage,
        populated_persisted(),
    )
}

/// An extra subscriber on the orchestrator's user-command bus.
fn command_spy(state: &AppState) -> crate::bus::core::BusReader<UserCommand> {
    state.app.orchestrator.user_command_bus.subscribe()
}

/// Drains a spy reader into a `Vec` of the commands it saw.
fn drained(spy: &crate::bus::core::BusReader<UserCommand>) -> Vec<UserCommand> {
    let mut seen = Vec::new();
    while let Ok(command) = spy.try_recv() {
        seen.push(command);
    }
    seen
}

/// A prompt candidate identified by `name`.
fn prompt(name: &str) -> PromptInfo {
    PromptInfo {
        path: PathBuf::from("/prompts").join(name),
        display_name: name.to_string(),
        library_name: "System".to_string(),
        content: format!("{name} body"),
    }
}

/// Opens the Background Operations Log and syncs its movable host.
fn open_logs_card(state: &mut AppState) {
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.show_background_logs = true;
    }
    state.rebuild_view_models();
    assert!(
        state.overlay.logs_movable.is_some(),
        "the fixture must open the log card's host"
    );
}

/// Reports a pointer position the way the root mouse area does.
fn report_pointer(state: &mut AppState, at: Point) {
    let _ = update(
        state,
        Message::Layout(LayoutMessage::CursorMoved { point: at }),
    );
}

/// Arm every tooltip controller through the real hover lane.
fn arm_all_tooltips(state: &mut AppState) {
    for (id, text) in [
        (TooltipId::SearchTrigger, None),
        (TooltipId::SearchClear, None),
        (
            TooltipId::ToolsError,
            Some("the tool call failed".to_string()),
        ),
    ] {
        let _ = update(
            state,
            Message::Layout(LayoutMessage::TooltipHoverEnter {
                id,
                tooltip_text: text,
            }),
        );
    }
}

// ───────────────────── Overlay: the Escape tooltip pre-pass ─────────────────────

/// `UIF-ATOM-007`: Escape dismisses an armed tooltip and clears every field the
/// overlay renders it from — the active id, the tools-error text **and** its
/// anchor — instead of leaving a hidden tooltip's payload behind.
#[test]
fn escape_dismisses_every_armed_tooltip_and_clears_its_payload() {
    let (mut state, _storage) = fresh_state();
    arm_all_tooltips(&mut state);
    // The anchor rectangle is written by the view layer; seed it so the reset
    // is observable from the reducer side.
    state.overlay.tooltip.tools_error_anchor = Some(TooltipAnchor::Control(Rectangle::new(
        Point::new(12.0, 34.0),
        Size::new(16.0, 16.0),
    )));
    assert_eq!(
        state.overlay.tooltip.active_tooltip,
        Some(TooltipId::ToolsError),
        "precondition: the last hovered control owns the visible tooltip"
    );
    assert!(
        state.overlay.tooltip.tools_error_controller.is_active(),
        "precondition: the error marker's controller is armed"
    );

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert_eq!(
        state.overlay.tooltip.active_tooltip, None,
        "Escape must hide the tooltip that was showing"
    );
    assert_eq!(
        state.overlay.tooltip.tools_error_tooltip, None,
        "the error text must not outlive the dismissal"
    );
    assert_eq!(
        state.overlay.tooltip.tools_error_anchor, None,
        "a stale anchor would re-anchor the next tooltip at the dismissed marker"
    );
    for (name, controller) in [
        (
            "search trigger",
            &state.overlay.tooltip.search_trigger_controller,
        ),
        (
            "search clear",
            &state.overlay.tooltip.search_clear_controller,
        ),
        ("tools error", &state.overlay.tooltip.tools_error_controller),
    ] {
        assert!(
            !controller.is_active() && controller.is_suppressed(),
            "{name}: Escape dismisses and suppresses until the pointer leaves, \
             so the tooltip cannot pop straight back under the cursor"
        );
    }
}

/// The pre-pass must not swallow the key: whatever band sits underneath still
/// receives the same Escape (`UIF-WIN-003` keeps one Escape path).
#[test]
fn escape_still_walks_the_z_bands_after_dismissing_a_tooltip() {
    let (mut state, _storage) = fresh_state();
    arm_all_tooltips(&mut state);
    let _ = update(
        &mut state,
        Message::Command(UserCommand::Delete(PathBuf::from("/notes/a.md"))),
    );
    assert!(
        state.overlay.dialog.is_open(),
        "precondition: the delete confirmation is the surface under the tooltip"
    );

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert!(
        !state.overlay.dialog.is_open(),
        "dismissing the tooltip is a pre-pass, not a replacement for the band walk"
    );
    assert_eq!(
        state.overlay.tooltip.active_tooltip, None,
        "and the tooltip dismissal still happened in the same reduction"
    );
}

/// With nothing armed the pre-pass must not invent a dismissal: the payload it
/// did not put there stays exactly where it was.
#[test]
fn escape_with_no_armed_tooltip_clears_nothing() {
    let (mut state, _storage) = fresh_state();
    state.overlay.tooltip.tools_error_tooltip = Some("text from an earlier frame".to_string());

    let _ = update(&mut state, Message::Overlay(OverlayMessage::EscapePressed));

    assert_eq!(
        state.overlay.tooltip.tools_error_tooltip.as_deref(),
        Some("text from an earlier frame"),
        "no controller was armed, so the dismissal block must not run at all"
    );
}

// ───────────────────── Movable: the close / drag boundary ─────────────────────

/// `Close ×` is honoured by the host's geometry, not by the message name: a
/// press the host reads as the start of a drag keeps the card open and publishes
/// no toggle command.
#[test]
fn a_close_press_the_host_reads_as_a_drag_publishes_no_toggle() {
    let (mut state, _storage) = fresh_state();
    state.viewport = ROOMY_VIEWPORT;
    open_logs_card(&mut state);
    let title_bar_corner = state
        .overlay
        .logs_movable
        .as_ref()
        .expect("the log card is open")
        .card()
        .position();
    let spy = command_spy(&state);
    // The leading corner of the title bar is the drag lane; the action cluster
    // sits at the trailing end, so this point is a drag as far as the host knows.
    report_pointer(
        &mut state,
        Point::new(title_bar_corner.x + 4.0, title_bar_corner.y + 4.0),
    );

    let task = update(
        &mut state,
        Message::Movable(MovableMessage::CloseRequested {
            card: MovableCard::Logs,
        }),
    );

    assert_eq!(
        task.units(),
        0,
        "a close the host does not honour must not schedule the deferred toggle"
    );
    assert_eq!(
        drained(&spy),
        Vec::<UserCommand>::new(),
        "and it must not publish the hide command either"
    );
    let host = state
        .overlay
        .logs_movable
        .as_ref()
        .expect("the card stays mounted");
    assert!(
        host.is_dragging(),
        "the same point on the title bar is the drag lane, so the host must have \
         begun a drag instead of closing"
    );
}

// ───────────────────── Menu: traffic that arrives with no menu ─────────────────────

/// A hover report for a menu that is not open is dropped without touching the
/// corridor or opening anything in its place.
#[test]
fn menu_hover_traffic_while_the_hamburger_is_closed_is_dropped() {
    let (mut state, _storage) = fresh_state();
    assert!(
        !state.overlay.hamburger_open,
        "precondition: no hamburger tree is mounted"
    );

    let tasks = reduce_menu(
        &mut state,
        MenuMessage::RowLeft {
            card: MenuId::new("hamburger.ghost"),
        },
    );
    assert!(
        tasks.is_empty(),
        "a leave report for a card that is not mounted schedules nothing"
    );

    let tasks = reduce_menu(
        &mut state,
        MenuMessage::AimExpired {
            card: MenuId::new("hamburger.ghost"),
        },
    );
    assert!(
        tasks.is_empty(),
        "a corridor deadline for a card that is not mounted schedules nothing"
    );
    assert!(
        !state.overlay.hamburger_open,
        "dropped menu traffic must not conjure a menu"
    );
}

// ───────────────────── Tree: the settled print ─────────────────────

/// A print that came back `Ok` settles silently: no error log, no follow-up work.
#[test]
fn a_successful_print_settles_without_an_error_log() {
    let (mut state, _storage) = fresh_state();
    let capture = Capture::new();

    let tasks = with_capture(&capture, || {
        reduce_tree(
            &mut state,
            TreeMessage::PrintFinished {
                path: PathBuf::from("/notes/a.md"),
                result: Ok(()),
            },
        )
    });

    assert!(tasks.is_empty(), "a settled print spawns no further work");
    assert!(
        capture.with_code(PRINT_NO_CHANNEL_ERROR_CODE).is_none(),
        "the failure arm must not fire on an `Ok` result"
    );
}

// ───────────────────── Layout: the ToC auto-fit's no-room guard ─────────────────────

/// A nested region with no width left is not a width to solve against: the
/// remembered right-panel width survives untouched instead of collapsing to zero.
#[test]
fn toc_auto_fit_without_any_nested_width_keeps_the_remembered_right_panel() {
    let (mut state, _storage) = fresh_state();
    state.viewport = Size::new(600.0, 700.0);
    state.app.layout.set_width(Some(600.0));
    state.app.layout.set_right_width(Some(200.0));
    let before = state.app.layout.right_panel_width;

    auto_fit_toc(&mut state);

    assert_eq!(
        state.app.layout.right_panel_width, before,
        "the left pane and the gutter already spent the whole viewport, so the \
         ToC must not resize the right pane to fit nothing"
    );
    assert_eq!(
        state.app.persisted_ui_state.right_panel_width, before,
        "and the persisted snapshot must not be rewritten from an unsolvable fit"
    );
}

// ───────────────────── About: the absorbed press ─────────────────────

/// `UIF-VER-003`: a press the About card absorbs is observable as having
/// arrived, and changes nothing — the window stays where it is.
#[test]
fn an_absorbed_press_keeps_the_about_window_open_and_unchanged() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Command(UserCommand::OpenAboutDialog));
    assert!(
        state.overlay.about.is_about(),
        "precondition: the About window is showing"
    );
    let before = state.mounted_layers();

    let tasks = reduce_about(&mut state, AboutMessage::Absorbed);

    assert!(
        tasks.is_empty(),
        "absorption is a deliberate no-op: nothing is scheduled"
    );
    assert!(
        state.overlay.about.is_about(),
        "a press inside the card must not dismiss the window it landed on"
    );
    assert_eq!(
        state.mounted_layers(),
        before,
        "and the mounted layers must be exactly what they were"
    );
}

// ───────────────────── Batch: prompt candidates arriving late ─────────────────────

/// Candidates for the dialog's current generation are applied to the view-model
/// **and** mirrored into the orchestrator's config, and a selection that is still
/// available follows its prompt to its new index.
#[test]
fn prompt_candidates_for_the_current_generation_are_applied_and_reindexed() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));
    let generation = state.overlay.batch.prompt_load_generation();
    assert!(
        state.overlay.batch.is_open(),
        "precondition: the batch dialog is open"
    );

    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
            generation,
            prompts: vec![prompt("alpha"), prompt("beta")],
        }),
    );
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptSelected(Some(1))),
    );
    assert_eq!(
        state.overlay.batch.model().selected_prompt_idx,
        Some(1),
        "precondition: 'beta' is the picked candidate"
    );

    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
            generation,
            prompts: vec![prompt("beta"), prompt("alpha")],
        }),
    );

    assert_eq!(
        state.models.batch.available_prompts,
        vec![prompt("beta"), prompt("alpha")],
        "the fresh candidate list replaces the old one"
    );
    assert_eq!(
        state.overlay.batch.model().selected_prompt_idx,
        Some(0),
        "the selection follows 'beta' to the index it now holds"
    );
    let config = &state.app.orchestrator.dialogs.batch_dialog_config;
    assert_eq!(
        config.available_prompts,
        vec![prompt("beta"), prompt("alpha")],
        "the orchestrator's config is the batch runner's input, so it must carry \
         the same list the card shows"
    );
    assert_eq!(
        config.selected_prompt_idx,
        Some(0),
        "and the same selection"
    );
}

/// Candidates from a superseded open request are dropped: a late discovery for a
/// dialog the user already re-opened must not overwrite the current list.
#[test]
fn prompt_candidates_from_a_stale_generation_are_ignored() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));
    let generation = state.overlay.batch.prompt_load_generation();
    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
            generation,
            prompts: vec![prompt("alpha")],
        }),
    );

    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
            generation: generation.wrapping_add(1),
            prompts: vec![prompt("ghost"), prompt("phantom")],
        }),
    );

    assert_eq!(
        state.models.batch.available_prompts,
        vec![prompt("alpha")],
        "a report for a generation that is no longer current must change nothing"
    );
    assert_eq!(
        state
            .app
            .orchestrator
            .dialogs
            .batch_dialog_config
            .available_prompts,
        vec![prompt("alpha")],
        "and the orchestrator's config must not be overwritten by it either"
    );
}

/// Candidates that arrive after the dialog was dismissed are dropped too — there
/// is no card left to show them on.
#[test]
fn prompt_candidates_for_a_closed_dialog_are_ignored() {
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Command(UserCommand::OpenBatchDialog));
    let generation = state.overlay.batch.prompt_load_generation();
    let _ = update(&mut state, Message::Command(UserCommand::CancelBatch));
    assert!(
        !state.overlay.batch.is_open(),
        "precondition: the dialog is dismissed"
    );

    let _ = update(
        &mut state,
        Message::Batch(BatchDialogMessage::PromptCandidatesLoaded {
            generation,
            prompts: vec![prompt("alpha")],
        }),
    );

    assert!(
        state.models.batch.available_prompts.is_empty(),
        "a closed dialog must not accumulate candidates it will never show"
    );
    assert!(
        state
            .app
            .orchestrator
            .dialogs
            .batch_dialog_config
            .available_prompts
            .is_empty(),
        "and the batch runner must not be handed a list for a dialog that is gone"
    );
}

// ───────────────────── Render: a dead link ─────────────────────

/// A click on a link that resolves to no file opens nothing at all: no tab, no
/// scroll, no system editor, and no published intent.
#[test]
fn a_link_that_resolves_to_nothing_opens_nothing() {
    let (mut state, _storage) = fresh_state();
    let spy = command_spy(&state);

    let task = update(
        &mut state,
        Message::Render(RenderMessage::LinkActivated {
            url: "nowhere.md".to_string(),
            source_file: None,
        }),
    );

    assert_eq!(
        task.units(),
        0,
        "a dead link schedules no work — the resolver answered `NotFound`"
    );
    assert_eq!(
        drained(&spy),
        Vec::<UserCommand>::new(),
        "nothing may be published for a path that does not exist"
    );
    assert_eq!(
        state.app.orchestrator.tabs.scroll_to_header_id, None,
        "and the document must not be scrolled for it"
    );
}

// ───────────────────── Window: persistence that fails ─────────────────────

/// Find a captured event by its `name` field - the [NFR-004] code convention
/// [`persisted`] uses (`name = persisted::UI_STATE_SAVE_ERROR_CODE`), not a
/// dedicated `error_code` field. Mirrors `persisted_tests::named`.
fn named(capture: &Capture, code: &str) -> Option<Captured> {
    capture
        .events()
        .into_iter()
        .find(|event| event.field("name") == Some(code))
}

/// A UI-state store that refuses the write must not take the close with it: the
/// failure is logged with its error code and the window still closes.
struct FailingUiStateStorage;

impl UiStateStorage for FailingUiStateStorage {
    fn save_ui_state(&self, _json: &str) -> Result<std::path::PathBuf, String> {
        Err("the disk is full".to_string())
    }

    fn load_ui_state(&self) -> Option<Result<String, String>> {
        None
    }
}

#[test]
fn a_failed_close_save_is_reported_and_the_window_still_closes() {
    let mut state = state_over(Arc::new(FailingUiStateStorage));
    state.app.layout.set_width(Some(184.0));
    let capture = Capture::new();

    let task = with_capture(&capture, || {
        update(
            &mut state,
            Message::Window(WindowMessage::CloseSnapshotCaptured(
                crate::ui_iced::message::WindowSnapshot {
                    id: iced::window::Id::unique(),
                    size: Size::new(1234.0, 567.0),
                    position: Some(Point::new(11.0, 22.0)),
                },
            )),
        )
    });

    let event = named(&capture, persisted::UI_STATE_SAVE_ERROR_CODE)
        .expect("a failed close-time save must report ERROR under the `name` code");
    assert_eq!(
        event.level,
        tracing::Level::ERROR,
        "NFR-001: an application failure is an ERROR log"
    );
    assert_eq!(
        event.field("error"),
        Some("the disk is full"),
        "the handler's own reason must be carried through, got {:?}",
        event.fields
    );
    assert_eq!(
        state.app.persisted_ui_state.window_width,
        Some(1234.0),
        "the metrics are still folded into the snapshot, so a later save writes \
         the geometry the user actually had"
    );
    assert!(
        task.units() >= 1,
        "the close itself must still be handed to the runtime"
    );
}
