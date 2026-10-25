//! Tier-2 iced **interaction** tests.
//!
//! Tier 1 for `ui_iced` is the per-module `*_tests.rs` sidecar set, which
//! exercises the pure model/mapping layer. This module is tier 2: it drives
//! the **real view + real reducer loop** through
//! [`iced_test::Simulator`], a headless interface driver that needs no
//! window ([`iced_test` 0.14.0], `doc/iced.md` "iced_test").
//!
//! # What "real" means here
//!
//! * the state comes from [`test_app_state`], the framework-free fixture
//!   over `InMemoryConfigStorage` + `InMemoryUiStateStorage`;
//! * the widgets are the production view functions
//!   ([`left_panel`], [`center_panel`], [`editor_card`], [`dialog_card`],
//!   [`top_bar`], [`view`]) — no stand-ins;
//! * where a test needs the state to actually change between two clicks,
//!   the messages are pushed through the production reducer
//!   [`update`] before the view is rebuilt.
//!
//! # Selector strategy (verified against `iced_selector` 0.14.0)
//!
//! `doc/iced.md` says a `&str` selector "selects a widget by the text it
//! contains"; the implementation is stricter — `Candidate::Text.content ==
//! query` and `Candidate::TextInput.text() == query` are **exact** matches.
//! Every selector used here therefore passes the full rendered string.
//!
//! For widgets that carry no selectable text the tests use the closure
//! `Selector` impl (`impl<F: FnMut(Candidate) -> Option<T>> Selector for F`)
//! to match a [`Candidate`] variant directly — e.g. `Candidate::TextInput`
//! to focus a field, `Candidate::Focusable` to focus a `text_editor`, or a
//! counted match to reach the *n*-th of several identical labels.
//!
//! # Known iced_test limitation: `pick_list` menus are opaque
//!
//! `iced_widget::pick_list` draws its closed-state label and its open menu
//! rows with `renderer.fill_text` rather than child `Text` widgets, and
//! `iced_widget::overlay::menu::Overlay` does not override
//! `Overlay::operate` (default no-op, `iced_core` 0.14 `overlay.rs:41`).
//! The menu is therefore **invisible to every `Selector`**, and because
//! `Simulator::point_at` deliberately emits no `CursorMoved` event the
//! menu's `hovered_option` never lands on a row — so a row click cannot be
//! driven at all. This was verified against a bare, isolated `pick_list`,
//! so it is not a FastMD composition problem.
//!
//! The tag filter is still a `pick_list` and stays covered as far as the
//! tool allows. The hamburger is a ☰ button plus the shared nested menu
//! widget mounted by the overlay host. [`test_top_bar_hamburger_consumes_the_open_click`]
//! proves the ☰ captures the open click, and
//! [`test_hamburger_menu_opens_and_routes_a_row_through_the_reducer`]
//! drives the whole loop — reducer opens, real `view()` mounts the menu,
//! a row click publishes its own `UserCommand`, the reducer closes it.
//!
//! # Test-environment safety ([RUST-006])
//!
//! Every filesystem path a test introduces is a `tempfile::TempDir`. No real
//! user path, `%APPDATA%` or `~/.fastmd*` is read or written, and no test
//! feeds an executing message (`Command`, `Dialog(Submitted)`) back into
//! the reducer — assertions stop at the emitted message.
//!
//! Declared from [`crate::ui_iced`] as `#[cfg(test)] pub mod interaction_tests;`
//! rather than as a `<file>_tests.rs` sidecar ([RUST-056]) because it spans
//! every `ui_iced` view module instead of pairing with one implementation
//! file. Tier 3 (screenshot baselines) and tier 4 (`.ice` replay) live in
//! `tests/iced_visual/` — see `tests/iced_visual/README.md`.

use crate::bus::events::user_command::UserCommand;
use crate::config::ContentLibrary;
use crate::ui::strings;
use crate::ui_iced::app::AppState;
use crate::ui_iced::dialogs::{DialogKind, DialogState};
use crate::ui_iced::editor::{EditorViewModel, editor_card};
use crate::ui_iced::hamburger::{
    HAMBURGER_MENU_ID, KEY_ABOUT, KEY_AGENT_DEBUG, KEY_BACKGROUND_OPERATIONS, KEY_BATCH,
    KEY_CHAT_MODELS, KEY_NO_MODELS, KEY_TABLE_WRAP, KEY_TOOLS, KEY_WINDOWS, RADIO_GROUP_CHAT_MODEL,
    RADIO_GROUP_TABLE_WRAP, command_for_activation, hamburger_menu, strategy_key,
};
use crate::ui_iced::menu::{MenuItem as WidgetMenuItem, MenuItemId};
use crate::ui_iced::message::{
    AgentMessage, DialogMessage, EditorMessage, LayoutMessage, MenuMessage, Message, TreeMessage,
};
use crate::ui_iced::panels::bottom::bottom_panel;
use crate::ui_iced::panels::center::{center_panel, tab_select_command};
use crate::ui_iced::panels::left::{left_panel, row_icon};
use crate::ui_iced::panels::top::{
    apply_agent_debug_toggle, apply_background_logs_toggle, apply_table_width_strategy_change,
    strategy_menu_order, top_bar,
};
use crate::ui_iced::test_helpers::test_app_state;
use crate::ui_iced::update::update;
use crate::ui_iced::view::view;
use iced::Element;
use iced::mouse;
use iced_pro::surface::menu::model::{ActivationEffect, Menu, MenuId, RadioGroupId, submenu_id};
use iced_test::Selector;
use iced_test::selector::{Candidate, Target};
use std::path::PathBuf;

