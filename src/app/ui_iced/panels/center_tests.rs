//! Tests for `ui_iced/panels/center.rs`.
//!
//! Widget construction cannot be driven headlessly without the iced
//! simulator, so these tests cover the state/mapping layer the widget tree
//! is built from: the tab-title/active-index mapping, the tab-strip entry
//! constructor, the "Close Others" set, the tab-select / close command
//! construction, the full tab context-menu item → message table, the
//! UI-048 thinking gate and the view-model constructor.
//!
//! [RUST-006]: every filesystem path used here is either a synthetic
//! string that is never opened, or lives inside a `tempfile::TempDir`. In
//! particular every `from_app` call runs against a "System" content
//! library rooted in a temp dir, so the note-skill lookup never touches
//! the real user profile, and the PDF-backing test creates its `.md`/`.pdf`
//! pair inside a temp dir.

// Several assertions in the INT-11 section target compile-time token values
// on purpose: pinning the band budget and the indicator's weight is the whole
// point, so a retuned token must fail the suite rather than pass silently.
// `clippy::assertions_on_constants` cannot tell a spec pin from a tautology
// (same treatment as `left_tests.rs` and `src/iced-pro/token/*_tests.rs`).
#![allow(clippy::assertions_on_constants)]

use super::*;
use crate::config::ContentLibrary;
use crate::ui_iced::message::TreeMessage;
use crate::ui_iced::test_helpers::test_app;
use tempfile::TempDir;

/// The visible labels of a context-row list, in render order.
fn row_labels(rows: &[ContextRow]) -> Vec<&str> {
    rows.iter().map(|item| item.label.as_str()).collect()
}

// --- fixtures -----------------------------------------------------------

/// A tab-strip entry for `path`, optionally active.
fn entry(path: &str, active: bool) -> TabEntry {
    TabEntry::new(PathBuf::from(path), active)
}

/// A synthetic note-skill descriptor (its file is never read by the view).
fn skill(name: &str, path: &str) -> SkillFile {
    SkillFile {
        name: name.to_string(),
        path: PathBuf::from(path),
    }
}

/// A test app whose "System" content library is rooted in a temp dir, so
/// `from_app`'s note-skill lookup stays off the real user profile
/// ([RUST-006]).
fn app_with_system_lib() -> (FastMdApp, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = test_app();
    app.content_libraries_mut().push(ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "System".to_string(),
        kind: "system".to_string(),
        readonly: false,
        priority: 0,
    });
    (app, dir)
}

/// Opens `paths` as tabs, in order.
fn open_tabs(app: &mut FastMdApp, paths: &[&str]) {
    for path in paths {
        app.tabs_mut().open_tab(PathBuf::from(path));
    }
}

/// Makes `path` the selected file.
fn select(app: &mut FastMdApp, path: &str) {
    *app.selection_mut().selected_file_mut() = Some(PathBuf::from(path));
}

/// The message a menu row with `label` carries.
fn message_for(items: &[ContextRow], label: &str) -> Message {
    items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("no menu row labelled {label:?}"))
        .message
        .clone()
}

// --- tab titles ---------------------------------------------------------

#[test]
fn tab_title_uses_the_file_name() {
    assert_eq!(tab_title(Path::new("docs/notes.md")), "notes.md");
    assert_eq!(tab_title(Path::new("C:\\lib\\a\\b\\Todo.MD")), "Todo.MD");
}

#[test]
fn tab_title_of_a_path_without_a_file_name_is_empty() {
    // Mirrors `Tabs::tab_strip`'s `unwrap_or_default` fallback.
    assert_eq!(tab_title(Path::new("/")), "");
}

// --- active-tab mapping -------------------------------------------------

#[test]
fn active_tab_index_finds_the_selected_path() {
    let paths = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    assert_eq!(active_tab_index(&paths, Some(Path::new("b.md"))), Some(1));
}

#[test]
fn active_tab_index_is_none_when_nothing_is_selected() {
    let paths = vec![PathBuf::from("a.md")];
    assert_eq!(active_tab_index(&paths, None), None);
}

#[test]
fn active_tab_index_is_none_when_the_selection_is_not_open() {
    let paths = vec![PathBuf::from("a.md")];
    assert_eq!(
        active_tab_index(&paths, Some(Path::new("never-opened.md"))),
        None
    );
}

#[test]
fn active_tab_index_on_an_empty_strip_is_none() {
    assert_eq!(active_tab_index(&[], Some(Path::new("a.md"))), None);
}

// --- tab-strip entries --------------------------------------------------

