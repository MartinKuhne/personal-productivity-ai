//! Tests for `ui_iced/panels/left.rs`.
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

// Several assertions in the INT-7b section target compile-time token values
// on purpose: pinning a constant is the whole point, so a retuned rung must
// fail the suite rather than pass silently. `clippy::assertions_on_constants`
// cannot tell a spec pin from a tautology (same treatment as
// `src/iced-pro/token/*_tests.rs`).
#![allow(clippy::assertions_on_constants)]

use super::*;
use crate::ui_iced::context_menu::{ContextGroup, ContextRow};
use crate::ui_iced::test_helpers::test_app;
use iced_pro::token::contrast::meets_text_contrast;
use tempfile::TempDir;

// --- fixtures -----------------------------------------------------------

/// A row width wide enough that no depth under test compresses.
///
/// The deepest nominal indent exercised here is `50 × 16 = 800px` and
/// `indent_allowance(2000) = 2000 − 128 = 1872`, so every depth in the
/// nominal-tier assertions fits with room to spare. Tests that want the
/// *compressed* tiers pass a real pane width instead.
const WIDE_ROW: f32 = 2_000.0;

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
    selection.extend(paths.iter().map(|path| PathBuf::from(path.to_string())));
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
fn row_label_uses_the_icon_space_name_format() {
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
    const { assert!(LEFT_PANEL_MAX_WIDTH_FRACTION * 500.0 < LEFT_PANEL_MIN_WIDTH) } // fixture sanity
    let rows = vec![file_row("a.md", "/lib/a.md", 0)];
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

/// The visible labels of a context-row list, in render order.
fn row_labels(rows: &[ContextRow]) -> Vec<&str> {
    rows.iter().map(|item| item.label.as_str()).collect()
}

#[test]
fn directory_menu_maps_every_row_to_its_user_command() {
    let items = directory_menu_items(Path::new("/lib/Work"), &[]);
    assert_eq!(
        row_labels(&items),
        vec![
            strings::NEW_DOCUMENT_ACTION,
            strings::CREATE_DIRECTORY_ACTION,
            strings::RENAME_ACTION,
            strings::MOVE_ACTION,
            strings::COPY_PATH_ACTION,
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::DELETE_ACTION,
        ],
        "INT-3b reordered to §4.9.1's Creation | Folder Automation & \
         Management | Destructive runs; no label changed"
    );
    assert_eq!(
        items[0].message,
        Message::Command(UserCommand::CreateDocument {
            parent: PathBuf::from("/lib/Work")
        })
    );
    assert_eq!(
        items[1].message,
        Message::Command(UserCommand::CreateDirectory {
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
    assert_eq!(items[2].label, "tidy");
    assert_eq!(
        items[2].message,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("/sys/Skills/Folder/tidy.md"),
            target_dir: Some(PathBuf::from("/lib/Work")),
            target_file: Some(PathBuf::from("/lib/Work")),
        })
    );
    assert_eq!(
        items[2].group,
        ContextGroup::Automation,
        "contextual skills belong to the automation run (§4.9.1)"
    );
}

#[test]
fn file_menu_maps_every_row_and_gates_save_as_pdf_on_typst() {
    let without_typst = file_menu_items(Path::new("/lib/note.md"), &[], false);
    assert_eq!(
        row_labels(&without_typst),
        vec![
            strings::EDIT_BUTTON,
            strings::PRINT_ACTION,
            strings::RUN_AS_PROMPT_ACTION,
            strings::RENAME_ACTION,
            strings::MOVE_ACTION,
            strings::COPY_PATH_ACTION,
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::DELETE_ACTION,
        ],
        "Open & Output | AI & Automation | File Management | Destructive"
    );
    assert_eq!(
        without_typst[0].message,
        Message::Command(UserCommand::OpenInEditor(PathBuf::from("/lib/note.md")))
    );
    assert_eq!(
        without_typst[1].message,
        Message::Tree(TreeMessage::PrintRequested {
            path: PathBuf::from("/lib/note.md")
        })
    );
    assert_eq!(
        without_typst[2].message,
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: PathBuf::from("/lib/note.md")
        })
    );

    let with_typst = file_menu_items(Path::new("/lib/note.md"), &[], true);
    assert_eq!(
        row_labels(&with_typst)[2],
        strings::SAVE_AS_PDF_ACTION,
        "Save as PDF sits right after Print"
    );
    assert_eq!(
        with_typst[2].message,
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

    assert_eq!(items[3].label, "summarize");
    assert_eq!(
        items[3].message,
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("/sys/Skills/Note/summarize.md"),
            target_dir: None,
            target_file: Some(PathBuf::from("/lib/note.md")),
        })
    );
    assert_eq!(items[3].group, ContextGroup::Automation);
}

#[test]
fn multi_select_menu_carries_the_whole_selection() {
    let paths = vec![PathBuf::from("/lib/a.md"), PathBuf::from("/lib/b.md")];
    let items = multi_select_menu_items(&paths);

    assert_eq!(
        row_labels(&items),
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
        row_labels(&items),
        row_labels(&file_menu_items(Path::new("/lib/a.md"), &[], false))
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

    app.workspace_mut().all_files = vec![
        lib_root.join("aardvark.md"),
        lib_root.join("zebra.md"),
        sub.join("inner.md"),
    ];
    app.workspace_mut().all_dirs = vec![sub.clone()];
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

    app.workspace_mut().all_files = vec![lib_root.join("top.md"), sub.join("inner.md")];
    app.workspace_mut().all_dirs = vec![sub.clone()];
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
    app.workspace_mut().all_files = vec![match_file.clone(), other_file.clone()];
    app.workspace_mut().all_dirs = vec![matching.clone(), orphan.clone()];
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
        "the pure view must not mutate app state"
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
        "the snapshot starts with no modifier tracked"
    );
}