/// Library name the tree fixtures install (arbitrary but stable).
const TEST_LIBRARY_NAME: &str = "System";

/// A headless iced interface driver over the app's `Message`.
type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Builds a [`Sim`] over a rendered element.
fn sim(element: Element<'_, Message>) -> Sim<'_> {
    iced_test::simulator(element)
}

/// The mouse events of a left click at the current cursor position.
fn left_click() -> [iced::Event; 2] {
    [
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]
}

/// The mouse events of a right click at the current cursor position.
fn right_click() -> [iced::Event; 2] {
    [
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)),
    ]
}

/// A pointer-motion event — the only way to move `hovered_option` inside a
/// widget that tracks the cursor (see the module header).
fn cursor_moved(position: iced::Point) -> iced::Event {
    iced::Event::Mouse(mouse::Event::CursorMoved { position })
}

/// Points the cursor at `position` and left-clicks it.
fn click_at(ui: &mut Sim<'_>, position: iced::Point) {
    ui.point_at(position);
    let _ = ui.simulate([cursor_moved(position)]);
    let _ = ui.simulate(left_click());
}

/// Selects the `nth` `Candidate::Text` whose content equals `label`.
///
/// `Simulator::find` stops at the first match, so reaching the second of
/// two identical labels needs a selector that counts the ones it skipped.
fn nth_text(label: &str, nth: usize) -> impl Selector<Output = Target> {
    let mut skipped = 0usize;
    move |candidate: Candidate<'_>| {
        let is_match = matches!(candidate, Candidate::Text { content, .. } if content == label);
        if is_match {
            if skipped == nth {
                return Some(Target::from(candidate));
            }
            skipped += 1;
        }
        None
    }
}

/// Focuses the first `Candidate::TextInput` by clicking its centre.
fn focus_first_text_input(ui: &mut Sim<'_>) -> Option<iced::Rectangle> {
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::TextInput { bounds, .. } => Some(bounds),
            _ => None,
        })
        .ok()?;
    click_at(ui, bounds.center());
    Some(bounds)
}

/// Focuses the `text_editor` — reported as `Candidate::Focusable`, never as
/// a `TextInput` (verified: `text_editor` 0.14 does not answer that query).
fn focus_text_editor(ui: &mut Sim<'_>) -> Option<iced::Rectangle> {
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Focusable { bounds, .. } => Some(bounds),
            _ => None,
        })
        .ok()?;
    click_at(ui, bounds.center());
    Some(bounds)
}

/// Collects every message the driver produced so far.
fn messages(ui: Sim<'_>) -> Vec<Message> {
    ui.into_messages().collect()
}

/// Index of the row carrying `key` inside `menu`.
///
/// A `RowPicked` addresses a row by position, so a test that wants to name
/// a specific row looks its key up rather than hardcoding an index that
/// shifts the moment a row is inserted above it.
fn row_at(menu: &Menu, key: &str) -> usize {
    menu.rows()
        .iter()
        .position(|row| row.key() == key)
        .unwrap_or_else(|| panic!("no hamburger row with key {key:?}"))
}

/// The `RowPicked` message that addresses row `key` of card `card`.
fn picked(card: MenuId, menu: &Menu, key: &str) -> Message {
    Message::Menu(MenuMessage::RowPicked {
        card,
        row: row_at(menu, key),
    })
}

/// Asserts `emitted` contains `expected`.
///
/// Containment rather than equality: `mouse_area` synthesises a
/// `Layout(CursorMoved)` anchor whenever the cursor position changes, and a
/// modal's scrim adds a `Dismissed` when a disabled control lets a click
/// fall through — both are correct behaviour the test is not about.
fn assert_emitted(emitted: &[Message], expected: &Message, context: &str) {
    assert!(
        emitted.contains(expected),
        "{context}: expected {expected:?} among {emitted:?}"
    );
}

/// A `AppState` whose "System" content library is rooted in a temp dir and
/// contains one markdown file, with the library folder expanded so the
/// file renders as a visible tree row ([RUST-006]).
fn state_with_one_file() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_string_lossy().to_string();

    let mut state = test_app_state();
    state.app.content_libraries_mut().push(ContentLibrary {
        root_folder: root.clone(),
        name: TEST_LIBRARY_NAME.to_string(),
        kind: "system".to_string(),
        readonly: false,
        priority: 0,
    });
    state
        .app
        .workspace_mut()
        .all_files
        .push(PathBuf::from(format!("{root}/alpha.md")));
    state
        .app
        .selection_mut()
        .expanded_dirs
        .insert(PathBuf::from(&root));
    state.rebuild_view_models();
    (state, dir)
}

