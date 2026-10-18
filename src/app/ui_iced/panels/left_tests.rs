//! Tests for `ui_iced/panels/left.rs` ([TASK-012]).
//!
//! Widget construction cannot be driven headlessly without the iced
//! simulator, so these tests cover the state/mapping layer the widget is
//! built from: the pure width solver, the row labels, the click →
//! `UserCommand` mapping, the context-menu item → command table, and the
//! view-model constructor.
//!
//! [RUST-006]: every filesystem path used here lives inside a
//! `tempfile::TempDir`. In particular every `from_app` call runs against a
//! "System" content library rooted in a temp dir, so the skill-folder
//! lookup never touches the real user profile.

use super::*;
use crate::ui_iced::overlay::menu_labels;
use crate::ui_iced::test_helpers::test_app;
use tempfile::TempDir;

// --- fixtures -----------------------------------------------------------

/// A synthetic collapsed directory row — never opened ([RUST-006]).
fn dir_row(name: &str, path: &str, depth: usize) -> FlatRow {
    FlatRow {
        depth,
        name: name.to_string(),
        path: PathBuf::from(path),
        is_dir: true,
        is_expanded: false,
    }
}

/// A synthetic expanded directory row.
fn expanded_dir_row(name: &str, path: &str, depth: usize) -> FlatRow {
    FlatRow {
        is_expanded: true,
        ..dir_row(name, path, depth)
    }
}

/// A synthetic file row.
fn file_row(name: &str, path: &str, depth: usize) -> FlatRow {
    FlatRow {
        depth,
        name: name.to_string(),
        path: PathBuf::from(path),
        is_dir: false,
        is_expanded: false,
    }
}

/// A deterministic stand-in for font metrics: 8 logical points per char.
///
/// The view cannot measure real glyphs (see the module header), so the
/// tests inject this and pin the *formula* around the measurement.
fn measure(label: &str) -> f32 {
    label.chars().count() as f32 * 8.0
}

/// A test app whose "System" content library is rooted in a temp dir, so
/// `from_app`'s skill lookup stays off the real user profile ([RUST-006]).
fn app_with_system_lib() -> (FastMdApp, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = test_app();
    app.content_libraries_mut().push(ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: SYSTEM_LIBRARY_NAME.to_string(),
        kind: "system".to_string(),
        readonly: false,
        priority: 0,
    });
    (app, dir)
}

/// Replaces the multi-selection with `paths`.
fn select_files(app: &mut FastMdApp, paths: &[&str]) {
    let selection = app.selection_mut().selected_files_mut();
    selection.clear();
    selection.extend(paths.iter().map(|path| PathBuf::from(path)));
}

/// Seeds one folder skill and one note skill under the system library root.
fn seed_skills(root: &Path) {
    let folder = root.join(SKILLS_SUBDIR).join(FOLDER_SKILLS_SUBDIR);
    let note = root.join(SKILLS_SUBDIR).join(NOTE_SKILLS_SUBDIR);
    std::fs::create_dir_all(&folder).expect("folder skills dir");
    std::fs::create_dir_all(&note).expect("note skills dir");
    std::fs::write(folder.join("tidy.md"), b"tidy this folder").expect("folder skill");
    std::fs::write(note.join("summarize.md"), b"summarize this note").expect("note skill");
}

// --- row labels ---------------------------------------------------------

#[test]
fn row_icon_uses_caret_state_for_directories_and_blank_slot_for_files() {
    assert_eq!(row_icon(true, false), strings::ICON_CARET_RIGHT);
    assert_eq!(row_icon(true, true), strings::ICON_CARET_DOWN);
    assert_eq!(row_icon(false, false), FILE_ROW_ICON);
    // A file is never expanded; the blank slot must win regardless.
    assert_eq!(row_icon(false, true), FILE_ROW_ICON);
}

