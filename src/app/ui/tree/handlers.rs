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
pub fn apply_file_row_click(
    ctx: &mut TreeNodeContext,
    row: &FlatRow,
) -> crate::bus::events::user_command::UserCommand {
    crate::bus::events::user_command::UserCommand::SelectFile {
        path: row.path.clone(),
        multi: ctx.modifiers().command || ctx.modifiers().ctrl || ctx.modifiers().shift,
    }
}

pub fn apply_directory_row_click(
    _ctx: &mut TreeNodeContext,
    row: &FlatRow,
) -> crate::bus::events::user_command::UserCommand {
    crate::bus::events::user_command::UserCommand::SelectDirectory {
        path: row.path.clone(),
        toggle_expand: true,
    }
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