/// The rendered label of the fixture's file row.
fn alpha_row_label() -> String {
    format!("{} alpha.md", row_icon(false, false))
}

// ---------------------------------------------------------------------------
// Left panel — tree row interactions
// ---------------------------------------------------------------------------

/// A left click on a tree row publishes `SelectFile` for that row's path
/// (UI-017).
#[test]
fn test_tree_row_left_click_emits_select_file() {
    let (state, _dir) = state_with_one_file();
    let label = alpha_row_label();
    let path = state.models.left.rows[1].path.clone();

    let mut ui = sim(left_panel(&state.models.left));
    assert!(
        ui.find(label.as_str()).is_ok(),
        "the expanded library must render the alpha.md row"
    );

    let _ = ui.click(label.as_str());
    let emitted = messages(ui);

    assert_emitted(
        &emitted,
        &Message::Command(crate::bus::events::user_command::UserCommand::SelectFile {
            path,
            multi: false,
        }),
        "a plain row click selects exactly that file",
    );
}

/// A right click on a tree row opens the context menu, and the menu is then
/// really mounted in the rebuilt full-app view with its own clickable rows
/// (UI-006).
#[test]
fn test_tree_row_right_click_mounts_a_live_context_menu() {
    let (mut state, _dir) = state_with_one_file();
    let label = alpha_row_label();
    let path = state.models.left.rows[1].path.clone();

    // 1. Right-click the row in the full app view so pointer tracking and the
    // menu overlay share the same root-space coordinate system.
    let mut ui = sim(view(&state));
    let bounds = ui
        .find(label.as_str())
        .expect("row")
        .visible_bounds()
        .expect("visible row");
    let click_point = bounds.center();
    ui.point_at(click_point);
    let _ = ui.simulate([cursor_moved(click_point)]);
    let _ = ui.simulate(right_click());
    let emitted = messages(ui);

    assert!(
        emitted.contains(&Message::Tree(TreeMessage::ContextMenuOpened {
            path: path.clone()
        })),
        "a right click must open the row's context menu, got {emitted:?}"
    );

    // 2. push the emitted messages through the real reducer.
    for message in emitted {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();
    let menu = state
        .overlay
        .context_menu
        .as_ref()
        .expect("the reducer must mount the context menu");
    assert_eq!(
        menu.anchor, click_point,
        "anchor is in full-window coordinates"
    );

    // 3. rebuild the full app view and assert the menu rows are on screen.
    let expected: Vec<String> = state
        .overlay
        .context_menu
        .as_ref()
        .unwrap()
        .items
        .iter()
        .filter_map(|item| item.label().map(str::to_owned))
        .collect();
    assert!(!expected.is_empty(), "the file menu must have rows");

    let mut view_ui = sim(view(&state));
    for row in &expected {
        assert!(
            view_ui.find(row.as_str()).is_ok(),
            "context-menu row {row:?} must be rendered"
        );
    }

    // 4. clicking a menu row publishes that row's own message.
    let first = expected[0].clone();
    let first_message = match &state.overlay.context_menu.as_ref().unwrap().items[0] {
        WidgetMenuItem::Action { message, .. } => message.clone(),
        item => panic!("expected an action row, got {item:?}"),
    };
    let _ = view_ui.click(first.as_str());
    assert_eq!(
        messages(view_ui),
        vec![first_message],
        "the clicked menu row must publish its own command",
    );
}

/// Right-clicking a tab mounts its shared-widget context menu in the full
/// application view and keeps the tab row's command attached to its action.
#[test]
fn test_tab_right_click_mounts_a_live_context_menu() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("tab.md");
    let mut state = test_app_state();
    state.app.orchestrator.tabs.tabs.push(path.clone());
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    let bounds = ui
        .find("tab.md")
        .expect("tab title")
        .visible_bounds()
        .expect("visible tab");
    let click_point = bounds.center();
    ui.point_at(click_point);
    let _ = ui.simulate([cursor_moved(click_point)]);
    let _ = ui.simulate(right_click());
    let emitted = messages(ui);
    let anchor = emitted
        .iter()
        .find_map(|message| match message {
            Message::Layout(LayoutMessage::CursorMoved { point }) => Some(*point),
            _ => None,
        })
        .expect("right-click reports the root-space cursor position");
    assert_eq!(
        anchor, click_point,
        "tab anchor is in full-window coordinates"
    );
    assert_emitted(
        &emitted,
        &Message::Tab(crate::ui_iced::message::TabMessage::ContextMenuOpened { index: 0 }),
        "right-clicking the tab opens its context menu",
    );

    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved { point: anchor }),
    );
    let _ = update(
        &mut state,
        Message::Tab(crate::ui_iced::message::TabMessage::ContextMenuOpened { index: 0 }),
    );
    let menu = state.overlay.context_menu.as_ref().expect("tab menu opens");
    assert_eq!(menu.anchor, click_point);
    assert!(!menu.items.is_empty());
    let first_message = match &menu.items[0] {
        WidgetMenuItem::Action { message, .. } => message.clone(),
        item => panic!("expected an action row, got {item:?}"),
    };

    let mut ui = sim(view(&state));
    assert!(ui.find(strings::EDIT_BUTTON).is_ok());
    let _ = ui.click(strings::EDIT_BUTTON);
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &first_message,
        "the tab menu row emits the action snapshot by the reducer",
    );
}