#[test]
fn tab_entries_preserve_strip_order_and_mark_only_the_active_tab() {
    let paths = vec![
        PathBuf::from("lib/one.md"),
        PathBuf::from("lib/two.md"),
        PathBuf::from("lib/three.md"),
    ];
    let tabs = tab_entries(&paths, Some(Path::new("lib/two.md")));

    assert_eq!(tabs.len(), 3);
    assert_eq!(
        tabs.iter()
            .map(|tab| tab.title.as_str())
            .collect::<Vec<_>>(),
        vec!["one.md", "two.md", "three.md"]
    );
    assert_eq!(
        tabs.iter().map(|tab| tab.active).collect::<Vec<_>>(),
        vec![false, true, false]
    );
    assert_eq!(tabs[1].path, PathBuf::from("lib/two.md"));
}

#[test]
fn tab_entries_are_never_unsaved_without_dirty_tracking() {
    // The app has no per-tab dirty flag, so the pure constructor reports
    // clean. The field exists for the reducer to fill.
    let paths = vec![PathBuf::from("a.md")];
    let tabs = tab_entries(&paths, Some(Path::new("a.md")));
    assert!(!tabs[0].unsaved);
}

#[test]
fn tab_entries_with_no_paths_is_empty() {
    assert!(tab_entries(&[], None).is_empty());
}

#[test]
fn tab_entry_new_derives_its_title_from_the_path() {
    let tab = entry("work/notes.md", true);
    assert_eq!(tab.title, "notes.md");
    assert!(tab.active);
    assert!(!tab.unsaved);
}

// --- Close Others (UI-028) ---------------------------------------------

#[test]
fn other_tab_indices_excludes_only_the_kept_tab() {
    let tabs = vec![
        entry("a.md", false),
        entry("b.md", true),
        entry("c.md", false),
    ];
    assert_eq!(other_tab_indices(&tabs, 1), vec![0, 2]);
    assert_eq!(other_tab_indices(&tabs, 0), vec![1, 2]);
    assert_eq!(other_tab_indices(&tabs, 2), vec![0, 1]);
}

#[test]
fn other_tab_indices_of_a_single_tab_is_empty() {
    let tabs = vec![entry("a.md", true)];
    assert!(other_tab_indices(&tabs, 0).is_empty());
}

#[test]
fn other_tab_indices_with_an_out_of_range_keep_closes_everything() {
    let tabs = vec![entry("a.md", false), entry("b.md", false)];
    assert_eq!(other_tab_indices(&tabs, 9), vec![0, 1]);
}

// --- tab select / close commands ----------------------------------------

#[test]
fn tab_select_command_is_a_single_select_select_file() {
    // The tab strip never multi-selects.
    assert_eq!(
        tab_select_command(Path::new("lib/note.md")),
        UserCommand::SelectFile {
            path: PathBuf::from("lib/note.md"),
            multi: false,
        }
    );
}

#[test]
fn tab_close_message_wraps_close_tab() {
    assert_eq!(
        tab_close_message(3),
        Message::Command(UserCommand::CloseTab(3))
    );
    assert_eq!(
        tab_close_message(0),
        Message::Command(apply_tab_close_click(0))
    );
}

#[test]
fn context_menu_open_message_names_the_tab_index() {
    assert_eq!(
        tab_context_menu_open_message(2),
        Message::Tab(TabMessage::ContextMenuOpened { index: 2 })
    );
}

#[test]
fn context_menu_close_message_is_the_dismissal() {
    assert_eq!(
        tab_context_menu_close_message(),
        Message::Tab(TabMessage::ContextMenuClosed)
    );
}

#[test]
fn agent_session_close_message_clears_the_agent_session() {
    assert_eq!(
        agent_session_close_message(),
        Message::Command(UserCommand::ClearAgentSession)
    );
}

// --- tab context menu (UI-026..UI-034) ---------------------------------

#[test]
fn tab_context_menu_labels_are_in_the_expected_order() {
    let items = tab_context_menu_items(1, Path::new("lib/note.md"), &[]);
    assert_eq!(
        row_labels(&items),
        vec![
            strings::CLOSE_TAB_MENU,
            strings::CLOSE_OTHERS_MENU,
            strings::CLOSE_ALL_TABS_MENU,
            strings::EDIT_BUTTON,
            strings::OPEN_IN_EDITOR_ACTION,
            strings::COPY_PATH_ACTION,
            strings::SHOW_IN_EXPLORER_ACTION,
        ],
        "INT-3b reordered to §4.9.1's Tab Lifecycle | Edit & Automation | \
         Location runs; no label changed"
    );
}

#[test]
fn tab_context_menu_close_rows_map_to_their_tab_commands() {
    let items = tab_context_menu_items(4, Path::new("note.md"), &[]);

    assert_eq!(
        message_for(&items, strings::CLOSE_TAB_MENU),
        Message::Command(UserCommand::CloseTab(4))
    );
    assert_eq!(
        message_for(&items, strings::CLOSE_OTHERS_MENU),
        Message::Command(UserCommand::CloseOtherTabs(4))
    );
    assert_eq!(
        message_for(&items, strings::CLOSE_ALL_TABS_MENU),
        Message::Command(UserCommand::CloseAllTabs)
    );
}