#[test]
fn from_app_carries_query_search_state_and_selection() {
    let (mut app, dir) = app_with_system_lib();
    let lib_root = dir.path().to_path_buf();
    let note = lib_root.join("note.md");
    std::fs::write(&note, b"# Notes\ncrisp apples are great.\n").unwrap();
    app.workspace_mut().all_files = vec![note.clone()];
    app.selection_mut().expanded_dirs.insert(lib_root.clone());

    *app.search_mut().query_mut() = "crisp".to_string();
    app.search_mut().apply(std::slice::from_ref(&note), &[]);
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

    app.workspace_mut().all_files = vec![backed.clone(), plain.clone()];
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
    assert!(
        dir_items
            .iter()
            .any(|item| item.label == "tidy" && item.group == ContextGroup::Automation),
        "the folder skill comes from the view model, not a re-read of disk: {:?}",
        row_labels(&dir_items)
    );
    assert_eq!(
        dir_items.last().map(|item| item.label.as_str()),
        Some(strings::DELETE_ACTION),
        "§4.9.1 puts the destructive run last, so skills are no longer trailing"
    );

    let file_items = view_model.menu_items_for(&file_row("note.md", "/lib/note.md", 0));
    assert!(
        file_items
            .iter()
            .any(|item| item.label == "summarize" && item.group == ContextGroup::Automation),
        "the note skill comes from the view model: {:?}",
        row_labels(&file_items)
    );
    assert_eq!(
        file_items.last().map(|item| item.label.as_str()),
        Some(strings::DELETE_ACTION)
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
    app.workspace_mut().all_files = vec![lib_root.join("note.md")];
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
    assert_eq!(row_text_color(&palette, true), palette.error);
    assert_eq!(row_text_color(&palette, false), palette.text);
    assert_ne!(
        row_text_color(&palette, true),
        row_text_color(&palette, false)
    );
}

// --- INT-7b: framework token bindings, the 12px floor, row geometry ---
//
// These pins are deliberately written against the **framework** constants
// rather than against numbers, plus an `assert_ne!` against each literal
// this increment retired (INT-7a precedent). A future edit that restores
// a retired value fails a test that names the value, not a diff against a
// constant that would have moved with it.

use iced_pro::composite::tree_row;
use iced_pro::state::{BaseState, VisualState};
use iced_pro::token::contrast::contrast_ratio;
use iced_pro::token::{size, space, typography};
use iced_pro::widget::text_input as pro_text_input;

/// The literal this increment retired for the tree row's label size.
const RETIRED_ROW_TEXT_SIZE: f32 = 14.0;
/// The literal this increment retired for the secondary search lines —
/// the illegal sub-floor size.
const RETIRED_SECONDARY_TEXT_SIZE: f32 = 11.0;
/// The literal this increment retired for the per-depth indent step.
const RETIRED_INDENT_PER_DEPTH: f32 = 18.0;
/// The literal this increment retired for the row's horizontal inset.
const RETIRED_ROW_PADDING_X: f32 = 6.0;
/// The literal this increment retired for the panel body's padding.
const RETIRED_PANEL_PADDING: f32 = 6.0;
/// The literal this increment retired for the auto-fit slack.
const RETIRED_ROW_WIDTH_SLACK: f32 = 40.0;
/// The literal this increment retired for the search-result card height.
const RETIRED_SEARCH_ROW_HEIGHT: f32 = 52.0;

#[test]
fn no_text_the_panel_renders_falls_below_the_twelve_point_floor() {
    let floor = typography::MIN_SIZE;
    for (site, text_size) in PANEL_TEXT_SITES {
        assert!(
            *text_size >= floor,
            "`{site}` renders at {text_size}pt, below the {floor}pt \
             UIF-TOK-004 interface floor"
        );
    }
}

#[test]
fn the_floor_guard_covers_every_render_site_the_panel_draws() {
    // Ten sites, four distinct sizes. Adding a render site without
    // extending this list (and re-checking it against the floor) fails
    // here rather than going unguarded. `f32` is not `Ord`, so the
    // distinct-size set is keyed on the bit patterns.
    assert_eq!(
        PANEL_TEXT_SITES.len(),
        10,
        "the panel renders ten text sites; the guard list has changed length"
    );

    let distinct: std::collections::BTreeSet<u32> = PANEL_TEXT_SITES
        .iter()
        .map(|(_, text_size)| text_size.to_bits())
        .collect();
    let expected: std::collections::BTreeSet<u32> = [
        HEADER_SIZE,
        ROW_TEXT_SIZE,
        SECONDARY_TEXT_SIZE,
        SEARCH_GLYPH_SIZE,
    ]
    .into_iter()
    .map(f32::to_bits)
    .collect();
    assert_eq!(
        distinct, expected,
        "the guard list must cover exactly the sizes the panel renders"
    );
}

#[test]
fn the_retired_eleven_point_secondary_size_is_gone() {
    assert_eq!(
        SECONDARY_TEXT_SIZE,
        typography::CAPTION.size,
        "the secondary lines sit on `type.caption`"
    );
    assert_eq!(
        SECONDARY_TEXT_SIZE,
        typography::MIN_SIZE,
        "`caption` is the rung that sits exactly on the floor, which is what \
         makes it the legal home for the smallest text in the panel"
    );
    assert_ne!(
        SECONDARY_TEXT_SIZE, RETIRED_SECONDARY_TEXT_SIZE,
        "11pt is below the UIF-TOK-004 floor and must not come back"
    );
}

#[test]
fn the_row_label_sits_on_the_framework_label_rung() {
    assert_eq!(
        ROW_TEXT_SIZE,
        tree_row::LABEL_RUNG.size,
        "the row's label rung is the composite's own, not a parallel number"
    );
    assert_eq!(ROW_TEXT_SIZE, typography::UI_COMPACT.size);
    assert_ne!(
        ROW_TEXT_SIZE, RETIRED_ROW_TEXT_SIZE,
        "14pt was `type.body`; §4.3.2 puts a tree row on `ui_compact`"
    );
    assert!(ROW_TEXT_SIZE >= typography::MIN_SIZE);
}

#[test]
fn row_geometry_reads_the_framework_row_constants() {
    assert_eq!(ROW_INDENT_PER_DEPTH, tree_row::INDENT_STEP);
    assert_eq!(ROW_INDENT_PER_DEPTH, space::LG);
    assert_ne!(
        ROW_INDENT_PER_DEPTH, RETIRED_INDENT_PER_DEPTH,
        "18pt had no token behind it; the framework's step is 16"
    );

    assert_eq!(ROW_PADDING_X, tree_row::ROW_INSET_X);
    assert_eq!(ROW_PADDING_X, space::XS);
    assert_ne!(
        ROW_PADDING_X, RETIRED_ROW_PADDING_X,
        "§4.3.2 prices the tree row's horizontal inset at space.xs"
    );

    assert_eq!(ROW_PADDING_Y, tree_row::ROW_INSET_Y);
    assert_eq!(ROW_PADDING_Y, space::XXS);

    assert_eq!(
        ROW_WIDTH_SLACK,
        tree_row::ROW_INSET_X * 2.0 + tree_row::CHEVRON_COLUMN
    );
    assert_eq!(ROW_WIDTH_SLACK, 32.0);
    assert_ne!(
        ROW_WIDTH_SLACK, RETIRED_ROW_WIDTH_SLACK,
        "the slack is now the framework's fixed furniture, not a guess"
    );

    assert_eq!(PANEL_PADDING, space::XS);
    assert_ne!(
        PANEL_PADDING, RETIRED_PANEL_PADDING,
        "6 is not on the spacing grid — it exists only as shape.radius_card"
    );
}

#[test]
fn row_width_delegates_to_the_framework_content_width() {
    for depth in [
        0usize,
        1,
        3,
        7,
        MAX_ROW_INDENT_DEPTH,
        MAX_ROW_INDENT_DEPTH + 25,
    ] {
        let row = file_row(
            "a-long-ish-file-name.md",
            "/lib/a-long-ish-file-name.md",
            depth,
        );
        let clamped = depth.min(MAX_ROW_INDENT_DEPTH);
        assert_eq!(
            row_width(&row, measure),
            tree_row::content_width(clamped, measure(&row_label(&row))),
            "depth {depth}: the app's auto-fit term and the framework's \
             content-width rule must be the same rule"
        );
    }
}

#[test]
fn row_indent_pixels_is_width_relative_rather_than_depth_clamped() {
    assert_eq!(row_indent_pixels(0, WIDE_ROW), 0.0);
    assert_eq!(row_indent_pixels(1, WIDE_ROW), tree_row::INDENT_STEP);
    assert_eq!(row_indent_pixels(4, WIDE_ROW), 4.0 * tree_row::INDENT_STEP);
    assert_eq!(
        row_indent_pixels(MAX_ROW_INDENT_DEPTH, WIDE_ROW),
        MAX_ROW_INDENT_DEPTH as f32 * tree_row::INDENT_STEP,
        "where the budget lasts, the nominal step is what draws"
    );

    // **This assertion is the opposite of the one INT-12 deleted.** The
    // old test said "one level past the cap must not indent further",
    // which was the `min(depth, 50)` clamp being asserted back to itself.
    // The clamp is gone because it was the bug: it capped the *depth*
    // while leaving the *label* unprotected, so at the cap the indent was
    // 800px and the label sat off-screen. Depth is now bounded by the
    // budget rather than by a constant.
    assert!(
        row_indent_pixels(MAX_ROW_INDENT_DEPTH + 1, WIDE_ROW)
            > row_indent_pixels(MAX_ROW_INDENT_DEPTH, WIDE_ROW),
        "the budget decides how deep a row indents, not a cap"
    );
}

#[test]
fn the_indent_is_total_and_never_borrows_from_the_label_floor() {
    // No depth at any width overflows, goes negative, or eats the room
    // the framework reserved for the label.
    for depth in [0usize, 1, 8, 50, 1_000, 10_000, usize::MAX] {
        for width in [0.0f32, 1.0, 128.0, 150.0, 280.0, 1_000.0, WIDE_ROW] {
            let indent = row_indent_pixels(depth, width);
            assert!(indent.is_finite(), "depth {depth} width {width} overflowed");
            assert!(
                indent >= 0.0,
                "depth {depth} width {width} indented negatively"
            );
            assert!(
                indent <= tree_row::indent_allowance(width) + f32::EPSILON,
                "depth {depth} width {width}: indent {indent} exceeded the \
                 allowance {}",
                tree_row::indent_allowance(width),
            );
        }
    }
}

#[test]
fn at_least_ninety_six_pixels_of_label_survive_the_worst_case() {
    // §4.3.2 / §3B.6: `TREE_LABEL_MIN_W = 96px` of label stays visible
    // at every depth, via proportional compression rather than a cap.
    // The worst case the spec names is the deepest nesting in the narrowest
    // pane, so that is what is checked rather than a friendly width.
    //
    // `LEFT_PANE_MIN_W = 150` is the spec's own figure (§4.3.1). The
    // app's own `LEFT_PANEL_MIN_WIDTH` is 180, i.e. 30px *wider* than the
    // spec's floor, so the spec's worst case is checked at 150 and the
    // app's own minimum is strictly safer.
    let spec_min_pane = 150.0;
    let row_width = spec_min_pane - PANEL_PADDING - PANEL_PADDING;
    let allowance = tree_row::indent_allowance(row_width);

    for depth in [0usize, 1, 8, 50, 200, 10_000] {
        let indent = row_indent_pixels(depth, row_width);
        let label = row_width - ROW_PADDING_X - ROW_PADDING_X - indent;
        assert!(
            label >= tree_row::MIN_LABEL_WIDTH - f32::EPSILON,
            "depth {depth} at the spec's minimum pane: label gets {label:.1}px, \
             floor is {}",
            tree_row::MIN_LABEL_WIDTH,
        );
        assert!(
            indent <= allowance + f32::EPSILON,
            "depth {depth}: indent must absorb the shortfall, not the label"
        );
    }

    // The compression is real at this width, not merely non-overflowing:
    // eight nominal levels would want `8 × 16 = 128px` and the allowance
    // is only `142 − 128 = 14px`, so the step must have come down.
    let compressed = tree_row::indent_geometry(8, row_width);
    assert!(
        compressed.step < tree_row::INDENT_STEP,
        "eight levels cannot fit nominally in a 142px row; the step should \
         have compressed from {} but stayed at {}",
        tree_row::INDENT_STEP,
        compressed.step,
    );
    assert_eq!(
        compressed.width, allowance,
        "once compressing, the indent takes exactly the allowance"
    );
}

#[test]
fn the_badge_fits_only_where_the_label_floor_can_pay_for_it() {
    // The badge is charged against what the indent left over, so the fit
    // test must move with the width and the depth rather than being a
    // constant yes.
    //
    // Wide row, shallow: plenty of room.
    assert!(
        pdf_badge_fits(0, WIDE_ROW),
        "a wide row at depth 0 must show the badge"
    );
    // The charge is exactly `slot + gap`, so the boundary is exact: at a
    // width whose allowance is the charge, it fits; a hair under, it does
    // not.
    let exact = tree_row::MIN_LABEL_WIDTH
        + tree_row::CHEVRON_COLUMN
        + ROW_PADDING_X
        + ROW_PADDING_X
        + PDF_BADGE_CHARGE;
    assert!(
        pdf_badge_fits(0, exact),
        "at a width that leaves exactly the charge, the badge fits"
    );
    assert!(
        !pdf_badge_fits(0, exact - 1.0),
        "one pixel short of the charge must hide the badge rather than \
         breach the label floor"
    );
    // Deeper rows spend the allowance on indent, so the badge drops out
    // earlier the deeper it sits.
    assert!(
        !pdf_badge_fits(200, row_width_for(Some(LEFT_PANEL_MIN_WIDTH))),
        "at depth 200 in the narrowest pane there is nothing left to charge"
    );
}

#[test]
fn the_badge_never_breaches_the_label_floor_wherever_it_shows() {
    // The invariant the fit gate exists to hold, checked rather than
    // argued: for every width and depth where the badge shows, the label
    // still clears 96px after the badge has been paid for.
    for width in [150.0f32, 180.0, 220.0, 280.0, 400.0, 800.0, WIDE_ROW] {
        for depth in [0usize, 1, 2, 4, 8, 16, 32, 64, 128] {
            if !pdf_badge_fits(depth, width) {
                continue;
            }
            let indent = row_indent_pixels(depth, width);
            let label = width - ROW_PADDING_X - ROW_PADDING_X - indent - PDF_BADGE_CHARGE;
            assert!(
                label >= tree_row::MIN_LABEL_WIDTH - f32::EPSILON,
                "width {width} depth {depth}: label down to {label:.1}px with \
                 the badge shown"
            );
        }
    }
}

#[test]
fn the_pdf_badge_survives_the_selection_lock() {
    // **THE requirement this increment exists for.** §4.3.2: "Special item
    // states (such as PDF-backed notes) include a text/icon badge
    // (`[PDF]`) so meaning and contrast (`4.80:1`) are preserved when
    // selected."
    //
    // INT-7b fixed the *contrast* of a selected PDF-backed row by
    // role-locking the label to `text.strong`, and in doing so destroyed
    // the *meaning*: once role-locked, a selected PDF-backed row was
    // pixel-identical to a selected ordinary row, so the PDF-backed
    // state silently vanished on selection. The badge is the second half
    // of that fix. It survives the lock because it is a word rather than
    // a colour, and it stays legible because [`pdf_badge_colour`] routes
    // it through the same `tree_row::locked_over` call the label uses —
    // so it arrives at `text.strong` exactly when the label does, rather
    // than carrying `status.danger.text` onto the selection band.
    let pro = Palette::pro();
    let band = row_paint(true, false).band;
    let badge = pdf_badge_colour(true, false);

    assert_eq!(
        band, pro.accent_fill,
        "a selected row's band is accent.fill"
    );
    assert_eq!(
        badge, pro.text_strong,
        "the badge must be locked to text.strong, not left danger-red"
    );
    assert_ne!(
        badge, band,
        "a badge painted in its own background colour is not a badge"
    );
    assert!(
        contrast_ratio(badge, band) >= 4.80,
        "selected PDF-backed badge measures {:.4}:1 against the spec's 4.80:1",
        contrast_ratio(badge, band),
    );
    // The badge and the label are the same colour here, which is the
    // point: they lock together, so neither can be left behind by the
    // other.
    assert_eq!(
        badge,
        row_label_colour(true, false, true),
        "the badge and a PDF-backed label must resolve identically when selected"
    );
}

#[test]
fn every_badge_posture_is_legible_computed_from_raw_srgb() {
    // Each posture the badge can appear in, measured rather than
    // transcribed. Nothing here is copied from the spec's rounded figures;
    // every number is recomputed from the palette's sRGB channels by
    // `contrast_ratio`.
    let pro = Palette::pro();
    let postures = [
        ("rest", false, false),
        ("hovered", false, true),
        ("selected", true, false),
        ("selected+hovered", true, true),
    ];

    for (name, selected, hovered) in postures {
        let band = row_paint(selected, hovered).band;
        let badge = pdf_badge_colour(selected, hovered);
        let ratio = contrast_ratio(badge, band);
        assert!(
            meets_text_contrast(badge, band),
            "badge in posture {name}: {ratio:.6}:1 against band {band:?} \
             fails the 4.5:1 AA text floor"
        );
        // The badge never resolves to the band it sits on.
        assert_ne!(badge, band, "posture {name}: badge invisible on its band");
    }

    // Pinned figures, so a palette retune that quietly moves one of them
    // fails here instead of in a review screenshot.
    let rest = contrast_ratio(pdf_badge_colour(false, false), row_paint(false, false).band);
    let hovered = contrast_ratio(pdf_badge_colour(false, true), row_paint(false, true).band);
    let selected = contrast_ratio(pdf_badge_colour(true, false), row_paint(true, false).band);
    let selected_hovered = contrast_ratio(pdf_badge_colour(true, true), row_paint(true, true).band);

    assert!((rest - 6.572479).abs() < 1e-5, "rest = {rest:.6}");
    assert!((hovered - 5.363299).abs() < 1e-5, "hovered = {hovered:.6}");
    assert!(
        (selected - 4.801988).abs() < 1e-5,
        "selected = {selected:.6}"
    );
    assert!(
        (selected_hovered - selected).abs() < 1e-9,
        "selection outranks hover, so the band must not move: \
         {selected_hovered:.6} vs {selected:.6}"
    );

    // **The defect this design prevents, pinned as illegal.** Had the
    // badge carried its own `status.danger.text` through selection rather
    // than taking the lock, it would paint this on the selection band.
    let unlocked = contrast_ratio(pro.status_danger_text, pro.accent_fill);
    assert!(
        unlocked < 2.0,
        "status.danger.text on accent.fill is {unlocked:.6}:1 — if this ever \
         becomes readable the lock being tested above is not what is saving it"
    );
}

#[test]
fn the_badge_cannot_move_the_label_because_the_box_never_sees_it() {
    // "Must not shift the label" is structural, not behavioural: the
    // badge is trailing content and [`row_box`] takes no PDF-backed
    // input at all, so there is no path by which the badge reaches the
    // label's left edge. Asserted rather than trusted, because the day
    // someone adds a `pdf_backed` parameter to `row_box` this test is
    // where they find out that was the invariant.
    for depth in [0usize, 1, 7, 33] {
        let left = ROW_PADDING_X + row_indent_pixels(depth, WIDE_ROW);
        // The label's left edge is a function of depth alone.
        assert_eq!(
            row_box(depth, WIDE_ROW, false, false).padding.left,
            left,
            "depth {depth}: label left edge must be inset + indent"
        );
        assert_eq!(
            row_box(depth, WIDE_ROW, true, true).padding.left,
            left,
            "depth {depth}: posture must not move the label either"
        );
    }
    // And the row's height is the pinned rung whatever the badge does.
    assert_eq!(row_box(9, WIDE_ROW, true, true).height, ROW_MIN_HEIGHT);
    assert!(ROW_MIN_HEIGHT >= size::HIT_TARGET_MIN);
}

#[test]
fn the_badge_is_a_shape_that_fits_the_compact_row() {
    // The badge must not push the row past its pinned rung, and the row's
    // interior must actually contain the badge's line box.
    assert_eq!(ROW_MIN_HEIGHT, size::HIT_TARGET_MIN);
    let interior = ROW_MIN_HEIGHT - ROW_PADDING_Y - ROW_PADDING_Y;
    assert!(
        interior >= PDF_BADGE_RUNG.line_height,
        "the badge's {}px line box must fit the row's {interior}px interior",
        PDF_BADGE_RUNG.line_height,
    );
    // The badge is typed on the badge atom's own rung, not a size picked
    // here, and that rung is at or above the interface-wide type floor.
    assert_eq!(PDF_BADGE_RUNG.size, badge::TYPE_RUNG.size);
    assert!(PDF_BADGE_RUNG.size >= typography::MIN_SIZE);
    // The slot is wide enough for the literal it declares, with slack.
    assert!(PDF_BADGE_SLOT > PDF_BADGE_GAP);
    assert_eq!(PDF_BADGE_CHARGE, PDF_BADGE_SLOT + PDF_BADGE_GAP);
}

#[test]
fn a_row_meets_the_pointer_target_minimum_and_still_holds_its_label() {
    assert_eq!(ROW_MIN_HEIGHT, tree_row::ROW_HEIGHT_COMPACT);
    assert_eq!(ROW_MIN_HEIGHT, size::HIT_TARGET_MIN);
    assert!(
        ROW_MIN_HEIGHT >= size::HIT_TARGET_MIN,
        "§4.3.2: every row has minimum height 24px"
    );
    assert!(
        tree_row::label_fits_row(ROW_MIN_HEIGHT),
        "the label's line box must fit inside the row at the compact rung"
    );
    // The row's own content (line box + both vertical insets) must fit
    // inside the rung the row is fixed to, or the label is clipped.
    assert!(
        tree_row::LABEL_RUNG.line_height + ROW_PADDING_Y * 2.0 <= ROW_MIN_HEIGHT,
        "18 + 2×2 must fit inside 24"
    );
}

#[test]
fn the_search_card_is_tall_enough_for_the_lines_it_fixes() {
    let content = tree_row::LABEL_RUNG.line_height + typography::CAPTION.line_height * 2.0;
    assert_eq!(
        SEARCH_ROW_HEIGHT,
        content + SEARCH_CARD_PADDING * 2.0,
        "the card height is derived from its three line boxes plus padding"
    );
    assert!(
        SEARCH_ROW_HEIGHT >= content + SEARCH_CARD_PADDING * 2.0,
        "the card must never be shorter than the text it is fixed to"
    );
    assert_eq!(SEARCH_ROW_HEIGHT, 58.0);
    assert_ne!(
        SEARCH_ROW_HEIGHT, RETIRED_SEARCH_ROW_HEIGHT,
        "52 could not hold the rung-height lines the card now renders"
    );
}

#[test]
fn a_selected_row_paints_the_framework_accent_band() {
    let pro = Palette::pro();
    let selected = row_paint(true, false);
    let resting = row_paint(false, false);

    assert_eq!(selected.background, pro.accent_fill);
    assert_eq!(selected.band, pro.accent_fill);
    assert_eq!(
        resting.background,
        iced::Color::TRANSPARENT,
        "a resting row paints no fill"
    );
    assert_eq!(resting.band, pro.surface_base);
}

#[test]
fn a_selected_pdf_backed_label_is_locked_off_the_unreadable_red() {
    let pro = Palette::pro();
    let app_palette = Palette::fastmd_dark();

    // The defect the lock exists for: the retired red-on-accent pairing.
    let red_on_accent = contrast_ratio(app_palette.error, pro.accent_fill);
    assert!(
        red_on_accent < 3.0,
        "fixture: status_danger_text on accent_fill must be the unreadable \
         pairing this test guards, measured {red_on_accent}:1"
    );

    assert_eq!(
        row_label_colour(true, false, true),
        pro.text_strong,
        "§3B.6 locks the row label to text.strong on the accent fill by role"
    );
    assert_ne!(
        row_label_colour(true, false, true),
        app_palette.error,
        "a selected PDF-backed row must not render red-on-indigo"
    );
    let locked_contrast = contrast_ratio(row_label_colour(true, false, true), pro.accent_fill);
    assert!(
        locked_contrast >= 4.5,
        "the locked label clears the text-contrast rule at {locked_contrast}:1"
    );
}

#[test]
fn an_unselected_row_keeps_the_apps_own_label_rule() {
    let app_palette = Palette::fastmd_dark();
    assert_eq!(row_label_colour(false, false, true), app_palette.error);
    assert_eq!(row_label_colour(false, false, false), app_palette.text);
    assert_ne!(
        row_label_colour(false, false, true),
        row_label_colour(false, false, false),
        "the PDF-backed tint must still be visible off the selection band"
    );
}

#[test]
fn the_search_field_paints_from_the_framework_search_input_kind() {
    use iced::widget::text_input::Status;

    let pro = Palette::pro();
    for (status, base, focused) in [
        (Status::Active, BaseState::Rest, false),
        (Status::Hovered, BaseState::Hovered, false),
        (Status::Focused { is_hovered: false }, BaseState::Rest, true),
        (Status::Disabled, BaseState::Disabled, false),
    ] {
        let resolved = pro_text_input::resolve_input_style(
            pro_text_input::InputKind::Search,
            VisualState { base, focused },
            &pro,
        );
        let style = search_field_style(status);
        assert_eq!(
            (style.background, style.border.color, style.border.width),
            (
                iced::Background::Color(resolved.background),
                resolved.border_color,
                resolved.border_width
            ),
            "search field at {status:?} must carry the framework's resolved fill"
        );
        assert_eq!(style.value, resolved.text);
        assert_eq!(style.placeholder, resolved.placeholder);
        assert_eq!(style.icon, resolved.icon);
        assert_eq!(style.selection, resolved.selection_background);
    }
}

#[test]
fn the_search_field_is_not_painted_from_iced_s_own_theme() {
    // A cheap tripwire: the focused border must be the framework's focus
    // stroke, which is strictly heavier than iced's resting input border.
    let focused =
        search_field_style(iced::widget::text_input::Status::Focused { is_hovered: false });
    let active = search_field_style(iced::widget::text_input::Status::Active);
    assert!(
        focused.border.width > active.border.width,
        "focus must be drawn by the framework's focus ring, not left to iced"
    );
}

#[test]
fn the_search_glyphs_are_on_the_icon_rung_not_the_text_rung() {
    assert_eq!(SEARCH_GLYPH_SIZE, pro_text_input::SLOT_GLYPH_SIZE);
    assert_eq!(SEARCH_GLYPH_SIZE, size::ICON_MD);
    assert_ne!(
        SEARCH_GLYPH_SIZE, RETIRED_ROW_TEXT_SIZE,
        "icon slots must not track the row's text rung"
    );
    assert_ne!(SEARCH_GLYPH_SIZE, ROW_TEXT_SIZE);
}

#[test]
fn the_search_row_spacing_is_on_the_grid() {
    assert_eq!(SEARCH_ROW_SPACING, space::XS);
    assert_eq!(RESULT_LINE_SPACING, space::XXS);
}

#[test]
fn the_header_sits_on_the_title_sm_rung() {
    assert_eq!(HEADER_SIZE, typography::TITLE_SM.size);
    assert!(HEADER_SIZE >= typography::MIN_SIZE);
}

// --- flatten depth guard (UI-017 robustness) --------------------------
//
// `flatten_tree`'s `MAX_TREE_DEPTH` stack guard is asserted through the
// iced left panel, which calls that same function.

/// A synthetic `root/l0/l1/…` directory chain of `depth` levels, plus the
/// expanded-dir set that opens every one of them ([RUST-006]: nothing here
/// touches the filesystem).
fn deep_chain(depth: usize) -> (TreeNode, HashSet<PathBuf>) {
    let mut root = TreeNode::new("root".to_string(), PathBuf::from("/root"), true);
    let mut expanded = HashSet::new();
    let mut parent = &mut root;
    for index in 0..depth {
        let name = format!("level-{index}");
        let path = parent.path.join(&name);
        expanded.insert(path.clone());
        parent
            .children
            .insert(name.clone(), TreeNode::new(name.clone(), path, true));
        parent = parent.children.get_mut(&name).expect("inserted above");
    }
    (root, expanded)
}

#[test]
fn tree_flatten_stops_at_the_depth_guard_instead_of_overflowing() {
    let depth_guard = crate::ui::tree::flatten::MAX_TREE_DEPTH;
    let (root, expanded) = deep_chain(depth_guard + 50);

    let mut rows = Vec::new();
    flatten_tree(&root, 0, &expanded, &mut rows);

    assert_eq!(
        rows.len(),
        depth_guard,
        "the guard must cut the walk at MAX_TREE_DEPTH rows, not recurse further"
    );
    assert_eq!(
        rows.last().map(|row| row.depth),
        Some(depth_guard - 1),
        "the deepest emitted row is the last level inside the guard"
    );
}

#[test]
fn tree_flatten_is_unaffected_below_the_depth_guard() {
    let (root, expanded) = deep_chain(4);

    let mut rows = Vec::new();
    flatten_tree(&root, 0, &expanded, &mut rows);

    assert_eq!(
        rows.len(),
        4,
        "a chain inside the guard flattens every level"
    );
    assert_eq!(
        rows.iter().map(|row| row.depth).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}

// --- UI-050 search-result branch -------------------------------------
//
// The state gate is covered by
// `from_app_carries_query_search_state_and_selection`; the tests below
// assert that both branches of the panel body actually build.

#[test]
fn search_result_branch_builds_the_empty_state() {
    let (app, _dir) = app_with_system_lib();
    let mut view_model = LeftPanelViewModel::from_app(&app);
    view_model.is_searching = true;
    view_model.results = Arc::new(Vec::new());

    // The no-results branch must build (and not fall through to the tree).
    let _element = search_results(&view_model);
}

#[test]
fn search_result_branch_builds_populated_and_sparse_cards() {
    let (app, _dir) = app_with_system_lib();
    let mut view_model = LeftPanelViewModel::from_app(&app);
    view_model.is_searching = true;
    view_model.results = Arc::new(vec![
        SearchResultEntry {
            path: PathBuf::from("/lib/notes.md"),
            file_name: "notes.md".to_string(),
            relative_path: "Work / notes.md".to_string(),
            snippet: "…the matching line…".to_string(),
            line_number: 7,
            match_count: 3,
        },
        SearchResultEntry {
            path: PathBuf::from("/lib/bare.md"),
            file_name: "bare.md".to_string(),
            relative_path: String::new(),
            snippet: String::new(),
            line_number: 1,
            match_count: 1,
        },
    ]);

    let _element = search_results(&view_model);
}

// ---------------------------------------------------------------------------
// INT-10 — tree-row hover (UIF-TOK-001 hover rung, §2.5 `color.surface.hover`)
// ---------------------------------------------------------------------------

#[test]
fn a_hovered_row_paints_the_hover_token_itself_not_merely_a_state() {
    let pro = Palette::pro();
    let hovered = row_paint(false, true);
    let resting = row_paint(false, false);

    // The requirement is the *colour*, not the enum. `surface_hover` is
    // the only band `resolve_style` produces for `BaseState::Hovered` on
    // an unselected row, so matching it proves the tier was actually
    // reached rather than defaulted.
    assert_eq!(
        hovered.background, pro.surface_hover,
        "a hovered row must paint color.surface.hover"
    );
    assert_eq!(
        hovered.band, pro.surface_hover,
        "the mark band must move onto the hover fill, not stay on the pane"
    );
    assert_eq!(
        hovered.background,
        iced::Color::from_rgb8(32, 32, 36),
        "§2.5 prices color.surface.hover at RGB(32,32,36)"
    );

    // Non-vacuity: the hovered and resting paints are genuinely
    // different, so the assertions above could not both hold if hover
    // were still being defaulted to Rest.
    assert_ne!(
        hovered.background, resting.background,
        "if hover were still resolving to Rest these two would be identical"
    );
    assert_eq!(
        resting.background,
        iced::Color::TRANSPARENT,
        "a resting row still paints no fill"
    );
}

#[test]
fn selection_outranks_the_pointer_so_hover_cannot_wash_it_out() {
    let pro = Palette::pro();

    // §3B.6's contrast lock depends on this: if hover could override the
    // selection band, a selected row under the cursor would change
    // surface and the label rule would no longer match what is painted.
    let selected_and_hovered = row_paint(true, true);
    let selected_only = row_paint(true, false);

    assert_eq!(
        selected_and_hovered.band, pro.accent_fill,
        "a selected row keeps accent.fill under the pointer"
    );
    assert_eq!(
        selected_and_hovered.band, selected_only.band,
        "hover must not change a selected row's band at all"
    );
    assert_ne!(
        selected_and_hovered.band, pro.surface_hover,
        "the hover fill must never replace the selection fill"
    );
}

#[test]
fn a_hovered_label_clears_the_text_contrast_floor_computed_from_raw_srgb() {
    let pro = Palette::pro();
    let app_palette = Palette::fastmd_dark();

    // Both figures below are measured through `contrast_ratio` against
    // the actual hover band, then pinned. They are not transcribed from
    // any printed table: if a token is retuned, this test fails and the
    // new number has to be re-derived.
    let body_on_hover = contrast_ratio(app_palette.text, pro.surface_hover);
    assert!(
        (body_on_hover - 10.738_035).abs() < 1e-4,
        "body label on the hover band was measured at {body_on_hover}:1 and \
         is pinned there"
    );
    assert!(
        body_on_hover >= 4.5,
        "the ordinary hovered label clears the text floor at {body_on_hover}:1"
    );

    // The worst case on the hover band is the PDF-backed `error` tint,
    // which the lock deliberately does NOT promote.
    let danger_on_hover = contrast_ratio(app_palette.error, pro.surface_hover);
    assert!(
        (danger_on_hover - 5.363_298).abs() < 1e-4,
        "error-tinted label on the hover band was measured at \
         {danger_on_hover}:1 and is pinned there"
    );
    assert!(
        danger_on_hover >= 4.5,
        "even the worst hovered pairing clears the text floor at {danger_on_hover}:1"
    );
}

#[test]
fn hover_leaves_the_label_rule_untouched_because_the_hover_band_is_not_locked() {
    let pro = Palette::pro();
    let app_palette = Palette::fastmd_dark();

    // The mechanism, stated and checked rather than inferred:
    // `surface_hover` is not one of the four locked solid fills, so
    // `locked_over` has no reason to promote anything and returns the
    // candidate unchanged.
    assert!(
        !iced_pro::token::contrast::fill_lock_role(pro.surface_hover, &pro).is_locked_fill(),
        "surface_hover must not be a locked fill, or hover would repaint \
         every label white"
    );
    assert!(
        iced_pro::token::contrast::fill_lock_role(pro.accent_fill, &pro).is_locked_fill(),
        "while the selection band still is, so the difference between the \
         two cases is real and not an accident of one lookup"
    );

    // Consequence: the resting label rule survives the pointer, the PDF
    // tint included.
    assert_eq!(
        row_label_colour(false, true, false),
        app_palette.text,
        "a hovered plain row keeps the body token"
    );
    assert_eq!(
        row_label_colour(false, true, true),
        app_palette.error,
        "a hovered PDF-backed row keeps its error tint"
    );
    assert_eq!(
        row_label_colour(false, true, true),
        row_label_colour(false, false, true),
        "hover must not change the label colour at all, only the band under it"
    );
    assert_ne!(
        row_paint(false, true).band,
        row_paint(false, false).band,
        "and the band really did move, so the equality above is not vacuous"
    );
}

#[test]
fn the_selection_lock_still_holds_on_a_selected_and_hovered_pdf_row() {
    let pro = Palette::pro();

    // The original defect was red-on-indigo. Hover adds a third input to
    // the same row; the lock must still win.
    let colour = row_label_colour(true, true, true);
    assert_eq!(
        colour, pro.text_strong,
        "selected + hovered + PDF-backed still locks to text.strong"
    );
    let contrast = contrast_ratio(colour, row_paint(true, true).band);
    assert!(
        contrast >= 4.5,
        "the locked label clears the floor against the band actually painted \
         at {contrast}:1"
    );
}

#[test]
fn hovering_moves_no_part_of_the_row_box_at_any_depth_or_posture() {
    // §2: a hover MUST NOT alter the outer bounding box or shift a
    // neighbour. `row_box` takes the posture as an input precisely so
    // this can be diffed rather than promised.
    for depth in 0..8usize {
        let rest = row_box(depth, WIDE_ROW, false, false);
        assert_eq!(
            row_box(depth, WIDE_ROW, false, true),
            rest,
            "hover moved the row box at depth {depth}"
        );
        assert_eq!(
            row_box(depth, WIDE_ROW, true, false),
            rest,
            "selection moved the row box at depth {depth}"
        );
        assert_eq!(
            row_box(depth, WIDE_ROW, true, true),
            rest,
            "selected+hover moved the row box at depth {depth}"
        );
        assert_eq!(
            rest.height, ROW_MIN_HEIGHT,
            "the row height is the compact rung at every depth"
        );
        assert_eq!(
            rest.padding.left,
            ROW_PADDING_X + row_indent_pixels(depth, WIDE_ROW),
            "the indent is the only thing that varies, and only with depth"
        );
    }
}

#[test]
fn hover_is_keyed_by_path_so_a_recycled_slot_cannot_borrow_a_highlight() {
    // The highest-risk property. iced matches widget state by position,
    // so the only thing standing between a re-flattened list and a
    // highlight on the wrong file is this comparison.
    let alpha = file_row("alpha.md", "/ws/alpha.md", 0);
    let beta = file_row("beta.md", "/ws/beta.md", 0);

    assert!(
        row_is_hovered(&alpha, Some(Path::new("/ws/alpha.md"))),
        "the row the pointer entered reads as hovered"
    );
    assert!(
        !row_is_hovered(&beta, Some(Path::new("/ws/alpha.md"))),
        "a different file in the same slot must NOT inherit the highlight"
    );
    assert!(
        !row_is_hovered(&alpha, Some(Path::new("/ws/beta.md"))),
        "alpha must not light up because beta is hovered"
    );
    assert!(
        !row_is_hovered(&alpha, None),
        "nothing is hovered when the pointer is off the tree"
    );
}

#[test]
fn a_stale_hover_path_paints_no_row_at_all() {
    // A deleted or filtered-out file leaves its path behind in the
    // reducer until the prune runs. Because the key is the path, the
    // worst a stale entry can do is match nothing, never light up a
    // surviving neighbour.
    let rows = [
        file_row("beta.md", "/ws/beta.md", 0),
        file_row("gamma.md", "/ws/gamma.md", 0),
    ];
    let ghost = PathBuf::from("/ws/deleted.md");

    let lit: Vec<&str> = rows
        .iter()
        .filter(|row| row_is_hovered(row, Some(ghost.as_path())))
        .map(|row| row.name.as_str())
        .collect();

    assert!(
        lit.is_empty(),
        "a hover path naming a gone file must paint nothing, got {lit:?}"
    );
}

// --- INT-11: the selection bar and the depth guides ---------------------

/// The selection bar's width is the framework's `SELECTION_BAR_WIDTH`, not a
/// literal introduced with the painting.
///
/// That constant is `shape.border.focus`, the token §2's border table names
/// against the "tree/rail selection bar" and §3B.6 prices at `2px`. Pinning
/// the identity rather than the number means a retune of `border.focus` moves
/// the bar with the token instead of leaving a stale `2.0` behind.
#[test]
fn the_selection_bar_uses_the_frameworks_bar_width_not_a_new_literal() {
    let overlay = row_overlay(2, WIDE_ROW, true, false);
    let bar = overlay
        .selection_bar
        .expect("a selected row must carry a selection bar");

    assert_eq!(
        bar.width,
        tree_row::SELECTION_BAR_WIDTH,
        "the bar must read `tree_row::SELECTION_BAR_WIDTH`"
    );
    assert_eq!(
        bar.width,
        iced_pro::token::shape::BORDER_FOCUS,
        "`SELECTION_BAR_WIDTH` is `border.focus`, i.e. the 2px §3B.6 draws"
    );
    assert_eq!(bar.x, 0.0, "the bar is the row's left edge");

    assert_eq!(
        row_overlay(2, WIDE_ROW, false, false).selection_bar,
        None,
        "an unselected row must have no bar at all, not a transparent one"
    );
}

/// The depth guides land on `INDENT_STEP` multiples.
///
/// Each line is centred in its step (`ROW_PADDING_X + level × step + step / 2`),
/// so the pitch between consecutive lines is exactly one step and the whole
/// set is keyed to [`tree_row::INDENT_STEP`] rather than to a spacing chosen
/// at the paint site.
#[test]
fn the_depth_guides_land_on_indent_step_multiples() {
    for depth in 1..8usize {
        let offsets = row_guide_offsets(depth, WIDE_ROW);
        assert_eq!(
            offsets.len(),
            depth,
            "one guide per indented level at depth {depth}"
        );

        for (level, x) in offsets.iter().enumerate() {
            let expected =
                ROW_PADDING_X + level as f32 * tree_row::INDENT_STEP + tree_row::INDENT_STEP / 2.0;
            assert_eq!(
                *x, expected,
                "guide {level} at depth {depth} must sit centred in its step"
            );
        }

        if let (Some(first), Some(second)) = (offsets.first(), offsets.get(1)) {
            assert_eq!(
                second - first,
                tree_row::INDENT_STEP,
                "the pitch between guides is exactly one indent step"
            );
        }
    }
}

/// A guide never reaches the label.
///
/// The deepest line sits half a step short of where the label starts, so no
/// stripe can draw through text at any depth — the property that makes the
/// guides safe to paint over a row whose label fills the pane.
#[test]
fn no_depth_guide_reaches_the_label() {
    for depth in 1..12usize {
        let label_left = ROW_PADDING_X + row_indent_pixels(depth, WIDE_ROW);
        let overlay = row_overlay(depth, WIDE_ROW, false, false);
        let deepest = overlay
            .guides
            .last()
            .unwrap_or_else(|| panic!("depth {depth} must draw guides"));

        assert!(
            deepest.x + deepest.width <= label_left,
            "the deepest guide ends at {} and the label starts at {}; a guide \
             under the label would draw through text",
            deepest.x + deepest.width,
            label_left,
        );
    }
}

/// The guide set is exactly the set of levels that actually drew an
/// indent tier — never more, never fewer.
///
/// **INT-12 rewrote this test.** It used to assert that the guide count
/// clamped at [`MAX_ROW_INDENT_DEPTH`], which was the depth cap being
/// asserted back to itself a second time. The cap is gone. The guide
/// count now follows [`tree_row::indent_geometry`], which in the
/// collapsed tier draws *fewer* guides than the depth and reports the
/// surplus as `collapsed_levels` rather than hanging it inside the label.
///
/// What has not changed is why the test exists at all: a guide drawn for
/// a level that was never indented sits in the label instead of beside
/// it. That is the invariant, and it now holds by construction because
/// both the indent and the guides read the same resolution — which is
/// exactly the coupling INT-11's note on [`row_guide_offsets`] demanded.
#[test]
fn the_guide_set_matches_the_levels_that_actually_drew() {
    // Nominal tier: one guide per level, none past the label's left edge.
    for depth in [0usize, 1, 4, MAX_ROW_INDENT_DEPTH] {
        let guides = row_guide_offsets(depth, WIDE_ROW);
        assert_eq!(guides.len(), depth, "depth {depth}: one guide per level");
        if let Some(last) = guides.last() {
            assert!(
                *last <= row_indent_pixels(depth, WIDE_ROW),
                "depth {depth}: last guide at {last} passed the label's left \
                 edge {}",
                row_indent_pixels(depth, WIDE_ROW),
            );
        }
    }

    // Collapsed tier: past the `4px` floor step the guide count stops
    // growing, and the surplus is reported rather than drawn.
    let narrow = row_width_for(Some(LEFT_PANEL_MIN_WIDTH));
    let drawn = tree_row::indent_geometry(200, narrow);
    assert!(
        drawn.guides.len() < 200,
        "200 levels cannot each get a line in a {narrow}px row"
    );
    assert_eq!(
        drawn.guides.len() + drawn.collapsed_levels,
        200,
        "every level is either drawn or reported collapsed, none lost"
    );
    assert_eq!(
        row_guide_offsets(200, narrow),
        drawn.guides,
        "the app's guides are the compressed geometry, not the nominal step"
    );
    assert_eq!(
        row_overlay(200, narrow, false, false)
            .guides
            .iter()
            .map(|stripe| stripe.x)
            .collect::<Vec<_>>(),
        drawn.guides,
        "the overlay reads the same compressed set"
    );
}

/// Every stripe colour is the framework's, read off [`row_paint`] — nothing in
/// the overlay re-decides a colour locally.
///
/// On a selected row §3B.6 locks the label, the chevron, the guides and the
/// left bar to `text.strong` **by role**, so they stay locked even though
/// `border.strong` over `accent.fill` measures `1.09:1` and `accent.border`
/// measures `1.61:1`. Off the band the resting tokens come back.
#[test]
fn every_overlay_stripe_colour_is_the_frameworks_and_clears_the_chrome_floor() {
    use iced_pro::token::contrast::MIN_CHROME_CONTRAST;

    let pro = Palette::pro();

    for depth in 0..6usize {
        for hovered in [false, true] {
            let paint = row_paint(false, hovered);
            let overlay = row_overlay(depth, WIDE_ROW, false, hovered);
            for guide in &overlay.guides {
                assert_eq!(
                    guide.color, paint.guide,
                    "the guide must carry `TreeRowStyle::guide`, nothing else"
                );
                assert_eq!(
                    guide.color, pro.border_strong,
                    "off the selection band the resting guide is `border.strong`"
                );
                let ratio = contrast_ratio(guide.color, paint.band);
                assert!(
                    ratio >= MIN_CHROME_CONTRAST,
                    "border.strong on the band actually painted at depth {depth} \
                     (hovered={hovered}) is {ratio:.6}:1, under the \
                     {MIN_CHROME_CONTRAST}:1 chrome floor §2.5 prices for tree \
                     guides"
                );
            }
        }
    }

    // Selected: both stripes lock to text.strong by role.
    for depth in 0..6usize {
        for hovered in [false, true] {
            let paint = row_paint(true, hovered);
            let overlay = row_overlay(depth, WIDE_ROW, true, hovered);

            let bar = overlay
                .selection_bar
                .expect("selected rows carry a bar at every depth");
            assert_eq!(bar.color, paint.selection_bar);
            assert_eq!(
                bar.color, pro.text_strong,
                "the bar locks by role, not because a measurement happened to \
                 clear a threshold"
            );

            let bar_ratio = contrast_ratio(bar.color, paint.band);
            assert!(
                bar_ratio >= MIN_CHROME_CONTRAST,
                "the locked bar against its own band is {bar_ratio:.6}:1"
            );

            for guide in &overlay.guides {
                assert_eq!(
                    guide.color, pro.text_strong,
                    "§3B.6 locks the depth guides on a selected row too"
                );
                let ratio = contrast_ratio(guide.color, paint.band);
                assert!(
                    ratio >= MIN_CHROME_CONTRAST,
                    "locked guide on accent.fill is {ratio:.6}:1 at depth {depth}"
                );
            }
        }
    }

    // The resting tokens the lock is protecting against stay unmeasurable on
    // the band, so the lock cannot be argued away as unnecessary.
    assert!(
        contrast_ratio(pro.border_strong, pro.accent_fill) < 2.0,
        "border.strong on accent.fill must stay near-invisible for the lock to \
         be doing real work"
    );
    assert!(
        contrast_ratio(pro.accent_border, pro.accent_fill) < 2.0,
        "accent.border on accent.fill must stay near-invisible too"
    );
}

/// Hover moves a guide's **colour** with the band and nothing else.
///
/// The geometry is posture-independent for the same reason [`RowBox`]'s is:
/// a stripe that grew, shifted or appeared on hover would move a neighbour,
/// and §2 forbids it. Only the fill changes, because the band under it moved
/// from the pane's resting surface to `surface.hover`.
#[test]
fn hover_moves_guide_geometry_never_colour_only_the_band() {
    for depth in 1..8usize {
        let rest = row_overlay(depth, WIDE_ROW, false, false);
        let hover = row_overlay(depth, WIDE_ROW, false, true);

        assert_eq!(
            rest.guides.len(),
            hover.guides.len(),
            "hover must not add or drop a guide"
        );
        for (a, b) in rest.guides.iter().zip(&hover.guides) {
            assert_eq!(a.x, b.x, "hover moved a guide horizontally");
            assert_eq!(a.width, b.width, "hover widened a guide");
        }

        // The band really did move, so the colour change is a consequence of
        // the surface rather than a coincidence.
        assert_ne!(
            row_paint(false, false).band,
            row_paint(false, true).band,
            "hover must change the band the guide is measured against"
        );
    }
}

/// A resting, unindented row carries no overlay, so it pays no `Stack`.
#[test]
fn a_resting_unindented_row_carries_no_overlay_at_all() {
    let overlay = row_overlay(0, WIDE_ROW, false, false);
    assert!(
        overlay.is_empty(),
        "depth 0 has no guides and an unselected row has no bar"
    );
    assert_eq!(
        overlay.widget_cost(),
        0,
        "nothing to paint means nothing to build"
    );
}

/// The overlay's per-row widget cost, stated rather than discovered later.
///
/// One `Stack`, one widget for the selection bar (it needs no offset wrapper at
/// `x = 0`), and **two per guide** — the bar plus the transparent full-width
/// wrapper that pushes it out, because iced 0.14's `Stack` lays every child at
/// its own top-left origin and `Container` has no `margin`. The tree is not
/// virtualised, so this is paid per visible row per frame; the bound is what
/// [FMD-060] / [FMD-061] are measured against.
#[test]
fn the_overlay_widget_cost_is_stated_and_bounded() {
    assert_eq!(row_overlay(0, WIDE_ROW, false, false).widget_cost(), 0);
    assert_eq!(row_overlay(0, WIDE_ROW, true, false).widget_cost(), 2);
    assert_eq!(row_overlay(1, WIDE_ROW, false, false).widget_cost(), 3);
    assert_eq!(row_overlay(1, WIDE_ROW, true, false).widget_cost(), 4);
    assert_eq!(row_overlay(3, WIDE_ROW, true, true).widget_cost(), 8);

    // The formula, so a future edit cannot change the shape silently. The one
    // exception is the empty overlay, which skips the `Stack` entirely.
    for depth in 0..8usize {
        for selected in [false, true] {
            let overlay = row_overlay(depth, WIDE_ROW, selected, false);
            let expected = if depth == 0 && !selected {
                0
            } else {
                1 + usize::from(selected) + depth * 2
            };
            assert_eq!(
                overlay.widget_cost(),
                expected,
                "cost at depth {depth}, selected={selected} must be \
                 1 Stack + the bar + 2 per guide"
            );
        }
    }

    // A worst-case row in this app's tree, bounded.
    let worst = row_overlay(MAX_ROW_INDENT_DEPTH, WIDE_ROW, true, true);
    assert_eq!(
        worst.widget_cost(),
        1 + 1 + MAX_ROW_INDENT_DEPTH * 2,
        "the deepest selectable row is the ceiling"
    );
}
