//! Tests for `ui_iced/panels/center.rs` ([TASK-013]).
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

use super::*;
use crate::config::ContentLibrary;
use crate::ui_iced::overlay::menu_labels;
use crate::ui_iced::test_helpers::test_app;
use tempfile::TempDir;

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
fn message_for(items: &[MenuItem<Message>], label: &str) -> Message {
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
    // The shipped the legacy UI build has no per-tab dirty flag, so the pure
    // constructor reports clean. The field exists for TASK-022 to fill.
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
    // legacy parity: the tab strip never multi-selects.
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
fn tab_context_menu_labels_are_in_thelegacy_order() {
    let items = tab_context_menu_items(1, Path::new("lib/note.md"), &[]);
    assert_eq!(
        menu_labels(&items),
        vec![
            strings::EDIT_BUTTON,
            strings::CLOSE_TAB_MENU,
            strings::CLOSE_OTHERS_MENU,
            strings::CLOSE_ALL_TABS_MENU,
            strings::COPY_PATH_ACTION,
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::OPEN_IN_EDITOR_ACTION,
        ]
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
fn tab_context_menu_skill_rows_come_after_the_fixed_rows() {
    let skills = vec![skill("FormatMarkdown", "sys/FormatMarkdown.md")];
    let items = tab_context_menu_items(0, Path::new("note.md"), &skills);
    assert_eq!(
        items.last().map(|item| item.label.as_str()),
        Some("FormatMarkdown")
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
    // Whitespace is still content — the legacy UI's `!is_empty()` said the same.
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
    };
    assert!(!vm.show_thinking());

    vm.thinking = "step 1".to_string();
    assert!(vm.show_thinking());
}

// --- marker + colour tokens ---------------------------------------------

#[test]
fn unsaved_marker_is_the_dot_only_for_dirty_documents() {
    assert_eq!(unsaved_marker(true), strings::ICON_UNSAVED_DOT);
    assert_eq!(unsaved_marker(false), "");
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
// Gap fill ([TASK-027]): UI-034 requires the tab menu's file actions to be
// available, identically, on a directory-tree file row. Each menu had its
// own mapping test; nothing asserted the two menus actually agree.

#[test]
fn tab_menu_file_actions_match_the_tree_file_menu() {
    use crate::ui_iced::panels::left::file_menu_items;

    let path = Path::new("/lib/notes.md");
    let skills = vec![skill("FormatMarkdown", "sys/Skills/Note/FormatMarkdown.md")];

    let tab_items = tab_context_menu_items(0, path, &skills);
    let tree_items = file_menu_items(path, &skills, false);

    let message_for = |items: &[MenuItem<Message>], label: &str| -> Option<Message> {
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
/// share it with the left panel instead of each panel hitting the FS
/// ([FMD-060]).
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