/// A context-menu submenu event updates reducer-owned path state while the
/// parent row stays visible beside its child surface.
#[test]
fn test_context_menu_nested_surface_keeps_its_parent_mounted() {
    let mut state = test_app_state();
    let submenu_id = MenuItemId::new("test.context.submenu");
    let path = PathBuf::from("notes/nested.md");
    state.overlay.context_menu = Some(crate::ui_iced::app::ContextMenu::new(
        iced::Point::new(420.0, 220.0),
        vec![WidgetMenuItem::submenu(
            submenu_id,
            strings::MENU_WINDOWS,
            vec![WidgetMenuItem::action(
                strings::OPEN_IN_EDITOR_ACTION,
                Message::Command(crate::bus::events::user_command::UserCommand::OpenInEditor(
                    path.clone(),
                )),
            )],
        )],
    ));

    let submenu_label = format!("{} >", strings::MENU_WINDOWS);
    let mut ui = sim(view(&state));
    let _ = ui.click(submenu_label.as_str());
    let emitted = messages(ui);
    let opened = Message::Menu(MenuMessage::ContextSubmenuOpened {
        parent_path: Vec::new(),
        id: submenu_id,
    });
    assert_emitted(
        &emitted,
        &opened,
        "context menu submenu rows emit a controlled open event",
    );
    let _ = update(&mut state, opened);

    let mut ui = sim(view(&state));
    assert!(ui.find(submenu_label.as_str()).is_ok());
    assert!(ui.find(strings::OPEN_IN_EDITOR_ACTION).is_ok());
    let _ = ui.click(strings::OPEN_IN_EDITOR_ACTION);
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Command(crate::bus::events::user_command::UserCommand::OpenInEditor(
            path,
        )),
        "the child action emits its original application command",
    );
}

// ---------------------------------------------------------------------------
// Center panel — tab strip
// ---------------------------------------------------------------------------

/// The `×` on a tab publishes `CloseTab(index)` (UI-049).
#[test]
fn test_tab_strip_close_button_emits_close_tab() {
    let mut state = test_app_state();
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/alpha.md"));
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/beta.md"));
    state.rebuild_view_models();
    assert_eq!(state.models.center.tabs.len(), 2);

    let mut ui = sim(center_panel(&state.models.center));
    let _ = ui.click(strings::ICON_X);

    assert_eq!(
        messages(ui),
        vec![Message::Command(
            crate::bus::events::user_command::UserCommand::CloseTab(0),
        )],
        "the first `×` must close tab 0",
    );
}

/// The *second* tab's `×` closes tab 1 — proves the per-tab index is
/// carried correctly rather than always addressing the first match.
#[test]
fn test_second_tab_close_button_emits_close_tab_one() {
    let mut state = test_app_state();
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/alpha.md"));
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/beta.md"));
    state.rebuild_view_models();

    let mut ui = sim(center_panel(&state.models.center));
    let _ = ui.click(nth_text(strings::ICON_X, 1));

    assert_eq!(
        messages(ui),
        vec![Message::Command(
            crate::bus::events::user_command::UserCommand::CloseTab(1),
        )],
        "the second `×` must close tab 1",
    );
}

/// Clicking a tab's title selects that document.
#[test]
fn test_tab_title_click_selects_that_document() {
    let mut state = test_app_state();
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/alpha.md"));
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/beta.md"));
    state.rebuild_view_models();

    let mut ui = sim(center_panel(&state.models.center));
    let _ = ui.click("beta.md");

    assert_emitted(
        &messages(ui),
        &Message::Command(tab_select_command(
            PathBuf::from("/notes/beta.md").as_path(),
        )),
        "clicking the beta.md tab selects beta.md",
    );
}

/// A middle click on a tab closes it (UI-049), driven through the
/// `on_middle_press` wiring in `center`.
#[test]
fn test_tab_middle_click_closes_that_tab() {
    let mut state = test_app_state();
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/alpha.md"));
    state
        .app
        .tabs_mut()
        .open_tab(PathBuf::from("/notes/beta.md"));
    state.rebuild_view_models();

    let mut ui = sim(center_panel(&state.models.center));
    let bounds = ui
        .find("beta.md")
        .expect("beta tab")
        .visible_bounds()
        .expect("visible beta tab");
    ui.point_at(bounds.center());
    let _ = ui.simulate([
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)),
    ]);

    assert_emitted(
        &messages(ui),
        &Message::Command(crate::bus::events::user_command::UserCommand::CloseTab(1)),
        "a middle click on the second tab must close index 1",
    );
}

