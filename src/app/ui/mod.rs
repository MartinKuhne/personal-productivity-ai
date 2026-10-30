//! UI subsystem — app state, agent view models, text/tab/selection/dialog
//! managers, theme tokens, and shared helpers.
//!
//! This tree is framework-free: no rendering code lives here. The live,
//! rendering UI is [`crate::ui_iced`], which builds on the view-models,
//! strings, palette tokens and the tab / selection / tree / persisted state
//! re-exported from here.

pub mod agent;
pub mod app;
pub mod attributions;
pub(crate) mod dialogs;
pub mod fonts;
pub(crate) mod link_resolver;
pub(crate) mod panel_layout;
pub(crate) mod persisted;
pub(crate) mod selection;
pub mod strings;
pub(crate) mod tab_item;
pub(crate) mod tabs;
pub(crate) mod text_buffer;
pub mod theme;
pub(crate) mod tree;
pub mod tree_search;

// `FastMdApp` is reached through `crate::ui::FastMdApp` only from tests;
// production code uses `crate::ui::app::FastMdApp` directly.
#[cfg(test)]
pub(crate) use app::FastMdApp;
pub(crate) use app::TreeNode;
pub use dialogs::{Dialogs, OAuthFlowStatus};
pub use link_resolver::{LinkAction, resolve_link};
pub use panel_layout::PanelLayout;
pub use persisted::{CURRENT_SCHEMA_VERSION, PersistedUiState};
pub use selection::FileSelection;
pub use tab_item::TabItem;
pub use tabs::Tabs;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use tree::{FlatRow, TREE_ROW_HEIGHT, flatten_tree};
pub(crate) use tree_search::TreeSearch;
