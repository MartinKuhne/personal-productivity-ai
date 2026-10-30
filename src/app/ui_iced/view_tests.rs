//! Unit tests for the iced view (`src/app/ui_iced/view.rs`).
//!
//! iced elements are not inspectable without a renderer, so the view is
//! tested two ways:
//!
//! * **smoke** — the full tree builds for every state that changes its
//!   shape (no file / file selected / every overlay mounted);
//! * **selector** — [`AppState::mounted_layers`] is the pure "what is
//!   mounted, in what order" decision the host renders, so asserting on it
//!   asserts the z-order without needing a renderer.

use super::*;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui_iced::app::{ContextMenu, OverlayLayer, build_state};
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::editor::EditorViewModel;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use iced_test::selector::Candidate;
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh `AppState` over in-memory storage.
fn fresh_state() -> AppState {
    build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        crate::ui::PersistedUiState::default(),
    )
}

/// Opens the background-process log window the way its `UserCommand` does.
fn show_logs(state: &mut AppState) {
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.show_background_logs = true;
    }
}

/// Opens the agent debug window the way its `UserCommand` does.
fn show_debug(state: &mut AppState) {
    state.app.orchestrator.agent_panel_state.show_debug_window = true;
}

/// Opens a real single-file context menu at `point`.
///
/// Built from the production row producer rather than a hand-made item
/// list, so the mount and render assertions describe the menu the app
/// actually shows.
fn open_context_menu(state: &mut AppState, point: iced::Point) {
    let path = PathBuf::from("/notes/a.md");
    let rows = crate::ui_iced::panels::left::file_menu_items(&path, &[], false);
    state.overlay.context_menu = ContextMenu::open(
        crate::ui_iced::context_menu::ContextTarget::TreeFile { path },
        point,
        &rows,
        state.viewport,
    );
    assert!(
        state.overlay.context_menu.is_some(),
        "the fixture must actually open a card"
    );
}

#[test]
fn test_band_spacing_matches_task_contract() {
    assert_eq!(BAND_SPACING_PIXELS, 8.0);
}

#[test]
fn test_nothing_is_mounted_on_a_fresh_state() {
    let state = fresh_state();
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "a booted app shows only the three bands"
    );
}

#[test]
fn test_context_menu_mounts_only_when_a_card_actually_opened() {
    let mut state = fresh_state();

    state.overlay.context_menu = None;
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "no surface means no click-blocking layer"
    );

    open_context_menu(&mut state, iced::Point::new(10.0, 20.0));
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::ContextMenu]);
}

#[test]
fn test_editor_mounts_while_a_buffer_is_open() {
    let mut state = fresh_state();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "# Title\n"));
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Editor]);
}

#[test]
fn test_small_dialog_mounts_when_open() {
    let mut state = fresh_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Dialog]);
}

#[test]
fn test_log_and_debug_windows_mount_from_their_domain_flags() {
    let mut state = fresh_state();
    show_logs(&mut state);
    state.rebuild_view_models();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Logs]);

    let mut state = fresh_state();
    show_debug(&mut state);
    state.rebuild_view_models();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::AgentDebug]);
}

#[test]
fn test_tools_about_and_attributions_mount_individually() {
    let mut state = fresh_state();
    state.overlay.tools.open();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Tools]);

    let mut state = fresh_state();
    state.overlay.about.open_about();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::About]);

    let mut state = fresh_state();
    state.overlay.about.open_attributions();
    assert_eq!(
        state.mounted_layers(),
        vec![OverlayLayer::Attributions],
        "the credits window replaces About rather than stacking beside it"
    );
}

#[test]
fn test_batch_dialog_is_the_topmost_layer() {
    let mut state = fresh_state();
    state.overlay.batch.open(state.models.batch.clone());
    show_logs(&mut state);
    show_debug(&mut state);
    state.overlay.tools.open();
    state.overlay.about.open_about();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "body"));
    state.overlay.dialog.open(DialogKind::DeleteConfirm {
        path: PathBuf::from("/notes/a.md"),
    });
    open_context_menu(&mut state, iced::Point::new(0.0, 0.0));
    state.rebuild_view_models();

    assert_eq!(
        state.mounted_layers(),
        vec![
            OverlayLayer::ContextMenu,
            OverlayLayer::Editor,
            OverlayLayer::Dialog,
            OverlayLayer::Logs,
            OverlayLayer::AgentDebug,
            OverlayLayer::Tools,
            OverlayLayer::About,
            OverlayLayer::Batch,
        ],
        "z-order matches the intended draw order"
    );
}