#[test]
fn tab_context_menu_path_rows_map_to_their_commands() {
    let path = Path::new("C:\\lib\\note.md");
    let items = tab_context_menu_items(0, path, &[]);
    let owned = path.to_path_buf();

    assert_eq!(
        message_for(&items, strings::EDIT_BUTTON),
        Message::Command(UserCommand::OpenInEditor(owned.clone()))
    );
    assert_eq!(
        message_for(&items, strings::COPY_PATH_ACTION),
        Message::Command(UserCommand::CopyPath(owned.clone()))
    );
    assert_eq!(
        message_for(&items, strings::SHOW_IN_EXPLORER_ACTION),
        Message::Command(UserCommand::ShowInExplorer(owned.clone()))
    );
    assert_eq!(
        message_for(&items, strings::OPEN_IN_EDITOR_ACTION),
        Message::Command(UserCommand::OpenInEditor(owned))
    );
}

#[test]
fn tab_context_menu_note_skill_rows_map_to_deferred_skill_runs() {
    // UI-033: "Format Markdown" is a VFS-121 note skill, not a
    // `UserCommand`. The row carries the deferred intent; the reducer reads
    // the skill file and publishes `RunSkillPrompt`.
    let skills = vec![
        skill("FormatMarkdown", "C:\\sys\\Skills\\Note\\FormatMarkdown.md"),
        skill("summarize", "C:\\sys\\Skills\\Note\\summarize.md"),
    ];
    let items = tab_context_menu_items(2, Path::new("lib/note.md"), &skills);

    assert_eq!(items.len(), 9, "seven fixed rows plus one per skill");
    assert_eq!(
        message_for(&items, "FormatMarkdown"),
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("C:\\sys\\Skills\\Note\\FormatMarkdown.md"),
            target_dir: None,
            target_file: Some(PathBuf::from("lib/note.md")),
        })
    );
    assert_eq!(
        message_for(&items, "summarize"),
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("C:\\sys\\Skills\\Note\\summarize.md"),
            target_dir: None,
            target_file: Some(PathBuf::from("lib/note.md")),
        })
    );
}

#[test]
fn tab_context_menu_skill_rows_sit_inside_the_automation_run() {
    let skills = vec![skill("FormatMarkdown", "sys/FormatMarkdown.md")];
    let items = tab_context_menu_items(0, Path::new("note.md"), &skills);
    let index = items
        .iter()
        .position(|item| item.label == "FormatMarkdown")
        .expect("the skill row must be present");

    assert_eq!(items[index].group, ContextGroup::Automation);
    assert_eq!(
        items[index - 1].label,
        strings::OPEN_IN_EDITOR_ACTION,
        "the skill run follows the editor rows, not the whole menu"
    );
    assert_eq!(
        items[index + 1].label,
        strings::COPY_PATH_ACTION,
        "the Location run comes after automation, so skills are not last"
    );
}

// --- view model ---------------------------------------------------------

#[test]
fn vm_menu_items_for_out_of_range_index_is_empty() {
    let mut vm = CenterPanelViewModel {
        tabs: vec![entry("a.md", true)],
        active_tab: Some(0),
        show_agent_session: false,
        agent_status: String::new(),
        agent_running: false,
        thinking: String::new(),
        active_pdf_backed: false,
        note_skills: vec![skill("FormatMarkdown", "sys/FormatMarkdown.md")],
        hovered_tab: None,
    };

    assert_eq!(vm.menu_items_for(0).len(), 8);
    assert!(vm.menu_items_for(1).is_empty());
    assert!(vm.menu_items_for(99).is_empty());

    vm.tabs.clear();
    vm.active_tab = None;
    assert!(vm.menu_items_for(0).is_empty());
    assert!(!vm.has_tabs());
}

#[test]
fn vm_active_tab_entry_agrees_with_the_active_index() {
    let mut vm = CenterPanelViewModel {
        tabs: vec![entry("a.md", false), entry("b.md", true)],
        active_tab: Some(1),
        show_agent_session: false,
        agent_status: String::new(),
        agent_running: false,
        thinking: String::new(),
        active_pdf_backed: false,
        note_skills: Vec::new(),
        hovered_tab: None,
    };

    assert_eq!(vm.active_tab_entry(), Some(&vm.tabs[1]));
    assert_eq!(
        vm.active_tab_entry().map(|tab| tab.title.as_str()),
        Some("b.md")
    );

    vm.active_tab = None;
    assert_eq!(vm.active_tab_entry(), None);

    vm.active_tab = Some(5);
    assert_eq!(vm.active_tab_entry(), None);
}