#[test]
fn row_label_reproduces_the_egui_icon_space_name_format() {
    assert_eq!(
        row_label(&dir_row("Work", "/lib/Work", 0)),
        format!("{} Work", strings::ICON_CARET_RIGHT)
    );
    assert_eq!(
        row_label(&expanded_dir_row("Work", "/lib/Work", 0)),
        format!("{} Work", strings::ICON_CARET_DOWN)
    );
    assert_eq!(
        row_label(&file_row("note.md", "/lib/note.md", 0)),
        "  note.md"
    );
}

// --- UI-016 width solver ------------------------------------------------

#[test]
fn row_width_is_indent_plus_measured_label_plus_slack() {
    let row = file_row("note.md", "/lib/note.md", 2);
    let label = row_label(&row);
    assert_eq!(
        row_width(&row, measure),
        2.0 * ROW_INDENT_PER_DEPTH + measure(&label) + ROW_WIDTH_SLACK
    );
}

#[test]
fn row_indent_clamps_at_the_maximum_depth() {
    let at_limit = file_row("n", "/lib/n", MAX_ROW_INDENT_DEPTH);
    let beyond = file_row("n", "/lib/n", MAX_ROW_INDENT_DEPTH + 40);
    assert_eq!(
        row_width(&at_limit, measure),
        row_width(&beyond, measure),
        "deeper rows must not indent further than MAX_ROW_INDENT_DEPTH"
    );
}

#[test]
fn longest_row_width_is_the_maximum_and_zero_when_empty() {
    let rows = vec![
        file_row("a.md", "/lib/a.md", 0),
        file_row("longer-name.md", "/lib/longer-name.md", 0),
        file_row("bb.md", "/lib/bb.md", 0),
    ];
    let longest = longest_row_width(&rows, measure);
    assert_eq!(longest, row_width(&rows[1], measure));
    assert_eq!(longest_row_width(&[], measure), 0.0);
}

#[test]
fn auto_fit_caps_at_twenty_percent_of_the_viewport() {
    let rows = vec![file_row(&"a".repeat(500), "/lib/long.md", 0)];
    let viewport = 2000.0;
    assert_eq!(
        auto_fit_width(&rows, viewport, measure),
        viewport * LEFT_PANEL_MAX_WIDTH_FRACTION,
        "UI-016: the left panel may never exceed 20 % of the viewport"
    );
}

#[test]
fn auto_fit_floors_at_the_minimum_panel_width() {
    let rows = vec![file_row("a.md", "/lib/a.md", 0)];
    let viewport = 4000.0;
    assert_eq!(
        auto_fit_width(&rows, viewport, measure),
        LEFT_PANEL_MIN_WIDTH
    );
}

#[test]
fn auto_fit_minimum_wins_when_the_cap_falls_below_it() {
    // 500 px viewport → 20 % cap is 100 px, below the 180 px minimum.
    let rows = vec![file_row("a.md", "/lib/a.md", 0)];
    assert!(
        500.0 * LEFT_PANEL_MAX_WIDTH_FRACTION < LEFT_PANEL_MIN_WIDTH,
        "fixture must actually put the cap below the minimum"
    );
    assert_eq!(auto_fit_width(&rows, 500.0, measure), LEFT_PANEL_MIN_WIDTH);
}

#[test]
fn auto_fit_tracks_the_longest_row_inside_the_band() {
    // 10 000 px viewport → 2 000 px cap, so the raw longest row fits.
    let rows = vec![
        file_row("a.md", "/lib/a.md", 0),
        file_row(&"b".repeat(120), "/lib/wide.md", 3),
    ];
    assert_eq!(
        auto_fit_width(&rows, 10_000.0, measure),
        row_width(&rows[1], measure)
    );
}

#[test]
fn panel_width_falls_back_to_the_default_and_respects_the_band() {
    assert_eq!(panel_width(None, 4000.0), LEFT_PANEL_DEFAULT_WIDTH);
    assert_eq!(panel_width(Some(240.0), 4000.0), 240.0);
    assert_eq!(
        panel_width(Some(5000.0), 2000.0),
        2000.0 * LEFT_PANEL_MAX_WIDTH_FRACTION
    );
    assert_eq!(panel_width(Some(10.0), 4000.0), LEFT_PANEL_MIN_WIDTH);
}

