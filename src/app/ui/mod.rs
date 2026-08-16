//! UI subsystem — app, panels, rendering, tree, tab/selection/dialog managers, and OS-shell helpers.

pub mod agent;
pub mod agent_debug_window;
pub mod app;
pub mod background_logs;
pub mod batch_dialog;
pub mod dialogs;
pub mod editor_egui;
mod modals;
pub mod os_shell;
pub mod panel_layout;
mod panels;
pub mod persisted;
pub mod render;
pub mod selection;
pub mod strings;
pub mod tab_item;
pub mod table_width;
pub mod tabs;
pub mod test_helpers;
pub mod text_buffer;
mod tools_dialog;
mod tree;

pub use crate::markdown::ToCEntry;
pub use app::{FastMdApp, TreeNode, generate_format_prompt};
pub use dialogs::{Dialogs, OAuthFlowStatus};
pub use os_shell::{open_in_system_editor, open_url, show_in_file_explorer};
pub use panel_layout::PanelLayout;
pub use persisted::{CURRENT_SCHEMA_VERSION, PersistedUiState};
pub use render::{build_toc, render_markdown};
pub use selection::FileSelection;
pub use tab_item::TabItem;
pub use tabs::Tabs;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use tree::{
    FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, TreeOpsContext, draw_tree_node, flatten_tree,
    render_flat_row,
};