#[test]
fn from_app_carries_open_tabs_and_the_active_flag() {
    let (mut app, _dir) = app_with_system_lib();
    open_tabs(&mut app, &["docs/one.md", "docs/two.md", "docs/three.md"]);
    select(&mut app, "docs/two.md");

    let vm = CenterPanelViewModel::from_app(&app);

    assert_eq!(vm.active_tab, Some(1));
    assert_eq!(
        vm.tabs
            .iter()
            .map(|tab| tab.title.as_str())
            .collect::<Vec<_>>(),
        vec!["one.md", "two.md", "three.md"]
    );
    assert_eq!(
        vm.tabs.iter().map(|tab| tab.active).collect::<Vec<_>>(),
        vec![false, true, false]
    );
    assert!(vm.has_tabs());
    assert_eq!(
        vm.active_tab_entry().map(|tab| tab.path.as_path()),
        Some(Path::new("docs/two.md"))
    );
}

#[test]
fn from_app_reports_no_active_tab_when_the_selection_is_not_open() {
    let (mut app, _dir) = app_with_system_lib();
    open_tabs(&mut app, &["docs/one.md"]);
    select(&mut app, "docs/other.md");

    let vm = CenterPanelViewModel::from_app(&app);
    assert_eq!(vm.active_tab, None);
    assert_eq!(vm.active_tab_entry(), None);
    assert!(vm.has_tabs());
}

#[test]
fn from_app_with_no_tabs_renders_an_empty_strip() {
    let (app, _dir) = app_with_system_lib();
    let vm = CenterPanelViewModel::from_app(&app);
    assert!(vm.tabs.is_empty());
    assert_eq!(vm.active_tab, None);
    assert!(!vm.has_tabs());
}

#[test]
fn from_app_flags_a_pdf_backed_active_document() {
    let dir = tempfile::tempdir().expect("temp dir");
    let markdown = dir.path().join("note.md");
    std::fs::write(&markdown, b"# note").expect("write md");
    std::fs::write(dir.path().join("note.pdf"), b"%PDF").expect("write pdf");

    let (mut app, _dir) = app_with_system_lib();
    app.pdf_backing_tracker()
        .process_discovered(std::slice::from_ref(&markdown));
    open_tabs(&mut app, &[markdown.to_str().unwrap()]);
    select(&mut app, markdown.to_str().unwrap());

    let vm = CenterPanelViewModel::from_app(&app);
    assert!(vm.active_pdf_backed);
}

#[test]
fn from_app_does_not_flag_pdf_backing_for_a_plain_document() {
    let dir = tempfile::tempdir().expect("temp dir");
    let markdown = dir.path().join("plain.md");
    std::fs::write(&markdown, b"# plain").expect("write md");

    let (mut app, _dir) = app_with_system_lib();
    app.pdf_backing_tracker()
        .process_discovered(std::slice::from_ref(&markdown));
    open_tabs(&mut app, &[markdown.to_str().unwrap()]);
    select(&mut app, markdown.to_str().unwrap());

    let vm = CenterPanelViewModel::from_app(&app);
    assert!(
        !vm.active_pdf_backed,
        "a .md with no .pdf sibling is not PDF-backed"
    );
}

#[test]
fn from_app_reads_note_skills_from_the_system_library_read_only() {
    let (mut app, dir) = app_with_system_lib();
    let note_dir = dir.path().join("Skills").join("Note");
    std::fs::create_dir_all(&note_dir).expect("note skills dir");
    std::fs::write(note_dir.join("FormatMarkdown.md"), b"format this note").expect("seed skill");
    open_tabs(&mut app, &["docs/note.md"]);
    select(&mut app, "docs/note.md");

    let vm = CenterPanelViewModel::from_app(&app);
    let items = vm.menu_items_for(0);

    let format_row = items
        .iter()
        .find(|item| item.label == "FormatMarkdown")
        .expect("FormatMarkdown row (UI-033)");
    assert_eq!(
        format_row.message,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: note_dir.join("FormatMarkdown.md"),
            target_dir: None,
            target_file: Some(PathBuf::from("docs/note.md")),
        })
    );
}

#[test]
fn from_app_without_a_note_skill_folder_yields_no_skill_rows() {
    let (mut app, _dir) = app_with_system_lib();
    open_tabs(&mut app, &["docs/note.md"]);
    select(&mut app, "docs/note.md");

    let vm = CenterPanelViewModel::from_app(&app);
    assert!(vm.note_skills.is_empty());
    assert_eq!(vm.menu_items_for(0).len(), 7);
}

#[test]
fn from_app_agent_session_gate_follows_show_results() {
    let (mut app, _dir) = app_with_system_lib();
    assert!(!CenterPanelViewModel::from_app(&app).show_agent_session);

    app.orchestrator.agent_panel_state.show_results = true;
    assert!(CenterPanelViewModel::from_app(&app).show_agent_session);

    app.orchestrator.agent_panel_state.show_results = false;
    assert!(!CenterPanelViewModel::from_app(&app).show_agent_session);
}

