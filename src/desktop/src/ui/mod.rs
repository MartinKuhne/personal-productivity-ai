//! UI subsystem — app, panels, rendering, tree, tab/selection/dialog managers, and OS-shell helpers.

pub mod app;
pub mod background_logs;
pub mod dialog_manager;
mod modals;
pub mod os_shell;
pub mod panel_layout;
mod panels;
pub mod render;
pub mod selection_manager;
pub mod tab_manager;
mod table_width;
mod tree;

pub use app::{FastMdApp, ToCEntry, TreeNode, generate_format_prompt};
pub use os_shell::{open_in_system_editor, show_in_file_explorer};
pub use render::{build_toc, render_markdown, render_yaml_table};
pub use tree::{
    FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, draw_tree_node, flatten_tree, render_flat_row,
};
mod render_tests;
