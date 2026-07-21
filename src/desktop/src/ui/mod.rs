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
mod tree;

pub use app::{generate_format_prompt, FastMdApp, ToCEntry, TreeNode};
pub use os_shell::{open_in_system_editor, show_in_file_explorer};
pub use render::{build_toc, render_markdown, render_yaml_table};
pub use tree::{draw_tree_node, TreeNodeContext};
mod render_tests;