#[test]
fn from_app_carries_agent_status_and_thinking() {
    let (mut app, _dir) = app_with_system_lib();
    app.agent_mut().set_status("running tools".to_string());
    app.orchestrator.agent_transcript.thinking = "weighing options".to_string();

    let vm = CenterPanelViewModel::from_app(&app);
    assert_eq!(vm.agent_status, "running tools");
    assert!(!vm.agent_running);
    assert_eq!(vm.thinking, "weighing options");
    assert!(vm.show_thinking());
}

// --- UI-048 thinking gate ----------------------------------------------

#[test]
fn thinking_section_visible_only_when_the_buffer_has_content() {
    assert!(!thinking_section_visible(""));
    assert!(thinking_section_visible("reasoning"));
    // Whitespace is still content.
    assert!(thinking_section_visible(" "));
}

#[test]
fn vm_show_thinking_uses_the_gate() {
    let mut vm = CenterPanelViewModel {
        tabs: Vec::new(),
        active_tab: None,
        show_agent_session: true,
        agent_status: String::new(),
        agent_running: false,
        thinking: String::new(),
        active_pdf_backed: false,
        note_skills: Vec::new(),
        hovered_tab: None,
    };
    assert!(!vm.show_thinking());

    vm.thinking = "step 1".to_string();
    assert!(vm.show_thinking());
}

// --- marker + colour tokens ---------------------------------------------

// --- INT-11: the 6px unsaved dot is a shape, not a glyph ----------------

/// The dot is the spec's `6px` (`size.indicator.dot`) round shape, and the
/// §4.4.1 / §2.4 conflict INT-7c recorded against it is genuinely closed.
///
/// The closure is asserted structurally rather than argued: the marker's
/// diameter is the indicator token, and that diameter sits **below** the
/// `12px` text floor. A value under the floor cannot be a point size at all
/// — which is the point. Under the retired glyph renderer the only way to
/// draw a 6px marker was illegal text; here it is a box, so the floor never
/// applies and no call-site exemption was needed to get the spec's number.
#[test]
fn the_unsaved_dot_is_the_specs_six_pixel_shape_not_a_glyph() {
    use iced_pro::token::typography;

    let dirty = unsaved_dot(true, &Palette::fastmd_dark(), false);

    assert_eq!(
        dirty.diameter,
        size::INDICATOR_DOT,
        "the marker must read its size from `size.indicator.dot`"
    );
    assert_eq!(
        dirty.diameter, 6.0,
        "`size.indicator.dot` is 6px, which is what §4.4.1 draws"
    );
    assert_eq!(
        dirty.radius,
        shape::RADIUS_PILL,
        "a pill radius on a square box is a circle: iced clamps every corner \
         to `min(w, h) / 2` before drawing, so 999 over a 6px box renders a \
         round dot rather than failing to"
    );

    // The load-bearing half: the marker is smaller than the type floor, so
    // it is not in the floor's domain. If anyone routes this back through a
    // `text(..).size(diameter)` call the floor guard's removal from
    // `converted_runtime_sizes` becomes a hole, and this pin is what makes
    // that visible rather than silent.
    assert!(
        dirty.diameter < typography::MIN_SIZE,
        "the dot is {}px against a {}px text floor; it must be painted as a \
         shape, because as type it would be illegal",
        dirty.diameter,
        typography::MIN_SIZE,
    );
}

/// A clean document draws nothing, and a dirty one draws the dot. The slot
/// itself is allocated either way, so a tab's shape never changes as it goes
/// dirty.
#[test]
fn the_unsaved_dot_is_visible_only_for_dirty_documents() {
    let palette = Palette::fastmd_dark();

    assert!(
        unsaved_dot(true, &palette, false).visible,
        "a dirty document must show the marker"
    );
    assert!(
        !unsaved_dot(false, &palette, false).visible,
        "a clean document must not"
    );
    assert_eq!(
        unsaved_dot(false, &palette, false).diameter,
        unsaved_dot(true, &palette, false).diameter,
        "visibility must not change the geometry, or the tab would resize"
    );
}

