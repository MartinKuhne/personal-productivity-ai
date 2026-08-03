//! Markdown table rendering — three sub-concerns, each its own file.
//!
//! - [`cell::render_table_cell`] paints a single cell at a known
//!   pinned width and height. Returns the cell's actual height so the
//!   row can align to the tallest cell.
//! - [`configured::render_table_with_config`] is the table dispatcher
//!   used in production: it runs FTWA, falls back to the §3.6
//!   horizontal-scroll path when the table physically cannot fit, and
//!   calls `render_table_cell` for each cell.
//! - [`dispatch::render_table`] is the `#[cfg(test)]` thin wrapper
//!   around `render_table_with_config` that uses the default
//!   `TableRenderConfig`. Production dispatch goes through
//!   `render_table_with_config` directly.
//!
//! `render_table_with_config` is re-exported here so that
//! `super::render_markdown` can call it via `table::render_table_with_config`
//! without a path into the inner file.

pub mod cell;
pub mod configured;
pub mod dispatch;

pub(crate) use configured::render_table_with_config;