// ---------------------------------------------------------------------------
// Editor — typing and saving
// ---------------------------------------------------------------------------

/// Typing into the `text_editor` and clicking Save produces
/// `EditorMessage::SaveRequested` carrying the open path (UI-037..UI-041).
#[test]
fn test_editor_typewrite_then_save_emits_save_requested() {
    let vm = EditorViewModel::open("/notes/a.md", "# Title\n");
    let mut ui = sim(editor_card(&vm));

    assert!(
        focus_text_editor(&mut ui).is_some(),
        "the editor card must expose its text_editor as a focusable target"
    );
    let typed = ui.typewrite("hello");
    assert!(
        matches!(typed, iced::event::Status::Captured),
        "the focused text_editor must consume the typed characters"
    );

    let _ = ui.click(strings::SAVE_BUTTON);

    let emitted = messages(ui);
    assert!(
        emitted.contains(&Message::Editor(EditorMessage::SaveRequested {
            path: PathBuf::from("/notes/a.md"),
        })),
        "Save must request a save for the open document, got {emitted:?}"
    );
    assert!(
        emitted
            .iter()
            .any(|m| matches!(m, Message::Editor(EditorMessage::Action(_)))),
        "typing must reach the editor as an action message, got {emitted:?}"
    );
}

/// Cancel dismisses without a save request.
#[test]
fn test_editor_cancel_never_requests_a_save() {
    let vm = EditorViewModel::open("/notes/a.md", "# Title\n");
    let mut ui = sim(editor_card(&vm));
    let _ = ui.click(strings::CANCEL_BUTTON);

    let emitted = messages(ui);
    assert_eq!(
        emitted,
        vec![Message::Editor(EditorMessage::CancelRequested)],
        "Cancel must publish exactly the cancel message",
    );
}

// ---------------------------------------------------------------------------
// Bottom band — command input Enter submit
// ---------------------------------------------------------------------------
/// Enter in the command band submits the field's text as
/// `AgentMessage::Submit`. iced's
/// `text_input` owns the Enter handling, so the tier-2 assertion is that the
/// real field really produces the submit message once the reducer has taken
/// the typed text in.
#[test]
fn test_command_band_enter_submits_the_typed_prompt() {
    let mut state = test_app_state();

    // 1. type into the real band field — keystrokes stream as InputChanged.
    let mut ui = sim(bottom_panel(&state.models.bottom));
    assert!(
        focus_first_text_input(&mut ui).is_some(),
        "the command band must expose its command input as a focusable field"
    );
    let _ = ui.typewrite("/models");
    let typed = messages(ui);
    assert_emitted(
        &typed,
        &Message::Agent(AgentMessage::InputChanged {
            text: "/models".to_string(),
        }),
        "typing must stream the field contents",
    );
    assert!(
        !typed
            .iter()
            .any(|m| matches!(m, Message::Agent(AgentMessage::Submit { .. }))),
        "typing alone must not submit, got {typed:?}"
    );

    // 2. run the typed text through the real reducer, rebuild, press Enter.
    for message in typed {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();
    assert_eq!(
        state.models.bottom.command_input, "/models",
        "the reducer must hold what the field reported"
    );

    let mut ui = sim(bottom_panel(&state.models.bottom));
    focus_first_text_input(&mut ui);
    let _ = ui.tap_key(iced::keyboard::key::Named::Enter);

    assert_emitted(
        &messages(ui),
        &Message::Agent(AgentMessage::Submit {
            text: "/models".to_string(),
        }),
        "Enter must submit the field's current text",
    );
}

// ---------------------------------------------------------------------------
// Right pane — ToC row click
// ---------------------------------------------------------------------------

/// Clicking a ToC row publishes `UserCommand::ScrollToHeader` for that
/// heading, driven through the command wiring in `view::toc_body`.
#[test]
fn test_toc_row_click_publishes_scroll_to_header() {
    use crate::markdown::ToCEntry;

    let (mut state, _dir) = state_with_one_file();
    let path = state.app.workspace().all_files[0].clone();
    state.app.selection_mut().selected_file = Some(path);
    state.app.orchestrator.tabs.toc = vec![
        ToCEntry::new("Getting Started", 1, "Getting Started"),
        ToCEntry::new("Nitty Gritty", 2, "Nitty Gritty"),
    ];
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    let _ = ui.click("Nitty Gritty");

    assert_emitted(
        &messages(ui),
        &Message::Command(
            crate::bus::events::user_command::UserCommand::ScrollToHeader(
                "Nitty Gritty".to_string(),
            ),
        ),
        "a ToC row click must publish the scroll command for that heading id",
    );
}

// ---------------------------------------------------------------------------
// Dialog — name entry
// ---------------------------------------------------------------------------

/// Typing in the name-entry dialog streams `NameChanged` messages, one per
/// character, with the accumulated text (UI-011 / UI-015).
#[test]
fn test_name_entry_dialog_streams_name_changed_messages() {
    let mut ds = DialogState::new();
    ds.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });

    let mut ui = sim(crate::ui_iced::dialogs::dialog_card(
        &ds,
        None,
        iced::Size::new(1024.0, 768.0),
    ));
    assert!(
        focus_first_text_input(&mut ui).is_some(),
        "the name-entry card must expose its text field"
    );
    let _ = ui.typewrite("sub");

    assert_eq!(
        messages(ui),
        vec![
            Message::Dialog(DialogMessage::NameChanged {
                text: "s".to_string(),
            }),
            Message::Dialog(DialogMessage::NameChanged {
                text: "su".to_string(),
            }),
            Message::Dialog(DialogMessage::NameChanged {
                text: "sub".to_string(),
            }),
        ],
        "each keystroke must publish the accumulated name",
    );
}