/// The dot's contrast against the surface it actually sits on, computed from
/// raw sRGB through the crate's solver rather than transcribed.
///
/// **Which floor applies.** The dot is a graphical object whose *shape*
/// carries the state, not a glyph the user reads, so the governing number is
/// `UIF-TOK-006` rule 3's `MIN_CHROME_CONTRAST` (`3.00:1`, WCAG 1.4.11
/// non-text contrast) rather than rule 1's `4.50:1` text floor. Both pairings
/// clear the chrome floor by a wide margin, and the active-tab pairing clears
/// the text floor as well — so the choice of floor does not change the
/// verdict, which is exactly what should be true of a resolved conflict.
///
/// The active-tab figure is not threshold-chosen at all: `palette.accent` *is*
/// `accent.fill`, one of the four solid fills rule 4 locks, so the dot is
/// promoted to `text.strong` by **role**. That is the win INT-7c bought over
/// the retired `status.warning`-on-`accent.fill` pairing (`1.65:1`, effectively
/// invisible on the tab the user is looking at) and this pin keeps the shape
/// change from spending it.
#[test]
fn the_unsaved_dot_clears_the_chrome_floor_on_the_surface_it_actually_sits_on() {
    use iced_pro::token::contrast::{MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, contrast_ratio};

    let pro = Palette::pro();

    // Inactive tab: `tab_fill` paints nothing, so the dot sits on the strip's
    // own `surface.base`.
    let inactive = unsaved_dot(true, &Palette::fastmd_dark(), false);
    assert_eq!(inactive.color, pro.status_warning);
    let on_base = contrast_ratio(inactive.color, pro.surface_base);
    assert!(
        on_base >= MIN_CHROME_CONTRAST,
        "status.warning on surface.base is {on_base:.6}:1, under the \
         {MIN_CHROME_CONTRAST}:1 chrome floor"
    );
    assert!(
        on_base >= MIN_TEXT_CONTRAST,
        "the pairing also clears the text floor at {on_base:.6}:1"
    );

    // Active tab: the dot sits on `accent.fill` and is role-locked.
    let active = unsaved_dot(true, &Palette::fastmd_dark(), true);
    assert_eq!(
        active.color, pro.text_strong,
        "a marker painted inside a locked fill must promote by role"
    );
    let on_accent = contrast_ratio(active.color, pro.accent_fill);
    assert!(
        on_accent >= MIN_CHROME_CONTRAST,
        "text_strong on accent_fill is {on_accent:.6}:1, under the \
         {MIN_CHROME_CONTRAST}:1 chrome floor"
    );
    assert!(
        on_accent >= MIN_TEXT_CONTRAST,
        "the locked pairing also clears the 4.50:1 text floor at \
         {on_accent:.6}:1 — the INT-7c fix must survive the shape change"
    );

    // The retired pairing stays illegal, so the promotion cannot be argued
    // away as unnecessary.
    let retired = contrast_ratio(pro.status_warning, pro.accent_fill);
    assert!(
        retired < 2.0,
        "status.warning over accent_fill should still measure near-invisible, \
         got {retired:.6}:1"
    );
}

/// The dot occupies its slot without moving anything.
///
/// Two independent guarantees, both pinned:
///
/// * **Horizontally** the slot is a fixed [`UNSAVED_MARKER_WIDTH`] whether or
///   not a dot is drawn, and the dot is strictly narrower than it, so the
///   title's x origin cannot depend on the dirty flag.
/// * **Vertically** the dot is smaller than the label's line box, so the
///   marker cell can never become the tallest thing in the label row. Swapping
///   an 18px line-box glyph for a 6px box therefore cannot raise the row, and
///   [`TAB_HEIGHT`] — which is derived from the band, not from its contents —
///   is untouched.
#[test]
fn the_unsaved_dot_occupies_its_slot_without_moving_tab_content() {
    let palette = Palette::fastmd_dark();
    let dirty = unsaved_dot(true, &palette, false);
    let clean = unsaved_dot(false, &palette, false);

    assert!(
        dirty.diameter < UNSAVED_MARKER_WIDTH,
        "a {}px dot in a {}px slot leaves the slot's width to hold the shape; \
         if the dot ever reached the slot width the centring would be load-bearing \
         rather than cosmetic",
        dirty.diameter,
        UNSAVED_MARKER_WIDTH,
    );
    assert_eq!(
        dirty.diameter, clean.diameter,
        "the clean state must reserve the same box, not a smaller one"
    );
    assert!(
        dirty.diameter <= TAB_RUNG.line_height,
        "the dot ({}px) must not exceed the label's {}px line box, or the \
         marker cell would drive the label row's height and shift the title",
        dirty.diameter,
        TAB_RUNG.line_height,
    );
    assert_eq!(
        TAB_HEIGHT,
        STRIP_HEIGHT - STRIP_PADDING - STRIP_PADDING,
        "the band closure is independent of the marker in every posture"
    );
}

// --- INT-11: the 2px top accent indicator -------------------------------