// --- selection --------------------------------------------------------

#[test]
fn directory_rows_are_never_highlighted() {
    let row = dir_row("Work", "/lib/Work", 0);
    let selected = vec![PathBuf::from("/lib/Work")];
    assert!(!is_row_selected(
        &row,
        Some(Path::new("/lib/Work")),
        &selected
    ));
}

#[test]
fn file_row_is_selected_by_either_the_single_or_the_multi_selection() {
    let row = file_row("note.md", "/lib/note.md", 1);
    let other = file_row("other.md", "/lib/other.md", 1);
    let multi = vec![PathBuf::from("/lib/note.md"), PathBuf::from("/lib/x.md")];

    assert!(is_row_selected(&row, Some(Path::new("/lib/note.md")), &[]));
    assert!(is_row_selected(&row, None, &multi));
    assert!(!is_row_selected(
        &other,
        Some(Path::new("/lib/note.md")),
        &multi
    ));
}

// --- click mapping ----------------------------------------------------

#[test]
fn row_click_toggles_directory_expansion_and_selects_files() {
    assert_eq!(
        row_click_command(&dir_row("Work", "/lib/Work", 0), false),
        UserCommand::SelectDirectory {
            path: PathBuf::from("/lib/Work"),
            toggle_expand: true,
        }
    );
    assert_eq!(
        row_click_command(&file_row("note.md", "/lib/note.md", 0), false),
        UserCommand::SelectFile {
            path: PathBuf::from("/lib/note.md"),
            multi: false,
        }
    );
    assert_eq!(
        row_click_command(&file_row("note.md", "/lib/note.md", 0), true),
        UserCommand::SelectFile {
            path: PathBuf::from("/lib/note.md"),
            multi: true,
        }
    );
}

#[test]
fn double_click_opens_files_and_is_a_no_op_on_directories() {
    assert_eq!(
        row_double_click_command(&file_row("note.md", "/lib/note.md", 0)),
        Some(UserCommand::OpenInEditor(PathBuf::from("/lib/note.md")))
    );
    assert_eq!(
        row_double_click_command(&expanded_dir_row("Work", "/lib/Work", 0)),
        None,
        "render-audit P1-9: a directory double-click must not toggle twice"
    );
}

// --- context menus ----------------------------------------------------

#[test]
fn directory_menu_maps_every_row_to_its_user_command() {
    let items = directory_menu_items(Path::new("/lib/Work"), &[]);
    assert_eq!(
        menu_labels(&items),
        vec![
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::COPY_PATH_ACTION,
            strings::RENAME_ACTION,
            strings::MOVE_ACTION,
            strings::CREATE_DIRECTORY_ACTION,
            strings::NEW_DOCUMENT_ACTION,
            strings::DELETE_ACTION,
        ]
    );
    assert_eq!(
        items[0].message,
        Message::Command(UserCommand::ShowInExplorer(PathBuf::from("/lib/Work")))
    );
    assert_eq!(
        items[4].message,
        Message::Command(UserCommand::CreateDirectory {
            parent: PathBuf::from("/lib/Work")
        })
    );
    assert_eq!(
        items[5].message,
        Message::Command(UserCommand::CreateDocument {
            parent: PathBuf::from("/lib/Work")
        })
    );
    assert_eq!(
        items[6].message,
        Message::Command(UserCommand::Delete(PathBuf::from("/lib/Work")))
    );
}

#[test]
fn folder_skills_run_against_the_directory_they_were_opened_on() {
    let skills = vec![SkillFile {
        name: "tidy".to_string(),
        path: PathBuf::from("/sys/Skills/Folder/tidy.md"),
    }];
    let items = directory_menu_items(Path::new("/lib/Work"), &skills);

    assert_eq!(items.len(), 8, "seven actions plus the folder skill");
    assert_eq!(items[7].label, "tidy");
    assert_eq!(
        items[7].message,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("/sys/Skills/Folder/tidy.md"),
            target_dir: Some(PathBuf::from("/lib/Work")),
            target_file: Some(PathBuf::from("/lib/Work")),
        })
    );
}