/// The full round trip: type in the dialog, feed the keystrokes through the
/// real reducer, rebuild the card, then click Ok — which is only enabled
/// once the reducer has accepted a valid name — and get `Submitted`.
#[test]
fn test_name_entry_dialog_submit_after_reducer_accepts_the_name() {
    let mut state = test_app_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    state.rebuild_view_models();

    // Ok is inert while the name is empty, so the dialog cannot be submitted
    // before the reducer has seen a name.
    let mut before = sim(crate::ui_iced::dialogs::dialog_card(
        &state.overlay.dialog,
        None,
        iced::Size::new(1024.0, 768.0),
    ));
    let _ = before.click(strings::OK_BUTTON);
    let inert = messages(before);
    assert!(
        !inert.contains(&Message::Dialog(DialogMessage::Submitted)),
        "an empty name must leave the Ok button inert, got {inert:?}"
    );
    assert_eq!(
        inert,
        vec![Message::Dialog(DialogMessage::Dismissed)],
        "the inert Ok click must fall through to the modal scrim, which dismisses",
    );

    // The reducer accepts the typed name.
    let _ = update(
        &mut state,
        Message::Dialog(DialogMessage::NameChanged {
            text: "subdir".to_string(),
        }),
    );
    state.rebuild_view_models();
    assert_eq!(state.overlay.dialog.pending_name(), "subdir");

    // Now the same click submits.
    let mut after = sim(crate::ui_iced::dialogs::dialog_card(
        &state.overlay.dialog,
        None,
        iced::Size::new(1024.0, 768.0),
    ));
    let _ = after.click(strings::OK_BUTTON);
    assert_eq!(
        messages(after),
        vec![Message::Dialog(DialogMessage::Submitted)],
        "a valid name must make Ok submit the dialog",
    );
}

// ---------------------------------------------------------------------------
// Top bar — hamburger menu
// ---------------------------------------------------------------------------

/// Clicking the ☰ button captures the click and opens the menu.
#[test]
fn test_top_bar_hamburger_consumes_the_open_click() {
    let state = test_app_state();
    let mut ui = sim(top_bar(&state.models.top));

    let glyph = strings::HAMBURGER_MENU_BUTTON;
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text {
                content, bounds, ..
            } if content == glyph => Some(bounds),
            _ => None,
        })
        .expect("the ☰ hamburger button");

    click_at(&mut ui, bounds.center());
    let emitted = messages(ui);
    assert!(
        emitted.contains(&Message::Menu(MenuMessage::Opened)),
        "the ☰ button must publish the open message, got {emitted:?}"
    );
}

/// The hamburger occupies the far-right end of the 1024-point simulator
/// viewport, rather than sitting immediately after the tag selector.
#[test]
fn test_top_bar_hamburger_is_at_the_far_right_edge() {
    let state = test_app_state();
    let mut ui = sim(top_bar(&state.models.top));
    let glyph = strings::HAMBURGER_MENU_BUTTON;
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text {
                content, bounds, ..
            } if content == glyph => Some(bounds),
            _ => None,
        })
        .expect("the ☰ hamburger button");

    assert!(
        bounds.x > 900.0,
        "hamburger should be at the far right of the 1024-point viewport, got x={}",
        bounds.x
    );
    assert!(
        bounds.x + bounds.width <= 1024.0,
        "hamburger should remain inside the viewport, got {bounds:?}"
    );
}

