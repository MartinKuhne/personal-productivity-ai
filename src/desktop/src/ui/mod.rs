//! UI subsystem — app, panels, rendering, tree, tab/selection/dialog managers, and OS-shell helpers.

pub mod app;
pub mod background_logs;
pub mod batch_dialog;
pub mod editor_egui;
mod modals;
pub mod os_shell;
mod panels;
pub mod render;
pub mod strings;
pub mod table_width;
mod tools_dialog;
#[cfg(test)]
mod tools_dialog_tests;
mod tree;

#[cfg(test)]
pub mod test_helpers;

pub use crate::markdown::ToCEntry;
pub use app::{FastMdApp, TreeNode, generate_format_prompt};
pub use os_shell::{open_in_system_editor, show_in_file_explorer};
pub use render::{build_toc, render_markdown};
pub use tree::{
    FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, TreeOpsContext, draw_tree_node, flatten_tree,
    render_flat_row,
};