#[test]
fn test_view_builds_without_selected_file() {
    let state = fresh_state();
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_with_selected_file() {
    let mut state = fresh_state();
    state.app.selection_mut().selected_file = Some(PathBuf::from("/notes/a.md"));
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_with_every_overlay_mounted() {
    let mut state = fresh_state();
    state.overlay.batch.open(state.models.batch.clone());
    show_logs(&mut state);
    show_debug(&mut state);
    state.overlay.tools.open();
    state.overlay.about.open_attributions();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "# Title\n"));
    state.overlay.dialog.open(DialogKind::Rename {
        path: PathBuf::from("/notes/a.md"),
    });
    open_context_menu(&mut state, iced::Point::new(4.0, 4.0));
    state.rebuild_view_models();

    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_while_the_agent_session_owns_the_center_pane() {
    let mut state = fresh_state();
    state.app.orchestrator.agent_panel_state.show_results = true;
    state.rebuild_view_models();
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_live_view_mounts_and_routes_the_agent_prompt_controls() {
    use crate::ui::strings;
    use crate::ui_iced::agent::prompt_input::PROMPT_INPUT_ID;
    use crate::ui_iced::message::AgentMessage;
    use crate::ui_iced::update::update;

    let mut state = fresh_state();
    state.app.orchestrator.agent_panel_state.show_results = true;
    state.rebuild_view_models();

    let prompt_id = iced::widget::Id::from(PROMPT_INPUT_ID);
    let mut ui = iced_test::simulator(view(&state));
    assert!(
        ui.find(strings::AGENT_SESSION_HEADER).is_ok(),
        "the full live view must mount the agent session header"
    );
    let prompt_bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput {
                id: Some(id),
                bounds,
                ..
            } if *id == prompt_id => Some(bounds),
            _ => None,
        })
        .expect("the full live view must expose the agent prompt field");
    assert!(
        prompt_bounds.width > 0.0 && prompt_bounds.height > 0.0,
        "the prompt field must receive usable center-pane bounds, got {prompt_bounds:?}"
    );

    ui.point_at(prompt_bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
    let _ = ui.typewrite("summarize");
    let input_messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        input_messages.contains(&Message::Agent(AgentMessage::InputChanged {
            text: "summarize".to_string(),
        })),
        "typing in the mounted prompt must emit the existing agent input message, got {input_messages:?}"
    );

    for message in input_messages {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();

    let mut ui = iced_test::simulator(view(&state));
    assert!(
        ui.find(strings::AGENT_SEND_BUTTON).is_ok(),
        "the mounted Send control must remain reachable in the live center pane"
    );
    let _ = ui.click(strings::AGENT_SEND_BUTTON);
    let send_messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        send_messages.contains(&Message::Agent(AgentMessage::Submit {
            text: "summarize".to_string(),
        })),
        "Send must use the existing agent message/reducer route, got {send_messages:?}"
    );
}

#[test]
fn test_live_view_gives_the_center_pane_to_the_agent_only_while_shown() {
    use crate::ui::strings;

    let mut state = fresh_state();
    let tab_path = PathBuf::from("/notes/agent-owned.md");
    state.app.tabs_mut().open_tab(tab_path);
    state.app.orchestrator.agent_panel_state.show_results = true;
    state.rebuild_view_models();

    {
        let mut agent_view = iced_test::simulator(view(&state));
        assert!(
            agent_view.find(strings::AGENT_SESSION_HEADER).is_ok(),
            "the visible agent session must own the live center pane"
        );
        assert!(
            agent_view.find("agent-owned.md").is_err(),
            "the document tab must not be layered into the center while the agent owns it"
        );
    }

    state.app.orchestrator.agent_panel_state.show_results = false;
    state.rebuild_view_models();
    let mut document_view = iced_test::simulator(view(&state));
    assert!(
        document_view.find(strings::AGENT_SESSION_HEADER).is_err(),
        "hiding the agent session must remove the panel from the live center"
    );
    assert!(
        document_view.find("agent-owned.md").is_ok(),
        "the previously-open document tab must reclaim the center when the session is hidden"
    );
}