/// The full loop on the framework path: `Opened` mounts the cascade's root
/// card in the real `view()`, a row click addresses that row by card and
/// index, and the reducer runs the cascade so the row's own close policy
/// decides whether the tree survives.
#[test]
fn test_hamburger_menu_opens_and_routes_a_row_through_the_reducer() {
    let mut state = test_app_state();

    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();
    assert!(state.overlay.hamburger_open, "Opened must flip the flag");
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        1,
        "open mounts exactly the root card"
    );
    assert_eq!(
        state.overlay.hamburger_rects.len(),
        1,
        "the registered surface carries the root card's solved rectangle"
    );

    let menu = hamburger_menu(&state.models.top);
    let mut ui = sim(view(&state));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text {
                content, bounds, ..
            } if content == strings::MENU_ABOUT => Some(bounds),
            _ => None,
        })
        .expect("the About row inside the mounted hamburger card");
    click_at(&mut ui, bounds.center());
    let emitted = messages(ui);
    let address = picked(MenuId::new(HAMBURGER_MENU_ID), &menu, KEY_ABOUT);
    assert_emitted(
        &emitted,
        &address,
        "clicking the About row must address it by card path and row index",
    );

    let _ = update(&mut state, address);
    assert!(
        !state.overlay.hamburger_open,
        "an Action row's ClosePolicy must close the tree in the same reduction"
    );
    assert!(
        state.overlay.hamburger_rects.is_empty(),
        "a closed tree registers no region"
    );
}

/// The label a framework row paints is the model's own label, not a
/// check-slot or chevron baked into the string.
///
/// Pins the rendered-text change INT-3a introduces: the previous path drew
/// `"✓ About FastMD..."` and `"Windows... >"` out of
/// `strings::format_menu_row_label`, so a test written against those
/// strings cannot find the row any more.
#[test]
fn test_hamburger_row_labels_are_the_bare_model_labels() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    for label in [
        strings::MENU_BATCH,
        strings::MENU_TOOLS,
        strings::MENU_ABOUT,
    ] {
        assert!(
            ui.find(label).is_ok(),
            "root row {label:?} must render under its own label"
        );
    }
    for baked in [
        strings::format_menu_row_label(None, strings::MENU_ABOUT, false),
        format!("{} >", strings::MENU_WINDOWS),
    ] {
        assert!(
            ui.find(baked.as_str()).is_err(),
            "the check-slot / chevron-baked label {baked:?} must no longer be painted"
        );
    }
}

/// Clicking a submenu parent mounts its card beside the root instead of
/// replacing it, and the cascade — not a hand-held path — records which
/// cards are up.
#[test]
fn test_hamburger_submenu_path_is_controlled_by_the_reducer() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();

    let root = MenuId::new(HAMBURGER_MENU_ID);
    let menu = hamburger_menu(&state.models.top);

    let mut ui = sim(view(&state));
    let _ = ui.click(strings::MENU_WINDOWS);
    let windows_pick = picked(root.clone(), &menu, KEY_WINDOWS);
    assert_emitted(
        &messages(ui),
        &windows_pick,
        "Windows must be a clickable submenu row on the root card",
    );

    let _ = update(&mut state, windows_pick);
    state.rebuild_view_models();
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "the submenu card mounts above the root rather than replacing it"
    );
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &submenu_id(&root, KEY_WINDOWS),
        "the second card is the Windows submenu the row opened"
    );
    assert_eq!(
        state.overlay.hamburger_rects.len(),
        2,
        "both cards register their own solved rectangle"
    );

    // `Sim` holds the rendered `Element`, which borrows `state`
    // invariantly, so each driver is scoped and dropped before the next
    // reduction rather than living across a `&mut state`.
    {
        let mut ui = sim(view(&state));
        assert!(ui.find(strings::MENU_BACKGROUND_OPERATIONS).is_ok());
        assert!(ui.find(strings::MENU_AGENT_DEBUG).is_ok());
        assert!(ui.find(strings::MENU_CHAT_MODELS).is_ok());
    }

    // A second submenu parent swaps the deep card for its own branch.
    let chat_pick = picked(root.clone(), &menu, KEY_CHAT_MODELS);
    let _ = update(&mut state, chat_pick);
    state.rebuild_view_models();
    assert_eq!(
        state.overlay.hamburger_cascade.open_cards()[1].id(),
        &submenu_id(&root, KEY_CHAT_MODELS),
        "opening another branch replaces the deep card"
    );
    {
        let mut ui = sim(view(&state));
        assert!(
            ui.find(strings::NO_CHAT_MODELS_CONFIGURED).is_ok(),
            "the empty-models placeholder must paint so the card is never blank"
        );
    }
}

/// A click that misses the mounted menu lands on the blocker and closes it.
#[test]
fn test_hamburger_menu_click_away_closes_the_menu() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    // Far bottom-left corner: outside the right-anchored menu.
    click_at(&mut ui, iced::Point::new(4.0, 760.0));
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Menu(MenuMessage::Closed),
        "a click outside the menu must close it",
    );
}