/// The indicator's three §4.4.1 states, all wired as of INT-12.
///
/// INT-11 built two of the three and recorded the third as needing the
/// strip to report the pointer into the reducer. INT-12 built that
/// crossing, so the third state is now reachable and asserted here.
#[test]
fn the_top_indicator_resolves_all_three_states() {
    let pro = Palette::pro();

    assert_eq!(
        tab_top_indicator(true, false),
        Some(pro.accent_border),
        "the active tab's bar is `accent.border`, §4.4.1 slot 1"
    );
    assert_eq!(
        tab_top_indicator(false, false),
        None,
        "an inactive tab at rest must paint nothing — `None`, not a \
         transparent bar, so no partial or leftover indicator is reachable"
    );
    assert_eq!(
        tab_top_indicator(false, true),
        Some(pro.border_strong),
        "a hovered inactive tab takes `border.strong`, §4.4.1 slot 1's \
          third state"
    );
    // Active outranks hover: a hovered active tab must not look like it
    // lost activation.
    assert_eq!(
        tab_top_indicator(true, true),
        tab_top_indicator(true, false),
        "hover must not change an active tab's bar"
    );
    // The hovered bar is a different token from the active bar, so the
    // two states cannot be confused.
    assert_ne!(
        tab_top_indicator(false, true),
        tab_top_indicator(true, false),
        "hovered-inactive and active must be visually distinct"
    );
    assert_eq!(
        TAB_INDICATOR_HEIGHT,
        shape::BORDER_FOCUS,
        "the bar's weight is `border.focus`, the token §2 names against the \
         active tab accent bar"
    );
    assert_eq!(
        TAB_INDICATOR_HEIGHT, 2.0,
        "`border.focus` is 2px, which is what §4.4.1 draws"
    );
}

/// The hovered bar's contrast, recomputed from raw sRGB rather than
/// transcribed from the spec's rounded `3.68:1`.
///
/// §4.4.2 prices the hovered inactive tab's top bar at `border.strong`
/// over the hovered `surface.hover` fill. That pairing is a chrome element
/// (a `2px` rule, not body text), so the floor is `MIN_CHROME_CONTRAST`
/// (`3.0:1`), and the measured figure clears it.
#[test]
fn the_hovered_top_bar_is_legible_against_the_hovered_tab_surface() {
    use iced_pro::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio};

    let pro = Palette::pro();
    let bar = tab_top_indicator(false, true).expect("hovered inactive tab has a bar");
    let hovered_surface = pro.surface_hover;

    let ratio = contrast_ratio(bar, hovered_surface);
    assert!(
        ratio >= MIN_CHROME_CONTRAST,
        "hovered bar measures {ratio:.6}:1 against surface.hover, floor is \
         {MIN_CHROME_CONTRAST}"
    );
    // The spec's rounded figure, matched to five decimal places.
    assert!(
        (ratio - 3.67718).abs() < 1e-5,
        "border.strong on surface.hover measures {ratio:.6}, expected ~3.67718 \
         (the spec prints 3.68:1)"
    );
    // And the resting inactive tab still paints no bar, so the hover is
    // a real change rather than always-on chrome.
    assert_eq!(tab_top_indicator(false, false), None);
}

/// The hover predicate keys on the tab's path, not its position in the
/// strip.
///
/// This is the INT-10 lesson applied to the tab strip. Every operation
/// that changes what index a file sits at — `Close Others`, `Close All`,
/// a middle-click, a watcher-driven deletion — would move an
/// index-keyed highlight onto a tab the pointer never entered.
#[test]
fn tab_hover_is_keyed_by_path_so_a_reorder_cannot_move_it() {
    let a = entry("a.md", false);
    let b = entry("b.md", true);

    assert!(tab_is_hovered(&a, Some(std::path::Path::new("a.md"))));
    assert!(!tab_is_hovered(&b, Some(std::path::Path::new("a.md"))));

    // Close `a`: `b` shifts from index 1 to index 0. A path-keyed
    // highlight on `a` cannot follow it onto `b`.
    let hovered_a = Some(std::path::PathBuf::from("a.md"));
    assert!(!tab_is_hovered(&b, hovered_a.as_deref()));
    // And once `a` is gone the stale highlight matches nothing, so no tab
    // paints a hover it did not earn.
    let tabs = [b.clone()];
    assert!(
        !tabs.iter().any(|t| tab_is_hovered(t, hovered_a.as_deref())),
        "a highlight on a closed tab must match no remaining tab"
    );
    // Nothing is hovered when the state is `None`.
    assert!(!tab_is_hovered(&a, None));
    assert!(!tab_is_hovered(&b, None));
}