#[test]
fn file_menu_maps_every_row_and_gates_save_as_pdf_on_typst() {
    let without_typst = file_menu_items(Path::new("/lib/note.md"), &[], false);
    assert_eq!(
        menu_labels(&without_typst),
        vec![
            strings::EDIT_BUTTON,
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::COPY_PATH_ACTION,
            strings::RUN_AS_PROMPT_ACTION,
            strings::PRINT_ACTION,
            strings::RENAME_ACTION,
            strings::MOVE_ACTION,
            strings::DELETE_ACTION,
        ]
    );
    assert_eq!(
        without_typst[0].message,
        Message::Command(UserCommand::OpenInEditor(PathBuf::from("/lib/note.md")))
    );
    assert_eq!(
        without_typst[3].message,
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: PathBuf::from("/lib/note.md")
        })
    );
    assert_eq!(
        without_typst[4].message,
        Message::Tree(TreeMessage::PrintRequested {
            path: PathBuf::from("/lib/note.md")
        })
    );

    let with_typst = file_menu_items(Path::new("/lib/note.md"), &[], true);
    assert_eq!(
        menu_labels(&with_typst)[5],
        strings::SAVE_AS_PDF_ACTION,
        "Save as PDF sits right after Print, as in the egui menu"
    );
    assert_eq!(
        with_typst[5].message,
        Message::Command(UserCommand::SaveAsPdf(PathBuf::from("/lib/note.md")))
    );
}

#[test]
fn note_skills_run_against_the_file_only() {
    let skills = vec![SkillFile {
        name: "summarize".to_string(),
        path: PathBuf::from("/sys/Skills/Note/summarize.md"),
    }];
    let items = file_menu_items(Path::new("/lib/note.md"), &skills, false);

    assert_eq!(
        items.last().map(|item| item.label.as_str()),
        Some("summarize")
    );
    assert_eq!(
        items.last().map(|item| item.message.clone()),
        Some(Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("/sys/Skills/Note/summarize.md"),
            target_dir: None,
            target_file: Some(PathBuf::from("/lib/note.md")),
        }))
    );
}

#[test]
fn multi_select_menu_carries_the_whole_selection() {
    let paths = vec![PathBuf::from("/lib/a.md"), PathBuf::from("/lib/b.md")];
    let items = multi_select_menu_items(&paths);

    assert_eq!(
        menu_labels(&items),
        vec![strings::MERGE_ACTION, strings::DELETE_ACTION]
    );
    assert_eq!(
        items[0].message,
        Message::Command(UserCommand::MergePrompt(paths.clone()))
    );
    assert_eq!(
        items[1].message,
        Message::Tree(TreeMessage::DeleteSelectedRequested { paths })
    );
}

#[test]
fn context_menu_chooses_the_multi_menu_only_for_a_selected_file_in_a_multi_selection() {
    let context = MenuContext::default();
    let selected = vec![PathBuf::from("/lib/a.md"), PathBuf::from("/lib/b.md")];
    let multi_row = file_row("a.md", "/lib/a.md", 0);
    let plain_row = file_row("z.md", "/lib/z.md", 0);
    let dir = dir_row("Work", "/lib/Work", 0);

    assert_eq!(
        context_menu_items(&multi_row, &selected, context),
        multi_select_menu_items(&selected)
    );
    assert_eq!(
        context_menu_items(&plain_row, &selected, context),
        file_menu_items(Path::new("/lib/z.md"), &[], false)
    );
    // A directory always gets the directory menu, whatever is selected.
    assert_eq!(
        context_menu_items(&dir, &selected, context),
        directory_menu_items(Path::new("/lib/Work"), &[])
    );
}

