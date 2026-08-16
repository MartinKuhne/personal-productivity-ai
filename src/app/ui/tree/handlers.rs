//! Click-handler functions for tree rows: file-click, dir-click, and multi-file merge.
//!
//! Unit tests live in the sibling `handlers_tests.rs` sidecar.

use super::context::TreeNodeContext;
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
pub fn apply_file_row_click(ctx: &mut TreeNodeContext, row: &FlatRow) {
    let modifiers = ctx.modifiers();
    // The fields are owned (no `&'a mut T`). We access them
    // through the `&mut self` accessor methods; the borrow
    // checker accepts the calls because each method takes
    // `&mut self` for the whole struct and the calls don't
    // overlap (we drop each `&mut` borrow before requesting
    // the next).
    if modifiers.shift || modifiers.ctrl || modifiers.command {
        let was_in_selection = ctx.selected_files().contains(&row.path);
        if was_in_selection {
            ctx.selected_files().remove(&row.path);
            if ctx.selected_file().as_ref() == Some(&row.path) {
                *ctx.selected_file() = None;
            }
        } else {
            ctx.selected_files().insert(row.path.clone());
            *ctx.selected_file() = Some(row.path.clone());
        }
    } else {
        ctx.selected_files().clear();
        ctx.selected_files().insert(row.path.clone());
        *ctx.selected_file() = Some(row.path.clone());
        if !ctx.tabs().contains(&row.path) {
            ctx.tabs().push(row.path.clone());
        }
    }
    // Always refresh the current directory context to the file's
    // containing directory. The bottom-panel prompt prefix and
    // the agent session dispatch both read `selected_dir`, so a
    // stale value would mislead both surfaces after a file click.
    // `Path::parent` returns `None` for bare filenames (no parent
    // component), which is the right neutral state for the bottom
    // panel's `>` prefix.
    *ctx.selected_dir() = row.path.parent().map(|p| p.to_path_buf());
}

/// Purpose: Applies the side effect of clicking a directory row in
/// the left panel's tree view.
/// Inputs: ctx (the `TreeNodeContext`; its `expanded_dirs`,
/// `selected_dir`, `selected_file`, and `selected_files` are
/// mutated), row (the clicked `FlatRow`; `row.is_dir` is `true`).
/// Outputs: ()
/// Purity: Impure (mutates the tree's expand/collapse state, the
/// current-directory context, and clears file selection).
/// Preconditions: `row.is_dir` is `true`.
/// Postconditions:
///   * Toggles `ctx.expanded_dirs()` for `row.path` — adds it if
///     the folder was collapsed, removes it if it was already
///     expanded.
///   * Updates `ctx.selected_dir()` to `Some(row.path.clone())`
///     so the bottom-panel prompt prefix and the agent session
///     reflect the folder the user just browsed to.
///   * Clears `ctx.selected_file()` and `ctx.selected_files()` so
///     the file context in the agent prompt clears when the user
///     navigates to a different directory.
pub fn apply_directory_row_click(ctx: &mut TreeNodeContext, row: &FlatRow) {
    if ctx.expanded_dirs().contains(&row.path) {
        ctx.expanded_dirs().remove(&row.path);
    } else {
        ctx.expanded_dirs().insert(row.path.clone());
    }
    *ctx.selected_dir() = Some(row.path.clone());
    // Clear file selection when user navigates to a directory
    *ctx.selected_file() = None;
    ctx.selected_files().clear();
    // Toggling `expanded_dirs` changes which rows are visible in the
    // tree, so the cached `Vec<FlatRow>` in `FastMdApp` (the P0
    // perf-optimization cache) is now stale. Mark it dirty so the
    // next `show_left_panel` pass rebuilds the flat rows. This is
    // independent of `left_panel_dirty` (the panel-width recalc
    // flag): a directory click does NOT need to recompute the panel
    // width, only the flat row cache. See the regression test
    // `test_directory_click_invalidates_tree_cache` in
    // `ui/panels/left.rs` for the user-visible invariant.
    *ctx.tree_dirty() = true;
}

pub fn build_merge_prompt(
    content_libraries: &[crate::config::ContentLibrary],
    selected_files: &HashSet<PathBuf>,
) -> String {
    let mut prompt = "Please read each of the following documents using the read_note tool and merge their content into a new document. Consolidate overlapping content, deduplicate repeated information, and produce a single unified document that combines all of the source material:\n".to_string();
    let mut sorted_files: Vec<&PathBuf> = selected_files.iter().collect();
    sorted_files.sort();
    for file in sorted_files {
        let rel_str = crate::config::library_display_label(content_libraries, file)
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        prompt.push_str(&format!("- {}\n", rel_str));
    }
    prompt
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `handlers_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
