//! Click-handler functions for tree rows: file-click, dir-click, and multi-file merge.

use super::context::{TreeNodeContext, TreeOpsContext};
use super::flatten::FlatRow;
use std::collections::HashSet;
use std::path::PathBuf;

/// Purpose: Applies the side effect of clicking a file row in the
/// left panel's tree view.
/// Inputs: ctx (the `TreeNodeContext`; its `selected_file`,
/// `selected_files`, and `tabs` are mutated), row (the clicked
/// `FlatRow`)
/// Outputs: ()
/// Purity: Impure (mutates the selection and tab state).
/// Preconditions: `row.is_dir` should be `false` (caller filters
/// files vs directories). Modifiers are taken from `ctx.modifiers()`
/// at the moment of the call.
/// Postconditions:
///   * With any of shift/ctrl/command held: toggles the row's
///     inclusion in `ctx.selected_files()`. If it was already in
///     the set, it is removed and `ctx.selected_file()` is cleared
///     if it pointed at this path. If it was not, it is added and
///     `ctx.selected_file()` is set to it.
///   * With no modifier: replaces the selection with this single
///     file and pushes it onto `ctx.tabs()` if not already open.
///   * **Always:** updates `ctx.selected_dir()` (the "current
///     directory context" used by the bottom-panel prompt prefix
///     and by the agent session dispatch) to the file's
///     containing directory — `row.path.parent()`. If the file
///     has no parent component, `selected_dir` is cleared to
///     `None`. This runs in both the modifier and no-modifier
///     branches: the user is operating in the clicked file's
///     directory regardless of how they selected it.
///
/// Takes `&mut TreeNodeContext` rather than three separate `&mut`
/// fields because Rust's borrow checker treats three calls to
/// `ctx.selected_file()` / `ctx.selected_files()` / `ctx.tabs()` as
/// overlapping re-borrows of `*ctx` (since the accessors all return
/// `&mut` to disjoint sub-fields of the same struct, but the
/// compiler does not see through the accessor boundary). The
/// function extracts the three sub-references via split-borrow
/// inside its own body.
///
/// The file-row click in `render_flat_row` calls this function.
/// It is extracted so the modifier logic can be unit-tested
/// without driving the egui harness. The directory-row click
/// path is unchanged — it has a different effect (toggle
/// expansion + set `selected_dir`).
pub fn apply_file_row_click(ctx: &mut TreeNodeContext<'_>, row: &FlatRow) {
    let modifiers = ctx.modifiers();
    // Use split borrows through the flat struct fields rather than
    // calling the accessor methods three times: the accessors are
    // `&mut self` methods, but the borrow checker cannot prove
    // disjointness through method boundaries. Direct field access
    // on the flat `TreeOpsContext` lets the compiler split-borrow.
    let TreeOpsContext {
        selected_dir,
        selected_file,
        selected_files,
        tabs,
        ..
    } = ctx;
    if modifiers.shift || modifiers.ctrl || modifiers.command {
        if selected_files.contains(&row.path) {
            selected_files.remove(&row.path);
            if selected_file.as_ref() == Some(&row.path) {
                **selected_file = None;
            }
        } else {
            selected_files.insert(row.path.clone());
            **selected_file = Some(row.path.clone());
        }
    } else {
        selected_files.clear();
        selected_files.insert(row.path.clone());
        **selected_file = Some(row.path.clone());
        if !tabs.contains(&row.path) {
            tabs.push(row.path.clone());
        }
    }
    // Always refresh the current directory context to the file's
    // containing directory. The bottom-panel prompt prefix and
    // the agent session dispatch both read `selected_dir`, so a
    // stale value would mislead both surfaces after a file click.
    // `Path::parent` returns `None` for bare filenames (no parent
    // component), which is the right neutral state for the bottom
    // panel's `>` prefix.
    **selected_dir = row.path.parent().map(|p| p.to_path_buf());
}