/// Every activatable row of the production hamburger tree resolves to a
/// self-contained `UserCommand`, and each kind resolves through the effect
/// the framework actually reports for it.
///
/// The previous path baked the checked state into the label and fired a
/// pre-built command, so "which command" and "what new value" were the
/// same string. Here the row's **key** says what was picked and the
/// [`ActivationEffect`] says what happened; the resolver joins them.
#[test]
fn test_hamburger_rows_emit_their_commands() {
    let state = test_app_state();
    let menu = hamburger_menu(&state.models.top);

    // Root: the three dialog actions are the only publishers there.
    let root_publishing: Vec<&str> = menu
        .rows()
        .iter()
        .filter(|row| row.activatable() && row.child_id().is_none())
        .map(|row| row.key())
        .collect();
    assert_eq!(
        root_publishing,
        [KEY_BATCH, KEY_TOOLS, KEY_ABOUT],
        "the root card publishes exactly the three dialog actions"
    );
    for key in &root_publishing {
        assert!(
            command_for_activation(key, &ActivationEffect::Command).is_some(),
            "root action {key:?} must resolve to a command"
        );
    }

    // Submenu parents open a card; they publish no command of their own.
    for key in [KEY_WINDOWS, KEY_CHAT_MODELS, KEY_TABLE_WRAP] {
        assert!(
            menu.rows()
                .iter()
                .any(|row| row.key() == key && row.child_id().is_some()),
            "{key:?} must be a submenu parent on the root card"
        );
    }

    // Check rows carry their new value on the effect, not in the label.
    assert_eq!(
        command_for_activation(
            KEY_BACKGROUND_OPERATIONS,
            &ActivationEffect::Toggled { checked: true }
        ),
        Some(apply_background_logs_toggle(true)),
        "Background operations must publish the value the cascade committed"
    );
    assert_eq!(
        command_for_activation(
            KEY_AGENT_DEBUG,
            &ActivationEffect::Toggled { checked: false }
        ),
        Some(apply_agent_debug_toggle(false)),
        "Agent debug must publish the value the cascade committed"
    );

    // Radio rows name what they select, whatever their siblings were on.
    assert_eq!(
        command_for_activation(
            "model:gpt-4o",
            &ActivationEffect::Selected {
                group: RadioGroupId::new(RADIO_GROUP_CHAT_MODEL),
                cleared: vec![0, 1],
            }
        ),
        Some(UserCommand::SelectChatModel("gpt-4o".to_owned())),
        "a chat-model radio row must select the model its own key names"
    );
    for variant in strategy_menu_order() {
        let key = strategy_key(variant);
        assert_eq!(
            command_for_activation(
                &key,
                &ActivationEffect::Selected {
                    group: RadioGroupId::new(RADIO_GROUP_TABLE_WRAP),
                    cleared: Vec::new(),
                }
            ),
            Some(apply_table_width_strategy_change(variant)),
            "{key:?} must resolve back to its own strategy"
        );
    }

    // Furniture and unknown keys publish nothing rather than guessing.
    assert_eq!(
        command_for_activation(KEY_NO_MODELS, &ActivationEffect::Command),
        None,
        "the disabled placeholder must publish nothing"
    );
    assert_eq!(
        command_for_activation("not-a-hamburger-row", &ActivationEffect::Command),
        None,
        "an unknown key must publish nothing"
    );
}

// ---------------------------------------------------------------------------
// Whole-app smoke
// ---------------------------------------------------------------------------

/// The complete `view(&AppState)` builds and drives under the simulator,
/// with an overlay mounted — the shape the shipped app renders.
#[test]
fn test_full_app_view_is_simulatable_with_an_overlay() {
    let mut state = test_app_state();
    state.overlay.dialog.open(DialogKind::CreateDocument {
        parent: PathBuf::from("/notes"),
    });
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    assert!(
        ui.find(strings::CREATE_DOCUMENT_WINDOW).is_ok(),
        "the mounted dialog title must be rendered above the bands"
    );
    assert!(
        focus_first_text_input(&mut ui).is_some(),
        "the dialog's name field must be reachable through the full view"
    );
}

/// The full-app content root reports the pointer position in window space;
/// row-local bounds and the scrollable's translation do not affect the menu
/// origin. (`Simulator::point_at` alone emits no move event.)
#[test]
fn test_tree_pointer_motion_emits_the_cursor_anchor() {
    let (state, _dir) = state_with_one_file();
    let label = alpha_row_label();

    let mut ui = sim(view(&state));
    let bounds = ui
        .find(label.as_str())
        .expect("row")
        .visible_bounds()
        .expect("visible row");
    let pointer = bounds.center();
    ui.point_at(pointer);
    let _ = ui.simulate([cursor_moved(pointer)]);

    let emitted = messages(ui);
    let anchor = emitted
        .iter()
        .find_map(|m| match m {
            Message::Layout(LayoutMessage::CursorMoved { point }) => Some(*point),
            _ => None,
        })
        .unwrap_or_else(|| panic!("pointer motion must report the menu anchor, got {emitted:?}"));

    assert!(
        anchor == pointer,
        "root-space pointer {anchor:?} should equal full-view point {pointer:?}"
    );
}
