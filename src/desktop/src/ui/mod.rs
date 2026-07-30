//! UI subsystem — app, panels, rendering, tree, tab/selection/dialog managers, and OS-shell helpers.

pub mod app;
pub mod background_logs;
mod modals;
pub mod os_shell;
mod panels;
pub mod render;
pub mod strings;
pub mod table_width;
mod tree;

#[cfg(test)]
pub mod test_helpers;

pub use crate::app::ToCEntry;
pub use app::{FastMdApp, TreeNode, generate_format_prompt};
pub use os_shell::{open_in_system_editor, show_in_file_explorer};
pub use render::{build_toc, render_markdown, render_yaml_table};
pub use tree::{
    FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, draw_tree_node, flatten_tree, render_flat_row,
};