#[test]
fn a_single_selection_still_gets_the_full_file_menu() {
    let row = file_row("a.md", "/lib/a.md", 0);
    let items = context_menu_items(&row, &[PathBuf::from("/lib/a.md")], MenuContext::default());
    assert_eq!(
        menu_labels(&items),
        menu_labels(&file_menu_items(Path::new("/lib/a.md"), &[], false))
    );
}

// --- skill directory resolution ---------------------------------------

#[test]
fn skill_dir_prefers_the_system_library_root() {
    let libraries = vec![
        ContentLibrary {
            root_folder: "D:/other".to_string(),
            name: "Other".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        },
        ContentLibrary {
            root_folder: "D:/system".to_string(),
            name: SYSTEM_LIBRARY_NAME.to_string(),
            kind: "system".to_string(),
            readonly: false,
            priority: 0,
        },
    ];
    assert_eq!(
        skill_dir(&libraries, FOLDER_SKILLS_SUBDIR, || PathBuf::from("unused")),
        PathBuf::from("D:/system")
            .join(SKILLS_SUBDIR)
            .join(FOLDER_SKILLS_SUBDIR)
    );
    assert_eq!(
        skill_dir(&libraries, NOTE_SKILLS_SUBDIR, || PathBuf::from("unused")),
        PathBuf::from("D:/system")
            .join(SKILLS_SUBDIR)
            .join(NOTE_SKILLS_SUBDIR)
    );
}

#[test]
fn skill_dir_falls_back_when_no_system_library_is_configured() {
    let libraries = vec![ContentLibrary {
        root_folder: "D:/other".to_string(),
        name: "Other".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    assert_eq!(
        skill_dir(&libraries, FOLDER_SKILLS_SUBDIR, || PathBuf::from(
            "fallback"
        )),
        PathBuf::from("fallback")
    );
    assert_eq!(
        skill_dir(&[], NOTE_SKILLS_SUBDIR, || PathBuf::from("fallback")),
        PathBuf::from("fallback")
    );
}

// --- view model: tree rows (UI-017 / UI-018) --------------------------

#[test]
fn tree_rows_keep_directories_before_files_at_every_level() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let sub = lib_root.join("middle");

    app.file_processor_mut().all_files = vec![
        lib_root.join("aardvark.md"),
        lib_root.join("zebra.md"),
        sub.join("inner.md"),
    ];
    app.file_processor_mut().all_dirs = vec![sub.clone()];
    app.selection_mut().expanded_dirs.insert(lib_root.clone());
    app.selection_mut().expanded_dirs.insert(sub.clone());

    let rows = tree_rows(&app);
    let paths: Vec<PathBuf> = rows.iter().map(|row| row.path.clone()).collect();

    assert_eq!(
        paths,
        vec![
            lib_root.clone(),
            sub.clone(),
            sub.join("inner.md"),
            lib_root.join("aardvark.md"),
            lib_root.join("zebra.md"),
        ],
        "UI-017: the reused flatten_tree order (dirs first, then name-ascending) must survive verbatim"
    );
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[2].depth, 2);
    assert_eq!(rows[3].depth, 1);
}

#[test]
fn collapsed_directories_hide_their_children() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let sub = lib_root.join("middle");

    app.file_processor_mut().all_files = vec![lib_root.join("top.md"), sub.join("inner.md")];
    app.file_processor_mut().all_dirs = vec![sub.clone()];
    // The library is expanded but `middle` is not.
    app.selection_mut().expanded_dirs.insert(lib_root.clone());

    let rows = tree_rows(&app);
    assert!(!rows.iter().any(|row| row.path == sub.join("inner.md")));
    assert!(rows.iter().any(|row| row.path == sub));
}

