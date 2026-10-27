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
// `src/hoarfrost/token/*_tests.rs`).
#![allow(clippy::assertions_on_constants)]

use super::*;
use crate::ui_iced::context_menu::{ContextGroup, ContextRow};
use crate::ui_iced::test_helpers::test_app;
use hoarfrost::binding::{Cancel, Commit, Live};
use hoarfrost::token::contrast::meets_text_contrast;
use hoarfrost::widget::text_input_controller::TextInputEmission;
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

#[test]
fn search_icon_buttons_keep_their_stable_ids_and_tooltip_labels() {
    let magnifier = super::search_trigger_button("needle".to_string());
    assert_eq!(
        magnifier.focus_id().as_str(),
        super::SEARCH_TRIGGER_FOCUS_ID
    );
    assert_eq!(
        magnifier.tooltip_text(),
        Some(strings::SEARCH_TRIGGER_TOOLTIP)
    );

    let clear = super::clear_trigger_button(false);
    assert_eq!(clear.focus_id().as_str(), super::SEARCH_CLEAR_FOCUS_ID);
    assert_eq!(clear.tooltip_text(), Some(strings::SEARCH_CLEAR_TOOLTIP));
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
    assert_eq!(view_model.query.value, "");
    assert_eq!(view_model.query.placeholder, strings::SEARCH_PLACEHOLDER);
    assert!(!view_model.query.clearable);
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
    assert_eq!(view_model.query.value, "crisp");
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
fn setting_the_search_query_rebuilds_its_non_clearable_field_model() {
    let (app, _dir) = app_with_system_lib();
    let mut view_model = LeftPanelViewModel::from_app(&app);

    view_model.set_query("overlay query");

    assert_eq!(view_model.query.value, "overlay query");
    assert_eq!(view_model.query.placeholder, strings::SEARCH_PLACEHOLDER);
    assert!(!view_model.query.clearable);
}

#[test]
fn controlled_search_emissions_keep_submit_blur_cancel_and_focus_distinct() {
    assert_eq!(
        search_field_emit(TextInputEmission::Changed(Live::new("live".to_string()))),
        Some(Message::Search(SearchMessage::QueryChanged {
            text: "live".to_string(),
        }))
    );
    assert_eq!(
        search_field_emit(TextInputEmission::Committed(Commit::new(
            "submitted".to_string()
        ))),
        Some(Message::Search(SearchMessage::Submitted {
            query: "submitted".to_string(),
        }))
    );
    assert_eq!(
        search_field_emit(TextInputEmission::FocusCommitted(Commit::new(
            "blurred".to_string()
        ))),
        Some(Message::Search(SearchMessage::QueryChanged {
            text: "blurred".to_string(),
        }))
    );
    assert_eq!(
        search_field_emit(TextInputEmission::Cancelled(Cancel::new(
            "restored".to_string()
        ))),
        Some(Message::Search(SearchMessage::QueryChanged {
            text: "restored".to_string(),
        }))
    );
    assert_eq!(
        search_field_emit(TextInputEmission::FocusChanged(true)),
        None
    );
    assert_eq!(
        search_field_emit(TextInputEmission::Cleared(Commit::new(String::new()))),
        None
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

/// The rule that replaced REQ-450's label tint.
///
/// `§3B.6:883`'s closing clause forbids carrying the `[PDF]`
/// distinction "in a danger-colored text label". The retired
/// `row_text_color` returned `palette.error` for PDF-backed files at
/// rest; that function is deleted, so the label resolves to the body
/// token for every row and the danger token reaches a row label nowhere.
///
/// **This test is the inverse of the one it replaced.** The retired test
/// `pdf_backed_rows_use_the_error_token_and_others_the_body_token`
/// asserted `row_text_color(&p, true) == p.error`. It is not kept and
/// flipped: the old assertion described behaviour the rule now prohibits,
/// so the whole test was rewritten around the new rule rather than
/// inverted to keep a green line.
#[test]
fn a_pdf_backed_row_label_never_uses_the_error_token() {
    let app = Palette::fastmd_dark();
    let pro = Palette::pro();

    // Rest and hover: the body token, because the band is not a locked
    // fill and `locked_over` passes the candidate through untouched.
    for hovered in [false, true] {
        assert_eq!(
            row_label_colour(false, hovered),
            app.text,
            "an unselected row labels with the body token (hovered = {hovered})"
        );
        assert_ne!(
            row_label_colour(false, hovered),
            app.error,
            "no unselected posture may render the danger token on a label \
             (hovered = {hovered})"
        );
    }

    // Selected: the lock promotes to text.strong, exactly as INT-7b left
    // it. The promotion is unchanged; what changed is that the candidate
    // handed to the lock is the body token rather than the danger token.
    for hovered in [false, true] {
        assert_eq!(
            row_label_colour(true, hovered),
            pro.text_strong,
            "the selection lock still promotes the label (hovered = {hovered})"
        );
        assert_ne!(
            row_label_colour(true, hovered),
            app.error,
            "the danger token is absent on the band too (hovered = {hovered})"
        );
    }

    // The retired pairing is still the unreadable one, so the rule this
    // test guards is still a real rule and not a tautology about a
    // palette that has since been fixed.
    let retired_pairing = hoarfrost::token::contrast::contrast_ratio(app.error, pro.accent_fill);
    assert!(
        retired_pairing < 3.0,
        "fixture: the retired danger-on-accent pairing must still be the \
         unreadable one, measured {retired_pairing:.6}:1"
    );
}

// --- INT-7b: framework token bindings, the 12px floor, row geometry ---
//
// These pins are deliberately written against the **framework** constants
// rather than against numbers, plus an `assert_ne!` against each literal
// this increment retired (INT-7a precedent). A future edit that restores
// a retired value fails a test that names the value, not a diff against a
// constant that would have moved with it.

use hoarfrost::composite::tree_row;
use hoarfrost::token::contrast::ChromeClass;
use hoarfrost::token::contrast::contrast_ratio;
use hoarfrost::token::{size, space, typography};
use hoarfrost::widget::badge_model::BadgeModel;
use hoarfrost::widget::text_input as pro_text_input;

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
    // **THE requirement this increment exists for.** §3B.6:883: the `[PDF]`
    // distinction "MUST NOT ride on badge text color (status.danger.text
    // on accent.fill is 1.59:1 and the lock forbids it). Render the badge
    // as a chip with BG: status.danger.fill and FG: text.strong (5.13:1),
    // so the chip fill itself carries the distinction and survives the
    // lock."
    //
    // INT-7b fixed the *contrast* of a selected PDF-backed row by
    // role-locking the label to `text.strong`, and in doing so destroyed
    // the *meaning*: once role-locked, a selected PDF-backed row was
    // pixel-identical to a selected ordinary row. The chip is the second
    // half of that fix, and it is built so that **no part of it is a
    // lock-driven text colour** — the mechanism the rule forbids is
    // structurally absent, not merely avoided.
    let pro = Palette::pro();
    let chip = pdf_badge_chip();
    let band = row_paint(true, false).band;

    // The fill is the mandated token, and it comes from the framework's
    // own tone resolver rather than a colour picked here (§4a).
    assert_eq!(
        chip.fill, pro.status_danger_fill,
        "the chip fill must be status.danger.fill"
    );
    assert_eq!(
        chip.fill,
        badge::resolve_tone(BadgeTone::Danger, &pro).solid_fill,
        "the fill must be sourced from badge::resolve_tone, not hand-picked"
    );

    // The foreground is the mandated token, and it is a constant — it does
    // not vary with posture, so it cannot be routed through a lock.
    assert_eq!(
        chip.foreground, pro.text_strong,
        "the chip foreground must be text.strong"
    );
    assert_eq!(
        chip.foreground,
        pdf_badge_chip().foreground,
        "the chip foreground must be posture-independent"
    );

    // The chip's own FG/BG pair is the pair the rule names, and it passes.
    assert!(
        meets_text_contrast(chip.foreground, chip.fill),
        "the chip's own FG/BG pair must clear the AA text floor: {}",
        contrast_ratio(chip.foreground, chip.fill),
    );

    // The chip never resolves to the band it sits on.
    assert_ne!(
        chip.fill, band,
        "a chip filled with its own background colour is not a chip"
    );

    // The chip's ink matches the locked label, so the chip never fights the
    // lock — but the *distinction* is the fill, not the ink. This is the
    // difference from the retired text-badge design, which made the ink do
    // work it could not do.
    assert_eq!(
        chip.foreground,
        row_label_colour(true, false),
        "the chip's text must match the locked label so the two cannot diverge"
    );

    // **And the honest half: on the selected band the fill does not carry
    // it.** Pinned in its own test below rather than glossed here.
    assert!(
        contrast_ratio(chip.fill, band) < contrast::MIN_COINCIDENT_FILL_CONTRAST,
        "see the_chip_fill_collapses_on_the_selected_band: the mandated fill is \
         not visible on accent.fill ({:.6}:1)",
        contrast_ratio(chip.fill, band),
    );
}

#[test]
fn the_chip_fill_collapses_on_the_selected_band_recorded_shortfall_not_a_pass() {
    // **THIS TEST RECORDS A FAILURE. IT IS NOT A PASSING RESULT.**
    //
    // §3B.6:883 says the chip fill "carries the distinction and survives
    // the lock". Measured from raw sRGB, it does not survive the lock:
    // `status.danger.fill` against `color.accent.fill` — the selected row's
    // own band — is **1.067676:1**. That is not a visible boundary. The
    // chip is built exactly as the spec mandates; the spec's stated
    // mechanism for why that works is simply false on the band the rule is
    // about.
    //
    // The spec author was shown this measurement and ruled: "if the spec
    // mandates it, then follow the spec." The chip therefore ships, and
    // this test exists so the shortfall cannot later be mistaken for a
    // resolved problem.
    //
    // **Read the assertions carefully: they assert the value is BELOW the
    // floor on purpose.** If a palette retune ever makes the chip visible
    // on the selection band, this test FAILS — and the correct response is
    // to come and delete this test, not to flip the comparison.
    let pro = Palette::pro();
    let chip = pdf_badge_chip();
    let floor = contrast::MIN_COINCIDENT_FILL_CONTRAST;
    let measured = contrast_ratio(chip.fill, pro.accent_fill);

    // The shortfall itself.
    assert!(
        measured < floor,
        "RECORDED SHORTFALL: chip fill vs selected band is {measured:.6}:1, \
         below the {floor:.2}:1 floor. If this now clears the floor, delete \
         this test — the shortfall has genuinely gone away."
    );
    assert!(
        (measured - 1.067676).abs() < 1e-5,
        "the recorded shortfall figure has moved: {measured:.6} vs 1.067676"
    );
    assert!(
        ((floor - measured) - 1.932324).abs() < 1e-5,
        "the shortfall is no longer 1.93 points: {:.6}",
        floor - measured
    );

    // The obligation is real and declarative: the solid pill classifies as
    // CoincidentWithFill, and that class demands 3.00:1.
    let pill = badge::resolve_style(&BadgeModel::count(1).with_tone(BadgeTone::Danger), &pro);
    assert_eq!(
        badge::badge_chrome_class(&pill, &pro),
        ChromeClass::CoincidentWithFill,
        "a zero-border solid pill is CoincidentWithFill, so the fill carries the boundary"
    );
    assert_eq!(
        ChromeClass::CoincidentWithFill.minimum_contrast(),
        Some(floor),
        "the class demands 3.00:1 against the mount"
    );

    // **And nothing enforces it here.** `gallery_audit::judge` returns true
    // unconditionally for every class except `Judged`, so the chip's
    // boundary against a selection band is never measured by any guard.
    // Asserted so that "the framework would catch it" is not believed.
    assert_eq!(
        ChromeClass::Judged.minimum_contrast(),
        Some(contrast::MIN_CHROME_CONTRAST),
        "only Judged pairs are gated; CoincidentWithFill is reported without a verdict"
    );
}

#[test]
fn every_chip_posture_is_measured_from_raw_srgb() {
    // Each posture the chip can appear in, measured rather than
    // transcribed. Nothing here is copied from the spec's rounded figures;
    // every number is recomputed from the palette's sRGB channels by
    // `contrast_ratio`.
    let pro = Palette::pro();
    let chip = pdf_badge_chip();
    let postures = [
        ("rest", false, false),
        ("hovered", false, true),
        ("selected", true, false),
        ("selected+hovered", true, true),
    ];

    for (name, selected, hovered) in postures {
        let band = row_paint(selected, hovered).band;
        // The chip is posture-independent by design: the fill carries the
        // mark, so nothing about the row changes the chip.
        assert_eq!(chip.fill, pro.status_danger_fill, "posture {name}");
        assert_eq!(chip.foreground, pro.text_strong, "posture {name}");
        // The chip's own FG/BG pair clears the AA text floor in every
        // posture, because it never changes.
        assert!(
            meets_text_contrast(chip.foreground, chip.fill),
            "chip FG/BG in posture {name}: {}",
            contrast_ratio(chip.foreground, chip.fill)
        );
        // The chip never paints the same colour as the band under it.
        assert_ne!(
            chip.fill, band,
            "posture {name}: chip invisible on its band"
        );
    }

    // Pinned figures, so a palette retune that quietly moves one of them
    // fails here instead of in a review screenshot.
    let fg_bg = contrast_ratio(chip.foreground, chip.fill);
    let rest_band = contrast_ratio(chip.fill, row_paint(false, false).band);
    let hover_band = contrast_ratio(chip.fill, row_paint(false, true).band);
    let selected_band = contrast_ratio(chip.fill, row_paint(true, false).band);
    let selected_hovered_band = contrast_ratio(chip.fill, row_paint(true, true).band);

    // The chip's own pair: the spec's 5.13:1, the atom header's 5.1270:1.
    assert!((fg_bg - 5.126966).abs() < 1e-5, "FG/BG = {fg_bg:.6}");
    // Visible at rest and on hover — both clear the 3.0 coincident floor.
    assert!(
        (rest_band - 3.880532).abs() < 1e-5,
        "fill vs base = {rest_band:.6}"
    );
    assert!(
        (hover_band - 3.166606).abs() < 1e-5,
        "fill vs hover = {hover_band:.6}"
    );
    assert!(rest_band >= contrast::MIN_COINCIDENT_FILL_CONTRAST);
    assert!(hover_band >= contrast::MIN_COINCIDENT_FILL_CONTRAST);
    // Collapsed on selection — the recorded shortfall, see
    // `the_chip_fill_collapses_on_the_selected_band`.
    assert!(
        (selected_band - 1.067676).abs() < 1e-5,
        "fill vs accent.fill = {selected_band:.6} (RECORDED SHORTFALL)"
    );
    assert!(
        (selected_hovered_band - selected_band).abs() < 1e-9,
        "selection outranks hover, so the band must not move: \
         {selected_hovered_band:.6} vs {selected_band:.6}"
    );

    // **The defect this design prevents, pinned as illegal.** Had the badge
    // carried `status.danger.text` as its own text colour rather than a
    // chip, it would paint this on the selection band.
    let danger_text_on_band = contrast_ratio(pro.status_danger_text, pro.accent_fill);
    assert!(
        danger_text_on_band < 2.0,
        "status.danger.text on accent.fill is {danger_text_on_band:.6}:1 — the \
         rule forbids this pairing, which is why the chip exists"
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
    // The chip must not push the row past its pinned rung, and the row's
    // interior must actually contain the chip's box.
    assert_eq!(ROW_MIN_HEIGHT, size::HIT_TARGET_MIN);
    let interior = ROW_MIN_HEIGHT - ROW_PADDING_Y - ROW_PADDING_Y;
    assert_eq!(interior, 20.0, "24 − 2×2");

    // The chip fits inside the row interior with 2px to spare, and its
    // height is the framework's own pill rung rather than a number picked
    // here.
    assert_eq!(PDF_BADGE_CHIP_HEIGHT, badge::COUNT_HEIGHT);
    assert!(
        PDF_BADGE_CHIP_HEIGHT <= interior,
        "the {}px chip must fit the {interior}px row interior",
        PDF_BADGE_CHIP_HEIGHT,
    );
    // The chip's own line box fits inside the chip.
    assert!(
        PDF_BADGE_RUNG.line_height <= PDF_BADGE_CHIP_HEIGHT,
        "the {}px line box must fit the {}px chip",
        PDF_BADGE_RUNG.line_height,
        PDF_BADGE_CHIP_HEIGHT,
    );
    // The badge is typed on the badge atom's own rung, not a size picked
    // here, and that rung is at or above the interface-wide type floor.
    assert_eq!(PDF_BADGE_RUNG.size, badge::TYPE_RUNG.size);
    assert!(PDF_BADGE_RUNG.size >= typography::MIN_SIZE);

    // **The slot IS the chip's box**, composed from the chip's own metrics
    // rather than a figure that merely happens to be large enough.
    assert_eq!(
        PDF_BADGE_CHIP_PADDING_TOTAL,
        PDF_BADGE_CHIP_H_PADDING + PDF_BADGE_CHIP_H_PADDING,
        "both side paddings, taken together"
    );
    assert_eq!(
        PDF_BADGE_SLOT,
        PDF_BADGE_TEXT_ALLOWANCE + PDF_BADGE_CHIP_PADDING_TOTAL,
        "slot = text allowance + both side paddings"
    );
    assert_eq!(PDF_BADGE_SLOT, 36.0, "20 + 8 + 8");
    assert_eq!(PDF_BADGE_CHIP_H_PADDING, badge::CHIP_H_PADDING);
    assert_eq!(PDF_BADGE_CHIP_RADIUS, shape::RADIUS_PILL);
    assert_eq!(PDF_BADGE_CHARGE, PDF_BADGE_SLOT + PDF_BADGE_GAP);
    // The chip is narrower than the row's full width at any pane the app
    // allows, so it can never crowd the row out.
    assert!(PDF_BADGE_SLOT < LEFT_PANEL_MIN_WIDTH);
}

#[test]
fn the_badge_charge_cannot_breach_the_label_floor_at_the_minimum_pane() {
    // The chip has a real box, so it charges the row for real width. At the
    // spec's minimum pane the row cannot afford both the chip and the
    // `TREE_LABEL_MIN_W = 96px` floor, and `pdf_badge_fits` must be the
    // thing that decides — not a hope that the numbers work out.
    //
    // `LEFT_PANE_MIN_W = 150` is the spec's own figure (§4.3.1). The app's
    // `LEFT_PANEL_MIN_WIDTH` is 180, i.e. 30px *wider*, so the spec's
    // worst case is checked here and the app's own minimum is safer.
    let spec_min_pane = 150.0;
    let row_width = spec_min_pane - PANEL_PADDING - PANEL_PADDING;
    assert_eq!(row_width, 142.0);

    let allowance = tree_row::indent_allowance(row_width);
    let drawn = tree_row::indent_geometry(8, row_width).width;
    assert_eq!(allowance, 14.0, "142 − 128");
    assert_eq!(
        drawn, allowance,
        "deep rows compress onto the whole allowance"
    );

    // The chip cannot be afforded here, and the gate says so.
    assert!(
        !pdf_badge_fits(8, row_width),
        "allowance − drawn = {:.1}, charge = {PDF_BADGE_CHARGE}: the gate must \
         suppress the chip at the minimum pane",
        allowance - drawn,
    );

    // With the chip suppressed the label keeps its full compressed width.
    let label = row_width - ROW_PADDING_X - ROW_PADDING_X - drawn;
    assert_eq!(label, 120.0);
    assert!(
        label >= tree_row::MIN_LABEL_WIDTH,
        "{label} must clear the {}px floor",
        tree_row::MIN_LABEL_WIDTH,
    );

    // **The gate is load-bearing, not decorative.** If the charge were
    // forced on at this width the floor would breach — which is exactly the
    // outcome `pdf_badge_fits` exists to prevent.
    let forced = label - PDF_BADGE_CHARGE;
    assert!(
        forced < tree_row::MIN_LABEL_WIDTH,
        "forcing the chip in would leave {forced:.1}px, below the floor; the \
         gate is what stops that"
    );
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
        row_label_colour(true, false),
        pro.text_strong,
        "§3B.6 locks the row label to text.strong on the accent fill by role"
    );
    assert_ne!(
        row_label_colour(true, false),
        app_palette.error,
        "a selected PDF-backed row must not render red-on-indigo"
    );
    let locked_contrast = contrast_ratio(row_label_colour(true, false), pro.accent_fill);
    assert!(
        locked_contrast >= 4.5,
        "the locked label clears the text-contrast rule at {locked_contrast}:1"
    );
}

/// The rule this replaces asserted the opposite: that an unselected
/// PDF-backed row kept its `error` tint and that the tint was the visible
/// PDF cue. `§3B.6:883`'s closing clause forbids that, so the resting
/// label is now the body token for every row and the chip is the only
/// mark. The test is rewritten around the new rule rather than flipped.
#[test]
fn an_unselected_pdf_backed_row_is_labelled_exactly_like_an_ordinary_row() {
    let app_palette = Palette::fastmd_dark();
    assert_eq!(
        row_label_colour(false, false),
        app_palette.text,
        "the resting label is the body token, PDF-backed or not"
    );
    assert_ne!(
        row_label_colour(false, false),
        app_palette.error,
        "the retired danger tint must not be the resting label colour"
    );
    // The retired rule made a PDF-backed row's resting label differ from
    // an ordinary row's. That difference is gone by construction: the
    // resolver takes no PDF-backed input, so there is no second colour
    // for it to return.
    let resting = row_label_colour(false, false);
    let resting_ratio = contrast_ratio(resting, row_paint(false, false).band);
    assert!(
        resting_ratio >= 4.5,
        "and the body token still clears the text floor on the resting band \
         at {resting_ratio:.6}:1"
    );
}

/// Every posture, swept against every danger token the palette offers.
///
/// `§3B.6:883` forbids carrying the `[PDF]` distinction "in a
/// danger-colored text label". The label resolver no longer takes a
/// PDF-backed input at all, so this is a sweep over the whole input space
/// of the function rather than a spot check: four postures × three danger
/// tokens, none of which may appear.
///
/// Non-vacuity: the fixture pins that the app's `error` token **is** the
/// framework's `status.danger.text` — the exact token `:883` names — and
/// that the danger fill is a distinct value, so the sweep is not
/// comparing one colour against itself three times.
#[test]
fn no_posture_resolves_a_row_label_to_any_danger_token() {
    let app = Palette::fastmd_dark();
    let pro = Palette::pro();

    // Fixture: the app's `error` token IS the framework's
    // `status.danger.text` (`theme.rs:202` maps it directly), which is
    // the very token `§3B.6:883` names as the forbidden label colour.
    // The fill is a different colour, so the sweep below is not sweeping
    // one value three times.
    assert_eq!(
        app.error, pro.status_danger_text,
        "fixture: the app's error token is the framework's status.danger.text, \
         the exact token :883 forbids on a label"
    );
    assert_ne!(
        pro.status_danger_text, pro.status_danger_fill,
        "fixture: danger text and danger fill are distinct"
    );

    let danger_tokens = [
        ("app.error", app.error),
        ("status.danger.text", pro.status_danger_text),
        ("status.danger.fill", pro.status_danger_fill),
    ];

    for selected in [false, true] {
        for hovered in [false, true] {
            let resolved = row_label_colour(selected, hovered);
            for (name, danger) in danger_tokens.iter() {
                assert_ne!(
                    resolved, *danger,
                    "a row label must never resolve to {name} \
                     (selected = {selected}, hovered = {hovered})"
                );
            }
        }
    }
}

/// The selection lock is **load-bearing for the body token too**, and this
/// increment did not change that.
///
/// Before INT-15 the candidate handed to `locked_over` on the selection
/// band was `error`, measuring **1.586348:1** raw. It is now the body
/// token, which measures **3.176079:1** raw — better, but still under the
/// `4.5` text floor. Only the role-based lock lifts it to `text.strong`
/// at **4.801987:1**. So the lock is not a leftover from the retired
/// tint: strip the lock and the resting token would still fail the text
/// rule on the band, just less badly than the danger token did. Removing
/// the tint therefore introduced no new bad pairing, and it did not make
/// the lock optional.
#[test]
fn the_lock_is_load_bearing_for_the_body_token_not_only_the_retired_tint() {
    let app = Palette::fastmd_dark();
    let pro = Palette::pro();

    let retired_pairing = contrast_ratio(app.error, pro.accent_fill);
    let body_raw = contrast_ratio(app.text, pro.accent_fill);
    let locked = contrast_ratio(row_label_colour(true, false), pro.accent_fill);

    assert!(
        retired_pairing < 3.0,
        "fixture: the retired danger-on-accent pairing is still the \
         unreadable one at {retired_pairing:.6}:1"
    );
    assert!(
        body_raw > retired_pairing,
        "the body token is strictly better than the retired pairing \
         ({body_raw:.6} > {retired_pairing:.6})"
    );
    assert!(
        body_raw < 4.5,
        "but the body token raw on the band is {body_raw:.6}:1, still under \
         the 4.5 text floor — the lock is required, not vestigial"
    );
    assert!(
        locked >= 4.5,
        "and the lock lifts the painted label to {locked:.6}:1"
    );
}

/// The chip is the sole carrier and INT-13's build is untouched.
///
/// Removing the label tint must not disturb the mark that replaced it, so
/// every field of the resolved chip is pinned to the value INT-13 landed,
/// and the chip's ink is still the locked label colour.
#[test]
fn the_chip_is_the_sole_pdf_carrier_and_is_unchanged() {
    let pro = Palette::pro();
    let chip = pdf_badge_chip();

    assert_eq!(chip.fill, pro.status_danger_fill, "the mandated fill");
    assert_eq!(
        chip.foreground, pro.text_strong,
        "the mandated foreground, constant in every posture"
    );
    assert_eq!(chip.height, badge::COUNT_HEIGHT, "18px count-pill rung");
    assert_eq!(chip.radius, shape::RADIUS_PILL, "pill radius");
    assert_eq!(chip.h_padding, badge::CHIP_H_PADDING, "space.sm h-padding");

    // The chip's own FG/BG pair is genuinely compliant, measured here
    // rather than transcribed.
    let chip_pair = contrast_ratio(chip.foreground, chip.fill);
    assert!(
        chip_pair >= 4.5,
        "text.strong on status.danger.fill clears AA at {chip_pair:.6}:1"
    );

    // And the chip is still mounted from the same slot the row charges for.
    assert_eq!(
        PDF_BADGE_SLOT,
        PDF_BADGE_TEXT_ALLOWANCE + PDF_BADGE_CHIP_PADDING_TOTAL,
        "charged width equals painted width by construction"
    );
}

/// **THE SUPPRESSION HOLE — characterised, not fixed.**
///
/// With the label tint gone the chip is the **only** carrier of the
/// PDF-backed distinction, and the chip is suppressed wherever mounting
/// it would breach the `96px` label floor ([`pdf_badge_fits`]). In that
/// posture a PDF-backed row is **visually identical to an ordinary row**:
/// same label colour, same band, no mark.
///
/// The condition is exact. The chip costs [`PDF_BADGE_CHARGE`] (`40px`)
/// and the indent solver leaves only `allowance - drawn` of headroom,
/// which is **`0` for every compressed or collapsed row**. The chip
/// therefore shows iff `row_width >= 168 + 16 × depth`, i.e.
/// `pane >= 176 + 16 × depth`:
///
/// | Pane | Chip shows at depths |
/// |---|---|
/// | 150 — spec `LEFT_PANE_MIN_W` | **none**, not even depth 0 |
/// | 176 | 0 only |
/// | 180 — app `LEFT_PANEL_MIN_WIDTH` | 0 only |
/// | 280 — app default | 0–6 |
///
/// **This is the cost of `§3B.6:883`, not an oversight.** The spec's own
/// fallback for a row that cannot mount a chip is the leading file-type
/// **icon shape** (`UIF-PRIN-005` redundant cueing), and this tree has no
/// file-type icon slot — the leading glyph is the dir/expanded state baked
/// into the measured label. Whether to build one is an open decision for
/// the spec author, recorded in the ledger; it is deliberately not
/// guessed at here.
#[test]
fn a_pdf_backed_row_can_end_up_with_no_visible_distinction_at_all() {
    let row_width_of = |pane: f32| (pane - PANEL_PADDING - PANEL_PADDING).max(0.0);

    // --- the exact boundary: pane = 176 + 16 × depth -----------------
    for depth in 0..=12usize {
        let fitting_pane = 176.0 + 16.0 * depth as f32;
        assert!(
            pdf_badge_fits(depth, row_width_of(fitting_pane)),
            "depth {depth} must fit at the boundary pane {fitting_pane}"
        );
        assert!(
            !pdf_badge_fits(depth, row_width_of(fitting_pane - 1.0)),
            "depth {depth} must NOT fit one point narrower than its boundary"
        );
    }

    // --- the two widths that matter ----------------------------------
    // Spec minimum: nothing shows, at any depth.
    let spec_min = row_width_of(150.0);
    let shown_at_spec_min: Vec<usize> = (0..=64usize)
        .filter(|d| pdf_badge_fits(*d, spec_min))
        .collect();
    assert!(
        shown_at_spec_min.is_empty(),
        "at the spec's LEFT_PANE_MIN_W = 150 (row_width {spec_min}) NO depth \
         shows the chip, so every PDF-backed row there is undistinct: {shown_at_spec_min:?}"
    );

    // App minimum: only depth 0 shows; every nested PDF-backed file is undistinct.
    let app_min = row_width_of(LEFT_PANEL_MIN_WIDTH);
    let shown_at_app_min: Vec<usize> = (0..=64usize)
        .filter(|d| pdf_badge_fits(*d, app_min))
        .collect();
    assert_eq!(
        shown_at_app_min,
        vec![0],
        "at the app's own LEFT_PANEL_MIN_WIDTH = {LEFT_PANEL_MIN_WIDTH} \
         (row_width {app_min}) only depth 0 shows the chip; every nested \
         PDF-backed file is visually identical to an ordinary one"
    );

    // App default: 0..=6 show, 7 does not.
    let app_default = row_width_of(LEFT_PANEL_DEFAULT_WIDTH);
    let shown_at_default: Vec<usize> = (0..=64usize)
        .filter(|d| pdf_badge_fits(*d, app_default))
        .collect();
    assert_eq!(
        shown_at_default,
        vec![0, 1, 2, 3, 4, 5, 6],
        "at the app's default 280 pane the chip reaches depth 6 and stops"
    );

    // --- and in every suppressed posture there is genuinely nothing ---
    // The label resolves to the ordinary row's colour in all four
    // postures, so a suppressed-chip PDF row and an ordinary row share
    // every painted pixel. Asserted against the tokens rather than
    // against itself: rest/hover are the body token, selected is the
    // lock's text.strong.
    let ordinary = [
        (false, false, Palette::fastmd_dark().text),
        (false, true, Palette::fastmd_dark().text),
        (true, false, Palette::pro().text_strong),
        (true, true, Palette::pro().text_strong),
    ];
    for depth in 1..=12usize {
        for (selected, hovered, expected) in ordinary {
            assert!(
                !pdf_badge_fits(depth, app_min),
                "fixture: depth {depth} is suppressed at the app minimum"
            );
            assert_eq!(
                row_label_colour(selected, hovered),
                expected,
                "a suppressed-chip PDF row at depth {depth} paints exactly \
                 what an ordinary row paints (selected = {selected}, \
                 hovered = {hovered}) — no distinction of any kind survives"
            );
        }
    }
}

/// The auto-fit does not pay for the chip, so it cannot rescue it.
///
/// `UIF-PANE-002` widens the pane to fit the longest row, which would
/// normally remove the width conflict. It does not: `auto_fit_width`
/// charges `tree_row::content_width` — indent plus label plus furniture —
/// and has **no badge term**, so a pane auto-fitted to a deep PDF-backed
/// row still suppresses that row's chip.
#[test]
fn auto_fit_does_not_widen_the_pane_to_keep_the_chip() {
    let measure = |label: &str| label.len() as f32 * 7.0;
    let deep_pdf_row = FlatRow {
        depth: 8,
        name: "deep.md".to_string(),
        path: PathBuf::from("a/b/c/d/e/f/g/h/deep.md"),
        is_dir: false,
        is_expanded: false,
    };
    let rows = [deep_pdf_row];

    let fitted = auto_fit_width(&rows, 10_000.0, measure);
    let row_width = (fitted.max(LEFT_PANEL_MIN_WIDTH) - PANEL_PADDING - PANEL_PADDING).max(0.0);
    assert!(
        !pdf_badge_fits(8, row_width),
        "a pane auto-fitted to a depth-8 row of {} chars still cannot mount \
         the chip: fitted {fitted}, row_width {row_width}, allowance-minus-drawn \
         {:.1} < charge {PDF_BADGE_CHARGE}",
        "deep.md".len(),
        tree_row::indent_allowance(row_width) - tree_row::indent_geometry(8, row_width).width,
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
        !hoarfrost::token::contrast::fill_lock_role(pro.surface_hover, &pro).is_locked_fill(),
        "surface_hover must not be a locked fill, or hover would repaint \
         every label white"
    );
    assert!(
        hoarfrost::token::contrast::fill_lock_role(pro.accent_fill, &pro).is_locked_fill(),
        "while the selection band still is, so the difference between the \
         two cases is real and not an accident of one lookup"
    );

    // Consequence: the resting label rule survives the pointer. It no
    // longer carries a PDF tint, so what survives is the body token.
    assert_eq!(
        row_label_colour(false, true),
        app_palette.text,
        "a hovered plain row keeps the body token"
    );
    assert_eq!(
        row_label_colour(false, true),
        row_label_colour(false, false),
        "hover must not change the label colour at all, only the band under it"
    );
    assert_ne!(
        row_label_colour(false, true),
        app_palette.error,
        "and the resting colour hover preserves is not the danger token"
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
    let colour = row_label_colour(true, true);
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
        hoarfrost::token::shape::BORDER_FOCUS,
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
    use hoarfrost::token::contrast::MIN_CHROME_CONTRAST;

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
