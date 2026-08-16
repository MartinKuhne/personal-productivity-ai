//! File-system tree widget — expand/collapse, file selection, multi-select,
//! and context-menu operations (rename, move, delete, new file/dir).
//!
//! # Module layout
//!
//! | Submodule | Contents |
//! |-----------|----------|
//! | [`context`] | [`TreeOpsContext`] flat context struct + [`TreeNodeContext`] alias |
//! | [`flatten`] | [`FlatRow`], [`flatten_tree`], constants |
//! | [`handlers`] | Click handlers: [`apply_file_row_click`], [`apply_directory_row_click`] |
//! | [`render`] | Row drawing: [`render_flat_row`], [`draw_tree_node`] |

pub mod context;
pub mod flatten;
pub(crate) mod handlers;
pub(crate) mod render;

pub use context::{TreeNodeContext, TreeOpsContext};
pub use flatten::{FlatRow, TREE_ROW_HEIGHT, flatten_tree};
pub use render::{draw_tree_node, render_flat_row};