/// Purpose: Applies the side effect of clicking a directory row in
/// the left panel's tree view.
/// Inputs: ctx (the `TreeNodeContext`; its `expanded_dirs` and
/// `selected_dir` are mutated), row (the clicked `FlatRow`;
/// `row.is_dir` is `true`).
/// Outputs: ()
/// Purity: Impure (mutates the tree's expand/collapse state and the
/// current-directory context).
/// Preconditions: `row.is_dir` is `true`.
/// Postconditions:
///   * Toggles `ctx.expanded_dirs()` for `row.path` — adds it if
///     the folder was collapsed, removes it if it was already
///     expanded.
///   * Updates `ctx.selected_dir()` to `Some(row.path.clone())`
///     so the bottom-panel prompt prefix and the agent session
///     reflect the folder the user just browsed to.
///   * **Does NOT touch** `ctx.selected_file()`, `ctx.selected_files()`,
///     or `ctx.tabs()`. Expanding/collapsing a folder is a
///     tree-navigation action, orthogonal to which file is open
///     in the editor. Clearing the file selection here would
///     hide the center panel body (the file header, YAML table,
///     and rendered markdown inside `ScrollArea`, all guarded by
///     `if let Some(selected_path) = app.selection().selected_file()`)
///     and the right (TOC) panel (`should_show_panel` requires a
///     selected file), even though `tab_manager.current_markdown`
///     and `loaded_path` are still set. The user would have to
///     click the file again to restore the preview. The unit
///     test `test_apply_directory_row_click_preserves_selected_file`
///     pins this invariant.
///
/// The directory-row click in `render_flat_row` and
/// `draw_tree_node` (legacy recursive path) calls this function.
/// It is extracted so the state mutation can be unit-tested
/// without driving the egui harness, mirroring the
/// `apply_file_row_click` pattern.
pub fn apply_directory_row_click(ctx: &mut TreeNodeContext<'_>, row: &FlatRow) {
    // Split-borrow through the flat struct fields rather than
    // calling the accessor methods: direct field access on the flat
    // `TreeOpsContext` lets the compiler prove disjointness without
    // seeing through method boundaries.
    let TreeOpsContext {
        selected_dir,
        expanded_dirs,
        ..
    } = ctx;
    if expanded_dirs.contains(&row.path) {
        expanded_dirs.remove(&row.path);
    } else {
        expanded_dirs.insert(row.path.clone());
    }
    **selected_dir = Some(row.path.clone());
}

