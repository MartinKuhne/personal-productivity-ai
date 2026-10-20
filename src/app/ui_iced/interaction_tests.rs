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
//! tool allows. The hamburger is **not affected**: it is a ☰ button
//! plus a real card mounted by the overlay host, so its rows are ordinary
//! `Text`/`Button` widgets. [`test_top_bar_hamburger_consumes_the_open_click`]
//! proves the ☰ captures the open click, and
//! [`test_hamburger_menu_opens_and_routes_a_row_through_the_reducer`]
//! drives the whole loop — reducer opens, real `view()` mounts the card,
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

use crate::config::ContentLibrary;
use crate::ui::strings;
use crate::ui_iced::app::AppState;
use crate::ui_iced::dialogs::{DialogKind, DialogState};
use crate::ui_iced::editor::{EditorViewModel, editor_card};
use crate::ui_iced::message::{
    AgentMessage, DialogMessage, EditorMessage, LayoutMessage, MenuMessage, Message, TreeMessage,
};
use crate::ui_iced::overlay::menu_labels;
use crate::ui_iced::panels::bottom::bottom_panel;
use crate::ui_iced::panels::center::{center_panel, tab_select_command};
use crate::ui_iced::panels::left::{left_panel, row_icon};
use crate::ui_iced::panels::top::{
    MenuEntry, MenuEntryKind, MenuSection, apply_about_button_click, build_menu_entries,
    hamburger_menu_card, hamburger_row_label, hamburger_row_message, top_bar,
};
use crate::ui_iced::test_helpers::test_app_state;
use crate::ui_iced::update::update;
use crate::ui_iced::view::view;
use iced::Element;
use iced::mouse;
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
        .file_processor_mut()
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

    // 1. right-click the row in the real left panel.
    let mut ui = sim(left_panel(&state.models.left));
    let bounds = ui
        .find(label.as_str())
        .expect("row")
        .visible_bounds()
        .expect("visible row");
    ui.point_at(bounds.center());
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
    assert!(
        state.overlay.context_menu.is_some(),
        "the reducer must mount the context menu"
    );

    // 3. rebuild the full app view and assert the menu rows are on screen.
    let expected = menu_labels(&state.overlay.context_menu.as_ref().unwrap().items);
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
    let first_message = state.overlay.context_menu.as_ref().unwrap().items[0]
        .message
        .clone();
    let _ = view_ui.click(first.as_str());
    assert_eq!(
        messages(view_ui),
        vec![first_message],
        "the clicked menu row must publish its own command",
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
    let path = state.app.file_processor().all_files[0].clone();
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

/// The full loop: `Opened` mounts the card in the real `view()`, a row pick
/// publishes that row's own [`UserCommand`] on the menu channel, and the
/// reducer closes the menu in the same reduction.
#[test]
fn test_hamburger_menu_opens_and_routes_a_row_through_the_reducer() {
    let mut state = test_app_state();

    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();
    assert!(state.overlay.hamburger_open, "Opened must flip the flag");

    let about = strings::format_menu_row_label(None, strings::MENU_ABOUT, false);
    let mut ui = sim(view(&state));
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text {
                content, bounds, ..
            } if content == about.as_str() => Some(bounds),
            _ => None,
        })
        .expect("the About row inside the mounted hamburger card");
    click_at(&mut ui, bounds.center());
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Menu(MenuMessage::RowActivated(apply_about_button_click())),
        "clicking the About row must publish its own command on the menu channel",
    );

    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::RowActivated(apply_about_button_click())),
    );
    assert!(
        !state.overlay.hamburger_open,
        "a row pick must close the menu in the same reduction"
    );
}

/// Clicking a submenu button changes the mounted card to that submenu, and
/// the back row returns to the root menu without executing an application
/// command.
#[test]
fn test_hamburger_submenu_navigation_round_trips_through_the_reducer() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));

    let mut ui = sim(view(&state));
    let submenu_label = strings::format_menu_submenu_label(strings::MENU_WINDOWS);
    let _ = ui.click(submenu_label.as_str());
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Menu(MenuMessage::SubmenuOpened(MenuSection::Windows)),
        "Windows must open as a clickable submenu",
    );

    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::SubmenuOpened(MenuSection::Windows)),
    );
    assert_eq!(state.overlay.hamburger_submenu, Some(MenuSection::Windows));

    let mut ui = sim(view(&state));
    let background_label = hamburger_row_label(strings::MENU_BACKGROUND_OPERATIONS, false);
    let debug_label = hamburger_row_label(strings::MENU_AGENT_DEBUG, false);
    assert!(ui.find(background_label.as_str()).is_ok());
    assert!(ui.find(debug_label.as_str()).is_ok());
    assert!(ui.find(strings::MENU_CHAT_MODELS).is_err());

    let _ = ui.click(strings::MENU_BACK_TO_MAIN);
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Menu(MenuMessage::SubmenuClosed),
        "the back row must return to the top-level menu",
    );
    let _ = update(&mut state, Message::Menu(MenuMessage::SubmenuClosed));
    assert_eq!(state.overlay.hamburger_submenu, None);

    let mut ui = sim(view(&state));
    let chat_models = strings::format_menu_submenu_label(strings::MENU_CHAT_MODELS);
    let _ = ui.click(chat_models.as_str());
    assert!(
        messages(ui).contains(&Message::Menu(MenuMessage::SubmenuOpened(
            MenuSection::ChatModels,
        )))
    );
    let _ = update(
        &mut state,
        Message::Menu(MenuMessage::SubmenuOpened(MenuSection::ChatModels)),
    );
    let mut ui = sim(view(&state));
    assert!(ui.find(strings::NO_CHAT_MODELS_CONFIGURED).is_ok());
}

