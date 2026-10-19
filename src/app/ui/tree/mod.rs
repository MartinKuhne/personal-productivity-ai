//! File-system tree model — pure flattening of the workspace tree into
//! renderable rows, plus the multi-file merge-prompt builder.
//!
//! The legacy UI row renderer (`render.rs`) and the legacy-modifier-typed
//! operation context (`context.rs`) were removed by [TASK-029b]; the iced
//! left panel ([`crate::ui_iced::panels::left`]) consumes [`FlatRow`]s
//! and emits [`UserCommand`](crate::bus::events::user_command::UserCommand)
//! intents directly.
//!
//! # Module layout
//!
//! | Submodule | Contents |
//! |-----------|----------|
//! | [`flatten`] | [`FlatRow`], [`flatten_tree`], constants |
//! | `handlers` | `build_merge_prompt` — the multi-file merge prompt |

pub(crate) mod flatten;
pub(crate) mod handlers;

pub use flatten::{FlatRow, TREE_ROW_HEIGHT, flatten_tree};