#[test]
fn tag_filter_prunes_directories_without_matching_files() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let matching = lib_root.join("matching_folder");
    let orphan = lib_root.join("non_matching_folder");

    let match_file = matching.join("keep.md");
    let other_file = orphan.join("drop.md");
    app.file_processor_mut().all_files = vec![match_file.clone(), other_file.clone()];
    app.file_processor_mut().all_dirs = vec![matching.clone(), orphan.clone()];
    app.tags_mut()
        .add_tags(match_file.clone(), vec!["keep".to_string()]);
    app.tags_mut()
        .add_tags(other_file.clone(), vec!["drop".to_string()]);
    app.tags_mut().selected_tag = Some("keep".to_string());
    app.selection_mut().expanded_dirs.insert(lib_root.clone());
    app.selection_mut().expanded_dirs.insert(matching.clone());

    let rows = tree_rows(&app);
    let paths: Vec<PathBuf> = rows.iter().map(|row| row.path.clone()).collect();

    assert!(paths.contains(&matching), "UI-018: matching dir stays");
    assert!(
        !paths.contains(&orphan),
        "UI-018: a directory with no file carrying the active tag is pruned, got {paths:?}"
    );
    assert_eq!(
        paths,
        vec![lib_root.clone(), matching.clone(), match_file.clone()]
    );
}

#[test]
fn tree_rows_prefer_the_cache_while_the_tree_is_clean() {
    let (mut app, _dir) = app_with_system_lib();
    let cached = Arc::new(vec![file_row("cached.md", "/lib/cached.md", 0)]);
    app.cached_tree_rows = Some(Arc::clone(&cached));
    *app.selection_mut().tree_dirty_mut() = false;

    assert_eq!(tree_rows(&app), cached);

    *app.selection_mut().tree_dirty_mut() = true;
    let rebuilt = tree_rows(&app);
    assert_ne!(
        rebuilt, cached,
        "a dirty tree must not be served from the stale cache"
    );
}

#[test]
fn tree_rows_never_write_the_cache_back() {
    let (mut app, _dir) = app_with_system_lib();
    *app.selection_mut().tree_dirty_mut() = true;
    let _ = tree_rows(&app);
    assert!(
        app.cached_tree_rows.is_none(),
        "the pure view must not mutate app state ([FMD-065])"
    );
}

// --- view model: constructor ------------------------------------------

#[test]
fn from_app_snapshots_an_empty_workspace() {
    let (app, _dir) = app_with_system_lib();
    let view_model = LeftPanelViewModel::from_app(&app);

    // The only row is the System library root itself — no files were
    // discovered, so nothing hangs beneath it.
    assert_eq!(view_model.rows.len(), 1);
    assert!(view_model.rows[0].is_dir);
    assert_eq!(view_model.query, "");
    assert!(!view_model.is_searching);
    assert!(view_model.results.is_empty());
    assert_eq!(view_model.selected_file, None);
    assert!(view_model.selected_files.is_empty());
    assert!(view_model.pdf_backed.is_empty());
    assert!(
        !view_model.multi_select,
        "modifier tracking is [TASK-022] work, so the snapshot starts inert"
    );
}

#[test]
fn from_app_carries_query_search_state_and_selection() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let note = lib_root.join("note.md");
    std::fs::write(&note, b"# Notes\ncrisp apples are great.\n").unwrap();
    app.file_processor_mut().all_files = vec![note.clone()];
    app.selection_mut().expanded_dirs.insert(lib_root.clone());

    *app.search_mut().query_mut() = "crisp".to_string();
    app.search_mut().apply(&[note.clone()], &[]);
    app.selection_mut().select_file(note.clone());

    let view_model = LeftPanelViewModel::from_app(&app);
    assert_eq!(view_model.query, "crisp");
    assert!(view_model.is_searching);
    assert_eq!(view_model.selected_file, Some(note.clone()));
    assert_eq!(
        view_model
            .results
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>(),
        vec![note]
    );
}

#[test]
fn from_app_sorts_the_multi_selection_so_emitted_commands_are_deterministic() {
    let (mut app, _dir) = app_with_system_lib();
    select_files(&mut app, &["/lib/z.md", "/lib/a.md", "/lib/m.md"]);

    let view_model = LeftPanelViewModel::from_app(&app);
    assert_eq!(
        view_model.selected_files,
        vec![
            PathBuf::from("/lib/a.md"),
            PathBuf::from("/lib/m.md"),
            PathBuf::from("/lib/z.md"),
        ]
    );
}