/// A click that misses the mounted card lands on the blocker and closes it.
#[test]
fn test_hamburger_menu_click_away_closes_the_card() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();

    let mut ui = sim(view(&state));
    // Far bottom-left corner: outside the right-anchored card.
    click_at(&mut ui, iced::Point::new(4.0, 760.0));
    let emitted = messages(ui);
    assert_emitted(
        &emitted,
        &Message::Menu(MenuMessage::Closed),
        "a click outside the card must close it",
    );
}

/// Every selectable row of the production [`hamburger_menu_card`] publishes
/// its own `UserCommand` on the menu channel when clicked.
///
/// The card is rendered standalone here; the full-view loop (reducer open →
/// mounted card → row pick) is covered by
/// [`test_hamburger_menu_opens_and_routes_a_row_through_the_reducer`].
#[test]
fn test_hamburger_rows_emit_their_commands() {
    let state = test_app_state();
    let entries = build_menu_entries(&state.models.top);
    let actions: Vec<&MenuEntry> = entries
        .iter()
        .filter(|entry| matches!(entry.kind, MenuEntryKind::Action(_)))
        .collect();
    assert!(
        actions.len() >= 5,
        "the hamburger must offer the full UI-061..UI-066 row set"
    );

    // `Element` is neither `Clone` nor `Debug`, so the card is rebuilt for
    // each click rather than shared.
    for entry in actions {
        let command = match &entry.kind {
            MenuEntryKind::Action(command) => command,
            other => unreachable!("filtered to Action rows, got {other:?}"),
        };
        let label = hamburger_row_label(&entry.label, entry.checked);
        let active_submenu = (entry.section != MenuSection::Direct).then_some(entry.section);
        let mut ui = sim(hamburger_menu_card(&entries, active_submenu));
        let _ = ui.click(label.as_str());
        assert_eq!(
            messages(ui),
            vec![hamburger_row_message(command)],
            "hamburger row {:?} must publish its own command",
            entry.label,
        );
    }
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

/// The `Layout(CursorMoved)` anchor the tree emits for menu positioning is
/// synthesised by the row's `mouse_area` as soon as the cursor position
/// changes (verified: `Simulator::point_at` alone emits no
/// event, so a following event is what surfaces the move).
#[test]
fn test_tree_pointer_motion_emits_the_cursor_anchor() {
    let (state, _dir) = state_with_one_file();
    let label = alpha_row_label();

    let mut ui = sim(left_panel(&state.models.left));
    let bounds = ui
        .find(label.as_str())
        .expect("row")
        .visible_bounds()
        .expect("visible row");
    ui.point_at(bounds.center());
    let _ = ui.simulate(left_click());

    let viewport = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Scrollable { bounds, .. } => Some(bounds),
            _ => None,
        })
        .expect("the tree body is a scrollable");

    let emitted = messages(ui);
    let anchor = emitted
        .iter()
        .find_map(|m| match m {
            Message::Layout(LayoutMessage::CursorMoved { point }) => Some(*point),
            _ => None,
        })
        .unwrap_or_else(|| panic!("pointer motion must report the menu anchor, got {emitted:?}"));

    // The anchor is reported in the scrollable's translated space, which is
    // the viewport-relative frame the context menu clamps against. Confirm
    // that relationship against the scrollable's own bounds rather than
    // asserting a bare coordinate.
    assert!(
        anchor.x >= viewport.x && anchor.y >= 0.0 && anchor.x < viewport.x + viewport.width,
        "anchor {anchor:?} must sit inside the viewport frame {viewport:?}"
    );
    assert!(
        anchor != bounds.center(),
        "the anchor is viewport-relative, not the row's absolute centre"
    );
}