#[test]
fn test_document_width_never_collapses_below_the_floor() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(100.0, 100.0);
    assert!(
        document_width(&state) >= MIN_DOCUMENT_WIDTH_PIXELS,
        "a tiny viewport must not hand the table solver a non-positive width"
    );
}

#[test]
fn test_document_width_excludes_both_side_panels_when_the_toc_shows() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(1000.0, 700.0);
    state.app.layout.set_width(Some(300.0));
    state.app.layout.set_right_width(Some(200.0));
    state.app.orchestrator.tabs.toc = vec![crate::markdown::ToCEntry::new("Intro", 1, "Intro")];
    state.app.selection_mut().selected_file = Some(PathBuf::from("/notes/a.md"));

    assert_eq!(document_width(&state), 500.0);
}

#[test]
fn test_document_width_ignores_the_right_pane_when_the_toc_is_hidden() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(1000.0, 700.0);
    state.app.layout.set_width(Some(300.0));
    state.app.layout.set_right_width(Some(200.0));

    assert_eq!(
        document_width(&state),
        700.0,
        "no ToC → the right pane is not mounted, so it takes no width"
    );
}

#[test]
fn toc_scrollable_id_is_stable_and_named() {
    assert_eq!(TOC_SCROLLABLE_ID, "toc_scroll");
}

// ---------------------------------------------------------------------------
// ToC entry typography + the icon-only tooltip crash guard (defect #13)
// ---------------------------------------------------------------------------
//
// Two defects lived in the same three lines of `toc_body`:
//
// * The entry label carried no `.size()`, so iced painted it at its 16px
//   default while the file tree beside it paints at `type.ui_compact`
//   (13px on an 18px line).
// * A one- or two-character markdown heading (`# A`) produces a row whose
//   content measures under `size::CONTROL_SM` (28x28). Hoarfrost's
//   `Button::outer_size` (`button_widget.rs:799`) `debug_assert!`s that an
//   icon-only button carries a tooltip, so simply scrolling a document that
//   opened with a short heading crashed the app.

/// The string every ToC typography probe measures — long enough that a 13px
/// and a 16px rendering of it cannot land on the same width.
const TOC_PROBE_TEXT: &str = "Getting Started With FastMD";