#[test]
fn from_app_marks_only_pdf_backed_visible_files() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let backed = lib_root.join("backed.md");
    let plain = lib_root.join("plain.md");
    std::fs::write(&backed, b"# backed").unwrap();
    std::fs::write(backed.with_extension("pdf"), b"%PDF-1.4").unwrap();
    std::fs::write(&plain, b"# plain").unwrap();

    app.file_processor_mut().all_files = vec![backed.clone(), plain.clone()];
    app.selection_mut().expanded_dirs.insert(lib_root.clone());
    app.pdf_backing_tracker()
        .process_discovered(&[backed.clone(), plain.clone()]);

    let view_model = LeftPanelViewModel::from_app(&app);
    assert!(view_model.pdf_backed.contains(&backed));
    assert!(
        !view_model.pdf_backed.contains(&plain),
        "a .md with no .pdf sibling must not be tinted"
    );
}

#[test]
fn from_app_lists_the_seeded_folder_and_note_skills() {
    let (app, dir) = app_with_system_lib();
    seed_skills(dir.path());

    let view_model = LeftPanelViewModel::from_app(&app);
    assert_eq!(
        view_model
            .folder_skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["tidy"]
    );
    assert_eq!(
        view_model
            .note_skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["summarize"]
    );
}

#[test]
fn menu_items_for_uses_the_view_model_skill_lists() {
    let (app, dir) = app_with_system_lib();
    seed_skills(dir.path());
    let view_model = LeftPanelViewModel::from_app(&app);

    let dir_items = view_model.menu_items_for(&dir_row("Work", "/lib/Work", 0));
    assert_eq!(
        dir_items.last().map(|item| item.label.as_str()),
        Some("tidy")
    );

    let file_items = view_model.menu_items_for(&file_row("note.md", "/lib/note.md", 0));
    assert_eq!(
        file_items.last().map(|item| item.label.as_str()),
        Some("summarize")
    );
}

#[test]
fn is_multi_selected_requires_more_than_one_file() {
    let (mut app, _dir) = app_with_system_lib();
    let one = PathBuf::from("/lib/a.md");
    select_files(&mut app, &["/lib/a.md"]);
    let view_model = LeftPanelViewModel::from_app(&app);
    assert!(
        !view_model.is_multi_selected(&one),
        "one selected file is a single selection"
    );

    let two = PathBuf::from("/lib/b.md");
    select_files(&mut app, &["/lib/a.md", "/lib/b.md"]);
    let view_model = LeftPanelViewModel::from_app(&app);
    assert!(view_model.is_multi_selected(&one));
    assert!(view_model.is_multi_selected(&two));
    assert!(
        !view_model.is_multi_selected(Path::new("/lib/c.md")),
        "a path outside the selection never gets the multi menu"
    );
}

#[test]
fn view_model_auto_fit_delegates_to_the_pure_solver() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    app.file_processor_mut().all_files = vec![lib_root.join("note.md")];
    app.selection_mut().expanded_dirs.insert(lib_root.clone());

    let view_model = LeftPanelViewModel::from_app(&app);
    assert_eq!(
        view_model.auto_fit(10_000.0, measure),
        auto_fit_width(&view_model.rows, 10_000.0, measure)
    );
}

// --- colour tokens ----------------------------------------------------

#[test]
fn pdf_backed_rows_use_the_error_token_and_others_the_body_token() {
    let palette = Palette::fastmd_dark();
    assert_eq!(row_text_color(&palette, true), to_iced_color(palette.error));
    assert_eq!(row_text_color(&palette, false), to_iced_color(palette.text));
    assert_ne!(
        row_text_color(&palette, true),
        row_text_color(&palette, false)
    );
}