pub fn build_merge_prompt(
    content_libraries: &[crate::config::ContentLibrary],
    selected_files: &HashSet<PathBuf>,
) -> String {
    let mut prompt = "Please read each of the following documents using the read_file tool and merge their content into a new document. Consolidate overlapping content, deduplicate repeated information, and produce a single unified document that combines all of the source material:\n".to_string();
    let mut sorted_files: Vec<&PathBuf> = selected_files.iter().collect();
    sorted_files.sort();
    for file in sorted_files {
        let rel_str = crate::config::library_display_label(content_libraries, file)
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        prompt.push_str(&format!("- {}\n", rel_str));
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::panel_layout::PanelLayout;
    use crate::ui::tree::context::TreeNodeContext;
    use eframe::egui;
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// Tier 1 test: a file row click with no modifier replaces
    /// the single selection with the clicked file and pushes it
    /// onto `tabs` if not already there.
    #[test]
    fn test_apply_file_row_click_no_modifier_replaces_selection_and_opens_tab() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("a.md")];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut None,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(selected_file, Some(PathBuf::from("b.md")));
        assert!(selected_files.contains(&PathBuf::from("b.md")));
        assert_eq!(
            selected_files.len(),
            1,
            "previous selection must be cleared"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            "clicked file must be pushed onto tabs"
        );
    }

    /// Tier 1 test: a file row click with shift held toggles the
    /// row's membership in `selected_files`. Toggling an
    /// already-selected file removes it and clears `selected_file`
    /// if it pointed at that file.
    #[test]
    fn test_apply_file_row_click_shift_toggles_off() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file = Some(PathBuf::from("b.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("b.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut None,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert!(
            !selected_files.contains(&PathBuf::from("b.md")),
            "shift-click on a selected file must remove it from selected_files"
        );
        assert!(
            selected_file.is_none(),
            "selected_file must be cleared when the toggled-off file was the selected one"
        );
    }

    /// Tier 1 test: shift-clicking a file that is NOT in
    /// `selected_files` adds it to the set and makes it the
    /// `selected_file` without touching `tabs` (multi-select does
    /// not auto-open tabs).
    #[test]
    fn test_apply_file_row_click_shift_adds_to_selection_without_opening_tab() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut None,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert!(selected_files.contains(&PathBuf::from("b.md")));
        assert_eq!(
            selected_file,
            Some(PathBuf::from("b.md")),
            "shift-click must set selected_file to the clicked file"
        );
        assert!(
            tabs.is_empty(),
            "shift-click must NOT auto-open the clicked file as a tab (multi-select mode)"
        );
    }

    /// Tier 1 test: clicking a file that is already open in a tab
    /// does NOT push a duplicate. Tab list is the unique set of
    /// open paths.
    #[test]
    fn test_apply_file_row_click_no_duplicate_tab() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("a.md")];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "a.md".to_string(),
            path: PathBuf::from("a.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut None,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            tabs,
            vec![PathBuf::from("a.md")],
            "clicking an already-open tab must not push a duplicate"
        );
    }

    /// TDD regression: clicking a file row in the left directory
    /// tree must update `selected_dir` (the "current directory
    /// context" used by the bottom-panel prompt prefix and the
    /// agent session) to the file's containing directory.
    ///
    /// Before the fix, `apply_file_row_click` only updated
    /// `selected_file` / `selected_files` / `tabs` — `selected_dir`
    /// kept whatever value the previous directory click (or app
    /// start) had set, so the bottom panel would keep showing a
    /// stale directory prefix and the agent would receive the
    /// wrong context once the user opened a file.
    #[test]
    fn test_apply_file_row_click_updates_selected_dir_to_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        // Pre-existing stale value to prove the click overwrites it.
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/old/dir"));
        let file_path = PathBuf::from("C:/notes/folder/file.md");
        let expected_parent = Some(PathBuf::from("C:/notes/folder"));
        let row = FlatRow {
            depth: 1,
            name: "file.md".to_string(),
            path: file_path.clone(),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut selected_dir,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            selected_dir, expected_parent,
            "clicking a file row must update selected_dir to the file's containing directory"
        );
    }

    /// TDD regression: even with a multi-select modifier (shift),
    /// clicking a file row must still update `selected_dir` to
    /// the file's containing directory. The user is operating
    /// in that directory and the bottom-panel prefix / agent
    /// context should reflect it.
    #[test]
    fn test_apply_file_row_click_shift_updates_selected_dir_to_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let mut selected_dir: Option<PathBuf> = None;
        let file_path = PathBuf::from("C:/notes/folder/file.md");
        let expected_parent = Some(PathBuf::from("C:/notes/folder"));
        let row = FlatRow {
            depth: 1,
            name: "file.md".to_string(),
            path: file_path.clone(),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut selected_dir,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            selected_dir, expected_parent,
            "shift-clicking a file row must also update selected_dir to the file's containing directory"
        );
    }

    /// Edge case: a file with no parent component (a bare
    /// filename like `file.md`) must refresh `selected_dir` away
    /// from any stale prior value. `Path::parent("file.md")`
    /// returns `Some(Path::new(""))` (an empty path), not `None`,
    /// because the OS-level "containing directory" of a bare
    /// filename is the empty path. The downstream
    /// `compute_prompt_prefix` already handles this case — an
    /// empty path falls through to its `is_empty()` branch and
    /// renders the bare `">"` prefix, matching the `None` case.
    #[test]
    fn test_apply_file_row_click_bare_filename_sets_empty_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/stale/dir"));
        let row = FlatRow {
            depth: 0,
            name: "file.md".to_string(),
            path: PathBuf::from("file.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut selected_dir,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_file_row_click(&mut ctx, &row);

        // `Path::parent("file.md")` is `Some(Path::new(""))`,
        // not `None`. Verify the click refreshes the stale
        // value to that canonical empty-parent form, and that
        // the resulting bottom-panel prefix renders as the bare
        // ">" (same surface as `selected_dir == None`).
        assert_eq!(
            selected_dir,
            Some(PathBuf::new()),
            "clicking a bare-filename row must set selected_dir to Some(Path::new(\"\"))"
        );
        let prefix = crate::ui::panels::bottom::compute_prompt_prefix(selected_dir.as_deref(), &[]);
        assert_eq!(
            prefix, ">",
            "an empty-path selected_dir must render as the bare `>` prefix in the bottom panel"
        );
    }

    /// TDD regression: clicking a directory row in the left
    /// directory tree must NOT clear the currently selected file
    /// or the multi-selection set.
    ///
    /// **Why this matters.** `render_tabs_and_content` in the
    /// center panel guards its body on
    /// `if let Some(selected_path) = app.selection().selected_file()`.
    /// If a directory click cleared `selected_file`, the body —
    /// the file's header, the YAML front-matter table, and the
    /// rendered markdown inside its `ScrollArea` — would be
    /// skipped on the next frame. The tab strip would still be
    /// visible, but the preview area would go blank. The right
    /// (TOC) panel would also disappear, because
    /// `should_show_panel(has_toc, has_selected_file)` requires
    /// a selected file. The user would have to click the file
    /// again to restore the preview, even though
    /// `tab_manager.current_markdown` / `current_yaml` /
    /// `loaded_path` were never touched.
    ///
    /// **The bug.** The directory-click branch in
    /// `render_flat_row` and `draw_tree_node` (legacy) used to
    /// unconditionally run `*ctx.selected_file() = None` and
    /// `ctx.selected_files().clear()`, conflating "expand this
    /// folder" with "deselect the open file." The two helpers
    /// now route through `apply_directory_row_click`, which
    /// only toggles `expanded_dirs` and refreshes `selected_dir`.
    ///
    /// **The contract pinned by this test.** After
    /// `apply_directory_row_click`:
    ///   * `selected_file` is unchanged.
    ///   * `selected_files` is unchanged.
    ///   * `tabs` is unchanged.
    ///   * `expanded_dirs` is toggled for `row.path`.
    ///   * `selected_dir` is set to `Some(row.path.clone())`
    ///     (the "current directory context" used by the
    ///     bottom-panel prompt prefix and the agent session).
    #[test]
    fn test_apply_directory_row_click_preserves_selected_file() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("doc.md")];
        let mut selected_file: Option<PathBuf> = Some(PathBuf::from("doc.md"));
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        selected_files.insert(PathBuf::from("doc.md"));
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        // Pre-existing stale value to prove the click overwrites it
        // (mirrors the `apply_file_row_click` `selected_dir` test).
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/old/dir"));
        let dir_path = PathBuf::from("C:/notes/folder");
        let row = FlatRow {
            depth: 0,
            name: "folder".to_string(),
            path: dir_path.clone(),
            is_dir: true,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut selected_dir,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_directory_row_click(&mut ctx, &row);

        // The contract: file selection and tabs are preserved.
        assert_eq!(
            selected_file,
            Some(PathBuf::from("doc.md")),
            "directory row click must NOT clear selected_file; clearing it \
             hides the center panel body and the right (TOC) panel"
        );
        assert!(
            selected_files.contains(&PathBuf::from("doc.md")),
            "directory row click must NOT clear selected_files"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("doc.md")],
            "directory row click must NOT touch the open tabs"
        );
        // The actual purpose: expand the folder and refresh the
        // current-directory context.
        assert!(
            expanded_dirs.contains(&dir_path),
            "directory row click must add the folder to expanded_dirs"
        );
        assert_eq!(
            selected_dir,
            Some(dir_path.clone()),
            "directory row click must update selected_dir to the folder's path"
        );
    }

    /// TDD regression (companion to
    /// `test_apply_directory_row_click_preserves_selected_file`):
    /// the second click on an already-expanded directory must
    /// collapse it. The same invariant holds — the open file
    /// selection and the open tabs are NOT touched.
    ///
    /// This is a separate test rather than a follow-up call in
    /// the previous test, because the borrow checker treats two
    /// sequential `&mut ctx` calls as overlapping re-borrows of
    /// `ctx`'s inner fields; splitting the test lets each
    /// assertion set live independently of the next call.
    #[test]
    fn test_apply_directory_row_click_collapses_expanded_folder_preserves_selection() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("doc.md")];
        let mut selected_file: Option<PathBuf> = Some(PathBuf::from("doc.md"));
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        selected_files.insert(PathBuf::from("doc.md"));
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let dir_path = PathBuf::from("C:/notes/folder");
        expanded_dirs.insert(dir_path.clone());
        let mut selected_dir: Option<PathBuf> = Some(dir_path.clone());
        let row = FlatRow {
            depth: 0,
            name: "folder".to_string(),
            path: dir_path.clone(),
            is_dir: true,
            is_expanded: true,
        };
        let mut ctx = TreeNodeContext {
            selected_file: &mut selected_file,
            selected_files: &mut selected_files,
            expanded_dirs: &mut expanded_dirs,
            tabs: &mut tabs,
            selected_dir: &mut selected_dir,
            create_dir_dialog_open: &mut false,
            create_dir_parent: &mut None,
            file_to_move: &mut None,
            move_dialog_open: &mut false,
            file_to_rename: &mut None,
            rename_dialog_open: &mut false,
            rename_new_name: &mut String::new(),
            create_document_dialog_open: &mut false,
            create_document_parent: &mut None,
            layout: &mut PanelLayout::default(),
            submit_prompt: &mut None,
            content_libraries: &[],
            open_editor: &mut None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
        };

        apply_directory_row_click(&mut ctx, &row);

        // Collapse: the folder is removed from `expanded_dirs`.
        assert!(
            !expanded_dirs.contains(&dir_path),
            "clicking an already-expanded directory must collapse it"
        );
        // Same invariant as the expand test: file selection and
        // tabs are untouched.
        assert_eq!(
            selected_file,
            Some(PathBuf::from("doc.md")),
            "collapsing a directory must NOT clear selected_file"
        );
        assert!(
            selected_files.contains(&PathBuf::from("doc.md")),
            "collapsing a directory must NOT clear selected_files"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("doc.md")],
            "collapsing a directory must NOT touch the open tabs"
        );
        // `selected_dir` is refreshed to the directory's path
        // regardless of whether the click expanded or collapsed it.
        assert_eq!(
            selected_dir,
            Some(dir_path),
            "collapsing a directory must still update selected_dir to its path"
        );
    }

    #[test]
    fn test_merge_prompt_includes_consolidate_instruction_and_files() {
        let libs = vec![crate::config::ContentLibrary {
            root_folder: "C:/notes".to_string(),
            name: "Notes".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let file1 = PathBuf::from("C:/notes/alpha.md");
        let file2 = PathBuf::from("C:/notes/beta.md");

        let mut selected_files = HashSet::new();
        selected_files.insert(file1.clone());
        selected_files.insert(file2.clone());

        let prompt = build_merge_prompt(&libs, &selected_files);

        assert!(
            prompt.to_lowercase().contains("merge"),
            "prompt should instruct merge: {}",
            prompt
        );
        assert!(
            prompt.to_lowercase().contains("consolidate"),
            "prompt should instruct consolidate: {}",
            prompt
        );
        assert!(prompt.contains("alpha.md"), "prompt should list alpha.md");
        assert!(prompt.contains("beta.md"), "prompt should list beta.md");
    }
}
