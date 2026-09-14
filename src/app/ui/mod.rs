//! UI subsystem — app, panels, rendering, tree, tab/selection/dialog managers, and OS-shell helpers.

pub mod about_dialog;
pub mod agent;
pub mod agent_debug_window;
pub mod app;
pub mod attributions;
pub mod background_logs;
pub mod batch_dialog;
pub(crate) mod dialogs;
pub mod editor_egui;
pub mod fonts;
pub(crate) mod link_resolver;
pub mod logo;
pub mod modals;
pub mod os_shell;
pub(crate) mod panel_layout;
pub mod panels;
pub(crate) mod persisted;
pub(crate) mod render;
pub(crate) mod selection;
pub mod strings;
pub(crate) mod tab_item;
pub mod table_width;
pub(crate) mod tabs;
pub mod test_helpers;
pub(crate) mod text_buffer;
pub mod tools_dialog;
pub(crate) mod tree;
pub mod tree_search;

// `ToCEntry` is reached through `crate::ui::ToCEntry` only from tests;
// its single canonical public path is `crate::markdown::ToCEntry`.
#[cfg(test)]
pub(crate) use crate::markdown::ToCEntry;
pub(crate) use app::{FastMdApp, TreeNode};
pub use dialogs::{Dialogs, OAuthFlowStatus};
pub use link_resolver::{LinkAction, resolve_link};
pub(crate) use os_shell::{open_in_system_editor, show_in_file_explorer};
pub use panel_layout::PanelLayout;
pub use persisted::{CURRENT_SCHEMA_VERSION, PersistedUiState};
pub use selection::FileSelection;
pub use tab_item::TabItem;
pub use tabs::Tabs;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use tree::{
    FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, TreeOpsContext, draw_tree_node, flatten_tree,
    render_flat_row,
};
pub(crate) use tree_search::TreeSearch;