/// The 36px strip budget with the 2px indicator inside it, written out.
///
/// The bar is an **overlay**, so the arithmetic that matters is not
/// `36 - 2` at all: the band closure keeps its exact pinned terms and the
/// indicator is priced against the tab's own top padding instead.
/// `TAB_INDICATOR_HEIGHT (2) <= TAB_PADDING_Y (2)` is the whole fit proof —
/// the bar occupies no more than the padding above the content box, so it
/// cannot reach the label and cannot push the band.
#[test]
fn the_top_indicator_fits_the_pinned_strip_budget_without_touching_it() {
    assert_eq!(
        STRIP_HEIGHT,
        STRIP_PADDING + TAB_HEIGHT + STRIP_PADDING,
        "36 = 4 + 28 + 4: the closure INT-7c pinned is unchanged by the \
         indicator, because the bar takes no layout"
    );
    assert_eq!(STRIP_HEIGHT, size::CONTROL_LG);
    assert_eq!(TAB_HEIGHT, size::CONTROL_SM);

    assert!(
        TAB_INDICATOR_HEIGHT <= TAB_PADDING_Y,
        "the bar is {}px and the tab's top padding is {}px; the bar must fit \
         inside that padding so it cannot overlap the label's line box",
        TAB_INDICATOR_HEIGHT,
        TAB_PADDING_Y,
    );

    // The band is still not merely the sum of its contents, which is the pin
    // that stops someone "reclaiming" the slack by shrinking it.
    assert!(
        STRIP_HEIGHT > TAB_INDICATOR_HEIGHT + TAB_TEXT_SIZE + STRIP_PADDING * 2.0,
        "the strip must stay a structural band, not a shrink-wrap around the \
         indicator and the label"
    );
}

#[test]
fn tab_text_color_uses_distinct_tokens_for_active_and_inactive() {
    let palette = Palette::fastmd_dark();
    let active = tab_text_color(&palette, true);
    let inactive = tab_text_color(&palette, false);

    assert_ne!(active, inactive, "the active tab must be visually distinct");
    assert_eq!(active, palette.text_strong);
    assert_eq!(inactive, palette.text_muted);
}

// --- UI-034 cross-menu parity ------------------------------------------
//
// UI-034 requires the tab menu's file actions to be available,
// identically, on a directory-tree file row; the test below asserts the two
// menus actually agree.

#[test]
fn tab_menu_file_actions_match_the_tree_file_menu() {
    use crate::ui_iced::panels::left::file_menu_items;

    let path = Path::new("/lib/notes.md");
    let skills = vec![skill("FormatMarkdown", "sys/Skills/Note/FormatMarkdown.md")];

    let tab_items = tab_context_menu_items(0, path, &skills);
    let tree_items = file_menu_items(path, &skills, false);

    let message_for = |items: &[ContextRow], label: &str| -> Option<Message> {
        items
            .iter()
            .find(|item| item.label == label)
            .map(|item| item.message.clone())
    };

    for label in [
        strings::EDIT_BUTTON,
        strings::COPY_PATH_ACTION,
        strings::SHOW_IN_EXPLORER_ACTION,
    ] {
        let from_tree = message_for(&tree_items, label);
        assert!(
            from_tree.is_some(),
            "UI-034: the tree file menu must offer {label}"
        );
        assert_eq!(
            message_for(&tab_items, label),
            from_tree,
            "UI-034: {label} must publish the same message from the tab menu and the tree menu"
        );
    }

    // The Format Markdown quick task (UI-019 / UI-033) is the same row in
    // both menus, carrying the same skill + target file.
    assert_eq!(
        message_for(&tab_items, "FormatMarkdown"),
        message_for(&tree_items, "FormatMarkdown"),
        "UI-034: the Format Markdown skill row must be identical in both menus"
    );
}

/// The caller-supplied note-skill listing must be used verbatim, so
/// `ViewModels::from_app` can read the skill directory once per rebuild and
/// share it with the left panel instead of each panel hitting the FS.
#[test]
fn from_app_with_note_skills_uses_the_supplied_listing() {
    let app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let supplied = vec![crate::config::SkillFile {
        name: "SuppliedSkill".to_string(),
        path: std::path::PathBuf::from("/skills/SuppliedSkill.md"),
    }];

    let vm = CenterPanelViewModel::from_app_with_note_skills(&app, supplied.clone());

    assert_eq!(
        vm.note_skills, supplied,
        "the supplied listing must land in the snapshot untouched"
    );
}

/// `from_app` and `from_app_with_note_skills` must agree on everything except
/// where the listing came from.
#[test]
fn from_app_and_the_parameterized_form_agree_apart_from_the_listing() {
    let app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let read_by_panel = CenterPanelViewModel::from_app(&app);
    let supplied = vec![crate::config::SkillFile {
        name: "SuppliedSkill".to_string(),
        path: std::path::PathBuf::from("/skills/SuppliedSkill.md"),
    }];
    let injected = CenterPanelViewModel::from_app_with_note_skills(&app, supplied.clone());

    assert_eq!(
        injected.tabs, read_by_panel.tabs,
        "the tab strip must not differ"
    );
    assert_eq!(
        injected.active_tab, read_by_panel.active_tab,
        "the active tab must not differ"
    );
    assert_eq!(
        injected.show_agent_session, read_by_panel.show_agent_session,
        "the agent-session gate must not differ"
    );
    assert_eq!(
        injected.note_skills, supplied,
        "only the listing source differs"
    );
}