type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Lays `element` out in a window of `viewport` size, headless.
fn laid_out(viewport: iced::Size, element: iced::Element<'_, Message>) -> Sim<'_> {
    iced_test::Simulator::with_size(iced_test::core::Settings::default(), viewport, element)
}

/// The laid-out bounds of `label`, measured through a shrink-wrapped
/// container carrying `id`.
///
/// iced 0.14's `Text` exposes `size` and `line_height` only as *builders* —
/// `format` is a private field with no reader — so the rung a label actually
/// carries is proven by measurement rather than by reading it back.
fn measured_label(id: &'static str, label: iced::widget::Text<'static>) -> iced::Rectangle {
    let probe = iced::widget::container(label)
        .id(iced::widget::Id::from(id))
        .width(Length::Shrink)
        .height(Length::Shrink);
    let mut ui = laid_out(iced::Size::new(1024.0, 512.0), probe.into());
    let bounds = ui
        .find(iced::widget::Id::from(id))
        .unwrap_or_else(|err| panic!("probe container {id:?} should be reachable: {err:?}"))
        .bounds();
    drop(ui);
    bounds
}

#[test]
fn toc_text_rung_is_the_same_ui_compact_rung_the_file_tree_paints() {
    assert_eq!(TOC_TEXT_RUNG, typography::UI_COMPACT);
    assert_eq!(
        TOC_TEXT_RUNG.size,
        crate::ui_iced::panels::left::ROW_TEXT_SIZE,
        "the ToC is read side by side with the file tree, so both must name one rung"
    );
    assert_eq!(TOC_TEXT_RUNG.size, 13.0);
    assert_ne!(
        TOC_TEXT_RUNG.size, 16.0,
        "16px is iced's own default text size — painting at it was the defect"
    );
}

#[test]
fn toc_entry_label_measures_at_the_ui_compact_rung_not_iced_default() {
    let label = measured_label("toc_probe_label", toc_entry_label(TOC_PROBE_TEXT.into()));
    let at_rung = measured_label(
        "toc_probe_at_rung",
        text(TOC_PROBE_TEXT)
            .size(typography::UI_COMPACT.size)
            .line_height(typography::UI_COMPACT.iced_line_height()),
    );
    let at_iced_default = measured_label(
        "toc_probe_at_16",
        text(TOC_PROBE_TEXT)
            .size(16.0)
            .line_height(typography::UI_COMPACT.iced_line_height()),
    );

    assert_eq!(
        label.width, at_rung.width,
        "the ToC label must measure exactly as a `type.ui_compact` rendering of the \
         same string (toc={:.2}, rung={:.2})",
        label.width, at_rung.width
    );
    assert!(
        label.width < at_iced_default.width,
        "the ToC label must be narrower than the same string at iced's 16px default \
         (toc={:.2}, 16px={:.2})",
        label.width,
        at_iced_default.width
    );
}

#[test]
fn toc_entry_label_line_height_is_an_absolute_18px_not_the_relative_unit_trap() {
    let label = measured_label(
        "toc_probe_line_height",
        toc_entry_label(TOC_PROBE_TEXT.into()),
    );

    assert_eq!(
        label.height,
        typography::UI_COMPACT.line_height,
        "one `type.ui_compact` line must be 18px tall, got {:.2}",
        label.height
    );
    assert_ne!(
        label.height,
        typography::UI_COMPACT.size * typography::UI_COMPACT.line_height,
        "`From<f32> for iced::LineHeight` yields `Relative(factor)`, so a bare `18.0` on \
         a 13px font asks for 13 x 18 = 234px per line"
    );
}

#[test]
fn a_one_character_toc_entry_engages_the_icon_only_rule_and_satisfies_it() {
    let entry = ToCEntry::new("A", 1, "A");
    let button = toc_entry_button(&entry);
    let tiny = iced::Size::new(8.0, 16.0);

    assert!(
        button.is_icon_only(tiny),
        "a one-character heading measures under the icon-only floor, so the rule must be \
         engaged for this row"
    );
    assert_eq!(
        button.tooltip_violation(tiny),
        None,
        "an icon-only button with no tooltip trips `Button::outer_size`'s debug_assert and \
         crashes the app while the user is only scrolling a document"
    );
    assert_eq!(
        button.tooltip_text(),
        Some("A"),
        "the tooltip is the entry's own heading — no new user-facing copy is invented"
    );
}

#[test]
fn a_normal_toc_entry_keeps_its_ghost_variant_and_title_tooltip() {
    let entry = ToCEntry::new("Getting Started", 1, "Getting Started");
    let button = toc_entry_button(&entry);

    assert_eq!(
        button.variant(),
        ButtonVariant::Ghost,
        "the entry keeps the variant it always had"
    );
    assert_eq!(button.tooltip_text(), Some("Getting Started"));
    assert_eq!(
        button.tooltip_violation(iced::Size::new(8.0, 16.0)),
        None,
        "every entry carries its title as a tooltip, not only the tiny ones"
    );
}

#[test]
fn the_live_view_lays_out_a_one_character_toc_entry_without_crashing() {
    let mut state = fresh_state();
    state.app.selection_mut().selected_file = Some(PathBuf::from("/notes/a.md"));
    state.app.orchestrator.tabs.toc =
        vec![ToCEntry::new("A", 1, "A"), ToCEntry::new("Bb", 2, "Bb")];
    state.rebuild_view_models();

    let mut ui = laid_out(iced::Size::new(1200.0, 800.0), view(&state));
    let bounds = ui
        .find(iced::widget::Id::from(TOC_SCROLLABLE_ID))
        .map(|target| target.bounds())
        .unwrap_or_else(|err| {
            panic!("the ToC pane must mount for a short-heading document: {err:?}")
        });
    drop(ui);

    assert!(
        bounds.width > 0.0 && bounds.height > 0.0,
        "the mounted ToC pane must receive usable bounds, got {bounds:?}"
    );
}
