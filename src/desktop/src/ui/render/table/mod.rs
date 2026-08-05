//! Markdown table rendering — two sub-concerns, each its own file.
//!
//! - [`cell::render_table_cell`] paints a single cell at a known
//!   pinned width and height. Returns the cell's actual height so the
//!   row can align to the tallest cell.
//! - [`configured::render_table_with_config`] is the table dispatcher
//!   used in production: it runs FTWA, falls back to the §3.6
//!   horizontal-scroll path when the table physically cannot fit, and
//!   calls `render_table_cell` for each cell.
//!
//! A test-only `render_table` thin wrapper (using the default
//! `TableRenderConfig`) previously lived in `table/dispatch.rs` and
//! required a 3-hop `pub(crate) use` re-export chain to reach the
//! e2e_tests. It has been moved into `e2e_tests/helpers.rs` next to
//! the other test helpers, and the re-export chain deleted.
//!
//! `render_table_with_config` is re-exported here so that
//! `super::render_markdown` can call it via `table::render_table_with_config`
//! without a path into the inner file.

pub mod cell;
pub mod configured;

pub(crate) use configured::render_table_with_config;
